//! Synthetic EasyWorship 6-8 library for tests (M4-07): a `Songs.db` and a
//! `SongWords.db` built from original text. The schema is an UNOBSERVED best
//! guess from public sources (docs/import.md, "EasyWorship library"), never
//! a copy of a real library. Shared by the library tests (`import::
//! ew_library`) and the GPUI tests (`src/tests/ew_library_import.rs`)
//! through `#[path]`, so both build the same folder.
#![allow(dead_code)]

use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

/// One `song` row and its `word` row.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixtureSong {
    pub title: &'static str,
    pub author: &'static str,
    pub copyright: &'static str,
    pub administrator: &'static str,
    /// Stored as given (the column has INTEGER affinity, so digits become
    /// an integer and anything else stays text); empty is NULL.
    pub vendor_id: &'static str,
    pub description: &'static str,
    pub tags: &'static str,
    pub song_uid: &'static str,
    /// RTF words; `None` leaves the song without a `word` row.
    pub words: Option<&'static str>,
}

/// The collation EW's schema names, as the fixture defines it.
pub fn utf8_u_ci(a: &str, b: &str) -> std::cmp::Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

const SONGS_SCHEMA: &str = "
    CREATE TABLE song (
        rowid INTEGER PRIMARY KEY AUTOINCREMENT,
        song_item_uid TEXT, song_rev_uid TEXT, song_uid TEXT,
        title TEXT COLLATE UTF8_U_CI, author TEXT COLLATE UTF8_U_CI,
        copyright TEXT, administrator TEXT, description TEXT, tags TEXT,
        reference_number TEXT, vendor_id INTEGER, presentation_id INTEGER,
        layout_revision INTEGER, revision INTEGER);
    CREATE INDEX song_title ON song (title COLLATE UTF8_U_CI);";

const WORDS_SCHEMA: &str = "
    CREATE TABLE word (
        rowid INTEGER PRIMARY KEY AUTOINCREMENT,
        song_id INTEGER, words TEXT, slide_uids TEXT,
        slide_layout_revisions TEXT, slide_revisions TEXT);
    CREATE INDEX word_song_id ON word (song_id);";

fn null_if_empty(text: &str) -> Option<&str> {
    (!text.is_empty()).then_some(text)
}

/// Writes `Databases/Data/Songs.db` and `SongWords.db` under `root` and
/// returns the data folder. Songs get rowids 1, 2, ... in slice order.
pub fn write_library(root: &Path, songs: &[FixtureSong]) -> PathBuf {
    let data = root.join("Databases").join("Data");
    std::fs::create_dir_all(&data).unwrap();
    write_databases(&data, songs, false);
    data
}

/// Writes both databases into `data`. `wal`: journal mode WAL, and the
/// returned connections stay open with automatic checkpoints off, so the
/// rows live only in the `-wal` files until they are dropped.
pub fn write_databases(
    data: &Path,
    songs: &[FixtureSong],
    wal: bool,
) -> Option<(Connection, Connection)> {
    let songs_db = Connection::open(data.join("Songs.db")).unwrap();
    let words_db = Connection::open(data.join("SongWords.db")).unwrap();
    for db in [&songs_db, &words_db] {
        db.create_collation("UTF8_U_CI", utf8_u_ci).unwrap();
        if wal {
            db.pragma_update(None, "journal_mode", "wal").unwrap();
            db.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        }
    }
    songs_db.execute_batch(SONGS_SCHEMA).unwrap();
    words_db.execute_batch(WORDS_SCHEMA).unwrap();
    for song in songs {
        let vendor: Option<rusqlite::types::Value> =
            null_if_empty(song.vendor_id).map(|v| match v.parse::<i64>() {
                Ok(n) => n.into(),
                Err(_) => v.to_string().into(),
            });
        songs_db
            .execute(
                "INSERT INTO song (song_uid, title, author, copyright, administrator,
                     description, tags, vendor_id, revision)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)",
                params![
                    null_if_empty(song.song_uid),
                    song.title,
                    null_if_empty(song.author),
                    null_if_empty(song.copyright),
                    null_if_empty(song.administrator),
                    null_if_empty(song.description),
                    null_if_empty(song.tags),
                    vendor,
                ],
            )
            .unwrap();
        let id = songs_db.last_insert_rowid();
        if let Some(words) = song.words {
            words_db
                .execute(
                    "INSERT INTO word (song_id, words) VALUES (?1, ?2)",
                    params![id, words],
                )
                .unwrap();
        }
    }
    if wal {
        Some((songs_db, words_db))
    } else {
        None
    }
}

/// Words in the EW8 dialect the parser was built from (docs/import.md,
/// "Observed format"): a hidden label paragraph per section, one paragraph
/// per line and a `\sdslidemarker` paragraph between slides. `text` is
/// already RTF-escaped.
pub fn ew8_words(sections: &[(&str, &[&str])]) -> String {
    let lyric = |text: &str| {
        format!(
            "{{\\pard\\sdlistlevel0\\qc\\qdef\\sdewparatemplatestyle101\\plain\\sdewtemplatestyle101\\fs156{{\\*\\sdfsreal 77.76}}{{\\*\\sdfsdef 77.76}}\\sdfsauto {text}\\par}}\r\n"
        )
    };
    let marker = "{\\pard\\sdslidemarker\\qc\\qdef\\sdewparatemplatestyle101\\plain\\sdewtemplatestyle101\\fs156\\sdfsauto\\par}\r\n";
    let mut out = String::from(
        "{\\rtf1\\ansi\\deff0\\sdeasyworship2\r\n{\\fonttbl{\\f0 Tahoma;}}\r\n{\\colortbl ;}\r\n",
    );
    let mut first = true;
    for (label, slides) in sections {
        if !first {
            out.push_str(marker);
        }
        first = false;
        out.push_str(&format!(
            "{{\\pard\\sdparawysiwghidden\\sdlistlevel-1\\qc\\qdef\\sdewparatemplatestyle102\\plain\\sdewtemplatestyle102\\fs156\\sdfsauto {label}\\par}}\r\n"
        ));
        for (index, slide) in slides.iter().enumerate() {
            if index > 0 {
                out.push_str(marker);
            }
            for line in slide.split('\n') {
                out.push_str(&lyric(line));
            }
        }
    }
    out.push('}');
    out
}

/// Every file under `dir` (recursively) with its SHA-256 and modification
/// time, sorted by path: the "before" and "after" of a source library.
pub fn hashes(dir: &Path) -> Vec<(PathBuf, String, std::time::SystemTime)> {
    use sha2::{Digest, Sha256};
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                let digest: String = Sha256::digest(&bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
                out.push((path, digest, modified));
            }
        }
    }
    out.sort();
    out
}
