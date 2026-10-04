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

mod operator;
mod song_library;
mod text_input;
use operator::Operator;

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--version" {
        println!("Sela {} (technical preview)", env!("CARGO_PKG_VERSION"));
        return;
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
