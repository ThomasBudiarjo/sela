use super::*;
use crate::operator::{Control, Indicator, Launcher};
use gpui::{Entity, Focusable, TestAppContext, VisualTestContext};
use sela::{
    arrangement::SectionId,
    delivery::{DeliveryError, Epoch, LiveState, Outcome, Payload, RendererSession},
    fonts,
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
    with_settings(
        cx,
        library,
        mode,
        crate::operator::SettingsConfig::default(),
    )
}

fn with_settings(
    cx: &mut TestAppContext,
    library: Option<std::path::PathBuf>,
    mode: &'static str,
    config: crate::operator::SettingsConfig,
) -> (VisualTestContext, Entity<Operator>, Rc<Cell<usize>>) {
    let seen = Rc::new(Cell::new(0));
    let window = cx.update(|cx| {
        bind_operator_keys(cx);
        cx.open_window(Default::default(), |window, cx| {
            let operator =
                cx.new(|cx| Operator::with_settings(library, fake_launcher(mode), config, cx));
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
        // 800 minus the 28px menu bar and the 42px of toolbar/borders.
        702.
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
        assert!(operator.read(cx).controls[Control::CollapseResources].is_focused(window));
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
    cx.update(|window, cx| {
        assert!(operator.read(cx).controls[Control::ResetLayout].is_focused(window))
    });
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
            assert!(operator.read(cx).controls[Control::ResetLayout].is_focused(window));
            assert!(operator.read(cx).focus.contains_focused(window, cx));
        });
    }
    for (w, h) in [(0., 0.), (10., 10.), (720., 440.), (1280., 800.)] {
        let d = operator.read_with(&cx, |o, _| o.dimensions(w, h));
        assert!(d.iter().all(|v| v.is_finite() && *v >= 0.));
        assert!((d[0] + d[1] + d[2] - (w - 12.).max(0.)).abs() < 0.01);
    }
}

fn assert_control(cx: &mut VisualTestContext, operator: &Entity<Operator>, control: Control) {
    cx.update(|window, cx| {
        assert!(
            operator.read(cx).controls[control].is_focused(window),
            "control {control:?}"
        );
        assert!(operator.read(cx).focus.contains_focused(window, cx));
    });
}

#[gpui::test]
fn keyboard_traversal_and_activation(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    // NewSongItem exists only while the New menu is open.
    let order = [
        Control::ResetLayout,
        Control::Quit,
        Control::CollapseResources,
        Control::TabSongs,
        Control::TabScriptures,
        Control::TabMedia,
        Control::TabPresentations,
        Control::TabThemes,
        Control::NewSongButton,
        Control::NewMenu,
        Control::LiveOutput,
        Control::GoLive,
        Control::Previous,
        Control::Next,
        Control::Logo,
        Control::Black,
        Control::Clear,
        Control::Normalize,
        Control::OpenSchedule,
        Control::SaveSchedule,
        Control::AddToSchedule,
        Control::MoveUp,
        Control::MoveDown,
        Control::RemoveItem,
    ];
    for index in order.iter().copied() {
        cx.simulate_keystrokes("tab");
        assert_control(&mut cx, &operator, index);
        assert_eq!(operator.read_with(&cx, |o, _| o.tab), 0);
    }
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, Control::ResetLayout);
    for index in order.iter().copied().rev() {
        cx.simulate_keystrokes("shift-tab");
        assert_control(&mut cx, &operator, index);
    }
    cx.simulate_keystrokes("tab tab tab tab");
    assert_control(&mut cx, &operator, Control::TabScriptures);
    cx.simulate_keystrokes("enter");
    assert_eq!(operator.read_with(&cx, |o, _| o.tab), 1);
    cx.simulate_keystrokes("tab space");
    assert_eq!(operator.read_with(&cx, |o, _| o.tab), 2);
    let schedule = cx.debug_bounds("Schedule").unwrap();
    let ratios = operator.read_with(&cx, |o, _| o.ratios);
    cx.simulate_keystrokes("ctrl-j down left shift-enter shift-space");
    assert_control(&mut cx, &operator, Control::TabMedia);
    assert_eq!(operator.read_with(&cx, |o, _| o.tab), 2);
    assert_eq!(operator.read_with(&cx, |o, _| o.ratios), ratios);
    assert_eq!(cx.debug_bounds("Schedule").unwrap(), schedule);
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 1, "root Quit routes from a child context");
    cx.simulate_keystrokes("shift-tab shift-tab shift-tab shift-tab enter space");
    assert_control(&mut cx, &operator, Control::Quit);
    assert_eq!(
        seen.get(),
        3,
        "each focused Quit activation dispatches once"
    );
    cx.update(|_, cx| cx.clear_key_bindings());
    cx.simulate_keystrokes("enter space tab ctrl-q");
    assert_control(&mut cx, &operator, Control::Quit);
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
    cx.update(|w, cx| {
        operator.read(cx).controls[Control::NewMenu]
            .clone()
            .focus(w, cx)
    });
    cx.simulate_keystrokes("enter");
    assert!(cx.debug_bounds("new-song-menu").is_some());
    assert_eq!(cx.debug_bounds("Schedule").unwrap(), schedule);
    assert_eq!(cx.debug_bounds("Live").unwrap(), live);
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, Control::NewSongItem);
    cx.simulate_keystrokes("shift-tab space");
    assert_control(&mut cx, &operator, Control::NewMenu);
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
    assert_control(&mut cx, &operator, Control::CollapseResources);
    assert!(cx.debug_bounds("Songs").is_none());
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, Control::NewMenu);
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, Control::LiveOutput);
    cx.simulate_keystrokes("shift-tab shift-tab space");
    assert_control(&mut cx, &operator, Control::CollapseResources);
    assert!(!operator.read_with(&cx, |o, _| o.collapsed));
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, Control::TabSongs);
    let b = cx.debug_bounds("collapse-resources").unwrap();
    cx.simulate_click(b.center(), Default::default());
    assert_control(&mut cx, &operator, Control::CollapseResources);
    // 2 → 9 → Live output, Go Live, Previous, Next → Logo, Black, Clear →
    // Open, Save → schedule Up, Down, Remove → 0 (Options lives in Edit › now).
    cx.simulate_keystrokes("tab tab tab tab tab tab tab tab tab tab tab tab tab tab enter");
    assert_control(&mut cx, &operator, Control::ResetLayout);
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
        background: None,
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
                master: None,
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
    assert_control(&mut cx, &operator, Control::GoLive);
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
fn go_live_defers_until_fonts_resolve_then_retries(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_formatted_song(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "faces resolved", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.slides.len() == 2 && p.resolved.is_some())
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    // A send racing the background resolver: strip the attached faces, then
    // Go Live. No cue may be submitted, nothing replaces the current scene
    // and the send waits (in production the landing callback re-attaches).
    cx.update(|_, cx| {
        operator.update(cx, |o, _| {
            o.live = o.preview.clone();
            o.live.as_mut().unwrap().resolved = None;
            o.preview.as_mut().unwrap().resolved = None;
        })
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    click(&mut cx, "go-live");
    operator.read_with(&cx, |o, _| {
        assert_eq!(submitted(o), sent, "no cue is submitted before faces land");
        assert_eq!(o.live_message.as_deref(), Some("Resolving fonts…"));
        assert_eq!(o.on_screen(), None, "no scene replaces anything");
    });
    // The resolution lands: the deferred cue retries and is acknowledged.
    let (version, formats) = operator.read_with(&cx, |o, _| {
        let item = o.live.as_ref().unwrap();
        (
            item.version,
            item.slides
                .iter()
                .map(|slide| slide.format.clone())
                .collect::<Vec<_>>(),
        )
    });
    cx.update(|_, cx| {
        operator.update(cx, |o, cx| {
            let resolved = formats.iter().map(fonts::Resolved::bundled).collect();
            o.fonts_resolved(version, resolved, cx);
        })
    });
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Fallback Hymn · Verse 1")
    });
    assert!(operator.read_with(&cx, |o, _| submitted(o)) > sent);
    assert_eq!(operator.read_with(&cx, |o, _| o.live_message.clone()), None);
}

