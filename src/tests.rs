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

mod arrangement_rendering;
mod autosave_recovery;
mod ew_library_import;
mod focus_modality;
mod layout_startup;
mod media_backend;
mod navigation;
mod resource_browser;
mod scripture;
mod scrutiny_r2;
mod song_import;
mod songs_browser;
mod stage_display;
mod themes;

const FAKE_EXTENT: Extent = Extent {
    width: 640,
    height: 360,
};

/// Renderer protocol stand-in: Ready, one surface report, then Accepted and
/// Applied (or a renderer failure in `reject` mode) for every cue. No GPU.
/// Extra modes: `reject-scene:<n>` fails only the nth scene payload (1-based),
/// `slow-scene:<ms>` delays every scene's terminal receipt by ms,
/// `slow-scene-at:<n>:<ms>` only the nth scene's.
#[test]
#[ignore = "subprocess fixture for operator live-output tests"]
fn fake_audience_child() {
    let mode = std::env::var("SELA_FAKE_AUDIENCE").unwrap();
    let epoch =
        Epoch(u128::from_str_radix(&std::env::var("SELA_FAKE_EPOCH").unwrap(), 16).unwrap());
    let reject_scene_at = mode
        .strip_prefix("reject-scene:")
        .map(|n| n.parse::<u64>().unwrap());
    let slow_scene = mode
        .strip_prefix("slow-scene:")
        .map(|ms| Duration::from_millis(ms.parse().unwrap()));
    let slow_scene_at = mode.strip_prefix("slow-scene-at:").map(|spec| {
        let (n, ms) = spec.split_once(':').unwrap();
        (
            n.parse::<u64>().unwrap(),
            Duration::from_millis(ms.parse().unwrap()),
        )
    });
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
    let mut scenes = 0u64;
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
                    Payload::Scene(_) => {
                        scenes += 1;
                        if reject_scene_at == Some(scenes) {
                            return Err(DeliveryError::RenderFailed);
                        }
                        if let Some(delay) = slow_scene {
                            std::thread::sleep(delay);
                        }
                        if let Some((at, delay)) = slow_scene_at
                            && at == scenes
                        {
                            std::thread::sleep(delay);
                        }
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
                administrator: String::new(),
                song_number: String::new(),
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

/// One step of a close that waits on real worker threads (the settings
/// writer, the autosave worker) while its bound, `CLOSE_WAIT`, runs on the
/// fake GPUI clock. While such work is outstanding the clock stands still
/// and replies are polled in real time, so a slow disk (a loaded Windows CI
/// runner) never runs out the bound; otherwise the clock moves on by `by`
/// and the close waits run. Returns whether fake time advanced.
fn close_step(cx: &mut VisualTestContext, operator: &Entity<Operator>, by: Duration) -> bool {
    let outstanding = operator.read_with(cx, |o, _| o.close_work_outstanding());
    if outstanding {
        operator.update(cx, |o, cx| o.poll(cx));
    } else {
        cx.executor().advance_clock(by);
    }
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(5));
    !outstanding
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
                administrator: String::new(),
                song_number: String::new(),
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

/// Verse 1 and Chorus each show an existing image, Bridge the master's
/// color, so an evicted cache defers the Chorus cue on its decode.
fn library_with_two_backgrounds(dir: &tempfile::TempDir) -> std::path::PathBuf {
    use sela::background::{Background, ImageRef};
    use sha2::Digest;
    let images = sela::images::images_dir(dir.path());
    std::fs::create_dir_all(&images).unwrap();
    let add = |name: &str, rgb: [u8; 4]| {
        image::RgbaImage::from_pixel(64, 36, image::Rgba(rgb))
            .save(images.join(name))
            .unwrap();
        let bytes = std::fs::read(images.join(name)).unwrap();
        Background::image(ImageRef {
            name: name.into(),
            sha256: sha2::Sha256::digest(&bytes).into(),
        })
    };
    let one = add("one.png", [30, 110, 210, 255]);
    let two = add("two.png", [210, 110, 30, 255]);
    let path = dir.path().join("library.sqlite");
    let section = |label: &str, background: Option<Background>| Section {
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
                title: "Duet Hymn".into(),
                authors: String::new(),
                copyright: String::new(),
                license: String::new(),
                administrator: String::new(),
                song_number: String::new(),
                variants: Vec::new(),
                sections: vec![
                    section("Verse 1", Some(one)),
                    section("Chorus", Some(two)),
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
                administrator: String::new(),
                song_number: String::new(),
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
                administrator: String::new(),
                song_number: String::new(),
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
        operator.read_with(&cx, |o, _| o.dialog.clone()),
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
        assert_eq!((o.dialog.clone(), o.selected_entry), (None, None));
    });

    // Right-click menu, then Ctrl+Del (no confirmation).
    let position = cx.debug_bounds("schedule-item-1").unwrap().center();
    cx.simulate_mouse_down(position, gpui::MouseButton::Right, Default::default());
    cx.simulate_mouse_up(position, gpui::MouseButton::Right, Default::default());
    click(&mut cx, "item-menu-remove");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dialog.clone()),
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

/// Opens `path` through the toolbar Open button and the native dialog.
fn open_file(cx: &mut VisualTestContext, operator: &Entity<Operator>, path: &std::path::Path) {
    click(cx, "open-schedule");
    assert!(cx.did_prompt_for_paths(), "Open asks for a file");
    let path = path.to_path_buf();
    cx.simulate_path_prompt_response(move |_| Some(vec![path]));
    cx.run_until_parked();
    let _ = operator;
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
    assert!(
        cx.did_prompt_for_new_path(),
        "untitled: the native Save dialog"
    );
    assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    operator.read_with(&cx, |o, _| {
        assert!(o.schedule.path().is_none());
        assert!(o.schedule.is_dirty(), "a cancelled dialog saves nothing");
    });
    assert!(std::fs::read_dir(dir.path()).unwrap().all(|e| {
        e.unwrap()
            .path()
            .extension()
            .is_none_or(|x| x != "selaschedule")
    }));
    cx.simulate_keystrokes("ctrl-s");
    let file = dir.path().join("Sunday.selaschedule");
    let typed = dir.path().join("Sunday");
    cx.simulate_new_path_selection(move |_| Some(typed));
    assert_control(&mut cx, &operator, Control::AddToSchedule);
    settle(&mut cx, &operator, "saved", |o| {
        o.schedule.path().is_some() && !o.schedule.is_dirty()
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| o
            .schedule
            .path()
            .map(std::path::Path::to_path_buf)),
        Some(file.clone()),
        "the extension is added once"
    );
    assert_eq!(
        operator.read_with(&cx, |o, _| o.window_title.clone()),
        "Sunday.selaschedule — Sela"
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
        operator.read_with(&cx, |o, _| o.dialog.clone()),
        Some(Dialog::Unsaved(Then::Open))
    );
    cx.simulate_keystrokes("enter");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.dialog, None, "Cancel is the default");
        assert_eq!(entry_titles(o), [SECOND_SONG]);
    });
    click(&mut cx, "open-schedule");
    click(&mut cx, "dialog-confirm");
    assert!(cx.did_prompt_for_paths());
    let picked = file.clone();
    cx.simulate_path_prompt_response(move |options| {
        assert!(options.files && !options.directories && !options.multiple);
        Some(vec![picked])
    });
    settle(&mut cx, &operator, "reopened", |o| {
        o.schedule_message.as_deref() == Some("Opened “Sunday.selaschedule”")
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

    // A saved schedule saves again without asking for a file name.
    click(&mut cx, "schedule-down");
    cx.simulate_keystrokes("ctrl-s");
    assert!(!cx.did_prompt_for_new_path());
    assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);
    settle(&mut cx, &operator, "saved again", |o| {
        !o.schedule.is_dirty()
    });
    let stored = sela::schedule_file::read(&file).unwrap();
    assert_eq!(stored.title, "Sunday");
    assert_eq!(stored.items[1].source, pinned);
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
    // Closing writes the layout first; store it so only the schedule guard is under test.
    settle(&mut cx, &operator, "settings loaded", |o| {
        o.settings.loaded_for_test()
    });
    cx.update(|window, cx| operator.update(cx, |o, _| o.save_layout(window)));
    settle(&mut cx, &operator, "layout stored", |o| o.settings_idle());
    assert!(may_close(&mut cx), "a clean schedule closes");
    add_song(&mut cx, &operator, "Signal Hymn");
    assert!(!may_close(&mut cx));
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dialog.clone()),
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
        operator.read_with(&cx, |o, _| o.dialog.clone()),
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
    let evening = dir.path().join("Evening.selaschedule");
    cx.simulate_new_path_selection(move |_| Some(evening));
    cx.run_until_parked();
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
    settle(&mut cx, &operator, "saved", |o| {
        !o.schedule.is_dirty() && o.settings_idle()
    });
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
            assert_eq!(focused.as_ref(), o.song_rows.get(&row_index))
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
    assert_eq!(operator.read_with(&cx, highlight_of), Some(4));
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
                administrator: String::new(),
                song_number: String::new(),
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
        operator.read_with(&cx, |o, _| o.dialog.clone()),
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
        assert_eq!(
            o.selected_song.map(|v| v.id),
            Some(o.catalog[0].0.id),
            "selection moves to the neighbour row"
        );
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
    let file = dir.path().join("After delete.selaschedule");
    let target = file.clone();
    cx.simulate_new_path_selection(move |_| Some(target));
    settle(&mut cx, &operator, "schedule saved", |o| {
        o.schedule.path().is_some() && !o.schedule.is_dirty()
    });
    let stored = sela::schedule_file::read(&file).unwrap();
    assert_eq!(stored.items.len(), 1);
    assert_eq!(stored.items[0].source, pinned);
    assert_eq!(stored.items[0].song.title, "Signal Hymn");
    assert_eq!(stored.items[0].song.sections[1].lyrics, "Original refrain");
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
            loader: settings::default_loader(),
            schedule_writer: sela::schedule_file::default_writer(),
            initial_layout: None,
            media_locator: None,
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
        // OK does nothing until the settings file has loaded, and that reply
        // can trail the monitor scan when the machine is busy.
        settle(&mut cx, &operator, "monitors and settings", |o| {
            o.settings.monitors.is_some() && o.settings_loaded()
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
            operator.read_with(&vcx, |o, _| o.dialog.clone()),
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
        assert_eq!(operator.read_with(&vcx, |o, _| o.dialog.clone()), None);
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
    fn layout_is_saved_on_close_and_restored_on_restart(cx: &mut TestAppContext) {
        use gpui::{MouseButton, point};
        let dir = tempfile::tempdir().unwrap();
        let (mut vcx, operator, _) = start(cx, &dir, settings::default_writer());
        settle(&mut vcx, &operator, "loaded", |o| {
            o.settings.loaded_for_test()
        });
        let from = vcx.debug_bounds("split-live").unwrap().center();
        vcx.simulate_mouse_down(from, MouseButton::Left, Default::default());
        vcx.simulate_mouse_move(
            from + point(px(-15.), px(0.)),
            MouseButton::Left,
            Default::default(),
        );
        vcx.simulate_mouse_move(
            from + point(px(-120.), px(0.)),
            MouseButton::Left,
            Default::default(),
        );
        vcx.simulate_mouse_up(
            from + point(px(-120.), px(0.)),
            MouseButton::Left,
            Default::default(),
        );
        click(&mut vcx, "collapse-resources");
        let ratios = operator.read_with(&vcx, |o, _| o.ratios);
        assert_ne!(ratios, [0.24, 0.62, 0.62]);
        let closed = vcx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx)));
        assert!(!closed, "close waits for the layout write");
        settle(&mut vcx, &operator, "layout written", |_| {
            file(&dir).is_some_and(|text| text.contains("pane_ratios="))
        });
        let text = file(&dir).unwrap();
        assert!(text.contains("resources_collapsed=1\n") && text.contains("window="));
        drop(vcx);

        let saved = settings::peek_layout(&settings::path_in(dir.path()));
        let (mut config, _) = config(settings::default_writer());
        config.initial_layout = saved;
        let (vcx, operator, _) = with_settings(cx, Some(library_with_song(&dir)), "apply", config);
        let restored = operator.read_with(&vcx, |o, _| (o.ratios, o.collapsed));
        assert!(restored.1);
        for (a, b) in restored.0.iter().zip(ratios) {
            assert!((a - b).abs() < 1e-3, "{a} vs {b}");
        }
    }

    #[gpui::test]
    fn corrupt_layout_starts_with_the_default_layout_and_is_overwritten(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = settings::path_in(dir.path());
        std::fs::write(&path, "sela-settings 1\npane_ratios=[oops\nwindow=1,2\n").unwrap();
        assert_eq!(settings::peek_layout(&path), None);
        let (mut vcx, operator, _) = start(cx, &dir, settings::default_writer());
        settle(&mut vcx, &operator, "loaded", |o| {
            o.settings.loaded_for_test()
        });
        assert_eq!(
            operator.read_with(&vcx, |o, _| (o.ratios, o.collapsed)),
            ([0.24, 0.62, 0.62], false)
        );
        let closed = vcx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx)));
        assert!(!closed);
        settle(&mut vcx, &operator, "layout rewritten", |_| {
            file(&dir).is_some_and(|text| text.contains("pane_ratios=2400,6200,6200"))
        });
        assert!(!file(&dir).unwrap().contains("oops"));
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
        assert_eq!(operator.read_with(&vcx, |o, _| o.dialog.clone()), None);
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
        assert_eq!(operator.read_with(&vcx, |o, _| o.dialog.clone()), None);
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

    fn start_loading(
        cx: &mut TestAppContext,
        dir: &tempfile::TempDir,
        writer: settings::Writer,
        gate: Arc<std::sync::atomic::AtomicBool>,
    ) -> (VisualTestContext, Entity<Operator>, Arc<MonitorSlot>) {
        let (mut config, slot) = config(writer);
        config.loader = Arc::new(move |path| {
            while !gate.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            settings::load(path)
        });
        let (mut cx, operator, _) =
            with_settings(cx, Some(library_with_song(dir)), "apply", config);
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "monitors", |o| {
            o.settings.monitors.is_some()
        });
        (cx, operator, slot)
    }

    fn windows(cx: &mut VisualTestContext) -> usize {
        cx.read(|cx| cx.windows().len())
    }

    #[gpui::test]
    fn quit_after_ok_waits_for_the_write_and_keeps_painting(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let writer: settings::Writer = Arc::new(|path, settings| {
            std::thread::sleep(Duration::from_secs(2));
            settings::save(path, settings)
        });
        let (mut vcx, operator, _slot) = start(cx, &dir, writer);
        open_options(&mut vcx);
        click(&mut vcx, "options-language-id");
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-confirm");
        let started = Instant::now();
        let closed = vcx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx)));
        assert!(!closed, "close waits for the pending save");
        assert!(started.elapsed() < Duration::from_millis(1000));
        assert!(
            operator.read_with(&vcx, |o, _| o
                .settings_note()
                .is_some_and(|note| note.contains("Menyimpan pengaturan")
                    || note.contains("Saving settings")))
        );
        assert_eq!(windows(&mut vcx), 1);
        let end = Instant::now() + Duration::from_secs(10);
        while file(&dir).is_none() {
            assert!(Instant::now() < end, "bounded wait for the write");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !file(&dir).unwrap().contains("pane_ratios="),
            "the layout comes with the close"
        );
        // The close also writes the layout, a second 2 s write; the fake
        // clock waits for it in real time, so the bound is not run out.
        while windows(&mut vcx) > 0 {
            assert!(Instant::now() < end, "bounded wait for the layout write");
            close_step(&mut vcx, &operator, Duration::from_millis(200));
        }
        assert_eq!(windows(&mut vcx), 0, "closed once the write landed");
        let text = file(&dir).unwrap();
        assert!(text.contains("DISPLAY6") && text.contains("language=id"));
        assert!(
            text.contains("pane_ratios="),
            "the close waited for the layout write: {text}"
        );
        i18n::set_language(Language::English);
    }

    #[gpui::test]
    fn quit_after_a_stuck_write_force_closes_after_the_bound(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let hold = release.clone();
        let writer: settings::Writer = Arc::new(move |path, settings| {
            while !hold.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            settings::save(path, settings)
        });
        let (mut vcx, operator, _slot) = start(cx, &dir, writer);
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-confirm");
        assert!(!vcx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx))));
        vcx.executor().advance_clock(Duration::from_secs(2));
        vcx.run_until_parked();
        assert_eq!(windows(&mut vcx), 1, "still within the bound");
        vcx.executor().advance_clock(Duration::from_secs(4));
        vcx.run_until_parked();
        assert_eq!(
            windows(&mut vcx),
            0,
            "a stuck writer cannot trap the window"
        );
        release.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn gated_writer() -> (settings::Writer, Arc<std::sync::atomic::AtomicBool>) {
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let hold = release.clone();
        let writer: settings::Writer = Arc::new(move |path, settings| {
            while !hold.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            settings::save(path, settings)
        });
        (writer, release)
    }

    /// Releases the writer and waits until the operator has seen the reply and
    /// resolved any close request. The file appearing is not enough: the reply
    /// is polled on the operator's own clock, so a single fixed tick races.
    fn drain_writer(
        vcx: &mut VisualTestContext,
        operator: &Entity<Operator>,
        dir: &tempfile::TempDir,
        release: &std::sync::atomic::AtomicBool,
    ) {
        release.store(true, std::sync::atomic::Ordering::SeqCst);
        let end = Instant::now() + Duration::from_secs(10);
        while file(dir).is_none() {
            assert!(Instant::now() < end, "bounded wait for the write");
            std::thread::sleep(Duration::from_millis(5));
        }
        while !operator.read_with(vcx, |o, _| o.settings_idle() && !o.close_pending()) {
            assert!(Instant::now() < end, "bounded wait for the reply");
            close_step(vcx, operator, Duration::from_millis(200));
        }
        vcx.run_until_parked();
    }

    fn quit(vcx: &mut VisualTestContext, operator: &Entity<Operator>) -> bool {
        vcx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx)))
    }

    #[gpui::test]
    fn schedule_edit_during_the_settings_wait_prompts_instead_of_closing(cx: &mut TestAppContext) {
        use crate::operator::{Dialog, Then};
        let dir = tempfile::tempdir().unwrap();
        let (writer, release) = gated_writer();
        let (mut vcx, operator, _slot) = start(cx, &dir, writer);
        settle(&mut vcx, &operator, "catalog", |o| o.catalog.len() == 1);
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-confirm");
        assert!(!quit(&mut vcx, &operator));
        add_song(&mut vcx, &operator, "Signal Hymn");
        drain_writer(&mut vcx, &operator, &dir, &release);
        assert_eq!(windows(&mut vcx), 1, "the edit is not lost");
        operator.read_with(&vcx, |o, _| {
            assert_eq!(o.dialog, Some(Dialog::Unsaved(Then::Quit)));
            assert!(!o.close_pending());
        });
    }

    #[gpui::test]
    fn dont_save_during_the_settings_wait_closes_without_asking_again(cx: &mut TestAppContext) {
        use crate::operator::{Dialog, Then};
        let dir = tempfile::tempdir().unwrap();
        let (writer, release) = gated_writer();
        let (mut vcx, operator, _slot) = start(cx, &dir, writer);
        settle(&mut vcx, &operator, "catalog", |o| o.catalog.len() == 1);
        add_song(&mut vcx, &operator, "Signal Hymn");
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-confirm");
        assert!(!quit(&mut vcx, &operator));
        assert_eq!(
            operator.read_with(&vcx, |o, _| o.dialog.clone()),
            Some(Dialog::Unsaved(Then::Quit))
        );
        click(&mut vcx, "dialog-confirm");
        release.store(true, std::sync::atomic::Ordering::SeqCst);
        let end = Instant::now() + Duration::from_secs(10);
        while windows(&mut vcx) > 0 {
            assert!(Instant::now() < end, "Don't Save closes the window");
            close_step(&mut vcx, &operator, Duration::from_millis(200));
        }
    }

    /// Don't Save covers the schedule, not an import preview opened while
    /// the settings write was still pending (PP-IMPF-09).
    #[gpui::test]
    fn an_import_opened_during_the_settings_wait_after_dont_save_keeps_the_window(
        cx: &mut TestAppContext,
    ) {
        use crate::operator::Dialog;
        use crate::operator::song_import::Phase;
        let dir = tempfile::tempdir().unwrap();
        let folder = super::song_import::batch_dir(&dir);
        let (writer, release) = gated_writer();
        let (mut vcx, operator, _slot) = start(cx, &dir, writer);
        settle(&mut vcx, &operator, "catalog", |o| o.catalog.len() == 1);
        add_song(&mut vcx, &operator, "Signal Hymn");
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-confirm");
        assert!(!quit(&mut vcx, &operator));
        click(&mut vcx, "dialog-confirm");
        assert_eq!(windows(&mut vcx), 1, "still waiting for the write");
        super::song_import::import_folder(&mut vcx, &operator, folder);
        super::song_import::settle_review(&mut vcx, &operator);
        drain_writer(&mut vcx, &operator, &dir, &release);
        assert_eq!(windows(&mut vcx), 1, "the open import is not lost");
        operator.read_with(&vcx, |o, _| {
            assert_eq!(o.dialog, Some(Dialog::ImportSongs));
            assert_eq!(o.song_import.phase, Phase::Review);
            assert_eq!(
                o.song_import.quit_note.as_deref(),
                Some("Finish or cancel the import before quitting.")
            );
        });
    }

    #[gpui::test]
    fn an_edit_after_dont_save_during_the_settings_wait_prompts_again(cx: &mut TestAppContext) {
        use crate::operator::{Dialog, Then};
        let dir = tempfile::tempdir().unwrap();
        let (writer, release) = gated_writer();
        let (mut vcx, operator, _slot) = start(cx, &dir, writer);
        settle(&mut vcx, &operator, "catalog", |o| o.catalog.len() == 1);
        add_song(&mut vcx, &operator, "Signal Hymn");
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-confirm");
        assert!(!quit(&mut vcx, &operator));
        click(&mut vcx, "dialog-confirm");
        assert_eq!(windows(&mut vcx), 1, "still waiting for the write");
        add_song(&mut vcx, &operator, "Signal Hymn");
        drain_writer(&mut vcx, &operator, &dir, &release);
        assert_eq!(windows(&mut vcx), 1, "the later edit is not lost");
        assert_eq!(
            operator.read_with(&vcx, |o, _| o.dialog.clone()),
            Some(Dialog::Unsaved(Then::Quit))
        );
    }

    #[gpui::test]
    fn second_quit_then_cancel_drops_the_close_request(cx: &mut TestAppContext) {
        use crate::operator::{Dialog, Then};
        let dir = tempfile::tempdir().unwrap();
        let (writer, release) = gated_writer();
        let (mut vcx, operator, _slot) = start(cx, &dir, writer);
        settle(&mut vcx, &operator, "catalog", |o| o.catalog.len() == 1);
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-confirm");
        assert!(!quit(&mut vcx, &operator));
        add_song(&mut vcx, &operator, "Signal Hymn");
        assert!(!quit(&mut vcx, &operator));
        assert_eq!(
            operator.read_with(&vcx, |o, _| o.dialog.clone()),
            Some(Dialog::Unsaved(Then::Quit))
        );
        click(&mut vcx, "dialog-cancel");
        assert!(!operator.read_with(&vcx, |o, _| o.close_pending()));
        drain_writer(&mut vcx, &operator, &dir, &release);
        assert_eq!(windows(&mut vcx), 1, "Cancel keeps the window open");
        assert_eq!(operator.read_with(&vcx, |o, _| o.dialog.clone()), None);
    }

    #[gpui::test]
    fn stuck_writer_with_a_dirty_schedule_prompts_after_the_bound(cx: &mut TestAppContext) {
        use crate::operator::{Dialog, Then};
        let dir = tempfile::tempdir().unwrap();
        let (writer, release) = gated_writer();
        let (mut vcx, operator, _slot) = start(cx, &dir, writer);
        settle(&mut vcx, &operator, "catalog", |o| o.catalog.len() == 1);
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-confirm");
        assert!(!quit(&mut vcx, &operator));
        add_song(&mut vcx, &operator, "Signal Hymn");
        vcx.executor().advance_clock(Duration::from_secs(7));
        vcx.run_until_parked();
        assert_eq!(windows(&mut vcx), 1);
        assert_eq!(
            operator.read_with(&vcx, |o, _| o.dialog.clone()),
            Some(Dialog::Unsaved(Then::Quit))
        );
        click(&mut vcx, "dialog-confirm");
        assert_eq!(windows(&mut vcx), 0, "discarding after the bound closes");
        release.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    #[gpui::test]
    fn options_before_loaded_never_rewrites_the_saved_monitor(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let saved = "sela-settings 1\noutput_monitor=\\\\.\\DISPLAY6\nlanguage=id\n";
        std::fs::write(settings::path_in(dir.path()), saved).unwrap();
        let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (mut vcx, operator, slot) =
            start_loading(cx, &dir, settings::default_writer(), gate.clone());
        open_options(&mut vcx);
        assert!(vcx.debug_bounds("options-loading").is_some());
        click(&mut vcx, "options-monitor-1");
        click(&mut vcx, "options-language-en");
        click(&mut vcx, "dialog-confirm");
        vcx.run_until_parked();
        assert_eq!(
            file(&dir).as_deref(),
            Some(saved),
            "inert OK writes nothing"
        );
        gate.store(true, std::sync::atomic::Ordering::SeqCst);
        settle(&mut vcx, &operator, "loaded", |o| {
            o.settings.current.output_monitor.is_some()
        });
        assert_eq!(slot.get(), r"name:\\.\DISPLAY6");
        vcx.run_until_parked();
        assert_eq!(file(&dir).as_deref(), Some(saved));
        assert_eq!(i18n::language(), Language::Indonesian);
        i18n::set_language(Language::English);
    }

    #[gpui::test]
    fn options_open_during_load_rebase_on_loaded_and_keep_a_missing_monitor(
        cx: &mut TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let saved = "sela-settings 1\noutput_monitor=\\\\.\\DISPLAY99\n";
        std::fs::write(settings::path_in(dir.path()), saved).unwrap();
        let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (mut vcx, operator, _slot) =
            start_loading(cx, &dir, settings::default_writer(), gate.clone());
        open_options(&mut vcx);
        gate.store(true, std::sync::atomic::Ordering::SeqCst);
        settle(&mut vcx, &operator, "loaded", |o| {
            o.settings.draft == Some(Some(r"\\.\DISPLAY99".to_string()))
        });
        assert!(vcx.debug_bounds("options-loading").is_none());
        click(&mut vcx, "dialog-confirm");
        vcx.run_until_parked();
        assert_eq!(file(&dir).as_deref(), Some(saved), "missing monitor kept");
    }

    #[gpui::test]
    fn the_song_tab_takes_no_input_until_settings_load(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (mut vcx, operator, _slot) =
            start_loading(cx, &dir, settings::default_writer(), gate.clone());
        open_options(&mut vcx);
        click(&mut vcx, "options-tab-song");
        assert!(vcx.debug_bounds("options-song-loading").is_some());
        click(&mut vcx, "options-license-info");
        vcx.simulate_input("CCLI 9");
        let license = |vcx: &mut VisualTestContext| {
            operator.read_with(vcx, |o, cx| {
                o.settings.song.license.read(cx).text().to_owned()
            })
        };
        assert_eq!(license(&mut vcx), "", "an edit OK would discard");
        gate.store(true, std::sync::atomic::Ordering::SeqCst);
        settle(&mut vcx, &operator, "loaded", |o| o.settings_loaded());
        vcx.run_until_parked();
        assert!(vcx.debug_bounds("options-song-loading").is_none());
        click(&mut vcx, "options-license-info");
        vcx.simulate_input("CCLI 9");
        assert_eq!(license(&mut vcx), "CCLI 9");
    }

    #[gpui::test]
    fn late_loaded_never_overwrites_an_applied_choice(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (mut vcx, operator, slot) =
            start_loading(cx, &dir, settings::default_writer(), gate.clone());
        // Settle the loader gate open only after the choice is applied.
        operator.update(&mut vcx, |o, cx| {
            o.settings.force_loaded_for_test();
            cx.notify();
        });
        open_options(&mut vcx);
        click(&mut vcx, "options-monitor-2");
        click(&mut vcx, "dialog-confirm");
        gate.store(true, std::sync::atomic::Ordering::SeqCst);
        settle(&mut vcx, &operator, "file written", |_| {
            file(&dir).is_some()
        });
        vcx.run_until_parked();
        assert_eq!(slot.get(), r"name:\\.\DISPLAY6");
        assert_eq!(
            operator.read_with(&vcx, |o, _| o.settings.current.output_monitor.clone()),
            Some(r"\\.\DISPLAY6".to_string())
        );
        assert!(file(&dir).unwrap().contains("DISPLAY6"));
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
    fn menu_contents_match_the_captures_plus_tracked_deviations(cx: &mut TestAppContext) {
        let (mut cx, _operator, _) = fixture(cx);
        cx.simulate_resize(size(px(1280.), px(800.)));
        // EW8-OBS-053 plus the menu-p/v/h.png captures; the New ▸ submenu is
        // flattened into a header row followed by Song.../Schedule.
        const EW_FILE: [&str; 11] = [
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
        ];
        // Rows Sela adds that the captures do NOT show: a known deviation,
        // not parity. Each names its docs/parity-pending.md check; it goes
        // once EasyWorship's import entry point is observed.
        // (after row, added rows, tracking id)
        const DEVIATIONS: [(&str, &[&str], &str); 1] = [(
            "bar-file-close",
            &[
                "bar-file-import",
                "bar-file-import-songs",
                "bar-file-import-folder",
                "bar-file-import-easyworship",
            ],
            "PP-IMPF-01, PP-EWL-01 (UNOBSERVED, M1-14, M4-07)",
        )];
        let mut file: Vec<&str> = Vec::new();
        for row in EW_FILE {
            file.push(row);
            for (after, added, tracking) in DEVIATIONS {
                if after == row {
                    assert!(!tracking.is_empty());
                    file.extend_from_slice(added);
                }
            }
        }
        let menus: [(&str, &[&str]); 6] = [
            ("alt-f", &file),
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
                    "bar-live-stage",
                    "bar-live-stage-message",
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
        assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);
        assert_eq!(operator.read_with(&cx, |o, _| o.bar_menu), None);

        // Options opens the existing Options dialog.
        cx.simulate_keystrokes("alt-e enter");
        assert_eq!(
            operator.read_with(&cx, |o, _| o.dialog.clone()),
            Some(Dialog::Options)
        );
        click(&mut cx, "dialog-cancel");
        assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);

        // Exit dispatches Quit, the same as Ctrl+Q (probe counts; window stays).
        cx.simulate_keystrokes("alt-f up enter");
        assert_eq!(seen.get(), 1);
    }

    #[gpui::test]
    fn disabled_items_are_rendered_but_inert(cx: &mut TestAppContext) {
        let (mut cx, operator, seen) = fixture(cx);
        cx.simulate_resize(size(px(1280.), px(800.)));
        cx.simulate_keystrokes("alt-f");
        for id in ["bar-file-open-recent", "bar-file-reports"] {
            click(&mut cx, id);
            assert_eq!(
                operator.read_with(&cx, |o, _| o.bar_menu.map(|m| m.top)),
                Some(TopMenu::File),
                "{id} did not close the menu"
            );
        }
        assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);
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

        let input = operator.read_with(&cx, |o, _| o.search_input.clone());
        cx.update(|window, cx| input.read(cx).focus_handle(cx).focus(window, cx));
        cx.simulate_keystrokes("f e l p v h");
        cx.update(|_, cx| assert_eq!(input.read(cx).text(), "felpvh"));
        cx.simulate_keystrokes("alt-f alt-l");
        assert_eq!(operator.read_with(&cx, |o, _| o.bar_menu), None);
        cx.update(|_, cx| assert_eq!(input.read(cx).text(), "felpvh"));
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
fn normalize_after_a_failed_cue_clears_the_failed_indicator(cx: &mut TestAppContext) {
    use sela::preparation::cue::{Built, CuePreparer};
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_fixture(cx, &dir);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    let failing = CuePreparer::with_builder(|_, _| -> Result<Built, _> {
        Err(sela::scene::PrepareError::InvalidScene)
    })
    .unwrap();
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(failing));
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "the failed cue", |o| o.cue_failed);
    operator.update(&mut cx, |o, _| {
        o.cue_preparer = Some(CuePreparer::new().unwrap());
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));
    click(&mut cx, "normalize-text");
    settle(&mut cx, &operator, "the re-sized cue presented", |o| {
        submitted(o) > sent && o.pending_cues.is_empty()
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.on_screen(), Some("Signal Hymn · Verse 1"));
        assert!(!o.cue_failed, "the re-sent slide prepared");
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
            Some("Still preparing earlier slides; this one follows")
        );
        assert!(
            o.deferred_send.is_some_and(|d| d.busy),
            "the newest is held"
        );
    });
    // The queue plus the held newest request.
    for _ in 0..=LIVE_DEPTH {
        open.send(()).unwrap();
    }
    settle(&mut cx, &operator, "queue drained", |o| {
        o.pending_cues.is_empty()
            && o.deferred_send.is_none()
            && o.on_screen() == Some("Signal Hymn · Chorus")
    });
    // The final selection is still accepted after the burst.
    open.send(()).unwrap();
    operator.update(&mut cx, |o, _| o.send(2));
    settle(&mut cx, &operator, "bridge on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Bridge")
    });
}

