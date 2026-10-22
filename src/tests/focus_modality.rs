//! sunday-core fix: Songs focus and editor modality (SC-RES-01, SC-SONG-01).
use super::*;

const HYMN_VERSE: &str = "Signal Hymn · Verse 1";

fn numbered_library(dir: &tempfile::TempDir, count: usize) -> std::path::PathBuf {
    let path = dir.path().join("library.sqlite");
    let mut repository = Repository::open(&path).unwrap();
    for n in 0..count {
        repository
            .save_song(
                None,
                Song {
                    title: format!("Song {n:04}"),
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

/// Live output on and the first song previewed, so a stray Go Live would
/// put it on the audience.
fn preview_first_song_with_output_on(cx: &mut VisualTestContext, operator: &Entity<Operator>) {
    click(cx, "live-output");
    settle(cx, operator, "output connection", connected);
    click(cx, "song-0");
    settle(cx, operator, "preview", |o| o.preview.is_some());
}

fn selected_titles(cx: &VisualTestContext, operator: &Entity<Operator>) -> Vec<String> {
    operator.read_with(cx, |o, _| {
        o.selected_songs().into_iter().map(|(_, t)| t).collect()
    })
}

fn row_focused(cx: &mut VisualTestContext, operator: &Entity<Operator>, index: usize) -> bool {
    cx.update(|window, cx| {
        operator
            .read(cx)
            .song_rows
            .get(&index)
            .is_some_and(|row| row.is_focused(window))
    })
}

fn row_shown(cx: &VisualTestContext, operator: &Entity<Operator>, index: usize) -> bool {
    operator.read_with(cx, |o, _| o.song_visible.contains(&index))
}

/// Polls long enough for a cue the operator wrongly sent to reach the
/// audience child.
fn idle(cx: &mut VisualTestContext, operator: &Entity<Operator>) {
    for _ in 0..20 {
        operator.update(cx, |o, cx| o.poll(cx));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn page_down_after_a_scrollbar_scroll_pages_the_songs_and_never_goes_live(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, numbered_library(&dir, 400), 400);
    preview_first_song_with_output_on(&mut cx, &operator);
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    let track = cx.debug_bounds("song-scrollbar").unwrap().center();
    cx.simulate_mouse_down(track, gpui::MouseButton::Left, Default::default());
    cx.simulate_mouse_up(track, gpui::MouseButton::Left, Default::default());
    assert!(
        !row_shown(&cx, &operator, 0),
        "the focused row scrolled out"
    );

    cx.simulate_keystrokes("pagedown");
    idle(&mut cx, &operator);
    operator.read_with(&cx, |o, _| {
        assert!(o.live.is_none(), "Page Down in Songs went live");
        assert_eq!(submitted(o), sent);
    });
    assert_eq!(
        selected_titles(&cx, &operator),
        ["Song 0004"],
        "one page of four rows"
    );
    assert!(row_shown(&cx, &operator, 4));
    assert!(row_focused(&mut cx, &operator, 4));
}

#[gpui::test]
fn ctrl_end_after_a_wheel_scroll_reaches_the_last_song(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, numbered_library(&dir, 400), 400);
    click(&mut cx, "song-0");
    let list = cx.debug_bounds("song-list").unwrap().center();
    cx.simulate_mouse_move(list, None, Default::default());
    cx.simulate_event(gpui::ScrollWheelEvent {
        position: list,
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-2000.))),
        ..Default::default()
    });
    assert!(
        !row_shown(&cx, &operator, 0),
        "the focused row scrolled out"
    );

    cx.simulate_keystrokes("ctrl-end");
    cx.run_until_parked();
    assert_eq!(selected_titles(&cx, &operator), ["Song 0399"]);
    assert!(row_shown(&cx, &operator, 399));
    assert!(row_focused(&mut cx, &operator, 399));
}

#[gpui::test]
fn deleting_the_last_song_hands_selection_and_focus_to_its_neighbour(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, numbered_library(&dir, 4), 4);
    click(&mut cx, "song-3");
    cx.simulate_keystrokes("delete");
    click(&mut cx, "dialog-confirm");
    settle(&mut cx, &operator, "deleted", |o| {
        o.catalog.len() == 3 && !o.catalog_refresh_pending()
    });
    cx.run_until_parked();
    assert_eq!(selected_titles(&cx, &operator), ["Song 0002"]);
    assert!(
        row_focused(&mut cx, &operator, 2),
        "the neighbour row has keyboard focus"
    );

    cx.simulate_keystrokes("up");
    assert_eq!(selected_titles(&cx, &operator), ["Song 0001"]);
    assert!(row_focused(&mut cx, &operator, 1));
}

