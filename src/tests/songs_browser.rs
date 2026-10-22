//! songs-browser (M1-04, M1-07 label search, M1-05 copyright options):
//! Sort by Author/Copyright/Administrator/Song Number (EW8-OBS-064), slide
//! label search (EW8-OBS-045/046), a late reply for an earlier query, delete
//! of the selected search hit, keyboard walks through virtualized rows and the
//! Options › Song Copyright Info group (EW8-OBS-055/056).
use super::*;
use crate::operator::Dialog;
use gpui::EntityInputHandler;
use sela::{settings, song_order::SongSort};

/// One original fixture song: title, authors, copyright, administrator,
/// song number and its section labels.
type Fixture<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str, &'a [&'a str]);

const KEYED: [Fixture<'static>; 4] = [
    (
        "Delta Hymn",
        "bravo writer",
        "© 2001 Zeta",
        "Northwind",
        "900",
        &["Verse 1"],
    ),
    ("Alpha Hymn", "", "", "", "", &["Verse 1"]),
    (
        "Echo Hymn",
        "Alpha Writer",
        "(c) 1999 Omega",
        "acme",
        "10",
        &["Verse 1", "Etsaer", "Verse 2"],
    ),
    (
        "Bravo Hymn",
        "Charlie Writer",
        "© 2001 Zeta",
        "",
        "0042",
        &["Verse 1"],
    ),
];