fn show_verse_then_black(
    cx: &mut TestAppContext,
    dir: &tempfile::TempDir,
) -> (VisualTestContext, Entity<Operator>) {
    let (mut cx, operator) = live_fixture(cx, dir);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });
    (cx, operator)
}

fn poll_for(cx: &mut VisualTestContext, operator: &Entity<Operator>, rounds: usize) {
    for _ in 0..rounds {
        operator.update(cx, |o, cx| o.poll(cx));
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn live_double_click_under_black_never_reveals_the_previous_slide(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = show_verse_then_black(cx, &dir);
    let (open, preparer) = gated_preparer(Default::default());
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(preparer));

    operator.update(&mut cx, |o, _| o.click_live(1, false));
    operator.update(&mut cx, |o, _| o.click_live(1, true));
    poll_for(&mut cx, &operator, 40);
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.pending_cues.len(), 1, "the double-click adds no cue");
        assert_eq!(o.masks.layer(), Layer::Black, "intent stays Black");
        assert_eq!(confirmed_mask(o), Some(Layer::Black), "never revealed");
        assert_eq!(o.live_slide, Some(0));
    });

    open.send(()).unwrap();
    let mut revealed_with = None;
    for _ in 0..400 {
        poll_for(&mut cx, &operator, 1);
        operator.read_with(&cx, |o, _| {
            if confirmed_mask(o) == Some(Layer::None) {
                revealed_with = Some(o.on_screen().map(str::to_owned));
            }
        });
        if revealed_with.is_some() {
            break;
        }
    }
    assert_eq!(
        revealed_with,
        Some(Some("Signal Hymn · Chorus".to_owned())),
        "the mask lifts only over the clicked slide"
    );
    operator.read_with(&cx, |o, _| assert_eq!(o.masks.layer(), Layer::None));
}

