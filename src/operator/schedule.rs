use super::*;

impl Operator {
    pub(crate) fn add_entry(&mut self, at: usize, version: Version, title: String) {
        self.schedule_message = match self.schedule.insert(at, version, title) {
            Ok(_) => None,
            Err(_) => Some(t!(Msg::ScheduleFull, max = MAX_ITEMS)),
        };
    }

    /// Never touches Live: a removed live item stays on screen (see `live_lines`).
    pub(crate) fn remove_entry(&mut self, id: EntryId) {
        if self.schedule.remove(id).is_some() && self.selected_entry == Some(id) {
            self.selected_entry = None;
        }
    }

    pub(crate) fn select_entry(&mut self, id: EntryId) {
        if let Some(entry) = self.schedule.get(id) {
            self.selected_entry = Some(id);
            self.selected_song = Some(entry.version);
            self.live_on_load = None;
            self.schedule_message = None;
        }
    }

    /// Down/Up. Stops at either end; provisional (EasyWorship unobserved).
    pub(crate) fn step_schedule(&mut self, forward: bool) {
        match self.schedule.neighbor(self.selected_entry, forward) {
            Some(id) => self.select_entry(id),
            None if self.schedule.entries().is_empty() => {
                self.schedule_message = Some(t!(Msg::ScheduleEmpty).into());
            }
            None => {}
        }
    }

    pub(crate) fn remove_selected(&mut self) {
        match self.selected_entry {
            Some(id) => self.remove_entry(id),
            None => self.schedule_message = Some(t!(Msg::SelectScheduleItem).into()),
        }
    }

    pub(crate) fn queue_save(&mut self, title: String) -> bool {
        let snapshot = self.schedule.snapshot(&title);
        let command = storage::Command::SaveSchedule(self.schedule.saved(), snapshot.clone());
        let queued = self.queue(command, Request::SaveSchedule(snapshot));
        if queued {
            self.schedule_message = Some(t!(Msg::Saving).into());
        }
        queued
    }

    pub(crate) fn queue(&mut self, command: storage::Command, request: Request) -> bool {
        if self.queued.is_some() || self.schedule_request() {
            self.schedule_message = Some(t!(Msg::WaitScheduleBusy).into());
            return false;
        }
        self.queued = Some((command, request));
        true
    }

    pub(crate) fn schedule_request(&self) -> bool {
        matches!(
            self.request,
            Some(Request::SaveSchedule(_) | Request::ScheduleCatalog | Request::OpenSchedule(_))
        )
    }

