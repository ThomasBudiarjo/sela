//! EasyWorship 6-8 song library (M4-07): the operator chooses the folder that
//! holds `Songs.db` and `SongWords.db` (or one above it); both databases are
//! copied into a private temporary folder and only the copies are opened, so
//! the user's library is never handed to SQLite and never written. Each
//! `song` row's metadata is mapped onto `storage::Song`, its `word` row is
//! parsed by `import::ew_rtf`, and the result is a `batch::Staged` row for
//! the same staged preview, duplicate policy and commit transaction as
//! every other importer. Runs on the `sela-import` thread only.
//!
//! Every schema detail (table and column names, `song_uid`, the
//! `UTF8_U_CI` collation's exact rules, where the folder lives) is an
//! UNOBSERVED best guess from public sources; docs/import.md, "EasyWorship
//! library", lists them and docs/parity-pending.md (Stream:
//! ew-library-import) the checks against a real EW 8.0.49 library copy.
use super::batch::{self, FileError, Format, Staged};
use super::{MAX_FILE_BYTES, ParsedSong, Rejected, Warning, ew_rtf, song};
use crate::storage::{MAX_ADMINISTRATOR, MAX_SONG_NUMBER};
use rusqlite::{Connection, OpenFlags, limits::Limit, types::ValueRef};
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};
use unicode_segmentation::UnicodeSegmentation;

pub const SONGS_DB: &str = "Songs.db";
pub const WORDS_DB: &str = "SongWords.db";
/// Folders below the chosen one searched for the two databases, in order.
/// EW 6 and 7 keep them in `...\EasyWorship\Default\v6.1\Databases\Data`
/// (public sources; UNOBSERVED for EW 8.0.49), so the profile folder, its
/// `v6.1` folder, `Databases` or `Data` itself may be chosen.
pub const DATA_FOLDERS: [&str; 5] = [
    "",
    "Data",
    "Databases/Data",
    "v6.1/Databases/Data",
    "Default/v6.1/Databases/Data",
];
/// Largest database (and each journal beside it) copied.
pub const MAX_DB_BYTES: u64 = 256 * 1024 * 1024;
/// The collation EW's schema declares on text columns.
pub const COLLATION: &str = "UTF8_U_CI";
/// Largest value SQLite hands back from a copy: room for words just over
/// `MAX_FILE_BYTES`, so the parser reports `TooLarge` itself.
const MAX_VALUE_BYTES: i32 = 2 * MAX_FILE_BYTES as i32;
/// Journals copied with a database when present: a `-wal` holds rows EW
/// committed but did not checkpoint yet, and a hot `-journal` rolls back an
/// interrupted write in the copy.
const SIDECARS: [&str; 2] = ["-wal", "-journal"];
/// Entries looked at per folder for a case-insensitive name match.
const MAX_LISTING: usize = 4096;
/// Copy buffer; the cancel flag is checked between chunks.
const CHUNK: usize = 1024 * 1024;
/// Longest title, field limits as `Song::validate` checks them.
const MAX_TITLE: usize = 1024;
const MAX_TEXT_FIELD: usize = 4096;
/// Title of a song row without one.
pub const UNTITLED: &str = "Untitled";

/// Why a library produced no songs at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryError {
    /// No `Songs.db` in the folder or the `DATA_FOLDERS` below it.
    NoSongsDb,
    /// `Songs.db` without a `SongWords.db` beside it.
    NoWordsDb,
    /// A database or journal is over `MAX_DB_BYTES`.
    TooLarge,
    /// A source file changed while it was copied (EasyWorship is saving).
    Changed,
    /// The operating system's message for a file that could not be read.
    Unreadable(String),
    /// Not a database, or not the tables and columns of an EW library; the
    /// string names the problem.
    NotLibrary(String),
    /// The import was cancelled while the library was copied.
    Canceled,
}

/// The optional `song` columns read when the copy has them.
const OPTIONAL: [&str; 7] = [
    "song_uid",
    "author",
    "copyright",
    "administrator",
    "vendor_id",
    "description",
    "tags",
];

/// One `song` row with its words, as read from the copy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EwSong {
    pub rowid: i64,
    pub uid: Option<String>,
    pub title: String,
    pub author: String,
    pub copyright: String,
    pub administrator: String,
    pub vendor_id: String,
    pub description: String,
    pub tags: String,
    /// The newest `word.words` for this song; `None` when it has no row or
    /// the value is NULL.
    pub words: Option<Vec<u8>>,
    /// More `word` rows than the one used.
    pub extra_word_rows: usize,
    /// Columns whose text was not valid UTF-8 (replaced characters).
    pub invalid_text: Vec<&'static str>,
    /// The words were over the value limit and not read.
    pub words_too_large: bool,
}

