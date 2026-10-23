//! Stage (Foldback) output on the operator side (M3-01/M3-02): a second,
//! independent renderer session next to the audience one.
//!
//! - Routing: the stage child gets only stage cues (`PreparedCue::for_stage`)
//!   and the audience child only slide cues; each renderer refuses the other
//!   kind, and masks, the logo and slides go to the audience supervisor only.
//! - Independence: each output has its own process, epoch, supervisor and
//!   receipts. A stage that fails, exits or is turned off never changes the
//!   audience session, and the reverse; the stage keeps its last panel.
//! - Content follows what was handed to the audience: the panel is built
//!   from `on_air`/`live_slide`, which change only when a slide cue is given
//!   to the audience supervisor, so a slide (or a newly chosen item) that
//!   fails to prepare or is refused changes neither output.
//! - The panel is drawn in the live slide's resolved face (the bytes the
//!   audience cue carries, shared by `Arc`), so the stage covers the same
//!   scripts as the audience; the renderer substitutes characters that face
//!   lacks instead of refusing the panel (`stage::covered`).
//! - The stage message is typed in a dialog and only sent on OK (EW alerts
//!   are M3-04; this is the stage-only line).
use super::*;
use sela::{
    scene::PreparedCue,
    settings::{self, StageSettings},
    stage::{self, StagePanel, Upcoming},
};

/// Builds the stage child command for a fresh epoch and the `--stage`
/// monitor argument.
pub(crate) type StageLauncher = Arc<dyn Fn(Epoch, String) -> Launch + Send + Sync>;

/// Recently sent panels remembered for naming the acknowledged one.
const SHOWN: usize = 8;
/// Message text bound in bytes (160 characters of up to four bytes).
const MESSAGE_BYTES: usize = stage::MAX_MESSAGE_CHARS * 4;
const FOLDBACK_TAB_INDEX: isize = 820;
const SECTION_TAB_INDEX: isize = 760;

/// Options sections that have a page (EW8-OBS-054 order).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OptionsSection {
    #[default]
    MainOutput,
    Foldback,
}

pub(crate) struct StageState {
    launcher: Option<StageLauncher>,
    /// `SELA_STAGE_MONITOR` is set: the launcher places the stage itself.
    placement_override: bool,
    pub(crate) output: Option<Supervisor>,
    extent: Option<Extent>,
    /// Panel and face handed to this session's supervisor; a differing
    /// panel or face is sent.
    sent: Option<(StagePanel, ContentVersion)>,
    /// Live output came on before the stored settings or the monitor list
    /// had landed: the start decision is taken again when they land.
    recheck: bool,
    revision: u64,
    /// (version, current label) of recently sent panels.
    shown: VecDeque<(ContentVersion, String)>,
    /// Message on the stage; empty when none.
    pub(crate) message: String,
    pub(crate) message_input: Option<Entity<TextInput>>,
    /// Why the stage did not start, or why a panel could not be built.
    pub(crate) note: Option<String>,
    pub(crate) section: OptionsSection,
    /// Foldback settings being edited while Options is open.
    pub(crate) draft: Option<StageSettings>,
    rows: Vec<FocusHandle>,
    sections: Vec<FocusHandle>,
}

impl Default for StageState {
    fn default() -> Self {
        Self {
            launcher: None,
            placement_override: false,
            output: None,
            extent: None,
            sent: None,
            recheck: false,
            revision: 0,
            shown: VecDeque::with_capacity(SHOWN),
            message: String::new(),
            message_input: None,
            note: None,
            section: OptionsSection::default(),
            draft: None,
            rows: Vec::new(),
            sections: Vec::new(),
        }
    }
}

/// The face for "Nothing live" (bundled DejaVu Sans, one shared copy).
fn idle_face() -> (ContentVersion, Arc<[u8]>, u32) {
    let resolved = fonts::Resolved::bundled(&Default::default());
    let face = resolved.face();
    (face.version(), face.bytes_arc(), face.index())
}

impl Operator {
    /// Installs the stage child command (`main` and tests); without one the
    /// stage cannot start. `placement_override` means the launcher places
    /// the window itself, so no free monitor is needed.
    pub(crate) fn set_stage_launcher(&mut self, launcher: StageLauncher, placement_override: bool) {
        self.stage.launcher = Some(launcher);
        self.stage.placement_override = placement_override;
    }

