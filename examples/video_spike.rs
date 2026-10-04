//! Opt-in FFmpeg / offscreen Vulkan integration; no native playback claims.
#[path = "video/decoder.rs"]
mod decoder;
#[path = "composition/gpu.rs"]
mod gpu;
#[path = "composition/text.rs"]
mod text;
use sela::scene::Extent;
use std::{
    error::Error,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const W: u32 = 320;
const H: u32 = 180;
const N: usize = 12;
fn original(index: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let c = if (20 + index as u32 * 17..60 + index as u32 * 17).contains(&x)
                && (90..147).contains(&y)
            {
                [231, 37, 91, 255]
            } else if x < 13 && y > 70 {
                [19, 201, 67, 255]
            } else {
                [23 + index as u8, 51, 113, 255]
            };
            v.extend(c);
        }
    }
    v
}
fn wait(d: &decoder::Decoder) -> Result<(), decoder::Failure> {
    let start = Instant::now();
    loop {
        if let Some(r) = d.completion() {
            return r;
        }
        assert!(start.elapsed() < Duration::from_secs(32));
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn encode(command: &mut Command, deadline: Duration) -> Result<(), Box<dyn Error>> {
    let mut child = command.stdin(Stdio::null()).spawn()?;
    let start = Instant::now();
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err("fixture encoder failed".into()),
            Err(error) => break Err(error.into()),
            Ok(None) if start.elapsed() >= deadline => {
                break Err("fixture encoder deadline".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("usage: video_spike OUTPUT_DIRECTORY".into());
    }
    let tmp = tempfile::tempdir()?;
    let raw = tmp.path().join("original.rgba");
    let video = tmp.path().join("original.mkv");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&raw)?;
        for i in 0..N {
            f.write_all(&original(i))?;
        }
    }
    // Fixture encoding is setup only; decoder spawn/read/wait are worker-owned.
    let mut encoder = Command::new("ffmpeg");
    encoder
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-xerror",
            "-protocol_whitelist",
            "file",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            "320x180",
            "-framerate",
            "12",
            "-threads",
            "1",
            "-i",
        ])
        .arg(&raw)
        .args([
            "-map",
            "0:v:0",
            "-filter_threads",
            "1",
            "-c:v",
            "ffv1",
            "-pix_fmt",
            "bgra",
            "-threads",
            "1",
            "-frames:v",
            "12",
            "-f",
            "matroska",
        ])
        .arg(&video);
    encode(&mut encoder, Duration::from_secs(10))?;
    let alpha = text::raster(
        include_bytes!("../tests/fixtures/DejaVuSans.ttf"),
        "Video · Café",
        W,
        H,
        24.,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(alpha.contains(&255));
    let gpu = gpu::Compositor::new(wgpu::Backends::VULKAN)?;
    println!("adapter={:?}", gpu.adapter_info());
    gpu.check_readback()?;
    std::fs::create_dir_all(&args[0])?;
    let output = std::path::Path::new(&args[0]);
    let d = decoder::Decoder::start(video.clone(), W, H, N, Duration::from_secs(10))
        .map_err(|e| format!("{e:?}"))?;
    let mut accepted = Vec::new();
    let start = Instant::now();
    let mut completion = None;
    for index in 0..N {
        let frame = loop {
            if let Some(f) = d.poll() {
                break f;
            }
            if let Some(r) = d.completion() {
                if r.is_ok() {
                    // The last frame can arrive between poll and completion.
                    // Completion is published after all successful sends.
                    completion = Some(r);
                    if let Some(f) = d.poll() {
                        break f;
                    }
                }
                return Err("decoder completed without the requested frame".into());
            }
            if start.elapsed() > Duration::from_secs(12) {
                return Err("frame timeout".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(frame.index, index);
        assert_eq!(frame.rgba, original(index));
        accepted = gpu.render(
            Extent {
                width: W,
                height: H,
            },
            gpu::Image {
                width: W,
                height: H,
                rgba: &frame.rgba,
            },
            &alpha,
            gpu::Fit::Cover,
        )?;
        for ((p, s), a) in accepted
            .as_chunks::<4>()
            .0
            .iter()
            .zip(frame.rgba.as_chunks::<4>().0.iter())
            .zip(&alpha)
        {
            if *a == 255 {
                assert_eq!(p, &[255; 4]);
            }
            if *a == 0 {
                assert!(p.iter().zip(s).all(|(a, b)| a.abs_diff(*b) <= 1));
            }
        }
        if index == 0 || index == N - 1 {
            image::save_buffer(
                output.join(format!("frame-{index:02}.png")),
                &accepted,
                W,
                H,
                image::ColorType::Rgba8,
            )?;
        }
    }
    assert_eq!(completion.unwrap_or_else(|| wait(&d)), Ok(()));
    let last = accepted.clone();
    let corrupt = tmp.path().join("corrupt.mkv");
    std::fs::write(&corrupt, b"not matroska")?;
    for (path, expected) in [
        (tmp.path().join("missing.mkv"), decoder::Failure::Input),
        (corrupt, decoder::Failure::Decoder),
    ] {
        let bad = decoder::Decoder::start(path, W, H, N, Duration::from_secs(2)).unwrap();
        assert_eq!(wait(&bad), Err(expected));
        assert!(bad.poll().is_none());
        assert_eq!(accepted, last);
    }
    for cancel in [false, true] {
        let full = decoder::Decoder::start(video.clone(), W, H, N, Duration::from_secs(2)).unwrap();
        let gate = Instant::now();
        while !full.saturated() {
            assert!(gate.elapsed() < Duration::from_secs(1));
            std::thread::sleep(Duration::from_millis(1));
        }
        if cancel {
            full.cancel();
        }
        assert_eq!(
            wait(&full),
            Err(if cancel {
                decoder::Failure::Cancelled
            } else {
                decoder::Failure::Deadline
            })
        );
        assert!(full.poll().is_none());
    }
    println!(
        "PASS: 12 exact ordered RGBA frames, actual text GPU pixels; missing/corrupt; saturated deadline/cancel and reaped completion; last accepted preserved"
    );
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn encoder_failure_and_hung_child_are_bounded() {
        for (script, expected) in [
            ("exit 7", "fixture encoder failed"),
            ("exec sleep 10", "fixture encoder deadline"),
        ] {
            let start = Instant::now();
            let error = encode(
                Command::new("sh").args(["-c", script]),
                Duration::from_millis(30),
            )
            .unwrap_err();
            assert_eq!(error.to_string(), expected);
            assert!(start.elapsed() < Duration::from_secs(2));
        }
    }
}
