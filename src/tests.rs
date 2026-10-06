use super::*;
use crate::operator::{LIVE_OUTPUT, Launcher, NEXT};
use gpui::{Entity, TestAppContext, VisualTestContext};
use sela::{
    arrangement::SectionId,
    delivery::{DeliveryError, Epoch, LiveState, Outcome, RendererSession},
    output::{Launch, Status, Stream},
    scene::{Extent, RendererCapabilities},
    storage::{Repository, Section, Song},
    transport::Frame,
};
use std::{
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

const FAKE_EXTENT: Extent = Extent {
    width: 640,
    height: 360,
};

/// Renderer protocol stand-in: Ready, one surface report, then Accepted and
/// Applied (or a renderer failure in `reject` mode) for every cue. No GPU.
#[test]
#[ignore = "subprocess fixture for operator live-output tests"]
fn fake_audience_child() {
    let mode = std::env::var("SELA_FAKE_AUDIENCE").unwrap();
    let epoch =
        Epoch(u128::from_str_radix(&std::env::var("SELA_FAKE_EPOCH").unwrap(), 16).unwrap());
    let caps = RendererCapabilities {
        max_texture_dimension: 2048,
    };
    let out = || std::io::stderr();
    Frame::ready(epoch, 2048).write(out()).unwrap();
    Frame::surface(epoch, FAKE_EXTENT).write(out()).unwrap();
    if mode == "exit" {
        std::process::exit(0);
    }
    let mut session = RendererSession::new(epoch);
    while let Ok(frame) = Frame::read(std::io::stdin()) {
        let now = Instant::now();
        let stamp = frame.command_stamp().unwrap();
        let ack = match frame.into_command(now, caps) {
            Ok(command) => session.accept(command, now),
            Err(_) => session.reject_preparation(stamp),
        };
        Frame::acknowledgment(ack).write(out()).unwrap();
        if ack.outcome == Outcome::Accepted {
            let ack = session
                .present(Instant::now(), |_| {
                    if mode == "reject" {
                        Err(DeliveryError::RenderFailed)
                    } else {
                        Ok(())
                    }
                })
                .unwrap();
            Frame::acknowledgment(ack).write(out()).unwrap();
        }
    }
    std::process::exit(0);
}

fn fake_launcher(mode: &'static str) -> Launcher {
    Arc::new(move |epoch| {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "tests::fake_audience_child",
                "--ignored",
                "--nocapture",
            ])
            .env("SELA_FAKE_AUDIENCE", mode)
            .env("SELA_FAKE_EPOCH", format!("{:x}", epoch.0));
        Launch {
            command,
            frames: Stream::Stderr,
        }
    })
}

// Count routed Quit actions without closing the test window, so subsequent
// input can still be tested. Actual close/termination is checked natively.
struct DispatchProbe {
    operator: Entity<Operator>,
    seen: Rc<Cell<usize>>,
}

impl Render for DispatchProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let seen = self.seen.clone();
        div()
            .size_full()
            .capture_action(move |_: &Quit, _, cx| {
                seen.set(seen.get() + 1);
                cx.stop_propagation();
            })
            .child(self.operator.clone())
    }
}

fn fixture(cx: &mut TestAppContext) -> (VisualTestContext, Entity<Operator>, Rc<Cell<usize>>) {
    with_library(cx, None, "apply")
}

fn with_library(
    cx: &mut TestAppContext,
    library: Option<std::path::PathBuf>,
    mode: &'static str,
) -> (VisualTestContext, Entity<Operator>, Rc<Cell<usize>>) {
    let seen = Rc::new(Cell::new(0));
    let window = cx.update(|cx| {
        bind_operator_keys(cx);
        cx.open_window(Default::default(), |window, cx| {
            let operator = cx.new(|cx| Operator::new(library, fake_launcher(mode), cx));
            operator.read(cx).focus.clone().focus(window, cx);
            cx.new(|_| DispatchProbe {
                operator,
                seen: seen.clone(),
            })
        })
        .unwrap()
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let operator = window
        .root(&mut visual)
        .unwrap()
        .read_with(&visual, |p, _| p.operator.clone());
    (visual, operator, seen)
}

#[gpui::test]
fn focused_operator_resolves_quit_input(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    cx.update(|window, cx| {
        assert!(operator.read(cx).focus.is_focused(window));
        assert!(window.is_action_available(&Quit, cx));
    });
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 1, "actual Sela key context resolves Quit");
}

#[gpui::test]
fn unbound_and_unfocused_input_do_not_dispatch(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    cx.simulate_keystrokes("ctrl-j");
    assert_eq!(seen.get(), 0);
    cx.update(|window, cx| window.blur(cx));
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 0, "no focused Sela context");
    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 1, "restored focus restores contextual binding");
}