    pub(crate) fn stage_enabled(&self) -> bool {
        self.settings.current.stage.enabled
    }

    /// A stage child command is installed (always, outside tests).
    pub(crate) fn stage_available(&self) -> bool {
        self.stage.launcher.is_some()
    }

    /// Live output on starts the stage when Foldback is enabled; Live output
    /// off stops it. Called after every Live output toggle, and again when
    /// the stored settings or the monitor list land after Live output came
    /// on (`recheck`).
    pub(crate) fn follow_live_output(&mut self) {
        self.stage.recheck = false;
        if self.output.is_some() {
            if !self.settings_loaded_for_options() {
                self.stage.recheck = true;
            } else if self.stage_enabled() && self.stage.output.is_none() {
                self.start_stage();
                self.stage.recheck = self.stage.output.is_none()
                    && !self.stage.placement_override
                    && self.settings.monitors.is_none();
            }
        } else {
            // A start note (no monitor yet, none free) no longer applies.
            self.stage.note = None;
            if self.stage.output.is_some() {
                self.stop_stage();
            }
        }
    }

    /// The stored settings or the monitor list landed: a start decision
    /// taken without them is taken again.
    pub(crate) fn stage_inputs_landed(&mut self) {
        if self.stage.recheck {
            self.follow_live_output();
        }
    }

    pub(crate) fn start_stage(&mut self) {
        // The stage follows Live output (EW8-OBS-STG-5, UNOBSERVED best
        // guess): it never runs on its own.
        if self.output.is_none() {
            self.stage.note = Some(t!(Msg::TurnOnLiveFirst).into());
            return;
        }
        let Some(launcher) = self.stage.launcher.clone() else {
            self.stage.note = Some(t!(Msg::StageUnavailable).into());
            return;
        };
        let current = &self.settings.current;
        let monitor = if self.stage.placement_override {
            Some("secondary".to_owned())
        } else {
            settings::stage_monitor_arg(
                current.stage.monitor.as_deref(),
                current.output_monitor.as_deref(),
                self.settings.monitors.as_deref(),
            )
        };
        let Some(monitor) = monitor else {
            self.stage.note = Some(
                if self.settings.monitors.is_none() {
                    t!(Msg::DetectingMonitors)
                } else {
                    t!(Msg::StageNoMonitor)
                }
                .into(),
            );
            return;
        };
        self.stage.output = Some(Supervisor::start(
            move |epoch| launcher(epoch, monitor),
            Instant::now(),
        ));
        self.stage.extent = None;
        self.stage.sent = None;
        self.stage.note = None;
    }

    /// Ends the stage session; dropping the supervisor reaps its child.
    pub(crate) fn stop_stage(&mut self) {
        self.stage.output = None;
        self.stage.extent = None;
        self.stage.sent = None;
    }

    /// Live › Show Stage Output: stops a running stage; starts one that is
    /// off; a lost stage is restarted at once (the lost-status hint names
    /// this item), never only stopped. Only the stage is touched.
    pub(crate) fn toggle_stage_output(&mut self) {
        match self.stage.output.as_ref().map(Supervisor::status) {
            Some(Status::Lost(_)) => {
                self.stop_stage();
                self.start_stage();
            }
            Some(_) => self.stop_stage(),
            None => self.start_stage(),
        }
    }

    /// Polls the stage session and sends the panel when it differs from the
    /// one already handed over. Building a panel copies the slide strings
    /// and clones the face's `Arc`; the supervisor then encodes the wire
    /// frame, face bytes included, on this thread like an audience cue
    /// (bounded by `MAX_SOURCE_BYTES`; see docs/stage-display.md).
    pub(crate) fn poll_stage(&mut self) -> bool {
        let now = Instant::now();
        let Some(output) = self.stage.output.as_mut() else {
            return false;
        };
        let mut changed = output.poll(now);
        let Status::Connected { extent, caps } = output.status() else {
            return changed;
        };
        if self.stage.extent != Some(extent) {
            self.stage.extent = Some(extent);
            self.stage.sent = None;
            changed = true;
        }
        let panel = self.stage_panel();
        let face = self.stage_face();
        if self
            .stage
            .sent
            .as_ref()
            .is_some_and(|(sent, version)| *sent == panel && *version == face.0)
        {
            return changed;
        }
        let face_version = face.0;
        self.stage.revision += 1;
        let version = ContentVersion {
            id: stage::STAGE_ID,
            revision: self.stage.revision,
        };
        let cue = PreparedCue::for_stage(
            version,
            extent,
            stage::BACKGROUND,
            panel.clone(),
            face,
            caps,
        );
        let Some(output) = self.stage.output.as_mut() else {
            return changed;
        };
        match cue {
            Ok(cue) => {
                // A refusal means the surface changed; the next poll resends.
                if output.present(Arc::new(cue), now).is_ok() {
                    if self.stage.shown.len() == SHOWN {
                        self.stage.shown.pop_front();
                    }
                    self.stage
                        .shown
                        .push_back((version, panel.current_label.clone()));
                    self.stage.sent = Some((panel, face_version));
                    self.stage.note = None;
                }
            }
            Err(error) => {
                self.stage.sent = Some((panel, face_version));
                self.stage.note = Some(t!(Msg::StageNotShown, error = error));
            }
        }
        true
    }

