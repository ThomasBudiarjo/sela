use super::song_select::{SONG_ROW_H, SongStep};
use super::*;

actions!(
    operator,
    [
        NextSongRow,
        PreviousSongRow,
        ClearSongSearch,
        ExtendNextSongRow,
        ExtendPreviousSongRow,
        FirstSongRow,
        LastSongRow,
        ExtendFirstSongRow,
        ExtendLastSongRow,
        PageDownSongRow,
        PageUpSongRow,
        SelectAllSongRows,
        DeleteSongRow
    ]
);

/// After the song rows (100+) and before the schedule (500), matching its
/// place under the list; adds no stop before the first song row.
pub(crate) const SEARCH_TAB_INDEX: isize = 499;
const SEARCH_WAKE: std::time::Duration = std::time::Duration::from_millis(2);
const SCROLLBAR_W: f32 = 10.;
const THUMB_MIN_H: f32 = 28.;

/// Payload of a Songs scrollbar drag.
struct SongScrollGrab;

impl Operator {
    /// The row's focus handle; created the first time the row is built.
    pub(crate) fn song_row_handle(&mut self, index: usize, cx: &mut App) -> FocusHandle {
        self.song_rows
            .entry(index)
            .or_insert_with(|| cx.focus_handle().tab_index(SONG_TAB_INDEX))
            .clone()
    }

    pub(crate) fn focus_song_row(&mut self, index: usize, window: &mut Window, cx: &mut App) {
        self.song_row_handle(index, cx).focus(window, cx);
    }

    /// Opens a Song Editor window: a new song, or the given saved revision.
    pub(crate) fn open_editor(&mut self, version: Option<Version>, cx: &mut Context<Self>) {
        self.library_error = self
            .library
            .clone()
            .or_else(crate::song_library::default_path)
            .ok_or_else(|| t!(Msg::NoUserDataDir).to_string())
            .and_then(|path| crate::song_library::open_song(path, version, cx))
            .err();
    }

    /// Deletes through the storage worker. Schedules keep their pinned
    /// revisions and nothing is sent to the audience.
    pub(crate) fn queue_delete(&mut self, version: Version) {
        let title = self
            .catalog
            .iter()
            .find(|(v, _)| *v == version)
            .map(|(_, title)| title.clone())
            .unwrap_or_default();
        self.song_message = Some(if self.worker.is_none() {
            (t!(Msg::NoLibraryOpen).into(), true)
        } else if self.queued.is_some() || matches!(self.request, Some(Request::DeleteSong(..))) {
            (t!(Msg::DeleteBusy).into(), true)
        } else {
            self.queued = Some((
                storage::Command::DeleteSong(version),
                Request::DeleteSong(version, title),
            ));
            (t!(Msg::Deleting).into(), false)
        });
    }

    /// The rows Songs shows: search hits while a query of two or more
    /// characters has answered, otherwise the whole catalog.
    pub(crate) fn songs(&self) -> &[(Version, String)] {
        self.search.hits().unwrap_or(&self.catalog)
    }

    /// Typed text changed. Submits through the storage worker at once; the
    /// reply is picked up by the next poll.
    pub(crate) fn search_text_changed(&mut self, cx: &mut Context<Self>) {
        let text = self.search_input.read(cx).text().to_string();
        if self.search.set_text(&text) {
            self.poll(cx);
            self.watch_search(cx);
            cx.notify();
        }
    }

    /// Picks up the worker's search reply every 2 ms instead of waiting for the
    /// 16 ms poll tick, until the newest query has been answered.
    fn watch_search(&mut self, cx: &mut Context<Self>) {
        let executor = cx.background_executor().clone();
        self._search_waker = Some(cx.spawn(async move |this, cx| {
            loop {
                executor.timer(SEARCH_WAKE).await;
                let busy = this.update(cx, |this, cx| {
                    this.poll(cx);
                    this.search.pending() || matches!(this.request, Some(Request::Search))
                });
                if !busy.unwrap_or(false) {
                    break;
                }
            }
        }));
    }

