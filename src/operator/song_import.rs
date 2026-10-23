//! File › Import › Songs… / Song Folder… (M1-14, M4-07 OpenSong/ChordPro)
//! and EasyWorship Library… (M4-07 EasyWorship 6-8 child):
//! a native file chooser that never blocks, the bounded `sela-import`
//! worker (`sela::import::worker`) that reads and stages each file, a staged
//! preview listing every song with its warnings and every file that failed,
//! a duplicate check against the library, and one storage transaction for
//! everything the operator accepts. Cancel at any point commits nothing.
//!
//! Where EasyWorship 8.0.49 offers library import is UNOBSERVED (the File
//! menu in EW8-OBS-053 has no Import row); the menu placement, duplicate
//! policy and dialog are best guesses recorded in
//! docs/reference-observations.md and docs/parity-pending.md. An EasyWorship
//! library is read from private copies of its `Songs.db` and `SongWords.db`
//! (`sela::import::ew_library`), one preview row per song.
use super::*;
use sela::import::{
    Rejected, Warning,
    batch::{self, FileError, Format, Staged},
    ew_library::{self, LibraryError},
    worker::{Event, Importer, Pace, Source, SubmitError},
};
use sela::storage::{ImportWrite, LibraryMatch};

actions!(operator, [ImportChoiceNext, ImportChoicePrevious]);

/// Tab band of the preview's choice groups: after the schedule rows, before
/// the dialog buttons (`DIALOG_TAB_INDEX`). Equal indices keep row order.
const CHOICE_TAB_INDEX: isize = DIALOG_TAB_INDEX - 100;
/// Worker events handled per poll; the rest wait for the next tick.
const EVENTS_PER_POLL: usize = 64;
pub(crate) const IMPORT_DIALOG_W: f32 = 640.;

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("right", ImportChoiceNext, Some("SelaImportChoice")),
        KeyBinding::new("down", ImportChoiceNext, Some("SelaImportChoice")),
        KeyBinding::new("left", ImportChoicePrevious, Some("SelaImportChoice")),
        KeyBinding::new("up", ImportChoicePrevious, Some("SelaImportChoice")),
    ]);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Phase {
    #[default]
    Idle,
    /// The native file chooser is open.
    Choosing,
    /// The worker is reading files; rows appear as they are staged.
    Reading,
    /// The storage worker looks for library duplicates.
    Checking,
    /// The operator chooses what to import.
    Review,
    /// The one import transaction is running.
    Committing,
    /// Cancel was pressed during the commit; the dialog is closed and the
    /// storage reply decides what the footer reports.
    Cancelling,
}

/// What happens to one staged song.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    /// Add as a new song ("Import as copy" for a duplicate).
    Import,
    Skip,
    /// Save as a new revision of the library song it duplicates.
    Replace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Duplicate {
    Library(LibraryMatch),
    /// The same title and words as an earlier row of this import.
    Batch(usize),
}

pub(crate) struct Row {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) result: Result<Box<Staged>, FileError>,
    pub(crate) duplicate: Option<Duplicate>,
    pub(crate) choice: Choice,
}

impl Row {
    /// Choices offered, in display order; none for a failed file.
    pub(crate) fn choices(&self) -> &'static [Choice] {
        match (&self.result, &self.duplicate) {
            (Err(_), _) => &[],
            (Ok(_), None) => &[Choice::Import, Choice::Skip],
            (Ok(_), Some(Duplicate::Library(_))) => {
                &[Choice::Skip, Choice::Import, Choice::Replace]
            }
            (Ok(_), Some(Duplicate::Batch(_))) => &[Choice::Skip, Choice::Import],
        }
    }
}

/// Whole-import state; one import runs at a time.
#[derive(Default)]
pub(crate) struct SongImport {
    importer: Option<Importer>,
    /// Test hook handed to the worker when it starts.
    pub(crate) pace: Option<Pace>,
    job: u64,
    pub(crate) phase: Phase,
    pub(crate) rows: Vec<Row>,
    /// Files the worker will report; `None` until it has expanded folders.
    pub(crate) total: Option<usize>,
    pub(crate) ignored: usize,
    pub(crate) over_limit: usize,
    /// Chosen folders the walk cut at `batch::MAX_FOLDER_ENTRIES`.
    pub(crate) truncated: Vec<PathBuf>,
    /// This import reads an EasyWorship library: rows are songs, not files.
    pub(crate) library: bool,
    /// Shown in the dialog (commit failure, replace conflict).
    pub(crate) error: Option<String>,
    /// Quit or window close asked while the preview is open. Its own slot,
    /// so it never hides `error` (e.g. a failed duplicate check).
    pub(crate) quit_note: Option<String>,
    /// A storage command waiting for the storage worker.
    storage: Option<storage::Command>,
    storage_cancel: Option<storage::Cancel>,
    /// Imported song to scroll to and focus once the Songs list has it.
    pub(crate) reveal: Option<Version>,
    close_pending: bool,
    prompt: Option<Task<()>>,
    choice_focus: Vec<FocusHandle>,
    bulk_focus: Option<FocusHandle>,
    /// The row list; keyboard focus on a row's choices scrolls it into view.
    pub(crate) list_scroll: ScrollHandle,
    /// Row whose choice group had keyboard focus at the last render.
    focused_row: Option<usize>,
    /// Saved versions of the last commit, in write order (tests and
    /// selection).
    pub(crate) imported: Vec<Version>,
}

impl SongImport {
    pub(crate) fn active(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// The preview dialog owns this import's state: closing the window or
    /// replacing the dialog must wait for Import or Cancel. The chooser
    /// (`Choosing`) and a cancelled commit (`Cancelling`) have no dialog.
    pub(crate) fn holds_dialog(&self) -> bool {
        matches!(
            self.phase,
            Phase::Reading | Phase::Checking | Phase::Review | Phase::Committing
        )
    }

    /// The notes under the summary: ignored files, files past the batch
    /// limit and folders cut at the entry budget.
    pub(crate) fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        if self.ignored > 0 {
            notes.push(t!(Msg::ImportIgnored, count = self.ignored));
        }
        if self.over_limit > 0 {
            notes.push(t!(
                if self.library {
                    Msg::ImportSongsOverLimit
                } else {
                    Msg::ImportOverLimit
                },
                count = self.over_limit,
                max = batch::MAX_FILES
            ));
        }
        for folder in &self.truncated {
            notes.push(t!(
                Msg::ImportFolderTruncated,
                max = batch::MAX_FOLDER_ENTRIES,
                name = folder.display()
            ));
        }
        notes
    }

