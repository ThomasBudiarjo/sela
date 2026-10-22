//! Options dialog and the operator's view of the settings store. The sections
//! follow the observed EW Options inventory (EW8-OBS-054); Main Output ›
//! General (output monitor) and Foldback › General (stage output, M3-01/02,
//! drawn by `stage::foldback_page`) exist, the rest are marked unavailable.

use super::stage::OptionsSection;
use super::*;
use sela::settings::{self, MonitorInfo, Settings};
use std::sync::Mutex;

/// The `--audience` monitor argument the audience launcher reads at spawn time.
#[derive(Default)]
pub(crate) struct MonitorSlot(Mutex<String>);

impl MonitorSlot {
    pub(crate) fn get(&self) -> String {
        let value = self.0.lock().map(|v| v.clone()).unwrap_or_default();
        if value.is_empty() {
            "secondary".into()
        } else {
            value
        }
    }

    fn set(&self, value: String) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = value;
        }
    }
}

type MonitorLister = Arc<dyn Fn() -> Vec<MonitorInfo> + Send + Sync>;

/// Process-level collaborators of the settings slice; tests inject fakes.
pub(crate) struct SettingsConfig {
    pub(crate) slot: Arc<MonitorSlot>,
    pub(crate) list_monitors: MonitorLister,
    pub(crate) writer: settings::Writer,
    pub(crate) loader: settings::Loader,
    /// Writes `.selaschedule` files on the schedule file worker.
    pub(crate) schedule_writer: sela::schedule_file::Writer,
    /// Layout read before the window opened, so the first frame already uses it.
    pub(crate) initial_layout: Option<sela::pane_layout::Layout>,
}

pub(crate) enum RecentChange {
    Remember(PathBuf),
    Forget(PathBuf),
    SongOrder(SongOrder),
}

impl RecentChange {
    fn apply(self, settings: &mut Settings) {
        match self {
            Self::Remember(path) => settings.remember_recent(path),
            Self::Forget(path) => settings.forget_recent(&path),
            Self::SongOrder(order) => settings.song_order = order,
        }
    }
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            slot: Arc::default(),
            list_monitors: Arc::new(Vec::new),
            writer: settings::default_writer(),
            loader: settings::default_loader(),
            schedule_writer: sela::schedule_file::default_writer(),
            initial_layout: None,
        }
    }
}

pub(crate) struct SettingsState {
    pub(crate) current: Settings,
    worker: Option<settings::Worker>,
    load_note: Option<String>,
    save_error: Option<String>,
    /// The initial load replied (or there is nothing to load). Options stays
    /// inert until then so a draft never starts from defaults.
    loaded: bool,
    /// Accepted saves whose `Saved` reply has not been polled yet.
    pending_saves: usize,
    /// Recent-schedule changes made before `Loaded`, applied in order on top
    /// of the loaded list so a fast save or open is not forgotten.
    queued_recent: Vec<RecentChange>,
    /// A choice was applied before `Loaded`; the late reply must not undo it.
    applied: bool,
    /// Waiting for pending saves before the window closes.
    closing: Option<Task<()>>,
    /// The wait bound expired: the close skips the settings wait and only
    /// the schedule guard still applies.
    close_forced: bool,
    /// Don't Save was chosen for the quit: the schedule as it was then
    /// (document generation and pins). The settings wait that follows closes
    /// without asking about those edits again, but an edit made during the
    /// wait still prompts.
    discarded: Option<(u64, Vec<Pin>)>,
    /// `None` while the monitor list is loading.
    pub(crate) monitors: Option<Vec<MonitorInfo>>,
    slot: Arc<MonitorSlot>,
    /// Output monitor being edited while Options is open (`None` = default).
    pub(crate) draft: Option<Option<String>>,
    /// Language being edited while Options is open.
    pub(crate) language_draft: Option<Language>,
    rows: Vec<FocusHandle>,
    language_rows: Vec<FocusHandle>,
    _monitor_scan: Option<Task<()>>,
}

