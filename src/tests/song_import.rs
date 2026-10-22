//! File › Import › Songs end to end (M1-14, M4-07 OpenSong/ChordPro): the
//! real Operator, storage worker and `sela-import` thread, driven through
//! the menu, the simulated native chooser and the staged preview.
use super::*;
use crate::operator::song_import::{Choice, Duplicate, Phase};
use gpui::EntityInputHandler;
use sela::import::{
    Rejected, Warning,
    batch::{FileError, Format},
};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, atomic::AtomicBool, mpsc};

fn batch_dir(dir: &tempfile::TempDir) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/import/batch");
    let target = dir.path().join("songs");
    std::fs::create_dir(&target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
    }
    target
}

fn library_of(dir: &tempfile::TempDir, songs: &[(&str, &[&str])]) -> PathBuf {
    let path = dir.path().join("library.sqlite");
    let mut repository = Repository::open(&path).unwrap();
    for (title, slides) in songs {
        repository
            .save_song(
                None,
                Song {
                    title: (*title).into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants: Vec::new(),
                    sections: slides
                        .iter()
                        .map(|lyrics| Section {
                            id: SectionId::allocate(),
                            label: String::new(),
                            lyrics: (*lyrics).into(),
                            format: Default::default(),
                            background: None,
                        })
                        .collect(),
                    master: None,
                },
            )
            .unwrap();
    }
    path
}

fn open(
    cx: &mut TestAppContext,
    library: PathBuf,
    songs: usize,
) -> (VisualTestContext, Entity<Operator>) {
    let (mut cx, operator, _) = with_library(cx, Some(library), "apply");
    cx.simulate_resize(size(px(1280.), px(860.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == songs);
    (cx, operator)
}

/// File › Import › Song Folder… answered with `folder`.
fn import_folder(cx: &mut VisualTestContext, operator: &Entity<Operator>, folder: PathBuf) {
    click(cx, "bar-file");
    click(cx, "bar-file-import-folder");
    assert_eq!(
        operator.read_with(cx, |o, _| o.song_import.phase),
        Phase::Choosing
    );
    cx.simulate_path_prompt_response(move |options| {
        assert!(options.directories && !options.files && options.multiple);
        Some(vec![folder])
    });
    cx.run_until_parked();
}

fn settle_review(cx: &mut VisualTestContext, operator: &Entity<Operator>) {
    settle(cx, operator, "staged preview", |o| {
        o.song_import.phase == Phase::Review
    });
    cx.run_until_parked();
}

fn row_index(operator: &Entity<Operator>, cx: &VisualTestContext, name: &str) -> usize {
    operator.read_with(cx, |o, _| {
        o.song_import
            .rows
            .iter()
            .position(|r| r.name == name)
            .unwrap_or_else(|| panic!("row {name}"))
    })
}

fn selector(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// Scrolls the preview list with the mouse wheel until `target` lies inside
/// it, then clicks it (the list is a bounded scroll area).
fn click_in_list(cx: &mut VisualTestContext, target: &'static str) {
    for _ in 0..40 {
        let list = cx.debug_bounds("import-list").expect("import-list");
        let bounds = cx
            .debug_bounds(target)
            .unwrap_or_else(|| panic!("{target}"));
        if bounds.top() >= list.top() && bounds.bottom() <= list.bottom() {
            cx.simulate_click(bounds.center(), Default::default());
            return;
        }
        let delta = if bounds.top() < list.top() { 60. } else { -60. };
        cx.simulate_event(gpui::ScrollWheelEvent {
            position: list.center(),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(delta))),
            ..Default::default()
        });
        cx.run_until_parked();
    }
    panic!("{target} never scrolled into the import list");
}

/// Every file of `folder` with its bytes and modification time.
fn snapshot(folder: &Path) -> Vec<(PathBuf, Vec<u8>, std::time::SystemTime)> {
    let mut files: Vec<_> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
            (path.clone(), std::fs::read(&path).unwrap(), modified)
        })
        .collect();
    files.sort();
    files
}

fn titles(o: &Operator) -> Vec<String> {
    let mut titles: Vec<String> = o.catalog.iter().map(|(_, t)| t.clone()).collect();
    titles.sort();
    titles
}

fn type_search(cx: &mut VisualTestContext, operator: &Entity<Operator>, text: &str) {
    let input = operator.read_with(cx, |o, _| o.search_input.clone());
    cx.update(|window, cx| {
        input.update(cx, |field, cx| {
            let end = field.text().encode_utf16().count();
            field.replace_text_in_range(Some(0..end), text, window, cx);
        });
    });
    cx.run_until_parked();
}

