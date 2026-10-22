use super::*;

actions!(
    operator,
    [
        OpenSongMenu,
        MenuNext,
        MenuPrevious,
        MenuConfirm,
        MenuDismiss,
        OpenFileMenu,
        OpenEditMenu,
        OpenLiveMenu,
        OpenProfilesMenu,
        OpenViewMenu,
        OpenHelpMenu,
        BarMenuLeft,
        BarMenuRight,
        MenuOpenSub,
        MenuCloseSub
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

    /// Update items in Schedule needs a schedule with items (EW8-OBS-061).
    pub(crate) fn enabled(self, schedule_items: usize) -> bool {
        match self {
            Self::UpdateSchedule => schedule_items > 0,
            _ => true,
        }
    }
}

/// One line of the open Songs menu. The Sort by choices follow their item
/// in place, because a flyout cannot be positioned reliably at a window edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SongLine {
    Item(SongItem),
    Sort(SongSort),
    Descending(bool),
}

pub(crate) fn song_menu_lines(sort_open: bool) -> Vec<SongLine> {
    let mut lines = Vec::new();
    for item in SONG_MENU {
        lines.push(SongLine::Item(item));
        if item == SongItem::SortBy && sort_open {
            lines.extend(SongSort::ALL.map(SongLine::Sort));
            lines.extend([SongLine::Descending(false), SongLine::Descending(true)]);
        }
    }
    lines
}

