//! Profile video resources: the managed Videos folder, import, pinning and
//! the thumbnail worker with its bounded cache (M2-02, M2-05 basic). No GPUI
//! types. Policies and limits: docs/media.md "Video resources".
//!
//! Videos are copied (managed, like images) into `<profile>/Resources/Videos/`.
//! A slide pins one by name plus [`fingerprint`] (the byte length and a
//! SHA-256 of the first and last MiB), so a replaced file is noticed without
//! hashing a multi-gigabyte clip on every check. All file and FFmpeg work
//! runs on the [`Worker`] or [`Thumbnailer`] threads; the UI only submits and
//! polls.
use crate::{
    background::{Aspect, VideoRef},
    i18n::{self, Msg},
    media::{
        MediaError,
        decode::{self, Tools},
        probe,
    },
    scene::Extent,
};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    time::Duration,
};

pub const VIDEOS: &str = "Resources/Videos";
const MAX_NAME: usize = 255;
/// Videos listed at most; a larger folder lists the first ones by directory order.
pub const MAX_VIDEOS: usize = 4096;
/// Largest video accepted for import (16 GiB).
pub const MAX_FILE_BYTES: u64 = 16 << 30;
/// Bytes hashed from each end of the file by [`fingerprint`].
pub const FINGERPRINT_SPAN: u64 = 1 << 20;
/// Copy granularity, so a cancelled import stops between chunks.
const COPY_CHUNK: usize = 1 << 20;
/// "name.mp4", then "name (2).mp4" up to this suffix.
const MAX_NAME_ATTEMPTS: u32 = 1000;
/// ffprobe bound for an import check.
pub const IMPORT_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// Media tab thumbnail size (2x a 80x45 row picture for high-DPI displays).
pub const THUMB: Extent = Extent {
    width: 160,
    height: 90,
};
/// One poster frame decode, probe included.
pub const THUMB_TIMEOUT: Duration = Duration::from_secs(10);
/// Thumbnails kept by default: bytes (about 270 at [`THUMB`]) and entries.
pub const THUMB_BUDGET: usize = 16 * 1024 * 1024;
pub const THUMB_ENTRIES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Another I/O failure; the kind is shown to the operator.
    Io(io::ErrorKind),
    /// The video is not in the profile's `Resources/Videos` folder.
    Missing,
    /// The file chosen for import no longer exists.
    SourceMissing,
    /// Not one of the accepted extensions.
    Unsupported,
    /// FFmpeg could not open it as a playable video (no video stream,
    /// unknown codec, damaged file).
    NotVideo(MediaError),
    /// Larger than [`MAX_FILE_BYTES`].
    TooLarge,
    InvalidName,
    ReadDenied,
    ProfileDenied,
    DiskFull,
    Cancelled,
    /// No usable FFmpeg (missing, GPL or nonfree build).
    Unavailable,
    Busy,
    Disconnected,
}