    /// The live slide's resolved face; the bundled face while nothing is on
    /// air. Arc clones only.
    pub(crate) fn stage_face(&self) -> (ContentVersion, Arc<[u8]>, u32) {
        self.on_air
            .as_ref()
            .zip(self.live_slide)
            .and_then(|(item, index)| item.resolved.as_ref()?.get(index).map(|r| r.face()))
            .map_or_else(idle_face, |face| {
                (face.version(), face.bytes_arc(), face.index())
            })
    }

    /// The panel for the slide last handed to the audience supervisor.
    pub(crate) fn stage_panel(&self) -> StagePanel {
        let options = &self.settings.current.stage;
        let mut panel = StagePanel {
            message: self.stage.message.clone(),
            show_clock: options.clock,
            show_next: options.next,
            ..StagePanel::default()
        };
        let live = self.on_air.as_ref().zip(self.live_slide);
        let Some((item, slide)) = live
            .and_then(|(item, index)| item.slides.get(index).map(|slide| (item, (index, slide))))
        else {
            panel.current_label = t!(Msg::StageNothingLive).into();
            return panel;
        };
        let (index, slide) = slide;
        panel.current_label = stage::clean_message(&format!(
            "{} · {}",
            shown_title(&item.title),
            slide_name(index, slide)
        ));
        panel.current = slide.text.clone();
        let texts: Vec<&str> = item.slides.iter().map(|s| s.text.as_str()).collect();
        // The entry after the live one, only while the live entry is still
        // in the schedule (a removed entry has no "next").
        let next_item = item
            .entry
            .filter(|id| self.schedule.index(*id).is_some())
            .and_then(|id| self.schedule.neighbor(Some(id), true))
            .and_then(|id| self.schedule.get(id))
            .map(|entry| entry.title.as_str());
        match stage::upcoming(&texts, index, next_item) {
            Upcoming::Slide { index, text } => {
                panel.next_label = stage::clean_message(&t!(
                    Msg::StageNextSlide,
                    name = slide_name(index, &item.slides[index])
                ));
                panel.next = text.to_owned();
            }
            Upcoming::Item { title } => {
                panel.next_label =
                    stage::clean_message(&t!(Msg::StageNextItem, title = shown_title(title)));
            }
            Upcoming::End => panel.next_label = t!(Msg::StageEnd).into(),
        }
        panel
    }

