//! Scriptures tab end to end in-process (M4-03/M4-04): the real Operator,
//! the real scripture worker over synthetic bundles in a temporary profile,
//! and the fake audience child for renderer receipts.
use super::*;
use crate::operator::ScriptureTab;
use sela::scripture::{TranslationInfo, VerseId, bundled, bundled::Bundle};

fn v(book: u8, chapter: u16, verse: u16) -> VerseId {
    VerseId::new(
        sela::scripture::Book::from_number(book).unwrap(),
        chapter,
        verse,
    )
    .unwrap()
}

fn info(id: &str, language: &str) -> TranslationInfo {
    TranslationInfo {
        id: id.into(),
        name: format!("Synthetic {id}"),
        language: language.into(),
        rights: "Synthetic test text".into(),
        source: "test".into(),
        redistributable: true,
        bundled: true,
    }
}

/// `SYA` has John 3:15-18, 3:36, 4:1-2, 3 John 1:14-15, a long Psalm 119
/// and no Genesis; `SYB` has Genesis 1:1, John 3:16-18 and 3 John 1:14 only
/// (another versification), and no John 4.
fn bundles() -> Vec<Bundle> {
    let mut a = Vec::new();
    for verse in 1..=24 {
        a.push((
            v(19, 119, verse),
            format!("Psalm line {verse} keeps a steady synthetic cadence for the reader to follow"),
        ));
    }
    a.push((v(43, 3, 15), "Alpha fifteen".into()));
    for verse in 16..=18 {
        a.push((v(43, 3, verse), format!("Alpha verse {verse} light")));
    }
    a.push((v(43, 3, 36), "Alpha end of chapter".into()));
    a.push((v(43, 4, 1), "Alpha next chapter".into()));
    a.push((v(43, 4, 2), "Alpha second".into()));
    for verse in 14..=15 {
        a.push((v(64, 1, verse), format!("Alpha third John {verse}")));
    }
    let mut b = vec![(v(1, 1, 1), "Beta beginning".to_string())];
    for verse in 16..=18 {
        b.push((v(43, 3, verse), format!("Beta verse {verse} lamp")));
    }
    b.push((v(64, 1, 14), "Beta third John 14".into()));
    vec![
        bundled::synthetic("SYA", &info("SYA", "en"), &a),
        bundled::synthetic("SYB", &info("SYB", "en"), &b),
    ]
}

fn tab(o: &Operator) -> &ScriptureTab {
    &o.scripture
}

fn open_tab(
    cx: &mut TestAppContext,
    prepare: impl FnOnce(&std::path::Path),
) -> (VisualTestContext, Entity<Operator>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_song(&dir);
    prepare(dir.path());
    let (mut cx, operator, _) = with_library(cx, Some(library), "apply");
    cx.simulate_resize(size(px(1600.), px(1000.)));
    operator.update(&mut cx, |o, _| o.scripture.bundles = bundles());
    click(&mut cx, "Scriptures");
    settle(&mut cx, &operator, "scripture catalog", |o| {
        !tab(o).catalog_pending && !tab(o).translations.is_empty()
    });
    (cx, operator, dir)
}

/// Types into the reference box with real keystrokes, replacing its text.
fn type_box(cx: &mut VisualTestContext, operator: &Entity<Operator>, text: &str) {
    let input = operator.read_with(cx, |o, _| o.scripture.input.clone());
    cx.update(|window, cx| input.read(cx).focus_handle(cx).focus(window, cx));
    cx.simulate_keystrokes("ctrl-a backspace");
    cx.simulate_input(text);
    cx.run_until_parked();
}

fn selected(o: &Operator) -> Vec<i64> {
    tab(o).selection.iter().map(|id| id.key()).collect()
}

fn note(o: &Operator) -> Option<String> {
    tab(o).note.as_ref().map(|n| n.text.clone())
}