/// Verse 1 shows a profile image, Chorus a missing one, Bridge the song
/// master's color.
fn library_with_background_song(dir: &tempfile::TempDir) -> std::path::PathBuf {
    use sela::background::{Background, ImageRef};
    use sha2::Digest;
    let images = sela::images::images_dir(dir.path());
    std::fs::create_dir_all(&images).unwrap();
    image::RgbaImage::from_pixel(64, 36, image::Rgba([30, 110, 210, 255]))
        .save(images.join("backdrop.png"))
        .unwrap();
    let bytes = std::fs::read(images.join("backdrop.png")).unwrap();
    let image = |name: &str, sha256| {
        Some(Background::image(ImageRef {
            name: name.into(),
            sha256,
        }))
    };
    let path = dir.path().join("library.sqlite");
    let section = |label: &str, background| Section {
        id: SectionId::allocate(),
        label: label.into(),
        lyrics: format!("{label} original line"),
        format: Default::default(),
        background,
    };
    Repository::open(&path)
        .unwrap()
        .save_song(
            None,
            Song {
                title: "Backdrop Hymn".into(),
                authors: String::new(),
                copyright: String::new(),
                license: String::new(),
                variants: Vec::new(),
                sections: vec![
                    section(
                        "Verse 1",
                        image("backdrop.png", sha2::Sha256::digest(&bytes).into()),
                    ),
                    section("Chorus", image("gone.png", [1; 32])),
                    section("Bridge", None),
                ],
                master: Some(Background::color([0, 0, 90])),
            },
        )
        .unwrap();
    path
}

#[gpui::test]
fn image_backgrounds_gate_the_cue_and_missing_ones_show_black_with_a_warning(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_background_song(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "faces resolved", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.slides.len() == 3 && p.resolved.is_some())
    });
    // Preview and its neighbour prefetch at the nominal size while output
    // is off; the missing image already warns before anything goes live.
    settle(&mut cx, &operator, "nominal prefetch", |o| {
        o.background_job.is_none() && o.backgrounds.len() == 2
    });
    assert_eq!(operator.read_with(&cx, |o, _| o.preview_warning()), None);
    click(&mut cx, "preview-slide-1");
    let missing = "Background image ‘gone.png’ is missing; showing black";
    assert_eq!(
        operator
            .read_with(&cx, |o, _| o.preview_warning())
            .as_deref(),
        Some(missing)
    );
    assert!(cx.debug_bounds("Preview-status").is_some());
    click(&mut cx, "preview-slide-0");
    assert!(cx.debug_bounds("Preview-status").is_none());

    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    settle(&mut cx, &operator, "output-size prefetch", |o| {
        o.background_job.is_none() && o.backgrounds.len() == 4
    });
    // Evict everything, as if the decode were still running: Go Live keeps
    // the current scene and waits instead of sending.
    cx.update(|_, cx| {
        operator.update(cx, |o, _| {
            o.backgrounds = sela::images::BackgroundCache::new(sela::images::BACKGROUND_BUDGET);
        })
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    click(&mut cx, "go-live");
    operator.read_with(&cx, |o, _| {
        assert_eq!(submitted(o), sent, "no cue before the background lands");
        assert_eq!(o.live_message.as_deref(), Some("Preparing background…"));
        assert_eq!(o.on_screen(), None);
    });
    // The decode lands: the deferred cue retries and is acknowledged.
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Backdrop Hymn · Verse 1")
    });
    assert_eq!(operator.read_with(&cx, |o, _| o.live_message.clone()), None);

    // A missing image goes live as black with the warning, never refused.
    settle(&mut cx, &operator, "neighbour prefetch", |o| {
        o.background_job.is_none() && o.backgrounds.len() >= 2
    });
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "chorus on screen", |o| {
        o.on_screen() == Some("Backdrop Hymn · Chorus")
    });
    assert_eq!(
        operator
            .read_with(&cx, |o, _| o.live_message.clone())
            .as_deref(),
        Some(missing)
    );
    // The master's color needs no preparation and clears the warning.
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "bridge on screen", |o| {
        o.on_screen() == Some("Backdrop Hymn · Bridge")
    });
    assert_eq!(operator.read_with(&cx, |o, _| o.live_message.clone()), None);
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
        operator.read(cx).controls[Control::Normalize]
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
/// One section names a family nothing installs, so every resolution path
/// (with or without a catalog) exercises the bundled fallback.
fn library_with_formatted_song(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("library.sqlite");
    let mut repository = Repository::open(&path).unwrap();
    repository
        .save_song(
            None,
            Song {
                title: "Fallback Hymn".into(),
                authors: String::new(),
                copyright: String::new(),
                license: String::new(),
                variants: Vec::new(),
                sections: vec![
                    Section {
                        id: SectionId::allocate(),
                        label: "Verse 1".into(),
                        lyrics: "Falling back line".into(),
                        format: sela::format::SlideFormat {
                            font: Some("Sela Missing Family".into()),
                            ..Default::default()
                        },
                        background: None,
                    },
                    Section {
                        id: SectionId::allocate(),
                        label: "Chorus".into(),
                        lyrics: "Second falling line".into(),
                        format: Default::default(),
                        background: None,
                    },
                ],
                master: None,
            },
        )
        .unwrap();
    path
}

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
                    background: None,
                }],
                master: None,
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
    assert_control(&mut cx, &operator, Control::DialogCancel);
    cx.simulate_keystrokes("enter");
    assert_control(&mut cx, &operator, Control::RemoveItem);
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

/// VAL-A11Y-005: with focus in the Songs list, Down/Up move the song
/// selection (and preview), never the schedule selection; the same keys in
/// the Schedule list move the schedule selection. Sela keyboard policy,
/// provisional (EasyWorship unobserved).
#[gpui::test]
fn song_row_arrows_move_the_song_selection_not_the_schedule(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_songs(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    let canticle = song_row(&mut cx, &operator, SECOND_SONG);
    let selected_title = |o: &Operator| {
        o.selected_song
            .and_then(|v| o.catalog.iter().find(|(c, _)| *c == v))
            .map(|(_, t)| t.clone())
    };
    // The catalog orders rows by version id, so derive them by position.
    let (first, second) =
        operator.read_with(&cx, |o, _| (o.catalog[0].1.clone(), o.catalog[1].1.clone()));

    click(&mut cx, "song-0");
    // Up at the first row stops at the start.
    cx.simulate_keystrokes("up");
    assert_eq!(
        operator.read_with(&cx, |o, _| selected_title(o)),
        Some(first.clone())
    );
    cx.simulate_keystrokes("down");
    let wanted = second.clone();
    settle(&mut cx, &operator, "second song previewed", |o| {
        previewed(o) == Some((None, wanted.as_str()))
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(selected_title(o), Some(second.clone()));
        assert_eq!(
            o.selected_entry, None,
            "Down/Up on a song row must not select a schedule entry"
        );
    });
    // Down past the last row stops at the end; Up walks back to the start.
    cx.simulate_keystrokes("down");
    assert_eq!(
        operator.read_with(&cx, |o, _| selected_title(o)),
        Some(second.clone())
    );
    cx.simulate_keystrokes("up");
    let wanted = first.clone();
    settle(&mut cx, &operator, "first song previewed again", |o| {
        previewed(o) == Some((None, wanted.as_str()))
    });

    // The schedule keeps its own arrows: focus a schedule row, then Down.
    add_song(&mut cx, &operator, "Signal Hymn");
    add_song(&mut cx, &operator, SECOND_SONG);
    click(&mut cx, canticle);
    let entries: Vec<_> = operator.read_with(&cx, |o, _| {
        o.schedule.entries().iter().map(|e| e.id).collect()
    });
    click(&mut cx, "schedule-item-0");
    cx.simulate_keystrokes("down");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.selected_entry, Some(entries[1]));
        assert_eq!(
            selected_title(o),
            Some(SECOND_SONG.to_owned()),
            "schedule selection pins its entry's song"
        );
    });
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
    assert_control(&mut cx, &operator, Control::AddToSchedule);
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