    /// The stage's own status lines in the Live pane; nothing while the
    /// stage is neither enabled nor running.
    pub(crate) fn stage_lines(&self) -> Vec<(String, u32)> {
        let mut lines = Vec::new();
        match self.stage.output.as_ref() {
            None if self.stage_enabled() => {
                lines.push((t!(Msg::StageOutputOff).into(), DARK_MUTED));
            }
            None => {}
            Some(output) => match output.status() {
                Status::Starting => lines.push((t!(Msg::StageStarting).into(), DARK_MUTED)),
                Status::Connected { extent, .. } => {
                    let slide = match output.live() {
                        LiveState::Confirmed(version) => self
                            .stage
                            .shown
                            .iter()
                            .find(|(v, _)| *v == version)
                            .map(|(_, label)| label.clone()),
                        LiveState::Unknown => None,
                    }
                    .unwrap_or_else(|| t!(Msg::StageNothingConfirmed).into());
                    lines.push((
                        t!(
                            Msg::StageConnected,
                            width = extent.width,
                            height = extent.height,
                            slide = slide
                        ),
                        DARK_TEXT,
                    ));
                }
                Status::Lost(loss) => {
                    let reason = match loss {
                        sela::output::Loss::Spawn => t!(Msg::StageLossSpawn).to_string(),
                        sela::output::Loss::StartupTimedOut => {
                            t!(Msg::StageLossStartup).to_string()
                        }
                        sela::output::Loss::Exited => t!(Msg::StageLossExited).to_string(),
                        sela::output::Loss::Protocol => t!(Msg::StageLossProtocol).to_string(),
                        sela::output::Loss::Delivery(error) => error.to_string(),
                    };
                    lines.push((t!(Msg::StageLost, reason = reason), DARK_DISCONNECTED));
                }
            },
        }
        if let Some((_, error)) = self.stage.output.as_ref().and_then(Supervisor::rejected) {
            lines.push((t!(Msg::StageNotShown, error = error), DARK_ERROR));
        }
        if let Some(note) = &self.stage.note {
            lines.push((note.clone(), DARK_ERROR));
        }
        if !self.stage.message.is_empty() && (self.stage_enabled() || self.stage.output.is_some()) {
            lines.push((
                t!(Msg::StageMessageLine, message = self.stage.message.clone()),
                DARK_MUTED,
            ));
        }
        lines
    }