fn library(dir: &tempfile::TempDir, songs: &[Fixture<'_>]) -> std::path::PathBuf {
    let path = dir.path().join("library.sqlite");
    let mut repository = Repository::open(&path).unwrap();
    for (title, authors, copyright, administrator, number, labels) in songs {
        repository
            .save_song(
                None,
                Song {
                    title: (*title).into(),
                    authors: (*authors).into(),
                    copyright: (*copyright).into(),
                    license: String::new(),
                    administrator: (*administrator).into(),
                    song_number: (*number).into(),
                    variants: Vec::new(),
                    sections: labels
                        .iter()
                        .map(|label| Section {
                            id: SectionId::allocate(),
                            label: (*label).into(),
                            lyrics: format!("{title} original line"),
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
    path: std::path::PathBuf,
    count: usize,
) -> (VisualTestContext, Entity<Operator>) {
    let (mut cx, operator, _) = with_library(cx, Some(path), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == count);
    cx.run_until_parked();
    (cx, operator)
}

fn titles(cx: &VisualTestContext, operator: &Entity<Operator>) -> Vec<String> {
    operator.read_with(cx, |o, _| {
        o.songs().iter().map(|(_, t)| t.clone()).collect()
    })
}

fn settings_file(dir: &tempfile::TempDir) -> String {
    std::fs::read_to_string(settings::path_in(dir.path())).unwrap_or_default()
}

fn type_search(cx: &mut VisualTestContext, operator: &Entity<Operator>, text: &str) {
    let input = operator.read_with(cx, |o, _| o.search_input.clone());
    cx.update(|window, cx| {
        input.update(cx, |field, cx| {
            let end = field.text().len();
            field.replace_text_in_range(Some(0..end), text, window, cx);
        });
    });
}

fn choose_sort(cx: &mut VisualTestContext, id: &'static str) {
    right_click(cx, "song-0");
    click(cx, "song-menu-sort");
    click(cx, id);
}

#[gpui::test]
fn sort_by_each_observed_key_reorders_the_list_and_persists(app: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = library(&dir, &KEYED);
    let (mut cx, operator) = open(app, path.clone(), 4);
    assert_eq!(
        titles(&cx, &operator),
        ["Alpha Hymn", "Bravo Hymn", "Delta Hymn", "Echo Hymn"]
    );
    for (id, sort, want) in [
        (
            "song-sort-author",
            SongSort::Author,
            ["Echo Hymn", "Delta Hymn", "Bravo Hymn", "Alpha Hymn"],
        ),
        (
            "song-sort-copyright",
            SongSort::Copyright,
            ["Echo Hymn", "Bravo Hymn", "Delta Hymn", "Alpha Hymn"],
        ),
        (
            "song-sort-administrator",
            SongSort::Administrator,
            ["Echo Hymn", "Delta Hymn", "Alpha Hymn", "Bravo Hymn"],
        ),
        (
            "song-sort-song-number",
            SongSort::SongNumber,
            ["Echo Hymn", "Bravo Hymn", "Delta Hymn", "Alpha Hymn"],
        ),
    ] {
        choose_sort(&mut cx, id);
        settle(&mut cx, &operator, id, |o| {
            o.song_order.sort == sort && !o.catalog_refresh_pending()
        });
        assert_eq!(titles(&cx, &operator), want, "{id}");
        assert!(
            cx.debug_bounds("song-menu-sort").is_none(),
            "a choice closes the menu"
        );
    }
    // Descending keeps songs without a number last.
    choose_sort(&mut cx, "song-sort-descending");
    settle(&mut cx, &operator, "descending", |o| {
        o.song_order.descending && !o.catalog_refresh_pending()
    });
    assert_eq!(
        titles(&cx, &operator),
        ["Delta Hymn", "Bravo Hymn", "Echo Hymn", "Alpha Hymn"]
    );
    settle(&mut cx, &operator, "settings written", |_| {
        settings_file(&dir).contains("song_sort=song_number\nsong_sort_descending=1\n")
    });
    drop(cx);

    // A restart with the same profile lists by the stored key again.
    let (mut cx, operator) = open(app, path, 4);
    settle(&mut cx, &operator, "stored order", |o| {
        o.song_order.sort == SongSort::SongNumber
            && o.song_order.descending
            && !o.catalog_refresh_pending()
    });
    assert_eq!(
        titles(&cx, &operator),
        ["Delta Hymn", "Bravo Hymn", "Echo Hymn", "Alpha Hymn"]
    );
}

#[gpui::test]
fn a_slide_label_query_finds_the_song_below_title_matches(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let songs: [Fixture<'_>; 3] = [
        ("Morning Song", "", "", "", "", &["Verse 1", "Etsaer"]),
        ("Etsaer Praise", "", "", "", "", &["Verse 1"]),
        ("Evening Song", "", "", "", "", &["Verse 1", "Chorus"]),
    ];
    let (mut cx, operator) = open(cx, library(&dir, &songs), 3);
    type_search(&mut cx, &operator, "ets");
    settle(&mut cx, &operator, "label hits", |o| {
        o.search.hits().is_some_and(|h| h.len() == 2)
    });
    cx.run_until_parked();
    assert_eq!(
        titles(&cx, &operator),
        ["Etsaer Praise", "Morning Song"],
        "the title match ranks above the label-only match"
    );
    // A label-only hit previews like any other row.
    click(&mut cx, "song-1");
    settle(&mut cx, &operator, "preview", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.title == "Morning Song" && p.slides.len() == 2)
    });
    type_search(&mut cx, &operator, "chorus");
    settle(&mut cx, &operator, "chorus label", |o| {
        o.search
            .hits()
            .is_some_and(|h| h.len() == 1 && h[0].1 == "Evening Song")
    });
    // Nothing typed reaches Live.
    operator.read_with(&cx, |o, _| assert!(o.live.is_none()));
}

#[gpui::test]
fn a_late_reply_for_an_earlier_query_never_replaces_newer_results(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, library(&dir, &KEYED), 4);
    // "al" is sent to the storage worker at once; "br" is typed before the
    // poll timer runs again, so the reply for "al" is always taken after
    // the newer query exists, whether the worker answered already or not.
    type_search(&mut cx, &operator, "al");
    assert!(operator.read_with(&cx, |o, _| o.search.submitted() == 1));
    type_search(&mut cx, &operator, "br");
    settle(&mut cx, &operator, "newest results", |o| {
        o.search
            .hits()
            .is_some_and(|h| h.iter().any(|(_, t)| t == "Bravo Hymn"))
            && !o.search.pending()
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.search.submitted(), 2);
        assert_eq!(o.search.stale(), 1, "the reply for \"al\" was dropped");
    });
    assert_eq!(titles(&cx, &operator), ["Bravo Hymn", "Delta Hymn"]);
    click(&mut cx, "song-0");
    let selected = operator.read_with(&cx, |o, _| o.selected_song);
    assert!(selected.is_some());

    // An even later answer for an older generation: still not shown, and the
    // selection it would have dropped stays.
    let alpha = operator.read_with(&cx, |o, _| {
        o.catalog.iter().find(|(_, t)| t == "Alpha Hymn").unwrap().0
    });
    let changed = operator.update(&mut cx, |o, _| {
        o.search_results(sela::search::Results {
            generation: 1,
            hits: vec![sela::search::Hit {
                version: alpha,
                title: "Alpha Hymn".into(),
            }],
            truncated: false,
        })
    });
    cx.run_until_parked();
    assert!(!changed);
    assert_eq!(titles(&cx, &operator), ["Bravo Hymn", "Delta Hymn"]);
    assert_eq!(operator.read_with(&cx, |o, _| o.selected_song), selected);
    assert_eq!(operator.read_with(&cx, |o, _| o.search.stale()), 2);
}

#[gpui::test]
fn deleting_the_selected_search_hit_selects_its_neighbour_hit(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut songs: Vec<Fixture<'_>> = KEYED.to_vec();
    songs.push(("Zulu Psalm", "", "", "", "", &["Verse 1"]));
    let (mut cx, operator) = open(cx, library(&dir, &songs), 5);
    type_search(&mut cx, &operator, "hymn");
    settle(&mut cx, &operator, "hymn hits", |o| {
        o.search.hits().is_some_and(|h| h.len() == 4)
    });
    cx.run_until_parked();
    let before = titles(&cx, &operator);
    click(&mut cx, "song-1");
    let doomed = before[1].clone();
    cx.simulate_keystrokes("delete");
    operator.read_with(&cx, |o, _| {
        assert!(matches!(o.dialog, Some(Dialog::DeleteSong(_))));
    });
    click(&mut cx, "dialog-confirm");
    settle(&mut cx, &operator, "deleted", |o| {
        o.catalog.len() == 4
            && !o.catalog_refresh_pending()
            && !o.search.pending()
            && o.search.hits().is_some_and(|h| h.len() == 3)
    });
    cx.run_until_parked();
    let after = titles(&cx, &operator);
    assert!(!after.contains(&doomed));
    assert!(!after.contains(&"Zulu Psalm".to_string()), "still filtered");
    operator.read_with(&cx, |o, _| {
        let selected: Vec<String> = o.selected_songs().into_iter().map(|(_, t)| t).collect();
        assert_eq!(selected, [before[2].clone()], "the next hit is selected");
        assert!(o.live.is_none(), "nothing went live");
    });
    cx.update(|window, cx| {
        let o = operator.read(cx);
        let row = o.song_rows.get(&1).cloned().unwrap();
        assert!(row.is_focused(window), "focus stays in the list");
    });
}

#[gpui::test]
fn arrow_keys_walk_past_the_viewport_in_a_sorted_virtual_list(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let numbers: Vec<String> = (0..300).map(|n| format!("{}", 1000 - n)).collect();
    let names: Vec<String> = (0..300).map(|n| format!("Study {n:03}")).collect();
    let songs: Vec<Fixture<'_>> = (0..300)
        .map(|n| {
            (
                names[n].as_str(),
                "",
                "",
                "",
                numbers[n].as_str(),
                &["Verse 1"][..],
            )
        })
        .collect();
    let (mut cx, operator) = open(cx, library(&dir, &songs), 300);
    choose_sort(&mut cx, "song-sort-song-number");
    settle(&mut cx, &operator, "number order", |o| {
        o.song_order.sort == SongSort::SongNumber
            && !o.catalog_refresh_pending()
            && o.catalog.first().is_some_and(|(_, t)| t == "Study 299")
    });
    cx.run_until_parked();
    click(&mut cx, "song-0");
    for _ in 0..40 {
        cx.simulate_keystrokes("down");
    }
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.song_cursor(), Some(40));
        assert!(o.song_visible.contains(&40), "{:?}", o.song_visible);
        assert!(!o.song_visible.contains(&0), "row 0 scrolled out");
        assert_eq!(o.selected_songs()[0].1, "Study 259");
    });
    cx.update(|window, cx| {
        let row = operator.read(cx).song_rows.get(&40).cloned().unwrap();
        assert!(row.is_focused(window), "the cursor row has keyboard focus");
    });
    cx.simulate_keystrokes("pageup");
    cx.run_until_parked();
    for _ in 0..3 {
        cx.simulate_keystrokes("up");
    }
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        let cursor = o.song_cursor().unwrap();
        assert!(cursor < 40, "moved back up to {cursor}");
        assert!(o.song_visible.contains(&cursor));
        assert!(o.live.is_none(), "list keys never go live");
    });
    cx.simulate_keystrokes("end");
    cx.run_until_parked();
    assert_eq!(
        operator.read_with(&cx, |o, _| o.selected_songs()[0].1.clone()),
        "Study 000"
    );
}

