//! M1-13 in the real Operator: File › Export Standalone Schedule…, opening
//! a standalone schedule through File › Open Schedule (unpack, open,
//! preflight, Locate…), Profiles › Back Up Profile… / Restore Profile…,
//! refusal of damaged or misplaced bundles, Cancel and the quit guard while
//! a job runs. The bundle worker is a real thread; the native dialogs are
//! GPUI's simulated prompts.
use super::*;
use crate::operator::Dialog;
use sela::{
    background::{Background, ImageRef},
    bundle::{self, ExportOptions, manifest::Kind},
    schedule_file::{Item, ScheduleFile},
    storage::{Id, Version},
};
use sha2::Digest;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

fn result_shown(o: &Operator) -> bool {
    o.dialog == Some(Dialog::Bundle) && o.bundles.view.as_ref().is_some_and(|v| !v.working)
}

fn result_lines(o: &Operator) -> Vec<String> {
    o.bundles
        .view
        .as_ref()
        .map(|v| v.lines.iter().map(|(text, _)| text.clone()).collect())
        .unwrap_or_default()
}

fn has(lines: &[String], text: &str) -> bool {
    lines.iter().any(|line| line.contains(text))
}

fn menu_item(cx: &mut VisualTestContext, keys: &str, item: &'static str) {
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
    click(cx, item);
}

fn png_bytes(rgb: [u8; 4]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(32, 18, image::Rgba(rgb))
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

fn song_with(title: &str, image: ImageRef, font: Option<&str>) -> Song {
    Song {
        title: title.into(),
        authors: String::new(),
        copyright: String::new(),
        license: String::new(),
        administrator: String::new(),
        song_number: String::new(),
        variants: Vec::new(),
        sections: vec![Section {
            id: SectionId::allocate(),
            label: "Verse 1".into(),
            lyrics: format!("{title} original line"),
            format: sela::format::SlideFormat {
                font: font.map(str::to_owned),
                ..Default::default()
            },
            background: None,
        }],
        master: Some(Background::image(image)),
    }
}

/// A standalone schedule made on another machine: "Sunrise.png" travels
/// inside it, "Gone.png" was missing there, a font nobody has is named.
struct Package {
    _dir: tempfile::TempDir,
    path: PathBuf,
    /// The missing image's original bytes, for Locate….
    original: PathBuf,
}

fn package_from_elsewhere() -> Package {
    let dir = tempfile::tempdir().unwrap();
    let profile = dir.path().join("elsewhere").join("sela");
    let images = sela::images::images_dir(&profile);
    std::fs::create_dir_all(&images).unwrap();
    let sunrise = png_bytes([220, 140, 40, 255]);
    std::fs::write(images.join("Sunrise.png"), &sunrise).unwrap();
    let gone = png_bytes([20, 20, 20, 255]);
    let original = dir.path().join("Gone.png");
    std::fs::write(&original, &gone).unwrap();
    let pin = |name: &str, bytes: &[u8]| ImageRef {
        name: name.into(),
        sha256: sha2::Sha256::digest(bytes).into(),
    };
    let item = |n: u8, song: Song| Item {
        source: Version {
            id: Id([n; 16]),
            revision: 1,
        },
        song,
        arrangement: None,
        theme: None,
    };
    let file = ScheduleFile {
        title: "Evening Service".into(),
        items: vec![
            item(
                41,
                song_with(
                    "Morning Song",
                    pin("Sunrise.png", &sunrise),
                    Some("Synthetic Missing Serif"),
                ),
            ),
            item(42, song_with("Faded Hymn", pin("Gone.png", &gone), None)),
        ],
        scripture: Vec::new(),
    };
    let path = dir.path().join("Evening.selapackage");
    bundle::export_schedule(
        &profile,
        &file,
        &path,
        &sela::fonts::Fonts::new(),
        ExportOptions::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    Package {
        _dir: dir,
        path,
        original,
    }
}

fn open_path(cx: &mut VisualTestContext, path: &Path) {
    cx.simulate_keystrokes("ctrl-o");
    assert!(cx.did_prompt_for_paths(), "Open asks for a file");
    let path = path.to_path_buf();
    cx.simulate_path_prompt_response(move |_| Some(vec![path]));
    cx.run_until_parked();
}

#[gpui::test]
fn export_standalone_schedule_writes_a_package_without_saving_the_schedule(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_background_song(&dir);
    let out = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    assert!(
        !operator.read_with(&cx, |o, _| o.bundle_enabled(true)),
        "nothing to export while the schedule is empty"
    );
    add_song(&mut cx, &operator, "Backdrop Hymn");
    assert!(operator.read_with(&cx, |o, _| o.bundle_enabled(true)));
    menu_item(&mut cx, "alt-f", "bar-file-export");
    assert!(cx.did_prompt_for_new_path(), "the native Save dialog");
    let typed = out.path().join("Sunday");
    cx.simulate_new_path_selection(move |_| Some(typed));
    settle(&mut cx, &operator, "export result", result_shown);
    cx.run_until_parked();
    let lines = operator.read_with(&cx, |o, _| result_lines(o));
    assert!(
        has(&lines, "Saved “Sunday.selapackage”: 1 items"),
        "{lines:?}"
    );
    assert!(has(&lines, "Images included: backdrop.png"), "{lines:?}");
    assert!(
        has(&lines, "gone.png"),
        "the missing image is named: {lines:?}"
    );
    assert!(cx.debug_bounds("bundle-result-0").is_some());
    let package = out.path().join("Sunday.selapackage");
    let manifest = bundle::inspect(&package).unwrap();
    assert_eq!(manifest.kind, Kind::Schedule);
    assert!(
        manifest
            .images
            .iter()
            .any(|i| i.name == "backdrop.png" && i.embedded)
    );
    operator.read_with(&cx, |o, _| {
        assert!(o.schedule.path().is_none(), "export is not Save As");
        assert!(o.schedule.is_dirty());
    });
    // OK closes the result.
    cx.simulate_keystrokes("enter");
    assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);
}

#[gpui::test]
fn opening_a_standalone_schedule_unpacks_it_and_preflight_explains_what_is_missing(
    cx: &mut TestAppContext,
) {
    let package = package_from_elsewhere();
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_song(&dir);
    let (mut cx, operator, _) = with_library(cx, Some(library), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    open_path(&mut cx, &package.path);
    let unpacked = dir
        .path()
        .join("Schedules")
        .join("Evening Service.selaschedule");
    settle(&mut cx, &operator, "opened", |o| {
        o.schedule.path() == Some(unpacked.as_path())
    });
    settle(&mut cx, &operator, "preflight report", |o| {
        o.dialog == Some(Dialog::Preflight)
            && o.assets
                .report
                .as_ref()
                .is_some_and(|r| !r.items.is_empty())
    });
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert_eq!(entry_titles(o), ["Morning Song", "Faded Hymn"]);
        assert!(!o.schedule.is_dirty(), "opened from its new file");
        let notes: Vec<String> = o
            .package_notes()
            .unwrap()
            .iter()
            .map(|(t, _)| t.clone())
            .collect();
        assert!(
            has(&notes, "Images added to this profile: Sunrise.png"),
            "{notes:?}"
        );
        assert!(has(&notes, "does not carry"), "{notes:?}");
        assert!(has(&notes, "Synthetic Missing Serif"), "{notes:?}");
    });
    assert!(cx.debug_bounds("preflight-package").is_some());
    assert!(
        sela::images::images_dir(dir.path())
            .join("Sunrise.png")
            .is_file()
    );
    // The image the package did not carry is repaired with Locate….
    assert!(cx.debug_bounds("locate-0").is_some());
    click(&mut cx, "locate-0");
    assert!(cx.did_prompt_for_paths());
    let original = package.original.clone();
    cx.simulate_path_prompt_response(move |_| Some(vec![original]));
    settle(&mut cx, &operator, "relinked", |o| {
        o.assets.note.as_ref().is_some_and(|(_, failed)| !*failed)
            && o.assets
                .report
                .as_ref()
                .is_some_and(|r| r.items.iter().all(|item| item.images.is_empty()))
    });
    assert!(
        sela::images::images_dir(dir.path())
            .join("Gone.png")
            .is_file()
    );
}

