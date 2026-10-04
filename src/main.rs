use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, KeyBinding, TitlebarOptions, Window,
    WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};

actions!(sela, [Quit]);

struct Operator {
    focus: FocusHandle,
}

impl Operator {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
        }
    }
}

impl Render for Operator {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Sela")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &Quit, _, cx| cx.quit()))
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .p_8()
            .gap_4()
            .bg(rgb(0xf4f6f9))
            .text_color(rgb(0x182536))
            .font_family("DejaVu Sans")
            .child(div().text_3xl().child("Sela"))
            .child("Offline worship presentation")
            .child(
                div()
                    .text_sm()
                    .child("Technical preview · Native GPUI operator bootstrap"),
            )
            .child("Service authoring and audience output are not available in this build.")
            .child(
                div()
                    .id("quit")
                    .cursor_pointer()
                    .p_3()
                    .bg(rgb(0xdce5f0))
                    .on_click(cx.listener(|_, _, _, cx| cx.quit()))
                    .child("Quit · Ctrl+Q"),
            )
    }
}

fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("Sela {} (technical preview)", env!("CARGO_PKG_VERSION"));
        return;
    }
    zlog::init();
    zlog::init_output_stderr();
    gpui_platform::application().run(|cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.bind_keys([KeyBinding::new("ctrl-q", Quit, Some("Sela"))]);
        let bounds = Bounds::centered(None, size(px(960.), px(600.)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Sela — Technical preview".into()),
                    ..Default::default()
                }),
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
