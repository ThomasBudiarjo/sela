use super::*;
use crate::operator::{
    CLEAR, Indicator, LIVE_OUTPUT, LOGO, Launcher, NEXT, NORMALIZE, OPEN_SCHEDULE, REMOVE_ITEM,
};
use gpui::{Entity, Focusable, TestAppContext, VisualTestContext};
use sela::{
    arrangement::SectionId,
    delivery::{DeliveryError, Epoch, LiveState, Outcome, Payload, RendererSession},
    masks::{Layer, Mask},
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
        let lane = frame.command_lane().unwrap();
        let ack = match frame.into_command(now, caps) {
            Ok(command) => session.accept(command, now),
            Err(_) => session.reject_preparation(stamp, lane),
        };
        Frame::acknowledgment(ack).write(out()).unwrap();
        if ack.outcome == Outcome::Accepted {
            let has_logo = session.logo().is_some();
            let ack = session
                .present(Instant::now(), |payload| match payload {
                    // Mirrors the renderer: a Logo mask needs a logo to draw.
                    Payload::Mask(Layer::Logo) if !has_logo => Err(DeliveryError::RenderFailed),
                    Payload::Mask(_) | Payload::Logo(_) => Ok(()),
                    Payload::Scene(_) if mode == "reject" => Err(DeliveryError::RenderFailed),
                    Payload::Scene(_) => Ok(()),
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
    let order: Vec<usize> = (0..10)
        .chain(LIVE_OUTPUT..=NEXT)
        .chain(LOGO..=CLEAR)
        .chain([NORMALIZE])
        .chain(OPEN_SCHEDULE..=REMOVE_ITEM)
        .collect();
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
    // 2 → 9 → Live output, Go Live, Previous, Next → Logo, Black, Clear →
    // Open, Save → schedule Up, Down, Remove → 0.
    cx.simulate_keystrokes("tab tab tab tab tab tab tab tab tab tab tab tab tab tab enter");
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
    // The unavailable Alerts label has neither a focus handle nor an activation route.
    let bounds = cx.debug_bounds("Alerts").unwrap();
    cx.simulate_click(bounds.center(), Default::default());
    cx.update(|window, cx| {
        assert!(operator.read(cx).focus.is_focused(window));
        assert!(!window.is_action_available(&ActivateControl, cx));
    });
    cx.simulate_keystrokes("enter space");
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
        format: Default::default(),
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

fn confirmed_mask(o: &Operator) -> Option<Layer> {
    o.output.as_ref().and_then(|output| output.mask())
}

fn has_line(o: &Operator, text: &str) -> bool {
    o.live_lines().iter().any(|(line, _)| line == text)
}

#[gpui::test]
fn show_keys_toggle_masks_with_acknowledged_indicators(cx: &mut TestAppContext) {
    let (mut cx, operator, _) = fixture(cx);
    cx.simulate_resize(size(px(1280.), px(800.)));
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    cx.simulate_keystrokes("ctrl-b");
    operator.read_with(&cx, |o, _| {
        assert!(o.masks.is_on(Mask::Black));
        assert_eq!(
            o.indicator(Mask::Black),
            Indicator::Pending,
            "sent is not shown"
        );
    });
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        o.indicator(Mask::Black) == Indicator::On
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
        assert!(has_line(o, "Mask: Black"));
        assert!(has_line(o, "Under mask: nothing confirmed"));
    });
    // Clear stacks under Black; both lit once the renderer has the layer.
    cx.simulate_keystrokes("ctrl-c");
    settle(&mut cx, &operator, "Clear under Black", |o| {
        o.indicator(Mask::Clear) == Indicator::On && o.indicator(Mask::Black) == Indicator::On
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Clear revealed", |o| {
        confirmed_mask(o) == Some(Layer::Clear) && o.indicator(Mask::Black) == Indicator::Off
    });
    assert!(operator.read_with(&cx, |o, _| has_line(o, "Mask: Clear")));
    // No logo is set: Logo refuses and explains, leaving masks as they are.
    cx.simulate_keystrokes("ctrl-l");
    operator.read_with(&cx, |o, _| {
        assert!(!o.masks.is_on(Mask::Logo));
        assert!(o.masks.is_on(Mask::Clear));
        assert!(
            o.live_message
                .as_deref()
                .unwrap()
                .starts_with("No logo set")
        );
    });
    click(&mut cx, "mask-clear");
    settle(&mut cx, &operator, "unmasked", |o| {
        confirmed_mask(o) == Some(Layer::None) && o.indicator(Mask::Clear) == Indicator::Off
    });
    assert!(operator.read_with(&cx, |o, _| has_line(o, "On screen: nothing confirmed")));

    // EW8-OBS-017: Live off keeps the mask armed; Live on shows it again.
    click(&mut cx, "mask-black");
    settle(&mut cx, &operator, "Black", |o| {
        o.indicator(Mask::Black) == Indicator::On
    });
    click(&mut cx, "live-output");
    operator.read_with(&cx, |o, _| {
        assert!(o.output.is_none());
        assert!(o.masks.is_on(Mask::Black));
        assert_eq!(o.indicator(Mask::Black), Indicator::Pending);
        assert!(has_line(
            o,
            "Mask armed: Black · shown when Live output is on"
        ));
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "Black in the new session", |o| {
        o.indicator(Mask::Black) == Indicator::On
    });
}

/// Mouse double-click as the platform reports it: two presses, counts 1 and 2.
fn double_click(cx: &mut VisualTestContext, selector: &'static str) {
    use gpui::{MouseButton, MouseDownEvent, MouseUpEvent};
    let position = cx.debug_bounds(selector).unwrap().center();
    for click_count in [1, 2] {
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count,
        });
    }
}

#[gpui::test]
fn live_slide_clicks_apply_and_double_click_unmasks(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_song(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "preview", |o| o.preview.is_some());
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });
    // Go Live and Live slide clicks keep the mask (EW8-OBS-016).
    click(&mut cx, "live-slide-1");
    settle(&mut cx, &operator, "chorus under Black", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
        assert!(has_line(o, "Under mask: Signal Hymn · Chorus"));
    });
    double_click(&mut cx, "live-slide-2");
    settle(&mut cx, &operator, "bridge unmasked", |o| {
        o.on_screen() == Some("Signal Hymn · Bridge") && confirmed_mask(o) == Some(Layer::None)
    });
    operator.read_with(&cx, |o, _| {
        assert!(!o.masks.any());
        assert!(has_line(o, "On screen: Signal Hymn · Bridge"));
    });
}

