//! Audio of a playing video (M2-04, feasible part): a supervised FFmpeg
//! child decodes the audio stream to interleaved 32-bit float PCM at the
//! output's rate and channel count, into a lock-free single-producer,
//! single-consumer ring ([`SampleRing`]) the output callback reads without
//! locking or allocating. [`sync_step`] keeps the audio within a tolerance
//! of the video's media clock, which is the master. No GPUI or device types:
//! the renderer owns the output (`cpal` on Windows, a null output elsewhere
//! and in tests). Design: docs/media.md "Audio of video backgrounds".
use super::MediaError;
use super::decode::Tools;
use super::ffmpeg;
use super::process::Supervised;
use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

/// The rate and channel count FFmpeg resamples to: the output's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioFormat {
    pub rate: u32,
    pub channels: u16,
}

impl AudioFormat {
    /// What the null output uses (and FFmpeg's usual output).
    pub const DEFAULT: Self = Self {
        rate: 48_000,
        channels: 2,
    };

    /// Samples (all channels) in `duration`.
    pub fn samples(self, duration: Duration) -> u64 {
        self.frames(duration) * u64::from(self.channels)
    }

    /// Frames (one sample per channel) in `duration`.
    pub fn frames(self, duration: Duration) -> u64 {
        (duration.as_nanos() * u128::from(self.rate) / 1_000_000_000) as u64
    }

    pub fn duration(self, frames: u64) -> Duration {
        Duration::from_nanos(
            (u128::from(frames) * 1_000_000_000 / u128::from(self.rate.max(1))) as u64,
        )
    }
}

/// Bounded single-producer, single-consumer ring of `f32` samples stored as
/// bits in atomics: the decoder thread pushes, the output callback pops,
/// neither locks, allocates nor blocks.
pub struct SampleRing {
    slots: Box<[AtomicU32]>,
    /// Samples ever written (producer only).
    written: AtomicU64,
    /// Samples ever read or skipped (consumer only).
    read: AtomicU64,
}

impl SampleRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: (0..capacity.max(1)).map(|_| AtomicU32::new(0)).collect(),
            written: AtomicU64::new(0),
            read: AtomicU64::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Samples ready to read.
    pub fn len(&self) -> usize {
        (self.written.load(Ordering::Acquire) - self.read.load(Ordering::Acquire)) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Producer: copies as many of `samples` as fit; returns how many.
    pub fn push(&self, samples: &[f32]) -> usize {
        let written = self.written.load(Ordering::Relaxed);
        let read = self.read.load(Ordering::Acquire);
        let free = self.slots.len() - (written - read) as usize;
        let count = free.min(samples.len());
        for (offset, sample) in samples[..count].iter().enumerate() {
            let at = ((written + offset as u64) % self.slots.len() as u64) as usize;
            self.slots[at].store(sample.to_bits(), Ordering::Relaxed);
        }
        self.written
            .store(written + count as u64, Ordering::Release);
        count
    }

    /// Consumer: fills the front of `out`; returns how many were read.
    pub fn pop(&self, out: &mut [f32]) -> usize {
        let read = self.read.load(Ordering::Relaxed);
        let written = self.written.load(Ordering::Acquire);
        let count = ((written - read) as usize).min(out.len());
        for (offset, sample) in out[..count].iter_mut().enumerate() {
            let at = ((read + offset as u64) % self.slots.len() as u64) as usize;
            *sample = f32::from_bits(self.slots[at].load(Ordering::Relaxed));
        }
        self.read.store(read + count as u64, Ordering::Release);
        count
    }

    /// Consumer: drops up to `count` samples; returns how many.
    pub fn skip(&self, count: usize) -> usize {
        let read = self.read.load(Ordering::Relaxed);
        let written = self.written.load(Ordering::Acquire);
        let count = ((written - read) as usize).min(count);
        self.read.store(read + count as u64, Ordering::Release);
        count
    }
}

/// What the output callback does with the next buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncStep {
    /// In step with the video: play the next samples.
    Play,
    /// Ahead of the video by more than the tolerance: output silence and
    /// read nothing until the video catches up.
    Wait,
    /// Behind by more than the tolerance: drop this many frames first.
    Skip(u64),
}

