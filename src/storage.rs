//! Durable song history and ordered schedule snapshots. Use `Worker` on UI threads.
use crate::arrangement::{
    Arrangement, Occurrence, OccurrenceId, SectionId, SourceSnapshot, Variant, VariantId,
};
use crate::background::Background;
use crate::format::SlideFormat;
use crate::search;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
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
pub const MAX_ITEMS: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        let size = [&self.title, &self.authors, &self.copyright, &self.license]
            .into_iter()
            .chain(self.sections.iter().flat_map(|s| [&s.label, &s.lyrics]))
            .try_fold(0usize, |n, s| n.checked_add(s.len())?.checked_add(4));
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
const SEARCH: &str = "CREATE TABLE search_rows(row INTEGER PRIMARY KEY, song BLOB NOT NULL UNIQUE REFERENCES songs(id));
CREATE VIRTUAL TABLE song_search USING fts5(title, lyrics, metadata, tokenize='unicode61 remove_diacritics 2');";
const SCHEMA_VERSIONS: [i64; 5] = [1, 2, 3, 4, 5];
const CURRENT: i64 = 5;
// bm25 column weights: title, lyrics, metadata.
const SEARCH_SQL: &str = "SELECT m.song,s.head,f.title FROM song_search f JOIN search_rows m ON m.row=f.rowid JOIN songs s ON s.id=m.song WHERE song_search MATCH ?1 AND s.deleted=0 ORDER BY bm25(song_search,10.0,1.0,2.0),f.title COLLATE NOCASE,f.title,m.song LIMIT ?2";

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
            tx.execute_batch("PRAGMA user_version=5")?;
            version = CURRENT;
        } else if app != APP || !SCHEMA_VERSIONS.contains(&version) {
            return Err(Error::Unsupported);
        }
        // quick_check also runs the FTS5 index check. When dropping the derived
        // index makes the file pass, only the index was damaged and it is
        // rebuilt; otherwise the transaction rolls the drop back.
        if !quick_check(&tx)? {
            if version != CURRENT {
                return Err(Error::Corrupt);
            }
            tx.execute_batch("DROP TABLE IF EXISTS song_search")
                .map_err(|_| Error::Corrupt)?;
            if !quick_check(&tx)? {
                return Err(Error::Corrupt);
            }
            rebuild_search(&tx)?;
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
            tx.prepare("SELECT title,lyrics,metadata FROM song_search")?;
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
        let v = advance(&tx, "songs", previous)?;
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
        index_song(&tx, v.id, &song)?;
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
        let mut statement = self.db.prepare("SELECT r.payload,f.title,f.lyrics,f.metadata FROM search_rows m JOIN songs s ON s.id=m.song JOIN song_revisions r ON r.id=s.id AND r.revision=s.head JOIN song_search f ON f.rowid=m.row")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let indexed: [String; 3] = [row.get(1)?, row.get(2)?, row.get(3)?];
            if search_fields(&Song::decode(&row.get::<_, Vec<u8>>(0)?)?) != indexed {
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
fn read_id(bytes: Vec<u8>) -> Result<Id> {
    Ok(Id(bytes.try_into().map_err(|_| Error::Corrupt)?))
}
fn quick_check(db: &Connection) -> Result<bool> {
    let check: String = db.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    Ok(check == "ok")
}
/// The indexed columns of a song: title, lyrics, metadata. Labels are not indexed.
fn search_fields(song: &Song) -> [String; 3] {
    let lyrics = song
        .sections
        .iter()
        .map(|s| s.lyrics.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let metadata = [&song.authors, &song.copyright, &song.license]
        .map(String::as_str)
        .join("\n");
    [song.title.clone(), lyrics, metadata]
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
        Some(row) => {
            db.execute("DELETE FROM song_search WHERE rowid=?", params![row])?;
            row
        }
        None => {
            db.execute(
                "INSERT INTO search_rows(song) VALUES(?)",
                params![&id.0[..]],
            )?;
            db.last_insert_rowid()
        }
    };
    let [title, lyrics, metadata] = search_fields(song);
    db.execute(
        "INSERT INTO song_search(rowid,title,lyrics,metadata) VALUES(?,?,?,?)",
        params![row, title, lyrics, metadata],
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
    let mut statement = db.prepare("SELECT s.id,r.payload FROM songs s JOIN song_revisions r ON r.id=s.id AND r.revision=s.head WHERE s.deleted=0 ORDER BY s.id")?;
    let mut rows = statement.query([])?;
    let mut songs = 0;
    while let Some(row) = rows.next()? {
        let id = read_id(row.get(0)?)?;
        index_song(db, id, &Song::decode(&row.get::<_, Vec<u8>>(1)?)?)?;
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
pub enum Command {
    SaveSong(Option<Version>, Song),
    SaveSchedule(Option<Version>, Schedule),
    Song(Version),
    Arrangement(Version),
    Schedule(Version),
    DeleteSong(Version),
    Heads {
        schedules: bool,
        after: Option<Id>,
    },
    Catalog(Option<Id>),
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
}
#[derive(Debug)]
pub enum Reply {
    Opened,
    Saved(Version),
    Song(Song),
    Arrangement(Arrangement),
    Schedule(Schedule, Vec<Song>),
    Deleted,
    Heads(Vec<Version>),
    Catalog(Vec<(Version, String)>),
    ScheduleCatalog(Vec<(Version, String)>),
    Copied,
    Search(search::Results),
    SearchIndex(search::IndexState),
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
        Command::DeleteSong(v) => repo.delete_song(v).map(|()| Reply::Deleted),
        Command::Heads { schedules, after } => repo.heads(schedules, after).map(Reply::Heads),
        Command::Catalog(after) => repo.catalog(after).map(Reply::Catalog),
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
            5
        );
        // One verified backup of the original schema, not one per step.
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
        repo.db.execute_batch("PRAGMA user_version=6").unwrap();
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
        for sql in ["PRAGMA user_version=6", "PRAGMA application_id=42"] {
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
        assert_eq!(schema_of(&path), 5);
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
        assert_eq!(schema_of(&path), 5);
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
        assert_eq!(schema_of(&path), 5);
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
}