/// Private copies of one library, open for reading. Dropping it deletes
/// the copies.
pub struct Library {
    songs: Connection,
    words: Connection,
    /// The source data folder (for row names and the preview).
    source: PathBuf,
    /// `OPTIONAL` columns present in `song`.
    present: Vec<&'static str>,
    // Last, so the connections close before the folder is removed.
    _copy: tempfile::TempDir,
}

/// The data folder under `folder` that holds both databases.
pub fn locate(folder: &Path) -> Result<(PathBuf, PathBuf), LibraryError> {
    for sub in DATA_FOLDERS {
        let dir = if sub.is_empty() {
            folder.to_path_buf()
        } else {
            folder.join(sub)
        };
        let Some(songs) = find_file(&dir, SONGS_DB) else {
            continue;
        };
        let words = find_file(&dir, WORDS_DB).ok_or(LibraryError::NoWordsDb)?;
        return Ok((songs, words));
    }
    Err(LibraryError::NoSongsDb)
}

/// `dir/name` as a regular file, matching the name without regard to ASCII
/// case (a library copied from Windows onto another file system).
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if std::fs::metadata(&exact).is_ok_and(|m| m.is_file()) {
        return Some(exact);
    }
    std::fs::read_dir(dir)
        .ok()?
        .take(MAX_LISTING)
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
                && entry.file_type().is_ok_and(|t| t.is_file())
        })
        .map(|entry| entry.path())
}

/// Size and modification time, compared before and after a copy.
fn stamp(path: &Path) -> Result<Option<(u64, Option<SystemTime>)>, LibraryError> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => Ok(Some((meta.len(), meta.modified().ok()))),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(LibraryError::Unreadable(error.to_string())),
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Copies `source` (and its journals) to `dir/name`, reading the source
/// only. `between` runs after the bytes are copied and before the source is
/// checked again (a test hook).
fn copy_db(
    source: &Path,
    dir: &Path,
    name: &str,
    cancel: &AtomicBool,
    between: &dyn Fn(),
) -> Result<PathBuf, LibraryError> {
    let target = dir.join(name);
    let mut files = vec![(source.to_path_buf(), target.clone())];
    for suffix in SIDECARS {
        let journal = with_suffix(source, suffix);
        if stamp(&journal)?.is_some() {
            files.push((journal, with_suffix(&target, suffix)));
        }
    }
    let mut before = Vec::new();
    for (from, to) in &files {
        let stamped = stamp(from)?;
        before.push(stamped);
        copy_file(from, to, cancel)?;
    }
    between();
    for ((from, _), stamped) in files.iter().zip(before) {
        if stamp(from)? != stamped {
            return Err(LibraryError::Changed);
        }
    }
    // A journal that appeared during the copy belongs to a write in progress.
    for suffix in SIDECARS {
        let journal = with_suffix(source, suffix);
        if !files.iter().any(|(from, _)| *from == journal) && stamp(&journal)?.is_some() {
            return Err(LibraryError::Changed);
        }
    }
    Ok(target)
}

fn copy_file(from: &Path, to: &Path, cancel: &AtomicBool) -> Result<(), LibraryError> {
    let unreadable = |e: std::io::Error| LibraryError::Unreadable(e.to_string());
    let (file, meta) = batch::open_regular(from).map_err(|error| match error {
        FileError::Unreadable(message) => LibraryError::Unreadable(message),
        _ => LibraryError::NotLibrary(format!("{} is not a regular file", from.display())),
    })?;
    if meta.len() > MAX_DB_BYTES {
        return Err(LibraryError::TooLarge);
    }
    let mut reader = file.take(MAX_DB_BYTES + 1);
    let mut out = std::fs::File::create(to).map_err(unreadable)?;
    let mut buffer = vec![0u8; CHUNK];
    let mut total = 0u64;
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(LibraryError::Canceled);
        }
        let read = reader.read(&mut buffer).map_err(unreadable)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > MAX_DB_BYTES {
            return Err(LibraryError::TooLarge);
        }
        out.write_all(&buffer[..read]).map_err(unreadable)?;
    }
    out.flush().map_err(unreadable)
}