fn metadata_library(dir: &tempfile::TempDir) -> std::path::PathBuf {
    library(
        dir,
        &[(
            "Metadata Hymn",
            "Quill Author",
            "Synthetic Copyright 2026",
            "Acme Administration",
            "7654321",
            &["Verse 1", "Chorus", "Bridge"],
        )],
    )
}

fn last_footer(o: &Operator) -> Option<(usize, Option<String>)> {
    o.presented_footers.last().cloned()
}

fn live_at(cx: &mut VisualTestContext, operator: &Entity<Operator>, index: usize) {
    let before = operator.read_with(cx, |o, _| o.presented_footers.len());
    click(cx, ["live-slide-0", "live-slide-1", "live-slide-2"][index]);
    settle(cx, operator, "cue presented", |o| {
        o.presented_footers.len() > before && o.live_slide == Some(index)
    });
}

fn open_song_tab(cx: &mut VisualTestContext, operator: &Entity<Operator>) {
    cx.update(|window, cx| operator.update(cx, |o, cx| o.open_options(window, cx)));
    cx.run_until_parked();
    click(cx, "options-tab-song");
    assert!(cx.debug_bounds("options-song-page").is_some());
    assert!(
        cx.debug_bounds("options-monitor-0").is_none(),
        "the General page is not shown under the Song tab"
    );
}

