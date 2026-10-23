//! What a media file holds, read with `ffprobe` (M2-02 open/probe step).
//!
//! [`parse`] turns ffprobe's JSON report into [`MediaInfo`] and is pure;
//! [`probe`] runs the supervised child with a time and output bound. It
//! blocks for up to the timeout, so callers run it on a worker thread (the
//! decoder runs it on its own supervisor thread).

use super::error::{MediaError, classify};
use super::ffmpeg;
use super::process::Supervised;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const CANCEL_TICK: Duration = Duration::from_millis(25);

/// Bound on one ffprobe run, from spawn to exit.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound on ffprobe's JSON report. A plain clip's report is about 3 KiB.
pub const PROBE_OUTPUT_LIMIT: usize = 1024 * 1024;
/// Bound on the stderr kept for diagnostics.
const STDERR_LIMIT: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rational {
    pub num: u32,
    pub den: u32,
}

impl Rational {
    fn parse(text: &str) -> Option<Self> {
        let (num, den) = text.split_once(['/', ':'])?;
        let (num, den) = (num.trim().parse().ok()?, den.trim().parse().ok()?);
        (num > 0 && den > 0).then_some(Self { num, den })
    }

    pub fn as_f64(self) -> f64 {
        f64::from(self.num) / f64::from(self.den)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    /// ffprobe's stream index.
    pub index: u32,
    /// FFmpeg's codec name (`h264`, `vp9`, ...).
    pub codec: String,
    /// Coded picture size.
    pub width: u32,
    pub height: u32,
    /// Display size: the coded size with the sample aspect ratio and a
    /// quarter-turn rotation applied, which is what FFmpeg's autorotation
    /// delivers.
    pub display_width: u32,
    pub display_height: u32,
    /// Average frame rate, when the container states one.
    pub frame_rate: Option<Rational>,
    /// Nominal (`r_frame_rate`) and average rates differ: likely variable
    /// frame rate. Frame timestamps are authoritative either way.
    pub variable_rate: bool,
    /// Clockwise display rotation in degrees (0, 90, 180, 270).
    pub rotation: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioInfo {
    pub index: u32,
    pub codec: String,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    /// Container (`matroska,webm`, `mov,mp4,m4a,3gp,3g2,mj2`, ...).
    pub format: String,
    pub duration: Option<Duration>,
    /// The first video stream that is not an attached picture.
    pub video: Option<VideoInfo>,
    pub audio: Option<AudioInfo>,
}

/// Reads ffprobe's `-print_format json -show_format -show_streams` report.
/// A video stream without a codec name (`none` or absent) is a format this
/// FFmpeg build does not know: [`MediaError::UnsupportedCodec`].
pub fn parse(json: &str) -> Result<MediaInfo, MediaError> {
    let root: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| MediaError::ProbeUnreadable(error.to_string()))?;
    let format = root.get("format");
    let streams = root
        .get("streams")
        .and_then(|streams| streams.as_array())
        .cloned()
        .unwrap_or_default();
    if format.is_none() && streams.is_empty() {
        return Err(MediaError::ProbeUnreadable("no format or streams".into()));
    }
    let text = |value: &serde_json::Value, key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .filter(|s| !s.is_empty())
    };
    let number = |value: &serde_json::Value, key: &str| -> Option<u64> {
        let v = value.get(key)?;
        v.as_u64().or_else(|| v.as_str()?.parse().ok())
    };
    // A duration past `Duration::MAX` (a crafted time base) is unknown;
    // `Duration::from_secs_f64` would panic on it (SEC-10).
    let seconds = |value: &serde_json::Value, key: &str| {
        text(value, key)
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(|s| Duration::try_from_secs_f64(s).ok())
    };
    let mut video = None;
    let mut audio = None;
    for stream in &streams {
        let kind = text(stream, "codec_type").unwrap_or_default();
        let attached = stream
            .get("disposition")
            .and_then(|d| d.get("attached_pic"))
            .and_then(|v| v.as_u64())
            == Some(1);
        let index = number(stream, "index").unwrap_or(0) as u32;
        let codec = text(stream, "codec_name").filter(|c| c != "none");
        if kind == "video" && !attached && video.is_none() {
            let Some(codec) = codec else {
                let tag = text(stream, "codec_tag_string").unwrap_or_else(|| "unknown".into());
                return Err(MediaError::UnsupportedCodec(format!(
                    "stream {index}: unknown codec {tag}"
                )));
            };
            let width = number(stream, "width").unwrap_or(0) as u32;
            let height = number(stream, "height").unwrap_or(0) as u32;
            if width == 0 || height == 0 {
                return Err(MediaError::Corrupt(format!(
                    "stream {index}: no picture size"
                )));
            }
            let rotation = rotation(stream);
            let sar = text(stream, "sample_aspect_ratio")
                .as_deref()
                .and_then(Rational::parse)
                .unwrap_or(Rational { num: 1, den: 1 });
            let shown_width = ((f64::from(width) * sar.as_f64()).round() as u32).max(1);
            let (display_width, display_height) = if rotation % 180 == 90 {
                (height, shown_width)
            } else {
                (shown_width, height)
            };
            let average = text(stream, "avg_frame_rate")
                .as_deref()
                .and_then(Rational::parse);
            let nominal = text(stream, "r_frame_rate")
                .as_deref()
                .and_then(Rational::parse);
            video = Some(VideoInfo {
                index,
                codec,
                width,
                height,
                display_width,
                display_height,
                frame_rate: average.or(nominal),
                variable_rate: matches!((average, nominal), (Some(a), Some(n))
                    if (a.as_f64() - n.as_f64()).abs() > 0.01),
                rotation,
            });
        } else if kind == "audio" && audio.is_none() {
            audio = Some(AudioInfo {
                index,
                codec: codec.unwrap_or_else(|| "unknown".into()),
                sample_rate: number(stream, "sample_rate").map(|r| r as u32),
                channels: number(stream, "channels").map(|c| c as u32),
            });
        }
    }
    let duration = format
        .and_then(|format| seconds(format, "duration"))
        .or_else(|| streams.iter().find_map(|s| seconds(s, "duration")));
    Ok(MediaInfo {
        format: format
            .and_then(|format| text(format, "format_name"))
            .unwrap_or_default(),
        duration,
        video,
        audio,
    })
}