/// The `UTF8_U_CI` collation as Sela defines it: Unicode lower case, then
/// code point order. EW's exact rules are UNOBSERVED (PP-EWL-03); only
/// sorting depends on them.
pub fn utf8_u_ci(a: &str, b: &str) -> std::cmp::Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

fn not_library(error: rusqlite::Error) -> LibraryError {
    LibraryError::NotLibrary(error.to_string())
}

/// Opens a private copy. Read-write only so SQLite can replay a copied
/// `-wal` or roll back a copied hot journal in the copy; `query_only` is
/// set before anything else runs, so no statement can write.
fn connect(path: &Path) -> Result<Connection, LibraryError> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(not_library)?;
    db.set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_VALUE_BYTES)
        .map_err(not_library)?;
    // The file is untrusted: its schema may not run functions with side
    // effects.
    db.pragma_update(None, "trusted_schema", false)
        .map_err(not_library)?;
    db.create_collation(COLLATION, utf8_u_ci)
        .map_err(not_library)?;
    // Reads the header now, so a file that is not a database says so here,
    // and replays a copied journal before writes are switched off.
    db.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .map_err(not_library)?;
    db.pragma_update(None, "query_only", true)
        .map_err(not_library)?;
    Ok(db)
}

/// Lower-case column names of `table`, empty when it does not exist.
fn columns(db: &Connection, table: &str) -> Result<Vec<String>, LibraryError> {
    let mut statement = db
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(not_library)?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(not_library)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(not_library)?;
    Ok(names.into_iter().map(|n| n.to_ascii_lowercase()).collect())
}

impl Library {
    /// Finds the databases under `folder`, copies them and opens the copies.
    pub fn open(folder: &Path, cancel: &AtomicBool) -> Result<Self, LibraryError> {
        Self::open_with(folder, cancel, &|| {})
    }

    fn open_with(
        folder: &Path,
        cancel: &AtomicBool,
        between: &dyn Fn(),
    ) -> Result<Self, LibraryError> {
        let (songs_path, words_path) = locate(folder)?;
        let copy = tempfile::Builder::new()
            .prefix("sela-ew-library-")
            .tempdir()
            .map_err(|e| LibraryError::Unreadable(e.to_string()))?;
        let songs_copy = copy_db(&songs_path, copy.path(), SONGS_DB, cancel, between)?;
        let words_copy = copy_db(&words_path, copy.path(), WORDS_DB, cancel, between)?;
        let songs = connect(&songs_copy)?;
        let words = connect(&words_copy)?;
        let song_columns = columns(&songs, "song")?;
        if song_columns.is_empty() {
            return Err(LibraryError::NotLibrary(format!(
                "{SONGS_DB} has no song table"
            )));
        }
        if !song_columns.iter().any(|c| c == "title") {
            return Err(LibraryError::NotLibrary(format!(
                "the song table in {SONGS_DB} has no title column"
            )));
        }
        let word_columns = columns(&words, "word")?;
        for needed in ["song_id", "words"] {
            if !word_columns.iter().any(|c| c == needed) {
                return Err(LibraryError::NotLibrary(format!(
                    "the word table in {WORDS_DB} has no {needed} column"
                )));
            }
        }
        let present = OPTIONAL
            .into_iter()
            .filter(|name| song_columns.iter().any(|c| c == name))
            .collect();
        Ok(Self {
            songs,
            words,
            source: songs_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            present,
            _copy: copy,
        })
    }

    /// The source `Songs.db` (rows are named after it).
    pub fn songs_path(&self) -> PathBuf {
        self.source.join(SONGS_DB)
    }

    /// Path of the private copy of `Songs.db` (tests).
    #[cfg(test)]
    fn copy_dir(&self) -> PathBuf {
        self._copy.path().to_path_buf()
    }

    pub fn count(&self) -> Result<usize, LibraryError> {
        self.songs
            .query_row("SELECT count(*) FROM song", [], |row| row.get::<_, i64>(0))
            .map(|n| n.max(0) as usize)
            .map_err(not_library)
    }

    /// The first `limit` song rowids by title in `UTF8_U_CI` order, then
    /// rowid (EW lists songs by title; its exact order is UNOBSERVED).
    pub fn song_ids(&self, limit: usize) -> Result<Vec<i64>, LibraryError> {
        let mut statement = self
            .songs
            .prepare(&format!(
                "SELECT rowid FROM song ORDER BY title COLLATE {COLLATION}, rowid LIMIT ?1"
            ))
            .map_err(not_library)?;
        statement
            .query_map([limit as i64], |row| row.get(0))
            .map_err(not_library)?
            .collect::<Result<Vec<i64>, _>>()
            .map_err(not_library)
    }