#[gpui::test]
fn failed_live_double_click_keeps_the_mask_and_reports(cx: &mut TestAppContext) {
    use sela::preparation::cue::{Built, CuePreparer};
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = show_verse_then_black(cx, &dir);
    let failing = CuePreparer::with_builder(|_, _| -> Result<Built, _> {
        Err(sela::scene::PrepareError::InvalidScene)
    })
    .unwrap();
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(failing));
    let sent = operator.read_with(&cx, |o, _| submitted(o));

    operator.update(&mut cx, |o, _| o.click_live(1, true));
    settle(&mut cx, &operator, "failure reported", |o| {
        o.live_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Slide could not be prepared"))
    });
    poll_for(&mut cx, &operator, 20);
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.masks.layer(), Layer::Black);
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
        assert_eq!(o.indicator(Mask::Black), Indicator::On);
        assert_eq!(o.unmask.armed(), None);
        assert_eq!(submitted(o), sent);
        assert_eq!(o.on_screen(), Some("Signal Hymn · Verse 1"));
    });
}

#[gpui::test]
fn double_click_after_the_first_cue_was_substituted_keeps_the_mask(cx: &mut TestAppContext) {
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
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    settle(&mut cx, &operator, "output-size prefetch", |o| {
        o.background_job.is_none() && !o.backgrounds.is_empty()
    });
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Backdrop Hymn · Verse 1")
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });

    // The first click settles as a missing-image substitute before the second.
    operator.update(&mut cx, |o, _| o.click_live(1, false));
    settle(&mut cx, &operator, "chorus on screen", |o| {
        o.on_screen() == Some("Backdrop Hymn · Chorus") && o.pending_cues.is_empty()
    });
    operator.update(&mut cx, |o, _| o.click_live(1, true));
    poll_for(&mut cx, &operator, 40);
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.masks.layer(), Layer::Black, "intent stays Black");
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
        assert_eq!(o.indicator(Mask::Black), Indicator::On);
        assert_eq!(o.unmask.armed(), None);
    });
}

#[gpui::test]
fn newer_deferred_request_cancels_the_double_click_unmask(cx: &mut TestAppContext) {
    use sela::preparation::cue::{Built, CuePreparer};
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = show_verse_then_black(cx, &dir);
    let (open, preparer) = gated_preparer(Default::default());
    operator.update(&mut cx, |o, _| o.cue_preparer = Some(preparer));

    // B: a double-click in preparation; C: a newer step waiting on faces.
    operator.update(&mut cx, |o, _| o.click_live(1, true));
    let resolved = operator.update(&mut cx, |o, _| {
        assert!(o.unmask.armed().is_some());
        let resolved = o.live.as_mut().unwrap().resolved.take();
        o.send(2);
        assert!(o.deferred_send.is_some());
        assert_eq!(o.unmask.armed(), None, "a newer request cancels it");
        resolved
    });
    open.send(()).unwrap();
    settle(&mut cx, &operator, "B on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });
    poll_for(&mut cx, &operator, 20);
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.masks.layer(), Layer::Black, "B landing never unmasks");
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
    });

    // C fails: the mask still stays.
    let failing = CuePreparer::with_builder(|_, _| -> Result<Built, _> {
        Err(sela::scene::PrepareError::InvalidScene)
    })
    .unwrap();
    operator.update(&mut cx, |o, _| {
        o.cue_preparer = Some(failing);
        o.live.as_mut().unwrap().resolved = resolved;
        o.retry_deferred();
    });
    settle(&mut cx, &operator, "C failure reported", |o| {
        o.live_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Slide could not be prepared"))
    });
    poll_for(&mut cx, &operator, 20);
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.masks.layer(), Layer::Black);
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
        assert_eq!(o.on_screen(), Some("Signal Hymn · Chorus"));
    });
}

#[gpui::test]
fn same_request_retry_keeps_the_double_click_unmask(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = show_verse_then_black(cx, &dir);
    let resolved = operator.update(&mut cx, |o, _| o.live.as_mut().unwrap().resolved.take());

    operator.update(&mut cx, |o, _| o.click_live(1, false));
    operator.update(&mut cx, |o, _| o.click_live(1, true));
    operator.read_with(&cx, |o, _| {
        assert!(o.deferred_send.is_some());
        assert!(
            o.unmask.armed().is_some(),
            "the double-click adds no request"
        );
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
    });
    operator.update(&mut cx, |o, _| {
        o.live.as_mut().unwrap().resolved = resolved;
        o.retry_deferred();
    });
    settle(
        &mut cx,
        &operator,
        "mask lifted over the clicked slide",
        |o| confirmed_mask(o) == Some(Layer::None) && o.on_screen() == Some("Signal Hymn · Chorus"),
    );
    operator.read_with(&cx, |o, _| assert_eq!(o.masks.layer(), Layer::None));
}

#[derive(Clone, Copy)]
enum Refresh {
    Surface,
    Fonts,
}

/// A masked double-click whose cue is deferred on an uncached image must
/// survive a surface/font refresh as the SAME request: the refresh rebuilds
/// the double-clicked slide with its own generation, so the slide on screen
/// never inherits the unmask intent (FOUNDATION-R3-PREP-01).
fn masked_deferred_request_survives_a_refresh(cx: &mut TestAppContext, refresh: Refresh) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_two_backgrounds(&dir)), "apply");
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
    settle(&mut cx, &operator, "output-size prefetch", |o| {
        o.background_job.is_none() && !o.backgrounds.is_empty()
    });
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Duet Hymn · Verse 1")
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });

    // The Chorus image is no longer cached: the double-click defers, masked.
    operator.update(&mut cx, |o, _| {
        o.backgrounds = sela::images::BackgroundCache::new(sela::images::BACKGROUND_BUDGET)
    });
    operator.update(&mut cx, |o, _| o.click_live(1, false));
    operator.update(&mut cx, |o, _| o.click_live(1, true));
    let armed = operator.read_with(&cx, |o, _| {
        assert_eq!(o.deferred_send.map(|d| d.index), Some(1));
        assert_eq!(o.masks.layer(), Layer::Black);
        o.unmask.armed()
    });
    assert!(armed.is_some());

    // The refresh must rebuild THAT request, not the slide on screen.
    match refresh {
        Refresh::Surface => {
            operator.update(&mut cx, |o, _| o.resurface());
            operator.update(&mut cx, |o, cx| o.poll(cx));
        }
        Refresh::Fonts => {
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
                    let landed = formats.iter().map(fonts::Resolved::bundled).collect();
                    o.fonts_resolved(version, landed, cx);
                })
            });
        }
    }
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            o.deferred_send.map(|d| d.index),
            Some(1),
            "the refresh keeps the double-clicked request"
        );
        assert_eq!(o.unmask.armed(), armed, "the intent stays with it");
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
    });

    // The image lands: the mask lifts only over the double-clicked slide.
    let mut revealed_with = None;
    for _ in 0..400 {
        poll_for(&mut cx, &operator, 1);
        operator.read_with(&cx, |o, _| {
            if confirmed_mask(o) == Some(Layer::None) {
                revealed_with = Some(o.on_screen().map(str::to_owned));
            }
        });
        if revealed_with.is_some() {
            break;
        }
    }
    assert_eq!(
        revealed_with,
        Some(Some("Duet Hymn · Chorus".to_owned())),
        "the mask lifts only over the double-clicked slide"
    );
    operator.read_with(&cx, |o, _| assert_eq!(o.masks.layer(), Layer::None));
}

#[gpui::test]
fn surface_refresh_keeps_the_masked_deferred_request(cx: &mut TestAppContext) {
    masked_deferred_request_survives_a_refresh(cx, Refresh::Surface);
}

#[gpui::test]
fn font_refresh_keeps_the_masked_deferred_request(cx: &mut TestAppContext) {
    masked_deferred_request_survives_a_refresh(cx, Refresh::Fonts);
}

/// Queue acceptance is not presentation: the mask stays owned by the request
/// until the renderer's Applied receipt, so a newer request cancels the
/// pending unmask even after the double-clicked cue was queued
/// (FOUNDATION-R3-PREP-02). The older cue may still complete under Black.
#[gpui::test]
fn newer_request_cancels_the_unmask_before_the_receipt(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_song(&dir)), "slow-scene:400");
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
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });

    // B double-clicked; its terminal receipt stays 400 ms out.
    operator.update(&mut cx, |o, _| o.click_live(1, false));
    operator.update(&mut cx, |o, _| o.click_live(1, true));
    settle(&mut cx, &operator, "B queued with the renderer", |o| {
        o.live_slide == Some(1)
    });
    // C: a newer same-item request, deferred on its fonts.
    operator.update(&mut cx, |o, _| {
        assert_eq!(
            o.masks.layer(),
            Layer::Black,
            "queueing B alone never lifts the mask"
        );
        let resolved = o.live.as_mut().unwrap().resolved.take();
        assert!(resolved.is_some());
        o.send(2);
        assert!(o.deferred_send.is_some());
        o.live.as_mut().unwrap().resolved = resolved;
    });

    // B's Applied lands: B completes under Black, the mask never lifts.
    settle(&mut cx, &operator, "B applied under Black", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });
    poll_for(&mut cx, &operator, 60);
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.masks.layer(), Layer::Black);
        assert_eq!(
            confirmed_mask(o),
            Some(Layer::Black),
            "B's receipt must not unmask after C superseded it"
        );
    });

    // C lands: still masked (no double-click intent belongs to it).
    operator.update(&mut cx, |o, _| o.retry_deferred());
    settle(&mut cx, &operator, "C applied under Black", |o| {
        o.on_screen() == Some("Signal Hymn · Bridge")
    });
    poll_for(&mut cx, &operator, 60);
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.masks.layer(), Layer::Black);
        assert_eq!(confirmed_mask(o), Some(Layer::Black), "C never unmasks");
    });
}

/// A renderer rejection of the double-clicked slide keeps the mask and the
/// previous scene; the empty cue slot is not a success receipt
/// (FOUNDATION-R3-PREP-02). A repeated double-click retries and unmasks.
#[gpui::test]
fn render_failed_double_click_keeps_the_mask_and_the_old_scene(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_song(&dir)), "reject-scene:2");
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
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });

    // The double-clicked chorus fails in the renderer.
    operator.update(&mut cx, |o, _| o.click_live(1, false));
    operator.update(&mut cx, |o, _| o.click_live(1, true));
    settle(&mut cx, &operator, "chorus rejected", |o| {
        o.output
            .as_ref()
            .is_some_and(|out| out.rejected().is_some())
    });
    poll_for(&mut cx, &operator, 40);
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.masks.layer(), Layer::Black);
        assert_eq!(
            confirmed_mask(o),
            Some(Layer::Black),
            "a failed presentation never lifts the mask"
        );
        assert_eq!(
            o.on_screen(),
            Some("Signal Hymn · Verse 1"),
            "the rejected slide never replaces the live scene"
        );
    });

    // Retried by hand: the stale rejection must not poison the new intent.
    operator.update(&mut cx, |o, _| o.click_live(1, false));
    operator.update(&mut cx, |o, _| o.click_live(1, true));
    settle(&mut cx, &operator, "unmasked over the chorus", |o| {
        confirmed_mask(o) == Some(Layer::None) && o.on_screen() == Some("Signal Hymn · Chorus")
    });
    operator.read_with(&cx, |o, _| assert_eq!(o.masks.layer(), Layer::None));
}

