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

/// Show keys that must do nothing while a popup menu is open (SC2-FOCUS-02).
/// Enter is left out: in an open menu it chooses the focused item.
const SHOW_KEYS: [&str; 12] = [
    "pagedown",
    "b",
    ".",
    "ctrl-b",
    "ctrl-c",
    "down",
    "up",
    "home",
    "end",
    "right",
    "ctrl-delete",
    "ctrl-l",
];

/// Presses every show key and checks that none reached the schedule, the
/// masks or the audience while `open` still holds.
fn assert_show_keys_inert(
    cx: &mut VisualTestContext,
    operator: &Entity<Operator>,
    what: &str,
    open: fn(&Operator) -> bool,
) {
    let before = operator.read_with(cx, |o, _| {
        (
            submitted(o),
            o.schedule.entries().len(),
            o.live.as_ref().map(|l| l.entry),
            o.on_screen().map(str::to_owned),
        )
    });
    for keys in SHOW_KEYS {
        cx.simulate_keystrokes(keys);
        idle(cx, operator);
        operator.read_with(cx, |o, _| {
            assert!(open(o), "{keys} closed the {what}");
            assert!(!o.masks.any(), "{keys} toggled a mask under the {what}");
            let now = (
                submitted(o),
                o.schedule.entries().len(),
                o.live.as_ref().map(|l| l.entry),
                o.on_screen().map(str::to_owned),
            );
            assert_eq!(now, before, "{keys} acted under the {what}");
        });
    }
}

/// SC2-FOCUS-02: right-clicking a Schedule row opened its Duplicate/Remove
/// menu with the focus left on the row, so Page Down, B, Ctrl+B, the arrows
/// and Ctrl+Delete still drove the show (or removed the entry the menu
/// pointed at) and Esc did not close it, unlike the Songs menu.
#[gpui::test]
fn show_keys_stay_inert_while_the_schedule_item_menu_is_open(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = two_songs_live(cx, &dir);
    add_song(&mut cx, &operator, "Signal Hymn");
    add_song(&mut cx, &operator, SECOND_SONG);
    click(&mut cx, "schedule-item-0");
    settle(&mut cx, &operator, "first entry previewed", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.entry.is_some() && p.resolved.is_some())
    });
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "hymn on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });

    right_click(&mut cx, "schedule-item-1");
    cx.run_until_parked();
    assert!(operator.read_with(&cx, |o, _| o.item_menu.is_some()));
    assert_control(&mut cx, &operator, Control::ItemMenuDuplicate);
    assert_show_keys_inert(&mut cx, &operator, "item menu", |o| o.item_menu.is_some());

    // Tab and Up/Down stay inside the menu.
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, Control::ItemMenuRemove);
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, Control::ItemMenuDuplicate);
    cx.simulate_keystrokes("up");
    assert_control(&mut cx, &operator, Control::ItemMenuRemove);

    // Esc closes it with the focus back on the row; the keys work again.
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(operator.read_with(&cx, |o, _| o.item_menu.is_none()));
    cx.update(|w, cx| assert!(operator.read(cx).schedule_rows[1].is_focused(w)));
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black after the menu closed", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });

    // Remove from the menu: the dialog returns focus to the row.
    right_click(&mut cx, "schedule-item-1");
    cx.run_until_parked();
    cx.simulate_keystrokes("down enter");
    assert_eq!(
        operator.read_with(&cx, |o, _| o
            .dialog
            .clone()
            .map(|d| matches!(d, crate::operator::Dialog::Remove(_)))),
        Some(true)
    );
    click(&mut cx, "dialog-cancel");
    cx.update(|w, cx| assert!(operator.read(cx).schedule_rows[1].is_focused(w)));
}

/// SC2-FOCUS-02: the toolbar New menu and the Media image menu left every
/// show key live while open, and Esc did not close them.
#[gpui::test]
fn show_keys_stay_inert_while_the_new_and_image_menus_are_open(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let images = sela::images::images_dir(dir.path());
    std::fs::create_dir_all(&images).unwrap();
    std::fs::write(images.join("logo.png"), png(4, 2, [200, 10, 10, 255])).unwrap();
    let (mut cx, operator) = two_songs_live(cx, &dir);
    settle(&mut cx, &operator, "image list", |o| {
        o.images == ["logo.png"]
    });
    preview_song(&mut cx, &operator, "Signal Hymn");

    // New ▾ keeps the focus on its button.
    click(&mut cx, "new-menu");
    assert!(operator.read_with(&cx, |o, _| o.new_menu));
    assert_show_keys_inert(&mut cx, &operator, "New menu", |o| o.new_menu);
    cx.simulate_keystrokes("escape");
    assert!(operator.read_with(&cx, |o, _| !o.new_menu));
    assert_control(&mut cx, &operator, Control::NewMenu);
    // A press elsewhere closes it, and a press on New ▾ again closes it
    // without reopening it.
    click(&mut cx, "new-menu");
    click(&mut cx, "Live");
    assert!(operator.read_with(&cx, |o, _| !o.new_menu));
    click(&mut cx, "new-menu");
    click(&mut cx, "new-menu");
    assert!(operator.read_with(&cx, |o, _| !o.new_menu));
    click(&mut cx, "new-menu");
    assert!(operator.read_with(&cx, |o, _| o.new_menu));
    cx.simulate_keystrokes("escape");

    // The image row menu takes the focus like the Schedule item menu.
    click(&mut cx, "Media");
    right_click(&mut cx, "image-0");
    cx.run_until_parked();
    assert!(operator.read_with(&cx, |o, _| o.image_menu.is_some()));
    assert_control(&mut cx, &operator, Control::ImageMenuLogo);
    assert_show_keys_inert(&mut cx, &operator, "image menu", |o| o.image_menu.is_some());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(operator.read_with(&cx, |o, _| o.image_menu.is_none()));
    cx.update(|w, cx| assert!(operator.read(cx).image_rows[0].is_focused(w)));
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black after the menus closed", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });
}

/// SC2-LIVE-04: Normalize re-sent only the acknowledged `live_slide`. Between
/// another item's Go Live and its acknowledgment that is None (SC2-LIVE-01),
/// so the toggle re-sent nothing and the new item's slide landed with the
/// old sizing until the next navigation.
#[gpui::test]
fn normalize_applies_to_a_go_live_still_in_preparation(cx: &mut TestAppContext) {
    use sela::slides::Sizing;
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = two_songs_live(cx, &dir);
    preview_song(&mut cx, &operator, "Signal Hymn");
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "hymn on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });

    preview_song(&mut cx, &operator, SECOND_SONG);
    let (open, preparer) = gated_preparer(Default::default());
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(preparer));
    click(&mut cx, "go-live");
    assert_eq!(operator.read_with(&cx, |o, _| o.pending_cues.len()), 1);
    click(&mut cx, "normalize-text");
    assert_eq!(
        operator.read_with(&cx, |o, _| (o.sizing, o.pending_cues.len())),
        (Sizing::Normalized, 2),
        "the canticle's cue is prepared again with the new sizing"
    );
    open.send(()).unwrap();
    open.send(()).unwrap();
    settle(&mut cx, &operator, "canticle on screen", |o| {
        o.pending_cues.is_empty() && o.on_screen() == Some("Quiet Canticle · Verse 1")
    });
    operator.read_with(&cx, |o, _| {
        let (item, _, sizing, _) = o.size_cap.unwrap();
        assert_eq!(item, o.live.as_ref().unwrap().version);
        assert_eq!(
            sizing,
            Sizing::Normalized,
            "the slide on screen is normalized"
        );
        assert_eq!(o.live_slide, Some(0));
    });
}
