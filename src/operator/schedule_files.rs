//! Schedule documents: the native Save and Open dialogs, the file worker,
//! Open Recent and the unsaved-changes guard. A schedule is a
//! `.selaschedule` file (see `sela::schedule_file`); the library database only
//! stores the song revisions it pins. Nothing here touches Live.
use super::*;
use sela::schedule_file::{ReadError, ScheduleFile, WriteError};

/// A save waiting for the songs it embeds.
pub(crate) struct SaveJob {
    pub(crate) path: PathBuf,
    pub(crate) versions: Vec<Version>,
    pub(crate) pins: Vec<Pin>,
    /// Guarded command to continue with once the file is written.
    pub(crate) then: Option<Then>,
    /// `Schedule::document` when the save began.
    pub(crate) document: u64,
    /// Each entry and its theme snapshot when the save began (M1-08); an
    /// entry without one takes its song's library theme in the file.
    pub(crate) themes: Vec<(EntryId, Option<sela::schedule::ItemTheme>)>,
}

/// The one file operation on the schedule file worker.
pub(crate) enum FileJob {
    Write(SaveJob),
    Read,
}

impl Operator {
    pub(crate) fn recent_schedules(&self) -> &[PathBuf] {
        &self.settings.current.recent_schedules
    }

    pub(crate) fn recent_label(&self, index: usize) -> String {
        self.recent_schedules()
            .get(index)
            .map(|path| schedule_file::display_name(path))
            .unwrap_or_default()
    }

    /// A schedule file save or open is running or about to start.
    pub(crate) fn schedule_busy(&self) -> bool {
        self.file_job.is_some()
            || self.followup.is_some()
            || self.file_prompt.is_some()
            || matches!(
                self.request,
                Some(Request::SaveSchedule(_) | Request::ImportSchedule(..))
            )
            || matches!(
                self.queued,
                Some((_, Request::SaveSchedule(_) | Request::ImportSchedule(..)))
            )
    }

    /// A file write is queued or running; the window must not close yet.
    pub(crate) fn saving_file(&self) -> bool {
        matches!(self.file_job, Some(FileJob::Write(_)))
            || matches!(self.request, Some(Request::SaveSchedule(_)))
            || matches!(self.queued, Some((_, Request::SaveSchedule(_))))
    }

    pub(crate) fn report_busy(&mut self) {
        self.schedule_message = Some(t!(Msg::WaitScheduleBusy).into());
    }

    /// Ctrl+S and the toolbar Save: the current file, or Save As when untitled.
    pub(crate) fn save_schedule(&mut self, cx: &mut Context<Self>) {
        self.save_then(None, cx);
    }

    pub(crate) fn save_then(&mut self, then: Option<Then>, cx: &mut Context<Self>) {
        if self.worker.is_none() || self.file_worker.is_none() {
            self.schedule_message = Some(t!(Msg::NoLibraryOpen).into());
        } else if self.schedule_busy() {
            self.report_busy();
        } else if let Some(path) = self.schedule.path().map(Path::to_path_buf) {
            self.begin_save(path, then);
        } else {
            self.prompt_save_as(then, cx);
        }
    }

    /// Save Schedule As...: always asks, then switches the current file.
    pub(crate) fn save_schedule_as(&mut self, cx: &mut Context<Self>) {
        if self.worker.is_none() || self.file_worker.is_none() {
            self.schedule_message = Some(t!(Msg::NoLibraryOpen).into());
        } else if self.schedule_busy() {
            self.report_busy();
        } else {
            self.prompt_save_as(None, cx);
        }
    }

    /// Where the native dialogs start: beside the current file, else beside
    /// the newest recent one, else the profile's `Schedules` folder.
    fn schedule_dir(&self) -> PathBuf {
        self.schedule
            .path()
            .or_else(|| self.recent_schedules().first().map(PathBuf::as_path))
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| self.profile.as_ref().map(|p| p.join("Schedules")))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn prompt_save_as(&mut self, then: Option<Then>, cx: &mut Context<Self>) {
        let suggested = self.schedule.name();
        let picked = cx.prompt_for_new_path(&self.schedule_dir(), suggested.as_deref());
        self.file_prompt = Some(cx.spawn(async move |this, cx| {
            let picked = picked.await;
            let _ = this.update(cx, |this, cx| {
                this.finish_prompt();
                // The dialog has no type filter, so the extension is added
                // here, once. A cancelled dialog changes nothing.
                if let Ok(Ok(Some(path))) = picked {
                    this.begin_save(schedule_file::with_extension(&path), then);
                }
                cx.notify();
            });
        }));
    }

    fn finish_prompt(&mut self) {
        if let Some(task) = self.file_prompt.take() {
            task.detach();
        }
    }

