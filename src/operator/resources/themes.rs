//! Themes tab (M1-08): the library's song themes with rendered thumbnails
//! and their warnings, the default song theme, and applying a theme to the
//! selected song or schedule items. The Themes tab exists in EasyWorship
//! 8.0.49 (EW8-OBS-010); its content is unobserved, so this layout is a
//! best guess (docs/reference-observations.md, "Themes — UNOBSERVED best
//! guesses"). "Set as Default" follows EW8-OBS-019 ("Set As Default Song
//! Theme"). Nothing here changes Live: a themed item reaches the audience
//! only through the next Go Live.
use super::*;
use crate::operator::controls::chrome_button;
use sela::theme::{self, Theme, ThemeId, ThemeSet};

/// Focus band of the tab's buttons and rows, after the image rows.
const THEME_TAB_INDEX: isize = 2400;
const ROW_TAB_INDEX: isize = 2500;
/// Thumbnail size shown in a row (the render is 160x90).
const THUMB_W: f32 = 96.;
const THUMB_H: f32 = 54.;

/// The tab's footer commands, in keyboard order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemeAction {
    New,
    Edit,
    Duplicate,
    Delete,
    SetDefault,
    ApplySong,
    ApplyItem,
    ItemSongTheme,
}

impl ThemeAction {
    pub(crate) const ALL: [ThemeAction; 8] = [
        Self::New,
        Self::Edit,
        Self::Duplicate,
        Self::Delete,
        Self::SetDefault,
        Self::ApplySong,
        Self::ApplyItem,
        Self::ItemSongTheme,
    ];

    /// Stable element and accessibility id, the same in every language.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::New => "theme-new",
            Self::Edit => "theme-edit",
            Self::Duplicate => "theme-duplicate",
            Self::Delete => "theme-delete",
            Self::SetDefault => "theme-set-default",
            Self::ApplySong => "theme-apply-song",
            Self::ApplyItem => "theme-apply-item",
            Self::ItemSongTheme => "theme-item-song-theme",
        }
    }

    fn label(self, confirming: bool) -> &'static str {
        t!(match self {
            Self::New => Msg::ThemeNew,
            Self::Edit => Msg::ThemeEdit,
            Self::Duplicate => Msg::ThemeDuplicate,
            Self::Delete if confirming => Msg::ThemeConfirmDelete,
            Self::Delete => Msg::ThemeDelete,
            Self::SetDefault => Msg::ThemeSetDefault,
            Self::ApplySong => Msg::ThemeApplySong,
            Self::ApplyItem => Msg::ThemeApplyItem,
            Self::ItemSongTheme => Msg::ThemeItemSongTheme,
        })
    }
}

/// What a theme storage reply completes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ThemeJob {
    Load,
    Saved(String),
    Deleted(String),
    Default(String),
    AppliedSong {
        name: String,
        count: usize,
        songs: Vec<storage::Id>,
        theme: ThemeId,
    },
}

/// One rendered row thumbnail and the theme's warnings at an output size.
pub(crate) struct ThemeThumb {
    pub(crate) image: Option<Arc<gpui::RenderImage>>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) struct ThemeState {
    pub(crate) set: ThemeSet,
    /// A reply has filled `set` at least once.
    pub(crate) loaded: bool,
    /// The library may have changed: reload when the worker is free.
    pub(crate) stale: bool,
    /// One write waiting for the storage worker.
    job: Option<(storage::Command, ThemeJob)>,
    /// Highlighted row; `ThemeId::BUILT_IN` is the built-in row.
    pub(crate) selected: ThemeId,
    pub(crate) message: Option<(String, bool)>,
    /// Delete was pressed once for this theme; the next press deletes.
    pub(crate) confirm_delete: Option<ThemeId>,
    /// By theme stamp and output extent; pruned to the listed themes.
    pub(crate) thumbs: HashMap<(u64, u32, u32), ThemeThumb>,
    thumb_task: Option<Task<()>>,
    rows: Vec<FocusHandle>,
    buttons: Vec<FocusHandle>,
    /// An opened schedule file's theme snapshots, waiting for its songs to
    /// resolve against the library.
    pub(crate) opening: Option<(PathBuf, Vec<Option<sela::schedule::ItemTheme>>)>,
}

