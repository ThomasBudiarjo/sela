//! Supervised video decoding into a bounded, timestamped frame queue
//! (M2-02). Design and API contract: docs/media.md, "Decoder".
//!
//! [`Decoder::start`] returns at once; a supervisor thread probes the file
//! with ffprobe, starts `ffmpeg` in its own kill domain
//! ([`super::process::Supervised`]) and reads raw RGBA frames from its
//! stdout. A `showinfo` filter at the end of the filter chain prints each
//! output frame's number, timestamp and size on stderr, so every frame
//! carries its own presentation time even for variable frame rate media.
//!
//! Events arrive in order: [`DecodeEvent::Opened`], any number of
//! [`DecodeEvent::Frame`], then exactly one terminal event
//! ([`DecodeEvent::End`] or [`DecodeEvent::Failed`]). The queue holds at most
//! [`Opened::queue_frames`] events; when it is full the reader stops reading,
//! the pipe fills and FFmpeg blocks (backpressure). A full queue never counts
//! as a stall: only time spent waiting on FFmpeg does. Dropping the
//! [`Decoder`] cancels: the child tree is killed at once and the threads end
//! on their own, so a consumer on the UI thread or frame path never waits.

use super::error::{MediaError, classify};
use super::ffmpeg::{self, Located};
use super::probe::{self, MediaInfo, VideoInfo};
use super::process::Supervised;
use crate::scene::{ContentVersion, Extent, PreparedBackground};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::fmt;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Upper bound on [`DecodeOptions::queue_frames`].
pub const MAX_QUEUE_FRAMES: usize = 16;
pub const DEFAULT_QUEUE_FRAMES: usize = 4;
/// Default bound on the bytes of queued frames: four 4K RGBA frames.
pub const DEFAULT_FRAME_BUDGET: usize = 4 * 3840 * 2160 * 4;
pub const DEFAULT_STALL_TIMEOUT: Duration = Duration::from_secs(5);
/// Largest output side in pixels; also the renderer's texture ceiling class.
pub const MAX_DIMENSION: u32 = 8192;
/// Longest stderr line kept; longer lines are cut.
const STDERR_LINE_LIMIT: usize = 4096;
/// Diagnostic stderr lines kept for the failure message.
const STDERR_TAIL_LINES: usize = 32;
/// Frame metadata may run ahead of frame bytes by FFmpeg's internal queues.
const META_QUEUE: usize = 256;
/// After stdout ends, how long FFmpeg gets to exit before it is killed.
const EXIT_GRACE: Duration = Duration::from_secs(5);
const WATCHDOG_TICK: Duration = Duration::from_millis(20);

/// How decoded pictures are sized. Output is always tightly packed RGBA8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// The display size (sample aspect and rotation applied), shrunk only
    /// to stay within [`MAX_DIMENSION`].
    Source,
    /// The display size shrunk, never enlarged, to fit inside the extent.
    Within(Extent),
    /// Exactly this extent: scaled to fit, aspect kept, centered on black
    /// (letterbox or pillarbox). Matches a cue's output extent.
    Exact(Extent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeOptions {
    pub fit: Fit,
    /// Frames the queue may hold, 1..=[`MAX_QUEUE_FRAMES`]; lowered further
    /// so the queue fits `frame_budget`.
    pub queue_frames: usize,
    /// Bytes the queued frames may occupy. A single frame larger than this
    /// fails with [`MediaError::TooLarge`].
    pub frame_budget: usize,
    /// How long FFmpeg may produce nothing while Sela waits for it.
    pub stall_timeout: Duration,
    pub probe_timeout: Duration,
    /// Stop after this many frames (a poster frame is `Some(1)`).
    pub max_frames: Option<u64>,
    /// `ContentVersion::id` of the frames; the revision is the frame index.
    pub content_id: u128,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            fit: Fit::Source,
            queue_frames: DEFAULT_QUEUE_FRAMES,
            frame_budget: DEFAULT_FRAME_BUDGET,
            stall_timeout: DEFAULT_STALL_TIMEOUT,
            probe_timeout: probe::PROBE_TIMEOUT,
            max_frames: None,
            content_id: 0,
        }
    }
}

/// The two executables of one located FFmpeg build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl From<&Located> for Tools {
    fn from(located: &Located) -> Self {
        Self {
            ffmpeg: located.ffmpeg.clone(),
            ffprobe: located.ffprobe.clone(),
        }
    }
}

