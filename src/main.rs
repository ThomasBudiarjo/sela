use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, KeyBinding, TitlebarOptions, Window,
    WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
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
        RemoveScheduleItem
    ]
);

/// Show controls (EW8-OBS-018). `!SelaTextInput` is evaluated against the
/// whole context stack, so a focused text field anywhere below keeps Ctrl+C
/// and friends for editing.
const SHOW: &str = "SelaShow && !SelaTextInput";

fn bind_operator_keys(cx: &mut App) {
    text_input::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("ctrl-b", ToggleBlack, Some(SHOW)),
        KeyBinding::new("ctrl-l", ToggleLogo, Some(SHOW)),
        KeyBinding::new("ctrl-c", ToggleClear, Some(SHOW)),
        KeyBinding::new("pagedown", GoLive, Some(SHOW)),
        KeyBinding::new("ctrl-s", SaveSchedule, Some(SHOW)),
        KeyBinding::new("ctrl-o", OpenSchedule, Some(SHOW)),
        KeyBinding::new("down", NextScheduleItem, Some(SHOW)),
        KeyBinding::new("up", PreviousScheduleItem, Some(SHOW)),
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
mod operator;
mod song_library;
mod text_input;
use operator::Operator;

/// Internal child mode: `sela --audience EPOCH_HEX MONITOR [BACKEND]`, where
/// MONITOR is a winit index, `secondary` or `window`. Spawned by the operator.
fn audience_main(args: &[std::ffi::OsString]) -> ! {
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
            index => audience::Placement::Fullscreen(audience::Monitor::Index(index.parse().ok()?)),
        };
        Some((epoch, placement, audience::parse_backend(backend)?))
    })();
    let Some((epoch, placement, backend)) = parsed else {
        eprintln!(
            "usage: sela --audience EPOCH_HEX <INDEX|secondary|window> [dx12|vulkan|metal|gl]"
        );
        std::process::exit(2);
    };
    let result = audience::run(audience::Config {
        epoch: sela::delivery::Epoch(epoch),
        backend,
        title: "Sela audience output",
        placement,
        lifetime: None,
        report_surface: true,
        centered_text: true,
        retain_on_disconnect: true,
    });
    match result {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            eprintln!("audience: {error}");
            std::process::exit(1);
        }
    }
}

/// Child command for one audience session. `SELA_AUDIENCE_MONITOR` selects a
/// monitor index, `secondary` (default) or `window`; `SELA_AUDIENCE_BACKEND`
/// overrides the graphics backend.
fn audience_launcher() -> operator::Launcher {
    std::sync::Arc::new(|epoch| {
        let program = std::env::current_exe().unwrap_or_default();
        let mut command = std::process::Command::new(program);
        command
            .arg("--audience")
            .arg(format!("{:x}", epoch.0))
            .arg(std::env::var_os("SELA_AUDIENCE_MONITOR").unwrap_or_else(|| "secondary".into()));
        if let Some(backend) = std::env::var_os("SELA_AUDIENCE_BACKEND") {
            command.arg(backend);
        }
        sela::output::Launch {
            command,
            frames: sela::output::Stream::Stdout,
        }
    })
}

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
        audience_main(&args[1..]);
    }
    let mode = match args.as_slice() {
        [] => Mode::Operator(song_library::default_path()),
        [flag, path] if flag == "--library" => Mode::Editor(path.into()),
        [flag, path] if flag == "--operator-library" => Mode::Operator(Some(path.into())),
        _ => {
            eprintln!(
                "usage: sela [--version | --library DATABASE_PATH | --operator-library DATABASE_PATH]"
            );
            std::process::exit(2);
        }
    };
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
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Sela — Technical preview".into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(720.), px(440.))),
                ..Default::default()
            },
            |window, cx| {
                let operator = cx.new(|cx| Operator::new(library, audience_launcher(), cx));
                let weak = operator.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    weak.update(cx, |operator, cx| operator.may_close(window, cx))
                        .unwrap_or(true)
                });
                operator.read(cx).focus.clone().focus(window, cx);
                operator
            },
        ) {
            eprintln!("Cannot open the Sela operator window: {error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests;