    /// One song row and its newest `word` row.
    pub fn song(&self, rowid: i64) -> Result<EwSong, FileError> {
        let mut select = vec!["title"];
        for name in OPTIONAL {
            select.push(if self.present.contains(&name) {
                name
            } else {
                "NULL"
            });
        }
        let sql = format!("SELECT {} FROM song WHERE rowid = ?1", select.join(", "));
        let mut song = EwSong {
            rowid,
            ..EwSong::default()
        };
        self.songs
            .query_row(&sql, [rowid], |row| {
                let mut field = |index: usize, name: &'static str| -> rusqlite::Result<String> {
                    let (text, valid) = text_of(row.get_ref(index)?);
                    if !valid {
                        song.invalid_text.push(name);
                    }
                    Ok(text)
                };
                let title = field(0, "title")?;
                let uid = field(1, "song_uid")?;
                let author = field(2, "author")?;
                let copyright = field(3, "copyright")?;
                let administrator = field(4, "administrator")?;
                let vendor_id = field(5, "vendor_id")?;
                let description = field(6, "description")?;
                let tags = field(7, "tags")?;
                song.title = title;
                song.uid = (!uid.trim().is_empty()).then(|| uid.trim().to_string());
                song.author = author;
                song.copyright = copyright;
                song.administrator = administrator;
                song.vendor_id = vendor_id;
                song.description = description;
                song.tags = tags;
                Ok(())
            })
            .map_err(row_error)?;
        let rows: i64 = self
            .words
            .query_row(
                "SELECT count(*) FROM word WHERE song_id = ?1",
                [rowid],
                |row| row.get(0),
            )
            .map_err(row_error)?;
        song.extra_word_rows = (rows.max(1) - 1) as usize;
        if rows > 0 {
            let words = self.words.query_row(
                "SELECT words FROM word WHERE song_id = ?1 ORDER BY rowid DESC LIMIT 1",
                [rowid],
                |row| {
                    Ok(match row.get_ref(0)? {
                        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => Some(bytes.to_vec()),
                        ValueRef::Null => None,
                        other => Some(text_of(other).0.into_bytes()),
                    })
                },
            );
            match words {
                Ok(words) => song.words = words,
                Err(error) if too_big(&error) => song.words_too_large = true,
                Err(error) => return Err(row_error(error)),
            }
        }
        Ok(song)
    }
}

fn too_big(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::TooBig
    )
}

/// A row that cannot be read is that song's failure, not the library's.
fn row_error(error: rusqlite::Error) -> FileError {
    if too_big(&error) {
        FileError::Rejected(Some(Format::EasyWorship), Rejected::TooLarge)
    } else {
        FileError::Library(LibraryError::NotLibrary(error.to_string()))
    }
}

/// A column value as text, and whether it was valid UTF-8.
fn text_of(value: ValueRef<'_>) -> (String, bool) {
    match value {
        ValueRef::Null => (String::new(), true),
        ValueRef::Integer(n) => (n.to_string(), true),
        ValueRef::Real(x) => (x.to_string(), true),
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => (text.to_string(), true),
            Err(_) => (String::from_utf8_lossy(bytes).into_owned(), false),
        },
    }
}

/// `text` cut at a grapheme boundary to `max` bytes, with a warning.
fn fit(parsed: &mut ParsedSong, text: &str, max: usize, field: &str) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    parsed.warn(Warning::Truncated(field.to_string()));
    let end = text
        .grapheme_indices(true)
        .map(|(at, g)| at + g.len())
        .take_while(|&end| end <= max)
        .last()
        .unwrap_or(0);
    text[..end].to_string()
}