/// One decoded picture, immutable and cheap to clone.
#[derive(Clone)]
pub struct VideoFrame {
    /// 0-based output frame number.
    pub index: u64,
    /// Presentation time relative to the first frame. Strictly increasing.
    pub pts: Duration,
    /// FFmpeg's frame duration, when known.
    pub duration: Option<Duration>,
    /// The source timestamp was missing or not after the previous one, so
    /// `pts` was derived from the previous frame and the nominal duration.
    pub repaired: bool,
    pub extent: Extent,
    /// `extent.width * extent.height * 4` bytes, rows top to bottom.
    pub rgba: Arc<[u8]>,
    pub version: ContentVersion,
}

impl VideoFrame {
    /// This frame as a cue background. Shares the pixels; no copy. A cue
    /// accepts it only when `extent` equals the cue's extent, so decode with
    /// [`Fit::Exact`] for that.
    pub fn background(&self) -> PreparedBackground {
        PreparedBackground::Image {
            version: self.version,
            extent: self.extent,
            rgba: self.rgba.clone(),
        }
    }

    /// RGBA of the pixel at (x, y).
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * self.extent.width + x) * 4) as usize;
        [
            self.rgba[at],
            self.rgba[at + 1],
            self.rgba[at + 2],
            self.rgba[at + 3],
        ]
    }
}

// Frames deliberately print no pixels.
impl fmt::Debug for VideoFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VideoFrame")
            .field("index", &self.index)
            .field("pts", &self.pts)
            .field("repaired", &self.repaired)
            .field("extent", &self.extent)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Opened {
    pub info: MediaInfo,
    /// Size of every frame that follows.
    pub extent: Extent,
    /// Queue capacity after the budget was applied.
    pub queue_frames: usize,
}