#[gpui::test]
fn normalize_text_size_applies_to_the_live_slide(cx: &mut TestAppContext) {
    use sela::slides::{Sizing, size_cap};
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_song(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "preview", |o| o.preview.is_some());
    // Without a live slide the toggle only changes the setting.
    click(&mut cx, "normalize-text");
    assert_eq!(
        operator.read_with(&cx, |o, _| (o.sizing, submitted(o))),
        (Sizing::Normalized, 0)
    );
    click(&mut cx, "normalize-text");
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "preview-slide-1");
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "chorus on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });
    let (version, slides, sent) = operator.read_with(&cx, |o, _| {
        let item = o.live.as_ref().unwrap();
        assert_eq!(
            o.size_cap,
            Some((item.version, FAKE_EXTENT, Sizing::PerSlide, None))
        );
        (item.version, item.slides.clone(), submitted(o))
    });
    let cap = {
        let resolved: Vec<_> = slides
            .iter()
            .map(|slide| sela::fonts::Resolved::bundled(&slide.format))
            .collect();
        size_cap(&slides, &resolved, FAKE_EXTENT, Sizing::Normalized)
    };
    assert!(cap.is_some());

    click(&mut cx, "normalize-text");
    settle(&mut cx, &operator, "chorus resent", |o| {
        submitted(o) == sent + 1 && o.on_screen() == Some("Signal Hymn · Chorus")
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.sizing, Sizing::Normalized);
        assert_eq!(
            o.size_cap,
            Some((version, FAKE_EXTENT, Sizing::Normalized, cap))
        );
        assert_eq!(o.live_slide, Some(1));
    });
    // Collapsed Resources hide the toggle; its keyboard route is inert too.
    click(&mut cx, "collapse-resources");
    assert!(cx.debug_bounds("normalize-text").is_none());
    cx.update(|window, cx| {
        operator.read(cx).controls[NORMALIZE]
            .clone()
            .focus(window, cx)
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(operator.read_with(&cx, |o, _| o.sizing), Sizing::Normalized);
}

#[gpui::test]
fn show_keys_stay_out_of_text_fields(cx: &mut TestAppContext) {
    struct Probe {
        root: FocusHandle,
        field: FocusHandle,
        seen: Rc<Cell<usize>>,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let seen = self.seen.clone();
            div()
                .key_context("Sela SelaShow")
                .track_focus(&self.root)
                .on_action(move |_: &ToggleClear, _, _| seen.set(seen.get() + 1))
                .size_full()
                .child(
                    div()
                        .key_context("SelaTextInput")
                        .track_focus(&self.field)
                        .size_full(),
                )
        }
    }
    let seen = Rc::new(Cell::new(0));
    let window = cx.update(|cx| {
        bind_operator_keys(cx);
        cx.open_window(Default::default(), |_, cx| {
            cx.new(|cx| Probe {
                root: cx.focus_handle(),
                field: cx.focus_handle(),
                seen: seen.clone(),
            })
        })
        .unwrap()
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let probe = window.root(&mut cx).unwrap();
    cx.update(|window, cx| probe.read(cx).field.clone().focus(window, cx));
    cx.simulate_keystrokes("ctrl-c ctrl-b ctrl-l pagedown");
    assert_eq!(seen.get(), 0, "text fields keep Ctrl+C");
    cx.update(|window, cx| probe.read(cx).root.clone().focus(window, cx));
    cx.simulate_keystrokes("ctrl-c");
    assert_eq!(seen.get(), 1);
}

fn png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    use image::ImageEncoder;
    let pixels: Vec<u8> = (0..width * height).flat_map(|_| rgba).collect();
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded)
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .unwrap();
    encoded
}