impl Error {
    /// The operator-facing message in the active language.
    pub fn message(&self) -> String {
        match self {
            Self::Io(kind) => i18n::format(Msg::VideoErrIo, &[("kind", kind.to_string())]),
            Self::NotVideo(why) => i18n::format(Msg::VideoErrNotVideo, &[("reason", reason(why))]),
            other => i18n::text(match other {
                Self::Missing => Msg::VideoErrMissing,
                Self::SourceMissing => Msg::VideoErrSourceMissing,
                Self::Unsupported => Msg::VideoErrUnsupported,
                Self::TooLarge => Msg::VideoErrTooLarge,
                Self::InvalidName => Msg::VideoErrInvalidName,
                Self::ReadDenied => Msg::VideoErrReadDenied,
                Self::ProfileDenied => Msg::VideoErrProfileDenied,
                Self::DiskFull => Msg::VideoErrDiskFull,
                Self::Cancelled => Msg::VideoErrCancelled,
                Self::Unavailable => Msg::VideoErrUnavailable,
                Self::Busy => Msg::VideoErrBusy,
                _ => Msg::VideoErrDisconnected,
            })
            .to_owned(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}
impl std::error::Error for Error {}

/// A short, translated reason for a probe or decode failure (the detail
/// FFmpeg printed stays in the log, not in the operator's message).
pub fn reason(error: &MediaError) -> String {
    i18n::text(match error {
        MediaError::Missing => Msg::VideoWhyMissing,
        MediaError::NoVideo => Msg::VideoWhyNoVideo,
        MediaError::UnsupportedCodec(_) => Msg::VideoWhyCodec,
        MediaError::Corrupt(_) | MediaError::NoFrames | MediaError::ProbeUnreadable(_) => {
            Msg::VideoWhyCorrupt
        }
        MediaError::TooLarge { .. } => Msg::VideoWhyTooLarge,
        MediaError::Input(_) | MediaError::ProtocolRefused(_) => Msg::VideoWhyNotLocal,
        MediaError::ProbeTimedOut(_) | MediaError::TimedOut(_) | MediaError::Stalled(_) => {
            Msg::VideoWhySlow
        }
        MediaError::Spawn(_) | MediaError::Crashed(_) | MediaError::ProbeOutputTooLarge => {
            Msg::VideoWhyDecoder
        }
    })
    .to_owned()
}

#[derive(Clone, Copy)]
enum Side {
    Source,
    Profile,
}

fn io_at(error: io::Error, side: Side) -> Error {
    use io::ErrorKind as Kind;
    match (error.kind(), side) {
        (Kind::NotFound, Side::Source) => Error::SourceMissing,
        (Kind::NotFound, Side::Profile) => Error::Missing,
        (Kind::PermissionDenied, Side::Source) => Error::ReadDenied,
        (Kind::PermissionDenied | Kind::ReadOnlyFilesystem, Side::Profile) => Error::ProfileDenied,
        (Kind::StorageFull, _) => Error::DiskFull,
        (kind, _) => Error::Io(kind),
    }
}

pub fn videos_dir(profile: &Path) -> PathBuf {
    profile.join(VIDEOS)
}

/// Containers of the provisional software-decode matrix (docs/media.md).
fn extension(name: &str) -> Option<&'static str> {
    let (_, ext) = name.rsplit_once('.')?;
    Some(match ext.to_ascii_lowercase().as_str() {
        "mp4" => "mp4",
        "m4v" => "m4v",
        "mov" => "mov",
        "mkv" => "mkv",
        "webm" => "webm",
        "avi" => "avi",
        "mpg" => "mpg",
        "mpeg" => "mpeg",
        _ => return None,
    })
}

/// A bare file name with a video extension; no separators or dot files.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && name.trim() == name
        && Path::new(name).file_name() == Some(name.as_ref())
        && extension(name).is_some()
}

/// The name without its extension, as EW's picker shows media
/// (EW8-OBS-039).
pub fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// Video names in the profile, sorted case-insensitively. A missing folder is empty.
pub fn list(profile: &Path) -> Result<Vec<String>, Error> {
    let entries = match fs::read_dir(videos_dir(profile)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_at(error, Side::Profile)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| io_at(e, Side::Profile))?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if valid_name(&name) && entry.file_type().is_ok_and(|t| t.is_file()) {
            names.push(name);
            if names.len() == MAX_VIDEOS {
                break;
            }
        }
    }
    names.sort_by_key(|name| name.to_lowercase());
    Ok(names)
}

/// (length, fingerprint) of an open file: SHA-256 over a version tag, the
/// length and up to [`FINGERPRINT_SPAN`] bytes from each end. Reads at most
/// 2 MiB whatever the size.
fn fingerprint_file(file: &mut File) -> io::Result<(u64, [u8; 32])> {
    let len = file.metadata()?.len();
    let mut hasher = Sha256::new();
    hasher.update(b"sela-video-fingerprint-1");
    hasher.update(len.to_le_bytes());
    let mut buffer = vec![0u8; FINGERPRINT_SPAN.min(len) as usize];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut buffer)?;
    hasher.update(&buffer);
    if len > FINGERPRINT_SPAN {
        let tail = FINGERPRINT_SPAN.min(len - FINGERPRINT_SPAN);
        buffer.truncate(tail as usize);
        file.seek(SeekFrom::Start(len - tail))?;
        file.read_exact(&mut buffer)?;
        hasher.update(&buffer);
    }
    Ok((len, hasher.finalize().into()))
}

