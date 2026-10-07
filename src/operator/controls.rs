//! Typed registry of the operator's focusable controls. Keyboard order,
//! focus handles and activation all key off `Control`, never bare integers.

use super::*;
use std::ops::Index;

/// Every focusable operator control. Declaration order is the keyboard order
/// except for the few controls that `tab_position` places explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum Control {
    ResetLayout,
    Quit,
    CollapseResources,
    TabSongs,
    TabScriptures,
    TabMedia,
    TabPresentations,
    TabThemes,
    NewSongButton,
    NewMenu,
    NewSongItem,
    LiveOutput,
    GoLive,
    Previous,
    Next,
    Logo,
    Black,
    Clear,
    ImportImage,
    UseAsLogo,
    ImageMenuLogo,
    Normalize,
    OpenSchedule,
    SaveSchedule,
    AddToSchedule,
    MoveUp,
    MoveDown,
    RemoveItem,
    DialogConfirm,
    DialogCancel,
    NewSchedule,
    ItemMenuRemove,
    Options,
}

/// First tab index of each focus band. Rows are placed by band so lists of
/// any length keep their position between the controls around them.
pub(crate) const SONG_TAB_INDEX: isize = 100;
pub(crate) const SCHEDULE_TAB_INDEX: isize = 500;
pub(crate) const SAVED_TAB_INDEX: isize = 700;
pub(crate) const DIALOG_TAB_INDEX: isize = 900;
pub(crate) const SLIDE_TAB_INDEX: isize = 1000;
pub(crate) const IMAGE_TAB_INDEX: isize = 2000;

impl Control {
    pub(crate) const ALL: [Control; 33] = [
        Self::ResetLayout,
        Self::Quit,
        Self::CollapseResources,
        Self::TabSongs,
        Self::TabScriptures,
        Self::TabMedia,
        Self::TabPresentations,
        Self::TabThemes,
        Self::NewSongButton,
        Self::NewMenu,
        Self::NewSongItem,
        Self::LiveOutput,
        Self::GoLive,
        Self::Previous,
        Self::Next,
        Self::Logo,
        Self::Black,
        Self::Clear,
        Self::ImportImage,
        Self::UseAsLogo,
        Self::ImageMenuLogo,
        Self::Normalize,
        Self::OpenSchedule,
        Self::SaveSchedule,
        Self::AddToSchedule,
        Self::MoveUp,
        Self::MoveDown,
        Self::RemoveItem,
        Self::DialogConfirm,
        Self::DialogCancel,
        Self::NewSchedule,
        Self::ItemMenuRemove,
        Self::Options,
    ];

    pub(crate) const COUNT: usize = Self::ALL.len();

    /// Resource tab buttons, in `TABS` order.
    pub(crate) const TABS: [Control; 5] = [
        Self::TabSongs,
        Self::TabScriptures,
        Self::TabMedia,
        Self::TabPresentations,
        Self::TabThemes,
    ];

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    /// The resource tab this button selects.
    pub(crate) fn tab_number(self) -> Option<usize> {
        Self::TABS.iter().position(|tab| *tab == self)
    }

    /// Controls inside the Resources area, which hide when it collapses.
    fn in_resources(self) -> bool {
        matches!(
            self,
            Self::TabSongs
                | Self::TabScriptures
                | Self::TabMedia
                | Self::TabPresentations
                | Self::TabThemes
                | Self::NewSongButton
                | Self::NewMenu
                | Self::NewSongItem
                | Self::ImportImage
                | Self::UseAsLogo
                | Self::ImageMenuLogo
                | Self::Normalize
                | Self::AddToSchedule
        )
    }

    /// Keyboard order follows declaration order, with room to place the New
    /// menu's second item next to the first and dialog buttons after their rows.
    pub(crate) fn tab_position(self) -> isize {
        match self {
            Self::NewSchedule => 2 * 11 + 1,
            Self::Options => 2 * (Self::SaveSchedule.index() as isize + 1) + 1,
            Self::DialogConfirm => DIALOG_TAB_INDEX + 1,
            Self::DialogCancel => DIALOG_TAB_INDEX + 2,
            _ => 2 * (self.index() as isize + 1),
        }
    }
}