impl SongLine {
    fn label(self) -> &'static str {
        match self {
            Self::Item(item) => item.label(),
            Self::Sort(SongSort::Title) => t!(Msg::MenuSortTitle),
            Self::Sort(SongSort::Modified) => t!(Msg::MenuSortModified),
            Self::Descending(false) => t!(Msg::MenuSortAscending),
            Self::Descending(true) => t!(Msg::MenuSortDescending),
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Item(item) => item.id(),
            Self::Sort(SongSort::Title) => "song-sort-title",
            Self::Sort(SongSort::Modified) => "song-sort-modified",
            Self::Descending(false) => "song-sort-ascending",
            Self::Descending(true) => "song-sort-descending",
        }
    }

    fn enabled(self, schedule_items: usize) -> bool {
        match self {
            Self::Item(item) => item.enabled(schedule_items),
            _ => true,
        }
    }

    fn checked(self, order: SongOrder) -> bool {
        match self {
            Self::Item(_) => false,
            Self::Sort(sort) => order.sort == sort,
            Self::Descending(descending) => order.descending == descending,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SongMenu {
    pub(crate) version: Version,
    position: Point<Pixels>,
    /// Keyboard highlight; the pointer uses hover styling only.
    pub(crate) highlight: Option<usize>,
    /// The Sort by choices are shown.
    pub(crate) sort_open: bool,
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
        let Some((version, _)) = self.songs().get(index).cloned() else {
            return;
        };
        // A right-click inside a multi-selection keeps it.
        if !(position.is_some() && self.is_song_selected(version)) {
            self.select_song(index, cx);
        }
        let keyboard = position.is_none();
        let position = position.unwrap_or_else(|| {
            let state = self.song_scroll.0.borrow();
            let bounds = state.base_handle.bounds();
            let top = bounds.origin.y + state.base_handle.offset().y;
            let row_bottom = top + px((index + 1) as f32 * super::resources::SONG_ROW_H);
            point(
                bounds.origin.x + px(16.),
                row_bottom.clamp(bounds.origin.y, bounds.bottom_left().y),
            )
        });
        self.new_menu = false;
        self.item_menu = None;
        self.image_menu = None;
        self.bar_menu = None;
        self.song_message = None;
        self.song_menu = Some(SongMenu {
            version,
            position,
            highlight: keyboard.then_some(0),
            sort_open: false,
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
            match self.song_index_of(menu.version) {
                Some(index) => {
                    self.song_scroll
                        .scroll_to_item(index, ScrollStrategy::Nearest);
                    self.focus_song_row(index, window, cx);
                }
                None => self.focus.focus(window, cx),
            }
        }
        cx.notify();
    }

    /// Up/Down (and Tab) move the highlight over enabled items, wrapping.
    pub(crate) fn move_song_highlight(&mut self, forward: bool, cx: &mut Context<Self>) {
        let items = self.schedule.entries().len();
        let Some(menu) = &mut self.song_menu else {
            return;
        };
        let lines = song_menu_lines(menu.sort_open);
        let count = lines.len();
        let mut next = menu.highlight;
        for _ in 0..count {
            let index = match next {
                None if forward => 0,
                None => count - 1,
                Some(index) if forward => (index + 1) % count,
                Some(index) => (index + count - 1) % count,
            };
            next = Some(index);
            if lines[index].enabled(items) {
                break;
            }
        }
        menu.highlight = next;
        cx.notify();
    }

    /// Right opens the Sort by choices from their item; Left closes them.
    pub(crate) fn set_song_sort_open(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(menu) = &mut self.song_menu else {
            return;
        };
        let sort_at = SONG_MENU.iter().position(|i| *i == SongItem::SortBy);
        if open && menu.highlight == sort_at {
            menu.sort_open = true;
            menu.highlight = sort_at.map(|i| i + 1);
        } else if !open && menu.sort_open {
            menu.sort_open = false;
            menu.highlight = sort_at;
        }
        cx.notify();
    }

    pub(crate) fn choose_song_item(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.song_menu else {
            return;
        };
        let Some(line) = song_menu_lines(menu.sort_open).get(index).copied() else {
            return;
        };
        if !line.enabled(self.schedule.entries().len()) {
            return;
        }
        let item = match line {
            SongLine::Item(SongItem::SortBy) => {
                if let Some(menu) = &mut self.song_menu {
                    menu.sort_open = !menu.sort_open;
                    menu.highlight = Some(index);
                }
                cx.notify();
                return;
            }
            SongLine::Item(item) => item,
            SongLine::Sort(sort) => {
                self.set_song_order(
                    SongOrder {
                        sort,
                        ..self.song_order
                    },
                    window,
                    cx,
                );
                return;
            }
            SongLine::Descending(descending) => {
                self.set_song_order(
                    SongOrder {
                        descending,
                        ..self.song_order
                    },
                    window,
                    cx,
                );
                return;
            }
        };
        self.dismiss_song_menu(window, cx);
        match item {
            SongItem::NewSong => self.open_editor(None, cx),
            SongItem::EditSong => self.open_editor(Some(menu.version), cx),
            SongItem::Delete => self.open_dialog(Dialog::DeleteSong(menu.version), window, cx),
            SongItem::Refresh => self.catalog_stale = true,
            SongItem::UpdateSchedule => self.begin_update_scan(),
            SongItem::SortBy => {}
        }
        cx.notify();
    }

    /// Applies and remembers a new Songs order; the list reloads from the
    /// storage worker.
    fn set_song_order(&mut self, order: SongOrder, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_song_menu(window, cx);
        if order != self.song_order {
            self.song_order = order;
            self.update_recent(RecentChange::SongOrder(order));
            self.catalog_stale = true;
            self.song_scroll.scroll_to_item(0, ScrollStrategy::Top);
        }
        cx.notify();
    }

    /// The Songs row context menu, drawn above the panes and kept inside the
    /// window (`deferred` + `anchored`, as in GPUI's popover example).
    pub(crate) fn song_menu_overlay(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let menu = self
            .song_menu
            .filter(|_| self.tab == 0 && !self.collapsed)?;
        let schedule_items = self.schedule.entries().len();
        let order = self.song_order;
        let items = song_menu_lines(menu.sort_open)
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let enabled = item.enabled(schedule_items);
                let checked = item.checked(order);
                let nested = !matches!(item, SongLine::Item(_));
                div()
                    .id(item.id())
                    .debug_selector(move || item.id().into())
                    .role(Role::MenuItem)
                    .aria_label(item.label())
                    .accessibility_id(item.id())
                    .when(!enabled, mark_disabled)
                    .when(nested, |d| {
                        d.role(Role::MenuItemCheckBox).aria_toggled(if checked {
                            Toggled::True
                        } else {
                            Toggled::False
                        })
                    })
                    .h(px(CONTROL_H))
                    .mx(px(MENU_ITEM_MARGIN_X))
                    .px(px(MENU_ITEM_PAD_X))
                    .when(nested, |d| d.pl(px(MENU_ITEM_PAD_X + 16.)))
                    .flex()
                    .items_center()
                    .justify_between()
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
                    .when(checked, |d| d.child("\u{2713}"))
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
                            .role(Role::Menu)
                            .aria_label(t!(Msg::A11ySongMenu))
                            .accessibility_id("song-menu")
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
                            .on_action(cx.listener(|this, _: &MenuOpenSub, _, cx| {
                                this.set_song_sort_open(true, cx);
                            }))
                            .on_action(cx.listener(|this, _: &MenuCloseSub, _, cx| {
                                this.set_song_sort_open(false, cx);
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

// ---------------------------------------------------------------------------
// Main menu bar (M1-02/M1-03): File, Edit, Live, Profiles, View, Help in the
// observed EW 8.0.49 order (EW8-OBS-010/053). GPUI app menus are macOS-only
// (zed rev a846890 gpui/src/app.rs set_menus), so this is an in-window menu
// bar with deferred+anchored popups like crates/gpui/examples/popover.rs.

/// Top-level menus in the observed EW 8.0.49 order (EW8-OBS-010).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TopMenu {
    File,
    Edit,
    Live,
    Profiles,
    View,
    Help,
}

pub(crate) const TOP_MENUS: [TopMenu; 6] = [
    TopMenu::File,
    TopMenu::Edit,
    TopMenu::Live,
    TopMenu::Profiles,
    TopMenu::View,
    TopMenu::Help,
];

impl TopMenu {
    fn label(self) -> &'static str {
        t!(match self {
            TopMenu::File => Msg::BarFile,
            TopMenu::Edit => Msg::BarEdit,
            TopMenu::Live => Msg::BarLive,
            TopMenu::Profiles => Msg::BarProfiles,
            TopMenu::View => Msg::BarView,
            TopMenu::Help => Msg::BarHelp,
        })
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            TopMenu::File => "bar-file",
            TopMenu::Edit => "bar-edit",
            TopMenu::Live => "bar-live",
            TopMenu::Profiles => "bar-profiles",
            TopMenu::View => "bar-view",
            TopMenu::Help => "bar-help",
        }
    }

    /// Menu rows; File grows one row per recent schedule file.
    fn entries(self, operator: &Operator) -> Vec<BarEntry> {
        match self {
            TopMenu::File => {
                let mut rows = Vec::with_capacity(FILE_MENU.len() + sela::settings::MAX_RECENT);
                for entry in FILE_MENU {
                    rows.push(*entry);
                    if *entry == BarEntry::Item(BarItem::OpenRecent) {
                        rows.extend(
                            (0..operator.recent_schedules().len())
                                .map(|index| BarEntry::Item(BarItem::Recent(index as u8))),
                        );
                    }
                }
                rows
            }
            TopMenu::Edit => EDIT_MENU.to_vec(),
            TopMenu::Live => LIVE_MENU.to_vec(),
            TopMenu::Profiles => PROFILES_MENU.to_vec(),
            TopMenu::View => VIEW_MENU.to_vec(),
            TopMenu::Help => HELP_MENU.to_vec(),
        }
    }
}

const RECENT_IDS: [&str; sela::settings::MAX_RECENT] = [
    "bar-file-recent-0",
    "bar-file-recent-1",
    "bar-file-recent-2",
    "bar-file-recent-3",
    "bar-file-recent-4",
    "bar-file-recent-5",
    "bar-file-recent-6",
    "bar-file-recent-7",
    "bar-file-recent-8",
    "bar-file-recent-9",
];

/// A menu row, or a separator between groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BarEntry {
    Separator,
    Item(BarItem),
}

/// Menu items across all six menus. The File › New ▸ submenu is flattened
/// into the popup as an inert group row followed by Song.../Schedule because
/// the popover only supports one level; later milestones may nest it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BarItem {
    NewHeader,
    NewSong,
    NewScheduleItem,
    OpenSchedule,
    OpenRecent,
    /// One Open Recent entry, flattened under its header like File › New.
    Recent(u8),
    SaveSchedule,
    SaveScheduleAs,
    ExportStandalone,
    CloseSchedule,
    ReportsPrinting,
    Exit,
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,
    Options,
    GoLive,
    GoBack,
    LogoOnLive,
    BlackOnLive,
    ClearOnLive,
    ShowLiveOutput,
    /// Sela addition (UNOBSERVED in EW8-OBS-053): restarts or stops only the
    /// stage output.
    ShowStageOutput,
    /// Sela addition (UNOBSERVED): the stage-only message line.
    StageMessage,
    ProfileDefault,
    ProfilesManager,
    DemoData,
    Utilities,
    ViewPreviewLive,
    ViewCombined,
    ViewResourcePreview,
    InterfaceColor,
    HelpContents,
    GettingStarted,
    TroubleshootingLogs,
    CheckUpdates,
    BackgroundHelper,
    About,
}

impl BarItem {
    fn label(self) -> &'static str {
        t!(match self {
            BarItem::NewHeader => Msg::BarNew,
            BarItem::NewSong => Msg::BarNewSong,
            BarItem::NewScheduleItem => Msg::BarNewSchedule,
            BarItem::OpenSchedule => Msg::BarOpenSchedule,
            BarItem::OpenRecent | BarItem::Recent(_) => Msg::BarOpenRecent,
            BarItem::SaveSchedule => Msg::BarSaveSchedule,
            BarItem::SaveScheduleAs => Msg::BarSaveScheduleAs,
            BarItem::ExportStandalone => Msg::BarExportStandalone,
            BarItem::CloseSchedule => Msg::BarCloseSchedule,
            BarItem::ReportsPrinting => Msg::BarReportsPrinting,
            BarItem::Exit => Msg::BarExit,
            BarItem::Cut => Msg::BarCut,
            BarItem::Copy => Msg::BarCopy,
            BarItem::Paste => Msg::BarPaste,
            BarItem::Delete => Msg::BarDelete,
            BarItem::SelectAll => Msg::BarSelectAll,
            BarItem::Options => Msg::BarOptions,
            BarItem::GoLive => Msg::BarGoLive,
            BarItem::GoBack => Msg::BarGoBack,
            BarItem::LogoOnLive => Msg::BarLogoOnLive,
            BarItem::BlackOnLive => Msg::BarBlackOnLive,
            BarItem::ClearOnLive => Msg::BarClearOnLive,
            BarItem::ShowLiveOutput => Msg::BarShowLiveOutput,
            BarItem::ShowStageOutput => Msg::BarShowStageOutput,
            BarItem::StageMessage => Msg::BarStageMessage,
            BarItem::ProfileDefault => Msg::BarDefault,
            BarItem::ProfilesManager => Msg::BarProfilesManager,
            BarItem::DemoData => Msg::BarDemoData,
            BarItem::Utilities => Msg::BarUtilities,
            BarItem::ViewPreviewLive => Msg::BarPreviewLive,
            BarItem::ViewCombined => Msg::BarLiveCombined,
            BarItem::ViewResourcePreview => Msg::BarLiveResourcePreview,
            BarItem::InterfaceColor => Msg::BarInterfaceColor,
            BarItem::HelpContents => Msg::BarHelpContents,
            BarItem::GettingStarted => Msg::BarGettingStarted,
            BarItem::TroubleshootingLogs => Msg::BarTroubleshootingLogs,
            BarItem::CheckUpdates => Msg::BarCheckUpdates,
            BarItem::BackgroundHelper => Msg::BarBackgroundHelper,
            BarItem::About => Msg::BarAbout,
        })
    }

    /// Right-aligned accelerator column, matching EW8-OBS-053.
    fn shortcut(self) -> Option<&'static str> {
        match self {
            BarItem::OpenSchedule => Some("Ctrl+O"),
            BarItem::SaveSchedule => Some("Ctrl+S"),
            BarItem::Cut => Some("Ctrl+X"),
            BarItem::Copy => Some("Ctrl+C"),
            BarItem::Paste => Some("Ctrl+V"),
            BarItem::Delete => Some("Ctrl+Del"),
            BarItem::SelectAll => Some("Ctrl+A"),
            _ => None,
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            BarItem::NewHeader => "bar-file-new",
            BarItem::NewSong => "bar-file-new-song",
            BarItem::NewScheduleItem => "bar-file-new-schedule",
            BarItem::OpenSchedule => "bar-file-open",
            BarItem::OpenRecent => "bar-file-open-recent",
            BarItem::Recent(index) => RECENT_IDS[index as usize % RECENT_IDS.len()],
            BarItem::SaveSchedule => "bar-file-save",
            BarItem::SaveScheduleAs => "bar-file-save-as",
            BarItem::ExportStandalone => "bar-file-export",
            BarItem::CloseSchedule => "bar-file-close",
            BarItem::ReportsPrinting => "bar-file-reports",
            BarItem::Exit => "bar-file-exit",
            BarItem::Cut => "bar-edit-cut",
            BarItem::Copy => "bar-edit-copy",
            BarItem::Paste => "bar-edit-paste",
            BarItem::Delete => "bar-edit-delete",
            BarItem::SelectAll => "bar-edit-select-all",
            BarItem::Options => "bar-edit-options",
            BarItem::GoLive => "bar-live-go-live",
            BarItem::GoBack => "bar-live-go-back",
            BarItem::LogoOnLive => "bar-live-logo",
            BarItem::BlackOnLive => "bar-live-black",
            BarItem::ClearOnLive => "bar-live-clear",
            BarItem::ShowLiveOutput => "bar-live-show",
            BarItem::ShowStageOutput => "bar-live-stage",
            BarItem::StageMessage => "bar-live-stage-message",
            BarItem::ProfileDefault => "bar-profiles-default",
            BarItem::ProfilesManager => "bar-profiles-manager",
            BarItem::DemoData => "bar-profiles-demo-data",
            BarItem::Utilities => "bar-profiles-utilities",
            BarItem::ViewPreviewLive => "bar-view-preview-live",
            BarItem::ViewCombined => "bar-view-combined",
            BarItem::ViewResourcePreview => "bar-view-resource",
            BarItem::InterfaceColor => "bar-view-interface-color",
            BarItem::HelpContents => "bar-help-contents",
            BarItem::GettingStarted => "bar-help-getting-started",
            BarItem::TroubleshootingLogs => "bar-help-logs",
            BarItem::CheckUpdates => "bar-help-updates",
            BarItem::BackgroundHelper => "bar-help-helper",
            BarItem::About => "bar-help-about",
        }
    }

    /// Only items whose feature exists are enabled (M1-02 scope); EW's
    /// unbuilt rows stay visible but inert so later tickets flip them on.
    fn enabled(self, operator: &Operator) -> bool {
        match self {
            BarItem::NewSong
            | BarItem::NewScheduleItem
            | BarItem::OpenSchedule
            | BarItem::SaveSchedule
            | BarItem::SaveScheduleAs
            | BarItem::CloseSchedule
            | BarItem::Exit
            | BarItem::Options
            | BarItem::GoLive
            | BarItem::BlackOnLive
            | BarItem::ClearOnLive
            | BarItem::ShowLiveOutput => true,
            // Mirrors the toolbar Logo button: usable only when a logo is
            // chosen or already showing.
            BarItem::LogoOnLive => !(operator.logo.is_none() && !operator.masks.is_on(Mask::Logo)),
            // Foldback must be enabled in Options (EW8-OBS-056) and a stage
            // child must exist; the message can be prepared before Live.
            BarItem::ShowStageOutput => {
                operator.stage_enabled() && operator.stage_available()
                    || operator.stage.output.is_some()
            }
            BarItem::StageMessage => operator.stage_enabled() || operator.stage.output.is_some(),
            BarItem::OpenRecent => !operator.recent_schedules().is_empty(),
            BarItem::Recent(index) => (index as usize) < operator.recent_schedules().len(),
            _ => false,
        }
    }

    fn checked(self, operator: &Operator) -> bool {
        match self {
            BarItem::ShowLiveOutput => operator.output.is_some(),
            BarItem::ShowStageOutput => operator.stage.output.is_some(),
            BarItem::ProfileDefault | BarItem::ViewPreviewLive => true,
            _ => false,
        }
    }

    /// Items EW draws with a checkmark column; exposed as checkable menu
    /// items to assistive technology.
    fn checkable(self) -> bool {
        matches!(
            self,
            BarItem::ShowLiveOutput
                | BarItem::ShowStageOutput
                | BarItem::ProfileDefault
                | BarItem::ViewPreviewLive
                | BarItem::ViewCombined
                | BarItem::ViewResourcePreview
        )
    }

    /// Group rows that only head the flattened rows below them.
    fn is_header(self) -> bool {
        matches!(self, BarItem::NewHeader | BarItem::OpenRecent)
    }

    /// Rows the keyboard highlight can land on and Enter can run.
    fn selectable(self, operator: &Operator) -> bool {
        self.enabled(operator) && !self.is_header()
    }
}

