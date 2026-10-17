//! Live and Preview keyboard navigation, label jumps, status indicators and
//! tile thumbnails (preview-live-navigation).
use super::*;

const QUIET: &str = "Quiet Canticle · Verse 1";

fn on_screen(o: &Operator, label: &str) -> bool {
    o.on_screen() == Some(label)
}

fn live_hymn(
    cx: &mut TestAppContext,
    dir: &tempfile::TempDir,
) -> (VisualTestContext, Entity<Operator>) {
    let (mut cx, operator, _) = with_library(cx, Some(library_with_songs(dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    add_song(&mut cx, &operator, "Signal Hymn");
    add_song(&mut cx, &operator, SECOND_SONG);
    click(&mut cx, "schedule-item-0");
    settle(&mut cx, &operator, "first item previewed", |o| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 3)
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    (cx, operator)
}

#[gpui::test]
fn arrows_home_end_move_live_and_stop_at_the_ends(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_hymn(cx, &dir);
    click(&mut cx, "preview-slide-1");
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "chorus", |o| {
        on_screen(o, "Signal Hymn · Chorus")
    });
    cx.simulate_keystrokes("right");
    settle(&mut cx, &operator, "bridge", |o| {
        on_screen(o, "Signal Hymn · Bridge")
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert_eq!(submitted(o), sent, "Right stops at the last slide");
        assert_eq!(
            o.live.as_ref().and_then(|l| l.entry),
            o.preview.as_ref().and_then(|p| p.entry)
        );
    });
    cx.simulate_keystrokes("left");
    settle(&mut cx, &operator, "chorus again", |o| {
        on_screen(o, "Signal Hymn · Chorus")
    });
    cx.simulate_keystrokes("end");
    settle(&mut cx, &operator, "end", |o| {
        on_screen(o, "Signal Hymn · Bridge")
    });
    cx.simulate_keystrokes("home");
    settle(&mut cx, &operator, "home", |o| {
        on_screen(o, "Signal Hymn · Verse 1")
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    cx.simulate_keystrokes("left");
    cx.run_until_parked();
    assert_eq!(operator.read_with(&cx, |o, _| submitted(o)), sent);
    assert_eq!(
        operator.read_with(&cx, |o, _| o.preview_slide),
        1,
        "Live keys never move the Preview selection"
    );
}

#[gpui::test]
fn page_keys_in_live_move_between_schedule_items_and_stop(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_hymn(cx, &dir);
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "hymn verse", |o| {
        on_screen(o, "Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("right right");
    settle(&mut cx, &operator, "hymn bridge", |o| {
        on_screen(o, "Signal Hymn · Bridge")
    });
    // The Live pane has focus after Go Live: PageDown goes to the next item.
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "canticle", |o| on_screen(o, QUIET));
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.selected_entry, o.live.as_ref().and_then(|l| l.entry));
        assert_eq!(o.live_slide, Some(0));
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    cx.simulate_keystrokes("pagedown");
    cx.run_until_parked();
    assert_eq!(
        operator.read_with(&cx, |o, _| submitted(o)),
        sent,
        "the end of the schedule stops"
    );
    cx.simulate_keystrokes("pageup");
    settle(&mut cx, &operator, "back to the hymn", |o| {
        on_screen(o, "Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("pageup");
    cx.run_until_parked();
    assert!(operator.read_with(&cx, |o, _| on_screen(o, "Signal Hymn · Verse 1")));
}

#[gpui::test]
fn label_keys_jump_live_and_ignore_unknown_labels_and_text_fields(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_hymn(cx, &dir);
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "verse", |o| {
        on_screen(o, "Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("c enter");
    settle(&mut cx, &operator, "chorus", |o| {
        on_screen(o, "Signal Hymn · Chorus")
    });
    cx.simulate_keystrokes("shift-b enter");
    settle(&mut cx, &operator, "bridge", |o| {
        on_screen(o, "Signal Hymn · Bridge")
    });
    cx.simulate_keystrokes("v 1 enter");
    settle(&mut cx, &operator, "verse again", |o| {
        on_screen(o, "Signal Hymn · Verse 1")
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    cx.simulate_keystrokes("v 2 enter");
    cx.simulate_keystrokes("t enter");
    cx.run_until_parked();
    assert_eq!(
        operator.read_with(&cx, |o, _| submitted(o)),
        sent,
        "unknown labels change nothing"
    );

    // Typing into the search field never jumps.
    cx.update(|window, cx| {
        operator
            .read(cx)
            .search_input
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx)
    });
    cx.simulate_keystrokes("c enter");
    cx.run_until_parked();
    assert_eq!(operator.read_with(&cx, |o, _| submitted(o)), sent);
    assert!(operator.read_with(&cx, |o, _| on_screen(o, "Signal Hymn · Verse 1")));
}

#[gpui::test]
fn presenter_remote_keys_black_and_period_toggle_the_mask(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_hymn(cx, &dir);
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "verse", |o| {
        on_screen(o, "Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("b");
    settle(&mut cx, &operator, "Black", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });
    cx.simulate_keystrokes(".");
    settle(&mut cx, &operator, "unmasked", |o| {
        confirmed_mask(o) == Some(Layer::None)
    });
}

#[gpui::test]
fn go_live_under_a_mask_keeps_the_mask_until_it_is_turned_off(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_hymn(cx, &dir);
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "verse", |o| {
        on_screen(o, "Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });
    click(&mut cx, "preview-slide-2");
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "bridge under Black", |o| {
        on_screen(o, "Signal Hymn · Bridge")
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
        assert!(o.masks.is_on(Mask::Black));
        assert_eq!(o.indicator(Mask::Black), Indicator::On);
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "unmasked", |o| {
        confirmed_mask(o) == Some(Layer::None)
    });
    assert!(operator.read_with(&cx, |o, _| on_screen(o, "Signal Hymn · Bridge")));
}

fn library_with_twelve_slides(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("library.sqlite");
    let sections = (1..=12)
        .map(|n| Section {
            id: SectionId::allocate(),
            label: format!("Verse {n}"),
            lyrics: format!("Line of verse {n}"),
            format: Default::default(),
            background: None,
        })
        .collect();
    Repository::open(&path)
        .unwrap()
        .save_song(
            None,
            Song {
                title: "Long Hymn".into(),
                authors: String::new(),
                copyright: String::new(),
                license: String::new(),
                administrator: String::new(),
                song_number: String::new(),
                variants: Vec::new(),
                sections,
                master: None,
            },
        )
        .unwrap();
    path
}

#[gpui::test]
fn rapid_next_presses_end_on_the_right_slide_in_order(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_twelve_slides(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "preview", |o| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 12)
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "first slide", |o| {
        on_screen(o, "Long Hymn · Verse 1")
    });
    cx.simulate_keystrokes("right right right right right right right right right right");
    settle(&mut cx, &operator, "slide 11", |o| {
        on_screen(o, "Long Hymn · Verse 11") && o.pending_cues.is_empty()
    });
    // Nothing older arrives afterwards.
    for _ in 0..40 {
        operator.update(&mut cx, |o, cx| o.poll(cx));
        std::thread::sleep(Duration::from_millis(5));
    }
    operator.read_with(&cx, |o, _| {
        assert!(on_screen(o, "Long Hymn · Verse 11"));
        assert_eq!(o.live_slide, Some(10));
    });
}

#[gpui::test]
fn a_full_preparation_lane_holds_the_newest_press_instead_of_dropping_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_twelve_slides(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "preview", |o| {
        o.preview.as_ref().is_some_and(|p| p.resolved.is_some())
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "first slide", |o| {
        on_screen(o, "Long Hymn · Verse 1")
    });
    let (open, preparer) = gated_preparer(Default::default());
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(preparer));
    cx.simulate_keystrokes("right right right right right right");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.pending_cues.len(), sela::preparation::cue::LIVE_DEPTH);
        assert_eq!(
            o.requested_slide(),
            Some(6),
            "every press counted, the newest is held"
        );
        assert!(has_line(o, "◌ Preparing the slide…"));
    });
    for _ in 0..8 {
        let _ = open.send(());
    }
    settle(&mut cx, &operator, "slide 7", |o| {
        on_screen(o, "Long Hymn · Verse 7") && o.deferred_send.is_none()
    });
}

#[gpui::test]
fn indicators_for_preparing_failed_and_disconnected_are_separate(cx: &mut TestAppContext) {
    use sela::preparation::cue::{Built, CuePreparer};
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_hymn(cx, &dir);
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "verse", |o| {
        on_screen(o, "Signal Hymn · Verse 1")
    });
    operator.read_with(&cx, |o, _| {
        for text in [
            "◌ Preparing the slide…",
            "✕ Failed: the slide was not applied",
            "⚠ Disconnected from the audience output",
        ] {
            assert!(!has_line(o, text), "{text} is not shown while confirmed");
        }
    });

    let (open, preparer) = gated_preparer(Default::default());
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(preparer));
    cx.simulate_keystrokes("right");
    operator.read_with(&cx, |o, _| {
        assert!(has_line(o, "◌ Preparing the slide…"));
        assert!(has_line(o, "On screen: Signal Hymn · Verse 1"));
    });
    open.send(()).unwrap();
    settle(&mut cx, &operator, "chorus", |o| {
        on_screen(o, "Signal Hymn · Chorus")
    });
    assert!(operator.read_with(&cx, |o, _| !has_line(o, "◌ Preparing the slide…")));

    let failing = CuePreparer::with_builder(|_, _| -> Result<Built, _> {
        Err(sela::scene::PrepareError::InvalidScene)
    })
    .unwrap();
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(failing));
    cx.simulate_keystrokes("right");
    settle(&mut cx, &operator, "failure shown", |o| {
        has_line(o, "✕ Failed: the slide was not applied")
    });
    operator.read_with(&cx, |o, _| {
        assert!(has_line(o, "On screen: Signal Hymn · Chorus"));
        assert!(!has_line(o, "◌ Preparing the slide…"));
    });

    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(&mut cx.cx, Some(library_with_song(&dir)), "exit");
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "loss", |o| {
        matches!(
            o.output.as_ref().map(|out| out.status()),
            Some(Status::Lost(_))
        )
    });
    assert!(operator.read_with(&cx, |o, _| has_line(
        o,
        "⚠ Disconnected from the audience output"
    )));
}

/// The intermittent i18n-windows.py failure: the audience window exists
/// before the renderer reports Ready, and a Go Live in that gap is refused
/// with a message, never queued or shown. Callers must wait for the
/// connected status before pressing Go Live.
#[gpui::test]
fn go_live_before_the_renderer_is_ready_is_refused_and_reported(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_song(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "preview", |o| o.preview.is_some());
    click(&mut cx, "live-output");
    operator.read_with(&cx, |o, _| assert!(!connected(o)));
    cx.simulate_keystrokes("pagedown");
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            o.live_message.as_deref(),
            Some("Live output is still starting")
        );
        assert!(o.live.is_none(), "nothing is queued for later");
    });
    settle(&mut cx, &operator, "output connection", connected);
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert!(o.live.is_none() && o.on_screen().is_none());
        assert!(has_line(o, "Live output 640×360"));
    });
}