/// A metadata field on one line, without surrounding space.
fn one_line(text: &str) -> String {
    text.split(['\r', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Maps one row onto the song model (docs/import.md, "EasyWorship
/// library", field table).
pub fn parse(row: &EwSong) -> Result<ParsedSong, FileError> {
    if row.words_too_large {
        return Err(FileError::Rejected(
            Some(Format::EasyWorship),
            Rejected::TooLarge,
        ));
    }
    let Some(words) = &row.words else {
        return Err(FileError::MissingWords);
    };
    let mut parsed =
        ew_rtf::parse(words).map_err(|r| FileError::Rejected(Some(Format::EasyWorship), r))?;
    let lyrics = std::mem::take(&mut parsed.warnings);
    for name in &row.invalid_text {
        parsed.warn(Warning::InvalidField((*name).into()));
    }
    let title = one_line(&row.title);
    parsed.title = fit(&mut parsed, &title, MAX_TITLE, "title");
    let author = one_line(&row.author);
    if !author.is_empty() {
        let author = fit(&mut parsed, &author, MAX_TEXT_FIELD, "author");
        parsed.authors = vec![author];
    }
    let copyright = row.copyright.trim().replace("\r\n", "\n");
    parsed.copyright = fit(&mut parsed, &copyright, MAX_TEXT_FIELD, "copyright");
    let administrator = one_line(&row.administrator);
    parsed.administrator = fit(
        &mut parsed,
        &administrator,
        MAX_ADMINISTRATOR,
        "administrator",
    );
    let number = row.vendor_id.trim();
    if !number.is_empty() {
        if number.len() <= MAX_SONG_NUMBER && number.bytes().all(|b| b.is_ascii_digit()) {
            parsed.song_number = number.to_string();
        } else {
            parsed.warn(Warning::InvalidField("vendor_id".into()));
        }
    }
    // Sela songs have no description or keyword field yet: say what was
    // left behind instead of dropping it silently.
    if !row.description.trim().is_empty() {
        parsed.warn(Warning::IgnoredAttribute("song/description".into()));
    }
    if !row.tags.trim().is_empty() {
        parsed.warn(Warning::IgnoredAttribute("song/tags".into()));
    }
    if row.extra_word_rows > 0 {
        parsed.warn(Warning::IgnoredAttribute("word (older rows)".into()));
    }
    for warning in lyrics {
        parsed.warn(warning);
    }
    if parsed.title.trim().is_empty() {
        parsed.title = UNTITLED.to_string();
        parsed.warnings.insert(0, Warning::MissingTitle);
        parsed.warnings.truncate(super::MAX_WARNINGS);
    }
    Ok(parsed)
}

/// One row staged for the preview, with its `song_uid` as the source id.
pub fn stage(row: &EwSong) -> Result<Staged, FileError> {
    let parsed = parse(row)?;
    let song = song::to_song(&parsed).map_err(|_| FileError::TooLargeForLibrary)?;
    Ok(Staged {
        format: Format::EasyWorship,
        key: batch::duplicate_key(&song),
        bytes: batch::song_bytes(&song),
        song: Arc::new(song),
        warnings: parsed.warnings,
        source_id: row.uid.clone(),
    })
}

/// Name of one song row in the preview: the source file and its rowid.
pub fn row_name(rowid: i64) -> String {
    format!("{SONGS_DB} #{rowid}")
}

#[cfg(test)]
#[path = "ew_fixture.rs"]
pub(crate) mod fixture;

#[cfg(test)]
mod tests {
    use super::fixture::{FixtureSong, ew8_words, hashes, write_databases, write_library};
    use super::*;

    const MORNING: FixtureSong = FixtureSong {
        title: "Morning Over the Harbor",
        author: "Rina Kartika",
        copyright: "2026 Sela Test Fixtures",
        administrator: "Fixture Music Admin",
        vendor_id: "7654321",
        description: "Opening song",
        tags: "morning; harbor",
        song_uid: "{A1}",
        words: None,
    };

    fn words(text: &[(&str, &[&str])]) -> &'static str {
        Box::leak(ew8_words(text).into_boxed_str())
    }

    fn morning() -> FixtureSong {
        FixtureSong {
            words: Some(words(&[
                (
                    "Verse 1",
                    &["Light on the water\nwakes the town", "Boats lean home"],
                ),
                ("Chorus", &["Sing, harbor, sing"]),
            ])),
            ..MORNING
        }
    }

    fn open(root: &Path) -> Library {
        Library::open(root, &AtomicBool::new(false)).unwrap()
    }

    fn staged_all(library: &Library) -> Vec<(i64, Result<Staged, FileError>)> {
        library
            .song_ids(1000)
            .unwrap()
            .into_iter()
            .map(|id| (id, library.song(id).and_then(|row| stage(&row))))
            .collect()
    }

    #[test]
    fn metadata_and_words_map_onto_the_song_model() {
        let dir = tempfile::tempdir().unwrap();
        write_library(dir.path(), &[morning()]);
        let library = open(dir.path());
        assert_eq!(library.count().unwrap(), 1);
        let row = library.song(1).unwrap();
        assert_eq!(row.uid.as_deref(), Some("{A1}"));
        let staged = stage(&row).unwrap();
        assert_eq!(staged.format, Format::EasyWorship);
        assert_eq!(staged.source_id.as_deref(), Some("{A1}"));
        let song = &staged.song;
        assert_eq!(song.title, "Morning Over the Harbor");
        assert_eq!(song.authors, "Rina Kartika");
        assert_eq!(song.copyright, "2026 Sela Test Fixtures");
        assert_eq!(song.administrator, "Fixture Music Admin");
        assert_eq!(song.song_number, "7654321", "vendor_id stored as INTEGER");
        let slides: Vec<(&str, &str)> = song
            .sections
            .iter()
            .map(|s| (s.label.as_str(), s.lyrics.as_str()))
            .collect();
        assert_eq!(
            slides,
            [
                ("Verse 1", "Light on the water\nwakes the town"),
                ("", "Boats lean home"),
                ("Chorus", "Sing, harbor, sing"),
            ]
        );
        assert_eq!(
            staged.warnings,
            [
                Warning::IgnoredAttribute("song/description".into()),
                Warning::IgnoredAttribute("song/tags".into()),
            ]
        );
    }

    #[test]
    fn the_sources_are_never_written_and_the_copies_are_deleted() {
        let dir = tempfile::tempdir().unwrap();
        write_library(dir.path(), &[morning()]);
        let before = hashes(dir.path());
        let copy = {
            let library = open(dir.path());
            assert_eq!(staged_all(&library).len(), 1);
            let copy = library.copy_dir();
            assert!(copy.join(SONGS_DB).is_file());
            assert!(!copy.starts_with(dir.path()));
            copy
        };
        assert!(!copy.exists(), "the private copy is removed on drop");
        assert_eq!(hashes(dir.path()), before, "bytes, names and mtimes");
    }

    #[test]
    fn rows_committed_only_to_the_wal_are_copied_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("Data");
        std::fs::create_dir(&data).unwrap();
        let writers = write_databases(&data, &[morning()], true);
        assert!(data.join("Songs.db-wal").is_file());
        let before = hashes(dir.path());
        let library = open(dir.path());
        let staged = staged_all(&library);
        assert_eq!(staged.len(), 1, "the row lives only in the -wal");
        assert!(staged[0].1.is_ok());
        drop(library);
        assert_eq!(hashes(dir.path()), before);
        drop(writers);
    }

    #[test]
    fn titles_sort_with_the_registered_case_insensitive_collation() {
        let dir = tempfile::tempdir().unwrap();
        let song = |title| FixtureSong {
            title,
            song_uid: "",
            ..morning()
        };
        let data = write_library(
            dir.path(),
            &[song("cedar"), song("Banyan"), song("apple"), song("Apple")],
        );
        let library = open(dir.path());
        assert_eq!(library.song_ids(10).unwrap(), [3, 4, 2, 1]);
        assert_eq!(library.song_ids(2).unwrap(), [3, 4], "bounded");
        // Without the collation SQLite cannot run the query at all.
        let raw =
            Connection::open_with_flags(data.join(SONGS_DB), OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        let error = raw
            .prepare("SELECT rowid FROM song ORDER BY title COLLATE UTF8_U_CI")
            .map(|_| ())
            .expect_err("no such collation");
        assert!(error.to_string().contains("UTF8_U_CI"), "{error}");
        // So does a plain sort on the column the schema declares with it.
        let error = raw
            .prepare("SELECT rowid FROM song ORDER BY title")
            .map(|_| ())
            .expect_err("the column's collation is missing");
        assert!(error.to_string().contains("UTF8_U_CI"), "{error}");
    }

    #[test]
    fn a_song_without_a_word_row_or_words_is_its_own_failure() {
        let dir = tempfile::tempdir().unwrap();
        let data = write_library(
            dir.path(),
            &[
                FixtureSong {
                    title: "No Words",
                    words: None,
                    ..MORNING
                },
                morning(),
            ],
        );
        let words = Connection::open(data.join(WORDS_DB)).unwrap();
        words
            .execute("INSERT INTO word (song_id, words) VALUES (2, NULL)", [])
            .unwrap();
        drop(words);
        let library = open(dir.path());
        assert_eq!(
            library.song(1).and_then(|r| stage(&r)),
            Err(FileError::MissingWords)
        );
        // The NULL row is the newest for song 2: no words either.
        assert_eq!(
            library.song(2).and_then(|r| stage(&r)),
            Err(FileError::MissingWords)
        );
    }

    #[test]
    fn older_word_rows_are_reported_and_the_newest_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let data = write_library(dir.path(), &[morning()]);
        let words = Connection::open(data.join(WORDS_DB)).unwrap();
        words
            .execute(
                "INSERT INTO word (song_id, words) VALUES (1, ?1)",
                [ew8_words(&[("Verse 1", &["Second take"])])],
            )
            .unwrap();
        drop(words);
        let library = open(dir.path());
        let staged = library.song(1).and_then(|r| stage(&r)).unwrap();
        assert_eq!(staged.song.sections[0].lyrics, "Second take");
        assert!(
            staged
                .warnings
                .contains(&Warning::IgnoredAttribute("word (older rows)".into()))
        );
    }

    #[test]
    fn invalid_long_and_missing_fields_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let long_title: &'static str = Box::leak("Panjang ".repeat(200).into_boxed_str());
        write_library(
            dir.path(),
            &[
                FixtureSong {
                    title: long_title,
                    vendor_id: "CCLI-99",
                    description: "",
                    tags: "",
                    ..morning()
                },
                FixtureSong {
                    title: "  ",
                    vendor_id: "",
                    description: "",
                    tags: "",
                    ..morning()
                },
            ],
        );
        let library = open(dir.path());
        let long = library.song(1).and_then(|r| stage(&r)).unwrap();
        assert!(long.song.title.len() <= MAX_TITLE);
        assert_eq!(
            long.warnings,
            [
                Warning::Truncated("title".into()),
                Warning::InvalidField("vendor_id".into())
            ]
        );
        assert_eq!(long.song.song_number, "");
        let untitled = library.song(2).and_then(|r| stage(&r)).unwrap();
        assert_eq!(untitled.song.title, UNTITLED);
        assert_eq!(untitled.warnings, [Warning::MissingTitle]);
        // Text that is not UTF-8 is replaced and reported.
        let data = dir.path().join("Databases/Data");
        drop(library);
        let songs = Connection::open(data.join(SONGS_DB)).unwrap();
        songs
            .create_collation(COLLATION, super::fixture::utf8_u_ci)
            .unwrap();
        songs
            .execute(
                "UPDATE song SET author = CAST(x'4b61c3' AS TEXT) WHERE rowid = 2",
                [],
            )
            .unwrap();
        drop(songs);
        let library = open(dir.path());
        let broken = library.song(2).and_then(|r| stage(&r)).unwrap();
        assert!(
            broken
                .warnings
                .contains(&Warning::InvalidField("author".into()))
        );
        assert_eq!(broken.song.authors, "Ka\u{fffd}");
    }

    #[test]
    fn words_the_parser_refuses_or_over_the_value_limit_fail_that_song() {
        let dir = tempfile::tempdir().unwrap();
        let huge: &'static str = Box::leak(
            format!("{{\\rtf1 {}}}", "a".repeat(MAX_VALUE_BYTES as usize + 10)).into_boxed_str(),
        );
        let big: &'static str =
            Box::leak(format!("{{\\rtf1 {}}}", "a".repeat(MAX_FILE_BYTES + 10)).into_boxed_str());
        write_library(
            dir.path(),
            &[
                FixtureSong {
                    title: "Plain",
                    words: Some("just text"),
                    ..MORNING
                },
                FixtureSong {
                    title: "Huge",
                    words: Some(huge),
                    ..MORNING
                },
                FixtureSong {
                    title: "Big",
                    words: Some(big),
                    ..MORNING
                },
                morning(),
            ],
        );
        let library = open(dir.path());
        let ew = |r| FileError::Rejected(Some(Format::EasyWorship), r);
        assert_eq!(
            library.song(1).and_then(|r| stage(&r)),
            Err(ew(Rejected::WrongFormat))
        );
        assert_eq!(
            library.song(2).and_then(|r| stage(&r)),
            Err(ew(Rejected::TooLarge))
        );
        assert_eq!(
            library.song(3).and_then(|r| stage(&r)),
            Err(ew(Rejected::TooLarge))
        );
        assert!(
            library.song(4).and_then(|r| stage(&r)).is_ok(),
            "the rest still read"
        );
    }

    #[test]
    fn the_folder_is_found_from_the_profile_or_the_data_folder() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("Default");
        let data = profile.join("v6.1/Databases/Data");
        std::fs::create_dir_all(&data).unwrap();
        write_databases(&data, &[morning()], false);
        for chosen in [
            dir.path().to_path_buf(),
            profile.clone(),
            profile.join("v6.1"),
            profile.join("v6.1/Databases"),
            data.clone(),
        ] {
            let (songs, words) = locate(&chosen).unwrap_or_else(|e| panic!("{chosen:?}: {e:?}"));
            assert_eq!(songs, data.join(SONGS_DB));
            assert_eq!(words, data.join(WORDS_DB));
        }
        // Names compare without ASCII case.
        let other = tempfile::tempdir().unwrap();
        std::fs::copy(data.join(SONGS_DB), other.path().join("songs.DB")).unwrap();
        assert_eq!(locate(other.path()), Err(LibraryError::NoWordsDb));
        std::fs::copy(data.join(WORDS_DB), other.path().join("SONGWORDS.db")).unwrap();
        assert!(Library::open(other.path(), &AtomicBool::new(false)).is_ok());
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(locate(empty.path()), Err(LibraryError::NoSongsDb));
    }

    #[test]
    fn files_that_are_not_an_easyworship_library_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SONGS_DB),
            b"not a database at all, just text",
        )
        .unwrap();
        std::fs::write(dir.path().join(WORDS_DB), b"").unwrap();
        assert!(matches!(
            Library::open(dir.path(), &AtomicBool::new(false)).err(),
            Some(LibraryError::NotLibrary(_))
        ));
        // A database with other tables.
        let other = tempfile::tempdir().unwrap();
        for name in [SONGS_DB, WORDS_DB] {
            Connection::open(other.path().join(name))
                .unwrap()
                .execute_batch("CREATE TABLE notes (body TEXT);")
                .unwrap();
        }
        let refused = Library::open(other.path(), &AtomicBool::new(false)).err();
        assert_eq!(
            refused,
            Some(LibraryError::NotLibrary(
                "Songs.db has no song table".into()
            ))
        );
        Connection::open(other.path().join(SONGS_DB))
            .unwrap()
            .execute_batch("CREATE TABLE song (title TEXT);")
            .unwrap();
        let refused = Library::open(other.path(), &AtomicBool::new(false)).err();
        assert_eq!(
            refused,
            Some(LibraryError::NotLibrary(
                "the word table in SongWords.db has no song_id column".into()
            ))
        );
        // Only `title` and the word columns are required.
        Connection::open(other.path().join(WORDS_DB))
            .unwrap()
            .execute_batch("CREATE TABLE word (song_id INTEGER, words TEXT);")
            .unwrap();
        assert!(Library::open(other.path(), &AtomicBool::new(false)).is_ok());
    }

    #[test]
    fn a_source_that_changes_during_the_copy_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let data = write_library(dir.path(), &[morning()]);
        let songs = data.join(SONGS_DB);
        let append = || {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&songs)
                .unwrap();
            file.write_all(b"\0").unwrap();
        };
        assert_eq!(
            Library::open_with(dir.path(), &AtomicBool::new(false), &append).err(),
            Some(LibraryError::Changed)
        );
        let fresh = tempfile::tempdir().unwrap();
        let data = write_library(fresh.path(), &[morning()]);
        let journal = with_suffix(&data.join(SONGS_DB), "-journal");
        let start_write = || std::fs::write(&journal, b"").unwrap();
        assert_eq!(
            Library::open_with(fresh.path(), &AtomicBool::new(false), &start_write).err(),
            Some(LibraryError::Changed),
            "a journal that appears during the copy"
        );
    }

    #[test]
    fn a_cancelled_copy_stops_before_opening() {
        let dir = tempfile::tempdir().unwrap();
        write_library(dir.path(), &[morning()]);
        assert_eq!(
            Library::open(dir.path(), &AtomicBool::new(true)).err(),
            Some(LibraryError::Canceled)
        );
    }

    #[test]
    fn an_oversized_database_is_refused_from_its_size() {
        let dir = tempfile::tempdir().unwrap();
        let file = std::fs::File::create(dir.path().join(SONGS_DB)).unwrap();
        file.set_len(MAX_DB_BYTES + 1).unwrap();
        std::fs::write(dir.path().join(WORDS_DB), b"").unwrap();
        assert_eq!(
            Library::open(dir.path(), &AtomicBool::new(false)).err(),
            Some(LibraryError::TooLarge)
        );
    }
}
