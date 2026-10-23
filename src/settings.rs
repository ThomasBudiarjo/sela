//! Per-profile settings file: versioned text, atomic replace, and a worker
//! thread so the operator never touches the disk on the UI thread.
//!
//! Format: first line `sela-settings <version>`, then `key=value` lines.
//! Unknown keys are ignored so a newer build's file still loads; an unknown
//! (newer) version is treated as unreadable rather than guessed at.

use crate::i18n::Language;
use crate::pane_layout::{self, Layout};
use crate::slides::CopyrightOptions;
use crate::song_order::{SongOrder, SongSort};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::JoinHandle,
};

pub const VERSION: u32 = 1;
pub const FILE_NAME: &str = "settings.txt";
const HEADER: &str = "sela-settings";
const QUEUE: usize = 8;
pub const MAX_RECENT: usize = 10;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// Device name of the audience output monitor (for example `\\.\DISPLAY6`);
    /// `None` means the default non-primary monitor.
    pub output_monitor: Option<String>,
    /// Operator UI language; the default (English) is not written to the file.
    pub language: Language,
    /// Schedule files most recently saved or opened, newest first, at most
    /// `MAX_RECENT` and without duplicates. Stored as `recent=<path>` lines
    /// (a key added within version 1: older builds ignore it, and a file
    /// without it loads with an empty list).
    pub recent_schedules: Vec<PathBuf>,
    /// Songs list order (`song_sort`, `song_sort_descending`; keys added
    /// within version 1, written only when not the default).
    pub song_order: SongOrder,
    /// Splitters, Resources collapsed state and window placement saved at the
    /// last normal close; `None` means the default layout.
    pub layout: Option<Layout>,
    /// Options › Main Output › Song › Copyright Info (`copyright_display`,
    /// `copyright_first_slide`, `license_info`; keys added within version 1,
    /// written only when not the default).
    pub copyright: CopyrightOptions,
    /// Stage (Foldback) output, Options › Foldback (M3-01/M3-02). Keys
    /// `stage_*` were added within version 1 and are written only when not
    /// the default, so older builds ignore them.
    pub stage: StageSettings,
}

/// Options › Foldback › General (EW8-OBS-056: "Enable support for Foldback
/// Output" plus its own monitor) and Sela's stage content switches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageSettings {
    /// Start the stage output together with Live output (EW default: off).
    pub enabled: bool,
    /// Device name of the stage monitor; `None` means the first monitor that
    /// is neither primary nor the audience monitor.
    pub monitor: Option<String>,
    /// Time of day in the stage header (default on).
    pub clock: bool,
    /// Next slide area (default on).
    pub next: bool,
}

impl Default for StageSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            monitor: None,
            clock: true,
            next: true,
        }
    }
}

/// The saved layout of `path`, read without side effects. Unreadable or
/// corrupt files give `None` here; `load` handles the backup and note.
pub fn peek_layout(path: &Path) -> Option<Layout> {
    let text = String::from_utf8(read_bounded(path).ok()?).ok()?;
    parse(&text).ok()?.layout
}

/// Largest settings file read. A real one is a few hundred bytes; ten recent
/// paths of the longest Windows length still fit. A larger file is damaged
/// and is treated as unreadable instead of being read into memory.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

fn read_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "settings file is too large",
        ));
    }
    Ok(bytes)
}

impl Settings {
    /// Moves `path` to the front of the recent schedules.
    pub fn remember_recent(&mut self, path: PathBuf) {
        self.recent_schedules.retain(|p| *p != path);
        self.recent_schedules.insert(0, path);
        self.recent_schedules.truncate(MAX_RECENT);
    }