fn hits(o: &Operator) -> Option<Vec<String>> {
    o.search
        .hits()
        .map(|hits| hits.iter().map(|(_, t)| t.clone()).collect())
}

#[gpui::test]
fn folder_import_previews_commits_and_finds_accented_words(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = batch_dir(&dir);
    let originals = snapshot(&folder);
    let (mut cx, operator) = open(cx, library_with_songs(&dir), 2);
    import_folder(&mut cx, &operator, folder.clone());
    settle_review(&mut cx, &operator);

    // Every file is listed: three songs with their warnings, two failures.
    operator.read_with(&cx, |o, _| {
        let import = &o.song_import;
        let names: Vec<&str> = import.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Sungai Kehidupan",
                "broken.xml",
                "cancion-del-alba.xml",
                "empty.cho",
                "lantern-hymn.cho"
            ]
        );
        assert_eq!(import.ignored, 1, "notes.txt is counted, not listed");
        let staged = |i: usize| import.rows[i].result.as_ref().unwrap();
        assert_eq!(staged(0).format, Format::OpenSong);
        assert_eq!(staged(2).format, Format::OpenLyrics);
        assert_eq!(staged(4).format, Format::ChordPro);
        for warning in [
            Warning::UnknownElement("tempo".into()),
            Warning::IgnoredAttribute("verse/@lang".into()),
            Warning::IgnoredAttribute("lines/@repeat".into()),
        ] {
            assert!(staged(2).warnings.contains(&warning), "{warning:?}");
        }
        assert!(staged(0).warnings.contains(&Warning::ChordsRemoved));
        assert!(
            staged(4)
                .warnings
                .contains(&Warning::UnknownDirective("key".into()))
        );
        assert!(matches!(
            import.rows[1].result,
            Err(FileError::Rejected(None, Rejected::Malformed(_)))
        ));
        assert_eq!(
            import.rows[3].result.as_ref().err(),
            Some(&FileError::Rejected(
                Some(Format::ChordPro),
                Rejected::NoLyrics
            ))
        );
        assert_eq!(import.duplicates(), 0);
        assert_eq!(import.writes(), 3);
        assert_eq!(o.catalog.len(), 2, "nothing is saved while staged");
    });
    for row in 0..5 {
        assert!(
            cx.debug_bounds(selector(format!("import-row-{row}")))
                .is_some()
        );
    }
    assert!(cx.debug_bounds("import-choice-1").is_none(), "failed file");
    assert!(cx.debug_bounds("import-choice-0-skip").is_some());

    click(&mut cx, "import-confirm");
    settle(&mut cx, &operator, "committed and reloaded", |o| {
        !o.song_import.active() && o.catalog.len() == 5 && !o.catalog_refresh_pending()
    });
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert!(o.dialog.is_none());
        assert_eq!(
            titles(o),
            [
                "Canción del Alba",
                "Lantern Hymn",
                "Quiet Canticle",
                "Signal Hymn",
                "Sungai Kehidupan"
            ]
        );
        assert_eq!(
            o.song_message.clone(),
            Some(("Imported: 3 · Skipped: 0 · Not importable: 2".into(), false))
        );
        let mut selected: Vec<String> = o.selected_songs().into_iter().map(|(_, t)| t).collect();
        selected.sort();
        assert_eq!(
            selected,
            ["Canción del Alba", "Lantern Hymn", "Sungai Kehidupan"]
        );
    });
    // Preview follows the first imported song in its presentation order.
    settle(&mut cx, &operator, "preview of the first import", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.title == "Sungai Kehidupan")
    });
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        let labels: Vec<&str> = o
            .preview
            .as_ref()
            .unwrap()
            .slides
            .iter()
            .map(|s| s.label.as_str())
            .collect();
        assert_eq!(labels, ["Verse 1", "Chorus", "Verse 2", "Chorus"]);
    });
    cx.update(|window, cx| {
        let o = operator.read(cx);
        let index = o.song_index_of(o.selected_song.unwrap()).unwrap();
        assert!(
            o.song_rows
                .get(&index)
                .is_some_and(|h| h.is_focused(window)),
            "keyboard focus moves to the first imported song"
        );
    });
    assert_eq!(
        snapshot(&folder),
        originals,
        "source files are never modified"
    );

    // The commit went through the search index: accented and folded words.
    type_search(&mut cx, &operator, "mañana");
    settle(&mut cx, &operator, "accented hit", |o| {
        hits(o).is_some_and(|h| h == ["Canción del Alba"])
    });
    type_search(&mut cx, &operator, "manana");
    settle(&mut cx, &operator, "folded hit", |o| {
        hits(o).is_some_and(|h| h == ["Canción del Alba"])
    });
    // The repeated [C] tag kept its second section's words.
    type_search(&mut cx, &operator, "sampai");
    settle(&mut cx, &operator, "repeated label text", |o| {
        hits(o).is_some_and(|h| h == ["Sungai Kehidupan"])
    });
    type_search(&mut cx, &operator, "guíanos");
    settle(&mut cx, &operator, "chorus words", |o| {
        hits(o).is_some_and(|h| h == ["Canción del Alba"])
    });
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "imported song preview", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.title == "Canción del Alba" && p.slides.len() == 4)
    });
    click(&mut cx, "preview-slide-1");
    cx.run_until_parked();
    assert_eq!(operator.read_with(&cx, |o, _| o.preview_slide), 1);
    // The audience renderer acknowledges an imported slide.
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "imported chorus on screen", |o| {
        o.on_screen() == Some("Canción del Alba · Chorus")
    });
    click(&mut cx, "live-output");
}

