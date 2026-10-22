//! sunday-core scrutiny round 2 (SC2-*): regression tests for the defects
//! found in the shipped sunday-core features. Each test fails on the sources
//! before its fix (see the scrutiny-r2 work-log record).
use super::*;
use sela::preparation::cue::{Built, CuePreparer};

/// Polls long enough for a cue the operator wrongly sent to reach the
/// audience child.
fn idle(cx: &mut VisualTestContext, operator: &Entity<Operator>) {
    for _ in 0..20 {
        operator.update(cx, |o, cx| o.poll(cx));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Two library songs (Signal Hymn: Verse 1, Chorus, Bridge; Quiet Canticle:
/// Verse 1) with Live output connected to the fake audience.
fn two_songs_live(
    cx: &mut TestAppContext,
    dir: &tempfile::TempDir,
) -> (VisualTestContext, Entity<Operator>) {
    let (mut cx, operator, _) = with_library(cx, Some(library_with_songs(dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    (cx, operator)
}

/// Clicks a Songs row and waits until its preview can go live.
fn preview_song(cx: &mut VisualTestContext, operator: &Entity<Operator>, title: &'static str) {
    let row = song_row(cx, operator, title);
    click(cx, row);
    settle(cx, operator, "preview with faces", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.title == title && p.entry.is_none() && p.resolved.is_some())
    });
}

fn failing_preparer() -> CuePreparer {
    CuePreparer::with_builder(|_, _| -> Result<Built, _> {
        Err(sela::scene::PrepareError::InvalidScene)
    })
    .unwrap()
}

/// SC2-FOCUS-01: the Songs row menu takes the keyboard focus like a menu bar
/// menu, so show shortcuts must stay inert until it closes (EW8-OBS-010
/// menus; the `!SelaMenuBar` rule already covers the bar menus).
#[gpui::test]
fn show_keys_stay_inert_while_the_songs_menu_is_open(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = two_songs_live(cx, &dir);
    preview_song(&mut cx, &operator, "Signal Hymn");
    let hymn = song_row(&mut cx, &operator, "Signal Hymn");
    right_click(&mut cx, hymn);
    assert!(operator.read_with(&cx, |o, _| o.song_menu.is_some()));
    let sent = operator.read_with(&cx, |o, _| submitted(o));

    for keys in ["b", ".", "ctrl-b", "ctrl-c", "pagedown", "v enter", "right"] {
        cx.simulate_keystrokes(keys);
        idle(&mut cx, &operator);
        operator.read_with(&cx, |o, _| {
            assert!(o.song_menu.is_some(), "{keys} closed the menu");
            assert!(!o.masks.any(), "{keys} toggled a mask under the menu");
            assert!(o.live.is_none(), "{keys} went live under the menu");
            assert_eq!(submitted(o), sent, "{keys} reached the audience");
        });
    }
    // Esc closes the menu; the same keys work again.
    cx.simulate_keystrokes("escape");
    assert!(operator.read_with(&cx, |o, _| o.song_menu.is_none()));
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black after the menu closed", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });
}

/// SC2-LIVE-01: when another item goes live, `live_slide` still pointed into
/// the previous item. After a failed Go Live the footer named a slide of the
/// new item that was never shown, Next stepped from the old index, and a
/// surface refresh put a slide of the new item on the audience that nobody
/// asked for.
#[gpui::test]
fn a_failed_go_live_of_another_item_never_borrows_the_old_slide_index(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = two_songs_live(cx, &dir);
    preview_song(&mut cx, &operator, SECOND_SONG);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "canticle on screen", |o| {
        o.on_screen() == Some("Quiet Canticle · Verse 1")
    });

    preview_song(&mut cx, &operator, "Signal Hymn");
    click(&mut cx, "preview-slide-2");
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(failing_preparer()));
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "failure reported", |o| {
        o.live_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Slide could not be prepared"))
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.on_screen(), Some("Quiet Canticle · Verse 1"));
        assert_eq!(
            o.live_footer(),
            "Signal Hymn · not sent",
            "no slide of the new item was shown"
        );
    });

    // A working preparer again: a surface refresh and Next send nothing.
    operator.update(&mut cx, |o, _| {
        o.cue_preparer = None;
        o.resurface();
    });
    idle(&mut cx, &operator);
    click(&mut cx, "live-next");
    idle(&mut cx, &operator);
    operator.read_with(&cx, |o, _| {
        assert_eq!(submitted(o), sent, "nothing the operator chose was sent");
        assert_eq!(o.on_screen(), Some("Quiet Canticle · Verse 1"));
        assert_eq!(
            o.live_message.as_deref(),
            Some("Signal Hymn is not on screen yet; use Go Live first")
        );
    });

    // Go Live again shows the slide the operator chose.
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "bridge on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Bridge")
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| o.live_footer()),
        "Signal Hymn · Slide 3 of 3"
    );
}