fn masked_verse_with_mode(
    cx: &mut TestAppContext,
    dir: &tempfile::TempDir,
    mode: &'static str,
) -> (VisualTestContext, Entity<Operator>) {
    let (mut cx, operator, _) = with_library(cx, Some(library_with_song(dir)), mode);
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
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });
    (cx, operator)
}

/// Masked A -> delayed B -> double-click A: the OLD confirmation of A has a
/// different submission id than the fresh A, so Black stays on until the
/// fresh A applies. B completes under Black and is never shown unmasked
/// (FOUNDATION-R4-PREP-01).
#[gpui::test]
fn double_click_a_while_b_is_in_flight_never_exposes_b_or_the_stale_a(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    // Scene 1 is A, scene 2 is B (delayed), scene 3 is the fresh A.
    let (mut cx, operator) = masked_verse_with_mode(cx, &dir, "slow-scene-at:2:400");

    operator.update(&mut cx, |o, _| o.click_live(1, false));
    settle(&mut cx, &operator, "B with the renderer", |o| {
        o.output
            .as_ref()
            .is_some_and(|out| out.in_flight().is_some())
    });
    operator.update(&mut cx, |o, _| o.click_live(0, false));
    operator.update(&mut cx, |o, _| o.click_live(0, true));
    operator.read_with(&cx, |o, _| {
        assert!(o.unmask.armed().is_some());
        assert_eq!(
            confirmed_mask(o),
            Some(Layer::Black),
            "the old confirmation of A does not satisfy the new intent"
        );
    });

    let mut saw_b_masked = false;
    let mut revealed_with = None;
    for _ in 0..600 {
        poll_for(&mut cx, &operator, 1);
        operator.read_with(&cx, |o, _| {
            if confirmed_mask(o) == Some(Layer::None) {
                revealed_with = Some(o.on_screen().map(str::to_owned));
            } else if o.on_screen() == Some("Signal Hymn · Chorus") {
                saw_b_masked = true;
            }
        });
        if revealed_with.is_some() {
            break;
        }
    }
    assert!(saw_b_masked, "B completed under Black");
    assert_eq!(
        revealed_with,
        Some(Some("Signal Hymn · Verse 1".to_owned())),
        "the mask lifts only over the fresh A"
    );
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.masks.layer(), Layer::None);
        assert_eq!(o.unmask.armed(), None);
    });
}

/// The same slide version confirmed before the double-click must not lift
/// the mask when the fresh submission of it fails in the renderer
/// (FOUNDATION-R4-PREP-01). A later successful double-click does.
#[gpui::test]
fn same_version_render_failed_retry_never_unmasks(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    // Scene 1 is A, scene 2 is the fresh A (fails), scene 3 succeeds.
    let (mut cx, operator) = masked_verse_with_mode(cx, &dir, "reject-scene:2");

    operator.update(&mut cx, |o, _| o.click_live(0, false));
    operator.update(&mut cx, |o, _| o.click_live(0, true));
    operator.read_with(&cx, |o, _| {
        assert_eq!(confirmed_mask(o), Some(Layer::Black), "old A is not enough");
    });
    settle(&mut cx, &operator, "fresh A rejected", |o| {
        o.output
            .as_ref()
            .is_some_and(|out| out.rejected().is_some())
    });
    for _ in 0..60 {
        poll_for(&mut cx, &operator, 1);
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.masks.layer(), Layer::Black);
            assert_eq!(confirmed_mask(o), Some(Layer::Black), "never unmasked");
        });
    }
    operator.read_with(&cx, |o, _| assert_eq!(o.unmask.armed(), None));

    operator.update(&mut cx, |o, _| o.click_live(0, false));
    operator.update(&mut cx, |o, _| o.click_live(0, true));
    settle(&mut cx, &operator, "unmasked over A", |o| {
        confirmed_mask(o) == Some(Layer::None) && o.on_screen() == Some("Signal Hymn · Verse 1")
    });
}

/// Two songs; the first is live on Verse 1 under Black. Scene 2 is slow.
fn masked_two_songs(
    cx: &mut TestAppContext,
    dir: &tempfile::TempDir,
) -> (VisualTestContext, Entity<Operator>) {
    let (mut cx, operator, _) =
        with_library(cx, Some(library_with_songs(dir)), "slow-scene-at:2:400");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    let row = song_row(&mut cx, &operator, "Signal Hymn");
    click(&mut cx, row);
    settle(&mut cx, &operator, "faces resolved", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.slides.len() == 3 && p.resolved.is_some())
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });
    (cx, operator)
}

/// Double-clicks the chorus while its slow cue is in flight.
fn arm_double_click_on_chorus(cx: &mut VisualTestContext, operator: &Entity<Operator>) {
    operator.update(cx, |o, _| o.click_live(1, false));
    operator.update(cx, |o, _| o.click_live(1, true));
    settle(cx, operator, "chorus with the renderer", |o| {
        o.output
            .as_ref()
            .is_some_and(|out| out.in_flight().is_some())
    });
    operator.read_with(cx, |o, _| assert!(o.unmask.armed().is_some()));
}

/// Polls until the delayed chorus is on screen; the mask never lifts.
fn chorus_lands_under_black(cx: &mut VisualTestContext, operator: &Entity<Operator>) {
    for _ in 0..600 {
        poll_for(cx, operator, 1);
        let landed = operator.read_with(cx, |o, _| {
            assert_eq!(o.masks.layer(), Layer::Black);
            assert_eq!(confirmed_mask(o), Some(Layer::Black), "mask never lifts");
            o.on_screen() == Some("Signal Hymn · Chorus")
        });
        if landed {
            return;
        }
    }
    panic!("the delayed chorus never landed");
}

fn canticle_row(o: &Operator) -> usize {
    o.catalog
        .iter()
        .position(|(_, title)| title == SECOND_SONG)
        .unwrap()
}

/// A Songs Go Live whose load is still pending is the newest request: the
/// older double-click on a live slide must not unmask the previous item.
#[gpui::test]
fn library_go_live_cancels_a_pending_double_click_unmask(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = masked_two_songs(cx, &dir);
    arm_double_click_on_chorus(&mut cx, &operator);

    let parked = operator.update(&mut cx, |o, _| o.worker.take());
    operator.update(&mut cx, |o, _| {
        let row = canticle_row(o);
        o.song_to_live(row);
        assert_eq!(o.unmask.armed(), None);
    });
    chorus_lands_under_black(&mut cx, &operator);
    poll_for(&mut cx, &operator, 40);
    operator.read_with(&cx, |o, _| {
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
        assert_eq!(o.on_screen(), Some("Signal Hymn · Chorus"));
    });

    operator.update(&mut cx, |o, _| o.worker = parked);
    settle(&mut cx, &operator, "the library song on screen", |o| {
        o.on_screen() == Some("Quiet Canticle · Verse 1")
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(confirmed_mask(o), Some(Layer::Black));
        assert_eq!(o.masks.layer(), Layer::Black);
    });
}

#[gpui::test]
fn failed_library_go_live_keeps_the_mask(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = masked_two_songs(cx, &dir);
    arm_double_click_on_chorus(&mut cx, &operator);

    operator.update(&mut cx, |o, _| {
        let row = canticle_row(o);
        o.catalog[row].0.revision += 1000;
        o.song_to_live(row);
        assert_eq!(o.unmask.armed(), None);
    });
    settle(&mut cx, &operator, "the load error", |o| {
        o.library_error.is_some()
    });
    chorus_lands_under_black(&mut cx, &operator);
    for _ in 0..60 {
        poll_for(&mut cx, &operator, 1);
        operator.read_with(&cx, |o, _| {
            assert!(o.library_error.is_some());
            assert_eq!(o.masks.layer(), Layer::Black);
            assert_eq!(confirmed_mask(o), Some(Layer::Black));
            assert_eq!(o.unmask.armed(), None);
            assert_eq!(o.on_screen(), Some("Signal Hymn · Chorus"));
        });
    }
}

fn refused_go_live_keeps_the_mask(cx: &mut TestAppContext, refuse: fn(&mut Operator)) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = masked_two_songs(cx, &dir);
    arm_double_click_on_chorus(&mut cx, &operator);

    operator.update(&mut cx, |o, _| {
        o.live_message = None;
        refuse(o);
        o.go_live();
        assert!(o.live_message.is_some(), "the refusal is reported");
        assert_eq!(o.unmask.armed(), None);
    });
    chorus_lands_under_black(&mut cx, &operator);
    poll_for(&mut cx, &operator, 40);
    operator.read_with(&cx, |o, _| {
        assert_eq!(confirmed_mask(o), Some(Layer::Black))
    });
}

#[gpui::test]
fn refused_go_live_cancels_a_pending_double_click_unmask(cx: &mut TestAppContext) {
    refused_go_live_keeps_the_mask(cx, |o| o.preview = None);
}

#[gpui::test]
fn slideless_go_live_cancels_a_pending_double_click_unmask(cx: &mut TestAppContext) {
    refused_go_live_keeps_the_mask(cx, |o| {
        o.preview.as_mut().unwrap().slides = Vec::new().into();
    });
}

/// The chorus already applied under Black is the latest intent when its
/// double-click completes, so the mask lifts at once without a new cue.
#[gpui::test]
fn double_click_on_an_already_applied_slide_unmasks_without_a_new_cue(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = masked_verse_with_mode(cx, &dir, "apply");
    operator.update(&mut cx, |o, _| o.click_live(1, false));
    settle(&mut cx, &operator, "chorus under Black", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus") && confirmed_mask(o) == Some(Layer::Black)
    });
    let before = operator.read_with(&cx, |o, _| o.request_generation);
    operator.update(&mut cx, |o, _| o.click_live(1, true));
    settle(&mut cx, &operator, "unmasked over the chorus", |o| {
        confirmed_mask(o) == Some(Layer::None)
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.masks.layer(), Layer::None);
        assert_eq!(o.on_screen(), Some("Signal Hymn · Chorus"));
        assert_eq!(o.request_generation, before, "no extra cue");
        assert!(o.pending_cues.is_empty());
    });
}

/// A refresh that leaves no cue carrying the armed request drops the intent,
/// so a later fallback can never satisfy it with another slide.
#[gpui::test]
fn refresh_without_a_cue_drops_the_armed_request(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = masked_two_songs(cx, &dir);
    arm_double_click_on_chorus(&mut cx, &operator);
    operator.update(&mut cx, |o, _| {
        let live = o.live.as_mut().unwrap();
        live.slides = live.slides[..1].to_vec().into();
        o.refresh_live_request();
        assert_eq!(o.unmask.armed(), None);
    });
}