#[gpui::test]
fn copyright_options_pick_the_slides_and_the_licensing_line(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, metadata_library(&dir), 1);
    settle(&mut cx, &operator, "settings", |o| o.settings_loaded());
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "preview", |o| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 3)
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "first slide", |o| {
        o.live_slide == Some(0) && !o.presented_footers.is_empty()
    });
    let strip = "Metadata Hymn - Quill Author\nSynthetic Copyright 2026";
    assert_eq!(
        operator.read_with(&cx, |o, _| last_footer(o)),
        Some((0, Some(strip.into()))),
        "observed default: every slide"
    );
    live_at(&mut cx, &operator, 1);
    assert_eq!(
        operator.read_with(&cx, |o, _| last_footer(o)),
        Some((1, Some(strip.into())))
    );

    // Show on first displayed slide only.
    open_song_tab(&mut cx, &operator);
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    click(&mut cx, "options-copyright-first-slide");
    click(&mut cx, "dialog-confirm");
    cx.run_until_parked();
    assert_eq!(
        operator.read_with(&cx, |o, _| submitted(o)),
        sent,
        "OK does not redraw the live slide"
    );
    live_at(&mut cx, &operator, 2);
    assert_eq!(
        operator.read_with(&cx, |o, _| last_footer(o)),
        Some((2, None))
    );
    live_at(&mut cx, &operator, 0);
    assert_eq!(
        operator.read_with(&cx, |o, _| last_footer(o)),
        Some((0, Some(strip.into())))
    );

    // License Info adds a third line with the song's CCLI number.
    open_song_tab(&mut cx, &operator);
    let license = operator.read_with(&cx, |o, _| o.settings.song.license.clone());
    cx.update(|window, cx| {
        license.update(cx, |field, cx| {
            field.replace_text_in_range(Some(0..0), "CCLI License # 99", window, cx);
        });
    });
    click(&mut cx, "dialog-confirm");
    live_at(&mut cx, &operator, 1);
    live_at(&mut cx, &operator, 0);
    assert_eq!(
        operator.read_with(&cx, |o, _| last_footer(o)),
        Some((
            0,
            Some(format!(
                "{strip}\nCCLI Song # 7654321 \u{b7} CCLI License # 99"
            ))
        ))
    );

    // Cancel keeps the stored choice; Display off hides the strip everywhere.
    open_song_tab(&mut cx, &operator);
    click(&mut cx, "options-copyright-display");
    click(&mut cx, "dialog-cancel");
    live_at(&mut cx, &operator, 1);
    live_at(&mut cx, &operator, 0);
    assert!(operator.read_with(&cx, |o, _| last_footer(o).unwrap().1.is_some()));
    open_song_tab(&mut cx, &operator);
    click(&mut cx, "options-copyright-display");
    click(&mut cx, "dialog-confirm");
    live_at(&mut cx, &operator, 1);
    live_at(&mut cx, &operator, 0);
    assert_eq!(
        operator.read_with(&cx, |o, _| last_footer(o)),
        Some((0, None))
    );
    settle(&mut cx, &operator, "settings written", |_| {
        let text = settings_file(&dir);
        text.contains("copyright_display=0\n")
            && text.contains("copyright_first_slide=1\n")
            && text.contains("license_info=CCLI License # 99\n")
    });
}

