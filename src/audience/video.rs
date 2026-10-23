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
use super::{audio, compositor::Compositor};
use sela::{
    media::{
        audio::{AudioDecoder, DecodeState},
        decode::{DecodeEvent, DecodeOptions, Decoder, Fit, Tools, VideoFrame},
    },
    playback::{AudioState, MediaAction, MediaStatus, PlayState, Timeline},
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
    audio: AudioState,
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
        inner
            .timeline
            .status(scene, position, inner.duration, inner.audio)
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

/// A started decoder with its first frame, the clip's length and whether it
/// has an audio stream.
struct Opened {
    decoder: Decoder,
    first: VideoFrame,
    duration: Option<Duration>,
    has_audio: bool,
}

/// Starts a decoder and waits for its first frame until `deadline`.
fn open(tools: &Tools, key: &Key, deadline: Instant) -> Result<Opened, Failure> {
    let decoder = Decoder::start(tools, &key.source.path, options(key));
    let (mut duration, mut has_audio) = (None, false);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Failure::TimedOut);
        }
        match decoder.next_timeout(left) {
            Some(DecodeEvent::Opened(opened)) => {
                duration = opened.info.duration;
                has_audio = opened.info.audio.is_some();
            }
            Some(DecodeEvent::Frame(first)) => {
                return Ok(Opened {
                    decoder,
                    first,
                    duration,
                    has_audio,
                });
            }
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
    audio: fn() -> audio::Output,
) -> Result<Arc<Playback>, Failure> {
    if let Some(running) = registry.find(&key) {
        return Ok(running);
    }
    let deadline = deadline
        .checked_sub(DEADLINE_MARGIN)
        .unwrap_or_else(Instant::now);
    let tools = tools.wait(deadline).ok_or(Failure::Unavailable)?;
    let Opened {
        decoder,
        first,
        duration,
        has_audio,
    } = open(&tools, &key, deadline)?;
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
            audio: AudioState::None,
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
        audio: has_audio.then_some(audio),
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
    /// Opens the audio output when the video has an audio stream.
    audio: Option<fn() -> audio::Output>,
}

/// A playing video's audio: its output and the decode feeding it.
struct Sound {
    output: audio::Output,
    decoder: Option<AudioDecoder>,
}

impl Sound {
    fn decode(tools: &Tools, key: &Key, output: &audio::Output) -> AudioDecoder {
        AudioDecoder::start(
            tools,
            &key.source.path,
            output.format(),
            key.source.repeat,
            output.shared().ring.clone(),
        )
    }

