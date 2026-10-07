use super::*;

actions!(
    operator,
    [
        OpenSongMenu,
        MenuNext,
        MenuPrevious,
        MenuConfirm,
        MenuDismiss
    ]
);

/// Songs row context menu entries, in the EW8-OBS-021 order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SongItem {
    NewSong,
    EditSong,
    Delete,
    UpdateSchedule,
    SortBy,
    Refresh,
}

pub(crate) const SONG_MENU: [SongItem; 6] = [
    SongItem::NewSong,
    SongItem::EditSong,
    SongItem::Delete,
    SongItem::UpdateSchedule,
    SongItem::SortBy,
    SongItem::Refresh,
];

impl SongItem {
    pub(crate) fn label(self) -> &'static str {
        t!(match self {
            Self::NewSong => Msg::MenuNewSong,
            Self::EditSong => Msg::MenuEditSong,
            Self::Delete => Msg::MenuDelete,
            Self::UpdateSchedule => Msg::MenuUpdateSchedule,
            Self::SortBy => Msg::MenuSortBy,
            Self::Refresh => Msg::MenuRefresh,
        })
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::NewSong => "song-menu-new",
            Self::EditSong => "song-menu-edit",
            Self::Delete => "song-menu-delete",
            Self::UpdateSchedule => "song-menu-update",
            Self::SortBy => "song-menu-sort",
            Self::Refresh => "song-menu-refresh",
        }
    }

    /// Shown but inert: what Update items in Schedule does is unobserved,
    /// and the Sort by options belong to library sorting (not built).
    pub(crate) fn enabled(self) -> bool {
        !matches!(self, Self::UpdateSchedule | Self::SortBy)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SongMenu {
    pub(crate) version: Version,
    position: Point<Pixels>,
    /// Keyboard highlight; the pointer uses hover styling only.
    pub(crate) highlight: Option<usize>,
}

impl Operator {
    /// Right-click (`position`) or Shift+F10 / Menu key (`None`) on a Songs
    /// row: selects the row like a click and opens its context menu.
    pub(crate) fn open_song_menu(
        &mut self,
        index: usize,
        position: Option<Point<Pixels>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dialog.is_some() {
            return;
        }
        let Some((version, _)) = self.catalog.get(index).cloned() else {
            return;
        };
        self.select_song(index, cx);
        let keyboard = position.is_none();
        let position = position.unwrap_or_else(|| {
            self.song_bounds
                .borrow()
                .get(index)
                .map(|row| row.bottom_left() + point(px(16.), px(0.)))
                .unwrap_or_default()
        });
        self.new_menu = false;
        self.item_menu = None;
        self.image_menu = None;
        self.song_message = None;
        self.song_menu = Some(SongMenu {
            version,
            position,
            highlight: keyboard.then_some(0),
        });
        self.song_menu_focus.focus(window, cx);
        cx.notify();
    }

    /// Closes the Songs menu; keyboard focus returns to the song's row.
    pub(crate) fn dismiss_song_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.song_menu.take() else {
            return;
        };
        if self.song_menu_focus.is_focused(window) {
            match self
                .catalog
                .iter()
                .position(|(v, _)| *v == menu.version)
                .and_then(|index| self.song_rows.get(index))
            {
                Some(row) => row.focus(window, cx),
                None => self.focus.focus(window, cx),
            }
        }
        cx.notify();
    }

    /// Up/Down (and Tab) move the highlight over enabled items, wrapping.
    pub(crate) fn move_song_highlight(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(menu) = &mut self.song_menu else {
            return;
        };
        let count = SONG_MENU.len();
        let mut next = menu.highlight;
        for _ in 0..count {
            let index = match next {
                None if forward => 0,
                None => count - 1,
                Some(index) if forward => (index + 1) % count,
                Some(index) => (index + count - 1) % count,
            };
            next = Some(index);
            if SONG_MENU[index].enabled() {
                break;
            }
        }
        menu.highlight = next;
        cx.notify();
    }

    pub(crate) fn choose_song_item(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(menu), Some(item)) = (self.song_menu, SONG_MENU.get(index).copied()) else {
            return;
        };
        if !item.enabled() {
            return;
        }
        self.dismiss_song_menu(window, cx);
        match item {
            SongItem::NewSong => self.open_editor(None, cx),
            SongItem::EditSong => self.open_editor(Some(menu.version), cx),
            SongItem::Delete => self.open_dialog(Dialog::DeleteSong(menu.version), window, cx),
            SongItem::Refresh => self.catalog_stale = true,
            SongItem::UpdateSchedule | SongItem::SortBy => {}
        }
        cx.notify();
    }

    /// The Songs row context menu, drawn above the panes and kept inside the
    /// window (`deferred` + `anchored`, as in GPUI's popover example).
    pub(crate) fn song_menu_overlay(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let menu = self
            .song_menu
            .filter(|_| self.tab == 0 && !self.collapsed)?;
        let items = SONG_MENU
            .iter()
            .enumerate()
            .map(|(index, &item)| {
                let enabled = item.enabled();
                div()
                    .id(item.id())
                    .debug_selector(move || item.id().into())
                    .h(px(CONTROL_H))
                    .mx_1()
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded(px(RADIUS))
                    .text_color(rgb(if enabled { TEXT } else { DISABLED }))
                    .when(enabled, |d| {
                        d.cursor_pointer()
                            .hover(|d| d.bg(rgb(HOVER)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.choose_song_item(index, window, cx);
                            }))
                    })
                    .when(menu.highlight == Some(index), |d| {
                        d.bg(rgb(FOCUS_BG)).text_color(rgb(FOCUS_TEXT))
                    })
                    .child(item.label())
            })
            .collect::<Vec<_>>();
        Some(
            deferred(
                anchored()
                    .position(menu.position)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .id("song-menu")
                            .debug_selector(|| "song-menu".into())
                            .track_focus(&self.song_menu_focus)
                            .key_context("SelaSongMenu")
                            .occlude()
                            .min_w(px(220.))
                            .py_1()
                            .flex()
                            .flex_col()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(SURFACE))
                            .shadow_md()
                            .text_size(px(TEXT_BASE))
                            .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                                this.dismiss_song_menu(window, cx);
                            }))
                            .on_action(cx.listener(|this, _: &MenuNext, _, cx| {
                                this.move_song_highlight(true, cx);
                            }))
                            .on_action(cx.listener(|this, _: &MenuPrevious, _, cx| {
                                this.move_song_highlight(false, cx);
                            }))
                            // Tab stays inside the open menu.
                            .on_action(cx.listener(|this, _: &FocusNext, _, cx| {
                                this.move_song_highlight(true, cx);
                            }))
                            .on_action(cx.listener(|this, _: &FocusPrevious, _, cx| {
                                this.move_song_highlight(false, cx);
                            }))
                            .on_action(cx.listener(|this, _: &MenuConfirm, window, cx| {
                                if let Some(index) = this.song_menu.and_then(|m| m.highlight) {
                                    this.choose_song_item(index, window, cx);
                                }
                            }))
                            .on_action(cx.listener(|this, _: &MenuDismiss, window, cx| {
                                this.dismiss_song_menu(window, cx);
                            }))
                            .children(items),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}
