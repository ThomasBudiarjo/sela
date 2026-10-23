//! M2-02 decoder end-to-end tests against the real pinned LGPL FFmpeg.
//!
//! Every clip is original synthetic media generated at test time by the
//! located FFmpeg itself (`testsrc`, `color` and `sine` lavfi sources) into
//! a temporary folder. When no usable FFmpeg is found (run
//! `scripts/fetch-ffmpeg` first) each test prints an explicit `SKIPPED` line
//! and passes, unless `SELA_REQUIRE_FFMPEG=1`, which turns the skip into a
//! failure so a qualifying run cannot pass without FFmpeg.

use sela::media::MediaError;
use sela::media::decode::{
    DecodeEvent, DecodeOptions, Decoder, Fit, Opened, Tools, VideoFrame, first_frame,
};
use sela::media::ffmpeg::{self, InputRefused, Refusal};
use sela::media::probe;
use sela::scene::{Extent, PreparedBackground};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(30);

fn tools(test: &str) -> Option<Tools> {
    match ffmpeg::locate() {
        Ok(located) => Some(Tools::from(&located)),
        Err(not_found) => {
            let message = format!(
                "SKIPPED {test}: no usable FFmpeg ({not_found:?}); run scripts/fetch-ffmpeg"
            );
            if std::env::var_os("SELA_REQUIRE_FFMPEG").is_some_and(|v| v == "1") {
                panic!("{message} (SELA_REQUIRE_FFMPEG=1)");
            }
            eprintln!("{message}");
            None
        }
    }
}

/// Runs the located ffmpeg to write a synthetic clip.
fn generate(tools: &Tools, dir: &Path, name: &str, args: &[&str]) -> PathBuf {
    let out = dir.join(name);
    let status = Command::new(&tools.ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .args(args)
        .arg(&out)
        .stdin(Stdio::null())
        .status()
        .expect("run ffmpeg to generate a fixture");
    assert!(status.success(), "generating {name} failed: {status}");
    out
}

fn testsrc(tools: &Tools, dir: &Path, name: &str, size: &str, rate: u32, seconds: &str) -> PathBuf {
    let source = format!("testsrc=size={size}:rate={rate}:duration={seconds}");
    generate(
        tools,
        dir,
        name,
        &["-f", "lavfi", "-i", &source, "-c:v", "ffv1"],
    )
}

fn solid(tools: &Tools, dir: &Path, name: &str, color: &str, size: &str, frames: u32) -> PathBuf {
    let source = format!("color=c={color}:size={size}:rate=10");
    let frames = frames.to_string();
    generate(
        tools,
        dir,
        name,
        &[
            "-f",
            "lavfi",
            "-i",
            &source,
            "-frames:v",
            &frames,
            "-c:v",
            "ffv1",
        ],
    )
}

/// Everything until the terminal event.
struct Run {
    opened: Option<Opened>,
    frames: Vec<VideoFrame>,
    end: Option<(u64, bool)>,
    failed: Option<MediaError>,
}

fn run(decoder: &Decoder) -> Run {
    run_after(decoder, false)
}

/// Like `run`, when the caller already took `Opened` (and maybe frames).
fn run_after(decoder: &Decoder, opened: bool) -> Run {
    let mut run = Run {
        opened: None,
        frames: Vec::new(),
        end: None,
        failed: None,
    };
    let deadline = Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match decoder.next_timeout(left) {
            Some(DecodeEvent::Opened(opened)) => {
                assert!(
                    run.opened.is_none() && run.frames.is_empty(),
                    "Opened first"
                );
                run.opened = Some(opened);
            }
            Some(DecodeEvent::Frame(frame)) => {
                assert!(opened || run.opened.is_some(), "a frame before Opened");
                run.frames.push(frame);
            }
            Some(DecodeEvent::End { frames, limited }) => {
                run.end = Some((frames, limited));
                break;
            }
            Some(DecodeEvent::Failed(error)) => {
                run.failed = Some(error);
                break;
            }
            None => panic!("no terminal event within {WAIT:?}"),
        }
    }
    assert!(
        decoder.next_timeout(Duration::from_millis(200)).is_none(),
        "nothing follows the terminal event"
    );
    run
}

