//! Schedule autosave, the recovery prompt and the close flush (M1-12).
//!
//! The domain (`sela::autosave`) owns the files and the cadence; this module
//! only feeds it the schedule from the poll loop and shows its state. On the
//! UI thread an autosave costs one fingerprint of at most 32 entries per poll
//! and, per commit, one snapshot clone handed to the worker's bounded queue.
//! No file, database or decode work happens here, and nothing touches Live.
use super::*;
use sela::autosave::{self as save, Job, Offer, Policy, Reply, Snapshot, Step};

/// How the operator window gets its autosave collaborators. `main` sets the
/// real ones; tests set fakes; without it there is no autosave.
pub(crate) struct AutosaveSetup(pub(crate) save::Config);

impl gpui::Global for AutosaveSetup {}

const CLOSE_WAIT: Duration = Duration::from_secs(5);
const CLOSE_POLL: Duration = Duration::from_millis(25);
/// Replies handled per poll; the queue is bounded far below it.
const REPLIES: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Health {
    /// No profile (no library) or no setup: autosave does not exist.
    Absent,
    /// The worker is scanning the recovery folder.
    Starting,
    Ready,
    /// The recovery folder cannot be used at all.
    Off(save::Error),
    /// The newest commit or marker failed; the policy retries.
    Failed(save::Error),
}

pub(crate) struct AutosaveState {
    worker: Option<save::Worker>,
    pub(crate) policy: Policy,
    pub(crate) health: Health,
    clock: save::WallClock,
    /// The previous run's leftovers, while the prompt is open.
    pub(crate) offer: Option<Offer>,
    /// The offer still has to be shown (another dialog was open).
    prompt_pending: bool,
    pub(crate) last: Option<save::Committed>,
    /// A commit stands but old copies could not be removed.
    pub(crate) cleanup: Option<save::Error>,
    /// A clean close was flushed and marked; `None` until then.
    pub(crate) closed: bool,
    /// The close wait expired: close without the flush.
    forced: bool,
    closing: Option<Task<()>>,
    /// The Close job is queued; resubmitted if the queue was full.
    close_sent: bool,
}

impl AutosaveState {
    pub(crate) fn start(profile: Option<&Path>, cx: &mut Context<Operator>) -> Self {
        let setup = cx.try_global::<AutosaveSetup>().map(|s| s.0.clone());
        let initial = save::fingerprint(&Schedule::default());
        let (worker, health, clock, cadence) = match (profile, setup) {
            (Some(profile), Some(config)) => {
                let clock = config.clock.clone();
                let cadence = config.cadence;
                match save::Worker::start(profile.to_path_buf(), config) {
                    Ok(worker) => (Some(worker), Health::Starting, clock, cadence),
                    Err(error) => (
                        None,
                        Health::Off(save::Error::from_io(&error)),
                        clock,
                        cadence,
                    ),
                }
            }
            (_, setup) => (
                None,
                Health::Absent,
                setup.map_or_else(save::system_clock, |s| s.clock),
                save::Cadence::default(),
            ),
        };
        Self {
            worker,
            policy: Policy::new(cadence, initial),
            health,
            clock,
            offer: None,
            prompt_pending: false,
            last: None,
            cleanup: None,
            closed: false,
            forced: false,
            closing: None,
            close_sent: false,
        }
    }

    fn working(&self) -> bool {
        matches!(self.health, Health::Ready | Health::Failed(_))
    }
}

fn reason(error: &save::Error) -> String {
    match error {
        save::Error::DiskFull => t!(Msg::ReasonDiskFull).into(),
        save::Error::ReadOnly => t!(Msg::ReasonReadOnly).into(),
        save::Error::Locked => t!(Msg::ReasonLocked).into(),
        save::Error::Budget => t!(Msg::ReasonBudget).into(),
        save::Error::Io(kind) => t!(Msg::ReasonIo, kind = kind),
    }
}

/// "3 min ago" from two wall-clock instants in ms.
fn ago(then: u64, now: u64) -> String {
    let minutes = now.saturating_sub(then) / 60_000;
    match minutes {
        0 => t!(Msg::AgoJustNow).into(),
        1..=59 => t!(Msg::AgoMinutes, count = minutes),
        60..=2879 => t!(Msg::AgoHours, count = minutes / 60),
        _ => t!(Msg::AgoDays, count = minutes / 1440),
    }
}

