//! Installed translations: one SQLite file per translation (M4-03).
//!
//! `<profile>/Scripture/<ID>.selabible` holds the verses keyed by
//! `VerseId::key`, the metadata, and an FTS5 index for keyword search. A file
//! is written whole to a temporary name and renamed into place, so a reader
//! never sees half a translation. `open` checks the file before anything is
//! read from it: application id and format version, SQLite `quick_check`,
//! the stored verse count and a SHA-256 of every verse, and the FTS index
//! against the verses (a damaged index alone is rebuilt in place). All I/O is
//! blocking; the scripture `Worker` runs it off the UI thread.
use super::model::{Inventory, MAX_META, TranslationInfo, VerseId, valid_id};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub const EXTENSION: &str = "selabible";
/// Format of the file; a newer one is refused as `Unsupported`.
pub const FORMAT_VERSION: i64 = 1;
/// `PRAGMA application_id`: "SBIB".
const APPLICATION_ID: i64 = 0x5342_4942;
/// Most verses one passage request returns (Psalm 119 has 176; a long
/// cross-chapter range stays well below this).
pub const MAX_PASSAGE: usize = 2000;
/// Longest stored verse.
pub const MAX_VERSE_BYTES: usize = super::import::MAX_VERSE_BYTES;

const SCHEMA: &str = "
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE verses(id INTEGER PRIMARY KEY, text TEXT NOT NULL);
";
const INDEX: &str = "CREATE VIRTUAL TABLE verse_index USING fts5(text, content='verses', \
                     content_rowid='id', tokenize='unicode61 remove_diacritics 2');";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NotFound,
    Io(String),
    /// Not a Sela Bible file, or one written by a newer Sela.
    Unsupported,
    /// The file fails a check; the string names which.
    Corrupt(String),
    /// Refused input (bad id, unordered or oversized verses).
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotFound => write!(f, "not found"),
            Error::Io(why) => write!(f, "I/O error: {why}"),
            Error::Unsupported => write!(f, "not a supported Bible file"),
            Error::Corrupt(what) => write!(f, "damaged ({what})"),
            Error::Invalid(what) => write!(f, "invalid ({what})"),
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        match error {
            rusqlite::Error::SqliteFailure(code, _)
                if matches!(
                    code.code,
                    rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt
                ) =>
            {
                Error::Corrupt("database".into())
            }
            other => Error::Io(other.to_string()),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// `<dir>/<id>.selabible`.
pub fn path_for(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.{EXTENSION}"))
}

/// SHA-256 over every verse in canonical order: key, text length, text.
pub fn content_hash<'a>(verses: impl IntoIterator<Item = (VerseId, &'a str)>) -> String {
    let mut hash = Sha256::new();
    for (id, text) in verses {
        hash.update(id.key().to_le_bytes());
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    }
    hex(&hash.finalize())
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn connect(path: &Path, flags: OpenFlags) -> Result<Connection> {
    use rusqlite::limits::Limit;
    let db = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    db.busy_timeout(Duration::from_millis(100))?;
    // Verses and metadata stay below 4 KiB; FTS5 segment blobs may be larger.
    db.set_limit(Limit::SQLITE_LIMIT_LENGTH, 1 << 20)?;
    db.execute_batch("PRAGMA cache_size=-4096;")?;
    Ok(db)
}

/// Writes a complete translation file at `path`, replacing any older one.
/// `origin` identifies the source bytes (for a bundle, its SHA-256), so a
/// changed bundle is noticed and rebuilt.
pub fn write(
    path: &Path,
    info: &TranslationInfo,
    origin: &str,
    verses: &[(VerseId, String)],
) -> Result<()> {
    if !valid_id(&info.id) {
        return Err(Error::Invalid("translation id".into()));
    }
    if verses.is_empty() {
        return Err(Error::Invalid("no verses".into()));
    }
    if verses.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(Error::Invalid("verse order".into()));
    }
    if verses
        .iter()
        .any(|(_, text)| text.is_empty() || text.len() > MAX_VERSE_BYTES)
    {
        return Err(Error::Invalid("verse text".into()));
    }
    for field in [&info.name, &info.language, &info.rights, &info.source] {
        if field.len() > MAX_META {
            return Err(Error::Invalid("metadata length".into()));
        }
    }
    let mut partial = path.as_os_str().to_owned();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    let _ = fs::remove_file(&partial);
    let written = (|| {
        let mut db = connect(
            &partial,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;")?;
        let tx = db.transaction()?;
        tx.execute_batch(SCHEMA)?;
        tx.execute_batch(INDEX)?;
        {
            let mut insert = tx.prepare("INSERT INTO verses(id,text) VALUES(?,?)")?;
            for (id, text) in verses {
                insert.execute(params![id.key(), text])?;
            }
        }
        tx.execute("INSERT INTO verse_index(verse_index) VALUES('rebuild')", [])?;
        let hash = content_hash(verses.iter().map(|(id, text)| (*id, text.as_str())));
        let flag = |b: bool| if b { "1" } else { "0" };
        let count = verses.len().to_string();
        let rows: [(&str, &str); 10] = [
            ("id", &info.id),
            ("name", &info.name),
            ("language", &info.language),
            ("rights", &info.rights),
            ("source", &info.source),
            ("redistributable", flag(info.redistributable)),
            ("bundled", flag(info.bundled)),
            ("verses", &count),
            ("content", &hash),
            ("origin", origin),
        ];
        for (key, value) in rows {
            tx.execute(
                "INSERT INTO meta(key,value) VALUES(?,?)",
                params![key, value],
            )?;
        }
        tx.execute_batch(&format!(
            "PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version={FORMAT_VERSION};"
        ))?;
        tx.commit()?;
        db.close().map_err(|(_, error)| Error::from(error))?;
        Ok::<(), Error>(())
    })();
    if let Err(error) = written {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    fs::File::open(&partial)
        .and_then(|file| file.sync_all())
        .map_err(|e| Error::Io(e.to_string()))?;
    fs::rename(&partial, path).map_err(|e| {
        let _ = fs::remove_file(&partial);
        Error::Io(e.to_string())
    })
}

/// An installed translation whose file passed every check.
pub struct Translation {
    pub info: TranslationInfo,
    pub inventory: Inventory,
    /// What the file was built from (`write`'s `origin`).
    pub origin: String,
    pub verse_count: usize,
    /// The FTS index was damaged and has been rebuilt by this `open`.
    pub index_rebuilt: bool,
    db: Connection,
}

fn quick_check(db: &Connection) -> Result<bool> {
    let result: String = db.query_row("PRAGMA quick_check(1)", [], |r| r.get(0))?;
    Ok(result == "ok")
}

fn index_consistent(db: &Connection) -> bool {
    db.execute(
        "INSERT INTO verse_index(verse_index, rank) VALUES('integrity-check', 1)",
        [],
    )
    .is_ok()
}

fn rebuild_index(db: &Connection) -> Result<()> {
    db.execute_batch("DROP TABLE IF EXISTS verse_index;")?;
    db.execute_batch(INDEX)?;
    db.execute("INSERT INTO verse_index(verse_index) VALUES('rebuild')", [])?;
    Ok(())
}

/// Opens and verifies a translation file.
pub fn open(path: &Path) -> Result<Translation> {
    if !path.is_file() {
        return Err(Error::NotFound);
    }
    let db = connect(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let app: i64 = db.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if app != APPLICATION_ID || version != FORMAT_VERSION {
        return Err(if app == APPLICATION_ID {
            Error::Unsupported
        } else {
            Error::Corrupt("not a Sela Bible file".into())
        });
    }
    let mut index_rebuilt = false;
    if !quick_check(&db)? {
        // Only the derived index may be dropped and rebuilt; damage anywhere
        // else leaves the file refused.
        db.execute_batch("DROP TABLE IF EXISTS verse_index;")
            .map_err(|_| Error::Corrupt("structure".into()))?;
        if !quick_check(&db)? {
            return Err(Error::Corrupt("structure".into()));
        }
        rebuild_index(&db)?;
        index_rebuilt = true;
    }
    let meta = |key: &str| -> Result<String> {
        db.query_row("SELECT value FROM meta WHERE key=?", [key], |r| r.get(0))
            .optional()?
            .ok_or_else(|| Error::Corrupt(format!("metadata {key}")))
    };
    let info = TranslationInfo {
        id: meta("id")?,
        name: meta("name")?,
        language: meta("language")?,
        rights: meta("rights")?,
        source: meta("source")?,
        redistributable: meta("redistributable")? == "1",
        bundled: meta("bundled")? == "1",
    };
    if !valid_id(&info.id) {
        return Err(Error::Corrupt("metadata id".into()));
    }
    let expected: usize = meta("verses")?
        .parse()
        .map_err(|_| Error::Corrupt("metadata verses".into()))?;
    let content = meta("content")?;
    let origin = meta("origin")?;
    let mut inventory = Inventory::default();
    let mut hash = Sha256::new();
    let mut count = 0usize;
    {
        let mut statement = db.prepare("SELECT id,text FROM verses ORDER BY id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let key: i64 = row.get(0)?;
            let text: String = row.get(1)?;
            let id = VerseId::from_key(key).ok_or_else(|| Error::Corrupt("verse key".into()))?;
            if text.is_empty() || text.len() > MAX_VERSE_BYTES {
                return Err(Error::Corrupt("verse text".into()));
            }
            hash.update(key.to_le_bytes());
            hash.update((text.len() as u64).to_le_bytes());
            hash.update(text.as_bytes());
            inventory.add(id);
            count += 1;
        }
    }
    if count != expected || count == 0 {
        return Err(Error::Corrupt("verse count".into()));
    }
    if hex(&hash.finalize()) != content {
        return Err(Error::Corrupt("verse text".into()));
    }
    if !index_rebuilt && !index_consistent(&db) {
        rebuild_index(&db)?;
        index_rebuilt = true;
    }
    Ok(Translation {
        info,
        inventory,
        origin,
        verse_count: count,
        index_rebuilt,
        db,
    })
}

impl Translation {
    /// Verses from `from` to `to` inclusive, in order. At most
    /// `MAX_PASSAGE + 1` are returned so a caller can tell a passage is too
    /// long without reading all of it.
    pub fn passage(&self, from: VerseId, to: VerseId) -> Result<Vec<(VerseId, String)>> {
        let mut statement = self.db.prepare_cached(
            "SELECT id,text FROM verses WHERE id BETWEEN ? AND ? ORDER BY id LIMIT ?",
        )?;
        let rows = statement.query_map(
            params![from.key(), to.key(), (MAX_PASSAGE + 1) as i64],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )?;
        let mut verses = Vec::new();
        for row in rows {
            let (key, text) = row?;
            let id = VerseId::from_key(key).ok_or_else(|| Error::Corrupt("verse key".into()))?;
            verses.push((id, text));
        }
        Ok(verses)
    }

    /// Keyword search: every term must match (the last one as a prefix),
    /// hits in canonical order, at most `limit`; the flag reports more.
    pub fn search(&self, query: &str, limit: usize) -> Result<(Vec<(VerseId, String)>, bool)> {
        if query.len() > crate::search::MAX_QUERY_BYTES {
            return Err(Error::Invalid("query length".into()));
        }
        let Some(expression) = crate::search::expression(query) else {
            return Ok((Vec::new(), false));
        };
        let mut statement = self.db.prepare_cached(
            "SELECT v.id, v.text FROM verse_index JOIN verses v ON v.id = verse_index.rowid \
             WHERE verse_index MATCH ? ORDER BY verse_index.rowid LIMIT ?",
        )?;
        let rows = statement.query_map(params![expression, (limit + 1) as i64], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut hits = Vec::new();
        for row in rows {
            let (key, text) = row?;
            let id = VerseId::from_key(key).ok_or_else(|| Error::Corrupt("verse key".into()))?;
            hits.push((id, text));
        }
        let more = hits.len() > limit;
        hits.truncate(limit);
        Ok((hits, more))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::scripture::Book;

    pub(crate) fn info(id: &str) -> TranslationInfo {
        TranslationInfo {
            id: id.into(),
            name: format!("Synthetic {id}"),
            language: "en".into(),
            rights: "Synthetic test text".into(),
            source: "test".into(),
            redistributable: true,
            bundled: false,
        }
    }

    pub(crate) fn v(book: u8, chapter: u16, verse: u16) -> VerseId {
        VerseId::new(Book::from_number(book).unwrap(), chapter, verse).unwrap()
    }

    fn corpus() -> Vec<(VerseId, String)> {
        vec![
            (v(43, 3, 16), "Alpha light shines".into()),
            (v(43, 3, 17), "Beta river flows".into()),
            (v(43, 3, 18), "Gamma light rests".into()),
            (v(43, 4, 1), "Delta stone stands".into()),
            (v(64, 1, 15), "Épsilon café closing".into()),
        ]
    }

    #[test]
    fn a_written_translation_opens_with_its_inventory_and_reads_passages() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_for(dir.path(), "SYN");
        write(&path, &info("SYN"), "origin-1", &corpus()).unwrap();
        let translation = open(&path).unwrap();
        assert_eq!(translation.info, info("SYN"));
        assert_eq!(translation.origin, "origin-1");
        assert_eq!(translation.verse_count, 5);
        assert!(!translation.index_rebuilt);
        let john = Book::from_number(43).unwrap();
        assert_eq!(translation.inventory.last_verse(john, 3), Some(18));
        assert_eq!(translation.inventory.last_verse(john, 4), Some(1));
        assert!(
            !translation
                .inventory
                .has_book(Book::from_number(1).unwrap())
        );
        let cross = translation.passage(v(43, 3, 17), v(43, 4, 1)).unwrap();
        assert_eq!(
            cross.iter().map(|(id, _)| id.key()).collect::<Vec<_>>(),
            [43_003_017, 43_003_018, 43_004_001]
        );
        assert!(!dir.path().join("SYN.selabible.partial").exists());
    }

    #[test]
    fn keyword_search_is_canonical_prefix_and_diacritic_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_for(dir.path(), "SYN");
        write(&path, &info("SYN"), "o", &corpus()).unwrap();
        let translation = open(&path).unwrap();
        let keys = |query: &str, limit| {
            let (hits, more) = translation.search(query, limit).unwrap();
            (
                hits.iter().map(|(id, _)| id.key()).collect::<Vec<_>>(),
                more,
            )
        };
        assert_eq!(keys("light", 10), (vec![43_003_016, 43_003_018], false));
        assert_eq!(keys("li", 10), (vec![43_003_016, 43_003_018], false));
        assert_eq!(keys("light", 1), (vec![43_003_016], true));
        assert_eq!(keys("cafe", 10), (vec![64_001_015], false));
        assert_eq!(keys("epsilon", 10), (vec![64_001_015], false));
        assert_eq!(keys("light gamma", 10), (vec![43_003_018], false));
        assert_eq!(keys("nothing", 10), (vec![], false));
        assert_eq!(keys("  ", 10), (vec![], false), "empty search");
        assert_eq!(keys("\"NEAR(*", 10), (vec![], false));
    }

    #[test]
    fn bad_input_is_refused_before_anything_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_for(dir.path(), "SYN");
        let mut unordered = corpus();
        unordered.swap(0, 1);
        assert!(matches!(
            write(&path, &info("SYN"), "o", &unordered),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            write(&path, &info("../x"), "o", &corpus()),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            write(&path, &info("SYN"), "o", &[]),
            Err(Error::Invalid(_))
        ));
        assert!(!path.exists());
    }

    fn damage(path: &Path, sql: &str) {
        let db = Connection::open(path).unwrap();
        db.execute_batch(sql).unwrap();
    }

    #[test]
    fn corrupt_files_are_refused_and_a_damaged_index_is_rebuilt() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_for(dir.path(), "SYN");
        assert_eq!(open(&path).err(), Some(Error::NotFound));

        write(&path, &info("SYN"), "o", &corpus()).unwrap();
        damage(&path, "UPDATE verses SET text='Tampered' WHERE id=43003016");
        assert_eq!(open(&path).err(), Some(Error::Corrupt("verse text".into())));

        write(&path, &info("SYN"), "o", &corpus()).unwrap();
        damage(&path, "DELETE FROM verses WHERE id=43004001");
        assert_eq!(
            open(&path).err(),
            Some(Error::Corrupt("verse count".into()))
        );

        write(&path, &info("SYN"), "o", &corpus()).unwrap();
        damage(&path, "DELETE FROM meta WHERE key='content'");
        assert_eq!(
            open(&path).err(),
            Some(Error::Corrupt("metadata content".into()))
        );

        write(&path, &info("SYN"), "o", &corpus()).unwrap();
        damage(&path, "PRAGMA user_version=2");
        assert_eq!(open(&path).err(), Some(Error::Unsupported));

        // The index alone out of step with the verses: rebuilt, searchable.
        write(&path, &info("SYN"), "o", &corpus()).unwrap();
        damage(
            &path,
            "INSERT INTO verse_index(verse_index, rowid, text) \
             VALUES('delete', 43003016, 'Alpha light shines')",
        );
        let translation = open(&path).unwrap();
        assert!(translation.index_rebuilt);
        assert_eq!(translation.search("alpha", 5).unwrap().0.len(), 1);

        // Random bytes and a foreign SQLite file.
        fs::write(&path, b"this is not a database at all, not even close....").unwrap();
        assert!(matches!(open(&path), Err(Error::Corrupt(_))));
        fs::remove_file(&path).unwrap();
        damage(&path, "CREATE TABLE other(x)");
        assert_eq!(
            open(&path).err(),
            Some(Error::Corrupt("not a Sela Bible file".into()))
        );

        // A truncated file.
        write(&path, &info("SYN"), "o", &corpus()).unwrap();
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        assert!(matches!(
            open(&path),
            Err(Error::Corrupt(_)) | Err(Error::Io(_))
        ));
    }

    #[test]
    fn rewriting_replaces_the_file_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_for(dir.path(), "SYN");
        write(&path, &info("SYN"), "one", &corpus()).unwrap();
        let mut shorter = corpus();
        shorter.truncate(2);
        write(&path, &info("SYN"), "two", &shorter).unwrap();
        let translation = open(&path).unwrap();
        assert_eq!(
            (translation.origin.as_str(), translation.verse_count),
            ("two", 2)
        );
    }
}
