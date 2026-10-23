//! Durable song history and ordered schedule snapshots. Use `Worker` on UI threads.
use crate::arrangement::{
    Arrangement, Occurrence, OccurrenceId, SectionId, SourceSnapshot, Variant, VariantId,
};
use crate::background::Background;
use crate::format::SlideFormat;
use crate::import::batch::{self, DuplicateKey};
use crate::search;
use crate::song_order::{SongOrder, SongSort};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

const APP: i64 = 0x53454c41;
pub const MAX_SONG_BYTES: usize = 256 * 1024;
/// Most songs one import commit or duplicate check carries.
pub const MAX_IMPORT_SONGS: usize = batch::MAX_FILES;
/// One song of an import commit: a new song, or a new revision of `replace`
/// (the operator chose Replace for a library duplicate).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportWrite {
    pub replace: Option<Version>,
    /// Shared with the staged preview, so a retry after a failure needs no copy.
    pub song: Arc<Song>,
}
/// A current library song an imported song duplicates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryMatch {
    pub version: Version,
    pub title: String,
    /// The words match too, not only the title.
    pub same_lyrics: bool,
}
pub const MAX_ITEMS: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(pub [u8; 16]);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version {
    pub id: Id,
    pub revision: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub id: SectionId,
    pub label: String,
    pub lyrics: String,
    /// The slide's formatting overrides (schema 3); default before that.
    pub format: SlideFormat,
    /// The slide's own background (schema 4); `None` follows the master.
    pub background: Option<Background>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    pub title: String,
    pub authors: String,
    pub copyright: String,
    pub license: String,
    /// Song properties Administrator and Song Number (schema 6).
    pub administrator: String,
    pub song_number: String,
    pub sections: Vec<Section>,
    pub variants: Vec<Variant>,
    /// Background for slides without their own (schema 4); `None` is black.
    pub master: Option<Background>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
    pub title: String,
    pub items: Vec<Version>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Conflict,
    Missing,
    Unsupported,
    Corrupt,
    Locked,
    Full,
    Io,
    Busy,
    Canceled,
    Closed,
    Exists,
    Budget,
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::SqliteFailure(e, _) => match e.code {
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                    Self::Locked
                }
                rusqlite::ErrorCode::DiskFull => Self::Full,
                rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                    Self::Corrupt
                }
                _ => Self::Io,
            },
            _ => Self::Corrupt,
        }
    }
}
type Result<T> = std::result::Result<T, Error>;
fn text_ok(s: &str, max: usize) -> bool {
    s.len() <= max
}
/// Byte limit of the Administrator property.
pub const MAX_ADMINISTRATOR: usize = 256;
/// Digit limit of the Song Number property (CCLI song numbers have at most
/// eight digits today; the margin keeps the field from rejecting real ones).
pub const MAX_SONG_NUMBER: usize = 12;
/// Song Number is empty or ASCII digits only.
pub fn song_number_ok(s: &str) -> bool {
    s.len() <= MAX_SONG_NUMBER && s.bytes().all(|b| b.is_ascii_digit())
}
impl Song {
    pub fn arrangement(&self, version: Version) -> Result<Arrangement> {
        let sections = self
            .sections
            .iter()
            .map(|s| crate::arrangement::Section {
                id: s.id,
                label: s.label.clone(),
                lyrics: s.lyrics.clone(),
            })
            .collect();
        Arrangement::new(
            SourceSnapshot::new(version, sections).map_err(|_| Error::Invalid)?,
            self.variants.clone(),
        )
        .map_err(|_| Error::Invalid)
    }
    pub fn validate(&self) -> Result<()> {
        self.validate_text()?;
        for (index, section) in self.sections.iter().enumerate() {
            if self.sections[..index].iter().any(|s| s.id == section.id) {
                return Err(Error::Invalid);
            }
        }
        // Unarranged legacy text retains its original codec budget. Creating an
        // arrangement also requires the domain's stricter text-plus-ID budget.
        if !self.variants.is_empty() {
            self.arrangement(Version {
                id: Id([0; 16]),
                revision: 1,
            })?;
        }
        Ok(())
    }
    fn validate_text(&self) -> Result<()> {
        if !text_ok(&self.title, 1024)
            || self.title.trim().is_empty()
            || [&self.authors, &self.copyright, &self.license]
                .iter()
                .any(|s| !text_ok(s, 4096))
            || !text_ok(&self.administrator, MAX_ADMINISTRATOR)
            || !song_number_ok(&self.song_number)
            || self.sections.len() > 128
            || self.sections.iter().any(|s| s.label.len() > 256)
            || self.sections.iter().any(|s| !s.format.is_valid())
            || self
                .sections
                .iter()
                .filter_map(|s| s.background.as_ref())
                .chain(&self.master)
                .any(|b| !b.is_valid())
        {
            return Err(Error::Invalid);
        }
        // The payload codec holds only these strings; administrator and song
        // number live in `song_metadata` and add their bytes, not a prefix, so
        // a song at the legacy payload limit stays valid.
        let size = [&self.title, &self.authors, &self.copyright, &self.license]
            .into_iter()
            .chain(self.sections.iter().flat_map(|s| [&s.label, &s.lyrics]))
            .try_fold(0usize, |n, s| n.checked_add(s.len())?.checked_add(4))
            .and_then(|n| n.checked_add(self.administrator.len() + self.song_number.len()));
        if size.is_none_or(|n| n > MAX_SONG_BYTES) {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for s in [&self.title, &self.authors, &self.copyright, &self.license]
            .into_iter()
            .chain(self.sections.iter().flat_map(|s| [&s.label, &s.lyrics]))
        {
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        out
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_SONG_BYTES {
            return Err(Error::Corrupt);
        }
        let mut rest = bytes;
        let mut strings = Vec::new();
        while !rest.is_empty() {
            let len = u32::from_le_bytes(
                rest.get(..4)
                    .ok_or(Error::Corrupt)?
                    .try_into()
                    .map_err(|_| Error::Corrupt)?,
            ) as usize;
            rest = &rest[4..];
            strings.push(
                std::str::from_utf8(rest.get(..len).ok_or(Error::Corrupt)?)
                    .map_err(|_| Error::Corrupt)?
                    .to_owned(),
            );
            rest = &rest[len..];
            if strings.len() > 260 {
                return Err(Error::Corrupt);
            }
        }
        if strings.len() < 4 || strings.len() % 2 != 0 {
            return Err(Error::Corrupt);
        }
        let mut it = strings.into_iter();
        let song = Self {
            title: it.next().unwrap(),
            authors: it.next().unwrap(),
            copyright: it.next().unwrap(),
            license: it.next().unwrap(),
            administrator: String::new(),
            song_number: String::new(),
            variants: Vec::new(),
            master: None,
            sections: {
                let mut v = Vec::new();
                while let Some(label) = it.next() {
                    v.push(Section {
                        id: SectionId(Id([0; 16])), // unbound legacy text, never identity
                        label,
                        lyrics: it.next().unwrap(),
                        format: SlideFormat::default(),
                        background: None,
                    });
                }
                v
            },
        };
        song.validate_text().map_err(|_| Error::Corrupt)?;
        Ok(song)
    }
}
const SCHEMA: &str = "CREATE TABLE songs(id BLOB PRIMARY KEY CHECK(length(id)=16), head INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE song_revisions(id BLOB NOT NULL REFERENCES songs(id), revision INTEGER NOT NULL, payload BLOB NOT NULL CHECK(length(payload)<=262144), PRIMARY KEY(id,revision));
CREATE TABLE schedules(id BLOB PRIMARY KEY CHECK(length(id)=16), head INTEGER NOT NULL);
CREATE TABLE schedule_revisions(id BLOB NOT NULL REFERENCES schedules(id), revision INTEGER NOT NULL, title TEXT NOT NULL, PRIMARY KEY(id,revision));
CREATE TABLE items(id BLOB NOT NULL, revision INTEGER NOT NULL, position INTEGER NOT NULL, song BLOB NOT NULL, song_revision INTEGER NOT NULL, PRIMARY KEY(id,revision,position), FOREIGN KEY(id,revision) REFERENCES schedule_revisions(id,revision), FOREIGN KEY(song,song_revision) REFERENCES song_revisions(id,revision));
PRAGMA application_id=1397050433; PRAGMA user_version=1;";
const SCHEMA2: &str = "CREATE TABLE section_ids(song BLOB NOT NULL, revision INTEGER NOT NULL, position INTEGER NOT NULL, section BLOB NOT NULL CHECK(length(section)=16), PRIMARY KEY(song,revision,position), UNIQUE(song,revision,section), FOREIGN KEY(song,revision) REFERENCES song_revisions(id,revision));
CREATE TABLE variants(song BLOB NOT NULL, revision INTEGER NOT NULL, position INTEGER NOT NULL, variant BLOB NOT NULL CHECK(length(variant)=16), name TEXT NOT NULL, PRIMARY KEY(song,revision,variant), UNIQUE(song,revision,position), FOREIGN KEY(song,revision) REFERENCES song_revisions(id,revision));
CREATE TABLE occurrences(song BLOB NOT NULL, revision INTEGER NOT NULL, variant BLOB NOT NULL, position INTEGER NOT NULL, occurrence BLOB NOT NULL CHECK(length(occurrence)=16), section BLOB NOT NULL, PRIMARY KEY(song,revision,variant,position), UNIQUE(song,revision,variant,occurrence), FOREIGN KEY(song,revision,variant) REFERENCES variants(song,revision,variant), FOREIGN KEY(song,revision,section) REFERENCES section_ids(song,revision,section));
PRAGMA user_version=2;";
// Rows exist only for slides with non-default formatting.
const SCHEMA3: &str = "CREATE TABLE section_formats(song BLOB NOT NULL, revision INTEGER NOT NULL, position INTEGER NOT NULL, format BLOB NOT NULL CHECK(length(format)<=256), PRIMARY KEY(song,revision,position), FOREIGN KEY(song,revision,position) REFERENCES section_ids(song,revision,position));
PRAGMA user_version=3;";
// Rows exist only for a song master and for slides with their own background.
const SCHEMA4: &str = "CREATE TABLE song_backgrounds(song BLOB NOT NULL, revision INTEGER NOT NULL, background BLOB NOT NULL CHECK(length(background)<=320), PRIMARY KEY(song,revision), FOREIGN KEY(song,revision) REFERENCES song_revisions(id,revision));
CREATE TABLE section_backgrounds(song BLOB NOT NULL, revision INTEGER NOT NULL, position INTEGER NOT NULL, background BLOB NOT NULL CHECK(length(background)<=320), PRIMARY KEY(song,revision,position), FOREIGN KEY(song,revision,position) REFERENCES section_ids(song,revision,position));
PRAGMA user_version=4;";
// Derived search index over each nondeleted song's head revision, rebuilt from
// the payloads whenever it is missing or damaged. `search_rows` gives every
// song a stable INTEGER PRIMARY KEY, because VACUUM may renumber the implicit
// rowids of `songs`. Diacritic folding mode 2 also folds letters that mode 1
// leaves unchanged.
// Since schema 7 the index also holds the section labels (EW8-OBS-046 matches
// them) and `search_rows` carries the derived Sort by keys of EW8-OBS-064
// (Author, Copyright, Administrator, Song Number), so the Songs list sorts
// without decoding a payload. Schema 5 and 6 files are rebuilt to this shape.
const SEARCH: &str = "CREATE TABLE search_rows(row INTEGER PRIMARY KEY, song BLOB NOT NULL UNIQUE REFERENCES songs(id), author TEXT NOT NULL DEFAULT '', copyright TEXT NOT NULL DEFAULT '', administrator TEXT NOT NULL DEFAULT '', song_number TEXT NOT NULL DEFAULT '');
CREATE VIRTUAL TABLE song_search USING fts5(title, lyrics, metadata, labels, tokenize='unicode61 remove_diacritics 2');";
// A stored `modified` per song replaces the search index row order as the
// Date modified key, so it survives an index rebuild. Existing songs take
// their index row number, which keeps the order the list showed before the
// migration (and is deterministic: rows were assigned in id order or by last
// save). Saves write milliseconds since the epoch, which is far above any row
// number. `song_metadata` rows exist only for a revision with an
// Administrator or Song Number.
const SCHEMA6: &str = "ALTER TABLE songs ADD COLUMN modified INTEGER NOT NULL DEFAULT 0;
CREATE INDEX songs_modified ON songs(modified);
CREATE TABLE song_metadata(song BLOB NOT NULL, revision INTEGER NOT NULL, administrator TEXT NOT NULL CHECK(length(CAST(administrator AS BLOB))<=256), song_number TEXT NOT NULL CHECK(length(CAST(song_number AS BLOB))<=12), PRIMARY KEY(song,revision), FOREIGN KEY(song,revision) REFERENCES song_revisions(id,revision));
UPDATE songs SET modified=COALESCE((SELECT m.row FROM search_rows m WHERE m.song=songs.id),0);
PRAGMA user_version=6;";
// Schema 7 only reshapes the derived search index (labels column and sort
// keys, see `SEARCH`); the migration rebuilds it from the song payloads.
const SCHEMA7: &str = "PRAGMA user_version=7;";
const SCHEMA_VERSIONS: [i64; 7] = [1, 2, 3, 4, 5, 6, 7];
const CURRENT: i64 = 7;
// bm25 column weights: title, lyrics, metadata, labels. FTS5's bm25
// normalizes by the length of the whole row, so a low label weight alone does
// not keep a short song's label hit below a long song's lyric hit. `l` sorts
// first instead: with the labels weight at 0 a row scores exactly 0 only when
// every hit is in its labels (each term's idf is positive), so label-only
// rows always follow the rows with a title, lyric or metadata hit, and bm25
// with labels at 0.5 orders within each group. A query that matches no label
// puts every row in the first group.
/// Ranks and limits inside the FTS scan, so only the kept rows are joined to
/// song heads (about 20% faster on 20k songs than joining every match first).
const SEARCH_SQL: &str = "SELECT m.song,s.head,t.title FROM (SELECT f.rowid AS row,f.title AS title,bm25(song_search,10.0,1.0,2.0,0.0)=0 AS l,bm25(song_search,10.0,1.0,2.0,0.5) AS r FROM song_search f WHERE song_search MATCH ?1 ORDER BY l,r,f.title COLLATE NOCASE,f.title,f.rowid LIMIT ?2) t JOIN search_rows m ON m.row=t.row JOIN songs s ON s.id=m.song WHERE s.deleted=0 ORDER BY t.l,t.r,t.title COLLATE NOCASE,t.title,m.song";

impl SectionId {
    /// CPU-only allocation with a process-local counter; no random-device I/O.
    /// IDs are opaque, not derived from lyrics, labels or vector positions.
    pub fn allocate() -> Self {
        use sha2::{Digest, Sha256};
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let mut hash = Sha256::new();
        hash.update(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .to_le_bytes(),
        );
        hash.update(std::process::id().to_le_bytes());
        hash.update(COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
        Self(Id(hash.finalize()[..16].try_into().unwrap()))
    }
}
/// Synchronous API: exclusively for background threads; all methods may perform I/O.
pub struct Repository {
    db: Connection,
}

// Private same-filesystem staging. Process termination may leave this directory;
// it is never a published backup and is never automatically trusted or restored.
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
const MAX_BACKUP_BYTES: i64 = 256 * 1024 * 1024;

fn copy_new(source: &Connection, destination: &std::path::Path, cancel: &AtomicBool) -> Result<()> {
    use rusqlite::backup::{Backup, StepResult};
    if destination.as_os_str().len() > 4096 || destination.file_name().is_none() {
        return Err(Error::Invalid);
    }
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = destination.as_os_str().to_owned();
        name.push(suffix);
        match std::fs::symlink_metadata(std::path::Path::new(&name)) {
            Ok(_) => return Err(Error::Exists),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(Error::Io),
        }
    }
    let app: i64 = source.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    let version: i64 = source.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if app != APP || !SCHEMA_VERSIONS.contains(&version) {
        return Err(Error::Unsupported);
    }
    let page_size: i64 = source.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let pages: i64 = source.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    if pages.saturating_mul(page_size) > MAX_BACKUP_BYTES {
        return Err(Error::Budget);
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let nonce: String = source.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
    let directory = parent.join(format!(".sela-backup-{nonce}"));
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&directory).map_err(|_| Error::Io)?;
    let staging = Staging(directory);
    let path = staging.0.join("database");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&path).map_err(|_| Error::Io)?;
    let mut target = Connection::open(&path)?;
    target.execute_batch("PRAGMA synchronous=FULL; PRAGMA cache_size=-2048;")?;
    target.busy_timeout(Duration::from_millis(100))?;
    let start = std::time::Instant::now();
    {
        let backup = Backup::new(source, &mut target)?;
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err(Error::Canceled);
            }
            if start.elapsed() >= Duration::from_secs(5) {
                return Err(Error::Budget);
            }
            let step = backup.step(64)?;
            #[cfg(test)]
            if std::env::var_os("SELA_ABORT_BACKUP_STEP").is_some() {
                assert!(matches!(step, StepResult::More));
                std::process::abort();
            }
            if i64::from(backup.progress().pagecount).saturating_mul(page_size) > MAX_BACKUP_BYTES {
                return Err(Error::Budget);
            }
            match step {
                StepResult::Done => break,
                StepResult::More => (),
                StepResult::Busy | StepResult::Locked => return Err(Error::Locked),
                _ => return Err(Error::Io),
            }
        }
    }
    // A WAL source can transfer WAL header mode. Convert only the private copy.
    target.execute_batch("PRAGMA journal_mode=DELETE;")?;
    target.close().map_err(|(_, e)| Error::from(e))?;
    let verified = Repository::open_internal(&path, false)?;
    verified.verify_history(cancel, start)?;
    verified.db.close().map_err(|(_, e)| Error::from(e))?;
    // Windows FlushFileBuffers requires a handle with write access.
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Io)?;
    if cancel.load(Ordering::Acquire) {
        return Err(Error::Canceled);
    }
    // hard_link is atomic no-replace (unlike rename). No fallible operation after
    // publication affects the reported outcome; directory durability is unqualified.
    std::fs::hard_link(&path, destination).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            Error::Exists
        } else {
            Error::Io
        }
    })?;
    Ok(())
}
impl Repository {
    /// Blocking, worker-only. Publishes a verified standalone database, never overwrites.
    pub fn backup_new(&self, destination: &std::path::Path, cancel: &AtomicBool) -> Result<()> {
        copy_new(&self.db, destination, cancel)
    }
    /// Read-only input; no migration, reset or in-place recovery. Parent must exist.
    pub fn restore_new(
        source: &std::path::Path,
        destination: &std::path::Path,
        cancel: &AtomicBool,
    ) -> Result<()> {
        if source.as_os_str().len() > 4096 {
            return Err(Error::Invalid);
        }
        let db = Connection::open_with_flags(
            source,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        db.busy_timeout(Duration::from_millis(100))?;
        db.execute_batch("PRAGMA cache_size=-2048;")?;
        copy_new(&db, destination, cancel)
    }
    fn verify_history(&self, cancel: &AtomicBool, start: std::time::Instant) -> Result<()> {
        let check_budget = || {
            if cancel.load(Ordering::Acquire) {
                Err(Error::Canceled)
            } else if start.elapsed() >= Duration::from_secs(5) {
                Err(Error::Budget)
            } else {
                Ok(())
            }
        };
        // Heads must resolve and positions must describe a contiguous ordered list.
        if self.db.prepare("SELECT 1 FROM songs s WHERE head<1 OR deleted NOT IN (0,1) OR NOT EXISTS(SELECT 1 FROM song_revisions r WHERE r.id=s.id AND r.revision=s.head) UNION ALL SELECT 1 FROM schedules s WHERE head<1 OR NOT EXISTS(SELECT 1 FROM schedule_revisions r WHERE r.id=s.id AND r.revision=s.head) UNION ALL SELECT 1 FROM items GROUP BY id,revision HAVING min(position)!=0 OR max(position)!=count(*)-1")?.exists([])? {
            return Err(Error::Corrupt);
        }
        let mut statement = self.db.prepare("SELECT id,revision FROM song_revisions")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            check_budget()?;
            self.song(Version {
                id: read_id(row.get(0)?)?,
                revision: row.get(1)?,
            })?;
        }
        let mut statement = self
            .db
            .prepare("SELECT id,revision FROM schedule_revisions")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            check_budget()?;
            let id: Vec<u8> = row.get(0)?;
            self.schedule(Version {
                id: Id(id.try_into().map_err(|_| Error::Corrupt)?),
                revision: row.get(1)?,
            })?;
        }
        Ok(())
    }
    /// Bounded stable-ID pagination; cursor is exclusive, not a search/ranking API.
    pub fn heads(&self, schedules: bool, after: Option<Id>) -> Result<Vec<Version>> {
        let sql = if schedules {
            "SELECT id,head FROM schedules WHERE id>COALESCE(?,x'') ORDER BY id LIMIT 128"
        } else {
            "SELECT id,head FROM songs WHERE deleted=0 AND id>COALESCE(?,x'') ORDER BY id LIMIT 128"
        };
        self.db
            .prepare(sql)?
            .query_map(params![after.map(|id| id.0.to_vec())], |r| {
                let id: Vec<u8> = r.get(0)?;
                Ok((id, r.get::<_, i64>(1)?))
            })?
            .map(|r| {
                let (id, revision) = r?;
                Ok(Version {
                    id: Id(id.try_into().map_err(|_| Error::Corrupt)?),
                    revision,
                })
            })
            .collect()
    }
    /// Bounded browser page. Decode one bounded payload at a time off the UI thread.
    pub fn catalog(&self, after: Option<Id>) -> Result<Vec<(Version, String)>> {
        self.heads(false, after)?
            .into_iter()
            .map(|version| Ok((version, self.song(version)?.title)))
            .collect()
    }
    /// Every current song with its title, in `order`, for the Songs list.
    /// Titles come from the search index, so no song payload is decoded.
    /// "Modified" is the stored `modified` stamp (schema 6), written by every
    /// save, so it is independent of the search index.
    ///
    /// Author, Copyright, Administrator and Song Number (schema 7 sort keys)
    /// put songs with an empty key last in either direction, order the rest
    /// by the key (ignoring ASCII case; Song Number numerically) and break
    /// ties by title ascending, then id. These rules are UNOBSERVED best
    /// guesses (docs/reference-observations.md, EW8-OBS-064).
    pub fn library(&self, order: SongOrder) -> Result<Vec<(Version, String)>> {
        let direction = if order.descending { "DESC" } else { "ASC" };
        let keyed = |key: &str| {
            format!(
                "m.{key}='',m.{key} COLLATE NOCASE {direction},m.{key} {direction},f.title COLLATE NOCASE,f.title,s.id"
            )
        };
        let ordering = match order.sort {
            SongSort::Title => {
                format!("f.title COLLATE NOCASE {direction},f.title {direction},s.id")
            }
            SongSort::Modified => format!("s.modified {direction},s.id {direction}"),
            SongSort::Author => keyed("author"),
            SongSort::Copyright => keyed("copyright"),
            SongSort::Administrator => keyed("administrator"),
            // Digits only (`song_number_ok`, at most 12, so it fits i64).
            SongSort::SongNumber => format!(
                "m.song_number='',CAST(m.song_number AS INTEGER) {direction},m.song_number {direction},f.title COLLATE NOCASE,f.title,s.id"
            ),
        };
        let sql = format!(
            "SELECT s.id,s.head,f.title FROM songs s JOIN search_rows m ON m.song=s.id JOIN song_search f ON f.rowid=m.row WHERE s.deleted=0 ORDER BY {ordering}"
        );
        let mut statement = self.db.prepare_cached(&sql)?;
        let mut rows = statement.query([])?;
        let mut songs = Vec::new();
        while let Some(row) = rows.next()? {
            let title: String = row.get(2)?;
            if title.len() > 1024 {
                return Err(Error::Corrupt);
            }
            songs.push((
                Version {
                    id: read_id(row.get(0)?)?,
                    revision: row.get(1)?,
                },
                title,
            ));
        }
        Ok(songs)
    }
    /// Saved schedules at their current revision, with titles; items are not
    /// resolved. Same paging as `heads`.
    pub fn schedule_catalog(&self, after: Option<Id>) -> Result<Vec<(Version, String)>> {
        self.heads(true, after)?
            .into_iter()
            .map(|version| {
                let title: String = self
                    .db
                    .query_row(
                        "SELECT title FROM schedule_revisions WHERE id=? AND revision=?",
                        params![&version.id.0[..], version.revision],
                        |r| r.get(0),
                    )
                    .optional()?
                    .ok_or(Error::Corrupt)?;
                if title.len() > 1024 || title.trim().is_empty() {
                    return Err(Error::Corrupt);
                }
                Ok((version, title))
            })
            .collect()
    }
    pub fn open(path: &std::path::Path) -> Result<Self> {
        Self::open_internal(path, true)
    }
    fn open_internal(path: &std::path::Path, upgrade: bool) -> Result<Self> {
        use rusqlite::{OpenFlags, limits::Limit};
        let mut db = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        db.busy_timeout(Duration::from_millis(100))?;
        db.set_limit(Limit::SQLITE_LIMIT_LENGTH, (MAX_SONG_BYTES + 4096) as i32)?;
        db.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA cache_size=-2048; PRAGMA synchronous=FULL;",
        )?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let app: i64 = tx.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        let mut version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if app == 0 && version == 0 {
            let count: i64 = tx.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )?;
            if count != 0 {
                return Err(Error::Unsupported);
            }
            tx.execute_batch(SCHEMA)?;
            tx.execute_batch(SCHEMA2)?;
            tx.execute_batch(SCHEMA3)?;
            tx.execute_batch(SCHEMA4)?;
            tx.execute_batch(SEARCH)?;
            tx.execute_batch(SCHEMA6)?;
            tx.execute_batch(SCHEMA7)?;
            version = CURRENT;
        } else if app != APP || !SCHEMA_VERSIONS.contains(&version) {
            return Err(Error::Unsupported);
        }
        // quick_check also runs the FTS5 index check. When dropping the derived
        // index makes the file pass, only the index was damaged: a current
        // file rebuilds it, an older one (schema 5 and 6 hold the index too)
        // leaves the rebuild to the schema 7 migration step. Opening an older
        // file without upgrading (verifying its backup copy) keeps the copy as
        // it was and skips the index shape check. Otherwise the transaction
        // rolls the drop back.
        let mut damaged_index = false;
        if !quick_check(&tx)? {
            if version < 5 {
                return Err(Error::Corrupt);
            }
            tx.execute_batch("SAVEPOINT index_check; DROP TABLE IF EXISTS song_search")
                .map_err(|_| Error::Corrupt)?;
            if !quick_check(&tx)? {
                return Err(Error::Corrupt);
            }
            if version == CURRENT {
                tx.execute_batch("RELEASE index_check")?;
                rebuild_search(&tx)?;
            } else if upgrade {
                tx.execute_batch("RELEASE index_check")?;
            } else {
                tx.execute_batch("ROLLBACK TO index_check; RELEASE index_check")?;
                damaged_index = true;
            }
        } else if version == CURRENT && !search_consistent(&tx)? {
            rebuild_search(&tx)?;
        }
        if tx.prepare("PRAGMA foreign_key_check")?.exists([])? {
            return Err(Error::Corrupt);
        }
        // Verify required columns before accepting an otherwise foreign schema.
        tx.prepare("SELECT s.deleted,r.payload FROM songs s JOIN song_revisions r ON s.id=r.id")?;
        tx.prepare("SELECT r.title,i.position,i.song_revision FROM schedule_revisions r JOIN items i ON r.id=i.id AND r.revision=i.revision JOIN schedules s ON s.id=r.id")?;
        if version < CURRENT && upgrade {
            // Hold the writer reservation across backup and migration. The separate
            // read connection copies the same committed state, without attempting
            // an online backup from a connection with an active write transaction.
            let source = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            source.busy_timeout(Duration::from_millis(100))?;
            let mut backup = path.as_os_str().to_owned();
            backup.push(format!(".schema{version}-backup"));
            copy_new(
                &source,
                std::path::Path::new(&backup),
                &AtomicBool::new(false),
            )?;
        }
        if version == 1 && upgrade {
            tx.execute_batch(SCHEMA2)?;
            let mut statement = tx.prepare("SELECT id,revision,payload FROM song_revisions")?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                let id: Vec<u8> = row.get(0)?;
                let revision: i64 = row.get(1)?;
                let mut song = Song::decode(&row.get::<_, Vec<u8>>(2)?)?;
                for (position, section) in song.sections.iter_mut().enumerate() {
                    section.id = SectionId(read_id(tx.query_row(
                        "SELECT randomblob(16)",
                        [],
                        |r| r.get(0),
                    )?)?);
                    tx.execute(
                        "INSERT INTO section_ids VALUES(?,?,?,?)",
                        params![id, revision, position as i64, &section.id.0.0[..]],
                    )?;
                    #[cfg(test)]
                    if std::env::var_os("SELA_ABORT_MIGRATION_ROW").is_some() {
                        std::process::abort();
                    }
                }
                song.validate().map_err(|_| Error::Corrupt)?;
            }
        }
        if version < 3 && upgrade {
            tx.execute_batch(SCHEMA3)?;
            #[cfg(test)]
            if std::env::var_os("SELA_ABORT_MIGRATION_SCHEMA3").is_some() {
                std::process::abort();
            }
        }
        if version < 4 && upgrade {
            tx.execute_batch(SCHEMA4)?;
            #[cfg(test)]
            if std::env::var_os("SELA_ABORT_MIGRATION_SCHEMA4").is_some() {
                std::process::abort();
            }
        }
        if version < 5 && upgrade {
            tx.execute_batch(SEARCH)?;
            fill_search(&tx)?;
            tx.execute_batch("PRAGMA user_version=5")?;
            #[cfg(test)]
            if std::env::var_os("SELA_ABORT_MIGRATION_SCHEMA5").is_some() {
                std::process::abort();
            }
        }
        if version < 6 && upgrade {
            tx.execute_batch(SCHEMA6)?;
            #[cfg(test)]
            if std::env::var_os("SELA_ABORT_MIGRATION_SCHEMA6").is_some() {
                std::process::abort();
            }
        }
        if version < 7 && upgrade {
            // A schema 5 or 6 index lacks the labels column and the sort
            // keys; an older file got the new shape in the schema 5 step.
            if version >= 5 {
                rebuild_search(&tx)?;
            }
            tx.execute_batch(SCHEMA7)?;
            #[cfg(test)]
            if std::env::var_os("SELA_ABORT_MIGRATION_SCHEMA7").is_some() {
                std::process::abort();
            }
        }
        if version >= 2 || upgrade {
            tx.prepare("SELECT section FROM section_ids")?;
            tx.prepare("SELECT variant,name FROM variants")?;
            tx.prepare("SELECT occurrence,section FROM occurrences")?;
        }
        if version >= 3 || upgrade {
            tx.prepare("SELECT position,format FROM section_formats")?;
        }
        if version >= 4 || upgrade {
            tx.prepare("SELECT background FROM song_backgrounds")?;
            tx.prepare("SELECT position,background FROM section_backgrounds")?;
        }
        if version >= 5 || upgrade {
            tx.prepare("SELECT row,song FROM search_rows")?;
            if !damaged_index {
                tx.prepare("SELECT title,lyrics,metadata FROM song_search")?;
            }
        }
        if version >= 6 || upgrade {
            tx.prepare("SELECT id,modified FROM songs")?;
            tx.prepare("SELECT song,revision,administrator,song_number FROM song_metadata")?;
        }
        if version >= 7 || upgrade {
            tx.prepare("SELECT author,copyright,administrator,song_number FROM search_rows")?;
            tx.prepare("SELECT labels FROM song_search")?;
        }
        tx.commit()?;
        Ok(Self { db })
    }
    pub fn song(&self, v: Version) -> Result<Song> {
        if v.revision <= 0 {
            return Err(Error::Corrupt);
        }
        let b: Vec<u8> = self
            .db
            .query_row(
                "SELECT payload FROM song_revisions WHERE id=? AND revision=?",
                params![&v.id.0[..], v.revision],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(Error::Missing)?;
        let mut song = Song::decode(&b)?;
        let schema: i64 = self.db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if schema == 1 {
            return Ok(song);
        } // private legacy backup verification only
        let mut statement = self.db.prepare("SELECT position,section FROM section_ids WHERE song=? AND revision=? ORDER BY position LIMIT 129")?;
        let ids = statement
            .query_map(params![&v.id.0[..], v.revision], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if ids.len() != song.sections.len() {
            return Err(Error::Corrupt);
        }
        for (index, (position, id)) in ids.into_iter().enumerate() {
            if position != index as i64 {
                return Err(Error::Corrupt);
            }
            song.sections[index].id = SectionId(read_id(id)?);
        }
        if schema >= 3 {
            let mut statement = self.db.prepare("SELECT position,format FROM section_formats WHERE song=? AND revision=? ORDER BY position LIMIT 129")?;
            let mut rows = statement.query(params![&v.id.0[..], v.revision])?;
            while let Some(row) = rows.next()? {
                let section = usize::try_from(row.get::<_, i64>(0)?)
                    .ok()
                    .and_then(|position| song.sections.get_mut(position))
                    .ok_or(Error::Corrupt)?;
                section.format =
                    SlideFormat::decode(&row.get::<_, Vec<u8>>(1)?).ok_or(Error::Corrupt)?;
            }
        }
        if schema >= 4 {
            let decode = |bytes: Vec<u8>| Background::decode(&bytes).ok_or(Error::Corrupt);
            song.master = self
                .db
                .query_row(
                    "SELECT background FROM song_backgrounds WHERE song=? AND revision=?",
                    params![&v.id.0[..], v.revision],
                    |r| r.get::<_, Vec<u8>>(0),
                )
                .optional()?
                .map(decode)
                .transpose()?;
            let mut statement = self.db.prepare("SELECT position,background FROM section_backgrounds WHERE song=? AND revision=? ORDER BY position LIMIT 129")?;
            let mut rows = statement.query(params![&v.id.0[..], v.revision])?;
            while let Some(row) = rows.next()? {
                let section = usize::try_from(row.get::<_, i64>(0)?)
                    .ok()
                    .and_then(|position| song.sections.get_mut(position))
                    .ok_or(Error::Corrupt)?;
                section.background = Some(decode(row.get(1)?)?);
            }
        }
        if schema >= 6 {
            apply_metadata(&mut song, read_metadata(&self.db, &v.id.0[..], v.revision)?);
        }
        let mut statement = self.db.prepare("SELECT position,variant,name FROM variants WHERE song=? AND revision=? ORDER BY position LIMIT 17")?;
        let mut rows = statement.query(params![&v.id.0[..], v.revision])?;
        while let Some(row) = rows.next()? {
            if row.get::<_, i64>(0)? != song.variants.len() as i64 {
                return Err(Error::Corrupt);
            }
            let id = read_id(row.get(1)?)?;
            let mut occurrences = Vec::new();
            let mut stmt = self.db.prepare("SELECT position,occurrence,section FROM occurrences WHERE song=? AND revision=? AND variant=? ORDER BY position LIMIT 513")?;
            let mut entries = stmt.query(params![&v.id.0[..], v.revision, &id.0[..]])?;
            while let Some(entry) = entries.next()? {
                if entry.get::<_, i64>(0)? != occurrences.len() as i64 {
                    return Err(Error::Corrupt);
                }
                occurrences.push(Occurrence {
                    id: OccurrenceId(read_id(entry.get(1)?)?),
                    section: SectionId(read_id(entry.get(2)?)?),
                });
            }
            song.variants.push(Variant {
                id: VariantId(id),
                name: row.get(2)?,
                occurrences,
            });
        }
        song.validate().map_err(|_| Error::Corrupt)?;
        Ok(song)
    }
    /// Resolves only the requested immutable revision, including tombstoned history.
    pub fn arrangement(&self, v: Version) -> Result<Arrangement> {
        self.song(v)?.arrangement(v)
    }
    pub fn schedule(&self, v: Version) -> Result<(Schedule, Vec<Song>)> {
        let title: String = self
            .db
            .query_row(
                "SELECT title FROM schedule_revisions WHERE id=? AND revision=?",
                params![&v.id.0[..], v.revision],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(Error::Missing)?;
        if title.len() > 1024 || title.trim().is_empty() {
            return Err(Error::Corrupt);
        }
        let mut stmt = self.db.prepare("SELECT song,song_revision FROM items WHERE id=? AND revision=? ORDER BY position LIMIT 33")?;
        let items = stmt
            .query_map(params![&v.id.0[..], v.revision], |r| {
                let b: Vec<u8> = r.get(0)?;
                Ok((b, r.get::<_, i64>(1)?))
            })?
            .map(|r| {
                let (b, revision) = r?;
                Ok(Version {
                    id: Id(b.try_into().map_err(|_| Error::Corrupt)?),
                    revision,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if items.len() > MAX_ITEMS {
            return Err(Error::Corrupt);
        }
        let songs = items
            .iter()
            .map(|v| self.song(*v))
            .collect::<Result<Vec<_>>>()?;
        Ok((Schedule { title, items }, songs))
    }
    pub fn save_song(&mut self, previous: Option<Version>, song: Song) -> Result<Version> {
        song.validate()?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let v = write_song(&tx, previous, &song)?;
        tx.commit()?;
        Ok(v)
    }
    /// Saves every write of an import batch in ONE transaction through the
    /// same path as `save_song`, so the search index follows (M1-07, M1-14).
    /// Any failure, or `canceled` seen between songs or before the commit,
    /// rolls the whole batch back: nothing of it is saved. A `replace`
    /// whose song moved on since it was matched fails with `Conflict`.
    pub fn commit_import(
        &mut self,
        writes: Vec<ImportWrite>,
        canceled: &AtomicBool,
    ) -> Result<Vec<Version>> {
        self.commit_import_until(writes, &|| canceled.load(Ordering::Acquire))
    }
    /// `commit_import` with the cancel check as a function, so a test can
    /// cancel between two songs or right before COMMIT.
    fn commit_import_until(
        &mut self,
        writes: Vec<ImportWrite>,
        canceled: &dyn Fn() -> bool,
    ) -> Result<Vec<Version>> {
        if writes.len() > MAX_IMPORT_SONGS {
            return Err(Error::Invalid);
        }
        for write in &writes {
            write.song.validate()?;
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut saved = Vec::with_capacity(writes.len());
        for write in &writes {
            if canceled() {
                return Err(Error::Canceled);
            }
            saved.push(write_song(&tx, write.replace, &write.song)?);
        }
        if canceled() {
            return Err(Error::Canceled);
        }
        tx.commit()?;
        Ok(saved)
    }
    /// For each key, the current library song it duplicates: one with the
    /// same title and words if any (`same_lyrics`), else the first with the
    /// same title. Titles and words come from the search index, so only the
    /// words of songs whose title matches are read. Ties go to the lowest id.
    pub fn find_duplicates(&self, keys: &[DuplicateKey]) -> Result<Vec<Option<LibraryMatch>>> {
        if keys.len() > MAX_IMPORT_SONGS {
            return Err(Error::Invalid);
        }
        let mut by_title: HashMap<&str, Vec<usize>> = HashMap::new();
        for (index, key) in keys.iter().enumerate() {
            by_title.entry(key.title.as_str()).or_default().push(index);
        }
        let mut found: Vec<Option<LibraryMatch>> = vec![None; keys.len()];
        if by_title.is_empty() {
            return Ok(found);
        }
        let mut statement = self.db.prepare_cached("SELECT s.id,s.head,f.title,m.row FROM songs s JOIN search_rows m ON m.song=s.id JOIN song_search f ON f.rowid=m.row WHERE s.deleted=0 ORDER BY s.id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let title: String = row.get(2)?;
            let Some(indices) = by_title.get(batch::title_key(&title).as_str()) else {
                continue;
            };
            let version = Version {
                id: read_id(row.get(0)?)?,
                revision: row.get(1)?,
            };
            let lyrics: String = self.db.query_row(
                "SELECT lyrics FROM song_search WHERE rowid=?",
                params![row.get::<_, i64>(3)?],
                |r| r.get(0),
            )?;
            let digest = batch::lyrics_key(&lyrics);
            for &index in indices {
                let same_lyrics = keys[index].lyrics == digest;
                let better = match &found[index] {
                    None => true,
                    Some(current) => same_lyrics && !current.same_lyrics,
                };
                if better {
                    found[index] = Some(LibraryMatch {
                        version,
                        title: title.clone(),
                        same_lyrics,
                    });
                }
            }
        }
        Ok(found)
    }
    /// The song at each version, in order, for a schedule file to embed.
    pub fn songs(&self, versions: &[Version]) -> Result<Vec<Song>> {
        // Twice a schedule: an update review loads the old and new revision.
        if versions.len() > 2 * MAX_ITEMS {
            return Err(Error::Invalid);
        }
        versions.iter().map(|v| self.song(*v)).collect()
    }
    /// Resolves a schedule file's song to a library revision. An identical
    /// revision already in the library is used as is. Otherwise the content
    /// is stored as a hidden revision (a tombstoned song, never listed or
    /// searched) under an id derived from the content, so opening the same
    /// file again reuses it and the Songs list never changes.
    pub fn import_hidden(&mut self, source: Version, song: &Song) -> Result<Version> {
        song.validate()?;
        if self.song(source).is_ok_and(|existing| &existing == song) {
            return Ok(source);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"sela-schedule-song-v1");
        hasher.update(source.id.0);
        hasher.update(source.revision.to_le_bytes());
        hasher.update(crate::schedule_file::song_json(song).to_string());
        let digest = hasher.finalize();
        let v = Version {
            id: Id(digest[..16].try_into().map_err(|_| Error::Corrupt)?),
            revision: 1,
        };
        let present: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM songs WHERE id=?)",
            params![&v.id.0[..]],
            |r| r.get(0),
        )?;
        if present {
            return match self.song(v) {
                Ok(existing) if &existing == song => Ok(v),
                _ => Err(Error::Corrupt),
            };
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO songs(id,head,deleted) VALUES(?,1,1)",
            params![&v.id.0[..]],
        )?;
        insert_revision(&tx, v, song)?;
        tx.commit()?;
        Ok(v)
    }
    pub fn delete_song(&mut self, v: Version) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute(
            "UPDATE songs SET deleted=1 WHERE id=? AND head=? AND deleted=0",
            params![&v.id.0[..], v.revision],
        )? != 1
        {
            return Err(Error::Conflict);
        }
        unindex_song(&tx, v.id)?;
        tx.commit()?;
        Ok(())
    }
    /// Ranked current songs matching every term of `query` (see `search`);
    /// an empty or punctuation-only query returns no hits without I/O.
    pub fn search(&self, query: &str, generation: u64) -> Result<search::Results> {
        if query.len() > search::MAX_QUERY_BYTES {
            return Err(Error::Invalid);
        }
        let mut results = search::Results {
            generation,
            hits: Vec::new(),
            truncated: false,
        };
        let Some(expression) = search::expression(query) else {
            return Ok(results);
        };
        let mut statement = self.db.prepare_cached(SEARCH_SQL)?;
        let mut rows = statement.query(params![expression, search::MAX_HITS as i64 + 1])?;
        while let Some(row) = rows.next()? {
            if results.hits.len() == search::MAX_HITS {
                results.truncated = true;
                break;
            }
            let title: String = row.get(2)?;
            if title.len() > 1024 {
                return Err(Error::Corrupt);
            }
            results.hits.push(search::Hit {
                version: Version {
                    id: read_id(row.get(0)?)?,
                    revision: row.get(1)?,
                },
                title,
            });
        }
        Ok(results)
    }
    /// Full check of the derived index: FTS5 integrity, one row per
    /// nondeleted song, and every row's text equal to its head revision.
    pub fn check_search(&self) -> Result<bool> {
        match self.check_search_rows() {
            Err(Error::Corrupt) => Ok(false),
            result => result,
        }
    }
    fn check_search_rows(&self) -> Result<bool> {
        if !search_consistent(&self.db)? {
            return Ok(false);
        }
        self.db.execute(
            "INSERT INTO song_search(song_search) VALUES('integrity-check')",
            [],
        )?;
        let mut statement = self.db.prepare("SELECT r.payload,f.title,f.lyrics,f.metadata,f.labels,s.id,s.head,m.author,m.copyright,m.administrator,m.song_number FROM search_rows m JOIN songs s ON s.id=m.song JOIN song_revisions r ON r.id=s.id AND r.revision=s.head JOIN song_search f ON f.rowid=m.row")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let indexed: [String; 4] = [row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?];
            let keys: [String; 4] = [row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?];
            let mut song = Song::decode(&row.get::<_, Vec<u8>>(0)?)?;
            apply_metadata(
                &mut song,
                read_metadata(&self.db, &row.get::<_, Vec<u8>>(5)?, row.get(6)?)?,
            );
            if search_fields(&song) != indexed || sort_keys(&song) != keys {
                return Ok(false);
            }
        }
        Ok(true)
    }
    /// Rebuilds the derived index from the current revisions in one
    /// transaction; song tables are only read.
    pub fn rebuild_search(&mut self) -> Result<search::IndexState> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let songs = rebuild_search(&tx)?;
        tx.commit()?;
        Ok(search::IndexState::Rebuilt { songs })
    }
    /// `check_search`, then `rebuild_search` only when the check fails.
    pub fn repair_search(&mut self) -> Result<search::IndexState> {
        if self.check_search()? {
            Ok(search::IndexState::Healthy)
        } else {
            self.rebuild_search()
        }
    }
    pub fn save_schedule(
        &mut self,
        previous: Option<Version>,
        schedule: Schedule,
    ) -> Result<Version> {
        if schedule.title.trim().is_empty()
            || schedule.title.len() > 1024
            || schedule.items.len() > MAX_ITEMS
        {
            return Err(Error::Invalid);
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let v = advance(&tx, "schedules", previous)?;
        tx.execute(
            "INSERT INTO schedule_revisions VALUES(?,?,?)",
            params![&v.id.0[..], v.revision, schedule.title],
        )?;
        for (pos, song) in schedule.items.iter().enumerate() {
            // FK checks roll back even after earlier items have been inserted.
            tx.execute(
                "INSERT INTO items VALUES(?,?,?,?,?)",
                params![
                    &v.id.0[..],
                    v.revision,
                    pos as i64,
                    &song.id.0[..],
                    song.revision
                ],
            )?;
        }
        tx.commit()?;
        Ok(v)
    }
}
fn insert_revision(tx: &rusqlite::Transaction, v: Version, song: &Song) -> Result<()> {
    tx.execute(
        "INSERT INTO song_revisions VALUES(?,?,?)",
        params![&v.id.0[..], v.revision, song.encode()],
    )?;
    for (position, section) in song.sections.iter().enumerate() {
        tx.execute(
            "INSERT INTO section_ids VALUES(?,?,?,?)",
            params![
                &v.id.0[..],
                v.revision,
                position as i64,
                &section.id.0.0[..]
            ],
        )?;
        if !section.format.is_default() {
            tx.execute(
                "INSERT INTO section_formats VALUES(?,?,?,?)",
                params![
                    &v.id.0[..],
                    v.revision,
                    position as i64,
                    section.format.encode()
                ],
            )?;
        }
        if let Some(background) = &section.background {
            tx.execute(
                "INSERT INTO section_backgrounds VALUES(?,?,?,?)",
                params![
                    &v.id.0[..],
                    v.revision,
                    position as i64,
                    background.encode()
                ],
            )?;
        }
    }
    if !song.administrator.is_empty() || !song.song_number.is_empty() {
        tx.execute(
            "INSERT INTO song_metadata VALUES(?,?,?,?)",
            params![
                &v.id.0[..],
                v.revision,
                song.administrator,
                song.song_number
            ],
        )?;
    }
    if let Some(master) = &song.master {
        tx.execute(
            "INSERT INTO song_backgrounds VALUES(?,?,?)",
            params![&v.id.0[..], v.revision, master.encode()],
        )?;
    }
    for (position, variant) in song.variants.iter().enumerate() {
        tx.execute(
            "INSERT INTO variants VALUES(?,?,?,?,?)",
            params![
                &v.id.0[..],
                v.revision,
                position as i64,
                &variant.id.0.0[..],
                variant.name
            ],
        )?;
        for (position, occurrence) in variant.occurrences.iter().enumerate() {
            tx.execute(
                "INSERT INTO occurrences VALUES(?,?,?,?,?,?)",
                params![
                    &v.id.0[..],
                    v.revision,
                    &variant.id.0.0[..],
                    position as i64,
                    &occurrence.id.0.0[..],
                    &occurrence.section.0.0[..]
                ],
            )?;
        }
    }
    Ok(())
}
fn read_id(bytes: Vec<u8>) -> Result<Id> {
    Ok(Id(bytes.try_into().map_err(|_| Error::Corrupt)?))
}
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}
fn apply_metadata(song: &mut Song, metadata: Option<(String, String)>) {
    if let Some((administrator, song_number)) = metadata {
        song.administrator = administrator;
        song.song_number = song_number;
    }
}
fn has_table(db: &Connection, name: &str) -> Result<bool> {
    Ok(db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?)",
        params![name],
        |r| r.get(0),
    )?)
}
/// Administrator and Song Number of one revision, if it has any.
fn read_metadata(db: &Connection, id: &[u8], revision: i64) -> Result<Option<(String, String)>> {
    db.query_row(
        "SELECT administrator,song_number FROM song_metadata WHERE song=? AND revision=?",
        params![id, revision],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()?
    .map(|(administrator, song_number): (String, String)| {
        if administrator.len() > MAX_ADMINISTRATOR || !song_number_ok(&song_number) {
            return Err(Error::Corrupt);
        }
        Ok((administrator, song_number))
    })
    .transpose()
}
fn quick_check(db: &Connection) -> Result<bool> {
    let check: String = db.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    Ok(check == "ok")
}
/// The indexed columns of a song: title, lyrics, metadata and its distinct
/// section labels (schema 7, EW8-OBS-046), in section order.
fn search_fields(song: &Song) -> [String; 4] {
    let lyrics = song
        .sections
        .iter()
        .map(|s| s.lyrics.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let metadata = [
        &song.authors,
        &song.copyright,
        &song.license,
        &song.administrator,
        &song.song_number,
    ]
    .map(String::as_str)
    .join("\n");
    let mut labels: Vec<&str> = Vec::new();
    for section in &song.sections {
        let label = section.label.trim();
        if !label.is_empty() && !labels.contains(&label) {
            labels.push(label);
        }
    }
    [song.title.clone(), lyrics, metadata, labels.join("\n")]
}
/// The Sort by keys of a song (EW8-OBS-064): Author, Copyright,
/// Administrator and Song Number, each on one line with runs of whitespace
/// collapsed, so a line break or stray space does not change its place.
fn sort_keys(song: &Song) -> [String; 4] {
    [
        &song.authors,
        &song.copyright,
        &song.administrator,
        &song.song_number,
    ]
    .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
}
/// Replaces the song's index entry inside the caller's transaction.
fn index_song(db: &Connection, id: Id, song: &Song) -> Result<()> {
    let row: Option<i64> = db
        .query_row(
            "SELECT row FROM search_rows WHERE song=?",
            params![&id.0[..]],
            |r| r.get(0),
        )
        .optional()?;
    let row = match row {
        Some(old) => {
            db.execute("DELETE FROM song_search WHERE rowid=?", params![old])?;
            old
        }
        None => {
            db.execute(
                "INSERT INTO search_rows(song) VALUES(?)",
                params![&id.0[..]],
            )?;
            db.last_insert_rowid()
        }
    };
    let [author, copyright, administrator, song_number] = sort_keys(song);
    db.execute(
        "UPDATE search_rows SET author=?,copyright=?,administrator=?,song_number=? WHERE row=?",
        params![author, copyright, administrator, song_number, row],
    )?;
    let [title, lyrics, metadata, labels] = search_fields(song);
    db.execute(
        "INSERT INTO song_search(rowid,title,lyrics,metadata,labels) VALUES(?,?,?,?,?)",
        params![row, title, lyrics, metadata, labels],
    )?;
    Ok(())
}
fn unindex_song(db: &Connection, id: Id) -> Result<()> {
    db.execute(
        "DELETE FROM song_search WHERE rowid=(SELECT row FROM search_rows WHERE song=?)",
        params![&id.0[..]],
    )?;
    db.execute("DELETE FROM search_rows WHERE song=?", params![&id.0[..]])?;
    Ok(())
}
/// Indexes every nondeleted head into empty search tables; returns the count.
fn fill_search(db: &Connection) -> Result<usize> {
    // Migrating from schema 4 builds the index before `song_metadata` exists.
    let metadata = has_table(db, "song_metadata")?;
    let mut statement = db.prepare("SELECT s.id,r.payload,s.head FROM songs s JOIN song_revisions r ON r.id=s.id AND r.revision=s.head WHERE s.deleted=0 ORDER BY s.id")?;
    let mut rows = statement.query([])?;
    let mut songs = 0;
    while let Some(row) = rows.next()? {
        let id = read_id(row.get(0)?)?;
        let mut song = Song::decode(&row.get::<_, Vec<u8>>(1)?)?;
        if metadata {
            apply_metadata(&mut song, read_metadata(db, &id.0[..], row.get(2)?)?);
        }
        index_song(db, id, &song)?;
        songs += 1;
    }
    Ok(songs)
}
/// Drops and rebuilds the derived index inside the caller's transaction.
fn rebuild_search(db: &Connection) -> Result<usize> {
    db.execute_batch("DROP TABLE IF EXISTS song_search; DROP TABLE IF EXISTS search_rows;")?;
    db.execute_batch(SEARCH)?;
    fill_search(db)
}
/// Cheap structural agreement between the index and the song table: both
/// tables exist and exactly the nondeleted songs have one index row each.
fn search_consistent(db: &Connection) -> Result<bool> {
    let tables: i64 = db.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name IN ('search_rows','song_search')",
        [],
        |r| r.get(0),
    )?;
    if tables != 2 {
        return Ok(false);
    }
    Ok(!db.prepare("SELECT 1 FROM songs s WHERE s.deleted=0 AND NOT EXISTS(SELECT 1 FROM search_rows m WHERE m.song=s.id) UNION ALL SELECT 1 FROM search_rows m LEFT JOIN songs s ON s.id=m.song WHERE s.id IS NULL OR s.deleted!=0 UNION ALL SELECT 1 FROM search_rows m WHERE NOT EXISTS(SELECT 1 FROM song_search f WHERE f.rowid=m.row) UNION ALL SELECT 1 FROM song_search f WHERE NOT EXISTS(SELECT 1 FROM search_rows m WHERE m.row=f.rowid) LIMIT 1")?.exists([])?)
}
/// The one song save path inside the caller's transaction: the next
/// revision (or a new song), its payload and tables, its search index entry
/// and the Date modified stamp. `song` is already validated.
fn write_song(
    tx: &rusqlite::Transaction<'_>,
    previous: Option<Version>,
    song: &Song,
) -> Result<Version> {
    let v = advance(tx, "songs", previous)?;
    insert_revision(tx, v, song)?;
    index_song(tx, v.id, song)?;
    tx.execute(
        "UPDATE songs SET modified=MAX(?1,(SELECT COALESCE(MAX(modified),0)+1 FROM songs)) WHERE id=?2",
        params![now_millis(), &v.id.0[..]],
    )?;
    Ok(v)
}
fn advance(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    previous: Option<Version>,
) -> Result<Version> {
    match previous {
        Some(v) => {
            let revision = v
                .revision
                .checked_add(1)
                .filter(|_| v.revision > 0)
                .ok_or(Error::Invalid)?;
            let guard = if table == "songs" {
                " AND deleted=0"
            } else {
                ""
            };
            if tx.execute(
                &format!("UPDATE {table} SET head=? WHERE id=? AND head=?{guard}"),
                params![revision, &v.id.0[..], v.revision],
            )? != 1
            {
                return Err(Error::Conflict);
            }
            Ok(Version { revision, ..v })
        }
        None => {
            let bytes: Vec<u8> = tx.query_row("SELECT randomblob(16)", [], |r| r.get(0))?;
            let id = Id(bytes.try_into().map_err(|_| Error::Corrupt)?);
            tx.execute(
                &format!("INSERT INTO {table}(id,head) VALUES(?,1)"),
                params![&id.0[..]],
            )?;
            Ok(Version { id, revision: 1 })
        }
    }
}
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    SaveSong(Option<Version>, Song),
    SaveSchedule(Option<Version>, Schedule),
    Song(Version),
    Arrangement(Version),
    Schedule(Version),
    /// The songs at these revisions (a schedule file's content to save).
    Songs(Vec<Version>),
    /// Resolves a schedule file's songs to library revisions; see
    /// `Repository::import_hidden`. The reply keeps the item order.
    ImportSongs(Vec<(Version, Song)>),
    DeleteSong(Version),
    Heads {
        schedules: bool,
        after: Option<Id>,
    },
    Catalog(Option<Id>),
    /// The whole Songs list in one reply (see `Repository::library`).
    Library(SongOrder),
    ScheduleCatalog(Option<Id>),
    BackupNew(PathBuf),
    RestoreNew {
        source: PathBuf,
        destination: PathBuf,
    },
    /// Ranked current songs; the reply returns `generation` unchanged.
    Search {
        query: String,
        generation: u64,
    },
    /// Checks the search index and rebuilds it only if the check fails.
    RepairSearch,
    /// Rebuilds the search index unconditionally.
    RebuildSearch,
    /// Library duplicates of staged import songs (`Repository::find_duplicates`).
    FindDuplicates(Vec<DuplicateKey>),
    /// Saves an import batch in one transaction (`Repository::commit_import`);
    /// cancelling the command rolls back whatever it has not committed.
    CommitImport(Vec<ImportWrite>),
}
#[derive(Debug)]
pub enum Reply {
    Opened,
    Saved(Version),
    Song(Song),
    Arrangement(Arrangement),
    Schedule(Schedule, Vec<Song>),
    Songs(Vec<Song>),
    Imported(Vec<(Version, Song)>),
    Deleted,
    Heads(Vec<Version>),
    Catalog(Vec<(Version, String)>),
    ScheduleCatalog(Vec<(Version, String)>),
    Copied,
    Search(search::Results),
    SearchIndex(search::IndexState),
    Duplicates(Vec<Option<LibraryMatch>>),
    /// The saved version of each import write, in order.
    Committed(Vec<Version>),
}
struct Request {
    command: Command,
    canceled: Arc<AtomicBool>,
}
fn execute(repo: &mut Repository, request: Request) -> Result<Reply> {
    if request.canceled.load(Ordering::Acquire) {
        return Err(Error::Canceled);
    }
    match request.command {
        Command::SaveSong(v, s) => repo.save_song(v, s).map(Reply::Saved),
        Command::SaveSchedule(v, s) => repo.save_schedule(v, s).map(Reply::Saved),
        Command::Song(v) => repo.song(v).map(Reply::Song),
        Command::Arrangement(v) => repo.arrangement(v).map(Reply::Arrangement),
        Command::Schedule(v) => repo.schedule(v).map(|(s, songs)| Reply::Schedule(s, songs)),
        Command::Songs(versions) => repo.songs(&versions).map(Reply::Songs),
        Command::ImportSongs(items) => items
            .into_iter()
            .map(|(source, song)| Ok((repo.import_hidden(source, &song)?, song)))
            .collect::<Result<Vec<_>>>()
            .map(Reply::Imported),
        Command::DeleteSong(v) => repo.delete_song(v).map(|()| Reply::Deleted),
        Command::Heads { schedules, after } => repo.heads(schedules, after).map(Reply::Heads),
        Command::Catalog(after) => repo.catalog(after).map(Reply::Catalog),
        Command::Library(order) => repo.library(order).map(Reply::Catalog),
        Command::ScheduleCatalog(after) => repo.schedule_catalog(after).map(Reply::ScheduleCatalog),
        Command::BackupNew(path) => repo
            .backup_new(&path, &request.canceled)
            .map(|()| Reply::Copied),
        Command::RestoreNew {
            source,
            destination,
        } => Repository::restore_new(&source, &destination, &request.canceled)
            .map(|()| Reply::Copied),
        Command::Search { query, generation } => repo.search(&query, generation).map(Reply::Search),
        Command::RepairSearch => repo.repair_search().map(Reply::SearchIndex),
        Command::RebuildSearch => repo.rebuild_search().map(Reply::SearchIndex),
        Command::FindDuplicates(keys) => repo.find_duplicates(&keys).map(Reply::Duplicates),
        Command::CommitImport(writes) => repo
            .commit_import(writes, &request.canceled)
            .map(Reply::Committed),
    }
}
/// Cancellation only wins before the worker starts a command. Always poll its result.
pub struct Cancel(Arc<AtomicBool>);
impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}
/// One outstanding request INCLUDING its unconsumed completion, plus asynchronous open.
/// Drop detaches rather than joins; accepted in-flight writes may still commit.
pub struct Worker {
    requests: mpsc::SyncSender<Request>,
    replies: mpsc::Receiver<Result<Reply>>,
    outstanding: bool,
    pending: Option<Arc<AtomicBool>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(pending) = &self.pending {
            pending.store(true, Ordering::Release);
        }
    }
}
impl Worker {
    pub fn open(path: PathBuf) -> Result<Self> {
        if path.as_os_str().len() > 4096 {
            return Err(Error::Invalid);
        }
        let (requests, rx) = mpsc::sync_channel::<Request>(1);
        let (tx, replies) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("sela-storage".into())
            .spawn(move || {
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty())
                    && std::fs::create_dir_all(parent).is_err()
                {
                    let _ = tx.send(Err(Error::Io));
                    return;
                }
                let mut repo = match Repository::open(&path) {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                };
                if tx.send(Ok(Reply::Opened)).is_err() {
                    return;
                }
                while let Ok(request) = rx.recv() {
                    let result = execute(&mut repo, request);
                    if tx.send(result).is_err() {
                        break;
                    }
                }
            })
            .map_err(|_| Error::Io)?;
        Ok(Self {
            requests,
            replies,
            outstanding: true,
            pending: None,
        })
    }
    pub fn submit(&mut self, command: Command) -> Result<Cancel> {
        if self.outstanding {
            return Err(Error::Busy);
        }
        match &command {
            Command::SaveSong(_, s) => s.validate()?,
            Command::SaveSchedule(_, s) if s.items.len() > MAX_ITEMS || s.title.len() > 1024 => {
                return Err(Error::Invalid);
            }
            Command::Songs(v) if v.len() > 2 * MAX_ITEMS => return Err(Error::Invalid),
            Command::ImportSongs(items) if items.len() > MAX_ITEMS => return Err(Error::Invalid),
            // Songs are validated on the storage thread, not here on the UI's.
            Command::CommitImport(writes) if writes.len() > MAX_IMPORT_SONGS => {
                return Err(Error::Invalid);
            }
            Command::FindDuplicates(keys) if keys.len() > MAX_IMPORT_SONGS => {
                return Err(Error::Invalid);
            }
            Command::BackupNew(p) if p.as_os_str().len() > 4096 => return Err(Error::Invalid),
            Command::RestoreNew {
                source,
                destination,
            } if source.as_os_str().len() > 4096 || destination.as_os_str().len() > 4096 => {
                return Err(Error::Invalid);
            }
            Command::Search { query, .. } if query.len() > search::MAX_QUERY_BYTES => {
                return Err(Error::Invalid);
            }
            _ => (),
        }
        let canceled = Arc::new(AtomicBool::new(false));
        self.requests
            .try_send(Request {
                command,
                canceled: canceled.clone(),
            })
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => Error::Busy,
                mpsc::TrySendError::Disconnected(_) => Error::Closed,
            })?;
        self.outstanding = true;
        self.pending = Some(canceled.clone());
        Ok(Cancel(canceled))
    }
    pub fn poll(&mut self) -> Option<Result<Reply>> {
        match self.replies.try_recv() {
            Ok(r) => {
                self.outstanding = false;
                self.pending = None;
                Some(r)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) if self.outstanding => {
                self.outstanding = false;
                Some(Err(Error::Closed))
            }
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn legacy(path: &std::path::Path) -> Version {
        let db = Connection::open(path).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        let v = Version {
            id: Id([41; 16]),
            revision: 1,
        };
        db.execute("INSERT INTO songs VALUES(?,2,0)", params![&v.id.0[..]])
            .unwrap();
        for revision in 1..=2 {
            db.execute(
                "INSERT INTO song_revisions VALUES(?,?,?)",
                params![&v.id.0[..], revision, song().encode()],
            )
            .unwrap();
        }
        let schedule = Id([42; 16]);
        db.execute(
            "INSERT INTO schedules VALUES(?,1)",
            params![&schedule.0[..]],
        )
        .unwrap();
        db.execute(
            "INSERT INTO schedule_revisions VALUES(?,1,'Legacy snapshots')",
            params![&schedule.0[..]],
        )
        .unwrap();
        for (position, revision) in [2, 1, 2].into_iter().enumerate() {
            db.execute(
                "INSERT INTO items VALUES(?,1,?,?,?)",
                params![&schedule.0[..], position as i64, &v.id.0[..], revision],
            )
            .unwrap();
        }
        v
    }
    #[test]
    fn migration_retains_bytes_assigns_once_per_revision_and_restores_schema1() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("legacy");
        let v = legacy(&path);
        let wal = Connection::open(&path).unwrap();
        wal.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; UPDATE songs SET head=2;",
        )
        .unwrap();
        let repo = Repository::open(&path).unwrap();
        assert_eq!(
            repo.db
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            CURRENT
        );
        // One verified backup of the original schema, not one per step.
        assert!(!d.path().join("legacy.schema6-backup").exists());
        assert!(!d.path().join("legacy.schema5-backup").exists());
        assert!(!d.path().join("legacy.schema2-backup").exists());
        assert!(!d.path().join("legacy.schema3-backup").exists());
        assert!(!d.path().join("legacy.schema4-backup").exists());
        let first = repo.song(v).unwrap();
        let second = repo.song(Version { revision: 2, ..v }).unwrap();
        assert_ne!(first.sections[0].id, second.sections[0].id);
        assert_ne!(first.sections[0].id, first.sections[1].id);
        assert_eq!(first.encode(), song().encode());
        let schedule = Version {
            id: Id([42; 16]),
            revision: 1,
        };
        assert_eq!(
            repo.schedule(schedule).unwrap().1,
            vec![second.clone(), first.clone(), second]
        );
        let backup = d.path().join("legacy.schema1-backup");
        let before = std::fs::read(&backup).unwrap();
        let restored = d.path().join("restored-legacy");
        Repository::restore_new(&backup, &restored, &AtomicBool::new(false)).unwrap();
        for p in [&backup, &restored] {
            let db = Connection::open(p).unwrap();
            assert_eq!(
                db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                db.query_row(
                    "SELECT payload FROM song_revisions WHERE revision=1",
                    [],
                    |r| r.get::<_, Vec<u8>>(0)
                )
                .unwrap(),
                song().encode()
            );
            assert_eq!(
                db.prepare("SELECT song_revision FROM items ORDER BY position")
                    .unwrap()
                    .query_map([], |r| r.get::<_, i64>(0))
                    .unwrap()
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .unwrap(),
                vec![2, 1, 2]
            );
        }
        drop(repo);
        assert_eq!(Repository::open(&path).unwrap().song(v).unwrap(), first);
        assert_eq!(std::fs::read(&backup).unwrap(), before);
    }
    #[test]
    fn migration_process_abort_rolls_back_rows_and_retains_verified_backup() {
        const CHILD: &str = "SELA_ABORT_MIGRATION_ROW";
        if let Some(path) = std::env::var_os(CHILD) {
            let _ = Repository::open(std::path::Path::new(&path));
            panic!("migration abort hook did not run");
        }
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("legacy");
        legacy(&path);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "storage::tests::migration_process_abort_rolls_back_rows_and_retains_verified_backup"])
            .env(CHILD, &path).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().unwrap();
        assert!(!status.success());
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(6));
        }
        let db = Connection::open(&path).unwrap();
        assert_eq!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(
            !db.prepare("SELECT 1 FROM sqlite_schema WHERE name='section_ids'")
                .unwrap()
                .exists([])
                .unwrap()
        );
        assert_eq!(
            db.query_row(
                "SELECT payload FROM song_revisions WHERE revision=1",
                [],
                |r| r.get::<_, Vec<u8>>(0)
            )
            .unwrap(),
            song().encode()
        );
        let backup = d.path().join("legacy.schema1-backup");
        Repository::open_internal(&backup, false)
            .unwrap()
            .verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        assert!(matches!(Repository::open(&path), Err(Error::Exists)));
    }
    #[test]
    fn maximum_legacy_payload_migrates_losslessly_without_inventing_arrangements() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("legacy");
        let v = legacy(&path);
        let mut large = song();
        large.title = "x".into();
        large.authors.clear();
        large.copyright.clear();
        large.license.clear();
        for section in &mut large.sections {
            section.label.clear();
            section.lyrics.clear();
        }
        large.sections[0].lyrics = "x".repeat(MAX_SONG_BYTES - large.encode().len());
        assert_eq!(large.encode().len(), MAX_SONG_BYTES);
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE song_revisions SET payload=?",
                params![large.encode()],
            )
            .unwrap();
        let mut worker = Worker::open(path.clone()).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Opened)));
        worker.submit(Command::Song(v)).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Song(s)) if s.encode() == large.encode()));
        let repo = Repository::open(&path).unwrap();
        assert_eq!(repo.song(v).unwrap().encode(), large.encode());
        // The arrangement domain accounts ID overhead, unlike the legacy codec.
        assert!(matches!(repo.arrangement(v), Err(Error::Invalid)));
    }
    #[test]
    fn migration_backup_gate_conflict_corruption_lock_and_ddl_rollback() {
        for failure in ["backup", "payload", "writer", "ddl"] {
            let d = tempfile::tempdir().unwrap();
            let path = d.path().join("legacy");
            legacy(&path);
            let db = Connection::open(&path).unwrap();
            let backup = d.path().join("legacy.schema1-backup");
            let expected = match failure {
                "backup" => {
                    std::fs::write(&backup, b"retain").unwrap();
                    Error::Exists
                }
                "payload" => {
                    db.execute_batch("UPDATE song_revisions SET payload=x'ff'")
                        .unwrap();
                    Error::Corrupt
                }
                "writer" => {
                    db.execute_batch("BEGIN IMMEDIATE; UPDATE songs SET deleted=1")
                        .unwrap();
                    Error::Locked
                }
                _ => {
                    db.execute_batch("CREATE TABLE section_ids(block_upgrade)")
                        .unwrap();
                    Error::Corrupt
                }
            };
            let before = std::fs::read(&path).unwrap();
            assert!(
                matches!(Repository::open(&path), Err(e) if e == expected),
                "{failure}"
            );
            assert_eq!(std::fs::read(&path).unwrap(), before, "{failure}");
            assert_eq!(
                db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                1
            );
            if failure == "backup" {
                assert_eq!(std::fs::read(backup).unwrap(), b"retain");
            } else if failure == "ddl" {
                let old = Repository::open_internal(&backup, false).unwrap();
                old.verify_history(&AtomicBool::new(false), std::time::Instant::now())
                    .unwrap();
                assert!(
                    !db.prepare("SELECT 1 FROM sqlite_schema WHERE name='variants'")
                        .unwrap()
                        .exists([])
                        .unwrap()
                );
            } else {
                assert!(!backup.exists());
            }
        }
    }
    fn arranged_song() -> Song {
        let mut s = song();
        s.sections[1].label = s.sections[0].label.clone();
        s.sections[1].lyrics = "Chorus original\r\n".into();
        s.sections.push(Section {
            id: SectionId(Id([3; 16])),
            label: "Verse".into(),
            lyrics: "Verse two distinct".into(),
            format: Default::default(),
            background: None,
        });
        s.variants = vec![Variant {
            id: VariantId(Id([8; 16])),
            name: "V1/C/V2/C/C".into(),
            occurrences: [0, 1, 2, 1, 1]
                .into_iter()
                .enumerate()
                .map(|(i, n)| Occurrence {
                    id: OccurrenceId(Id([i as u8; 16])),
                    section: s.sections[n].id,
                })
                .collect(),
        }];
        s
    }
    #[test]
    fn arrangements_exact_revision_reorder_atomic_missing_and_worker_roundtrip() {
        let (d, path, mut repo) = fixture();
        let original = arranged_song();
        let first = repo.save_song(None, original.clone()).unwrap();
        let schedule = repo
            .save_schedule(
                None,
                Schedule {
                    title: "Historical".into(),
                    items: vec![first, first],
                },
            )
            .unwrap();
        let mut edited = original.clone();
        edited.sections.swap(0, 2);
        edited.sections[1].lyrics = "New chorus".into();
        let second = repo.save_song(Some(first), edited.clone()).unwrap();
        let resolved = repo.arrangement(first).unwrap();
        assert_eq!(resolved.source().version(), first);
        assert_eq!(
            resolved
                .resolve(original.variants[0].id)
                .unwrap()
                .iter()
                .map(|o| o.section.lyrics.as_str())
                .collect::<Vec<_>>(),
            vec![
                "one\r\n\n二\n",
                "Chorus original\r\n",
                "Verse two distinct",
                "Chorus original\r\n",
                "Chorus original\r\n"
            ]
        );
        let mut missing = edited.clone();
        missing.sections.remove(1);
        let before = std::fs::read(&path).unwrap();
        assert_eq!(repo.save_song(Some(second), missing), Err(Error::Invalid));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(
            repo.save_song(Some(first), edited.clone()),
            Err(Error::Conflict)
        );
        repo.db.execute_batch("CREATE TRIGGER fail_occurrence BEFORE INSERT ON occurrences WHEN NEW.position=3 BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(repo.save_song(Some(second), edited.clone()).is_err());
        assert_eq!(repo.heads(false, None).unwrap(), vec![second]);
        repo.db
            .execute_batch("DROP TRIGGER fail_occurrence")
            .unwrap();
        repo.delete_song(second).unwrap();
        assert_eq!(
            repo.schedule(schedule).unwrap().1,
            vec![original.clone(), original.clone()]
        );
        let backup = d.path().join("schema2-backup");
        repo.backup_new(&backup, &AtomicBool::new(false)).unwrap();
        let restored = d.path().join("schema2-restored");
        Repository::restore_new(&backup, &restored, &AtomicBool::new(false)).unwrap();
        assert_eq!(
            Repository::open(&restored).unwrap().song(second).unwrap(),
            edited
        );
        let mut worker = Worker::open(path).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Opened)));
        worker.submit(Command::Song(first)).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Song(s)) if s == original));
        worker.submit(Command::Arrangement(first)).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Arrangement(a)) if a == resolved));
        worker
            .submit(Command::SaveSong(None, original.clone()))
            .unwrap();
        let duplicate = match wait(&mut worker).unwrap() {
            Reply::Saved(v) => v,
            _ => panic!(),
        };
        worker.submit(Command::Song(duplicate)).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Song(s)) if s == original));
    }
    #[test]
    fn persisted_arrangement_corruption_is_not_truncated_and_never_backed_up() {
        let (d, path, mut repo) = fixture();
        let original = arranged_song();
        let v = repo.save_song(None, original.clone()).unwrap();
        let mut duplicate = original.clone();
        duplicate.sections[2].id = duplicate.sections[0].id;
        assert_eq!(repo.save_song(Some(v), duplicate), Err(Error::Invalid));
        let other = Connection::open(&path).unwrap();
        other.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        // Break a later occurrence, not the first, on a deliberately corrupt fixture.
        other
            .execute(
                "UPDATE occurrences SET section=zeroblob(16) WHERE position=3",
                [],
            )
            .unwrap();
        assert_eq!(repo.song(v), Err(Error::Corrupt));
        assert_eq!(repo.arrangement(v), Err(Error::Corrupt));
        let output = d.path().join("bad-backup");
        assert_eq!(
            repo.backup_new(&output, &AtomicBool::new(false)),
            Err(Error::Corrupt)
        );
        assert!(!output.exists());
        other
            .execute(
                "UPDATE occurrences SET section=? WHERE position=3",
                params![&original.sections[1].id.0.0[..]],
            )
            .unwrap();
        other
            .execute("UPDATE section_ids SET position=7 WHERE position=2", [])
            .unwrap();
        assert_eq!(repo.song(v), Err(Error::Corrupt));
        assert_eq!(
            repo.backup_new(&output, &AtomicBool::new(false)),
            Err(Error::Corrupt)
        );
        assert!(!output.exists());
    }
    #[test]
    fn backup_restore_history_wal_and_worker() {
        let (d, p, mut repo) = fixture();
        repo.db
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
            .unwrap();
        let original = song();
        let a = repo.save_song(None, original.clone()).unwrap();
        let mut distinct = song();
        distinct.sections[0].lyrics = "Asymmetric second occurrence\n三".into();
        let b = repo.save_song(None, distinct.clone()).unwrap();
        let snapshot = Schedule {
            title: "Historical order".into(),
            items: vec![b, a, b],
        };
        let s = repo.save_schedule(None, snapshot.clone()).unwrap();
        let mut edit = song();
        edit.title = "Edited".into();
        let next = repo.save_song(Some(a), edit.clone()).unwrap();
        repo.delete_song(next).unwrap();
        let backup = d.path().join("backup");
        repo.backup_new(&backup, &AtomicBool::new(false)).unwrap();
        let restored = d.path().join("restored");
        let mut worker = Worker::open(p).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Opened)));
        let worker_backup = d.path().join("worker-backup");
        worker
            .submit(Command::BackupNew(worker_backup.clone()))
            .unwrap();
        assert!(matches!(
            worker.submit(Command::Catalog(None)),
            Err(Error::Busy)
        ));
        assert!(matches!(wait(&mut worker), Ok(Reply::Copied)));
        worker
            .submit(Command::RestoreNew {
                source: backup.clone(),
                destination: restored.clone(),
            })
            .unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Copied)));
        for path in [backup, restored, worker_backup] {
            let reopened = Repository::open(&path).unwrap();
            assert_eq!(reopened.song(a).unwrap(), original);
            assert_eq!(reopened.song(next).unwrap(), edit);
            assert_eq!(reopened.heads(false, None).unwrap(), vec![b]);
            assert_eq!(
                reopened.schedule(s).unwrap(),
                (
                    snapshot.clone(),
                    vec![distinct.clone(), original.clone(), distinct.clone()]
                )
            );
        }
    }
    #[test]
    fn backup_failures_never_publish_or_replace() {
        let (d, p, mut repo) = fixture();
        let cancel = AtomicBool::new(false);
        let before = std::fs::read(&p).unwrap();
        assert_eq!(repo.backup_new(&p, &cancel), Err(Error::Exists));
        assert_eq!(std::fs::read(&p).unwrap(), before);
        let output = d.path().join("output");
        std::fs::write(&output, b"retain").unwrap();
        assert_eq!(repo.backup_new(&output, &cancel), Err(Error::Exists));
        assert_eq!(std::fs::read(&output).unwrap(), b"retain");
        std::fs::remove_file(&output).unwrap();
        let alias = d.path().join("source-alias");
        std::fs::hard_link(&p, &alias).unwrap();
        assert_eq!(repo.backup_new(&alias, &cancel), Err(Error::Exists));
        let sidecar = d.path().join("output-wal");
        std::fs::write(&sidecar, b"retain sidecar").unwrap();
        assert_eq!(repo.backup_new(&output, &cancel), Err(Error::Exists));
        assert_eq!(std::fs::read(&sidecar).unwrap(), b"retain sidecar");
        std::fs::remove_file(sidecar).unwrap();
        assert_eq!(
            repo.backup_new(&output, &AtomicBool::new(true)),
            Err(Error::Canceled)
        );
        let other = Connection::open(&p).unwrap();
        other.execute_batch("BEGIN EXCLUSIVE").unwrap();
        assert_eq!(repo.backup_new(&output, &cancel), Err(Error::Locked));
        other.execute_batch("ROLLBACK").unwrap();
        let committed = repo.save_song(None, song()).unwrap();
        // An uncommitted competing writer is excluded, not copied as mixed state.
        other
            .execute_batch("BEGIN IMMEDIATE; UPDATE songs SET head=999")
            .unwrap();
        repo.backup_new(&output, &cancel).unwrap();
        assert_eq!(
            Repository::open(&output)
                .unwrap()
                .heads(false, None)
                .unwrap(),
            vec![committed]
        );
        other.execute_batch("ROLLBACK").unwrap();
        std::fs::remove_file(&output).unwrap();
        let v = repo.save_song(None, song()).unwrap();
        repo.db
            .execute(
                "UPDATE song_revisions SET payload=x'ff' WHERE id=?",
                params![&v.id.0[..]],
            )
            .unwrap();
        assert_eq!(repo.backup_new(&output, &cancel), Err(Error::Corrupt));
        assert!(!output.exists());
        {
            let bytes = b"not sqlite".as_slice();
            let input = d.path().join("input");
            std::fs::write(&input, bytes).unwrap();
            assert_eq!(
                Repository::restore_new(&input, &output, &cancel),
                Err(Error::Corrupt)
            );
            assert_eq!(std::fs::read(input).unwrap(), bytes);
        }
        repo.db.execute_batch("PRAGMA user_version=8").unwrap();
        drop(repo);
        let before = std::fs::read(&p).unwrap();
        assert_eq!(
            Repository::restore_new(&p, &output, &cancel),
            Err(Error::Unsupported)
        );
        assert_eq!(std::fs::read(&p).unwrap(), before);
        assert!(!output.exists());
        assert!(!std::fs::read_dir(d.path()).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".sela-backup-")
        }));
    }
    // Spawn this same test executable: abort skips destructors and SQLite close.
    #[test]
    fn process_termination_during_backup_never_publishes() {
        const CHILD: &str = "SELA_ABORT_BACKUP_STEP";
        if let Some(directory) = std::env::var_os(CHILD) {
            let directory = std::path::Path::new(&directory);
            let repo = Repository::open(&directory.join("source")).unwrap();
            repo.backup_new(&directory.join("output"), &AtomicBool::new(false))
                .unwrap();
            panic!("backup interruption hook did not run");
        }
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("source");
        let mut repo = Repository::open(&path).unwrap();
        let mut large = song();
        large.sections[0].lyrics = "x".repeat(200_000);
        let a = repo.save_song(None, large.clone()).unwrap();
        repo.save_song(None, large.clone()).unwrap();
        drop(repo);
        let before = std::fs::read(&path).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::tests::process_termination_during_backup_never_publishes",
            ])
            .env(CHILD, d.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(6));
        }
        assert!(!status.success());
        assert!(!d.path().join("output").exists());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(Repository::open(&path).unwrap().song(a).unwrap(), large);
        assert!(std::fs::read_dir(d.path()).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".sela-backup-")
        }));
    }
    #[test]
    fn process_termination_recovers_committed_and_uncommitted_writes() {
        const CHILD: &str = "SELA_STORAGE_CRASH_FIXTURE";
        if let Some(path) = std::env::var_os(CHILD) {
            let mut repo = Repository::open(std::path::Path::new(&path)).unwrap();
            let a = repo.save_song(None, song()).unwrap();
            repo.save_schedule(
                None,
                Schedule {
                    title: "Committed service".into(),
                    items: vec![a, a],
                },
            )
            .unwrap();
            repo.db.execute_batch("PRAGMA cache_size=1; BEGIN IMMEDIATE; UPDATE songs SET head=999; DELETE FROM items; INSERT INTO song_revisions SELECT id,999,zeroblob(200000) FROM songs;").unwrap();
            std::process::abort();
        }
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("crash");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::tests::process_termination_recovers_committed_and_uncommitted_writes",
                "--nocapture",
            ])
            .env(CHILD, &p)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success());
        let repo = Repository::open(&p).unwrap();
        let heads = repo.heads(false, None).unwrap();
        assert_eq!(heads.len(), 1);
        assert_eq!(heads[0].revision, 1);
        assert_eq!(repo.song(heads[0]).unwrap(), song());
        let schedules = repo.heads(true, None).unwrap();
        assert_eq!(schedules.len(), 1);
        assert_eq!(
            repo.schedule(schedules[0]).unwrap(),
            (
                Schedule {
                    title: "Committed service".into(),
                    items: vec![heads[0], heads[0]]
                },
                vec![song(), song()]
            )
        );
        assert_eq!(
            repo.db
                .query_row("SELECT count(*) FROM song_revisions", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn cancellation_before_start_never_writes() {
        let (_d, _p, mut repo) = fixture();
        let canceled = Arc::new(AtomicBool::new(false));
        let request = Request {
            command: Command::SaveSong(None, song()),
            canceled: canceled.clone(),
        };
        Cancel(canceled).cancel();
        assert!(matches!(execute(&mut repo, request), Err(Error::Canceled)));
        assert!(repo.heads(false, None).unwrap().is_empty());
    }
    fn song() -> Song {
        Song {
            title: "Café e\u{301} سلام".into(),
            authors: "Original 作者".into(),
            copyright: "© test".into(),
            license: "test-only".into(),
            administrator: String::new(),
            song_number: String::new(),
            variants: Vec::new(),
            sections: vec![
                Section {
                    id: SectionId(Id([1; 16])),
                    label: "Verse".into(),
                    lyrics: "one\r\n\n二\n".into(),
                    format: Default::default(),
                    background: None,
                },
                Section {
                    id: SectionId(Id([2; 16])),
                    label: "".into(),
                    lyrics: "".into(),
                    format: Default::default(),
                    background: None,
                },
            ],
            master: None,
        }
    }
    fn fixture() -> (tempfile::TempDir, PathBuf, Repository) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("test.sqlite");
        let r = Repository::open(&p).unwrap();
        (d, p, r)
    }
    #[test]
    fn schedule_file_songs_reuse_identical_revisions_or_import_hidden() {
        let (_d, p, mut r) = fixture();
        let original = song();
        let v = r.save_song(None, original.clone()).unwrap();
        assert_eq!(r.import_hidden(v, &original).unwrap(), v, "identical");
        assert_eq!(r.songs(&[v, v]).unwrap(), [original.clone(), original]);

        // A different library: the embedded content becomes a hidden revision.
        let (_d2, p2, mut other) = fixture();
        let mut foreign = song();
        foreign.title = "Only in the file".into();
        let hidden = other.import_hidden(v, &foreign).unwrap();
        assert_ne!(hidden, v);
        assert_eq!(other.song(hidden).unwrap(), foreign);
        assert!(other.catalog(None).unwrap().is_empty(), "not listed");
        assert!(
            other.search("Only", 1).unwrap().hits.is_empty(),
            "not searched"
        );
        assert_eq!(other.import_hidden(v, &foreign).unwrap(), hidden, "reused");
        drop(other);
        let mut reopened = Repository::open(&p2).unwrap();
        assert_eq!(reopened.import_hidden(v, &foreign).unwrap(), hidden);
        assert!(reopened.catalog(None).unwrap().is_empty());

        // The source song deleted from its library: its content still resolves.
        let mut repo = Repository::open(&p).unwrap();
        let content = repo.song(v).unwrap();
        repo.delete_song(v).unwrap();
        assert_eq!(repo.import_hidden(v, &content).unwrap(), v);
        assert!(repo.catalog(None).unwrap().is_empty());
    }
    #[test]
    fn reopen_unicode_duplicate_titles_and_snapshot_history() {
        let (_d, p, mut r) = fixture();
        let original = song();
        let a = r.save_song(None, original.clone()).unwrap();
        let mut distinct = original.clone();
        distinct.sections[0].lyrics = "Different lyrics with the same title\n".into();
        let b = r.save_song(None, distinct.clone()).unwrap();
        assert_ne!(a.id, b.id);
        let s = Schedule {
            title: "Sunday".into(),
            items: vec![b, a, a],
        };
        let v = r.save_schedule(None, s.clone()).unwrap();
        let mut edited = original.clone();
        edited.title = "Changed".into();
        let a2 = r.save_song(Some(a), edited).unwrap();
        r.delete_song(a2).unwrap();
        assert_eq!(r.save_song(Some(a), song()), Err(Error::Conflict));
        drop(r);
        let r = Repository::open(&p).unwrap();
        assert_eq!(
            r.schedule(v).unwrap(),
            (s, vec![distinct, original.clone(), original.clone()])
        );
        assert_eq!(r.song(a).unwrap(), original);
        assert_eq!(r.heads(false, None).unwrap(), vec![b]);
    }
    #[test]
    fn schedule_conflicts_and_second_item_failure_rollback() {
        let (_d, p, mut r) = fixture();
        let a = r.save_song(None, song()).unwrap();
        let s = Schedule {
            title: "test".into(),
            items: vec![a],
        };
        let v = r.save_schedule(None, s.clone()).unwrap();
        let before = std::fs::read(&p).unwrap();
        let mut bad = s.clone();
        bad.items.push(Version {
            id: Id([0; 16]),
            revision: 1,
        });
        assert!(r.save_schedule(Some(v), bad).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), before);
        let next = r.save_schedule(Some(v), s.clone()).unwrap();
        let before = std::fs::read(&p).unwrap();
        assert_eq!(r.save_schedule(Some(v), s.clone()), Err(Error::Conflict));
        assert_eq!(std::fs::read(&p).unwrap(), before);
        assert_eq!(r.schedule(next).unwrap().0, s);
        r.db.execute_batch("CREATE TRIGGER fail_second BEFORE INSERT ON items WHEN NEW.position=1 BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        let mut bad = s.clone();
        bad.items.push(a);
        assert!(r.save_schedule(Some(next), bad).is_err());
        assert_eq!(r.heads(true, None).unwrap(), vec![next]);
    }
    #[test]
    fn rejects_newer_foreign_corrupt_without_replacement() {
        for sql in ["PRAGMA user_version=8", "PRAGMA application_id=42"] {
            let (_d, p, r) = fixture();
            r.db.execute_batch(sql).unwrap();
            drop(r);
            let before = std::fs::read(&p).unwrap();
            assert!(matches!(Repository::open(&p), Err(Error::Unsupported)));
            assert_eq!(std::fs::read(&p).unwrap(), before);
        }
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("bad");
        std::fs::write(&p, b"not a database\0private lyrics").unwrap();
        let before = std::fs::read(&p).unwrap();
        assert!(matches!(Repository::open(&p), Err(Error::Corrupt)));
        assert_eq!(std::fs::read(&p).unwrap(), before);
        let p = d.path().join("foreign");
        let db = Connection::open(&p).unwrap();
        db.execute_batch("CREATE TABLE unrelated(x)").unwrap();
        drop(db);
        let before = std::fs::read(&p).unwrap();
        assert!(matches!(Repository::open(&p), Err(Error::Unsupported)));
        assert_eq!(std::fs::read(&p).unwrap(), before);
    }
    #[test]
    fn migration_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("migration");
        let mut db = Connection::open(&path).unwrap();
        {
            let tx = db.transaction().unwrap();
            tx.execute_batch(SCHEMA).unwrap();
            assert!(tx.execute_batch("CREATE TABLE songs(x)").is_err());
        }
        assert_eq!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn real_lock_timeout_and_disk_full_rollback() {
        let (_d, p, mut r) = fixture();
        let other = Connection::open(&p).unwrap();
        other.execute_batch("BEGIN IMMEDIATE").unwrap();
        let start = std::time::Instant::now();
        assert_eq!(r.save_song(None, song()), Err(Error::Locked));
        assert!(start.elapsed() >= Duration::from_millis(80));
        assert!(start.elapsed() < Duration::from_secs(3));
        other.execute_batch("ROLLBACK").unwrap();
        let pages: i64 =
            r.db.query_row("PRAGMA page_count", [], |r| r.get(0))
                .unwrap();
        r.db.execute_batch(&format!("PRAGMA max_page_count={pages}"))
            .unwrap();
        let mut big = song();
        big.sections[0].lyrics = "x".repeat(200_000);
        assert_eq!(r.save_song(None, big), Err(Error::Full));
        assert!(r.heads(false, None).unwrap().is_empty());
        assert!(
            Repository::open(&p)
                .unwrap()
                .heads(false, None)
                .unwrap()
                .is_empty()
        );
    }
    fn wait(w: &mut Worker) -> Result<Reply> {
        let end = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(r) = w.poll() {
                return r;
            }
            assert!(std::time::Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn catalog_pages_current_revisions_and_excludes_tombstones() {
        let (_d, _p, mut repo) = fixture();
        let mut expected = Vec::new();
        for i in 0..130 {
            let mut song = song();
            song.title = format!("Original {i}");
            let version = repo.save_song(None, song.clone()).unwrap();
            expected.push((version, song.title));
        }
        repo.delete_song(expected.remove(0).0).unwrap();
        let mut edited = song();
        edited.title = "Revised title".into();
        expected[0] = (
            repo.save_song(Some(expected[0].0), edited.clone()).unwrap(),
            edited.title,
        );
        expected.sort_by_key(|(v, _)| v.id.0);
        let first = repo.catalog(None).unwrap();
        assert_eq!(first, expected[..128]);
        let last = repo.catalog(Some(first[127].0.id)).unwrap();
        assert_eq!(last, expected[128..]);
        assert!(repo.catalog(Some(last[0].0.id)).unwrap().is_empty());
    }
    #[test]
    fn library_lists_every_song_by_title_or_by_last_save() {
        let (_d, _p, mut repo) = fixture();
        let mut saved = Vec::new();
        for i in 0..300 {
            let mut song = song();
            song.title = format!("Song {:03}", (i * 7) % 300);
            saved.push((repo.save_song(None, song.clone()).unwrap(), song));
        }
        fn title(repo: &Repository, order: SongOrder) -> Vec<String> {
            repo.library(order)
                .unwrap()
                .into_iter()
                .map(|(_, t)| t)
                .collect()
        }
        let ascending = title(&repo, SongOrder::default());
        assert_eq!(ascending.len(), 300, "not limited to one catalog page");
        assert!(ascending.windows(2).all(|p| p[0] <= p[1]));
        let mut descending = ascending.clone();
        descending.reverse();
        assert_eq!(
            title(
                &repo,
                SongOrder {
                    sort: SongSort::Title,
                    descending: true
                }
            ),
            descending
        );

        // Case-insensitive, with the exact title breaking ties.
        let mut mixed = song();
        mixed.title = "song 000b".into();
        repo.save_song(None, mixed).unwrap();
        let ascending = title(&repo, SongOrder::default());
        let at = |t: &str| ascending.iter().position(|x| x == t).unwrap();
        assert!(at("Song 000") < at("song 000b") && at("song 000b") < at("Song 001"));

        // Modified: creation order at first, a save moves a song to the end,
        // a deleted song is gone.
        let (first, first_song) = saved[0].clone();
        let modified = SongOrder {
            sort: SongSort::Modified,
            descending: false,
        };
        let listed = repo.library(modified).unwrap();
        assert_eq!(listed[0].0, first);
        assert_eq!(listed.last().unwrap().1, "song 000b");
        let mut edited = first_song;
        edited.title = "Edited".into();
        let first = repo.save_song(Some(first), edited).unwrap();
        let listed = repo.library(modified).unwrap();
        assert_eq!(listed.last().unwrap(), &(first, "Edited".to_string()));
        assert_ne!(listed[0].0.id, first.id);
        let newest = repo
            .library(SongOrder {
                sort: SongSort::Modified,
                descending: true,
            })
            .unwrap();
        assert_eq!(newest[0].0, first);
        repo.delete_song(first).unwrap();
        assert!(
            repo.library(modified)
                .unwrap()
                .iter()
                .all(|(v, _)| v.id != first.id)
        );
        assert_eq!(repo.library(modified).unwrap().len(), 300);
    }
    #[test]
    fn schedule_catalog_lists_current_titles() {
        let (_d, _p, mut repo) = fixture();
        let a = repo.save_song(None, song()).unwrap();
        let first = repo
            .save_schedule(
                None,
                Schedule {
                    title: "Morning".into(),
                    items: vec![a, a],
                },
            )
            .unwrap();
        let first = repo
            .save_schedule(
                Some(first),
                Schedule {
                    title: "Morning (revised)".into(),
                    items: vec![a],
                },
            )
            .unwrap();
        let second = repo
            .save_schedule(
                None,
                Schedule {
                    title: "Evening".into(),
                    items: Vec::new(),
                },
            )
            .unwrap();
        let mut expected = vec![
            (first, "Morning (revised)".to_string()),
            (second, "Evening".to_string()),
        ];
        expected.sort_by_key(|(v, _)| v.id.0);
        assert_eq!(repo.schedule_catalog(None).unwrap(), expected);
        assert!(
            repo.schedule_catalog(Some(expected[1].0.id))
                .unwrap()
                .is_empty()
        );
        repo.db
            .execute(
                "UPDATE schedule_revisions SET title=' ' WHERE id=?",
                params![&second.id.0[..]],
            )
            .unwrap();
        assert_eq!(repo.schedule_catalog(None), Err(Error::Corrupt));
    }
    #[test]
    fn worker_creates_parent_and_reports_unusable_parent_without_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new/profile/library.sqlite");
        let mut worker = Worker::open(path.clone()).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Opened)));
        assert!(path.is_file());
        worker.submit(Command::Catalog(None)).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Catalog(rows)) if rows.is_empty()));
        let blocker = dir.path().join("existing-file");
        std::fs::write(&blocker, b"preserve these bytes").unwrap();
        let mut worker = Worker::open(blocker.join("library.sqlite")).unwrap();
        assert!(matches!(wait(&mut worker), Err(Error::Io)));
        assert_eq!(std::fs::read(blocker).unwrap(), b"preserve these bytes");
    }
    #[test]
    fn worker_backpressure_results_and_nonblocking_drop() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("w");
        let mut w = Worker::open(p.clone()).unwrap();
        assert!(matches!(
            w.submit(Command::SaveSong(None, song())),
            Err(Error::Busy)
        ));
        assert!(matches!(wait(&mut w), Ok(Reply::Opened)));
        let cancel = w.submit(Command::SaveSong(None, song())).unwrap();
        cancel.cancel();
        assert!(matches!(
            w.submit(Command::SaveSong(None, song())),
            Err(Error::Busy)
        ));
        let result = wait(&mut w);
        assert!(matches!(result, Err(Error::Canceled) | Ok(Reply::Saved(_))));
        w.submit(Command::SaveSong(None, song())).unwrap();
        assert!(matches!(wait(&mut w), Ok(Reply::Saved(_))));
        let other = Connection::open(&p).unwrap();
        other.execute_batch("BEGIN EXCLUSIVE").unwrap();
        w.submit(Command::SaveSong(None, song())).unwrap();
        let start = std::time::Instant::now();
        drop(w);
        assert!(start.elapsed() < Duration::from_millis(50));
        other.execute_batch("ROLLBACK").unwrap();
    }
    #[test]
    fn validation_is_bounded_and_empty_sections_lossless() {
        let mut s = song();
        s.sections[0].lyrics = "x".repeat(MAX_SONG_BYTES);
        assert_eq!(s.validate(), Err(Error::Invalid));
        s = song();
        s.sections = vec![];
        assert_eq!(Song::decode(&s.encode()).unwrap(), s);
        assert_eq!(Song::decode(&[255; 8]), Err(Error::Corrupt));
    }
    fn schema_of(path: &std::path::Path) -> i64 {
        Connection::open(path)
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    }
    /// The legacy library after the schema 2 migration, with known section IDs.
    fn schema2(path: &std::path::Path) -> Version {
        let v = legacy(path);
        let db = Connection::open(path).unwrap();
        db.execute_batch(SCHEMA2).unwrap();
        for revision in 1..=2u8 {
            for position in 0..2u8 {
                db.execute(
                    "INSERT INTO section_ids VALUES(?,?,?,?)",
                    params![
                        &v.id.0[..],
                        revision,
                        position,
                        &[revision * 10 + position; 16][..]
                    ],
                )
                .unwrap();
            }
        }
        v
    }
    #[test]
    fn schema2_migrates_with_a_verified_backup_and_keeps_ids() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        let v = schema2(&path);
        let repo = Repository::open(&path).unwrap();
        assert_eq!(schema_of(&path), CURRENT);
        let first = repo.song(v).unwrap();
        assert_eq!(first.sections[1].id, SectionId(Id([11; 16])));
        assert!(first.sections.iter().all(|s| s.format.is_default()));
        assert_eq!(first.encode(), song().encode());
        let backup = d.path().join("library.schema2-backup");
        let before = std::fs::read(&backup).unwrap();
        assert_eq!(schema_of(&backup), 2);
        let old = Repository::open_internal(&backup, false).unwrap();
        old.verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        assert_eq!(old.song(v).unwrap(), first);
        let restored = d.path().join("restored");
        Repository::restore_new(&backup, &restored, &AtomicBool::new(false)).unwrap();
        assert_eq!(schema_of(&restored), 2);
        drop((old, repo));
        assert_eq!(Repository::open(&path).unwrap().song(v).unwrap(), first);
        assert_eq!(std::fs::read(&backup).unwrap(), before);
    }
    #[test]
    fn schema2_migration_gates_leave_the_library_unchanged() {
        for failure in ["backup", "writer", "ddl"] {
            let d = tempfile::tempdir().unwrap();
            let path = d.path().join("library");
            schema2(&path);
            let db = Connection::open(&path).unwrap();
            let backup = d.path().join("library.schema2-backup");
            match failure {
                "backup" => std::fs::write(&backup, b"retain").unwrap(),
                "writer" => db
                    .execute_batch("BEGIN IMMEDIATE; UPDATE songs SET deleted=1")
                    .unwrap(),
                _ => db
                    .execute_batch("CREATE TABLE section_formats(block_upgrade)")
                    .unwrap(),
            }
            let before = std::fs::read(&path).unwrap();
            let result = Repository::open(&path).map(|_| ());
            match failure {
                "backup" => assert_eq!(result, Err(Error::Exists)),
                "writer" => assert_eq!(result, Err(Error::Locked)),
                _ => assert_eq!(result, Err(Error::Corrupt)),
            }
            assert_eq!(std::fs::read(&path).unwrap(), before, "{failure}");
            assert_eq!(
                db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                2,
                "{failure}"
            );
            match failure {
                "backup" => assert_eq!(std::fs::read(&backup).unwrap(), b"retain"),
                "writer" => assert!(!backup.exists()),
                _ => Repository::open_internal(&backup, false)
                    .unwrap()
                    .verify_history(&AtomicBool::new(false), std::time::Instant::now())
                    .unwrap(),
            }
        }
    }
    #[test]
    fn schema2_migration_abort_keeps_schema2_and_the_verified_backup() {
        const CHILD: &str = "SELA_ABORT_MIGRATION_SCHEMA3";
        if let Some(path) = std::env::var_os(CHILD) {
            let _ = Repository::open(std::path::Path::new(&path));
            panic!("schema 3 abort hook did not run");
        }
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        schema2(&path);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::tests::schema2_migration_abort_keeps_schema2_and_the_verified_backup",
            ])
            .env(CHILD, &path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success());
        assert_eq!(schema_of(&path), 2);
        assert!(
            !Connection::open(&path)
                .unwrap()
                .prepare("SELECT 1 FROM sqlite_schema WHERE name='section_formats'")
                .unwrap()
                .exists([])
                .unwrap()
        );
        Repository::open_internal(&d.path().join("library.schema2-backup"), false)
            .unwrap()
            .verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        assert!(matches!(Repository::open(&path), Err(Error::Exists)));
    }
    #[test]
    fn slide_formats_round_trip_per_revision_and_default_stores_nothing() {
        let (d, path, mut repo) = fixture();
        let rows = |repo: &Repository| -> i64 {
            repo.db
                .query_row("SELECT count(*) FROM section_formats", [], |r| r.get(0))
                .unwrap()
        };
        let mut formatted = song();
        formatted.sections[0].format = crate::format::tests::full();
        let first = repo.save_song(None, formatted.clone()).unwrap();
        assert_eq!(rows(&repo), 1);
        assert_eq!(repo.song(first).unwrap(), formatted);
        let mut edited = formatted.clone();
        edited.sections[0].format = SlideFormat::default();
        edited.sections[1].format.italic = Some(true);
        let second = repo.save_song(Some(first), edited.clone()).unwrap();
        assert_eq!(rows(&repo), 2);
        assert_eq!(repo.song(first).unwrap(), formatted);
        assert_eq!(repo.song(second).unwrap(), edited);
        let backup = d.path().join("formats-backup");
        repo.backup_new(&backup, &AtomicBool::new(false)).unwrap();
        drop(repo);
        assert_eq!(
            Repository::open(&path).unwrap().song(first).unwrap(),
            formatted
        );
        assert_eq!(
            Repository::open(&backup).unwrap().song(second).unwrap(),
            edited
        );
        let mut worker = Worker::open(path).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Opened)));
        worker.submit(Command::Song(first)).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Song(s)) if s == formatted));
    }
    #[test]
    fn invalid_or_corrupt_formats_are_rejected_not_dropped() {
        let (d, path, mut repo) = fixture();
        let mut bad = song();
        bad.sections[1].format.size = Some(crate::format::Size::Fixed(0));
        let before = std::fs::read(&path).unwrap();
        assert_eq!(repo.save_song(None, bad), Err(Error::Invalid));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let mut good = song();
        good.sections[0].format = crate::format::tests::full();
        let v = repo.save_song(None, good).unwrap();
        for (index, blob) in [&[1u8, 0, 0][..], &[1, 2, 0, 9], &[0xff; 4]]
            .into_iter()
            .enumerate()
        {
            repo.db
                .execute("UPDATE section_formats SET format=?", params![blob])
                .unwrap();
            assert_eq!(repo.song(v), Err(Error::Corrupt), "{blob:?}");
            let output = d.path().join(format!("backup-{index}"));
            assert_eq!(
                repo.backup_new(&output, &AtomicBool::new(false)),
                Err(Error::Corrupt)
            );
            assert!(!output.exists());
        }
        repo.db
            .execute(
                "UPDATE section_formats SET format=?",
                params![crate::format::tests::full().encode()],
            )
            .unwrap();
        drop(repo);
        // Only a writer without foreign keys can attach a format to a missing slide.
        let db = Connection::open(&path).unwrap();
        db.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        db.execute(
            "INSERT INTO section_formats VALUES(?,?,5,?)",
            params![
                &v.id.0[..],
                v.revision,
                crate::format::tests::full().encode()
            ],
        )
        .unwrap();
        drop(db);
        assert!(matches!(Repository::open(&path), Err(Error::Corrupt)));
    }
    /// The schema 2 library after the schema 3 migration, with one format.
    fn schema3(path: &std::path::Path) -> Version {
        let v = schema2(path);
        let db = Connection::open(path).unwrap();
        db.execute_batch(SCHEMA3).unwrap();
        db.execute(
            "INSERT INTO section_formats VALUES(?,1,0,?)",
            params![&v.id.0[..], crate::format::tests::full().encode()],
        )
        .unwrap();
        v
    }
    #[test]
    fn schema3_migrates_with_a_verified_backup_and_keeps_formats() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        let v = schema3(&path);
        let repo = Repository::open(&path).unwrap();
        assert_eq!(schema_of(&path), CURRENT);
        let first = repo.song(v).unwrap();
        assert_eq!(first.sections[0].format, crate::format::tests::full());
        assert!(first.master.is_none());
        assert!(first.sections.iter().all(|s| s.background.is_none()));
        let backup = d.path().join("library.schema3-backup");
        let before = std::fs::read(&backup).unwrap();
        assert_eq!(schema_of(&backup), 3);
        let old = Repository::open_internal(&backup, false).unwrap();
        old.verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        assert_eq!(old.song(v).unwrap(), first);
        drop((old, repo));
        assert_eq!(Repository::open(&path).unwrap().song(v).unwrap(), first);
        assert_eq!(std::fs::read(&backup).unwrap(), before);
    }
    #[test]
    fn schema3_migration_gates_leave_the_library_unchanged() {
        for failure in ["backup", "writer", "ddl"] {
            let d = tempfile::tempdir().unwrap();
            let path = d.path().join("library");
            schema3(&path);
            let db = Connection::open(&path).unwrap();
            let backup = d.path().join("library.schema3-backup");
            match failure {
                "backup" => std::fs::write(&backup, b"retain").unwrap(),
                "writer" => db
                    .execute_batch("BEGIN IMMEDIATE; UPDATE songs SET deleted=1")
                    .unwrap(),
                _ => db
                    .execute_batch("CREATE TABLE song_backgrounds(block_upgrade)")
                    .unwrap(),
            }
            let before = std::fs::read(&path).unwrap();
            let result = Repository::open(&path).map(|_| ());
            match failure {
                "backup" => assert_eq!(result, Err(Error::Exists)),
                "writer" => assert_eq!(result, Err(Error::Locked)),
                _ => assert_eq!(result, Err(Error::Corrupt)),
            }
            assert_eq!(std::fs::read(&path).unwrap(), before, "{failure}");
            assert_eq!(
                db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                3,
                "{failure}"
            );
            match failure {
                "backup" => assert_eq!(std::fs::read(&backup).unwrap(), b"retain"),
                "writer" => assert!(!backup.exists()),
                _ => Repository::open_internal(&backup, false)
                    .unwrap()
                    .verify_history(&AtomicBool::new(false), std::time::Instant::now())
                    .unwrap(),
            }
        }
    }
    #[test]
    fn schema3_migration_abort_keeps_schema3_and_the_verified_backup() {
        const CHILD: &str = "SELA_ABORT_MIGRATION_SCHEMA4";
        if let Some(path) = std::env::var_os(CHILD) {
            let _ = Repository::open(std::path::Path::new(&path));
            panic!("schema 4 abort hook did not run");
        }
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        schema3(&path);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::tests::schema3_migration_abort_keeps_schema3_and_the_verified_backup",
            ])
            .env(CHILD, &path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success());
        assert_eq!(schema_of(&path), 3);
        assert!(
            !Connection::open(&path)
                .unwrap()
                .prepare("SELECT 1 FROM sqlite_schema WHERE name='song_backgrounds'")
                .unwrap()
                .exists([])
                .unwrap()
        );
        Repository::open_internal(&d.path().join("library.schema3-backup"), false)
            .unwrap()
            .verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        assert!(matches!(Repository::open(&path), Err(Error::Exists)));
    }
    #[test]
    fn backgrounds_round_trip_per_revision_and_none_stores_nothing() {
        use crate::background::{Background, tests::image};
        let (d, path, mut repo) = fixture();
        let rows = |repo: &Repository| -> (i64, i64) {
            let count = |table: &str| {
                repo.db
                    .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                    .unwrap()
            };
            (count("song_backgrounds"), count("section_backgrounds"))
        };
        let plain = repo.save_song(None, song()).unwrap();
        assert_eq!(rows(&repo), (0, 0));
        let mut styled = song();
        styled.master = Some(Background::color([0, 0, 255]));
        styled.sections[1].background = Some(image("Sunrise.jpg"));
        let first = repo.save_song(Some(plain), styled.clone()).unwrap();
        assert_eq!(rows(&repo), (1, 1));
        assert_eq!(repo.song(first).unwrap(), styled);
        assert_eq!(repo.song(plain).unwrap(), song());
        let mut edited = styled.clone();
        edited.master = None;
        edited.sections[0].background = Some(Background::color([9, 9, 9]));
        let second = repo.save_song(Some(first), edited.clone()).unwrap();
        assert_eq!(repo.song(first).unwrap(), styled);
        assert_eq!(repo.song(second).unwrap(), edited);
        let schedule = repo
            .save_schedule(
                None,
                Schedule {
                    title: "Pinned backgrounds".into(),
                    items: vec![first, second],
                },
            )
            .unwrap();
        assert_eq!(
            repo.schedule(schedule).unwrap().1,
            vec![styled.clone(), edited.clone()]
        );
        let backup = d.path().join("backgrounds-backup");
        repo.backup_new(&backup, &AtomicBool::new(false)).unwrap();
        drop(repo);
        assert_eq!(
            Repository::open(&path).unwrap().song(first).unwrap(),
            styled
        );
        assert_eq!(
            Repository::open(&backup).unwrap().song(second).unwrap(),
            edited
        );
        let mut worker = Worker::open(path).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Opened)));
        worker.submit(Command::Song(first)).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Song(s)) if s == styled));
    }
    #[test]
    fn invalid_or_corrupt_backgrounds_are_rejected_not_dropped() {
        use crate::background::{Background, tests::image};
        let (d, path, mut repo) = fixture();
        let mut bad = song();
        bad.master = Some(image("../escape.png"));
        let before = std::fs::read(&path).unwrap();
        assert_eq!(repo.save_song(None, bad), Err(Error::Invalid));
        let mut bad_slide = song();
        bad_slide.sections[0].background = Some(image("a.gif"));
        assert_eq!(repo.save_song(None, bad_slide), Err(Error::Invalid));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let mut good = song();
        good.master = Some(Background::color([1, 2, 3]));
        good.sections[0].background = Some(image("a.png"));
        let v = repo.save_song(None, good).unwrap();
        for table in ["song_backgrounds", "section_backgrounds"] {
            for (index, blob) in [&[1u8, 9, 0][..], &[2, 0, 0], &[1, 1, 0, 0]]
                .into_iter()
                .enumerate()
            {
                let original: Vec<u8> = repo
                    .db
                    .query_row(&format!("SELECT background FROM {table}"), [], |r| r.get(0))
                    .unwrap();
                repo.db
                    .execute(&format!("UPDATE {table} SET background=?"), params![blob])
                    .unwrap();
                assert_eq!(repo.song(v), Err(Error::Corrupt), "{table} {blob:?}");
                let output = d.path().join(format!("backup-{table}-{index}"));
                assert_eq!(
                    repo.backup_new(&output, &AtomicBool::new(false)),
                    Err(Error::Corrupt)
                );
                assert!(!output.exists());
                repo.db
                    .execute(
                        &format!("UPDATE {table} SET background=?"),
                        params![original],
                    )
                    .unwrap();
            }
        }
        assert!(repo.song(v).is_ok());
        drop(repo);
        // Only a writer without foreign keys can attach a background to a missing slide.
        let db = Connection::open(&path).unwrap();
        db.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        db.execute(
            "INSERT INTO section_backgrounds VALUES(?,?,5,?)",
            params![&v.id.0[..], v.revision, image("a.png").encode()],
        )
        .unwrap();
        drop(db);
        assert!(matches!(Repository::open(&path), Err(Error::Corrupt)));
    }
    fn lyric_song(title: &str, lyrics: &str) -> Song {
        Song {
            title: title.into(),
            authors: String::new(),
            copyright: String::new(),
            license: String::new(),
            administrator: String::new(),
            song_number: String::new(),
            variants: Vec::new(),
            sections: vec![Section {
                id: SectionId::allocate(),
                label: "Verse 1".into(),
                lyrics: lyrics.into(),
                format: Default::default(),
                background: None,
            }],
            master: None,
        }
    }
    fn titles(repo: &Repository, query: &str) -> Vec<String> {
        repo.search(query, 1)
            .unwrap()
            .hits
            .into_iter()
            .map(|hit| hit.title)
            .collect()
    }
    fn sorted(mut titles: Vec<String>) -> Vec<String> {
        titles.sort();
        titles
    }
    fn table_rows(db: &Connection, sql: &str) -> Vec<Vec<rusqlite::types::Value>> {
        let mut statement = db.prepare(sql).unwrap();
        let columns = statement.column_count();
        statement
            .query_map([], |r| (0..columns).map(|i| r.get(i)).collect())
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap()
    }
    /// Every song-owned table, for proving that index repair leaves them alone.
    fn song_tables(path: &std::path::Path) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
        let db = Connection::open(path).unwrap();
        [
            "songs",
            "song_revisions",
            "section_ids",
            "section_formats",
            "song_backgrounds",
            "section_backgrounds",
            "variants",
            "occurrences",
        ]
        .iter()
        .map(|table| table_rows(&db, &format!("SELECT * FROM {table} ORDER BY rowid")))
        .collect()
    }
    #[test]
    fn fts5_is_compiled_into_the_bundled_sqlite() {
        let db = Connection::open_in_memory().unwrap();
        let options = table_rows(&db, "PRAGMA compile_options");
        assert!(
            options
                .iter()
                .any(|row| row[0] == rusqlite::types::Value::Text("ENABLE_FTS5".into()))
        );
        db.execute_batch(
            "CREATE VIRTUAL TABLE probe USING fts5(x, tokenize='unicode61 remove_diacritics 2')",
        )
        .unwrap();
    }
    #[test]
    fn search_folds_case_and_diacritics_and_keeps_display_text() {
        let (_d, _p, mut repo) = fixture();
        let composed = lyric_song("Café Lumière", "Original morning line");
        let decomposed = lyric_song("Cafe\u{301} Noir", "Another original line");
        let kasih = lyric_song(
            "Kasih-Mu Sungguh Ajaib",
            "Yesus, Engkau s'lamanya setia\nKu t'lah menerima anugerah-Mu",
        );
        let mut bapa = lyric_song("Bapa Yang Kekal", "Bersyukur kepada-Mu, ya Tuhan");
        bapa.authors = "Penulis Asli".into();
        for song in [&composed, &decomposed, &kasih, &bapa] {
            repo.save_song(None, song.clone()).unwrap();
        }
        let both = vec!["Cafe\u{301} Noir".to_string(), "Café Lumière".to_string()];
        for query in ["cafe", "CAFÉ", "cafe\u{301}", "Café"] {
            assert_eq!(sorted(titles(&repo, query)), both, "{query:?}");
        }
        let expectations: [(&str, &[&str]); 18] = [
            ("lumiere", &["Café Lumière"]),
            ("LUMIÈRE", &["Café Lumière"]),
            ("kasih mu", &["Kasih-Mu Sungguh Ajaib"]),
            ("kasih-mu", &["Kasih-Mu Sungguh Ajaib"]),
            ("KASIH-MU", &["Kasih-Mu Sungguh Ajaib"]),
            // Punctuation separates words, so the joined spelling does not match.
            ("kasihmu", &[]),
            ("sungguh", &["Kasih-Mu Sungguh Ajaib"]),
            ("t'lah", &["Kasih-Mu Sungguh Ajaib"]),
            ("s'lamanya", &["Kasih-Mu Sungguh Ajaib"]),
            ("slamanya", &[]),
            ("anugerah", &["Kasih-Mu Sungguh Ajaib"]),
            ("tuhan", &["Bapa Yang Kekal"]),
            ("penulis", &["Bapa Yang Kekal"]),
            ("yesus engkau", &["Kasih-Mu Sungguh Ajaib"]),
            ("yesus bapa", &[]),
            ("ajai", &["Kasih-Mu Sungguh Ajaib"]),
            ("keka", &["Bapa Yang Kekal"]),
            // Only the last term is a prefix.
            ("kek yang", &[]),
        ];
        for (query, expected) in expectations {
            assert_eq!(titles(&repo, query), expected, "{query:?}");
        }
        for hit in repo.search("cafe", 1).unwrap().hits {
            let stored = repo.song(hit.version).unwrap();
            assert_eq!(hit.title, stored.title);
            assert!(stored == composed || stored == decomposed);
        }
        assert!(repo.check_search().unwrap());
    }
    #[test]
    fn search_treats_punctuation_and_fts_syntax_as_text() {
        let (_d, _p, mut repo) = fixture();
        for (title, lyrics) in [
            (
                "It Is Well (With My Soul)",
                "When peace like a river\nAND sorrows like sea billows roll",
            ),
            ("Draw Near", "Nearer still, O Lord, your title: glory"),
            ("Plain Song", "Nothing special"),
        ] {
            repo.save_song(None, lyric_song(title, lyrics)).unwrap();
        }
        let well: &[&str] = &["It Is Well (With My Soul)"];
        let near: &[&str] = &["Draw Near"];
        let expectations: [(&str, &[&str]); 21] = [
            ("\"", &[]),
            ("\"\"", &[]),
            ("*", &[]),
            ("-", &[]),
            ("(", &[]),
            (")", &[]),
            ("well,", well),
            ("(with", well),
            ("soul)", well),
            ("\"well", well),
            ("^when", well),
            ("peace*", well),
            // Operators are words: AND must occur, OR/NOT/NEAR are not operators.
            ("AND", well),
            ("OR plain", &[]),
            ("NOT peace", &[]),
            ("NEAR(", near),
            ("NEAR(peace river)", &[]),
            // A column filter is a two-word phrase.
            ("title:glory", near),
            ("lyrics:nothing", &[]),
            ("a\"b", &[]),
            ("'; DROP TABLE songs; --", &[]),
        ];
        for (query, expected) in expectations {
            assert_eq!(titles(&repo, query), expected, "{query:?}");
        }
        assert_eq!(repo.heads(false, None).unwrap().len(), 3);
        let longest = format!("well {}", "x".repeat(search::MAX_QUERY_BYTES - 5));
        assert_eq!(titles(&repo, &longest), Vec::<String>::new());
        assert_eq!(repo.search(&format!("{longest}x"), 1), Err(Error::Invalid));
    }
    #[test]
    fn empty_and_whitespace_queries_return_no_hits() {
        let (_d, path, mut repo) = fixture();
        repo.save_song(None, lyric_song("Morning Light", "original"))
            .unwrap();
        for query in ["", " ", "\t\n", "\u{3000}", " \" * "] {
            assert_eq!(
                repo.search(query, 9),
                Ok(search::Results {
                    generation: 9,
                    hits: Vec::new(),
                    truncated: false
                }),
                "{query:?}"
            );
        }
        let mut worker = Worker::open(path).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Opened)));
        worker
            .submit(Command::Search {
                query: "   ".into(),
                generation: 7,
            })
            .unwrap();
        assert!(matches!(
            wait(&mut worker),
            Ok(Reply::Search(r)) if r.generation == 7 && r.hits.is_empty()
        ));
        assert!(matches!(
            worker.submit(Command::Search {
                query: "x".repeat(search::MAX_QUERY_BYTES + 1),
                generation: 8,
            }),
            Err(Error::Invalid)
        ));
    }
    #[test]
    fn search_ranks_title_above_lyrics_and_breaks_ties_by_title_then_id() {
        let (_d, _p, mut repo) = fixture();
        repo.save_song(None, lyric_song("Evening Hymn", "morning comes again"))
            .unwrap();
        repo.save_song(None, lyric_song("Morning Light", "original first song"))
            .unwrap();
        let mut faithful = Vec::new();
        // bm25 normalizes by the whole row's token count, so these duplicates
        // have equally long lyrics to tie exactly.
        for lyrics in ["one", "two", "six"] {
            faithful.push(
                repo.save_song(None, lyric_song("Great Is Thy Faithfulness", lyrics))
                    .unwrap(),
            );
        }
        for title in ["Zeal Grace", "amber grace", "Amber Grace"] {
            repo.save_song(None, lyric_song(title, "unrelated words"))
                .unwrap();
        }
        assert_eq!(titles(&repo, "morning"), ["Morning Light", "Evening Hymn"]);
        // Equal scores: title ignoring ASCII case, then exact title, then ID.
        assert_eq!(
            titles(&repo, "grace"),
            ["Amber Grace", "amber grace", "Zeal Grace"]
        );
        faithful.sort_by_key(|v| v.id.0);
        for _ in 0..3 {
            let hits = repo.search("great faithfulness", 1).unwrap().hits;
            assert_eq!(
                hits.iter().map(|hit| hit.version).collect::<Vec<_>>(),
                faithful
            );
        }
        // With equal title matches, the shorter song ranks first.
        repo.save_song(
            None,
            lyric_song("Great Is Thy Faithfulness", "much longer original lyrics"),
        )
        .unwrap();
        let hits = repo.search("great faithfulness", 1).unwrap().hits;
        assert_eq!(hits.len(), 4);
        assert_eq!(
            hits[..3].iter().map(|hit| hit.version).collect::<Vec<_>>(),
            faithful
        );
    }
    #[test]
    fn damage_outside_the_search_index_keeps_the_library_closed() {
        let (_d, path, mut repo) = fixture();
        repo.save_song(None, lyric_song("Morning Light", "original"))
            .unwrap();
        let (root, page_size): (i64, i64) = repo
            .db
            .query_row(
                "SELECT rootpage,(SELECT page_size FROM pragma_page_size) FROM sqlite_schema WHERE name='song_revisions'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        drop(repo);
        let mut bytes = std::fs::read(&path).unwrap();
        // An invalid b-tree page type on the song table's root page.
        bytes[((root - 1) * page_size) as usize] = 0x07;
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(Repository::open(&path), Err(Error::Corrupt)));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    #[test]
    fn search_truncates_at_the_hit_bound_in_title_order() {
        let (_d, _p, mut repo) = fixture();
        let mut expected = Vec::new();
        for i in 0..=search::MAX_HITS {
            let title = format!("Common {i}");
            repo.save_song(None, lyric_song(&title, "original"))
                .unwrap();
            expected.push(title);
        }
        expected.sort();
        expected.truncate(search::MAX_HITS);
        let results = repo.search("common", 3).unwrap();
        assert!(results.truncated);
        assert_eq!(
            results
                .hits
                .into_iter()
                .map(|hit| hit.title)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(!repo.search("common 7", 3).unwrap().truncated);
    }
    #[test]
    fn search_index_follows_save_edit_delete_in_the_same_transaction() {
        let (_d, path, mut repo) = fixture();
        let first = repo
            .save_song(None, lyric_song("Shelter", "refuge in the storm"))
            .unwrap();
        assert_eq!(titles(&repo, "refuge"), ["Shelter"]);
        let mut edited = lyric_song("Harbor", "anchor holds");
        edited.copyright = "Original copyright holder".into();
        let second = repo.save_song(Some(first), edited).unwrap();
        assert!(titles(&repo, "refuge").is_empty());
        assert!(titles(&repo, "shelter").is_empty());
        assert_eq!(
            repo.search("anchor", 1).unwrap().hits,
            [search::Hit {
                version: second,
                title: "Harbor".into()
            }]
        );
        assert_eq!(titles(&repo, "holder"), ["Harbor"]);
        // A failure while indexing rolls back the whole save.
        repo.db.execute_batch("CREATE TRIGGER fail_index BEFORE INSERT ON search_rows BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(
            repo.save_song(None, lyric_song("Lantern", "light"))
                .is_err()
        );
        assert_eq!(repo.heads(false, None).unwrap(), [second]);
        assert!(titles(&repo, "lantern").is_empty());
        repo.db.execute_batch("DROP TRIGGER fail_index; CREATE TRIGGER fail_unindex BEFORE DELETE ON search_rows BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(repo.delete_song(second).is_err());
        assert_eq!(repo.heads(false, None).unwrap(), [second]);
        assert_eq!(titles(&repo, "anchor"), ["Harbor"]);
        repo.db.execute_batch("DROP TRIGGER fail_unindex").unwrap();
        repo.delete_song(second).unwrap();
        assert!(titles(&repo, "anchor").is_empty());
        assert!(repo.check_search().unwrap());
        // History still resolves after the song left the index.
        assert_eq!(repo.song(first).unwrap().title, "Shelter");
        drop(repo);
        let mut worker = Worker::open(path).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Opened)));
        worker
            .submit(Command::SaveSong(None, lyric_song("Lantern", "light")))
            .unwrap();
        let Ok(Reply::Saved(lantern)) = wait(&mut worker) else {
            panic!()
        };
        worker
            .submit(Command::Search {
                query: "lant".into(),
                generation: 4,
            })
            .unwrap();
        assert!(matches!(
            wait(&mut worker),
            Ok(Reply::Search(r)) if r.generation == 4 && r.hits == [search::Hit { version: lantern, title: "Lantern".into() }]
        ));
    }
    #[test]
    fn search_generations_reject_late_and_canceled_older_replies() {
        let (_d, path, mut repo) = fixture();
        repo.save_song(None, lyric_song("Faith Alone", "original faith line"))
            .unwrap();
        repo.save_song(None, lyric_song("Grace Alone", "original grace line"))
            .unwrap();
        drop(repo);
        let mut a = Worker::open(path.clone()).unwrap();
        let mut b = Worker::open(path).unwrap();
        assert!(matches!(wait(&mut a), Ok(Reply::Opened)));
        assert!(matches!(wait(&mut b), Ok(Reply::Opened)));
        let mut generations = search::Generations::default();
        let mut shown = None;
        let older = generations.advance();
        a.submit(Command::Search {
            query: "faith".into(),
            generation: older,
        })
        .unwrap();
        let newer = generations.advance();
        b.submit(Command::Search {
            query: "grace".into(),
            generation: newer,
        })
        .unwrap();
        // The newer reply is consumed first; the older one arrives after it.
        for worker in [&mut b, &mut a] {
            let Ok(Reply::Search(results)) = wait(worker) else {
                panic!()
            };
            if generations.is_current(results.generation) {
                shown = Some(results);
            }
        }
        let shown = shown.unwrap();
        assert_eq!(shown.generation, newer);
        assert_eq!(shown.hits[0].title, "Grace Alone");
        assert_eq!(shown.hits.len(), 1);
        // One worker: a query superseded while busy is canceled and ignored.
        let first = generations.advance();
        let cancel = a
            .submit(Command::Search {
                query: "faith".into(),
                generation: first,
            })
            .unwrap();
        let second = generations.advance();
        assert!(matches!(
            a.submit(Command::Search {
                query: "grace".into(),
                generation: second,
            }),
            Err(Error::Busy)
        ));
        cancel.cancel();
        match wait(&mut a) {
            Err(Error::Canceled) => (),
            Ok(Reply::Search(r)) => {
                assert!(r.generation == first && !generations.is_current(first))
            }
            other => panic!("{other:?}"),
        }
        a.submit(Command::Search {
            query: "grace".into(),
            generation: second,
        })
        .unwrap();
        assert!(matches!(
            wait(&mut a),
            Ok(Reply::Search(r)) if generations.is_current(r.generation) && r.hits[0].title == "Grace Alone"
        ));
        let (_d, _p, mut repo) = fixture();
        let canceled = Arc::new(AtomicBool::new(true));
        let request = Request {
            command: Command::Search {
                query: "grace".into(),
                generation: 1,
            },
            canceled,
        };
        assert!(matches!(execute(&mut repo, request), Err(Error::Canceled)));
    }
    #[test]
    fn damaged_or_missing_search_index_is_rebuilt_without_touching_songs() {
        for damage in ["stale", "content", "data", "mapping", "missing"] {
            let (_d, path, mut repo) = fixture();
            let mut arranged = arranged_song();
            arranged.title = "Morning Light".into();
            repo.save_song(None, arranged).unwrap();
            repo.save_song(None, lyric_song("Evening Hymn", "quiet night"))
                .unwrap();
            let retired = repo
                .save_song(None, lyric_song("Retired Anthem", "gone"))
                .unwrap();
            repo.delete_song(retired).unwrap();
            let songs = song_tables(&path);
            let other = Connection::open(&path).unwrap();
            other
                .execute_batch(match damage {
                    "stale" => "UPDATE song_search SET title='Ghost' WHERE title='Evening Hymn'",
                    "content" => {
                        "UPDATE song_search_content SET c0='Ghost' WHERE c0='Evening Hymn'"
                    }
                    "data" => "DELETE FROM song_search_data WHERE id>10",
                    "mapping" => {
                        "DELETE FROM search_rows WHERE row=(SELECT min(row) FROM search_rows)"
                    }
                    _ => "DROP TABLE song_search",
                })
                .unwrap();
            drop(other);
            if damage != "missing" {
                assert!(!repo.check_search().unwrap(), "{damage}");
            }
            let correct = |repo: &Repository| {
                assert_eq!(titles(repo, "morning"), ["Morning Light"], "{damage}");
                assert_eq!(titles(repo, "evening"), ["Evening Hymn"], "{damage}");
                assert!(titles(repo, "ghost").is_empty(), "{damage}");
                assert!(titles(repo, "retired").is_empty(), "{damage}");
                assert!(repo.check_search().unwrap(), "{damage}");
            };
            if matches!(damage, "stale" | "content") {
                // Only the full check finds these; reopening keeps them.
                assert_eq!(
                    repo.repair_search(),
                    Ok(search::IndexState::Rebuilt { songs: 2 })
                );
                correct(&repo);
            } else {
                drop(repo);
                let repo = Repository::open(&path).unwrap_or_else(|e| panic!("{damage}: {e:?}"));
                correct(&repo);
            }
            assert_eq!(song_tables(&path), songs, "{damage}");
        }
        let (_d, path, mut repo) = fixture();
        repo.save_song(None, lyric_song("Morning Light", "original"))
            .unwrap();
        drop(repo);
        let mut worker = Worker::open(path).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Opened)));
        worker.submit(Command::RepairSearch).unwrap();
        assert!(matches!(
            wait(&mut worker),
            Ok(Reply::SearchIndex(search::IndexState::Healthy))
        ));
        worker.submit(Command::RebuildSearch).unwrap();
        assert!(matches!(
            wait(&mut worker),
            Ok(Reply::SearchIndex(search::IndexState::Rebuilt { songs: 1 }))
        ));
    }
    /// The schema 3 library after the schema 4 migration, with a master and a
    /// deleted song.
    fn schema4(path: &std::path::Path) -> Version {
        let v = schema3(path);
        let db = Connection::open(path).unwrap();
        db.execute_batch(SCHEMA4).unwrap();
        db.execute(
            "INSERT INTO song_backgrounds VALUES(?,2,?)",
            params![&v.id.0[..], Background::color([1, 2, 3]).encode()],
        )
        .unwrap();
        let retired = Id([43; 16]);
        db.execute("INSERT INTO songs VALUES(?,1,1)", params![&retired.0[..]])
            .unwrap();
        let mut song = song();
        song.title = "Retired anthem".into();
        song.sections.clear();
        db.execute(
            "INSERT INTO song_revisions VALUES(?,1,?)",
            params![&retired.0[..], song.encode()],
        )
        .unwrap();
        v
    }
    #[test]
    fn schema4_migrates_with_a_verified_backup_and_builds_the_search_index() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        let v = schema4(&path);
        let repo = Repository::open(&path).unwrap();
        assert_eq!(schema_of(&path), CURRENT);
        let head = Version { revision: 2, ..v };
        let current = repo.song(head).unwrap();
        assert_eq!(current.master, Some(Background::color([1, 2, 3])));
        for query in ["cafe", "CAFÉ", "سلام", "二", "作者", "test"] {
            let hits = repo.search(query, 1).unwrap().hits;
            assert_eq!(
                hits,
                [search::Hit {
                    version: head,
                    title: "Café e\u{301} سلام".into()
                }],
                "{query:?}"
            );
        }
        assert!(titles(&repo, "retired").is_empty());
        assert!(repo.check_search().unwrap());
        let backup = d.path().join("library.schema4-backup");
        let before = std::fs::read(&backup).unwrap();
        assert_eq!(schema_of(&backup), 4);
        let old = Repository::open_internal(&backup, false).unwrap();
        old.verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        assert_eq!(old.song(head).unwrap(), current);
        let restored = d.path().join("restored");
        Repository::restore_new(&backup, &restored, &AtomicBool::new(false)).unwrap();
        assert_eq!(schema_of(&restored), 4);
        drop((old, repo));
        assert_eq!(
            Repository::open(&path).unwrap().song(head).unwrap(),
            current
        );
        assert_eq!(std::fs::read(&backup).unwrap(), before);
    }
    #[test]
    fn schema4_migration_gates_leave_the_library_unchanged() {
        for failure in ["backup", "writer", "ddl"] {
            let d = tempfile::tempdir().unwrap();
            let path = d.path().join("library");
            schema4(&path);
            let db = Connection::open(&path).unwrap();
            let backup = d.path().join("library.schema4-backup");
            match failure {
                "backup" => std::fs::write(&backup, b"retain").unwrap(),
                "writer" => db
                    .execute_batch("BEGIN IMMEDIATE; UPDATE songs SET deleted=1")
                    .unwrap(),
                _ => db
                    .execute_batch("CREATE TABLE search_rows(block_upgrade)")
                    .unwrap(),
            }
            let before = std::fs::read(&path).unwrap();
            let result = Repository::open(&path).map(|_| ());
            match failure {
                "backup" => assert_eq!(result, Err(Error::Exists)),
                "writer" => assert_eq!(result, Err(Error::Locked)),
                _ => assert_eq!(result, Err(Error::Corrupt)),
            }
            assert_eq!(std::fs::read(&path).unwrap(), before, "{failure}");
            assert_eq!(
                db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                4,
                "{failure}"
            );
            match failure {
                "backup" => assert_eq!(std::fs::read(&backup).unwrap(), b"retain"),
                "writer" => assert!(!backup.exists()),
                _ => Repository::open_internal(&backup, false)
                    .unwrap()
                    .verify_history(&AtomicBool::new(false), std::time::Instant::now())
                    .unwrap(),
            }
        }
    }
    #[test]
    fn schema4_migration_abort_keeps_schema4_and_the_verified_backup() {
        const CHILD: &str = "SELA_ABORT_MIGRATION_SCHEMA5";
        if let Some(path) = std::env::var_os(CHILD) {
            let _ = Repository::open(std::path::Path::new(&path));
            panic!("schema 5 abort hook did not run");
        }
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        schema4(&path);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::tests::schema4_migration_abort_keeps_schema4_and_the_verified_backup",
            ])
            .env(CHILD, &path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success());
        assert_eq!(schema_of(&path), 4);
        for table in ["search_rows", "song_search", "song_search_data"] {
            assert!(
                !Connection::open(&path)
                    .unwrap()
                    .prepare(&format!("SELECT 1 FROM sqlite_schema WHERE name='{table}'"))
                    .unwrap()
                    .exists([])
                    .unwrap(),
                "{table}"
            );
        }
        Repository::open_internal(&d.path().join("library.schema4-backup"), false)
            .unwrap()
            .verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        assert!(matches!(Repository::open(&path), Err(Error::Exists)));
    }
    fn modified_order(repo: &Repository) -> Vec<String> {
        repo.library(SongOrder {
            sort: SongSort::Modified,
            descending: false,
        })
        .unwrap()
        .into_iter()
        .map(|(_, t)| t)
        .collect()
    }
    fn titled(title: &str) -> Song {
        let mut s = song();
        s.title = title.into();
        s
    }
    #[test]
    fn song_properties_persist_across_revisions_reopen_and_search() {
        let (_d, p, mut repo) = fixture();
        let mut first = song();
        first.administrator = "Acme Music".into();
        first.song_number = "7654321".into();
        let v1 = repo.save_song(None, first.clone()).unwrap();
        assert_eq!(repo.song(v1).unwrap(), first);

        let mut edited = first.clone();
        edited.song_number = "42".into();
        edited.administrator.clear();
        let v2 = repo.save_song(Some(v1), edited.clone()).unwrap();
        assert_eq!(repo.song(v2).unwrap(), edited);
        assert_eq!(repo.song(v1).unwrap(), first, "old revision is immutable");

        // Neither field is a stored row once empty.
        let mut plain = first.clone();
        plain.administrator.clear();
        plain.song_number.clear();
        let v3 = repo.save_song(Some(v2), plain.clone()).unwrap();
        assert_eq!(repo.song(v3).unwrap(), plain);
        let rows: i64 = Connection::open(&p)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM song_metadata WHERE revision=3",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rows, 0);

        let v4 = repo.save_song(Some(v3), edited.clone()).unwrap();
        assert_eq!(titles(&repo, "42"), vec![edited.title.clone()]);
        assert!(titles(&repo, "7654321").is_empty(), "head only");
        let mut with_admin = first.clone();
        with_admin.song_number = "99".into();
        let v5 = repo.save_song(Some(v4), with_admin.clone()).unwrap();
        assert_eq!(titles(&repo, "acme"), vec![with_admin.title.clone()]);
        assert!(repo.check_search().unwrap());
        drop(repo);
        let repo = Repository::open(&p).unwrap();
        assert_eq!(repo.song(v5).unwrap(), with_admin);
        assert!(repo.check_search().unwrap(), "index agrees after reopen");
    }
    #[test]
    fn invalid_song_properties_are_rejected_without_saving() {
        let (_d, _p, mut repo) = fixture();
        for (administrator, number) in [
            ("", "12ab"),
            ("", "-5"),
            ("", " 5"),
            ("", "١٢٣"),
            ("", "1234567890123"),
            (&"a".repeat(MAX_ADMINISTRATOR + 1)[..], ""),
        ] {
            let mut s = song();
            s.administrator = administrator.into();
            s.song_number = number.into();
            assert_eq!(s.validate(), Err(Error::Invalid), "{number:?}");
            assert_eq!(repo.save_song(None, s), Err(Error::Invalid));
        }
        assert!(repo.heads(false, None).unwrap().is_empty());
        let mut limit = song();
        limit.administrator = "a".repeat(MAX_ADMINISTRATOR);
        limit.song_number = "1".repeat(MAX_SONG_NUMBER);
        repo.save_song(None, limit).unwrap();
    }
    #[test]
    fn date_modified_uses_the_stored_stamp_and_survives_an_index_rebuild() {
        let (_d, p, mut repo) = fixture();
        let a = repo.save_song(None, titled("Alpha")).unwrap();
        repo.save_song(None, titled("Bravo")).unwrap();
        repo.save_song(None, titled("Charlie")).unwrap();
        assert_eq!(modified_order(&repo), ["Alpha", "Bravo", "Charlie"]);
        repo.save_song(Some(a), titled("Alpha 2")).unwrap();
        let expected = ["Bravo", "Charlie", "Alpha 2"];
        assert_eq!(modified_order(&repo), expected);

        // A rebuild reassigns every search row in id order; the stamps stay.
        repo.rebuild_search().unwrap();
        assert_eq!(modified_order(&repo), expected);
        drop(repo);
        let db = Connection::open(&p).unwrap();
        db.execute_batch("DROP TABLE song_search; DROP TABLE search_rows;")
            .unwrap();
        drop(db);
        let mut repo = Repository::open(&p).unwrap();
        assert_eq!(modified_order(&repo), expected, "repair on open");
        let stamps: Vec<i64> = {
            let db = Connection::open(&p).unwrap();
            let mut s = db
                .prepare("SELECT modified FROM songs ORDER BY modified")
                .unwrap();
            s.query_map([], |r| r.get(0))
                .unwrap()
                .collect::<std::result::Result<_, _>>()
                .unwrap()
        };
        assert!(stamps.windows(2).all(|w| w[0] < w[1]), "{stamps:?}");
        assert!(stamps[0] > 1_000_000_000_000, "a clock time, not a row");
        // Saves in the same millisecond still order strictly.
        let c = repo.save_song(None, titled("Delta")).unwrap();
        repo.save_song(Some(c), titled("Delta 2")).unwrap();
        assert_eq!(modified_order(&repo).last().unwrap(), "Delta 2");
    }
    /// The search index as schemas 5 and 6 created it: no labels column, no
    /// sort keys.
    const SEARCH5: &str = "CREATE TABLE search_rows(row INTEGER PRIMARY KEY, song BLOB NOT NULL UNIQUE REFERENCES songs(id));
