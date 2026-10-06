//! Durable song history and ordered schedule snapshots. Use `Worker` on UI threads.
use crate::arrangement::{
    Arrangement, Occurrence, OccurrenceId, SectionId, SourceSnapshot, Variant, VariantId,
};
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
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    pub title: String,
    pub authors: String,
    pub copyright: String,
    pub license: String,
    pub sections: Vec<Section>,
    pub variants: Vec<Variant>,
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
            sections: {
                let mut v = Vec::new();
                while let Some(label) = it.next() {
                    v.push(Section {
                        id: SectionId(Id([0; 16])), // unbound legacy text, never identity
                        label,
                        lyrics: it.next().unwrap(),
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
    if app != APP || ![1, 2].contains(&version) {
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
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
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
        } else if app != APP || ![1, 2].contains(&version) {
            return Err(Error::Unsupported);
        }
        let check: String = tx.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if check != "ok" {
            return Err(Error::Corrupt);
        }
        if tx.prepare("PRAGMA foreign_key_check")?.exists([])? {
            return Err(Error::Corrupt);
        }
        // Verify required columns before accepting an otherwise foreign schema.
        tx.prepare("SELECT s.deleted,r.payload FROM songs s JOIN song_revisions r ON s.id=r.id")?;
        tx.prepare("SELECT r.title,i.position,i.song_revision FROM schedule_revisions r JOIN items i ON r.id=i.id AND r.revision=i.revision JOIN schedules s ON s.id=r.id")?;
        if version == 1 && upgrade {
            // Hold the writer reservation across backup and migration. The separate
            // read connection copies the same committed state, without attempting
            // an online backup from a connection with an active write transaction.
            let source = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            source.busy_timeout(Duration::from_millis(100))?;
            let mut backup = path.as_os_str().to_owned();
            backup.push(".schema1-backup");
            copy_new(
                &source,
                std::path::Path::new(&backup),
                &AtomicBool::new(false),
            )?;
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
        if version == 2 || upgrade {
            tx.prepare("SELECT section FROM section_ids")?;
            tx.prepare("SELECT variant,name FROM variants")?;
            tx.prepare("SELECT occurrence,section FROM occurrences")?;
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
        tx.commit()?;
        Ok(v)
    }
    pub fn delete_song(&mut self, v: Version) -> Result<()> {
        if self.db.execute(
            "UPDATE songs SET deleted=1 WHERE id=? AND head=? AND deleted=0",
            params![&v.id.0[..], v.revision],
        )? != 1
        {
            return Err(Error::Conflict);
        }
        Ok(())
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
    BackupNew(PathBuf),
    RestoreNew {
        source: PathBuf,
        destination: PathBuf,
    },
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
    Copied,
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
        Command::BackupNew(path) => repo
            .backup_new(&path, &request.canceled)
            .map(|()| Reply::Copied),
        Command::RestoreNew {
            source,
            destination,
        } => Repository::restore_new(&source, &destination, &request.canceled)
            .map(|()| Reply::Copied),
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
        repo.db.execute_batch("PRAGMA user_version=3").unwrap();
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
                },
                Section {
                    id: SectionId(Id([2; 16])),
                    label: "".into(),
                    lyrics: "".into(),
                },
            ],
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
        for sql in ["PRAGMA user_version=3", "PRAGMA application_id=42"] {
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
}