/// (length, fingerprint) of the file at `path`.
pub fn fingerprint(path: &Path) -> io::Result<(u64, [u8; 32])> {
    fingerprint_file(&mut File::open(path)?)
}

/// The absolute path of a profile video; FFmpeg only accepts absolute
/// local paths (`ffmpeg::input_args`).
pub fn path(profile: &Path, name: &str) -> PathBuf {
    let joined = videos_dir(profile).join(name);
    std::path::absolute(&joined).unwrap_or(joined)
}

/// Pins a profile video's current bytes for a slide background.
pub fn video_ref(profile: &Path, name: &str) -> Result<VideoRef, Error> {
    if !valid_name(name) {
        return Err(Error::InvalidName);
    }
    let (bytes, fingerprint) =
        fingerprint(&path(profile, name)).map_err(|e| io_at(e, Side::Profile))?;
    Ok(VideoRef {
        name: name.to_owned(),
        bytes,
        fingerprint,
    })
}

/// Why a background video shows black instead (the image rule of M1-05h,
/// extended to videos).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Substitute {
    Missing,
    /// The file under the pinned name has other bytes.
    Changed,
    Invalid,
    /// No usable FFmpeg, so nothing can be played.
    Unavailable,
}

impl Substitute {
    pub fn warning(self, name: &str) -> String {
        let msg = match self {
            Self::Missing => Msg::BgVideoMissing,
            Self::Changed => Msg::BgVideoChanged,
            Self::Invalid => Msg::BgVideoUnreadable,
            Self::Unavailable => Msg::BgVideoUnavailable,
        };
        i18n::format(msg, &[("name", stem(name).to_owned())])
    }
}

/// The playable path of a pinned video, only if its bytes still match the
/// pin. Reads at most 2 MiB: workers only, never the UI thread.
pub fn verify(profile: &Path, video: &VideoRef) -> Result<PathBuf, Substitute> {
    if !valid_name(&video.name) {
        return Err(Substitute::Invalid);
    }
    let path = path(profile, &video.name);
    match fingerprint(&path) {
        Ok((bytes, fingerprint)) if bytes == video.bytes && fingerprint == video.fingerprint => {
            Ok(path)
        }
        Ok(_) => Err(Substitute::Changed),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(Substitute::Missing),
        Err(_) => Err(Substitute::Invalid),
    }
}

/// The pinned video's first frame fitted to `extent` with `aspect`, the
/// still that stands for it in previews and thumbnails. Blocks for up to
/// [`THUMB_TIMEOUT`]: workers only.
pub fn poster(
    profile: &Path,
    tools: Option<&Tools>,
    video: &VideoRef,
    aspect: Aspect,
    extent: Extent,
) -> Thumb {
    let path = verify(profile, video)?;
    let tools = tools.ok_or(Substitute::Unavailable)?;
    decode::first_frame_fit(
        tools,
        &path,
        decode::Fit::for_aspect(aspect, extent),
        THUMB_TIMEOUT,
    )
    .map(|(_, frame)| frame.rgba)
    .map_err(|_| Substitute::Invalid)
}

/// Identity of a video cue background: the video's fingerprint, the fit
/// and whether it repeats.
pub fn background_version(
    video: &VideoRef,
    aspect: Aspect,
    repeat: bool,
) -> crate::scene::ContentVersion {
    crate::scene::ContentVersion {
        id: u128::from_le_bytes(video.fingerprint[16..].try_into().unwrap()),
        revision: aspect as u64 | u64::from(repeat) << 8,
    }
}

