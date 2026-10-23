//! Standalone schedules and profile backups in the operator (M1-13):
//! File › Export Standalone Schedule… (EW8-OBS-053), standalone schedules
//! through File › Open Schedule, and Profiles › Back Up Profile… / Restore
//! Profile… (UNOBSERVED placement). Every file operation runs on
//! `bundle::Worker`; the UI only shows progress and the result. Format and
//! policy: `docs/bundle.md`.
use super::*;
use sela::bundle::{
    self, BackupSummary, Done, ExportSummary, ImportSummary, Job, RestoreSummary, Skip,
    manifest::Kind,
};

/// Names shown in one result line before "and n more".
const LISTED: usize = 8;

/// The job running on the bundle worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Running {
    Export(PathBuf),
    Open(PathBuf),
    Backup(PathBuf),
    Restore(PathBuf),
}

/// What the bundle dialog shows: a heading and result lines (true: error).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BundleView {
    pub(crate) heading: String,
    pub(crate) lines: Vec<(String, bool)>,
    /// A job is running: the only button is Cancel.
    pub(crate) working: bool,
}

#[derive(Default)]
pub(crate) struct Bundles {
    worker: Option<bundle::Worker>,
    pub(crate) running: Option<Running>,
    pub(crate) view: Option<BundleView>,
    /// Lines about the standalone schedule being opened, shown with its
    /// preflight report (or alone when the report is clean), and the
    /// schedule file they belong to.
    pub(crate) opened: Option<(PathBuf, Vec<(String, bool)>)>,
    /// The cancel was requested; the reply is still to come.
    pub(crate) cancelling: bool,
}

impl Bundles {
    pub(crate) fn start(profile: Option<&Path>) -> Self {
        Self {
            worker: profile.and_then(|p| bundle::Worker::start(p.to_path_buf()).ok()),
            ..Self::default()
        }
    }

    /// A job could start now.
    pub(crate) fn ready(&self) -> bool {
        self.worker.as_ref().is_some_and(|w| !w.busy()) && self.running.is_none()
    }

    pub(crate) fn running(&self) -> bool {
        self.running.is_some()
    }

    /// Test hook: a worker that waits before each job while the returned
    /// flag is true, so a job can be caught while it runs.
    #[cfg(test)]
    pub(crate) fn hold(&mut self, profile: &Path) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        let hold = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        self.worker = bundle::Worker::start_gated(
            profile.to_path_buf(),
            sela::fonts::shared(),
            Some(hold.clone()),
        )
        .ok();
        hold
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn size_label(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / MB)
    } else {
        format!("{} KB", bytes.div_ceil(1024).max(1))
    }
}

/// "a, b, c" or "a, b and 3 more".
fn names(list: &[String]) -> String {
    if list.len() <= LISTED {
        return list.join(", ");
    }
    t!(
        Msg::BundleMore,
        names = list[..LISTED].join(", "),
        count = list.len() - LISTED
    )
}

fn error_line(error: &bundle::Error, name: &str) -> String {
    use bundle::Error as E;
    match error {
        E::NotBundle => t!(Msg::BundleErrNotBundle, name = name),
        E::WrongKind(Kind::Profile) => t!(Msg::BundleErrIsBackup, name = name),
        E::WrongKind(Kind::Schedule) => t!(Msg::BundleErrIsSchedule, name = name),
        E::NewerFormat(version) => t!(Msg::BundleErrNewer, name = name, version = version),
        E::NewerLibrary(version) => {
            t!(Msg::BundleErrNewerLibrary, name = name, version = version)
        }
        E::NewerSchedule(version) => {
            t!(Msg::BundleErrNewerSchedule, name = name, version = version)
        }
        E::Unsafe(detail) => t!(Msg::BundleErrUnsafe, name = name, detail = detail),
        E::Damaged(detail) => t!(Msg::BundleErrDamaged, name = name, detail = detail),
        E::TooLarge(detail) => t!(Msg::BundleErrTooLarge, name = name, detail = detail),
        E::DestinationExists => t!(Msg::BundleErrExists, name = name),
        E::Library(error) => t!(Msg::BundleErrLibrary, detail = format!("{error:?}")),
        E::Image(image, error) => {
            t!(
                Msg::BundleErrImage,
                image = image,
                detail = error.to_string()
            )
        }
        E::Empty => t!(Msg::BundleErrEmpty).into(),
        E::DiskFull => t!(Msg::BundleErrDiskFull).into(),
        E::PermissionDenied => t!(Msg::BundleErrDenied).into(),
        E::Io(kind) => t!(Msg::BundleErrIo, detail = kind),
        E::Cancelled => t!(Msg::BundleCancelled).into(),
    }
}