CREATE VIRTUAL TABLE song_search USING fts5(title, lyrics, metadata, tokenize='unicode61 remove_diacritics 2');";
    /// Fills a `SEARCH5` index the way schema 5 and 6 did.
    fn fill_search5(db: &Connection) {
        let metadata = has_table(db, "song_metadata").unwrap();
        let mut statement = db.prepare("SELECT s.id,r.payload,s.head FROM songs s JOIN song_revisions r ON r.id=s.id AND r.revision=s.head WHERE s.deleted=0 ORDER BY s.id").unwrap();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let id: Vec<u8> = row.get(0).unwrap();
            let mut song = Song::decode(&row.get::<_, Vec<u8>>(1).unwrap()).unwrap();
            if metadata {
                apply_metadata(
                    &mut song,
                    read_metadata(db, &id, row.get(2).unwrap()).unwrap(),
                );
            }
            db.execute("INSERT INTO search_rows(song) VALUES(?)", params![id])
                .unwrap();
            let [title, lyrics, metadata, _] = search_fields(&song);
            db.execute(
                "INSERT INTO song_search(rowid,title,lyrics,metadata) VALUES(?,?,?,?)",
                params![db.last_insert_rowid(), title, lyrics, metadata],
            )
            .unwrap();
        }
    }
    /// The schema 4 library after the schema 5 migration: search index only.
    fn schema5(path: &std::path::Path) -> Version {
        let v = schema4(path);
        let db = Connection::open(path).unwrap();
        db.execute_batch(SEARCH5).unwrap();
        fill_search5(&db);
        db.execute_batch("PRAGMA user_version=5").unwrap();
        v
    }
    #[test]
    fn schema5_migration_stamps_modified_from_the_index_order_and_keeps_history() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        let v = schema5(&path);
        let rows: Vec<(Vec<u8>, i64)> = {
            let db = Connection::open(&path).unwrap();
            let mut s = db
                .prepare("SELECT song,row FROM search_rows ORDER BY row")
                .unwrap();
            s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<std::result::Result<_, _>>()
                .unwrap()
        };
        assert!(!rows.is_empty());
        let repo = Repository::open(&path).unwrap();
        assert_eq!(schema_of(&path), CURRENT);
        let db = Connection::open(&path).unwrap();
        for (song, row) in &rows {
            let modified: i64 = db
                .query_row("SELECT modified FROM songs WHERE id=?", [song], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(modified, *row);
        }
        let retired: i64 = db
            .query_row("SELECT modified FROM songs WHERE deleted=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(retired, 0, "songs without an index row take 0");
        let head = Version { revision: 2, ..v };
        let current = repo.song(head).unwrap();
        assert_eq!(
            (current.administrator.as_str(), current.song_number.as_str()),
            ("", "")
        );
        assert!(repo.check_search().unwrap());
        let backup = d.path().join("library.schema5-backup");
        assert_eq!(schema_of(&backup), 5);
        let old = Repository::open_internal(&backup, false).unwrap();
        old.verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        assert_eq!(old.song(head).unwrap(), current);
        // The next save outranks every migrated row.
        drop((old, repo, db));
        let mut repo = Repository::open(&path).unwrap();
        repo.save_song(None, titled("Newest")).unwrap();
        assert_eq!(modified_order(&repo).last().unwrap(), "Newest");
    }
    #[test]
    fn schema5_migration_abort_keeps_schema5_and_the_verified_backup() {
        const CHILD: &str = "SELA_ABORT_MIGRATION_SCHEMA6";
        if let Some(path) = std::env::var_os(CHILD) {
            let _ = Repository::open(std::path::Path::new(&path));
            panic!("schema 6 abort hook did not run");
        }
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        schema5(&path);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::tests::schema5_migration_abort_keeps_schema5_and_the_verified_backup",
            ])
            .env(CHILD, &path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success());
        assert_eq!(schema_of(&path), 5);
        let db = Connection::open(&path).unwrap();
        assert!(db.prepare("SELECT modified FROM songs").is_err());
        assert!(db.prepare("SELECT 1 FROM song_metadata").is_err());
        Repository::open_internal(&d.path().join("library.schema5-backup"), false)
            .unwrap()
            .verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
    }

    fn import_write(title: &str, lyrics: &str) -> ImportWrite {
        ImportWrite {
            replace: None,
            song: Arc::new(lyric_song(title, lyrics)),
        }
    }

    #[test]
    fn an_import_commits_every_song_in_one_transaction_and_indexes_it() {
        let (_d, _path, mut repo) = fixture();
        let existing = repo
            .save_song(None, lyric_song("Kasih Setia", "old words"))
            .unwrap();
        let mut replacement = import_write("Kasih Setia", "kasih-Mu sétia selamanya");
        replacement.replace = Some(existing);
        let saved = repo
            .commit_import(
                vec![
                    import_write("Bapa yang Kekal", "Engkau sungguh ajaib"),
                    replacement,
                    import_write("Harbor Light", "the lamp is lit"),
                ],
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(saved.len(), 3);
        assert_eq!(saved[1].id, existing.id, "Replace keeps the song");
        assert_eq!(saved[1].revision, existing.revision + 1);
        // The same index as an editor save: accented words, prefix search.
        assert_eq!(titles(&repo, "sétia"), ["Kasih Setia"]);
        assert_eq!(titles(&repo, "ajai"), ["Bapa yang Kekal"]);
        assert!(titles(&repo, "old").is_empty());
        assert!(repo.check_search().unwrap());
        assert_eq!(repo.heads(false, None).unwrap().len(), 3);
        // History of the replaced song stays readable.
        assert_eq!(repo.song(existing).unwrap().sections[0].lyrics, "old words");
    }

    #[test]
    fn a_failing_save_rolls_the_whole_import_back() {
        let (_d, _path, mut repo) = fixture();
        let kept = repo
            .save_song(None, lyric_song("Kept", "kept words"))
            .unwrap();
        // The third song's index insert fails (failure injection).
        repo.db
            .execute_batch(
                "CREATE TRIGGER fail_third BEFORE INSERT ON search_rows WHEN (SELECT count(*) FROM search_rows) >= 3 BEGIN SELECT RAISE(ABORT,'injected'); END;",
            )
            .unwrap();
        let writes = vec![
            import_write("One", "first"),
            import_write("Two", "second"),
            import_write("Three", "third"),
        ];
        assert_eq!(
            repo.commit_import(writes.clone(), &AtomicBool::new(false)),
            Err(Error::Io)
        );
        assert_eq!(repo.heads(false, None).unwrap(), [kept]);
        assert!(titles(&repo, "first").is_empty());
        assert!(repo.check_search().unwrap());
        // A replace of a song that moved on conflicts, and also rolls back.
        repo.db.execute_batch("DROP TRIGGER fail_third").unwrap();
        let newer = repo
            .save_song(Some(kept), lyric_song("Kept", "newer"))
            .unwrap();
        let mut stale = import_write("Kept", "stale replace");
        stale.replace = Some(kept);
        assert_eq!(
            repo.commit_import(
                vec![import_write("One", "first"), stale],
                &AtomicBool::new(false)
            ),
            Err(Error::Conflict)
        );
        assert_eq!(repo.heads(false, None).unwrap(), [newer]);
        // A cancelled commit saves nothing.
        assert_eq!(
            repo.commit_import(writes.clone(), &AtomicBool::new(true)),
            Err(Error::Canceled)
        );
        assert_eq!(repo.heads(false, None).unwrap(), [newer]);
        // An invalid song refuses the whole batch before any write.
        let mut invalid = writes;
        Arc::make_mut(&mut invalid[1].song).title = " ".into();
        assert_eq!(
            repo.commit_import(invalid, &AtomicBool::new(false)),
            Err(Error::Invalid)
        );
        assert_eq!(repo.heads(false, None).unwrap(), [newer]);
        let too_many = vec![import_write("X", "x"); MAX_IMPORT_SONGS + 1];
        assert_eq!(
            repo.commit_import(too_many, &AtomicBool::new(false)),
            Err(Error::Invalid)
        );
    }

    #[test]
    fn a_cancel_between_songs_or_before_commit_rolls_back() {
        let (_d, _path, mut repo) = fixture();
        let kept = repo
            .save_song(None, lyric_song("Kept", "kept words"))
            .unwrap();
        let writes = vec![
            import_write("One", "first"),
            import_write("Two", "second"),
            import_write("Three", "third"),
        ];
        // Check 1 is before song 1, 2 and 3 between songs, 4 before COMMIT.
        for cancel_at in 2..=4 {
            let calls = std::cell::Cell::new(0);
            let canceled = || {
                calls.set(calls.get() + 1);
                calls.get() >= cancel_at
            };
            assert_eq!(
                repo.commit_import_until(writes.clone(), &canceled),
                Err(Error::Canceled),
                "cancel at check {cancel_at}"
            );
            assert_eq!(calls.get(), cancel_at, "stopped at check {cancel_at}");
            assert_eq!(repo.heads(false, None).unwrap(), [kept]);
            assert!(titles(&repo, "first").is_empty());
            assert!(repo.check_search().unwrap());
        }
        // Never cancelled: four checks, then the batch lands.
        let calls = std::cell::Cell::new(0);
        let saved = repo
            .commit_import_until(writes, &|| {
                calls.set(calls.get() + 1);
                false
            })
            .unwrap();
        assert_eq!((saved.len(), calls.get()), (3, 4));
    }

    #[test]
    fn duplicates_prefer_same_words_then_same_title() {
        let (_d, path, mut repo) = fixture();
        let other = repo
            .save_song(None, lyric_song("Amazing  Grace", "different words"))
            .unwrap();
        let same = repo
            .save_song(None, lyric_song("amazing grace!", "How sweet the sound"))
            .unwrap();
        let deleted = repo
            .save_song(None, lyric_song("Gone Song", "gone"))
            .unwrap();
        repo.delete_song(deleted).unwrap();
        let key = |title: &str, lyrics: &str| batch::duplicate_key(&lyric_song(title, lyrics));
        let found = repo
            .find_duplicates(&[
                key("Amazing Grace", "how sweet, the SOUND"),
                key("Amazing Grace", "new words"),
                key("Gone Song", "gone"),
                key("Brand New", "x"),
            ])
            .unwrap();
        assert_eq!(
            found[0],
            Some(LibraryMatch {
                version: same,
                title: "amazing grace!".into(),
                same_lyrics: true
            })
        );
        let title_only = found[1].clone().unwrap();
        assert!(!title_only.same_lyrics);
        assert!([other, same].contains(&title_only.version));
        assert_eq!(found[2], None, "deleted songs are not duplicates");
        assert_eq!(found[3], None);
        assert_eq!(repo.find_duplicates(&[]).unwrap(), []);
        // Through the worker, with the commit after it.
        drop(repo);
        let mut worker = Worker::open(path).unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Opened)));
        worker
            .submit(Command::FindDuplicates(vec![key("Brand New", "x")]))
            .unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Duplicates(d)) if d == [None]));
        worker
            .submit(Command::CommitImport(vec![import_write("Brand New", "x")]))
            .unwrap();
        assert!(matches!(wait(&mut worker), Ok(Reply::Committed(v)) if v.len() == 1));
        assert!(matches!(
            worker.submit(Command::CommitImport(vec![
                import_write("X", "x");
                MAX_IMPORT_SONGS + 1
            ])),
            Err(Error::Invalid)
        ));
    }

    fn keyed(title: &str, authors: &str, copyright: &str, admin: &str, number: &str) -> Song {
        let mut s = titled(title);
        s.authors = authors.into();
        s.copyright = copyright.into();
        s.administrator = admin.into();
        s.song_number = number.into();
        s
    }
    fn listed(repo: &Repository, sort: SongSort, descending: bool) -> Vec<String> {
        repo.library(SongOrder { sort, descending })
            .unwrap()
            .into_iter()
            .map(|(_, t)| t)
            .collect()
    }
    #[test]
    fn library_sorts_by_author_copyright_administrator_and_song_number() {
        let (_d, _p, mut repo) = fixture();
        for (title, authors, copyright, admin, number) in [
            ("Delta", "bravo writer", "© 2001 Zeta", "Northwind", "900"),
            ("Alpha", "", "", "", ""),
            ("Echo", "Alpha Writer", "(c) 1999 Omega", "acme", "10"),
            ("Bravo", "Charlie  Writer", "© 2001 Zeta", "", "0042"),
            ("Charlie", "Alpha Writer", "", "Acme", "7"),
        ] {
            repo.save_song(None, keyed(title, authors, copyright, admin, number))
                .unwrap();
        }
        // Empty keys last; ASCII case ignored; equal keys by title.
        assert_eq!(
            listed(&repo, SongSort::Author, false),
            ["Charlie", "Echo", "Delta", "Bravo", "Alpha"]
        );
        assert_eq!(
            listed(&repo, SongSort::Author, true),
            ["Bravo", "Delta", "Charlie", "Echo", "Alpha"],
            "descending reverses the keys, keeps title ties ascending and empties last"
        );
        assert_eq!(
            listed(&repo, SongSort::Copyright, false),
            ["Echo", "Bravo", "Delta", "Alpha", "Charlie"]
        );
        assert_eq!(
            listed(&repo, SongSort::Administrator, false),
            ["Charlie", "Echo", "Delta", "Alpha", "Bravo"]
        );
        // Numeric, not text: 7 < 10 < 0042 < 900.
        assert_eq!(
            listed(&repo, SongSort::SongNumber, false),
            ["Charlie", "Echo", "Bravo", "Delta", "Alpha"]
        );
        assert_eq!(
            listed(&repo, SongSort::SongNumber, true),
            ["Delta", "Bravo", "Echo", "Charlie", "Alpha"]
        );
        // A key follows the head revision and a deleted song is gone.
        let alpha = repo
            .library(SongOrder::default())
            .unwrap()
            .into_iter()
            .find(|(_, t)| t == "Alpha")
            .unwrap()
            .0;
        let alpha = repo
            .save_song(Some(alpha), keyed("Alpha", "Aaron", "", "", "1"))
            .unwrap();
        assert_eq!(listed(&repo, SongSort::Author, false)[0], "Alpha");
        assert_eq!(listed(&repo, SongSort::SongNumber, false)[0], "Alpha");
        repo.delete_song(alpha).unwrap();
        assert_eq!(listed(&repo, SongSort::Author, false).len(), 4);
        assert!(repo.check_search().unwrap());
    }
    #[test]
    fn section_labels_are_searchable_and_rank_below_title_and_lyric_matches() {
        let (_d, _p, mut repo) = fixture();
        let with_labels = |title: &str, lyrics: &str, labels: &[&str]| {
            let mut s = titled(title);
            s.authors.clear();
            s.copyright.clear();
            s.license.clear();
            s.sections = labels
                .iter()
                .enumerate()
                .map(|(i, label)| Section {
                    id: SectionId(Id([i as u8 + 1; 16])),
                    label: (*label).into(),
                    lyrics: lyrics.into(),
                    format: Default::default(),
                    background: None,
                })
                .collect();
            s
        };
        repo.save_song(
            None,
            with_labels(
                "Plain Song",
                "nothing here",
                &["Etsaer", "Sadsdasd", "Etsaer"],
            ),
        )
        .unwrap();
        repo.save_song(
            None,
            with_labels("Etsaer Anthem", "other words", &["Verse 1"]),
        )
        .unwrap();
        repo.save_song(
            None,
            with_labels("Lyric Song", "we sing etsaer loud", &["Verse 1"]),
        )
        .unwrap();
        // EW8-OBS-046 examples: word prefixes of a label find the song.
        assert_eq!(titles(&repo, "sad"), ["Plain Song"]);
        assert_eq!(titles(&repo, "SADS"), ["Plain Song"]);
        assert!(
            titles(&repo, "dsd").is_empty(),
            "word prefix, not substring"
        );
        // Title beats lyrics beats a label-only match.
        assert_eq!(
            titles(&repo, "ets"),
            ["Etsaer Anthem", "Lyric Song", "Plain Song"]
        );
        // A label with a number: both words must match.
        assert_eq!(
            sorted(titles(&repo, "verse 1")),
            ["Etsaer Anthem", "Lyric Song"]
        );
        // The stored label column holds each distinct label once, in order.
        let labels: String = repo
            .db
            .query_row(
                "SELECT labels FROM song_search WHERE title='Plain Song'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(labels, "Etsaer\nSadsdasd");
        assert!(repo.check_search().unwrap());
    }
    #[test]
    fn a_label_only_hit_ranks_below_a_lyric_hit_in_a_much_longer_song() {
        // bm25 normalizes by row length, so a 0.5 label weight alone let the
        // short song's label hit outrank the long song's lyric hit.
        let (_d, _p, mut repo) = fixture();
        let long: Vec<String> = (0..400).map(|i| format!("word{}", i % 50)).collect();
        let mut lyric = lyric_song("Long Lyric Song", &format!("{} etsaer", long.join(" ")));
        lyric.sections[0].label = "Verse 1".into();
        for label in ["Chorus", "Bridge"] {
            let mut section = lyric.sections[0].clone();
            section.id = SectionId::allocate();
            section.label = label.into();
            section.lyrics = "more lines".into();
            lyric.sections.push(section);
        }
        repo.save_song(None, lyric).unwrap();
        let mut label = lyric_song("Label Song", "short words here");
        label.sections[0].label = "Etsaer".into();
        repo.save_song(None, label).unwrap();
        for i in 0..20 {
            repo.save_song(
                None,
                lyric_song(&format!("Filler {i}"), "some filler lyric text"),
            )
            .unwrap();
        }
        assert_eq!(titles(&repo, "ets"), ["Long Lyric Song", "Label Song"]);
        // A query with a hit outside the labels keeps its bm25 place even
        // when another of its words is only in a label.
        assert_eq!(titles(&repo, "etsaer short"), ["Label Song"]);
        assert_eq!(titles(&repo, "filler 7"), ["Filler 7"]);
    }
    #[test]
    fn schema5_and_6_libraries_with_only_a_damaged_index_still_migrate() {
        for version in [5, 6] {
            let d = tempfile::tempdir().unwrap();
            let path = d.path().join("library");
            let v = if version == 5 {
                schema5(&path)
            } else {
                schema6(&path)
            };
            Connection::open(&path)
                .unwrap()
                .execute_batch("DELETE FROM song_search_data WHERE id>10")
                .unwrap();
            assert!(!quick_check(&Connection::open(&path).unwrap()).unwrap());
            let repo = Repository::open(&path).unwrap_or_else(|e| panic!("{version}: {e:?}"));
            assert_eq!(schema_of(&path), CURRENT);
            assert!(repo.check_search().unwrap(), "{version}");
            let head = Version { revision: 2, ..v };
            let current = repo.song(head).unwrap();
            assert_eq!(
                titles(&repo, "vers"),
                std::slice::from_ref(&current.title),
                "{version}: rebuilt with labels"
            );
            // The verified backup is the file as it was, damaged index included.
            let backup = d.path().join(format!("library.schema{version}-backup"));
            assert_eq!(schema_of(&backup), version);
            assert!(!quick_check(&Connection::open(&backup).unwrap()).unwrap());
            let old = Repository::open_internal(&backup, false).unwrap();
            old.verify_history(&AtomicBool::new(false), std::time::Instant::now())
                .unwrap();
            assert_eq!(old.song(head).unwrap(), current, "{version}");
        }
        // Damage outside the index is still refused, unchanged and unbacked.
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        schema6(&path);
        let (root, page_size): (i64, i64) = Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT rootpage,(SELECT page_size FROM pragma_page_size) FROM sqlite_schema WHERE name='song_revisions'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[((root - 1) * page_size) as usize] = 0x07;
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(Repository::open(&path), Err(Error::Corrupt)));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(!d.path().join("library.schema6-backup").exists());
    }
    #[test]
    fn stale_labels_or_sort_keys_fail_the_check_and_are_repaired() {
        for damage in [
            "UPDATE search_rows SET author='Nobody'",
            "UPDATE search_rows SET song_number='1'",
            "UPDATE song_search SET labels='Ghost'",
        ] {
            let (_d, _p, mut repo) = fixture();
            repo.save_song(None, keyed("Psalm", "Writer", "© Owner", "Admin", "77"))
                .unwrap();
            assert!(repo.check_search().unwrap());
            repo.db.execute_batch(damage).unwrap();
            assert!(!repo.check_search().unwrap(), "{damage}");
            assert_eq!(
                repo.repair_search().unwrap(),
                search::IndexState::Rebuilt { songs: 1 }
            );
            assert!(repo.check_search().unwrap(), "{damage}");
            assert_eq!(titles(&repo, "verse"), ["Psalm"], "{damage}");
        }
    }
    /// The schema 5 library after the schema 6 migration, with properties on
    /// the head revision that only `song_metadata` holds.
    fn schema6(path: &std::path::Path) -> Version {
        let v = schema5(path);
        let db = Connection::open(path).unwrap();
        db.execute_batch(SCHEMA6).unwrap();
        db.execute(
            "INSERT INTO song_metadata VALUES(?,2,'Zed Admin','42')",
            params![&v.id.0[..]],
        )
        .unwrap();
        v
    }
    #[test]
    fn schema6_migration_indexes_labels_and_sort_keys_and_keeps_the_stamps() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        let v = schema6(&path);
        let stamps = |path: &std::path::Path| -> Vec<(Vec<u8>, i64)> {
            let db = Connection::open(path).unwrap();
            let mut s = db
                .prepare("SELECT id,modified FROM songs ORDER BY id")
                .unwrap();
            s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<std::result::Result<_, _>>()
                .unwrap()
        };
        let before = stamps(&path);
        {
            let db = Connection::open(&path).unwrap();
            assert!(db.prepare("SELECT labels FROM song_search").is_err());
        }
        let repo = Repository::open(&path).unwrap();
        assert_eq!(schema_of(&path), CURRENT);
        assert_eq!(stamps(&path), before, "Date modified survives");
        let head = Version { revision: 2, ..v };
        let current = repo.song(head).unwrap();
        assert_eq!(
            (current.administrator.as_str(), current.song_number.as_str()),
            ("Zed Admin", "42")
        );
        assert!(current.sections.iter().any(|s| s.label == "Verse"));
        assert_eq!(
            titles(&repo, "vers"),
            std::slice::from_ref(&current.title),
            "label search"
        );
        assert_eq!(
            repo.library(SongOrder {
                sort: SongSort::SongNumber,
                descending: false
            })
            .unwrap(),
            [(head, current.title.clone())]
        );
        let keys: (String, String) = repo
            .db
            .query_row(
                "SELECT administrator,song_number FROM search_rows",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(keys, ("Zed Admin".into(), "42".into()));
        assert!(repo.check_search().unwrap());
        let backup = d.path().join("library.schema6-backup");
        assert_eq!(schema_of(&backup), 6);
        let old = Repository::open_internal(&backup, false).unwrap();
        old.verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        assert_eq!(old.song(head).unwrap(), current);
        drop((old, repo));
        // Reopening at schema 7 neither migrates nor rebuilds again.
        let rows = |path: &std::path::Path| -> Vec<i64> {
            let db = Connection::open(path).unwrap();
            let mut s = db
                .prepare("SELECT row FROM search_rows ORDER BY row")
                .unwrap();
            s.query_map([], |r| r.get(0))
                .unwrap()
                .collect::<std::result::Result<_, _>>()
                .unwrap()
        };
        let after = rows(&path);
        let repo = Repository::open(&path).unwrap();
        assert!(repo.check_search().unwrap());
        drop(repo);
        assert_eq!(rows(&path), after);
        assert!(!d.path().join("library.schema7-backup").exists());
    }
    #[test]
    fn schema6_migration_abort_keeps_schema6_and_the_verified_backup() {
        const CHILD: &str = "SELA_ABORT_MIGRATION_SCHEMA7";
        if let Some(path) = std::env::var_os(CHILD) {
            let _ = Repository::open(std::path::Path::new(&path));
            panic!("schema 7 abort hook did not run");
        }
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("library");
        schema6(&path);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::tests::schema6_migration_abort_keeps_schema6_and_the_verified_backup",
            ])
            .env(CHILD, &path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success());
        assert_eq!(schema_of(&path), 6);
        let db = Connection::open(&path).unwrap();
        assert!(db.prepare("SELECT labels FROM song_search").is_err());
        assert!(db.prepare("SELECT author FROM search_rows").is_err());
        Repository::open_internal(&d.path().join("library.schema6-backup"), false)
            .unwrap()
            .verify_history(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        assert!(matches!(Repository::open(&path), Err(Error::Exists)));
    }
}