fn right_click(cx: &mut VisualTestContext, selector: &'static str) {
    let position = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is rendered"))
        .center();
    cx.simulate_mouse_down(position, gpui::MouseButton::Right, Default::default());
    cx.simulate_mouse_up(position, gpui::MouseButton::Right, Default::default());
}

fn menu_open(o: &Operator) -> bool {
    o.song_menu.is_some()
}

fn highlight(o: &Operator) -> Option<usize> {
    o.song_menu.and_then(|menu| menu.highlight)
}

fn version_of(o: &Operator, title: &str) -> sela::storage::Version {
    o.catalog
        .iter()
        .find(|(_, t)| t == title)
        .map(|(v, _)| *v)
        .unwrap_or_else(|| panic!("{title} is in Songs"))
}

fn editor_windows(cx: &VisualTestContext) -> Vec<gpui::WindowHandle<crate::song_library::Library>> {
    cx.cx
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<crate::song_library::Library>())
        .collect()
}

type EditorState = (Option<sela::storage::Version>, Song, bool);

/// The editor window opened since `before`, polled until `done`.
fn opened_editor(
    cx: &mut VisualTestContext,
    before: &[gpui::WindowHandle<crate::song_library::Library>],
    done: impl Fn(&EditorState) -> bool,
) -> (VisualTestContext, Entity<crate::song_library::Library>) {
    let opened: Vec<_> = editor_windows(cx)
        .into_iter()
        .filter(|window| !before.contains(window))
        .collect();
    assert_eq!(opened.len(), 1, "exactly one editor window opened");
    let mut editor_cx = VisualTestContext::from_window(opened[0].into(), &cx.cx);
    let editor = opened[0].root(&mut editor_cx).unwrap();
    settle_editor(&mut editor_cx, &editor, done);
    (editor_cx, editor)
}

fn settle_editor(
    cx: &mut VisualTestContext,
    editor: &Entity<crate::song_library::Library>,
    done: impl Fn(&EditorState) -> bool,
) {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        editor.update(cx, |e, cx| e.probe_poll(cx));
        cx.run_until_parked();
        if editor.read_with(cx, |e, cx| done(&e.probe(cx))) {
            return;
        }
        assert!(Instant::now() < end, "bounded wait for the editor");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn song_menu_items_order_disabled_items_and_dismissal(cx: &mut TestAppContext) {
    use crate::operator::{SONG_MENU, SongItem};
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_songs(&dir);
    let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    let hymn = song_row(&mut cx, &operator, "Signal Hymn");

    // Right-click selects the row and opens the menu (EW8-OBS-021 order).
    right_click(&mut cx, hymn);
    let selected = operator.read_with(&cx, |o, _| version_of(o, "Signal Hymn"));
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.song_menu.map(|m| m.version), Some(selected));
        assert_eq!((o.selected_song, o.selected_entry), (Some(selected), None));
        assert_eq!(
            highlight(o),
            None,
            "a pointer-opened menu highlights nothing"
        );
    });
    assert_eq!(
        SONG_MENU.map(SongItem::label),
        [
            "New Song…",
            "Edit Song…",
            "Delete",
            "Update items in Schedule",
            "Sort by ▸",
            "Refresh"
        ]
    );
    let rendered: Vec<_> = [
        "song-menu-new",
        "song-menu-edit",
        "song-menu-delete",
        "song-menu-update",
        "song-menu-sort",
        "song-menu-refresh",
    ]
    .into_iter()
    .map(|id| {
        cx.debug_bounds(id)
            .unwrap_or_else(|| panic!("{id} rendered"))
    })
    .collect();
    assert!(
        rendered.windows(2).all(|p| p[0].bottom() <= p[1].top()),
        "items stack top to bottom in that order"
    );
    let menu = cx.debug_bounds("song-menu").unwrap();
    let row = cx.debug_bounds(hymn).unwrap();
    // The anchored popup rounds to whole pixels; the pointer may sit half a
    // pixel off when the pane ratio lands on a fractional height.
    let pointer = row.center();
    assert!(
        (f32::from(menu.origin.x) - f32::from(pointer.x)).abs() <= 1.
            && (f32::from(menu.origin.y) - f32::from(pointer.y)).abs() <= 1.,
        "the menu opens at the pointer ({menu:?} vs {pointer:?})"
    );

    // Disabled items are inert: the menu stays, nothing opens.
    let editors = editor_windows(&cx).len();
    click(&mut cx, "song-menu-update");
    click(&mut cx, "song-menu-sort");
    operator.read_with(&cx, |o, _| {
        assert!(menu_open(o));
        assert_eq!(o.dialog, None);
        assert!(o.library_error.is_none());
    });
    assert_eq!(editor_windows(&cx).len(), editors);

    // Escape closes it and returns focus to the row.
    cx.simulate_keystrokes("escape");
    assert!(!operator.read_with(&cx, |o, _| menu_open(o)));
    assert!(cx.debug_bounds("song-menu").is_none());
    let row_index = if hymn == "song-0" { 0 } else { 1 };
    cx.update(|window, cx| {
        let focused = window.focused(cx);
        operator.read_with(cx, |o, _| {
            assert_eq!(focused.as_ref(), o.song_rows.get(row_index))
        })
    });

    // A click outside closes it.
    right_click(&mut cx, hymn);
    click(&mut cx, "Preview");
    assert!(!operator.read_with(&cx, |o, _| menu_open(o)));

    // Keyboard: Shift+F10 opens on the focused row with the first item lit;
    // Up/Down skip disabled items and wrap; they never reach the schedule.
    click(&mut cx, hymn);
    cx.simulate_keystrokes("shift-f10");
    assert_eq!(operator.read_with(&cx, highlight_of), Some(0));
    cx.simulate_keystrokes("down");
    assert_eq!(operator.read_with(&cx, highlight_of), Some(1));
    cx.simulate_keystrokes("down");
    assert_eq!(operator.read_with(&cx, highlight_of), Some(2));
    cx.simulate_keystrokes("down");
    assert_eq!(operator.read_with(&cx, highlight_of), Some(5));
    cx.simulate_keystrokes("down");
    assert_eq!(operator.read_with(&cx, highlight_of), Some(0));
    cx.simulate_keystrokes("up");
    assert_eq!(operator.read_with(&cx, highlight_of), Some(5));
    cx.simulate_keystrokes("tab");
    assert_eq!(
        operator.read_with(&cx, highlight_of),
        Some(0),
        "Tab stays in the menu"
    );
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.schedule_message, None, "Up/Down went to the menu");
        assert!(o.schedule.entries().is_empty());
    });
    cx.simulate_keystrokes("escape");
    assert!(!operator.read_with(&cx, |o, _| menu_open(o)));

    // Refresh reloads Songs through the storage worker.
    Repository::open(&path)
        .unwrap()
        .save_song(
            None,
            Song {
                title: "Third Psalm".into(),
                authors: String::new(),
                copyright: String::new(),
                license: String::new(),
                variants: Vec::new(),
                sections: vec![Section {
                    id: SectionId::allocate(),
                    label: String::new(),
                    lyrics: "An added original line".into(),
                    format: Default::default(),
                    background: None,
                }],
                master: None,
            },
        )
        .unwrap();
    cx.simulate_keystrokes("menu up enter");
    assert!(!operator.read_with(&cx, |o, _| menu_open(o)));
    settle(&mut cx, &operator, "refreshed songs", |o| {
        o.catalog.len() == 3
    });
    assert!(operator.read_with(&cx, |o, _| {
        o.catalog.iter().any(|(_, t)| t == "Third Psalm")
    }));
}