impl Operator {
    /// Replies first, then one cadence step for the current schedule.
    pub(crate) fn poll_autosave(&mut self, cx: &mut Context<Self>) -> bool {
        let now = cx.background_executor().now();
        let mut changed = false;
        for _ in 0..REPLIES {
            let Some(reply) = self.autosave.worker.as_ref().and_then(save::Worker::poll) else {
                break;
            };
            changed = true;
            self.autosave_reply(reply, now);
        }
        // A close in progress keeps the prompt back: the window is going
        // away, and a cancelled close shows it on the next poll.
        if self.autosave.prompt_pending
            && self.autosave.closing.is_none()
            && self.show_dialog_later(Dialog::Recovery)
        {
            self.autosave.prompt_pending = false;
            changed = true;
        }
        // Nothing is written while the previous run's prompt is open, nor
        // once the close flush is on its way.
        if !self.autosave.working() || self.autosave.offer.is_some() || self.autosave.close_sent {
            return changed;
        }
        let fingerprint = save::fingerprint(&self.schedule);
        let step = self
            .autosave
            .policy
            .step(fingerprint, self.schedule.is_dirty(), now);
        let job = match step {
            Step::Idle => return changed,
            Step::Mark(f) => Job::Mark(f, self.schedule.entries().len()),
            Step::Unmark => Job::Unmark,
            Step::Commit(f) => Job::Commit(f, Box::new(Snapshot::of(&self.schedule))),
        };
        let queued = self
            .autosave
            .worker
            .as_ref()
            .is_some_and(|worker| worker.submit(job));
        if !queued {
            self.autosave.policy.unqueued(step);
        }
        if self.autosave.closed && self.autosave.policy.uncommitted() {
            // Work after a flushed close that was then cancelled.
            self.autosave.closed = false;
        }
        changed
    }

    fn autosave_reply(&mut self, reply: Reply, now: Instant) {
        let state = &mut self.autosave;
        match reply {
            Reply::Opened(Ok(startup)) => {
                state.health = Health::Ready;
                if let Some(offer) = startup.offer {
                    state.offer = Some(offer);
                    state.prompt_pending = true;
                }
            }
            Reply::Opened(Err(failure)) => {
                eprintln!("autosave unavailable: {:?}", failure.error);
                state.health = Health::Off(failure.error);
                state.worker = None;
                // The previous run's copy was still readable: it can be
                // restored, but the answer cannot be recorded.
                if let Some(offer) = failure.offer {
                    state.offer = Some(*offer);
                    state.prompt_pending = true;
                }
            }
            Reply::Committed(fingerprint, Ok((committed, warning))) => {
                state.policy.finished(fingerprint, true, now);
                state.last = Some(committed);
                state.cleanup = warning;
                state.health = Health::Ready;
            }
            Reply::Committed(fingerprint, Err(error)) => {
                state.policy.finished(fingerprint, false, now);
                state.health = Health::Failed(error);
            }
            Reply::Marked(Ok(())) => {}
            Reply::Marked(Err(error)) => state.health = Health::Failed(error),
            Reply::Resolved(fingerprint, result) => {
                let ok = result.is_ok();
                if let Some(f) = fingerprint {
                    state.policy.finished(f, ok, now);
                }
                match result {
                    Ok(committed) => {
                        state.last = committed.or(state.last);
                        state.health = Health::Ready;
                    }
                    Err(error) => state.health = Health::Failed(error),
                }
            }
            Reply::Closed(fingerprint, result) => {
                if let Some(f) = fingerprint {
                    state.policy.flushed(f);
                }
                if let Err(error) = result {
                    // Closing anyway: the operator asked to quit.
                    eprintln!("autosave close: {error:?}");
                }
                state.closed = true;
                state.close_sent = false;
            }
        }
    }