/// One focus handle per `Control`.
pub(crate) struct ControlSet([FocusHandle; Control::COUNT]);

impl ControlSet {
    pub(crate) fn new(cx: &mut App) -> Self {
        Self(std::array::from_fn(|index| {
            cx.focus_handle()
                .tab_index(Control::ALL[index].tab_position())
                .tab_stop(true)
        }))
    }

    fn resources_focused(&self, window: &Window) -> bool {
        Control::ALL
            .iter()
            .filter(|control| control.in_resources())
            .any(|control| self[*control].is_focused(window))
    }
}

impl Index<Control> for ControlSet {
    type Output = FocusHandle;

    fn index(&self, control: Control) -> &FocusHandle {
        &self.0[control.index()]
    }
}

pub(crate) fn chrome_button(
    id: &'static str,
    label: impl Into<SharedString>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(|| id.into())
        .px_3()
        .h(px(CONTROL_H))
        .flex()
        .items_center()
        .rounded(px(RADIUS))
        .cursor_pointer()
        .text_color(rgb(MUTED))
        .hover(|d| d.bg(rgb(HOVER)).text_color(rgb(TEXT)))
        .active(|d| d.bg(rgb(PRESSED)))
        .child(label.into())
}

impl Operator {
    pub(crate) fn activate(
        &mut self,
        control: Control,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match control {
            Control::ResetLayout => {
                self.ratios = [0.24, 0.62, 0.62];
                self.collapsed = false;
                self.drag = None;
            }
            Control::NewMenu => self.new_menu = !self.new_menu,
            Control::Quit => window.dispatch_action(Box::new(Quit), cx),
            Control::CollapseResources => {
                self.collapsed = !self.collapsed;
                self.drag = None;
                if self.collapsed
                    && (self.controls.resources_focused(window)
                        || self.song_rows.iter().any(|f| f.is_focused(window))
                        || self.image_rows.iter().any(|f| f.is_focused(window)))
                {
                    self.controls[Control::CollapseResources].focus(window, cx);
                }
            }
            tab if tab.tab_number().is_some() && !self.collapsed => {
                self.tab = tab.tab_number().unwrap_or_default();
            }
            Control::NewSongButton | Control::NewSongItem => {
                self.new_menu = false;
                if control == Control::NewSongItem {
                    self.controls[Control::NewMenu].focus(window, cx);
                }
                self.open_editor(None, cx);
            }
            Control::LiveOutput => self.toggle_output(),
            Control::GoLive => self.go_live(),
            Control::Previous => self.step_live(false),
            Control::Next => self.step_live(true),
            Control::Logo => self.toggle_mask(Mask::Logo),
            Control::Black => self.toggle_mask(Mask::Black),
            Control::Clear => self.toggle_mask(Mask::Clear),
            Control::ImportImage => self.prompt_import(cx),
            Control::UseAsLogo => self.use_as_logo(),
            Control::ImageMenuLogo => {
                self.image_menu = None;
                self.controls[Control::UseAsLogo].focus(window, cx);
                self.use_as_logo();
            }
            Control::Normalize if !self.collapsed && self.tab == 0 => {
                self.sizing = match self.sizing {
                    Sizing::PerSlide => Sizing::Normalized,
                    Sizing::Normalized => Sizing::PerSlide,
                };
                // Applies to the live slide now, so its size never changes
                // only on the next navigation.
                if let Some(index) = self.live_slide
                    && matches!(
                        self.output.as_ref().map(Supervisor::status),
                        Some(Status::Connected { .. })
                    )
                {
                    self.send(index);
                }
            }
            Control::Options => self.open_options(window, cx),
            Control::OpenSchedule => self.open_schedule(window, cx),
            Control::SaveSchedule => self.save_schedule(window, cx),
            Control::AddToSchedule if !self.collapsed && self.tab == 0 => {
                match self
                    .selected_song
                    .filter(|_| self.selected_entry.is_none())
                    .and_then(|v| self.catalog.iter().find(|(c, _)| *c == v).cloned())
                {
                    Some((version, title)) => self.add_entry(usize::MAX, version, title),
                    None => self.schedule_message = Some(t!(Msg::SelectSongFirst).into()),
                }
            }
            Control::MoveUp | Control::MoveDown => match self.selected_entry {
                Some(id) => {
                    self.schedule.move_by(id, control == Control::MoveUp);
                }
                None => self.schedule_message = Some(t!(Msg::SelectScheduleItem).into()),
            },
            Control::RemoveItem => match self.selected_entry {
                Some(id) => self.open_dialog(Dialog::Remove(id), window, cx),
                None => self.schedule_message = Some(t!(Msg::SelectScheduleItem).into()),
            },
            Control::ItemMenuRemove => {
                if let Some((id, _)) = self.item_menu.take() {
                    self.open_dialog(Dialog::Remove(id), window, cx);
                }
            }
            Control::NewSchedule if self.new_menu => {
                self.new_menu = false;
                self.controls[Control::NewMenu].focus(window, cx);
                self.guard(Then::New, window, cx);
            }
            Control::DialogConfirm => match self.dialog {
                Some(Dialog::SaveAs) => {
                    let title = self
                        .title_input
                        .as_ref()
                        .map(|input| input.read(cx).text().trim().to_owned())
                        .unwrap_or_default();
                    if title.is_empty() {
                        self.dialog_message = Some(t!(Msg::EnterScheduleTitle).into());
                    } else if self.queue_save(title) {
                        self.close_dialog(window, cx);
                    }
                }
                Some(Dialog::Remove(id)) => {
                    self.close_dialog(window, cx);
                    self.remove_entry(id);
                }
                Some(Dialog::DeleteSong(version)) => {
                    self.close_dialog(window, cx);
                    self.queue_delete(version);
                }
                Some(Dialog::Unsaved(then)) => {
                    self.close_dialog(window, cx);
                    self.proceed(then, window, cx);
                }
                Some(Dialog::Options) => {
                    self.apply_options(cx);
                    self.close_dialog(window, cx);
                }
                Some(Dialog::Open) | None => return,
            },
            Control::DialogCancel if self.dialog.is_some() => self.close_dialog(window, cx),
            _ => return,
        }
        cx.notify();
    }

