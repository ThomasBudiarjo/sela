//! Durable song history and ordered schedule snapshots. Use `Worker` on UI threads.
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
    pub fn validate(&self) -> Result<()> {
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
            sections: {
                let mut v = Vec::new();
                while let Some(label) = it.next() {
                    v.push(Section {
                        label,
                        lyrics: it.next().unwrap(),
                    });
                }
                v
            },
        };
        song.validate().map_err(|_| Error::Corrupt)?;
        Ok(song)
    }
}
const SCHEMA: &str = "CREATE TABLE songs(id BLOB PRIMARY KEY CHECK(length(id)=16), head INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE song_revisions(id BLOB NOT NULL REFERENCES songs(id), revision INTEGER NOT NULL, payload BLOB NOT NULL CHECK(length(payload)<=262144), PRIMARY KEY(id,revision));
CREATE TABLE schedules(id BLOB PRIMARY KEY CHECK(length(id)=16), head INTEGER NOT NULL);
CREATE TABLE schedule_revisions(id BLOB NOT NULL REFERENCES schedules(id), revision INTEGER NOT NULL, title TEXT NOT NULL, PRIMARY KEY(id,revision));
CREATE TABLE items(id BLOB NOT NULL, revision INTEGER NOT NULL, position INTEGER NOT NULL, song BLOB NOT NULL, song_revision INTEGER NOT NULL, PRIMARY KEY(id,revision,position), FOREIGN KEY(id,revision) REFERENCES schedule_revisions(id,revision), FOREIGN KEY(song,song_revision) REFERENCES song_revisions(id,revision));
PRAGMA application_id=1397050433; PRAGMA user_version=1;";
/// Synchronous API: exclusively for background threads; all methods may perform I/O.
pub struct Repository {
    db: Connection,
}
impl Repository {
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
    pub fn open(path: &std::path::Path) -> Result<Self> {
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
        } else if app != APP || version != 1 {
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
        tx.commit()?;
        Ok(Self { db })
    }
    pub fn song(&self, v: Version) -> Result<Song> {
        let b: Vec<u8> = self
            .db
            .query_row(
                "SELECT payload FROM song_revisions WHERE id=? AND revision=?",
                params![&v.id.0[..], v.revision],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(Error::Missing)?;
        Song::decode(&b)
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
    Schedule(Version),
    DeleteSong(Version),
    Heads { schedules: bool, after: Option<Id> },
}
#[derive(Debug)]
pub enum Reply {
    Opened,
    Saved(Version),
    Song(Song),
    Schedule(Schedule, Vec<Song>),
    Deleted,
    Heads(Vec<Version>),
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
        Command::Schedule(v) => repo.schedule(v).map(|(s, songs)| Reply::Schedule(s, songs)),
        Command::DeleteSong(v) => repo.delete_song(v).map(|()| Reply::Deleted),
        Command::Heads { schedules, after } => repo.heads(schedules, after).map(Reply::Heads),
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
            sections: vec![
                Section {
                    label: "Verse".into(),
                    lyrics: "one\r\n\n二\n".into(),
                },
                Section {
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
        for sql in ["PRAGMA user_version=2", "PRAGMA application_id=42"] {
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