fn highlight_of(o: &Operator, _: &gpui::App) -> Option<usize> {
    highlight(o)
}

#[gpui::test]
fn edit_song_opens_that_song_new_song_a_blank_one_and_a_save_refreshes_songs(
    cx: &mut TestAppContext,
) {
    use gpui::EntityInputHandler;
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_songs(&dir);
    let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    let hymn = song_row(&mut cx, &operator, "Signal Hymn");
    let canticle = song_row(&mut cx, &operator, SECOND_SONG);
    let first = operator.read_with(&cx, |o, _| version_of(o, "Signal Hymn"));

    // New Song… opens an empty, unsaved editor.
    let before = editor_windows(&cx);
    right_click(&mut cx, canticle);
    click(&mut cx, "song-menu-new");
    let (_, blank) = opened_editor(&mut cx, &before, |(_, _, busy)| !busy);
    blank.read_with(&cx, |e, cx| {
        let (version, song, _) = e.probe(cx);
        assert_eq!(version, None);
        assert_eq!(song.title, "");
        assert_eq!(song.sections.len(), 1);
        assert_eq!(
            (
                song.sections[0].label.as_str(),
                song.sections[0].lyrics.as_str()
            ),
            ("", "")
        );
    });

    // Edit Song… opens the right-clicked song's saved revision.
    let before = editor_windows(&cx);
    right_click(&mut cx, hymn);
    click(&mut cx, "song-menu-edit");
    let (mut editor_cx, editor) = opened_editor(&mut cx, &before, |(version, _, busy)| {
        version.is_some() && !busy
    });
    editor.read_with(&editor_cx, |e, cx| {
        let (version, song, _) = e.probe(cx);
        assert_eq!(version, Some(first));
        assert_eq!(song.title, "Signal Hymn");
        let lyrics: Vec<_> = song.sections.iter().map(|s| s.lyrics.as_str()).collect();
        assert_eq!(
            lyrics,
            [
                "First original line\nSecond original line",
                "Original refrain",
                "Original bridge"
            ]
        );
    });
    assert_eq!(
        editor_cx.window_title().as_deref(),
        Some("Song Editor - Signal Hymn")
    );
    settle(&mut cx, &operator, "preview", |o| {
        o.preview.as_ref().is_some_and(|p| p.version == first)
    });

    // Apply in the editor: the operator's Songs reload without a cue.
    let title = editor.read_with(&editor_cx, |e, _| e.probe_title_field());
    editor_cx.update(|window, cx| {
        title.update(cx, |field, cx| {
            let end = field.text().encode_utf16().count();
            field.replace_text_in_range(Some(0..end), "Signal Hymn (edited)", window, cx);
        });
        editor.update(cx, |e, cx| e.probe_apply(window, cx));
    });
    settle_editor(&mut editor_cx, &editor, |(version, _, busy)| {
        !busy && version.is_some_and(|v| v.revision == first.revision + 1)
    });
    settle(&mut cx, &operator, "songs show the saved title", |o| {
        o.catalog.iter().any(|(_, t)| t == "Signal Hymn (edited)")
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.catalog.len(), 2);
        assert!(!o.catalog.iter().any(|(_, t)| t == "Signal Hymn"));
        assert!(o.output.is_none() && o.live.is_none());
    });
    // The selection follows the song to its saved revision.
    settle(&mut cx, &operator, "preview of the saved revision", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.title == "Signal Hymn (edited)" && p.version.id == first.id)
    });
    assert_eq!(
        Repository::open(&path)
            .unwrap()
            .catalog(None)
            .unwrap()
            .into_iter()
            .filter(|(v, _)| v.id == first.id)
            .map(|(_, t)| t)
            .collect::<Vec<_>>(),
        ["Signal Hymn (edited)"]
    );
}

#[gpui::test]
fn deleting_a_scheduled_live_song_keeps_snapshots_and_the_live_scene(cx: &mut TestAppContext) {
    use crate::operator::Dialog;
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_songs(&dir);
    let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    add_song(&mut cx, &operator, "Signal Hymn");
    let pinned = operator.read_with(&cx, |o, _| o.schedule.entries()[0].version);
    assert_eq!(
        pinned,
        operator.read_with(&cx, |o, _| version_of(o, "Signal Hymn"))
    );
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    let hymn = song_row(&mut cx, &operator, "Signal Hymn");
    double_click(&mut cx, hymn);
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));

    // Delete asks first; Keep (the default) leaves the song.
    right_click(&mut cx, hymn);
    click(&mut cx, "song-menu-delete");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dialog),
        Some(Dialog::DeleteSong(pinned))
    );
    assert_control(&mut cx, &operator, Control::DialogCancel);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.dialog, None);
        assert_eq!(o.catalog.len(), 2);
        assert_eq!(o.song_message, None);
    });
    assert_eq!(
        Repository::open(&path)
            .unwrap()
            .catalog(None)
            .unwrap()
            .len(),
        2
    );

    right_click(&mut cx, hymn);
    click(&mut cx, "song-menu-delete");
    click(&mut cx, "dialog-confirm");
    settle(&mut cx, &operator, "song deleted", |o| {
        o.song_message == Some(("Deleted “Signal Hymn”".into(), false))
    });
    settle(&mut cx, &operator, "songs refreshed", |o| {
        o.catalog.len() == 1 && !o.catalog_refresh_pending()
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.catalog[0].1, SECOND_SONG);
        assert_eq!(submitted(o), sent, "a delete sends no cue");
        assert_eq!(o.on_screen(), Some("Signal Hymn · Verse 1"));
        assert_eq!(o.live.as_ref().unwrap().version, pinned);
        // Preview keeps the slides it had prepared.
        let preview = o.preview.as_ref().unwrap();
        assert_eq!((preview.version, preview.slides.len()), (pinned, 3));
        assert_eq!(o.selected_song, None);
        assert_eq!(entry_titles(o), ["Signal Hymn"]);
    });
    let repository = Repository::open(&path).unwrap();
    assert_eq!(
        repository
            .catalog(None)
            .unwrap()
            .into_iter()
            .map(|(_, t)| t)
            .collect::<Vec<_>>(),
        [SECOND_SONG]
    );
    assert_eq!(repository.song(pinned).unwrap().title, "Signal Hymn");
    drop(repository);

    // The scheduled snapshot still previews and navigates live.
    click(&mut cx, "schedule-item-0");
    settle(&mut cx, &operator, "scheduled snapshot previewed", |o| {
        o.preview.as_ref().is_some_and(|p| {
            p.entry.is_some() && p.version == pinned && p.slides[2].text == "Original bridge"
        })
    });
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "live still navigates", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });

    // And it still saves and reopens from storage.
    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("ctrl-s");
    let input = operator.read_with(&cx, |o, _| o.title_input.clone().unwrap());
    cx.update(|_, cx| input.update(cx, |i, cx| i.set_text("After delete", cx).unwrap()));
    cx.simulate_keystrokes("enter");
    settle(&mut cx, &operator, "schedule saved", |o| {
        o.schedule.saved().is_some() && !o.schedule.is_dirty()
    });
    let saved = operator.read_with(&cx, |o, _| o.schedule.saved().unwrap());
    let (stored, songs) = Repository::open(&path).unwrap().schedule(saved).unwrap();
    assert_eq!(stored.items, [pinned]);
    assert_eq!(songs[0].title, "Signal Hymn");
    assert_eq!(songs[0].sections[1].lyrics, "Original refrain");
}