#[gpui::test]
fn file_import_by_keyboard_reports_unsupported_files_and_cancels(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = batch_dir(&dir);
    let (mut cx, operator) = open(cx, library_with_songs(&dir), 2);
    cx.update(|window, cx| {
        operator.read(cx).controls[Control::GoLive]
            .clone()
            .focus(window, cx)
    });
    // File › Import › Songs… is the sixth selectable row after New Song.
    cx.simulate_keystrokes("alt-f down down down down down down");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.bar_menu.and_then(|m| m.highlight)),
        Some(12)
    );
    cx.simulate_keystrokes("enter");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.song_import.phase),
        Phase::Choosing
    );
    let chosen = vec![folder.join("lantern-hymn.cho"), folder.join("notes.txt")];
    cx.simulate_path_prompt_response(move |options| {
        assert!(options.files && !options.directories && options.multiple);
        Some(chosen)
    });
    settle_review(&mut cx, &operator);
    operator.read_with(&cx, |o, _| {
        let rows = &o.song_import.rows;
        assert_eq!(rows.len(), 2);
        assert!(rows[0].result.is_ok());
        // A chosen file of another type is listed, not dropped silently.
        assert_eq!(rows[1].result.as_ref().err(), Some(&FileError::Unsupported));
        assert_eq!(o.dialog, Some(crate::operator::Dialog::ImportSongs));
    });
    // Focus starts on Cancel; Tab wraps inside the dialog to the first
    // choice group, where Right/Left move the choice.
    cx.update(|window, cx| {
        assert!(operator.read(cx).controls[Control::DialogCancel].is_focused(window));
    });
    cx.simulate_keystrokes("tab");
    cx.simulate_keystrokes("right");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.song_import.rows[0].choice),
        Choice::Skip
    );
    assert_eq!(operator.read_with(&cx, |o, _| o.song_import.writes()), 0);
    cx.simulate_keystrokes("left");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.song_import.rows[0].choice),
        Choice::Import
    );
    // Esc cancels the whole import: nothing is saved.
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    operator.update(&mut cx, |o, _| o.reload_songs());
    settle(&mut cx, &operator, "reload", |o| {
        !o.catalog_refresh_pending()
    });
    operator.read_with(&cx, |o, _| {
        assert!(o.dialog.is_none());
        assert_eq!(o.song_import.phase, Phase::Idle);
        assert_eq!(o.catalog.len(), 2);
        assert_eq!(
            o.song_message.clone(),
            Some(("Import cancelled. Nothing was imported.".into(), false))
        );
    });
    cx.update(|window, cx| {
        assert!(operator.read(cx).controls[Control::GoLive].is_focused(window));
    });
}

#[gpui::test]
fn cancel_while_files_are_read_commits_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = batch_dir(&dir);
    let (mut cx, operator) = open(cx, library_with_songs(&dir), 2);
    // Hold the worker before its second file, once.
    let (reached_tx, reached) = mpsc::channel::<usize>();
    let (go, go_rx) = mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let held = AtomicBool::new(false);
    operator.update(&mut cx, |o, _| {
        o.song_import.pace = Some(Arc::new(move |index| {
            let _ = reached_tx.send(index);
            if index == 1 && !held.swap(true, std::sync::atomic::Ordering::AcqRel) {
                let _ = go_rx.lock().unwrap().recv();
            }
        }));
    });
    import_folder(&mut cx, &operator, folder.clone());
    while reached.recv_timeout(Duration::from_secs(10)).unwrap() != 1 {}
    settle(&mut cx, &operator, "first row shown", |o| {
        o.song_import.rows.len() == 1 && o.song_import.phase == Phase::Reading
    });
    assert!(cx.debug_bounds("import-status").is_some());
    click(&mut cx, "import-cancel");
    go.send(()).unwrap();
    settle(&mut cx, &operator, "worker done", |o| {
        !o.song_import.worker_busy()
    });
    operator.update(&mut cx, |o, _| o.reload_songs());
    settle(&mut cx, &operator, "reload", |o| {
        !o.catalog_refresh_pending()
    });
    operator.read_with(&cx, |o, _| {
        assert!(o.dialog.is_none());
        assert_eq!(o.song_import.phase, Phase::Idle);
        assert!(o.song_import.rows.is_empty(), "late results are dropped");
        assert_eq!(o.catalog.len(), 2);
    });
    // The same worker takes the next import.
    import_folder(&mut cx, &operator, folder);
    settle_review(&mut cx, &operator);
    assert_eq!(operator.read_with(&cx, |o, _| o.song_import.rows.len()), 5);
}