fn preview_title(o: &Operator) -> Option<&str> {
    o.preview.as_ref().map(|p| p.title.as_str())
}

#[gpui::test]
fn typing_a_reference_previews_it_and_go_live_shows_it_with_its_reference(cx: &mut TestAppContext) {
    let (mut cx, operator, _dir) = open_tab(cx, |_| {});
    operator.read_with(&cx, |o, _| {
        assert_eq!(tab(o).current.as_deref(), Some("SYA"));
        assert!(cx_has_rows(o, 0));
    });
    assert!(cx.debug_bounds("translation-SYA").is_some());
    assert!(cx.debug_bounds("translation-SYB").is_some());
    type_box(&mut cx, &operator, "John 3:16-18");
    settle(&mut cx, &operator, "passage", |o| selected(o).len() == 3);
    operator.read_with(&cx, |o, _| {
        assert_eq!(selected(o), [43_003_016, 43_003_017, 43_003_018]);
        // The whole chapter is listed around the reference.
        assert_eq!(tab(o).rows.len(), 5);
        assert_eq!(note(o), None);
        let item = o.preview.as_ref().expect("preview");
        assert_eq!(item.title, "John 3:16-18 (SYA)");
        assert_eq!(item.slides.len(), 1);
        assert_eq!(
            item.slides[0].text,
            "16 Alpha verse 16 light 17 Alpha verse 17\nlight 18 Alpha verse 18 light"
        );
        assert_eq!(
            item.footers.as_ref().unwrap()[0].as_ref(),
            "John 3:16-18\nSynthetic SYA (SYA)"
        );
        assert!(o.live.is_none(), "selection never reaches Live");
    });
    assert!(cx.debug_bounds("verse-1").is_some());

    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "scripture on screen", |o| {
        o.on_screen() == Some("John 3:16-18 (SYA) · John 3:16-18")
    });
    assert!(operator.read_with(&cx, |o, _| submitted(o)) >= 1);
}

fn cx_has_rows(o: &Operator, count: usize) -> bool {
    tab(o).rows.len() == count
}

#[gpui::test]
fn typing_in_the_box_never_triggers_live_shortcuts(cx: &mut TestAppContext) {
    let (mut cx, operator, _dir) = open_tab(cx, |_| {});
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    type_box(&mut cx, &operator, "John 3:16");
    settle(&mut cx, &operator, "passage", |o| selected(o).len() == 1);
    // B, period and PageDown are Black and Go Live outside text fields.
    cx.simulate_keystrokes("b . pagedown ctrl-b");
    cx.run_until_parked();
    operator.read_with(&cx, |o, cx| {
        assert!(!o.masks.any(), "no mask from typing");
        assert!(o.live.is_none(), "PageDown in the box is not Go Live");
        assert_eq!(o.scripture.input.read(cx).text(), "John 3:16b.");
    });
    // From the verse list, PageDown is Go Live again.
    click(&mut cx, "verse-1");
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "verse live", |o| {
        o.on_screen() == Some("John 3:16 (SYA) · John 3:16")
    });
}