fn decode(tools: &Tools, input: &Path, options: DecodeOptions) -> Run {
    run(&Decoder::start(tools, input, options))
}

fn near(a: [u8; 4], b: [u8; 4], tolerance: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tolerance)
}

#[cfg(target_os = "linux")]
fn process_exists(pid: u32) -> bool {
    // A reaped process has no /proc entry; a zombie would show state Z.
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map(|stat| {
            !stat
                .rsplit_once(')')
                .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'))
        })
        .unwrap_or(false)
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .stdin(Stdio::null())
        .output()
        .expect("tasklist");
    String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
}

#[test]
fn a_generated_clip_decodes_every_frame_in_order_with_timestamps() {
    let Some(tools) = tools("a_generated_clip_decodes_every_frame_in_order_with_timestamps") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = testsrc(&tools, dir.path(), "clip.mkv", "320x240", 30, "1");
    let result = decode(
        &tools,
        &clip,
        DecodeOptions {
            content_id: Some(7),
            ..Default::default()
        },
    );
    assert_eq!(result.failed, None);
    let opened = result.opened.unwrap();
    assert_eq!(opened.content_id, 7);
    let video = opened.info.video.as_ref().unwrap();
    assert_eq!(
        (video.codec.as_str(), video.width, video.height),
        ("ffv1", 320, 240)
    );
    assert_eq!(
        opened.extent,
        Extent {
            width: 320,
            height: 240
        }
    );
    assert_eq!(result.end, Some((30, false)));
    assert_eq!(result.frames.len(), 30);
    for (index, frame) in result.frames.iter().enumerate() {
        assert_eq!(frame.index, index as u64);
        assert_eq!(frame.rgba.len(), 320 * 240 * 4);
        assert_eq!(frame.version.id, 7);
        assert_eq!(frame.version.revision, index as u64);
        assert!(!frame.repaired);
        let expected = Duration::from_secs_f64(index as f64 / 30.);
        assert!(
            frame.pts.abs_diff(expected) <= Duration::from_millis(2),
            "frame {index} at {:?}, expected {expected:?}",
            frame.pts
        );
    }
    // testsrc draws moving content: consecutive frames differ.
    assert_ne!(result.frames[0].rgba, result.frames[1].rgba);
}

#[test]
fn variable_frame_rate_keeps_each_frames_own_timestamp() {
    let Some(tools) = tools("variable_frame_rate_keeps_each_frames_own_timestamp") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    // Frames 0-9 at 30 fps, then a 0.5 s gap before frame 10.
    let clip = generate(
        &tools,
        dir.path(),
        "vfr.mkv",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=160x120:rate=30:duration=1",
            "-vf",
            "setpts='if(gte(N,10),PTS+15,PTS)'",
            "-fps_mode",
            "passthrough",
            "-c:v",
            "ffv1",
        ],
    );
    let result = decode(&tools, &clip, DecodeOptions::default());
    assert_eq!(result.failed, None);
    assert_eq!(result.frames.len(), 30);
    let at = |index: usize| result.frames[index].pts.as_secs_f64();
    assert!((at(9) - 0.3).abs() < 0.002, "frame 9 at {}", at(9));
    assert!(
        (at(10) - (10. / 30. + 0.5)).abs() < 0.002,
        "frame 10 at {}",
        at(10)
    );
    assert!(
        (at(29) - (29. / 30. + 0.5)).abs() < 0.002,
        "frame 29 at {}",
        at(29)
    );
    assert!(result.frames.windows(2).all(|w| w[0].pts < w[1].pts));
}