#[gpui::test]
fn damaged_or_misplaced_bundles_change_nothing_and_say_why(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_song(&dir);
    let elsewhere = tempfile::tempdir().unwrap();
    let damaged = elsewhere.path().join("Damaged.selapackage");
    std::fs::write(&damaged, b"this is not a ZIP archive at all").unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library.clone()), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    add_song(&mut cx, &operator, "Signal Hymn");
    cx.simulate_keystrokes("ctrl-s");
    let saved = dir.path().join("Mine.selaschedule");
    let typed = dir.path().join("Mine");
    cx.simulate_new_path_selection(move |_| Some(typed));
    settle(&mut cx, &operator, "saved", |o| !o.schedule.is_dirty());
    open_path(&mut cx, &damaged);
    settle(&mut cx, &operator, "refusal", result_shown);
    let lines = operator.read_with(&cx, |o, _| result_lines(o));
    assert!(
        has(
            &lines,
            "“Damaged.selapackage” is not a Sela standalone schedule"
        ),
        "{lines:?}"
    );
    assert!(has(&lines, "Nothing was changed."), "{lines:?}");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.schedule.path(), Some(saved.as_path()));
        assert_eq!(entry_titles(o), ["Signal Hymn"]);
    });
    assert!(!dir.path().join("Schedules").exists());
    cx.simulate_keystrokes("enter");

    // A profile backup picked in Open Schedule points to Restore.
    let backup = elsewhere.path().join("Church.selabackup");
    bundle::backup_profile(dir.path(), &backup, &AtomicBool::new(false)).unwrap();
    open_path(&mut cx, &backup);
    settle(&mut cx, &operator, "wrong kind", result_shown);
    let lines = operator.read_with(&cx, |o, _| result_lines(o));
    assert!(
        has(
            &lines,
            "is a profile backup; use Profiles › Restore Profile..."
        ),
        "{lines:?}"
    );
    assert!(!dir.path().join("Schedules").exists());
}