#[gpui::test]
fn wrong_context_rejects_key_but_direct_action_bypasses_binding(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    cx.update(|_, cx| {
        cx.clear_key_bindings();
        cx.bind_keys([KeyBinding::new("ctrl-q", Quit, Some("NotSela"))]);
    });
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 0);
    cx.update(|window, cx| {
        operator
            .read(cx)
            .focus
            .clone()
            .dispatch_action(&Quit, window, cx);
    });
    assert_eq!(seen.get(), 1, "direct dispatch is not key-context E2E");
}

#[gpui::test]
fn shell_controls_and_resize(cx: &mut TestAppContext) {
    let (mut cx, operator, _) = fixture(cx);
    cx.simulate_resize(size(px(1280.), px(800.)));
    for (index, tab) in ["Songs", "Scriptures", "Media", "Presentations", "Themes"]
        .into_iter()
        .enumerate()
    {
        let bounds = cx.debug_bounds(tab).unwrap();
        cx.simulate_click(bounds.center(), Default::default());
        assert_eq!(operator.read_with(&cx, |o, _| o.tab), index);
    }
    let bounds = cx.debug_bounds("collapse-resources").unwrap();
    cx.simulate_click(bounds.center(), Default::default());
    assert!(operator.read_with(&cx, |o, _| o.collapsed));
    assert!(cx.debug_bounds("library-detail").is_none());
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dimensions(1280., 800.))[3],
        730.
    );
    let bounds = cx.debug_bounds("collapse-resources").unwrap();
    cx.simulate_click(bounds.center(), Default::default());
    assert!(!operator.read_with(&cx, |o, _| o.collapsed));
    cx.simulate_resize(size(px(720.), px(440.)));
    for selector in ["Schedule", "Preview", "Live", "library-detail"] {
        let b = cx.debug_bounds(selector).unwrap();
        assert!(b.size.width > px(0.) && b.size.height > px(0.));
        assert!(b.right() <= px(720.) && b.bottom() <= px(440.));
    }
    cx.update(|window, cx| {
        assert!(operator.read(cx).controls[2].is_focused(window));
        assert!(operator.read(cx).focus.contains_focused(window, cx));
    });
}