#[gpui::test]
fn malformed_or_missing_references_keep_the_previous_state(cx: &mut TestAppContext) {
    let (mut cx, operator, _dir) = open_tab(cx, |_| {});
    type_box(&mut cx, &operator, "John 3:16-18");
    settle(&mut cx, &operator, "passage", |o| selected(o).len() == 3);
    let before = operator.read_with(&cx, |o, _| (selected(o), tab(o).rows.clone()));
    for (typed, expected) in [
        ("John 3:99", "John 3 has 36 verses in SYA"),
        ("John 3:20-22", "Not in SYA: John 3:20-22"),
        ("John 5:1", "John 5 is not in SYA"),
        ("John 22:1", "John has no chapter 22"),
        ("Hezekiah 1:1", "No book is named “Hezekiah”"),
        ("John 3:18-16", "The range ends before it starts"),
        ("John 3:16,18", "Not a reference. Try “John 3:16-18”"),
        ("Genesis 1:1", "Genesis is not in SYA"),
    ] {
        type_box(&mut cx, &operator, typed);
        settle(&mut cx, &operator, typed, |o| note(o).is_some());
        operator.read_with(&cx, |o, _| {
            assert_eq!(note(o).as_deref(), Some(expected), "{typed}");
            assert!(tab(o).note.as_ref().unwrap().error);
            assert_eq!(
                (selected(o), tab(o).rows.clone()),
                before,
                "{typed} keeps state"
            );
            assert_eq!(preview_title(o), Some("John 3:16-18 (SYA)"));
        });
    }
    assert!(cx.debug_bounds("scripture-note").is_some());
    // Indonesian phrasing of the same errors.
    sela::i18n::set_language(sela::i18n::Language::Indonesian);
    type_box(&mut cx, &operator, "Yoh 3:99");
    let indonesian = operator.read_with(&cx, |o, _| note(o));
    type_box(&mut cx, &operator, "Hizkia 1");
    let unknown = operator.read_with(&cx, |o, _| note(o));
    sela::i18n::set_language(sela::i18n::Language::English);
    assert_eq!(
        indonesian.as_deref(),
        Some("Yohanes 3 memiliki 36 ayat dalam SYA")
    );
    assert_eq!(unknown.as_deref(), Some("Tidak ada kitab bernama “Hizkia”"));
    // A book alone is a hint, not an error, and shows suggestions.
    type_box(&mut cx, &operator, "Jo");
    operator.read_with(&cx, |o, _| {
        assert_eq!(note(o).as_deref(), Some("Type a chapter after Joshua"));
        assert!(!tab(o).note.as_ref().unwrap().error);
        assert_eq!(
            tab(o)
                .suggestions
                .iter()
                .map(|b| b.name(sela::i18n::Language::English))
                .collect::<Vec<_>>(),
            ["Joshua", "Job", "Joel", "Jonah", "John"]
        );
    });
    assert!(cx.debug_bounds("scripture-suggestion-4").is_some());
    click(&mut cx, "scripture-suggestion-4");
    cx.simulate_input("4:1");
    settle(&mut cx, &operator, "suggested book", |o| {
        selected(o) == [43_004_001]
    });
    operator.read_with(&cx, |o, cx| {
        assert_eq!(o.scripture.input.read(cx).text(), "John 4:1");
    });
    // Tab accepts the first suggestion; without one it moves focus on.
    type_box(&mut cx, &operator, "Rev");
    cx.simulate_keystrokes("tab");
    operator.read_with(&cx, |o, cx| {
        assert_eq!(o.scripture.input.read(cx).text(), "Revelation ");
    });
    cx.simulate_input("1:1");
    settle(&mut cx, &operator, "missing book note", |o| {
        note(o).as_deref() == Some("Revelation is not in SYA")
    });
}