#[gpui::test]
fn a_failed_delete_shows_why_and_keeps_the_row(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_songs(&dir);
    let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    let hymn = song_row(&mut cx, &operator, "Signal Hymn");
    let shown = operator.read_with(&cx, |o, _| version_of(o, "Signal Hymn"));
    right_click(&mut cx, hymn);
    click(&mut cx, "song-menu-delete");

    // A newer revision is saved elsewhere before the delete is confirmed.
    let mut repository = Repository::open(&path).unwrap();
    let mut revised = repository.song(shown).unwrap();
    revised.sections[0].lyrics = "Revised elsewhere".into();
    let head = repository.save_song(Some(shown), revised).unwrap();
    drop(repository);

    click(&mut cx, "dialog-confirm");
    settle(&mut cx, &operator, "delete refused", |o| {
        o.song_message.as_ref().is_some_and(|(_, failed)| *failed)
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| o.song_message.clone()),
        Some((
            "Not deleted: “Signal Hymn” was changed or deleted elsewhere".into(),
            true
        ))
    );
    assert!(cx.debug_bounds("song-message").is_some());
    settle(&mut cx, &operator, "songs refreshed", |o| {
        o.catalog.iter().any(|(v, _)| *v == head) && !o.catalog_refresh_pending()
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.catalog.len(), 2, "the row stays");
        assert_eq!(
            o.selected_song,
            Some(head),
            "the selection follows the song"
        );
    });
    let catalog = Repository::open(&path).unwrap().catalog(None).unwrap();
    assert!(catalog.contains(&(head, "Signal Hymn".to_string())));
}

mod settings_ui {
    use super::*;
    use crate::operator::{Dialog, MonitorSlot, SettingsConfig};
    use sela::i18n::{self, Language};
    use sela::settings::{self, MonitorInfo};

    fn monitor(name: &str, primary: bool) -> MonitorInfo {
        MonitorInfo {
            name: name.into(),
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            primary,
        }
    }

    fn config(writer: settings::Writer) -> (SettingsConfig, Arc<MonitorSlot>) {
        let slot = Arc::new(MonitorSlot::default());
        let config = SettingsConfig {
            slot: slot.clone(),
            list_monitors: Arc::new(|| {
                vec![
                    monitor(r"\\.\DISPLAY5", true),
                    monitor(r"\\.\DISPLAY6", false),
                ]
            }),
            writer,
        };
        (config, slot)
    }

    fn start(
        cx: &mut TestAppContext,
        dir: &tempfile::TempDir,
        writer: settings::Writer,
    ) -> (VisualTestContext, Entity<Operator>, Arc<MonitorSlot>) {
        let (config, slot) = config(writer);
        let (mut cx, operator, _) =
            with_settings(cx, Some(library_with_song(dir)), "apply", config);
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "monitors", |o| {
            o.settings.monitors.is_some()
        });
        (cx, operator, slot)
    }

    fn file(dir: &tempfile::TempDir) -> Option<String> {
        std::fs::read_to_string(settings::path_in(dir.path())).ok()
    }

    /// The toolbar has no Options button (EW8-OBS-010); Edit › Options... is
    /// the only route. Alt+E opens Edit with the first enabled row lit.
    fn open_options(cx: &mut VisualTestContext) {
        cx.simulate_keystrokes("alt-e enter");
    }

    #[gpui::test]
    fn ok_applies_and_persists_the_output_monitor(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut vcx, operator, slot) = start(cx, &dir, settings::default_writer());
        assert_eq!(slot.get(), "secondary");
        open_options(&mut vcx);
        assert_eq!(
            operator.read_with(&vcx, |o, _| o.dialog),
            Some(Dialog::Options)
        );
        for section in [
            "main-output",
            "alternate-output",
            "foldback",
            "service-intervals",
            "slide-labels",
            "advanced",
        ] {
            assert!(
                vcx.debug_bounds(Box::leak(
                    format!("options-section-{section}").into_boxed_str()
                ))
                .is_some(),
                "{section}"
            );
        }
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-confirm");
        assert_eq!(operator.read_with(&vcx, |o, _| o.dialog), None);
        assert_eq!(slot.get(), r"name:\\.\DISPLAY6");
        settle(&mut vcx, &operator, "file written", |_| {
            file(&dir).is_some()
        });
        assert_eq!(
            file(&dir).unwrap(),
            "sela-settings 1\noutput_monitor=\\\\.\\DISPLAY6\n"
        );
        drop(vcx);

        // A restart with the same profile restores the choice.
        let (mut vcx, operator, slot) = start(cx, &dir, settings::default_writer());
        settle(&mut vcx, &operator, "loaded", |o| {
            o.settings.current.output_monitor.is_some()
        });
        assert_eq!(slot.get(), r"name:\\.\DISPLAY6");
        open_options(&mut vcx);
        assert_eq!(
            operator.read_with(&vcx, |o, _| o.settings.draft.clone()),
            Some(Some(r"\\.\DISPLAY6".to_string()))
        );
    }

    #[gpui::test]
    fn language_switches_at_runtime_and_persists(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut vcx, operator, _slot) = start(cx, &dir, settings::default_writer());
        assert_eq!(i18n::language(), Language::English);
        open_options(&mut vcx);
        click(&mut vcx, "options-language-id");
        click(&mut vcx, "dialog-confirm");
        assert_eq!(i18n::language(), Language::Indonesian);
        assert_eq!(
            operator.read_with(&vcx, |o, _| o.settings.current.language),
            Language::Indonesian
        );
        settle(&mut vcx, &operator, "file written", |_| {
            file(&dir).is_some()
        });
        assert!(file(&dir).unwrap().contains("language=id\n"));
        drop(vcx);

        // A restart with the same profile opens in Indonesian.
        let (mut vcx, operator, _slot) = start(cx, &dir, settings::default_writer());
        settle(&mut vcx, &operator, "loaded", |o| {
            o.settings.current.language == Language::Indonesian
        });
        assert_eq!(i18n::language(), Language::Indonesian);
        open_options(&mut vcx);
        click(&mut vcx, "options-language-en");
        click(&mut vcx, "dialog-confirm");
        assert_eq!(i18n::language(), Language::English);
        settle(&mut vcx, &operator, "file rewritten", |_| {
            file(&dir).is_some_and(|text| !text.contains("language="))
        });
    }

    #[gpui::test]
    fn cancel_discards_and_leaves_the_file_alone(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut vcx, operator, slot) = start(cx, &dir, settings::default_writer());
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-cancel");
        assert_eq!(operator.read_with(&vcx, |o, _| o.dialog), None);
        assert_eq!(slot.get(), "secondary");
        assert!(operator.read_with(&vcx, |o, _| o.settings.current.output_monitor.is_none()));
        vcx.run_until_parked();
        assert_eq!(file(&dir), None);
    }

    #[gpui::test]
    fn missing_saved_monitor_falls_back_with_a_note_and_no_rewrite(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let saved = "sela-settings 1\noutput_monitor=\\\\.\\DISPLAY9\n";
        std::fs::write(settings::path_in(dir.path()), saved).unwrap();
        let (mut vcx, operator, slot) = start(cx, &dir, settings::default_writer());
        settle(&mut vcx, &operator, "loaded", |o| {
            o.settings_note().is_some()
        });
        assert_eq!(slot.get(), "secondary");
        assert!(operator.read_with(&vcx, |o, _| o.settings_note().unwrap().contains("DISPLAY9")));
        assert!(vcx.debug_bounds("settings-note").is_some());
        open_options(&mut vcx);
        assert!(vcx.debug_bounds("options-missing").is_some());
        click(&mut vcx, "dialog-confirm");
        vcx.run_until_parked();
        assert_eq!(
            file(&dir).as_deref(),
            Some(saved),
            "unchanged OK never rewrites"
        );
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-0");
        click(&mut vcx, "dialog-confirm");
        settle(&mut vcx, &operator, "rewritten", |_| {
            file(&dir).as_deref() == Some("sela-settings 1\n")
        });
        assert_eq!(operator.read_with(&vcx, |o, _| o.settings_note()), None);
    }

    #[gpui::test]
    fn corrupt_settings_start_with_defaults_and_keep_a_backup(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(settings::path_in(dir.path()), "sela-settings 1\nout").unwrap();
        let (mut vcx, operator, slot) = start(cx, &dir, settings::default_writer());
        settle(&mut vcx, &operator, "note", |o| o.settings_note().is_some());
        assert_eq!(slot.get(), "secondary");
        assert!(dir.path().join("settings.txt.bad").exists());
        assert!(vcx.debug_bounds("settings-note").is_some());
    }

    #[gpui::test]
    fn saving_does_not_wait_for_a_slow_writer(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let writer: settings::Writer = Arc::new(|path, settings| {
            std::thread::sleep(Duration::from_secs(2));
            settings::save(path, settings)
        });
        let (mut vcx, operator, slot) = start(cx, &dir, writer);
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-2");
        let started = Instant::now();
        click(&mut vcx, "dialog-confirm");
        assert!(
            started.elapsed() < Duration::from_millis(1000),
            "OK waited for the write"
        );
        assert_eq!(operator.read_with(&vcx, |o, _| o.dialog), None);
        assert_eq!(slot.get(), r"name:\\.\DISPLAY6");
        // The UI still takes input and paints while the write is pending.
        open_options(&mut vcx);
        assert!(vcx.debug_bounds("options-dialog").is_some());
        click(&mut vcx, "dialog-cancel");
        assert!(file(&dir).is_none(), "the write is still running");
        settle(&mut vcx, &operator, "file written", |_| {
            file(&dir).is_some()
        });
    }
}