#[gpui::test]
fn media_logo_is_imported_persisted_and_shown(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_song(&dir);
    let images = sela::images::images_dir(dir.path());
    std::fs::create_dir_all(&images).unwrap();
    std::fs::write(images.join("logo.png"), png(4, 2, [200, 10, 10, 255])).unwrap();
    let outside = dir.path().join("second.png");
    std::fs::write(&outside, png(2, 2, [10, 200, 10, 255])).unwrap();

    let (mut cx, operator, _) = with_library(cx, Some(library.clone()), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "image list", |o| {
        o.images == ["logo.png"]
    });
    assert!(operator.read_with(&cx, |o, _| o.logo.is_none()));
    click(&mut cx, "Media");
    click(&mut cx, "image-0");
    click(&mut cx, "use-as-logo");
    settle(&mut cx, &operator, "logo chosen", |o| o.logo.is_some());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("logo.txt")).unwrap(),
        "logo.png\n"
    );

    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("ctrl-l");
    settle(&mut cx, &operator, "logo shown", |o| {
        confirmed_mask(o) == Some(Layer::Logo) && o.indicator(Mask::Logo) == Indicator::On
    });
    operator.read_with(&cx, |o, _| {
        let output = o.output.as_ref().unwrap();
        let version =
            sela::images::logo_version(o.logo.as_ref().unwrap().resource.version, FAKE_EXTENT);
        assert_eq!(output.logo(), Some(version));
        assert_eq!(
            output.mask_rejected(),
            None,
            "Logo never sent before its logo"
        );
        assert!(has_line(o, "Mask: Logo"));
    });

    click(&mut cx, "import-image");
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|_| Some(vec![outside.clone()]));
    settle(&mut cx, &operator, "imported", |o| o.images.len() == 2);
    assert_eq!(
        operator.read_with(&cx, |o, _| o.selected_image.clone()),
        Some("second.png".into())
    );

    // A fresh operator reads the stored choice.
    let (mut cx, operator, _) = with_library(&mut cx.cx, Some(library), "apply");
    settle(&mut cx, &operator, "stored logo", |o| {
        o.logo.as_ref().is_some_and(|logo| logo.name == "logo.png")
    });
}