#[gpui::test]
fn deleting_the_only_song_leaves_keys_working_and_page_down_out_of_live(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open(cx, library_with_song(&dir), 1);
    preview_first_song_with_output_on(&mut cx, &operator);
    cx.simulate_keystrokes("delete");
    click(&mut cx, "dialog-confirm");
    settle(&mut cx, &operator, "deleted", |o| {
        o.catalog.is_empty() && !o.catalog_refresh_pending()
    });
    cx.run_until_parked();

    cx.simulate_keystrokes("ctrl-b");
    assert!(
        operator.read_with(&cx, |o, _| o.masks.is_on(Mask::Black)),
        "show keys still reach the operator"
    );
    cx.simulate_keystrokes("pagedown");
    idle(&mut cx, &operator);
    assert!(
        operator.read_with(&cx, |o, _| o.live.is_none()),
        "Page Down in the empty Songs list went live"
    );
}

/// Schedule [Signal Hymn, Quiet Canticle] with the hymn live and the Live
/// pane focused, beside a song editor with an unsaved title.
fn live_hymn_beside_a_dirty_editor(
    cx: &mut TestAppContext,
    dir: &tempfile::TempDir,
) -> (VisualTestContext, Entity<Operator>, VisualTestContext) {
    use gpui::EntityInputHandler;
    let (mut cx, operator) = open(cx, library_with_songs(dir), 2);
    add_song(&mut cx, &operator, "Signal Hymn");
    add_song(&mut cx, &operator, SECOND_SONG);
    let before = editor_windows(&cx);
    click(&mut cx, "open-library");
    let (mut editor_cx, editor) = opened_editor(&mut cx, &before, |(_, _, busy)| !busy);
    let title = editor.read_with(&editor_cx, |e, _| e.probe_title_field());
    editor_cx.update(|window, cx| {
        title.update(cx, |field, cx| {
            field.replace_text_in_range(Some(0..0), "Unsaved draft", window, cx);
        });
    });
    click(&mut cx, "schedule-item-0");
    settle(&mut cx, &operator, "first item previewed", |o| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 3)
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "hymn verse", |o| {
        o.on_screen() == Some(HYMN_VERSE)
    });
    cx.run_until_parked();
    (cx, operator, editor_cx)
}

#[gpui::test]
fn the_editor_close_prompt_blocks_operator_clicks_and_keys_until_cancelled(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, mut editor_cx) = live_hymn_beside_a_dirty_editor(cx, &dir);
    assert!(!editor_cx.simulate_close(), "a dirty editor asks first");
    assert!(editor_cx.debug_bounds("close-prompt").is_some());
    let sent = operator.read_with(&cx, |o, _| submitted(o));

    // The Live pane has focus, so unblocked Page Down would show the next
    // item and Right the next slide.
    for keys in ["b", "ctrl-b", "right", "pagedown", "alt-l enter"] {
        cx.simulate_keystrokes(keys);
    }
    for control in ["mask-black", "live-next", "go-live", "live-output"] {
        click(&mut cx, control);
    }
    idle(&mut cx, &operator);
    operator.read_with(&cx, |o, _| {
        assert_eq!(submitted(o), sent, "nothing reached the audience");
        assert_eq!(o.on_screen(), Some(HYMN_VERSE));
        assert!(!o.masks.is_on(Mask::Black));
        assert!(connected(o), "Live output stays on");
    });

    editor_cx.simulate_keystrokes("escape");
    assert!(editor_cx.debug_bounds("close-prompt").is_none());
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "the next item after Cancel", |o| {
        o.on_screen() == Some("Quiet Canticle · Verse 1")
    });
}

#[gpui::test]
fn dont_save_in_the_editor_close_prompt_unblocks_the_operator(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, mut editor_cx) = live_hymn_beside_a_dirty_editor(cx, &dir);
    assert!(!editor_cx.simulate_close());
    click(&mut cx, "mask-black");
    assert!(!operator.read_with(&cx, |o, _| o.masks.is_on(Mask::Black)));

    let no = editor_cx.debug_bounds("close-prompt-no").unwrap().center();
    editor_cx.simulate_click(no, Default::default());
    assert!(
        editor_windows(&cx).is_empty(),
        "Don't Save closed the editor"
    );
    click(&mut cx, "mask-black");
    assert!(operator.read_with(&cx, |o, _| o.masks.is_on(Mask::Black)));
}

#[gpui::test]
fn an_editor_removed_with_its_prompt_open_unblocks_the_operator(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, mut editor_cx) = live_hymn_beside_a_dirty_editor(cx, &dir);
    assert!(!editor_cx.simulate_close());
    assert!(editor_cx.debug_bounds("close-prompt").is_some());

    editor_cx.update(|window, _| window.remove_window());
    drop(editor_cx);
    cx.run_until_parked();
    assert!(editor_windows(&cx).is_empty(), "the editor window is gone");
    assert!(
        !cx.update(|_, cx| crate::song_library::editor_prompt_open(cx)),
        "a closed editor no longer holds the prompt"
    );
    click(&mut cx, "mask-black");
    settle(&mut cx, &operator, "Black after the editor is gone", |o| {
        o.masks.is_on(Mask::Black)
    });
}