impl ThemeState {
    pub(crate) fn new(cx: &mut App) -> Self {
        Self {
            set: ThemeSet::default(),
            loaded: false,
            stale: true,
            job: None,
            selected: ThemeId::BUILT_IN,
            message: None,
            confirm_delete: None,
            thumbs: HashMap::new(),
            thumb_task: None,
            opening: None,
            rows: Vec::new(),
            buttons: (0..ThemeAction::ALL.len())
                .map(|i| {
                    cx.focus_handle()
                        .tab_index(THEME_TAB_INDEX + i as isize)
                        .tab_stop(true)
                })
                .collect(),
        }
    }

    /// The built-in row first, then the library themes by name.
    pub(crate) fn rows(&self) -> Vec<Theme> {
        std::iter::once(Theme::built_in())
            .chain(self.set.themes.iter().cloned())
            .collect()
    }

    pub(crate) fn selected_theme(&self) -> Option<Theme> {
        if self.selected == ThemeId::BUILT_IN {
            return Some(Theme::built_in());
        }
        self.set.get(self.selected).cloned()
    }

    /// The default song theme's id as rows show it.
    fn default_id(&self) -> ThemeId {
        self.set
            .default
            .filter(|id| self.set.get(*id).is_some())
            .unwrap_or(ThemeId::BUILT_IN)
    }
}

/// A theme's name as the operator reads it (the built-in name is localized).
pub(crate) fn theme_name(theme: &Theme) -> String {
    if theme.is_built_in() {
        t!(Msg::ThemeBuiltIn).to_string()
    } else {
        theme.name.clone()
    }
}

impl Operator {
    /// The next theme storage command, ahead of a Songs reload: a queued
    /// write, else a reload when the library may have changed.
    pub(crate) fn take_theme_job(&mut self) -> Option<(storage::Command, Request)> {
        if let Some((command, job)) = self.themes.job.take() {
            return Some((command, Request::Themes(job)));
        }
        if self.themes.stale {
            self.themes.stale = false;
            return Some((storage::Command::Themes, Request::Themes(ThemeJob::Load)));
        }
        None
    }

    /// A theme reply landed: show the new set and say what changed. A failed
    /// write leaves the previous set and reports the error.
    pub(crate) fn themes_replied(
        &mut self,
        job: ThemeJob,
        result: Result<ThemeSet, storage::Error>,
    ) {
        match result {
            Ok(set) => {
                self.themes.set = set;
                self.themes.loaded = true;
                if self.themes.selected != ThemeId::BUILT_IN
                    && self.themes.set.get(self.themes.selected).is_none()
                {
                    self.themes.selected = ThemeId::BUILT_IN;
                }
                let stamps: HashSet<u64> = self.themes.rows().iter().map(Theme::stamp).collect();
                self.themes
                    .thumbs
                    .retain(|(stamp, _, _), _| stamps.contains(stamp));
                self.themes.message = match job {
                    ThemeJob::Load => self.themes.message.take(),
                    ThemeJob::Saved(name) => Some((t!(Msg::ThemeSavedNamed, name = name), false)),
                    ThemeJob::Deleted(name) => Some((t!(Msg::ThemeDeleted, name = name), false)),
                    ThemeJob::Default(name) => Some((t!(Msg::ThemeDefaultSet, name = name), false)),
                    ThemeJob::AppliedSong {
                        name,
                        count,
                        songs,
                        theme,
                    } => {
                        // A library preview of a restyled song reloads with
                        // its new theme; schedule entries keep snapshots
                        // and Live keeps its scene.
                        if let Some(item) = self
                            .preview
                            .as_mut()
                            .filter(|item| item.entry.is_none() && songs.contains(&item.version.id))
                        {
                            item.song_theme = Some(theme);
                        }
                        Some((t!(Msg::ThemeAppliedSong, name = name, count = count), false))
                    }
                };
            }
            Err(storage::Error::Budget) => {
                self.themes.message = Some((t!(Msg::ThemeLimit, max = theme::MAX_THEMES), true));
            }
            Err(error) => {
                self.themes.message = Some((
                    t!(Msg::ThemeStorageFailed, error = format!("{error:?}")),
                    true,
                ));
            }
        }
    }