    pub fn forget_recent(&mut self, path: &Path) {
        self.recent_schedules.retain(|p| p != path);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    Header,
    Version(u32),
    Line(usize),
}

pub fn parse(text: &str) -> Result<Settings, ParseError> {
    if text.is_empty() {
        return Err(ParseError::Empty);
    }
    // LF, CRLF and a bare CR (old Mac line ends, a stray CR from an editor)
    // all end a line, so a CR never becomes part of a path or name.
    let text = text.replace("\r\n", "\n");
    let mut lines = text.split(['\n', '\r']);
    let header = lines.next().ok_or(ParseError::Empty)?;
    let version = header
        .strip_prefix(HEADER)
        .and_then(|rest| rest.trim().parse::<u32>().ok())
        .ok_or(ParseError::Header)?;
    if version == 0 || version > VERSION {
        return Err(ParseError::Version(version));
    }
    let mut settings = Settings::default();
    let mut layout = pane_layout::Parts::default();
    for (index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or(ParseError::Line(index + 2))?;
        if layout.accept(key, value) {
            continue;
        }
        // A name or path with control characters cannot be saved faithfully
        // (`render` leaves it out), so it is ignored like an unknown value.
        let usable = !value.is_empty() && !value.chars().any(char::is_control);
        if key == "output_monitor" && usable {
            settings.output_monitor = Some(value.to_owned());
        } else if key == "language" {
            // Unknown codes are ignored like unknown keys: English applies.
            settings.language = Language::from_code(value).unwrap_or_default();
        } else if key == "song_sort" {
            settings.song_order.sort = SongSort::from_code(value).unwrap_or_default();
        } else if key == "song_sort_descending" {
            settings.song_order.descending = value == "1";
        } else if key == "copyright_display" {
            // Only an explicit 0 turns the observed default (on) off.
            settings.copyright.display = value != "0";
        } else if key == "copyright_first_slide" {
            settings.copyright.first_slide_only = value == "1";
        } else if key == "license_info" {
            settings.copyright.license_info = CopyrightOptions::clean_license(value);
        } else if key == "stage_enabled" {
            settings.stage.enabled = value == "1";
        } else if key == "stage_monitor" && usable {
            settings.stage.monitor = Some(value.to_owned());
        } else if key == "stage_clock" {
            settings.stage.clock = value != "0";
        } else if key == "stage_next" {
            settings.stage.next = value != "0";
        } else if key == "recent" && usable {
            // Lines are newest first; keep the first occurrence of a path.
            let path = PathBuf::from(value);
            if settings.recent_schedules.len() < MAX_RECENT
                && !settings.recent_schedules.contains(&path)
            {
                settings.recent_schedules.push(path);
            }
        }
    }
    settings.layout = layout.finish();
    Ok(settings)
}

pub fn render(settings: &Settings) -> String {
    let mut text = format!("{HEADER} {VERSION}\n");
    // A value with a control character (a line break would corrupt the
    // file) is left out rather than saved as a different name or path.
    let writable = |value: &str| !value.is_empty() && !value.chars().any(char::is_control);
    if let Some(name) = settings.output_monitor.as_deref().filter(|n| writable(n)) {
        text.push_str(&format!("output_monitor={name}\n"));
    }
    if settings.language != Language::default() {
        text.push_str(&format!("language={}\n", settings.language.code()));
    }
    if settings.song_order.sort != SongSort::default() {
        text.push_str(&format!("song_sort={}\n", settings.song_order.sort.code()));
    }
    if settings.song_order.descending {
        text.push_str("song_sort_descending=1\n");
    }
    if !settings.copyright.display {
        text.push_str("copyright_display=0\n");
    }
    if settings.copyright.first_slide_only {
        text.push_str("copyright_first_slide=1\n");
    }
    let license = CopyrightOptions::clean_license(&settings.copyright.license_info);
    if !license.is_empty() {
        text.push_str(&format!("license_info={license}\n"));
    }
    if settings.stage.enabled {
        text.push_str("stage_enabled=1\n");
    }
    if let Some(name) = settings.stage.monitor.as_deref().filter(|n| writable(n)) {
        text.push_str(&format!("stage_monitor={name}\n"));
    }
    if !settings.stage.clock {
        text.push_str("stage_clock=0\n");
    }
    if !settings.stage.next {
        text.push_str("stage_next=0\n");
    }
    for path in settings.recent_schedules.iter().take(MAX_RECENT) {
        // A path that is not valid Unicode cannot be written back either.
        if let Some(path) = path.to_str().filter(|p| writable(p)) {
            text.push_str(&format!("recent={path}\n"));
        }
    }
    if let Some(layout) = &settings.layout {
        text.push_str(&layout.render());
    }
    text
}

pub fn path_in(profile: &Path) -> PathBuf {
    profile.join(FILE_NAME)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Loaded {
    pub settings: Settings,
    /// Set once, when the file existed but could not be used.
    pub note: Option<String>,
}

/// Reads the settings. A missing file gives defaults silently; an unreadable
/// one gives defaults plus a note, and is renamed aside (never deleted).
pub fn load(path: &Path) -> Loaded {
    let text = match read_bounded(path) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| ()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                settings: Settings::default(),
                note: None,
            };
        }
        Err(_) => Err(()),
    };
    match text.ok().and_then(|text| parse(&text).ok()) {
        Some(settings) => Loaded {
            settings,
            note: None,
        },
        None => {
            let backup = backup_path(path);
            let kept = std::fs::rename(path, &backup).is_ok();
            let note = if kept {
                format!(
                    "Settings file was unreadable; using defaults. The old file was kept as {}.",
                    backup
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                )
            } else {
                "Settings file was unreadable; using defaults.".to_owned()
            };
            Loaded {
                settings: Settings::default(),
                note: Some(note),
            }
        }
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| FILE_NAME.into());
    let mut candidate = path.with_file_name(format!("{name}.bad"));
    let mut n = 1;
    while candidate.exists() {
        candidate = path.with_file_name(format!("{name}.bad{n}"));
        n += 1;
    }
    candidate
}