/// Audio follows the video's clock. `played` is the frames the output has
/// consumed from the source, `target` where the video is, both in frames.
pub fn sync_step(played: u64, target: u64, tolerance: u64) -> SyncStep {
    if played + tolerance < target {
        SyncStep::Skip(target - played)
    } else if played > target + tolerance {
        SyncStep::Wait
    } else {
        SyncStep::Play
    }
}

/// The `ffmpeg` arguments that decode `input`'s first audio stream to raw
/// interleaved `f32le` at `format`; `looping` repeats it like the video.
pub fn audio_args(
    input: &Path,
    format: AudioFormat,
    looping: bool,
) -> Result<Vec<OsString>, MediaError> {
    let mut args: Vec<OsString> = ["-loglevel", "error", "-nostats"]
        .into_iter()
        .map(OsString::from)
        .collect();
    if looping {
        args.extend(["-stream_loop", "-1"].map(OsString::from));
    }
    args.extend(ffmpeg::input_args(input).map_err(MediaError::Input)?);
    let rate = format.rate.to_string();
    let channels = format.channels.to_string();
    args.extend(
        [
            "-map",
            "0:a:0",
            "-vn",
            "-sn",
            "-dn",
            "-ac",
            channels.as_str(),
            "-ar",
            rate.as_str(),
            "-f",
            "f32le",
            "pipe:1",
        ]
        .into_iter()
        .map(OsString::from),
    );
    Ok(args)
}

/// Where an audio decode stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DecodeState {
    Running = 0,
    /// The source ran out (play once); the ring drains to silence.
    Ended = 1,
    /// FFmpeg could not start or failed; the video plays on silently.
    Failed = 2,
}

struct Shared {
    cancelled: AtomicBool,
    state: AtomicU8,
    child: std::sync::Mutex<Option<Supervised>>,
}

/// One running audio decode into a [`SampleRing`]. Dropping it kills FFmpeg
/// without waiting.
pub struct AudioDecoder {
    shared: Arc<Shared>,
}

/// How long the decoder waits for room in a full ring before looking again.
const ROOM_TICK: Duration = Duration::from_millis(5);

impl AudioDecoder {
    pub fn start(
        tools: &Tools,
        input: &Path,
        format: AudioFormat,
        looping: bool,
        ring: Arc<SampleRing>,
    ) -> AudioDecoder {
        let shared = Arc::new(Shared {
            cancelled: AtomicBool::new(false),
            state: AtomicU8::new(DecodeState::Running as u8),
            child: std::sync::Mutex::new(None),
        });
        let job = shared.clone();
        let (ffmpeg, input) = (tools.ffmpeg.clone(), input.to_path_buf());
        let spawned = std::thread::Builder::new()
            .name("sela-audio-decoder".into())
            .spawn(move || {
                let state = run(&ffmpeg, &input, format, looping, &ring, &job);
                job.state.store(state as u8, Ordering::Release);
                if let Some(mut child) = job.child.lock().ok().and_then(|mut c| c.take()) {
                    child.kill_and_wait();
                }
            });
        if spawned.is_err() {
            shared
                .state
                .store(DecodeState::Failed as u8, Ordering::Release);
        }
        AudioDecoder { shared }
    }

