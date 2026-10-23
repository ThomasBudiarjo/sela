// Unsafe code is listed and justified in docs/security.md.
#![deny(unsafe_code)]

use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, KeyBinding, TitlebarOptions, Window,
    WindowBounds, WindowOptions, actions, div, point, prelude::*, px, rgb, size,
};

actions!(
    sela,
    [
        Quit,
        FocusNext,
        FocusPrevious,
        ActivateControl,
        ToggleBlack,
        ToggleLogo,
        ToggleClear,
        GoLive,
        SaveSchedule,
        OpenSchedule,
        NextScheduleItem,
        PreviousScheduleItem,
        ExtendNextScheduleItem,
        ExtendPreviousScheduleItem,
        RemoveScheduleItem,
        LiveNextSlide,
        LivePreviousSlide,
        LiveFirstSlide,
        LiveLastSlide,
        LiveNextItem,
        LivePreviousItem,
        ScheduleFirstItem,
        ScheduleLastItem,
        PreviewPreviousSlide,
        PreviewNextSlide,
        ToggleVideoPlayback,
        RestartVideo
    ]
);

/// A completed label shortcut: kind letter, optional number, Enter
/// (EW8-OBS-052, EW8-OBS-056).
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = sela, no_json)]
pub struct LabelJump {
    pub key: char,
    pub number: Option<u32>,
}

/// Highest label number reachable from the keyboard ("v 1 2 enter").
const LABEL_NUMBERS: u32 = 30;

/// Show controls (EW8-OBS-018). `!SelaTextInput` is evaluated against the
/// whole context stack, so a focused text field anywhere below keeps Ctrl+C
/// and friends for editing; `!SelaMenuBar` and `!SelaSongMenu` keep an open
/// menu modal-ish, so show shortcuts stay inert until it closes (SC2-FOCUS-01).
const SHOW: &str = "SelaShow && !SelaTextInput && !SelaMenuBar && !SelaSongMenu";
/// Shift+Up/Down extend the Schedule selection, not a Songs row focus.
const EXTEND: &str = "SelaShow && !SelaTextInput && !SelaMenuBar && !SelaSongMenu && !SelaSongRow";
/// Live navigation keys (EW8-OBS-014/050): Songs rows and Schedule rows keep
/// Home/End and the arrows for their own selection (EW8-OBS-048).
const NAVIGATE: &str = "SelaShow && !SelaTextInput && !SelaMenuBar && !SelaSongMenu \
     && !SelaSongRow && !SelaScheduleRow";

/// Label shortcuts as GPUI multi-keystroke bindings ("v 1 enter"): a pending
/// prefix times out by itself, an unknown sequence replays as ordinary keys
/// and nothing changes. Bridge needs Shift+B because bare B is Black, the
/// key presenter remotes send.
fn label_bindings() -> Vec<KeyBinding> {
    let mut bindings = Vec::new();
    for letter in ['c', 'e', 'i', 'm', 'r', 't', 'v', 'b'] {
        let prefix = if letter == 'b' {
            "shift-b".to_owned()
        } else {
            letter.to_string()
        };
        let jump = |number| LabelJump {
            key: letter,
            number,
        };
        bindings.push(KeyBinding::new(
            &format!("{prefix} enter"),
            jump(None),
            Some(NAVIGATE),
        ));
        for number in 1..=LABEL_NUMBERS {
            let digits: Vec<String> = number.to_string().chars().map(String::from).collect();
            bindings.push(KeyBinding::new(
                &format!("{prefix} {} enter", digits.join(" ")),
                jump(Some(number)),
                Some(NAVIGATE),
            ));
        }
    }
    bindings
}