impl SettingsState {
    pub(super) fn start(
        profile: Option<&Path>,
        config: SettingsConfig,
        cx: &mut Context<Operator>,
    ) -> Self {
        let worker = profile.and_then(|profile| {
            let worker = settings::Worker::start_with(
                settings::path_in(profile),
                config.writer.clone(),
                config.loader.clone(),
            )
            .ok()?;
            worker.load().then_some(worker)
        });
        let lister = config.list_monitors;
        let scan = cx.background_executor().spawn(async move { lister() });
        let monitor_scan = cx.spawn(async move |this, cx| {
            let monitors = scan.await;
            let _ = this.update(cx, |this, cx| {
                this.settings.monitors = Some(monitors);
                this.settings.sync_slot();
                cx.notify();
            });
        });
        let loaded = worker.is_none();
        let state = Self {
            current: Settings::default(),
            worker,
            load_note: None,
            save_error: None,
            loaded,
            pending_saves: 0,
            queued_recent: Vec::new(),
            applied: false,
            closing: None,
            close_forced: false,
            discarded: None,
            monitors: None,
            slot: config.slot,
            draft: None,
            language_draft: None,
            rows: Vec::new(),
            language_rows: Vec::new(),
            _monitor_scan: Some(monitor_scan),
        };
        state.sync_slot();
        state
    }

    #[cfg(test)]
    pub(crate) fn loaded_for_test(&self) -> bool {
        self.loaded
    }

    #[cfg(test)]
    pub(crate) fn force_loaded_for_test(&mut self) {
        self.loaded = true;
    }

    fn sync_slot(&self) {
        self.slot.set(settings::monitor_arg(
            self.current.output_monitor.as_deref(),
            self.monitors.as_deref(),
        ));
    }

    /// Size of the monitor the audience opens on, resolved like
    /// `monitor_arg`: the saved name, else the first non-primary monitor.
    pub(crate) fn audience_extent(&self) -> Option<Extent> {
        let list = self.monitors.as_deref()?;
        let saved = self.current.output_monitor.as_deref();
        let monitor = saved
            .and_then(|name| list.iter().find(|m| m.name == name))
            .or_else(|| list.iter().find(|m| !m.primary))?;
        Some(Extent {
            width: monitor.width,
            height: monitor.height,
        })
    }

    fn monitor_missing(&self) -> bool {
        settings::monitor_missing(
            self.current.output_monitor.as_deref(),
            self.monitors.as_deref(),
        )
    }
}

/// Section label, stable selector slug, built or not (EW8-OBS-054).
const SECTIONS: [(Msg, &str, bool); 6] = [
    (Msg::SectionMainOutput, "main-output", true),
    (Msg::SectionAlternateOutput, "alternate-output", false),
    (Msg::SectionFoldback, "foldback", true),
    (Msg::SectionServiceIntervals, "service-intervals", false),
    (Msg::SectionSlideLabels, "slide-labels", false),
    (Msg::SectionAdvanced, "advanced", false),
];

const MAIN_OUTPUT_TABS: [(Msg, &str, bool); 6] = [
    (Msg::OptionsTabGeneral, "general", true),
    (Msg::OptionsTabSong, "song", false),
    (Msg::OptionsTabScripture, "scripture", false),
    (Msg::TabPresentations, "presentations", false),
    (Msg::OptionsTabTransitions, "transitions", false),
    (Msg::ToolbarAlerts, "alerts", false),
];

/// Foldback tabs (EW8-OBS-056: General, Song, Scripture, Alerts and Clocks);
/// only General is built.
const FOLDBACK_TABS: [(Msg, &str, bool); 4] = [
    (Msg::OptionsTabGeneral, "general", true),
    (Msg::OptionsTabSong, "song", false),
    (Msg::OptionsTabScripture, "scripture", false),
    (Msg::OptionsTabAlertsClocks, "alerts-clocks", false),
];

const CLOSE_WAIT: Duration = Duration::from_secs(5);
const CLOSE_POLL: Duration = Duration::from_millis(25);
const MONITOR_TAB_INDEX: isize = 800;
const LANGUAGE_TAB_INDEX: isize = 780;