#[derive(Debug)]
pub enum DecodeEvent {
    Opened(Opened),
    Frame(VideoFrame),
    /// The decoder finished: the end of the stream, or `max_frames` reached
    /// (`limited`).
    End {
        frames: u64,
        limited: bool,
    },
    /// Terminal failure. Frames delivered before it stay valid.
    Failed(MediaError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Phase {
    /// ffprobe runs with its own timeout.
    Probing = 0,
    /// Waiting on FFmpeg: the stall clock runs.
    Producing = 1,
    /// Waiting for the consumer to make room: no stall.
    Blocked = 2,
    Finished = 3,
}

struct Shared {
    cancelled: AtomicBool,
    stalled: AtomicBool,
    phase: AtomicU8,
    /// Milliseconds since `start` of the last byte or line from FFmpeg.
    progress: AtomicU64,
    start: Instant,
    pid: AtomicU32,
    child: Mutex<Option<Supervised>>,
}

impl Shared {
    fn set_phase(&self, phase: Phase) {
        self.phase.store(phase as u8, Ordering::SeqCst);
    }

    fn touch(&self) {
        self.progress
            .store(self.start.elapsed().as_millis() as u64, Ordering::SeqCst);
    }

    fn kill(&self) {
        if let Ok(mut child) = self.child.lock()
            && let Some(child) = child.as_mut()
        {
            child.kill();
        }
    }
}

/// A running decode: the video source handle a compositor or preparation
/// worker consumes. Not tied to any UI framework.
pub struct Decoder {
    events: Receiver<DecodeEvent>,
    shared: Arc<Shared>,
}

impl Decoder {
    /// Starts decoding `input` on a supervisor thread and returns at once.
    /// A refused path or option fails through the event queue like every
    /// other error, so callers have one failure path.
    pub fn start(tools: &Tools, input: &Path, options: DecodeOptions) -> Decoder {
        let queue = options.queue_frames.clamp(1, MAX_QUEUE_FRAMES);
        let (sender, events) = mpsc::sync_channel(queue);
        let shared = Arc::new(Shared {
            cancelled: AtomicBool::new(false),
            stalled: AtomicBool::new(false),
            phase: AtomicU8::new(Phase::Probing as u8),
            progress: AtomicU64::new(0),
            start: Instant::now(),
            pid: AtomicU32::new(0),
            child: Mutex::new(None),
        });
        let job = Job {
            tools: tools.clone(),
            input: input.to_path_buf(),
            options,
            shared: shared.clone(),
            events: sender,
        };
        std::thread::Builder::new()
            .name("sela-decoder".into())
            .spawn(move || job.run())
            .expect("spawn decoder supervisor thread");
        Decoder { events, shared }
    }

    /// The next event if one is queued. Never blocks: for the UI thread and
    /// the frame path.
    pub fn try_next(&self) -> Option<DecodeEvent> {
        self.events.try_recv().ok()
    }

    /// Waits up to `timeout` for the next event. Worker threads only.
    pub fn next_timeout(&self, timeout: Duration) -> Option<DecodeEvent> {
        self.events.recv_timeout(timeout).ok()
    }

    /// The running `ffmpeg` child's process id, for diagnostics.
    pub fn pid(&self) -> Option<u32> {
        Some(self.shared.pid.load(Ordering::SeqCst)).filter(|pid| *pid != 0)
    }

    /// Stops decoding and kills the child tree; same as dropping.
    pub fn cancel(self) {}
}

impl Drop for Decoder {
    fn drop(&mut self) {
        self.shared.cancelled.store(true, Ordering::SeqCst);
        self.shared.kill();
    }
}

/// Decodes the first frame of `input` fitted to `extent` (a poster frame or
/// a still background). Blocks until it lands or fails: worker threads only.
pub fn first_frame(
    tools: &Tools,
    input: &Path,
    extent: Extent,
    timeout: Duration,
) -> Result<(MediaInfo, VideoFrame), MediaError> {
    let decoder = Decoder::start(
        tools,
        input,
        DecodeOptions {
            fit: Fit::Exact(extent),
            queue_frames: 1,
            max_frames: Some(1),
            ..Default::default()
        },
    );
    let deadline = Instant::now() + timeout;
    let mut info = None;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match decoder.next_timeout(left) {
            Some(DecodeEvent::Opened(opened)) => info = Some(opened.info),
            Some(DecodeEvent::Frame(frame)) => {
                return Ok((info.expect("Opened comes first"), frame));
            }
            Some(DecodeEvent::End { .. }) => return Err(MediaError::NoFrames),
            Some(DecodeEvent::Failed(error)) => return Err(error),
            None => return Err(MediaError::Stalled(timeout)),
        }
    }
}

/// Output size for `video` under `fit`: (picture size, padded frame size).
pub fn output_extent(video: &VideoInfo, fit: Fit) -> Result<(Extent, Extent), MediaError> {
    let (width, height) = (
        f64::from(video.display_width),
        f64::from(video.display_height),
    );
    let scaled = |factor: f64, max: Extent| Extent {
        width: ((width * factor).round() as u32).clamp(1, max.width),
        height: ((height * factor).round() as u32).clamp(1, max.height),
    };
    let ceiling = Extent {
        width: MAX_DIMENSION,
        height: MAX_DIMENSION,
    };
    let within = |max: Extent| {
        let factor = (f64::from(max.width) / width)
            .min(f64::from(max.height) / height)
            .min(1.);
        scaled(factor, max)
    };
    let too_large = |extent: Extent| MediaError::TooLarge {
        width: extent.width,
        height: extent.height,
    };
    match fit {
        Fit::Source => {
            let picture = within(ceiling);
            Ok((picture, picture))
        }
        Fit::Within(max) | Fit::Exact(max)
            if max.width == 0
                || max.height == 0
                || max.width > MAX_DIMENSION
                || max.height > MAX_DIMENSION =>
        {
            Err(too_large(max))
        }
        Fit::Within(max) => {
            let picture = within(max);
            Ok((picture, picture))
        }
        Fit::Exact(frame) => {
            let factor = (f64::from(frame.width) / width).min(f64::from(frame.height) / height);
            Ok((scaled(factor, frame), frame))
        }
    }
}

/// The `-vf` chain: scale and pad to the output size, RGBA, then `showinfo`
/// last so it reports exactly the frames written to stdout.
fn filter_chain(picture: Extent, frame: Extent) -> String {
    let mut chain = format!(
        "scale={}:{}:flags=bicubic,setsar=1",
        picture.width, picture.height
    );
    if picture != frame {
        chain += &format!(
            ",pad={}:{}:{}:{}:color=black",
            frame.width,
            frame.height,
            (frame.width - picture.width) / 2,
            (frame.height - picture.height) / 2
        );
    }
    chain + ",format=rgba,showinfo=checksum=0"
}

/// The full `ffmpeg` command line for one decode.
pub fn decode_args(
    input: &Path,
    stream: u32,
    picture: Extent,
    frame: Extent,
) -> Result<Vec<OsString>, MediaError> {
    let mut args: Vec<OsString> = ["-loglevel", "info", "-nostats"]
        .into_iter()
        .map(OsString::from)
        .collect();
    args.extend(ffmpeg::input_args(input).map_err(MediaError::Input)?);
    let map = format!("0:{stream}");
    let filter = filter_chain(picture, frame);
    args.extend(
        [
            "-map",
            map.as_str(),
            "-an",
            "-sn",
            "-dn",
            "-vf",
            filter.as_str(),
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ]
        .into_iter()
        .map(OsString::from),
    );
    Ok(args)
}

/// One `showinfo` frame line.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrameMeta {
    pub n: u64,
    /// Seconds; `None` for `NOPTS`.
    pub pts_time: Option<f64>,
    pub duration_time: Option<f64>,
    pub size: Option<(u32, u32)>,
}

