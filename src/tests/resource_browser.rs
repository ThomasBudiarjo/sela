//! Songs list: multi-select, sort, virtualization, keyboard reach, delete and
//! the shared modal dialog focus scope (focus-scopes-resource-browser).
use super::*;
use crate::operator::Dialog;
use gpui::EntityInputHandler;

fn library_of(dir: &tempfile::TempDir, titles: &[String]) -> std::path::PathBuf {
    let path = dir.path().join("library.sqlite");
    let mut repository = Repository::open(&path).unwrap();
    for title in titles {
        repository
            .save_song(
                None,
                Song {
                    title: title.clone(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants: Vec::new(),
                    sections: vec![Section {
                        id: SectionId::allocate(),
                        label: "Verse 1".into(),
                        lyrics: "Psalm line".into(),
                        format: Default::default(),
                        background: None,
                    }],
                    master: None,
                },
            )
            .unwrap();
    }
    path
}

fn numbered(count: usize) -> Vec<String> {
    (0..count).map(|n| format!("Song {n:04}")).collect()
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

fn selected_titles(cx: &VisualTestContext, operator: &Entity<Operator>) -> Vec<String> {
    operator.read_with(cx, |o, _| {
        o.selected_songs().into_iter().map(|(_, t)| t).collect()
    })
}

fn focus_row(cx: &mut VisualTestContext, operator: &Entity<Operator>, index: usize) {
    cx.update(|window, cx| {
        operator.update(cx, |o, cx| o.focus_song_row(index, window, cx));
    });
}

#[gpui::test]
fn shift_and_ctrl_clicks_select_ranges_and_add_to_schedule_keeps_list_order(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, library_of(&dir, &numbered(8)), 8);
    click(&mut cx, "song-0");
    let shift = gpui::Modifiers {
        shift: true,
        ..Default::default()
    };
    let ctrl = gpui::Modifiers {
        control: true,
        ..Default::default()
    };
    let at = |cx: &mut VisualTestContext, row: &'static str, modifiers| {
        let bounds = cx.debug_bounds(row).unwrap();
        cx.simulate_click(bounds.center(), modifiers);
    };
    at(&mut cx, "song-3", shift);
    assert_eq!(
        selected_titles(&cx, &operator),
        ["Song 0000", "Song 0001", "Song 0002", "Song 0003"]
    );
    at(&mut cx, "song-1", ctrl);
    at(&mut cx, "song-4", ctrl);
    let want = ["Song 0000", "Song 0002", "Song 0003", "Song 0004"];
    assert_eq!(selected_titles(&cx, &operator), want);
    click(&mut cx, "add-to-schedule");
    operator.read_with(&cx, |o, _| assert_eq!(entry_titles(o), want));
    // A plain click collapses the selection to one song.
    click(&mut cx, "song-3");
    assert_eq!(selected_titles(&cx, &operator), ["Song 0003"]);
}

#[gpui::test]
fn shift_arrows_extend_from_the_anchor_and_ctrl_a_selects_all(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, library_of(&dir, &numbered(6)), 6);
    click(&mut cx, "song-1");
    cx.simulate_keystrokes("shift-down shift-down");
    assert_eq!(
        selected_titles(&cx, &operator),
        ["Song 0001", "Song 0002", "Song 0003"]
    );
    cx.simulate_keystrokes("shift-up");
    assert_eq!(selected_titles(&cx, &operator), ["Song 0001", "Song 0002"]);
    cx.simulate_keystrokes("ctrl-a");
    assert_eq!(selected_titles(&cx, &operator).len(), 6);
}

#[gpui::test]
fn sort_by_menu_reorders_by_title_or_last_save_and_ticks_the_choice(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let titles: Vec<String> = ["Beta", "Alpha", "Gamma"].map(String::from).into();
    let (mut cx, operator) = open(cx, library_of(&dir, &titles), 3);
    let order = |cx: &VisualTestContext| -> Vec<String> {
        operator.read_with(cx, |o, _| {
            o.catalog.iter().map(|(_, t)| t.clone()).collect()
        })
    };
    assert_eq!(
        order(&cx),
        ["Alpha", "Beta", "Gamma"],
        "title is the default"
    );

    right_click(&mut cx, "song-0");
    click(&mut cx, "song-menu-sort");
    assert!(cx.debug_bounds("song-sort-title").is_some());
    click(&mut cx, "song-sort-modified");
    settle(&mut cx, &operator, "date order", |o| {
        o.catalog.first().is_some_and(|(_, t)| t == "Beta") && !o.catalog_refresh_pending()
    });
    assert_eq!(order(&cx), ["Beta", "Alpha", "Gamma"]);

    right_click(&mut cx, "song-0");
    click(&mut cx, "song-menu-sort");
    click(&mut cx, "song-sort-descending");
    settle(&mut cx, &operator, "descending", |o| {
        o.catalog.first().is_some_and(|(_, t)| t == "Gamma") && !o.catalog_refresh_pending()
    });
    assert_eq!(order(&cx), ["Gamma", "Alpha", "Beta"]);
    operator.read_with(&cx, |o, _| {
        assert!(o.song_order.descending);
    });
}

