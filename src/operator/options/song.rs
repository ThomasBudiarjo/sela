//! Options › Main Output › Song tab: the Copyright Info group observed in
//! EasyWorship 8.0.49 (EW8-OBS-055, EW8-OBS-056): "Display Copyright
//! Information" (on), "Show on first displayed slide only" (off) and the
//! License Info box. The Copyright Font menu, Song Font and "Show
//! Verse/Chorus Label" controls of the same tab are not built yet.

use super::*;
use sela::slides::{CopyrightOptions, MAX_LICENSE_INFO};

/// Tab headers sit before the page controls in the Tab order.
const TAB_HEADER_INDEX: isize = 760;
const COPYRIGHT_TAB_INDEX: isize = 790;

/// The Main Output tab shown in Options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OptionsTab {
    #[default]
    General,
    Song,
}

impl OptionsTab {
    /// Every choosable tab, in strip order (one focus handle each).
    const ALL: [OptionsTab; 2] = [Self::General, Self::Song];

    fn slug(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Song => "song",
        }
    }

    fn of(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.slug() == slug)
    }
}

/// The two checkboxes of the Copyright Info group, in Tab order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Check {
    Display,
    FirstSlide,
}

impl Check {
    const ALL: [Check; 2] = [Check::Display, Check::FirstSlide];

    fn id(self) -> &'static str {
        match self {
            Self::Display => "options-copyright-display",
            Self::FirstSlide => "options-copyright-first-slide",
        }
    }

    fn label(self) -> &'static str {
        t!(match self {
            Self::Display => Msg::OptionsCopyrightDisplay,
            Self::FirstSlide => Msg::OptionsCopyrightFirstSlide,
        })
    }

    fn get(self, options: &CopyrightOptions) -> bool {
        match self {
            Self::Display => options.display,
            Self::FirstSlide => options.first_slide_only,
        }
    }

    fn flip(self, options: &mut CopyrightOptions) {
        match self {
            Self::Display => options.display = !options.display,
            Self::FirstSlide => options.first_slide_only = !options.first_slide_only,
        }
    }
}

/// Song tab state while Options is open; the License Info text lives in its
/// field until OK.
pub(crate) struct SongOptions {
    pub(crate) tab: OptionsTab,
    /// Copyright choices being edited (`None` while Options is closed).
    pub(crate) draft: Option<CopyrightOptions>,
    tabs: Vec<FocusHandle>,
    checks: Vec<FocusHandle>,
    pub(crate) license: Entity<TextInput>,
}

impl SongOptions {
    pub(crate) fn new(cx: &mut Context<Operator>) -> Self {
        let license = cx.new(|cx| {
            TextInput::new(
                "",
                false,
                MAX_LICENSE_INFO,
                COPYRIGHT_TAB_INDEX + Check::ALL.len() as isize,
                cx,
            )
            .expect("empty text is valid")
        });
        Self {
            tab: OptionsTab::General,
            draft: None,
            tabs: OptionsTab::ALL
                .iter()
                .map(|&tab| {
                    cx.focus_handle()
                        .tab_index(TAB_HEADER_INDEX + tab as isize)
                        .tab_stop(true)
                })
                .collect(),
            checks: Check::ALL
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    cx.focus_handle()
                        .tab_index(COPYRIGHT_TAB_INDEX + i as isize)
                        .tab_stop(true)
                })
                .collect(),
            license,
        }
    }
}

#[cfg(test)]
impl SongOptions {
    pub(crate) fn tab_focused(&self, index: usize, window: &Window) -> bool {
        self.tabs[index].is_focused(window)
    }

    pub(crate) fn check_focused(&self, index: usize, window: &Window) -> bool {
        self.checks[index].is_focused(window)
    }
}

impl Operator {
    /// Options opened: the General tab, drafts from the stored settings.
    pub(crate) fn begin_song_options(&mut self, cx: &mut Context<Self>) {
        let current = self.settings.current.copyright.clone();
        self.settings.song.tab = OptionsTab::General;
        self.reset_song_options(current, cx);
    }

    /// Replaces the drafts, e.g. when the stored settings load late.
    pub(crate) fn reset_song_options(&mut self, current: CopyrightOptions, cx: &mut Context<Self>) {
        let license = current.license_info.clone();
        self.settings.song.draft = Some(current);
        self.settings.song.license.update(cx, |input, cx| {
            let _ = input.set_text(&license, cx);
        });
    }

    /// The edited copyright choices with the License Info field's text.
    pub(crate) fn song_options_draft(&self, cx: &App) -> Option<CopyrightOptions> {
        let mut draft = self.settings.song.draft.clone()?;
        draft.license_info =
            CopyrightOptions::clean_license(self.settings.song.license.read(cx).text());
        Some(draft)
    }

    fn choose_options_tab(&mut self, tab: OptionsTab, cx: &mut Context<Self>) {
        self.settings.song.tab = tab;
        cx.notify();
    }

    fn toggle_copyright(&mut self, check: Check, cx: &mut Context<Self>) {
        if !self.settings.loaded {
            return;
        }
        if let Some(draft) = &mut self.settings.song.draft {
            check.flip(draft);
            cx.notify();
        }
    }