fn export_lines(path: &Path, summary: &ExportSummary) -> Vec<(String, bool)> {
    let mut lines = vec![(
        t!(
            Msg::BundleExported,
            name = file_name(path),
            items = summary.items,
            size = size_label(summary.bytes)
        ),
        false,
    )];
    if !summary.embedded.is_empty() {
        lines.push((
            t!(Msg::BundleEmbedded, names = names(&summary.embedded)),
            false,
        ));
    }
    let missing: Vec<String> = summary
        .not_embedded
        .iter()
        .map(|(name, _)| name.clone())
        .chain(summary.listed_only.iter().cloned())
        .collect();
    if !missing.is_empty() {
        lines.push((t!(Msg::BundleNotEmbedded, names = names(&missing)), true));
    }
    let fonts: Vec<String> = summary.fonts.iter().map(|f| f.family.clone()).collect();
    if !fonts.is_empty() {
        lines.push((t!(Msg::BundleFontsNeeded, names = names(&fonts)), false));
    }
    let absent: Vec<String> = summary
        .fonts
        .iter()
        .filter(|f| f.installed == Some(false))
        .map(|f| f.family.clone())
        .collect();
    if !absent.is_empty() {
        lines.push((
            t!(Msg::BundleFontsMissingHere, names = names(&absent)),
            true,
        ));
    }
    let omitted: Vec<String> = summary.omitted.iter().map(|o| o.title.clone()).collect();
    if !omitted.is_empty() {
        lines.push((t!(Msg::BundleOmitted, names = names(&omitted)), true));
    }
    lines
}

fn open_lines(package: &Path, summary: &ImportSummary) -> Vec<(String, bool)> {
    let mut lines = vec![(
        t!(
            Msg::BundleOpened,
            title = summary.title,
            name = file_name(package),
            items = summary.items,
            file = file_name(&summary.schedule)
        ),
        false,
    )];
    let mut push = |list: Vec<String>, msg: Msg, error: bool| {
        if !list.is_empty() {
            lines.push((t!(msg, names = names(&list)), error));
        }
    };
    push(summary.added.clone(), Msg::BundleAdded, false);
    push(summary.reused.clone(), Msg::BundleReused, false);
    push(
        summary
            .renamed
            .iter()
            .map(|(from, to)| format!("{from} → {to}"))
            .collect(),
        Msg::BundleRenamed,
        false,
    );
    push(summary.not_included.clone(), Msg::BundleNotIncluded, true);
    push(
        summary.fonts.iter().map(|f| f.family.clone()).collect(),
        Msg::BundleFontsNeeded,
        false,
    );
    push(
        summary.omitted.iter().map(|o| o.title.clone()).collect(),
        Msg::BundleOmittedBySender,
        true,
    );
    lines
}

fn skip_label(path: &str, why: Skip) -> String {
    let reason = t!(match why {
        Skip::Link => Msg::BundleSkipLink,
        Skip::TooLarge => Msg::BundleSkipTooLarge,
        Skip::Unusable => Msg::BundleSkipUnusable,
        Skip::Unreadable => Msg::BundleSkipUnreadable,
    });
    format!("{path} ({reason})")
}

fn backup_lines(path: &Path, summary: &BackupSummary, unsaved: bool) -> Vec<(String, bool)> {
    let mut lines = vec![
        (
            t!(
                Msg::BundleBackedUp,
                name = file_name(path),
                files = summary.files,
                size = size_label(summary.bytes)
            ),
            false,
        ),
        (t!(Msg::BundleBackupExcluded).into(), false),
    ];
    if !summary.skipped.is_empty() {
        let list: Vec<String> = summary
            .skipped
            .iter()
            .map(|(path, why)| skip_label(path, *why))
            .collect();
        let mut text = names(&list);
        if summary.more_skipped > 0 {
            text = t!(Msg::BundleMore, names = text, count = summary.more_skipped);
        }
        lines.push((t!(Msg::BundleSkipped, names = text), true));
    }
    if summary.recent_dropped > 0 {
        lines.push((
            t!(Msg::BundleRecentDropped, count = summary.recent_dropped),
            false,
        ));
    }
    if unsaved {
        lines.push((t!(Msg::BundleUnsavedNotInBackup).into(), true));
    }
    lines
}