    /// The recovery prompt's Restore: the revision replaces the (empty)
    /// schedule with its own file and baseline, so unsaved edits stay
    /// unsaved, and is committed again at once as this run's revision. Its
    /// songs then get the same missing-file check as an opened schedule.
    /// With autosave off (the profile could not be written) the schedule is
    /// still restored, read-only: nothing is committed or acknowledged.
    pub(crate) fn restore_recovery(&mut self) {
        let Some(offer) = self.autosave.offer.take() else {
            return;
        };
        let Some(revision) = offer.revision else {
            return self.decline_recovery_offer();
        };
        let snapshot = revision.snapshot;
        let items = snapshot
            .entries
            .iter()
            .map(|e| (e.pin, e.title.clone()))
            .collect();
        if self
            .schedule
            .restore(snapshot.path.clone(), items, snapshot.baseline.clone())
            .is_err()
        {
            return self.decline_recovery_offer();
        }
        self.clear_entry_selection();
        if !self.schedule.entries().is_empty() {
            self.queue(
                storage::Command::Songs(self.schedule.versions()),
                Request::RecoveryCheck(self.schedule.document()),
            );
        }
        let fingerprint = save::fingerprint(&self.schedule);
        let queued = self.autosave.worker.as_ref().is_some_and(|worker| {
            worker.submit(Job::Resolve(Some((
                fingerprint,
                Box::new(Snapshot::of(&self.schedule)),
            ))))
        });
        if queued {
            self.autosave.policy.restoring(fingerprint);
        }
        self.schedule_message = Some(t!(
            Msg::RecoveryRestored,
            count = self.schedule.entries().len()
        ));
    }

    /// Don't Restore (or OK when nothing could be restored): the previous
    /// run is acknowledged and its copies stay on disk.
    pub(crate) fn decline_recovery(&mut self) {
        if self.autosave.offer.take().is_some() {
            self.decline_recovery_offer();
        }
    }

    fn decline_recovery_offer(&mut self) {
        if let Some(worker) = &self.autosave.worker {
            let _ = worker.submit(Job::Resolve(None));
        }
    }

    /// The prompt's text, one line per fact, honest about what is lost.
    pub(crate) fn recovery_lines(&self) -> Vec<String> {
        let Some(offer) = &self.autosave.offer else {
            return Vec::new();
        };
        let now = (self.autosave.clock)();
        let first = if offer.unanswered {
            Msg::RecoveryUnanswered
        } else {
            Msg::RecoveryUnclean
        };
        let mut lines = vec![t!(first).to_string()];
        match &offer.revision {
            Some(revision) => {
                let snapshot = &revision.snapshot;
                let name = snapshot
                    .path
                    .as_deref()
                    .map(schedule_file::display_name)
                    .unwrap_or_else(|| t!(Msg::RecoveryUntitled).into());
                lines.push(t!(
                    Msg::RecoveryRevision,
                    ago = ago(revision.saved_at, now),
                    name = name,
                    count = snapshot.entries.len()
                ));
                if let Some(modified) = offer.file_saved {
                    // Its file was saved after this copy: whatever the copy
                    // held unsaved may be in the file now, with newer work.
                    lines.push(t!(Msg::RecoveryFileNewer, ago = ago(modified, now)));
                } else if snapshot.unsaved() {
                    lines.push(t!(Msg::RecoveryNotInFile).into());
                }
            }
            None => lines.push(t!(Msg::RecoveryNoCopy).into()),
        }
        if let Some(lost) = offer.lost {
            lines.push(t!(
                Msg::RecoveryLost,
                ago = ago(lost.since, now),
                count = lost.items
            ));
        }
        if offer.damaged > 0 {
            lines.push(t!(Msg::RecoveryDamaged, count = offer.damaged));
        }
        if offer.interrupted {
            lines.push(t!(Msg::RecoveryInterrupted).into());
        }
        if offer.revision.is_some() {
            lines.push(t!(Msg::RecoveryKept).into());
        }
        if let Health::Off(error) = &self.autosave.health {
            lines.push(t!(Msg::RecoveryAutosaveOff, reason = reason(error)));
        }
        lines
    }

    pub(crate) fn recovery_restorable(&self) -> bool {
        self.autosave
            .offer
            .as_ref()
            .is_some_and(|o| o.revision.is_some())
    }

    pub(crate) fn recovery_body(&self) -> gpui::AnyElement {
        let lines = self.recovery_lines();
        let text = lines.join(" ");
        div()
            .id("recovery-body")
            .debug_selector(|| "recovery-body".into())
            .role(Role::Label)
            // AccessKit names a Label node by its value (UIA and AT-SPI).
            .aria_label(text.clone())
            .aria_value(text)
            .accessibility_id("recovery-body")
            .flex()
            .flex_col()
            .gap_1()
            .text_size(px(TEXT_SMALL))
            .text_color(rgb(TEXT))
            .children(lines.into_iter().map(|line| div().child(line)))
            .into_any_element()
    }

