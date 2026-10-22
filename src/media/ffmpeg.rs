//! FFmpeg location, build-license check and decoder input arguments (M2-01).
//! Policy and decisions: docs/media.md.
//!
//! The lookup order lives in [`candidates`] and the refusal rules in
//! [`locate_with`] and [`check_buildconf`], which take everything they need as
//! arguments. [`locate`] is the thin shell that reads the real environment and
//! runs the real FFmpeg. Nothing calls it yet.

use std::env::consts::EXE_SUFFIX;
use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// Environment variable naming a directory that holds `ffmpeg` and `ffprobe`.
pub const ENV_DIR: &str = "SELA_FFMPEG_DIR";
/// Folder name of the pinned LGPL build under `.amp/in/ffmpeg/` in a
/// development checkout.
pub const PINNED_ID: &str = "n8.1.3-lgpl";
/// Bound on each probe command, from spawn to exit.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// Bound on the stdout a probe command may print. The pinned build's
/// `-buildconf` output is about 1 KiB.
pub const PROBE_OUTPUT_LIMIT: usize = 64 * 1024;

/// Where a candidate directory came from, in lookup order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// [`ENV_DIR`].
    Env,
    /// `<exe dir>/ffmpeg/bin`, the installed layout.
    Install,
    /// `.amp/in/ffmpeg/<PINNED_ID>/bin` in the source checkout; debug builds only.
    Dev,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub source: Source,
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Ffmpeg,
    Ffprobe,
}

impl Tool {
    pub fn file_name(self) -> String {
        let stem = match self {
            Tool::Ffmpeg => "ffmpeg",
            Tool::Ffprobe => "ffprobe",
        };
        format!("{stem}{EXE_SUFFIX}")
    }
}

/// What `ffmpeg -hide_banner -buildconf` and `ffmpeg -hide_banner -version`
/// printed on stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutput {
    pub buildconf: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    /// The child could not start, could not be read or exited unsuccessfully.
    Failed(String),
    /// The child printed more than [`PROBE_OUTPUT_LIMIT`] bytes and was killed.
    OutputTooLarge,
    /// The child did not finish within [`PROBE_TIMEOUT`] and was killed.
    TimedOut,
}

/// Why a candidate directory was passed over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The directory is relative, so what it names would depend on the
    /// directory Sela was started from.
    RelativeDir,
    Missing(Tool),
    /// Built with `--enable-gpl`.
    Gpl,
    /// Built with `--enable-nonfree`, which also makes it non-redistributable.
    Nonfree,
    /// The probe ran but printed no build configuration or no version line.
    Unrecognized,
    Probe(ProbeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tried {
    pub candidate: Candidate,
    pub refusal: Refusal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub source: Source,
    pub dir: PathBuf,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    /// The token after `ffmpeg version`, e.g. `n8.1.3-9-g29e619e767`.
    pub version: String,
    /// Earlier candidates and why each was passed over, so a refused
    /// `SELA_FFMPEG_DIR` is still reported when a later candidate is used.
    pub refused: Vec<Tried>,
}

/// Every candidate tried, in lookup order, with its refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotFound {
    pub tried: Vec<Tried>,
}

/// Lookup order: `env_dir` ([`ENV_DIR`], skipped when empty), then
/// `<exe_dir>/ffmpeg/bin`, then `dev_dir`. PATH is never consulted: it could
/// hold a GPL build that differs from the one Sela ships. A relative
/// `env_dir` stays a candidate so that [`locate_with`] reports its refusal.
pub fn candidates(
    env_dir: Option<&Path>,
    exe_dir: Option<&Path>,
    dev_dir: Option<&Path>,
) -> Vec<Candidate> {
    let env = env_dir
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| (Source::Env, dir.to_path_buf()));
    let install = exe_dir.map(|dir| (Source::Install, dir.join("ffmpeg").join("bin")));
    let dev = dev_dir.map(|dir| (Source::Dev, dir.to_path_buf()));
    [env, install, dev]
        .into_iter()
        .flatten()
        .map(|(source, dir)| Candidate { source, dir })
        .collect()
}