// EW8-OBS-053 (File/Edit/Live) and the menu-p/v/h.png captures; separators
// are read from the menuF-full.png capture (EW shows none in the ledger text).
const FILE_MENU: &[BarEntry] = &[
    BarEntry::Item(BarItem::NewHeader),
    BarEntry::Item(BarItem::NewSong),
    BarEntry::Item(BarItem::NewScheduleItem),
    BarEntry::Separator,
    BarEntry::Item(BarItem::OpenSchedule),
    BarEntry::Item(BarItem::OpenRecent),
    BarEntry::Item(BarItem::SaveSchedule),
    BarEntry::Item(BarItem::SaveScheduleAs),
    BarEntry::Item(BarItem::ExportStandalone),
    BarEntry::Item(BarItem::CloseSchedule),
    BarEntry::Separator,
    BarEntry::Item(BarItem::ReportsPrinting),
    BarEntry::Separator,
    BarEntry::Item(BarItem::Exit),
];
const EDIT_MENU: &[BarEntry] = &[
    BarEntry::Item(BarItem::Cut),
    BarEntry::Item(BarItem::Copy),
    BarEntry::Item(BarItem::Paste),
    BarEntry::Item(BarItem::Delete),
    BarEntry::Item(BarItem::SelectAll),
    BarEntry::Separator,
    BarEntry::Item(BarItem::Options),
];
const LIVE_MENU: &[BarEntry] = &[
    BarEntry::Item(BarItem::GoLive),
    BarEntry::Item(BarItem::GoBack),
    BarEntry::Separator,
    BarEntry::Item(BarItem::LogoOnLive),
    BarEntry::Item(BarItem::BlackOnLive),
    BarEntry::Item(BarItem::ClearOnLive),
    BarEntry::Separator,
    BarEntry::Item(BarItem::ShowLiveOutput),
    BarEntry::Item(BarItem::ShowStageOutput),
    BarEntry::Item(BarItem::StageMessage),
];
const PROFILES_MENU: &[BarEntry] = &[
    BarEntry::Item(BarItem::ProfileDefault),
    BarEntry::Item(BarItem::ProfilesManager),
    BarEntry::Separator,
    BarEntry::Item(BarItem::DemoData),
    BarEntry::Separator,
    BarEntry::Item(BarItem::Utilities),
];
const VIEW_MENU: &[BarEntry] = &[
    BarEntry::Item(BarItem::ViewPreviewLive),
    BarEntry::Item(BarItem::ViewCombined),
    BarEntry::Item(BarItem::ViewResourcePreview),
    BarEntry::Separator,
    BarEntry::Item(BarItem::InterfaceColor),
];
const HELP_MENU: &[BarEntry] = &[
    BarEntry::Item(BarItem::HelpContents),
    BarEntry::Item(BarItem::GettingStarted),
    BarEntry::Separator,
    BarEntry::Item(BarItem::TroubleshootingLogs),
    BarEntry::Item(BarItem::CheckUpdates),
    BarEntry::Separator,
    BarEntry::Item(BarItem::BackgroundHelper),
    BarEntry::Separator,
    BarEntry::Item(BarItem::About),
];