fn restore_lines(summary: &RestoreSummary) -> Vec<(String, bool)> {
    let mut lines = vec![
        (
            t!(
                Msg::BundleRestored,
                folder = summary.destination.display(),
                files = summary.files
            ),
            false,
        ),
        (
            t!(
                Msg::BundleRestoreOpen,
                library = bundle::library_in(&summary.destination).display()
            ),
            false,
        ),
    ];
    if summary.settings_skipped {
        lines.push((t!(Msg::BundleSettingsSkipped).into(), true));
    }
    lines
}

impl Operator {
    fn bundle_heading(running: &Running) -> String {
        t!(match running {
            Running::Export(_) => Msg::BundleExportHeading,
            Running::Open(_) => Msg::BundleOpenHeading,
            Running::Backup(_) => Msg::BundleBackupHeading,
            Running::Restore(_) => Msg::BundleRestoreHeading,
        })
        .into()
    }

    /// Hands `job` to the worker and shows the working dialog.
    fn start_bundle_job(&mut self, running: Running, job: Job) {
        if !self.bundles.ready() {
            self.schedule_message = Some(t!(Msg::BundleBusy).into());
            return;
        }
        let Some(worker) = self.bundles.worker.as_mut() else {
            self.schedule_message = Some(t!(Msg::BundleNoProfile).into());
            return;
        };
        if !worker.submit(job) {
            self.schedule_message = Some(t!(Msg::BundleBusy).into());
            return;
        }
        let working = match &running {
            Running::Export(path) => t!(Msg::BundleWorkingExport, name = file_name(path)),
            Running::Open(path) => t!(Msg::BundleWorkingOpen, name = file_name(path)),
            Running::Backup(path) => t!(Msg::BundleWorkingBackup, name = file_name(path)),
            Running::Restore(path) => t!(Msg::BundleWorkingRestore, name = file_name(path)),
        };
        self.schedule_message = Some(working.clone());
        self.bundles.view = Some(BundleView {
            heading: Self::bundle_heading(&running),
            lines: vec![(working, false), (t!(Msg::BundleWorkingNote).into(), false)],
            working: true,
        });
        self.bundles.running = Some(running);
        self.bundles.cancelling = false;
        if self.dialog.is_none() {
            self.show_dialog_later(Dialog::Bundle);
        }
    }

    /// The working dialog's Cancel: the job stops at its next check and
    /// replies `Cancelled`; nothing it wrote stays behind.
    pub(crate) fn cancel_bundle_job(&mut self) {
        if let (Some(worker), Some(_)) = (&self.bundles.worker, &self.bundles.running) {
            worker.cancel();
            self.bundles.cancelling = true;
            self.schedule_message = Some(t!(Msg::BundleCancelling).into());
        }
        self.bundles.view = None;
    }

    pub(crate) fn bundle_enabled(&self, schedule_needed: bool) -> bool {
        self.bundles.ready()
            && self.profile.is_some()
            && (!schedule_needed || !self.schedule.entries().is_empty())
    }

    /// File › Export Standalone Schedule…: the native Save dialog, then the
    /// same song read a schedule save does, then the bundle worker.
    pub(crate) fn prompt_export_standalone(&mut self, cx: &mut Context<Self>) {
        if self.worker.is_none() {
            self.schedule_message = Some(t!(Msg::NoLibraryOpen).into());
            return;
        }
        if self.schedule_busy() || !self.bundles.ready() {
            self.schedule_message = Some(t!(Msg::BundleBusy).into());
            return;
        }
        let suggested = format!(
            "{}.{}",
            self.schedule
                .name()
                .map(|name| {
                    Path::new(&name)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or(name)
                })
                .unwrap_or_else(|| "Schedule".into()),
            bundle::SCHEDULE_EXTENSION
        );
        let picked = cx.prompt_for_new_path(&self.bundle_dir(), Some(&suggested));
        self.file_prompt = Some(cx.spawn(async move |this, cx| {
            let picked = picked.await;
            let _ = this.update(cx, |this, cx| {
                this.finish_file_prompt();
                if let Ok(Ok(Some(path))) = picked {
                    let path = bundle::with_extension(&path, bundle::SCHEDULE_EXTENSION);
                    this.begin_export(path);
                }
                cx.notify();
            });
        }));
    }