/// Writes `settings` to a temporary file, syncs it, then replaces `path` with
/// `replace`. A failure before `replace` completes leaves the old file intact.
pub fn save_with(
    path: &Path,
    settings: &Settings,
    replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let temp = path.with_extension("tmp");
    let result = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(render(settings).as_bytes())?;
        file.sync_all()?;
        drop(file);
        replace(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    save_with(path, settings, |from, to| std::fs::rename(from, to))
}

/// One audience-capable monitor as reported by `sela --list-monitors`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub primary: bool,
}

impl MonitorInfo {
    pub fn to_line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            self.name.replace(['\t', '\n', '\r'], " "),
            self.width,
            self.height,
            self.x,
            self.y,
            u8::from(self.primary)
        )
    }

    pub fn from_line(line: &str) -> Option<Self> {
        let mut parts = line.trim_end().split('\t');
        let info = Self {
            name: parts.next()?.to_owned(),
            width: parts.next()?.parse().ok()?,
            height: parts.next()?.parse().ok()?,
            x: parts.next()?.parse().ok()?,
            y: parts.next()?.parse().ok()?,
            primary: parts.next()? == "1",
        };
        (parts.next().is_none() && !info.name.is_empty()).then_some(info)
    }

    /// `\\.\DISPLAY6` shown as `DISPLAY6`.
    pub fn short_name(&self) -> &str {
        self.name.rsplit('\\').next().unwrap_or(&self.name)
    }
}

/// The `--audience` monitor argument for a saved choice.
pub fn monitor_arg(saved: Option<&str>, monitors: Option<&[MonitorInfo]>) -> String {
    match (saved, monitors) {
        (Some(name), Some(list)) if !list.iter().any(|m| m.name == name) => "secondary".into(),
        (Some(name), _) => format!("name:{name}"),
        (None, _) => "secondary".into(),
    }
}

/// The `--stage` monitor argument: the saved stage monitor while it is
/// connected (or the list is unknown), else the first monitor that is neither
/// primary nor the audience's. `None` when there is no such monitor (or the
/// list is still unknown and nothing was chosen): the stage is then not
/// started, so a stage feed is never silently mapped over the audience
/// (M3-01). Choosing the audience or primary monitor explicitly is allowed.
pub fn stage_monitor_arg(
    saved: Option<&str>,
    audience: Option<&str>,
    monitors: Option<&[MonitorInfo]>,
) -> Option<String> {
    let Some(list) = monitors else {
        return saved.map(|name| format!("name:{name}"));
    };
    if let Some(name) = saved
        && list.iter().any(|m| m.name == name)
    {
        return Some(format!("name:{name}"));
    }
    // The audience monitor as `monitor_arg` resolves it.
    let audience = audience
        .filter(|name| list.iter().any(|m| m.name == *name))
        .or_else(|| list.iter().find(|m| !m.primary).map(|m| m.name.as_str()));
    list.iter()
        .find(|m| !m.primary && Some(m.name.as_str()) != audience)
        .map(|free| format!("name:{}", free.name))
}

