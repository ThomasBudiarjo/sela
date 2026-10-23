//! File › Import › EasyWorship Library… end to end (M4-07 EasyWorship 6-8
//! child): the real Operator, storage worker and `sela-import` thread with a
//! synthetic `Songs.db` + `SongWords.db` built in test code, driven through
//! the menu, the simulated native folder chooser and the staged preview.
use super::song_import::settle_review;
use super::*;
use crate::operator::song_import::{Choice, Duplicate, Phase};
use sela::import::{
    Warning,
    batch::{FileError, Format},
    ew_library::LibraryError,
};
use std::path::PathBuf;

#[path = "../import/ew_fixture.rs"]
mod ew_fixture;
use ew_fixture::{FixtureSong, ew8_words, hashes, write_library};

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// Five song rows (original text): EW8 and legacy words, a `\uN` escape, a
/// song without a `word` row and a second row with the first one's
/// `song_uid`.
fn ew_library(root: &std::path::Path) -> PathBuf {
    let harbor = leak(ew8_words(&[
        (
            "Verse 1",
            &["Light on the water\nwakes the town", "Boats lean home"],
        ),
        ("Chorus", &["Sing, harbor, sing"]),
    ]));
    let anchor = "{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\fcharset0 Arial;}}\\f0 Verse 1\\par Hold fast the line\\par\\par Chorus\\par Grace holds the rope\\par}";
    let kasih = leak(ew8_words(&[(
        "Verse 1",
        &["Kasih-Mu \\u8212? setia\\line selamanya"],
    )]));
    write_library(
        root,
        &[
            FixtureSong {
                title: "Harbor Morning",
                author: "Rina Kartika",
                copyright: "2026 Sela Test Fixtures",
                administrator: "Fixture Music Admin",
                vendor_id: "7654321",
                description: "Opening song",
                tags: "",
                song_uid: "{H}",
                words: Some(harbor),
            },
            FixtureSong {
                title: "anchor of grace",
                song_uid: "{G}",
                vendor_id: "CCLI-12",
                words: Some(anchor),
                ..FixtureSong::default()
            },
            FixtureSong {
                title: "Kasih Setia",
                song_uid: "{K}",
                words: Some(kasih),
                ..FixtureSong::default()
            },
            FixtureSong {
                title: "Lost Words",
                song_uid: "{L}",
                words: None,
                ..FixtureSong::default()
            },
            FixtureSong {
                title: "Harbor Morning (copy)",
                song_uid: "{H}",
                words: Some(harbor),
                ..FixtureSong::default()
            },
        ],
    )
}

