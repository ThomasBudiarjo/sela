//! Video playback transport shared by the operator and the renderer
//! (M2-03): the operator's commands, the renderer's reported state and the
//! renderer's media clock. No GPUI, wgpu or FFmpeg types; time is passed in,
//! so the clock is tested with a fake one.
//!
//! Behavior rules and their sources (docs/media.md "Playback"):
//! - A background video starts at its first frame when its cue is applied,
//!   never earlier: preparation only primes the first frame.
//! - Repeating on loops until something else goes live (SRC-10); off, the
//!   video plays once and holds its last frame (UNOBSERVED best guess).
//! - Pause holds the current frame; Play continues from there; Restart plays
//!   from the first frame, also from Paused or Ended (UNOBSERVED best guess).
//! - Masks never pause the clock: Black, Clear and Logo hide or show the
//!   picture while the video keeps running underneath (EW8-OBS-006 is
//!   unobserved; best guess, so lifting a mask shows the current position).
use crate::scene::ContentVersion;
use std::time::{Duration, Instant};

/// An operator transport command for the live video (Safety lane: it never
/// waits behind a slide in preparation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaAction {
    Play,
    Pause,
    Restart,
}

/// Where the renderer's applied video stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlayState {
    /// First frame shown, clock not started (prepared, not yet applied).
    Primed,
    Playing,
    Paused,
    /// Played once to the end (Repeating off); the last frame stays.
    Ended,
    /// The decoder failed after the cue was applied; the last good frame
    /// stays on screen.
    Failed,
}

/// The renderer's report about the video of its applied scene. Position is
/// the presented frame's time, not the requested one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaStatus {
    /// The applied scene the video belongs to.
    pub scene: ContentVersion,
    pub state: PlayState,
    pub position: Duration,
    pub duration: Option<Duration>,
}

/// The media clock of one playing video: position advances with wall time
/// only while playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeline {
    state: PlayState,
    /// Position when the clock last stopped or `since` was taken.
    base: Duration,
    /// When the clock started running from `base` (Playing only).
    since: Option<Instant>,
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            state: PlayState::Primed,
            base: Duration::ZERO,
            since: None,
        }
    }

    pub fn state(&self) -> PlayState {
        self.state
    }

    /// The media time to present at `now`.
    pub fn position(&self, now: Instant) -> Duration {
        match self.since {
            Some(since) => self.base + now.saturating_duration_since(since),
            None => self.base,
        }
    }

    /// The cue was applied: a primed video starts from its first frame.
    /// Idempotent for a video already running (a later slide sharing it).
    pub fn start(&mut self, now: Instant) {
        if self.state == PlayState::Primed {
            self.run(Duration::ZERO, now);
        }
    }

    fn run(&mut self, from: Duration, now: Instant) {
        self.state = PlayState::Playing;
        self.base = from;
        self.since = Some(now);
    }

    fn hold(&mut self, state: PlayState, now: Instant) {
        self.base = self.position(now);
        self.since = None;
        self.state = state;
    }

    /// Applies an operator command; false when it changes nothing (Play
    /// while playing, Pause while paused, anything after a failure but
    /// Restart). The renderer still acknowledges it as applied.
    pub fn apply(&mut self, action: MediaAction, now: Instant) -> bool {
        match (action, self.state) {
            (MediaAction::Restart, _) => {
                self.run(Duration::ZERO, now);
                true
            }
            (_, PlayState::Failed) => false,
            (MediaAction::Pause, PlayState::Playing | PlayState::Primed) => {
                self.hold(PlayState::Paused, now);
                true
            }
            (MediaAction::Play, PlayState::Paused | PlayState::Primed) => {
                let from = self.base;
                self.run(from, now);
                true
            }
            // Play after the end replays, like Restart (best guess).
            (MediaAction::Play, PlayState::Ended) => {
                self.run(Duration::ZERO, now);
                true
            }
            _ => false,
        }
    }

    /// The source ran out (Repeating off) at the last frame's time.
    pub fn end(&mut self, last: Duration) {
        self.base = last;
        self.since = None;
        self.state = PlayState::Ended;
    }

    pub fn fail(&mut self, now: Instant) {
        self.hold(PlayState::Failed, now);
    }

    /// Whether a frame stamped `pts` is due at `now`.
    pub fn due(&self, pts: Duration, now: Instant) -> bool {
        pts <= self.position(now)
    }

    /// How long until a frame stamped `pts` is due; `None` while the clock
    /// stands still (nothing becomes due by waiting).
    pub fn until(&self, pts: Duration, now: Instant) -> Option<Duration> {
        self.since?;
        Some(pts.saturating_sub(self.position(now)))
    }

    /// The renderer's report, `position` being the presented frame's time.
    pub fn status(
        &self,
        scene: ContentVersion,
        position: Duration,
        duration: Option<Duration>,
    ) -> MediaStatus {
        MediaStatus {
            scene,
            state: self.state,
            position,
            duration,
        }
    }
}