/// True when a saved choice is known to name no connected monitor.
pub fn monitor_missing(saved: Option<&str>, monitors: Option<&[MonitorInfo]>) -> bool {
    match (saved, monitors) {
        (Some(name), Some(list)) => !list.iter().any(|m| m.name == name),
        _ => false,
    }
}

pub type Writer = Arc<dyn Fn(&Path, &Settings) -> std::io::Result<()> + Send + Sync>;

pub fn default_writer() -> Writer {
    Arc::new(save)
}

pub type Loader = Arc<dyn Fn(&Path) -> Loaded + Send + Sync>;

pub fn default_loader() -> Loader {
    Arc::new(load)
}

enum Job {
    Load,
    // Boxed: the settings grew past clippy's large-error bound once the
    // copyright and stage options were both added (integration).
    Save(Box<Settings>),
}

#[derive(Debug)]
pub enum Reply {
    Loaded(Loaded),
    Saved(Result<(), String>),
}

/// Owns the settings file. All reads and writes happen on its own thread;
/// requests go through a bounded queue and replies are polled.
pub struct Worker {
    jobs: Option<SyncSender<Job>>,
    replies: Receiver<Reply>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn start(path: PathBuf, writer: Writer) -> std::io::Result<Self> {
        Self::start_with(path, writer, default_loader())
    }

    pub fn start_with(path: PathBuf, writer: Writer, loader: Loader) -> std::io::Result<Self> {
        let (jobs, job_rx) = mpsc::sync_channel::<Job>(QUEUE);
        let (reply_tx, replies) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("sela-settings".into())
            .spawn(move || {
                for job in job_rx {
                    let reply = match job {
                        Job::Load => Reply::Loaded(loader(&path)),
                        Job::Save(settings) => Reply::Saved(
                            writer(&path, &settings).map_err(|error| error.to_string()),
                        ),
                    };
                    // A dropped receiver must not discard saves still queued.
                    let _ = reply_tx.send(reply);
                }
            })?;
        Ok(Self {
            jobs: Some(jobs),
            replies,
            thread: Some(thread),
        })
    }

    /// `false` when the queue is full; the caller reports it and may retry.
    pub fn load(&self) -> bool {
        self.submit(Job::Load)
    }

    pub fn save(&self, settings: Settings) -> bool {
        self.submit(Job::Save(Box::new(settings)))
    }

    fn submit(&self, job: Job) -> bool {
        match self.jobs.as_ref().map(|jobs| jobs.try_send(job)) {
            Some(Ok(())) => true,
            Some(Err(TrySendError::Full(_) | TrySendError::Disconnected(_))) | None => false,
        }
    }