    fn state(&self) -> AudioState {
        match self.decoder.as_ref().map(AudioDecoder::state) {
            Some(DecodeState::Failed) => AudioState::Failed,
            _ => self.output.state(),
        }
    }
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
        // The output opens here, off the preparation worker: a slow device
        // never delays the first-frame gate.
        let mut sound = self.audio.map(|open| {
            let output = open();
            let decoder = Sound::decode(&self.tools, &self.key, &output);
            Sound {
                output,
                decoder: Some(decoder),
            }
        });
        // A decoded frame not yet due.
        let mut next: Option<VideoFrame> = None;
        // A terminal event read while looking for later frames.
        let mut held: Option<DecodeEvent> = None;
        loop {
            let mut inner = self.shared.lock();
            if inner.stop {
                return;
            }
            let now = Instant::now();
            if let Some(sound) = &sound {
                // Audio follows the video's clock and state (masks never
                // mute it: best guess).
                let playing = inner.timeline.state() == PlayState::Playing;
                sound.output.follow(playing, inner.timeline.position(now));
                inner.audio = sound.state();
            }
            if let Some(action) = inner.command.take() {
                if action == MediaAction::Restart {
                    drop(inner);
                    // Reopening waits for FFmpeg: never under the lock.
                    decoder = None;
                    next = None;
                    held = None;
                    if let Some(sound) = &mut sound {
                        // The old audio stops before the ring is emptied,
                        // then the new decode starts from the beginning.
                        if let Some(old) = sound.decoder.take() {
                            old.stop(Duration::from_millis(500));
                        }
                        sound.output.flush();
                        sound.decoder = Some(Sound::decode(&self.tools, &self.key, &sound.output));
                    }
                    match open(&self.tools, &self.key, Instant::now() + RESTART_TIMEOUT) {
                        Ok(Opened {
                            decoder: fresh,
                            first,
                            ..
                        }) => {
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
                let event = held
                    .take()
                    .or_else(|| decoder.as_ref().and_then(Decoder::try_next));
                match event {
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
                    // Anything else (the end, a failure) waits in `held`.
                    while held.is_none() {
                        match decoder.as_ref().and_then(Decoder::try_next) {
                            Some(DecodeEvent::Frame(later)) => {
                                if self.shared.lock().timeline.due(later.pts, Instant::now()) {
                                    frame = later;
                                } else {
                                    next = Some(later);
                                    break;
                                }
                            }
                            Some(other) => held = Some(other),
                            None => break,
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

#[cfg(test)]
mod tests {
    use super::*;
    use sela::background::Aspect;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const EXTENT: Extent = Extent {
        width: 160,
        height: 90,
    };

    /// A real compositor and FFmpeg, or `None` with a SKIPPED line (unless
    /// `SELA_REQUIRE_FFMPEG=1` / `SELA_REQUIRE_GPU=1`).
    fn setup(test: &str) -> Option<(Arc<Compositor>, Tools)> {
        let skip = |why: String, flag: &str| {
            let message = format!("SKIPPED {test}: {why}");
            assert!(
                std::env::var_os(flag).is_none_or(|v| v != "1"),
                "{message} ({flag}=1)"
            );
            eprintln!("{message}");
            None
        };
        let tools = match sela::media::ffmpeg::locate() {
            Ok(found) => Tools::from(&found),
            Err(error) => return skip(format!("no FFmpeg ({error:?})"), "SELA_REQUIRE_FFMPEG"),
        };
        match Compositor::new(wgpu::Backends::all()) {
            Ok(compositor) => Some((Arc::new(compositor), tools)),
            Err(error) => skip(format!("no GPU adapter ({error})"), "SELA_REQUIRE_GPU"),
        }
    }

    /// An original clip: a green 20x20 box moving right at 100 px/s over navy.
    fn clip(tools: &Tools, dir: &Path, seconds: &str) -> PathBuf {
        let out = dir.join("motion.mkv");
        let graph = format!(
            "color=c=0x203060:size=160x90:rate=20:d={seconds}[bg];\
             color=c=0x00ff00:size=20x20:rate=20[box];\
             [bg][box]overlay=x='t*100':y=10:shortest=1"
        );
        let status = Command::new(&tools.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
            .args(["-filter_complex", &graph, "-c:v", "ffv1"])
            .arg(&out)
            .status()
            .unwrap();
        assert!(status.success());
        out
    }

    fn key(path: &Path, repeat: bool) -> Key {
        Key {
            source: VideoSource {
                path: path.to_owned(),
                aspect: Aspect::Stretch,
                repeat,
            },
            extent: EXTENT,
            version: ContentVersion { id: 1, revision: 1 },
        }
    }

    /// Left edge of the green box in row 20, if any.
    fn box_x(compositor: &Compositor, playback: &Playback) -> Option<u32> {
        let rgba = compositor.read_texture(playback.texture()).unwrap();
        let row = 20 * EXTENT.width as usize * 4;
        (0..EXTENT.width).find(|x| {
            let at = row + *x as usize * 4;
            rgba[at] < 60 && rgba[at + 1] > 200 && rgba[at + 2] < 60
        })
    }

    fn wait(what: &str, mut done: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(10);
        while !done() {
            assert!(Instant::now() < end, "bounded wait for {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn noop() -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }

    fn null_audio() -> audio::Output {
        audio::Output::null(sela::media::audio::AudioFormat::DEFAULT)
    }

    #[test]
    fn a_primed_video_holds_its_first_frame_until_applied_then_plays_pauses_and_restarts() {
        let Some((compositor, tools)) = setup("a_primed_video_holds_its_first_frame") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let path = clip(&tools, dir.path(), "3");
        let registry = Registry::default();
        let woken = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = woken.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });
        let playback = prime(
            &ToolsCell::ready(Some(tools)),
            &registry,
            &compositor,
            key(&path, true),
            Instant::now() + Duration::from_secs(5),
            wake,
            null_audio,
        )
        .unwrap();
        let scene = ContentVersion { id: 7, revision: 1 };
        assert_eq!(playback.status(scene).state, PlayState::Primed);
        assert_eq!(box_x(&compositor, &playback), Some(0), "the first frame");
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(box_x(&compositor, &playback), Some(0), "primed: no clock");
        assert_eq!(woken.load(std::sync::atomic::Ordering::Relaxed), 0);
        // Applied: frames are presented at their timestamps.
        playback.start();
        wait("the box to move", || {
            box_x(&compositor, &playback).is_some_and(|x| x >= 30)
        });
        assert!(
            woken.load(std::sync::atomic::Ordering::Relaxed) > 0,
            "redraws asked"
        );
        assert_eq!(playback.status(scene).state, PlayState::Playing);
        // Pause holds the presented frame.
        playback.command(MediaAction::Pause);
        wait("paused", || playback.state() == PlayState::Paused);
        std::thread::sleep(Duration::from_millis(100));
        let held = box_x(&compositor, &playback);
        let position = playback.status(scene).position;
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(box_x(&compositor, &playback), held);
        assert_eq!(playback.status(scene).position, position);
        // Play continues from there, not from the start.
        playback.command(MediaAction::Play);
        wait("moving again", || {
            box_x(&compositor, &playback).is_some_and(|x| x > held.unwrap() + 10)
        });
        // Restart presents the first frames again.
        playback.command(MediaAction::Restart);
        wait("restarted", || {
            playback.state() == PlayState::Playing
                && box_x(&compositor, &playback).is_some_and(|x| x < 20)
        });
        // Past the 3 s clip it loops: playing, the position wraps.
        std::thread::sleep(Duration::from_millis(3300));
        let status = playback.status(scene);
        assert_eq!(status.state, PlayState::Playing);
        assert!(status.position < Duration::from_secs(1), "{status:?}");
        assert_eq!(status.duration, Some(Duration::from_secs(3)));
    }

    #[test]
    fn the_same_video_is_shared_and_stops_with_its_last_handle() {
        let Some((compositor, tools)) = setup("the_same_video_is_shared") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let path = clip(&tools, dir.path(), "2");
        let registry = Registry::default();
        let cell = ToolsCell::ready(Some(tools));
        let deadline = || Instant::now() + Duration::from_secs(5);
        let first = prime(
            &cell,
            &registry,
            &compositor,
            key(&path, true),
            deadline(),
            noop(),
            null_audio,
        )
        .unwrap();
        let again = prime(
            &cell,
            &registry,
            &compositor,
            key(&path, true),
            deadline(),
            noop(),
            null_audio,
        )
        .unwrap();
        assert!(Arc::ptr_eq(&first, &again), "a later slide keeps the video");
        let other = prime(
            &cell,
            &registry,
            &compositor,
            key(&path, false),
            deadline(),
            noop(),
            null_audio,
        )
        .unwrap();
        assert!(
            !Arc::ptr_eq(&first, &other),
            "another repeat choice is another video"
        );
        assert_eq!(registry.live(), 2);
        drop((first, again, other));
        assert_eq!(registry.live(), 0);
    }

    #[test]
    fn a_video_that_plays_once_ends_on_its_last_frame() {
        let Some((compositor, tools)) = setup("a_video_that_plays_once_ends") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let path = clip(&tools, dir.path(), "0.5");
        let playback = prime(
            &ToolsCell::ready(Some(tools)),
            &Registry::default(),
            &compositor,
            key(&path, false),
            Instant::now() + Duration::from_secs(5),
            noop(),
            null_audio,
        )
        .unwrap();
        playback.start();
        wait("the end", || playback.state() == PlayState::Ended);
        let last = box_x(&compositor, &playback);
        assert!(last.is_some_and(|x| x >= 40), "{last:?}");
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(box_x(&compositor, &playback), last, "the last frame stays");
        // Play replays it from the start.
        playback.command(MediaAction::Play);
        wait("replaying", || playback.state() == PlayState::Playing);
    }

    #[test]
    fn failures_before_the_first_frame_never_give_a_playback() {
        let Some((compositor, tools)) = setup("failures_before_the_first_frame") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        let missing = dir.path().join("gone.mkv");
        let cell = ToolsCell::ready(Some(tools.clone()));
        assert!(matches!(
            prime(
                &cell,
                &registry,
                &compositor,
                key(&missing, true),
                deadline,
                noop(),
                null_audio,
            ),
            Err(Failure::Decode(sela::media::MediaError::Missing))
        ));
        let garbage = dir.path().join("garbage.mkv");
        std::fs::write(&garbage, b"not a video").unwrap();
        assert!(matches!(
            prime(
                &cell,
                &registry,
                &compositor,
                key(&garbage, true),
                deadline,
                noop(),
                null_audio,
            ),
            Err(Failure::Decode(_))
        ));
        let path = clip(&tools, dir.path(), "1");
        // A deadline already inside the margin: no time to wait.
        assert!(matches!(
            prime(
                &cell,
                &registry,
                &compositor,
                key(&path, true),
                Instant::now(),
                noop(),
                null_audio,
            ),
            Err(Failure::TimedOut | Failure::Unavailable)
        ));
        assert!(matches!(
            prime(
                &ToolsCell::none(),
                &registry,
                &compositor,
                key(&path, true),
                deadline,
                noop(),
                null_audio,
            ),
            Err(Failure::Unavailable)
        ));
        assert_eq!(registry.live(), 0);
    }

    /// The audio output the last audio test opened, to watch it consume.
    static OPENED: Mutex<Option<Arc<audio::Shared>>> = Mutex::new(None);

    fn watched_audio() -> audio::Output {
        let output = null_audio();
        *OPENED.lock().unwrap() = Some(output.shared().clone());
        output
    }

    /// `clip` with a 440 Hz tone, PCM in Matroska.
    fn clip_with_audio(tools: &Tools, dir: &Path, seconds: &str) -> PathBuf {
        let out = dir.join("motion-tone.mkv");
        let graph = format!(
            "color=c=0x203060:size=160x90:rate=20:d={seconds}[bg];\
             color=c=0x00ff00:size=20x20:rate=20[box];\
             [bg][box]overlay=x='t*100':y=10:shortest=1[v];\
             sine=frequency=440:sample_rate=48000:duration={seconds}[a]"
        );
        let status = Command::new(&tools.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
            .args(["-filter_complex", &graph, "-map", "[v]", "-map", "[a]"])
            .args(["-c:v", "ffv1", "-c:a", "pcm_s16le"])
            .arg(&out)
            .status()
            .unwrap();
        assert!(status.success());
        out
    }

    #[test]
    fn a_videos_audio_follows_its_clock_and_a_silent_one_has_none() {
        let Some((compositor, tools)) = setup("a_videos_audio_follows") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let scene = ContentVersion { id: 3, revision: 1 };
        let cell = ToolsCell::ready(Some(tools.clone()));
        let deadline = || Instant::now() + Duration::from_secs(5);
        // A silent clip never opens an output.
        let silent = clip(&tools, dir.path(), "1");
        let quiet = prime(
            &cell,
            &Registry::default(),
            &compositor,
            key(&silent, true),
            deadline(),
            noop(),
            watched_audio,
        )
        .unwrap();
        quiet.start();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(quiet.status(scene).audio, AudioState::None);
        assert!(OPENED.lock().unwrap().is_none(), "no output for no audio");
        drop(quiet);
        // With a tone: primed, the output (no device here) reads nothing.
        let path = clip_with_audio(&tools, dir.path(), "3");
        let playback = prime(
            &cell,
            &Registry::default(),
            &compositor,
            key(&path, true),
            deadline(),
            noop(),
            watched_audio,
        )
        .unwrap();
        wait("the output", || OPENED.lock().unwrap().is_some());
        let shared = OPENED.lock().unwrap().clone().unwrap();
        wait("the audio report", || {
            playback.status(scene).audio == AudioState::NoDevice
        });
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(shared.played(), 0, "primed: silent");
        // Applied: it consumes in step with the video (48 kHz frames).
        playback.start();
        std::thread::sleep(Duration::from_millis(1000));
        let played = shared.played();
        let position = playback.status(scene).position.as_secs_f64();
        let video = (position * 48_000.) as u64;
        assert!(
            played.abs_diff(video) < 48_000 / 5,
            "audio {played} frames, video {position:.3} s"
        );
        // Paused: the audio stops too.
        playback.command(MediaAction::Pause);
        wait("paused", || playback.state() == PlayState::Paused);
        std::thread::sleep(Duration::from_millis(100));
        let held = shared.played();
        std::thread::sleep(Duration::from_millis(300));
        assert!(shared.played() <= held + 48_000 / 50, "paused audio ran on");
        // Restart starts the audio over from the beginning.
        playback.command(MediaAction::Restart);
        wait("restarted", || {
            playback.state() == PlayState::Playing && shared.played() < 48_000 / 2
        });
        assert_eq!(playback.status(scene).audio, AudioState::NoDevice);
        drop(playback);
    }
}
