//! Media backend availability at startup (M2-01; VAL-MEDIA-001/002/003).
//!
//! The operator runs [`super::ffmpeg::locate`] once on a worker thread,
//! logs [`log_line`] and, when FFmpeg is unusable, shows a non-blocking
//! warning built from [`Unavailable`]. Everything else keeps working: songs,
//! scriptures, images, Live output and the safety controls never wait for
//! or depend on FFmpeg.

use super::ffmpeg::{Located, NotFound, Refusal};

/// Why video and audio are unavailable, most actionable reason first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    /// A candidate was built with `--enable-nonfree`.
    Nonfree,
    /// A candidate was built with `--enable-gpl` (for example a system
    /// FFmpeg put into `SELA_FFMPEG_DIR`).
    Gpl,
    /// A candidate exists but did not run or did not look like FFmpeg.
    Broken,
    /// No candidate folder holds `ffmpeg` and `ffprobe`.
    Missing,
}

impl Unavailable {
    /// The reason to show: a license refusal beats a broken copy, which
    /// beats nothing found, whatever the lookup order.
    pub fn of(not_found: &NotFound) -> Self {
        let refusals = || not_found.tried.iter().map(|tried| &tried.refusal);
        if refusals().any(|r| *r == Refusal::Nonfree) {
            Self::Nonfree
        } else if refusals().any(|r| *r == Refusal::Gpl) {
            Self::Gpl
        } else if refusals().any(|r| matches!(r, Refusal::Probe(_) | Refusal::Unrecognized)) {
            Self::Broken
        } else {
            Self::Missing
        }
    }
}

/// Startup media state, as the operator holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// `locate` has not answered yet (or no locator is configured).
    Checking,
    Ready(Located),
    Unavailable(Unavailable),
}

impl Availability {
    pub fn from_result(result: &Result<Located, NotFound>) -> Self {
        match result {
            Ok(located) => Self::Ready(located.clone()),
            Err(not_found) => Self::Unavailable(Unavailable::of(not_found)),
        }
    }

    pub fn located(&self) -> Option<&Located> {
        match self {
            Self::Ready(located) => Some(located),
            _ => None,
        }
    }
}

/// One log line for the locate result: the accepted version, folder and
/// source, plus every refused candidate with its reason (VAL-MEDIA-002).
pub fn log_line(result: &Result<Located, NotFound>) -> String {
    let refused = |tried: &[super::ffmpeg::Tried]| {
        tried
            .iter()
            .map(|t| {
                format!(
                    "{} {} refused: {}",
                    t.candidate.source,
                    t.candidate.dir.display(),
                    t.refusal
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    match result {
        Ok(located) => {
            let mut line = format!(
                "media: FFmpeg {} accepted from the {} ({})",
                located.version,
                located.source,
                located.dir.display()
            );
            if !located.refused.is_empty() {
                line += &format!("; earlier: {}", refused(&located.refused));
            }
            line
        }
        Err(not_found) if not_found.tried.is_empty() => {
            "media: FFmpeg unavailable: no candidate folder".to_owned()
        }
        Err(not_found) => format!(
            "media: FFmpeg unavailable ({:?}): {}",
            Unavailable::of(not_found),
            refused(&not_found.tried)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ffmpeg::{Candidate, ProbeError, Source, Tool, Tried};
    use std::path::PathBuf;

    fn tried(source: Source, dir: &str, refusal: Refusal) -> Tried {
        Tried {
            candidate: Candidate {
                source,
                dir: PathBuf::from(dir),
            },
            refusal,
        }
    }

    fn located(refused: Vec<Tried>) -> Located {
        Located {
            source: Source::Dev,
            dir: PathBuf::from("/sela/.amp/in/ffmpeg/n8.1.3-lgpl/bin"),
            ffmpeg: PathBuf::from("/sela/.amp/in/ffmpeg/n8.1.3-lgpl/bin/ffmpeg"),
            ffprobe: PathBuf::from("/sela/.amp/in/ffmpeg/n8.1.3-lgpl/bin/ffprobe"),
            version: "n8.1.3-9-g29e619e767".into(),
            refused,
        }
    }

    #[test]
    fn license_refusals_win_over_missing_copies_whatever_the_order() {
        let not_found = NotFound {
            tried: vec![
                tried(Source::Env, "/x", Refusal::Missing(Tool::Ffmpeg)),
                tried(Source::Install, "/usr/bin", Refusal::Gpl),
                tried(Source::Dev, "/d", Refusal::Probe(ProbeError::TimedOut)),
            ],
        };
        assert_eq!(Unavailable::of(&not_found), Unavailable::Gpl);
        let nonfree = NotFound {
            tried: vec![
                tried(Source::Env, "/a", Refusal::Gpl),
                tried(Source::Install, "/b", Refusal::Nonfree),
            ],
        };
        assert_eq!(Unavailable::of(&nonfree), Unavailable::Nonfree);
        let broken = NotFound {
            tried: vec![
                tried(Source::Env, "/a", Refusal::Unrecognized),
                tried(Source::Install, "/b", Refusal::Missing(Tool::Ffprobe)),
            ],
        };
        assert_eq!(Unavailable::of(&broken), Unavailable::Broken);
        let missing = NotFound {
            tried: vec![tried(Source::Install, "/b", Refusal::Missing(Tool::Ffmpeg))],
        };
        assert_eq!(Unavailable::of(&missing), Unavailable::Missing);
        assert_eq!(
            Unavailable::of(&NotFound { tried: Vec::new() }),
            Unavailable::Missing
        );
    }

    #[test]
    fn the_log_line_names_the_accepted_version_and_earlier_refusals() {
        let line = log_line(&Ok(located(vec![tried(
            Source::Env,
            "/usr/bin",
            Refusal::Gpl,
        )])));
        assert_eq!(
            line,
            "media: FFmpeg n8.1.3-9-g29e619e767 accepted from the development copy \
             (/sela/.amp/in/ffmpeg/n8.1.3-lgpl/bin); earlier: SELA_FFMPEG_DIR /usr/bin \
             refused: the build is configured with --enable-gpl"
        );
        let line = log_line(&Err(NotFound {
            tried: vec![tried(
                Source::Install,
                "/opt/sela/ffmpeg/bin",
                Refusal::Missing(Tool::Ffmpeg),
            )],
        }));
        assert!(line.starts_with("media: FFmpeg unavailable (Missing): install folder"));
        assert!(line.contains("is missing"), "{line}");
    }

    #[test]
    fn availability_follows_the_result() {
        let ready = Availability::from_result(&Ok(located(Vec::new())));
        assert_eq!(ready.located().unwrap().version, "n8.1.3-9-g29e619e767");
        let gpl = Availability::from_result(&Err(NotFound {
            tried: vec![tried(Source::Env, "/usr/bin", Refusal::Gpl)],
        }));
        assert_eq!(gpl, Availability::Unavailable(Unavailable::Gpl));
        assert!(gpl.located().is_none());
    }
}