/// Identity of a poster frame as a prepared background: the video's
/// fingerprint and the fit.
pub fn poster_version(video: &VideoRef, aspect: Aspect) -> crate::scene::ContentVersion {
    crate::scene::ContentVersion {
        id: u128::from_le_bytes(video.fingerprint[..16].try_into().unwrap()),
        revision: aspect as u64,
    }
}

static TOOLS: std::sync::OnceLock<Option<Tools>> = std::sync::OnceLock::new();

/// Records the FFmpeg build the operator located at startup, so later
/// [`tools`] calls in this process need not locate it again.
pub fn remember_tools(tools: Option<Tools>) {
    let _ = TOOLS.set(tools);
}

/// The process's FFmpeg build: the one remembered, else located now (up to
/// seconds per candidate, `ffmpeg::locate`). Workers only.
pub fn tools() -> Option<Tools> {
    TOOLS
        .get_or_init(|| {
            crate::media::ffmpeg::locate()
                .ok()
                .map(|located| Tools::from(&located))
        })
        .clone()
}

/// What an import stored.
#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    pub name: String,
    pub video: VideoRef,
    pub duration: Option<Duration>,
    /// Display size of the video stream.
    pub extent: Extent,
    /// An identical file was already in the profile under this name; nothing was copied.
    pub reused: bool,
}

fn numbered(stem: &str, attempt: u32, ext: &str) -> String {
    if attempt == 1 {
        format!("{stem}.{ext}")
    } else {
        format!("{stem} ({attempt}).{ext}")
    }
}

/// A dot-prefixed temporary in the videos folder (never listed), removed on drop.
struct Temporary(PathBuf);

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn copy_temporary(
    dir: &Path,
    source: &mut File,
    cancel: &mut dyn FnMut() -> bool,
) -> Result<Temporary, Error> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let profile = |e| io_at(e, Side::Profile);
    let mut opened = None;
    for _ in 0..16 {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(".sela-video-{}-{n}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                opened = Some((Temporary(path), file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(profile(error)),
        }
    }
    let (temporary, mut file) = opened.ok_or(Error::Io(io::ErrorKind::AlreadyExists))?;
    source
        .seek(SeekFrom::Start(0))
        .map_err(|e| io_at(e, Side::Source))?;
    let mut chunk = vec![0u8; COPY_CHUNK];
    let mut copied = 0u64;
    loop {
        if cancel() {
            return Err(Error::Cancelled);
        }
        let read = match source.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(io_at(error, Side::Source)),
        };
        copied += read as u64;
        if copied > MAX_FILE_BYTES {
            return Err(Error::TooLarge);
        }
        file.write_all(&chunk[..read]).map_err(profile)?;
    }
    file.sync_all().map_err(profile)?;
    drop(file);
    if cancel() {
        return Err(Error::Cancelled);
    }
    Ok(temporary)
}

