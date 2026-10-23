//! The FFmpeg subprocess boundary: the input path handed to `ffmpeg -i`, the
//! build output read back from the bundled binary, ffprobe's JSON report and
//! ffmpeg's `showinfo` frame lines. A media path comes from a schedule file
//! or the file chooser; the reports describe a file anyone may have crafted,
//! printed by whatever executable sits where FFmpeg is expected.
use super::{config, mutated, within};
use crate::media::decode::{self, Clock, Fit, MAX_DIMENSION};
use crate::media::error::MediaError;
use crate::media::ffmpeg::{self, Refusal};
use crate::media::probe;
use proptest::prelude::*;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

/// FFmpeg's own protocol test: two or more scheme characters, then `:`.
fn looks_like_url(path: &str) -> bool {
    let scheme = path
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        .count();
    scheme >= 2 && path.as_bytes().get(scheme) == Some(&b':')
}

fn check_path(text: &str) {
    let path = Path::new(text);
    if let Ok(args) = ffmpeg::input_args(path) {
        assert!(path.is_absolute(), "{text:?}");
        assert!(!looks_like_url(text), "{text:?}");
        assert_eq!(args.last(), Some(&OsString::from(text)));
        let i = args.len() - 2;
        assert_eq!(args[i], "-i");
        // The whitelist comes before the input, so it applies to it.
        let whitelist = args.iter().position(|a| a == "-protocol_whitelist");
        assert!(whitelist.is_some_and(|w| w < i && args[w + 1] == "file"));
    }
}

proptest! {
    #![proptest_config(config(0x5e1a_0701, 512, "tests/fixtures/fuzz/media.proptest-regressions"))]

    #[test]
    fn only_absolute_local_paths_reach_ffmpeg(
        path in prop_oneof![
            any::<String>(),
            "(/|[A-Za-z]:\\\\|\\\\\\\\[?.]\\\\|[a-z+.-]{1,8}:|\\.\\./)[ -~]{0,40}",
        ],
    ) {
        check_path(&path);
    }

    #[test]
    fn build_output_is_classified_without_panicking(
        words in proptest::collection::vec(
            prop_oneof![
                Just("configuration:"),
                Just("--enable-gpl"),
                Just("--enable-nonfree"),
                Just("--enable-version3"),
                Just("ffmpeg version n8.1"),
                Just("\n"),
                Just(" "),
                Just("\u{a0}"),
                Just("configuration:--enable-gpl"),
            ],
            0..20,
        ),
    ) {
        let output = words.concat();
        let flags: Vec<&str> = output.split_whitespace().collect();
        match ffmpeg::check_buildconf(&output) {
            Ok(()) => {
                prop_assert!(flags.contains(&"configuration:"));
                prop_assert!(!flags.contains(&"--enable-gpl"));
                prop_assert!(!flags.contains(&"--enable-nonfree"));
            }
            Err(Refusal::Gpl) => prop_assert!(flags.contains(&"--enable-gpl")),
            Err(Refusal::Nonfree) => prop_assert!(flags.contains(&"--enable-nonfree")),
            Err(Refusal::Unrecognized) => prop_assert!(!flags.contains(&"configuration:")),
            Err(other) => prop_assert!(false, "{other:?}"),
        }
    }
}

/// ffprobe reports: whatever parses describes a picture Sela can size, and
/// anything else is a typed error, never a panic.
fn check_report(json: &str) {
    match within(Duration::from_secs(5), "probe report", || {
        probe::parse(json)
    }) {
        Ok(info) => {
            if let Some(video) = info.video {
                assert!(video.width > 0 && video.height > 0);
                assert!(video.display_width > 0 && video.display_height > 0);
                assert!(matches!(video.rotation, 0 | 90 | 180 | 270));
                let (picture, frame) = decode::output_extent(&video, Fit::Source).unwrap();
                assert_eq!(picture, frame);
                assert!(picture.width.clamp(1, MAX_DIMENSION) == picture.width);
                assert!(picture.height.clamp(1, MAX_DIMENSION) == picture.height);
            }
        }
        Err(
            MediaError::ProbeUnreadable(_)
            | MediaError::UnsupportedCodec(_)
            | MediaError::Corrupt(_),
        ) => {}
        Err(other) => panic!("parsing a report gave {other:?}"),
    }
}

