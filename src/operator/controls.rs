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
    DialogSave,
    DialogConfirm,
    DialogCancel,
    NewSchedule,
    ItemMenuRemove,
    CancelImport,
    ReviewFiles,
    ItemMenuDuplicate,
    Arrangement,
    /// The Songs list itself. Its rows are the tab stops; the list holds
    /// focus after a click below the rows or once the focused row is no
    /// longer rendered.
    SongList,
}

/// First tab index of each focus band. Rows are placed by band so lists of
/// any length keep their position between the controls around them.
pub(crate) const SONG_TAB_INDEX: isize = 100;
pub(crate) const SCHEDULE_TAB_INDEX: isize = 500;
pub(crate) const DIALOG_TAB_INDEX: isize = 900;
pub(crate) const SLIDE_TAB_INDEX: isize = 1000;
pub(crate) const IMAGE_TAB_INDEX: isize = 2000;

impl Control {
    pub(crate) const ALL: [Control; 38] = [
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
        Self::DialogSave,
        Self::DialogConfirm,
        Self::DialogCancel,
        Self::NewSchedule,
        Self::ItemMenuRemove,
        Self::CancelImport,
        Self::ReviewFiles,
        Self::ItemMenuDuplicate,
        Self::Arrangement,
        Self::SongList,
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
                | Self::CancelImport
                | Self::UseAsLogo
                | Self::ImageMenuLogo
                | Self::Normalize
                | Self::AddToSchedule
                | Self::SongList
        )
    }

    /// Keyboard order follows declaration order, with room to place the New
    /// menu's second item next to the first and dialog buttons after their rows.
    pub(crate) fn tab_position(self) -> isize {
        match self {
            Self::NewSchedule => 2 * 11 + 1,
            Self::DialogSave => DIALOG_TAB_INDEX + 1,
            Self::DialogConfirm => DIALOG_TAB_INDEX + 2,
            Self::DialogCancel => DIALOG_TAB_INDEX + 3,
            Self::SongList => SONG_TAB_INDEX,
            _ => 2 * (self.index() as isize + 1),
        }
    }

    /// AccessKit role exposed to UI Automation (zed rev a846890
    /// gpui/src/elements/div.rs `StatefulInteractiveElement::role`).
    pub(crate) fn role(self) -> Role {
        if self.tab_number().is_some() {
            return Role::Tab;
        }
        match self {
            Self::NewSongItem
            | Self::NewSchedule
            | Self::ItemMenuRemove
            | Self::ItemMenuDuplicate
            | Self::ImageMenuLogo => Role::MenuItem,
            Self::SongList => Role::List,
            _ => Role::Button,
        }
    }
}

/// The accessibility toggle state of a mask button follows the lit
/// indicator: lit means on, a pending request is mixed (VAL-A11Y-002).
pub(crate) fn mask_toggled(state: Indicator) -> Toggled {
    match state {
        Indicator::On => Toggled::True,
        Indicator::Pending => Toggled::Mixed,
        Indicator::Off => Toggled::False,
    }
}

/// Expose a visually disabled control as disabled to assistive technology.
/// GPUI's builder has no `disabled` setter at this rev, so the flag is
/// written onto the element's accessibility node directly.
pub(crate) fn mark_disabled(element: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    element.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
}

/// One focus handle per `Control`.
pub(crate) struct ControlSet([FocusHandle; Control::COUNT]);

