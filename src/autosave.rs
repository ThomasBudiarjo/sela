//! Schedule autosave and crash recovery (M1-12, scenario S03).
//!
//! Terms:
//!
//! - **Saved**: the schedule equals its `.selaschedule` file (the operator's
//!   explicit Save). `Schedule::is_dirty` is the opposite.
//! - **Committed**: a revision of the schedule (entries, file path and the
//!   file's baseline) is durably in the profile's `Recovery/` folder. A commit
//!   is one file written to a temporary name, synced, then renamed into place
//!   (and the folder synced on Unix); a revision file that is not complete and
//!   checksummed is never read back as a revision.
//! - **Uncommitted**: the schedule differs from the newest committed revision.
//!   The first unsaved edit after a commit (or during one) writes a small
//!   marker into `Recovery/session.json` (`uncommitted`), so a crash before
//!   the next commit can disclose that newer work was lost instead of
//!   claiming zero loss. The marker is withdrawn when the schedule is back at
//!   the commit or equals its saved file (see [`Policy::step`]).
//!
//! Revisions pin library song revisions (`storage::Version`) and do not embed
//! song content: the library keeps every committed revision and never
//! garbage-collects (docs/storage.md), and a schedule only pins committed
//! revisions. An autosave therefore never needs the storage worker. Scripture
//! items (M4-04) have no library revision, so a revision embeds their pinned
//! verses, as a `.selaschedule` does.
//!
//! Cadence ([`Policy`]): a commit starts once the schedule has been unchanged
//! for `quiet` (2 s) or `max_delay` (10 s) after the first uncommitted change,
//! whichever comes first; one commit runs at a time; a failed commit is
//! retried after `retry` (30 s). Close flushes (see [`Job::Close`]).
//!
//! Retention ([`Budget`]): after each commit the oldest revisions are removed
//! until at most `max_revisions` (20) remain and they total at most
//! `max_bytes` (4 MiB); the newest is never removed. A revision larger than
//! `max_bytes` is refused (`Error::Budget`) and the older revisions stay.
//!
//! Recovery: `session.json` says `running` while an operator owns the
//! profile, `clean` after a clean close and `offered` after a close that left
//! the previous run's offer unanswered (offered again, not acknowledged).
//! Finding `running` at start (the
//! profile lock guarantees no other process is running) means the previous
//! run ended uncleanly; [`Startup::offer`] then names the newest valid
//! revision newer than the last acknowledged one, damaged newer revisions,
//! an interrupted commit and the uncommitted marker. Nothing is restored
//! without the operator's choice, and declined revisions stay on disk.
//!
//! All I/O is blocking and runs on [`Worker`]'s thread. `Policy`,
//! [`fingerprint`] and [`Snapshot::of`] are cheap and run on the caller.
use crate::arrangement::VariantId;
use crate::schedule::{ItemTheme, Pin, Schedule};
use crate::storage::{Id, MAX_ITEMS, Version};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub const RECOVERY_DIR: &str = "Recovery";
pub const SESSION_FILE: &str = "session.json";
const REVISION_PREFIX: &str = "autosave-";
const REVISION_SUFFIX: &str = ".selarecovery";
const TEMP_SUFFIX: &str = ".tmp";
const FORMAT: &str = "sela-recovery";
const SESSION_FORMAT: &str = "sela-recovery-session";
/// Revision format version. Version 2 (themes integration, M1-08) adds each
/// item's theme snapshot (`theme`, hex of `Theme::encode`, and
/// `theme_explicit`) and each pin's explicit theme stamp (`theme_stamp`);
/// version 1 revisions still read, without snapshots. The session file
/// keeps version 1. Version 3 (scripture integration, M4-04) adds a
/// scripture item's pinned verses (`scripture`, the `.selaschedule` snapshot
/// fields); versions 1 and 2 still read, with song items only.
const VERSION: u64 = 3;
const SESSION_VERSION: u64 = 1;
/// Larger files are damaged, not read: 32 items with 1 KiB titles stay far
/// below it. Scripture items embed their verses (a chapter is tens of KiB);
/// a schedule whose revision would exceed it is refused (`Error::Budget`),
/// the older revisions stay and the autosave status reports the failure.
pub const MAX_REVISION_BYTES: u64 = 512 * 1024;
const MAX_SESSION_BYTES: u64 = 64 * 1024;
const MAX_PATH_BYTES: usize = 4096;
const MAX_TITLE_BYTES: usize = 1024;
/// Revision files read while looking for a valid one at start.
const MAX_SCAN: usize = 64;
const QUEUE: usize = 4;

/// Wall-clock milliseconds since the Unix epoch. Injected so tests and the
/// recovery prompt's "minutes ago" never depend on the real clock.
pub type WallClock = Arc<dyn Fn() -> u64 + Send + Sync>;

pub fn system_clock() -> WallClock {
    Arc::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    })
}

