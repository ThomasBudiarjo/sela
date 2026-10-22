use super::*;

actions!(operator, [DismissDialog]);

/// Upper bound on tab-stop hops while wrapping focus inside a dialog; far
/// above the number of tab stops the operator renders at once.
const TRAP_HOPS: usize = 4096;

/// What follows once unsaved schedule changes are discarded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Then {
    Open,
    /// Open Recent entry. Holds the path, not a list index: saving first
    /// reorders the recent list.
    Recent(PathBuf),
    /// New Schedule and Close Schedule.
    New,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Songs menu → Update items in Schedule: the reviewable diff.
    UpdateItems,
    /// The previous run ended uncleanly: restore its autosave (M1-12).
    Recovery,
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

    /// Tab/Shift+Tab inside a modal dialog: the next tab stop, skipping every
    /// stop outside the dialog so focus wraps within it.
    pub(crate) fn trap_focus(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for _ in 0..TRAP_HOPS {
            if forward {
                window.focus_next(cx);
            } else {
                window.focus_prev(cx);
            }
            if self.dialog_scope.contains_focused(window, cx) {
                return;
            }
        }
        self.controls[Control::DialogCancel].focus(window, cx);
    }

    /// Esc closes a dialog as its Cancel button (EW8-OBS-065 saw Esc ignored
    /// on the Delete confirmation; Sela treats it as No).
    pub(crate) fn dismiss_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            self.activate(Control::DialogCancel, window, cx);
        }
    }

    /// A full-window backdrop that keeps the pointer from the window behind
    /// and centres `dialog` over it. Keyboard focus stays inside through
    /// `trap_focus`, and Esc cancels.
    pub(crate) fn modal(
        &self,
        id: &'static str,
        dialog: gpui::AnyElement,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .debug_selector(move || id.into())
            .occlude()
            .absolute()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .track_focus(&self.dialog_scope)
            .key_context("SelaDialog")
            .on_action(cx.listener(|this, _: &DismissDialog, window, cx| {
                this.dismiss_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusNext, window, cx| {
                this.trap_focus(true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusPrevious, window, cx| {
                this.trap_focus(false, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, window, _| window.prevent_default()),
            )
            .child(dialog)
            .into_any_element()
    }

    /// Runs `then` now, or asks first when the schedule has unsaved changes.
    pub(crate) fn guard(&mut self, then: Then, window: &mut Window, cx: &mut Context<Self>) {
        // A running save would otherwise land on whichever document replaced
        // the one it was saving.
        if self.schedule_busy() {
            self.report_busy();
        } else if self.schedule.is_dirty() {
            self.open_dialog(Dialog::Unsaved(then), window, cx);
        } else {
            self.proceed(then, window, cx);
        }
    }

    pub(crate) fn proceed(&mut self, then: Then, window: &mut Window, cx: &mut Context<Self>) {
        match then {
            Then::Open => self.prompt_open(cx),
            Then::Recent(path) => self.open_recent(path),
            Then::New => {
                self.schedule.clear();
                self.clear_entry_selection();
                self.schedule_message = None;
            }
            Then::Quit => {
                if self.allow_close(window, cx) {
                    window.remove_window();
                }
            }
        }
    }

    pub(crate) fn dialog_overlay(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let dialog = self.dialog.clone()?;
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
            Dialog::UpdateItems => t!(Msg::MenuUpdateSchedule).into(),
            Dialog::Recovery if self.recovery_restorable() => t!(Msg::RecoveryHeading).into(),
            Dialog::Recovery => t!(Msg::RecoveryLostHeading).into(),
            Dialog::DeleteSong(version) => match self.catalog.iter().find(|(v, _)| *v == version) {
                Some((_, title)) => t!(Msg::DialogDeleteTitled, title = shown_title(title)),
                None => t!(Msg::DialogDeleteGeneric).into(),
            },
        };
        let confirm = match dialog {
            Dialog::Unsaved(_) => Some(t!(Msg::ConfirmDontSave)),
            Dialog::Remove(_) => Some(t!(Msg::ConfirmRemove)),
            Dialog::DeleteSong(_) => Some(t!(Msg::ConfirmYes)),
            Dialog::Options | Dialog::Preflight | Dialog::FileError => None,
            Dialog::GoLiveMissing => Some(t!(Msg::GoLiveAnyway)),
            Dialog::UpdateItems => Some(t!(Msg::UpdateItemsConfirm)),
            Dialog::Recovery => self.recovery_restorable().then(|| t!(Msg::RecoveryRestore)),
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
            Dialog::UpdateItems => Some(self.update_items_body()),
            Dialog::Recovery => Some(self.recovery_body()),
            Dialog::DeleteSong(_) => Some(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(MUTED))
                    .child(t!(Msg::DialogDeleteNote))
                    .into_any_element(),
            ),
            _ => None,
        };
        let panel = div()
            .debug_selector(|| "schedule-dialog".into())
            .max_w_full()
            .w(px(
                if matches!(
                    dialog,
                    Dialog::Preflight | Dialog::UpdateItems | Dialog::Recovery
                ) {
                    PREFLIGHT_DIALOG_W
                } else {
                    380.
                },
            ))
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
                        if matches!(dialog, Dialog::DeleteSong(_)) {
                            t!(Msg::ButtonNo)
                        } else if matches!(dialog, Dialog::Remove(_)) {
                            t!(Msg::ButtonKeep)
                        } else if dialog == Dialog::Preflight {
                            t!(Msg::PreflightClose)
                        } else if dialog == Dialog::FileError
                            || (dialog == Dialog::Recovery && !self.recovery_restorable())
                        {
                            t!(Msg::ButtonOk)
                        } else if dialog == Dialog::Recovery {
                            t!(Msg::RecoveryDiscard)
                        } else {
                            t!(Msg::ButtonCancel)
                        },
                        cx,
                    )),
            )
            .into_any_element();
        Some(self.modal("dialog-backdrop", panel, cx))
    }
}