#[test]
fn odd_and_tiny_sizes_decode_at_their_own_size_and_color() {
    let Some(tools) = tools("odd_and_tiny_sizes_decode_at_their_own_size_and_color") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    for (size, width, height) in [("317x239", 317, 239), ("33x17", 33, 17), ("1x1", 1, 1)] {
        let clip = solid(
            &tools,
            dir.path(),
            &format!("red-{size}.mkv"),
            "0xFF0000",
            size,
            3,
        );
        let result = decode(&tools, &clip, DecodeOptions::default());
        assert_eq!(result.failed, None, "{size}");
        assert_eq!(result.frames.len(), 3, "{size}");
        let frame = &result.frames[0];
        assert_eq!(frame.extent, Extent { width, height }, "{size}");
        assert_eq!(frame.rgba.len(), (width * height * 4) as usize);
        let corner = frame.pixel(width - 1, height - 1);
        assert!(near(corner, [255, 0, 0, 255], 12), "{size}: {corner:?}");
    }
}

#[test]
fn exact_fit_letterboxes_into_the_cue_extent_as_a_background() {
    let Some(tools) = tools("exact_fit_letterboxes_into_the_cue_extent_as_a_background") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    // 4:3 green into 16:9: pillarboxed, black bars left and right.
    let clip = solid(&tools, dir.path(), "green.mkv", "0x00FF00", "320x240", 2);
    let extent = Extent {
        width: 640,
        height: 360,
    };
    let (info, frame) = first_frame(&tools, &clip, extent, WAIT).unwrap();
    assert_eq!(info.video.unwrap().display_width, 320);
    assert_eq!(frame.extent, extent);
    assert!(near(frame.pixel(320, 180), [0, 255, 0, 255], 12));
    assert!(near(frame.pixel(5, 180), [0, 0, 0, 255], 4));
    assert!(near(frame.pixel(634, 180), [0, 0, 0, 255], 4));
    let PreparedBackground::Image {
        extent: background,
        rgba,
        ..
    } = frame.background()
    else {
        panic!("an image background");
    };
    assert_eq!(background, extent);
    assert!(std::sync::Arc::ptr_eq(&rgba, &frame.rgba), "no pixel copy");
}

#[test]
fn within_fit_shrinks_large_pictures_and_never_enlarges_small_ones() {
    let Some(tools) = tools("within_fit_shrinks_large_pictures_and_never_enlarges_small_ones")
    else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = solid(&tools, dir.path(), "wide.mkv", "blue", "400x100", 1);
    let within = |width, height| DecodeOptions {
        fit: Fit::Within(Extent { width, height }),
        ..Default::default()
    };
    let shrunk = decode(&tools, &clip, within(200, 200));
    assert_eq!(
        shrunk.frames[0].extent,
        Extent {
            width: 200,
            height: 50
        }
    );
    let kept = decode(&tools, &clip, within(1920, 1080));
    assert_eq!(
        kept.frames[0].extent,
        Extent {
            width: 400,
            height: 100
        }
    );
}

#[test]
fn short_clips_end_cleanly() {
    let Some(tools) = tools("short_clips_end_cleanly") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let one = solid(&tools, dir.path(), "one.mkv", "white", "64x36", 1);
    let result = decode(&tools, &one, DecodeOptions::default());
    assert_eq!((result.frames.len(), result.end), (1, Some((1, false))));
    let tenth = testsrc(&tools, dir.path(), "tenth.mkv", "64x36", 30, "0.1");
    let result = decode(&tools, &tenth, DecodeOptions::default());
    assert_eq!(result.failed, None);
    assert_eq!(result.end, Some((3, false)));
    // A frame limit stops early and says so.
    let long = testsrc(&tools, dir.path(), "long.mkv", "64x36", 30, "2");
    let result = decode(
        &tools,
        &long,
        DecodeOptions {
            max_frames: Some(5),
            ..Default::default()
        },
    );
    assert_eq!((result.frames.len(), result.end), (5, Some((5, true))));
}