/// Filesystem operations the store needs, so tests can inject faults (disk
/// full, read-only or locked files, a crash in the middle of a write).
pub trait Fs: Send + Sync {
    fn create_dir_all(&self, dir: &Path) -> io::Result<()>;
    /// File names in `dir` with their sizes.
    fn list(&self, dir: &Path) -> io::Result<Vec<(String, u64)>>;
    /// At most `max + 1` bytes, so a larger file is detected without loading it.
    fn read(&self, path: &Path, max: u64) -> io::Result<Vec<u8>>;
    /// Creates `path` (it must not exist), writes `bytes` and syncs the file.
    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove(&self, path: &Path) -> io::Result<()>;
    /// Makes a rename in `dir` durable (Unix; a no-op elsewhere).
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;
    /// Last-modified time of `path` in wall-clock ms since the Unix epoch.
    fn modified(&self, path: &Path) -> io::Result<u64>;
}

pub struct RealFs;

impl Fs for RealFs {
    fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<(String, u64)>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                out.push((
                    entry.file_name().to_string_lossy().into_owned(),
                    metadata.len(),
                ));
            }
        }
        Ok(out)
    }

    fn read(&self, path: &Path, max: u64) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(max + 1)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            fs::File::open(dir)?.sync_all()
        }
        #[cfg(not(unix))]
        {
            let _ = dir;
            Ok(())
        }
    }

    fn modified(&self, path: &Path) -> io::Result<u64> {
        let since = fs::metadata(path)?
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        Ok(u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
    }
}

/// Why a store operation failed. Carries no paths or schedule content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// No space left on the profile's volume (or its quota).
    DiskFull,
    /// The profile folder or a recovery file cannot be written.
    ReadOnly,
    /// Another process holds a recovery file open (a Windows sharing lock,
    /// an antivirus scan, ...).
    Locked,
    /// The revision is larger than the whole retention budget.
    Budget,
    /// The profile lock could not be taken, so another operator may share
    /// the folder: autosave never starts there (see `sela::profile`).
    Unlocked,
    /// Any other OS error, by its kind.
    Io(String),
}

impl Error {
    pub fn from_io(error: &io::Error) -> Self {
        use io::ErrorKind as K;
        match error.kind() {
            K::StorageFull | K::QuotaExceeded => Self::DiskFull,
            K::PermissionDenied | K::ReadOnlyFilesystem => Self::ReadOnly,
            K::ResourceBusy => Self::Locked,
            // ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION.
            _ if cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33)) => Self::Locked,
            kind => Self::Io(format!("{kind:?}")),
        }
    }
}

fn io_error(error: io::Error) -> Error {
    Error::from_io(&error)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    pub max_revisions: usize,
    pub max_bytes: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_revisions: 20,
            max_bytes: 4 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cadence {
    pub quiet: Duration,
    pub max_delay: Duration,
    pub retry: Duration,
}

impl Default for Cadence {
    fn default() -> Self {
        Self {
            quiet: Duration::from_secs(2),
            max_delay: Duration::from_secs(10),
            retry: Duration::from_secs(30),
        }
    }
}

/// Everything the autosave needs from outside; tests replace each part.
#[derive(Clone)]
pub struct Config {
    pub fs: Arc<dyn Fs>,
    pub clock: WallClock,
    pub budget: Budget,
    pub cadence: Cadence,
}

impl Config {
    pub fn system() -> Self {
        Self {
            fs: Arc::new(RealFs),
            clock: system_clock(),
            budget: Budget::default(),
            cadence: Cadence::default(),
        }
    }
}

/// One schedule entry as a revision records it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub pin: Pin,
    pub title: String,
    /// The entry's theme snapshot (M1-08), explicit or captured.
    pub theme: Option<ItemTheme>,
    /// A scripture item's pinned verses (version 3); `None` for a song.
    pub scripture: Option<Arc<crate::scripture::Snapshot>>,
}

/// The schedule as committed: entries, its file and the file's baseline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The `.selaschedule` file; `None` while untitled (or for a path that
    /// is not UTF-8, which a revision cannot record).
    pub path: Option<PathBuf>,
    pub entries: Vec<Entry>,
    /// Pins as last saved to or opened from `path`.
    pub baseline: Vec<Pin>,
}

impl Snapshot {
    pub fn of(schedule: &Schedule) -> Self {
        Self {
            path: schedule.path().map(Path::to_path_buf),
            entries: schedule
                .entries()
                .iter()
                .zip(schedule.pins())
                .map(|(e, pin)| Entry {
                    pin,
                    title: e.title.clone(),
                    theme: e.theme.clone(),
                    scripture: e.scripture.clone(),
                })
                .collect(),
            baseline: schedule.baseline().to_vec(),
        }
    }

    /// The schedule had changes not saved to its file (or no file at all).
    pub fn unsaved(&self) -> bool {
        self.entries
            .iter()
            .map(|e| e.pin)
            .ne(self.baseline.iter().copied())
    }
}

