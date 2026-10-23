//! Background video playback inside the renderer process (M2-02/M2-03).
//!
//! [`prime`] runs on the preparation worker: it starts a supervised FFmpeg
//! decoder for the cue's video, waits (bounded by the cue's deadline) for
//! the first frame and uploads it into a texture the composition samples.
//! Only then can the cue be accepted, so a video that does not open, is
//! damaged or is too slow never replaces the live scene: the worker rejects
//! the cue instead. A primed [`Playback`] stands on its first frame until
//! its scene is applied ([`Playback::start`]); its own thread then presents
//! decoded frames at their timestamps by writing them into the same texture
//! and waking the event loop, so the redraw only encodes a pass over
//! existing bindings (no decode, upload or file work on the frame path).
//!
//! Masks never touch the playback: under Black or Logo the video runs on
//! (UNOBSERVED best guess for EW8-OBS-006), under Clear it shows without
//! text. Dropping the last handle stops the thread and kills FFmpeg without
//! waiting.
use super::compositor::Compositor;
use sela::{
    media::decode::{DecodeEvent, DecodeOptions, Decoder, Fit, Tools, VideoFrame},
    playback::{MediaAction, MediaStatus, PlayState, Timeline},
    scene::{ContentVersion, Extent, VideoSource},
};
use std::{
    sync::{Arc, Condvar, Mutex, OnceLock, Weak},
    time::{Duration, Instant},
};

/// Decoded frames a playback may hold ahead of the presented one.
const QUEUE_FRAMES: usize = 4;
/// A restart's first frame must arrive within this.
const RESTART_TIMEOUT: Duration = Duration::from_secs(5);
/// Margin kept before the cue's deadline, so the rejection receipt itself
/// still arrives in time.
const DEADLINE_MARGIN: Duration = Duration::from_millis(150);
/// Longest wait between clock checks while playing.
const TICK: Duration = Duration::from_millis(10);
/// Idle wait while paused, primed or ended (commands wake it at once).
const IDLE: Duration = Duration::from_millis(200);

/// Why a video cue could not be prepared; the scene is never replaced.
#[derive(Debug)]
pub enum Failure {
    /// No usable FFmpeg in this process (missing, GPL or nonfree build).
    Unavailable,
    /// The decoder failed before the first frame (missing, damaged,
    /// unsupported, no video).
    Decode(sela::media::MediaError),
    /// The first frame did not arrive before the cue's deadline.
    TimedOut,
    /// The first frame could not be uploaded.
    Upload,
}

impl Failure {
    /// A diagnostic line for the renderer's stderr (never file contents).
    pub fn describe(&self) -> String {
        match self {
            Self::Unavailable => "no usable FFmpeg".into(),
            Self::Decode(error) => error.to_string(),
            Self::TimedOut => "the first frame missed the cue deadline".into(),
            Self::Upload => "the first frame could not be uploaded".into(),
        }
    }
}

/// FFmpeg as located by this renderer process (the operator's rules and
/// environment, `ffmpeg::locate`), found once on its own thread at start.
pub struct ToolsCell(OnceLock<Option<Tools>>);

impl ToolsCell {
    pub fn locate_in_background() -> Arc<Self> {
        let cell = Arc::new(Self(OnceLock::new()));
        let fill = cell.clone();
        let spawned = std::thread::Builder::new()
            .name("audience-media-locate".into())
            .spawn(move || {
                let tools = sela::media::ffmpeg::locate()
                    .ok()
                    .map(|located| Tools::from(&located));
                let _ = fill.0.set(tools);
            });
        if spawned.is_err() {
            let _ = cell.0.set(None);
        }
        cell
    }

    /// A renderer that never plays video (the stage).
    pub fn none() -> Arc<Self> {
        Self::ready(None)
    }

    pub fn ready(tools: Option<Tools>) -> Arc<Self> {
        let cell = Arc::new(Self(OnceLock::new()));
        let _ = cell.0.set(tools);
        cell
    }