#[gpui::test]
fn schedule_home_end_and_preview_up_down_move_selections_only(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_hymn(cx, &dir);
    let ids: Vec<_> = operator.read_with(&cx, |o, _| {
        o.schedule.entries().iter().map(|e| e.id).collect()
    });
    click(&mut cx, "schedule-item-0");
    cx.simulate_keystrokes("end");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.selected_entry),
        Some(ids[1])
    );
    cx.simulate_keystrokes("home");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.selected_entry),
        Some(ids[0])
    );
    settle(&mut cx, &operator, "hymn previewed", |o| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 3)
    });
    click(&mut cx, "preview-slide-1");
    cx.simulate_keystrokes("down");
    assert_eq!(operator.read_with(&cx, |o, _| o.preview_slide), 2);
    cx.simulate_keystrokes("down");
    assert_eq!(operator.read_with(&cx, |o, _| o.preview_slide), 2);
    cx.simulate_keystrokes("up up up");
    assert_eq!(operator.read_with(&cx, |o, _| o.preview_slide), 0);
    assert!(operator.read_with(&cx, |o, _| o.live.is_none()));
}

#[gpui::test]
fn image_backgrounds_show_as_tile_thumbnails(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_background_song(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "thumbnails", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| o.thumbnail(&p.slides[0]).is_some() && o.tile_images.len() == 2)
    });
    operator.read_with(&cx, |o, _| {
        let slides = &o.preview.as_ref().unwrap().slides;
        let thumb = o.thumbnail(&slides[0]).expect("verse image decoded");
        let bgra = thumb.as_bytes(0).unwrap();
        assert_eq!(
            &bgra[..4],
            &[210, 110, 30, 255],
            "the tile shows the image's pixels"
        );
        assert!(
            o.thumbnail(&slides[1]).is_none(),
            "missing image stays black"
        );
        assert!(
            o.thumbnail(&slides[2]).is_none(),
            "colors need no thumbnail"
        );
    });
}