#[test]
fn an_unknown_codec_is_unsupported_and_yields_no_frame() {
    let Some(tools) = tools("an_unknown_codec_is_unsupported_and_yields_no_frame") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let avi = generate(
        &tools,
        dir.path(),
        "ffv1.avi",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x48:rate=10:duration=1",
            "-c:v",
            "ffv1",
        ],
    );
    // Rename the codec tag to one no FFmpeg build knows.
    let mut bytes = std::fs::read(&avi).unwrap();
    let mut patched = 0;
    for at in 0..bytes.len().saturating_sub(4) {
        if &bytes[at..at + 4] == b"FFV1" {
            bytes[at..at + 4].copy_from_slice(b"SELA");
            patched += 1;
        }
    }
    assert!(patched > 0);
    let unknown = dir.path().join("unknown.avi");
    std::fs::write(&unknown, bytes).unwrap();
    let result = decode(&tools, &unknown, DecodeOptions::default());
    assert!(result.frames.is_empty());
    assert!(result.opened.is_none());
    match result.failed {
        Some(MediaError::UnsupportedCodec(detail)) => assert!(detail.contains("SELA"), "{detail}"),
        other => panic!("expected UnsupportedCodec, got {other:?}"),
    }
}

#[test]
fn corrupt_and_truncated_files_fail_or_end_without_hanging() {
    let Some(tools) = tools("corrupt_and_truncated_files_fail_or_end_without_hanging") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let garbage = dir.path().join("garbage.mp4");
    let noise: Vec<u8> = (0..64 * 1024u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    std::fs::write(&garbage, noise).unwrap();
    let result = decode(&tools, &garbage, DecodeOptions::default());
    assert!(result.frames.is_empty());
    assert!(
        matches!(result.failed, Some(MediaError::Corrupt(_))),
        "{:?}",
        result.failed
    );
    // Half a clip: whatever decodes is delivered, then a terminal event.
    let clip = testsrc(&tools, dir.path(), "whole.mkv", "160x120", 30, "2");
    let bytes = std::fs::read(&clip).unwrap();
    let half = dir.path().join("half.mkv");
    std::fs::write(&half, &bytes[..bytes.len() / 2]).unwrap();
    let result = decode(&tools, &half, DecodeOptions::default());
    assert!(!result.frames.is_empty() && result.frames.len() < 60);
    assert!(result.end.is_some() || result.failed.is_some());
    // Only the header: nothing decodes.
    let header = dir.path().join("header.mkv");
    std::fs::write(&header, &bytes[..200]).unwrap();
    let result = decode(&tools, &header, DecodeOptions::default());
    assert!(result.frames.is_empty());
    assert!(result.failed.is_some(), "{:?}", result.end);
}

#[test]
fn a_file_without_video_is_refused() {
    let Some(tools) = tools("a_file_without_video_is_refused") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let tone = generate(
        &tools,
        dir.path(),
        "tone.mka",
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-c:a",
            "flac",
        ],
    );
    let result = decode(&tools, &tone, DecodeOptions::default());
    assert_eq!(result.failed, Some(MediaError::NoVideo));
    let info = probe::probe(&tools.ffprobe, &tone, WAIT).unwrap();
    assert_eq!(info.audio.unwrap().codec, "flac");
}

