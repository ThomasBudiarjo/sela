use super::*;

/// What follows once unsaved schedule changes are discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Then {
    Open,
    /// Open Recent entry (index into the recent list).
    Recent(usize),
    /// New Schedule and Close Schedule.
    New,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dialog {
    /// Save / Don't Save / Cancel before a command that would drop edits.
    Unsaved(Then),
    /// A schedule file could not be saved or opened (ile_error).
    FileError,
    Remove(EntryId),
    /// Delete a library song (Songs menu → Delete).
    DeleteSong(Version),
    /// Options (Edit › Options in EW8-OBS-053); drawn by `options_overlay`.
    Options,
    /// Missing, changed or corrupt files found by the schedule preflight.
    Preflight,
    /// Go Live on an item whose images cannot be shown as chosen.
    GoLiveMissing,
}

impl Operator {
    pub(crate) fn open_dialog(
        &mut self,
        dialog: Dialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dialog.is_none() {
            self.dialog_return = window.focused(cx);
        }
        self.dialog = Some(dialog);
        self.dialog_message = None;
        self.item_menu = None;
        self.song_menu = None;
        self.bar_menu = None;
        self.new_menu = false;
        // The non-destructive choice, so Enter never discards by accident.
        self.controls[Control::DialogCancel].focus(window, cx);
    }

    pub(crate) fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog = None;
        self.dialog_message = None;
        self.settings.draft = None;
        self.settings.language_draft = None;
        match self.dialog_return.take() {
            Some(handle) => handle.focus(window, cx),
            None => self.focus.focus(window, cx),
        }
    }

    /// Runs `then` now, or asks first when the schedule has unsaved changes.
    pub(crate) fn guard(&mut self, then: Then, window: &mut Window, cx: &mut Context<Self>) {
        if self.schedule.is_dirty() {
            self.open_dialog(Dialog::Unsaved(then), window, cx);
        } else {
            self.proceed(then, window, cx);
        }
    }

    pub(crate) fn proceed(&mut self, then: Then, window: &mut Window, cx: &mut Context<Self>) {
        match then {
            Then::Open => self.prompt_open(cx),
            Then::Recent(index) => self.open_recent(index),
            Then::New => {
                self.schedule.clear();
                self.selected_entry = None;
                self.schedule_message = None;
            }
            Then::Quit => {
                if self.settings_allow_close(window, cx) {
                    window.remove_window();
                }
            }
        }
    }

    pub(crate) fn dialog_overlay(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let dialog = self.dialog?;
        let heading = match dialog {
            Dialog::Unsaved(_) => t!(Msg::DialogUnsaved).to_string(),
            Dialog::FileError => t!(Msg::ScheduleFileHeading).into(),
            Dialog::Remove(id) => match self.schedule.get(id) {
                Some(entry) => t!(Msg::DialogRemoveTitled, title = entry.title),
                None => t!(Msg::DialogRemoveGeneric).into(),
            },
            Dialog::Options => return None,
            Dialog::Preflight => t!(Msg::PreflightHeading).into(),
            Dialog::GoLiveMissing => t!(Msg::GoLiveMissingHeading).into(),
            Dialog::DeleteSong(version) => match self.catalog.iter().find(|(v, _)| *v == version) {
                Some((_, title)) => t!(Msg::DialogDeleteTitled, title = shown_title(title)),
                None => t!(Msg::DialogDeleteGeneric).into(),
            },
        };
        let confirm = match dialog {
            Dialog::Unsaved(_) => Some(t!(Msg::ConfirmDontSave)),
            Dialog::Remove(_) => Some(t!(Msg::ConfirmRemove)),
            Dialog::DeleteSong(_) => Some(t!(Msg::ConfirmDelete)),
            Dialog::Options | Dialog::Preflight | Dialog::FileError => None,
            Dialog::GoLiveMissing => Some(t!(Msg::GoLiveAnyway)),
        };
        let body = match dialog {
            Dialog::FileError => self.file_error.clone().map(|message| {
                div()
                    .id("schedule-file-error")
                    .debug_selector(|| "schedule-file-error".into())
                    .role(Role::Label)
                    .aria_label(message.clone())
                    .accessibility_id("schedule-file-error")
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(ERROR))
                    .child(message)
                    .into_any_element()
            }),
            Dialog::Preflight => Some(self.preflight_body(cx)),
            Dialog::GoLiveMissing => Some(self.go_live_missing_body()),
            Dialog::DeleteSong(_) => Some(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(MUTED))
                    .child(t!(Msg::DialogDeleteNote))
                    .into_any_element(),
            ),
            _ => None,
        };
        Some(
            div()
                .debug_selector(|| "schedule-dialog".into())
                .occlude()
                .absolute()
                .top(px(MENU_BAR_H + 44.))
                .left(px(8.))
                .w(px(if dialog == Dialog::Preflight {
                    PREFLIGHT_DIALOG_W
                } else {
                    380.
                }))
                .p(px(DIALOG_PAD))
                .flex()
                .flex_col()
                .gap(px(DIALOG_GAP))
                .rounded(px(6.))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(SURFACE))
                .shadow_md()
                .child(div().font_weight(FontWeight::MEDIUM).child(heading))
                .children(body)
                .children(self.dialog_message.clone().map(|message| {
                    div()
                        .text_size(px(TEXT_SMALL))
                        .text_color(rgb(ERROR))
                        .child(message)
                }))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_1()
                        .when(matches!(dialog, Dialog::Unsaved(_)), |d| {
                            d.child(
                                self.button(
                                    Control::DialogSave,
                                    "dialog-save",
                                    t!(Msg::ConfirmSave),
                                    cx,
                                )
                                .text_color(rgb(TEXT)),
                            )
                        })
                        .children(confirm.map(|label| {
                            self.button(Control::DialogConfirm, "dialog-confirm", label, cx)
                                .text_color(rgb(TEXT))
                        }))
                        .child(self.button(
                            Control::DialogCancel,
                            "dialog-cancel",
                            if matches!(dialog, Dialog::Remove(_) | Dialog::DeleteSong(_)) {
                                t!(Msg::ButtonKeep)
                            } else if dialog == Dialog::Preflight {
                                t!(Msg::PreflightClose)
                            } else if dialog == Dialog::FileError {
                                t!(Msg::ButtonOk)
                            } else {
                                t!(Msg::ButtonCancel)
                            },
                            cx,
                        )),
                )
                .into_any_element(),
        )
    }
}