#[gpui::test]
fn cross_chapter_ranges_and_nonconsecutive_selection(cx: &mut TestAppContext) {
    let (mut cx, operator, _dir) = open_tab(cx, |_| {});
    type_box(&mut cx, &operator, "John 3:36-4:2");
    settle(&mut cx, &operator, "cross-chapter passage", |o| {
        selected(o).len() == 3
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(selected(o), [43_003_036, 43_004_001, 43_004_002]);
        assert_eq!(tab(o).rows.len(), 7, "both chapters are listed");
        assert_eq!(preview_title(o), Some("John 3:36-4:2 (SYA)"));
        assert_eq!(
            o.preview.as_ref().unwrap().slides[0]
                .text
                .replace('\n', " "),
            "36 Alpha end of chapter 4:1 Alpha next chapter 2 Alpha second"
        );
    });
    // Rows: 0 3:15, 1 3:16, 2 3:17, 3 3:18, 4 3:36, 5 4:1, 6 4:2.
    let ctrl = gpui::Modifiers {
        control: true,
        ..Default::default()
    };
    let bounds = cx.debug_bounds("verse-1").unwrap();
    cx.simulate_click(bounds.center(), ctrl);
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            selected(o),
            [43_003_016, 43_003_036, 43_004_001, 43_004_002]
        );
        assert_eq!(preview_title(o), Some("John 3:16, 36-4:2 (SYA)"));
        let slide = &o.preview.as_ref().unwrap().slides[0];
        assert!(slide.text.starts_with("16 Alpha verse 16 light 36 Alpha"));
    });
    // Ctrl-click again removes it; a plain click selects one verse; Shift
    // extends from it (consecutive).
    let bounds = cx.debug_bounds("verse-1").unwrap();
    cx.simulate_click(bounds.center(), ctrl);
    assert_eq!(operator.read_with(&cx, |o, _| selected(o)).len(), 3);
    click(&mut cx, "verse-0");
    let bounds = cx.debug_bounds("verse-2").unwrap();
    cx.simulate_click(
        bounds.center(),
        gpui::Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    operator.read_with(&cx, |o, _| {
        assert_eq!(selected(o), [43_003_015, 43_003_016, 43_003_017]);
        assert_eq!(preview_title(o), Some("John 3:15-17 (SYA)"));
    });
    // Keyboard: the list keeps focus; Down moves, Shift+Down extends.
    cx.simulate_keystrokes("down");
    assert_eq!(operator.read_with(&cx, |o, _| selected(o)), [43_003_018]);
    cx.simulate_keystrokes("shift-down shift-down");
    assert_eq!(
        operator.read_with(&cx, |o, _| selected(o)),
        [43_003_018, 43_003_036, 43_004_001]
    );
    cx.simulate_keystrokes("home");
    assert_eq!(operator.read_with(&cx, |o, _| selected(o)), [43_003_015]);
    cx.simulate_keystrokes("ctrl-a");
    assert_eq!(operator.read_with(&cx, |o, _| selected(o)).len(), 7);
    assert!(cx.debug_bounds("scripture-selection-count").is_some());
    // Double-click sends the selection live.
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    double_click(&mut cx, "verse-4");
    // The first click of the double-click selects that verse alone.
    settle(&mut cx, &operator, "double-click live", |o| {
        o.on_screen() == Some("John 3:36 (SYA) · John 3:36")
    });
}

#[gpui::test]
fn translation_switch_keeps_the_verses_and_reports_mismatched_canon(cx: &mut TestAppContext) {
    let (mut cx, operator, _dir) = open_tab(cx, |_| {});
    type_box(&mut cx, &operator, "3 John 1:14-15");
    settle(&mut cx, &operator, "passage", |o| selected(o).len() == 2);
    click(&mut cx, "translation-SYB");
    settle(&mut cx, &operator, "switch", |o| {
        tab(o).current.as_deref() == Some("SYB")
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(selected(o), [64_001_014]);
        assert_eq!(note(o).as_deref(), Some("Not in SYB: 3 John 1:15"));
        assert!(!tab(o).note.as_ref().unwrap().error);
        assert_eq!(preview_title(o), Some("3 John 1:14 (SYB)"));
        assert_eq!(
            o.preview.as_ref().unwrap().slides[0].text,
            "14 Beta third John 14"
        );
    });
    // SYB lacks John 4: the reference is refused and SYB's state stays.
    type_box(&mut cx, &operator, "John 4:1");
    settle(&mut cx, &operator, "missing chapter", |o| note(o).is_some());
    operator.read_with(&cx, |o, _| {
        assert_eq!(note(o).as_deref(), Some("John 4 is not in SYB"));
        assert_eq!(selected(o), [64_001_014]);
    });
    type_box(&mut cx, &operator, "Gen 1:1");
    settle(&mut cx, &operator, "genesis", |o| {
        selected(o) == [1_001_001]
    });
    // SYA has no Genesis: switching is refused and SYB stays shown.
    click(&mut cx, "translation-SYA");
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert_eq!(tab(o).current.as_deref(), Some("SYB"));
        assert_eq!(note(o).as_deref(), Some("Genesis is not in SYA"));
        assert_eq!(selected(o), [1_001_001]);
        assert_eq!(preview_title(o), Some("Genesis 1:1 (SYB)"));
    });
}