    /// Live › Stage Message…: the text stays private to this dialog until
    /// Show on Stage.
    pub(crate) fn open_stage_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            return;
        }
        let message = self.stage.message.clone();
        let input = cx.new(|cx| {
            let mut input = TextInput::new(&message, false, MESSAGE_BYTES, DIALOG_TAB_INDEX, cx)
                .unwrap_or_else(|_| {
                    TextInput::new("", false, MESSAGE_BYTES, DIALOG_TAB_INDEX, cx)
                        .expect("empty text is valid")
                });
            input.set_accessible("stage-message-input", t!(Msg::StageMessageLabel));
            input
        });
        self.stage.message_input = Some(input.clone());
        self.open_dialog(Dialog::StageMessage, window, cx);
        input.read(cx).focus_handle(cx).focus(window, cx);
    }

    /// Show on Stage: the typed line replaces the stage message.
    pub(crate) fn confirm_stage_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = self.stage.message_input.take() {
            self.stage.message = stage::clean_message(input.read(cx).text());
        }
        self.close_dialog(window, cx);
    }

    pub(crate) fn clear_stage_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stage.message_input = None;
        self.stage.message.clear();
        self.close_dialog(window, cx);
    }

    pub(crate) fn stage_message_overlay(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.dialog != Some(Dialog::StageMessage) {
            return None;
        }
        let input = self.stage.message_input.clone()?;
        let dialog = div()
            .debug_selector(|| "stage-message-dialog".into())
            .w(px(460.))
            .max_w_full()
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
                    .px(px(PANE_PAD_X))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(CHROME))
                    .font_weight(FontWeight::MEDIUM)
                    .child(t!(Msg::StageMessageHeading)),
            )
            .child(
                div()
                    .p(px(DIALOG_PAD))
                    .flex()
                    .flex_col()
                    .gap(px(DIALOG_GAP))
                    .child(
                        div()
                            .text_size(px(TEXT_SMALL))
                            .text_color(rgb(MUTED))
                            .child(t!(Msg::StageMessageHint)),
                    )
                    .child(input),
            )
            .child(
                div()
                    .h(px(PANE_HEADER_H + 8.))
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
                            t!(Msg::StageMessageShow),
                            cx,
                        )
                        .text_color(rgb(TEXT)),
                    )
                    .child(self.button(
                        Control::DialogSave,
                        "stage-message-clear",
                        t!(Msg::StageMessageClear),
                        cx,
                    ))
                    .child(self.button(
                        Control::DialogCancel,
                        "dialog-cancel",
                        t!(Msg::ButtonCancel),
                        cx,
                    )),
            )
            .into_any_element();
        Some(self.modal("stage-message-backdrop", dialog, cx))
    }

    /// Options opened: Main Output first, Foldback draft from the stored value.
    pub(crate) fn open_stage_options(&mut self) {
        self.stage.section = OptionsSection::MainOutput;
        self.stage.draft = Some(self.settings.current.stage.clone());
    }

    /// OK in Options: applies the Foldback draft. Returns whether it changed.
    /// A running stage follows at once: disabled stops it, a new monitor
    /// restarts it, enabled with Live output on starts it.
    pub(crate) fn apply_stage_options(&mut self) -> bool {
        let Some(draft) = self.stage.draft.take() else {
            return false;
        };
        let before = self.settings.current.stage.clone();
        if draft == before {
            return false;
        }
        self.settings.current.stage = draft.clone();
        if !draft.enabled {
            self.stop_stage();
        } else if self.stage.output.is_some() && draft.monitor != before.monitor {
            self.stop_stage();
            self.start_stage();
        } else {
            self.follow_live_output();
        }
        true
    }

    pub(crate) fn select_options_section(&mut self, section: OptionsSection) {
        if self.dialog == Some(Dialog::Options) {
            self.stage.section = section;
        }
    }

    /// The selectable Options section rows (Main Output, Foldback).
    pub(crate) fn options_section_focus(
        &mut self,
        section: OptionsSection,
        cx: &mut Context<Self>,
    ) -> FocusHandle {
        while self.stage.sections.len() < 2 {
            let index = self.stage.sections.len() as isize;
            self.stage.sections.push(
                cx.focus_handle()
                    .tab_index(SECTION_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        self.stage.sections[section as usize].clone()
    }

    fn edit_stage_draft(&mut self, edit: impl FnOnce(&mut StageSettings)) {
        if self.settings_loaded_for_options()
            && let Some(draft) = self.stage.draft.as_mut()
        {
            edit(draft);
        }
    }

    /// Options › Foldback › General: enable, monitor and stage content.
    pub(crate) fn foldback_page(&mut self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let draft = self.stage.draft.clone().unwrap_or_default();
        let monitors = self.settings.monitors.clone();
        let mut monitor_rows = vec![(t!(Msg::FoldbackAutoMonitor).to_string(), None)];
        for info in monitors.iter().flatten() {
            let primary = if info.primary {
                t!(Msg::PrimarySuffix)
            } else {
                ""
            };
            monitor_rows.push((
                format!(
                    "{} · {}×{}{primary}",
                    info.short_name(),
                    info.width,
                    info.height
                ),
                Some(info.name.clone()),
            ));
        }
        let count = 3 + monitor_rows.len();
        while self.stage.rows.len() < count {
            let index = self.stage.rows.len() as isize;
            self.stage.rows.push(
                cx.focus_handle()
                    .tab_index(FOLDBACK_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        let check = |this: &Self,
                     index: usize,
                     id: &'static str,
                     label: &'static str,
                     on: bool,
                     enabled: bool,
                     toggle: fn(&mut StageSettings),
                     cx: &mut Context<Self>| {
            let row = div()
                .id(id)
                .debug_selector(move || id.into())
                .role(Role::CheckBox)
                .aria_label(label)
                .accessibility_id(id)
                .aria_toggled(if on { Toggled::True } else { Toggled::False })
                .track_focus(&this.stage.rows[index])
                .key_context("SelaControl")
                .h(px(CONTROL_H + 2.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .rounded(px(RADIUS))
                .when(enabled, |d| {
                    d.cursor_pointer()
                        .hover(|d| d.bg(rgb(HOVER)))
                        .focus(|d| d.bg(rgb(FOCUS_BG)))
                })
                .when(!enabled, |d| d.text_color(rgb(DISABLED)))
                .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                    if this.stage.rows[index].is_focused(window) {
                        window.prevent_default();
                        if enabled {
                            this.edit_stage_draft(toggle);
                        }
                        cx.notify();
                    }
                }))
                .on_click(
                    cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                        if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                            return;
                        }
                        this.stage.rows[index].focus(window, cx);
                        if enabled {
                            this.edit_stage_draft(toggle);
                        }
                        cx.notify();
                    }),
                )
                .child(div().w(px(14.)).child(if on { "☑" } else { "☐" }))
                .child(div().flex_1().truncate().child(label));
            if enabled { row } else { mark_disabled(row) }
        };
        let enabled = draft.enabled;
        let mut page = vec![
            check(
                self,
                0,
                "options-stage-enabled",
                t!(Msg::FoldbackEnable),
                draft.enabled,
                true,
                |d| d.enabled = !d.enabled,
                cx,
            )
            .into_any_element(),
            div()
                .text_size(px(TEXT_SMALL))
                .text_color(rgb(MUTED))
                .child(t!(Msg::FoldbackHint))
                .into_any_element(),
            div()
                .mt_2()
                .font_weight(FontWeight::MEDIUM)
                .when(!enabled, |d| d.text_color(rgb(DISABLED)))
                .child(t!(Msg::FoldbackMonitorHeading))
                .into_any_element(),
        ];
        if monitors.is_none() {
            page.push(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(MUTED))
                    .child(t!(Msg::DetectingMonitors))
                    .into_any_element(),
            );
        }
        let rows = monitor_rows
            .into_iter()
            .enumerate()
            .map(|(offset, (label, name))| {
                let index = 3 + offset;
                let selected = draft.monitor == name;
                let id: &'static str = STAGE_MONITOR_IDS
                    .get(offset)
                    .copied()
                    .unwrap_or("options-stage-monitor");
                let row = div()
                    .id(id)
                    .debug_selector(move || id.into())
                    .role(Role::RadioButton)
                    .aria_label(label.clone())
                    .accessibility_id(id)
                    .aria_toggled(if selected {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .track_focus(&self.stage.rows[index])
                    .key_context("SelaControl")
                    .h(px(CONTROL_H + 2.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(RADIUS))
                    .bg(rgb(if selected && enabled {
                        SELECTED_ROW
                    } else {
                        SURFACE
                    }))
                    .when(enabled, |d| {
                        d.cursor_pointer()
                            .hover(|d| d.bg(rgb(HOVER)))
                            .focus(|d| d.bg(rgb(FOCUS_BG)))
                    })
                    .when(!enabled, |d| d.text_color(rgb(DISABLED)))
                    .on_action({
                        let name = name.clone();
                        cx.listener(move |this, _: &ActivateControl, window, cx| {
                            if this.stage.rows[index].is_focused(window) {
                                window.prevent_default();
                                if enabled {
                                    let name = name.clone();
                                    this.edit_stage_draft(|d| d.monitor = name);
                                }
                                cx.notify();
                            }
                        })
                    })
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                return;
                            }
                            this.stage.rows[index].focus(window, cx);
                            if enabled {
                                let name = name.clone();
                                this.edit_stage_draft(|d| d.monitor = name);
                            }
                            cx.notify();
                        }),
                    )
                    .child(div().w(px(14.)).child(if selected { "●" } else { "○" }))
                    .child(div().flex_1().truncate().child(label));
                if enabled { row } else { mark_disabled(row) }.into_any_element()
            })
            .collect::<Vec<_>>();
        page.push(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(rows)
                .into_any_element(),
        );
        if enabled && settings::monitor_missing(draft.monitor.as_deref(), monitors.as_deref()) {
            page.push(
                div()
                    .debug_selector(|| "options-stage-missing".into())
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(SENDING))
                    .child(t!(
                        Msg::StageMonitorMissing,
                        name = draft
                            .monitor
                            .as_deref()
                            .map(|n| n.rsplit('\\').next().unwrap_or(n))
                            .unwrap_or_default()
                    ))
                    .into_any_element(),
            );
        }
        page.push(
            div()
                .mt_2()
                .font_weight(FontWeight::MEDIUM)
                .when(!enabled, |d| d.text_color(rgb(DISABLED)))
                .child(t!(Msg::FoldbackContentHeading))
                .into_any_element(),
        );
        page.push(
            check(
                self,
                1,
                "options-stage-clock",
                t!(Msg::FoldbackShowClock),
                draft.clock,
                enabled,
                |d| d.clock = !d.clock,
                cx,
            )
            .into_any_element(),
        );
        page.push(
            check(
                self,
                2,
                "options-stage-next",
                t!(Msg::FoldbackShowNext),
                draft.next,
                enabled,
                |d| d.next = !d.next,
                cx,
            )
            .into_any_element(),
        );
        page
    }
}

/// Stable selectors for the first Foldback monitor rows (Automatic first).
const STAGE_MONITOR_IDS: [&str; 8] = [
    "options-stage-monitor-0",
    "options-stage-monitor-1",
    "options-stage-monitor-2",
    "options-stage-monitor-3",
    "options-stage-monitor-4",
    "options-stage-monitor-5",
    "options-stage-monitor-6",
    "options-stage-monitor-7",
];