#[gpui::test]
fn deferred_cue_dies_with_a_newer_request_or_a_new_session(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = live_fixture(cx, &dir);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    let strip = |o: &mut Operator| {
        let live = o.live.as_mut().unwrap();
        let resolved = live.resolved.take();
        (live.version, resolved)
    };

    // A newer request supersedes the deferred one.
    let (version, resolved) = operator.update(&mut cx, |o, _| strip(o));
    operator.update(&mut cx, |o, _| {
        o.send(1);
        assert_eq!(o.deferred_send.map(|d| d.index), Some(1));
        o.live.as_mut().unwrap().resolved = resolved.clone();
        o.send(2);
        assert!(o.deferred_send.is_none());
        assert_eq!(o.pending_cues.len(), 1);
        o.retry_deferred();
        assert_eq!(o.pending_cues.len(), 1, "no stale cue is added");
    });
    settle(&mut cx, &operator, "bridge on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Bridge")
    });

    // Cancelling drops it, and a late landing sends nothing.
    operator.update(&mut cx, |o, _| {
        strip(o);
        o.send(1);
        assert!(o.deferred_send.is_some());
        o.cancel_cues();
        assert!(o.deferred_send.is_none());
    });

    // A restarted output session never replays it.
    operator.update(&mut cx, |o, _| {
        o.send(1);
        assert!(o.deferred_send.is_some());
    });
    click(&mut cx, "live-output");
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "new session", connected);
    let formats: Vec<_> = operator.read_with(&cx, |o, _| {
        o.live
            .as_ref()
            .unwrap()
            .slides
            .iter()
            .map(|slide| slide.format.clone())
            .collect()
    });
    cx.update(|_, cx| {
        operator.update(cx, |o, cx| {
            let landed = formats.iter().map(fonts::Resolved::bundled).collect();
            o.fonts_resolved(version, landed, cx);
        })
    });
    poll_for(&mut cx, &operator, 40);
    operator.read_with(&cx, |o, _| {
        assert!(o.deferred_send.is_none());
        assert!(o.pending_cues.is_empty());
        assert_eq!(submitted(o), 0, "nothing reached the new session");
        assert_eq!(o.live_slide, None);
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

/// Measurement, not a gate: UI-thread time of one Go Live on a 100-slide song
/// with normalized sizing. Only the first sample is cold on the worker; the
/// loop clears the operator's size cap, not the worker's cache (`cargo test --release heavy_cue -- --ignored --nocapture`).
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

mod song_search_ui {
    use super::*;
    use gpui::EntityInputHandler;
    use sela::search::{Hit, Results};

    fn type_search(cx: &mut VisualTestContext, operator: &Entity<Operator>, text: &str) {
        let input = operator.read_with(cx, |o, _| o.search_input.clone());
        cx.update(|window, cx| {
            input.update(cx, |field, cx| {
                let end = field.text().encode_utf16().count();
                field.replace_text_in_range(Some(0..end), text, window, cx);
            });
        });
        cx.run_until_parked();
    }

    fn shown(o: &Operator) -> Vec<String> {
        let mut titles: Vec<_> = o.songs().iter().map(|(_, t)| t.clone()).collect();
        titles.sort();
        titles
    }

    fn settle_hits(cx: &mut VisualTestContext, operator: &Entity<Operator>, titles: &[&str]) {
        let want: Vec<String> = titles.iter().map(|t| t.to_string()).collect();
        settle(cx, operator, "search results", |o| {
            o.search.hits().is_some() && shown(o) == want
        });
    }

    fn loaded(cx: &mut TestAppContext) -> (VisualTestContext, Entity<Operator>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, _) = with_library(cx, Some(library_with_songs(&dir)), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
        (cx, operator, dir)
    }

    #[gpui::test]
    fn two_characters_filter_by_title_and_words_and_one_character_queries_nothing(
        cx: &mut TestAppContext,
    ) {
        let (mut cx, operator, _dir) = loaded(cx);
        type_search(&mut cx, &operator, "s");
        for _ in 0..3 {
            operator.update(&mut cx, |o, cx| o.poll(cx));
            cx.run_until_parked();
        }
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.search.submitted(), 0, "one character issues no FTS query");
            assert!(o.search.hits().is_none());
            assert_eq!(shown(o), ["Quiet Canticle", "Signal Hymn"]);
        });
        type_search(&mut cx, &operator, "si");
        operator.read_with(&cx, |o, _| assert_eq!(o.search.submitted(), 1));
        settle_hits(&mut cx, &operator, &["Signal Hymn"]);
        // Lyric words, not only the title.
        type_search(&mut cx, &operator, "refrain");
        settle_hits(&mut cx, &operator, &["Signal Hymn"]);
        type_search(&mut cx, &operator, "orig");
        settle_hits(&mut cx, &operator, &["Quiet Canticle", "Signal Hymn"]);
        // Back to one character: unfiltered again, no new query.
        let before = operator.read_with(&cx, |o, _| o.search.submitted());
        type_search(&mut cx, &operator, "o");
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.search.submitted(), before);
            assert!(o.search.hits().is_none());
            assert_eq!(shown(o).len(), 2);
        });
    }

    #[gpui::test]
    fn no_results_state_clear_button_and_escape(cx: &mut TestAppContext) {
        let (mut cx, operator, _dir) = loaded(cx);
        type_search(&mut cx, &operator, "zzqx");
        settle(&mut cx, &operator, "empty hits", |o| {
            o.search.hits().is_some_and(<[_]>::is_empty)
        });
        assert!(cx.debug_bounds("song-no-results").is_some());
        click(&mut cx, "song-search-clear");
        operator.read_with(&cx, |o, cx| {
            assert_eq!(o.search_input.read(cx).text(), "");
            assert!(o.search.hits().is_none());
            assert_eq!(shown(o).len(), 2);
        });
        assert!(cx.debug_bounds("song-no-results").is_none());
        // Esc empties the box too and keeps focus in it.
        type_search(&mut cx, &operator, "sig");
        settle_hits(&mut cx, &operator, &["Signal Hymn"]);
        cx.update(|window, cx| {
            let focus = operator.read(cx).search_input.read(cx).focus_handle(cx);
            focus.focus(window, cx);
        });
        cx.simulate_keystrokes("escape");
        operator.read_with(&cx, |o, cx| {
            assert_eq!(o.search_input.read(cx).text(), "");
            assert_eq!(shown(o).len(), 2);
        });
        cx.update(|window, cx| {
            let focus = operator.read(cx).search_input.read(cx).focus_handle(cx);
            assert!(focus.is_focused(window), "focus stays in the box");
        });
    }

    #[gpui::test]
    fn typing_in_the_box_never_toggles_masks_or_goes_live(cx: &mut TestAppContext) {
        let (mut cx, operator, _dir) = loaded(cx);
        click(&mut cx, "live-output");
        settle(&mut cx, &operator, "output connection", connected);
        let submitted_before = operator.read_with(&cx, |o, _| submitted(o));
        cx.update(|window, cx| {
            let focus = operator.read(cx).search_input.read(cx).focus_handle(cx);
            focus.focus(window, cx);
        });
        cx.simulate_keystrokes("b c l shift-b shift-c shift-l ctrl-b ctrl-l pagedown right");
        operator.read_with(&cx, |o, cx| {
            assert_eq!(o.search_input.read(cx).text(), "bclBCL");
            assert!(!o.masks.is_on(Mask::Black));
            assert!(!o.masks.is_on(Mask::Clear));
            assert!(!o.masks.is_on(Mask::Logo));
            assert!(o.live.is_none());
            assert_eq!(submitted(o), submitted_before, "no new renderer cue");
        });
    }

    #[gpui::test]
    fn selection_survives_a_matching_query_and_clears_without_replacing_preview(
        cx: &mut TestAppContext,
    ) {
        let (mut cx, operator, _dir) = loaded(cx);
        let row = song_row(&mut cx, &operator, "Signal Hymn");
        click(&mut cx, row);
        let signal = operator.read_with(&cx, |o, _| version_of(o, "Signal Hymn"));
        settle(&mut cx, &operator, "preview", |o| {
            o.preview.as_ref().is_some_and(|p| p.version == signal)
        });
        type_search(&mut cx, &operator, "sig");
        settle_hits(&mut cx, &operator, &["Signal Hymn"]);
        type_search(&mut cx, &operator, "sign");
        settle_hits(&mut cx, &operator, &["Signal Hymn"]);
        operator.read_with(&cx, |o, _| assert_eq!(o.selected_song, Some(signal)));
        // The selected song drops out: no selection, Preview unchanged, and
        // the remaining song is not silently previewed.
        type_search(&mut cx, &operator, "quiet");
        settle_hits(&mut cx, &operator, &["Quiet Canticle"]);
        for _ in 0..3 {
            operator.update(&mut cx, |o, cx| o.poll(cx));
            cx.run_until_parked();
        }
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.selected_song, None);
            assert_eq!(o.preview.as_ref().unwrap().title, "Signal Hymn");
        });
        // A click on the result previews that song.
        click(&mut cx, "song-0");
        settle(&mut cx, &operator, "result previewed", |o| {
            o.preview.as_ref().is_some_and(|p| p.title == SECOND_SONG)
        });
    }

    #[gpui::test]
    fn results_refresh_after_a_song_is_saved(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = library_with_songs(&dir);
        let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
        type_search(&mut cx, &operator, "grace");
        settle(&mut cx, &operator, "empty hits", |o| {
            o.search.hits().is_some_and(<[_]>::is_empty)
        });
        let canticle = operator.read_with(&cx, |o, _| version_of(o, SECOND_SONG));
        let mut repository = Repository::open(&path).unwrap();
        let mut song = repository.song(canticle).unwrap();
        song.sections[0].lyrics = "Amazing grace".into();
        repository.save_song(Some(canticle), song.clone()).unwrap();
        cx.update(|_, cx| cx.default_global::<crate::song_library::LibraryChanged>().0 += 1);
        settle_hits(&mut cx, &operator, &["Quiet Canticle"]);
        // Renaming it so that it no longer matches removes it again.
        let head = operator.read_with(&cx, |o, _| version_of(o, SECOND_SONG));
        song.sections[0].lyrics = "Quiet line".into();
        repository.save_song(Some(head), song).unwrap();
        cx.update(|_, cx| cx.default_global::<crate::song_library::LibraryChanged>().0 += 1);
        settle(&mut cx, &operator, "song left the results", |o| {
            o.search.hits().is_some_and(<[_]>::is_empty)
        });
    }

    #[gpui::test]
    fn a_late_reply_for_an_older_query_never_overwrites_a_newer_one(cx: &mut TestAppContext) {
        let (mut cx, operator, _dir) = loaded(cx);
        type_search(&mut cx, &operator, "sig");
        settle_hits(&mut cx, &operator, &["Signal Hymn"]);
        let older = operator.read_with(&cx, |o, _| o.songs()[0].0);
        type_search(&mut cx, &operator, "quiet");
        settle_hits(&mut cx, &operator, &["Quiet Canticle"]);
        // The first query's reply finally arrives, under its old generation.
        let applied = operator.update(&mut cx, |o, _| {
            o.search.apply(
                Results {
                    generation: 1,
                    hits: vec![Hit {
                        version: older,
                        title: "Signal Hymn".into(),
                    }],
                    truncated: false,
                },
                false,
                &mut None,
            )
        });
        assert!(!applied);
        assert_eq!(operator.read_with(&cx, |o, _| shown(o)), ["Quiet Canticle"]);
    }
}

mod preflight_ui {
    use super::*;
    use crate::operator::Dialog;
    use sela::{
        background::{Background, ImageRef},
        storage::Repository,
    };
    use sha2::Digest;

    struct Fixture {
        library: std::path::PathBuf,
        original: std::path::PathBuf,
        schedule: std::path::PathBuf,
    }

    /// A saved schedule whose only song pins `pinned.png`, which is gone from
    /// the managed folder; `original` holds the same bytes elsewhere.
    fn vanished_image(dir: &tempfile::TempDir) -> Fixture {
        let bytes = png(64, 36, [30, 110, 210, 255]);
        let original = dir.path().join("elsewhere.png");
        std::fs::write(&original, &bytes).unwrap();
        let library = dir.path().join("library.sqlite");
        let mut repository = Repository::open(&library).unwrap();
        let version = repository
            .save_song(
                None,
                Song {
                    title: "Pinned Hymn".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants: Vec::new(),
                    sections: vec![Section {
                        id: SectionId::allocate(),
                        label: "Verse 1".into(),
                        lyrics: "Line one".into(),
                        format: Default::default(),
                        background: Some(Background::image(ImageRef {
                            name: "pinned.png".into(),
                            sha256: sha2::Sha256::digest(&bytes).into(),
                        })),
                    }],
                    master: Some(Background::color([0, 0, 90])),
                },
            )
            .unwrap();
        let schedule = dir.path().join("Sunday.selaschedule");
        let song = repository.song(version).unwrap();
        let file = sela::schedule_file::ScheduleFile {
            title: "Sunday".into(),
            scripture: Vec::new(),
            items: vec![sela::schedule_file::Item {
                source: version,
                song,
                arrangement: None,
                theme: None,
            }],
        };
        std::fs::write(&schedule, sela::schedule_file::encode(&file).unwrap()).unwrap();
        Fixture {
            library,
            original,
            schedule,
        }
    }

    fn open_saved(
        cx: &mut VisualTestContext,
        operator: &Entity<Operator>,
        schedule: &std::path::Path,
    ) {
        settle(cx, operator, "catalog", |o| o.catalog.len() == 1);
        open_file(cx, operator, schedule);
    }

    fn report_shown(o: &Operator) -> bool {
        o.dialog == Some(Dialog::Preflight)
            && o.assets
                .report
                .as_ref()
                .is_some_and(|r| !r.items.is_empty())
    }

    #[gpui::test]
    fn missing_image_lists_in_the_report_and_locate_relinks_it(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let fixture = vanished_image(&dir);
        let (mut cx, operator, _) = with_library(cx, Some(fixture.library.clone()), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        open_saved(&mut cx, &operator, &fixture.schedule);
        settle(&mut cx, &operator, "report", report_shown);
        cx.run_until_parked();
        assert!(cx.debug_bounds("preflight-row-0").is_some());
        assert!(cx.debug_bounds("locate-0").is_some());

        // Other bytes under the right name are refused and the row stays.
        let wrong = dir.path().join("wrong.png");
        std::fs::write(&wrong, png(64, 36, [1, 2, 3, 255])).unwrap();
        click(&mut cx, "locate-0");
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response(|_| Some(vec![wrong.clone()]));
        settle(&mut cx, &operator, "refusal", |o| {
            o.assets.note.as_ref().is_some_and(|(_, failed)| *failed)
        });
        assert!(
            !sela::images::images_dir(dir.path())
                .join("pinned.png")
                .exists()
        );
        assert!(operator.read_with(&cx, |o, _| report_shown(o)));

        let original = fixture.original.clone();
        click(&mut cx, "locate-0");
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response(|_| Some(vec![original.clone()]));
        settle(&mut cx, &operator, "relinked and rechecked", |o| {
            o.assets.note.as_ref().is_some_and(|(_, failed)| !*failed)
                && o.assets.report.as_ref().is_some_and(|r| r.items.is_empty())
        });
        assert!(
            sela::images::images_dir(dir.path())
                .join("pinned.png")
                .exists()
        );
    }

    #[gpui::test]
    fn go_live_on_a_missing_image_asks_and_cancel_keeps_the_scene(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let fixture = vanished_image(&dir);
        let (mut cx, operator, _) = with_library(cx, Some(fixture.library), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        open_saved(&mut cx, &operator, &fixture.schedule);
        settle(&mut cx, &operator, "report", report_shown);
        click(&mut cx, "dialog-cancel");
        assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);

        click(&mut cx, "live-output");
        settle(&mut cx, &operator, "output connection", connected);
        cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
        cx.simulate_keystrokes("down");
        settle(&mut cx, &operator, "preview", |o| {
            o.preview.as_ref().is_some_and(|p| p.resolved.is_some())
        });
        let sent = operator.read_with(&cx, |o, _| submitted(o));
        click(&mut cx, "go-live");
        settle(&mut cx, &operator, "warning", |o| {
            o.dialog == Some(Dialog::GoLiveMissing)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("go-live-missing").is_some());
        assert_eq!(operator.read_with(&cx, |o, _| submitted(o)), sent);

        click(&mut cx, "dialog-cancel");
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.dialog, None);
            assert_eq!(o.on_screen(), None, "the audience scene is unchanged");
            assert_eq!(submitted(o), sent);
            assert_eq!(
                o.live_message.as_deref(),
                Some("Go Live cancelled; the current scene is unchanged")
            );
        });