#[gpui::test]
fn only_visible_rows_are_built_and_home_end_reach_the_first_and_last_row(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, library_of(&dir, &numbered(400)), 400);
    operator.read_with(&cx, |o, _| {
        assert!(
            (1..40).contains(&o.song_visible.len()),
            "built {:?} of 400 rows",
            o.song_visible
        );
    });
    click(&mut cx, "song-0");
    cx.simulate_keystrokes("ctrl-end");
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.selected_songs().len(), 1);
        assert_eq!(o.selected_songs()[0].1, "Song 0399");
        assert!(o.song_visible.contains(&399), "last row scrolled into view");
    });
    assert!(cx.debug_bounds("song-399").is_some());
    cx.update(|window, cx| {
        let focused = operator.read(cx).song_rows.get(&399).cloned().unwrap();
        assert!(
            focused.is_focused(window),
            "the last row has keyboard focus"
        );
    });
    cx.simulate_keystrokes("ctrl-home");
    cx.run_until_parked();
    assert_eq!(selected_titles(&cx, &operator), ["Song 0000"]);
    assert!(cx.debug_bounds("song-0").is_some());
    cx.simulate_keystrokes("pagedown pagedown");
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        let cursor = o.song_cursor().unwrap();
        assert_eq!(cursor, 8, "two pages of four rows");
        assert!(o.song_visible.contains(&cursor), "{:?}", o.song_visible);
    });
}

#[gpui::test]
fn delete_asks_with_yes_no_defaults_to_no_and_selection_moves_to_a_neighbour(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = library_of(&dir, &numbered(4));
    let (mut cx, operator) = open(cx, path.clone(), 4);
    click(&mut cx, "song-1");
    cx.simulate_keystrokes("delete");
    operator.read_with(&cx, |o, _| {
        assert!(matches!(o.dialog, Some(Dialog::DeleteSong(_))));
    });
    assert!(cx.debug_bounds("dialog-confirm").is_some());
    assert_control(&mut cx, &operator, Control::DialogCancel);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        operator.read_with(&cx, |o, _| o.catalog.len()),
        4,
        "No keeps it"
    );

    click(&mut cx, "song-1");
    cx.simulate_keystrokes("delete escape");
    assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);
    assert_eq!(operator.read_with(&cx, |o, _| o.catalog.len()), 4);

    cx.simulate_keystrokes("delete");
    click(&mut cx, "dialog-confirm");
    settle(&mut cx, &operator, "deleted", |o| {
        o.catalog.len() == 3 && !o.catalog_refresh_pending()
    });
    operator.read_with(&cx, |o, _| {
        assert!(o.catalog.iter().all(|(_, t)| t != "Song 0001"));
        assert_eq!(o.selected_songs().len(), 1, "a neighbour is selected");
    });
}

#[gpui::test]
fn modal_dialogs_centre_trap_tab_block_live_shortcuts_and_return_focus_on_escape(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, library_of(&dir, &numbered(3)), 3);
    click(&mut cx, "song-1");
    cx.simulate_keystrokes("delete");
    let window = cx.debug_bounds("dialog-backdrop").unwrap();
    let panel = cx.debug_bounds("schedule-dialog").unwrap();
    let (dx, dy) = (
        f32::from(panel.center().x - window.center().x).abs(),
        f32::from(panel.center().y - window.center().y).abs(),
    );
    assert!(dx < 2. && dy < 2., "dialog is centred, off by {dx},{dy}");

    for _ in 0..8 {
        cx.simulate_keystrokes("tab");
        let inside =
            cx.update(|window, cx| operator.read(cx).dialog_scope.contains_focused(window, cx));
        assert!(inside, "Tab stays inside the dialog");
    }
    for _ in 0..3 {
        cx.simulate_keystrokes("shift-tab");
        let inside =
            cx.update(|window, cx| operator.read(cx).dialog_scope.contains_focused(window, cx));
        assert!(inside, "Shift+Tab stays inside the dialog");
    }
    cx.simulate_keystrokes("ctrl-b ctrl-l pagedown");
    operator.read_with(&cx, |o, _| {
        assert!(o.live.is_none());
        assert!(!o.masks.is_on(Mask::Black));
        assert!(!o.masks.is_on(Mask::Logo));
    });

    cx.simulate_keystrokes("escape");
    operator.read_with(&cx, |o, _| assert_eq!(o.dialog, None));
    cx.update(|window, cx| {
        let row = operator.read(cx).song_rows.get(&1).cloned().unwrap();
        assert!(row.is_focused(window), "focus returns to the song row");
    });
}

#[gpui::test]
fn options_open_centred_and_escape_returns_focus(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, library_of(&dir, &numbered(2)), 2);
    focus_row(&mut cx, &operator, 0);
    cx.update(|window, cx| {
        operator.update(cx, |o, cx| o.open_options(window, cx));
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("options-backdrop").is_some());
    cx.simulate_keystrokes("escape");
    operator.read_with(&cx, |o, _| assert_eq!(o.dialog, None));
    cx.update(|window, cx| {
        let row = operator.read(cx).song_rows.get(&0).cloned().unwrap();
        assert!(row.is_focused(window));
    });
}

#[gpui::test]
fn more_than_the_result_limit_shows_a_refine_hint(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, library_of(&dir, &numbered(260)), 260);
    assert!(cx.debug_bounds("song-search-more").is_none());
    let input = operator.read_with(&cx, |o, _| o.search_input.clone());
    cx.update(|window, cx| {
        input.update(cx, |field, cx| {
            field.replace_text_in_range(Some(0..0), "psalm", window, cx);
        });
    });
    settle(&mut cx, &operator, "capped hits", |o| {
        o.search.hits().is_some_and(|h| h.len() == 200)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("song-search-more").is_some());
    operator.read_with(&cx, |o, _| assert!(o.search.truncated()));
}