const SECOND_SONG: &str = "Quiet Canticle";

fn library_with_songs(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = library_with_song(dir);
    Repository::open(&path)
        .unwrap()
        .save_song(
            None,
            Song {
                title: SECOND_SONG.into(),
                authors: String::new(),
                copyright: String::new(),
                license: String::new(),
                variants: Vec::new(),
                sections: vec![Section {
                    id: SectionId::allocate(),
                    label: "Verse 1".into(),
                    lyrics: "Quiet original line".into(),
                    format: Default::default(),
                }],
            },
        )
        .unwrap();
    path
}

/// Catalog order follows random song IDs; find rows by title.
fn song_row(cx: &mut VisualTestContext, operator: &Entity<Operator>, title: &str) -> &'static str {
    let index = operator
        .read_with(cx, |o, _| o.catalog.iter().position(|(_, t)| t == title))
        .unwrap();
    ["song-0", "song-1"][index]
}

fn entry_titles(o: &Operator) -> Vec<String> {
    o.schedule
        .entries()
        .iter()
        .map(|e| e.title.clone())
        .collect()
}

fn previewed(o: &Operator) -> Option<(Option<sela::schedule::EntryId>, &str)> {
    o.preview
        .as_ref()
        .map(|item| (item.entry, item.title.as_str()))
}

fn add_song(cx: &mut VisualTestContext, operator: &Entity<Operator>, title: &str) {
    let row = song_row(cx, operator, title);
    click(cx, row);
    click(cx, "add-to-schedule");
}

#[gpui::test]
fn schedule_add_reorder_select_navigate_and_remove(cx: &mut TestAppContext) {
    use crate::operator::Dialog;
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_songs(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    click(&mut cx, "add-to-schedule");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.schedule_message.clone()),
        Some("Select a song in Songs first".into())
    );
    add_song(&mut cx, &operator, "Signal Hymn");
    click(&mut cx, "add-to-schedule");
    add_song(&mut cx, &operator, SECOND_SONG);
    let ids: Vec<_> = operator.read_with(&cx, |o, _| {
        assert_eq!(entry_titles(o), ["Signal Hymn", "Signal Hymn", SECOND_SONG]);
        assert!(o.schedule.is_dirty());
        o.schedule.entries().iter().map(|e| e.id).collect()
    });
    assert_ne!(ids[0], ids[1], "duplicates are separate entries");

    // Down/Up select schedule items and preview them; never Live.
    cx.simulate_keystrokes("down");
    settle(&mut cx, &operator, "first item previewed", |o| {
        previewed(o) == Some((Some(ids[0]), "Signal Hymn"))
    });
    cx.simulate_keystrokes("down down");
    settle(&mut cx, &operator, "third item previewed", |o| {
        previewed(o) == Some((Some(ids[2]), SECOND_SONG))
    });
    cx.simulate_keystrokes("down");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.selected_entry),
        Some(ids[2])
    );
    cx.simulate_keystrokes("up");
    settle(&mut cx, &operator, "second item previewed", |o| {
        previewed(o) == Some((Some(ids[1]), "Signal Hymn"))
    });
    assert!(operator.read_with(&cx, |o, _| o.output.is_none() && o.live.is_none()));

    click(&mut cx, "schedule-up");
    click(&mut cx, "schedule-down");
    click(&mut cx, "schedule-down");
    operator.read_with(&cx, |o, _| {
        let order: Vec<_> = o.schedule.entries().iter().map(|e| e.id).collect();
        assert_eq!(order, [ids[0], ids[2], ids[1]]);
        assert_eq!(
            o.selected_entry,
            Some(ids[1]),
            "selection follows the entry"
        );
    });

    // Remove From Schedule asks first; Keep leaves it.
    click(&mut cx, "schedule-remove");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dialog),
        Some(Dialog::Remove(ids[1]))
    );
    assert_control(&mut cx, &operator, crate::operator::DIALOG_CANCEL);
    cx.simulate_keystrokes("enter");
    assert_control(&mut cx, &operator, REMOVE_ITEM);
    assert_eq!(
        operator.read_with(&cx, |o, _| o.schedule.entries().len()),
        3
    );
    click(&mut cx, "schedule-remove");
    click(&mut cx, "dialog-confirm");
    operator.read_with(&cx, |o, _| {
        assert_eq!(entry_titles(o), ["Signal Hymn", SECOND_SONG]);
        assert_eq!((o.dialog, o.selected_entry), (None, None));
    });

    // Right-click menu, then Ctrl+Del (no confirmation).
    let position = cx.debug_bounds("schedule-item-1").unwrap().center();
    cx.simulate_mouse_down(position, gpui::MouseButton::Right, Default::default());
    cx.simulate_mouse_up(position, gpui::MouseButton::Right, Default::default());
    click(&mut cx, "item-menu-remove");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dialog),
        Some(Dialog::Remove(ids[2]))
    );
    click(&mut cx, "dialog-cancel");
    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("ctrl-delete");
    operator.read_with(&cx, |o, _| {
        assert_eq!(entry_titles(o), ["Signal Hymn"]);
        assert_eq!(o.selected_entry, None);
    });
    cx.simulate_keystrokes("ctrl-delete");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.schedule_message.clone()),
        Some("Select a schedule item first".into())
    );
}