    pub(crate) fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.search_input.read(cx).focus_handle(cx);
        if self.search_input.read(cx).text().is_empty() {
            focus.focus(window, cx);
            return;
        }
        self.search_input.update(cx, |input, cx| {
            let _ = input.set_text("", cx);
        });
        self.search_text_changed(cx);
        focus.focus(window, cx);
    }

    fn search_row(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.search_input.update(cx, |input, _| {
            input.set_placeholder(t!(Msg::SongSearchPlaceholder));
            input.set_accessible("song-search", t!(Msg::SongSearchLabel));
        });
        let has_text = !self.search_input.read(cx).text().is_empty();
        div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .px(px(PANE_PAD_X))
            .py_1()
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .key_context("SelaSongSearch")
                    .on_action(cx.listener(|this, _: &ClearSongSearch, window, cx| {
                        if this.search_input.read(cx).text().is_empty() {
                            cx.propagate();
                        } else {
                            this.clear_search(window, cx);
                        }
                    }))
                    .child(self.search_input.clone()),
            )
            .when(has_text, |d| {
                d.child(
                    div()
                        .id("song-search-clear")
                        .debug_selector(|| "song-search-clear".into())
                        .role(Role::Button)
                        .aria_label(t!(Msg::SongSearchClear))
                        .accessibility_id("song-search-clear")
                        .flex_shrink_0()
                        .w(px(24.))
                        .h(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(RADIUS))
                        .text_color(rgb(MUTED))
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(HOVER)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_, _, window, _| window.prevent_default()),
                        )
                        .on_click(cx.listener(|this, _, window, cx| this.clear_search(window, cx)))
                        .child("\u{00d7}"),
                )
            })
            .into_any_element()
    }

    pub(crate) fn song_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.library.is_none() {
            return empty_detail(t!(Msg::SongsNoLibrary), t!(Msg::SongsNoLibraryHint));
        }
        let body = if !self.songs().is_empty() {
            self.song_rows_list(cx)
        } else if self.search.active() && self.search.hits().is_some() {
            div()
                .id("song-no-results")
                .debug_selector(|| "song-no-results".into())
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .child(t!(Msg::SongSearchNoResults))
                .into_any_element()
        } else {
            empty_detail(t!(Msg::SongsEmptyTitle), t!(Msg::SongsEmptyHint))
        };
        let search = self.search_row(cx);
        let more = self.search.truncated().then(|| {
            div()
                .id("song-search-more")
                .debug_selector(|| "song-search-more".into())
                .role(Role::Status)
                .aria_label(t!(Msg::SongSearchMore))
                .accessibility_id("song-search-more")
                .flex_shrink_0()
                .px(px(PANE_PAD_X))
                .py_1()
                .border_t_1()
                .border_color(rgb(BORDER))
                .text_size(px(TEXT_CAPTION))
                .text_color(rgb(MUTED))
                .child(t!(Msg::SongSearchMore))
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().child(body))
            .children(more)
            .child(search)
            .into_any_element()
    }

    fn song_rows_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let count = self.songs().len();
        let list = uniform_list(
            "song-rows",
            count,
            cx.processor(|this, range: Range<usize>, window, cx| {
                this.song_visible = range.clone();
                this.build_song_rows(range, window, cx)
            }),
        )
        .track_scroll(&self.song_scroll)
        .flex_1()
        .min_w_0()
        .h_full();
        div()
            .id("song-list")
            .role(Role::List)
            .aria_label(t!(Msg::TabSongs))
            .accessibility_id("song-list")
            .debug_selector(|| "song-list".into())
            .size_full()
            .flex()
            .on_action(cx.listener(|this, _: &NextSongRow, _, cx| {
                this.move_song(SongStep::Next, false, cx);
            }))
            .on_action(cx.listener(|this, _: &PreviousSongRow, _, cx| {
                this.move_song(SongStep::Previous, false, cx);
            }))
            .on_action(cx.listener(|this, _: &ExtendNextSongRow, _, cx| {
                this.move_song(SongStep::Next, true, cx);
            }))
            .on_action(cx.listener(|this, _: &ExtendPreviousSongRow, _, cx| {
                this.move_song(SongStep::Previous, true, cx);
            }))
            .on_action(cx.listener(|this, _: &FirstSongRow, _, cx| {
                this.move_song(SongStep::First, false, cx);
            }))
            .on_action(cx.listener(|this, _: &LastSongRow, _, cx| {
                this.move_song(SongStep::Last, false, cx);
            }))
            .on_action(cx.listener(|this, _: &ExtendFirstSongRow, _, cx| {
                this.move_song(SongStep::First, true, cx);
            }))
            .on_action(cx.listener(|this, _: &ExtendLastSongRow, _, cx| {
                this.move_song(SongStep::Last, true, cx);
            }))
            .on_action(cx.listener(|this, _: &PageDownSongRow, _, cx| {
                this.move_song(SongStep::PageDown, false, cx);
            }))
            .on_action(cx.listener(|this, _: &PageUpSongRow, _, cx| {
                this.move_song(SongStep::PageUp, false, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectAllSongRows, _, cx| {
                this.select_all_songs(cx);
            }))
            .on_action(cx.listener(|this, _: &DeleteSongRow, window, cx| {
                if let Some(version) = this
                    .song_cursor()
                    .and_then(|i| this.songs().get(i).map(|(v, _)| *v))
                {
                    this.open_dialog(Dialog::DeleteSong(version), window, cx);
                }
            }))
            .child(list)
            .child(self.song_scrollbar(count, cx))
            .into_any_element()
    }

    /// Builds the rows of the visible range only.
    fn build_song_rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        if let Some(pending) = self.song_focus_pending
            && range.contains(&pending)
        {
            self.song_focus_pending = None;
            self.focus_song_row(pending, window, cx);
        }
        let cursor = self.song_cursor().filter(|i| range.contains(i));
        let tab_stop = cursor.unwrap_or(range.start);
        let count = self.songs().len();
        let rows: Vec<(usize, Version, String)> = self
            .songs()
            .iter()
            .enumerate()
            .skip(range.start)
            .take(range.len())
            .map(|(index, (version, title))| (index, *version, title.clone()))
            .collect();
        rows.into_iter()
            .map(|(index, version, title)| {
                let selected = self.is_song_selected(version);
                let handle = self.song_row_handle(index, cx).tab_stop(index == tab_stop);
                let drag = SongDrag {
                    version,
                    title: title.clone(),
                };
                let name = if title.trim().is_empty() {
                    t!(Msg::Untitled).to_string()
                } else {
                    title
                };
                div()
                    .id(("song", index))
                    .debug_selector(move || format!("song-{index}"))
                    .role(Role::ListItem)
                    .aria_label(name.clone())
                    .aria_selected(selected)
                    .aria_position_in_set(index)
                    .aria_size_of_set(count)
                    .accessibility_id(format!("song-{index}"))
                    .track_focus(&handle)
                    .key_context("SelaControl SelaSongRow")
                    .w_full()
                    .h(px(SONG_ROW_H))
                    .px(px(PANE_PAD_X))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .cursor_pointer()
                    .bg(rgb(if selected { SELECTED_ROW } else { SURFACE }))
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        window.prevent_default();
                        this.select_song(index, cx);
                    }))
                    .on_action(cx.listener(move |this, _: &OpenSongMenu, window, cx| {
                        this.open_song_menu(index, None, window, cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                            // The menu takes focus, not the row.
                            window.prevent_default();
                            this.open_song_menu(index, Some(event.position), window, cx);
                        }),
                    )
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_))
                                || event.is_right_click()
                            {
                                return;
                            }
                            this.focus_song_row(index, window, cx);
                            let modifiers = event.modifiers();
                            if modifiers.shift {
                                this.extend_song(index, cx);
                            } else if modifiers.secondary() {
                                this.toggle_song(index, cx);
                            } else if event.click_count() >= 2 {
                                this.select_song(index, cx);
                                this.song_to_live(index);
                                cx.notify();
                            } else {
                                this.select_song(index, cx);
                            }
                        }),
                    )
                    .on_drag(drag, |drag, _, _, cx| {
                        cx.new(|_| DragLabel(drag.title.clone().into()))
                    })
                    .child(name)
                    .into_any_element()
            })
            .collect()
    }

    /// A thumb and track, since GPUI's lists paint no scrollbar. Dragging
    /// either maps the pointer straight to a scroll position.
    fn song_scrollbar(&mut self, count: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let handle = self.song_scroll.0.borrow().base_handle.clone();
        let viewport = f32::from(handle.bounds().size.height);
        let content = count as f32 * SONG_ROW_H;
        let overflow = content - viewport;
        let thumb = (overflow > 0. && viewport > 0.).then(|| {
            let height = (viewport * viewport / content).clamp(THUMB_MIN_H.min(viewport), viewport);
            let travel = viewport - height;
            let top = (-f32::from(handle.offset().y) / overflow).clamp(0., 1.) * travel;
            (top, height)
        });
        let scroll_to = move |y: Pixels, handle: &ScrollHandle| {
            let bounds = handle.bounds();
            let viewport = f32::from(bounds.size.height);
            let overflow = count as f32 * SONG_ROW_H - viewport;
            let height = (viewport * viewport / (count as f32 * SONG_ROW_H))
                .clamp(THUMB_MIN_H.min(viewport), viewport);
            let travel = (viewport - height).max(1.);
            let ratio = ((f32::from(y - bounds.origin.y) - height / 2.) / travel).clamp(0., 1.);
            handle.set_offset(point(px(0.), px(-ratio * overflow.max(0.))));
        };
        let grabbed = self.song_scrolling;
        div()
            .id("song-scrollbar")
            .debug_selector(|| "song-scrollbar".into())
            .role(Role::ScrollBar)
            .aria_label(t!(Msg::A11ySongScrollbar))
            .accessibility_id("song-scrollbar")
            .flex_shrink_0()
            .w(px(SCROLLBAR_W))
            .h_full()
            .relative()
            .bg(rgb(CHROME))
            .when(thumb.is_none(), |d| d.invisible())
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, {
                let handle = handle.clone();
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    window.prevent_default();
                    this.song_scrolling = true;
                    scroll_to(event.position.y, &handle);
                    cx.notify();
                })
            })
            .on_drag(SongScrollGrab, |_, _, _, cx| cx.new(|_| Empty))
            .on_drag_move::<SongScrollGrab>({
                let handle = handle.clone();
                cx.listener(
                    move |this, event: &gpui::DragMoveEvent<SongScrollGrab>, _, cx| {
                        if this.song_scrolling {
                            scroll_to(event.event.position.y, &handle);
                            cx.notify();
                        }
                    },
                )
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.song_scrolling = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.song_scrolling = false;
                    cx.notify();
                }),
            )
            .children(thumb.map(|(top, height)| {
                div()
                    .absolute()
                    .left(px(2.))
                    .right(px(2.))
                    .top(px(top))
                    .h(px(height))
                    .rounded(px(RADIUS))
                    .bg(rgb(if grabbed { ACCENT } else { DISABLED }))
            }))
            .into_any_element()
    }
}