    pub(crate) fn button(
        &self,
        control: Control,
        id: &'static str,
        label: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        chrome_button(id, label)
            .track_focus(&self.controls[control])
            .key_context("SelaControl")
            // Background distinguishes keyboard focus from the selected-tab underline.
            .focus(|d| d.bg(rgb(FOCUS_BG)).text_color(rgb(FOCUS_TEXT)))
            .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                if this.controls[control].is_focused(window) {
                    // GPUI also synthesizes keyboard clicks for on_click; use only
                    // this semantic route for keyboard activation.
                    window.prevent_default();
                    this.activate(control, window, cx);
                }
            }))
            .on_click(cx.listener(move |this, event, window, cx| {
                if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                    return;
                }
                this.controls[control].focus(window, cx);
                this.activate(control, window, cx);
            }))
    }

    pub(crate) fn mask_button(
        &self,
        control: Control,
        id: &'static str,
        label: &'static str,
        mask: Mask,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let state = self.indicator(mask);
        let disabled = mask == Mask::Logo && self.logo.is_none() && !self.masks.is_on(mask);
        self.button(control, id, label, cx)
            .border_1()
            .border_color(rgb(if state == Indicator::Pending {
                SENDING
            } else {
                CHROME
            }))
            .when(state == Indicator::On, |d| {
                d.bg(rgb(MASK_ON))
                    .text_color(rgb(ON_SCREEN))
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(disabled, |d| d.text_color(rgb(DISABLED)))
    }
}