impl ControlSet {
    pub(crate) fn new(cx: &mut App) -> Self {
        Self(std::array::from_fn(|index| {
            let control = Control::ALL[index];
            cx.focus_handle()
                .tab_index(control.tab_position())
                .tab_stop(control != Control::SongList)
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
            Control::NewMenu => self.toggle_new_menu(),
            Control::Quit => window.dispatch_action(Box::new(Quit), cx),
            Control::CollapseResources => {
                self.collapsed = !self.collapsed;
                self.drag = None;
                if self.collapsed
                    && (self.controls.resources_focused(window)
                        || self.song_rows.values().any(|f| f.is_focused(window))
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
            Control::GoLive => self.go_live_requested(),
            Control::Previous => self.step_live(false),
            Control::Next => self.step_live(true),
            Control::Logo => self.toggle_mask(Mask::Logo),
            Control::Black => self.toggle_mask(Mask::Black),
            Control::Clear => self.toggle_mask(Mask::Clear),
            Control::ImportImage => self.prompt_import(cx),
            Control::CancelImport => self.cancel_import(),
            Control::ReviewFiles => self.review_files(window, cx),
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
                // only on the next navigation. The newest request, not only
                // the acknowledged slide: a cue still in preparation (a new
                // item's Go Live, a step) was built with the old sizing
                // (SC2-LIVE-04).
                if let Some(index) = self.requested_slide()
                    && matches!(
                        self.output.as_ref().map(Supervisor::status),
                        Some(Status::Connected { .. })
                    )
                {
                    self.send(index);
                }
            }
            Control::Arrangement => self.cycle_arrangement(),
            Control::OpenSchedule => self.open_schedule(window, cx),
            Control::SaveSchedule => self.save_schedule(cx),
            Control::AddToSchedule if !self.collapsed && self.tab == 0 => {
                let mut songs = self.selected_songs();
                if songs.is_empty() {
                    songs.extend(
                        self.selected_song
                            .filter(|_| self.selected_entry.is_none())
                            .and_then(|v| self.catalog.iter().find(|(c, _)| *c == v).cloned()),
                    );
                }
                if songs.is_empty() {
                    self.schedule_message = Some(t!(Msg::SelectSongFirst).into());
                }
                for (version, title) in songs {
                    self.add_entry(usize::MAX, version, title);
                }
            }
            Control::MoveUp | Control::MoveDown => {
                let group = self.selected_ids();
                if group.is_empty() {
                    self.schedule_message = Some(t!(Msg::SelectScheduleItem).into());
                } else {
                    self.schedule
                        .move_group_by(&group, control == Control::MoveUp);
                }
            }
            Control::RemoveItem => match self.selected_entry {
                Some(id) => self.open_dialog(Dialog::Remove(id), window, cx),
                None => self.schedule_message = Some(t!(Msg::SelectScheduleItem).into()),
            },
            Control::ItemMenuRemove => {
                if let Some((id, _)) = self.item_menu {
                    // The dialog returns focus to the row, not the menu item.
                    self.dismiss_popup_menus(true, window, cx);
                    self.open_dialog(Dialog::Remove(id), window, cx);
                }
            }
            Control::ItemMenuDuplicate => {
                if let Some((id, _)) = self.item_menu {
                    self.dismiss_popup_menus(true, window, cx);
                    self.duplicate_entries(id);
                }
            }
            Control::NewSchedule if self.new_menu => {
                self.new_menu = false;
                self.controls[Control::NewMenu].focus(window, cx);
                self.guard(Then::New, window, cx);
            }
            Control::DialogSave => {
                let Some(Dialog::Unsaved(then)) = self.dialog.clone() else {
                    return;
                };
                self.close_dialog(window, cx);
                self.save_then(Some(then), cx);
            }
            Control::DialogConfirm => match self.dialog.clone() {
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
                    if then == Then::Quit {
                        self.discard_for_quit();
                    }
                    self.proceed(then, window, cx);
                }
                Some(Dialog::Options) => {
                    self.apply_options(cx);
                    self.close_dialog(window, cx);
                }
                Some(Dialog::GoLiveMissing) => {
                    self.close_dialog(window, cx);
                    self.go_live_anyway();
                }
                Some(Dialog::UpdateItems) => {
                    self.close_dialog(window, cx);
                    self.apply_update_review();
                }
                Some(Dialog::Preflight | Dialog::FileError) | None => return,
            },
            Control::DialogCancel if self.dialog.is_some() => {
                if self.dialog == Some(Dialog::UpdateItems) {
                    self.discard_update_review();
                }
                if self.dialog == Some(Dialog::Unsaved(Then::Quit)) {
                    self.cancel_close_request();
                }
                if self.dialog == Some(Dialog::GoLiveMissing) {
                    self.hold_go_live_back();
                }
                self.close_dialog(window, cx);
            }
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
        let label = label.into();
        chrome_button(id, label.clone())
            .role(control.role())
            .aria_label(label)
            .accessibility_id(id)
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
            .aria_toggled(mask_toggled(state))
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
            .when(disabled, |d| mark_disabled(d).text_color(rgb(DISABLED)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_has_an_exposed_role() {
        for control in Control::ALL {
            assert!(
                matches!(
                    control.role(),
                    Role::Button | Role::Tab | Role::MenuItem | Role::List
                ),
                "{control:?} has no accessibility role"
            );
        }
        for tab in Control::TABS {
            assert_eq!(tab.role(), Role::Tab, "{tab:?} is a resource tab");
        }
        for item in [
            Control::NewSongItem,
            Control::NewSchedule,
            Control::ItemMenuRemove,
            Control::ItemMenuDuplicate,
            Control::ImageMenuLogo,
        ] {
            assert_eq!(
                item.role(),
                Role::MenuItem,
                "{item:?} lives in a popup menu"
            );
        }
        assert_eq!(Control::GoLive.role(), Role::Button);
        assert_eq!(Control::LiveOutput.role(), Role::Button);
    }

    #[test]
    fn mask_toggle_state_matches_the_lit_indicator() {
        assert_eq!(mask_toggled(Indicator::On), Toggled::True);
        assert_eq!(mask_toggled(Indicator::Pending), Toggled::Mixed);
        assert_eq!(mask_toggled(Indicator::Off), Toggled::False);
    }
}