#[gpui::test]
fn keyword_search_starts_at_two_characters_and_handles_empty_results(cx: &mut TestAppContext) {
    let (mut cx, operator, _dir) = open_tab(cx, |_| {});
    click(&mut cx, "scripture-mode-keyword");
    type_box(&mut cx, &operator, "l");
    operator.read_with(&cx, |o, _| {
        assert_eq!(note(o).as_deref(), Some("Type at least 2 characters"));
        assert!(tab(o).rows.is_empty());
    });
    type_box(&mut cx, &operator, "lig");
    settle(&mut cx, &operator, "hits", |o| tab(o).rows.len() == 3);
    operator.read_with(&cx, |o, _| {
        assert!(tab(o).hits);
        assert!(tab(o).selection.is_empty());
        assert_eq!(
            tab(o)
                .rows
                .iter()
                .map(|(id, _)| id.key())
                .collect::<Vec<_>>(),
            [43_003_016, 43_003_017, 43_003_018]
        );
    });
    click(&mut cx, "verse-2");
    assert_eq!(
        operator.read_with(&cx, |o, _| preview_title(o).map(str::to_owned)),
        Some("John 3:18 (SYA)".into())
    );
    // Switching translation re-runs the search there.
    click(&mut cx, "translation-SYB");
    settle(&mut cx, &operator, "no hits in SYB", |o| {
        tab(o).current.as_deref() == Some("SYB") && tab(o).rows.is_empty()
    });
    assert!(cx.debug_bounds("scripture-empty").is_some());
    type_box(&mut cx, &operator, "lamp");
    settle(&mut cx, &operator, "SYB hits", |o| tab(o).rows.len() == 3);
    type_box(&mut cx, &operator, "zzqq");
    settle(&mut cx, &operator, "no results", |o| tab(o).rows.is_empty());
    assert!(cx.debug_bounds("scripture-empty").is_some());
    // An empty box is no query at all.
    type_box(&mut cx, &operator, "");
    operator.read_with(&cx, |o, _| {
        assert_eq!(note(o), None);
        assert!(tab(o).rows.is_empty());
    });
}

#[gpui::test]
fn long_passages_split_into_readable_slides_in_order(cx: &mut TestAppContext) {
    let (mut cx, operator, _dir) = open_tab(cx, |_| {});
    type_box(&mut cx, &operator, "Psalm 119:1-24");
    settle(&mut cx, &operator, "long passage", |o| {
        selected(o).len() == 24
    });
    let slides = operator.read_with(&cx, |o, _| o.preview.as_ref().unwrap().slides.clone());
    assert!(slides.len() >= 8, "{} slides", slides.len());
    assert!(
        slides
            .iter()
            .all(|s| s.text.lines().count() <= sela::scripture::passage::LAYOUT.lines)
    );
    let joined: Vec<String> = slides.iter().map(|s| s.text.replace('\n', " ")).collect();
    let all = joined.join(" ");
    let mut at = 0;
    for verse in 1..=24 {
        let needle = format!("{verse} Psalm line {verse} ");
        let found = all[at..].find(&needle).expect("verses in order") + at;
        at = found + needle.len();
    }
    assert_eq!(slides[0].label, "Psalms 119:1-3");
    // Live steps through the pages with their own reference footers.
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "first page", |o| {
        o.on_screen() == Some("Psalms 119:1-24 (SYA) · Psalms 119:1-3")
    });
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "second page", |o| {
        o.on_screen() == Some("Psalms 119:1-24 (SYA) · Psalms 119:4-6")
    });
}