    /// The tools once located, waiting up to `deadline` for the locate.
    fn wait(&self, deadline: Instant) -> Option<Tools> {
        loop {
            if let Some(tools) = self.0.get() {
                return tools.clone();
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

/// What makes two cues play the same video: they can share one playback,
/// so moving to the next slide over the same background keeps it running.
#[derive(Clone, PartialEq, Eq)]
pub struct Key {
    pub source: VideoSource,
    pub extent: Extent,
    pub version: ContentVersion,
}

struct Inner {
    timeline: Timeline,
    command: Option<MediaAction>,
    stop: bool,
    /// Time of the frame now in the texture.
    presented: Duration,
    duration: Option<Duration>,
}

struct Shared {
    inner: Mutex<Inner>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// One playing (or primed) background video. Shared by every prepared or
/// applied composition that shows it.
pub struct Playback {
    pub key: Key,
    texture: wgpu::Texture,
    shared: Arc<Shared>,
}

impl Playback {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// Its scene was applied: start from the first frame unless running.
    pub fn start(&self) {
        let mut inner = self.shared.lock();
        inner.timeline.start(Instant::now());
        drop(inner);
        self.shared.wake.notify_all();
    }

    /// An operator command; the thread carries it out (a restart reopens
    /// the decoder there, never on the caller's thread).
    pub fn command(&self, action: MediaAction) {
        self.shared.lock().command = Some(action);
        self.shared.wake.notify_all();
    }

    /// The report for `scene`: state and the presented frame's time.
    /// A repeating video reports its place in the clip (the decoder's
    /// timestamps keep rising across loops).
    pub fn status(&self, scene: ContentVersion) -> MediaStatus {
        let inner = self.shared.lock();
        let position = match inner.duration {
            Some(duration) if self.key.source.repeat && !duration.is_zero() => {
                Duration::from_nanos((inner.presented.as_nanos() % duration.as_nanos()) as u64)
            }
            _ => inner.presented,
        };
        inner.timeline.status(scene, position, inner.duration)
    }

    pub fn state(&self) -> PlayState {
        self.shared.lock().timeline.state()
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.wake.notify_all();
    }
}

/// Playbacks alive in this process, for sharing between consecutive cues.
#[derive(Default)]
pub struct Registry(Mutex<Vec<Weak<Playback>>>);

impl Registry {
    fn find(&self, key: &Key) -> Option<Arc<Playback>> {
        let mut all = self.0.lock().unwrap_or_else(|e| e.into_inner());
        all.retain(|weak| weak.strong_count() > 0);
        all.iter()
            .filter_map(Weak::upgrade)
            .find(|playback| playback.key == *key && playback.state() != PlayState::Failed)
    }

    fn add(&self, playback: &Arc<Playback>) {
        let mut all = self.0.lock().unwrap_or_else(|e| e.into_inner());
        all.push(Arc::downgrade(playback));
    }

    #[cfg(test)]
    pub fn live(&self) -> usize {
        let mut all = self.0.lock().unwrap_or_else(|e| e.into_inner());
        all.retain(|weak| weak.strong_count() > 0);
        all.len()
    }
}

fn options(key: &Key) -> DecodeOptions {
    let frame_bytes = key.extent.width as usize * key.extent.height as usize * 4;
    DecodeOptions {
        fit: Fit::for_aspect(key.source.aspect, key.extent),
        queue_frames: QUEUE_FRAMES,
        frame_budget: frame_bytes * QUEUE_FRAMES,
        looping: key.source.repeat,
        ..Default::default()
    }
}

/// Starts a decoder and waits for its first frame until `deadline`.
fn open(
    tools: &Tools,
    key: &Key,
    deadline: Instant,
) -> Result<(Decoder, VideoFrame, Option<Duration>), Failure> {
    let decoder = Decoder::start(tools, &key.source.path, options(key));
    let mut duration = None;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Failure::TimedOut);
        }
        match decoder.next_timeout(left) {
            Some(DecodeEvent::Opened(opened)) => duration = opened.info.duration,
            Some(DecodeEvent::Frame(frame)) => return Ok((decoder, frame, duration)),
            Some(DecodeEvent::End { .. }) => {
                return Err(Failure::Decode(sela::media::MediaError::NoFrames));
            }
            Some(DecodeEvent::Failed(error)) => return Err(Failure::Decode(error)),
            None => return Err(Failure::TimedOut),
        }
    }
}

/// The playback for a video cue: a running one with the same key, else a
/// new decoder primed on its first frame. Blocks until the first frame is
/// uploaded or `deadline` (less a margin) passes: preparation worker only.
pub fn prime(
    tools: &ToolsCell,
    registry: &Registry,
    compositor: &Arc<Compositor>,
    key: Key,
    deadline: Instant,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> Result<Arc<Playback>, Failure> {
    if let Some(running) = registry.find(&key) {
        return Ok(running);
    }
    let deadline = deadline
        .checked_sub(DEADLINE_MARGIN)
        .unwrap_or_else(Instant::now);
    let tools = tools.wait(deadline).ok_or(Failure::Unavailable)?;
    let (decoder, first, duration) = open(&tools, &key, deadline)?;
    let texture = compositor.video_texture(key.extent);
    compositor
        .upload_frame(&texture, &first.rgba)
        .map_err(|_| Failure::Upload)?;
    let shared = Arc::new(Shared {
        inner: Mutex::new(Inner {
            timeline: Timeline::new(),
            command: None,
            stop: false,
            presented: first.pts,
            duration,
        }),
        wake: Condvar::new(),
    });
    let playback = Arc::new(Playback {
        key: key.clone(),
        texture: texture.clone(),
        shared: shared.clone(),
    });
    let player = Player {
        tools,
        key,
        texture,
        compositor: compositor.clone(),
        shared,
        wake,
    };
    std::thread::Builder::new()
        .name("audience-video".into())
        .spawn(move || player.run(decoder))
        .map_err(|_| Failure::Upload)?;
    registry.add(&playback);
    Ok(playback)
}

struct Player {
    tools: Tools,
    key: Key,
    texture: wgpu::Texture,
    compositor: Arc<Compositor>,
    shared: Arc<Shared>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Player {
    fn present(&self, frame: &VideoFrame) -> bool {
        if self
            .compositor
            .upload_frame(&self.texture, &frame.rgba)
            .is_err()
        {
            return false;
        }
        self.shared.lock().presented = frame.pts;
        (self.wake)();
        true
    }

    fn run(self, decoder: Decoder) {
        let mut decoder = Some(decoder);
        // A decoded frame not yet due.
        let mut next: Option<VideoFrame> = None;
        loop {
            let mut inner = self.shared.lock();
            if inner.stop {
                return;
            }
            let now = Instant::now();
            if let Some(action) = inner.command.take() {
                if action == MediaAction::Restart {
                    drop(inner);
                    // Reopening waits for FFmpeg: never under the lock.
                    decoder = None;
                    next = None;
                    match open(&self.tools, &self.key, Instant::now() + RESTART_TIMEOUT) {
                        Ok((fresh, first, _)) => {
                            decoder = Some(fresh);
                            let shown = self.present(&first);
                            let mut inner = self.shared.lock();
                            inner.timeline.apply(MediaAction::Restart, Instant::now());
                            if !shown {
                                inner.timeline.fail(Instant::now());
                            }
                        }
                        Err(_) => self.shared.lock().timeline.fail(Instant::now()),
                    }
                    (self.wake)();
                    continue;
                }
                let replay = inner.timeline.state() == PlayState::Ended;
                if inner.timeline.apply(action, now) && replay {
                    // Play after the end replays from the first frame.
                    inner.command = Some(MediaAction::Restart);
                    continue;
                }
                drop(inner);
                (self.wake)();
                continue;
            }
            if inner.timeline.state() != PlayState::Playing {
                let _ = self.shared.wake.wait_timeout(inner, IDLE);
                continue;
            }
            drop(inner);
            if next.is_none() {
                match decoder.as_ref().and_then(Decoder::try_next) {
                    Some(DecodeEvent::Frame(frame)) => next = Some(frame),
                    Some(DecodeEvent::Opened(_)) => continue,
                    Some(DecodeEvent::End { .. }) => {
                        decoder = None;
                        let mut inner = self.shared.lock();
                        let last = inner.presented;
                        inner.timeline.end(last);
                        drop(inner);
                        (self.wake)();
                        continue;
                    }
                    Some(DecodeEvent::Failed(_)) => {
                        decoder = None;
                        self.shared.lock().timeline.fail(Instant::now());
                        (self.wake)();
                        continue;
                    }
                    None if decoder.is_none() => {
                        // Ended or failed already; wait for a command.
                        let inner = self.shared.lock();
                        let _ = self.shared.wake.wait_timeout(inner, IDLE);
                        continue;
                    }
                    None => {}
                }
            }
            let inner = self.shared.lock();
            let now = Instant::now();
            match &next {
                Some(frame) if inner.timeline.due(frame.pts, now) => {
                    drop(inner);
                    let mut frame = next.take().expect("matched");
                    // Late: skip to the newest frame that is already due.
                    while let Some(DecodeEvent::Frame(later)) =
                        decoder.as_ref().and_then(Decoder::try_next)
                    {
                        if self.shared.lock().timeline.due(later.pts, Instant::now()) {
                            frame = later;
                        } else {
                            next = Some(later);
                            break;
                        }
                    }
                    if !self.present(&frame) {
                        self.shared.lock().timeline.fail(Instant::now());
                    }
                }
                Some(frame) => {
                    let wait = inner
                        .timeline
                        .until(frame.pts, now)
                        .unwrap_or(TICK)
                        .min(TICK);
                    let _ = self.shared.wake.wait_timeout(inner, wait);
                }
                None => {
                    // The decoder is behind; check again shortly.
                    let _ = self
                        .shared
                        .wake
                        .wait_timeout(inner, Duration::from_millis(2));
                }
            }
        }
    }
}