/// Parses `[Parsed_showinfo_N @ 0x...] n:   0 pts:      0 pts_time:0
/// duration: 33 duration_time:0.033 fmt:rgba ... s:317x239 ...`. Other
/// showinfo lines (configuration, side data) give `None`.
pub(crate) fn parse_showinfo(line: &str) -> Option<FrameMeta> {
    if !line.contains("Parsed_showinfo") {
        return None;
    }
    let body = line.split_once("] ")?.1;
    let mut tokens = body.split_whitespace();
    let (mut n, mut pts_time, mut duration_time, mut size) = (None, None, None, None);
    while let Some(token) = tokens.next() {
        let Some((key, value)) = token.split_once(':') else {
            continue;
        };
        let value = if value.is_empty() {
            tokens.next().unwrap_or_default()
        } else {
            value
        };
        let seconds = || value.parse::<f64>().ok().filter(|v| v.is_finite());
        match key {
            "n" => n = value.parse().ok(),
            "pts_time" => pts_time = seconds(),
            "duration_time" => duration_time = seconds(),
            "s" => {
                size = value
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
            }
            _ => {}
        }
    }
    Some(FrameMeta {
        n: n?,
        pts_time,
        duration_time,
        size,
    })
}

/// Turns source timestamps into strictly increasing times from the first
/// frame, repairing missing or non-increasing ones.
#[derive(Debug, Default)]
pub(crate) struct Clock {
    first: Option<f64>,
    previous: Option<f64>,
    nominal: f64,
}

impl Clock {
    pub(crate) fn new(frame_rate: Option<f64>) -> Self {
        Self {
            first: None,
            previous: None,
            nominal: frame_rate
                .filter(|rate| *rate > 0. && rate.is_finite())
                .map_or(1. / 30., |rate| 1. / rate),
        }
    }

    /// (time from the first frame, repaired).
    pub(crate) fn place(&mut self, meta: &FrameMeta) -> (Duration, bool) {
        let step = meta
            .duration_time
            .filter(|d| *d > 0.)
            .unwrap_or(self.nominal);
        let (seconds, repaired) = match (meta.pts_time, self.previous) {
            (Some(t), Some(previous)) if t > previous => (t, false),
            (Some(t), None) => (t, false),
            (_, Some(previous)) => (previous + step, true),
            (None, None) => (0., true),
        };
        self.previous = Some(seconds);
        let first = *self.first.get_or_insert(seconds);
        (Duration::from_secs_f64((seconds - first).max(0.)), repaired)
    }
}

struct Job {
    tools: Tools,
    input: PathBuf,
    options: DecodeOptions,
    shared: Arc<Shared>,
    events: SyncSender<DecodeEvent>,
}

/// How the frame loop stopped.
enum Stop {
    /// stdout ended (`partial` bytes of an incomplete frame were read).
    Eof {
        partial: usize,
    },
    /// `max_frames` frames were delivered.
    Limit,
    /// The consumer is gone.
    Gone,
    Failed(MediaError),
}

impl Job {
    fn run(self) {
        let result = self.decode();
        self.shared.set_phase(Phase::Finished);
        if let Some(mut child) = self.shared.child.lock().ok().and_then(|mut c| c.take()) {
            child.kill_and_wait();
        }
        if self.shared.cancelled.load(Ordering::SeqCst) {
            return;
        }
        let event = match result {
            Ok(Some(event)) => event,
            Ok(None) => return,
            Err(error) => DecodeEvent::Failed(error),
        };
        let _ = self.events.send(event);
    }