fn bind_operator_keys(cx: &mut App) {
    text_input::bind_keys(cx);
    song_library::bind_keys(cx);
    cx.bind_keys(label_bindings());
    cx.bind_keys([
        KeyBinding::new("right", LiveNextSlide, Some(NAVIGATE)),
        KeyBinding::new("left", LivePreviousSlide, Some(NAVIGATE)),
        KeyBinding::new("home", LiveFirstSlide, Some(NAVIGATE)),
        KeyBinding::new("end", LiveLastSlide, Some(NAVIGATE)),
        KeyBinding::new(
            "pagedown",
            LiveNextItem,
            Some("SelaLivePane && !SelaTextInput"),
        ),
        KeyBinding::new(
            "pageup",
            LivePreviousItem,
            Some("SelaLivePane && !SelaTextInput"),
        ),
        KeyBinding::new("home", ScheduleFirstItem, Some("SelaScheduleRow")),
        KeyBinding::new("end", ScheduleLastItem, Some("SelaScheduleRow")),
        KeyBinding::new("up", PreviewPreviousSlide, Some("SelaPreviewSlide")),
        KeyBinding::new("down", PreviewNextSlide, Some("SelaPreviewSlide")),
        KeyBinding::new("b", ToggleBlack, Some(SHOW)),
        KeyBinding::new(".", ToggleBlack, Some(SHOW)),
    ]);
    cx.bind_keys([
        KeyBinding::new("ctrl-b", ToggleBlack, Some(SHOW)),
        KeyBinding::new("ctrl-l", ToggleLogo, Some(SHOW)),
        KeyBinding::new("ctrl-c", ToggleClear, Some(SHOW)),
        KeyBinding::new("pagedown", GoLive, Some(SHOW)),
        KeyBinding::new("ctrl-s", SaveSchedule, Some(SHOW)),
        KeyBinding::new("ctrl-o", OpenSchedule, Some(SHOW)),
        KeyBinding::new("down", NextScheduleItem, Some(SHOW)),
        KeyBinding::new("up", PreviousScheduleItem, Some(SHOW)),
        KeyBinding::new("shift-down", ExtendNextScheduleItem, Some(EXTEND)),
        KeyBinding::new("shift-up", ExtendPreviousScheduleItem, Some(EXTEND)),
        KeyBinding::new("ctrl-delete", RemoveScheduleItem, Some(SHOW)),
        KeyBinding::new("ctrl-q", Quit, Some("Sela")),
        KeyBinding::new("tab", FocusNext, Some("Sela")),
        KeyBinding::new("shift-tab", FocusPrevious, Some("Sela")),
        KeyBinding::new("enter", ActivateControl, Some("SelaControl")),
        KeyBinding::new("space", ActivateControl, Some("SelaControl")),
        KeyBinding::new("ctrl-s", song_library::Save, Some("SongLibrary")),
        KeyBinding::new(
            "ctrl-enter",
            song_library::SplitSection,
            Some("SongLibrary"),
        ),
    ]);
}

mod audience;
mod instance;
mod operator;
mod song_library;
mod text_input;
mod ui;
use operator::Operator;

/// Internal child modes: `sela --audience EPOCH_HEX MONITOR [BACKEND]` and
/// `sela --stage EPOCH_HEX MONITOR [BACKEND]` (the stage/confidence output,
/// M3-01/M3-02), where MONITOR is a winit index, `secondary`, `window` or
/// `name:DEVICE`. Spawned by the operator, one process per output.
fn audience_main(args: &[std::ffi::OsString], stage: bool) -> ! {
    let parsed = (|| {
        let (epoch, monitor, backend) = match args {
            [epoch, monitor] => (epoch, monitor, audience::default_backend()),
            [epoch, monitor, backend] => (epoch, monitor, backend.to_str()?),
            _ => return None,
        };
        let epoch = u128::from_str_radix(epoch.to_str()?, 16).ok()?;
        let placement = match monitor.to_str()? {
            "secondary" => audience::Placement::Fullscreen(audience::Monitor::FirstSecondary),
            "window" => audience::Placement::Window {
                width: 960,
                height: 540,
            },
            name if name.starts_with("name:") => audience::Placement::Fullscreen(
                audience::Monitor::Named(name["name:".len()..].to_owned()),
            ),
            index => audience::Placement::Fullscreen(audience::Monitor::Index(index.parse().ok()?)),
        };
        Some((epoch, placement, audience::parse_backend(backend)?))
    })();
    let Some((epoch, placement, backend)) = parsed else {
        eprintln!(
            "usage: sela <--audience|--stage> EPOCH_HEX <INDEX|secondary|window|name:DEVICE> [dx12|vulkan|metal|gl]"
        );
        std::process::exit(2);
    };
    sela::timer::raise_resolution();
    let result = audience::run(audience::Config {
        epoch: sela::delivery::Epoch(epoch),
        backend,
        title: if stage {
            "Sela stage output"
        } else {
            "Sela audience output"
        },
        placement,
        lifetime: None,
        report_surface: true,
        retain_on_disconnect: true,
        stage,
    });
    match result {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            eprintln!("{}: {error}", if stage { "stage" } else { "audience" });
            std::process::exit(1);
        }
    }
}

