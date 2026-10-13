use super::*;
use update_items::UpdateReview;

pub(crate) const SCHEDULE_ROW_H: f32 = 28.;
/// Pointer distance from the list edge where a drag starts autoscrolling.
const EDGE_ZONE: f32 = 24.;
const SCROLL_STEP: f32 = 10.;
const SCROLL_TICK: Duration = Duration::from_millis(30);

/// Schedule pane state beyond the focused entry.
#[derive(Default)]
pub(crate) struct ScheduleView {
    /// Fixed end of a Shift range; the focused entry is the moving end.
    anchor: Option<EntryId>,
    /// Every selected entry, including the focused one.
    selected: HashSet<EntryId>,
    /// Drop indicator: the group lands before the row now at this index.
    drop_before: Option<usize>,
    pub(crate) scroll: ScrollHandle,
    list_bounds: Option<gpui::Bounds<Pixels>>,
    pointer: Option<Point<Pixels>>,
    scroller: Option<Task<()>>,
    /// Update items was chosen while the catalog was refreshing.
    pub(crate) scan_wanted: bool,
    pub(crate) review: Option<UpdateReview>,
}

/// Horizontal insertion line at a row's top (`before`) or bottom edge.
fn drop_line(top: bool) -> gpui::Div {
    let line = div()
        .debug_selector(|| "schedule-drop-indicator".into())
        .absolute()
        .left_0()
        .right_0()
        .h(px(2.))
        .bg(rgb(ACCENT));
    if top { line.top_0() } else { line.bottom_0() }
}

impl Operator {
    pub(crate) fn is_entry_selected(&self, id: EntryId) -> bool {
        self.view.selected.contains(&id)
    }

    /// Selected entries in schedule order.
    pub(crate) fn selected_ids(&self) -> Vec<EntryId> {
        self.schedule
            .entries()
            .iter()
            .map(|e| e.id)
            .filter(|id| self.view.selected.contains(id))
            .collect()
    }

    pub(crate) fn clear_entry_selection(&mut self) {
        self.selected_entry = None;
        self.view.anchor = None;
        self.view.selected.clear();
    }

    /// The entries a drag or menu command acts on: the whole selection when
    /// `id` is part of it, otherwise just `id`.
    pub(crate) fn group_of(&self, id: EntryId) -> Vec<EntryId> {
        if self.view.selected.contains(&id) {
            self.selected_ids()
        } else {
            vec![id]
        }
    }

    /// Ctrl+click: adds or removes one entry. An added entry becomes the
    /// focused one; removing the focused one hands focus to the first entry
    /// still selected.
    pub(crate) fn toggle_entry(&mut self, id: EntryId) {
        if self.schedule.get(id).is_none() {
            return;
        }
        if !self.view.selected.remove(&id) {
            self.view.selected.insert(id);
            self.focus_entry(id);
            self.view.anchor = Some(id);
            return;
        }
        if self.selected_entry == Some(id) {
            match self.selected_ids().first().copied() {
                Some(next) => self.focus_entry(next),
                None => self.selected_entry = None,
            }
        }
        if self.view.anchor == Some(id) {
            self.view.anchor = self.selected_entry;
        }
    }

    /// Shift+click or Shift+Down/Up: selects the range from the anchor to
    /// `id` and focuses `id`.
    pub(crate) fn extend_to(&mut self, id: EntryId) {
        let Some(to) = self.schedule.index(id) else {
            return;
        };
        let anchor = self
            .view
            .anchor
            .filter(|a| self.schedule.get(*a).is_some())
            .or(self.selected_entry)
            .unwrap_or(id);
        let from = self.schedule.index(anchor).unwrap_or(to);
        let (low, high) = (from.min(to), from.max(to));
        self.view.selected = self.schedule.entries()[low..=high]
            .iter()
            .map(|e| e.id)
            .collect();
        self.view.anchor = Some(anchor);
        self.focus_entry(id);
    }

    pub(crate) fn add_entry(&mut self, at: usize, version: Version, title: String) {
        self.schedule_message = match self.schedule.insert(at, version, title) {
            Ok(_) => None,
            Err(_) => Some(t!(Msg::ScheduleFull, max = MAX_ITEMS)),
        };
    }

    /// Context menu Duplicate: a copy of each selected entry right after its
    /// original. The copies become the selection.
    pub(crate) fn duplicate_entries(&mut self, id: EntryId) {
        let group = self.group_of(id);
        match self.schedule.duplicate(&group) {
            Ok(copies) => {
                self.schedule_message = None;
                if let Some(last) = copies.last().copied() {
                    self.view.selected = copies.iter().copied().collect();
                    self.view.anchor = copies.first().copied();
                    self.focus_entry(last);
                }
            }
            Err(_) => self.schedule_message = Some(t!(Msg::ScheduleFull, max = MAX_ITEMS)),
        }
    }

