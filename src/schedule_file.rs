//! Schedule documents: `.selaschedule` files, like EasyWorship's `.ews`.
//!
//! The file is versioned UTF-8 JSON: `{"format":"sela-schedule","version":4,
//! "title":..,"items":[..]}`. Each item embeds the full pinned song revision
//! (text, arrangement, formats, backgrounds, metadata) next to the source song
//! id and revision, so a file opens against any library. Version 2 adds the
//! item's chosen `arrangement` (a variant id of its embedded song); version 1
//! files still open, with every item on the song's first arrangement. Version 3
//! adds the song's optional `administrator` and `song_number`; older files open
//! with both empty. Version 4 adds the item's theme snapshot (`theme`, hex of
//! `Theme::encode`, and `theme_explicit`, M1-08); older files open without
//! one and take the library theme when first shown. A later
//! version adds item kinds or asset snapshots; readers reject a version
//! they do not know instead of guessing. Formats and backgrounds are embedded as hex of
//! their own library codecs (`SlideFormat::encode`, `Background::encode`).
//!
//! All I/O here is blocking. Use `Worker`, which owns a thread, from the UI.
use crate::arrangement::{Occurrence, OccurrenceId, SectionId, Variant, VariantId};
use crate::background::Background;
use crate::format::SlideFormat;
use crate::schedule::ItemTheme;
use crate::storage::{Id, MAX_ITEMS, Section, Song, Version};
use crate::theme::Theme;
use serde_json::{Map, Value, json};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
};

pub const FORMAT: &str = "sela-schedule";
pub const VERSION: u32 = 4;
pub const EXTENSION: &str = "selaschedule";
pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TITLE_BYTES: usize = 1024;
const QUEUE: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The library song and revision the item was saved from.
    pub source: Version,
    pub song: Song,
    /// The arrangement this occurrence plays; None is the song's first.
    pub arrangement: Option<VariantId>,
    /// The item's theme snapshot (version 4); `None` takes the library theme
    /// when the item is first shown.
    pub theme: Option<ItemTheme>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduleFile {
    pub title: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    NotFound,
    /// The file could not be opened or read (permissions, a folder, ...).
    Unreadable(String),
    TooLarge,
    /// Not UTF-8 JSON, truncated, or not a Sela schedule.
    Corrupt,
    /// Written by a newer Sela (`version` is the file's).
    Newer(u64),
    TooManyItems,
    TitleTooLong,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    TooLarge,
    Invalid,
    Io(String),
}

/// `path` with the `.selaschedule` extension, added only when missing so a
/// name that already has it is never doubled.
pub fn with_extension(path: &Path) -> PathBuf {
    let has = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION));
    if has {
        return path.to_path_buf();
    }
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".");
    name.push(EXTENSION);
    path.with_file_name(name)
}

/// The file name shown for a schedule.
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
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

fn id_hex(id: Id) -> String {
    hex(&id.0)
}

fn id_from(value: &Value) -> Option<Id> {
    Some(Id(unhex(value.as_str()?)?.try_into().ok()?))
}