/// State of the one open menu-bar popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BarMenu {
    pub(crate) top: TopMenu,
    /// Keyboard highlight into `top.entries()`; `None` for a plain mouse open.
    pub(crate) highlight: Option<usize>,
}

impl Operator {
    /// First enabled (selectable) entry index of a menu, if any.
    fn first_enabled(&self, top: TopMenu) -> Option<usize> {
        top.entries(self)
            .iter()
            .position(|e| matches!(e, BarEntry::Item(item) if item.selectable(self)))
    }

    /// Alt+mnemonic (`keyboard: true`) or label click. Keyboard opens
    /// highlight the first selectable row like EW's underlined mnemonics;
    /// clicking the open menu's label closes it again.
    pub(crate) fn toggle_bar_menu(
        &mut self,
        top: TopMenu,
        keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.bar_menu.is_some_and(|menu| menu.top == top) {
            self.dismiss_bar_menu(window, cx);
            return;
        }
        self.open_bar_menu(top, window, cx);
        if keyboard {
            let first = self.first_enabled(top);
            if let Some(menu) = self.bar_menu.as_mut() {
                menu.highlight = first;
            }
            cx.notify();
        }
    }

    pub(crate) fn open_bar_menu(
        &mut self,
        top: TopMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dialog.is_some() {
            return;
        }
        if self.bar_menu.is_none() {
            self.bar_return = window.focused(cx);
        }
        self.new_menu = false;
        self.item_menu = None;
        self.image_menu = None;
        self.song_menu = None;
        self.bar_menu = Some(BarMenu {
            top,
            highlight: None,
        });
        self.bar_menu_focus.focus(window, cx);
        cx.notify();
    }