        click(&mut cx, "go-live");
        settle(&mut cx, &operator, "warning again", |o| {
            o.dialog == Some(Dialog::GoLiveMissing)
        });
        click(&mut cx, "dialog-confirm");
        settle(&mut cx, &operator, "live anyway", |o| {
            o.on_screen() == Some("Pinned Hymn · Verse 1")
        });
    }

    /// The saved schedule opened, its report dismissed, output connected and
    /// the only item previewed.
    fn previewed_with_output(
        cx: &mut TestAppContext,
        dir: &tempfile::TempDir,
    ) -> (VisualTestContext, Entity<Operator>, Fixture) {
        let fixture = vanished_image(dir);
        let (mut cx, operator, _) = with_library(cx, Some(fixture.library.clone()), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        open_saved(&mut cx, &operator, &fixture.schedule);
        settle(&mut cx, &operator, "report", report_shown);
        click(&mut cx, "dialog-cancel");
        click(&mut cx, "live-output");
        settle(&mut cx, &operator, "output connection", connected);
        cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
        cx.simulate_keystrokes("down");
        settle(&mut cx, &operator, "preview", |o| {
            o.preview.as_ref().is_some_and(|p| p.resolved.is_some())
        });
        (cx, operator, fixture)
    }

    /// Replaces the check worker with one that stays stuck until the
    /// returned gate is cleared, and fills its queue.
    fn stalled_worker(
        cx: &mut VisualTestContext,
        operator: &Entity<Operator>,
        dir: &tempfile::TempDir,
    ) -> Arc<std::sync::atomic::AtomicBool> {
        let gate = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let mut worker = sela::preflight::Worker::start_gated(
            dir.path().to_path_buf(),
            sela::fonts::shared(),
            Some(gate.clone()),
        )
        .unwrap();
        // The thread takes one request before it stalls, so a first Busy may
        // be followed by room; two refusals around a pause mean it is full.
        let mut refused = 0;
        for _ in 0..50 {
            if matches!(
                worker.check_images(Vec::new()),
                Err(sela::images::Error::Busy)
            ) {
                refused += 1;
                if refused == 2 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(30));
            } else {
                refused = 0;
            }
        }
        assert_eq!(refused, 2, "the queue filled up");
        operator.update(cx, |o, _| o.assets.worker = Some(worker));
        gate
    }

    fn live_focused(cx: &mut VisualTestContext, operator: &Entity<Operator>) -> bool {
        cx.update(|window, cx| operator.read(cx).live_focus.is_focused(window))
    }

    #[gpui::test]
    fn locate_shows_the_thumbnail_without_reselecting(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let fixture = vanished_image(&dir);
        let (mut cx, operator, _) = with_library(cx, Some(fixture.library.clone()), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        open_saved(&mut cx, &operator, &fixture.schedule);
        settle(&mut cx, &operator, "report", report_shown);
        click(&mut cx, "dialog-cancel");
        cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
        cx.simulate_keystrokes("down");
        settle(&mut cx, &operator, "thumbnail failed while missing", |o| {
            o.tile_images.iter().any(|(_, shown)| shown.is_none())
        });

        let thumbnail = |o: &Operator| {
            o.preview
                .as_ref()
                .and_then(|p| p.slides.first())
                .and_then(|slide| o.thumbnail(slide))
        };
        assert!(operator.read_with(&cx, |o, _| thumbnail(o).is_none()));
        let reopened = report_shown;
        cx.update(|window, cx| operator.update(cx, |o, cx| o.review_files(window, cx)));
        settle(&mut cx, &operator, "report", reopened);
        click(&mut cx, "locate-0");
        let original = fixture.original.clone();
        cx.simulate_path_prompt_response(|_| Some(vec![original.clone()]));
        settle(&mut cx, &operator, "relinked", |o| {
            o.assets.note.as_ref().is_some_and(|(_, failed)| !*failed)
        });
        settle(&mut cx, &operator, "thumbnail landed", |o| {
            thumbnail(o).is_some()
        });
    }

    #[gpui::test]
    fn go_live_with_a_full_check_queue_still_asks(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, _) = previewed_with_output(cx, &dir);
        let gate = stalled_worker(&mut cx, &operator, &dir);
        let sent = operator.read_with(&cx, |o, _| submitted(o));

        click(&mut cx, "go-live");
        cx.run_until_parked();
        operator.read_with(&cx, |o, _| {
            assert!(o.assets.is_held(), "held, not sent black");
            assert_eq!(o.dialog, None);
            assert_eq!(submitted(o), sent, "nothing reached the audience");
        });

        gate.store(false, std::sync::atomic::Ordering::Release);
        settle(&mut cx, &operator, "warning", |o| {
            o.dialog == Some(Dialog::GoLiveMissing)
        });
        assert_eq!(operator.read_with(&cx, |o, _| submitted(o)), sent);
    }

    #[gpui::test]
    fn a_held_check_never_overrides_a_newer_command(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, _) = previewed_with_output(cx, &dir);
        let gate = stalled_worker(&mut cx, &operator, &dir);
        click(&mut cx, "go-live");
        cx.run_until_parked();
        assert!(operator.read_with(&cx, |o, _| o.assets.is_held()));

        // A newer Go Live on an item without an image goes out at once.
        operator.update(&mut cx, |o, _| {
            let mut plain = o.preview.clone().unwrap();
            plain.slides = plain
                .slides
                .iter()
                .cloned()
                .map(|mut slide| {
                    slide.background = None;
                    slide
                })
                .collect();
            o.preview = Some(plain);
            o.go_live();
            assert!(!o.assets.is_held(), "the newer Go Live replaced the hold");
        });
        gate.store(false, std::sync::atomic::Ordering::Release);
        for _ in 0..20 {
            operator.update(&mut cx, |o, cx| o.poll(cx));
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(5));
        }
        operator.read_with(&cx, |o, _| assert_eq!(o.dialog, None, "no stale warning"));
    }

    #[gpui::test]
    fn output_toggle_and_cue_cancel_drop_a_held_go_live(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, _) = previewed_with_output(cx, &dir);
        let _gate = stalled_worker(&mut cx, &operator, &dir);
        click(&mut cx, "go-live");
        cx.run_until_parked();
        assert!(operator.read_with(&cx, |o, _| o.assets.is_held()));
        operator.update(&mut cx, |o, _| o.cancel_cues());
        assert!(!operator.read_with(&cx, |o, _| o.assets.is_held()));

        click(&mut cx, "go-live");
        cx.run_until_parked();
        assert!(operator.read_with(&cx, |o, _| o.assets.is_held()));
        operator.update(&mut cx, |o, _| o.toggle_output());
        assert!(!operator.read_with(&cx, |o, _| o.assets.is_held()));
    }

    #[gpui::test]
    fn go_live_anyway_hands_focus_to_live(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, _) = previewed_with_output(cx, &dir);
        click(&mut cx, "go-live");
        settle(&mut cx, &operator, "warning", |o| {
            o.dialog == Some(Dialog::GoLiveMissing)
        });
        click(&mut cx, "dialog-confirm");
        settle(&mut cx, &operator, "live anyway", |o| {
            o.on_screen() == Some("Pinned Hymn · Verse 1")
        });
        cx.run_until_parked();
        assert!(live_focused(&mut cx, &operator));
    }

    #[gpui::test]
    fn a_clean_delayed_check_hands_focus_to_live(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, fixture) = previewed_with_output(cx, &dir);
        cx.update(|window, cx| operator.update(cx, |o, cx| o.review_files(window, cx)));
        click(&mut cx, "locate-0");
        let original = fixture.original.clone();
        cx.simulate_path_prompt_response(|_| Some(vec![original.clone()]));
        settle(&mut cx, &operator, "relinked", |o| {
            o.assets.note.as_ref().is_some_and(|(_, failed)| !*failed)
        });
        click(&mut cx, "dialog-cancel");
        cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
        click(&mut cx, "go-live");
        settle(&mut cx, &operator, "live after the check", |o| {
            o.on_screen() == Some("Pinned Hymn · Verse 1")
        });
        cx.run_until_parked();
        assert!(live_focused(&mut cx, &operator));
    }

    #[gpui::test]
    fn import_with_output_off_fits_the_configured_monitor(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let library = library_with_song(&dir);
        let (mut cx, operator, _) = with_library(cx, Some(library), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        operator.update(&mut cx, |o, _| {
            let monitor = |name: &str, primary| sela::settings::MonitorInfo {
                name: name.into(),
                width: if primary { 2560 } else { 1280 },
                height: if primary { 1440 } else { 720 },
                x: 0,
                y: 0,
                primary,
            };
            o.settings.monitors = Some(vec![monitor("A", true), monitor("B", false)]);
            assert!(o.output.is_none());
            assert_eq!(
                o.import_extent(),
                sela::scene::Extent {
                    width: 1280,
                    height: 720
                }
            );
        });
    }

    fn big_jpeg(path: &std::path::Path) {
        image::RgbImage::from_pixel(6000, 4000, image::Rgb([40, 90, 160]))
            .save(path)
            .unwrap();
    }

    #[gpui::test]
    fn cancelled_import_leaves_the_library_unchanged(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let library = library_with_song(&dir);
        let source = dir.path().join("big.jpg");
        big_jpeg(&source);
        let (mut cx, operator, _) = with_library(cx, Some(library), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "list", |o| o.images.is_empty());
        click(&mut cx, "Media");
        click(&mut cx, "import-image");
        assert!(cx.did_prompt_for_paths());
        let picked = source.clone();
        cx.simulate_path_prompt_response(|_| Some(vec![picked.clone()]));
        cx.run_until_parked();
        assert!(operator.read_with(&cx, |o, _| o.assets.importing.is_some()));
        operator.update(&mut cx, |o, _| o.cancel_import());
        settle(&mut cx, &operator, "cancelled", |o| {
            o.assets.importing.is_none()
        });
        operator.read_with(&cx, |o, _| assert!(o.images.is_empty()));
        let managed = sela::images::images_dir(dir.path());
        let files = std::fs::read_dir(&managed).map_or(0, |d| d.count());
        assert_eq!(files, 0, "no managed copy or partial file");
    }

    #[gpui::test]
    fn oversize_photo_imports_as_a_downscaled_copy_with_a_note(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let library = library_with_song(&dir);
        let source = dir.path().join("big.jpg");
        big_jpeg(&source);
        let before = sha2::Sha256::digest(std::fs::read(&source).unwrap());
        let (mut cx, operator, _) = with_library(cx, Some(library), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "list", |o| o.images.is_empty());
        click(&mut cx, "Media");
        click(&mut cx, "import-image");
        let picked = source.clone();
        cx.simulate_path_prompt_response(|_| Some(vec![picked.clone()]));
        settle(&mut cx, &operator, "imported", |o| o.images.len() == 1);
        cx.run_until_parked();
        operator.read_with(&cx, |o, _| {
            assert!(
                o.assets
                    .import_notes
                    .iter()
                    .any(|n| n.contains("6000×4000")),
                "{:?}",
                o.assets.import_notes
            );
        });
        assert!(cx.debug_bounds("import-notes").is_some());
        let after = sha2::Sha256::digest(std::fs::read(&source).unwrap());
        assert_eq!(before, after, "the original file is untouched");
    }
}

mod schedule_files_ui {
    use super::*;
    use crate::operator::Dialog;
    use sela::schedule_file::{self, Item, ScheduleFile};
    use std::path::{Path, PathBuf};

    fn save_new(cx: &mut VisualTestContext, operator: &Entity<Operator>, path: PathBuf) {
        assert!(cx.did_prompt_for_new_path());
        let expect = schedule_file::with_extension(&path);
        cx.simulate_new_path_selection(move |_| Some(path));
        settle(cx, operator, "saved", |o| {
            o.schedule.path() == Some(expect.as_path())
                && !o.schedule.is_dirty()
                && o.settings_idle()
        });
    }