/// Cheap change detector for the poll loop: equal schedules give equal
/// values, so a commit is only considered when this changes.
pub fn fingerprint(schedule: &Schedule) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let pin = |pin: &Pin, hasher: &mut std::collections::hash_map::DefaultHasher| {
        pin.version.id.0.hash(hasher);
        pin.version.revision.hash(hasher);
        pin.arrangement.map(|a| a.0.0).hash(hasher);
        pin.theme.hash(hasher);
    };
    schedule.path().hash(&mut hasher);
    schedule.entries().len().hash(&mut hasher);
    for entry in schedule.entries() {
        pin(
            &Pin {
                version: entry.version,
                arrangement: entry.arrangement,
                theme: None,
            },
            &mut hasher,
        );
        // An applied (explicit) theme snapshot, hashed in place: no encode or
        // digest on the UI thread. A captured snapshot (taken when an entry
        // is first shown) is not a schedule edit, just as it never makes the
        // schedule dirty, so it starts no commit; the next commit records it.
        entry
            .theme
            .as_ref()
            .filter(|t| t.explicit)
            .map(|t| &t.theme)
            .hash(&mut hasher);
        // Titles change with the pinned revision (insert, re-pin, open,
        // restore), so their length is enough and keeps a 32-entry
        // fingerprint cheap on the UI thread (whole titles measured 149 us
        // per tick, docs/autosave.md). Only reopening a hand-edited file
        // could change a title alone; the revision then keeps the old
        // display title for the same pinned song until the next change.
        entry.title.len().hash(&mut hasher);
    }
    schedule.baseline().len().hash(&mut hasher);
    for p in schedule.baseline() {
        pin(p, &mut hasher);
    }
    hasher.finish()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revision {
    pub sequence: u64,
    pub saved_at: u64,
    pub snapshot: Snapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uncommitted {
    /// Newest committed sequence when the first newer edit was made.
    pub after: u64,
    /// Wall-clock ms of that edit.
    pub since: u64,
    /// Entries in the schedule at that moment.
    pub items: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Running,
    Clean,
    /// Closed by the operator while this run's recovery prompt was still
    /// unanswered: the previous run's offer stands for the next start.
    Offered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Session {
    state: State,
    started_at: u64,
    /// Revisions up to here were seen by the operator (closed cleanly,
    /// restored or declined) and are never offered again.
    acknowledged: u64,
    uncommitted: Option<Uncommitted>,
}

/// What the previous run left behind, for the recovery prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    /// Newest valid revision the operator has not acknowledged.
    pub revision: Option<Revision>,
    /// Edits made after the newest commit; they are lost.
    pub lost: Option<Uncommitted>,
    /// Newer revisions that failed their checksum or did not parse.
    pub damaged: usize,
    /// A commit was cut off; its partial file was removed.
    pub interrupted: bool,
    /// The revision's schedule file was modified (wall-clock ms) after the
    /// revision was committed, so the file may hold newer work than the
    /// revision (for example a Save in the gap before the next commit).
    pub file_saved: Option<u64>,
    /// The previous run closed normally, but with this same offer still
    /// unanswered (it was already shown once and neither restored nor
    /// declined).
    pub unanswered: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Startup {
    /// The previous run did not close cleanly.
    pub unclean: bool,
    /// `Some` only when there is something to restore or to disclose.
    pub offer: Option<Offer>,
}

/// [`Store::open`] could not make this run's session durable, so autosave is
/// off. The previous run's offer was still read and is returned with it: the
/// operator can restore it (read-only), and since nothing was acknowledged it
/// is offered again on the next start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenFailure {
    pub error: Error,
    pub offer: Option<Box<Offer>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Committed {
    pub sequence: u64,
    pub saved_at: u64,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

fn id_from(value: &Value) -> Option<Id> {
    let text = value.as_str()?;
    if text.len() != 32 || !text.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..32)
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect();
    Some(Id(bytes?.try_into().ok()?))
}

fn pin_json(pin: &Pin) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("song_id".into(), hex(&pin.version.id.0).into());
    out.insert("revision".into(), pin.version.revision.into());
    out.insert(
        "arrangement".into(),
        pin.arrangement.map_or(Value::Null, |a| hex(&a.0.0).into()),
    );
    if let Some(stamp) = pin.theme {
        out.insert("theme_stamp".into(), stamp.into());
    }
    out
}

fn pin_from(value: &Value) -> Option<Pin> {
    Some(Pin {
        version: Version {
            id: id_from(value.get("song_id")?)?,
            revision: value.get("revision")?.as_i64().filter(|r| *r > 0)?,
        },
        arrangement: match value.get("arrangement")? {
            Value::Null => None,
            other => Some(VariantId(id_from(other)?)),
        },
        theme: match value.get("theme_stamp") {
            Some(stamp) => Some(stamp.as_u64()?),
            None => None,
        },
    })
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

/// A version 2 item's theme snapshot; a damaged one makes the revision
/// damaged rather than silently restyling it.
fn theme_from(item: &Value) -> Option<Option<ItemTheme>> {
    let Some(value) = item.get("theme") else {
        return Some(None);
    };
    let text = value.as_str()?;
    if text.len() > 2 * crate::theme::MAX_ENCODED {
        return None;
    }
    Some(Some(ItemTheme {
        theme: crate::theme::Theme::decode(&unhex(text)?)?,
        explicit: match item.get("theme_explicit") {
            Some(flag) => flag.as_bool()?,
            None => false,
        },
    }))
}

fn revision_name(sequence: u64) -> String {
    format!("{REVISION_PREFIX}{sequence:012}{REVISION_SUFFIX}")
}

fn revision_sequence(name: &str) -> Option<u64> {
    let digits = name
        .strip_prefix(REVISION_PREFIX)?
        .strip_suffix(REVISION_SUFFIX)?;
    if digits.len() != 12 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|s| *s > 0)
}

/// A revision file: one JSON line, then `sha256 <hex of that line>`.
pub fn encode_revision(revision: &Revision) -> Vec<u8> {
    let snapshot = &revision.snapshot;
    let items: Vec<Value> = snapshot
        .entries
        .iter()
        .map(|e| {
            let mut item = pin_json(&e.pin);
            item.insert("title".into(), e.title.clone().into());
            if let Some(theme) = &e.theme {
                item.insert("theme".into(), hex(&theme.theme.encode()).into());
                item.insert("theme_explicit".into(), theme.explicit.into());
            }
            if let Some(snapshot) = &e.scripture {
                item.insert(
                    "scripture".into(),
                    Value::Object(crate::schedule_file::snapshot_json(snapshot)),
                );
            }
            Value::Object(item)
        })
        .collect();
    let baseline: Vec<Value> = snapshot
        .baseline
        .iter()
        .map(|p| Value::Object(pin_json(p)))
        .collect();
    let path = snapshot
        .path
        .as_deref()
        .and_then(Path::to_str)
        .filter(|p| p.len() <= MAX_PATH_BYTES)
        .map_or(Value::Null, Value::from);
    let body = json!({
        "format": FORMAT,
        "version": VERSION,
        "sequence": revision.sequence,
        "saved_at_ms": revision.saved_at,
        "path": path,
        "items": items,
        "baseline": baseline,
    })
    .to_string();
    let digest = hex(&Sha256::digest(body.as_bytes()));
    format!("{body}\nsha256 {digest}\n").into_bytes()
}

/// `None` for anything but a complete, matching revision file.
pub fn decode_revision(bytes: &[u8]) -> Option<Revision> {
    if bytes.len() as u64 > MAX_REVISION_BYTES {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let (body, trailer) = text.strip_suffix('\n')?.split_once('\n')?;
    if trailer.strip_prefix("sha256 ")? != hex(&Sha256::digest(body.as_bytes())) {
        return None;
    }
    let value: Value = serde_json::from_str(body).ok()?;
    let version = value.get("version")?.as_u64()?;
    if value.get("format")?.as_str()? != FORMAT || !(1..=VERSION).contains(&version) {
        return None;
    }
    let items = value.get("items")?.as_array()?;
    let baseline = value.get("baseline")?.as_array()?;
    if items.len() > MAX_ITEMS || baseline.len() > MAX_ITEMS {
        return None;
    }
    let entries = items
        .iter()
        .map(|item| {
            let title = item.get("title")?.as_str()?;
            (title.len() <= MAX_TITLE_BYTES).then_some(())?;
            let (pin, theme) = if version >= 2 {
                (pin_from(item)?, theme_from(item)?)
            } else {
                // Version 1 has no theme fields; ignore any.
                (
                    Pin {
                        theme: None,
                        ..pin_from(item)?
                    },
                    None,
                )
            };
            // Version 3: a scripture item's verses, which must match the pin
            // (their content identity); a damaged one makes the revision
            // damaged.
            let scripture = match item.get("scripture").filter(|_| version >= 3) {
                Some(value) => {
                    let snapshot = crate::schedule_file::snapshot_from_json(value)?;
                    (snapshot.version() == pin.version).then_some(())?;
                    Some(Arc::new(snapshot))
                }
                None => None,
            };
            Some(Entry {
                pin,
                title: title.to_owned(),
                theme,
                scripture,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let baseline = baseline
        .iter()
        .map(|p| {
            pin_from(p).map(|pin| Pin {
                theme: pin.theme.filter(|_| version >= 2),
                ..pin
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let path = match value.get("path")? {
        Value::Null => None,
        Value::String(p) if p.len() <= MAX_PATH_BYTES => Some(PathBuf::from(p)),
        _ => return None,
    };
    Some(Revision {
        sequence: value.get("sequence")?.as_u64().filter(|s| *s > 0)?,
        saved_at: value.get("saved_at_ms")?.as_u64()?,
        snapshot: Snapshot {
            path,
            entries,
            baseline,
        },
    })
}

pub(crate) fn encode_session(session: &Session) -> Vec<u8> {
    let uncommitted = session.uncommitted.map_or(
        Value::Null,
        |u| json!({"after": u.after, "since_ms": u.since, "items": u.items}),
    );
    let state = match session.state {
        State::Running => "running",
        State::Clean => "clean",
        State::Offered => "offered",
    };
    let mut text = json!({
        "format": SESSION_FORMAT,
        "version": SESSION_VERSION,
        "state": state,
        "started_at_ms": session.started_at,
        "acknowledged": session.acknowledged,
        "uncommitted": uncommitted,
    })
    .to_string();
    text.push('\n');
    text.into_bytes()
}

pub(crate) fn decode_session(bytes: &[u8]) -> Option<Session> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    if value.get("format")?.as_str()? != SESSION_FORMAT
        || value.get("version")?.as_u64()? != SESSION_VERSION
    {
        return None;
    }
    let uncommitted = match value.get("uncommitted")? {
        Value::Null => None,
        u => Some(Uncommitted {
            after: u.get("after")?.as_u64()?,
            since: u.get("since_ms")?.as_u64()?,
            items: usize::try_from(u.get("items")?.as_u64()?).ok()?,
        }),
    };
    Some(Session {
        state: match value.get("state")?.as_str()? {
            "running" => State::Running,
            "clean" => State::Clean,
            "offered" => State::Offered,
            _ => return None,
        },
        started_at: value.get("started_at_ms")?.as_u64()?,
        acknowledged: value.get("acknowledged")?.as_u64()?,
        uncommitted,
    })
}

/// The recovery folder of one profile. Worker-only: every method blocks.
pub struct Store {
    dir: PathBuf,
    fs: Arc<dyn Fs>,
    clock: WallClock,
    budget: Budget,
    session: Session,
    /// Newest sequence on disk (valid or not); the next commit is one more.
    newest: u64,
    /// Fingerprint of the newest commit of this run.
    last_fingerprint: Option<u64>,
    /// `open` found an offer that has not been resolved yet.
    offer_open: bool,
}

impl Store {
    /// Scans `<profile>/Recovery`, removes partial files of interrupted
    /// writes, assesses the previous run and marks this run `running`.
    pub fn open(profile: &Path, config: &Config) -> Result<(Self, Startup), Error> {
        Self::open_keeping_offer(profile, config).map_err(|failure| failure.error)
    }

    /// [`Store::open`], but a failure to write this run's session (disk full,
    /// read-only, locked) still returns the previous run's offer.
    pub fn open_keeping_offer(
        profile: &Path,
        config: &Config,
    ) -> Result<(Self, Startup), OpenFailure> {
        let fail = |error| OpenFailure { error, offer: None };
        let dir = profile.join(RECOVERY_DIR);
        let fs = config.fs.clone();
        fs.create_dir_all(&dir).map_err(io_error).map_err(fail)?;
        let listing = fs.list(&dir).map_err(io_error).map_err(fail)?;
        let mut interrupted = false;
        let mut sequences = Vec::new();
        for (name, _) in &listing {
            if let Some(sequence) = revision_sequence(name) {
                sequences.push(sequence);
            } else if name.ends_with(TEMP_SUFFIX) {
                interrupted |= name.starts_with(REVISION_PREFIX);
                // A leftover we cannot remove is harmless: it is never read.
                let _ = fs.remove(&dir.join(name));
            }
        }
        sequences.sort_unstable();
        let newest = sequences.last().copied().unwrap_or(0);
        let previous = match fs.read(&dir.join(SESSION_FILE), MAX_SESSION_BYTES) {
            Ok(bytes) if bytes.len() as u64 <= MAX_SESSION_BYTES => Some(decode_session(&bytes)),
            Ok(_) => Some(None),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(fail(io_error(e))),
        };
        // A damaged session file counts as an unclean run with nothing
        // acknowledged, so a dirty revision is still offered.
        let (unclean, unanswered, acknowledged, marker) = match &previous {
            None => (false, false, newest, None),
            Some(None) => (true, false, 0, None),
            Some(Some(s)) => (
                s.state == State::Running,
                s.state == State::Offered,
                s.acknowledged,
                s.uncommitted,
            ),
        };
        let mut offer = None;
        if unclean || unanswered {
            let mut damaged = 0;
            let mut revision = None;
            for &sequence in sequences.iter().rev().take(MAX_SCAN) {
                if sequence <= acknowledged {
                    break;
                }
                let read = fs
                    .read(&dir.join(revision_name(sequence)), MAX_REVISION_BYTES)
                    .ok()
                    .and_then(|bytes| decode_revision(&bytes))
                    .filter(|r| r.sequence == sequence);
                match read {
                    Some(found) => {
                        revision = Some(found);
                        break;
                    }
                    None => damaged += 1,
                }
            }
            let lost = marker.filter(|m| m.after >= newest);
            let unsaved = revision.as_ref().is_some_and(|r| r.snapshot.unsaved());
            if unsaved || lost.is_some() || damaged > 0 {
                // One metadata read of the revision's own file: a newer
                // modification time means a save the revision does not hold.
                let file_saved = revision.as_ref().and_then(|r| {
                    let modified = fs.modified(r.snapshot.path.as_deref()?).ok()?;
                    (modified > r.saved_at).then_some(modified)
                });
                offer = Some(Offer {
                    revision,
                    lost,
                    damaged,
                    interrupted,
                    unanswered,
                    file_saved,
                });
            }
        }
        let session = Session {
            state: State::Running,
            started_at: (config.clock)(),
            // While an offer is open, nothing is acknowledged and the marker
            // stays, so a second crash before the answer offers it again.
            acknowledged: if offer.is_some() {
                acknowledged
            } else {
                newest
            },
            uncommitted: if offer.is_some() { marker } else { None },
        };
        let mut store = Self {
            dir,
            fs,
            clock: config.clock.clone(),
            budget: config.budget,
            session,
            newest,
            last_fingerprint: None,
            offer_open: offer.is_some(),
        };
        if let Err(error) = store.write_session() {
            return Err(OpenFailure {
                error,
                offer: offer.map(Box::new),
            });
        }
        Ok((store, Startup { unclean, offer }))
    }

    fn write_session(&mut self) -> Result<(), Error> {
        let bytes = encode_session(&self.session);
        let path = self.dir.join(SESSION_FILE);
        self.publish(&path, &bytes)
    }

    /// Temporary file, sync, rename, folder sync. The temporary file is
    /// removed on failure; `path` is either the old or the new file.
    fn publish(&self, path: &Path, bytes: &[u8]) -> Result<(), Error> {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(TEMP_SUFFIX);
        let temp = path.with_file_name(name);
        let _ = self.fs.remove(&temp);
        let result = self
            .fs
            .write_new(&temp, bytes)
            .and_then(|()| self.fs.rename(&temp, path));
        if let Err(error) = result {
            let _ = self.fs.remove(&temp);
            return Err(io_error(error));
        }
        self.fs.sync_dir(&self.dir).map_err(io_error)
    }

    pub fn newest(&self) -> u64 {
        self.newest
    }

    /// Commits `snapshot` (whose fingerprint is `fingerprint`) as the next
    /// revision, then rotates old ones. Returns the commit and, separately, a
    /// rotation or session failure (the commit itself stands).
    pub fn commit(
        &mut self,
        fingerprint: u64,
        snapshot: &Snapshot,
    ) -> Result<(Committed, Option<Error>), Error> {
        let sequence = self.newest + 1;
        let saved_at = (self.clock)();
        let bytes = encode_revision(&Revision {
            sequence,
            saved_at,
            snapshot: snapshot.clone(),
        });
        if bytes.len() as u64 > self.budget.max_bytes.min(MAX_REVISION_BYTES) {
            return Err(Error::Budget);
        }
        self.publish(&self.dir.join(revision_name(sequence)), &bytes)?;
        self.newest = sequence;
        self.last_fingerprint = Some(fingerprint);
        let mut warning = None;
        if self.session.state != State::Running {
            // A close was flushed but the operator kept working.
            self.session.state = State::Running;
            warning = self.write_session().err();
        }
        let rotation = self.rotate().err();
        Ok((Committed { sequence, saved_at }, rotation.or(warning)))
    }

    fn rotate(&mut self) -> Result<(), Error> {
        let mut revisions: Vec<(u64, u64)> = self
            .fs
            .list(&self.dir)
            .map_err(io_error)?
            .into_iter()
            .filter_map(|(name, size)| Some((revision_sequence(&name)?, size)))
            .collect();
        revisions.sort_unstable();
        let mut total: u64 = revisions.iter().map(|(_, size)| size).sum();
        let mut count = revisions.len();
        for &(sequence, size) in &revisions {
            if count <= 1 || sequence == self.newest {
                break;
            }
            if count <= self.budget.max_revisions && total <= self.budget.max_bytes {
                break;
            }
            self.fs
                .remove(&self.dir.join(revision_name(sequence)))
                .map_err(io_error)?;
            count -= 1;
            total -= size;
        }
        Ok(())
    }

    /// The schedule no longer has changes that a crash would lose (it went
    /// back to the newest commit, or equals its saved file): the marker goes.
    pub fn clear_uncommitted(&mut self) -> Result<(), Error> {
        if self.offer_open || self.session.uncommitted.is_none() {
            return Ok(());
        }
        let marker = self.session.uncommitted.take();
        let result = self.write_session();
        if result.is_err() {
            // The file may still hold the marker: a retry must write again.
            self.session.uncommitted = marker;
        }
        result
    }

    /// `open` found an offer that is not resolved yet.
    pub fn offer_open(&self) -> bool {
        self.offer_open
    }

    /// Records that the schedule (`fingerprint`, `items` entries) differs
    /// from the newest commit. A marker for exactly the committed schedule
    /// arrives late and is dropped. While the previous run's offer is open
    /// its marker stays (like [`Store::clear_uncommitted`]): the operator
    /// writes nothing until the answer, and the store keeps that promise
    /// on its own.
    pub fn mark_uncommitted(&mut self, fingerprint: u64, items: usize) -> Result<(), Error> {
        if self.offer_open || self.last_fingerprint == Some(fingerprint) {
            return Ok(());
        }
        self.session.state = State::Running;
        self.session.uncommitted = Some(Uncommitted {
            after: self.newest,
            since: (self.clock)(),
            items,
        });
        self.write_session()
    }

    /// The operator answered the recovery prompt. With `restore`, the
    /// restored schedule is committed first as a new revision, so a crash
    /// right after the answer still finds it. Either way the previous run's
    /// revisions are never offered again; declined ones stay on disk until
    /// rotation removes them.
    pub fn resolve(
        &mut self,
        restore: Option<(u64, &Snapshot)>,
    ) -> Result<Option<Committed>, Error> {
        // Answered, whatever the writes below do: a close after this is a
        // normal one.
        self.offer_open = false;
        let previous = self.newest;
        let committed = match restore {
            Some((fingerprint, snapshot)) => Some(self.commit(fingerprint, snapshot)?.0),
            None => None,
        };
        self.session.state = State::Running;
        self.session.acknowledged = previous;
        self.session.uncommitted = None;
        self.write_session()?;
        Ok(committed)
    }

    /// Clean close: commits `last` when given, then marks the run clean.
    /// A failed final commit is reported, but the run is still marked clean
    /// when that write succeeds: the operator chose to close.
    ///
    /// While the previous run's offer is unresolved (the prompt is open, or
    /// its reply has not even reached the window yet), nothing is committed
    /// or acknowledged: the run is recorded as `offered`, so the next start
    /// offers the same revision again and says the prompt went unanswered.
    pub fn close(&mut self, last: Option<(u64, &Snapshot)>) -> Result<Option<Committed>, Error> {
        if self.offer_open {
            self.session.state = State::Offered;
            self.write_session()?;
            return Ok(None);
        }
        let committed =
            last.map(|(fingerprint, snapshot)| self.commit(fingerprint, snapshot).map(|(c, _)| c));
        self.session.state = State::Clean;
        self.session.acknowledged = self.newest;
        self.session.uncommitted = None;
        self.write_session()?;
        committed.transpose()
    }
}

pub enum Job {
    Commit(u64, Box<Snapshot>),
    /// Fingerprint and entry count of the uncommitted schedule.
    Mark(u64, usize),
    /// Nothing would be lost by a crash now: drop the marker.
    Unmark,
    /// The recovery prompt was answered; `Some` restores (and commits) the
    /// schedule with that fingerprint.
    Resolve(Option<(u64, Box<Snapshot>)>),
    /// Flush and mark clean; the final schedule is committed first when it
    /// differs from the newest commit.
    Close(Option<(u64, Box<Snapshot>)>),
}

#[derive(Debug)]
pub enum Reply {
    /// On failure autosave is off, but the previous run's offer may still
    /// be restorable (see [`OpenFailure`]).
    Opened(Result<Startup, OpenFailure>),
    /// The fingerprint the commit was for, and its outcome (plus a
    /// rotation/session warning that does not undo it).
    Committed(u64, Result<(Committed, Option<Error>), Error>),
    /// A `Job::Mark` outcome.
    Marked(Result<(), Error>),
    /// A `Job::Unmark` outcome.
    Unmarked(Result<(), Error>),
    /// The restored fingerprint, if any, and the outcome.
    Resolved(Option<u64>, Result<Option<Committed>, Error>),
    /// The flushed fingerprint, if any, and the outcome.
    Closed(Option<u64>, Result<Option<Committed>, Error>),
}

/// Runs the [`Store`] on its own thread. Jobs are bounded (`QUEUE`); a full
/// queue refuses the job and the caller keeps it as uncommitted.
pub struct Worker {
    jobs: Option<SyncSender<Job>>,
    replies: Receiver<Reply>,
    _thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn start(profile: PathBuf, config: Config) -> io::Result<Self> {
        let (jobs, job_rx) = mpsc::sync_channel::<Job>(QUEUE);
        let (reply_tx, replies) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("sela-autosave".into())
            .spawn(move || {
                let mut store = match Store::open_keeping_offer(&profile, &config) {
                    Ok((store, startup)) => {
                        let _ = reply_tx.send(Reply::Opened(Ok(startup)));
                        store
                    }
                    Err(failure) => {
                        let _ = reply_tx.send(Reply::Opened(Err(failure)));
                        return;
                    }
                };
                for job in job_rx {
                    let reply = match job {
                        Job::Commit(fingerprint, snapshot) => {
                            Reply::Committed(fingerprint, store.commit(fingerprint, &snapshot))
                        }
                        Job::Mark(fingerprint, items) => {
                            Reply::Marked(store.mark_uncommitted(fingerprint, items))
                        }
                        Job::Unmark => Reply::Unmarked(store.clear_uncommitted()),
                        Job::Resolve(restore) => Reply::Resolved(
                            restore.as_ref().map(|(f, _)| *f),
                            store.resolve(restore.as_ref().map(|(f, s)| (*f, &**s))),
                        ),
                        Job::Close(last) => Reply::Closed(
                            // An open offer commits nothing at close.
                            last.as_ref()
                                .filter(|_| !store.offer_open())
                                .map(|(f, _)| *f),
                            store.close(last.as_ref().map(|(f, s)| (*f, &**s))),
                        ),
                    };
                    if reply_tx.send(reply).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            jobs: Some(jobs),
            replies,
            _thread: Some(thread),
        })
    }

    /// `false` when the queue is full or the thread is gone.
    pub fn submit(&self, job: Job) -> bool {
        matches!(
            self.jobs.as_ref().map(|jobs| jobs.try_send(job)),
            Some(Ok(()))
        )
    }

    pub fn poll(&self) -> Option<Reply> {
        self.replies.try_recv().ok()
    }
}

/// What the caller should do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Idle,
    /// Record the uncommitted marker for the schedule with this
    /// fingerprint (first unsaved change after a commit, or the first one
    /// made while a commit runs).
    Mark(u64),
    /// Drop the marker: the schedule is back at the newest commit, or equals
    /// its saved file, so a crash would lose nothing.
    Unmark,
    /// Commit the schedule whose fingerprint this is.
    Commit(u64),
}

/// Autosave cadence and committed/uncommitted state, driven by an injected
/// monotonic clock (`now`). Framework-free; one instance per operator.
#[derive(Clone, Debug)]
pub struct Policy {
    cadence: Cadence,
    committed: u64,
    seen: u64,
    first_change: Option<Instant>,
    last_change: Option<Instant>,
    in_flight: Option<u64>,
    retry_at: Option<Instant>,
    /// A marker is (or is being) recorded for work after the newest commit.
    marked: bool,
    /// A marker for an edit made while the in-flight commit runs is queued
    /// behind it, so it lands with `after` = that commit.
    flight_marked: bool,
    /// A marker write failed: no marker step before this instant.
    marker_retry_at: Option<Instant>,
}

impl Policy {
    /// `initial`: the fingerprint that needs no commit (the empty schedule
    /// at start).
    pub fn new(cadence: Cadence, initial: u64) -> Self {
        Self {
            cadence,
            committed: initial,
            seen: initial,
            first_change: None,
            last_change: None,
            in_flight: None,
            retry_at: None,
            marked: false,
            flight_marked: false,
            marker_retry_at: None,
        }
    }

    /// The cadence this policy runs with.
    pub fn cadence(&self) -> Cadence {
        self.cadence
    }

    /// Fingerprint of the newest successful commit (or the initial one).
    pub fn committed(&self) -> u64 {
        self.committed
    }

    /// The newest observed schedule differs from the newest commit.
    pub fn uncommitted(&self) -> bool {
        self.seen != self.committed
    }

    pub fn in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    /// A failed commit waits for its retry.
    pub fn failing(&self) -> bool {
        self.retry_at.is_some()
    }

    /// The newest observed fingerprint.
    pub fn seen(&self) -> u64 {
        self.seen
    }

    /// One cadence step for the schedule with `fingerprint`; `unsaved` says
    /// whether it has changes that are not in its file (`Schedule::is_dirty`).
    /// Only unsaved work is marked: a crash loses nothing that is in a file.
    /// A marker is withdrawn when it would no longer be true, but never while
    /// a commit runs (the commit may still fail, and the marker is then the
    /// only record).
    pub fn step(&mut self, fingerprint: u64, unsaved: bool, now: Instant) -> Step {
        if fingerprint != self.seen {
            self.seen = fingerprint;
            self.last_change = Some(now);
            if self.first_change.is_none() {
                self.first_change = Some(now);
            }
        }
        let settled = self.in_flight.is_none();
        let marker_due = self.marker_retry_at.is_none_or(|at| now >= at);
        if self.seen == self.committed {
            self.first_change = None;
            if self.marked && settled && marker_due {
                self.marked = false;
                return Step::Unmark;
            }
            return Step::Idle;
        }
        if unsaved && !self.marked && marker_due {
            self.marked = true;
            // Queued behind a running commit, it lands after that commit.
            self.flight_marked = !settled;
            return Step::Mark(self.seen);
        }
        if !unsaved && self.marked && settled && marker_due {
            self.marked = false;
            return Step::Unmark;
        }
        if unsaved
            && !self.flight_marked
            && marker_due
            && self.in_flight.is_some_and(|f| f != self.seen)
        {
            // The newer edit would be lost if the process died right after
            // the running commit lands; the worker records this marker after
            // that commit, so it reads `after` = the commit's sequence.
            self.flight_marked = true;
            return Step::Mark(self.seen);
        }
        if self.in_flight.is_some() || self.retry_at.is_some_and(|at| now < at) {
            return Step::Idle;
        }
        let quiet = self
            .last_change
            .is_none_or(|t| now.duration_since(t) >= self.cadence.quiet);
        let overdue = self
            .first_change
            .is_none_or(|t| now.duration_since(t) >= self.cadence.max_delay);
        if quiet || overdue || self.retry_at.is_some() {
            self.in_flight = Some(self.seen);
            return Step::Commit(self.seen);
        }
        Step::Idle
    }

    /// The commit of `fingerprint` finished.
    pub fn finished(&mut self, fingerprint: u64, ok: bool, now: Instant) {
        if self.in_flight == Some(fingerprint) {
            self.in_flight = None;
        }
        if ok {
            self.committed = fingerprint;
            self.retry_at = None;
            // Edits made while it ran need their own commit; they already
            // have a marker if one was queued behind the commit.
            self.marked = self.flight_marked;
            self.first_change = (self.seen != self.committed).then_some(now);
        } else {
            self.retry_at = Some(now + self.cadence.retry);
        }
        self.flight_marked = false;
    }

    /// A marker or commit could not be queued: try again on a later step.
    pub fn unqueued(&mut self, step: Step) {
        match step {
            // Marking again is harmless; a missing marker is not.
            Step::Mark(_) => {
                self.marked = false;
                self.flight_marked = false;
            }
            Step::Unmark => self.marked = true,
            Step::Commit(fingerprint) if self.in_flight == Some(fingerprint) => {
                self.in_flight = None;
            }
            _ => {}
        }
    }

    /// The worker could not write the marker change `step` (a `Mark` or an
    /// `Unmark`): the disk does not hold what this policy assumed, so the
    /// transition is taken again. A marker is a few hundred bytes and, while
    /// commits fail, the only record of unsaved work, so it is retried after
    /// the quiet period rather than the commit retry interval.
    pub fn marker_failed(&mut self, step: Step, now: Instant) {
        self.unqueued(step);
        self.marker_retry_at = Some(now + self.cadence.quiet);
    }

    /// A restored schedule (`fingerprint`) is being committed by the
    /// recovery answer; it needs no separate marker.
    pub fn restoring(&mut self, fingerprint: u64) {
        self.seen = fingerprint;
        self.in_flight = Some(fingerprint);
        self.marked = true;
        self.first_change = None;
        self.last_change = None;
    }

    /// A close committed (or found committed) the schedule `fingerprint`.
    pub fn flushed(&mut self, fingerprint: u64) {
        self.committed = fingerprint;
        self.in_flight = None;
        self.marked = false;
        self.flight_marked = false;
        self.first_change = None;
        self.retry_at = None;
        self.marker_retry_at = None;
    }
}

#[cfg(test)]
mod tests;