const REPORTS: &[&str] = &[
    r#"{"streams": [{"index": 0, "codec_name": "ffv1", "codec_type": "video",
      "codec_tag_string": "[0][0][0][0]", "width": 317, "height": 239,
      "sample_aspect_ratio": "1:1", "r_frame_rate": "30/1", "avg_frame_rate": "30/1",
      "disposition": {"attached_pic": 0}, "duration": "1.5"},
      {"index": 1, "codec_name": "opus", "codec_type": "audio", "sample_rate": "48000",
      "channels": 2}], "format": {"format_name": "matroska,webm", "duration": "1.500000"}}"#,
    r#"{"streams": [{"index": 0, "codec_name": "h264", "codec_type": "video",
      "width": 720, "height": 576, "sample_aspect_ratio": "16:15",
      "side_data_list": [{"side_data_type": "Display Matrix", "rotation": -90}],
      "tags": {"rotate": "90"}}], "format": {"format_name": "mov"}}"#,
    r#"{"streams": [{"index": 1, "codec_name": "mjpeg", "codec_type": "video",
      "width": 300, "height": 300, "disposition": {"attached_pic": 1}}],
      "format": {"format_name": "mp3", "duration": "3.0"}}"#,
];

const JSON_TOKENS: &[&str] = &[
    "{",
    "}",
    "[",
    "]",
    ",",
    ":",
    "\"",
    "null",
    "true",
    "0",
    "-1",
    "1e300",
    "-1e300",
    "4294967296",
    "18446744073709551616",
    "\"NaN\"",
    "\"inf\"",
    "\"1e300\"",
    "\"0/0\"",
    "\"4294967295:1\"",
    "\"1:4294967295\"",
    "\"video\"",
    "\"audio\"",
    "\"none\"",
    "\"width\":",
    "\"height\":",
    "\"rotation\":",
    "\"duration\":",
    "\"sample_aspect_ratio\":",
    "\"avg_frame_rate\":",
    "\"codec_name\":",
];

/// Numbers and seconds as FFmpeg prints them, with values at and past the
/// ranges of `u32`, `u64`, `f64` and `Duration`.
const NUMBERS: &[&str] = &[
    "0",
    "1",
    "2",
    "-1",
    "90",
    "-90",
    "270",
    "45",
    "1e300",
    "4294967295",
    "4294967296",
    "18446744073709551616",
    "8192",
    "8193",
    "16384",
    "NaN",
    "inf",
];
const SECONDS: &[&str] = &[
    "0",
    "0.033",
    "1.5",
    "-1",
    "-0",
    "1e300",
    "-1e300",
    "1e308",
    "1.7976931348623157e308",
    "18446744073709551615",
    "18446744073709551616",
    "5e18",
    "2e19",
    "NOPTS",
    "N/A",
    "inf",
    "-inf",
    "NaN",
    "",
];
const RATIOS: &[&str] = &[
    "1:1",
    "16:15",
    "0:1",
    "1:0",
    "4294967295:1",
    "1:4294967295",
    "30/1",
    "30000/1001",
    "0/0",
    "-1/2",
    "x",
    "",
];

fn check_frames(rate: Option<f64>, lines: impl Iterator<Item = String>) {
    let mut clock = Clock::new(rate);
    let mut last = Duration::ZERO;
    for line in lines {
        if let Some(meta) = decode::parse_showinfo(&line) {
            assert!(meta.pts_time.is_none_or(f64::is_finite));
            assert!(meta.duration_time.is_none_or(f64::is_finite));
            let (time, _) = clock.place(&meta);
            assert!(time >= last, "frame times never go back: {line}");
            last = time;
        }
    }
}