mod menu_bar {
    use super::*;
    use crate::operator::{Dialog, TOP_MENUS, TopMenu};

    #[gpui::test]
    fn labels_follow_the_ew_order(cx: &mut TestAppContext) {
        let (mut cx, _operator, _) = fixture(cx);
        cx.simulate_resize(size(px(1280.), px(800.)));
        let mut previous: Option<gpui::Bounds<gpui::Pixels>> = None;
        for top in TOP_MENUS {
            let bounds = cx
                .debug_bounds(top.id())
                .unwrap_or_else(|| panic!("{top:?} label is rendered"));
            if let Some(before) = previous {
                assert!(bounds.origin.x > before.origin.x, "EW8-OBS-010 order");
                assert_eq!(bounds.origin.y, before.origin.y, "one menu row");
            }
            previous = Some(bounds);
        }
        // The labels are not tab stops: the keyboard traversal test's fixed
        // control order would fail if they were.
    }

    #[gpui::test]
    fn menu_contents_match_the_captures(cx: &mut TestAppContext) {
        let (mut cx, _operator, _) = fixture(cx);
        cx.simulate_resize(size(px(1280.), px(800.)));
        // EW8-OBS-053 plus the menu-p/v/h.png captures; the New ▸ submenu is
        // flattened into a header row followed by Song.../Schedule.
        let menus: [(&str, &[&str]); 6] = [
            (
                "alt-f",
                &[
                    "bar-file-new",
                    "bar-file-new-song",
                    "bar-file-new-schedule",
                    "bar-file-open",
                    "bar-file-open-recent",
                    "bar-file-save",
                    "bar-file-save-as",
                    "bar-file-export",
                    "bar-file-close",
                    "bar-file-reports",
                    "bar-file-exit",
                ],
            ),
            (
                "alt-e",
                &[
                    "bar-edit-cut",
                    "bar-edit-copy",
                    "bar-edit-paste",
                    "bar-edit-delete",
                    "bar-edit-select-all",
                    "bar-edit-options",
                ],
            ),
            (
                "alt-l",
                &[
                    "bar-live-go-live",
                    "bar-live-go-back",
                    "bar-live-logo",
                    "bar-live-black",
                    "bar-live-clear",
                    "bar-live-show",
                ],
            ),
            (
                "alt-p",
                &[
                    "bar-profiles-default",
                    "bar-profiles-manager",
                    "bar-profiles-demo-data",
                    "bar-profiles-utilities",
                ],
            ),
            (
                "alt-v",
                &[
                    "bar-view-preview-live",
                    "bar-view-combined",
                    "bar-view-resource",
                    "bar-view-interface-color",
                ],
            ),
            (
                "alt-h",
                &[
                    "bar-help-contents",
                    "bar-help-getting-started",
                    "bar-help-logs",
                    "bar-help-updates",
                    "bar-help-helper",
                    "bar-help-about",
                ],
            ),
        ];
        for (key, ids) in menus {
            cx.simulate_keystrokes(key);
            let mut previous: Option<gpui::Bounds<gpui::Pixels>> = None;
            for id in ids {
                let bounds = cx
                    .debug_bounds(id)
                    .unwrap_or_else(|| panic!("{id} is rendered for {key}"));
                if let Some(before) = previous {
                    assert!(
                        bounds.origin.y > before.origin.y,
                        "{id} after the previous row"
                    );
                }
                previous = Some(bounds);
            }
            cx.simulate_keystrokes("escape");
        }
    }

    #[gpui::test]
    fn mouse_opens_switches_and_closes_menus(cx: &mut TestAppContext) {
        let (mut cx, operator, _) = fixture(cx);
        cx.simulate_resize(size(px(1280.), px(800.)));
        let menu = |cx: &VisualTestContext, o: &Entity<Operator>| {
            o.read_with(cx, |o, _| o.bar_menu.map(|m| (m.top, m.highlight)))
        };

        click(&mut cx, "bar-live");
        assert_eq!(menu(&cx, &operator), Some((TopMenu::Live, None)));
        // A mouse-opened menu keeps the menu keys: the press must not let
        // the root's track_focus auto-focus steal the menu's focus.
        cx.simulate_keystrokes("down");
        assert_eq!(menu(&cx, &operator), Some((TopMenu::Live, Some(0))));
        cx.simulate_keystrokes("escape");
        assert_eq!(menu(&cx, &operator), None);
        click(&mut cx, "bar-live");
        assert_eq!(menu(&cx, &operator), Some((TopMenu::Live, None)));
        // While a menu is open, hovering another label switches to it.
        let profiles = cx.debug_bounds("bar-profiles").unwrap();
        cx.simulate_mouse_move(profiles.center(), None, gpui::Modifiers::none());
        assert_eq!(menu(&cx, &operator), Some((TopMenu::Profiles, None)));
        // Clicking another label while open is swallowed: the menu closes
        // and the press does nothing else, like a Windows menu.
        click(&mut cx, "bar-live");
        assert_eq!(menu(&cx, &operator), None);
        // A second click opens it.
        click(&mut cx, "bar-live");
        assert_eq!(menu(&cx, &operator), Some((TopMenu::Live, None)));
        // Clicking the open menu's label closes it again.
        click(&mut cx, "bar-live");
        assert_eq!(menu(&cx, &operator), None);
        // A click outside dismisses the open menu.
        click(&mut cx, "bar-view");
        assert_eq!(menu(&cx, &operator), Some((TopMenu::View, None)));
        click(&mut cx, "Preview");
        assert_eq!(menu(&cx, &operator), None);
    }

