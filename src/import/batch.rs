//! One import batch (M1-14): which files a choice of files and folders
//! covers, which parser reads each file, and the storable song staged from
//! it. Framework independent; the `sela-import` worker (`import::worker`)
//! runs this off the UI thread and the operator only shows the results.
//!
//! Format by file (docs/import.md "Import framework"):
//! - `.cho`, `.chopro`, `.chordpro`, `.crd`, `.pro`: ChordPro.
//! - `.xml`, or no extension with XML content: OpenLyrics when the root is
//!   `<song>` in the OpenLyrics namespace, otherwise OpenSong.
//! - Anything else chosen as a file is reported as unsupported; inside a
//!   chosen folder it is only counted, because song folders hold pictures
//!   and notes too. Files without an extension are the exception: OpenSong
//!   songs have none, so each one is read, and one whose content is not a
//!   song gets an unsupported row (`FileError::Unsupported`).
use super::{MAX_FILE_BYTES, ParsedSong, Rejected, Warning, chordpro, openlyrics, opensong, song};
use crate::storage::Song;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Most files one batch stages; the rest are reported as `BatchFull`.
pub const MAX_FILES: usize = 500;
/// Song text one batch may hold in memory while it waits for the operator.
pub const MAX_STAGED_BYTES: usize = 32 * 1024 * 1024;
/// Folder levels searched below a chosen folder (OpenSong keeps categories
/// in subfolders).
pub const MAX_FOLDER_DEPTH: usize = 3;
/// Directory entries looked at per chosen folder, so a huge tree is bounded.
pub const MAX_FOLDER_ENTRIES: usize = 20_000;
/// Longest title made from a file name (`Song::validate` allows 1,024 bytes).
const MAX_TITLE: usize = 1024;

const CHORDPRO: [&str; 5] = ["cho", "chopro", "chordpro", "crd", "pro"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    OpenLyrics,
    OpenSong,
    ChordPro,
}

/// Why one file of a batch produced no song. The other files still import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileError {
    /// Not a song file type Sela imports, by extension or content.
    Unsupported,
    /// The file or folder could not be read; the operating system's message.
    Unreadable(String),
    /// The parser refused the file. The format is known once the root
    /// element or extension identified it.
    Rejected(Option<Format>, Rejected),
    /// Parsed, but the song is over the library's limits
    /// (`storage::MAX_SONG_BYTES`, sections, arrangement size).
    TooLargeForLibrary,
    /// The batch already holds `MAX_FILES` files.
    BatchFull,
    /// The batch already holds `MAX_STAGED_BYTES` of song text.
    BatchTooLarge,
}

/// Normalized identity of a song for duplicate detection: the title and a
/// digest of the words, both case-folded with punctuation and spacing
/// ignored. Labels, metadata and slide breaks do not count.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DuplicateKey {
    pub title: String,
    pub lyrics: [u8; 32],
}

/// A file read and mapped onto the library's song model, not yet saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staged {
    pub format: Format,
    /// Shared with the commit, so accepting a song never copies its text.
    pub song: Arc<Song>,
    /// Everything the parser dropped or guessed, in the parser's order.
    pub warnings: Vec<Warning>,
    pub key: DuplicateKey,
    /// Bytes of song text held, counted against `MAX_STAGED_BYTES`.
    pub bytes: usize,
}

/// What a choice of files and folders covers.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// Files to stage, chosen files first in the order given, then each
    /// folder's song files sorted by path.
    pub files: Vec<PathBuf>,
    /// Files inside chosen folders that are not song files.
    pub ignored: usize,
    /// Song files past `MAX_FILES`, not staged.
    pub over_limit: usize,
    /// Folders that could not be listed.
    pub unreadable: Vec<(PathBuf, String)>,
    /// Chosen folders whose walk stopped at `MAX_FOLDER_ENTRIES`: entries
    /// after that point were not looked at, so the preview must say so.
    pub truncated: Vec<PathBuf>,
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
}

fn hidden(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
}

/// A file inside a chosen folder that may be a song: a known extension or
/// none (OpenSong song files usually have none).
pub fn is_candidate(path: &Path) -> bool {
    !hidden(path)
        && match extension(path) {
            None => true,
            Some(ext) => ext == "xml" || CHORDPRO.contains(&ext.as_str()),
        }
}