    fn queue_theme(&mut self, command: storage::Command, job: ThemeJob) {
        if self.worker.is_none() {
            self.themes.message = Some((t!(Msg::NoLibraryOpen).into(), true));
            return;
        }
        if self.themes.job.is_some() || matches!(self.request, Some(Request::Themes(_))) {
            self.themes.message = Some((t!(Msg::ThemeBusy).into(), true));
            return;
        }
        self.themes.job = Some((command, job));
    }

    /// The theme a library song (by its stored theme) renders with now.
    pub(crate) fn library_theme(&self, song_theme: Option<ThemeId>) -> theme::Resolution {
        self.themes.set.resolve(song_theme)
    }

    /// Preview item for a loaded song: its schedule entry's theme snapshot
    /// (captured from the library on first show), else the library theme.
    pub(crate) fn themed_item(
        &mut self,
        version: Version,
        song: &storage::Song,
        song_theme: Option<ThemeId>,
    ) -> Item {
        let resolution = self.library_theme(song_theme);
        let (applied, missing) = match self.selected_entry {
            Some(entry) => match self.schedule.capture_theme(entry, &resolution.theme) {
                Some(item) => (item.theme.clone(), None),
                None => (resolution.theme, resolution.missing),
            },
            None => (resolution.theme, resolution.missing),
        };
        let chosen = schedule::entry_arrangement(&self.schedule, self.selected_entry);
        let themed = theme::apply(song, &applied);
        let projection = slides::project(&themed, chosen);
        Item {
            version,
            entry: self.selected_entry,
            title: song.title.clone(),
            slides: projection.slides.into(),
            pages: projection.pages.into(),
            arrangements: song
                .variants
                .iter()
                .map(|v| (v.id, v.name.clone()))
                .collect(),
            chosen,
            copyright: theme::copyright_strip(song, &applied).map(Into::into),
            resolved: None,
            theme_key: applied.key(),
            theme_stamp: applied.stamp(),
            theme_name: theme_name(&applied),
            theme_missing: missing.is_some(),
            song_theme,
        }
    }

    /// The theme key the previewed item should have now. A library song
    /// follows the library; an entry keeps its snapshot.
    pub(crate) fn wanted_theme_key(&self, item: &Item) -> u64 {
        match item
            .entry
            .and_then(|id| self.schedule.get(id))
            .and_then(|e| e.theme.as_ref())
        {
            Some(snapshot) => snapshot.theme.key(),
            None => self.library_theme(item.song_theme).theme.key(),
        }
    }

    /// "Theme: name" under Preview for an item with a theme of its own
    /// look (the built-in look stays unlabelled, as before themes).
    pub(crate) fn theme_label(&self) -> Option<String> {
        let item = self.preview.as_ref().filter(|item| item.theme_key != 0)?;
        Some(t!(Msg::ThemeUsing, name = item.theme_name))
    }

    /// Theme notes for the Preview pane: a missing song theme and faces
    /// that fall back to the bundled font, so no substitution is silent.
    pub(crate) fn theme_notes(&self) -> Vec<String> {
        let Some(item) = &self.preview else {
            return Vec::new();
        };
        let mut notes = Vec::new();
        if item.theme_missing {
            notes.push(t!(Msg::ThemeMissing).to_string());
        }
        if let (Some(resolved), Some(slide)) = (
            item.resolved
                .as_ref()
                .and_then(|r| r.get(self.preview_slide)),
            item.slides.get(self.preview_slide),
        ) && resolved.warning().is_some()
            && let Some(name) = slide.format.font.as_deref()
        {
            notes.push(t!(Msg::ThemeWarnFont, name = name));
        }
        notes
    }