#[gpui::test]
fn corrupt_translation_files_are_rebuilt_or_listed_unusable(cx: &mut TestAppContext) {
    let (mut cx, operator, _dir) = open_tab(cx, |profile| {
        let folder = profile.join("Scripture");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("SYA.selabible"),
            b"not a database, damaged on disk",
        )
        .unwrap();
        std::fs::write(folder.join("USER.selabible"), b"also damaged").unwrap();
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            tab(o).message.clone(),
            Some(("Rebuilt SYA from the bundled copy".into(), false))
        );
        assert_eq!(tab(o).translations.len(), 3);
        assert_eq!(tab(o).current.as_deref(), Some("SYA"));
    });
    assert!(cx.debug_bounds("scripture-message").is_some());
    click(&mut cx, "translation-USER");
    operator.read_with(&cx, |o, _| {
        assert_eq!(tab(o).current.as_deref(), Some("SYA"));
        assert!(
            note(o).is_some_and(|n| n.starts_with("USER is damaged: damaged")),
            "{:?}",
            note(o)
        );
    });
    // The rebuilt bundle works.
    type_box(&mut cx, &operator, "John 3:16");
    settle(&mut cx, &operator, "passage", |o| {
        selected(o) == [43_003_016]
    });
}

#[gpui::test]
fn importing_a_user_file_adds_a_restricted_translation(cx: &mut TestAppContext) {
    let (mut cx, operator, dir) = open_tab(cx, |_| {});
    let file = dir.path().join("synthetic-zefania.xml");
    std::fs::write(
        &file,
        r#"<XMLBIBLE biblename="Synthetic Zefania"><INFORMATION><identifier>SZEF</identifier>
        <language>IND</language><rights>Synthetic rights</rights></INFORMATION>
        <BIBLEBOOK bnumber="43"><CHAPTER cnumber="3"><VERS vnumber="16">Teks sintetis enam belas</VERS>
        <VERS vnumber="17">Teks sintetis tujuh belas</VERS></CHAPTER></BIBLEBOOK></XMLBIBLE>"#,
    )
    .unwrap();
    operator.update(&mut cx, |o, _| o.start_scripture_import(file.clone()));
    settle(&mut cx, &operator, "import", |o| tab(o).importing.is_none());
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            tab(o).message.clone(),
            Some(("Imported SZEF: 2 verses, 0 notes".into(), false))
        );
    });
    settle(&mut cx, &operator, "catalog refresh", |o| {
        tab(o).translations.len() == 3
    });
    click(&mut cx, "translation-SZEF");
    type_box(&mut cx, &operator, "Yoh 3:16-17");
    settle(&mut cx, &operator, "imported passage", |o| {
        selected(o).len() == 2
    });
    operator.read_with(&cx, |o, _| {
        let entry = tab(o).current_entry().unwrap();
        assert!(!entry.info.redistributable, "imported rights are unknown");
        // An Indonesian text gets Indonesian book names on its slides.
        assert_eq!(preview_title(o), Some("Yohanes 3:16-17 (SZEF)"));
    });
    // A file that is not a Bible is refused with its reason.
    std::fs::write(&file, b"<song/>").unwrap();
    operator.update(&mut cx, |o, _| o.start_scripture_import(file.clone()));
    settle(&mut cx, &operator, "refused import", |o| {
        tab(o).importing.is_none()
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| tab(o).message.clone()),
        Some((
            "Import failed: not an OSIS, USFX or Zefania file".into(),
            true
        ))
    );
}