/// Expands chosen paths into the files to stage. Folders are walked up to
/// `MAX_FOLDER_DEPTH` levels without following symbolic links; hidden
/// entries are skipped, and only regular files are candidates (a named
/// pipe or device would block the reader). A folder whose tree holds more
/// than `MAX_FOLDER_ENTRIES` entries is listed in `Found::truncated`.
/// Stops early when `cancel` is set.
pub fn find(paths: &[PathBuf], cancel: &AtomicBool) -> Found {
    find_bounded(paths, cancel, MAX_FOLDER_ENTRIES)
}

fn find_bounded(paths: &[PathBuf], cancel: &AtomicBool, entries: usize) -> Found {
    let mut found = Found::default();
    let push = |found: &mut Found, file: PathBuf| {
        if found.files.len() < MAX_FILES {
            found.files.push(file);
        } else {
            found.over_limit += 1;
        }
    };
    for path in paths {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => {
                let mut files = Vec::new();
                let mut budget = entries;
                let mut exhausted = false;
                walk(
                    path,
                    0,
                    &mut files,
                    &mut found,
                    (&mut budget, &mut exhausted),
                    cancel,
                );
                if exhausted && !cancel.load(Ordering::Acquire) {
                    found.truncated.push(path.clone());
                }
                files.sort();
                for file in files {
                    push(&mut found, file);
                }
            }
            // A chosen file (or link to one) is always staged, so an
            // unsupported one is reported rather than skipped.
            _ => push(&mut found, path.clone()),
        }
    }
    found
}

fn walk(
    dir: &Path,
    depth: usize,
    files: &mut Vec<PathBuf>,
    found: &mut Found,
    (budget, exhausted): (&mut usize, &mut bool),
    cancel: &AtomicBool,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            found
                .unreadable
                .push((dir.to_path_buf(), error.to_string()));
            return;
        }
    };
    let mut folders = Vec::new();
    for entry in entries {
        if cancel.load(Ordering::Acquire) {
            return;
        }
        if *budget == 0 {
            // Another entry exists past the budget: report the folder.
            *exhausted = true;
            return;
        }
        *budget -= 1;
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if hidden(&path) || kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if depth < MAX_FOLDER_DEPTH {
                folders.push(path);
            }
        } else if kind.is_file() && is_candidate(&path) {
            files.push(path);
        } else {
            // Not a song file: pictures, notes, and anything that is not a
            // regular file (named pipe, socket, device).
            found.ignored += 1;
        }
    }
    folders.sort();
    for folder in folders {
        if *exhausted {
            return;
        }
        walk(
            &folder,
            depth + 1,
            files,
            found,
            (&mut *budget, &mut *exhausted),
            cancel,
        );
    }
}

/// Reads at most `MAX_FILE_BYTES` (plus one byte to tell a larger file).
/// Only regular files (or links to one) are opened: opening a named pipe
/// with no writer would block the `sela-import` thread for good.
pub fn read(path: &Path) -> Result<Vec<u8>, FileError> {
    let meta = std::fs::metadata(path).map_err(|e| FileError::Unreadable(e.to_string()))?;
    if !meta.is_file() {
        return Err(FileError::Unsupported);
    }
    let (file, meta) = open_regular(path)?;
    if meta.len() > MAX_FILE_BYTES as u64 {
        return Err(FileError::Rejected(None, Rejected::TooLarge));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| FileError::Unreadable(e.to_string()))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(FileError::Rejected(None, Rejected::TooLarge));
    }
    Ok(bytes)
}

/// Opens `path` and checks the opened file is a regular one. On Unix the
/// open does not wait (`O_NONBLOCK`), so a named pipe swapped in after the
/// type check above returns at once and is refused here instead of
/// blocking the thread; reads from a regular file ignore the flag.
fn open_regular(path: &Path) -> Result<(std::fs::File, std::fs::Metadata), FileError> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|e| FileError::Unreadable(e.to_string()))?;
    let meta = file
        .metadata()
        .map_err(|e| FileError::Unreadable(e.to_string()))?;
    if !meta.is_file() {
        return Err(FileError::Unsupported);
    }
    Ok((file, meta))
}