fn drag(cx: &mut VisualTestContext, from: &'static str, to: &'static str) {
    use gpui::{MouseButton, point};
    let start = cx.debug_bounds(from).unwrap().center();
    let end = cx.debug_bounds(to).unwrap().center();
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        start + point(px(8.), px(8.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
}

#[gpui::test]
fn drag_songs_into_the_schedule_reorder_and_cancel(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_songs(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    let hymn = song_row(&mut cx, &operator, "Signal Hymn");
    let canticle = song_row(&mut cx, &operator, SECOND_SONG);
    drag(&mut cx, hymn, "schedule-list");
    assert_eq!(
        operator.read_with(&cx, |o, _| entry_titles(o)),
        ["Signal Hymn"]
    );
    drag(&mut cx, canticle, "schedule-item-0");
    assert_eq!(
        operator.read_with(&cx, |o, _| entry_titles(o)),
        [SECOND_SONG, "Signal Hymn"],
        "a drop on a row inserts before it"
    );
    drag(&mut cx, "schedule-item-0", "schedule-item-1");
    assert_eq!(
        operator.read_with(&cx, |o, _| entry_titles(o)),
        ["Signal Hymn", SECOND_SONG]
    );
    drag(&mut cx, "schedule-item-1", "schedule-list");
    assert_eq!(
        operator.read_with(&cx, |o, _| entry_titles(o)),
        ["Signal Hymn", SECOND_SONG],
        "dropping the last item on the list end keeps it last"
    );

    // Released outside the Schedule: nothing changes.
    drag(&mut cx, hymn, "Preview");
    drag(&mut cx, "schedule-item-0", "Live");
    operator.read_with(&cx, |o, _| {
        assert_eq!(entry_titles(o), ["Signal Hymn", SECOND_SONG]);
        assert_eq!(o.selected_entry, None, "a drag is not a click");
        assert!(o.live.is_none());
    });
    drag(&mut cx, hymn, "schedule-list");
    assert_eq!(
        operator.read_with(&cx, |o, _| entry_titles(o)),
        ["Signal Hymn", SECOND_SONG, "Signal Hymn"],
        "the drag ended; a new one works"
    );
}

#[gpui::test]
fn live_item_identity_survives_reorder_duplicates_and_removal(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_songs(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    add_song(&mut cx, &operator, "Signal Hymn");
    add_song(&mut cx, &operator, SECOND_SONG);
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("down");
    let live_id = operator.read_with(&cx, |o, _| o.selected_entry.unwrap());
    settle(&mut cx, &operator, "preview", |o| {
        previewed(o) == Some((Some(live_id), "Signal Hymn"))
    });
    cx.simulate_keystrokes("pagedown");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| o.live.as_ref().unwrap().entry),
        Some(live_id)
    );
    let marker = cx.debug_bounds("schedule-live-marker").unwrap().center();
    assert!(
        cx.debug_bounds("schedule-item-0")
            .unwrap()
            .contains(&marker)
    );
    let sent = operator.read_with(&cx, |o, _| submitted(o));

    click(&mut cx, "schedule-down");
    add_song(&mut cx, &operator, "Signal Hymn");
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert_eq!(entry_titles(o), [SECOND_SONG, "Signal Hymn", "Signal Hymn"]);
        assert_eq!(o.schedule.index(live_id), Some(1));
        assert_eq!(submitted(o), sent, "reordering never sends a cue");
        assert_eq!(o.live.as_ref().unwrap().entry, Some(live_id));
        assert_eq!(o.on_screen(), Some("Signal Hymn · Verse 1"));
    });
    // The marker follows the moved entry, not its old row or the duplicate.
    let marker = cx.debug_bounds("schedule-live-marker").unwrap().center();
    assert!(
        cx.debug_bounds("schedule-item-1")
            .unwrap()
            .contains(&marker)
    );

    click(&mut cx, "schedule-item-1");
    cx.simulate_keystrokes("ctrl-delete");
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert_eq!(entry_titles(o), [SECOND_SONG, "Signal Hymn"]);
        assert!(o.schedule.get(live_id).is_none());
        assert_eq!(
            submitted(o),
            sent,
            "removing the live item leaves the output"
        );
        assert_eq!(o.on_screen(), Some("Signal Hymn · Verse 1"));
        assert!(has_line(o, "Live item is no longer in the schedule"));
    });
    assert!(cx.debug_bounds("schedule-live-marker").is_none());
    click(&mut cx, "live-next");
    settle(
        &mut cx,
        &operator,
        "the removed item still navigates",
        |o| o.on_screen() == Some("Signal Hymn · Chorus"),
    );
}

