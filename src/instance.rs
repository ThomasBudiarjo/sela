//! Single-instance policy (M1-12). A second operator on a profile that
//! another process holds (`sela::profile::LockError::Held`) opens no library
//! and writes no profile file: it shows this notice and quits on OK, Enter,
//! Esc or the window's close button. Bringing the first window to the front
//! would need inter-process signalling, which Sela does not have (no
//! listeners); EasyWorship's behaviour here is unobserved.
use crate::ui::theme::*;
use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, FontWeight, KeyBinding, Role, TitlebarOptions,
    Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};
use sela::{
    i18n::{self, Msg},
    settings, t,
};
use std::{path::Path, rc::Rc};

actions!(instance, [Dismiss]);

pub(crate) struct AlreadyRunning {
    focus: FocusHandle,
    message: String,
    /// Quits the app; tests count instead (the test platform's quit is inert).
    dismiss: Rc<dyn Fn(&mut App)>,
}

impl Render for AlreadyRunning {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let dismiss = self.dismiss.clone();
        let click = self.dismiss.clone();
        div()
            .key_context("SelaAlreadyRunning")
            .track_focus(&self.focus)
            .on_action(move |_: &Dismiss, _, cx| dismiss(cx))
            .size_full()
            .bg(rgb(SURFACE))
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .text_size(px(TEXT_BASE))
            .text_color(rgb(TEXT))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(t!(Msg::AlreadyRunningTitle)),
            )
            .child(
                div()
                    .id("already-running-message")
                    .role(Role::Label)
                    .aria_label(self.message.clone())
                    .aria_value(self.message.clone())
                    .accessibility_id("already-running-message")
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(MUTED))
                    .child(self.message.clone()),
            )
            .child(
                div().flex().justify_end().child(
                    div()
                        .id("already-running-ok")
                        .debug_selector(|| "already-running-ok".into())
                        .role(Role::Button)
                        .aria_label(t!(Msg::ButtonOk))
                        .accessibility_id("already-running-ok")
                        .px_3()
                        .h(px(CONTROL_H))
                        .flex()
                        .items_center()
                        .rounded(px(RADIUS))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(HOVER)))
                        .active(|d| d.bg(rgb(PRESSED)))
                        .child(t!(Msg::ButtonOk))
                        .on_click(move |_, _, cx| click(cx)),
                ),
            )
    }
}

/// The notice for `profile`, in the profile's saved language (read without
/// side effects, like the layout peek before the operator window opens).
pub(crate) fn open(profile: &Path, cx: &mut App) {
    open_with(profile, Rc::new(|cx: &mut App| cx.quit()), cx);
}

fn open_with(profile: &Path, dismiss: Rc<dyn Fn(&mut App)>, cx: &mut App) {
    // Bounded like every settings read (SEC-06): an oversized file gives the
    // default language instead of being read whole.
    if let Some(language) = settings::peek(&settings::path_in(profile)).map(|s| s.language) {
        i18n::set_language(language);
    }
    cx.bind_keys([
        KeyBinding::new("enter", Dismiss, Some("SelaAlreadyRunning")),
        KeyBinding::new("escape", Dismiss, Some("SelaAlreadyRunning")),
    ]);
    let message = t!(
        Msg::AlreadyRunningBody,
        path = profile.display().to_string()
    );
    eprintln!("{message}");
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(460.), px(170.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(t!(Msg::AlreadyRunningTitle).into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| AlreadyRunning {
            focus: cx.focus_handle(),
            message,
            dismiss,
        });
        view.read(cx).focus.clone().focus(window, cx);
        view
    });
    if opened.is_err() {
        cx.quit();
    }
    cx.activate(true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    // fuzz-audit review: the notice read settings.txt whole, bypassing the
    // 1 MiB settings bound (SEC-06) on the second instance's start.
    #[gpui::test]
    fn the_notice_reads_the_language_within_the_settings_bound(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = settings::path_in(dir.path());
        let small = "sela-settings 1\nlanguage=id\n";
        let mut large = String::from(small);
        large.push_str(&"\n".repeat(settings::MAX_FILE_BYTES as usize));
        std::fs::write(&path, &large).unwrap();
        i18n::set_language(i18n::Language::English);
        cx.update(|cx| open_with(dir.path(), Rc::new(|_| {}), cx));
        assert_eq!(
            i18n::language(),
            i18n::Language::English,
            "oversized file not read"
        );
        std::fs::write(&path, small).unwrap();
        cx.update(|cx| open_with(dir.path(), Rc::new(|_| {}), cx));
        assert_eq!(i18n::language(), i18n::Language::Indonesian);
        i18n::set_language(i18n::Language::English);
    }

    #[gpui::test]
    fn the_notice_names_the_profile_and_quits_on_enter_esc_or_ok(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let quits = Rc::new(std::cell::Cell::new(0));
        let count = quits.clone();
        cx.update(|cx| open_with(dir.path(), Rc::new(move |_| count.set(count.get() + 1)), cx));
        let window = cx.update(|cx| cx.windows()[0]);
        let view = window.downcast::<AlreadyRunning>().unwrap();
        let message = view.read_with(cx, |view, _| view.message.clone()).unwrap();
        assert!(
            message.contains(&dir.path().display().to_string()),
            "{message}"
        );
        assert!(message.contains("already using this profile"));
        let mut vcx = gpui::VisualTestContext::from_window(window, cx);
        vcx.simulate_keystrokes("enter");
        assert_eq!(quits.get(), 1);
        vcx.simulate_keystrokes("escape");
        assert_eq!(quits.get(), 2);
        let ok = vcx.debug_bounds("already-running-ok").unwrap();
        vcx.simulate_click(ok.center(), Default::default());
        assert_eq!(quits.get(), 3);
        assert!(
            std::fs::read_dir(dir.path()).unwrap().next().is_none(),
            "the second instance writes nothing into the profile"
        );
    }
}
