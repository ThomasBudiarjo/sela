//! The scripture provider thread (M4-03/M4-04). It owns every translation
//! file: installs the bundled translations on first use, verifies and opens
//! installed ones, reads passages, runs keyword searches and imports user
//! OSIS/USFX/Zefania files. The UI submits commands through a bounded queue
//! and polls replies; it never waits on this thread. Replies carry the
//! caller's generation so a late answer to an older request can be dropped.
use super::bundled::{self, Bundle};
use super::import;
use super::model::{Inventory, MAX_ID, TranslationInfo, VerseId, derive_id, valid_id};
use super::store::{self, MAX_PASSAGE, Translation};
use std::{
    cell::Cell,
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    thread::JoinHandle,
};

/// Commands waiting for the thread at most.
pub const QUEUE: usize = 4;
/// Most hits one keyword search returns.
pub const MAX_HITS: usize = crate::search::MAX_HITS;
/// Most translations listed and kept open.
pub const MAX_TRANSLATIONS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Ready {
        inventory: Arc<Inventory>,
        verses: usize,
    },
    /// Present but unusable, and why.
    Damaged(Damage),
}

/// Why a translation file is unusable, for the UI to phrase in its own
/// language (the strings inside are technical detail only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Damage {
    /// The installed file fails to open or a check.
    Store(store::Error),
    /// The bundled copy shipped with Sela cannot be installed.
    Bundle(bundled::Error),
    /// The file's name does not match the translation inside it.
    FileName,
}

impl std::fmt::Display for Damage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Damage::Store(error) => write!(f, "{error}"),
            Damage::Bundle(error) => write!(f, "{error}"),
            Damage::FileName => write!(f, "file name"),
        }
    }
}

