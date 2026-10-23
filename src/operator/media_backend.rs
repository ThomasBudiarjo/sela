//! FFmpeg availability at startup (M2-01; VAL-MEDIA-001/002/003).
//!
//! `locate` probes up to three folders with child processes, bounded by
//! seconds each, so it runs once on its own thread; `poll` picks up the
//! answer, logs it and, when FFmpeg is unusable, the status bar shows a
//! non-blocking warning. Nothing else waits for it: songs, images, Live
//! output and Black/Clear/Logo work the same either way. Sela-only
//! behavior (EasyWorship has no separate decoder to report), so no EW8-OBS
//! applies.

use super::*;
use sela::media::backend::{Availability, Unavailable, log_line};
use sela::media::ffmpeg::{Located, NotFound};
use std::sync::mpsc;

/// Finds FFmpeg; `ffmpeg::locate` in the app, a fake in tests. `None` in
/// `SettingsConfig` skips the check (state stays `Checking`, no warning).
pub(crate) type MediaLocator = Arc<dyn Fn() -> Result<Located, NotFound> + Send + Sync>;

pub(crate) struct MediaBackend {
    pub(crate) availability: Availability,
    pending: Option<mpsc::Receiver<Result<Located, NotFound>>>,
    /// The thread `locate` ran on, for the off-UI-thread check.
    #[cfg(test)]
    pub(crate) located_on: Arc<std::sync::Mutex<Option<std::thread::ThreadId>>>,
}

impl MediaBackend {
    pub(crate) fn start(locator: Option<MediaLocator>) -> Self {
        #[cfg(test)]
        let located_on = Arc::new(std::sync::Mutex::new(None));
        let pending = locator.and_then(|locate| {
            let (sender, receiver) = mpsc::channel();
            #[cfg(test)]
            let record = located_on.clone();
            std::thread::Builder::new()
                .name("sela-media-locate".into())
                .spawn(move || {
                    #[cfg(test)]
                    {
                        *record.lock().unwrap() = Some(std::thread::current().id());
                    }
                    let _ = sender.send(locate());
                })
                .ok()
                .map(|_| receiver)
        });
        Self {
            availability: Availability::Checking,
            pending,
            #[cfg(test)]
            located_on,
        }
    }

    /// Takes the locate result once it arrived; true when the state changed.
    fn poll(&mut self) -> bool {
        let Some(receiver) = &self.pending else {
            return false;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => Err(NotFound { tried: Vec::new() }),
        };
        self.pending = None;
        eprintln!("sela: {}", log_line(&result));
        self.availability = Availability::from_result(&result);
        true
    }

    /// The status bar warning, or `None` while checking or when ready.
    pub(crate) fn warning(&self) -> Option<&'static str> {
        let Availability::Unavailable(why) = self.availability else {
            return None;
        };
        Some(t!(match why {
            Unavailable::Missing => Msg::MediaUnavailableMissing,
            Unavailable::Gpl => Msg::MediaUnavailableGpl,
            Unavailable::Nonfree => Msg::MediaUnavailableNonfree,
            Unavailable::Broken => Msg::MediaUnavailableBroken,
        }))
    }
}

impl Operator {
    pub(crate) fn poll_media_backend(&mut self) -> bool {
        self.media_backend.poll()
    }

    /// The accepted FFmpeg build, for the video stream (M2-02 playback).
    // Only tests call it until the video-playback stream wires decoding in.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn media_tools(&self) -> Option<sela::media::decode::Tools> {
        self.media_backend
            .availability
            .located()
            .map(sela::media::decode::Tools::from)
    }

    /// Status bar item: an amber dot and the reason, exposed as a status.
    pub(crate) fn media_warning(&self) -> Option<gpui::AnyElement> {
        let text = self.media_backend.warning()?;
        Some(
            div()
                .id("media-unavailable")
                .debug_selector(|| "media-unavailable".into())
                .role(Role::Status)
                .aria_label(text)
                .accessibility_id("media-unavailable")
                // Shrinks and truncates in a narrow header (the accessible
                // name keeps the full text, hover shows it in a hint) so the
                // controls keep their size.
                .tooltip(StatusHint::builder(text))
                .min_w_0()
                .overflow_hidden()
                .flex()
                .items_center()
                .gap_1()
                .text_size(px(TEXT_CAPTION))
                .text_color(rgb(SENDING))
                .child(
                    div()
                        .flex_shrink_0()
                        .size(px(6.))
                        .rounded_full()
                        .bg(rgb(SENDING)),
                )
                .child(div().min_w_0().truncate().child(text))
                .into_any_element(),
        )
    }
}