/// The development copy's `bin` directory under this crate's checkout, or
/// `None` in release builds. The checkout comes from the build, not from the
/// executable's location, because `CARGO_TARGET_DIR` may be outside it.
pub fn dev_dir() -> Option<PathBuf> {
    cfg!(debug_assertions).then(|| {
        [".amp", "in", "ffmpeg", PINNED_ID, "bin"]
            .iter()
            .fold(PathBuf::from(env!("CARGO_MANIFEST_DIR")), |dir, part| {
                dir.join(part)
            })
    })
}

/// Returns the first candidate whose directory is absolute, holds both
/// executables and whose `ffmpeg` passes [`check_buildconf`] and reports a
/// version. `probe` runs the `ffmpeg` executable it is given.
pub fn locate_with(
    candidates: Vec<Candidate>,
    mut probe: impl FnMut(&Path) -> Result<ProbeOutput, ProbeError>,
) -> Result<Located, NotFound> {
    let mut refused = Vec::new();
    for candidate in candidates {
        let ffmpeg = candidate.dir.join(Tool::Ffmpeg.file_name());
        let ffprobe = candidate.dir.join(Tool::Ffprobe.file_name());
        let checked = if !candidate.dir.is_absolute() {
            Err(Refusal::RelativeDir)
        } else if !ffmpeg.is_file() {
            Err(Refusal::Missing(Tool::Ffmpeg))
        } else if !ffprobe.is_file() {
            Err(Refusal::Missing(Tool::Ffprobe))
        } else {
            probe(&ffmpeg).map_err(Refusal::Probe).and_then(|output| {
                check_buildconf(&output.buildconf)?;
                version(&output.version).ok_or(Refusal::Unrecognized)
            })
        };
        match checked {
            Ok(version) => {
                return Ok(Located {
                    source: candidate.source,
                    dir: candidate.dir,
                    ffmpeg,
                    ffprobe,
                    version,
                    refused,
                });
            }
            Err(refusal) => refused.push(Tried { candidate, refusal }),
        }
    }
    Err(NotFound { tried: refused })
}

/// Accepts an LGPL build's `-buildconf` (or `-version`) output and refuses
/// `--enable-nonfree` before `--enable-gpl`, since nonfree is the stronger
/// restriction. Output without a `configuration:` section is not FFmpeg's.
pub fn check_buildconf(output: &str) -> Result<(), Refusal> {
    let flags: Vec<&str> = output.split_whitespace().collect();
    if !flags.contains(&"configuration:") {
        Err(Refusal::Unrecognized)
    } else if flags.contains(&"--enable-nonfree") {
        Err(Refusal::Nonfree)
    } else if flags.contains(&"--enable-gpl") {
        Err(Refusal::Gpl)
    } else {
        Ok(())
    }
}

fn version(output: &str) -> Option<String> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("ffmpeg version "))
        .and_then(|rest| rest.split_whitespace().next())
        .map(str::to_owned)
}

/// Finds FFmpeg from the real environment and probes each candidate with up
/// to two child processes, each bounded by [`PROBE_TIMEOUT`]. It blocks for
/// that long, so callers must run it on a worker thread, never on the UI
/// thread or the live frame path. It is not wired into the app yet.
pub fn locate() -> Result<Located, NotFound> {
    let env_dir = std::env::var_os(ENV_DIR).map(PathBuf::from);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let dev_dir = dev_dir();
    locate_with(
        candidates(env_dir.as_deref(), exe_dir.as_deref(), dev_dir.as_deref()),
        probe,
    )
}

fn probe(ffmpeg: &Path) -> Result<ProbeOutput, ProbeError> {
    Ok(ProbeOutput {
        buildconf: run(ffmpeg, &["-hide_banner", "-buildconf"], PROBE_TIMEOUT)?,
        version: run(ffmpeg, &["-hide_banner", "-version"], PROBE_TIMEOUT)?,
    })
}