    #[gpui::test]
    fn alt_mnemonics_open_and_esc_restores_focus(cx: &mut TestAppContext) {
        let (mut cx, operator, _) = fixture(cx);
        cx.simulate_resize(size(px(1280.), px(800.)));
        cx.update(|window, cx| {
            operator.read(cx).controls[Control::GoLive]
                .clone()
                .focus(window, cx)
        });
        // First enabled row per menu: File skips the New group header, Edit
        // skips the clipboard rows, Live starts at Go Live; the menus whose
        // features are not built yet have nothing selectable.
        let cases = [
            ("alt-f", TopMenu::File, Some(1)),
            ("alt-e", TopMenu::Edit, Some(6)),
            ("alt-l", TopMenu::Live, Some(0)),
            ("alt-p", TopMenu::Profiles, None),
            ("alt-v", TopMenu::View, None),
            ("alt-h", TopMenu::Help, None),
        ];
        for (key, top, highlight) in cases {
            cx.simulate_keystrokes(key);
            assert_eq!(
                operator.read_with(&cx, |o, _| o.bar_menu.map(|m| (m.top, m.highlight))),
                Some((top, highlight)),
                "{key} opens {top:?}"
            );
            cx.update(|window, cx| {
                let operator = operator.read(cx);
                assert!(
                    !operator.controls[Control::GoLive].is_focused(window),
                    "the menu took focus"
                );
                assert!(operator.focus.contains_focused(window, cx));
            });
            cx.simulate_keystrokes("escape");
            assert_eq!(operator.read_with(&cx, |o, _| o.bar_menu), None);
            cx.update(|window, cx| {
                assert!(
                    operator.read(cx).controls[Control::GoLive].is_focused(window),
                    "Esc returned focus to the previous control"
                );
            });
        }
    }

    #[gpui::test]
    fn arrows_move_the_highlight_and_switch_menus(cx: &mut TestAppContext) {
        let (mut cx, operator, _) = fixture(cx);
        cx.simulate_resize(size(px(1280.), px(800.)));
        let state = |cx: &VisualTestContext, o: &Entity<Operator>| {
            o.read_with(cx, |o, _| o.bar_menu.map(|m| (m.top, m.highlight)))
        };
        cx.simulate_keystrokes("alt-l");
        assert_eq!(state(&cx, &operator), Some((TopMenu::Live, Some(0))));
        // Down skips disabled Go Back, the separator, and Logo (no logo yet).
        cx.simulate_keystrokes("down");
        assert_eq!(state(&cx, &operator), Some((TopMenu::Live, Some(4))));
        cx.simulate_keystrokes("up");
        assert_eq!(state(&cx, &operator), Some((TopMenu::Live, Some(0))));
        // Right/Left walk across menus; menus without enabled rows keep None.
        cx.simulate_keystrokes("right");
        assert_eq!(state(&cx, &operator), Some((TopMenu::Profiles, None)));
        cx.simulate_keystrokes("right");
        assert_eq!(state(&cx, &operator), Some((TopMenu::View, None)));
        cx.simulate_keystrokes("left");
        assert_eq!(state(&cx, &operator), Some((TopMenu::Profiles, None)));
        cx.simulate_keystrokes("left");
        assert_eq!(state(&cx, &operator), Some((TopMenu::Live, Some(0))));
        // Tab stays inside the menu, like the Songs context menu.
        cx.simulate_keystrokes("tab");
        assert_eq!(state(&cx, &operator), Some((TopMenu::Live, Some(4))));
        cx.simulate_keystrokes("shift-tab");
        assert_eq!(state(&cx, &operator), Some((TopMenu::Live, Some(0))));
        cx.simulate_keystrokes("escape");
        assert_eq!(state(&cx, &operator), None);
    }

    #[gpui::test]
    fn items_dispatch_the_same_actions_as_the_shortcuts(cx: &mut TestAppContext) {
        let (mut cx, operator, seen) = fixture(cx);
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "monitors", |o| {
            o.settings.monitors.is_some()
        });

        // Black on Live Output toggles the same mask as Ctrl+B / the button.
        cx.simulate_keystrokes("alt-l down enter");
        assert!(operator.read_with(&cx, |o, _| o.masks.is_on(Mask::Black)));
        assert_eq!(operator.read_with(&cx, |o, _| o.bar_menu), None);
        cx.simulate_keystrokes("alt-l down enter");
        assert!(!operator.read_with(&cx, |o, _| o.masks.is_on(Mask::Black)));

        // Show Live Output toggles the audience window like the Live button.
        cx.simulate_keystrokes("alt-l up enter");
        assert!(operator.read_with(&cx, |o, _| o.output.is_some()));
        cx.simulate_keystrokes("alt-l up enter");
        assert!(operator.read_with(&cx, |o, _| o.output.is_none()));

        // Open Schedule routes the same guard as Ctrl+O (no library here).
        cx.simulate_keystrokes("alt-f down down enter");
        assert!(operator.read_with(&cx, |o, _| o.schedule_message.is_some()));

        // New Schedule clears without asking (nothing unsaved).
        cx.simulate_keystrokes("alt-f down enter");
        assert_eq!(operator.read_with(&cx, |o, _| o.dialog), None);
        assert_eq!(operator.read_with(&cx, |o, _| o.bar_menu), None);

        // Options opens the existing Options dialog.
        cx.simulate_keystrokes("alt-e enter");
        assert_eq!(
            operator.read_with(&cx, |o, _| o.dialog),
            Some(Dialog::Options)
        );
        click(&mut cx, "dialog-cancel");
        assert_eq!(operator.read_with(&cx, |o, _| o.dialog), None);

        // Exit dispatches Quit, the same as Ctrl+Q (probe counts; window stays).
        cx.simulate_keystrokes("alt-f up enter");
        assert_eq!(seen.get(), 1);
    }

    #[gpui::test]
    fn disabled_items_are_rendered_but_inert(cx: &mut TestAppContext) {
        let (mut cx, operator, seen) = fixture(cx);
        cx.simulate_resize(size(px(1280.), px(800.)));
        cx.simulate_keystrokes("alt-f");
        for id in [
            "bar-file-open-recent",
            "bar-file-save-as",
            "bar-file-reports",
        ] {
            click(&mut cx, id);
            assert_eq!(
                operator.read_with(&cx, |o, _| o.bar_menu.map(|m| m.top)),
                Some(TopMenu::File),
                "{id} did not close the menu"
            );
        }
        assert_eq!(operator.read_with(&cx, |o, _| o.dialog), None);
        assert_eq!(seen.get(), 0);
        cx.simulate_keystrokes("escape");

        cx.simulate_keystrokes("alt-p");
        cx.simulate_keystrokes("enter");
        assert_eq!(
            operator.read_with(&cx, |o, _| o.bar_menu.map(|m| m.top)),
            Some(TopMenu::Profiles),
            "Enter with nothing selectable is a no-op"
        );
        click(&mut cx, "bar-profiles-manager");
        assert_eq!(
            operator.read_with(&cx, |o, _| o.bar_menu.map(|m| m.top)),
            Some(TopMenu::Profiles)
        );
        cx.simulate_keystrokes("escape");
        assert_eq!(operator.read_with(&cx, |o, _| o.bar_menu), None);
    }

    #[gpui::test]
    fn typing_in_text_fields_never_opens_menus(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, _) = with_library(cx, Some(library_with_song(&dir)), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);

        cx.simulate_keystrokes("ctrl-s");
        let input = operator.read_with(&cx, |o, _| {
            assert_eq!(o.dialog, Some(Dialog::SaveAs));
            o.title_input.clone().unwrap()
        });
        cx.simulate_keystrokes("f e l p v h");
        cx.update(|_, cx| assert_eq!(input.read(cx).text(), "felpvh"));
        cx.simulate_keystrokes("alt-f alt-l");
        assert_eq!(operator.read_with(&cx, |o, _| o.bar_menu), None);
        assert_eq!(
            operator.read_with(&cx, |o, _| o.dialog),
            Some(Dialog::SaveAs)
        );
        cx.update(|_, cx| assert_eq!(input.read(cx).text(), "felpvh"));
        click(&mut cx, "dialog-cancel");
        assert_eq!(operator.read_with(&cx, |o, _| o.dialog), None);
    }

    #[gpui::test]
    fn file_new_song_opens_the_editor(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, _) = with_library(cx, Some(library_with_song(&dir)), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
        let before = editor_windows(&cx);
        cx.simulate_keystrokes("alt-f enter");
        let (_editor_cx, editor) = opened_editor(&mut cx, &before, |(_, _, busy)| !busy);
        editor.read_with(&cx, |e, cx| {
            let (version, _, _) = e.probe(cx);
            assert_eq!(version, None, "a blank unsaved song");
        });
        assert_eq!(operator.read_with(&cx, |o, _| o.bar_menu), None);
    }
}