pub fn song_json(song: &Song) -> Value {
    let sections: Vec<Value> = song
        .sections
        .iter()
        .map(|s| {
            let mut section = Map::new();
            section.insert("id".into(), id_hex(s.id.0).into());
            section.insert("label".into(), s.label.clone().into());
            section.insert("lyrics".into(), s.lyrics.clone().into());
            if !s.format.is_default() {
                section.insert("format".into(), hex(&s.format.encode()).into());
            }
            if let Some(background) = &s.background {
                section.insert("background".into(), hex(&background.encode()).into());
            }
            Value::Object(section)
        })
        .collect();
    let variants: Vec<Value> = song
        .variants
        .iter()
        .map(|v| {
            json!({
                "id": id_hex(v.id.0),
                "name": v.name,
                "occurrences": v.occurrences.iter().map(|o| json!({
                    "id": id_hex(o.id.0),
                    "section": id_hex(o.section.0),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let mut out = Map::new();
    out.insert("title".into(), song.title.clone().into());
    out.insert("authors".into(), song.authors.clone().into());
    out.insert("copyright".into(), song.copyright.clone().into());
    out.insert("license".into(), song.license.clone().into());
    // Written only when set, so a song without them keeps its content hash
    // (`import_hidden`) and its encoding from before version 3.
    if !song.administrator.is_empty() {
        out.insert("administrator".into(), song.administrator.clone().into());
    }
    if !song.song_number.is_empty() {
        out.insert("song_number".into(), song.song_number.clone().into());
    }
    if let Some(master) = &song.master {
        out.insert("master".into(), hex(&master.encode()).into());
    }
    out.insert("sections".into(), sections.into());
    out.insert("variants".into(), variants.into());
    Value::Object(out)
}

fn text(value: &Value, key: &str) -> Option<String> {
    Some(value.get(key)?.as_str()?.to_owned())
}

/// Absent is empty (files before version 3); present must be a string.
fn optional_text(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        None => Some(String::new()),
        Some(v) => Some(v.as_str()?.to_owned()),
    }
}

pub fn song_from_json(value: &Value) -> Option<Song> {
    let sections = value
        .get("sections")?
        .as_array()?
        .iter()
        .map(|s| {
            Some(Section {
                id: SectionId(id_from(s.get("id")?)?),
                label: text(s, "label")?,
                lyrics: text(s, "lyrics")?,
                format: match s.get("format") {
                    Some(f) => SlideFormat::decode(&unhex(f.as_str()?)?)?,
                    None => SlideFormat::default(),
                },
                background: match s.get("background") {
                    Some(b) => Some(Background::decode(&unhex(b.as_str()?)?)?),
                    None => None,
                },
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let variants = value
        .get("variants")?
        .as_array()?
        .iter()
        .map(|v| {
            Some(Variant {
                id: VariantId(id_from(v.get("id")?)?),
                name: text(v, "name")?,
                occurrences: v
                    .get("occurrences")?
                    .as_array()?
                    .iter()
                    .map(|o| {
                        Some(Occurrence {
                            id: OccurrenceId(id_from(o.get("id")?)?),
                            section: SectionId(id_from(o.get("section")?)?),
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let song = Song {
        title: text(value, "title")?,
        authors: text(value, "authors")?,
        copyright: text(value, "copyright")?,
        license: text(value, "license")?,
        administrator: optional_text(value, "administrator")?,
        song_number: optional_text(value, "song_number")?,
        sections,
        variants,
        master: match value.get("master") {
            Some(m) => Some(Background::decode(&unhex(m.as_str()?)?)?),
            None => None,
        },
    };
    song.validate().ok()?;
    Some(song)
}

pub fn encode(file: &ScheduleFile) -> Result<Vec<u8>, WriteError> {
    if file.items.len() > MAX_ITEMS || file.title.len() > MAX_TITLE_BYTES {
        return Err(WriteError::Invalid);
    }
    let items: Vec<String> = file
        .items
        .iter()
        .map(|item| {
            let mut value = Map::new();
            value.insert("kind".into(), "song".into());
            value.insert("song_id".into(), id_hex(item.source.id).into());
            value.insert("revision".into(), item.source.revision.into());
            value.insert("song".into(), song_json(&item.song));
            if let Some(arrangement) = item.arrangement {
                value.insert("arrangement".into(), id_hex(arrangement.0).into());
            }
            if let Some(theme) = &item.theme {
                value.insert("theme".into(), hex(&theme.theme.encode()).into());
                value.insert("theme_explicit".into(), theme.explicit.into());
            }
            Value::Object(value).to_string()
        })
        .collect();
    let title = Value::from(file.title.clone()).to_string();
    let out = format!(
        "{{\"format\":\"{FORMAT}\",\"version\":{VERSION},\"title\":{title},\"items\":[{}]}}\n",
        items.join(",")
    );
    if out.len() > MAX_FILE_BYTES {
        return Err(WriteError::TooLarge);
    }
    Ok(out.into_bytes())
}

/// Parses a schedule document. `bytes` must already be bounded by the caller
/// (`read` does); the checks below keep a hostile file from reaching the
/// library.
pub fn decode(bytes: &[u8]) -> Result<ScheduleFile, ReadError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(ReadError::TooLarge);
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ReadError::Corrupt)?;
    let object = value.as_object().ok_or(ReadError::Corrupt)?;
    if object.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(ReadError::Corrupt);
    }
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .ok_or(ReadError::Corrupt)?;
    if version > u64::from(VERSION) {
        return Err(ReadError::Newer(version));
    }
    if version == 0 {
        return Err(ReadError::Corrupt);
    }
    let title = object
        .get("title")
        .and_then(Value::as_str)
        .ok_or(ReadError::Corrupt)?;
    if title.len() > MAX_TITLE_BYTES {
        return Err(ReadError::TitleTooLong);
    }
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .ok_or(ReadError::Corrupt)?;
    if items.len() > MAX_ITEMS {
        return Err(ReadError::TooManyItems);
    }
    let items = items
        .iter()
        .map(|item| {
            if item.get("kind")?.as_str()? != "song" {
                return None;
            }
            let revision = item.get("revision")?.as_i64().filter(|r| *r > 0)?;
            let song = song_from_json(item.get("song")?)?;
            // A choice the embedded song does not have is dropped, not an error.
            let arrangement = match item.get("arrangement").filter(|_| version >= 2) {
                Some(value) => Some(VariantId(id_from(value)?)),
                None => None,
            }
            .filter(|id| song.variants.iter().any(|v| v.id == *id));
            // Version 4: the item's theme snapshot (M1-08). A damaged theme
            // makes the file corrupt rather than silently restyling it.
            let theme = match item.get("theme").filter(|_| version >= 4) {
                Some(value) => {
                    let bytes = unhex(value.as_str()?)?;
                    if bytes.len() > crate::theme::MAX_ENCODED {
                        return None;
                    }
                    Some(ItemTheme {
                        theme: Theme::decode(&bytes)?,
                        explicit: match item.get("theme_explicit") {
                            Some(flag) => flag.as_bool()?,
                            None => false,
                        },
                    })
                }
                None => None,
            };
            Some(Item {
                source: Version {
                    id: id_from(item.get("song_id")?)?,
                    revision,
                },
                song,
                arrangement,
                theme,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(ReadError::Corrupt)?;
    Ok(ScheduleFile {
        title: title.to_owned(),
        items,
    })
}

fn read_error(error: &io::Error) -> ReadError {
    if error.kind() == io::ErrorKind::NotFound {
        ReadError::NotFound
    } else {
        ReadError::Unreadable(error.to_string())
    }
}

/// Reads at most `MAX_FILE_BYTES + 1` bytes: a larger file is refused without
/// loading it.
pub fn read(path: &Path) -> Result<ScheduleFile, ReadError> {
    let file = fs::File::open(path).map_err(|e| read_error(&e))?;
    let metadata = file.metadata().map_err(|e| read_error(&e))?;
    if !metadata.is_file() {
        return Err(ReadError::Unreadable("not a file".into()));
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return Err(ReadError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| read_error(&e))?;
    decode(&bytes)
}

/// Writes `bytes` to a temporary file beside `path`, syncs it, then replaces
/// `path`. A failure at any step removes the temporary file and leaves the
/// previous `path` byte-identical.
pub fn write_atomic_with(
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
    replace: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    if fs::metadata(path).is_ok_and(|m| m.permissions().readonly()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the file is read-only",
        ));
    }
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    let temp = path.with_file_name(name);
    let result = (|| {
        let mut file = fs::File::create(&temp)?;
        write(&mut file, bytes)?;
        file.sync_all()?;
        drop(file);
        replace(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_with(
        path,
        bytes,
        |file, bytes| file.write_all(bytes),
        |from, to| fs::rename(from, to),
    )
}

pub type Writer = Arc<dyn Fn(&Path, &[u8]) -> io::Result<()> + Send + Sync>;

pub fn default_writer() -> Writer {
    Arc::new(write_atomic)
}

enum Job {
    Read(PathBuf),
    Write(PathBuf, Box<ScheduleFile>),
}

#[derive(Debug)]
pub enum Reply {
    Read(PathBuf, Result<ScheduleFile, ReadError>),
    Written(PathBuf, Result<(), WriteError>),
}

/// Reads and writes schedule files on its own thread; replies are polled.
pub struct Worker {
    jobs: Option<SyncSender<Job>>,
    replies: Receiver<Reply>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn start(writer: Writer) -> io::Result<Self> {
        let (jobs, job_rx) = mpsc::sync_channel::<Job>(QUEUE);
        let (reply_tx, replies) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("sela-schedule-file".into())
            .spawn(move || {
                for job in job_rx {
                    let reply = match job {
                        Job::Read(path) => {
                            let result = read(&path);
                            Reply::Read(path, result)
                        }
                        Job::Write(path, file) => {
                            let result = encode(&file).and_then(|bytes| {
                                writer(&path, &bytes).map_err(|e| WriteError::Io(e.to_string()))
                            });
                            Reply::Written(path, result)
                        }
                    };
                    let _ = reply_tx.send(reply);
                }
            })?;
        Ok(Self {
            jobs: Some(jobs),
            replies,
            thread: Some(thread),
        })
    }

    /// `false` when the queue is full or the thread is gone.
    pub fn read(&self, path: PathBuf) -> bool {
        self.submit(Job::Read(path))
    }

    pub fn write(&self, path: PathBuf, file: ScheduleFile) -> bool {
        self.submit(Job::Write(path, Box::new(file)))
    }

    fn submit(&self, job: Job) -> bool {
        matches!(
            self.jobs.as_ref().map(|jobs| jobs.try_send(job)),
            Some(Ok(()))
        )
    }

    pub fn poll(&self) -> Option<Reply> {
        self.replies.try_recv().ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Queued writes still finish; the handle is detached so a slow disk
        // never delays the UI thread.
        self.jobs = None;
        self.thread.take();
    }
}

#[cfg(test)]
mod tests;
