#[path = "../src/text_input.rs"]
mod text_input;
use gpui::{prelude::*, *};
use text_input::TextInput;
actions!(input_fixture, [Next, Previous]);
struct Fixture {
    title: Entity<TextInput>,
    lyrics: Entity<TextInput>,
}
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("InputFixture")
            .on_action(cx.listener(|_, _: &Next, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &Previous, window, cx| window.focus_prev(cx)))
            .size_full()
            .bg(rgb(0xf4f5f7))
            .p_6()
            .flex()
            .flex_col()
            .gap_3()
            .text_size(px(13.))
            .child("Native text input • title (64 bytes) and lyrics • Tab switches fields")
            .child(self.title.clone())
            .child(self.lyrics.clone())
            .child(format!(
                "Title {} bytes / edits {} / {:?} • Lyrics {} bytes / edits {} / {:?}",
                self.title.read(cx).text().len(),
                self.title.read(cx).edit_count(),
                self.title.read(cx).error(),
                self.lyrics.read(cx).text().len(),
                self.lyrics.read(cx).edit_count(),
                self.lyrics.read(cx).error()
            ))
    }
}
fn main() {
    gpui_platform::application().run(|cx| {
        text_input::bind_keys(cx);
        cx.bind_keys([
            KeyBinding::new("tab", Next, Some("InputFixture")),
            KeyBinding::new("shift-tab", Previous, Some("InputFixture")),
        ]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(720.), px(440.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Sela input check".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let title = cx.new(|cx| TextInput::new("Title", false, 64, 0, cx).unwrap());
                let lyrics = cx.new(|cx| {
                    TextInput::new(
                        "First line\r\nUnicode: Café 😀 e\u{301}\n\nLast line\n",
                        true,
                        256 * 1024,
                        1,
                        cx,
                    )
                    .unwrap()
                });
                title.read(cx).focus_handle(cx).focus(window, cx);
                let fixture = cx.new(|_| Fixture {
                    title: title.clone(),
                    lyrics: lyrics.clone(),
                });
                for field in [title, lyrics] {
                    cx.observe(&field, {
                        let fixture = fixture.clone();
                        move |_, cx| fixture.update(cx, |_, cx| cx.notify())
                    })
                    .detach();
                }
                fixture
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
