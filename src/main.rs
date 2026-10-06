use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, KeyBinding, TitlebarOptions, Window,
    WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};

actions!(sela, [Quit, FocusNext, FocusPrevious, ActivateControl]);

fn bind_operator_keys(cx: &mut App) {
    text_input::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("ctrl-q", Quit, Some("Sela")),
        KeyBinding::new("tab", FocusNext, Some("Sela")),
        KeyBinding::new("shift-tab", FocusPrevious, Some("Sela")),
        KeyBinding::new("enter", ActivateControl, Some("SelaControl")),
        KeyBinding::new("space", ActivateControl, Some("SelaControl")),
        KeyBinding::new("ctrl-s", song_library::Save, Some("SongLibrary")),
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

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--version" {
        println!("Sela {} (technical preview)", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.first().is_some_and(|a| a == "--audience") {
        audience_main(&args[1..]);
    }
    let library = if args.len() == 2 && args[0] == "--library" {
        Some(std::path::PathBuf::from(&args[1]))
    } else if args.is_empty() {
        None
    } else {
        eprintln!("usage: sela [--version | --library DATABASE_PATH]");
        std::process::exit(2);
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
        if let Some(path) = library {
            if let Err(error) = song_library::open(path, cx) {
                eprintln!("{error}");
                cx.quit();
            }
            cx.activate(true);
            return;
        }
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
                let operator = cx.new(Operator::new);
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
