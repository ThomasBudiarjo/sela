//! Why a media file could not be probed or decoded (M2-02).

use super::ffmpeg::InputRefused;
use std::fmt;
use std::time::Duration;

/// A probe or decode failure. Messages are diagnostics for logs and the
/// operator's warning text; they never include file contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaError {
    /// The path is not an absolute local file path (see `ffmpeg::input_args`).
    Input(InputRefused),
    /// The file does not exist or is not a regular file.
    Missing,
    /// FFmpeg refused to open another protocol from inside a local file, e.g.
    /// a playlist that names an `http://` URL (VAL-MEDIA-038). No network
    /// request is made: `-protocol_whitelist file` stops it before opening.
    ProtocolRefused(String),
    /// The container or stream data is not readable media.
    Corrupt(String),
    /// The file has a video stream this FFmpeg build cannot decode.
    UnsupportedCodec(String),
    /// The file has no video stream.
    NoVideo,
    /// The decoded frame size does not fit the frame budget.
    TooLarge { width: u32, height: u32 },
    /// The ffprobe or ffmpeg child could not start.
    Spawn(String),
    /// ffprobe did not finish in time and was killed.
    ProbeTimedOut(Duration),
    /// ffprobe printed more than its output bound and was killed.
    ProbeOutputTooLarge,
    /// ffprobe printed something that is not its JSON report.
    ProbeUnreadable(String),
    /// The decoder produced nothing for this long while Sela was waiting for
    /// it (not while Sela's queue was full) and was killed.
    Stalled(Duration),
    /// A blocking helper's overall deadline (e.g. `first_frame`'s `timeout`)
    /// passed before a result, whatever was slow: probe, start or decode.
    TimedOut(Duration),
    /// The decoder exited abnormally or was killed by someone else.
    Crashed(String),
    /// The decoder ended without a single frame (failed first-frame
    /// preparation).
    NoFrames,
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(InputRefused::Url) => f.write_str("only local files can be played"),
            Self::Input(InputRefused::Relative) => f.write_str("the path is not absolute"),
            Self::Input(InputRefused::Network) => {
                f.write_str("network shares cannot be played directly")
            }
            Self::Input(InputRefused::Device) => f.write_str("device paths cannot be played"),
            Self::Missing => f.write_str("the file is missing"),
            Self::ProtocolRefused(detail) => {
                write!(f, "the file refers to a non-local source ({detail})")
            }
            Self::Corrupt(detail) => write!(f, "the file is damaged or not media ({detail})"),
            Self::UnsupportedCodec(codec) => write!(f, "unsupported video format ({codec})"),
            Self::NoVideo => f.write_str("the file has no video"),
            Self::TooLarge { width, height } => {
                write!(f, "the video is too large ({width}x{height})")
            }
            Self::Spawn(detail) => write!(f, "FFmpeg could not start ({detail})"),
            Self::ProbeTimedOut(timeout) => {
                write!(
                    f,
                    "reading the file took longer than {} s",
                    timeout.as_secs()
                )
            }
            Self::ProbeOutputTooLarge => f.write_str("the file's description is too large"),
            Self::ProbeUnreadable(detail) => write!(f, "unreadable media description ({detail})"),
            Self::Stalled(timeout) => write!(
                f,
                "the decoder stopped responding for {} s",
                timeout.as_secs_f32()
            ),
            Self::TimedOut(timeout) => write!(
                f,
                "preparing the picture took longer than {} s",
                timeout.as_secs_f32()
            ),
            Self::Crashed(detail) => write!(f, "the decoder stopped unexpectedly ({detail})"),
            Self::NoFrames => f.write_str("no picture could be decoded"),
        }
    }
}

impl std::error::Error for MediaError {}

/// Maps what a failed ffprobe/ffmpeg printed on stderr to a failure. The
/// patterns are FFmpeg 8.1's own messages; anything else is `fallback`.
pub(crate) fn classify(stderr: &str, fallback: impl FnOnce() -> MediaError) -> MediaError {
    let line = |needle: &str| {
        stderr
            .lines()
            .find(|line| line.contains(needle))
            .map(|line| short(line.trim()))
    };
    if let Some(line) = line("not on whitelist").or_else(|| line("Unsafe file name")) {
        return MediaError::ProtocolRefused(line);
    }
    if line("No such file or directory").is_some() {
        return MediaError::Missing;
    }
    for needle in [
        "Decoder (codec",
        "Decoding requested, but no decoder found",
        "Unsupported codec",
        "no decoder found",
    ] {
        if let Some(line) = line(needle) {
            return MediaError::UnsupportedCodec(line);
        }
    }
    for needle in [
        "Invalid data found when processing input",
        "moov atom not found",
        "EBML header parsing failed",
        "could not find codec parameters",
        "Could not find codec parameters",
    ] {
        if let Some(line) = line(needle) {
            return MediaError::Corrupt(line);
        }
    }
    fallback()
}

/// FFmpeg log lines start with `[component @ 0x...]` and often repeat the
/// full path; keep only the message so diagnostics stay short.
fn short(line: &str) -> String {
    let message = match line
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("] "))
    {
        Some((_, message)) => message,
        None => line,
    };
    let mut message = message.to_owned();
    if message.len() > 160 {
        let mut end = 160;
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        message.truncate(end);
        message.push('…');
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    fn other() -> MediaError {
        MediaError::Crashed("exit status: 1".into())
    }

    #[test]
    fn whitelist_refusals_are_protocol_errors() {
        let stderr = "[http @ 0x563b064c2c40] Protocol 'http' not on whitelist 'file'!\n\
                      [hls @ 0x563b] Error when loading first segment 'http://127.0.0.1:9/seg.ts'\n\
                      /tmp/evil.m3u8: Invalid data found when processing input\n";
        assert_eq!(
            classify(stderr, other),
            MediaError::ProtocolRefused("Protocol 'http' not on whitelist 'file'!".into())
        );
        let concat = "[in#0 @ 0x55] Unsafe file name 'http://127.0.0.1:9/a.mp4'\n";
        assert!(matches!(
            classify(concat, other),
            MediaError::ProtocolRefused(_)
        ));
    }

    #[test]
    fn decoder_and_data_failures_are_told_apart() {
        assert!(matches!(
            classify(
                "[vist#0:0 @ 0x1] Decoder (codec none) not found for input stream #0:0",
                other
            ),
            MediaError::UnsupportedCodec(_)
        ));
        assert!(matches!(
            classify("/m/x.mp4: Invalid data found when processing input", other),
            MediaError::Corrupt(_)
        ));
        assert_eq!(
            classify("x: No such file or directory", other),
            MediaError::Missing
        );
        assert_eq!(classify("something else", other), other());
    }

    #[test]
    fn long_lines_are_shortened_on_a_char_boundary() {
        let line = format!(
            "[x @ 0x1] Invalid data found when processing input {}",
            "é".repeat(200)
        );
        let MediaError::Corrupt(message) = classify(&line, other) else {
            panic!("corrupt");
        };
        assert!(message.starts_with("Invalid data"));
        assert!(message.len() <= 160 + '…'.len_utf8());
    }
}