    pub(crate) fn theme_action(
        &mut self,
        action: ThemeAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action != ThemeAction::Delete {
            self.themes.confirm_delete = None;
        }
        let selected = self.themes.selected_theme();
        match action {
            ThemeAction::New => {
                let theme = Theme::new(t!(Msg::ThemeNewName));
                self.open_theme_editor(theme, cx);
            }
            ThemeAction::Edit => match selected {
                Some(theme) if theme.is_built_in() => {
                    self.themes.message = Some((t!(Msg::ThemeBuiltInReadOnly).into(), true));
                }
                Some(theme) => self.open_theme_editor(theme, cx),
                None => self.themes.message = Some((t!(Msg::ThemeSelectTheme).into(), true)),
            },
            ThemeAction::Duplicate => {
                if let Some(theme) = selected {
                    let name = t!(Msg::ThemeCopyName, name = theme_name(&theme));
                    let copy = Theme {
                        id: ThemeId::allocate(),
                        name: truncate_name(&name),
                        ..theme
                    };
                    self.themes.selected = copy.id;
                    self.queue_theme(
                        storage::Command::SaveTheme(copy.clone()),
                        ThemeJob::Saved(copy.name),
                    );
                }
            }
            ThemeAction::Delete => match selected {
                Some(theme) if theme.is_built_in() => {
                    self.themes.message = Some((t!(Msg::ThemeBuiltInReadOnly).into(), true));
                }
                Some(theme) if self.themes.confirm_delete == Some(theme.id) => {
                    self.themes.confirm_delete = None;
                    self.queue_theme(
                        storage::Command::DeleteTheme(theme.id),
                        ThemeJob::Deleted(theme.name),
                    );
                }
                Some(theme) => {
                    self.themes.confirm_delete = Some(theme.id);
                    self.themes.message =
                        Some((t!(Msg::ThemeDeleteHint, name = theme_name(&theme)), false));
                }
                None => self.themes.message = Some((t!(Msg::ThemeSelectTheme).into(), true)),
            },
            ThemeAction::SetDefault => {
                if let Some(theme) = selected {
                    let id = (!theme.is_built_in()).then_some(theme.id);
                    self.queue_theme(
                        storage::Command::SetDefaultTheme(id),
                        ThemeJob::Default(theme_name(&theme)),
                    );
                }
            }
            ThemeAction::ApplySong => {
                let mut songs: Vec<storage::Id> = self
                    .selected_songs()
                    .into_iter()
                    .map(|(v, _)| v.id)
                    .collect();
                if songs.is_empty()
                    && self.selected_entry.is_none()
                    && let Some(song) = self.selected_song
                {
                    songs.push(song.id);
                }
                match (selected, songs.is_empty()) {
                    (_, true) => {
                        self.themes.message = Some((t!(Msg::ThemeSelectSong).into(), true));
                    }
                    (Some(theme), false) => {
                        let count = songs.len();
                        self.queue_theme(
                            storage::Command::SetSongTheme(songs.clone(), Some(theme.id)),
                            ThemeJob::AppliedSong {
                                name: theme_name(&theme),
                                count,
                                songs,
                                theme: theme.id,
                            },
                        );
                    }
                    (None, false) => {}
                }
            }
            ThemeAction::ApplyItem | ThemeAction::ItemSongTheme => {
                let mut ids = self.selected_ids();
                if ids.is_empty() {
                    ids.extend(self.selected_entry);
                }
                if ids.is_empty() {
                    self.themes.message = Some((t!(Msg::ThemeSelectItem).into(), true));
                } else if action == ThemeAction::ApplyItem {
                    if let Some(theme) = selected {
                        let count = self.schedule.set_theme(&ids, Some(&theme));
                        self.themes.message = Some((
                            t!(
                                Msg::ThemeAppliedItem,
                                name = theme_name(&theme),
                                count = count
                            ),
                            false,
                        ));
                    }
                } else {
                    let count = self.schedule.set_theme(&ids, None);
                    self.themes.message = Some((t!(Msg::ThemeItemCleared, count = count), false));
                }
            }
        }
        let _ = window;
        cx.notify();
    }

