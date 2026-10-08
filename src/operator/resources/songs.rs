use super::*;

actions!(operator, [NextSongRow, PreviousSongRow]);

impl Operator {
    /// Down/Up with a Songs row focused: move the song selection (and its
    /// preview), stopping at the ends. Provisional Sela keyboard policy
    /// (EasyWorship unobserved); before M1-16 these keys moved the schedule
    /// selection even with a song row focused (VAL-A11Y-005).
    pub(crate) fn step_song(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.catalog.is_empty() {
            return;
        }
        let current = self
            .selected_song
            .filter(|_| self.selected_entry.is_none())
            .and_then(|v| self.catalog.iter().position(|(c, _)| *c == v));
        let next = match current {
            Some(index) if forward => (index + 1).min(self.catalog.len() - 1),
            Some(index) => index.saturating_sub(1),
            None if forward => 0,
            None => self.catalog.len() - 1,
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
        if let Some((version, _)) = self.catalog.get(index) {
            self.selected_song = Some(*version);
            self.selected_entry = None;
            self.live_on_load = None;
            cx.notify();
        }
    }

    pub(crate) fn song_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        while self.song_rows.len() < self.catalog.len() {
            let index = self.song_rows.len() as isize;
            self.song_rows.push(
                cx.focus_handle()
                    .tab_index(SONG_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        if self.library.is_none() {
            return empty_detail(t!(Msg::SongsNoLibrary), t!(Msg::SongsNoLibraryHint));
        }
        if self.catalog.is_empty() {
            return empty_detail(t!(Msg::SongsEmptyTitle), t!(Msg::SongsEmptyHint));
        }
        let bounds = self.song_bounds.clone();
        let count = self.catalog.len();
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
            .children(
                self.catalog
                    .iter()
                    .enumerate()
                    .map(|(index, (version, title))| {
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
                            .px_3()
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
                                cx.listener(
                                    move |this, event: &gpui::MouseDownEvent, window, cx| {
                                        // The menu takes focus, not the row.
                                        window.prevent_default();
                                        this.open_song_menu(
                                            index,
                                            Some(event.position),
                                            window,
                                            cx,
                                        );
                                    },
                                ),
                            )
                            .on_click(cx.listener(
                                move |this, event: &gpui::ClickEvent, window, cx| {
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
                                },
                            ))
                            .on_drag(drag, |drag, _, _, cx| {
                                cx.new(|_| DragLabel(drag.title.clone().into()))
                            })
                            .child(name)
                    }),
            )
            .into_any_element()
    }
}