    /// Returns the terminal event, or `None` when there is nobody to tell.
    fn decode(&self) -> Result<Option<DecodeEvent>, MediaError> {
        let cancelled = || self.shared.cancelled.load(Ordering::SeqCst);
        ffmpeg::input_args(&self.input).map_err(MediaError::Input)?;
        let info = probe::probe_cancellable(
            &self.tools.ffprobe,
            &self.input,
            self.options.probe_timeout,
            &self.shared.cancelled,
        )?;
        if cancelled() {
            return Ok(None);
        }
        let video = info.video.clone().ok_or(MediaError::NoVideo)?;
        let (picture, frame) = output_extent(&video, self.options.fit)?;
        let frame_bytes = frame.width as usize * frame.height as usize * 4;
        let queue_frames = self
            .options
            .queue_frames
            .clamp(1, MAX_QUEUE_FRAMES)
            .min(self.options.frame_budget / frame_bytes);
        if queue_frames == 0 {
            return Err(MediaError::TooLarge {
                width: frame.width,
                height: frame.height,
            });
        }
        let args = decode_args(&self.input, video.index, picture, frame)?;
        let frame_rate = video.frame_rate.map(|rate| rate.as_f64());
        if self
            .events
            .send(DecodeEvent::Opened(Opened {
                info,
                extent: frame,
                queue_frames,
            }))
            .is_err()
        {
            return Ok(None);
        }
        let mut command = Command::new(&self.tools.ffmpeg);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child =
            Supervised::spawn(command).map_err(|error| MediaError::Spawn(error.to_string()))?;
        let stdout = child.take_stdout().expect("stdout is piped");
        let stderr = child.take_stderr().expect("stderr is piped");
        self.shared.pid.store(child.id(), Ordering::SeqCst);
        {
            let mut slot = self.shared.child.lock().expect("decoder child lock");
            *slot = Some(child);
            if cancelled() {
                slot.as_mut().expect("just stored").kill();
            }
        }
        self.shared.touch();
        self.shared.set_phase(Phase::Producing);
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let (meta_tx, meta_rx) = mpsc::sync_channel(META_QUEUE);
        let stderr_thread = {
            let (shared, tail) = (self.shared.clone(), tail.clone());
            std::thread::Builder::new()
                .name("sela-decoder-stderr".into())
                .spawn(move || read_stderr(stderr, &shared, &tail, &meta_tx))
                .map_err(|error| MediaError::Spawn(error.to_string()))?
        };
        let watchdog = {
            let shared = self.shared.clone();
            let timeout = self.options.stall_timeout;
            std::thread::Builder::new()
                .name("sela-decoder-watchdog".into())
                .spawn(move || watch(&shared, timeout))
                .map_err(|error| MediaError::Spawn(error.to_string()))?
        };
        let (stop, delivered) = self.frames(stdout, &meta_rx, frame, frame_rate);
        let status = match stop {
            Stop::Eof { .. } if !self.shared.stalled.load(Ordering::SeqCst) => self.wait_exit(),
            _ => None,
        };
        self.shared.set_phase(Phase::Finished);
        let stalled = self.shared.stalled.load(Ordering::SeqCst);
        self.shared.kill();
        let _ = stderr_thread.join();
        let _ = watchdog.join();
        if cancelled() {
            return Ok(None);
        }
        if stalled {
            return Err(MediaError::Stalled(self.options.stall_timeout));
        }
        let stderr_text = tail
            .lock()
            .map(|lines| lines.iter().cloned().collect::<Vec<String>>().join("\n"))
            .unwrap_or_default();
        match stop {
            Stop::Gone => Ok(None),
            Stop::Failed(error) => Err(error),
            Stop::Limit => Ok(Some(DecodeEvent::End {
                frames: delivered,
                limited: true,
            })),
            Stop::Eof { partial } => match status {
                Some(status) if status.success() && partial == 0 => {
                    if delivered == 0 {
                        Err(classify(&stderr_text, || MediaError::NoFrames))
                    } else {
                        Ok(Some(DecodeEvent::End {
                            frames: delivered,
                            limited: false,
                        }))
                    }
                }
                Some(status) if status.success() => Err(MediaError::Corrupt(format!(
                    "the decoder ended inside a frame ({partial} of {} bytes)",
                    frame_bytes
                ))),
                Some(status) => Err(classify(&stderr_text, || {
                    MediaError::Crashed(format!("ffmpeg {status}"))
                })),
                None => Err(MediaError::Crashed(
                    "ffmpeg did not exit after its output ended".into(),
                )),
            },
        }
    }