/// Copies `source` into the profile's Videos folder after ffprobe confirmed
/// it holds a decodable video stream. The same name with other bytes gets
/// the next free name ("name (2).mp4"); an identical file (same length and
/// fingerprint) under the same stem is reused. `cancel` is polled between
/// steps and copy chunks; a cancelled import leaves no file.
pub fn import(
    profile: &Path,
    source: &Path,
    tools: &Tools,
    cancel: &mut dyn FnMut() -> bool,
) -> Result<Imported, Error> {
    let name = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(Error::InvalidName)?;
    let ext = extension(name).ok_or(Error::Unsupported)?;
    let (stem, _) = name.rsplit_once('.').ok_or(Error::Unsupported)?;
    let stem: String = stem
        .chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .collect();
    let stem = stem.trim().trim_start_matches('.');
    let stem = if stem.is_empty() { "video" } else { stem };
    let mut file = File::open(source).map_err(|e| io_at(e, Side::Source))?;
    let metadata = file.metadata().map_err(|e| io_at(e, Side::Source))?;
    if !metadata.is_file() {
        return Err(Error::Io(io::ErrorKind::InvalidInput));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(Error::TooLarge);
    }
    let absolute = std::path::absolute(source).map_err(|e| io_at(e, Side::Source))?;
    let info =
        probe::probe(&tools.ffprobe, &absolute, IMPORT_PROBE_TIMEOUT).map_err(Error::NotVideo)?;
    let video = info
        .video
        .as_ref()
        .ok_or(Error::NotVideo(MediaError::NoVideo))?;
    let extent = Extent {
        width: video.display_width,
        height: video.display_height,
    };
    if cancel() {
        return Err(Error::Cancelled);
    }
    let (bytes, fingerprint) = fingerprint_file(&mut file).map_err(|e| io_at(e, Side::Source))?;
    let imported = |name: String, reused: bool| Imported {
        video: VideoRef {
            name: name.clone(),
            bytes,
            fingerprint,
        },
        name,
        duration: info.duration,
        extent,
        reused,
    };
    let dir = videos_dir(profile);
    fs::create_dir_all(&dir).map_err(|e| io_at(e, Side::Profile))?;
    let mut free = None;
    for attempt in 1..=MAX_NAME_ATTEMPTS {
        let candidate = numbered(stem, attempt, ext);
        if !valid_name(&candidate) {
            return Err(Error::InvalidName);
        }
        let at = dir.join(&candidate);
        match fs::symlink_metadata(&at) {
            Ok(existing) if existing.is_file() && existing.len() == bytes => {
                if self::fingerprint(&at).is_ok_and(|f| f == (bytes, fingerprint)) {
                    return Ok(imported(candidate, true));
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                free = Some(attempt);
                break;
            }
            Err(error) => return Err(io_at(error, Side::Profile)),
        }
    }
    let free = free.ok_or(Error::InvalidName)?;
    let temporary = copy_temporary(&dir, &mut file, cancel)?;
    // The copy must be the bytes that were probed and fingerprinted.
    if self::fingerprint(&temporary.0).ok() != Some((bytes, fingerprint)) {
        return Err(Error::SourceMissing);
    }
    for attempt in free..=MAX_NAME_ATTEMPTS {
        let candidate = numbered(stem, attempt, ext);
        if !valid_name(&candidate) {
            return Err(Error::InvalidName);
        }
        match crate::images::link_new(&temporary.0, &dir.join(&candidate)) {
            Ok(()) => return Ok(imported(candidate, false)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_at(error, Side::Profile)),
        }
    }
    Err(Error::InvalidName)
}

/// Least recently used values with a byte budget and an entry cap; the
/// newest entry is always kept, even over budget. Linear lookups: a few
/// hundred entries at most.
pub struct Lru<K, V> {
    entries: VecDeque<(K, V, usize)>,
    budget: usize,
    max_entries: usize,
}

impl<K: PartialEq, V> Lru<K, V> {
    pub fn new(budget: usize, max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            budget,
            max_entries: max_entries.max(1),
        }
    }

    /// Looks up and marks the entry as most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let at = self.entries.iter().position(|(k, _, _)| k == key)?;
        let entry = self.entries.remove(at)?;
        self.entries.push_back(entry);
        self.entries.back().map(|(_, v, _)| v)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.entries.iter().any(|(k, _, _)| k == key)
    }

    pub fn insert(&mut self, key: K, value: V, bytes: usize) {
        self.entries.retain(|(k, _, _)| *k != key);
        self.entries.push_back((key, value, bytes));
        while self.entries.len() > 1
            && (self.bytes() > self.budget || self.entries.len() > self.max_entries)
        {
            self.entries.pop_front();
        }
    }

    /// Keeps only entries whose key passes `keep` (invalidation).
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) {
        self.entries.retain(|(k, _, _)| keep(k));
    }

    pub fn bytes(&self) -> usize {
        self.entries.iter().map(|(_, _, b)| b).sum()
    }

    /// Entries, least recently used first, without touching their order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v, _)| (k, v))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A poster thumbnail request: a listed file (Media tab rows, keyed by