    fn loaded(
        cx: &mut TestAppContext,
        dir: &tempfile::TempDir,
    ) -> (VisualTestContext, Entity<Operator>) {
        let (mut cx, operator, _) = with_library(cx, Some(library_with_songs(dir)), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
        (cx, operator)
    }

    fn recent(cx: &VisualTestContext, operator: &Entity<Operator>) -> Vec<PathBuf> {
        operator.read_with(cx, |o, _| o.recent_schedules().to_vec())
    }

    #[gpui::test]
    fn save_as_switches_the_file_and_leaves_the_old_one_alone(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = loaded(cx, &dir);
        add_song(&mut cx, &operator, "Signal Hymn");
        cx.simulate_keystrokes("ctrl-s");
        let a = dir.path().join("A.selaschedule");
        save_new(&mut cx, &operator, a.clone());
        let before = std::fs::read(&a).unwrap();

        add_song(&mut cx, &operator, SECOND_SONG);
        cx.simulate_keystrokes("alt-f");
        click(&mut cx, "bar-file-save-as");
        let b = dir.path().join("B");
        save_new(&mut cx, &operator, b);
        let b = dir.path().join("B.selaschedule");
        assert_eq!(std::fs::read(&a).unwrap(), before);
        assert_eq!(schedule_file::read(&b).unwrap().items.len(), 2);
        assert_eq!(
            operator.read_with(&cx, |o, _| o.schedule.path().map(Path::to_path_buf)),
            Some(b.clone())
        );
        assert_eq!(recent(&cx, &operator), [b, a]);
    }

    #[gpui::test]
    fn a_save_before_settings_load_still_reaches_open_recent(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("older.selaschedule");
        let mut stored = sela::settings::Settings::default();
        stored.remember_recent(older.clone());
        sela::settings::save(&sela::settings::path_in(dir.path()), &stored).unwrap();
        let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut config = crate::operator::SettingsConfig::default();
        let opened = gate.clone();
        config.loader = Arc::new(move |path| {
            while !opened.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            sela::settings::load(path)
        });
        let (mut cx, operator, _) =
            with_settings(cx, Some(library_with_songs(&dir)), "apply", config);
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
        add_song(&mut cx, &operator, "Signal Hymn");
        cx.simulate_keystrokes("ctrl-s");
        let a = dir.path().join("A.selaschedule");
        save_new(&mut cx, &operator, a.clone());
        assert!(recent(&cx, &operator).is_empty(), "settings not loaded yet");

        gate.store(true, std::sync::atomic::Ordering::SeqCst);
        settle(&mut cx, &operator, "loaded and written", |o| {
            o.settings_idle() && o.recent_schedules().len() == 2
        });
        assert_eq!(recent(&cx, &operator), [a.clone(), older.clone()]);
        let on_disk = sela::settings::load(&sela::settings::path_in(dir.path())).settings;
        assert_eq!(on_disk.recent_schedules, [a, older]);
    }

    #[gpui::test]
    fn open_recent_lists_files_and_drops_a_missing_one(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = loaded(cx, &dir);
        add_song(&mut cx, &operator, "Signal Hymn");
        cx.simulate_keystrokes("ctrl-s");
        let a = dir.path().join("A.selaschedule");
        save_new(&mut cx, &operator, a.clone());
        cx.simulate_keystrokes("alt-f");
        click(&mut cx, "bar-file-save-as");
        let b = dir.path().join("B.selaschedule");
        save_new(&mut cx, &operator, b.clone());
        assert_eq!(recent(&cx, &operator), [b.clone(), a.clone()]);

        // Open Recent picks A; it becomes the newest entry without a copy.
        cx.simulate_keystrokes("alt-f");
        click(&mut cx, "bar-file-recent-1");
        settle(&mut cx, &operator, "opened A", |o| {
            o.schedule.path() == Some(a.as_path()) && o.settings_idle()
        });
        assert_eq!(recent(&cx, &operator), [a.clone(), b.clone()]);

        std::fs::remove_file(&b).unwrap();
        cx.simulate_keystrokes("alt-f");
        click(&mut cx, "bar-file-recent-1");
        settle(&mut cx, &operator, "missing", |o| {
            o.dialog == Some(Dialog::FileError) && o.settings_idle()
        });
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.schedule.path(), Some(a.as_path()), "schedule unchanged");
            assert_eq!(entry_titles(o), ["Signal Hymn"]);
            assert!(o.live.is_none());
        });
        assert_eq!(recent(&cx, &operator), [a]);
    }

    #[gpui::test]
    fn invalid_files_are_rejected_and_change_nothing(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = loaded(cx, &dir);
        add_song(&mut cx, &operator, "Signal Hymn");
        let good = dir.path().join("good.selaschedule");
        cx.simulate_keystrokes("ctrl-s");
        save_new(&mut cx, &operator, good.clone());
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&good).unwrap()).unwrap();
        value["version"] = (schedule_file::VERSION + 1).into();
        let cases = [
            ("garbage", b"not a schedule".to_vec()),
            ("truncated", std::fs::read(&good).unwrap()[..40].to_vec()),
            ("newer", value.to_string().into_bytes()),
        ];
        for (name, bytes) in cases {
            let path = dir.path().join(format!("{name}.selaschedule"));
            std::fs::write(&path, &bytes).unwrap();
            open_file(&mut cx, &operator, &path);
            settle(&mut cx, &operator, name, |o| {
                o.dialog == Some(Dialog::FileError)
            });
            operator.read_with(&cx, |o, _| {
                assert_eq!(o.schedule.path(), Some(good.as_path()), "{name}");
                assert_eq!(entry_titles(o), ["Signal Hymn"]);
                assert!(!o.schedule.is_dirty());
                assert!(o.live.is_none());
            });
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            click(&mut cx, "dialog-cancel");
            assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);
        }
        assert_eq!(recent(&cx, &operator), [good]);
    }

    #[gpui::test]
    fn a_file_opens_with_its_slides_in_a_library_without_the_song(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        // The file comes from one library; the profile under test has another.
        let source = library_with_song(&dir);
        let mut repository = Repository::open(&source).unwrap();
        let (version, title) = repository.catalog(None).unwrap().remove(0);
        let song = repository.song(version).unwrap();
        let file = dir.path().join("Sunday.selaschedule");
        let document = ScheduleFile {
            title: "Sunday".into(),
            scripture: Vec::new(),
            items: vec![Item {
                source: version,
                song,
                arrangement: None,
                theme: None,
            }],
        };
        std::fs::write(&file, schedule_file::encode(&document).unwrap()).unwrap();

        let (mut cx, operator, _) =
            with_library(cx, Some(library_with_songs_named(&other)), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
        let before = operator.read_with(&cx, |o, _| o.catalog.clone());
        open_file(&mut cx, &operator, &file);
        settle(&mut cx, &operator, "opened", |o| {
            o.schedule.path() == Some(file.as_path())
        });
        click(&mut cx, "schedule-item-0");
        settle(&mut cx, &operator, "previewed", |o| {
            o.preview
                .as_ref()
                .is_some_and(|p| p.title == title && !p.slides.is_empty())
        });
        cx.run_until_parked();
        let after = operator.read_with(&cx, |o, _| o.catalog.clone());
        assert_eq!(after, before, "Songs list unchanged");
        assert!(operator.read_with(&cx, |o, _| o.live.is_none()));
        // The same file opened again reuses the hidden revision.
        open_file(&mut cx, &operator, &file);
        settle(&mut cx, &operator, "reopened", |o| !o.schedule.is_dirty());
        let _ = &mut repository;
    }

    fn library_with_songs_named(dir: &tempfile::TempDir) -> PathBuf {
        let path = dir.path().join("library.sqlite");
        let mut repository = Repository::open(&path).unwrap();
        repository
            .save_song(
                None,
                Song {
                    title: "Different Song".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants: Vec::new(),
                    sections: vec![Section {
                        id: SectionId::allocate(),
                        label: "Verse".into(),
                        lyrics: "Different words".into(),
                        format: Default::default(),
                        background: None,
                    }],
                    master: None,
                },
            )
            .unwrap();
        path
    }

    #[gpui::test]
    fn the_unsaved_guard_saves_then_continues(cx: &mut TestAppContext) {
        use crate::operator::Then;
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = loaded(cx, &dir);
        add_song(&mut cx, &operator, "Signal Hymn");
        cx.simulate_keystrokes("alt-f");
        click(&mut cx, "bar-file-close");
        assert_eq!(
            operator.read_with(&cx, |o, _| o.dialog.clone()),
            Some(Dialog::Unsaved(Then::New))
        );
        click(&mut cx, "dialog-save");
        let file = dir.path().join("Guarded.selaschedule");
        assert!(cx.did_prompt_for_new_path());
        let target = file.clone();
        cx.simulate_new_path_selection(move |_| Some(target));
        settle(&mut cx, &operator, "cleared", |o| {
            o.schedule.entries().is_empty() && o.schedule.path().is_none()
        });
        assert_eq!(schedule_file::read(&file).unwrap().items.len(), 1);
    }

    /// A schedule writer that blocks until the returned flag is set.
    fn gated_loaded(
        cx: &mut TestAppContext,
        dir: &tempfile::TempDir,
    ) -> (
        VisualTestContext,
        Entity<Operator>,
        Arc<std::sync::atomic::AtomicBool>,
    ) {
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let hold = release.clone();
        let inner = schedule_file::default_writer();
        let config = crate::operator::SettingsConfig {
            schedule_writer: Arc::new(move |path, bytes| {
                while !hold.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                inner(path, bytes)
            }),
            ..Default::default()
        };
        let (mut cx, operator, _) =
            with_settings(cx, Some(library_with_songs(dir)), "apply", config);
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
        (cx, operator, release)
    }

    /// Starts an untitled save to `path` that stays in flight until released.
    fn begin_blocked_save(cx: &mut VisualTestContext, operator: &Entity<Operator>, path: PathBuf) {
        cx.simulate_keystrokes("ctrl-s");
        assert!(cx.did_prompt_for_new_path());
        cx.simulate_new_path_selection(move |_| Some(path));
        settle(cx, operator, "save in flight", |o| o.saving_file());
    }

    #[gpui::test]
    fn recent_after_a_guarded_save_opens_the_chosen_file(cx: &mut TestAppContext) {
        use crate::operator::Then;
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = loaded(cx, &dir);
        add_song(&mut cx, &operator, "Signal Hymn");
        cx.simulate_keystrokes("ctrl-s");
        let a = dir.path().join("A.selaschedule");
        save_new(&mut cx, &operator, a.clone());
        cx.simulate_keystrokes("alt-f");
        click(&mut cx, "bar-file-save-as");
        let b = dir.path().join("B.selaschedule");
        save_new(&mut cx, &operator, b.clone());
        assert_eq!(recent(&cx, &operator), [b.clone(), a.clone()]);

        // An untitled, edited schedule picks A (index 1); saving it first
        // puts the new file at the top of the list and shifts every index.
        cx.simulate_keystrokes("alt-f");
        click(&mut cx, "bar-file-close");
        assert!(operator.read_with(&cx, |o, _| o.schedule.path().is_none()));
        add_song(&mut cx, &operator, "Signal Hymn");
        cx.simulate_keystrokes("alt-f");
        click(&mut cx, "bar-file-recent-1");
        assert_eq!(
            operator.read_with(&cx, |o, _| o.dialog.clone()),
            Some(Dialog::Unsaved(Then::Recent(a.clone())))
        );
        click(&mut cx, "dialog-save");
        let c = dir.path().join("C.selaschedule");
        assert!(cx.did_prompt_for_new_path());
        let target = c.clone();
        cx.simulate_new_path_selection(move |_| Some(target));
        settle(&mut cx, &operator, "opened A", |o| {
            o.schedule.path() == Some(a.as_path()) && o.settings_idle()
        });
        assert!(c.exists(), "the guarded Save wrote the new file");
    }

    #[gpui::test]
    fn an_edit_during_a_guarded_save_prompts_again(cx: &mut TestAppContext) {
        use crate::operator::Then;
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, release) = gated_loaded(cx, &dir);
        add_song(&mut cx, &operator, "Signal Hymn");
        cx.simulate_keystrokes("alt-f");
        click(&mut cx, "bar-file-close");
        click(&mut cx, "dialog-save");
        assert!(cx.did_prompt_for_new_path());
        let file = dir.path().join("Guarded.selaschedule");
        cx.simulate_new_path_selection(move |_| Some(file));
        settle(&mut cx, &operator, "save in flight", |o| o.saving_file());
        add_song(&mut cx, &operator, SECOND_SONG);
        release.store(true, std::sync::atomic::Ordering::SeqCst);
        settle(&mut cx, &operator, "prompted again", |o| {
            o.dialog == Some(Dialog::Unsaved(Then::New))
        });
        operator.read_with(&cx, |o, _| {
            assert_eq!(entry_titles(o), ["Signal Hymn", SECOND_SONG]);
            assert!(o.schedule.is_dirty());
        });
    }

    /// Save, then Quit: an import preview opened while the write ran still
    /// holds the close back once the Save lands (PP-IMPF-09).
    #[gpui::test]
    fn an_import_opened_during_a_save_then_quit_keeps_the_window(cx: &mut TestAppContext) {
        use crate::operator::Then;
        use crate::operator::song_import::Phase;
        let dir = tempfile::tempdir().unwrap();
        let folder = super::song_import::batch_dir(&dir);
        let (mut cx, operator, release) = gated_loaded(cx, &dir);
        add_song(&mut cx, &operator, "Signal Hymn");
        assert!(!cx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx))));
        cx.run_until_parked();
        assert_eq!(
            operator.read_with(&cx, |o, _| o.dialog.clone()),
            Some(Dialog::Unsaved(Then::Quit))
        );
        click(&mut cx, "dialog-save");
        assert!(cx.did_prompt_for_new_path());
        let file = dir.path().join("Quit.selaschedule");
        cx.simulate_new_path_selection(move |_| Some(file));
        settle(&mut cx, &operator, "save in flight", |o| o.saving_file());
        super::song_import::import_folder(&mut cx, &operator, folder);
        super::song_import::settle_review(&mut cx, &operator);
        release.store(true, std::sync::atomic::Ordering::SeqCst);
        settle(&mut cx, &operator, "saved and quit refused", |o| {
            !o.saving_file() && o.song_import.quit_note.is_some()
        });
        assert_eq!(
            cx.read(|cx| cx.windows().len()),
            1,
            "the import is not lost"
        );
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.dialog, Some(Dialog::ImportSongs));
            assert_eq!(o.song_import.phase, Phase::Review);
            assert!(!o.schedule.is_dirty());
        });
    }

    #[gpui::test]
    fn new_close_and_open_are_refused_while_a_save_runs(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, release) = gated_loaded(cx, &dir);
        add_song(&mut cx, &operator, "Signal Hymn");
        let file = dir.path().join("Busy.selaschedule");
        begin_blocked_save(&mut cx, &operator, file.clone());
        cx.simulate_keystrokes("alt-f");
        click(&mut cx, "bar-file-close");
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.dialog, None);
            assert_eq!(entry_titles(o), ["Signal Hymn"]);
        });
        click(&mut cx, "open-schedule");
        assert!(!cx.did_prompt_for_paths(), "Open is refused too");
        release.store(true, std::sync::atomic::Ordering::SeqCst);
        settle(&mut cx, &operator, "saved", |o| {
            o.schedule.path() == Some(file.as_path()) && !o.schedule.is_dirty()
        });
    }

    #[gpui::test]
    fn a_save_that_finishes_late_leaves_the_new_document_alone(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, release) = gated_loaded(cx, &dir);
        add_song(&mut cx, &operator, "Signal Hymn");
        let file = dir.path().join("Late.selaschedule");
        begin_blocked_save(&mut cx, &operator, file.clone());
        operator.update(&mut cx, |o, _| o.schedule.clear());
        release.store(true, std::sync::atomic::Ordering::SeqCst);
        settle(&mut cx, &operator, "write finished", |o| {
            !o.saving_file() && o.settings_idle()
        });
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.schedule.path(), None, "still untitled");
            assert!(!o.schedule.is_dirty());
        });
        assert!(file.exists(), "the file itself was written");
    }
}

mod schedule_remainder_ui {
    use super::*;
    use crate::operator::Dialog;
    use gpui::{Modifiers, MouseButton, point};
    use sela::schedule::EntryId;
    use sela::storage::Version;

    const ROWS: [&str; 8] = [
        "schedule-item-0",
        "schedule-item-1",
        "schedule-item-2",
        "schedule-item-3",
        "schedule-item-4",
        "schedule-item-5",
        "schedule-item-6",
        "schedule-item-7",
    ];