    /// Reads frames until stdout ends, the limit is reached, the consumer
    /// leaves or a frame is inconsistent. Returns how it stopped and how
    /// many frames were delivered.
    fn frames(
        &self,
        mut stdout: impl Read,
        meta: &Receiver<FrameMeta>,
        extent: Extent,
        frame_rate: Option<f64>,
    ) -> (Stop, u64) {
        let frame_bytes = extent.width as usize * extent.height as usize * 4;
        let mut clock = Clock::new(frame_rate);
        let mut delivered = 0u64;
        loop {
            if self
                .options
                .max_frames
                .is_some_and(|limit| delivered >= limit)
            {
                return (Stop::Limit, delivered);
            }
            self.shared.set_phase(Phase::Producing);
            // The showinfo line comes before the frame's bytes: FFmpeg logs
            // it in the filter graph, before encoding and muxing.
            let Ok(meta) = meta.recv() else {
                // stderr closed: FFmpeg exited or was killed. Anything left
                // on stdout is an incomplete frame.
                let partial = drain(&mut stdout, &self.shared);
                return (Stop::Eof { partial }, delivered);
            };
            if meta.size != Some((extent.width, extent.height)) {
                let size = meta
                    .size
                    .map_or("unknown".into(), |(w, h)| format!("{w}x{h}"));
                return (
                    Stop::Failed(MediaError::Corrupt(format!(
                        "frame {} is {size}, expected {}x{}",
                        meta.n, extent.width, extent.height
                    ))),
                    delivered,
                );
            }
            let mut rgba: Arc<[u8]> = std::iter::repeat_n(0u8, frame_bytes).collect();
            let buffer = Arc::get_mut(&mut rgba).expect("fresh buffer is unique");
            let read = read_full(&mut stdout, buffer, &self.shared);
            if read < frame_bytes {
                return (Stop::Eof { partial: read }, delivered);
            }
            let (pts, repaired) = clock.place(&meta);
            let frame = VideoFrame {
                index: delivered,
                pts,
                duration: meta
                    .duration_time
                    .filter(|d| *d > 0.)
                    .map(Duration::from_secs_f64),
                repaired,
                extent,
                rgba,
                version: ContentVersion {
                    id: self.options.content_id,
                    revision: delivered,
                },
            };
            self.shared.set_phase(Phase::Blocked);
            if self.events.send(DecodeEvent::Frame(frame)).is_err() {
                return (Stop::Gone, delivered);
            }
            self.shared.touch();
            delivered += 1;
        }
    }