    /// Stops the decode and waits (bounded) until its thread pushes no
    /// more samples, so a ring can be flushed after it. Worker threads only.
    pub fn stop(self, timeout: Duration) {
        self.shared.cancelled.store(true, Ordering::Release);
        if let Ok(mut child) = self.shared.child.lock()
            && let Some(child) = child.as_mut()
        {
            child.kill();
        }
        let deadline = std::time::Instant::now() + timeout;
        while self.state() == DecodeState::Running && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn state(&self) -> DecodeState {
        match self.shared.state.load(Ordering::Acquire) {
            0 => DecodeState::Running,
            1 => DecodeState::Ended,
            _ => DecodeState::Failed,
        }
    }
}

impl Drop for AudioDecoder {
    fn drop(&mut self) {
        self.shared.cancelled.store(true, Ordering::Release);
        if let Ok(mut child) = self.shared.child.lock()
            && let Some(child) = child.as_mut()
        {
            child.kill();
        }
    }
}

fn run(
    ffmpeg: &Path,
    input: &Path,
    format: AudioFormat,
    looping: bool,
    ring: &SampleRing,
    shared: &Shared,
) -> DecodeState {
    let Ok(args) = audio_args(input, format, looping) else {
        return DecodeState::Failed;
    };
    let mut command = Command::new(ffmpeg);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Ok(mut child) = Supervised::spawn(command) else {
        return DecodeState::Failed;
    };
    let Some(mut stdout) = child.take_stdout() else {
        return DecodeState::Failed;
    };
    {
        let Ok(mut slot) = shared.child.lock() else {
            return DecodeState::Failed;
        };
        *slot = Some(child);
    }
    let mut bytes = vec![0u8; 16 * 1024];
    let mut samples: Vec<f32> = Vec::with_capacity(bytes.len() / 4);
    // A sample split across two reads.
    let mut carry: Vec<u8> = Vec::with_capacity(4);
    loop {
        if shared.cancelled.load(Ordering::Acquire) {
            return DecodeState::Ended;
        }
        let read = match stdout.read(&mut bytes) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return DecodeState::Failed,
        };
        samples.clear();
        let mut chunk = &bytes[..read];
        if !carry.is_empty() {
            let need = 4 - carry.len();
            let take = need.min(chunk.len());
            carry.extend_from_slice(&chunk[..take]);
            chunk = &chunk[take..];
            if carry.len() == 4 {
                samples.push(f32::from_le_bytes(carry[..4].try_into().unwrap()));
                carry.clear();
            }
        }
        let (whole, rest) = chunk.as_chunks::<4>();
        samples.extend(whole.iter().map(|b| f32::from_le_bytes(*b)));
        carry.extend_from_slice(rest);
        let mut pending = &samples[..];
        while !pending.is_empty() {
            if shared.cancelled.load(Ordering::Acquire) {
                return DecodeState::Ended;
            }
            let pushed = ring.push(pending);
            pending = &pending[pushed..];
            if !pending.is_empty() {
                // Full: the output is playing what is buffered (backpressure
                // stops FFmpeg through the pipe meanwhile).
                std::thread::sleep(ROOM_TICK);
            }
        }
    }
    // stdout ended: FFmpeg is exiting. Poll briefly, never holding the lock
    // while waiting, so a drop can always kill it at once.
    let mut status = None;
    for _ in 0..200 {
        status = shared
            .child
            .lock()
            .ok()
            .and_then(|mut child| child.as_mut().and_then(|c| c.try_wait().ok().flatten()));
        if status.is_some() || shared.cancelled.load(Ordering::Acquire) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    match status {
        Some(status) if status.success() => DecodeState::Ended,
        _ if shared.cancelled.load(Ordering::Acquire) => DecodeState::Ended,
        _ => DecodeState::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_keeps_order_across_the_wrap_and_never_overfills() {
        let ring = SampleRing::new(5);
        assert_eq!(ring.push(&[1., 2., 3.]), 3);
        let mut out = [0f32; 2];
        assert_eq!(ring.pop(&mut out), 2);
        assert_eq!(out, [1., 2.]);
        // 1 left, 4 free: only 4 of 6 fit.
        assert_eq!(ring.push(&[4., 5., 6., 7., 8., 9.]), 4);
        assert_eq!(ring.len(), 5);
        assert_eq!(ring.push(&[10.]), 0, "full");
        let mut all = [0f32; 8];
        assert_eq!(ring.pop(&mut all), 5);
        assert_eq!(all[..5], [3., 4., 5., 6., 7.]);
        assert!(ring.is_empty());
        assert_eq!(ring.pop(&mut all), 0);
        ring.push(&[1., 2., 3.]);
        assert_eq!(ring.skip(2), 2);
        assert_eq!(ring.skip(9), 1);
        assert!(ring.is_empty());
    }

    #[test]
    fn a_producer_and_a_consumer_thread_see_every_sample_once_in_order() {
        let ring = Arc::new(SampleRing::new(64));
        let producer = {
            let ring = ring.clone();
            std::thread::spawn(move || {
                let mut next = 0u32;
                while next < 20_000 {
                    let batch: Vec<f32> =
                        (next..(next + 37).min(20_000)).map(|v| v as f32).collect();
                    let mut pending = &batch[..];
                    while !pending.is_empty() {
                        let pushed = ring.push(pending);
                        pending = &pending[pushed..];
                        std::thread::yield_now();
                    }
                    next += 37;
                }
            })
        };
        let mut expected = 0u32;
        let mut out = [0f32; 23];
        while expected < 20_000 {
            let read = ring.pop(&mut out);
            for sample in &out[..read] {
                assert_eq!(*sample, expected as f32);
                expected += 1;
            }
            std::thread::yield_now();
        }
        producer.join().unwrap();
    }

    #[test]
    fn audio_follows_the_video_clock_within_the_tolerance() {
        assert_eq!(sync_step(1000, 1000, 480), SyncStep::Play);
        assert_eq!(sync_step(1000, 1480, 480), SyncStep::Play);
        assert_eq!(sync_step(1000, 1481, 480), SyncStep::Skip(481));
        assert_eq!(sync_step(1481, 1000, 480), SyncStep::Wait);
        assert_eq!(sync_step(0, 0, 0), SyncStep::Play);
        let format = AudioFormat::DEFAULT;
        assert_eq!(format.frames(Duration::from_millis(40)), 1920);
        assert_eq!(format.samples(Duration::from_millis(40)), 3840);
        assert_eq!(format.duration(48_000), Duration::from_secs(1));
    }

    #[test]
    #[cfg(not(windows))]
    fn audio_args_decode_the_first_audio_stream_to_float_pcm_and_loop_before_the_input() {
        let args = audio_args(Path::new("/media/a.mkv"), AudioFormat::DEFAULT, true).unwrap();
        let args: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let joined = args.join(" ");
        assert!(
            joined.starts_with(
                "-loglevel error -nostats -stream_loop -1 -nostdin -hide_banner \
                 -protocol_whitelist file -i /media/a.mkv -map 0:a:0 -vn"
            ),
            "{joined}"
        );
        assert!(
            joined.ends_with("-ac 2 -ar 48000 -f f32le pipe:1"),
            "{joined}"
        );
        assert_eq!(
            audio_args(Path::new("http://x/a.mkv"), AudioFormat::DEFAULT, false),
            Err(MediaError::Input(ffmpeg::InputRefused::Url))
        );
    }

    /// The located FFmpeg decodes a 440 Hz tone; skipped without it unless
    /// `SELA_REQUIRE_FFMPEG=1`.
    #[test]
    fn a_tone_decodes_to_float_pcm_at_its_frequency_and_ends() {
        let tools = match ffmpeg::locate() {
            Ok(found) => Tools::from(&found),
            Err(error) => {
                let message = format!("SKIPPED a_tone_decodes_to_float_pcm: {error:?}");
                assert!(
                    std::env::var_os("SELA_REQUIRE_FFMPEG").is_none_or(|v| v != "1"),
                    "{message}"
                );
                eprintln!("{message}");
                return;
            }
        };
        let dir = tempfile::tempdir().unwrap();
        let clip = dir.path().join("tone.mkv");
        let status = Command::new(&tools.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=0.5"])
            .args(["-c:a", "pcm_s16le"])
            .arg(&clip)
            .status()
            .unwrap();
        assert!(status.success());
        let format = AudioFormat {
            rate: 48_000,
            channels: 2,
        };
        // Room for the whole clip: no consumer needed.
        let ring = Arc::new(SampleRing::new(
            format.samples(Duration::from_secs(1)) as usize
        ));
        let decoder = AudioDecoder::start(&tools, &clip, format, false, ring.clone());
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while decoder.state() == DecodeState::Running {
            assert!(std::time::Instant::now() < deadline, "decode finished");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(decoder.state(), DecodeState::Ended);
        let mut samples = vec![0f32; ring.len()];
        ring.pop(&mut samples);
        // Half a second of stereo at 48 kHz (FFmpeg pads to whole frames).
        let frames = samples.len() / 2;
        assert!((23_500..=24_600).contains(&frames), "{frames}");
        // The left channel crosses zero upwards 440 times a second.
        let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
        let rising = left.windows(2).filter(|w| w[0] < 0. && w[1] >= 0.).count();
        assert!(
            (215..=225).contains(&rising),
            "{rising} rising zero crossings"
        );
        // Missing input: the decoder fails, nothing is pushed.
        let empty = Arc::new(SampleRing::new(16));
        let missing = AudioDecoder::start(
            &tools,
            &dir.path().join("gone.mkv"),
            format,
            false,
            empty.clone(),
        );
        while missing.state() == DecodeState::Running {
            assert!(std::time::Instant::now() < deadline, "failure reported");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(missing.state(), DecodeState::Failed);
        assert!(empty.is_empty());
    }
}
