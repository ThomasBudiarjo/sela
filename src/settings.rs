//! Per-profile settings file: versioned text, atomic replace, and a worker
//! thread so the operator never touches the disk on the UI thread.
//!
//! Format: first line `sela-settings <version>`, then `key=value` lines.
//! Unknown keys are ignored so a newer build's file still loads; an unknown
//! (newer) version is treated as unreadable rather than guessed at.

use crate::i18n::Language;
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// Device name of the audience output monitor (for example `\\.\DISPLAY6`);
    /// `None` means the default non-primary monitor.
    pub output_monitor: Option<String>,
    /// Operator UI language; the default (English) is not written to the file.
    pub language: Language,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    Header,
    Version(u32),
    Line(usize),
}

pub fn parse(text: &str) -> Result<Settings, ParseError> {
    let mut lines = text.lines();
    let header = lines.next().ok_or(ParseError::Empty)?;
    let version = header
        .strip_prefix(HEADER)
        .and_then(|rest| rest.trim().parse::<u32>().ok())
        .ok_or(ParseError::Header)?;
    if version == 0 || version > VERSION {
        return Err(ParseError::Version(version));
    }
    let mut settings = Settings::default();
    for (index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or(ParseError::Line(index + 2))?;
        if key == "output_monitor" && !value.is_empty() {
            settings.output_monitor = Some(value.to_owned());
        } else if key == "language" {
            // Unknown codes are ignored like unknown keys: English applies.
            settings.language = Language::from_code(value).unwrap_or_default();
        }
    }
    Ok(settings)
}

pub fn render(settings: &Settings) -> String {
    let mut text = format!("{HEADER} {VERSION}\n");
    if let Some(name) = &settings.output_monitor {
        // Names never contain line breaks; strip them so one cannot corrupt the file.
        let name: String = name.chars().filter(|c| !c.is_control()).collect();
        text.push_str(&format!("output_monitor={name}\n"));
    }
    if settings.language != Language::default() {
        text.push_str(&format!("language={}\n", settings.language.code()));
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
    let text = match std::fs::read(path) {
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
    Save(Settings),
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
        self.submit(Job::Save(settings))
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
}