#[gpui::test]
fn backup_and_restore_from_the_profiles_menu_never_touch_the_open_profile(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_background_song(&dir);
    let other = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library.clone()), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    menu_item(&mut cx, "alt-p", "bar-profiles-backup");
    assert!(cx.did_prompt_for_new_path());
    let typed = other.path().join("Church");
    cx.simulate_new_path_selection(move |_| Some(typed));
    settle(&mut cx, &operator, "backup result", result_shown);
    let lines = operator.read_with(&cx, |o, _| result_lines(o));
    assert!(has(&lines, "Backed up to “Church.selabackup”"), "{lines:?}");
    assert!(
        has(&lines, "are not included; Sela recreates them"),
        "{lines:?}"
    );
    let backup = other.path().join("Church.selabackup");
    let manifest = bundle::inspect(&backup).unwrap();
    assert_eq!(manifest.kind, Kind::Profile);
    assert!(
        manifest
            .files
            .iter()
            .any(|f| f.path == "Resources/Images/backdrop.png")
    );
    cx.simulate_keystrokes("enter");

    menu_item(&mut cx, "alt-p", "bar-profiles-restore");
    assert!(cx.did_prompt_for_paths(), "first the backup file");
    let picked = backup.clone();
    cx.simulate_path_prompt_response(move |_| Some(vec![picked]));
    cx.run_until_parked();
    assert!(cx.did_prompt_for_new_path(), "then the new profile folder");
    let destination = other.path().join("Church restored");
    let chosen = destination.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    settle(&mut cx, &operator, "restore result", result_shown);
    let lines = operator.read_with(&cx, |o, _| result_lines(o));
    assert!(
        has(&lines, "Restored as the new profile folder"),
        "{lines:?}"
    );
    assert!(has(&lines, "--operator-library"), "{lines:?}");
    let restored = Repository::open(&bundle::library_in(&destination)).unwrap();
    assert_eq!(restored.catalog(None).unwrap().len(), 1);
    drop(restored);
    assert!(
        sela::images::images_dir(&destination)
            .join("backdrop.png")
            .is_file()
    );
    cx.simulate_keystrokes("enter");

    // Restoring again onto the same folder is refused; it stays as it was.
    let before = std::fs::read(bundle::library_in(&destination)).unwrap();
    menu_item(&mut cx, "alt-p", "bar-profiles-restore");
    let picked = backup.clone();
    cx.simulate_path_prompt_response(move |_| Some(vec![picked]));
    cx.run_until_parked();
    let chosen = destination.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    settle(&mut cx, &operator, "refused", result_shown);
    let lines = operator.read_with(&cx, |o, _| result_lines(o));
    assert!(has(&lines, "already exists"), "{lines:?}");
    assert!(has(&lines, "Nothing was changed."), "{lines:?}");
    assert_eq!(
        std::fs::read(bundle::library_in(&destination)).unwrap(),
        before
    );
    // The open profile still has its library and its one song.
    assert_eq!(operator.read_with(&cx, |o, _| o.catalog.len()), 1);
    assert!(library.is_file());
}

#[gpui::test]
fn a_running_job_can_be_cancelled_and_holds_back_quit(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_song(&dir);
    let other = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    let profile = dir.path().to_path_buf();
    let hold = operator.update(&mut cx, |o, _| o.bundles.hold(&profile));
    menu_item(&mut cx, "alt-p", "bar-profiles-backup");
    let typed = other.path().join("Held");
    cx.simulate_new_path_selection(move |_| Some(typed));
    settle(&mut cx, &operator, "working dialog", |o| {
        o.dialog == Some(Dialog::Bundle)
            && o.bundles.running()
            && o.bundles.view.as_ref().is_some_and(|v| v.working)
    });
    cx.run_until_parked();
    assert!(
        !operator.read_with(&cx, |o, _| o.bundle_enabled(false)),
        "one job at a time"
    );
    // Quit waits for the job (or its Cancel).
    let closes = cx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx)));
    assert!(!closes);
    assert_eq!(
        operator.read_with(&cx, |o, _| o.schedule_message.clone()),
        Some(sela::t!(sela::i18n::Msg::BundleBusy).to_string())
    );
    cx.simulate_keystrokes("escape");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.dialog, None);
        assert_eq!(o.schedule_message.as_deref(), Some("Cancelling…"));
    });
    hold.store(false, Ordering::Release);
    settle(&mut cx, &operator, "cancelled", |o| !o.bundles.running());
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            o.schedule_message.as_deref(),
            Some("Cancelled; nothing was changed")
        );
        assert_eq!(o.dialog, None);
    });
    assert!(!other.path().join("Held.selabackup").exists());
    assert_eq!(
        std::fs::read_dir(other.path()).unwrap().count(),
        0,
        "no temporary file or staging folder"
    );
}