/// XML content: after a BOM and whitespace, the first character is `<`.
fn looks_like_xml(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    bytes
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|&b| b == b'<')
}

/// Picks the parser for one file and runs it.
pub fn parse(path: &Path, bytes: &[u8]) -> Result<(Format, ParsedSong), FileError> {
    let xml = match extension(path) {
        Some(ext) if CHORDPRO.contains(&ext.as_str()) => false,
        Some(ext) if ext == "xml" => true,
        None if looks_like_xml(bytes) => true,
        _ => return Err(FileError::Unsupported),
    };
    if !xml {
        return chordpro::parse(bytes)
            .map(|song| (Format::ChordPro, song))
            .map_err(|r| FileError::Rejected(Some(Format::ChordPro), r));
    }
    match openlyrics::parse(bytes) {
        Ok(song) => Ok((Format::OpenLyrics, song)),
        // Well formed, but not an OpenLyrics root: try OpenSong.
        Err(Rejected::WrongFormat) => match opensong::parse(bytes) {
            Ok(song) => Ok((Format::OpenSong, song)),
            Err(Rejected::WrongFormat) => Err(FileError::Unsupported),
            Err(reason) => Err(FileError::Rejected(Some(Format::OpenSong), reason)),
        },
        // The root was OpenLyrics; only the content failed.
        Err(Rejected::NoLyrics) => Err(FileError::Rejected(
            Some(Format::OpenLyrics),
            Rejected::NoLyrics,
        )),
        // Malformed or hostile XML: neither format can read it.
        Err(reason) => Err(FileError::Rejected(None, reason)),
    }
}

/// The title used when a file has none: its name without the extension.
pub fn title_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Untitled".to_string());
    let mut end = stem.len().min(MAX_TITLE);
    while !stem.is_char_boundary(end) {
        end -= 1;
    }
    stem[..end].to_string()
}

/// Reads one file into a storable song. A missing title becomes the file
/// name and keeps its `MissingTitle` warning, so the preview says so.
pub fn stage(path: &Path, bytes: &[u8]) -> Result<Staged, FileError> {
    let (format, mut parsed) = parse(path, bytes)?;
    if parsed.title.trim().is_empty() {
        parsed.title = title_from_path(path);
        if !parsed.warnings.contains(&Warning::MissingTitle) {
            parsed.warnings.insert(0, Warning::MissingTitle);
        }
    }
    let song = song::to_song(&parsed).map_err(|_| FileError::TooLargeForLibrary)?;
    let key = duplicate_key(&song);
    let bytes = song_bytes(&song);
    Ok(Staged {
        format,
        song: Arc::new(song),
        warnings: parsed.warnings,
        key,
        bytes,
    })
}

/// Text bytes a staged song holds (fields, labels and lyrics, plus a small
/// fixed cost per slide).
pub fn song_bytes(song: &Song) -> usize {
    [
        &song.title,
        &song.authors,
        &song.copyright,
        &song.license,
        &song.administrator,
        &song.song_number,
    ]
    .iter()
    .map(|s| s.len())
    .sum::<usize>()
        + song
            .sections
            .iter()
            .map(|s| s.label.len() + s.lyrics.len() + 64)
            .sum::<usize>()
}

/// Lower-cased words of `text`: letters and digits are kept, every other
/// run of characters becomes one space.
fn words(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut gap = false;
    for c in text.chars() {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.extend(c.to_lowercase());
        } else {
            gap = true;
        }
    }
    out
}

/// Duplicate-detection form of a title ("Amazing  Grace!" = "amazing grace").
pub fn title_key(title: &str) -> String {
    words(title)
}

/// Digest of a song's words in slide order, as the search index stores them
/// (`storage` joins every section's lyrics with `\n`).
pub fn lyrics_key(lyrics: &str) -> [u8; 32] {
    Sha256::digest(words(lyrics).as_bytes()).into()
}