/// Child command for one stage session on the monitor Options › Foldback
/// resolves to (`monitor`). `SELA_STAGE_MONITOR` (a monitor index,
/// `secondary`, `window` or `name:DEVICE`) overrides it for development and
/// the X11 harness; `SELA_AUDIENCE_BACKEND` also picks the stage backend.
fn stage_launcher() -> operator::StageLauncher {
    std::sync::Arc::new(move |epoch, monitor| {
        let program = std::env::current_exe().unwrap_or_default();
        let mut command = std::process::Command::new(program);
        command
            .arg("--stage")
            .arg(format!("{:x}", epoch.0))
            .arg(std::env::var_os("SELA_STAGE_MONITOR").unwrap_or_else(|| monitor.into()));
        if let Some(backend) = std::env::var_os("SELA_AUDIENCE_BACKEND") {
            command.arg(backend);
        }
        sela::output::Launch {
            command,
            frames: sela::output::Stream::Stdout,
        }
    })
}

/// Child command for one audience session. The monitor comes from the saved
/// Options choice; `SELA_AUDIENCE_MONITOR` (a monitor index, `secondary` or
/// `window`) overrides it for development, and `SELA_AUDIENCE_BACKEND`
/// overrides the graphics backend.
fn audience_launcher(slot: std::sync::Arc<operator::MonitorSlot>) -> operator::Launcher {
    std::sync::Arc::new(move |epoch| {
        let program = std::env::current_exe().unwrap_or_default();
        let mut command = std::process::Command::new(program);
        command
            .arg("--audience")
            .arg(format!("{:x}", epoch.0))
            .arg(std::env::var_os("SELA_AUDIENCE_MONITOR").unwrap_or_else(|| slot.get().into()));
        if let Some(backend) = std::env::var_os("SELA_AUDIENCE_BACKEND") {
            command.arg(backend);
        }
        sela::output::Launch {
            command,
            frames: sela::output::Stream::Stdout,
        }
    })
}

/// Runs `sela --list-monitors` and parses its output; called off the UI thread.
fn list_monitors() -> Vec<sela::settings::MonitorInfo> {
    let Ok(program) = std::env::current_exe() else {
        return Vec::new();
    };
    std::process::Command::new(program)
        .arg("--list-monitors")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(sela::settings::MonitorInfo::from_line)
                .collect()
        })
        .unwrap_or_default()
}

/// The operator window at its saved placement, fitted to the connected work
/// areas and opened on the monitor that holds it: without a `display_id` GPUI
/// opens on the primary monitor and replaces bounds that are not on it.
fn operator_window_options(
    saved: Option<sela::pane_layout::Placement>,
    work_areas: &[(gpui::DisplayId, Bounds<gpui::Pixels>)],
    primary: Option<gpui::DisplayId>,
    default_bounds: Bounds<gpui::Pixels>,
) -> WindowOptions {
    let (window_bounds, display_id) = match saved {
        Some(saved) => {
            let areas: Vec<_> = work_areas
                .iter()
                .map(|(_, b)| sela::pane_layout::Area {
                    x: f32::from(b.origin.x).round() as i32,
                    y: f32::from(b.origin.y).round() as i32,
                    width: f32::from(b.size.width).round().max(1.) as u32,
                    height: f32::from(b.size.height).round().max(1.) as u32,
                })
                .collect();
            let primary = work_areas
                .iter()
                .position(|(id, _)| Some(*id) == primary)
                .unwrap_or(0);
            let (fit, area) = sela::pane_layout::fit_placement(saved, &areas, primary);
            let bounds = Bounds::new(
                point(px(fit.x as f32), px(fit.y as f32)),
                size(px(fit.width as f32), px(fit.height as f32)),
            );
            let bounds = if fit.maximized {
                WindowBounds::Maximized(bounds)
            } else {
                WindowBounds::Windowed(bounds)
            };
            (bounds, area.map(|index| work_areas[index].0))
        }
        None => (WindowBounds::Windowed(default_bounds), None),
    };
    WindowOptions {
        window_bounds: Some(window_bounds),
        display_id,
        titlebar: Some(TitlebarOptions {
            title: Some(sela::t!(sela::i18n::Msg::WindowTitleIdle).into()),
            ..Default::default()
        }),
        window_min_size: Some(size(px(720.), px(440.))),
        ..Default::default()
    }
}