/// "m:ss" (or "h:mm:ss") of a media position, for the Live transport row.
pub fn clock_text(position: Duration) -> String {
    let seconds = position.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn a_primed_video_stands_still_until_its_cue_applies() {
        let t0 = Instant::now();
        let mut timeline = Timeline::new();
        assert_eq!(timeline.state(), PlayState::Primed);
        assert_eq!(timeline.position(t0 + ms(500)), Duration::ZERO);
        assert!(timeline.due(Duration::ZERO, t0));
        assert!(!timeline.due(ms(1), t0 + ms(900)));
        assert_eq!(timeline.until(ms(40), t0), None);
        timeline.start(t0 + ms(1000));
        assert_eq!(timeline.state(), PlayState::Playing);
        assert_eq!(timeline.position(t0 + ms(1250)), ms(250));
        assert_eq!(timeline.until(ms(300), t0 + ms(1250)), Some(ms(50)));
        // A later slide sharing the video does not restart it.
        timeline.start(t0 + ms(2000));
        assert_eq!(timeline.position(t0 + ms(2000)), ms(1000));
    }

    #[test]
    fn pause_holds_play_continues_and_restart_starts_over() {
        let t0 = Instant::now();
        let mut timeline = Timeline::new();
        timeline.start(t0);
        assert!(timeline.apply(MediaAction::Pause, t0 + ms(700)));
        assert_eq!(timeline.state(), PlayState::Paused);
        assert_eq!(timeline.position(t0 + ms(5000)), ms(700));
        assert!(!timeline.apply(MediaAction::Pause, t0 + ms(800)), "repeat");
        assert!(timeline.apply(MediaAction::Play, t0 + ms(5000)));
        assert_eq!(timeline.position(t0 + ms(5300)), ms(1000));
        assert!(!timeline.apply(MediaAction::Play, t0 + ms(5300)), "repeat");
        assert!(timeline.apply(MediaAction::Restart, t0 + ms(6000)));
        assert_eq!(timeline.position(t0 + ms(6100)), ms(100));
        // Restart from Paused plays.
        timeline.apply(MediaAction::Pause, t0 + ms(6100));
        timeline.apply(MediaAction::Restart, t0 + ms(7000));
        assert_eq!(timeline.state(), PlayState::Playing);
        assert_eq!(timeline.position(t0 + ms(7010)), ms(10));
    }

    #[test]
    fn the_end_holds_the_last_frame_and_play_or_restart_replays() {
        let t0 = Instant::now();
        let mut timeline = Timeline::new();
        timeline.start(t0);
        timeline.end(ms(2900));
        assert_eq!(timeline.state(), PlayState::Ended);
        assert_eq!(timeline.position(t0 + ms(9000)), ms(2900));
        assert!(!timeline.apply(MediaAction::Pause, t0 + ms(9000)));
        assert!(timeline.apply(MediaAction::Play, t0 + ms(9000)));
        assert_eq!(timeline.position(t0 + ms(9100)), ms(100));
    }

    #[test]
    fn a_failure_freezes_and_only_restart_leaves_it() {
        let t0 = Instant::now();
        let mut timeline = Timeline::new();
        timeline.start(t0);
        timeline.fail(t0 + ms(400));
        assert_eq!(timeline.state(), PlayState::Failed);
        assert_eq!(timeline.position(t0 + ms(4000)), ms(400));
        assert!(!timeline.apply(MediaAction::Play, t0 + ms(4000)));
        assert!(!timeline.apply(MediaAction::Pause, t0 + ms(4000)));
        assert!(timeline.apply(MediaAction::Restart, t0 + ms(4000)));
        assert_eq!(timeline.state(), PlayState::Playing);
    }

    #[test]
    fn pause_before_apply_keeps_the_first_frame_and_play_starts_it() {
        let t0 = Instant::now();
        let mut timeline = Timeline::new();
        assert!(timeline.apply(MediaAction::Pause, t0));
        timeline.start(t0 + ms(100)); // applied while paused: stays paused
        assert_eq!(timeline.state(), PlayState::Paused);
        assert!(timeline.apply(MediaAction::Play, t0 + ms(200)));
        assert_eq!(timeline.position(t0 + ms(300)), ms(100));
    }

    #[test]
    fn clock_text_uses_minutes_and_seconds_then_hours() {
        assert_eq!(clock_text(Duration::ZERO), "0:00");
        assert_eq!(clock_text(ms(65_900)), "1:05");
        assert_eq!(clock_text(Duration::from_secs(3 * 3600 + 7)), "3:00:07");
    }
}