    fn open_theme_editor(&mut self, theme: Theme, cx: &mut Context<Self>) {
        let opened = self
            .library
            .clone()
            .or_else(crate::song_library::default_path)
            .ok_or_else(|| t!(Msg::NoUserDataDir).to_string())
            .and_then(|path| crate::song_library::open_theme(path, theme, cx));
        if let Err(error) = opened {
            self.themes.message = Some((error, true));
        }
    }

    /// Renders the next missing row thumbnail off the UI thread, one at a
    /// time, at the current output size (aspect warnings depend on it).
    pub(crate) fn ensure_theme_thumbs(&mut self, cx: &mut Context<Self>) {
        if self.themes.thumb_task.is_some() || self.tab != 4 || self.collapsed {
            return;
        }
        let extent = self.background_extent();
        let Some(theme) = self.themes.rows().into_iter().find(|t| {
            !self
                .themes
                .thumbs
                .contains_key(&(t.stamp(), extent.width, extent.height))
        }) else {
            return;
        };
        let profile = self.profile.clone();
        let key = (theme.stamp(), extent.width, extent.height);
        let render = cx.background_executor().spawn(async move {
            crate::song_library::theme_preview(&theme, profile.as_deref(), extent)
        });
        self.themes.thumb_task = Some(cx.spawn(async move |this, cx| {
            let (image, warnings) = render.await;
            let _ = this.update(cx, |this, cx| {
                this.themes.thumb_task = None;
                if let Some(old) = this
                    .themes
                    .thumbs
                    .insert(key, ThemeThumb { image, warnings })
                    .and_then(|old| old.image)
                {
                    cx.drop_image(old, None);
                }
                cx.notify();
            });
        }));
    }