    /// Close the popup and return focus to whatever was focused before it
    /// opened (Esc/selection/dismiss all share this path).
    pub(crate) fn dismiss_bar_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.bar_menu.take().is_none() {
            return;
        }
        self.bar_opened_with_mouse = None;
        match self.bar_return.take() {
            Some(handle) => handle.focus(window, cx),
            None => self.focus.focus(window, cx),
        }
        cx.notify();
    }

    /// Up/Down move the keyboard highlight, wrapping and skipping separators,
    /// disabled rows, and the inert New group header.
    pub(crate) fn move_bar_highlight(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(menu) = self.bar_menu else {
            return;
        };
        let entries = menu.top.entries(self);
        let selectable = |index: usize| matches!(entries.get(index), Some(BarEntry::Item(item)) if item.selectable(self));
        if !(0..entries.len()).any(&selectable) {
            return;
        }
        let mut next = menu.highlight;
        for _ in 0..entries.len() {
            let index = match next {
                None if forward => 0,
                None => entries.len() - 1,
                Some(i) if forward => (i + 1) % entries.len(),
                Some(i) => (i + entries.len() - 1) % entries.len(),
            };
            next = Some(index);
            if selectable(index) {
                break;
            }
        }
        if let Some(menu) = self.bar_menu.as_mut() {
            menu.highlight = next;
        }
        cx.notify();
    }

    /// Right moves to the next menu, Left to the previous one, wrapping;
    /// both highlight the next menu's first selectable row (keyboard flow).
    pub(crate) fn move_bar_menu(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.bar_menu else {
            return;
        };
        let index = TOP_MENUS.iter().position(|t| *t == menu.top).unwrap_or(0);
        let next = if forward {
            TOP_MENUS[(index + 1) % TOP_MENUS.len()]
        } else {
            TOP_MENUS[(index + TOP_MENUS.len() - 1) % TOP_MENUS.len()]
        };
        self.open_bar_menu(next, window, cx);
        let first = self.first_enabled(next);
        if let Some(menu) = self.bar_menu.as_mut() {
            menu.highlight = first;
        }
        cx.notify();
    }

    /// Enter/Space activates the highlighted row.
    pub(crate) fn confirm_bar_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.bar_menu.and_then(|menu| menu.highlight) else {
            return;
        };
        let Some(menu) = self.bar_menu else {
            return;
        };
        let Some(BarEntry::Item(item)) = menu.top.entries(self).get(index).copied() else {
            return;
        };
        self.choose_bar_item(item, window, cx);
    }

    /// Dispatch the same handler the toolbar/keyboard equivalent uses.
    pub(crate) fn choose_bar_item(
        &mut self,
        item: BarItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !item.selectable(self) {
            return;
        }
        self.dismiss_bar_menu(window, cx);
        match item {
            BarItem::NewSong => self.open_editor(None, cx),
            BarItem::NewScheduleItem => self.guard(Then::New, window, cx),
            BarItem::OpenSchedule => self.open_schedule(window, cx),
            BarItem::SaveSchedule => self.save_schedule(cx),
            BarItem::SaveScheduleAs => self.save_schedule_as(cx),
            BarItem::CloseSchedule => self.guard(Then::New, window, cx),
            BarItem::Recent(index) => {
                match self.recent_schedules().get(usize::from(index)).cloned() {
                    Some(path) => self.guard(Then::Recent(path), window, cx),
                    None => self.schedule_message = Some(t!(Msg::ScheduleEmpty).into()),
                }
            }
            BarItem::Exit => {
                window.dispatch_action(Box::new(Quit), cx);
            }
            BarItem::Options => self.open_options(window, cx),
            BarItem::GoLive => self.go_live(),
            BarItem::LogoOnLive => self.toggle_mask(Mask::Logo),
            BarItem::BlackOnLive => self.toggle_mask(Mask::Black),
            BarItem::ClearOnLive => self.toggle_mask(Mask::Clear),
            BarItem::ShowLiveOutput => self.toggle_output(),
            BarItem::ShowStageOutput => self.toggle_stage_output(),
            BarItem::StageMessage => self.open_stage_message(window, cx),
            _ => unreachable!("enabled() gates unbuilt items"),
        }
        cx.notify();
    }

    /// The EW-style menu row rendered above the toolbar (not a tab stop;
    /// Alt+mnemonics and mouse reach it, keeping the pane traversal intact).
    pub(crate) fn menu_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.bar_menu.map(|menu| menu.top);
        let bounds = self.bar_bounds.clone();
        div()
            .h(px(MENU_BAR_H))
            .w_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_1()
            .border_b_1()
            .border_color(rgb(BORDER))
            .children(TOP_MENUS.into_iter().map(|top| {
                let is_open = open == Some(top);
                div()
                    .id(top.id())
                    .debug_selector(move || top.id().into())
                    .role(Role::MenuItem)
                    .aria_label(top.label())
                    .aria_expanded(is_open)
                    .accessibility_id(top.id())
                    .h(px(CONTROL_H))
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded(px(RADIUS))
                    .cursor_pointer()
                    .text_size(px(TEXT_BASE))
                    .text_color(rgb(if is_open { FOCUS_TEXT } else { TEXT }))
                    .when(is_open, |d| d.bg(rgb(FOCUS_BG)))
                    .when(!is_open, |d| d.hover(|d| d.bg(rgb(HOVER))))
                    .child(top.label())
                    // While a menu is open, hovering another label switches
                    // to it, like the EW menu bar (and Windows menus generally).
                    .on_hover(cx.listener(move |this, hovered, window, cx| {
                        if *hovered && this.bar_menu.is_some_and(|menu| menu.top != top) {
                            this.open_bar_menu(top, window, cx);
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            // Keep the root's track_focus auto-focus from
                            // stealing the menu's focus on this same press.
                            window.prevent_default();
                            this.open_bar_menu(top, window, cx);
                            if this.bar_menu.is_some() {
                                this.bar_opened_with_mouse = Some((top, Instant::now()));
                            }
                        }),
                    )
                    .on_click(cx.listener(move |this, event, window, cx| {
                        if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                            return;
                        }
                        // The mouse-up completing the press that opened this
                        // menu must not immediately close it again.
                        if this.bar_opened_with_mouse.is_some_and(|(t, at)| {
                            t == top && at.elapsed() < Duration::from_millis(400)
                        }) {
                            this.bar_opened_with_mouse = None;
                            return;
                        }
                        if this.bar_menu.is_some_and(|menu| menu.top == top) {
                            this.dismiss_bar_menu(window, cx);
                        }
                    }))
            }))
            .on_children_prepainted(move |child_bounds, _, _| {
                *bounds.borrow_mut() = child_bounds.into_iter().collect();
            })
            .id("menu-bar")
            .role(Role::MenuBar)
            .aria_label(t!(Msg::A11yMenuBar))
            .accessibility_id("menu-bar")
    }

    fn bar_row(&self, item: BarItem, index: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let enabled = item.enabled(self);
        let highlighted = self
            .bar_menu
            .is_some_and(|menu| menu.highlight == Some(index));
        let checked = item.checked(self);
        let label: SharedString = match item {
            BarItem::Recent(index) => self.recent_label(usize::from(index)).into(),
            _ => item.label().into(),
        };
        let shortcut = item.shortcut();
        let is_header = item.is_header();
        let recent = matches!(item, BarItem::Recent(_));
        div()
            .id(item.id())
            .debug_selector(move || item.id().into())
            .role(if item.checkable() {
                Role::MenuItemCheckBox
            } else {
                Role::MenuItem
            })
            .aria_label(label.clone())
            .accessibility_id(item.id())
            .when(!enabled, mark_disabled)
            .when(item.checkable(), |d| {
                d.aria_toggled(if checked {
                    Toggled::True
                } else {
                    Toggled::False
                })
            })
            .h(px(CONTROL_H))
            .mx_1()
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(RADIUS))
            .text_color(rgb(if enabled { TEXT } else { DISABLED }))
            .when(is_header, |d| d.text_color(rgb(MUTED)))
            .when(highlighted, |d| {
                d.bg(rgb(FOCUS_BG)).text_color(rgb(FOCUS_TEXT))
            })
            .when(enabled && !is_header, |d| {
                d.cursor_pointer()
                    .when(!highlighted, |d| d.hover(|d| d.bg(rgb(HOVER))))
                    .on_click(cx.listener(move |this, event, window, cx| {
                        if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                            return;
                        }
                        this.choose_bar_item(item, window, cx);
                    }))
            })
            .when(is_header, |d| d.child(label.clone()))
            .when(recent, |d| d.pl_4())
            .when(!is_header, |d| {
                d.child(
                    div()
                        .w(px(16.))
                        .flex_shrink_0()
                        .child(if checked { "✓" } else { "" }),
                )
                .child(div().flex_1().min_w_0().truncate().child(label))
                .children(shortcut.map(|s| {
                    div()
                        .flex_shrink_0()
                        .text_size(px(TEXT_SMALL))
                        .text_color(rgb(MUTED))
                        .child(s)
                }))
            })
            .into_any_element()
    }

    /// The deferred+anchored popup for the open menu, with a focused
    /// SelaMenuBar context so Esc/arrows/Enter work and dismiss restores the
    /// previous focus (zed rev a846890 gpui/examples/popover.rs pattern).
    pub(crate) fn bar_menu_overlay(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let menu = self.bar_menu?;
        let index = TOP_MENUS.iter().position(|t| *t == menu.top).unwrap_or(0);
        let position = self
            .bar_bounds
            .borrow()
            .get(index)
            .map(|b| b.bottom_left())
            .unwrap_or(point(px(4.), px(MENU_BAR_H)));
        let rows: Vec<gpui::AnyElement> = menu
            .top
            .entries(self)
            .iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                BarEntry::Separator => div()
                    .h(px(1.))
                    .mx_2()
                    .my_1()
                    .bg(rgb(BORDER))
                    .into_any_element(),
                BarEntry::Item(item) => self.bar_row(*item, index, cx),
            })
            .collect();
        let width = if menu.top == TopMenu::File {
            px(280.)
        } else {
            px(240.)
        };
        let popup = div()
            .w(width)
            .py_1()
            .flex()
            .flex_col()
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE))
            .shadow_md()
            .text_size(px(TEXT_BASE))
            .text_color(rgb(TEXT))
            .children(rows);
        Some(
            deferred(
                anchored()
                    .position(position)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .id("bar-menu")
                            .debug_selector(|| "bar-menu".into())
                            .role(Role::Menu)
                            .aria_label(menu.top.label())
                            .accessibility_id("bar-menu")
                            .track_focus(&self.bar_menu_focus)
                            .key_context("SelaMenuBar")
                            .occlude()
                            .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                                this.dismiss_bar_menu(window, cx);
                                // Swallow the dismissing press like a Windows
                                // menu: it must not also open another menu,
                                // move focus, or trigger what lies beneath.
                                cx.stop_propagation();
                            }))
                            .on_action(cx.listener(|this, _: &MenuNext, _, cx| {
                                this.move_bar_highlight(true, cx);
                            }))
                            .on_action(cx.listener(|this, _: &MenuPrevious, _, cx| {
                                this.move_bar_highlight(false, cx);
                            }))
                            .on_action(cx.listener(|this, _: &BarMenuRight, window, cx| {
                                this.move_bar_menu(true, window, cx);
                            }))
                            .on_action(cx.listener(|this, _: &BarMenuLeft, window, cx| {
                                this.move_bar_menu(false, window, cx);
                            }))
                            .on_action(cx.listener(|this, _: &MenuConfirm, window, cx| {
                                this.confirm_bar_menu(window, cx);
                            }))
                            .on_action(cx.listener(|this, _: &MenuDismiss, window, cx| {
                                this.dismiss_bar_menu(window, cx);
                            }))
                            .child(popup),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}