/// VAL-MEDIA-038: a crafted local playlist that names an `http://` URL
/// fails with a protocol error, and nothing connects to the URL.
#[test]
fn a_local_playlist_naming_http_fails_with_a_protocol_error_and_no_connection() {
    let Some(tools) =
        tools("a_local_playlist_naming_http_fails_with_a_protocol_error_and_no_connection")
    else {
        return;
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let dir = tempfile::tempdir().unwrap();
    let hls = dir.path().join("crafted.m3u8");
    std::fs::write(
        &hls,
        format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:2\n#EXTINF:2.0,\nhttp://127.0.0.1:{port}/segment.ts\n#EXT-X-ENDLIST\n"
        ),
    )
    .unwrap();
    let concat = dir.path().join("crafted.ffconcat");
    std::fs::write(
        &concat,
        format!("ffconcat version 1.0\nfile http://127.0.0.1:{port}/clip.mp4\n"),
    )
    .unwrap();
    for playlist in [&hls, &concat] {
        let result = decode(&tools, playlist, DecodeOptions::default());
        assert!(result.frames.is_empty());
        assert!(
            matches!(result.failed, Some(MediaError::ProtocolRefused(_))),
            "{}: {:?}",
            playlist.display(),
            result.failed
        );
    }
    // The decoder's own ffmpeg arguments refuse it too (not only ffprobe's).
    let mut command = Command::new(&tools.ffmpeg);
    command
        .args(ffmpeg::input_args(&hls).unwrap())
        .args(["-f", "null", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not on whitelist"));
    match listener.accept() {
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
        other => panic!("something connected to the playlist URL: {other:?}"),
    }
}

#[test]
fn refused_and_missing_inputs_fail_before_any_child_starts() {
    let tools = Tools {
        ffmpeg: PathBuf::from("/nonexistent/ffmpeg"),
        ffprobe: PathBuf::from("/nonexistent/ffprobe"),
    };
    let fail = |input: &str| decode(&tools, Path::new(input), DecodeOptions::default()).failed;
    assert_eq!(
        fail("clip.mp4"),
        Some(MediaError::Input(InputRefused::Relative))
    );
    assert_eq!(
        fail("http://example.invalid/clip.mp4"),
        Some(MediaError::Input(InputRefused::Url))
    );
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing.mp4");
    assert_eq!(
        decode(&tools, &missing, DecodeOptions::default()).failed,
        Some(MediaError::Missing)
    );
}

#[test]
fn backpressure_holds_the_decoder_without_a_stall_or_lost_frames() {
    let Some(tools) = tools("backpressure_holds_the_decoder_without_a_stall_or_lost_frames") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = testsrc(&tools, dir.path(), "clip.mkv", "160x120", 30, "2");
    let decoder = Decoder::start(
        &tools,
        &clip,
        DecodeOptions {
            queue_frames: 2,
            stall_timeout: Duration::from_millis(300),
            ..Default::default()
        },
    );
    // Do not consume for well over the stall timeout. 60 frames of 75 KiB
    // cannot fit the queue and a pipe buffer, so a decoder that is held back
    // is still running; one that was not would have finished in ~0.1 s.
    let deadline = Instant::now() + WAIT;
    while decoder.pid().is_none() {
        assert!(Instant::now() < deadline, "ffmpeg started");
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(1200));
    #[cfg(any(target_os = "linux", windows))]
    assert!(
        process_exists(decoder.pid().unwrap()),
        "ffmpeg is held back by the full queue, not finished"
    );
    let result = run(&decoder);
    assert_eq!(result.failed, None, "backpressure is not a stall");
    assert_eq!(result.end, Some((60, false)));
    assert_eq!(result.frames.len(), 60, "no frame lost");
    assert_eq!(result.opened.unwrap().queue_frames, 2);
}

#[test]
fn try_next_never_blocks_while_the_decoder_works() {
    let Some(tools) = tools("try_next_never_blocks_while_the_decoder_works") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = testsrc(&tools, dir.path(), "clip.mkv", "320x240", 30, "1");
    let started = Instant::now();
    let decoder = Decoder::start(&tools, &clip, DecodeOptions::default());
    let mut events = 0;
    let mut longest = Duration::ZERO;
    let deadline = Instant::now() + WAIT;
    loop {
        let poll = Instant::now();
        let event = decoder.try_next();
        longest = longest.max(poll.elapsed());
        match event {
            Some(DecodeEvent::End { .. }) => break,
            Some(DecodeEvent::Failed(error)) => panic!("{error}"),
            Some(_) => events += 1,
            None => std::thread::sleep(Duration::from_millis(1)),
        }
        assert!(Instant::now() < deadline);
    }
    assert!(started.elapsed() > Duration::ZERO);
    assert_eq!(events, 31);
    assert!(
        longest < Duration::from_millis(20),
        "try_next took {longest:?}"
    );
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn dropping_the_decoder_kills_ffmpeg() {
    let Some(tools) = tools("dropping_the_decoder_kills_ffmpeg") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = testsrc(&tools, dir.path(), "clip.mkv", "64x36", 30, "10");
    let decoder = Decoder::start(
        &tools,
        &clip,
        DecodeOptions {
            queue_frames: 1,
            ..Default::default()
        },
    );
    // Wait for the first frame so ffmpeg is running and blocked on the pipe.
    loop {
        match decoder.next_timeout(WAIT) {
            Some(DecodeEvent::Frame(_)) => break,
            Some(DecodeEvent::Opened(_)) => {}
            other => panic!("{other:?}"),
        }
    }
    let pid = decoder.pid().unwrap();
    assert!(process_exists(pid));
    let dropped = Instant::now();
    drop(decoder);
    assert!(
        dropped.elapsed() < Duration::from_millis(100),
        "drop must not wait"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while process_exists(pid) {
        assert!(Instant::now() < deadline, "ffmpeg {pid} survived the drop");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(any(unix, windows))]
#[test]
fn a_decoder_killed_from_outside_reports_a_crash() {
    let Some(tools) = tools("a_decoder_killed_from_outside_reports_a_crash") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = testsrc(&tools, dir.path(), "clip.mkv", "64x36", 30, "10");
    let decoder = Decoder::start(
        &tools,
        &clip,
        DecodeOptions {
            queue_frames: 1,
            ..Default::default()
        },
    );
    loop {
        match decoder.next_timeout(WAIT) {
            Some(DecodeEvent::Frame(_)) => break,
            Some(DecodeEvent::Opened(_)) => {}
            other => panic!("{other:?}"),
        }
    }
    let pid = decoder.pid().unwrap().to_string();
    #[cfg(unix)]
    let (status, killed) = (
        Command::new("kill").args(["-KILL", &pid]).status().unwrap(),
        "signal",
    );
    #[cfg(windows)]
    let (status, killed) = (
        Command::new("taskkill")
            .args(["/PID", &pid, "/F"])
            .status()
            .unwrap(),
        "exit code",
    );
    assert!(status.success());
    let rest = run_after(&decoder, true);
    match rest.failed {
        Some(MediaError::Crashed(detail)) => assert!(detail.contains(killed), "{detail}"),
        other => panic!(
            "expected a crash, got {other:?} after {} frames",
            rest.frames.len()
        ),
    }
}

/// A fake `ffmpeg` that answers nothing: the watchdog kills it.
#[cfg(unix)]
#[test]
fn a_silent_decoder_is_killed_at_the_stall_timeout() {
    use std::os::unix::fs::PermissionsExt;
    let Some(real) = tools("a_silent_decoder_is_killed_at_the_stall_timeout") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = testsrc(&real, dir.path(), "clip.mkv", "64x36", 30, "1");
    let pid_file = dir.path().join("fake.pid");
    let fake = dir.path().join("ffmpeg");
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\necho $$ > '{}'\nexec sleep 30\n",
            pid_file.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let tools = Tools {
        ffmpeg: fake,
        ffprobe: real.ffprobe.clone(),
    };
    let stall = Duration::from_millis(400);
    let started = Instant::now();
    let result = decode(
        &tools,
        &clip,
        DecodeOptions {
            stall_timeout: stall,
            ..Default::default()
        },
    );
    assert_eq!(result.failed, Some(MediaError::Stalled(stall)));
    assert!(result.opened.is_some() && result.frames.is_empty());
    assert!(started.elapsed() < Duration::from_secs(10));
    #[cfg(target_os = "linux")]
    {
        let pid: u32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while process_exists(pid) {
            assert!(Instant::now() < deadline, "stalled child {pid} survived");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// A fake `ffprobe` that hangs: `start` still returns at once, and the probe
/// timeout ends it.
#[cfg(unix)]
#[test]
fn a_hanging_probe_never_blocks_start_and_times_out() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("ffprobe");
    std::fs::write(&fake, "#!/bin/sh\nexec sleep 30\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let clip = dir.path().join("clip.mkv");
    std::fs::write(&clip, b"not decoded").unwrap();
    let tools = Tools {
        ffmpeg: PathBuf::from("/nonexistent/ffmpeg"),
        ffprobe: fake,
    };
    let started = Instant::now();
    let decoder = Decoder::start(
        &tools,
        &clip,
        DecodeOptions {
            probe_timeout: Duration::from_millis(300),
            ..Default::default()
        },
    );
    assert!(started.elapsed() < Duration::from_millis(50));
    assert!(decoder.try_next().is_none());
    let result = run(&decoder);
    assert_eq!(
        result.failed,
        Some(MediaError::ProbeTimedOut(Duration::from_millis(300)))
    );
}

#[test]
fn a_frame_larger_than_the_budget_is_refused() {
    let Some(tools) = tools("a_frame_larger_than_the_budget_is_refused") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = solid(&tools, dir.path(), "red.mkv", "red", "320x240", 1);
    let result = decode(
        &tools,
        &clip,
        DecodeOptions {
            frame_budget: 320 * 240 * 4 - 1,
            ..Default::default()
        },
    );
    assert_eq!(
        result.failed,
        Some(MediaError::TooLarge {
            width: 320,
            height: 240
        })
    );
    // A budget for two frames lowers the queue to two.
    let result = decode(
        &tools,
        &clip,
        DecodeOptions {
            queue_frames: 8,
            frame_budget: 2 * 320 * 240 * 4,
            ..Default::default()
        },
    );
    assert_eq!(result.opened.unwrap().queue_frames, 2);
}

/// The budget is enforced, not only reported: a consumer that stops
/// reading leaves at most `Opened::queue_frames` frames held by the decoder
/// (review finding: the channel used to keep the requested 8).
#[test]
fn the_frame_budget_bounds_the_frames_held_while_the_consumer_waits() {
    let Some(tools) = tools("the_frame_budget_bounds_the_frames_held_while_the_consumer_waits")
    else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = testsrc(&tools, dir.path(), "clip.mkv", "320x240", 30, "2");
    let frame_bytes = 320 * 240 * 4;
    let decoder = Decoder::start(
        &tools,
        &clip,
        DecodeOptions {
            queue_frames: 8,
            frame_budget: 2 * frame_bytes,
            stall_timeout: Duration::from_millis(300),
            ..Default::default()
        },
    );
    let opened = match decoder.next_timeout(WAIT) {
        Some(DecodeEvent::Opened(opened)) => opened,
        other => panic!("{other:?}"),
    };
    assert_eq!(opened.queue_frames, 2);
    // Do not consume: the decoder fills what it may hold, then waits.
    let mut most = 0;
    let watched = Instant::now();
    while watched.elapsed() < Duration::from_millis(1200) {
        most = most.max(decoder.buffered_frames());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(most, 2, "held frames stay within the budget");
    let rest = run_after(&decoder, true);
    assert_eq!(rest.failed, None, "waiting on the consumer is not a stall");
    assert_eq!(rest.end, Some((60, false)));
    assert_eq!(rest.frames.len(), 60, "no frame lost");
    assert_eq!(decoder.buffered_frames(), 0);
}

/// Tiny frames let far more than the 256 queued frame lines pile up on
/// stderr. Stopping at `max_frames` must still end the decode instead of
/// waiting forever on the stderr thread (review finding).
#[test]
fn stopping_early_with_many_pending_frame_lines_still_ends() {
    let Some(tools) = tools("stopping_early_with_many_pending_frame_lines_still_ends") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = testsrc(&tools, dir.path(), "clip.mkv", "64x36", 30, "20");
    let decoder = Decoder::start(
        &tools,
        &clip,
        DecodeOptions {
            fit: Fit::Within(Extent {
                width: 1,
                height: 1,
            }),
            queue_frames: 1,
            max_frames: Some(2),
            ..Default::default()
        },
    );
    match decoder.next_timeout(WAIT) {
        Some(DecodeEvent::Opened(opened)) => {
            assert_eq!(
                opened.extent,
                Extent {
                    width: 1,
                    height: 1
                }
            );
        }
        other => panic!("{other:?}"),
    }
    // FFmpeg decodes all 600 frames ahead into the pipes meanwhile.
    std::thread::sleep(Duration::from_millis(1500));
    let rest = run_after(&decoder, true);
    assert_eq!(rest.failed, None);
    assert_eq!(rest.frames.len(), 2);
    assert_eq!(rest.end, Some((2, true)));
}

/// Decoders started with the default options never share frame versions.
#[test]
fn default_decoders_get_distinct_content_ids() {
    let Some(tools) = tools("default_decoders_get_distinct_content_ids") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let clip = solid(&tools, dir.path(), "red.mkv", "red", "16x16", 1);
    let first = decode(&tools, &clip, DecodeOptions::default());
    let second = decode(&tools, &clip, DecodeOptions::default());
    let (a, b) = (&first.frames[0].version, &second.frames[0].version);
    assert_ne!(a.id, b.id);
    assert_eq!((a.revision, b.revision), (0, 0));
    assert_eq!(first.opened.unwrap().content_id, a.id);
    assert_ne!(a.id, 0);
}

/// `first_frame`'s own deadline is a timeout, not a decoder stall: a probe
/// still running when it passes is reported as `TimedOut`.
#[cfg(unix)]
#[test]
fn first_frame_reports_its_own_deadline_as_a_timeout() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("ffprobe");
    std::fs::write(&fake, "#!/bin/sh\nexec sleep 30\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let clip = dir.path().join("clip.mkv");
    std::fs::write(&clip, b"not decoded").unwrap();
    let tools = Tools {
        ffmpeg: PathBuf::from("/nonexistent/ffmpeg"),
        ffprobe: fake,
    };
    let timeout = Duration::from_millis(300);
    let started = Instant::now();
    let result = first_frame(
        &tools,
        &clip,
        Extent {
            width: 64,
            height: 36,
        },
        timeout,
    );
    assert_eq!(result.err(), Some(MediaError::TimedOut(timeout)));
    assert!(started.elapsed() < Duration::from_secs(5));
}

/// The system FFmpeg of this Linux distribution is a GPL build: the license
/// check refuses it (VAL-MEDIA-003 negative case), and the pinned build is
/// accepted with its version.
#[test]
fn the_locator_refuses_a_gpl_system_build_and_accepts_the_pinned_one() {
    let system = Path::new("/usr/bin");
    if system.join("ffmpeg").is_file() && system.join("ffprobe").is_file() {
        let result = ffmpeg::locate_with(
            ffmpeg::candidates(Some(system), None, None),
            ffmpeg::probe_build,
        );
        let not_found = result.expect_err("a GPL system build must be refused");
        assert_eq!(not_found.tried[0].refusal, Refusal::Gpl);
    } else {
        eprintln!("SKIPPED GPL half: no /usr/bin/ffmpeg and ffprobe");
    }
    let Some(tools) = tools("the_locator_refuses_a_gpl_system_build_and_accepts_the_pinned_one")
    else {
        return;
    };
    let located = ffmpeg::locate().unwrap();
    assert_eq!(located.ffmpeg, tools.ffmpeg);
    assert!(
        located.version.starts_with("n8.1"),
        "pinned 8.1 build, got {}",
        located.version
    );
}

/// `scripts/fetch-ffmpeg` treats a flag without its value as a usage error
/// (exit 2 with a message), not a silent exit 1 that looks like a failed
/// download (review finding). Nothing is downloaded on these paths.
#[cfg(unix)]
#[test]
fn fetch_ffmpeg_reports_a_missing_flag_value_as_usage() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/fetch-ffmpeg");
    for args in [
        &["--platform"][..],
        &["--dest"],
        &["--platform", ""],
        &["--dest", "/nonexistent/x", "--platform"],
    ] {
        let output = Command::new("bash")
            .arg(&script)
            .args(args)
            .env("SELA_FFMPEG_BASE_URL", "file:///nonexistent")
            .stdin(Stdio::null())
            .output()
            .expect("run bash");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains("needs a value"), "{args:?}: {stderr}");
    }
}