    fn begin_save(&mut self, path: PathBuf, then: Option<Then>) {
        if self.schedule_busy() {
            return self.report_busy();
        }
        let versions = self.schedule.versions();
        let job = SaveJob {
            path,
            versions: versions.clone(),
            pins: self.schedule.pins().collect(),
            then,
            document: self.schedule.document(),
            themes: self
                .schedule
                .entries()
                .iter()
                .map(|e| (e.id, e.theme.clone()))
                .collect(),
        };
        if self.queue(
            storage::Command::ThemedSongs(versions),
            Request::SaveSchedule(job),
        ) {
            self.schedule_message = Some(t!(Msg::Saving).into());
        }
    }

    /// Ctrl+O and the toolbar Open, after the unsaved-changes guard.
    pub(crate) fn open_schedule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.guard(Then::Open, window, cx);
    }

    pub(crate) fn prompt_open(&mut self, cx: &mut Context<Self>) {
        if self.worker.is_none() || self.file_worker.is_none() {
            return self.schedule_message = Some(t!(Msg::NoLibraryOpen).into());
        }
        if self.schedule_busy() {
            return self.report_busy();
        }
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t!(Msg::BarOpenSchedule).into()),
        });
        self.file_prompt = Some(cx.spawn(async move |this, cx| {
            let picked = picked.await;
            let _ = this.update(cx, |this, cx| {
                this.finish_prompt();
                if let Ok(Ok(Some(paths))) = picked
                    && let Some(path) = paths.into_iter().next()
                {
                    this.begin_read(path);
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn open_recent(&mut self, path: PathBuf) {
        self.begin_read(path);
    }

    fn begin_read(&mut self, path: PathBuf) {
        if self.worker.is_none() {
            return self.schedule_message = Some(t!(Msg::NoLibraryOpen).into());
        }
        if self.schedule_busy() {
            return self.report_busy();
        }
        let name = schedule_file::display_name(&path);
        let queued = self
            .file_worker
            .as_ref()
            .is_some_and(|worker| worker.read(path.clone()));
        if queued {
            self.file_job = Some(FileJob::Read);
            self.schedule_message = Some(t!(Msg::OpeningSchedule, title = name));
        } else {
            self.schedule_message = Some(t!(Msg::WaitScheduleBusy).into());
        }
    }

    /// The saved songs arrived: hand the document to the file worker.
    pub(crate) fn write_schedule(
        &mut self,
        job: SaveJob,
        songs: Vec<(storage::Song, Option<sela::theme::ThemeId>)>,
    ) {
        let title = job
            .path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut items = Vec::with_capacity(songs.len());
        for (((source, (song, song_theme)), pin), (entry, snapshot)) in job
            .versions
            .iter()
            .zip(songs)
            .zip(&job.pins)
            .zip(&job.themes)
        {
            // A saved item carries its theme snapshot (M1-08), so the file
            // reopens with the same look on another library (image
            // backgrounds are references: name and SHA-256). One never shown
            // takes its song's library theme now, in the file and in the
            // session alike, unless that cannot be trusted (themes not
            // loaded, or the song's theme is missing): then it is saved
            // without a snapshot and resolves again when reopened.
            let theme = match snapshot {
                Some(snapshot) => Some(snapshot.clone()),
                None => self.capturable_theme(song_theme).map(|theme| {
                    if job.document == self.schedule.document() {
                        self.schedule.capture_theme(*entry, &theme);
                    }
                    sela::schedule::ItemTheme {
                        theme,
                        explicit: false,
                    }
                }),
            };
            items.push(schedule_file::Item {
                source: *source,
                song,
                arrangement: pin.arrangement,
                theme,
            });
        }
        let queued = self
            .file_worker
            .as_ref()
            .is_some_and(|worker| worker.write(job.path.clone(), ScheduleFile { title, items }));
        if queued {
            self.file_job = Some(FileJob::Write(job));
        } else {
            let name = schedule_file::display_name(&job.path);
            self.report_file_error(t!(
                Msg::ScheduleSaveFailed,
                name = name,
                error = "worker unavailable"
            ));
        }
    }

    /// A schedule file's songs are in the library: show them as the schedule.
    pub(crate) fn finish_open(
        &mut self,
        path: PathBuf,
        items: Vec<(Version, storage::Song)>,
        arrangements: Vec<Option<VariantId>>,
    ) -> Option<Vec<storage::Song>> {
        let name = schedule_file::display_name(&path);
        let mut themes = match self.themes.opening.take() {
            Some((opened, themes)) if opened == path && themes.len() == items.len() => themes,
            _ => vec![None; items.len()],
        }
        .into_iter();
        let entries = items
            .iter()
            .zip(arrangements)
            .map(|((version, song), arrangement)| {
                let pin = Pin {
                    version: *version,
                    arrangement,
                    theme: None,
                };
                (pin, song.title.clone(), themes.next().flatten())
            })
            .collect();
        match self.schedule.open(path.clone(), entries) {
            Ok(()) => {
                self.clear_entry_selection();
                self.remember_recent(path);
                self.schedule_message = Some(t!(Msg::OpenedSchedule, title = name));
                Some(items.into_iter().map(|(_, song)| song).collect())
            }
            Err(_) => {
                self.report_file_error(t!(Msg::ScheduleReadCorrupt, name = name));
                None
            }
        }
    }

    pub(crate) fn poll_schedule_files(&mut self) -> bool {
        let mut changed = false;
        // One job is outstanding at a time; the bound is only a backstop.
        for _ in 0..4 {
            let Some(reply) = self
                .file_worker
                .as_ref()
                .and_then(schedule_file::Worker::poll)
            else {
                break;
            };
            changed = true;
            match (self.file_job.take(), reply) {
                (Some(FileJob::Write(job)), schedule_file::Reply::Written(path, result)) => {
                    self.file_written(job, path, result);
                }
                (Some(FileJob::Read), schedule_file::Reply::Read(path, result)) => {
                    self.file_read(path, result);
                }
                (job, _) => self.file_job = job,
            }
        }
        changed
    }

    fn file_written(&mut self, job: SaveJob, path: PathBuf, result: Result<(), WriteError>) {
        let name = schedule_file::display_name(&path);
        match result {
            Ok(()) => {
                // The file is written either way, but a different document may
                // be showing now; it must keep its own title and baseline.
                if job.document == self.schedule.document() {
                    self.schedule.mark_saved(path.clone(), job.pins);
                    self.resume = job.then;
                }
                self.remember_recent(path);
                self.schedule_message = Some(t!(Msg::SavedSchedule, title = name));
            }
            Err(error) => {
                let reason = match error {
                    WriteError::TooLarge => "too large".to_owned(),
                    WriteError::Invalid => "invalid schedule".to_owned(),
                    WriteError::Io(reason) => reason,
                };
                self.report_file_error(t!(Msg::ScheduleSaveFailed, name = name, error = reason));
            }
        }
    }

    fn file_read(&mut self, path: PathBuf, result: Result<ScheduleFile, ReadError>) {
        let name = schedule_file::display_name(&path);
        match result {
            Ok(file) => {
                let arrangements = file.items.iter().map(|item| item.arrangement).collect();
                // The snapshots wait beside the library import (M1-08).
                self.themes.opening = Some((
                    path.clone(),
                    file.items.iter().map(|item| item.theme.clone()).collect(),
                ));
                let items = file
                    .items
                    .into_iter()
                    .map(|item| (item.source, item.song))
                    .collect();
                self.followup = Some((
                    storage::Command::ImportSongs(items),
                    Request::ImportSchedule(path, arrangements),
                ));
            }
            Err(error) => {
                let message = match error {
                    ReadError::NotFound => {
                        self.forget_recent(&path);
                        t!(Msg::ScheduleReadNotFound, name = name)
                    }
                    ReadError::Unreadable(reason) => {
                        t!(Msg::ScheduleReadUnreadable, name = name, error = reason)
                    }
                    ReadError::TooLarge => t!(Msg::ScheduleReadTooLarge, name = name),
                    ReadError::Corrupt => t!(Msg::ScheduleReadCorrupt, name = name),
                    ReadError::Newer(version) => {
                        t!(Msg::ScheduleReadNewer, name = name, version = version)
                    }
                    ReadError::TooManyItems => {
                        t!(Msg::ScheduleReadTooMany, name = name, max = MAX_ITEMS)
                    }
                    ReadError::TitleTooLong => t!(
                        Msg::ScheduleReadTitleLong,
                        name = name,
                        max = schedule_file::MAX_TITLE_BYTES
                    ),
                };
                self.report_file_error(message);
            }
        }
    }

    /// Footer note plus a dialog the operator dismisses; the open schedule and
    /// Live stay as they were.
    pub(crate) fn report_file_error(&mut self, message: String) {
        self.schedule_message = Some(message.clone());
        self.file_error = Some(message);
        self.show_dialog_later(Dialog::FileError);
    }

    fn remember_recent(&mut self, path: PathBuf) {
        self.update_recent(RecentChange::Remember(path));
    }

    fn forget_recent(&mut self, path: &Path) {
        self.update_recent(RecentChange::Forget(path.to_path_buf()));
    }

    /// Continues a guarded command after its Save landed. Runs from `render`,
    /// which has the window `proceed` needs.
    pub(crate) fn resume_guarded(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(then) = self.resume.take() {
            cx.defer_in(window, move |this, window, cx| {
                this.guard(then, window, cx);
                cx.notify();
            });
        }
    }

    /// Title bar: the schedule file name with a dirty marker.
    pub(crate) fn sync_window_title(&mut self, window: &mut Window) {
        let title = match self.schedule.name() {
            Some(name) if self.schedule.is_dirty() => format!("{name}* — Sela"),
            Some(name) => format!("{name} — Sela"),
            None if self.schedule.is_dirty() => "Untitled schedule* — Sela".to_owned(),
            None => "Sela — Technical preview".to_owned(),
        };
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title;
        }
    }
}
