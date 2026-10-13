use super::*;

actions!(operator, [NextSongRow, PreviousSongRow, ClearSongSearch]);

/// After the song rows (100+) and before the schedule (500), matching its
/// place under the list; adds no stop before the first song row.
pub(crate) const SEARCH_TAB_INDEX: isize = 499;

impl Operator {
    /// Down/Up with a Songs row focused: move the song selection (and its
    /// preview), stopping at the ends. Provisional Sela keyboard policy
    /// (EasyWorship unobserved); before M1-16 these keys moved the schedule
    /// selection even with a song row focused (VAL-A11Y-005).
    pub(crate) fn step_song(&mut self, forward: bool, cx: &mut Context<Self>) {
        let songs = self.songs();
        if songs.is_empty() {
            return;
        }
        let current = self
            .selected_song
            .filter(|_| self.selected_entry.is_none())
            .and_then(|v| songs.iter().position(|(c, _)| *c == v));
        let next = match current {
            Some(index) if forward => (index + 1).min(songs.len() - 1),
            Some(index) => index.saturating_sub(1),
            None if forward => 0,
            None => songs.len() - 1,
        };
        self.select_song(next, cx);
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

    pub(crate) fn select_song(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some((version, _)) = self.songs().get(index) {
            self.selected_song = Some(*version);
            self.selected_entry = None;
            self.live_on_load = None;
            cx.notify();
        }
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
            cx.notify();
        }
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
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().child(body))
            .child(search)
            .into_any_element()
    }

    fn song_rows_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        while self.song_rows.len() < self.songs().len() {
            let index = self.song_rows.len() as isize;
            self.song_rows.push(
                cx.focus_handle()
                    .tab_index(SONG_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        let bounds = self.song_bounds.clone();
        let songs = self.search.hits().unwrap_or(&self.catalog);
        let count = songs.len();
        div()
            .on_children_prepainted(move |rows, _, _| *bounds.borrow_mut() = rows)
            .id("song-list")
            .role(Role::List)
            .aria_label(t!(Msg::TabSongs))
            .accessibility_id("song-list")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(songs.iter().enumerate().map(|(index, (version, title))| {
                let selected =
                    self.selected_song == Some(*version) && self.selected_entry.is_none();
                let drag = SongDrag {
                    version: *version,
                    title: title.clone(),
                };
                let name = if title.trim().is_empty() {
                    t!(Msg::Untitled).to_string()
                } else {
                    title.clone()
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
                    .track_focus(&self.song_rows[index])
                    .key_context("SelaControl SelaSongRow")
                    .flex_shrink_0()
                    .h(px(28.))
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
                            this.song_rows[index].focus(window, cx);
                            if event.click_count() >= 2 {
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
            }))
            .into_any_element()
    }
}