    /// Schedule footer line about autosave, with whether it is a problem.
    pub(crate) fn autosave_note(&self) -> Option<(String, bool)> {
        let state = &self.autosave;
        if state.closing.is_some() && !state.closed {
            return Some((t!(Msg::AutosaveClosing).into(), false));
        }
        match &state.health {
            Health::Off(error) => {
                return Some((t!(Msg::AutosaveOff, reason = reason(error)), true));
            }
            Health::Failed(error) => {
                return Some((t!(Msg::AutosaveFailed, reason = reason(error)), true));
            }
            Health::Absent | Health::Starting => return None,
            Health::Ready => {}
        }
        if let Some(error) = &state.cleanup {
            return Some((t!(Msg::AutosaveCleanup, reason = reason(error)), true));
        }
        (self.schedule.is_dirty() && !state.policy.uncommitted() && state.last.is_some())
            .then(|| (t!(Msg::AutosaveProtected).into(), false))
    }

    pub(crate) fn autosave_footer(&self) -> Option<gpui::AnyElement> {
        let (text, problem) = self.autosave_note()?;
        Some(
            div()
                .id("autosave-status")
                .debug_selector(|| "autosave-status".into())
                .role(Role::Status)
                .aria_label(text.clone())
                .accessibility_id("autosave-status")
                .flex_shrink_0()
                .px(px(PANE_PAD_X))
                .py_1()
                .border_t_1()
                .border_color(rgb(BORDER))
                .text_size(px(TEXT_CAPTION))
                .text_color(rgb(if problem { ERROR } else { MUTED }))
                .child(text)
                .into_any_element(),
        )
    }

    /// Every close path ends here once the schedule guard has passed.
    pub(crate) fn allow_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.settings_allow_close(window, cx) && self.autosave_allow_close(window, cx)
    }

    /// Close flush: the final schedule is committed when it differs from the
    /// newest commit, and the run is marked clean, before the window closes.
    /// Bounded by `CLOSE_WAIT`, so a stuck disk never traps the operator; the
    /// next start then finds an unclean run and offers what was committed.
    pub(crate) fn autosave_allow_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let state = &self.autosave;
        // With the previous run's offer unanswered the close commits nothing
        // (the store records the run as `offered` instead of clean, so the
        // offer is not acknowledged unseen). That also holds while the worker
        // is still Starting: its Opened reply, and any offer, comes first.
        let flushed = state.closed
            && (state.offer.is_some()
                || save::fingerprint(&self.schedule) == state.policy.committed());
        if state.worker.is_none()
            || matches!(state.health, Health::Absent | Health::Off(_))
            || state.forced
            || flushed
        {
            return true;
        }
        self.submit_close();
        if self.autosave.closing.is_none() {
            let started = cx.background_executor().now();
            self.autosave.closing = Some(cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor().timer(CLOSE_POLL).await;
                    let timed_out = cx.background_executor().now() - started >= CLOSE_WAIT;
                    let done = this.update_in(cx, |this, window, cx| {
                        this.poll_autosave(cx);
                        // Autosave turned off while the close waited (the
                        // start-up open failed): nothing will answer it.
                        if !this.autosave.closed && this.autosave.worker.is_some() {
                            if !timed_out {
                                this.submit_close();
                                return false;
                            }
                            eprintln!("autosave close: no reply within {CLOSE_WAIT:?}");
                            this.autosave.forced = true;
                        }
                        if let Some(task) = this.autosave.closing.take() {
                            task.detach();
                        }
                        // An import preview opened during the wait still
                        // holds the close back (PP-IMPF-09).
                        let discarded =
                            !this.song_import.holds_dialog() && this.discard_still_holds();
                        let close = if discarded {
                            true
                        } else {
                            this.may_close(window, cx)
                        };
                        if close {
                            window.remove_window();
                        }
                        cx.notify();
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

    fn submit_close(&mut self) {
        if self.autosave.close_sent || self.autosave.closed {
            return;
        }
        let fingerprint = save::fingerprint(&self.schedule);
        let last = (fingerprint != self.autosave.policy.committed())
            .then(|| (fingerprint, Box::new(Snapshot::of(&self.schedule))));
        self.autosave.close_sent = self
            .autosave
            .worker
            .as_ref()
            .is_some_and(|worker| worker.submit(Job::Close(last)));
    }

    #[cfg(test)]
    pub(crate) fn autosave_ready(&self) -> bool {
        self.autosave.health == Health::Ready
    }
}