    /// Waits for FFmpeg to exit after its stdout ended; kills it after
    /// [`EXIT_GRACE`].
    fn wait_exit(&self) -> Option<std::process::ExitStatus> {
        let deadline = Instant::now() + EXIT_GRACE;
        loop {
            {
                let mut slot = self.shared.child.lock().ok()?;
                let child = slot.as_mut()?;
                match child.try_wait() {
                    Ok(Some(status)) => return Some(status),
                    Ok(None) if Instant::now() < deadline => {}
                    Ok(None) | Err(_) => {
                        child.kill_and_wait();
                        return None;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

/// Fills `buffer` from `pipe`, marking progress per chunk. Returns the bytes
/// read: less than `buffer.len()` only at the end of the stream.
fn read_full(pipe: &mut impl Read, buffer: &mut [u8], shared: &Shared) -> usize {
    let mut filled = 0;
    while filled < buffer.len() {
        match pipe.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => {
                filled += read;
                shared.touch();
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    filled
}

/// Counts and discards what is left on stdout.
fn drain(pipe: &mut impl Read, shared: &Shared) -> usize {
    let mut buffer = vec![0u8; 64 * 1024];
    let mut total = 0;
    loop {
        let read = read_full(pipe, &mut buffer, shared);
        total += read;
        if read < buffer.len() {
            return total;
        }
    }
}

/// Reads FFmpeg's stderr line by line: frame lines go to `meta`, the rest
/// to a bounded tail for diagnostics. Every line counts as progress.
fn read_stderr(
    stderr: impl Read,
    shared: &Shared,
    tail: &Mutex<VecDeque<String>>,
    meta: &SyncSender<FrameMeta>,
) {
    let mut reader = BufReader::new(stderr);
    let mut line = Vec::new();
    loop {
        line.clear();
        match (&mut reader)
            .take(STDERR_LINE_LIMIT as u64)
            .read_until(b'\n', &mut line)
        {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if line.last() != Some(&b'\n') && line.len() >= STDERR_LINE_LIMIT {
            // Discard the rest of an overlong line.
            let mut rest = Vec::new();
            loop {
                rest.clear();
                match (&mut reader)
                    .take(STDERR_LINE_LIMIT as u64)
                    .read_until(b'\n', &mut rest)
                {
                    Ok(0) | Err(_) => break,
                    Ok(_) if rest.last() == Some(&b'\n') => break,
                    Ok(_) => {}
                }
            }
        }
        shared.touch();
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end();
        if let Some(frame) = parse_showinfo(text) {
            if meta.send(frame).is_err() {
                return;
            }
        } else if !text.contains("Parsed_showinfo")
            && !text.is_empty()
            && let Ok(mut tail) = tail.lock()
        {
            if tail.len() == STDERR_TAIL_LINES {
                tail.pop_front();
            }
            tail.push_back(text.to_owned());
        }
    }
}

/// Kills FFmpeg when it produced nothing for `timeout` while the reader
/// was waiting on it. Ends when the decode finishes.
fn watch(shared: &Shared, timeout: Duration) {
    loop {
        std::thread::sleep(WATCHDOG_TICK);
        let phase = shared.phase.load(Ordering::SeqCst);
        if phase == Phase::Finished as u8 || shared.cancelled.load(Ordering::SeqCst) {
            return;
        }
        if phase != Phase::Producing as u8 {
            continue;
        }
        let idle = shared.start.elapsed().saturating_sub(Duration::from_millis(
            shared.progress.load(Ordering::SeqCst),
        ));
        if idle > timeout {
            shared.stalled.store(true, Ordering::SeqCst);
            shared.kill();
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = "[Parsed_showinfo_2 @ 0x7f4ed800f540] n:  10 pts:    833 \
        pts_time:0.833   duration:     33 duration_time:0.033   fmt:rgba \
        cl:unspecified sar:1/1 s:317x239 i:P iskey:1 type:I ";

    fn video(width: u32, height: u32) -> VideoInfo {
        VideoInfo {
            index: 0,
            codec: "ffv1".into(),
            width,
            height,
            display_width: width,
            display_height: height,
            frame_rate: None,
            variable_rate: false,
            rotation: 0,
        }
    }

    fn extent(width: u32, height: u32) -> Extent {
        Extent { width, height }
    }

    #[test]
    fn showinfo_frame_lines_give_number_time_and_size() {
        assert_eq!(
            parse_showinfo(LINE),
            Some(FrameMeta {
                n: 10,
                pts_time: Some(0.833),
                duration_time: Some(0.033),
                size: Some((317, 239)),
            })
        );
        let nopts = "[Parsed_showinfo_3 @ 0x1] n:   0 pts:NOPTS pts_time:NOPTS \
                     duration:N/A s:2x2 fmt:rgba";
        assert_eq!(
            parse_showinfo(nopts),
            Some(FrameMeta {
                n: 0,
                pts_time: None,
                duration_time: None,
                size: Some((2, 2)),
            })
        );
    }

    #[test]
    fn other_lines_are_not_frames() {
        for line in [
            "[Parsed_showinfo_2 @ 0x7f] config in time_base: 1/1000, frame_rate: 30/1",
            "[Parsed_showinfo_2 @ 0x7f]   side data - display matrix: rotation of -90.00 degrees",
            "Stream mapping:",
            "[mkv @ 0x1] n: 3 pts_time:1 s:2x2",
            "",
        ] {
            assert_eq!(parse_showinfo(line), None, "{line}");
        }
    }

    #[test]
    fn the_clock_starts_at_zero_and_keeps_source_gaps() {
        let mut clock = Clock::new(Some(30.));
        let meta = |pts: f64| FrameMeta {
            n: 0,
            pts_time: Some(pts),
            duration_time: None,
            size: None,
        };
        assert_eq!(clock.place(&meta(10.0)), (Duration::ZERO, false));
        assert_eq!(
            clock.place(&meta(10.5)),
            (Duration::from_millis(500), false)
        );
    }

    #[test]
    fn missing_and_backward_timestamps_are_repaired_monotonically() {
        let mut clock = Clock::new(Some(25.));
        let meta = |pts: Option<f64>, duration: Option<f64>| FrameMeta {
            n: 0,
            pts_time: pts,
            duration_time: duration,
            size: None,
        };
        assert_eq!(clock.place(&meta(None, None)), (Duration::ZERO, true));
        let (second, repaired) = clock.place(&meta(None, None));
        assert!(repaired);
        assert!((second.as_secs_f64() - 0.04).abs() < 1e-9);
        // Backwards: one step past the previous, using the frame's duration.
        let (third, repaired) = clock.place(&meta(Some(0.01), Some(0.1)));
        assert!(repaired);
        assert!((third.as_secs_f64() - 0.14).abs() < 1e-9);
        // A duplicate is repaired too.
        let (fourth, repaired) = clock.place(&meta(Some(0.14), None));
        assert!(repaired && fourth > third);
        // A later timestamp is taken as is.
        assert_eq!(
            clock.place(&meta(Some(1.0), None)),
            (Duration::from_secs(1), false)
        );
    }

    #[test]
    fn source_fit_keeps_the_display_size_under_the_ceiling() {
        assert_eq!(
            output_extent(&video(317, 239), Fit::Source),
            Ok((extent(317, 239), extent(317, 239)))
        );
        let huge = output_extent(&video(16384, 4096), Fit::Source).unwrap();
        assert_eq!(huge.0, extent(8192, 2048));
    }

    #[test]
    fn within_fit_shrinks_but_never_enlarges() {
        assert_eq!(
            output_extent(&video(3840, 2160), Fit::Within(extent(1920, 1080))),
            Ok((extent(1920, 1080), extent(1920, 1080)))
        );
        assert_eq!(
            output_extent(&video(640, 480), Fit::Within(extent(1920, 1080))),
            Ok((extent(640, 480), extent(640, 480)))
        );
        // Extreme aspect: the short side never rounds to zero.
        assert_eq!(
            output_extent(&video(4000, 1), Fit::Within(extent(100, 100))),
            Ok((extent(100, 1), extent(100, 1)))
        );
    }

    #[test]
    fn exact_fit_scales_to_fit_and_pads_to_the_frame() {
        let (picture, frame) =
            output_extent(&video(320, 240), Fit::Exact(extent(640, 360))).unwrap();
        assert_eq!((picture, frame), (extent(480, 360), extent(640, 360)));
        assert_eq!(
            filter_chain(picture, frame),
            "scale=480:360:flags=bicubic,setsar=1,pad=640:360:80:0:color=black,\
             format=rgba,showinfo=checksum=0"
        );
        let (picture, frame) =
            output_extent(&video(1920, 1080), Fit::Exact(extent(1280, 720))).unwrap();
        assert_eq!(picture, frame);
        assert_eq!(
            filter_chain(picture, frame),
            "scale=1280:720:flags=bicubic,setsar=1,format=rgba,showinfo=checksum=0"
        );
    }

    #[test]
    fn empty_or_oversized_targets_are_refused() {
        for target in [extent(0, 10), extent(10, 0), extent(MAX_DIMENSION + 1, 10)] {
            assert_eq!(
                output_extent(&video(10, 10), Fit::Exact(target)),
                Err(MediaError::TooLarge {
                    width: target.width,
                    height: target.height
                })
            );
            assert!(output_extent(&video(10, 10), Fit::Within(target)).is_err());
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn decode_args_keep_the_local_only_input_and_raw_rgba_output() {
        let args = decode_args(
            Path::new("/media/clip.mkv"),
            1,
            extent(64, 36),
            extent(64, 36),
        )
        .unwrap();
        let args: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let joined = args.join(" ");
        assert!(
            joined.starts_with(
                "-loglevel info -nostats -nostdin -hide_banner -protocol_whitelist file \
                 -i /media/clip.mkv -map 0:1 -an -sn -dn -vf "
            ),
            "{joined}"
        );
        assert!(joined.ends_with("-fps_mode passthrough -f rawvideo -pix_fmt rgba pipe:1"));
        assert_eq!(
            decode_args(Path::new("http://x/y.mp4"), 0, extent(1, 1), extent(1, 1)),
            Err(MediaError::Input(ffmpeg::InputRefused::Url))
        );
    }

    #[test]
    fn frames_share_their_pixels_as_backgrounds_and_hide_them_in_debug() {
        let frame = VideoFrame {
            index: 3,
            pts: Duration::from_millis(100),
            duration: None,
            repaired: false,
            extent: extent(1, 1),
            rgba: Arc::from(vec![1u8, 2, 3, 4]),
            version: ContentVersion { id: 9, revision: 3 },
        };
        assert_eq!(frame.pixel(0, 0), [1, 2, 3, 4]);
        let PreparedBackground::Image { version, rgba, .. } = frame.background() else {
            panic!("image");
        };
        assert_eq!(version, ContentVersion { id: 9, revision: 3 });
        assert!(Arc::ptr_eq(&rgba, &frame.rgba));
        let debug = format!("{frame:?}");
        assert!(
            debug.contains("index: 3") && !debug.contains("rgba"),
            "{debug}"
        );
    }
}