#[gpui::test]
fn cancel_before_the_commit_reaches_storage_saves_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = batch_dir(&dir);
    let (mut cx, operator) = open(cx, library_with_songs(&dir), 2);
    import_folder(&mut cx, &operator, folder);
    settle_review(&mut cx, &operator);
    click(&mut cx, "import-confirm");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.song_import.phase, Phase::Committing);
        assert!(o.song_import.commit_queued());
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    operator.update(&mut cx, |o, _| o.reload_songs());
    settle(&mut cx, &operator, "reload", |o| {
        !o.catalog_refresh_pending()
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.song_import.phase, Phase::Idle);
        assert!(o.dialog.is_none());
        assert_eq!(o.catalog.len(), 2);
    });
}

#[gpui::test]
fn a_failing_save_rolls_back_the_batch_and_keeps_the_preview(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = batch_dir(&dir);
    let library = library_with_songs(&dir);
    // Failure injection: the second imported song's index insert fails.
    let db = rusqlite::Connection::open(&library).unwrap();
    db.execute_batch("CREATE TRIGGER fail_import BEFORE INSERT ON search_rows WHEN (SELECT count(*) FROM search_rows) >= 3 BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let (mut cx, operator) = open(cx, library, 2);
    import_folder(&mut cx, &operator, folder);
    settle_review(&mut cx, &operator);
    click(&mut cx, "import-confirm");
    settle(&mut cx, &operator, "commit failure", |o| {
        o.song_import.phase == Phase::Review && o.song_import.error.is_some()
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("import-error").is_some());
    operator.read_with(&cx, |o, _| {
        let error = o.song_import.error.clone().unwrap();
        assert!(error.starts_with("Nothing was imported"), "{error}");
        assert_eq!(o.song_import.rows.len(), 5, "the preview stays for a retry");
    });
    operator.update(&mut cx, |o, _| o.reload_songs());
    settle(&mut cx, &operator, "reload", |o| {
        !o.catalog_refresh_pending()
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| titles(o)),
        ["Quiet Canticle", "Signal Hymn"],
        "the first song of the batch was rolled back too"
    );
    // Without the fault the same preview commits.
    db.execute_batch("DROP TRIGGER fail_import").unwrap();
    click(&mut cx, "import-confirm");
    settle(&mut cx, &operator, "retry committed", |o| {
        !o.song_import.active() && o.catalog.len() == 5 && !o.catalog_refresh_pending()
    });
}

#[gpui::test]
fn duplicate_choices_skip_copy_and_replace(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = batch_dir(&dir);
    // A second file with the same title as the library's "Canción del Alba"
    // but other words.
    std::fs::write(
        folder.join("cancion-2.cho"),
        "{title: Canción del Alba}\nOtra letra distinta\n",
    )
    .unwrap();
    let library = library_of(
        &dir,
        &[
            (
                "Lantern Hymn",
                &[
                    "Light the lantern in the night\nhold it steady, hold it bright",
                    "Shine, shine, the road is long",
                ],
            ),
            ("Canción del Alba", &["Palabras antiguas"]),
        ],
    );
    let (mut cx, operator) = open(cx, library, 2);
    let original = operator.read_with(&cx, |o, _| {
        o.catalog
            .iter()
            .find(|(_, t)| t == "Canción del Alba")
            .unwrap()
            .0
    });
    import_folder(&mut cx, &operator, folder);
    settle_review(&mut cx, &operator);
    let lantern = row_index(&operator, &cx, "lantern-hymn.cho");
    let cancion = row_index(&operator, &cx, "cancion-del-alba.xml");
    let second = row_index(&operator, &cx, "cancion-2.cho");
    let sungai = row_index(&operator, &cx, "Sungai Kehidupan");
    operator.read_with(&cx, |o, _| {
        let rows = &o.song_import.rows;
        // Same words: Skip by default. Same title only: Import as copy.
        assert!(matches!(
            &rows[lantern].duplicate,
            Some(Duplicate::Library(m)) if m.same_lyrics
        ));
        assert_eq!(rows[lantern].choice, Choice::Skip);
        assert!(matches!(
            &rows[cancion].duplicate,
            Some(Duplicate::Library(m)) if !m.same_lyrics && m.version == original
        ));
        assert_eq!(rows[cancion].choice, Choice::Import);
        assert!(matches!(
            &rows[second].duplicate,
            Some(Duplicate::Library(m)) if !m.same_lyrics && m.version == original
        ));
        assert_eq!(rows[sungai].duplicate, None);
        assert_eq!(o.song_import.duplicates(), 3);
        assert_eq!(o.bulk_choice(), None, "mixed defaults");
    });
    assert!(cx.debug_bounds("import-bulk").is_some());
    // Bulk choice, then per-row choices by mouse and keyboard.
    click(&mut cx, "import-bulk-replace");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.bulk_choice()),
        Some(Choice::Replace)
    );
    // Two files may not replace the same song.
    click(&mut cx, "import-confirm");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.song_import.phase, Phase::Review);
        let error = o.song_import.error.clone().unwrap();
        assert!(error.starts_with("Two files would replace"), "{error}");
    });
    click(&mut cx, "import-bulk-skip");
    click_in_list(
        &mut cx,
        selector(format!("import-choice-{cancion}-replace")),
    );
    assert_eq!(
        operator.read_with(&cx, |o, _| o.song_import.rows[cancion].choice),
        Choice::Replace
    );
    click_in_list(&mut cx, selector(format!("import-choice-{lantern}-import")));
    assert_eq!(
        operator.read_with(&cx, |o, _| o.song_import.rows[lantern].choice),
        Choice::Import
    );
    cx.simulate_keystrokes("left");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.song_import.rows[lantern].choice),
        Choice::Skip,
        "Left moves the focused group's choice back"
    );
    assert_eq!(operator.read_with(&cx, |o, _| o.song_import.writes()), 2);
    click(&mut cx, "import-confirm");
    settle(&mut cx, &operator, "committed", |o| {
        !o.song_import.active() && o.catalog.len() == 3 && !o.catalog_refresh_pending()
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            titles(o),
            ["Canción del Alba", "Lantern Hymn", "Sungai Kehidupan"]
        );
        let replaced = o
            .catalog
            .iter()
            .find(|(_, t)| t == "Canción del Alba")
            .unwrap()
            .0;
        assert_eq!(replaced.id, original.id, "Replace keeps the library song");
        assert_eq!(replaced.revision, original.revision + 1);
        assert_eq!(
            o.song_message.clone().unwrap().0,
            "Imported: 2 · Skipped: 2 · Not importable: 2"
        );
    });
    type_search(&mut cx, &operator, "mañana");
    settle(&mut cx, &operator, "replaced words indexed", |o| {
        hits(o).is_some_and(|h| h == ["Canción del Alba"])
    });
    type_search(&mut cx, &operator, "antiguas");
    settle(&mut cx, &operator, "old words gone", |o| {
        hits(o).is_some_and(|h| h.is_empty())
    });
}