    /// The Main Output tab strip: General and Song can be chosen, the other
    /// observed tabs are shown as not available yet.
    pub(crate) fn options_tabs(
        &mut self,
        tabs: &[(Msg, &'static str, bool)],
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let shown = self.settings.song.tab;
        tabs.iter()
            .map(|&(msg, slug, _)| {
                let Some(tab) = OptionsTab::of(slug) else {
                    return div()
                        .debug_selector(move || format!("options-tab-{slug}"))
                        .h_full()
                        .px_2()
                        .flex()
                        .items_center()
                        .border_b_2()
                        .border_color(rgb(SURFACE))
                        .text_color(rgb(DISABLED))
                        .child(t!(msg))
                        .into_any_element();
                };
                let handle = self.settings.song.tabs[tab as usize].clone();
                let selected = tab == shown;
                div()
                    .id(("options-tab", tab as usize))
                    .debug_selector(move || format!("options-tab-{slug}"))
                    .role(Role::Tab)
                    .aria_label(t!(msg))
                    .aria_selected(selected)
                    .accessibility_id(match tab {
                        OptionsTab::General => "options-tab-general",
                        OptionsTab::Song => "options-tab-song",
                    })
                    .track_focus(&handle)
                    .key_context("SelaControl")
                    .h_full()
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_2()
                    .border_color(rgb(if selected { ACCENT } else { SURFACE }))
                    .text_color(rgb(if selected { TEXT } else { MUTED }))
                    .when(selected, |d| d.font_weight(FontWeight::MEDIUM))
                    .cursor_pointer()
                    .hover(|d| d.text_color(rgb(TEXT)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        if this.settings.song.tabs[tab as usize].is_focused(window) {
                            window.prevent_default();
                            this.choose_options_tab(tab, cx);
                        }
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                return;
                            }
                            this.settings.song.tabs[tab as usize].focus(window, cx);
                            this.choose_options_tab(tab, cx);
                        }),
                    )
                    .child(t!(msg))
                    .into_any_element()
            })
            .collect()
    }

    /// The Song tab page: the Copyright Info group.
    pub(crate) fn song_options_page(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let draft = self.settings.song.draft.clone().unwrap_or_default();
        // Until the stored settings load, OK cannot save and the load
        // replaces the drafts, so nothing here takes input (the General
        // tab's monitor rows behave the same way).
        let loaded = self.settings.loaded;
        self.settings.song.license.update(cx, |input, _| {
            input.set_placeholder(t!(Msg::OptionsLicenseInfoPlaceholder));
            input.set_accessible("options-license-info", t!(Msg::OptionsLicenseInfo));
            input.set_read_only(!loaded);
        });
        let checks = Check::ALL
            .into_iter()
            .enumerate()
            .map(|(index, check)| {
                let on = check.get(&draft);
                div()
                    .id(("options-copyright", index))
                    .debug_selector(move || check.id().into())
                    .role(Role::CheckBox)
                    .aria_label(check.label())
                    .aria_toggled(if on { Toggled::True } else { Toggled::False })
                    .accessibility_id(check.id())
                    .track_focus(&self.settings.song.checks[index])
                    .key_context("SelaControl")
                    .h(px(CONTROL_H + 2.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(RADIUS))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        if this.settings.song.checks[index].is_focused(window) {
                            window.prevent_default();
                            this.toggle_copyright(check, cx);
                        }
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                return;
                            }
                            this.settings.song.checks[index].focus(window, cx);
                            this.toggle_copyright(check, cx);
                        }),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .w(px(14.))
                            .h(px(14.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(rgb(if on { ACCENT } else { BORDER }))
                            .bg(rgb(if on { ACCENT } else { SURFACE }))
                            .text_size(px(TEXT_CAPTION))
                            .text_color(rgb(SURFACE))
                            .child(if on { "\u{2713}" } else { "" }),
                    )
                    .child(div().flex_1().truncate().child(check.label()))
            })
            .collect::<Vec<_>>();
        div()
            .debug_selector(|| "options-song-page".into())
            .flex()
            .flex_col()
            .gap(px(DIALOG_GAP))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(t!(Msg::OptionsCopyrightHeading)),
            )
            .child(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(MUTED))
                    .child(t!(Msg::OptionsCopyrightHint)),
            )
            .children((!loaded).then(|| {
                div()
                    .debug_selector(|| "options-song-loading".into())
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(MUTED))
                    .child(t!(Msg::LoadingSettings))
            }))
            .child(div().flex().flex_col().gap_1().children(checks))
            .child(
                div()
                    .mt_2()
                    .font_weight(FontWeight::MEDIUM)
                    .child(t!(Msg::OptionsLicenseInfo)),
            )
            .child(
                div()
                    .debug_selector(|| "options-license-info".into())
                    .child(self.settings.song.license.clone()),
            )
            .child(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(MUTED))
                    .child(t!(Msg::OptionsLicenseInfoHint)),
            )
            .into_any_element()
    }
}