const SHOWINFO_TOKENS: &[&str] = &[
    "[Parsed_showinfo_0 @ 0x1] ",
    "n:",
    "n: ",
    "pts_time:",
    "pts_time: ",
    "duration_time:",
    "s:",
    "x",
    "0",
    "1",
    "-1",
    "0.033",
    "1e300",
    "-1e300",
    "1e308",
    "NOPTS",
    "N/A",
    "inf",
    "NaN",
    "18446744073709551616",
    " ",
    "]",
    ":",
];

proptest! {
    #![proptest_config(config(0x5e1a_0702, 512, "tests/fixtures/fuzz/media-reports.proptest-regressions"))]

    #[test]
    fn mutated_probe_reports_are_sized_pictures_or_typed_errors(
        bytes in mutated(REPORTS.iter().map(|r| r.as_bytes().to_vec()).collect(), JSON_TOKENS),
    ) {
        check_report(&String::from_utf8_lossy(&bytes));
    }

    #[test]
    fn generated_probe_reports_are_sized_pictures_or_typed_errors(
        kind in proptest::sample::select(&["video", "audio", "subtitle", ""][..]),
        size in (proptest::sample::select(NUMBERS), proptest::sample::select(NUMBERS)),
        sar in proptest::sample::select(RATIOS),
        rates in (proptest::sample::select(RATIOS), proptest::sample::select(RATIOS)),
        rotation in proptest::sample::select(NUMBERS),
        durations in (proptest::sample::select(SECONDS), proptest::sample::select(SECONDS)),
        attached in any::<bool>(),
    ) {
        let number = |text: &str| text.parse::<serde_json::Value>().unwrap_or_else(|_| text.into());
        let report = serde_json::json!({
            "streams": [{
                "index": 0, "codec_name": "h264", "codec_type": kind,
                "width": number(size.0), "height": number(size.1),
                "sample_aspect_ratio": sar, "r_frame_rate": rates.0, "avg_frame_rate": rates.1,
                "side_data_list": [{"rotation": number(rotation)}],
                "tags": {"rotate": rotation},
                "disposition": {"attached_pic": u8::from(attached)},
                "duration": durations.1,
            }],
            "format": {"format_name": "fuzz", "duration": durations.0},
        });
        check_report(&report.to_string());
    }

    #[test]
    fn generated_showinfo_frames_give_ordered_times(
        frames in proptest::collection::vec(
            (
                proptest::option::of(proptest::sample::select(NUMBERS)),
                proptest::option::of(proptest::sample::select(SECONDS)),
                proptest::option::of(proptest::sample::select(SECONDS)),
                proptest::option::of(proptest::sample::select(NUMBERS)),
            ),
            1..12,
        ),
        rate in proptest::option::of(prop_oneof![Just(0.), Just(-1.), Just(f64::NAN), Just(1e-300), 1.0..120.0f64]),
    ) {
        let lines = frames.iter().map(|(n, pts, duration, width)| {
            let mut line = String::from("[Parsed_showinfo_1 @ 0x5e1a] ");
            for (key, value) in [("n", n), ("pts_time", pts), ("duration_time", duration)] {
                if let Some(value) = value {
                    line += &format!("{key}:{value} ");
                }
            }
            if let Some(width) = width {
                line += &format!("s:{width}x{width} ");
            }
            line
        });
        check_frames(rate, lines);
    }

    #[test]
    fn showinfo_lines_give_ordered_times_without_panicking(
        lines in proptest::collection::vec(
            proptest::collection::vec(proptest::sample::select(SHOWINFO_TOKENS), 0..14),
            1..20,
        ),
        rate in proptest::option::of(prop_oneof![Just(0.), Just(-1.), Just(f64::NAN), Just(1e-300), 1.0..120.0f64]),
    ) {
        check_frames(rate, lines.iter().map(|line| line.concat()));
    }
}
