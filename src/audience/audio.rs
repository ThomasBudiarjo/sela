//! The renderer's audio output for video backgrounds (M2-04, feasible part).
//!
//! One [`Output`] per playing video: the default device through `cpal` on
//! Windows, otherwise (or when no device opens) a null output that consumes
//! samples in real time, so the clock and the decoder behave the same with
//! or without speakers. The output callback ([`render`]) never locks,
//! allocates or blocks: it reads the lock-free [`SampleRing`] and follows
//! the video's media clock through atomics ([`sela::media::audio::sync_step`]).
//! Masks never mute it (best guess, EW8-OBS-006 unobserved).
use sela::{
    media::audio::{AudioFormat, SampleRing, SyncStep, sync_step},
    playback::AudioState,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// Audio may drift this far from the video before it is corrected.
pub const TOLERANCE: Duration = Duration::from_millis(40);
/// Samples buffered ahead of the output: half a second.
const RING: Duration = Duration::from_millis(500);
/// How often the null output consumes.
const NULL_TICK: Duration = Duration::from_millis(10);
/// How long a flush waits for the output to drain the ring.
const FLUSH_WAIT: Duration = Duration::from_millis(200);

/// State shared with the output callback; atomics only.
pub struct Shared {
    pub ring: Arc<SampleRing>,
    playing: AtomicBool,
    /// Where the video is, in frames from the clip's start.
    target: AtomicU64,
    /// Frames consumed from the source (played or skipped).
    played: AtomicU64,
    /// Set by the owner; the callback empties the ring and clears it.
    flush: AtomicBool,
    /// Linear gain as `f32` bits.
    volume: AtomicU32,
    failed: AtomicBool,
    stop: AtomicBool,
    format: AudioFormat,
}

impl Shared {
    fn new(format: AudioFormat) -> Arc<Self> {
        Arc::new(Self {
            ring: Arc::new(SampleRing::new(format.samples(RING) as usize)),
            playing: AtomicBool::new(false),
            target: AtomicU64::new(0),
            played: AtomicU64::new(0),
            flush: AtomicBool::new(false),
            volume: AtomicU32::new(1f32.to_bits()),
            failed: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            format,
        })
    }

    /// Frames consumed from the source so far.
    #[cfg(test)]
    pub fn played(&self) -> u64 {
        self.played.load(Ordering::Acquire)
    }
}

/// Fills one output buffer of interleaved samples. The real-time callback:
/// atomics and the lock-free ring only.
pub fn render(shared: &Shared, out: &mut [f32]) {
    let channels = u64::from(shared.format.channels.max(1));
    if shared.flush.load(Ordering::Acquire) {
        shared.ring.skip(shared.ring.len());
        shared.played.store(0, Ordering::Release);
        shared.flush.store(false, Ordering::Release);
    }
    if !shared.playing.load(Ordering::Acquire) {
        out.fill(0.);
        return;
    }
    let played = shared.played.load(Ordering::Acquire);
    let target = shared.target.load(Ordering::Acquire);
    let tolerance = shared.format.frames(TOLERANCE);
    match sync_step(played, target, tolerance) {
        SyncStep::Wait => {
            out.fill(0.);
            return;
        }
        SyncStep::Skip(frames) => {
            let skipped = shared.ring.skip((frames * channels) as usize) as u64;
            shared
                .played
                .fetch_add(skipped / channels, Ordering::AcqRel);
        }
        SyncStep::Play => {}
    }
    // Whole frames only, so channels never swap.
    let whole = out.len() - out.len() % channels as usize;
    let available = shared.ring.len() - shared.ring.len() % channels as usize;
    let read = shared.ring.pop(&mut out[..whole.min(available)]);
    shared
        .played
        .fetch_add(read as u64 / channels, Ordering::AcqRel);
    out[read..].fill(0.);
    let volume = f32::from_bits(shared.volume.load(Ordering::Relaxed));
    if volume != 1. {
        out[..read].iter_mut().for_each(|s| *s *= volume);
    }
}

/// One video's audio output.
pub struct Output {
    shared: Arc<Shared>,
    device: bool,
}

impl Output {
    /// The default device on Windows, else (or when it does not open) the
    /// null output. Opens on a thread of its own; blocks until it runs.
    pub fn open() -> Output {
        #[cfg(windows)]
        if let Some(output) = device::open() {
            return output;
        }
        Self::null(AudioFormat::DEFAULT)
    }

    /// Consumes in real time without a device.
    pub fn null(format: AudioFormat) -> Output {
        let shared = Shared::new(format);
        let consumer = shared.clone();
        let spawned = std::thread::Builder::new()
            .name("audience-audio-null".into())
            .spawn(move || {
                let mut buffer = Vec::new();
                let mut last = Instant::now();
                while !consumer.stop.load(Ordering::Acquire) {
                    std::thread::sleep(NULL_TICK);
                    let now = Instant::now();
                    let samples = consumer.format.samples(now - last) as usize;
                    last = now;
                    buffer.resize(samples, 0.);
                    render(&consumer, &mut buffer);
                }
            });
        if spawned.is_err() {
            shared.failed.store(true, Ordering::Release);
        }
        Output {
            shared,
            device: false,
        }
    }

    pub fn format(&self) -> AudioFormat {
        self.shared.format
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    /// Follows the video: playing or not, and where it is.
    pub fn follow(&self, playing: bool, position: Duration) {
        self.shared
            .target
            .store(self.shared.format.frames(position), Ordering::Release);
        self.shared.playing.store(playing, Ordering::Release);
    }

    /// Empties the ring and restarts the count (a Restart). Call after the
    /// old decoder stopped pushing; waits (bounded) for the callback.
    pub fn flush(&self) {
        self.shared.playing.store(false, Ordering::Release);
        self.shared.target.store(0, Ordering::Release);
        self.shared.flush.store(true, Ordering::Release);
        let deadline = Instant::now() + FLUSH_WAIT;
        while self.shared.flush.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Frames consumed from the source so far.
    #[cfg(test)]
    pub fn played(&self) -> u64 {
        self.shared.played()
    }

    pub fn state(&self) -> AudioState {
        if self.shared.failed.load(Ordering::Acquire) {
            AudioState::Failed
        } else if self.device {
            AudioState::Playing
        } else {
            AudioState::NoDevice
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        self.shared.playing.store(false, Ordering::Release);
        self.shared.stop.store(true, Ordering::Release);
    }
}

/// The default output device through `cpal` (Windows: WASAPI shared mode).
#[cfg(windows)]
mod device {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    /// Builds the stream on its own thread (a `cpal::Stream` stays on the
    /// thread that made it) and keeps it until the output is dropped.
    pub(super) fn open() -> Option<Output> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("audience-audio".into())
            .spawn(move || {
                let host = cpal::default_host();
                let Some(device) = host.default_output_device() else {
                    let _ = sender.send(None);
                    return;
                };
                let Ok(config) = device.default_output_config() else {
                    let _ = sender.send(None);
                    return;
                };
                let format = AudioFormat {
                    rate: config.sample_rate(),
                    channels: config.channels(),
                };
                let shared = Shared::new(format);
                let (callback, errors) = (shared.clone(), shared.clone());
                // f32 in the stream; WASAPI's shared-mode mix format is float.
                let stream = device.build_output_stream::<f32, _, _>(
                    config.config(),
                    move |data: &mut [f32], _| render(&callback, data),
                    move |_| errors.failed.store(true, Ordering::Release),
                    None,
                );
                let Ok(stream) = stream else {
                    let _ = sender.send(None);
                    return;
                };
                if stream.play().is_err() {
                    let _ = sender.send(None);
                    return;
                }
                let keep = shared.clone();
                if sender.send(Some(shared)).is_err() {
                    return;
                }
                while !keep.stop.load(Ordering::Acquire) {
                    std::thread::park_timeout(Duration::from_millis(100));
                }
                drop(stream);
            })
            .ok()?;
        let shared = receiver.recv_timeout(Duration::from_secs(3)).ok()??;
        Some(Output {
            shared,
            device: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared(format: AudioFormat) -> Arc<Shared> {
        Shared::new(format)
    }

    #[test]
    fn paused_or_flushed_output_is_silent_and_reads_nothing() {
        let format = AudioFormat {
            rate: 1000,
            channels: 2,
        };
        let shared = shared(format);
        shared.ring.push(&[0.5; 40]);
        let mut out = [1f32; 10];
        render(&shared, &mut out);
        assert_eq!(out, [0.; 10], "not playing yet");
        assert_eq!(shared.ring.len(), 40);
        shared.playing.store(true, Ordering::Release);
        shared.target.store(0, Ordering::Release);
        render(&shared, &mut out);
        assert_eq!(out, [0.5; 10]);
        assert_eq!(shared.played.load(Ordering::Acquire), 5);
        shared.flush.store(true, Ordering::Release);
        render(&shared, &mut out);
        assert!(shared.ring.is_empty() && !shared.flush.load(Ordering::Acquire));
        assert_eq!(shared.played.load(Ordering::Acquire), 0);
        assert_eq!(out, [0.; 10], "underrun is silence");
    }

    #[test]
    fn behind_the_video_skips_ahead_and_ahead_waits_in_silence() {
        let format = AudioFormat {
            rate: 1000,
            channels: 1,
        };
        let shared = shared(format);
        let samples: Vec<f32> = (0..400).map(|v| v as f32).collect();
        shared.ring.push(&samples);
        shared.playing.store(true, Ordering::Release);
        // The video is at frame 100; tolerance at 1 kHz is 40 frames.
        shared.target.store(100, Ordering::Release);
        let mut out = [0f32; 4];
        render(&shared, &mut out);
        assert_eq!(out, [100., 101., 102., 103.], "skipped to the video");
        // Ahead: the video went back (or stood still) by more than 40.
        shared.target.store(50, Ordering::Release);
        render(&shared, &mut out);
        assert_eq!(out, [0.; 4]);
        assert_eq!(shared.played.load(Ordering::Acquire), 104, "nothing read");
        // Within the tolerance it just plays on.
        shared.target.store(120, Ordering::Release);
        render(&shared, &mut out);
        assert_eq!(out, [104., 105., 106., 107.]);
    }

    #[test]
    fn whole_frames_only_and_volume_scales() {
        let format = AudioFormat {
            rate: 1000,
            channels: 2,
        };
        let shared = shared(format);
        shared.ring.push(&[1., -1., 1.]);
        shared.playing.store(true, Ordering::Release);
        shared.volume.store(0.5f32.to_bits(), Ordering::Release);
        let mut out = [9f32; 4];
        render(&shared, &mut out);
        assert_eq!(out, [0.5, -0.5, 0., 0.], "the half frame waits");
        assert_eq!(shared.ring.len(), 1);
    }

    #[test]
    fn the_null_output_consumes_in_real_time_while_playing() {
        let format = AudioFormat {
            rate: 8000,
            channels: 2,
        };
        let output = Output::null(format);
        assert_eq!(output.state(), AudioState::NoDevice);
        let shared = output.shared().clone();
        // Keep the ring fed for a second of playback.
        let feeder = std::thread::spawn(move || {
            let chunk = [0.25f32; 160];
            let end = Instant::now() + Duration::from_millis(1100);
            while Instant::now() < end {
                shared.ring.push(&chunk);
                std::thread::sleep(Duration::from_millis(2));
            }
        });
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(1) {
            output.follow(true, start.elapsed());
            std::thread::sleep(Duration::from_millis(5));
        }
        let played = output.played();
        feeder.join().unwrap();
        // About one second of frames, within the tolerance and scheduling.
        assert!((7000..=8800).contains(&played), "{played}");
        output.follow(false, start.elapsed());
        std::thread::sleep(Duration::from_millis(50));
        let paused = output.played();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(output.played(), paused, "paused reads nothing");
    }
}