    /// Drop of a dragged entry (or its selection) at the drop indicator.
    pub(crate) fn drop_entries(&mut self, dragged: EntryId, before: usize) {
        let group = self.group_of(dragged);
        self.schedule.move_group(&group, before);
        self.view.drop_before = None;
        self.view.scroller = None;
    }

    pub(crate) fn drop_indicator(&self) -> Option<usize> {
        self.view.drop_before
    }

    /// Pointer over the list during an entry drag: where it would land, and
    /// whether it is close enough to an edge to scroll.
    fn drag_over_list(
        &mut self,
        event: &DragMoveEvent<EntryDrag>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.view.list_bounds = Some(event.bounds);
        self.view.pointer = Some(event.event.position);
        if !event.bounds.contains(&event.event.position) {
            self.view.drop_before = None;
            return;
        }
        self.locate_drop();
        let y = event.event.position.y;
        let near_top = y < event.bounds.top() + px(EDGE_ZONE);
        let near_bottom = y > event.bounds.bottom() - px(EDGE_ZONE);
        if (near_top || near_bottom) && self.view.scroller.is_none() {
            let up = near_top;
            self.view.scroller = Some(cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor().timer(SCROLL_TICK).await;
                    let go = this
                        .update_in(cx, |this, _, cx| {
                            if !cx.has_active_drag() || !this.autoscroll(up) {
                                this.view.scroller = None;
                                return false;
                            }
                            cx.notify();
                            true
                        })
                        .unwrap_or(false);
                    if !go {
                        break;
                    }
                }
            }));
        }
        cx.notify();
    }

    /// Scrolls one step while the pointer stays in the edge zone. False when
    /// it left the zone or the list cannot scroll further that way.
    fn autoscroll(&mut self, up: bool) -> bool {
        let (Some(bounds), Some(pointer)) = (self.view.list_bounds, self.view.pointer) else {
            return false;
        };
        let in_zone = if up {
            pointer.y < bounds.top() + px(EDGE_ZONE)
        } else {
            pointer.y > bounds.bottom() - px(EDGE_ZONE)
        } && bounds.contains(&pointer);
        let handle = &self.view.scroll;
        let mut offset = handle.offset();
        let limit = handle.max_offset().y;
        let next = if up {
            (offset.y + px(SCROLL_STEP)).min(px(0.))
        } else {
            (offset.y - px(SCROLL_STEP)).max(-limit)
        };
        if !in_zone || next == offset.y {
            return false;
        }
        offset.y = next;
        handle.set_offset(offset);
        self.locate_drop();
        true
    }

    /// Nearest row boundary under the pointer, in schedule order.
    fn locate_drop(&mut self) {
        let (Some(bounds), Some(pointer)) = (self.view.list_bounds, self.view.pointer) else {
            return;
        };
        let rel = f32::from(pointer.y - bounds.top() - self.view.scroll.offset().y);
        let boundary = (rel / SCHEDULE_ROW_H).round().max(0.) as usize;
        self.view.drop_before = Some(boundary.min(self.schedule.entries().len()));
    }

    /// Esc during an entry drag: nothing moves.
    pub(crate) fn cancel_entry_drag(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !cx.has_active_drag() {
            return false;
        }
        cx.stop_active_drag(window);
        self.view.drop_before = None;
        self.view.scroller = None;
        cx.notify();
        true
    }

    /// Never touches Live: a removed live item stays on screen (see `live_lines`).
    pub(crate) fn remove_entry(&mut self, id: EntryId) {
        if self.schedule.remove(id).is_some() {
            self.view.selected.remove(&id);
            if self.view.anchor == Some(id) {
                self.view.anchor = None;
            }
            if self.selected_entry == Some(id) {
                self.selected_entry = None;
            }
        }
    }

    /// Focuses one entry (Preview follows) without touching the selection set.
    fn focus_entry(&mut self, id: EntryId) {
        if let Some(entry) = self.schedule.get(id) {
            self.selected_entry = Some(id);
            self.selected_song = Some(entry.version);
            self.live_on_load = None;
            self.schedule_message = None;
        }
    }

    /// Plain click or keyboard selection: this entry alone.
    pub(crate) fn select_entry(&mut self, id: EntryId) {
        if self.schedule.get(id).is_some() {
            self.focus_entry(id);
            self.view.anchor = Some(id);
            self.view.selected.clear();
            self.view.selected.insert(id);
        }
    }

    /// Down/Up (`extend`: Shift+Down/Up). Stops at either end; provisional
    /// (EasyWorship unobserved).
    pub(crate) fn step_schedule(&mut self, forward: bool, extend: bool) {
        match self.schedule.neighbor(self.selected_entry, forward) {
            Some(id) if extend => self.extend_to(id),
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

    pub(crate) fn queue(&mut self, command: storage::Command, request: Request) -> bool {
        if self.queued.is_some() || self.schedule_request() {
            self.schedule_message = Some(t!(Msg::WaitScheduleBusy).into());
            return false;
        }
        self.queued = Some((command, request));
        true
    }

    pub(crate) fn schedule_request(&self) -> bool {
        self.schedule_busy()
    }

    /// Quit and window-close guard: unsaved schedule changes are confirmed
    /// first, and a save in progress finishes before the window closes.
    pub(crate) fn may_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.saving_file() {
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
        let header = match self.schedule.name() {
            Some(name) => t!(Msg::ScheduleTitled, title = name),
            None => t!(Msg::PaneSchedule).into(),
        };
        let dirty = self.schedule.is_dirty();
        let idle = self.selected_entry.is_none();
        if !cx.has_active_drag() {
            self.view.drop_before = None;
        }
        let drop_at = self.view.drop_before;
        let chosen = self.view.selected.len();
        let rows = self
            .schedule
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let id = entry.id;
                let title = entry.title.clone();
                let selected = self.is_entry_selected(id);
                let drag_title = if selected && chosen > 1 {
                    format!("{title} (+{})", chosen - 1)
                } else {
                    title.clone()
                };
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
                    .relative()
                    .h(px(SCHEDULE_ROW_H))
                    .px(px(PANE_PAD_X))
                    .flex()
                    .items_center()
                    .gap(px(ROW_GAP))
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
                            let keys = event.modifiers();
                            if keys.shift {
                                this.extend_to(id);
                            } else if keys.secondary() {
                                this.toggle_entry(id);
                            } else {
                                this.select_entry(id);
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                            this.schedule_rows[index].focus(window, cx);
                            if !this.is_entry_selected(id) {
                                this.select_entry(id);
                            }
                            this.item_menu = Some((id, event.position));
                            cx.notify();
                        }),
                    )
                    .on_drag(EntryDrag(id), move |_, _, _, cx| {
                        cx.new(|_| DragLabel(drag_title.clone().into()))
                    })
                    // Dropping a library song on a row inserts it before the row.
                    .on_drop(cx.listener(move |this, drag: &SongDrag, _, cx| {
                        this.add_entry(index, drag.version, drag.title.clone());
                        cx.notify();
                    }))
                    .on_drop(cx.listener(move |this, drag: &EntryDrag, _, cx| {
                        let before = this.drop_indicator().unwrap_or(index);
                        this.drop_entries(drag.0, before);
                        cx.notify();
                    }))
                    .when(drop_at == Some(index), |d| d.child(drop_line(true)))
                    .when(drop_at == Some(count) && index + 1 == count, |d| {
                        d.child(drop_line(false))
                    })
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
                    .gap(px(ROW_GAP))
                    .px(px(PANE_PAD_X))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(CHROME))
                    .child(
                        div()
                            .id("schedule-title")
                            .debug_selector(|| "schedule-title".into())
                            .role(Role::Heading)
                            .aria_label(header.clone())
                            .accessibility_id("schedule-title")
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(header),
                    )
                    .when(dirty, |d| {
                        d.child(
                            div()
                                .id("schedule-unsaved")
                                .debug_selector(|| "schedule-unsaved".into())
                                .role(Role::Label)
                                .aria_label(t!(Msg::ScheduleUnsaved))
                                .accessibility_id("schedule-unsaved")
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
                    .track_scroll(&self.view.scroll)
                    .flex()
                    .flex_col()
                    .bg(rgb(SURFACE))
                    .drag_over::<SongDrag>(|s, _, _, _| s.bg(rgb(DROP_TARGET)))
                    .on_drag_move::<EntryDrag>(cx.listener(
                        |this, event: &DragMoveEvent<EntryDrag>, window, cx| {
                            this.drag_over_list(event, window, cx);
                        },
                    ))
                    .on_drop(cx.listener(|this, drag: &SongDrag, _, cx| {
                        this.add_entry(usize::MAX, drag.version, drag.title.clone());
                        cx.notify();
                    }))
                    .on_drop(cx.listener(|this, drag: &EntryDrag, _, cx| {
                        let before = this.drop_indicator().unwrap_or(usize::MAX);
                        this.drop_entries(drag.0, before);
                        cx.notify();
                    }))
                    .when(rows.is_empty(), |d| {
                        d.items_center()
                            .justify_center()
                            .p(px(PANE_PAD_X))
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
                    .px(px(PANE_PAD_X))
                    .py_1()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .text_size(px(TEXT_CAPTION))
                    .text_color(rgb(MUTED))
                    .truncate()
                    .child(message)
            }))
            .when(self.assets.needs_review(), |d| {
                d.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .px_1()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .child(self.button(
                            Control::ReviewFiles,
                            "review-files",
                            t!(Msg::PreflightReview),
                            cx,
                        )),
                )
            })
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
}