#[gpui::test]
fn options_song_tab_is_reachable_by_keyboard_and_escape_discards(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, metadata_library(&dir), 1);
    settle(&mut cx, &operator, "settings", |o| o.settings_loaded());
    cx.update(|window, cx| operator.update(cx, |o, cx| o.open_options(window, cx)));
    cx.run_until_parked();
    // Tab from Cancel wraps through the tab strip to the Song tab.
    let mut reached = false;
    for _ in 0..40 {
        cx.simulate_keystrokes("tab");
        let on_song =
            cx.update(|window, cx| operator.read(cx).settings.song.tab_focused(1, window));
        if on_song {
            reached = true;
            break;
        }
    }
    assert!(reached, "the Song tab is a tab stop");
    cx.simulate_keystrokes("enter");
    assert!(cx.debug_bounds("options-song-page").is_some());
    // The first checkbox follows the tab strip; Space/Enter toggles it.
    let mut toggled = false;
    for _ in 0..10 {
        cx.simulate_keystrokes("tab");
        let on_first =
            cx.update(|window, cx| operator.read(cx).settings.song.check_focused(1, window));
        if on_first {
            cx.simulate_keystrokes("enter");
            toggled = true;
            break;
        }
    }
    assert!(toggled, "the first-slide checkbox is a tab stop");
    assert_eq!(
        operator.read_with(&cx, |o, _| o
            .settings
            .song
            .draft
            .as_ref()
            .map(|d| d.first_slide_only)),
        Some(true)
    );
    cx.simulate_keystrokes("escape");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.dialog, None);
        assert!(
            !o.settings.current.copyright.first_slide_only,
            "Esc discards"
        );
    });
}
