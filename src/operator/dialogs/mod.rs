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
                    self.schedule_message = Some("No song library open".into());
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
            Then::Quit => window.remove_window(),
        }
    }

    pub(crate) fn dialog_overlay(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let dialog = self.dialog?;
        let heading = match dialog {
            Dialog::SaveAs => "Save schedule as".to_string(),
            Dialog::Open => "Open schedule".into(),
            Dialog::Unsaved(Then::Open) => "Open another schedule without saving changes?".into(),
            Dialog::Unsaved(Then::New) => "Start a new schedule without saving changes?".into(),
            Dialog::Unsaved(Then::Quit) => "Quit without saving schedule changes?".into(),
            Dialog::Remove(id) => match self.schedule.get(id) {
                Some(entry) => format!("Remove “{}” from the schedule?", entry.title),
                None => "Remove this item from the schedule?".into(),
            },
            Dialog::DeleteSong(version) => match self.catalog.iter().find(|(v, _)| *v == version) {
                Some((_, title)) => {
                    format!("Delete “{}” from the song library?", shown_title(title))
                }
                None => "Delete this song from the song library?".into(),
            },
        };
        let confirm = match dialog {
            Dialog::SaveAs => Some("Save"),
            Dialog::Open => None,
            Dialog::Unsaved(_) => Some("Discard changes"),
            Dialog::Remove(_) => Some("Remove"),
            Dialog::DeleteSong(_) => Some("Delete"),
        };
        let body = match dialog {
            Dialog::SaveAs => self.title_input.clone().map(IntoElement::into_any_element),
            Dialog::Open => Some(self.saved_list(cx)),
            Dialog::DeleteSong(_) => Some(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(MUTED))
                    .child("Schedules that use this song keep their copy. Live output does not change.")
                    .into_any_element(),
            ),
            _ => None,
        };
        Some(
            div()
                .debug_selector(|| "schedule-dialog".into())
                .occlude()
                .absolute()
                .top(px(44.))
                .left(px(8.))
                .w(px(380.))
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
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
                                "Keep"
                            } else {
                                "Cancel"
                            },
                            cx,
                        )),
                )
                .into_any_element(),
        )
    }
}