/// Clockwise rotation from the display matrix side data (or the legacy
/// `rotate` tag), normalized to 0/90/180/270.
fn rotation(stream: &serde_json::Value) -> u32 {
    let matrix = stream
        .get("side_data_list")
        .and_then(|list| list.as_array())
        .and_then(|list| {
            list.iter()
                .find_map(|side| side.get("rotation").and_then(|r| r.as_f64()))
        });
    let tag = stream
        .get("tags")
        .and_then(|tags| tags.get("rotate"))
        .and_then(|r| r.as_str())
        .and_then(|r| r.parse::<f64>().ok());
    // The display matrix rotation is counter-clockwise; the tag clockwise.
    let clockwise = matrix.map(|r| -r).or(tag).unwrap_or(0.);
    let quarter = (clockwise / 90.).round() as i64;
    (quarter.rem_euclid(4) * 90) as u32
}

/// Runs ffprobe on `input` with `-protocol_whitelist file`, bounded by
/// `timeout` and [`PROBE_OUTPUT_LIMIT`].
pub fn probe(ffprobe: &Path, input: &Path, timeout: Duration) -> Result<MediaInfo, MediaError> {
    probe_cancellable(ffprobe, input, timeout, &AtomicBool::new(false))
}

/// [`probe`] that also gives up (killing ffprobe) soon after `cancel` is set;
/// the error is then [`MediaError::ProbeTimedOut`] and is not reported.
pub fn probe_cancellable(
    ffprobe: &Path,
    input: &Path,
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<MediaInfo, MediaError> {
    let args = ffmpeg::probe_args(input).map_err(MediaError::Input)?;
    if !input.is_file() {
        return Err(MediaError::Missing);
    }
    let mut command = Command::new(ffprobe);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child =
        Supervised::spawn(command).map_err(|error| MediaError::Spawn(error.to_string()))?;
    let deadline = Instant::now() + timeout;
    let stdout = child.take_stdout().expect("stdout is piped");
    let stderr = child.take_stderr().expect("stderr is piped");
    let (out_tx, out_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = stdout
            .take(PROBE_OUTPUT_LIMIT as u64 + 1)
            .read_to_end(&mut bytes);
        let _ = out_tx.send(read.map(|_| bytes));
    });
    let (err_tx, err_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = err_tx.send(read_tail(stderr, STDERR_LIMIT));
    });
    let remaining = || deadline.saturating_duration_since(Instant::now());
    let received = loop {
        match out_rx.recv_timeout(remaining().min(CANCEL_TICK)) {
            Err(mpsc::RecvTimeoutError::Timeout)
                if !remaining().is_zero() && !cancel.load(Ordering::SeqCst) => {}
            other => break other,
        }
    };
    let stdout = match received {
        Ok(Ok(bytes)) if bytes.len() > PROBE_OUTPUT_LIMIT => {
            child.kill_and_wait();
            return Err(MediaError::ProbeOutputTooLarge);
        }
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            child.kill_and_wait();
            return Err(MediaError::ProbeUnreadable(error.to_string()));
        }
        Err(_) => {
            child.kill_and_wait();
            return Err(MediaError::ProbeTimedOut(timeout));
        }
    };
    let Some(status) = child.wait_timeout(remaining()) else {
        child.kill_and_wait();
        return Err(MediaError::ProbeTimedOut(timeout));
    };
    let stderr = err_rx
        .recv_timeout(remaining().max(Duration::from_millis(100)))
        .unwrap_or_default();
    if !status.success() {
        return Err(classify(&stderr, || {
            MediaError::Corrupt(format!("ffprobe {status}"))
        }));
    }
    parse(&String::from_utf8_lossy(&stdout))
}