/// One installed translation as listed in the Scriptures tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub info: TranslationInfo,
    pub state: State,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Install missing or damaged bundled translations, then list all.
    Catalog,
    Passage {
        generation: u64,
        translation: String,
        from: VerseId,
        to: VerseId,
    },
    Search {
        generation: u64,
        translation: String,
        query: String,
    },
    Import(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// No translation with that id is installed.
    Unknown(String),
    Unavailable(Damage),
    /// More than `MAX_PASSAGE` verses.
    TooLong,
    Store(store::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unknown(id) => write!(f, "{id} is not installed"),
            Error::Unavailable(why) => write!(f, "{why}"),
            Error::TooLong => write!(f, "more than {MAX_PASSAGE} verses"),
            Error::Store(error) => write!(f, "{error}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    Unreadable(String),
    Rejected(import::Rejected),
    TooMany,
    Store(store::Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Imported {
    pub info: TranslationInfo,
    pub verses: usize,
    pub warnings: Vec<import::Warning>,
}

/// Something the catalog refresh did or could not do, for the UI to phrase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogNote {
    /// A bundled translation file was missing its checks and was rebuilt.
    Rebuilt(String),
    /// A bundled translation could not be installed: id and reason.
    Failed(String, Damage),
    /// The Scripture folder could not be created: the system's reason.
    Folder(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    /// Installed translations, bundled first, then by id. `notes` reports
    /// bundled files that were (re)built or could not be.
    Catalog {
        entries: Vec<Entry>,
        notes: Vec<CatalogNote>,
    },
    Passage {
        generation: u64,
        translation: String,
        result: Result<Vec<(VerseId, String)>, Error>,
    },
    Search {
        generation: u64,
        translation: String,
        result: Result<(Vec<(VerseId, String)>, bool), Error>,
    },
    Imported(Result<Imported, ImportError>),
}

pub struct Worker {
    commands: Option<SyncSender<Command>>,
    replies: Receiver<Reply>,
    thread: Option<JoinHandle<()>>,
    stopped: Cell<bool>,
}

impl Worker {
    /// Starts the thread over `dir` (created when missing) with these
    /// bundles. Nothing is read until the first command.
    pub fn start(dir: PathBuf, bundles: Vec<Bundle>) -> std::io::Result<Worker> {
        let (commands, inbox) = mpsc::sync_channel(QUEUE);
        let (outbox, replies) = mpsc::sync_channel(QUEUE * 2);
        let thread = std::thread::Builder::new()
            .name("sela-scripture".into())
            .spawn(move || {
                let mut provider = Provider {
                    dir,
                    bundles,
                    open: BTreeMap::new(),
                    damaged: BTreeMap::new(),
                };
                for command in inbox {
                    if outbox.send(provider.handle(command)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Worker {
            commands: Some(commands),
            replies,
            thread: Some(thread),
            stopped: Cell::new(false),
        })
    }

    /// Queues a command; `Err` hands it back when the queue is full or the
    /// thread has stopped.
    pub fn submit(&self, command: Command) -> Result<(), Command> {
        match self.commands.as_ref().map(|c| c.try_send(command)) {
            Some(Ok(())) => Ok(()),
            Some(Err(TrySendError::Full(c) | TrySendError::Disconnected(c))) => Err(c),
            None => unreachable!("commands live until drop"),
        }
    }

    /// The next reply, if one is ready. `None` also when the thread stopped;
    /// `stopped` tells the two apart.
    pub fn poll(&self) -> Option<Reply> {
        match self.replies.try_recv() {
            Ok(reply) => Some(reply),
            Err(TryRecvError::Disconnected) => {
                self.stopped.set(true);
                None
            }
            Err(TryRecvError::Empty) => None,
        }
    }

    /// The thread has ended (it panicked or could not continue); seen by
    /// the `poll` that found its reply queue closed.
    pub fn stopped(&self) -> bool {
        self.stopped.get()
    }
}

impl Worker {
    /// Stops the worker and waits for its thread, so every file it opened
    /// is closed when this returns (`Drop` does not wait). It blocks for as
    /// long as the running command takes, so it is for tests only: Windows
    /// refuses to delete a translation file the thread still has open.
    #[doc(hidden)]
    pub fn shut_down(mut self) {
        self.commands.take();
        if let Some(thread) = self.thread.take() {
            while !thread.is_finished() {
                while self.replies.try_recv().is_ok() {}
                std::thread::yield_now();
            }
            thread.join().expect("scripture worker thread panicked");
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.commands.take();
        // Draining lets a thread blocked on a full reply queue finish.
        while self.replies.try_recv().is_ok() {}
        if let Some(thread) = self.thread.take() {
            // A long import may still be running; do not block the UI on it.
            if thread.is_finished() {
                let _ = thread.join();
            }
        }
    }
}

struct Provider {
    dir: PathBuf,
    bundles: Vec<Bundle>,
    open: BTreeMap<String, Translation>,
    damaged: BTreeMap<String, (TranslationInfo, Damage)>,
}

impl Provider {
    fn handle(&mut self, command: Command) -> Reply {
        match command {
            Command::Catalog => {
                let notes = self.refresh();
                Reply::Catalog {
                    entries: self.entries(),
                    notes,
                }
            }
            Command::Passage {
                generation,
                translation,
                from,
                to,
            } => {
                let result = self.translation(&translation).and_then(|t| {
                    let verses = t.passage(from, to).map_err(Error::Store)?;
                    if verses.len() > MAX_PASSAGE {
                        return Err(Error::TooLong);
                    }
                    Ok(verses)
                });
                Reply::Passage {
                    generation,
                    translation,
                    result,
                }
            }
            Command::Search {
                generation,
                translation,
                query,
            } => {
                let result = self
                    .translation(&translation)
                    .and_then(|t| t.search(&query, MAX_HITS).map_err(Error::Store));
                Reply::Search {
                    generation,
                    translation,
                    result,
                }
            }
            Command::Import(path) => Reply::Imported(self.import(&path)),
        }
    }

    fn translation(&self, id: &str) -> Result<&Translation, Error> {
        if let Some((_, why)) = self.damaged.get(id) {
            return Err(Error::Unavailable(why.clone()));
        }
        self.open
            .get(id)
            .ok_or_else(|| Error::Unknown(id.to_owned()))
    }

    /// Installs bundles that are missing, outdated or damaged, then opens
    /// every other `.selabible` file in the folder.
    fn refresh(&mut self) -> Vec<CatalogNote> {
        let mut notes = Vec::new();
        if let Err(error) = std::fs::create_dir_all(&self.dir) {
            notes.push(CatalogNote::Folder(error.to_string()));
            return notes;
        }
        self.open.clear();
        self.damaged.clear();
        for bundle in self.bundles.clone() {
            let path = store::path_for(&self.dir, bundle.id);
            let current = store::open(&path)
                .ok()
                .filter(|t| t.origin == bundle.sha256 && t.info.id == bundle.id);
            let translation = match current {
                Some(translation) => Ok(translation),
                None => {
                    let existed = path.exists();
                    let built = install(&path, &bundle);
                    if built.is_ok() && existed {
                        notes.push(CatalogNote::Rebuilt(bundle.id.to_owned()));
                    }
                    built
                }
            };
            match translation {
                Ok(translation) => {
                    self.open.insert(bundle.id.to_owned(), translation);
                }
                Err(why) => {
                    notes.push(CatalogNote::Failed(bundle.id.to_owned(), why.clone()));
                    let info = placeholder(bundle.id, true);
                    self.damaged.insert(bundle.id.to_owned(), (info, why));
                }
            }
        }
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .map(|dir| {
                dir.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|e| e == store::EXTENSION))
                    .collect()
            })
            .unwrap_or_default();
        paths.sort();
        for path in paths {
            if self.open.len() + self.damaged.len() >= MAX_TRANSLATIONS {
                break;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
                continue;
            };
            if !valid_id(&id) || self.open.contains_key(&id) || self.damaged.contains_key(&id) {
                continue;
            }
            match store::open(&path) {
                Ok(translation) if translation.info.id == id => {
                    self.open.insert(id, translation);
                }
                Ok(_) => {
                    self.damaged
                        .insert(id.clone(), (placeholder(&id, false), Damage::FileName));
                }
                Err(error) => {
                    self.damaged
                        .insert(id.clone(), (placeholder(&id, false), Damage::Store(error)));
                }
            }
        }
        notes
    }

    fn entries(&self) -> Vec<Entry> {
        let mut entries: Vec<Entry> = self
            .open
            .values()
            .map(|t| Entry {
                info: t.info.clone(),
                state: State::Ready {
                    inventory: Arc::new(t.inventory.clone()),
                    verses: t.verse_count,
                },
            })
            .chain(self.damaged.values().map(|(info, why)| Entry {
                info: info.clone(),
                state: State::Damaged(why.clone()),
            }))
            .collect();
        let order: Vec<&str> = self.bundles.iter().map(|b| b.id).collect();
        entries.sort_by_key(|e| {
            (
                order
                    .iter()
                    .position(|id| *id == e.info.id)
                    .unwrap_or(usize::MAX),
                e.info.id.clone(),
            )
        });
        entries
    }

    fn import(&mut self, path: &Path) -> Result<Imported, ImportError> {
        let bytes = read_capped(path).map_err(ImportError::Unreadable)?;
        let parsed = import::parse(&bytes).map_err(ImportError::Rejected)?;
        if self.open.len() + self.damaged.len() >= MAX_TRANSLATIONS {
            return Err(ImportError::TooMany);
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let base = derive_id(&parsed.abbreviation)
            .or_else(|| derive_id(&stem))
            .unwrap_or_else(|| "BIBLE".into());
        let taken = |id: &str| {
            self.open.contains_key(id)
                || self.damaged.contains_key(id)
                || self.bundles.iter().any(|b| b.id == id)
                || store::path_for(&self.dir, id).exists()
        };
        let id = std::iter::once(base.clone())
            .chain((2..100).map(|n| {
                let suffix = format!("-{n}");
                let keep = MAX_ID - suffix.len();
                format!("{}{suffix}", &base[..base.len().min(keep)])
            }))
            .find(|id| !taken(id))
            .ok_or(ImportError::TooMany)?;
        let name = if parsed.name.is_empty() {
            stem.clone()
        } else {
            parsed.name.clone()
        };
        let info = TranslationInfo {
            id: id.clone(),
            name,
            language: language_code(&parsed.language),
            rights: parsed.rights.clone(),
            source: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            // Rights of an imported file are unknown to Sela, so it is marked
            // restricted. The flag is recorded, not enforced: a schedule
            // item pinned from it still carries its verse text into the
            // .selaschedule file (stripping is open, M1-13).
            redistributable: false,
            bundled: false,
        };
        let origin = bundled::sha256(&bytes);
        let target = store::path_for(&self.dir, &id);
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| ImportError::Store(store::Error::Io(e.to_string())))?;
        store::write(&target, &info, &origin, &parsed.verses).map_err(ImportError::Store)?;
        let translation = store::open(&target).map_err(ImportError::Store)?;
        let imported = Imported {
            info: translation.info.clone(),
            verses: translation.verse_count,
            warnings: parsed.warnings,
        };
        self.open.insert(id, translation);
        Ok(imported)
    }
}

fn placeholder(id: &str, bundled: bool) -> TranslationInfo {
    TranslationInfo {
        id: id.to_owned(),
        name: id.to_owned(),
        language: String::new(),
        rights: String::new(),
        source: String::new(),
        redistributable: false,
        bundled,
    }
}

/// `en`, `eng`, `English` → `en`; `id`, `ind`, `IND` → `id`; others kept
/// lowercased (cut to 16 characters).
fn language_code(text: &str) -> String {
    let lower = text.trim().to_ascii_lowercase();
    match lower.as_str() {
        "en" | "eng" | "english" => "en".into(),
        "id" | "ind" | "indonesian" | "bahasa indonesia" => "id".into(),
        other => other.chars().take(16).collect(),
    }
}

fn install(path: &Path, bundle: &Bundle) -> Result<Translation, Damage> {
    let (info, verses) = bundled::decode(bundle).map_err(Damage::Bundle)?;
    store::write(path, &info, &bundle.sha256, &verses).map_err(Damage::Store)?;
    store::open(path).map_err(Damage::Store)
}

fn read_capped(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(import::MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::scripture::Book;
    use crate::scripture::store::tests::{info, v};
    use std::time::{Duration, Instant};

    /// Two small synthetic translations with different versification: `SYA`
    /// has 3 John 1:15 and no Genesis; `SYB` ends 3 John at verse 14, has
    /// Genesis 1:1 and lacks John 4.
    pub(crate) fn synthetic_bundles() -> Vec<Bundle> {
        let mut a = vec![(v(43, 3, 15), "Alpha fifteen".to_string())];
        for verse in 16..=18 {
            a.push((v(43, 3, verse), format!("Alpha verse {verse} light")));
        }
        a.push((v(43, 3, 36), "Alpha end of chapter".into()));
        a.push((v(43, 4, 1), "Alpha next chapter".into()));
        a.push((v(43, 4, 2), "Alpha second".into()));
        for verse in 14..=15 {
            a.push((v(64, 1, verse), format!("Alpha third John {verse}")));
        }
        let mut b = vec![(v(1, 1, 1), "Beta beginning".to_string())];
        for verse in 16..=18 {
            b.push((v(43, 3, verse), format!("Beta verse {verse} lamp")));
        }
        b.push((v(64, 1, 14), "Beta third John 14".into()));
        vec![
            bundled::synthetic("SYA", &info("SYA"), &a),
            bundled::synthetic("SYB", &info("SYB"), &b),
        ]
    }

    pub(crate) fn wait(worker: &Worker) -> Reply {
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(reply) = worker.poll() {
                return reply;
            }
            assert!(Instant::now() < end, "worker reply");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn catalog(worker: &Worker) -> (Vec<Entry>, Vec<CatalogNote>) {
        worker.submit(Command::Catalog).unwrap();
        match wait(worker) {
            Reply::Catalog { entries, notes } => (entries, notes),
            other => panic!("{other:?}"),
        }
    }

    fn passage(worker: &Worker, id: &str, from: VerseId, to: VerseId) -> Result<Vec<i64>, Error> {
        worker
            .submit(Command::Passage {
                generation: 7,
                translation: id.into(),
                from,
                to,
            })
            .unwrap();
        match wait(worker) {
            Reply::Passage {
                generation: 7,
                result,
                ..
            } => result.map(|verses| verses.iter().map(|(id, _)| id.key()).collect()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bundles_install_once_and_differing_versification_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let worker = Worker::start(dir.path().join("Scripture"), synthetic_bundles()).unwrap();
        let (entries, notes) = catalog(&worker);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(
            entries
                .iter()
                .map(|e| e.info.id.as_str())
                .collect::<Vec<_>>(),
            ["SYA", "SYB"]
        );
        let inventory = |i: usize| match &entries[i].state {
            State::Ready { inventory, .. } => inventory.clone(),
            other => panic!("{other:?}"),
        };
        let third_john = Book::from_number(64).unwrap();
        assert_eq!(inventory(0).last_verse(third_john, 1), Some(15));
        assert_eq!(inventory(1).last_verse(third_john, 1), Some(14));
        assert!(!inventory(0).has_book(Book::from_number(1).unwrap()));
        assert_eq!(
            passage(&worker, "SYA", v(43, 3, 36), v(43, 4, 2)),
            Ok(vec![43_003_036, 43_004_001, 43_004_002])
        );
        assert_eq!(
            passage(&worker, "SYB", v(43, 4, 1), v(43, 4, 2)),
            Ok(vec![])
        );
        assert_eq!(
            passage(&worker, "NOPE", v(43, 4, 1), v(43, 4, 2)),
            Err(Error::Unknown("NOPE".into()))
        );
        // A second catalog reuses the installed files.
        let modified = std::fs::metadata(store::path_for(&dir.path().join("Scripture"), "SYA"))
            .unwrap()
            .modified()
            .unwrap();
        let (_, notes) = catalog(&worker);
        assert!(notes.is_empty());
        let again = std::fs::metadata(store::path_for(&dir.path().join("Scripture"), "SYA"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(modified, again);
    }

    #[test]
    fn a_damaged_bundled_file_is_rebuilt_and_a_damaged_import_is_listed_unusable() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Scripture");
        {
            let worker = Worker::start(folder.clone(), synthetic_bundles()).unwrap();
            catalog(&worker);
        }
        std::fs::write(store::path_for(&folder, "SYA"), b"garbage garbage garbage").unwrap();
        std::fs::write(store::path_for(&folder, "USER"), b"also garbage").unwrap();
        let worker = Worker::start(folder.clone(), synthetic_bundles()).unwrap();
        let (entries, notes) = catalog(&worker);
        assert_eq!(notes, [CatalogNote::Rebuilt("SYA".into())]);
        assert!(matches!(entries[0].state, State::Ready { .. }));
        let user = entries.iter().find(|e| e.info.id == "USER").unwrap();
        assert!(matches!(
            &user.state,
            State::Damaged(Damage::Store(
                store::Error::Corrupt(_) | store::Error::Unsupported
            ))
        ));
        assert!(matches!(
            passage(&worker, "USER", v(1, 1, 1), v(1, 1, 1)),
            Err(Error::Unavailable(_))
        ));
        // A bundle whose bytes do not match their checksum is reported.
        let mut bad = synthetic_bundles();
        bad[1].sha256 = "0".repeat(64);
        // Windows refuses to delete a file the worker still has open.
        worker.shut_down();
        std::fs::remove_file(store::path_for(&folder, "SYB")).unwrap();
        let worker = Worker::start(folder, bad).unwrap();
        let (entries, notes) = catalog(&worker);
        assert_eq!(
            notes,
            [CatalogNote::Failed(
                "SYB".into(),
                Damage::Bundle(bundled::Error::Checksum)
            )]
        );
        assert!(matches!(&entries[1].state, State::Damaged(_)));
    }

    #[test]
    fn keyword_search_and_import_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Scripture");
        let worker = Worker::start(folder.clone(), synthetic_bundles()).unwrap();
        catalog(&worker);
        worker
            .submit(Command::Search {
                generation: 3,
                translation: "SYA".into(),
                query: "lig".into(),
            })
            .unwrap();
        match wait(&worker) {
            Reply::Search {
                generation: 3,
                result: Ok((hits, false)),
                ..
            } => assert_eq!(hits.len(), 3),
            other => panic!("{other:?}"),
        }
        let file = dir.path().join("my-bible.xml");
        std::fs::write(
            &file,
            r#"<XMLBIBLE biblename="Synthetic Import"><INFORMATION><identifier>SYA</identifier>
            <language>IND</language></INFORMATION><BIBLEBOOK bnumber="43"><CHAPTER cnumber="3">
            <VERS vnumber="16">Teks sintetis</VERS></CHAPTER></BIBLEBOOK></XMLBIBLE>"#,
        )
        .unwrap();
        worker.submit(Command::Import(file.clone())).unwrap();
        let imported = match wait(&worker) {
            Reply::Imported(Ok(imported)) => imported,
            other => panic!("{other:?}"),
        };
        assert_eq!(imported.info.id, "SYA-2", "a bundled id is never replaced");
        assert_eq!(imported.info.language, "id");
        assert_eq!(imported.info.source, "my-bible.xml");
        assert!(!imported.info.redistributable);
        assert_eq!(imported.verses, 1);
        assert!(store::path_for(&folder, "SYA-2").exists());
        let (entries, _) = catalog(&worker);
        assert_eq!(entries.len(), 3);

        std::fs::write(&file, b"<song/>").unwrap();
        worker.submit(Command::Import(file)).unwrap();
        assert_eq!(
            wait(&worker),
            Reply::Imported(Err(ImportError::Rejected(import::Rejected::WrongFormat)))
        );
        worker
            .submit(Command::Import(dir.path().join("missing.xml")))
            .unwrap();
        assert!(matches!(
            wait(&worker),
            Reply::Imported(Err(ImportError::Unreadable(_)))
        ));
    }

    #[test]
    fn the_queue_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let worker = Worker::start(dir.path().join("Scripture"), synthetic_bundles()).unwrap();
        let mut refused = 0;
        for _ in 0..(QUEUE * 4) {
            if worker.submit(Command::Catalog).is_err() {
                refused += 1;
            }
        }
        assert!(refused > 0, "a full queue hands the command back");
    }
}