#[gpui::test]
fn a_duplicate_reply_for_an_older_import_is_ignored(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = batch_dir(&dir);
    let (mut cx, operator) = open(cx, library_with_songs(&dir), 2);
    import_folder(&mut cx, &operator, folder);
    settle_review(&mut cx, &operator);
    let stale = sela::storage::LibraryMatch {
        version: operator.read_with(&cx, |o, _| o.catalog[0].0),
        title: "Signal Hymn".into(),
        same_lyrics: true,
    };
    operator.update(&mut cx, |o, _| {
        // As if this import were still checking when an earlier, cancelled
        // import's duplicate check answered.
        o.song_import.phase = Phase::Checking;
        let job = o.song_import.job();
        o.song_import_reply(
            job - 1,
            Ok(sela::storage::Reply::Duplicates(vec![
                Some(stale.clone());
                3
            ])),
        );
        assert_eq!(o.song_import.phase, Phase::Checking);
        assert_eq!(o.song_import.duplicates(), 0, "stale reply dropped");
        o.song_import_reply(
            job,
            Ok(sela::storage::Reply::Duplicates(vec![
                Some(stale.clone());
                3
            ])),
        );
        assert_eq!(o.song_import.phase, Phase::Review);
        assert_eq!(o.song_import.duplicates(), 3);
    });
}