/// name, length and modification time so a replaced file gets a new key)
/// or a pinned slide background with its aspect and picture size (the
/// shape of `images::BackgroundKey`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThumbKey {
    File(Listed),
    Pinned(VideoRef, Aspect, Extent),
}

impl ThumbKey {
    /// The picture size this key decodes to.
    pub fn extent(&self) -> Extent {
        match self {
            Self::File(_) => THUMB,
            Self::Pinned(_, _, extent) => *extent,
        }
    }
}

/// RGBA of `key.extent()`, or why there is none.
pub type Thumb = Result<Arc<[u8]>, Substitute>;

/// One listed video file: its name and the metadata that changes with it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Listed {
    pub name: String,
    pub bytes: u64,
    pub modified: Option<std::time::SystemTime>,
}

/// [`list`] with each file's length and modification time (metadata only).
pub fn list_entries(profile: &Path) -> Result<Vec<Listed>, Error> {
    let dir = videos_dir(profile);
    list(profile).map(|names| {
        names
            .into_iter()
            .map(|name| {
                let metadata = fs::metadata(dir.join(&name)).ok();
                Listed {
                    bytes: metadata.as_ref().map_or(0, fs::Metadata::len),
                    modified: metadata.and_then(|m| m.modified().ok()),
                    name,
                }
            })
            .collect()
    })
}

struct Wanted<K> {
    /// Most urgent first; replaced as a whole by every `want`.
    keys: VecDeque<K>,
    closed: bool,
}

/// One thread that works through the most urgent wanted keys one at a
/// time, never more than the replies buffer holds ahead of the caller.
/// Requests are a latest-wins list, so scrolling a long list never queues
/// stale work; a key is worked at most once per `want` list it stays on
/// (in-flight deduplication: the caller wants only keys its cache lacks),
/// and results go to the caller's [`Lru`]. Used for poster thumbnails
/// ([`Thumbnailer`]) and pin checks ([`Checker`]). Audience playback runs
/// in another process with its own decoder, so this work never takes a
/// live slot; one FFmpeg child at a time bounds the CPU it takes.
pub struct Lookups<K, V> {
    queue: Arc<(Mutex<Wanted<K>>, Condvar)>,
    replies: Receiver<(K, V)>,
}

pub type Thumbnailer = Lookups<ThumbKey, Thumb>;
/// Pin checks of slide videos before a cue: the playable path or why not.
pub type Checker = Lookups<VideoRef, Result<PathBuf, Substitute>>;

impl Lookups<ThumbKey, Thumb> {
    /// Poster frames of profile videos (first frames, Zoom for listed files).
    pub fn thumbnails(profile: PathBuf, tools: Tools) -> io::Result<Self> {
        Self::start_with("sela-video-thumbs", move |key: &ThumbKey| match key {
            ThumbKey::File(listed) => {
                let video = video_ref(&profile, &listed.name).map_err(|_| Substitute::Invalid)?;
                poster(&profile, Some(&tools), &video, Aspect::Zoom, THUMB)
            }
            ThumbKey::Pinned(video, aspect, extent) => {
                poster(&profile, Some(&tools), video, *aspect, *extent)
            }
        })
    }
}

impl Lookups<VideoRef, Result<PathBuf, Substitute>> {
    /// [`verify`] of each wanted pin (reads at most 2 MiB per video).
    pub fn checks(profile: PathBuf) -> io::Result<Self> {
        Self::start_with("sela-video-check", move |video: &VideoRef| {
            verify(&profile, video)
        })
    }
}

