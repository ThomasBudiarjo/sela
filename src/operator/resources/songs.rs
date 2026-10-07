use super::*;

impl Operator {
    /// Opens a Song Editor window: a new song, or the given saved revision.
    pub(crate) fn open_editor(&mut self, version: Option<Version>, cx: &mut Context<Self>) {
        self.library_error = self
            .library
            .clone()
            .or_else(crate::song_library::default_path)
            .ok_or_else(|| {
                "No user data directory. Launch with --library DATABASE_PATH.".to_string()
            })
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
            ("No song library open".into(), true)
        } else if self.queued.is_some() || matches!(self.request, Some(Request::DeleteSong(..))) {
            (
                "Not deleted: wait for the library to finish the previous change".into(),
                true,
            )
        } else {
            self.queued = Some((
                storage::Command::DeleteSong(version),
                Request::DeleteSong(version, title),
            ));
            ("Deleting…".into(), false)
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
            return empty_detail(
                "Songs · no library open",
                "Launch Sela without arguments to use the default library.",
            );
        }
        if self.catalog.is_empty() {
            return empty_detail(
                "Songs · offline library",
                "No songs yet. Use + New Song to create one.",
            );
        }
        let bounds = self.song_bounds.clone();
        div()
            .on_children_prepainted(move |rows, _, _| *bounds.borrow_mut() = rows)
            .id("song-list")
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
                        div()
                            .id(("song", index))
                            .debug_selector(move || format!("song-{index}"))
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
                            .child(if title.trim().is_empty() {
                                "Untitled".to_string()
                            } else {
                                title.clone()
                            })
                    }),
            )
            .into_any_element()
    }
}