#[gpui::test]
fn save_and_reopen_pin_revisions_behind_an_unsaved_guard(cx: &mut TestAppContext) {
    use crate::operator::{Dialog, Then};
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_songs(&dir);
    let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    add_song(&mut cx, &operator, "Signal Hymn");
    add_song(&mut cx, &operator, SECOND_SONG);
    let pinned = operator.read_with(&cx, |o, _| o.schedule.entries()[0].version);

    cx.simulate_keystrokes("ctrl-s");
    let input = operator.read_with(&cx, |o, _| {
        assert_eq!(o.dialog, Some(Dialog::SaveAs));
        o.title_input.clone().unwrap()
    });
    cx.update(|window, cx| assert!(input.read(cx).focus_handle(cx).is_focused(window)));
    cx.simulate_keystrokes("enter");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dialog_message.clone()),
        Some("Enter a title for the schedule".into())
    );
    cx.update(|_, cx| input.update(cx, |i, cx| i.set_text("Sunday", cx).unwrap()));
    cx.simulate_keystrokes("enter");
    assert_eq!(operator.read_with(&cx, |o, _| o.dialog), None);
    assert_control(&mut cx, &operator, crate::operator::ADD_TO_SCHEDULE);
    settle(&mut cx, &operator, "saved", |o| {
        o.schedule.saved().is_some() && !o.schedule.is_dirty()
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| o.schedule.title().map(str::to_owned)),
        Some("Sunday".into())
    );

    // A library edit never updates the scheduled revision.
    let mut repository = Repository::open(&path).unwrap();
    let mut revised = repository.song(pinned).unwrap();
    revised.title = "Signal Hymn (revised)".into();
    revised.sections[0].lyrics = "Revised line".into();
    repository.save_song(Some(pinned), revised).unwrap();
    drop(repository);

    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("down ctrl-delete");
    assert!(operator.read_with(&cx, |o, _| o.schedule.is_dirty()));
    cx.simulate_keystrokes("ctrl-o");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dialog),
        Some(Dialog::Unsaved(Then::Open))
    );
    cx.simulate_keystrokes("enter");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.dialog, None, "Cancel is the default");
        assert_eq!(entry_titles(o), [SECOND_SONG]);
    });
    click(&mut cx, "open-schedule");
    click(&mut cx, "dialog-confirm");
    assert_eq!(operator.read_with(&cx, |o, _| o.dialog), Some(Dialog::Open));
    settle(&mut cx, &operator, "saved list", |o| {
        o.saved_schedules
            .as_ref()
            .is_some_and(|list| list.len() == 1 && list[0].1 == "Sunday")
    });
    click(&mut cx, "saved-0");
    settle(&mut cx, &operator, "reopened", |o| {
        o.schedule_message.as_deref() == Some("Opened “Sunday”")
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(entry_titles(o), ["Signal Hymn", SECOND_SONG]);
        assert_eq!(o.schedule.entries()[0].version, pinned);
        assert!(!o.schedule.is_dirty());
        assert_eq!(o.dialog, None);
    });
    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("down");
    settle(&mut cx, &operator, "pinned revision previewed", |o| {
        o.preview.as_ref().is_some_and(|p| {
            p.version == pinned && p.slides[0].text.starts_with("First original line")
        })
    });

    // A saved schedule saves again without asking for a title.
    click(&mut cx, "schedule-down");
    let first = operator.read_with(&cx, |o, _| o.schedule.saved().unwrap());
    cx.simulate_keystrokes("ctrl-s");
    assert_eq!(operator.read_with(&cx, |o, _| o.dialog), None);
    settle(&mut cx, &operator, "second revision", |o| {
        o.schedule
            .saved()
            .is_some_and(|v| v.id == first.id && v.revision == first.revision + 1)
            && !o.schedule.is_dirty()
    });
    let (stored, _) = Repository::open(&path)
        .unwrap()
        .schedule(operator.read_with(&cx, |o, _| o.schedule.saved().unwrap()))
        .unwrap();
    assert_eq!(stored.title, "Sunday");
    assert_eq!(stored.items[1], pinned);
}