    pub(crate) fn staged(&self) -> usize {
        self.rows.iter().filter(|r| r.result.is_ok()).count()
    }

    pub(crate) fn failed(&self) -> usize {
        self.rows.iter().filter(|r| r.result.is_err()).count()
    }

    pub(crate) fn duplicates(&self) -> usize {
        self.rows.iter().filter(|r| r.duplicate.is_some()).count()
    }

    /// Rows the commit would write.
    pub(crate) fn writes(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| r.result.is_ok() && r.choice != Choice::Skip)
            .count()
    }

    /// The storage command for the next free storage slot, with its job.
    pub(crate) fn take_storage(&mut self) -> Option<(storage::Command, u64)> {
        self.storage.take().map(|command| (command, self.job))
    }

    /// Number of the current import job.
    #[cfg(test)]
    pub(crate) fn job(&self) -> u64 {
        self.job
    }

    /// The `sela-import` thread still has a job that has not reported Done.
    #[cfg(test)]
    pub(crate) fn worker_busy(&self) -> bool {
        self.importer.as_ref().is_some_and(Importer::busy)
    }

    /// The commit is waiting for the storage worker, not yet handed over.
    #[cfg(test)]
    pub(crate) fn commit_queued(&self) -> bool {
        matches!(self.storage, Some(storage::Command::CommitImport(_)))
    }

    pub(crate) fn hold_cancel(&mut self, cancel: storage::Cancel) {
        self.storage_cancel = Some(cancel);
    }

    fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.rows.clear();
        self.total = None;
        self.ignored = 0;
        self.over_limit = 0;
        self.truncated.clear();
        self.library = false;
        self.error = None;
        self.quit_note = None;
        self.storage = None;
        self.storage_cancel = None;
    }
}

/// Earlier rows with the same title and words, or the same source id (an
/// EasyWorship `song_uid` listed twice): the first one keeps its choice,
/// later ones point at it.
pub(crate) fn batch_duplicates(rows: &[Row]) -> Vec<Option<usize>> {
    let mut first: HashMap<&batch::DuplicateKey, usize> = HashMap::new();
    let mut ids: HashMap<&str, usize> = HashMap::new();
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let staged = row.result.as_ref().ok()?;
            let by_id = staged
                .source_id
                .as_deref()
                .and_then(|id| ids.get(id).copied());
            let by_key = first.get(&staged.key).copied();
            if let Some(id) = staged.source_id.as_deref() {
                ids.entry(id).or_insert(index);
            }
            first.entry(&staged.key).or_insert(index);
            match (by_id, by_key) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (earlier, None) | (None, earlier) => earlier,
            }
        })
        .collect()
}

pub(crate) fn format_name(format: Format) -> &'static str {
    match format {
        Format::OpenLyrics => "OpenLyrics",
        Format::OpenSong => "OpenSong",
        Format::ChordPro => "ChordPro",
        Format::EasyWorship => "EasyWorship",
    }
}

/// One warning, as the staged preview shows it.
pub(crate) fn warning_text(warning: &Warning) -> String {
    match warning {
        Warning::UnknownElement(name) => t!(Msg::ImportWarnElement, name = name),
        Warning::UnknownDirective(name) => t!(Msg::ImportWarnDirective, name = name),
        Warning::MissingOrderReference(name) => t!(Msg::ImportWarnOrder, name = name),
        Warning::MissingTitle => t!(Msg::ImportWarnTitle).into(),
        Warning::EmptySection(label) => t!(Msg::ImportWarnEmpty, label = label),
        Warning::ChordsRemoved => t!(Msg::ImportWarnChords).into(),
        Warning::Truncated(field) => t!(Msg::ImportWarnTruncated, field = field),
        Warning::InvalidField(field) => t!(Msg::ImportWarnInvalid, field = field),
        Warning::LimitReached => t!(Msg::ImportWarnLimit).into(),
        Warning::IgnoredAttribute(name) => t!(Msg::ImportWarnAttribute, name = name),
    }
}

/// Why a file was not imported.
pub(crate) fn file_error_text(error: &FileError) -> String {
    let reason = match error {
        FileError::Unsupported => return t!(Msg::ImportErrUnsupported).into(),
        FileError::Unreadable(error) => return t!(Msg::ImportErrUnreadable, error = error),
        FileError::TooLargeForLibrary => return t!(Msg::ImportErrLibraryLimit).into(),
        FileError::MissingWords => return t!(Msg::ImportErrMissingWords).into(),
        FileError::Library(error) => return library_error_text(error),
        FileError::BatchFull => return t!(Msg::ImportErrBatchFull, max = batch::MAX_FILES),
        FileError::BatchTooLarge => {
            return t!(
                Msg::ImportErrBatchBytes,
                max = batch::MAX_STAGED_BYTES / (1024 * 1024)
            );
        }
        FileError::Rejected(_, reason) => reason,
    };
    match reason {
        Rejected::TooLarge => t!(Msg::ImportErrTooLarge).into(),
        Rejected::InvalidUtf8 => t!(Msg::ImportErrEncoding).into(),
        Rejected::Malformed(error) => t!(Msg::ImportErrMalformed, error = error),
        Rejected::Entities => t!(Msg::ImportErrEntities).into(),
        Rejected::TooDeep => t!(Msg::ImportErrTooDeep).into(),
        Rejected::WrongFormat => t!(Msg::ImportErrUnsupported).into(),
        Rejected::NoLyrics => t!(Msg::ImportErrNoLyrics).into(),
    }
}