fn open(cx: &mut TestAppContext, library: PathBuf) -> (VisualTestContext, Entity<Operator>) {
    let (mut cx, operator, _) = with_library(cx, Some(library), "apply");
    cx.simulate_resize(size(px(1280.), px(860.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    (cx, operator)
}

/// File › Import › EasyWorship Library… answered with `folder`.
fn import_library(cx: &mut VisualTestContext, operator: &Entity<Operator>, folder: PathBuf) {
    click(cx, "bar-file");
    click(cx, "bar-file-import-easyworship");
    assert_eq!(
        operator.read_with(cx, |o, _| o.song_import.phase),
        Phase::Choosing
    );
    cx.simulate_path_prompt_response(move |options| {
        assert!(options.directories && !options.files && !options.multiple);
        Some(vec![folder])
    });
    cx.run_until_parked();
}

#[gpui::test]
fn an_easyworship_library_is_previewed_committed_and_reimported_as_duplicates(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let ew_root = dir.path().join("EasyWorship").join("Default");
    let data = ew_library(&ew_root.join("v6.1"));
    let before = hashes(&ew_root);
    let library = library_with_songs(&dir);
    let (mut cx, operator) = open(cx, library.clone());
    // The profile folder is chosen; the data folder is found below it.
    import_library(&mut cx, &operator, ew_root.clone());
    settle_review(&mut cx, &operator);

    operator.read_with(&cx, |o, _| {
        let import = &o.song_import;
        assert!(import.library);
        let names: Vec<&str> = import.rows.iter().map(|r| r.name.as_str()).collect();
        // Title order, case ignored (UTF8_U_CI), then rowid.
        assert_eq!(
            names,
            [
                "Songs.db #2",
                "Songs.db #1",
                "Songs.db #5",
                "Songs.db #3",
                "Songs.db #4"
            ]
        );
        for row in &import.rows {
            assert_eq!(row.path, data.join("Songs.db"));
        }
        let staged = |i: usize| import.rows[i].result.as_ref().unwrap();
        assert_eq!(staged(0).format, Format::EasyWorship);
        assert_eq!(staged(0).song.title, "anchor of grace");
        assert!(
            staged(0)
                .warnings
                .contains(&Warning::InvalidField("vendor_id".into()))
        );
        assert_eq!(staged(1).song.title, "Harbor Morning");
        assert_eq!(
            staged(1).warnings,
            [Warning::IgnoredAttribute("song/description".into())]
        );
        assert_eq!(
            staged(3).song.sections[0].lyrics,
            "Kasih-Mu — setia\nselamanya"
        );
        assert_eq!(
            import.rows[4].result.as_ref().err(),
            Some(&FileError::MissingWords)
        );
        // The second row with song_uid {H} points at the first.
        assert_eq!(import.rows[2].duplicate, Some(Duplicate::Batch(1)));
        assert_eq!(import.rows[2].choice, Choice::Skip);
        assert_eq!(import.duplicates(), 1);
        assert_eq!(import.writes(), 3);
        assert_eq!(o.catalog.len(), 2, "nothing is saved while staged");
    });
    assert!(
        cx.debug_bounds("import-confirm").is_some(),
        "the dialog is shown"
    );
    assert!(cx.debug_bounds("import-choice-4").is_none(), "failed row");

    click(&mut cx, "import-confirm");
    settle(&mut cx, &operator, "committed and reloaded", |o| {
        !o.song_import.active() && o.catalog.len() == 5 && !o.catalog_refresh_pending()
    });
    cx.run_until_parked();
    let imported = operator.read_with(&cx, |o, _| {
        assert_eq!(
            o.song_message.clone(),
            Some(("Imported: 3 · Skipped: 1 · Not importable: 1".into(), false))
        );
        o.song_import.imported.clone()
    });
    let repository = Repository::open(&library).unwrap();
    let harbor = imported
        .iter()
        .map(|v| repository.song(*v).unwrap())
        .find(|s| s.title == "Harbor Morning")
        .expect("Harbor Morning saved");
    assert_eq!(harbor.authors, "Rina Kartika");
    assert_eq!(harbor.copyright, "2026 Sela Test Fixtures");
    assert_eq!(harbor.administrator, "Fixture Music Admin");
    assert_eq!(harbor.song_number, "7654321");
    let slides: Vec<(&str, &str)> = harbor
        .sections
        .iter()
        .map(|s| (s.label.as_str(), s.lyrics.as_str()))
        .collect();
    assert_eq!(
        slides,
        [
            ("Verse 1", "Light on the water\nwakes the town"),
            ("", "Boats lean home"),
            ("Chorus", "Sing, harbor, sing"),
        ]
    );
    drop(repository);

    // Re-import: every song is a library duplicate with Skip chosen.
    import_library(&mut cx, &operator, ew_root.clone());
    settle_review(&mut cx, &operator);
    operator.read_with(&cx, |o, _| {
        let import = &o.song_import;
        for row in [0, 1, 3] {
            assert!(
                matches!(&import.rows[row].duplicate, Some(Duplicate::Library(m)) if m.same_lyrics),
                "row {row}"
            );
            assert_eq!(import.rows[row].choice, Choice::Skip);
        }
        assert_eq!(import.rows[2].duplicate, Some(Duplicate::Batch(1)));
        assert_eq!(import.writes(), 0);
    });
    click(&mut cx, "import-cancel");
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert!(!o.song_import.active());
        assert_eq!(o.catalog.len(), 5);
    });
    assert_eq!(
        hashes(&ew_root),
        before,
        "the EasyWorship files are never written"
    );
}

#[gpui::test]
fn a_folder_without_a_library_is_one_failed_row(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("Not EasyWorship");
    std::fs::create_dir(&empty).unwrap();
    std::fs::write(empty.join("notes.txt"), "not a library").unwrap();
    let (mut cx, operator) = open(cx, library_with_songs(&dir));
    import_library(&mut cx, &operator, empty.clone());
    settle_review(&mut cx, &operator);
    operator.read_with(&cx, |o, _| {
        let import = &o.song_import;
        assert_eq!(import.rows.len(), 1);
        assert_eq!(import.rows[0].name, "Not EasyWorship");
        assert_eq!(
            import.rows[0].result.as_ref().err(),
            Some(&FileError::Library(LibraryError::NoSongsDb))
        );
        assert_eq!(import.writes(), 0);
    });
    assert!(cx.debug_bounds("import-row-0").is_some());
    click(&mut cx, "import-cancel");
    cx.run_until_parked();
    assert_eq!(operator.read_with(&cx, |o, _| o.catalog.len()), 2);
}