#[gpui::test]
fn quit_and_new_schedule_are_guarded(cx: &mut TestAppContext) {
    use crate::operator::{Dialog, Then};
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_song(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    let may_close = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx)))
    };
    assert!(may_close(&mut cx), "a clean schedule closes");
    add_song(&mut cx, &operator, "Signal Hymn");
    assert!(!may_close(&mut cx));
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dialog),
        Some(Dialog::Unsaved(Then::Quit))
    );
    click(&mut cx, "dialog-cancel");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.schedule.entries().len()),
        1
    );

    click(&mut cx, "new-menu");
    click(&mut cx, "new-schedule-menu");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dialog),
        Some(Dialog::Unsaved(Then::New))
    );
    click(&mut cx, "dialog-confirm");
    operator.read_with(&cx, |o, _| {
        assert!(o.schedule.entries().is_empty());
        assert!(!o.schedule.is_dirty());
        assert_eq!(o.dialog, None);
    });

    // A save in flight finishes before the window may close.
    add_song(&mut cx, &operator, "Signal Hymn");
    click(&mut cx, "save-schedule");
    let input = operator.read_with(&cx, |o, _| o.title_input.clone().unwrap());
    cx.update(|_, cx| input.update(cx, |i, cx| i.set_text("Evening", cx).unwrap()));
    click(&mut cx, "dialog-confirm");
    assert!(!may_close(&mut cx));
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.dialog, None);
        assert!(
            o.schedule_message
                .as_deref()
                .unwrap()
                .starts_with("Saving the schedule")
        );
    });
    settle(&mut cx, &operator, "saved", |o| !o.schedule.is_dirty());
    assert!(may_close(&mut cx));
}

#[gpui::test]
fn library_double_click_goes_straight_to_live(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_song(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    double_click(&mut cx, "song-0");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.live.as_ref().unwrap().entry, None);
        assert!(o.schedule.entries().is_empty(), "not added to the schedule");
    });
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "chorus", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });
    // Already previewed: a second double-click restarts from the first slide.
    double_click(&mut cx, "song-0");
    settle(&mut cx, &operator, "verse again", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
}