    /// Where the bundle dialogs start: beside the current schedule file,
    /// else the profile's `Schedules` folder, else the profile's parent.
    fn bundle_dir(&self) -> PathBuf {
        self.schedule
            .path()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| {
                self.profile
                    .as_ref()
                    .map(|p| p.join("Schedules"))
                    .filter(|p| p.is_dir())
            })
            .or_else(|| {
                self.profile
                    .as_deref()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
            })
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// The backup dialogs start beside the profile folder (a backup inside
    /// the profile would be carried by the next one).
    fn backup_dir(&self) -> PathBuf {
        self.profile
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// A standalone schedule picked in File › Open Schedule.
    pub(crate) fn begin_package_open(&mut self, path: PathBuf) {
        if self.profile.is_none() {
            self.schedule_message = Some(t!(Msg::BundleNoProfile).into());
            return;
        }
        self.bundles.opened = None;
        self.start_bundle_job(
            Running::Open(path.clone()),
            Job::ImportSchedule { package: path },
        );
    }

    /// The songs of the schedule arrived (`write_schedule` for a package).
    pub(crate) fn export_document(&mut self, path: PathBuf, file: schedule_file::ScheduleFile) {
        self.start_bundle_job(
            Running::Export(path.clone()),
            Job::ExportSchedule {
                file: Box::new(file),
                destination: path,
                options: bundle::ExportOptions::default(),
            },
        );
    }

    /// Profiles › Back Up Profile….
    pub(crate) fn prompt_backup_profile(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.profile.clone() else {
            self.schedule_message = Some(t!(Msg::BundleNoProfile).into());
            return;
        };
        if !self.bundles.ready() || self.file_prompt.is_some() {
            self.schedule_message = Some(t!(Msg::BundleBusy).into());
            return;
        }
        let stem = profile
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "sela".into());
        let suggested = format!(
            "{stem} {}.{}",
            chrono::Local::now().format("%Y-%m-%d"),
            bundle::BACKUP_EXTENSION
        );
        let picked = cx.prompt_for_new_path(&self.backup_dir(), Some(&suggested));
        self.file_prompt = Some(cx.spawn(async move |this, cx| {
            let picked = picked.await;
            let _ = this.update(cx, |this, cx| {
                this.finish_file_prompt();
                if let Ok(Ok(Some(path))) = picked {
                    let path = bundle::with_extension(&path, bundle::BACKUP_EXTENSION);
                    this.start_bundle_job(
                        Running::Backup(path.clone()),
                        Job::BackupProfile { destination: path },
                    );
                }
                cx.notify();
            });
        }));
    }