impl Operator {
    pub(crate) fn poll_settings(&mut self) -> bool {
        let mut changed = false;
        // The queue holds a handful of replies at most; bound the loop anyway.
        for _ in 0..4 {
            let Some(reply) = self
                .settings
                .worker
                .as_ref()
                .and_then(settings::Worker::poll)
            else {
                break;
            };
            changed = true;
            match reply {
                settings::Reply::Loaded(loaded) => {
                    self.settings.loaded = true;
                    self.settings.load_note = loaded.note;
                    if !self.settings.applied {
                        i18n::set_language(loaded.settings.language);
                        self.settings.current = loaded.settings;
                        self.settings.sync_slot();
                        if self.settings.draft.is_some() {
                            self.settings.draft =
                                Some(self.settings.current.output_monitor.clone());
                            self.settings.language_draft = Some(self.settings.current.language);
                            self.stage.draft = Some(self.settings.current.stage.clone());
                        }
                    } else {
                        self.settings.current.recent_schedules = loaded.settings.recent_schedules;
                    }
                    let queued = std::mem::take(&mut self.settings.queued_recent);
                    if !queued.is_empty() {
                        for change in queued {
                            change.apply(&mut self.settings.current);
                        }
                        self.save_settings_now();
                    }
                    // The stored Songs order applies once, after the load.
                    if self.settings.current.song_order != self.song_order {
                        self.song_order = self.settings.current.song_order;
                        self.catalog_stale = true;
                    }
                }
                settings::Reply::Saved(Ok(())) => {
                    self.settings.pending_saves = self.settings.pending_saves.saturating_sub(1);
                    self.settings.save_error = None;
                }
                settings::Reply::Saved(Err(error)) => {
                    self.settings.pending_saves = self.settings.pending_saves.saturating_sub(1);
                    self.settings.save_error = Some(t!(Msg::SettingsSaveFailed, error = error));
                }
            }
        }
        changed
    }

    /// Changes the stored settings (the recent schedule list) and writes them
    /// through the settings worker. Skipped until the initial load replied, so
    /// defaults never overwrite the stored file.
    pub(crate) fn update_recent(&mut self, change: RecentChange) {
        if !self.settings.loaded {
            self.settings.queued_recent.push(change);
            return;
        }
        change.apply(&mut self.settings.current);
        self.save_settings_now();
    }

    /// Writes the current splitters, collapsed state and window placement when
    /// they differ from the stored layout. Skipped until the initial load
    /// replied so defaults never overwrite the stored file.
    pub(crate) fn save_layout(&mut self, window: &Window) {
        if !self.settings.loaded {
            return;
        }
        let (bounds, maximized) = match window.window_bounds() {
            WindowBounds::Windowed(bounds) => (Some(bounds), false),
            WindowBounds::Maximized(bounds) => (Some(bounds), true),
            WindowBounds::Fullscreen(_) => (None, false),
        };
        let stored = self.settings.current.layout;
        let layout = sela::pane_layout::Layout {
            ratios: sela::pane_layout::Layout::from_fractions(self.ratios),
            collapsed: self.collapsed,
            window: bounds
                .map(|b| sela::pane_layout::Placement {
                    x: f32::from(b.origin.x).round() as i32,
                    y: f32::from(b.origin.y).round() as i32,
                    width: f32::from(b.size.width).round().max(1.) as u32,
                    height: f32::from(b.size.height).round().max(1.) as u32,
                    maximized,
                })
                .or(stored.and_then(|l| l.window)),
        };
        if stored != Some(layout) {
            self.settings.current.layout = Some(layout);
            self.save_settings_now();
        }
    }

    fn save_settings_now(&mut self) {
        let Some(worker) = &self.settings.worker else {
            return;
        };
        if worker.save(self.settings.current.clone()) {
            self.settings.pending_saves += 1;
        } else {
            self.settings.save_error = Some(t!(Msg::SettingsQueueFull).into());
        }
    }