/// SC2-LIVE-02: Go Live on another schedule entry of the same song revision
/// (a duplicate) kept the earlier entry's cues in preparation, so a slide the
/// operator had moved away from still reached the audience after the newer
/// Go Live.
#[gpui::test]
fn go_live_on_a_duplicate_entry_drops_the_previous_entrys_pending_cue(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = two_songs_live(cx, &dir);
    add_song(&mut cx, &operator, "Signal Hymn");
    add_song(&mut cx, &operator, "Signal Hymn");
    let ids: Vec<_> = operator.read_with(&cx, |o, _| {
        o.schedule.entries().iter().map(|e| e.id).collect()
    });
    click(&mut cx, "schedule-item-0");
    settle(&mut cx, &operator, "first entry previewed", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.entry == Some(ids[0]) && p.resolved.is_some())
    });
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });

    // The first entry's Chorus is still being prepared when the duplicate
    // goes live at its Verse 1.
    let (open, preparer) = gated_preparer(Default::default());
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(preparer));
    click(&mut cx, "live-next");
    assert_eq!(operator.read_with(&cx, |o, _| o.pending_cues.len()), 1);
    click(&mut cx, "schedule-item-1");
    settle(&mut cx, &operator, "duplicate previewed", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.entry == Some(ids[1]) && p.resolved.is_some())
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    click(&mut cx, "go-live");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.pending_cues.len()),
        1,
        "only the duplicate's cue is still wanted"
    );
    open.send(()).unwrap();
    open.send(()).unwrap();
    settle(&mut cx, &operator, "duplicate live", |o| {
        o.pending_cues.is_empty() && o.live_slide == Some(0)
    });
    idle(&mut cx, &operator);
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            submitted(o),
            sent + 1,
            "the abandoned Chorus never reaches the audience"
        );
        assert_eq!(o.live.as_ref().unwrap().entry, Some(ids[1]));
        assert_eq!(o.on_screen(), Some("Signal Hymn · Verse 1"));
    });
}

/// SC2-LIVE-03: a Live-pane Page Down waiting for the next entry's preview
/// kept its target after the operator picked a library song instead, so the
/// next Page Down started from an entry that never went live and skipped one.
#[gpui::test]
fn picking_a_song_drops_a_waiting_page_down_target(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = two_songs_live(cx, &dir);
    add_song(&mut cx, &operator, "Signal Hymn");
    add_song(&mut cx, &operator, SECOND_SONG);
    add_song(&mut cx, &operator, "Signal Hymn");
    let ids: Vec<_> = operator.read_with(&cx, |o, _| {
        o.schedule.entries().iter().map(|e| e.id).collect()
    });
    click(&mut cx, "schedule-item-0");
    settle(&mut cx, &operator, "first entry previewed", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.entry == Some(ids[0]) && p.resolved.is_some())
    });
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "first entry on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });

    // Page Down asks for entry 2; before its preview lands the operator
    // picks the hymn in Songs (one update, so no reply lands in between).
    let hymn = operator.read_with(&cx, |o, _| {
        o.songs()
            .iter()
            .position(|(_, t)| t == "Signal Hymn")
            .unwrap()
    });
    operator.update(&mut cx, |o, cx| {
        o.step_live_item(true);
        o.select_song(hymn, cx);
    });
    idle(&mut cx, &operator);
    assert_eq!(
        operator.read_with(&cx, |o, _| o.live.as_ref().unwrap().entry),
        Some(ids[0]),
        "the library pick cancelled the Page Down"
    );

    // The next Page Down goes from the live entry to entry 2, not entry 3.
    operator.update(&mut cx, |o, _| o.step_live_item(true));
    settle(&mut cx, &operator, "second entry on screen", |o| {
        o.on_screen() == Some("Quiet Canticle · Verse 1")
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| o.live.as_ref().unwrap().entry),
        Some(ids[1])
    );
}

/// SC2-I18N-01: the operator window title was English in every language.
#[gpui::test]
fn the_window_title_follows_the_ui_language(cx: &mut TestAppContext) {
    use sela::i18n::{self, Language};
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_songs(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    let title = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        operator.read_with(cx, |o, _| o.window_title.clone())
    };
    assert_eq!(title(&mut cx), "Sela — Technical preview");

    i18n::set_language(Language::Indonesian);
    operator.update(&mut cx, |_, cx| cx.notify());
    assert_eq!(title(&mut cx), "Sela — Pratinjau teknis");
    add_song(&mut cx, &operator, "Signal Hymn");
    operator.update(&mut cx, |_, cx| cx.notify());
    let shown = title(&mut cx);
    i18n::set_language(Language::English);
    assert_eq!(shown, "Jadwal tanpa judul* — Sela");
    operator.update(&mut cx, |_, cx| cx.notify());
    assert_eq!(title(&mut cx), "Untitled schedule* — Sela");
    assert_eq!(
        cx.window_title().as_deref(),
        Some("Untitled schedule* — Sela")
    );
}