fn run(program: &Path, args: &[&str], timeout: Duration) -> Result<String, ProbeError> {
    let deadline = Instant::now() + timeout;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|error| ProbeError::Failed(format!("could not start: {error}")))?;
    let stdout = child.stdout.take().expect("stdout is piped");
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = stdout
            .take(PROBE_OUTPUT_LIMIT as u64 + 1)
            .read_to_end(&mut bytes);
        let _ = sender.send(read.map(|_| bytes));
    });
    let result = match receiver.recv_timeout(timeout) {
        Err(RecvTimeoutError::Timeout) => Err(ProbeError::TimedOut),
        Err(RecvTimeoutError::Disconnected) => {
            Err(ProbeError::Failed("output reader stopped".into()))
        }
        Ok(Err(error)) => Err(ProbeError::Failed(format!("reading output: {error}"))),
        Ok(Ok(bytes)) if bytes.len() > PROBE_OUTPUT_LIMIT => Err(ProbeError::OutputTooLarge),
        Ok(Ok(bytes)) => match wait_until(&mut child, deadline) {
            Some(status) if status.success() => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Some(status) => Err(ProbeError::Failed(format!("exited with {status}"))),
            None => Err(ProbeError::TimedOut),
        },
    };
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    result
}

fn wait_until(child: &mut Child, deadline: Instant) -> Option<ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(None) | Err(_) => return None,
        }
    }
}

/// Why a path was refused as decoder input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputRefused {
    /// Starts with a URL scheme or FFmpeg protocol prefix (`http:`, `file:`,
    /// `concat:`, ...).
    Url,
    Relative,
    /// A UNC share (`\\server\share`, `\\?\UNC\server\share`).
    Network,
    /// A device or volume path (`\\.\...`, `\\?\Volume{...}`, `\\?\GLOBALROOT`).
    Device,
}

/// The leading `ffmpeg` arguments for decoding `input`, ending in `-i
/// <input>`; callers append output options. Only absolute local file paths
/// are accepted, and `-protocol_whitelist file` stops a playlist inside the
/// file from opening anything but local files. These are `ffmpeg` arguments:
/// `ffprobe` has no `-nostdin`.
pub fn input_args(input: &Path) -> Result<Vec<OsString>, InputRefused> {
    check_input(input)?;
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-hide_banner",
        "-protocol_whitelist",
        "file",
        "-i",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.push(input.as_os_str().to_owned());
    Ok(args)
}

fn check_input(input: &Path) -> Result<(), InputRefused> {
    if has_scheme(input.as_os_str().as_encoded_bytes()) {
        return Err(InputRefused::Url);
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        if let Some(Component::Prefix(prefix)) = input.components().next() {
            match prefix.kind() {
                Prefix::Disk(_) | Prefix::VerbatimDisk(_) => {}
                Prefix::UNC(..) | Prefix::VerbatimUNC(..) => return Err(InputRefused::Network),
                Prefix::DeviceNS(_) | Prefix::Verbatim(_) => return Err(InputRefused::Device),
            }
        }
    }
    if input.is_absolute() {
        Ok(())
    } else {
        Err(InputRefused::Relative)
    }
}

/// FFmpeg's own test (`url_find_protocol`): scheme characters followed by
/// `:`. A single letter is a Windows drive, which FFmpeg also reads as a file.
fn has_scheme(path: &[u8]) -> bool {
    let len = path
        .iter()
        .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
        .count();
    len >= 2 && path.get(len) == Some(&b':')
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Source::Env => ENV_DIR,
            Source::Install => "install folder",
            Source::Dev => "development copy",
        })
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::RelativeDir => f.write_str("the folder is not an absolute path"),
            Refusal::Missing(tool) => write!(f, "{} is missing", tool.file_name()),
            Refusal::Gpl => f.write_str("the build is configured with --enable-gpl"),
            Refusal::Nonfree => f.write_str("the build is configured with --enable-nonfree"),
            Refusal::Unrecognized => {
                f.write_str("it did not report an FFmpeg build configuration and version")
            }
            Refusal::Probe(ProbeError::Failed(reason)) => write!(f, "ffmpeg {reason}"),
            Refusal::Probe(ProbeError::OutputTooLarge) => write!(
                f,
                "ffmpeg printed more than {} KiB",
                PROBE_OUTPUT_LIMIT / 1024
            ),
            Refusal::Probe(ProbeError::TimedOut) => write!(
                f,
                "ffmpeg did not finish within {} s",
                PROBE_TIMEOUT.as_secs()
            ),
        }
    }
}