    pub fn poll(&self) -> Option<Reply> {
        self.replies.try_recv().ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Closing the queue lets the thread finish every queued write and exit;
        // the handle is detached so a slow write never delays the UI thread.
        self.jobs = None;
        self.thread.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn chosen(name: &str) -> Settings {
        Settings {
            output_monitor: Some(name.into()),
            ..Settings::default()
        }
    }

    #[test]
    fn layout_round_trips_and_default_layout_is_not_written() {
        assert!(!render(&Settings::default()).contains("pane_ratios"));
        let settings = Settings {
            layout: Some(Layout {
                ratios: [2000, 5000, 7000],
                collapsed: true,
                window: Some(pane_layout::Placement {
                    x: 10,
                    y: 20,
                    width: 1300,
                    height: 810,
                    maximized: false,
                }),
            }),
            ..chosen(r"\\.\DISPLAY6")
        };
        assert_eq!(parse(&render(&settings)), Ok(settings));
    }

    #[test]
    fn corrupt_layout_keeps_other_settings_and_is_overwritten_by_the_next_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        std::fs::write(
            &path,
            "sela-settings 1\nlanguage=id\npane_ratios=[1,2\nwindow=a,b,c,d\n",
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.note, None, "no crash, no unreadable-file backup");
        assert_eq!(loaded.settings.layout, None);
        assert_eq!(loaded.settings.language, Language::Indonesian);
        assert_eq!(peek_layout(&path), None);
        let mut settings = loaded.settings;
        settings.layout = Some(Layout::default());
        save(&path, &settings).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("pane_ratios=2400,6200,6200") && !text.contains("[1,2"));
        assert_eq!(peek_layout(&path), Some(Layout::default()));
    }

    #[test]
    fn extreme_window_coordinates_fall_back_to_the_default_layout() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        for window in [
            "-2147483648,0,1280,800",
            "0,-2147483648,1280,800",
            "2147483647,2147483647,1280,800",
            "0,0,4294967295,4294967295",
        ] {
            std::fs::write(
                &path,
                format!(
                    "sela-settings 1\nlanguage=id\npane_ratios=2000,5000,7000\nwindow={window}\n"
                ),
            )
            .unwrap();
            assert_eq!(peek_layout(&path), None, "startup read of window={window}");
            let loaded = load(&path);
            assert_eq!(loaded.note, None, "window={window}");
            assert_eq!(loaded.settings.layout, None, "window={window}");
            assert_eq!(loaded.settings.language, Language::Indonesian);
        }
    }

    #[test]
    fn language_round_trips_and_defaults_to_english() {
        let mut settings = chosen(r"\\.\DISPLAY6");
        assert!(!render(&settings).contains("language="));
        settings.language = Language::Indonesian;
        let text = render(&settings);
        assert!(text.contains("language=id\n"));
        assert_eq!(parse(&text), Ok(settings));
        assert_eq!(
            parse("sela-settings 1\nlanguage=de\n"),
            Ok(Settings::default())
        );
    }

    #[test]
    fn song_order_round_trips_and_the_default_is_not_written() {
        let mut settings = Settings::default();
        assert!(!render(&settings).contains("song_sort"));
        settings.song_order = SongOrder {
            sort: SongSort::Modified,
            descending: true,
        };
        let text = render(&settings);
        assert!(text.contains("song_sort=modified\n") && text.contains("song_sort_descending=1\n"));
        assert_eq!(parse(&text), Ok(settings.clone()));
        assert_eq!(
            parse("sela-settings 1\nsong_sort=composer\n"),
            Ok(Settings::default())
        );
        for sort in SongSort::ALL {
            settings.song_order = SongOrder {
                sort,
                descending: false,
            };
            assert_eq!(parse(&render(&settings)), Ok(settings.clone()));
        }
        assert_eq!(
            parse("sela-settings 1\nsong_sort=song_number\n")
                .unwrap()
                .song_order
                .sort,
            SongSort::SongNumber
        );
    }

    #[test]
    fn copyright_options_round_trip_and_defaults_are_not_written() {
        let mut settings = Settings::default();
        let text = render(&settings);
        assert!(!text.contains("copyright") && !text.contains("license_info"));
        settings.copyright = CopyrightOptions {
            display: false,
            first_slide_only: true,
            license_info: "CCLI License # 11223344".into(),
        };
        let text = render(&settings);
        assert!(text.contains("copyright_display=0\n"));
        assert!(text.contains("copyright_first_slide=1\n"));
        assert!(text.contains("license_info=CCLI License # 11223344\n"));
        assert_eq!(parse(&text), Ok(settings.clone()));
        // A line break cannot inject another key; overlong text is capped.
        settings.copyright.license_info = "x\nlanguage=id".into();
        let parsed = parse(&render(&settings)).unwrap();
        assert_eq!(parsed.language, Language::English);
        assert_eq!(parsed.copyright.license_info, "x language=id");
        let long = format!("sela-settings 1\nlicense_info={}\n", "a".repeat(400));
        assert_eq!(
            parse(&long).unwrap().copyright.license_info.len(),
            crate::slides::MAX_LICENSE_INFO
        );
        // Unknown values keep the default.
        let odd = parse("sela-settings 1\ncopyright_display=maybe\ncopyright_first_slide=yes\n");
        assert_eq!(odd.unwrap().copyright, CopyrightOptions::default());
    }

    #[test]
    fn recent_schedules_keep_ten_newest_first_without_duplicates() {
        let mut settings = Settings::default();
        for n in 0..12 {
            settings.remember_recent(PathBuf::from(format!("C:/s/{n}.selaschedule")));
        }
        settings.remember_recent(PathBuf::from("C:/s/5.selaschedule"));
        assert_eq!(settings.recent_schedules.len(), MAX_RECENT);
        assert_eq!(
            settings.recent_schedules[..2],
            [
                PathBuf::from("C:/s/5.selaschedule"),
                PathBuf::from("C:/s/11.selaschedule")
            ]
        );
        assert!(
            !settings
                .recent_schedules
                .contains(&PathBuf::from("C:/s/0.selaschedule"))
        );
        assert_eq!(parse(&render(&settings)), Ok(settings.clone()));
        settings.forget_recent(Path::new("C:/s/5.selaschedule"));
        assert_eq!(settings.recent_schedules.len(), MAX_RECENT - 1);
    }

    #[test]
    fn version_one_files_without_recent_keys_load_and_extra_lines_are_capped() {
        let old = "sela-settings 1\noutput_monitor=X\nlanguage=id\n";
        let loaded = parse(old).unwrap();
        assert!(loaded.recent_schedules.is_empty());
        assert_eq!(loaded.output_monitor.as_deref(), Some("X"));
        let mut many = String::from("sela-settings 1\n");
        for n in 0..30 {
            many.push_str(&format!("recent=C:/s/{}.selaschedule\n", n % 15));
        }
        let loaded = parse(&many).unwrap();
        assert_eq!(loaded.recent_schedules.len(), MAX_RECENT);
        assert_eq!(
            loaded.recent_schedules[0],
            PathBuf::from("C:/s/0.selaschedule")
        );
    }

    #[test]
    fn round_trips_and_ignores_unknown_keys() {
        let settings = chosen(r"\\.\DISPLAY6");
        assert_eq!(parse(&render(&settings)), Ok(settings));
        assert_eq!(
            parse("sela-settings 1\nfuture=1\n"),
            Ok(Settings::default())
        );
    }

    #[test]
    fn rejects_bad_headers_versions_and_lines() {
        assert_eq!(parse(""), Err(ParseError::Empty));
        assert_eq!(parse("hello\n"), Err(ParseError::Header));
        assert_eq!(parse("sela-settings 2\n"), Err(ParseError::Version(2)));
        assert_eq!(
            parse("sela-settings 1\nnonsense\n"),
            Err(ParseError::Line(2))
        );
    }

    #[test]
    fn missing_file_gives_defaults_without_a_note() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load(&path_in(dir.path()));
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.note, None);
    }

    #[test]
    fn corrupt_file_gives_defaults_a_note_and_a_kept_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        std::fs::write(&path, "sela-settings 1\noutput_mon").unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.note.is_some());
        assert!(!path.exists());
        let backup = dir.path().join("settings.txt.bad");
        assert_eq!(
            std::fs::read_to_string(backup).unwrap(),
            "sela-settings 1\noutput_mon"
        );
        // The problem is reported once: the next load sees no file.
        assert_eq!(load(&path).note, None);
    }

    #[test]
    fn binary_garbage_is_treated_as_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        std::fs::write(&path, [0xff, 0xfe, 0x00, 0x9f]).unwrap();
        assert!(load(&path).note.is_some());
    }

    #[test]
    fn save_interrupted_before_rename_keeps_the_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        save(&path, &chosen("A")).unwrap();
        let result = save_with(&path, &chosen("B"), |_, _| {
            Err(std::io::Error::other("power loss"))
        });
        assert!(result.is_err());
        assert_eq!(load(&path).settings, chosen("A"));
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn save_replaces_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        save(&path, &chosen("A")).unwrap();
        save(&path, &chosen("B")).unwrap();
        assert_eq!(load(&path).settings, chosen("B"));
    }

    #[test]
    fn worker_save_returns_immediately_while_the_writer_is_slow() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let writer: Writer = Arc::new(|path, settings| {
            std::thread::sleep(Duration::from_millis(600));
            save(path, settings)
        });
        let worker = Worker::start(path.clone(), writer).unwrap();
        let started = Instant::now();
        assert!(worker.save(chosen("A")));
        assert!(started.elapsed() < Duration::from_millis(200));
        assert!(worker.poll().is_none());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(reply) = worker.poll() {
                assert!(matches!(reply, Reply::Saved(Ok(()))));
                break;
            }
            assert!(Instant::now() < deadline, "no reply");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(load(&path).settings, chosen("A"));
    }

    #[test]
    fn worker_drains_every_queued_save_after_the_worker_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let writer: Writer = Arc::new(|path, settings| {
            std::thread::sleep(Duration::from_millis(200));
            save(path, settings)
        });
        let worker = Worker::start(path.clone(), writer).unwrap();
        assert!(worker.save(chosen("A")));
        assert!(worker.save(chosen("B")));
        drop(worker);
        let deadline = Instant::now() + Duration::from_secs(5);
        while load(&path).settings != chosen("B") {
            assert!(Instant::now() < deadline, "second save was dropped");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn worker_loads_off_thread_and_reports_write_failures() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        std::fs::write(&path, render(&chosen("X"))).unwrap();
        let writer: Writer = Arc::new(|_, _| Err(std::io::Error::other("disk full")));
        let worker = Worker::start(path, writer).unwrap();
        assert!(worker.load());
        assert!(worker.save(chosen("Y")));
        let mut replies = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while replies.len() < 2 {
            match worker.poll() {
                Some(reply) => replies.push(reply),
                None => {
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        assert!(matches!(&replies[0], Reply::Loaded(l) if l.settings == chosen("X")));
        assert!(matches!(&replies[1], Reply::Saved(Err(e)) if e.contains("disk full")));
    }

    #[test]
    fn monitor_lines_round_trip() {
        let info = MonitorInfo {
            name: r"\\.\DISPLAY6".into(),
            width: 1920,
            height: 1080,
            x: -1920,
            y: 0,
            primary: false,
        };
        assert_eq!(MonitorInfo::from_line(&info.to_line()), Some(info.clone()));
        assert_eq!(info.short_name(), "DISPLAY6");
        assert_eq!(MonitorInfo::from_line("junk"), None);
    }

    #[test]
    fn stage_settings_round_trip_and_defaults_are_not_written() {
        let mut settings = Settings::default();
        assert!(settings.stage.clock && settings.stage.next && !settings.stage.enabled);
        assert!(!render(&settings).contains("stage_"));
        settings.stage = StageSettings {
            enabled: true,
            monitor: Some(r"\\.\DISPLAY7".into()),
            clock: false,
            next: false,
        };
        let text = render(&settings);
        assert!(text.contains("stage_enabled=1\n") && text.contains("stage_clock=0\n"));
        assert_eq!(parse(&text), Ok(settings));
        // A file from before the stage keys loads with the defaults.
        assert_eq!(
            parse("sela-settings 1\noutput_monitor=X\n").unwrap().stage,
            StageSettings::default()
        );
    }

    #[test]
    fn stage_monitor_never_defaults_onto_the_audience_monitor() {
        let info = |name: &str, primary| MonitorInfo {
            name: name.into(),
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            primary,
        };
        let three = [info("P", true), info("A", false), info("S", false)];
        // Default audience is the first non-primary monitor (A); the stage
        // takes the next free one.
        let arg = |saved, audience, list| stage_monitor_arg(saved, audience, list);
        assert_eq!(arg(None, None, Some(&three[..])).as_deref(), Some("name:S"));
        assert_eq!(
            arg(None, Some("S"), Some(&three[..])).as_deref(),
            Some("name:A")
        );
        assert_eq!(
            arg(Some("P"), None, Some(&three[..])).as_deref(),
            Some("name:P")
        );
        // A saved stage monitor that is gone falls back to a free one.
        assert_eq!(
            arg(Some("Z"), Some("A"), Some(&three[..])).as_deref(),
            Some("name:S")
        );
        // Two monitors: no free one, so no stage rather than the audience's.
        let two = [info("P", true), info("A", false)];
        assert_eq!(arg(None, None, Some(&two[..])), None);
        assert_eq!(arg(Some("Z"), None, Some(&two[..])), None);
        // An explicit choice of the audience monitor is honoured.
        assert_eq!(
            arg(Some("A"), None, Some(&two[..])).as_deref(),
            Some("name:A")
        );
        assert_eq!(arg(Some("Q"), None, None).as_deref(), Some("name:Q"));
        assert_eq!(arg(None, None, None), None);
    }

    #[test]
    fn monitor_arg_falls_back_to_secondary_when_saved_monitor_is_gone() {
        let list = [MonitorInfo {
            name: "A".into(),
            width: 1,
            height: 1,
            x: 0,
            y: 0,
            primary: false,
        }];
        assert_eq!(monitor_arg(Some("A"), Some(&list)), "name:A");
        assert_eq!(monitor_arg(Some("B"), Some(&list)), "secondary");
        assert_eq!(monitor_arg(Some("B"), None), "name:B");
        assert_eq!(monitor_arg(None, Some(&list)), "secondary");
        assert!(monitor_missing(Some("B"), Some(&list)));
        assert!(!monitor_missing(Some("B"), None));
        assert!(!monitor_missing(Some("A"), Some(&list)));
    }

    // M5-02 fuzz finding: a bare CR (old Mac line ends, an editor's stray
    // CR) stayed inside a recent path, which then never opened, and saving
    // silently rewrote it into another path.
    #[test]
    fn a_bare_cr_ends_a_line_and_never_joins_a_path() {
        let parsed = parse("sela-settings 1\rlanguage=id\rrecent=C:/s/b.selaschedule\r").unwrap();
        assert_eq!(parsed.language, Language::Indonesian);
        assert_eq!(
            parsed.recent_schedules,
            [PathBuf::from("C:/s/b.selaschedule")]
        );
        let parsed = parse("sela-settings 1\r\nrecent=C:/a\rrecent=C:/b\r\n").unwrap();
        assert_eq!(
            parsed.recent_schedules,
            [PathBuf::from("C:/a"), PathBuf::from("C:/b")]
        );
        assert_eq!(parse(&render(&parsed)), Ok(parsed));
    }

    // M5-02 fuzz finding: `render` stripped control characters, so loading
    // and saving changed a path or monitor name into a different one.
    #[test]
    fn values_with_control_characters_are_left_out_not_rewritten() {
        let mut settings = Settings {
            output_monitor: Some("DISPLAY\u{1}6".into()),
            ..Settings::default()
        };
        settings.remember_recent(PathBuf::from("C:/s/kept.selaschedule"));
        settings.remember_recent(PathBuf::from("C:/s/a\tb.selaschedule"));
        let text = render(&settings);
        assert!(!text.contains("output_monitor"), "{text:?}");
        assert!(!text.contains("ab.selaschedule"), "{text:?}");
        let loaded = parse(&text).unwrap();
        assert_eq!(loaded.output_monitor, None);
        assert_eq!(
            loaded.recent_schedules,
            [PathBuf::from("C:/s/kept.selaschedule")]
        );
        assert_eq!(
            parse("sela-settings 1\noutput_monitor=X\u{7f}\nrecent=a\u{85}b\n").unwrap(),
            Settings::default()
        );
    }

    // M5-02 audit finding: the whole file was read into memory, so a damaged
    // multi-gigabyte settings.txt stalled or exhausted startup.
    #[test]
    fn an_oversized_file_is_unreadable_and_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let mut text = String::from("sela-settings 1\nlanguage=id\npane_ratios=2000,5000,7000\n");
        text.push_str(&"\n".repeat(MAX_FILE_BYTES as usize));
        std::fs::write(&path, &text).unwrap();
        assert_eq!(peek_layout(&path), None);
        let loaded = load(&path);
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.note.is_some());
        assert!(dir.path().join("settings.txt.bad").exists());
        // Exactly at the bound still loads.
        std::fs::write(&path, &text[..MAX_FILE_BYTES as usize]).unwrap();
        assert!(peek_layout(&path).is_some());
        assert_eq!(load(&path).settings.language, Language::Indonesian);
    }
}