/// The operator's profile lock, held for the life of the process.
struct HeldProfile(#[allow(dead_code)] sela::profile::ProfileLock);

impl gpui::Global for HeldProfile {}

enum Mode {
    Operator(Option<std::path::PathBuf>),
    Editor(std::path::PathBuf),
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--version" {
        println!("Sela {} (technical preview)", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.first().is_some_and(|a| a == "--audience") {
        audience_main(&args[1..], false);
    }
    if args.first().is_some_and(|a| a == "--stage") {
        audience_main(&args[1..], true);
    }
    if args.len() == 1 && args[0] == "--list-monitors" {
        match audience::list_monitors() {
            Ok(monitors) => {
                for monitor in monitors {
                    println!("{}", monitor.to_line());
                }
                return;
            }
            Err(error) => {
                eprintln!("list monitors: {error}");
                std::process::exit(1);
            }
        }
    }
    let mode = match args.as_slice() {
        [] => Mode::Operator(song_library::default_path()),
        [flag, path] if flag == "--library" => Mode::Editor(path.into()),
        [flag, path] if flag == "--operator-library" => {
            Mode::Operator(Some(sela::profile::qualified_library(path.into())))
        }
        _ => {
            eprintln!(
                "usage: sela [--version | --library DATABASE_PATH | --operator-library DATABASE_PATH]"
            );
            std::process::exit(2);
        }
    };
    sela::timer::raise_resolution();
    sela::transport::inbound().arm();
    zlog::init();
    zlog::init_output_stderr();
    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        bind_operator_keys(cx);
        let library = match mode {
            Mode::Editor(path) => {
                if let Err(error) = song_library::open(path, cx) {
                    eprintln!("{error}");
                    cx.quit();
                }
                cx.activate(true);
                return;
            }
            Mode::Operator(library) => library,
        };
        // One operator per profile (M1-12): a second one opens nothing but
        // a notice. The OS lock dies with the process, however it ends.
        let profile = library
            .as_deref()
            .and_then(std::path::Path::parent)
            .filter(|p| !p.as_os_str().is_empty());
        if let Some(profile) = profile {
            match sela::profile::ProfileLock::acquire(profile) {
                Ok(lock) => cx.set_global(HeldProfile(lock)),
                Err(sela::profile::LockError::Held) => {
                    instance::open(profile, cx);
                    return;
                }
                // A read-only or lock-less folder: run, but without autosave,
                // which says why (another operator may share the folder).
                Err(sela::profile::LockError::Unavailable(error)) => {
                    eprintln!("profile lock unavailable: {error}");
                    let reason = match sela::autosave::Error::from_io(&error) {
                        sela::autosave::Error::ReadOnly => sela::autosave::Error::ReadOnly,
                        _ => sela::autosave::Error::Unlocked,
                    };
                    cx.set_global(operator::AutosaveRefused(reason));
                }
            }
        }
        if !cx.has_global::<operator::AutosaveRefused>() {
            cx.set_global(operator::AutosaveSetup(sela::autosave::Config::system()));
        }
        let layout = library
            .as_deref()
            .and_then(std::path::Path::parent)
            .and_then(|profile| sela::settings::peek_layout(&sela::settings::path_in(profile)));
        let work_areas: Vec<_> = cx
            .displays()
            .iter()
            .map(|d| (d.id(), d.visible_bounds()))
            .collect();
        let options = operator_window_options(
            layout.and_then(|l| l.window),
            &work_areas,
            cx.primary_display().map(|d| d.id()),
            Bounds::centered(None, size(px(1280.), px(800.)), cx),
        );
        let requested = match options.window_bounds {
            Some(WindowBounds::Windowed(bounds)) => Some(bounds.size),
            _ => None,
        };
        if let Err(error) = cx.open_window(options, move |window, cx| {
            // GPUI (zed a846890, gpui_windows `retrieve_window_placement`)
            // creates the window on the primary monitor and then places it
            // on `display_id`; on a monitor with another scale the
            // WM_DPICHANGED that follows rescales the size again, so a
            // window saved on a 100% spare monitor next to a 125% primary
            // reopened 20% smaller on every launch.
            if let Some(requested) = requested {
                let opened = window.bounds().size;
                let off = (f32::from(opened.width) - f32::from(requested.width)).abs()
                    + (f32::from(opened.height) - f32::from(requested.height)).abs();
                if off > 2. {
                    window.resize(requested);
                }
            }
            let slot = std::sync::Arc::new(operator::MonitorSlot::default());
            let config = operator::SettingsConfig {
                slot: slot.clone(),
                list_monitors: std::sync::Arc::new(list_monitors),
                writer: sela::settings::default_writer(),
                loader: sela::settings::default_loader(),
                schedule_writer: sela::schedule_file::default_writer(),
                initial_layout: layout,
                media_locator: Some(std::sync::Arc::new(sela::media::ffmpeg::locate)),
            };
            let operator = cx.new(|cx| {
                let mut operator =
                    Operator::with_settings(library, audience_launcher(slot), config, cx);
                operator.set_stage_launcher(
                    stage_launcher(),
                    std::env::var_os("SELA_STAGE_MONITOR").is_some(),
                );
                operator
            });
            let weak = operator.downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                weak.update(cx, |operator, cx| operator.may_close(window, cx))
                    .unwrap_or(true)
            });
            operator.read(cx).focus.clone().focus(window, cx);
            operator
        }) {
            eprintln!("Cannot open the Sela operator window: {error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests;