impl<K: Send + 'static, V: Send + 'static> Lookups<K, V> {
    /// `make` turns one key into its value on the worker thread.
    pub fn start_with(name: &str, make: impl Fn(&K) -> V + Send + 'static) -> io::Result<Self> {
        let queue = Arc::new((
            Mutex::new(Wanted {
                keys: VecDeque::new(),
                closed: false,
            }),
            Condvar::new(),
        ));
        let (sender, replies) = mpsc::sync_channel(2);
        let shared = queue.clone();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                loop {
                    let key = {
                        let (lock, ready) = &*shared;
                        let Ok(mut state) = lock.lock() else { return };
                        loop {
                            if state.closed {
                                return;
                            }
                            if let Some(key) = state.keys.pop_front() {
                                break key;
                            }
                            state = match ready.wait(state) {
                                Ok(state) => state,
                                Err(_) => return,
                            };
                        }
                    };
                    let value = make(&key);
                    if sender.send((key, value)).is_err() {
                        return;
                    }
                }
            })?;
        Ok(Self { queue, replies })
    }

    /// Replaces the wanted list (most urgent first); a key already being
    /// worked finishes and replies anyway.
    pub fn want(&self, keys: impl IntoIterator<Item = K>) {
        let (lock, ready) = &*self.queue;
        if let Ok(mut state) = lock.lock() {
            state.keys = keys.into_iter().collect();
            ready.notify_one();
        }
    }

    /// A finished result, if one is waiting. Never blocks.
    pub fn poll(&self) -> Option<(K, V)> {
        self.replies.try_recv().ok()
    }
}

impl<K, V> Drop for Lookups<K, V> {
    fn drop(&mut self) {
        let (lock, ready) = &*self.queue;
        if let Ok(mut state) = lock.lock() {
            state.closed = true;
        }
        ready.notify_all();
    }
}

pub enum Job {
    Scan,
    /// Copy a video in after checking that this FFmpeg can play it.
    Import(PathBuf, Tools),
}

pub enum Reply {
    Videos(Result<Vec<Listed>, Error>),
    Imported(Result<Imported, Error>),
}

/// The Media tab's video file work: one thread, two queued jobs and two
/// buffered replies; never blocks the caller (the image worker's shape).
pub struct Worker {
    jobs: SyncSender<Job>,
    replies: Receiver<Reply>,
    imports: AtomicU64,
    cancelled: Arc<AtomicU64>,
}

impl Worker {
    pub fn start(profile: PathBuf) -> io::Result<Self> {
        let (jobs, incoming) = mpsc::sync_channel::<Job>(2);
        let (outgoing, replies) = mpsc::sync_channel(2);
        let cancelled = Arc::new(AtomicU64::new(0));
        let cancel = cancelled.clone();
        std::thread::Builder::new()
            .name("sela-videos".into())
            .spawn(move || {
                let mut imports = 0;
                while let Ok(job) = incoming.recv() {
                    let reply = match job {
                        Job::Scan => Reply::Videos(list_entries(&profile)),
                        Job::Import(source, tools) => {
                            imports += 1;
                            let mut cancelled = || cancel.load(Ordering::Acquire) >= imports;
                            Reply::Imported(import(&profile, &source, &tools, &mut cancelled))
                        }
                    };
                    if outgoing.send(reply).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            jobs,
            replies,
            imports: AtomicU64::new(0),
            cancelled,
        })
    }

    pub fn submit(&self, job: Job) -> Result<(), Error> {
        let import = matches!(job, Job::Import(..));
        self.jobs.try_send(job).map_err(|error| match error {
            TrySendError::Full(_) => Error::Busy,
            TrySendError::Disconnected(_) => Error::Disconnected,
        })?;
        if import {
            self.imports.fetch_add(1, Ordering::AcqRel);
        }
        Ok(())
    }

    /// Cancels every import submitted so far, running or queued.
    pub fn cancel_imports(&self) {
        self.cancelled
            .fetch_max(self.imports.load(Ordering::Acquire), Ordering::AcqRel);
    }

    pub fn poll(&self) -> Option<Result<Reply, Error>> {
        match self.replies.try_recv() {
            Ok(reply) => Some(Ok(reply)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(Error::Disconnected)),
        }
    }
}

#[cfg(test)]
mod tests;