/// Why an EasyWorship library gave no songs.
pub(crate) fn library_error_text(error: &LibraryError) -> String {
    match error {
        LibraryError::NoSongsDb => t!(Msg::ImportEwNoSongs).into(),
        LibraryError::NoWordsDb => t!(Msg::ImportEwNoWords).into(),
        LibraryError::TooLarge => t!(
            Msg::ImportEwTooLarge,
            max = ew_library::MAX_DB_BYTES / (1024 * 1024)
        ),
        LibraryError::Changed => t!(Msg::ImportEwChanged).into(),
        LibraryError::Unreadable(error) => t!(Msg::ImportErrUnreadable, error = error),
        LibraryError::CopyFailed(error) => t!(Msg::ImportEwCopyFailed, error = error),
        LibraryError::NotLibrary(error) => t!(Msg::ImportEwNotLibrary, error = error),
        // The worker ends a cancelled job with Done; no row says this.
        LibraryError::Canceled => t!(Msg::ImportCancelled).into(),
    }
}

fn choice_label(choice: Choice, duplicate: bool) -> &'static str {
    match choice {
        Choice::Import if duplicate => t!(Msg::ImportChoiceCopy),
        Choice::Import => t!(Msg::ImportChoiceImport),
        Choice::Skip => t!(Msg::ImportChoiceSkip),
        Choice::Replace => t!(Msg::ImportChoiceReplace),
    }
}

fn choice_id(choice: Choice) -> &'static str {
    match choice {
        Choice::Import => "import",
        Choice::Skip => "skip",
        Choice::Replace => "replace",
    }
}