impl fmt::Display for NotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("no usable FFmpeg found")?;
        for tried in &self.tried {
            let Candidate { source, dir } = &tried.candidate;
            write!(f, "; {source} {}: {}", dir.display(), tried.refusal)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const LGPL_BUILDCONF: &str = "  configuration:
    --prefix=/ffbuild/prefix
    --pkg-config=pkg-config
    --pkg-config-flags=--static
    --cross-prefix=x86_64-w64-mingw32-
    --arch=x86_64
    --target-os=mingw32
    --enable-version3
    --disable-debug
    --enable-shared
    --disable-static
    --enable-iconv
    --enable-zlib
    --enable-libxml2
    --enable-fontconfig
    --enable-libharfbuzz
    --enable-libfreetype
    --enable-libvpl
    --enable-libdav1d
    --enable-libaom
    --enable-libvpx
    --enable-libopus
    --enable-libopenh264
    --enable-amf
    --enable-ffnvcodec
    --disable-libx264
    --disable-libx265
    --disable-libxvid
    --disable-libvidstab
    --disable-librubberband
    --disable-libfdk-aac
    --extra-version=20260930
";
    const LGPL_VERSION: &str = "ffmpeg version n8.1.3-9-g29e619e767 Copyright (c) 2000-2026 the FFmpeg developers
built with gcc 15.2.0 (crosstool-NG 1.28.0)
configuration: --prefix=/ffbuild/prefix --enable-version3 --enable-shared --disable-libx264 --disable-libx265
libavutil      60. 26.103 / 60. 26.103
libavcodec     62. 28.103 / 62. 28.103
libavformat    62. 12.103 / 62. 12.103
";
    const GPL_BUILDCONF: &str = "  configuration:
    --enable-gpl
    --enable-version3
    --enable-static
    --disable-w32threads
    --disable-autodetect
    --enable-fontconfig
    --enable-iconv
    --enable-gnutls
    --enable-libx264
    --enable-libx265
    --enable-libxvid
";
    const NONFREE_BUILDCONF: &str = "  configuration:
    --enable-version3
    --enable-shared
    --enable-nonfree
    --enable-libfdk-aac
";

    fn lgpl(_: &Path) -> Result<ProbeOutput, ProbeError> {
        Ok(ProbeOutput {
            buildconf: LGPL_BUILDCONF.into(),
            version: LGPL_VERSION.into(),
        })
    }

    fn with_buildconf(buildconf: &str) -> Result<ProbeOutput, ProbeError> {
        Ok(ProbeOutput {
            buildconf: buildconf.into(),
            version: LGPL_VERSION.into(),
        })
    }

    fn put(dir: &Path, tools: &[Tool]) {
        fs::create_dir_all(dir).unwrap();
        for tool in tools {
            fs::write(dir.join(tool.file_name()), "not a real executable").unwrap();
        }
    }

    fn pair(dir: &Path) {
        put(dir, &[Tool::Ffmpeg, Tool::Ffprobe]);
    }

    struct Layout {
        _root: tempfile::TempDir,
        env: PathBuf,
        exe: PathBuf,
        install: PathBuf,
        dev: PathBuf,
    }

    fn layout() -> Layout {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("Sela");
        fs::create_dir_all(&exe).unwrap();
        Layout {
            env: root.path().join("env"),
            install: exe.join("ffmpeg").join("bin"),
            exe,
            dev: root
                .path()
                .join("checkout/.amp/in/ffmpeg")
                .join(PINNED_ID)
                .join("bin"),
            _root: root,
        }
    }

    impl Layout {
        fn candidates(&self, env_set: bool) -> Vec<Candidate> {
            candidates(
                env_set.then_some(self.env.as_path()),
                Some(&self.exe),
                Some(&self.dev),
            )
        }
    }

    fn tried(source: Source, dir: &Path, refusal: Refusal) -> Tried {
        Tried {
            candidate: Candidate {
                source,
                dir: dir.to_path_buf(),
            },
            refusal,
        }
    }

    #[test]
    fn candidates_are_the_env_dir_then_the_install_dir_then_the_dev_copy() {
        let found = candidates(
            Some(Path::new("/tools/ffmpeg")),
            Some(Path::new("/apps/Sela")),
            Some(Path::new("/src/sela/dev")),
        );
        assert_eq!(
            found,
            [
                Candidate {
                    source: Source::Env,
                    dir: PathBuf::from("/tools/ffmpeg"),
                },
                Candidate {
                    source: Source::Install,
                    dir: Path::new("/apps/Sela").join("ffmpeg").join("bin"),
                },
                Candidate {
                    source: Source::Dev,
                    dir: PathBuf::from("/src/sela/dev"),
                },
            ]
        );
    }

    #[test]
    fn an_unset_or_empty_env_dir_is_not_a_candidate() {
        for env in [None, Some(Path::new(""))] {
            let sources: Vec<Source> = candidates(env, Some(Path::new("/apps/Sela")), None)
                .into_iter()
                .map(|candidate| candidate.source)
                .collect();
            assert_eq!(sources, [Source::Install]);
        }
    }

    #[test]
    fn the_dev_copy_is_the_pinned_lgpl_folder_of_this_checkout_in_debug_builds_only() {
        let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".amp")
            .join("in")
            .join("ffmpeg")
            .join("n8.1.3-lgpl")
            .join("bin");
        assert_eq!(dev_dir(), cfg!(debug_assertions).then_some(expected));
    }

    #[test]
    fn a_relative_env_dir_is_refused_without_probing() {
        let l = layout();
        pair(&l.install);
        let relative = Path::new("ffmpeg").join("bin");
        let located = locate_with(candidates(Some(&relative), Some(&l.exe), None), |ffmpeg| {
            assert!(ffmpeg.starts_with(&l.install), "probed {}", ffmpeg.display());
            lgpl(ffmpeg)
        })
        .unwrap();
        assert_eq!(located.source, Source::Install);
        assert_eq!(
            located.refused,
            [tried(Source::Env, &relative, Refusal::RelativeDir)]
        );
        assert!(Refusal::RelativeDir.to_string().contains("absolute"));
    }

    #[test]
    fn the_env_dir_wins_when_every_location_has_a_pair() {
        let l = layout();
        for dir in [&l.env, &l.install, &l.dev] {
            pair(dir);
        }
        let located = locate_with(l.candidates(true), lgpl).unwrap();
        assert_eq!(located.source, Source::Env);
        assert_eq!(located.dir, l.env);
        assert_eq!(located.ffmpeg, l.env.join(Tool::Ffmpeg.file_name()));
        assert_eq!(located.ffprobe, l.env.join(Tool::Ffprobe.file_name()));
        assert_eq!(located.refused, []);
    }

    #[test]
    fn the_install_dir_wins_over_the_dev_copy_without_an_env_dir() {
        let l = layout();
        for dir in [&l.env, &l.install, &l.dev] {
            pair(dir);
        }
        let located = locate_with(l.candidates(false), lgpl).unwrap();
        assert_eq!((located.source, located.dir), (Source::Install, l.install));
    }

    #[test]
    fn the_dev_copy_is_used_when_it_holds_the_only_pair() {
        let l = layout();
        pair(&l.dev);
        let located = locate_with(l.candidates(true), lgpl).unwrap();
        assert_eq!((located.source, &located.dir), (Source::Dev, &l.dev));
        assert_eq!(
            located.refused,
            [
                tried(Source::Env, &l.env, Refusal::Missing(Tool::Ffmpeg)),
                tried(Source::Install, &l.install, Refusal::Missing(Tool::Ffmpeg)),
            ]
        );
    }

    #[test]
    fn an_env_dir_without_ffprobe_is_passed_over_for_the_install_dir() {
        let l = layout();
        put(&l.env, &[Tool::Ffmpeg]);
        pair(&l.install);
        let located = locate_with(l.candidates(true), lgpl).unwrap();
        assert_eq!(located.source, Source::Install);
        assert_eq!(
            located.refused,
            [tried(Source::Env, &l.env, Refusal::Missing(Tool::Ffprobe))]
        );
    }

    #[test]
    fn the_pinned_lgpl_build_is_accepted_with_its_version() {
        assert_eq!(check_buildconf(LGPL_BUILDCONF), Ok(()));
        let l = layout();
        pair(&l.install);
        let located = locate_with(l.candidates(false), lgpl).unwrap();
        assert_eq!(located.version, "n8.1.3-9-g29e619e767");
    }

    #[test]
    fn a_gpl_build_is_refused_naming_the_flag() {
        assert_eq!(check_buildconf(GPL_BUILDCONF), Err(Refusal::Gpl));
        assert!(Refusal::Gpl.to_string().contains("--enable-gpl"));
    }

    #[test]
    fn a_nonfree_build_is_refused_naming_the_flag() {
        assert_eq!(check_buildconf(NONFREE_BUILDCONF), Err(Refusal::Nonfree));
        assert!(Refusal::Nonfree.to_string().contains("--enable-nonfree"));
        let both = format!("{GPL_BUILDCONF}    --enable-nonfree\n");
        assert_eq!(check_buildconf(&both), Err(Refusal::Nonfree));
    }

    #[test]
    fn the_one_line_configuration_of_version_output_is_checked_too() {
        let gpl_version =
            LGPL_VERSION.replace("--enable-version3", "--enable-gpl --enable-version3");
        assert_eq!(check_buildconf(LGPL_VERSION), Ok(()));
        assert_eq!(check_buildconf(&gpl_version), Err(Refusal::Gpl));
    }

    #[test]
    fn output_without_a_configuration_is_unrecognized() {
        assert_eq!(check_buildconf(""), Err(Refusal::Unrecognized));
        assert_eq!(
            check_buildconf("Usage: not-ffmpeg [options]\n  --enable-version3\n"),
            Err(Refusal::Unrecognized)
        );
    }

    #[test]
    fn a_gpl_env_build_is_refused_and_the_install_build_is_used() {
        let l = layout();
        pair(&l.env);
        pair(&l.install);
        let env = l.env.clone();
        let located = locate_with(l.candidates(true), |ffmpeg| {
            if ffmpeg.starts_with(&env) {
                with_buildconf(GPL_BUILDCONF)
            } else {
                lgpl(ffmpeg)
            }
        })
        .unwrap();
        assert_eq!(located.source, Source::Install);
        assert_eq!(located.version, "n8.1.3-9-g29e619e767");
        assert_eq!(located.refused, [tried(Source::Env, &l.env, Refusal::Gpl)]);
    }

    #[test]
    fn a_nonfree_install_build_is_refused_and_the_dev_copy_is_used() {
        let l = layout();
        pair(&l.install);
        pair(&l.dev);
        let install = l.install.clone();
        let located = locate_with(l.candidates(false), |ffmpeg| {
            if ffmpeg.starts_with(&install) {
                with_buildconf(NONFREE_BUILDCONF)
            } else {
                lgpl(ffmpeg)
            }
        })
        .unwrap();
        assert_eq!(located.source, Source::Dev);
        assert_eq!(
            located.refused,
            [tried(Source::Install, &l.install, Refusal::Nonfree)]
        );
    }

    #[test]
    fn a_probe_that_times_out_or_fails_moves_on_to_the_next_candidate() {
        let l = layout();
        for dir in [&l.env, &l.install, &l.dev] {
            pair(dir);
        }
        let (env, install) = (l.env.clone(), l.install.clone());
        let located = locate_with(l.candidates(true), |ffmpeg| {
            if ffmpeg.starts_with(&env) {
                Err(ProbeError::TimedOut)
            } else if ffmpeg.starts_with(&install) {
                Err(ProbeError::Failed("exited with exit code: 1".into()))
            } else {
                lgpl(ffmpeg)
            }
        })
        .unwrap();
        assert_eq!(located.source, Source::Dev);
        assert_eq!(
            located.refused,
            [
                tried(Source::Env, &l.env, Refusal::Probe(ProbeError::TimedOut)),
                tried(
                    Source::Install,
                    &l.install,
                    Refusal::Probe(ProbeError::Failed("exited with exit code: 1".into()))
                ),
            ]
        );
    }

    #[test]
    fn not_found_lists_every_candidate_with_its_reason() {
        let l = layout();
        pair(&l.env);
        pair(&l.install);
        put(&l.dev, &[Tool::Ffmpeg]);
        let env = l.env.clone();
        let not_found = locate_with(l.candidates(true), |ffmpeg| {
            if ffmpeg.starts_with(&env) {
                with_buildconf(GPL_BUILDCONF)
            } else {
                Ok(ProbeOutput {
                    buildconf: LGPL_BUILDCONF.into(),
                    version: "Usage: something else\n".into(),
                })
            }
        })
        .unwrap_err();
        assert_eq!(
            not_found.tried,
            [
                tried(Source::Env, &l.env, Refusal::Gpl),
                tried(Source::Install, &l.install, Refusal::Unrecognized),
                tried(Source::Dev, &l.dev, Refusal::Missing(Tool::Ffprobe)),
            ]
        );
        let message = not_found.to_string();
        assert!(message.contains("--enable-gpl"), "{message}");
        assert!(message.contains(&Tool::Ffprobe.file_name()), "{message}");
    }

    const CHILD_MARKER: &str = "sela-ffmpeg-probe-child";

    fn child_args(test: &str) -> Vec<String> {
        let module = module_path!().split_once("::").unwrap().1;
        [
            "--exact",
            &format!("{module}::{test}"),
            "--ignored",
            "--nocapture",
            CHILD_MARKER,
        ]
        .map(String::from)
        .into()
    }

    fn in_child() -> bool {
        std::env::args().any(|arg| arg == CHILD_MARKER)
    }

    fn run_child(test: &str, timeout: Duration) -> Result<String, ProbeError> {
        let args = child_args(test);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        run(&std::env::current_exe().unwrap(), &args, timeout)
    }

    #[test]
    #[ignore = "child process for a_probe_child_that_hangs_is_killed_at_the_timeout"]
    fn child_hangs() {
        if in_child() {
            std::thread::sleep(Duration::from_secs(60));
        }
    }

    #[test]
    #[ignore = "child process for a_probe_child_that_floods_stdout_is_refused"]
    fn child_floods_stdout() {
        if in_child() {
            print!("{}", "configuration: ".repeat(PROBE_OUTPUT_LIMIT / 8));
        }
    }

    #[test]
    fn a_probe_child_that_hangs_is_killed_at_the_timeout() {
        let started = Instant::now();
        assert_eq!(
            run_child("child_hangs", Duration::from_millis(500)),
            Err(ProbeError::TimedOut)
        );
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_probe_child_that_floods_stdout_is_refused() {
        assert_eq!(
            run_child("child_floods_stdout", Duration::from_secs(60)),
            Err(ProbeError::OutputTooLarge)
        );
    }

    #[test]
    #[ignore = "child process for a_child_that_finishes_in_time_is_read"]
    fn child_succeeds() {}

    #[test]
    fn a_child_that_finishes_in_time_is_read() {
        let output = run_child("child_succeeds", Duration::from_secs(60)).unwrap();
        assert!(output.contains("1 passed"), "{output}");
    }

    #[test]
    fn a_program_that_is_not_ffmpeg_fails_the_probe() {
        let error = probe(&std::env::current_exe().unwrap()).unwrap_err();
        assert!(matches!(error, ProbeError::Failed(_)), "{error:?}");
    }

    #[test]
    fn a_file_that_cannot_run_fails_the_probe() {
        let dir = tempfile::tempdir().unwrap();
        pair(dir.path());
        let error = probe(&dir.path().join(Tool::Ffmpeg.file_name())).unwrap_err();
        assert!(matches!(error, ProbeError::Failed(_)), "{error:?}");
    }

    #[test]
    fn locate_reads_the_env_dir_and_never_path() {
        let root = tempfile::tempdir().unwrap();
        let (env, on_path) = (root.path().join("empty"), root.path().join("on-path"));
        fs::create_dir_all(&env).unwrap();
        pair(&on_path);
        let output = Command::new(std::env::current_exe().unwrap())
            .args(child_args("child_locates"))
            .env(ENV_DIR, &env)
            .env("PATH", &on_path)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    #[ignore = "child process for locate_reads_the_env_dir_and_never_path"]
    fn child_locates() {
        if !in_child() {
            return;
        }
        let env = PathBuf::from(std::env::var_os(ENV_DIR).unwrap());
        let path: Vec<PathBuf> =
            std::env::split_paths(&std::env::var_os("PATH").unwrap()).collect();
        let exe_dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let (located, tried) = match locate() {
            Ok(located) => (Some(located.dir), located.refused),
            Err(not_found) => (None, not_found.tried),
        };
        assert_eq!(
            tried[..2],
            [
                tried_entry(Source::Env, &env),
                tried_entry(Source::Install, &exe_dir.join("ffmpeg").join("bin")),
            ]
        );
        // Only a fetched development copy may be found here, never the PATH pair.
        if dev_dir().is_some_and(|dev| !dev.join(Tool::Ffmpeg.file_name()).is_file()) {
            assert_eq!(located, None);
        }
        for dir in tried.iter().map(|t| &t.candidate.dir).chain(located.iter()) {
            assert!(
                !path.contains(dir),
                "PATH entry {} was searched",
                dir.display()
            );
        }
    }

    fn tried_entry(source: Source, dir: &Path) -> Tried {
        tried(source, dir, Refusal::Missing(Tool::Ffmpeg))
    }

    fn assert_local_input(input: &str) {
        let args = input_args(Path::new(input)).unwrap();
        let i = args.iter().position(|arg| arg == "-i").unwrap();
        assert_eq!(args[i + 1..], [OsString::from(input)]);
        let before = &args[..i];
        for flag in ["-nostdin", "-hide_banner"] {
            assert!(before.iter().any(|arg| arg == flag), "{flag} in {args:?}");
        }
        let whitelist = before
            .iter()
            .position(|arg| arg == "-protocol_whitelist")
            .unwrap();
        assert_eq!(before.get(whitelist + 1), Some(&OsString::from("file")));
    }

    #[test]
    #[cfg(windows)]
    fn absolute_local_paths_get_the_local_only_flags_before_the_input() {
        assert_local_input(r"C:\Media\Café ✝.mp4");
        assert_local_input(r"D:\Worship\loops\blue.mov");
        assert_local_input(r"\\?\C:\Media\clip.mp4");
    }

    #[test]
    #[cfg(not(windows))]
    fn absolute_local_paths_get_the_local_only_flags_before_the_input() {
        assert_local_input("/media/Café ✝.mp4");
    }

    #[test]
    fn urls_and_protocol_prefixes_are_refused() {
        for input in [
            "http://example.com/live.m3u8",
            "https://example.com/clip.mp4",
            "rtmp://example.com/app/stream",
            "tcp://127.0.0.1:9000",
            "file:C:/Media/clip.mp4",
            "file:///media/clip.mp4",
            "concat:a.mp4|b.mp4",
            "data:video/mp4;base64,AAAA",
            "pipe:0",
        ] {
            assert_eq!(
                input_args(Path::new(input)),
                Err(InputRefused::Url),
                "{input}"
            );
        }
    }

    #[test]
    fn relative_paths_are_refused() {
        for input in [
            "",
            "clip.mp4",
            "videos/clip.mp4",
            "../clip.mp4",
            "-clip.mp4",
        ] {
            assert_eq!(
                input_args(Path::new(input)),
                Err(InputRefused::Relative),
                "{input}"
            );
        }
        #[cfg(windows)]
        for input in [r"C:clip.mp4", r"\Media\clip.mp4"] {
            assert_eq!(
                input_args(Path::new(input)),
                Err(InputRefused::Relative),
                "{input}"
            );
        }
    }

    #[test]
    #[cfg(windows)]
    fn network_and_device_paths_are_refused() {
        for (input, refused) in [
            (r"\\server\share\clip.mp4", InputRefused::Network),
            ("//server/share/clip.mp4", InputRefused::Network),
            (r"\\?\UNC\server\share\clip.mp4", InputRefused::Network),
            (r"\\.\pipe\clip", InputRefused::Device),
            (r"\\.\C:\Media\clip.mp4", InputRefused::Device),
            (
                r"\\?\Volume{0b5c5b3e-1111-2222-3333-444455556666}\clip.mp4",
                InputRefused::Device,
            ),
        ] {
            assert_eq!(input_args(Path::new(input)), Err(refused), "{input}");
        }
    }
}