    /// The Themes tab list: one row per theme with its thumbnail, name,
    /// badges and warnings.
    pub(crate) fn theme_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let rows = self.themes.rows();
        while self.themes.rows.len() < rows.len() {
            let index = self.themes.rows.len() as isize;
            self.themes.rows.push(
                cx.focus_handle()
                    .tab_index(ROW_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        self.ensure_theme_thumbs(cx);
        let extent = self.background_extent();
        let default = self.themes.default_id();
        let previewed = self.preview.as_ref().map(|item| item.theme_stamp);
        div()
            .id("theme-list")
            .role(Role::List)
            .aria_label(t!(Msg::TabThemes))
            .accessibility_id("theme-list")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(rows.into_iter().enumerate().map(|(index, theme)| {
                let id = theme.id;
                let selected = self.themes.selected == id;
                let name = theme_name(&theme);
                let thumb = self
                    .themes
                    .thumbs
                    .get(&(theme.stamp(), extent.width, extent.height));
                let mut badges = Vec::new();
                if id == default {
                    badges.push(t!(Msg::ThemeDefaultBadge).to_string());
                }
                if previewed == Some(theme.stamp()) {
                    badges.push(t!(Msg::ThemeInPreview).to_string());
                }
                let warnings = thumb.map(|t| t.warnings.clone()).unwrap_or_default();
                let mut label = name.clone();
                for badge in &badges {
                    label.push_str(" · ");
                    label.push_str(badge);
                }
                for warning in &warnings {
                    label.push_str(" · ");
                    label.push_str(warning);
                }
                let select = move |this: &mut Self| {
                    if this.themes.selected != id {
                        this.themes.confirm_delete = None;
                    }
                    this.themes.selected = id;
                };
                div()
                    .id(("theme", index))
                    .debug_selector(move || format!("theme-{index}"))
                    .role(Role::ListItem)
                    .aria_label(label)
                    .aria_selected(selected)
                    .accessibility_id(format!("theme-{index}"))
                    .track_focus(&self.themes.rows[index])
                    .key_context("SelaControl")
                    .flex_shrink_0()
                    .px_3()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .cursor_pointer()
                    .bg(rgb(if selected { SELECTED_ROW } else { SURFACE }))
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        window.prevent_default();
                        select(this);
                        cx.notify();
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                return;
                            }
                            this.themes.rows[index].focus(window, cx);
                            select(this);
                            if event.click_count() >= 2 {
                                this.theme_action(ThemeAction::Edit, window, cx);
                            }
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .w(px(THUMB_W))
                            .h(px(THUMB_H))
                            .flex_shrink_0()
                            .rounded(px(RADIUS_SMALL))
                            .overflow_hidden()
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(BLACK))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(TEXT_TILE))
                            .text_color(rgb(CANVAS_TEXT))
                            .map(|d| match thumb.and_then(|t| t.image.clone()) {
                                Some(image) => d.child(gpui::img(image).size_full()),
                                None if thumb.is_some() => d.child(t!(Msg::SlideRejectedNote)),
                                None => d.child(t!(Msg::ThemeRendering)),
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().truncate().child(name))
                                    .children(badges.into_iter().map(|badge| {
                                        div()
                                            .px_1()
                                            .rounded(px(RADIUS_SMALL))
                                            .border_1()
                                            .border_color(rgb(BORDER))
                                            .text_size(px(TEXT_CAPTION))
                                            .text_color(rgb(MUTED))
                                            .child(badge)
                                    })),
                            )
                            .children(warnings.into_iter().map(|warning| {
                                div()
                                    .truncate()
                                    .text_size(px(TEXT_CAPTION))
                                    .text_color(rgb(SENDING))
                                    .child(warning)
                            })),
                    )
            }))
            .into_any_element()
    }

    fn theme_button(
        &self,
        index: usize,
        action: ThemeAction,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let confirming = action == ThemeAction::Delete && self.themes.confirm_delete.is_some();
        let label = action.label(confirming);
        let button = chrome_button(action.id(), label)
            .role(Role::Button)
            .aria_label(label)
            .accessibility_id(action.id())
            .track_focus(&self.themes.buttons[index])
            .key_context("SelaControl")
            .focus(|d| d.bg(rgb(FOCUS_BG)).text_color(rgb(FOCUS_TEXT)))
            .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                if this.themes.buttons[index].is_focused(window) {
                    window.prevent_default();
                    this.theme_action(action, window, cx);
                }
            }))
            .on_click(cx.listener(move |this, event, window, cx| {
                if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                    return;
                }
                this.themes.buttons[index].focus(window, cx);
                this.theme_action(action, window, cx);
            }));
        if enabled {
            button
        } else {
            mark_disabled(button).text_color(rgb(DISABLED))
        }
    }

    /// The Themes tab footer: commands and the last outcome.
    pub(crate) fn theme_footer(&self, cx: &mut Context<Self>) -> gpui::Div {
        let library = self.themes.selected != ThemeId::BUILT_IN;
        let entry = self.selected_entry.is_some() || !self.selected_ids().is_empty();
        let song = self.selected_song.is_some() && self.selected_entry.is_none();
        div()
            .debug_selector(|| "theme-footer".into())
            .h(px(STATUS_BAR_H))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .border_t_1()
            .border_color(rgb(BORDER))
            .children(ThemeAction::ALL.iter().enumerate().map(|(index, &action)| {
                let enabled = match action {
                    ThemeAction::Edit | ThemeAction::Delete => library,
                    ThemeAction::ApplySong => song || !self.selected_songs().is_empty(),
                    ThemeAction::ApplyItem | ThemeAction::ItemSongTheme => entry,
                    _ => true,
                };
                self.theme_button(index, action, enabled, cx)
            }))
            .children(self.themes.message.clone().map(|(message, failed)| {
                div()
                    .id("theme-message")
                    .debug_selector(|| "theme-message".into())
                    .role(Role::Status)
                    .aria_label(message.clone())
                    .accessibility_id("theme-message")
                    .px_2()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(TEXT_CAPTION))
                    .text_color(rgb(if failed { ERROR } else { MUTED }))
                    .truncate()
                    .child(message)
            }))
    }
}

/// `name` cut to the theme name limit at a character boundary.
fn truncate_name(name: &str) -> String {
    let mut end = name.len().min(theme::MAX_NAME);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].trim().to_owned()
}