impl Operator {
    /// File › Import › Songs… (`folder: false`) or Song Folder… (`true`).
    pub(crate) fn begin_song_import(&mut self, folder: bool, cx: &mut Context<Self>) {
        if self.worker.is_none() {
            self.song_message = Some((t!(Msg::NoLibraryOpen).into(), true));
            return;
        }
        if self.song_import.active() {
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: !folder,
            directories: folder,
            multiple: true,
            prompt: Some(t!(Msg::ImportPrompt).into()),
        });
        self.song_import.phase = Phase::Choosing;
        self.song_import.prompt = Some(cx.spawn(async move |this, cx| {
            let picked = picked.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(task) = this.song_import.prompt.take() {
                    task.detach();
                }
                match picked {
                    Ok(Ok(Some(paths))) if !paths.is_empty() => this.start_song_import(paths),
                    Ok(Err(error)) => {
                        this.song_import.phase = Phase::Idle;
                        this.song_message =
                            Some((t!(Msg::ImportFailedStart, error = error.to_string()), true));
                    }
                    // Cancelled chooser: nothing happens.
                    _ => this.song_import.phase = Phase::Idle,
                }
                cx.notify();
            });
        }));
    }

    /// File › Import › EasyWorship Library…: one folder, the EasyWorship
    /// profile or the data folder that holds `Songs.db` (UNOBSERVED where
    /// EW keeps it; `ew_library::DATA_FOLDERS`).
    pub(crate) fn begin_ew_library_import(&mut self, cx: &mut Context<Self>) {
        if self.worker.is_none() {
            self.song_message = Some((t!(Msg::NoLibraryOpen).into(), true));
            return;
        }
        if self.song_import.active() {
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!(Msg::ImportPrompt).into()),
        });
        self.song_import.phase = Phase::Choosing;
        self.song_import.prompt = Some(cx.spawn(async move |this, cx| {
            let picked = picked.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(task) = this.song_import.prompt.take() {
                    task.detach();
                }
                match picked {
                    Ok(Ok(Some(mut paths))) if !paths.is_empty() => {
                        let source = Source::EasyWorship {
                            folder: paths.swap_remove(0),
                            known_titles: this
                                .catalog
                                .iter()
                                .map(|(_, title)| title.clone())
                                .collect(),
                        };
                        this.start_song_import_from(source)
                    }
                    Ok(Err(error)) => {
                        this.song_import.phase = Phase::Idle;
                        this.song_message =
                            Some((t!(Msg::ImportFailedStart, error = error.to_string()), true));
                    }
                    _ => this.song_import.phase = Phase::Idle,
                }
                cx.notify();
            });
        }));
    }

    /// Reads `paths` on the `sela-import` thread and opens the preview.
    pub(crate) fn start_song_import(&mut self, paths: Vec<PathBuf>) {
        self.start_song_import_from(Source::Paths(paths));
    }

    /// Reads `source` on the `sela-import` thread and opens the preview.
    pub(crate) fn start_song_import_from(&mut self, source: Source) {
        if self.song_import.importer.is_none() {
            match Importer::start_paced(self.song_import.pace.clone()) {
                Ok(importer) => self.song_import.importer = Some(importer),
                Err(error) => {
                    self.song_import.phase = Phase::Idle;
                    self.song_message =
                        Some((t!(Msg::ImportFailedStart, error = error.to_string()), true));
                    return;
                }
            }
        }
        self.song_import.reset();
        self.song_import.library = matches!(source, Source::EasyWorship { .. });
        self.song_import.job += 1;
        let job = self.song_import.job;
        let importer = self.song_import.importer.as_mut().expect("started above");
        match importer.submit_source(job, source) {
            Ok(()) => {}
            Err(SubmitError::Busy) => {
                self.song_message = Some((t!(Msg::ImportBusy).into(), true));
                return;
            }
            Err(SubmitError::Closed) => {
                // Its thread is gone: the next import starts a new one.
                self.song_import.importer = None;
                self.song_message = Some((t!(Msg::ImportStopped).into(), true));
                return;
            }
        }
        if !self.show_dialog_later(Dialog::ImportSongs) {
            // Another dialog opened while the chooser was up: say so, not
            // "another import".
            if let Some(importer) = &self.song_import.importer {
                importer.cancel();
            }
            self.song_message = Some((t!(Msg::ImportDialogBusy).into(), true));
            return;
        }
        self.song_import.phase = Phase::Reading;
        self.song_message = None;
    }

    /// Worker events, bounded per poll. Returns true when something changed.
    pub(crate) fn poll_song_import(&mut self) -> bool {
        let mut changed = false;
        for _ in 0..EVENTS_PER_POLL {
            let Some(event) = self.song_import.importer.as_mut().and_then(Importer::poll) else {
                break;
            };
            changed = true;
            let current = self.song_import.job;
            let reading = self.song_import.phase == Phase::Reading;
            match event {
                Event::Found {
                    job,
                    total,
                    ignored,
                    over_limit,
                    unreadable,
                    truncated,
                } if job == current && reading => {
                    self.song_import.truncated = truncated;
                    self.song_import.total = Some(total + unreadable.len());
                    self.song_import.ignored = ignored;
                    self.song_import.over_limit = over_limit;
                    for (path, error) in unreadable {
                        self.song_import.rows.push(Row {
                            name: batch::display_name(&path),
                            path,
                            result: Err(FileError::Unreadable(error)),
                            duplicate: None,
                            choice: Choice::Skip,
                        });
                    }
                }
                Event::File {
                    job,
                    path,
                    name,
                    result,
                    ..
                } if job == current && reading => {
                    self.song_import.rows.push(Row {
                        name,
                        path,
                        choice: if result.is_ok() {
                            Choice::Import
                        } else {
                            Choice::Skip
                        },
                        result,
                        duplicate: None,
                    });
                }
                Event::Done { job, canceled } if job == current && reading => {
                    if canceled {
                        self.song_import.reset();
                    } else {
                        self.check_import_duplicates();
                    }
                }
                Event::Failed { job } => {
                    // A new thread for the next import.
                    self.song_import.importer = None;
                    if job == current && reading {
                        self.song_import.reset();
                        self.close_import_dialog_later();
                        self.song_message = Some((t!(Msg::ImportStopped).into(), true));
                    }
                }
                // A cancelled or older job: its results are dropped.
                _ => {}
            }
        }
        changed
    }

    /// Marks duplicates inside the batch, then asks the library about the
    /// rest. Default choices: same words → Skip, same title only → Import
    /// as copy (UNOBSERVED best guess).
    fn check_import_duplicates(&mut self) {
        let inside = batch_duplicates(&self.song_import.rows);
        for (row, earlier) in self.song_import.rows.iter_mut().zip(inside) {
            if let Some(earlier) = earlier {
                row.duplicate = Some(Duplicate::Batch(earlier));
                row.choice = Choice::Skip;
            }
        }
        let keys: Vec<batch::DuplicateKey> = self
            .song_import
            .rows
            .iter()
            .filter_map(|row| row.result.as_ref().ok().map(|s| s.key.clone()))
            .collect();
        if keys.is_empty() {
            self.song_import.phase = Phase::Review;
            return;
        }
        self.song_import.storage = Some(storage::Command::FindDuplicates(keys));
        self.song_import.phase = Phase::Checking;
    }

    /// A storage reply for the import (`Request::SongImport`). A reply for
    /// an earlier job (cancelled while its duplicate check ran) is dropped.
    pub(crate) fn song_import_reply(&mut self, job: u64, reply: Result<Reply, storage::Error>) {
        if job != self.song_import.job {
            return;
        }
        self.song_import.storage_cancel = None;
        match (self.song_import.phase, reply) {
            (Phase::Checking, Ok(Reply::Duplicates(found))) => {
                let mut found = found.into_iter();
                for row in &mut self.song_import.rows {
                    if row.result.is_err() {
                        continue;
                    }
                    let Some(Some(library)) = found.next() else {
                        continue;
                    };
                    if row.duplicate.is_none() {
                        row.choice = if library.same_lyrics {
                            Choice::Skip
                        } else {
                            Choice::Import
                        };
                        row.duplicate = Some(Duplicate::Library(library));
                    }
                }
                self.song_import.phase = Phase::Review;
            }
            (Phase::Checking, Err(error)) => {
                // Without the check every song is offered as new; say so.
                self.song_import.error = Some(t!(
                    Msg::ImportDuplicateCheckFailed,
                    error = format!("{error:?}")
                ));
                self.song_import.phase = Phase::Review;
            }
            (Phase::Committing | Phase::Cancelling, Ok(Reply::Committed(versions))) => {
                let late = self.song_import.phase == Phase::Cancelling;
                self.finish_song_import(versions, late);
            }
            (Phase::Committing | Phase::Cancelling, Err(storage::Error::Canceled)) => {
                self.song_import.reset();
                self.close_import_dialog_later();
                self.song_message = Some((t!(Msg::ImportCancelled).into(), false));
            }
            (Phase::Committing, Err(error)) => {
                // The transaction rolled back: nothing of the batch is saved.
                self.song_import.phase = Phase::Review;
                self.song_import.error =
                    Some(t!(Msg::ImportCommitFailed, error = format!("{error:?}")));
            }
            (Phase::Cancelling, Err(error)) => {
                self.song_import.reset();
                self.song_message = Some((
                    t!(Msg::ImportCommitFailed, error = format!("{error:?}")),
                    true,
                ));
            }
            // The import was cancelled while a duplicate check ran.
            _ => {}
        }
    }

    /// Import button: builds the writes and hands them to storage.
    pub(crate) fn confirm_song_import(&mut self) {
        if self.song_import.phase != Phase::Review {
            return;
        }
        let mut writes = Vec::new();
        let mut replaced: HashMap<sela::storage::Id, String> = HashMap::new();
        for row in &self.song_import.rows {
            let Ok(staged) = &row.result else { continue };
            let replace = match (row.choice, &row.duplicate) {
                (Choice::Skip, _) => continue,
                (Choice::Replace, Some(Duplicate::Library(library))) => {
                    if replaced
                        .insert(library.version.id, library.title.clone())
                        .is_some()
                    {
                        self.song_import.error = Some(t!(
                            Msg::ImportReplaceTwice,
                            title = shown_title(&library.title)
                        ));
                        return;
                    }
                    Some(library.version)
                }
                _ => None,
            };
            writes.push(ImportWrite {
                replace,
                song: staged.song.clone(),
            });
        }
        if writes.is_empty() {
            return;
        }
        self.song_import.error = None;
        self.song_import.storage = Some(storage::Command::CommitImport(writes));
        self.song_import.phase = Phase::Committing;
    }

    /// Cancel button and Esc, in every phase. Before the commit the staged
    /// songs are dropped; during it the storage worker rolls back.
    pub(crate) fn cancel_song_import(&mut self) {
        match self.song_import.phase {
            Phase::Reading => {
                if let Some(importer) = &self.song_import.importer {
                    importer.cancel();
                }
                self.song_import.reset();
                self.song_message = Some((t!(Msg::ImportCancelled).into(), false));
            }
            Phase::Checking | Phase::Review => {
                self.song_import.reset();
                self.song_message = Some((t!(Msg::ImportCancelled).into(), false));
            }
            Phase::Committing => {
                match self.song_import.storage.take() {
                    // Not handed to storage yet: nothing to roll back.
                    Some(_) => {
                        self.song_import.reset();
                        self.song_message = Some((t!(Msg::ImportCancelled).into(), false));
                    }
                    None => {
                        if let Some(cancel) = &self.song_import.storage_cancel {
                            cancel.cancel();
                        }
                        self.song_import.phase = Phase::Cancelling;
                        self.song_message = Some((t!(Msg::ImportCancelling).into(), false));
                    }
                }
            }
            Phase::Idle | Phase::Choosing | Phase::Cancelling => {}
        }
    }

    fn finish_song_import(&mut self, versions: Vec<Version>, late: bool) {
        let skipped = self
            .song_import
            .rows
            .iter()
            .filter(|r| r.result.is_ok() && r.choice == Choice::Skip)
            .count();
        let failed = self.song_import.failed();
        let count = versions.len();
        self.song_import.reset();
        self.song_import.imported = versions.clone();
        if !late {
            self.close_import_dialog_later();
        }
        self.song_message = Some((
            if late {
                t!(Msg::ImportDoneLate, count = count)
            } else {
                t!(
                    Msg::ImportDone,
                    count = count,
                    skipped = skipped,
                    failed = failed
                )
            },
            false,
        ));
        // The imported songs become the Songs selection; Preview loads the
        // first one. Live is untouched.
        if let Some(first) = versions.first().copied() {
            self.song_selection.only(first.id);
            for version in &versions[1..] {
                if !self.song_selection.contains(version.id) {
                    self.song_selection.toggle(version.id);
                }
            }
            self.selected_song = Some(first);
            self.clear_entry_selection();
            self.live_on_load = None;
            self.song_import.reveal = Some(first);
        }
        self.catalog_stale = true;
    }

    /// Test hook: reload the Songs list as Songs › Refresh does.
    #[cfg(test)]
    pub(crate) fn reload_songs(&mut self) {
        self.catalog_stale = true;
    }

    fn close_import_dialog_later(&mut self) {
        if self.dialog == Some(Dialog::ImportSongs) {
            self.song_import.close_pending = true;
        }
    }

    /// From `render`: closes the dialog after a commit and moves keyboard
    /// focus to the first imported song once the Songs list shows it.
    pub(crate) fn song_import_render(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.song_import.close_pending)
            && self.dialog == Some(Dialog::ImportSongs)
        {
            self.close_dialog(window, cx);
        }
        let Some(version) = self.song_import.reveal else {
            return;
        };
        if self.catalog_stale || matches!(self.request, Some(Request::Catalog)) {
            return;
        }
        self.song_import.reveal = None;
        if self.dialog.is_none()
            && self.tab == 0
            && let Some(index) = self.song_index_of(version)
        {
            self.reveal_song(index);
            self.focus_song_row(index, window, cx);
        }
    }

    fn set_import_choice(&mut self, row: usize, choice: Choice) {
        if self.song_import.phase != Phase::Review {
            return;
        }
        if let Some(row) = self.song_import.rows.get_mut(row)
            && row.choices().contains(&choice)
        {
            row.choice = choice;
            self.song_import.error = None;
        }
    }

    fn step_import_choice(&mut self, row: usize, forward: bool) {
        let Some(current) = self.song_import.rows.get(row) else {
            return;
        };
        let choices = current.choices();
        let Some(at) = choices.iter().position(|c| *c == current.choice) else {
            return;
        };
        let next = if forward {
            (at + 1).min(choices.len() - 1)
        } else {
            at.saturating_sub(1)
        };
        self.set_import_choice(row, choices[next]);
    }

    /// The choice every library duplicate shows, if they all agree.
    pub(crate) fn bulk_choice(&self) -> Option<Choice> {
        let mut choices = self
            .song_import
            .rows
            .iter()
            .filter(|r| matches!(r.duplicate, Some(Duplicate::Library(_))))
            .map(|r| r.choice);
        let first = choices.next()?;
        choices.all(|c| c == first).then_some(first)
    }

    /// Applies `choice` to every duplicate that offers it.
    pub(crate) fn set_bulk_choice(&mut self, choice: Choice) {
        for index in 0..self.song_import.rows.len() {
            if self.song_import.rows[index].duplicate.is_some() {
                self.set_import_choice(index, choice);
            }
        }
    }

    fn step_bulk_choice(&mut self, forward: bool) {
        const ORDER: [Choice; 3] = [Choice::Skip, Choice::Import, Choice::Replace];
        let at = self
            .bulk_choice()
            .and_then(|c| ORDER.iter().position(|o| *o == c))
            .unwrap_or(0);
        let next = if forward {
            (at + 1).min(ORDER.len() - 1)
        } else {
            at.saturating_sub(1)
        };
        self.set_bulk_choice(ORDER[next]);
    }

    /// One segmented choice group: a radio group that is one tab stop,
    /// with Left/Right (or Up/Down) moving the choice.
    #[allow(clippy::too_many_arguments)]
    fn choice_group(
        &self,
        row: Option<usize>,
        focus: FocusHandle,
        choices: &'static [Choice],
        current: Option<Choice>,
        duplicate: bool,
        label: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let id: SharedString = match row {
            Some(index) => format!("import-choice-{index}").into(),
            None => "import-bulk".into(),
        };
        let mut segments = Vec::with_capacity(choices.len());
        for &choice in choices {
            let selected = current == Some(choice);
            let segment_id: SharedString = format!("{id}-{}", choice_id(choice)).into();
            let selector = segment_id.clone();
            let focus = focus.clone();
            segments.push(
                div()
                    .id(segment_id.clone())
                    .debug_selector(move || selector.to_string())
                    .role(Role::RadioButton)
                    .aria_label(choice_label(choice, duplicate))
                    .aria_toggled(if selected {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .accessibility_id(segment_id)
                    .px_2()
                    .h(px(CONTROL_H - 4.))
                    .flex()
                    .items_center()
                    .rounded(px(RADIUS_SMALL))
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(if !enabled {
                        DISABLED
                    } else if selected {
                        FOCUS_TEXT
                    } else {
                        MUTED
                    }))
                    .when(selected, |d| {
                        d.bg(rgb(SURFACE))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .when(enabled && !selected, |d| {
                        d.cursor_pointer().hover(|d| d.text_color(rgb(TEXT)))
                    })
                    .when(enabled, |d| {
                        d.on_click(cx.listener(move |this, _, window, cx| {
                            focus.focus(window, cx);
                            match row {
                                Some(index) => this.set_import_choice(index, choice),
                                None => this.set_bulk_choice(choice),
                            }
                            cx.notify();
                        }))
                    })
                    .child(choice_label(choice, duplicate)),
            );
        }
        div()
            .id(id.clone())
            .debug_selector({
                let id = id.clone();
                move || id.to_string()
            })
            .role(Role::RadioGroup)
            .aria_label(label)
            .accessibility_id(id)
            .track_focus(&focus)
            .key_context("SelaImportChoice")
            .on_action(cx.listener(move |this, _: &ImportChoiceNext, _, cx| {
                match row {
                    Some(index) => this.step_import_choice(index, true),
                    None => this.step_bulk_choice(true),
                }
                cx.notify();
            }))
            .on_action(cx.listener(move |this, _: &ImportChoicePrevious, _, cx| {
                match row {
                    Some(index) => this.step_import_choice(index, false),
                    None => this.step_bulk_choice(false),
                }
                cx.notify();
            }))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(RADIUS))
            .bg(rgb(CHROME))
            .border_1()
            .border_color(rgb(CHROME))
            .focus(|d| d.border_color(rgb(ACCENT)))
            .children(segments)
            .into_any_element()
    }

    fn import_row(&self, index: usize, row: &Row, cx: &mut Context<Self>) -> gpui::AnyElement {
        let caption = |text: String, color: u32| {
            div()
                .text_size(px(TEXT_CAPTION))
                .text_color(rgb(color))
                .whitespace_normal()
                .child(text)
        };
        let heading = match &row.result {
            Ok(staged) => shown_title(&staged.song.title),
            Err(_) => row.name.clone(),
        };
        let mut body = div()
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
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(if row.result.is_ok() { TEXT } else { MUTED }))
                            .child(heading.clone()),
                    )
                    .children(row.result.as_ref().ok().map(|staged| {
                        div()
                            .flex_shrink_0()
                            .px_1()
                            .rounded(px(RADIUS_SMALL))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .text_size(px(TEXT_CAPTION))
                            .text_color(rgb(MUTED))
                            .child(format_name(staged.format))
                    })),
            );
        match &row.result {
            Ok(staged) => {
                let slides = staged.song.sections.len();
                body = body.child(caption(
                    t!(Msg::ImportRowDetail, name = row.name, count = slides),
                    MUTED,
                ));
                for warning in &staged.warnings {
                    body = body.child(caption(format!("• {}", warning_text(warning)), MUTED));
                }
            }
            Err(error) => {
                body = body.child(caption(file_error_text(error), ERROR));
            }
        }
        match &row.duplicate {
            Some(Duplicate::Library(library)) => {
                let title = shown_title(&library.title);
                body = body.child(caption(
                    if library.same_lyrics {
                        t!(Msg::ImportDuplicateSame, title = title)
                    } else {
                        t!(Msg::ImportDuplicateTitle, title = title)
                    },
                    TEXT,
                ));
            }
            Some(Duplicate::Batch(earlier)) => {
                let name = self
                    .song_import
                    .rows
                    .get(*earlier)
                    .map(|r| r.name.clone())
                    .unwrap_or_default();
                body = body.child(caption(t!(Msg::ImportDuplicateBatch, name = name), TEXT));
            }
            None => {}
        }
        let review = self.song_import.phase == Phase::Review;
        let choices = row.choices();
        let group = (!choices.is_empty()).then(|| {
            let focus = self.song_import.choice_focus[index].clone();
            self.choice_group(
                Some(index),
                focus,
                choices,
                Some(row.choice),
                row.duplicate.is_some(),
                t!(Msg::ImportChoiceA11y, title = heading.clone()),
                review,
                cx,
            )
        });
        div()
            .id(("import-row", index))
            .debug_selector(move || format!("import-row-{index}"))
            .role(Role::ListItem)
            .aria_label(heading)
            .aria_description(row.path.to_string_lossy().into_owned())
            .accessibility_id(format!("import-row-{index}"))
            .px_2()
            .py(px(6.))
            .flex()
            .items_start()
            .gap_3()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(body)
            .children(group)
            .into_any_element()
    }

    /// The staged preview, drawn over the window while `Dialog::ImportSongs`
    /// is open.
    pub(crate) fn import_overlay(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.dialog != Some(Dialog::ImportSongs) {
            return None;
        }
        // Tab does not scroll by itself: bring a row focused from the
        // keyboard into view. A click must not move the row under the
        // pointer between mouse down and up.
        let focused = self
            .song_import
            .choice_focus
            .iter()
            .take(self.song_import.rows.len())
            .position(|handle| handle.is_focused(window));
        if focused.is_some()
            && focused != self.song_import.focused_row
            && window.last_input_was_keyboard()
        {
            self.song_import
                .list_scroll
                .scroll_to_item(focused.unwrap_or_default());
        }
        self.song_import.focused_row = focused;
        while self.song_import.choice_focus.len() < self.song_import.rows.len() {
            self.song_import
                .choice_focus
                .push(cx.focus_handle().tab_index(CHOICE_TAB_INDEX).tab_stop(true));
        }
        let bulk_focus = self
            .song_import
            .bulk_focus
            .get_or_insert_with(|| {
                cx.focus_handle()
                    .tab_index(CHOICE_TAB_INDEX - 1)
                    .tab_stop(true)
            })
            .clone();
        let import = &self.song_import;
        let phase = import.phase;
        let status = match phase {
            Phase::Reading => Some(match import.total {
                None => t!(Msg::ImportFinding).to_string(),
                Some(total) => t!(
                    Msg::ImportReading,
                    done = import.rows.len().min(total),
                    total = total
                ),
            }),
            Phase::Checking => Some(t!(Msg::ImportChecking).to_string()),
            Phase::Committing => Some(t!(Msg::ImportCommitting).to_string()),
            _ => None,
        };
        let summary = t!(
            Msg::ImportSummary,
            songs = import.staged(),
            failed = import.failed(),
            duplicates = import.duplicates()
        );
        let notes = import.notes();
        let library_duplicates = import
            .rows
            .iter()
            .any(|r| matches!(r.duplicate, Some(Duplicate::Library(_))));
        let writes = import.writes();
        let can_import = phase == Phase::Review && writes > 0;
        let error = import.error.clone();
        let quit_note = import.quit_note.clone();
        let rows: Vec<gpui::AnyElement> = (0..self.song_import.rows.len())
            .map(|index| {
                let row = &self.song_import.rows[index];
                self.import_row(index, row, cx)
            })
            .collect();
        let empty = rows.is_empty() && phase == Phase::Review;
        let bulk = library_duplicates.then(|| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(TEXT_SMALL))
                                .child(t!(Msg::ImportBulkLabel)),
                        )
                        .child(
                            div()
                                .text_size(px(TEXT_CAPTION))
                                .text_color(rgb(MUTED))
                                .whitespace_normal()
                                .child(t!(Msg::ImportReplaceNote)),
                        ),
                )
                .child(self.choice_group(
                    None,
                    bulk_focus,
                    &[Choice::Skip, Choice::Import, Choice::Replace],
                    self.bulk_choice(),
                    true,
                    t!(Msg::ImportBulkLabel).into(),
                    phase == Phase::Review,
                    cx,
                ))
        });
        let confirm = self
            .button(
                Control::DialogConfirm,
                "import-confirm",
                t!(Msg::ImportConfirm, count = writes),
                cx,
            )
            .when(can_import, |d| {
                d.text_color(rgb(FOCUS_TEXT))
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!can_import, |d| mark_disabled(d).text_color(rgb(DISABLED)));
        let panel = div()
            .id("import-dialog")
            .debug_selector(|| "import-dialog".into())
            .role(Role::Dialog)
            .aria_label(t!(Msg::ImportHeading))
            .accessibility_id("import-dialog")
            .max_w_full()
            .w(px(IMPORT_DIALOG_W))
            .p(px(DIALOG_PAD))
            .flex()
            .flex_col()
            .gap(px(DIALOG_GAP))
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE))
            .shadow_md()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!(Msg::ImportHeading)),
                    )
                    .child(
                        div()
                            .id("import-summary")
                            .debug_selector(|| "import-summary".into())
                            .role(Role::Status)
                            .aria_label(summary.clone())
                            .accessibility_id("import-summary")
                            .text_size(px(TEXT_SMALL))
                            .text_color(rgb(MUTED))
                            .child(summary),
                    ),
            )
            .children(status.map(|text| {
                div()
                    .id("import-status")
                    .debug_selector(|| "import-status".into())
                    .role(Role::Status)
                    .aria_label(text.clone())
                    .accessibility_id("import-status")
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(MUTED))
                    .child(text)
            }))
            .children(notes.into_iter().enumerate().map(|(index, note)| {
                div()
                    .id(("import-note", index))
                    .debug_selector(move || format!("import-note-{index}"))
                    .role(Role::Label)
                    // AccessKit names a Label from its value (AT-SPI, UIA).
                    .aria_label(note.clone())
                    .aria_value(note.clone())
                    .accessibility_id(format!("import-note-{index}"))
                    .text_size(px(TEXT_CAPTION))
                    .text_color(rgb(MUTED))
                    .child(note)
            }))
            .children(bulk)
            .child(
                div()
                    .id("import-list")
                    .debug_selector(|| "import-list".into())
                    .role(Role::List)
                    .aria_label(t!(Msg::ImportListA11y))
                    .accessibility_id("import-list")
                    .max_h(px(380.))
                    .overflow_y_scroll()
                    .track_scroll(&self.song_import.list_scroll)
                    .flex()
                    .flex_col()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .when(empty, |d| {
                        d.child(
                            div()
                                .py_2()
                                .text_size(px(TEXT_SMALL))
                                .text_color(rgb(MUTED))
                                .child(t!(Msg::ImportNothing)),
                        )
                    })
                    .children(rows),
            )
            .children(error.map(|message| {
                div()
                    .id("import-error")
                    .debug_selector(|| "import-error".into())
                    .role(Role::Label)
                    .aria_label(message.clone())
                    .aria_value(message.clone())
                    .accessibility_id("import-error")
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(ERROR))
                    .whitespace_normal()
                    .child(message)
            }))
            .children(quit_note.map(|message| {
                div()
                    .id("import-quit-note")
                    .debug_selector(|| "import-quit-note".into())
                    .role(Role::Label)
                    .aria_label(message.clone())
                    .aria_value(message.clone())
                    .accessibility_id("import-quit-note")
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(ERROR))
                    .whitespace_normal()
                    .child(message)
            }))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_1()
                    .child(confirm)
                    .child(self.button(
                        Control::DialogCancel,
                        "import-cancel",
                        t!(Msg::ButtonCancel),
                        cx,
                    )),
            )
            .into_any_element();
        Some(self.modal("import-backdrop", panel, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sela::storage::{Id, Section, Song};

    fn staged(title: &str, lyrics: &str) -> Result<Box<Staged>, FileError> {
        let song = Song {
            title: title.into(),
            authors: String::new(),
            copyright: String::new(),
            license: String::new(),
            administrator: String::new(),
            song_number: String::new(),
            sections: vec![Section {
                id: sela::arrangement::SectionId::allocate(),
                label: String::new(),
                lyrics: lyrics.into(),
                format: Default::default(),
                background: None,
            }],
            variants: Vec::new(),
            master: None,
        };
        Ok(Box::new(Staged {
            format: Format::ChordPro,
            key: batch::duplicate_key(&song),
            bytes: batch::song_bytes(&song),
            song: Arc::new(song),
            warnings: Vec::new(),
            source_id: None,
        }))
    }

    fn staged_with_id(title: &str, lyrics: &str, id: &str) -> Result<Box<Staged>, FileError> {
        let mut staged = staged(title, lyrics)?;
        staged.source_id = Some(id.into());
        Ok(staged)
    }

    fn row(result: Result<Box<Staged>, FileError>) -> Row {
        Row {
            path: PathBuf::from("x"),
            name: "x".into(),
            result,
            duplicate: None,
            choice: Choice::Import,
        }
    }

    #[test]
    fn later_copies_inside_one_import_point_at_the_first() {
        let rows = vec![
            row(staged("A", "one")),
            row(Err(FileError::Unsupported)),
            row(staged("a!", "ONE")),
            row(staged("A", "two")),
            row(staged("A", "one")),
        ];
        assert_eq!(
            batch_duplicates(&rows),
            [None, None, Some(0), None, Some(0)]
        );
    }

    #[test]
    fn a_repeated_easyworship_song_uid_is_a_duplicate_inside_the_import() {
        let rows = vec![
            row(staged_with_id("A", "one", "{1}")),
            row(staged_with_id("B", "other words", "{2}")),
            // Same uid, other title and words: still the same EW song.
            row(staged_with_id("A (edited)", "changed", "{1}")),
            // Same words, new uid: the key still matches.
            row(staged_with_id("b", "OTHER words", "{3}")),
            row(staged_with_id("C", "three", "{4}")),
        ];
        assert_eq!(
            batch_duplicates(&rows),
            [None, None, Some(0), Some(1), None]
        );
    }

    #[test]
    fn choices_follow_the_kind_of_duplicate() {
        let mut r = row(staged("A", "one"));
        assert_eq!(r.choices(), [Choice::Import, Choice::Skip]);
        r.duplicate = Some(Duplicate::Batch(0));
        assert_eq!(r.choices(), [Choice::Skip, Choice::Import]);
        r.duplicate = Some(Duplicate::Library(LibraryMatch {
            version: Version {
                id: Id([1; 16]),
                revision: 1,
            },
            title: "A".into(),
            same_lyrics: true,
        }));
        assert_eq!(r.choices(), [Choice::Skip, Choice::Import, Choice::Replace]);
        assert!(row(Err(FileError::BatchFull)).choices().is_empty());
    }

    #[test]
    fn every_warning_and_error_has_text() {
        for warning in [
            Warning::UnknownElement("key".into()),
            Warning::UnknownDirective("capo".into()),
            Warning::MissingOrderReference("v3".into()),
            Warning::MissingTitle,
            Warning::EmptySection("Verse 2".into()),
            Warning::ChordsRemoved,
            Warning::Truncated("title".into()),
            Warning::InvalidField("ccliNo".into()),
            Warning::LimitReached,
            Warning::IgnoredAttribute("lines/@repeat".into()),
        ] {
            let text = warning_text(&warning);
            assert!(
                !text.is_empty() && !text.contains('{'),
                "{warning:?}: {text}"
            );
        }
        assert!(
            warning_text(&Warning::IgnoredAttribute("lines/@repeat".into()))
                .contains("lines/@repeat")
        );
        for error in [
            FileError::Unsupported,
            FileError::Unreadable("denied".into()),
            FileError::TooLargeForLibrary,
            FileError::BatchFull,
            FileError::BatchTooLarge,
            FileError::Rejected(None, Rejected::TooLarge),
            FileError::Rejected(None, Rejected::InvalidUtf8),
            FileError::Rejected(None, Rejected::Malformed("line 2".into())),
            FileError::Rejected(None, Rejected::Entities),
            FileError::Rejected(None, Rejected::TooDeep),
            FileError::Rejected(None, Rejected::WrongFormat),
            FileError::Rejected(Some(Format::OpenLyrics), Rejected::NoLyrics),
            FileError::MissingWords,
            FileError::Library(LibraryError::NoSongsDb),
            FileError::Library(LibraryError::NoWordsDb),
            FileError::Library(LibraryError::TooLarge),
            FileError::Library(LibraryError::Changed),
            FileError::Library(LibraryError::Unreadable("denied".into())),
            FileError::Library(LibraryError::NotLibrary("no song table".into())),
            FileError::Library(LibraryError::Canceled),
        ] {
            let text = file_error_text(&error);
            assert!(!text.is_empty() && !text.contains('{'), "{error:?}: {text}");
        }
        // Replace warns what the new revision loses (both languages).
        for (note, formatting) in [
            (Msg::ImportReplaceNote.english(), "formatting"),
            (Msg::ImportReplaceNote.indonesian(), "format slide"),
        ] {
            assert!(
                note.contains(formatting) && note.contains("master"),
                "{note}"
            );
        }
        // The byte limit names itself, not the file count.
        let bytes = file_error_text(&FileError::BatchTooLarge);
        assert_ne!(bytes, file_error_text(&FileError::BatchFull));
        assert!(bytes.contains("32"), "{bytes}");
        assert!(!bytes.contains(&batch::MAX_FILES.to_string()), "{bytes}");
    }
}
