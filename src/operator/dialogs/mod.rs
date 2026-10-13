use super::*;

/// What follows once unsaved schedule changes are discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Then {
    Open,
    New,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dialog {
    SaveAs,
    Open,
    Unsaved(Then),
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
        if dialog == Dialog::SaveAs {
            let input = self.title_input.get_or_insert_with(|| {
                cx.new(|cx| {
                    TextInput::new("", false, 1024, DIALOG_TAB_INDEX, cx)
                        .expect("empty text is valid")
                })
            });
            input.update(cx, |input, cx| {
                let _ = input.set_text("", cx);
            });
            input.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            // The non-destructive choice, so Enter never discards by accident.
            self.controls[Control::DialogCancel].focus(window, cx);
        }
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
            Then::Open => {
                if self.worker.is_none() {
                    self.schedule_message = Some(t!(Msg::NoLibraryOpen).into());
                } else if self.queue(
                    storage::Command::ScheduleCatalog(None),
                    Request::ScheduleCatalog,
                ) {
                    self.saved_schedules = None;
                    self.open_dialog(Dialog::Open, window, cx);
                }
            }
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
            Dialog::SaveAs => t!(Msg::DialogSaveAs).to_string(),
            Dialog::Open => t!(Msg::DialogOpen).into(),
            Dialog::Unsaved(Then::Open) => t!(Msg::DialogUnsavedOpen).into(),
            Dialog::Unsaved(Then::New) => t!(Msg::DialogUnsavedNew).into(),
            Dialog::Unsaved(Then::Quit) => t!(Msg::DialogUnsavedQuit).into(),
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
            Dialog::SaveAs => Some(t!(Msg::ConfirmSave)),
            Dialog::Open => None,
            Dialog::Unsaved(_) => Some(t!(Msg::ConfirmDiscard)),
            Dialog::Remove(_) => Some(t!(Msg::ConfirmRemove)),
            Dialog::DeleteSong(_) => Some(t!(Msg::ConfirmDelete)),
            Dialog::Options | Dialog::Preflight => None,
            Dialog::GoLiveMissing => Some(t!(Msg::GoLiveAnyway)),
        };
        let body = match dialog {
            Dialog::SaveAs => self.title_input.clone().map(IntoElement::into_any_element),
            Dialog::Open => Some(self.saved_list(cx)),
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
                // Enter in the title field saves; the field itself ignores it.
                .capture_action(cx.listener(|this, _: &text_input::Enter, window, cx| {
                    if this.dialog == Some(Dialog::SaveAs) {
                        cx.stop_propagation();
                        this.activate(Control::DialogConfirm, window, cx);
                    }
                }))
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