#[gpui::test]
fn scripture_schedule_items_are_pinned_snapshots_that_survive_save_and_reopen(
    cx: &mut TestAppContext,
) {
    let (mut cx, operator, dir) = open_tab(cx, |_| {});
    // Nothing selected yet: Add to Schedule explains instead of adding.
    click(&mut cx, "scripture-add");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.schedule.entries().len()),
        0
    );
    type_box(&mut cx, &operator, "John 3:16-18");
    settle(&mut cx, &operator, "passage", |o| selected(o).len() == 3);
    click(&mut cx, "scripture-add");
    // Nonconsecutive verses make a second item.
    type_box(&mut cx, &operator, "John 3:36-4:2");
    settle(&mut cx, &operator, "second passage", |o| {
        selected(o).len() == 3
    });
    let ctrl = gpui::Modifiers {
        control: true,
        ..Default::default()
    };
    let bounds = cx.debug_bounds("verse-5").unwrap();
    cx.simulate_click(bounds.center(), ctrl);
    click(&mut cx, "scripture-add");
    click(&mut cx, "Songs");
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    add_song(&mut cx, &operator, "Signal Hymn");
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            entry_titles(o),
            ["John 3:16-18 (SYA)", "John 3:36; 4:2 (SYA)", "Signal Hymn"]
        );
        let first = o.schedule.entries()[0].scripture.as_ref().unwrap();
        assert_eq!(first.translation, "SYA");
        assert_eq!(first.verses.len(), 3);
        assert_eq!(
            tab(o).message.clone(),
            Some(("Added John 3:36; 4:2 (SYA) to the schedule".into(), false))
        );
    });
    // Selecting the item previews its own verses at once, with no library read.
    click(&mut cx, "schedule-item-0");
    operator.read_with(&cx, |o, _| {
        let entry = o.schedule.entries()[0].id;
        assert_eq!(previewed(o), Some((Some(entry), "John 3:16-18 (SYA)")));
        assert!(o.selected_song.is_none());
    });
    // Live from the schedule item, then Page Down to the next item.
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "scripture item live", |o| {
        o.on_screen() == Some("John 3:16-18 (SYA) · John 3:16-18")
    });

    // Save: version 4 with the verses in place between the song items.
    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("ctrl-s");
    let typed = dir.path().join("Scripture service");
    cx.simulate_new_path_selection(move |_| Some(typed));
    settle(&mut cx, &operator, "saved", |o| {
        o.schedule.path().is_some() && !o.schedule.is_dirty()
    });
    let file = dir.path().join("Scripture service.selaschedule");
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(document["version"], 4);
    let kinds: Vec<&str> = document["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["scripture", "scripture", "song"]);
    assert_eq!(document["items"][0]["verses"][1][1], "Alpha verse 17 light");

    // The translation file vanishing does not matter: the snapshot plays.
    std::fs::remove_file(dir.path().join("Scripture").join("SYA.selabible")).unwrap();
    let before: Vec<_> = operator.read_with(&cx, |o, _| {
        o.schedule.entries().iter().map(|e| e.id).collect()
    });
    open_file(&mut cx, &operator, &file);
    settle(&mut cx, &operator, "reopened", |o| {
        o.schedule.entries().len() == 3
            && o.schedule.entries().iter().all(|e| !before.contains(&e.id))
            && !o.schedule.is_dirty()
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            entry_titles(o),
            ["John 3:16-18 (SYA)", "John 3:36; 4:2 (SYA)", "Signal Hymn"]
        );
        assert!(o.schedule.entries()[2].scripture.is_none());
    });
    click(&mut cx, "schedule-item-1");
    operator.read_with(&cx, |o, _| {
        let item = o.preview.as_ref().unwrap();
        assert_eq!(item.title, "John 3:36; 4:2 (SYA)");
        assert_eq!(
            item.slides[0].text.replace('\n', " "),
            "36 Alpha end of chapter 4:2 Alpha second"
        );
    });
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "reopened item live", |o| {
        o.on_screen() == Some("John 3:36; 4:2 (SYA) · John 3:36; 4:2")
    });
}