    pub(crate) fn open_schedule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.guard(Then::Open, window, cx);
    }

    pub(crate) fn save_schedule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.worker.is_none() {
            self.schedule_message = Some(t!(Msg::NoLibraryOpen).into());
            return;
        }
        match self.schedule.title().map(str::to_owned) {
            Some(title) => {
                self.queue_save(title);
            }
            None => self.open_dialog(Dialog::SaveAs, window, cx),
        }
    }

    pub(crate) fn open_saved(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((version, title)) = self
            .saved_schedules
            .as_ref()
            .and_then(|list| list.get(index))
            .cloned()
        else {
            return;
        };
        if self.queue(
            storage::Command::Schedule(version),
            Request::OpenSchedule(version),
        ) {
            self.schedule_message = Some(t!(Msg::OpeningSchedule, title = title));
            self.close_dialog(window, cx);
        }
        cx.notify();
    }

    /// Quit and window-close guard: unsaved schedule changes are confirmed
    /// first, and a save in progress finishes before the window closes.
    pub(crate) fn may_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let saving = matches!(self.request, Some(Request::SaveSchedule(_)))
            || matches!(self.queued, Some((_, Request::SaveSchedule(_))));
        if saving {
            self.schedule_message = Some(t!(Msg::SavingWaitDone).into());
        } else if self.schedule.is_dirty() {
            self.open_dialog(Dialog::Unsaved(Then::Quit), window, cx);
        } else {
            return self.settings_allow_close(window, cx);
        }
        cx.notify();
        false
    }

    pub(crate) fn schedule_pane(&mut self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.schedule.entries().len();
        while self.schedule_rows.len() < count {
            let index = self.schedule_rows.len() as isize;
            self.schedule_rows.push(
                cx.focus_handle()
                    .tab_index(SCHEDULE_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        let confirmed = self.output.as_ref().and_then(|o| match o.live() {
            LiveState::Confirmed(version) => Some(version),
            LiveState::Unknown => None,
        });
        // Marked only once the renderer acknowledged a slide of this entry.
        let on_screen = self
            .live_index(confirmed)
            .and(self.live.as_ref())
            .and_then(|item| item.entry);
        let header = match self.schedule.title() {
            Some(title) => t!(Msg::ScheduleTitled, title = title),
            None => t!(Msg::PaneSchedule).into(),
        };
        let idle = self.selected_entry.is_none();
        let rows = self
            .schedule
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let id = entry.id;
                let title = entry.title.clone();
                let selected = self.selected_entry == Some(id);
                div()
                    .id(("schedule-item", index))
                    .debug_selector(move || format!("schedule-item-{index}"))
                    .role(Role::ListItem)
                    .aria_label(title.clone())
                    .aria_selected(selected)
                    .aria_position_in_set(index)
                    .aria_size_of_set(count)
                    .accessibility_id(format!("schedule-item-{index}"))
                    .track_focus(&self.schedule_rows[index])
                    .key_context("SelaControl")
                    .flex_shrink_0()
                    .h(px(28.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .cursor_pointer()
                    .bg(rgb(if selected { SELECTED_ROW } else { SURFACE }))
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)))
                    .drag_over::<SongDrag>(|s, _, _, _| s.bg(rgb(FOCUS_BG)))
                    .drag_over::<EntryDrag>(|s, _, _, _| s.bg(rgb(FOCUS_BG)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        window.prevent_default();
                        this.select_entry(id);
                        cx.notify();
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_))
                                || event.is_right_click()
                            {
                                return;
                            }
                            this.schedule_rows[index].focus(window, cx);
                            this.select_entry(id);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                            this.schedule_rows[index].focus(window, cx);
                            this.select_entry(id);
                            this.item_menu = Some((id, event.position));
                            cx.notify();
                        }),
                    )
                    .on_drag(EntryDrag(id), {
                        let title = title.clone();
                        move |_, _, _, cx| cx.new(|_| DragLabel(title.clone().into()))
                    })
                    // Dropping on a row inserts a song before it, or moves the
                    // dragged entry into its place (provisional).
                    .on_drop(cx.listener(move |this, drag: &SongDrag, _, cx| {
                        this.add_entry(index, drag.version, drag.title.clone());
                        cx.notify();
                    }))
                    .on_drop(cx.listener(move |this, drag: &EntryDrag, _, cx| {
                        this.schedule.move_to(drag.0, index);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(18.))
                            .flex_shrink_0()
                            .text_size(px(TEXT_CAPTION))
                            .text_color(rgb(MUTED))
                            .child(format!("{}", index + 1)),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(title))
                    .when(on_screen == Some(id), |d| {
                        d.child(
                            div()
                                .debug_selector(|| "schedule-live-marker".into())
                                .flex_shrink_0()
                                .text_size(px(TEXT_CAPTION))
                                .text_color(rgb(ON_SCREEN))
                                .child(t!(Msg::ScheduleLiveMarker)),
                        )
                    })
            })
            .collect::<Vec<_>>();
        div()
            .debug_selector(|| "Schedule".into())
            .w(px(width))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .h(px(PANE_HEADER_H))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(CHROME))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(header),
                    )
                    .when(self.schedule.is_dirty(), |d| {
                        d.child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(TEXT_CAPTION))
                                .text_color(rgb(MUTED))
                                .child(t!(Msg::ScheduleUnsaved)),
                        )
                    }),
            )
            .child(
                div()
                    .id("schedule-list")
                    .debug_selector(|| "schedule-list".into())
                    .role(Role::List)
                    .aria_label(t!(Msg::PaneSchedule))
                    .accessibility_id("schedule-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .bg(rgb(SURFACE))
                    .drag_over::<SongDrag>(|s, _, _, _| s.bg(rgb(DROP_TARGET)))
                    .on_drop(cx.listener(|this, drag: &SongDrag, _, cx| {
                        this.add_entry(usize::MAX, drag.version, drag.title.clone());
                        cx.notify();
                    }))
                    .on_drop(cx.listener(|this, drag: &EntryDrag, _, cx| {
                        this.schedule.move_to(drag.0, usize::MAX);
                        cx.notify();
                    }))
                    .when(rows.is_empty(), |d| {
                        d.items_center()
                            .justify_center()
                            .p_3()
                            .text_color(rgb(MUTED))
                            .text_center()
                            .child(t!(Msg::ScheduleEmptyHint))
                    })
                    .children(rows),
            )
            .children(self.schedule_message.clone().map(|message| {
                div()
                    .debug_selector(|| "schedule-message".into())
                    .flex_shrink_0()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .text_size(px(TEXT_CAPTION))
                    .text_color(rgb(MUTED))
                    .truncate()
                    .child(message)
            }))
            .child(
                div()
                    .h(px(STATUS_BAR_H))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_1()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(CHROME))
                    .children(
                        [
                            (Control::MoveUp, "schedule-up", t!(Msg::ButtonUp)),
                            (Control::MoveDown, "schedule-down", t!(Msg::ButtonDown)),
                            (
                                Control::RemoveItem,
                                "schedule-remove",
                                t!(Msg::ButtonRemove),
                            ),
                        ]
                        .map(|(index, id, label)| {
                            self.button(index, id, label, cx)
                                .when(idle, |d| d.text_color(rgb(DISABLED)))
                        }),
                    ),
            )
    }

    pub(crate) fn saved_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(list) = &self.saved_schedules else {
            return div()
                .text_color(rgb(MUTED))
                .child(t!(Msg::LoadingSavedSchedules))
                .into_any_element();
        };
        if list.is_empty() {
            return div()
                .text_color(rgb(MUTED))
                .child(t!(Msg::NoSavedSchedules))
                .into_any_element();
        }
        while self.saved_rows.len() < list.len() {
            let index = self.saved_rows.len() as isize;
            self.saved_rows.push(
                cx.focus_handle()
                    .tab_index(SAVED_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        div()
            .id("saved-schedules")
            .role(Role::List)
            .aria_label(t!(Msg::BarOpenSchedule))
            .accessibility_id("saved-schedules")
            .max_h(px(240.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .border_1()
            .border_color(rgb(BORDER))
            .children(list.iter().enumerate().map(|(index, (_, title))| {
                div()
                    .id(("saved", index))
                    .debug_selector(move || format!("saved-{index}"))
                    .role(Role::ListItem)
                    .aria_label(title.clone())
                    .accessibility_id(format!("saved-{index}"))
                    .track_focus(&self.saved_rows[index])
                    .key_context("SelaControl")
                    .flex_shrink_0()
                    .h(px(28.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        window.prevent_default();
                        this.open_saved(index, window, cx);
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if !matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                this.open_saved(index, window, cx);
                            }
                        }),
                    )
                    .child(div().truncate().child(title.clone()))
            }))
            .into_any_element()
    }
}