/// A cue builder the test releases one token at a time; it also records which
/// thread assembled each cue.
fn gated_preparer(
    threads: Arc<std::sync::Mutex<Vec<std::thread::ThreadId>>>,
) -> (
    std::sync::mpsc::Sender<()>,
    sela::preparation::cue::CuePreparer,
) {
    use sela::preparation::cue::{Built, CuePreparer};
    let (open, gate) = std::sync::mpsc::channel::<()>();
    let preparer = CuePreparer::with_builder(move |job, _| {
        threads.lock().unwrap().push(std::thread::current().id());
        gate.recv_timeout(Duration::from_secs(10)).unwrap();
        sela::slides::cue(
            job.version,
            &job.slides[job.index],
            &job.resolved[job.index],
            job.extent,
            job.caps,
            None,
            job.background,
        )
        .map(|cue| Built {
            cue,
            size_cap: None,
        })
    })
    .unwrap();
    (open, preparer)
}

fn live_fixture(
    cx: &mut TestAppContext,
    dir: &tempfile::TempDir,
) -> (VisualTestContext, Entity<Operator>) {
    let (mut cx, operator, _) = with_library(cx, Some(library_with_song(dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "faces resolved", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.slides.len() == 3 && p.resolved.is_some())
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    (cx, operator)
}

#[gpui::test]
fn black_applies_within_the_bound_while_a_heavy_cue_prepares(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_fixture(cx, &dir);
    let threads = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (open, preparer) = gated_preparer(threads.clone());
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(preparer));
    let sent = operator.read_with(&cx, |o, _| submitted(o));

    click(&mut cx, "go-live");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.pending_cues.len(), 1);
        assert_eq!(submitted(o), sent, "the cue is not built or sent yet");
        assert_eq!(o.live_slide, None);
    });
    let start = Instant::now();
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });
    let took = start.elapsed();
    eprintln!("Black receipt while a cue prepares: {took:?}");
    assert!(took < Duration::from_millis(100), "Black took {took:?}");
    assert_eq!(operator.read_with(&cx, |o, _| o.pending_cues.len()), 1);

    open.send(()).unwrap();
    settle(&mut cx, &operator, "cue on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    let recorded = threads.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    assert_ne!(recorded[0], std::thread::current().id());
    assert_eq!(
        operator.read_with(&cx, |o, _| confirmed_mask(o)),
        Some(Layer::Black),
        "the mask stays over the new scene"
    );
}

#[gpui::test]
fn failed_or_stale_preparation_never_replaces_the_current_scene(cx: &mut TestAppContext) {
    use sela::preparation::cue::{Built, CuePreparer};
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_fixture(cx, &dir);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });

    // Failure: the audience keeps the verse and the operator is told.
    let failing = CuePreparer::with_builder(|_, _| -> Result<Built, _> {
        Err(sela::scene::PrepareError::InvalidScene)
    })
    .unwrap();
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(failing));
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "failure reported", |o| {
        o.live_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Slide could not be prepared"))
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(submitted(o), sent);
        assert_eq!(o.on_screen(), Some("Signal Hymn · Verse 1"));
        assert_eq!(o.live_slide, Some(0));
        assert!(o.pending_cues.is_empty());
    });

    // Stale: a request superseded (surface or session change) is dropped even
    // though its build completes afterwards.
    let (open, preparer) = gated_preparer(Default::default());
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(preparer));
    click(&mut cx, "live-next");
    operator.update(&mut cx, |o, _| {
        assert_eq!(o.pending_cues.len(), 1);
        o.cancel_cues();
        assert!(o.pending_cues.is_empty());
    });
    open.send(()).unwrap();
    for _ in 0..40 {
        operator.update(&mut cx, |o, cx| o.poll(cx));
        std::thread::sleep(Duration::from_millis(5));
    }
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            submitted(o),
            sent,
            "the stale cue never reaches the audience"
        );
        assert_eq!(o.on_screen(), Some("Signal Hymn · Verse 1"));
        assert_eq!(o.live_slide, Some(0));
    });
}

#[gpui::test]
fn the_cue_queue_is_bounded_and_refuses_with_a_message(cx: &mut TestAppContext) {
    use sela::preparation::cue::LIVE_DEPTH;
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_fixture(cx, &dir);
    let (open, preparer) = gated_preparer(Default::default());
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(preparer));
    click(&mut cx, "go-live");
    for _ in 0..200 {
        operator.update(&mut cx, |o, _| o.send(1));
        assert!(operator.read_with(&cx, |o, _| o.pending_cues.len()) <= LIVE_DEPTH);
    }
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.pending_cues.len(), LIVE_DEPTH);
        assert_eq!(
            o.live_message.as_deref(),
            Some("Still preparing earlier slides; try again")
        );
    });
    for _ in 0..LIVE_DEPTH {
        open.send(()).unwrap();
    }
    settle(&mut cx, &operator, "queue drained", |o| {
        o.pending_cues.is_empty() && o.on_screen() == Some("Signal Hymn · Chorus")
    });
    // The final selection is still accepted after the burst.
    open.send(()).unwrap();
    operator.update(&mut cx, |o, _| o.send(2));
    settle(&mut cx, &operator, "bridge on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Bridge")
    });
}
fn library_with_heavy_song(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("library.sqlite");
    let line = "Original heavy line of lyrics for the fitting measurement";
    let sections = (0..100)
        .map(|i| Section {
            id: SectionId::allocate(),
            label: format!("Verse {i}"),
            lyrics: vec![format!("{line} {i}"); 28].join("\n"),
            format: Default::default(),
            background: None,
        })
        .collect();
    Repository::open(&path)
        .unwrap()
        .save_song(
            None,
            Song {
                title: "Heavy Hymn".into(),
                authors: String::new(),
                copyright: String::new(),
                license: String::new(),
                variants: Vec::new(),
                sections,
                master: None,
            },
        )
        .unwrap();
    path
}

/// Measurement, not a gate: UI-thread time of one Go Live on a 100-slide song
/// with normalized sizing (`cargo test --release heavy_cue -- --ignored --nocapture`).
#[gpui::test]
#[ignore = "measurement recorded in docs/delivery.md"]
fn heavy_cue_ui_thread_time(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_heavy_song(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    click(&mut cx, "song-0");
    settle(&mut cx, &operator, "faces", |o| {
        o.preview.as_ref().is_some_and(|p| p.resolved.is_some())
    });
    click(&mut cx, "normalize-text");
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    let mut samples = Vec::new();
    for _ in 0..5 {
        operator.update(&mut cx, |o, _| o.size_cap = None);
        let start = Instant::now();
        operator.update(&mut cx, |o, _| o.go_live());
        samples.push(start.elapsed());
        settle(&mut cx, &operator, "on screen", |o| o.on_screen().is_some());
    }
    eprintln!("go_live UI-thread ms: {samples:?}");
}