    /// The initial settings load replied; Options edits wait for it.
    pub(crate) fn settings_loaded_for_options(&self) -> bool {
        self.settings.loaded
    }

    #[cfg(test)]
    pub(crate) fn settings_idle(&self) -> bool {
        self.settings.pending_saves == 0
    }

    /// The worker's initial `Loaded` reply has landed; OK is ignored before it.
    #[cfg(test)]
    pub(crate) fn settings_loaded(&self) -> bool {
        self.settings.loaded
    }

    /// Notes about the settings, shown until resolved or replaced.
    pub(crate) fn settings_note(&self) -> Option<String> {
        let mut notes = Vec::new();
        notes.extend(self.settings.load_note.clone());
        if self.settings.closing.is_some() {
            notes.push(t!(Msg::SavingSettings).into());
        }
        if self.settings.monitor_missing() {
            notes.push(t!(
                Msg::MonitorMissingNote,
                name = self
                    .settings
                    .current
                    .output_monitor
                    .as_deref()
                    .map(short)
                    .unwrap_or_default()
            ));
        }
        notes.extend(self.settings.save_error.clone());
        (!notes.is_empty()).then(|| notes.join(" · "))
    }

    pub(crate) fn open_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            return;
        }
        self.settings.draft = Some(self.settings.current.output_monitor.clone());
        self.settings.language_draft = Some(self.settings.current.language);
        self.open_stage_options();
        self.open_dialog(Dialog::Options, window, cx);
    }

    /// OK: keeps the drafts, persists them through the worker and moves the
    /// audience window if Live output is on. A language change repaints every
    /// window at once; song content and live output are untouched.
    pub(crate) fn apply_options(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.settings.draft.take() else {
            return;
        };
        if !self.settings.loaded {
            self.settings.language_draft = None;
            self.stage.draft = None;
            return;
        }
        let language = self
            .settings
            .language_draft
            .take()
            .unwrap_or(self.settings.current.language);
        let monitor_changed = draft != self.settings.current.output_monitor;
        let language_changed = language != self.settings.current.language;
        let stage_changed = self.apply_stage_options();
        if !monitor_changed && !language_changed && !stage_changed {
            return;
        }
        self.settings.applied = true;
        self.settings.current.output_monitor = draft;
        self.settings.current.language = language;
        self.settings.sync_slot();
        if let Some(worker) = &self.settings.worker {
            if worker.save(self.settings.current.clone()) {
                self.settings.pending_saves += 1;
            } else {
                self.settings.save_error = Some(t!(Msg::SettingsQueueFull).into());
            }
        } else {
            self.settings.save_error = Some(t!(Msg::SettingsNoProfile).into());
        }
        if language_changed {
            i18n::set_language(language);
            cx.refresh_windows();
        }
        if monitor_changed && self.output.is_some() {
            // A new session spawns the audience on the chosen monitor.
            self.toggle_output();
            self.toggle_output();
        }
    }

    /// Close guard for settings writes. While an accepted save is unwritten
    /// the close is deferred, with a status, and retried once the writer
    /// answers; after `CLOSE_WAIT` the window closes anyway and the failure
    /// is recorded so a stuck writer cannot trap the operator.
    pub(crate) fn settings_allow_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.save_layout(window);
        if self.settings.pending_saves == 0 || self.settings.close_forced {
            return true;
        }
        if self.settings.closing.is_none() {
            let started = cx.background_executor().now();
            self.settings.closing = Some(cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor().timer(CLOSE_POLL).await;
                    let timed_out = cx.background_executor().now() - started >= CLOSE_WAIT;
                    let done = this.update_in(cx, |this, window, cx| {
                        if timed_out && this.settings.pending_saves > 0 {
                            eprintln!("{}", t!(Msg::SettingsCloseTimeout));
                            this.settings.close_forced = true;
                        }
                        if !timed_out && this.settings.pending_saves > 0 {
                            return false;
                        }
                        // The request is finished: hand it back to the full
                        // guard, which may prompt for schedule edits made
                        // during the wait instead of closing silently.
                        if let Some(task) = this.settings.closing.take() {
                            task.detach();
                        }
                        if this.dialog == Some(Dialog::Unsaved(Then::Quit)) {
                            // The prompt already owns the decision.
                            return true;
                        }
                        if this.discard_still_holds() || this.may_close(window, cx) {
                            window.remove_window();
                        }
                        true
                    });
                    if done.unwrap_or(true) {
                        break;
                    }
                }
            }));
        }
        cx.notify();
        false
    }

    #[cfg(test)]
    pub(crate) fn close_pending(&self) -> bool {
        self.settings.closing.is_some()
    }

    /// Drops a pending close request, e.g. when the operator cancels the
    /// unsaved-schedule prompt.
    pub(crate) fn cancel_close_request(&mut self) {
        self.settings.closing = None;
        self.settings.close_forced = false;
        self.settings.discarded = None;
    }

    /// Records Don't Save for the quit in progress.
    pub(crate) fn discard_for_quit(&mut self) {
        self.settings.discarded = Some((self.schedule.document(), self.schedule.pins().collect()));
    }

    /// Don't Save was chosen and the schedule has not changed since.
    fn discard_still_holds(&self) -> bool {
        self.settings
            .discarded
            .as_ref()
            .is_some_and(|(document, pins)| {
                *document == self.schedule.document()
                    && self.schedule.pins().eq(pins.iter().copied())
            })
    }

    fn choose_monitor(&mut self, index: usize) {
        let name = index
            .checked_sub(1)
            .and_then(|i| self.settings.monitors.as_ref()?.get(i))
            .map(|m| m.name.clone());
        if self.settings.loaded && self.settings.draft.is_some() {
            self.settings.draft = Some(name);
        }
    }

    fn choose_language(&mut self, language: Language) {
        if self.settings.loaded && self.settings.language_draft.is_some() {
            self.settings.language_draft = Some(language);
        }
    }

    pub(crate) fn options_overlay(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let draft = self
            .settings
            .draft
            .clone()
            .filter(|_| self.dialog == Some(Dialog::Options))?;
        let monitors = self.settings.monitors.clone();
        let count = monitors.as_ref().map_or(0, Vec::len) + 1;
        while self.settings.rows.len() < count {
            let index = self.settings.rows.len() as isize;
            self.settings.rows.push(
                cx.focus_handle()
                    .tab_index(MONITOR_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        let draft_missing = settings::monitor_missing(draft.as_deref(), monitors.as_deref());
        let language_draft = self.settings.language_draft.unwrap_or_default();
        while self.settings.language_rows.len() < Language::ALL.len() {
            let index = self.settings.language_rows.len() as isize;
            self.settings.language_rows.push(
                cx.focus_handle()
                    .tab_index(LANGUAGE_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        let language_rows = Language::ALL
            .into_iter()
            .enumerate()
            .map(|(index, language)| {
                let selected = language_draft == language;
                div()
                    .id(("options-language", index))
                    .debug_selector(move || format!("options-language-{}", language.code()))
                    .track_focus(&self.settings.language_rows[index])
                    .key_context("SelaControl")
                    .h(px(CONTROL_H + 2.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(RADIUS))
                    .cursor_pointer()
                    .bg(rgb(if selected { SELECTED_ROW } else { SURFACE }))
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        if this.settings.language_rows[index].is_focused(window) {
                            window.prevent_default();
                            this.choose_language(language);
                            cx.notify();
                        }
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                return;
                            }
                            this.settings.language_rows[index].focus(window, cx);
                            this.choose_language(language);
                            cx.notify();
                        }),
                    )
                    .child(div().w(px(14.)).child(if selected { "●" } else { "○" }))
                    .child(div().flex_1().truncate().child(language.label()))
            })
            .collect::<Vec<_>>();
        let mut options = vec![(t!(Msg::DefaultMonitor).to_string(), None)];
        for info in monitors.iter().flatten() {
            let primary = if info.primary {
                t!(Msg::PrimarySuffix)
            } else {
                ""
            };
            options.push((
                format!(
                    "{} · {}×{}{primary}",
                    info.short_name(),
                    info.width,
                    info.height
                ),
                Some(info.name.clone()),
            ));
        }
        let rows = options
            .into_iter()
            .enumerate()
            .map(|(index, (label, name))| {
                let selected = draft == name;
                div()
                    .id(("options-monitor", index))
                    .debug_selector(move || format!("options-monitor-{index}"))
                    .track_focus(&self.settings.rows[index])
                    .key_context("SelaControl")
                    .h(px(CONTROL_H + 2.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(RADIUS))
                    .cursor_pointer()
                    .bg(rgb(if selected { SELECTED_ROW } else { SURFACE }))
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        if this.settings.rows[index].is_focused(window) {
                            window.prevent_default();
                            this.choose_monitor(index);
                            cx.notify();
                        }
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                return;
                            }
                            this.settings.rows[index].focus(window, cx);
                            this.choose_monitor(index);
                            cx.notify();
                        }),
                    )
                    .child(div().w(px(14.)).child(if selected { "●" } else { "○" }))
                    .child(div().flex_1().truncate().child(label))
            })
            .collect::<Vec<_>>();
        let foldback = self.stage.section == OptionsSection::Foldback;
        let shown = if foldback {
            Msg::SectionFoldback
        } else {
            Msg::SectionMainOutput
        };
        let section_focus = [
            self.options_section_focus(OptionsSection::MainOutput, cx),
            self.options_section_focus(OptionsSection::Foldback, cx),
        ];
        let sections = SECTIONS.map(|(msg, slug, available)| {
            let section = match msg {
                Msg::SectionMainOutput => Some(OptionsSection::MainOutput),
                Msg::SectionFoldback => Some(OptionsSection::Foldback),
                _ => None,
            };
            div()
                .id(slug)
                .debug_selector(move || format!("options-section-{slug}"))
                .min_h(px(CONTROL_H + 2.))
                .px(px(PANE_PAD_X))
                .flex()
                .flex_col()
                .justify_center()
                .when(!available, |d| d.py_1())
                .when(msg == shown, |d| {
                    d.bg(rgb(SELECTED_ROW)).font_weight(FontWeight::MEDIUM)
                })
                .when_some(section, |d, section| {
                    let focus = section_focus[section as usize].clone();
                    d.role(Role::Tab)
                        .aria_label(t!(msg))
                        .accessibility_id(format!("options-section-{slug}"))
                        .aria_selected(msg == shown)
                        .track_focus(&focus)
                        .key_context("SelaControl")
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(HOVER)))
                        .focus(|d| d.bg(rgb(FOCUS_BG)))
                        .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                            if focus.is_focused(window) {
                                window.prevent_default();
                                this.select_options_section(section);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                return;
                            }
                            this.select_options_section(section);
                            cx.notify();
                        }))
                })
                .when(!available, |d| d.text_color(rgb(DISABLED)))
                .child(div().child(t!(msg)))
                .when(!available, |d| {
                    d.child(div().text_size(px(TEXT_TILE)).child(t!(Msg::NotAvailable)))
                })
        });
        let tab_list: &[(Msg, &str, bool)] = if foldback {
            &FOLDBACK_TABS
        } else {
            &MAIN_OUTPUT_TABS
        };
        let tabs = tab_list.iter().copied().map(|(msg, slug, available)| {
            div()
                .debug_selector(move || format!("options-tab-{slug}"))
                .h_full()
                .px_2()
                .flex()
                .items_center()
                .border_b_2()
                .border_color(rgb(if available { ACCENT } else { SURFACE }))
                .when(available, |d| {
                    d.font_weight(FontWeight::MEDIUM).text_color(rgb(TEXT))
                })
                .when(!available, |d| d.text_color(rgb(DISABLED)))
                .child(t!(msg))
        });
        let loading = monitors.is_none();
        let settings_loaded = self.settings.loaded;
        let foldback_page = foldback.then(|| self.foldback_page(cx));
        let dialog = {
            div()
                .debug_selector(|| "options-dialog".into())
                .w(px(720.))
                .max_w_full()
                .h(px(500.))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(6.))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(SURFACE))
                .shadow_md()
                .text_size(px(TEXT_BASE))
                .text_color(rgb(TEXT))
                .child(
                    div()
                        .h(px(PANE_HEADER_H))
                        .flex_shrink_0()
                        .px(px(PANE_PAD_X))
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .bg(rgb(CHROME))
                        .font_weight(FontWeight::MEDIUM)
                        .child(t!(Msg::OptionsTitle)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .child(
                            div()
                                .w(px(224.))
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .py_1()
                                .border_r_1()
                                .border_color(rgb(BORDER))
                                .bg(rgb(CHROME))
                                .children(sections),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .h(px(TAB_BAR_H))
                                        .flex_shrink_0()
                                        .flex()
                                        .px_2()
                                        .border_b_1()
                                        .border_color(rgb(BORDER))
                                        .children(tabs),
                                )
                                .children(foldback_page.map(|page| {
                                    div()
                                        .debug_selector(|| "options-foldback-page".into())
                                        .flex_1()
                                        .min_h_0()
                                        .p(px(DIALOG_PAD))
                                        .flex()
                                        .flex_col()
                                        .gap(px(DIALOG_GAP))
                                        .children(page)
                                }))
                                .when(!foldback, |d| {
                                    d.child(
                                        div()
                                            .flex_1()
                                            .min_h_0()
                                            .p(px(DIALOG_PAD))
                                            .flex()
                                            .flex_col()
                                            .gap(px(DIALOG_GAP))
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(t!(Msg::LanguageHeading)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(TEXT_SMALL))
                                                    .text_color(rgb(MUTED))
                                                    .child(t!(Msg::LanguageHint)),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .children(language_rows),
                                            )
                                            .child(
                                                div()
                                                    .mt_2()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(t!(Msg::MonitorHeading)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(TEXT_SMALL))
                                                    .text_color(rgb(MUTED))
                                                    .child(t!(Msg::MonitorHint)),
                                            )
                                            .children((!settings_loaded).then(|| {
                                                div()
                                                    .debug_selector(|| "options-loading".into())
                                                    .text_size(px(TEXT_SMALL))
                                                    .text_color(rgb(MUTED))
                                                    .child(t!(Msg::LoadingSettings))
                                            }))
                                            .children(loading.then(|| {
                                                div()
                                                    .text_size(px(TEXT_SMALL))
                                                    .text_color(rgb(MUTED))
                                                    .child(t!(Msg::DetectingMonitors))
                                            }))
                                            .child(div().flex().flex_col().gap_1().children(rows))
                                            .children(draft_missing.then(|| {
                                                div()
                                                    .debug_selector(|| "options-missing".into())
                                                    .text_size(px(TEXT_SMALL))
                                                    .text_color(rgb(SENDING))
                                                    .child(t!(
                                                        Msg::MonitorMissingDialog,
                                                        name = draft
                                                            .as_deref()
                                                            .map(short)
                                                            .unwrap_or_default()
                                                    ))
                                            })),
                                    )
                                }),
                        ),
                )
                .child(
                    div()
                        .h(px(PANE_HEADER_H + 8.))
                        .flex_shrink_0()
                        .px(px(PANE_PAD_X))
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_1()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .child(
                            self.button(
                                Control::DialogConfirm,
                                "dialog-confirm",
                                t!(Msg::ButtonOk),
                                cx,
                            )
                            .text_color(rgb(TEXT)),
                        )
                        .child(self.button(
                            Control::DialogCancel,
                            "dialog-cancel",
                            t!(Msg::ButtonCancel),
                            cx,
                        )),
                )
                .into_any_element()
        };
        Some(self.modal("options-backdrop", dialog, cx))
    }
}

fn short(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}
