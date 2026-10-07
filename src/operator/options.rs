//! Options dialog and the operator's view of the settings store. The sections
//! follow the observed EW Options inventory (EW8-OBS-054); only Main Output ›
//! General (output monitor) exists so far, the rest are marked unavailable.

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
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            slot: Arc::default(),
            list_monitors: Arc::new(Vec::new),
            writer: settings::default_writer(),
        }
    }
}

pub(crate) struct SettingsState {
    pub(crate) current: Settings,
    worker: Option<settings::Worker>,
    load_note: Option<String>,
    save_error: Option<String>,
    /// `None` while the monitor list is loading.
    pub(crate) monitors: Option<Vec<MonitorInfo>>,
    slot: Arc<MonitorSlot>,
    /// Output monitor being edited while Options is open (`None` = default).
    pub(crate) draft: Option<Option<String>>,
    rows: Vec<FocusHandle>,
    _monitor_scan: Option<Task<()>>,
}

impl SettingsState {
    pub(super) fn start(
        profile: Option<&Path>,
        config: SettingsConfig,
        cx: &mut Context<Operator>,
    ) -> Self {
        let worker = profile.and_then(|profile| {
            let worker =
                settings::Worker::start(settings::path_in(profile), config.writer.clone()).ok()?;
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
        let state = Self {
            current: Settings::default(),
            worker,
            load_note: None,
            save_error: None,
            monitors: None,
            slot: config.slot,
            draft: None,
            rows: Vec::new(),
            _monitor_scan: Some(monitor_scan),
        };
        state.sync_slot();
        state
    }

    fn sync_slot(&self) {
        self.slot.set(settings::monitor_arg(
            self.current.output_monitor.as_deref(),
            self.monitors.as_deref(),
        ));
    }

    fn monitor_missing(&self) -> bool {
        settings::monitor_missing(
            self.current.output_monitor.as_deref(),
            self.monitors.as_deref(),
        )
    }
}

const SECTIONS: [(&str, bool); 6] = [
    ("Main Output", true),
    ("Alternate Output", false),
    ("Foldback", false),
    ("Service Intervals", false),
    ("Slide Labels", false),
    ("Advanced", false),
];

const MAIN_OUTPUT_TABS: [(&str, bool); 6] = [
    ("General", true),
    ("Song", false),
    ("Scripture", false),
    ("Presentations", false),
    ("Transitions", false),
    ("Alerts", false),
];

const MONITOR_TAB_INDEX: isize = 800;

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
                    self.settings.current = loaded.settings;
                    self.settings.load_note = loaded.note;
                    self.settings.sync_slot();
                }
                settings::Reply::Saved(Ok(())) => self.settings.save_error = None,
                settings::Reply::Saved(Err(error)) => {
                    self.settings.save_error = Some(format!("Settings were not saved ({error})"));
                }
            }
        }
        changed
    }

    /// Notes about the settings, shown until resolved or replaced.
    pub(crate) fn settings_note(&self) -> Option<String> {
        let mut notes = Vec::new();
        notes.extend(self.settings.load_note.clone());
        if self.settings.monitor_missing() {
            notes.push(format!(
                "Saved output monitor {} was not found; using the default non-primary monitor. Choose one in Options.",
                self.settings.current.output_monitor.as_deref().map(short).unwrap_or_default()
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
        self.open_dialog(Dialog::Options, window, cx);
    }

    /// OK: keeps the draft, persists it through the worker and moves the
    /// audience window if Live output is on.
    pub(crate) fn apply_options(&mut self) {
        let Some(draft) = self.settings.draft.take() else {
            return;
        };
        if draft == self.settings.current.output_monitor {
            return;
        }
        self.settings.current.output_monitor = draft;
        self.settings.sync_slot();
        if let Some(worker) = &self.settings.worker {
            if !worker.save(self.settings.current.clone()) {
                self.settings.save_error = Some("Settings were not saved (queue full)".into());
            }
        } else {
            self.settings.save_error =
                Some("No profile folder; the choice lasts this session".into());
        }
        if self.output.is_some() {
            // A new session spawns the audience on the chosen monitor.
            self.toggle_output();
            self.toggle_output();
        }
    }

    fn choose_monitor(&mut self, index: usize) {
        let name = index
            .checked_sub(1)
            .and_then(|i| self.settings.monitors.as_ref()?.get(i))
            .map(|m| m.name.clone());
        if self.settings.draft.is_some() {
            self.settings.draft = Some(name);
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
        let mut options = vec![("Default (first non-primary monitor)".to_string(), None)];
        for info in monitors.iter().flatten() {
            let primary = if info.primary { " · primary" } else { "" };
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
        let sections = SECTIONS.map(|(label, available)| {
            div()
                .debug_selector(move || format!("options-section-{label}"))
                .h(px(CONTROL_H + 2.))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .when(label == "Main Output", |d| {
                    d.bg(rgb(SELECTED_ROW)).font_weight(FontWeight::MEDIUM)
                })
                .when(!available, |d| d.text_color(rgb(DISABLED)))
                .child(label)
                .when(!available, |d| {
                    d.child(div().text_size(px(TEXT_TILE)).child("not available"))
                })
        });
        let tabs = MAIN_OUTPUT_TABS.map(|(label, available)| {
            div()
                .debug_selector(move || format!("options-tab-{label}"))
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
                .child(label)
        });
        let loading = monitors.is_none();
        Some(
            div()
                .debug_selector(|| "options-backdrop".into())
                .occlude()
                .absolute()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .debug_selector(|| "options-dialog".into())
                        .w(px(660.))
                        .max_w_full()
                        .h(px(372.))
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
                                .px_3()
                                .flex()
                                .items_center()
                                .border_b_1()
                                .border_color(rgb(BORDER))
                                .bg(rgb(CHROME))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Options"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .child(
                                    div()
                                        .w(px(196.))
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
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_h_0()
                                                .p_3()
                                                .flex()
                                                .flex_col()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child("Output monitor"),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(TEXT_SMALL))
                                                        .text_color(rgb(MUTED))
                                                        .child("The audience output opens on this monitor."),
                                                )
                                                .children(loading.then(|| {
                                                    div()
                                                        .text_size(px(TEXT_SMALL))
                                                        .text_color(rgb(MUTED))
                                                        .child("Detecting monitors…")
                                                }))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1()
                                                        .children(rows),
                                                )
                                                .children(draft_missing.then(|| {
                                                    div()
                                                        .debug_selector(|| "options-missing".into())
                                                        .text_size(px(TEXT_SMALL))
                                                        .text_color(rgb(SENDING))
                                                        .child(format!(
                                                            "Saved monitor {} is not connected.",
                                                            draft.as_deref().map(short).unwrap_or_default()
                                                        ))
                                                })),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .h(px(PANE_HEADER_H + 8.))
                                .flex_shrink_0()
                                .px_3()
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap_1()
                                .border_t_1()
                                .border_color(rgb(BORDER))
                                .child(
                                    self.button(Control::DialogConfirm, "dialog-confirm", "OK", cx)
                                        .text_color(rgb(TEXT)),
                                )
                                .child(self.button(
                                    Control::DialogCancel,
                                    "dialog-cancel",
                                    "Cancel",
                                    cx,
                                )),
                        ),
                )
                .into_any_element(),
        )
    }
}

fn short(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}