/// Reads a pipe to its end, keeping at most the last `limit` bytes.
pub(crate) fn read_tail(mut pipe: impl Read, limit: usize) -> String {
    let mut kept: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 8192];
    while let Ok(read) = pipe.read(&mut buffer) {
        if read == 0 {
            break;
        }
        kept.extend_from_slice(&buffer[..read]);
        if kept.len() > limit * 2 {
            kept.drain(..kept.len() - limit);
        }
    }
    if kept.len() > limit {
        kept.drain(..kept.len() - limit);
    }
    String::from_utf8_lossy(&kept).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIP: &str = r#"{
        "streams": [
            {"index": 0, "codec_name": "ffv1", "codec_type": "video",
             "codec_tag_string": "[0][0][0][0]", "width": 317, "height": 239,
             "sample_aspect_ratio": "1:1", "r_frame_rate": "30/1",
             "avg_frame_rate": "30/1", "disposition": {"attached_pic": 0}},
            {"index": 1, "codec_name": "opus", "codec_type": "audio",
             "sample_rate": "48000", "channels": 2}
        ],
        "format": {"format_name": "matroska,webm", "duration": "1.500000"}
    }"#;

    #[test]
    fn a_clip_report_gives_streams_sizes_rates_and_duration() {
        let info = parse(CLIP).unwrap();
        assert_eq!(info.format, "matroska,webm");
        assert_eq!(info.duration, Some(Duration::from_millis(1500)));
        let video = info.video.unwrap();
        assert_eq!(
            (video.codec.as_str(), video.width, video.height),
            ("ffv1", 317, 239)
        );
        assert_eq!((video.display_width, video.display_height), (317, 239));
        assert_eq!(video.frame_rate, Some(Rational { num: 30, den: 1 }));
        assert!(!video.variable_rate);
        assert_eq!(
            info.audio,
            Some(AudioInfo {
                index: 1,
                codec: "opus".into(),
                sample_rate: Some(48000),
                channels: Some(2)
            })
        );
    }

    #[test]
    fn rotation_and_sample_aspect_change_the_display_size() {
        let json = r#"{"streams": [{"index": 0, "codec_name": "h264",
            "codec_type": "video", "width": 720, "height": 576,
            "sample_aspect_ratio": "16:15", "r_frame_rate": "25/1",
            "avg_frame_rate": "25/1",
            "side_data_list": [{"side_data_type": "Display Matrix", "rotation": -90}]}],
            "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2"}}"#;
        let video = parse(json).unwrap().video.unwrap();
        assert_eq!(video.rotation, 90);
        assert_eq!((video.display_width, video.display_height), (576, 768));
    }

    #[test]
    fn differing_nominal_and_average_rates_hint_at_variable_rate() {
        let json = CLIP.replace(r#""avg_frame_rate": "30/1""#, r#""avg_frame_rate": "20/1""#);
        let video = parse(&json).unwrap().video.unwrap();
        assert!(video.variable_rate);
        assert_eq!(video.frame_rate, Some(Rational { num: 20, den: 1 }));
    }

    #[test]
    fn an_unknown_video_codec_is_unsupported() {
        let json = r#"{"streams": [{"index": 0, "codec_type": "video",
            "codec_tag_string": "SELA", "width": 64, "height": 64}],
            "format": {"format_name": "avi"}}"#;
        assert_eq!(
            parse(json),
            Err(MediaError::UnsupportedCodec(
                "stream 0: unknown codec SELA".into()
            ))
        );
    }

    #[test]
    fn cover_art_is_not_the_video_and_audio_only_has_none() {
        let json = r#"{"streams": [
            {"index": 0, "codec_name": "mp3", "codec_type": "audio"},
            {"index": 1, "codec_name": "mjpeg", "codec_type": "video",
             "width": 300, "height": 300, "disposition": {"attached_pic": 1}}],
            "format": {"format_name": "mp3", "duration": "3.0"}}"#;
        let info = parse(json).unwrap();
        assert!(info.video.is_none());
        assert_eq!(info.audio.unwrap().codec, "mp3");
    }

    #[test]
    fn garbage_and_empty_reports_are_unreadable() {
        assert!(matches!(
            parse("not json"),
            Err(MediaError::ProbeUnreadable(_))
        ));
        assert!(matches!(parse("{}"), Err(MediaError::ProbeUnreadable(_))));
        let sizeless = r#"{"streams": [{"index": 0, "codec_name": "h264",
            "codec_type": "video"}], "format": {}}"#;
        assert!(matches!(parse(sizeless), Err(MediaError::Corrupt(_))));
    }

    // SEC-10 (fuzz-audit): a duration too large for `Duration` (a crafted
    // time base gives ffprobe one) panicked the probing thread.
    #[test]
    fn a_duration_past_the_duration_range_is_unknown_not_a_panic() {
        for huge in ["1e300", "1.8446744073709552e19", "inf", "-1", "NaN"] {
            let json = CLIP.replace("1.500000", huge);
            assert_eq!(parse(&json).unwrap().duration, None, "{huge}");
            let stream = format!(
                r#"{{"streams": [{{"index": 0, "codec_name": "opus",
                "codec_type": "audio", "duration": "{huge}"}}]}}"#
            );
            assert_eq!(parse(&stream).unwrap().duration, None, "{huge}");
        }
    }

    #[test]
    fn read_tail_keeps_the_end() {
        let text = "a".repeat(100) + "END";
        assert_eq!(read_tail(text.as_bytes(), 3), "END");
        assert_eq!(read_tail("short".as_bytes(), 64), "short");
    }
}