    /// Profiles › Restore Profile…: the backup file, then the name of the
    /// new profile folder.
    pub(crate) fn prompt_restore_profile(&mut self, cx: &mut Context<Self>) {
        if self.profile.is_none() {
            self.schedule_message = Some(t!(Msg::BundleNoProfile).into());
            return;
        }
        if !self.bundles.ready() || self.file_prompt.is_some() {
            self.schedule_message = Some(t!(Msg::BundleBusy).into());
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t!(Msg::BundlePickBackup).into()),
        });
        self.file_prompt = Some(cx.spawn(async move |this, cx| {
            let picked = picked.await;
            let _ = this.update(cx, |this, cx| {
                this.finish_file_prompt();
                if let Ok(Ok(Some(paths))) = picked
                    && let Some(backup) = paths.into_iter().next()
                {
                    this.prompt_restore_destination(backup, cx);
                }
                cx.notify();
            });
        }));
    }

    fn prompt_restore_destination(&mut self, backup: PathBuf, cx: &mut Context<Self>) {
        let stem = backup
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "sela".into());
        let suggested = format!("{stem} restored");
        let start = backup
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.backup_dir());
        let picked = cx.prompt_for_new_path(&start, Some(&suggested));
        self.file_prompt = Some(cx.spawn(async move |this, cx| {
            let picked = picked.await;
            let _ = this.update(cx, |this, cx| {
                this.finish_file_prompt();
                if let Ok(Ok(Some(destination))) = picked {
                    this.start_bundle_job(
                        Running::Restore(destination.clone()),
                        Job::RestoreProfile {
                            backup,
                            destination,
                        },
                    );
                }
                cx.notify();
            });
        }));
    }

    fn finish_file_prompt(&mut self) {
        if let Some(task) = self.file_prompt.take() {
            task.detach();
        }
    }

    /// Shows a finished job's result in the bundle dialog.
    fn show_bundle_result(&mut self, running: &Running, lines: Vec<(String, bool)>) {
        self.bundles.view = Some(BundleView {
            heading: Self::bundle_heading(running),
            lines,
            working: false,
        });
        if self.dialog.is_none() {
            self.show_dialog_later(Dialog::Bundle);
        }
    }

    pub(crate) fn poll_bundles(&mut self) -> bool {
        let Some(done) = self.bundles.worker.as_mut().and_then(bundle::Worker::poll) else {
            return false;
        };
        let Some(running) = self.bundles.running.take() else {
            return true;
        };
        self.bundles.cancelling = false;
        // A working view is replaced by the result (or dropped on cancel).
        // A job that finished before it saw the cancel shows its result:
        // the file exists, so "nothing was changed" would be untrue.
        let showing = self.dialog == Some(Dialog::Bundle);
        let failed = |this: &mut Self, error: bundle::Error, path: &Path, unchanged: bool| {
            if error == bundle::Error::Cancelled {
                this.bundles.view = None;
                if showing {
                    this.dialog = None;
                }
                this.schedule_message = Some(t!(Msg::BundleCancelled).into());
                return;
            }
            let mut lines = vec![(error_line(&error, &file_name(path)), true)];
            if unchanged {
                lines.push((t!(Msg::BundleUnchanged).into(), false));
            }
            this.schedule_message = Some(lines[0].0.clone());
            this.show_bundle_result(&running, lines);
        };
        match done {
            Done::Exported(path, Ok(summary)) => {
                self.schedule_message = Some(t!(
                    Msg::BundleExported,
                    name = file_name(&path),
                    items = summary.items,
                    size = size_label(summary.bytes)
                ));
                self.show_bundle_result(&running, export_lines(&path, &summary));
            }
            Done::Exported(path, Err(error)) => failed(self, error, &path, false),
            Done::Imported(package, Ok(summary)) => {
                // The schedule file is new and complete; it opens like any
                // other, and the preflight report follows.
                self.bundles.view = None;
                if showing {
                    self.dialog = None;
                }
                let lines = open_lines(&package, &summary);
                self.bundles.opened = Some((summary.schedule.clone(), lines));
                self.open_recent(summary.schedule);
            }
            Done::Imported(package, Err(error)) => failed(self, error, &package, true),
            Done::BackedUp(path, Ok(summary)) => {
                let lines = backup_lines(&path, &summary, self.schedule.is_dirty());
                self.schedule_message = Some(lines[0].0.clone());
                self.show_bundle_result(&running, lines);
            }
            Done::BackedUp(path, Err(error)) => failed(self, error, &path, false),
            Done::Restored(_, Ok(summary)) => {
                let lines = restore_lines(&summary);
                self.schedule_message = Some(lines[0].0.clone());
                self.show_bundle_result(&running, lines);
            }
            Done::Restored(path, Err(error)) => failed(self, error, &path, true),
        }
        true
    }

    /// After the opened package's preflight: with nothing to review, its
    /// notes show on their own.
    pub(crate) fn show_package_notes_if_clean(&mut self) {
        let Some((path, lines)) = self.bundles.opened.clone() else {
            return;
        };
        if self.schedule.path() != Some(path.as_path()) {
            self.bundles.opened = None;
            return;
        }
        self.bundles.opened = None;
        self.show_bundle_result(&Running::Open(path), lines);
    }

    /// The package's notes inside the preflight report, while that report is
    /// about the schedule just opened from it.
    pub(crate) fn package_notes(&self) -> Option<&[(String, bool)]> {
        let (path, lines) = self.bundles.opened.as_ref()?;
        (self.schedule.path() == Some(path.as_path())).then_some(lines.as_slice())
    }

    pub(crate) fn bundle_body(&self) -> gpui::AnyElement {
        let lines = self
            .bundles
            .view
            .as_ref()
            .map(|view| view.lines.clone())
            .unwrap_or_default();
        bundle_lines("bundle-result", &lines)
    }
}

/// Result lines as one labelled list (errors in the error color).
pub(crate) fn bundle_lines(id: &'static str, lines: &[(String, bool)]) -> gpui::AnyElement {
    let mut list = div()
        .id(id)
        .debug_selector(move || id.into())
        .role(Role::List)
        .accessibility_id(id)
        .max_h(px(280.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_1();
    for (n, (text, error)) in lines.iter().enumerate() {
        let line_id = format!("{id}-{n}");
        list = list.child(
            div()
                .id((id, n))
                .debug_selector({
                    let line_id = line_id.clone();
                    move || line_id.clone()
                })
                // A list item, so assistive technology reads the line itself
                // (an AccessKit Label exposes no name over AT-SPI here).
                .role(Role::ListItem)
                .aria_label(text.clone())
                .accessibility_id(line_id)
                .text_size(px(TEXT_SMALL))
                .text_color(rgb(if *error { ERROR } else { MUTED }))
                .child(text.clone()),
        );
    }
    list.into_any_element()
}