    fn ready(
        cx: &mut TestAppContext,
        dir: &tempfile::TempDir,
    ) -> (VisualTestContext, Entity<Operator>) {
        let (mut cx, operator, _) = with_library(cx, Some(library_with_songs(dir)), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
        (cx, operator)
    }

    /// Appends `count` entries alternating between the two library songs.
    fn fill(cx: &mut VisualTestContext, operator: &Entity<Operator>, count: usize) -> Vec<EntryId> {
        for n in 0..count {
            operator.update(cx, |o, cx| {
                let (version, title) = o.catalog[n % 2].clone();
                o.add_entry(usize::MAX, version, title);
                cx.notify();
            });
        }
        cx.run_until_parked();
        order(cx, operator)
    }

    fn order(cx: &VisualTestContext, operator: &Entity<Operator>) -> Vec<EntryId> {
        operator.read_with(cx, |o, _| {
            o.schedule.entries().iter().map(|e| e.id).collect()
        })
    }

    fn selected(cx: &VisualTestContext, operator: &Entity<Operator>) -> Vec<usize> {
        operator.read_with(cx, |o, _| {
            o.schedule
                .entries()
                .iter()
                .enumerate()
                .filter(|(_, e)| o.is_entry_selected(e.id))
                .map(|(i, _)| i)
                .collect()
        })
    }

    fn click_with(cx: &mut VisualTestContext, selector: &'static str, modifiers: Modifiers) {
        let at = cx.debug_bounds(selector).unwrap().center();
        cx.simulate_click(at, modifiers);
    }

    fn focus_root(cx: &mut VisualTestContext, operator: &Entity<Operator>) {
        cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    }

    #[gpui::test]
    fn shift_ctrl_click_and_shift_arrows_select_in_the_schedule(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = ready(cx, &dir);
        let ids = fill(&mut cx, &operator, 7);
        click(&mut cx, ROWS[1]);
        assert_eq!(selected(&cx, &operator), [1]);
        click_with(&mut cx, ROWS[4], Modifiers::shift());
        assert_eq!(selected(&cx, &operator), [1, 2, 3, 4], "range 2 to 5");
        click_with(&mut cx, ROWS[2], Modifiers::control());
        assert_eq!(selected(&cx, &operator), [1, 3, 4], "ctrl removes");
        click_with(&mut cx, ROWS[5], Modifiers::control());
        assert_eq!(selected(&cx, &operator), [1, 3, 4, 5], "ctrl adds");
        assert_eq!(
            operator.read_with(&cx, |o, _| o.selected_entry),
            Some(ids[5]),
            "the added row is the focused one"
        );
        // Selection is visible on each selected row through the a11y state.
        click(&mut cx, ROWS[1]);
        cx.simulate_keystrokes("shift-down shift-down");
        assert_eq!(
            selected(&cx, &operator),
            [1, 2, 3],
            "extends from the anchor"
        );
        cx.simulate_keystrokes("shift-up");
        assert_eq!(selected(&cx, &operator), [1, 2]);
        cx.simulate_keystrokes("down");
        assert_eq!(selected(&cx, &operator), [3], "a plain arrow collapses it");
        click_with(&mut cx, ROWS[0], Modifiers::shift());
        assert_eq!(selected(&cx, &operator), [0, 1, 2, 3], "from anchor 4 to 1");
        click(&mut cx, ROWS[6]);
        assert_eq!(selected(&cx, &operator), [6]);
    }

    #[gpui::test]
    fn a_multi_selection_sends_only_the_focused_item_live(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = ready(cx, &dir);
        let ids = fill(&mut cx, &operator, 5);
        click(&mut cx, "live-output");
        settle(&mut cx, &operator, "output connection", connected);
        click(&mut cx, ROWS[1]);
        click_with(&mut cx, ROWS[3], Modifiers::shift());
        assert_eq!(selected(&cx, &operator), [1, 2, 3]);
        settle(&mut cx, &operator, "focused item previewed", |o| {
            o.preview.as_ref().and_then(|p| p.entry) == Some(ids[3])
        });
        let sent = operator.read_with(&cx, |o, _| submitted(o));
        focus_root(&mut cx, &operator);
        cx.simulate_keystrokes("pagedown");
        settle(&mut cx, &operator, "one cue", |o| {
            o.live.as_ref().and_then(|l| l.entry) == Some(ids[3]) && o.on_screen().is_some()
        });
        operator.read_with(&cx, |o, _| {
            assert_eq!(submitted(o), sent + 1, "exactly one cue");
            assert_eq!(o.live.as_ref().unwrap().entry, Some(ids[3]));
        });
        assert_eq!(
            selected(&cx, &operator),
            [1, 2, 3],
            "selection is untouched"
        );
    }

    #[gpui::test]
    fn moving_a_selection_keeps_relative_order_and_selection(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = ready(cx, &dir);
        let ids = fill(&mut cx, &operator, 6);
        click(&mut cx, ROWS[1]);
        click_with(&mut cx, ROWS[3], Modifiers::control());
        drag(&mut cx, ROWS[1], "schedule-list");
        assert_eq!(
            order(&cx, &operator),
            [ids[0], ids[2], ids[4], ids[5], ids[1], ids[3]],
            "1,3,5,6,2,4"
        );
        assert_eq!(selected(&cx, &operator), [4, 5], "both stay selected");
        // Move Up steps the whole group.
        click(&mut cx, "schedule-up");
        assert_eq!(
            order(&cx, &operator),
            [ids[0], ids[2], ids[4], ids[1], ids[3], ids[5]]
        );
        assert_eq!(selected(&cx, &operator), [3, 4]);
    }

    #[gpui::test]
    fn escape_cancels_a_drag_and_the_drop_indicator_shows(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = ready(cx, &dir);
        let ids = fill(&mut cx, &operator, 4);
        let start = cx.debug_bounds(ROWS[0]).unwrap().center();
        let end = cx.debug_bounds(ROWS[2]).unwrap().center();
        cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
        cx.simulate_mouse_move(
            start + point(px(8.), px(8.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("schedule-drop-indicator").is_some(),
            "an insertion line shows while dragging"
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(
            !cx.update(|_, cx| cx.has_active_drag()),
            "Esc ends the drag"
        );
        assert!(cx.debug_bounds("schedule-drop-indicator").is_none());
        cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
        assert_eq!(order(&cx, &operator), ids, "order unchanged");
        // A later drag still works.
        drag(&mut cx, ROWS[0], "schedule-list");
        assert_eq!(
            order(&cx, &operator),
            [ids[1], ids[2], ids[3], ids[0]],
            "dropped past the last row"
        );
    }

    #[gpui::test]
    fn dragging_to_the_bottom_edge_autoscrolls(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = ready(cx, &dir);
        let ids = fill(&mut cx, &operator, 30);
        let list = cx.debug_bounds("schedule-list").unwrap();
        let start = cx.debug_bounds(ROWS[0]).unwrap().center();
        let edge = point(start.x, list.bottom() - px(4.));
        cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
        cx.simulate_mouse_move(
            start + point(px(8.), px(8.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_move(edge, MouseButton::Left, Default::default());
        for _ in 0..10 {
            cx.executor().advance_clock(Duration::from_millis(60));
            cx.run_until_parked();
        }
        let offset = operator.read_with(&cx, |o, _| o.view.scroll.offset().y);
        assert!(offset < px(0.), "the list scrolled ({offset:?})");
        cx.simulate_mouse_up(edge, MouseButton::Left, Default::default());
        let landed = order(&cx, &operator)
            .iter()
            .position(|id| *id == ids[0])
            .unwrap();
        assert!(
            landed > 15,
            "dropped at the indicator, not the start: {landed}"
        );
    }

    #[gpui::test]
    fn duplicate_makes_an_independent_copy_right_after_the_original(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = ready(cx, &dir);
        let ids = fill(&mut cx, &operator, 3);
        let title = operator.read_with(&cx, |o, _| o.schedule.entries()[1].title.clone());
        right_click(&mut cx, ROWS[1]);
        click(&mut cx, "item-menu-duplicate");
        let after = order(&cx, &operator);
        assert_eq!(after.len(), 4);
        assert_eq!((after[0], after[1], after[3]), (ids[0], ids[1], ids[2]));
        let copy = after[2];
        assert_ne!(copy, ids[1]);
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.schedule.get(copy).unwrap().title, title);
            assert_eq!(
                o.schedule.get(copy).unwrap().version,
                o.schedule.get(ids[1]).unwrap().version
            );
            assert_eq!(o.selected_entry, Some(copy), "the copy is selected");
        });
        assert_eq!(selected(&cx, &operator), [2]);
        settle(&mut cx, &operator, "copy previewed", |o| {
            o.preview.as_ref().and_then(|p| p.entry) == Some(copy)
        });
        // Removing the copy leaves the original.
        focus_root(&mut cx, &operator);
        cx.simulate_keystrokes("ctrl-delete");
        assert_eq!(order(&cx, &operator), [ids[0], ids[1], ids[2]]);
    }

    /// Saves a new revision of "Signal Hymn" in the library file and returns
    /// it, then lets the Songs list pick it up.
    fn edit_hymn(
        cx: &mut VisualTestContext,
        operator: &Entity<Operator>,
        dir: &tempfile::TempDir,
    ) -> Version {
        let old = operator.read_with(cx, |o, _| version_of(o, "Signal Hymn"));
        let mut repository = Repository::open(&dir.path().join("library.sqlite")).unwrap();
        let mut song = repository.song(old).unwrap();
        song.sections[0].lyrics = "First revised line".into();
        let new = repository.save_song(Some(old), song).unwrap();
        let hymn = song_row(cx, operator, "Signal Hymn");
        right_click(cx, hymn);
        click(cx, "song-menu-refresh");
        settle(cx, operator, "catalog has the new revision", |o| {
            o.catalog.iter().any(|(v, _)| *v == new)
        });
        new
    }

    fn open_update(cx: &mut VisualTestContext, operator: &Entity<Operator>) {
        let hymn = song_row(cx, operator, "Signal Hymn");
        right_click(cx, hymn);
        click(cx, "song-menu-update");
        settle(cx, operator, "review", |o| {
            o.dialog == Some(Dialog::UpdateItems)
        });
    }

    fn versions(cx: &VisualTestContext, operator: &Entity<Operator>) -> Vec<Version> {
        operator.read_with(cx, |o, _| o.schedule.versions())
    }

    #[gpui::test]
    fn update_items_shows_a_diff_and_applies_only_on_confirm(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = ready(cx, &dir);
        // Two hymns and two canticles: the canticles are never listed.
        fill(&mut cx, &operator, 4);
        let hymn = operator.read_with(&cx, |o, _| version_of(o, "Signal Hymn"));
        let before = versions(&cx, &operator);
        assert_eq!(before.iter().filter(|v| **v == hymn).count(), 2);
        let up_to_date = || "Schedule items are up to date";
        open_update_expect_none(&mut cx, &operator, up_to_date());

        let new = edit_hymn(&mut cx, &operator, &dir);
        open_update(&mut cx, &operator);
        assert!(cx.debug_bounds("update-item-0").is_some());
        assert!(cx.debug_bounds("update-item-1").is_some());
        assert!(
            cx.debug_bounds("update-item-2").is_none(),
            "canticle unlisted"
        );
        operator.read_with(&cx, |o, _| {
            let review = o.view.review.as_ref().unwrap();
            assert_eq!(review.updates.len(), 2);
            let change = &review.updates[0].changes[0];
            assert_eq!(change.number, 1);
            assert!(
                change
                    .old
                    .as_deref()
                    .unwrap()
                    .contains("First original line")
            );
            assert!(
                change
                    .new
                    .as_deref()
                    .unwrap()
                    .contains("First revised line")
            );
        });
        assert_eq!(
            versions(&cx, &operator),
            before,
            "nothing changes before confirm"
        );

        click(&mut cx, "dialog-cancel");
        assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);
        assert_eq!(versions(&cx, &operator), before, "Cancel leaves the items");

        open_update(&mut cx, &operator);
        click(&mut cx, "dialog-confirm");
        let after = versions(&cx, &operator);
        let expected: Vec<_> = before
            .iter()
            .map(|v| if *v == hymn { new } else { *v })
            .collect();
        assert_eq!(after, expected);
        operator.read_with(&cx, |o, _| {
            assert!(o.schedule.is_dirty());
            assert_eq!(
                o.schedule_message.as_deref(),
                Some("Updated 2 item(s) in the schedule")
            );
        });
        open_update_expect_none(&mut cx, &operator, up_to_date());
    }

    fn open_update_expect_none(
        cx: &mut VisualTestContext,
        operator: &Entity<Operator>,
        text: &str,
    ) {
        let hymn = song_row(cx, operator, "Signal Hymn");
        right_click(cx, hymn);
        click(cx, "song-menu-update");
        cx.run_until_parked();
        operator.read_with(cx, |o, _| {
            assert_eq!(o.dialog, None);
            assert_eq!(o.schedule_message.as_deref(), Some(text));
        });
    }

    #[gpui::test]
    fn updating_a_live_item_leaves_the_audience_until_go_live(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator) = ready(cx, &dir);
        operator.update(&mut cx, |o, cx| {
            let version = version_of(o, "Signal Hymn");
            o.add_entry(usize::MAX, version, "Signal Hymn".into());
            cx.notify();
        });
        cx.run_until_parked();
        let ids = order(&cx, &operator);
        click(&mut cx, "live-output");
        settle(&mut cx, &operator, "output connection", connected);
        click(&mut cx, ROWS[0]);
        settle(&mut cx, &operator, "previewed", |o| {
            o.preview.as_ref().and_then(|p| p.entry) == Some(ids[0])
        });
        focus_root(&mut cx, &operator);
        cx.simulate_keystrokes("pagedown");
        settle(&mut cx, &operator, "verse on screen", |o| {
            o.on_screen() == Some("Signal Hymn · Verse 1")
        });
        let sent = operator.read_with(&cx, |o, _| submitted(o));

        let new = edit_hymn(&mut cx, &operator, &dir);
        open_update(&mut cx, &operator);
        click(&mut cx, "dialog-confirm");
        settle(&mut cx, &operator, "preview shows the new text", |o| {
            o.preview.as_ref().is_some_and(|p| {
                p.version == new && p.slides[0].text.contains("First revised line")
            })
        });
        operator.read_with(&cx, |o, _| {
            let live = o.live.as_ref().unwrap();
            assert!(live.slides[0].text.contains("First original line"));
            assert_eq!(live.entry, Some(ids[0]));
            assert_eq!(submitted(o), sent, "the audience got no new cue");
            assert_eq!(o.on_screen(), Some("Signal Hymn · Verse 1"));
        });
        focus_root(&mut cx, &operator);
        cx.simulate_keystrokes("pagedown");
        settle(
            &mut cx,
            &operator,
            "explicit Go Live shows the update",
            |o| {
                o.live
                    .as_ref()
                    .is_some_and(|l| l.slides[0].text.contains("First revised line"))
                    && submitted(o) == sent + 1
            },
        );
    }
}

mod arrangements {
    use super::*;
    use sela::arrangement::{Occurrence, OccurrenceId, Variant, VariantId};
    use sela::schedule_file;

    /// A song with sections v, c, b and two arrangements: "Full" (v c b) and
    /// "Short" (c c).
    fn library_with_arrangements(dir: &tempfile::TempDir) -> std::path::PathBuf {
        let path = dir.path().join("library.sqlite");
        let section = |label: &str, lyrics: &str| Section {
            id: SectionId::allocate(),
            label: label.into(),
            lyrics: lyrics.into(),
            format: Default::default(),
            background: None,
        };
        let sections = vec![
            section("Verse", "verse words"),
            section("Chorus", "chorus words"),
            section("Bridge", "bridge words"),
        ];
        let variant = |name: &str, order: &[usize]| Variant {
            id: VariantId(SectionId::allocate().0),
            name: name.into(),
            occurrences: order
                .iter()
                .map(|n| Occurrence {
                    id: OccurrenceId(SectionId::allocate().0),
                    section: sections[*n].id,
                })
                .collect(),
        };
        let variants = vec![variant("Full", &[0, 1, 2]), variant("Short", &[1, 1])];
        Repository::open(&path)
            .unwrap()
            .save_song(
                None,
                Song {
                    title: "Arranged".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants,
                    sections,
                    master: None,
                },
            )
            .unwrap();
        path
    }

    fn order_of(
        cx: &VisualTestContext,
        operator: &Entity<Operator>,
    ) -> Vec<sela::schedule::EntryId> {
        operator.read_with(cx, |o, _| {
            o.schedule.entries().iter().map(|e| e.id).collect()
        })
    }

    fn texts(o: &Operator) -> Vec<String> {
        o.preview
            .as_ref()
            .map(|p| p.slides.iter().map(|s| s.text.clone()).collect())
            .unwrap_or_default()
    }

    #[gpui::test]
    fn each_schedule_item_keeps_its_own_arrangement_through_save_and_reopen(
        cx: &mut TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, _) =
            with_library(cx, Some(library_with_arrangements(&dir)), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
        add_song(&mut cx, &operator, "Arranged");
        settle(&mut cx, &operator, "original", |o| texts(o).len() == 3);
        right_click(&mut cx, "schedule-item-0");
        click(&mut cx, "item-menu-duplicate");
        settle(&mut cx, &operator, "copy", |o| {
            o.preview.as_ref().and_then(|p| p.entry) == o.selected_entry
                && o.schedule.entries().len() == 2
                && texts(o).len() == 3
        });
        click(&mut cx, "preview-arrangement");
        settle(&mut cx, &operator, "short", |o| texts(o).len() == 2);
        operator.read_with(&cx, |o, _| {
            assert_eq!(texts(o), ["chorus words", "chorus words"]);
            assert_eq!(o.schedule.entries()[0].arrangement, None);
            assert!(o.schedule.entries()[1].arrangement.is_some());
            assert!(o.schedule.is_dirty(), "choosing an arrangement is an edit");
        });
        click(&mut cx, "schedule-item-0");
        settle(&mut cx, &operator, "original again", |o| {
            texts(o).len() == 3
        });

        cx.simulate_keystrokes("ctrl-s");
        let file = dir.path().join("Two.selaschedule");
        assert!(cx.did_prompt_for_new_path());
        let target = file.clone();
        cx.simulate_new_path_selection(move |_| Some(target));
        settle(&mut cx, &operator, "saved", |o| {
            o.schedule.path() == Some(file.as_path()) && !o.schedule.is_dirty()
        });
        let saved = schedule_file::read(&file).unwrap();
        assert_eq!(saved.items[0].arrangement, None);
        assert_eq!(
            saved.items[1].arrangement,
            operator.read_with(&cx, |o, _| o.schedule.entries()[1].arrangement)
        );

        // Closing and reopening restores each item's own slides.
        let before = order_of(&cx, &operator);
        open_file(&mut cx, &operator, &file);
        settle(&mut cx, &operator, "reopened", |o| {
            o.schedule.entries().len() == 2
                && !o.schedule.is_dirty()
                && o.schedule
                    .entries()
                    .iter()
                    .map(|e| e.id)
                    .collect::<Vec<_>>()
                    != before
        });
        // The rows are rebuilt for the new entries on the next frame.
        cx.simulate_resize(size(px(1281.), px(800.)));
        click(&mut cx, "schedule-item-1");
        settle(&mut cx, &operator, "copy slides", |o| {
            texts(o) == ["chorus words", "chorus words"]
        });
        click(&mut cx, "schedule-item-0");
        settle(&mut cx, &operator, "original slides", |o| {
            texts(o) == ["verse words", "chorus words", "bridge words"]
        });
        assert!(!operator.read_with(&cx, |o, _| o.schedule.is_dirty()));
    }

    #[gpui::test]
    fn live_keeps_the_audience_slides_until_the_item_is_sent_again(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, operator, _) =
            with_library(cx, Some(library_with_arrangements(&dir)), "apply");
        cx.simulate_resize(size(px(1280.), px(800.)));
        settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
        add_song(&mut cx, &operator, "Arranged");
        click(&mut cx, "schedule-item-0");
        settle(&mut cx, &operator, "preview", |o| {
            texts(o).len() == 3 && o.preview.as_ref().and_then(|p| p.entry).is_some()
        });
        click(&mut cx, "live-output");
        settle(&mut cx, &operator, "output connection", connected);
        click(&mut cx, "go-live");
        settle(&mut cx, &operator, "verse on screen", |o| {
            o.on_screen() == Some("Arranged · Verse")
        });
        let live = operator.read_with(&cx, |o, _| o.live.as_ref().unwrap().slides.len());
        assert_eq!(live, 3);

        click(&mut cx, "preview-arrangement");
        settle(&mut cx, &operator, "short preview", |o| texts(o).len() == 2);
        operator.read_with(&cx, |o, _| {
            assert_eq!(o.live.as_ref().unwrap().slides.len(), 3, "Live is stable");
            assert_eq!(o.on_screen(), Some("Arranged · Verse"));
        });
        click(&mut cx, "go-live");
        settle(&mut cx, &operator, "short live", |o| {
            o.live.as_ref().is_some_and(|l| l.slides.len() == 2)
                && o.on_screen() == Some("Arranged · Chorus")
        });
    }
}