#[gpui::test]
fn splitters_bound_release_and_reset(cx: &mut TestAppContext) {
    use gpui::{MouseButton, point};
    let (mut cx, operator, _) = fixture(cx);
    cx.simulate_resize(size(px(1280.), px(800.)));
    for selector in ["split-schedule", "split-live", "split-resources"] {
        let old = operator.read_with(&cx, |o, _| o.ratios);
        let start = cx.debug_bounds(selector).unwrap().center();
        cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
        cx.simulate_mouse_move(
            start + point(px(15.), px(15.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_move(
            point(px(3000.), px(3000.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_up(
            point(px(3000.), px(3000.)),
            MouseButton::Left,
            Default::default(),
        );
        let before = operator.read_with(&cx, |o, _| o.ratios);
        if selector == "split-resources" {
            assert_eq!(&old[..2], &before[..2]);
        } else {
            assert_eq!(old[2], before[2], "vertical drag must not resize resources");
        }
        cx.simulate_mouse_move(point(px(0.), px(0.)), None, Default::default());
        assert_eq!(before, operator.read_with(&cx, |o, _| o.ratios));
        let d = operator.read_with(&cx, |o, _| o.dimensions(1280., 800.));
        assert!(d[..3].iter().all(|v| *v >= 149.99));
        assert!(d[3] >= 140. && d[3] <= 584.);
        cx.update(|window, cx| assert!(operator.read(cx).focus.is_focused(window)));
    }
    assert_ne!(operator.read_with(&cx, |o, _| o.ratios), [0.24, 0.62, 0.62]);
    let b = cx.debug_bounds("reset-layout").unwrap();
    cx.simulate_click(b.center(), Default::default());
    cx.update(|window, cx| assert!(operator.read(cx).controls[0].is_focused(window)));
    assert_eq!(operator.read_with(&cx, |o, _| o.ratios), [0.24, 0.62, 0.62]);
    for (selector, dimension, minimum) in [
        ("split-schedule", 0, 150.),
        ("split-live", 1, 150.),
        ("split-resources", 3, 140.),
    ] {
        let start = cx.debug_bounds(selector).unwrap().center();
        cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
        cx.simulate_mouse_move(
            start - point(px(15.), px(15.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_move(
            point(px(-3000.), px(-3000.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_up(
            point(px(-3000.), px(-3000.)),
            MouseButton::Left,
            Default::default(),
        );
        let d = operator.read_with(&cx, |o, _| o.dimensions(1280., 800.));
        assert!((d[dimension] - minimum).abs() < 0.01);
        cx.update(|window, cx| {
            assert!(operator.read(cx).controls[0].is_focused(window));
            assert!(operator.read(cx).focus.contains_focused(window, cx));
        });
    }
    for (w, h) in [(0., 0.), (10., 10.), (720., 440.), (1280., 800.)] {
        let d = operator.read_with(&cx, |o, _| o.dimensions(w, h));
        assert!(d.iter().all(|v| v.is_finite() && *v >= 0.));
        assert!((d[0] + d[1] + d[2] - (w - 12.).max(0.)).abs() < 0.01);
    }
}

fn assert_control(cx: &mut VisualTestContext, operator: &Entity<Operator>, index: usize) {
    cx.update(|window, cx| {
        assert!(
            operator.read(cx).controls[index].is_focused(window),
            "control {index}"
        );
        assert!(operator.read(cx).focus.contains_focused(window, cx));
    });
}

#[gpui::test]
fn keyboard_traversal_and_activation(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    // Control 10 exists only while the New menu is open.
    let order: Vec<usize> = (0..10).chain(LIVE_OUTPUT..=NEXT).collect();
    for index in order.iter().copied() {
        cx.simulate_keystrokes("tab");
        assert_control(&mut cx, &operator, index);
        assert_eq!(operator.read_with(&cx, |o, _| o.tab), 0);
    }
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, 0);
    for index in order.iter().copied().rev() {
        cx.simulate_keystrokes("shift-tab");
        assert_control(&mut cx, &operator, index);
    }
    cx.simulate_keystrokes("tab tab tab tab");
    assert_control(&mut cx, &operator, 4);
    cx.simulate_keystrokes("enter");
    assert_eq!(operator.read_with(&cx, |o, _| o.tab), 1);
    cx.simulate_keystrokes("tab space");
    assert_eq!(operator.read_with(&cx, |o, _| o.tab), 2);
    let schedule = cx.debug_bounds("Schedule").unwrap();
    let ratios = operator.read_with(&cx, |o, _| o.ratios);
    cx.simulate_keystrokes("ctrl-j down left shift-enter shift-space");
    assert_control(&mut cx, &operator, 5);
    assert_eq!(operator.read_with(&cx, |o, _| o.tab), 2);
    assert_eq!(operator.read_with(&cx, |o, _| o.ratios), ratios);
    assert_eq!(cx.debug_bounds("Schedule").unwrap(), schedule);
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 1, "root Quit routes from a child context");
    cx.simulate_keystrokes("shift-tab shift-tab shift-tab shift-tab enter space");
    assert_control(&mut cx, &operator, 1);
    assert_eq!(
        seen.get(),
        3,
        "each focused Quit activation dispatches once"
    );
    cx.update(|_, cx| cx.clear_key_bindings());
    cx.simulate_keystrokes("enter space tab ctrl-q");
    assert_control(&mut cx, &operator, 1);
    assert_eq!(
        seen.get(),
        3,
        "unbound keys must not bypass semantic actions"
    );
}

#[gpui::test]
fn new_menu_is_keyboard_accessible_without_reflowing_panes(cx: &mut TestAppContext) {
    let (mut cx, operator, _) = fixture(cx);
    cx.simulate_resize(size(px(1280.), px(800.)));
    let schedule = cx.debug_bounds("Schedule").unwrap();
    let live = cx.debug_bounds("Live").unwrap();
    cx.update(|w, cx| operator.read(cx).controls[9].clone().focus(w, cx));
    cx.simulate_keystrokes("enter");
    assert!(cx.debug_bounds("new-song-menu").is_some());
    assert_eq!(cx.debug_bounds("Schedule").unwrap(), schedule);
    assert_eq!(cx.debug_bounds("Live").unwrap(), live);
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, 10);
    cx.simulate_keystrokes("shift-tab space");
    assert_control(&mut cx, &operator, 9);
    assert!(cx.debug_bounds("new-song-menu").is_none());
    assert_eq!(cx.debug_bounds("Schedule").unwrap(), schedule);
}

#[gpui::test]
fn collapsed_traversal_reset_and_rejection(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    cx.simulate_resize(size(px(1280.), px(800.)));
    let original = operator.read_with(&cx, |o, _| (o.tab, o.ratios, o.collapsed));
    cx.simulate_keystrokes("enter space ctrl-j down left");
    assert_eq!(
        original,
        operator.read_with(&cx, |o, _| (o.tab, o.ratios, o.collapsed))
    );
    cx.simulate_keystrokes("tab tab tab enter");
    assert!(operator.read_with(&cx, |o, _| o.collapsed));
    assert_control(&mut cx, &operator, 2);
    assert!(cx.debug_bounds("Songs").is_none());
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, 9);
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, LIVE_OUTPUT);
    cx.simulate_keystrokes("shift-tab shift-tab space");
    assert_control(&mut cx, &operator, 2);
    assert!(!operator.read_with(&cx, |o, _| o.collapsed));
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, 3);
    let b = cx.debug_bounds("collapse-resources").unwrap();
    cx.simulate_click(b.center(), Default::default());
    assert_control(&mut cx, &operator, 2);
    // 2 → 9 → Live output, Go Live, Previous, Next → 0.
    cx.simulate_keystrokes("tab tab tab tab tab tab enter");
    assert_control(&mut cx, &operator, 0);
    assert!(!operator.read_with(&cx, |o, _| o.collapsed));
    assert!(operator.read_with(&cx, |o, _| o.output.is_none()));
    cx.update(|window, cx| window.blur(cx));
    cx.simulate_keystrokes("tab enter space ctrl-q");
    cx.update(|window, cx| assert!(!operator.read(cx).focus.contains_focused(window, cx)));
    assert_eq!(seen.get(), 0);
    assert_eq!(
        original,
        operator.read_with(&cx, |o, _| (o.tab, o.ratios, o.collapsed))
    );
    // Unavailable mask/alert labels have neither focus handles nor activation routes.
    for label in ["Alerts", "Black", "Clear", "Logo"] {
        let bounds = cx.debug_bounds(label).unwrap();
        cx.simulate_click(bounds.center(), Default::default());
        cx.update(|window, cx| {
            assert!(operator.read(cx).focus.is_focused(window));
            assert!(!window.is_action_available(&ActivateControl, cx));
        });
        cx.simulate_keystrokes("enter space");
    }
    assert_eq!(
        original,
        operator.read_with(&cx, |o, _| (o.tab, o.ratios, o.collapsed))
    );
    assert_eq!(seen.get(), 0);
}

fn library_with_song(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("library.sqlite");
    let mut repository = Repository::open(&path).unwrap();
    let section = |label: &str, lyrics: &str| Section {
        id: SectionId::allocate(),
        label: label.into(),
        lyrics: lyrics.into(),
    };
    repository
        .save_song(
            None,
            Song {
                title: "Signal Hymn".into(),
                authors: String::new(),
                copyright: String::new(),
                license: String::new(),
                variants: Vec::new(),
                sections: vec![
                    section("Verse 1", "First original line\nSecond original line"),
                    section("Chorus", "Original refrain"),
                    section("Bridge", "Original bridge"),
                ],
            },
        )
        .unwrap();
    path
}

/// Real storage worker and audience child threads; GPUI timers are not
/// advanced, so poll explicitly.
fn settle(
    cx: &mut VisualTestContext,
    operator: &Entity<Operator>,
    what: &str,
    done: impl Fn(&Operator) -> bool,
) {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        operator.update(cx, |o, cx| o.poll(cx));
        cx.run_until_parked();
        if operator.read_with(cx, |o, _| done(o)) {
            return;
        }
        assert!(Instant::now() < end, "bounded wait for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is rendered"));
    cx.simulate_click(bounds.center(), Default::default());
}

fn connected(o: &Operator) -> bool {
    matches!(
        o.output.as_ref().map(|output| output.status()),
        Some(Status::Connected { .. })
    )
}

fn submitted(o: &Operator) -> u64 {
    o.output
        .as_ref()
        .map_or(0, |output| output.counters().submitted)
}

#[gpui::test]
fn go_live_requires_preview_and_output(cx: &mut TestAppContext) {
    let (mut cx, operator, _) = fixture(cx);
    cx.simulate_resize(size(px(1280.), px(800.)));
    click(&mut cx, "go-live");
    assert_control(&mut cx, &operator, crate::operator::GO_LIVE);
    assert_eq!(
        operator.read_with(&cx, |o, _| o.live_message.clone()),
        Some("Select a song to preview first".into())
    );
    click(&mut cx, "live-next");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.live_message.clone()),
        Some("Nothing is live".into())
    );
    assert!(operator.read_with(&cx, |o, _| o.live.is_none() && o.output.is_none()));
    assert_eq!(
        operator.read_with(&cx, |o, _| o.output_line()),
        "Live output off"
    );
}

#[gpui::test]
fn go_live_shows_renderer_acknowledged_slide(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_song(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "preview", |o| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 3)
    });
    click(&mut cx, "go-live");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.live_message.clone()),
        Some("Turn on Live output first".into())
    );
    assert!(operator.read_with(&cx, |o, _| o.live.is_none()));

    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    assert_eq!(
        operator.read_with(&cx, |o, _| o.output_line()),
        "Live output 640×360"
    );
    click(&mut cx, "preview-slide-1");
    cx.run_until_parked();
    assert_eq!(operator.read_with(&cx, |o, _| o.preview_slide), 1);
    assert_eq!(
        operator.read_with(&cx, |o, _| submitted(o)),
        0,
        "preview selection never reaches the audience"
    );
    assert!(cx.debug_bounds("live-slide-0").is_none());

    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "chorus on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });
    assert!(cx.debug_bounds("live-slide-2").is_some());
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "bridge on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Bridge")
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    click(&mut cx, "live-next");
    cx.run_until_parked();
    assert_eq!(
        operator.read_with(&cx, |o, _| submitted(o)),
        sent,
        "last slide"
    );
    assert_eq!(operator.read_with(&cx, |o, _| o.live_slide), Some(2));
    click(&mut cx, "live-previous");
    settle(&mut cx, &operator, "chorus again", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });
    assert_eq!(operator.read_with(&cx, |o, _| o.preview_slide), 1);

    click(&mut cx, "live-output");
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert!(o.output.is_none());
        assert_eq!(o.on_screen(), None);
        assert_eq!(o.live_slide, None, "a new session never replays");
        assert!(o.live.is_some());
    });
}

#[gpui::test]
fn rejected_or_lost_output_is_never_shown_as_live(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_song(&dir);
    let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "reject");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "preview", |o| o.preview.is_some());
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "rejection", |o| {
        o.output
            .as_ref()
            .is_some_and(|out| out.rejected().is_some())
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.on_screen(), None);
        assert_eq!(
            o.output.as_ref().unwrap().live(),
            LiveState::Unknown,
            "nothing was ever confirmed"
        );
    });

    let (mut cx, operator, _) = with_library(&mut cx.cx, Some(path), "exit");
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "loss", |o| {
        matches!(
            o.output.as_ref().map(|out| out.status()),
            Some(Status::Lost(_))
        )
    });
    assert!(
        operator
            .read_with(&cx, |o, _| o.output_line())
            .contains("output state unknown")
    );
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "preview", |o| o.preview.is_some());
    click(&mut cx, "go-live");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.live_message.clone()),
        Some("Output state unknown · turn Live off and on".into())
    );
    assert!(operator.read_with(&cx, |o, _| o.live.is_none()));
}