pub fn duplicate_key(song: &Song) -> DuplicateKey {
    let lyrics = song
        .sections
        .iter()
        .map(|s| s.lyrics.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    DuplicateKey {
        title: title_key(&song.title),
        lyrics: lyrics_key(&lyrics),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(dir: &str, name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/import")
            .join(dir)
            .join(name)
    }

    fn stage_file(path: &Path) -> Result<Staged, FileError> {
        stage(path, &read(path)?)
    }

    #[test]
    fn each_format_is_chosen_by_extension_and_root() {
        let openlyrics = stage_file(&fixture("openlyrics", "fields.xml")).unwrap();
        assert_eq!(openlyrics.format, Format::OpenLyrics);
        assert_eq!(openlyrics.song.title, "Lantern by the Road");
        let opensong = stage_file(&fixture("opensong", "morning-river.xml")).unwrap();
        assert_eq!(opensong.format, Format::OpenSong);
        let chordpro = stage_file(&fixture("chordpro", "harbor-light.cho")).unwrap();
        assert_eq!(chordpro.format, Format::ChordPro);
        // OpenSong files usually have no extension; content decides.
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("Morning River");
        std::fs::copy(fixture("opensong", "morning-river.xml"), &bare).unwrap();
        assert_eq!(stage_file(&bare).unwrap().format, Format::OpenSong);
        for ext in ["chopro", "crd", "pro", "CHO", "chordpro"] {
            let path = dir.path().join(format!("song.{ext}"));
            std::fs::copy(fixture("chordpro", "harbor-light.cho"), &path).unwrap();
            assert_eq!(stage_file(&path).unwrap().format, Format::ChordPro, "{ext}");
        }
    }

    #[test]
    fn unsupported_malformed_and_oversized_files_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("notes.txt");
        std::fs::write(&text, "Just a note").unwrap();
        assert_eq!(stage_file(&text), Err(FileError::Unsupported));
        let bare = dir.path().join("README");
        std::fs::write(&bare, "plain words, not xml").unwrap();
        assert_eq!(stage_file(&bare), Err(FileError::Unsupported));
        let other_root = dir.path().join("set.xml");
        std::fs::write(&other_root, "<set><slide/></set>").unwrap();
        assert_eq!(stage_file(&other_root), Err(FileError::Unsupported));
        assert!(matches!(
            stage_file(&fixture("openlyrics", "malformed-unclosed.xml")),
            Err(FileError::Rejected(None, Rejected::Malformed(_)))
        ));
        assert_eq!(
            stage_file(&fixture("openlyrics", "xxe-file.xml")),
            Err(FileError::Rejected(None, Rejected::Entities))
        );
        let big = dir.path().join("big.cho");
        std::fs::write(&big, vec![b'a'; MAX_FILE_BYTES + 1]).unwrap();
        assert_eq!(
            stage_file(&big),
            Err(FileError::Rejected(None, Rejected::TooLarge))
        );
        let empty = dir.path().join("empty.cho");
        std::fs::write(&empty, "{title: Nothing}\n").unwrap();
        assert_eq!(
            stage_file(&empty),
            Err(FileError::Rejected(
                Some(Format::ChordPro),
                Rejected::NoLyrics
            ))
        );
        assert!(matches!(
            stage_file(&dir.path().join("missing.xml")),
            Err(FileError::Unreadable(_))
        ));
    }

    #[test]
    fn a_song_over_the_library_limit_is_not_staged() {
        // 128 slides of 2.5 KiB pass the parser's own budget but not
        // `storage::MAX_SONG_BYTES` (256 KiB).
        let line = "word ".repeat(500);
        let mut text = String::from("{title: Long}\n");
        for _ in 0..120 {
            text.push_str(&line);
            text.push_str("\n\n");
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("long.cho");
        std::fs::write(&path, &text).unwrap();
        assert_eq!(stage_file(&path), Err(FileError::TooLargeForLibrary));
    }

    #[test]
    fn a_missing_title_becomes_the_file_name_and_stays_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Lagu Tanpa Judul.cho");
        std::fs::write(&path, "Kasih-Mu besar\nsetiap pagi\n").unwrap();
        let staged = stage_file(&path).unwrap();
        assert_eq!(staged.song.title, "Lagu Tanpa Judul");
        assert_eq!(staged.warnings.first(), Some(&Warning::MissingTitle));
    }

    #[test]
    fn duplicate_keys_ignore_case_spacing_and_punctuation_only() {
        assert_eq!(title_key("  Amazing   GRACE! "), "amazing grace");
        assert_eq!(title_key("Kasih-Mu Besar"), title_key("kasih mu besar"));
        assert_ne!(title_key("Sétia"), title_key("Setia"));
        assert_eq!(
            lyrics_key("Line one,\nline TWO"),
            lyrics_key("line one line two")
        );
        assert_ne!(lyrics_key("line one"), lyrics_key("line two"));
    }

    #[test]
    fn folders_are_walked_bounded_sorted_and_without_links() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("b/c/d/e")).unwrap();
        for file in [
            "z.cho",
            "a.xml",
            "Bare",
            "photo.jpg",
            ".hidden.cho",
            "b/one.pro",
            "b/c/two.crd",
            "b/c/d/three.chopro",
            "b/c/d/e/too-deep.cho",
        ] {
            std::fs::write(root.join(file), "x").unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("b"), root.join("link")).unwrap();
        let chosen_file = root.join("photo.jpg");
        let found = find(
            &[chosen_file.clone(), root.to_path_buf()],
            &AtomicBool::new(false),
        );
        let names: Vec<String> = found
            .files
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            names,
            [
                "photo.jpg",
                "Bare",
                "a.xml",
                "b/c/d/three.chopro",
                "b/c/two.crd",
                "b/one.pro",
                "z.cho"
            ]
        );
        assert_eq!(found.ignored, 1, "photo.jpg inside the folder");
        assert_eq!(found.over_limit, 0);
        let canceled = find(&[root.to_path_buf()], &AtomicBool::new(true));
        assert!(canceled.files.is_empty());
    }

    #[test]
    fn a_folder_over_the_entry_budget_is_reported_not_silently_cut() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("sub")).unwrap();
        for n in 0..6 {
            std::fs::write(root.join(format!("{n}.cho")), "x").unwrap();
        }
        let cut = find_bounded(&[root.to_path_buf()], &AtomicBool::new(false), 4);
        assert_eq!(cut.truncated, [root.to_path_buf()]);
        assert!(cut.files.len() <= 4);
        // Exactly the budget: nothing was left unseen.
        let exact = find_bounded(&[root.to_path_buf()], &AtomicBool::new(false), 7);
        assert!(exact.truncated.is_empty());
        assert_eq!(exact.files.len(), 6);
        let whole = find(&[root.to_path_buf()], &AtomicBool::new(false));
        assert!(whole.truncated.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_named_pipe_is_never_opened() {
        let dir = tempfile::tempdir().unwrap();
        // Named like a song file, so only the regular-file check keeps the
        // walk from staging it.
        let pipe = dir.path().join("pipe.cho");
        let made = std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .expect("mkfifo runs");
        assert!(made.success());
        std::fs::write(dir.path().join("song.cho"), "Line\n").unwrap();
        let found = find(&[dir.path().to_path_buf()], &AtomicBool::new(false));
        assert_eq!(found.files, [dir.path().join("song.cho")]);
        assert_eq!(found.ignored, 1, "the pipe is counted, not staged");
        // Chosen directly, it is refused without blocking: opening a FIFO
        // with no writer would wait forever, so read on a thread with a
        // bounded wait.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(read(&pipe));
        });
        let result = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("read returns instead of blocking");
        assert_eq!(result, Err(FileError::Unsupported));
    }

    /// A pipe swapped in between the type check and the open: the open
    /// itself must not wait for a writer.
    #[cfg(unix)]
    #[test]
    fn opening_a_named_pipe_returns_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let pipe = dir.path().join("swapped.cho");
        let made = std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .expect("mkfifo runs");
        assert!(made.success());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(open_regular(&pipe).map(|_| ()));
        });
        let result = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the open returns instead of blocking");
        assert_eq!(result, Err(FileError::Unsupported));
    }

    #[test]
    fn a_batch_stops_at_the_file_limit() {
        let dir = tempfile::tempdir().unwrap();
        for n in 0..MAX_FILES + 3 {
            std::fs::write(dir.path().join(format!("{n:04}.cho")), "x").unwrap();
        }
        let found = find(&[dir.path().to_path_buf()], &AtomicBool::new(false));
        assert_eq!(found.files.len(), MAX_FILES);
        assert_eq!(found.over_limit, 3);
    }
}
