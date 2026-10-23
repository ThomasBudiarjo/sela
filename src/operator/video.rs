//! Operator side of video (M2-02/M2-03/M2-05 basic): the Media tab's Videos
//! list with poster thumbnails, video import, the pin checks a video cue
//! needs, and the Live pane's transport row.
//!
//! All file and FFmpeg work runs on `videos::Worker` (scan, import),
//! `videos::Thumbnailer` (posters) and `videos::Checker` (pin checks); the UI
//! thread only submits, polls and reads bounded caches. Thumbnails are the
//! lowest priority: the renderer decodes the live video in its own process,
//! and checks for the live slide are wanted ahead of the previewed one.
//!
//! EasyWorship references: EW8-OBS-019 (Media › Images, +), EW8-OBS-039
//! (Videos listed first, thumbnails named without extensions), SRC-10
//! (Repeating loops until stopped or something else goes live). Placement
//! of the Videos category in the Media tab, the transport row and its
//! labels are UNOBSERVED best guesses (docs/reference-observations.md,
//! "Video playback best guesses").
use super::*;
use sela::{
    background::{Aspect, VideoRef},
    media::backend::Availability,
    playback::{MediaAction, PlayState, clock_text},
    videos::{self, Listed, Lru, Substitute, ThumbKey},
};

/// Thumbnails wanted for the Videos list at once (rows past this show a
/// placeholder until the list is shorter).
pub(crate) const LIST_THUMBS: usize = 64;
/// Pin checks remembered (live, preview and their neighbours).
const CHECKS: usize = 32;
/// List row picture, in logical pixels (half of `videos::THUMB`).
const ROW_THUMB_W: f32 = 80.;
const ROW_THUMB_H: f32 = 45.;
/// First tab index of the Videos list rows.
pub(crate) const VIDEO_TAB_INDEX: isize = 2500;

/// The Media tab's category (EW8-OBS-019 Media › Images; the Videos
/// category's place is a best guess). Images stays the default so the tab
/// opens as before.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Category {
    Videos,
    #[default]
    Images,
}

pub(crate) struct VideoState {
    pub(crate) category: Category,
    profile: Option<PathBuf>,
    worker: Option<videos::Worker>,
    pub(crate) listed: Vec<Listed>,
    scanned: bool,
    pub(crate) selected: Option<String>,
    pub(crate) rows: Vec<FocusHandle>,
    /// The file name of an import in flight.
    pub(crate) importing: Option<String>,
    pub(crate) message: Option<String>,
    thumbs: Option<videos::Thumbnailer>,
    /// Decoded posters (BGRA images for GPUI); `None` = could not be made.
    pub(crate) thumb_cache: Lru<ThumbKey, Option<Arc<gpui::RenderImage>>>,
    /// The list last handed to the thumbnailer, so an unchanged list is
    /// not re-sent every poll (the worker itself never repeats the poster
    /// it is making: the in-flight deduplication of M2-05).
    wanted_thumbs: Vec<ThumbKey>,
    checks: Option<videos::Checker>,
    pub(crate) checked: Lru<VideoRef, Result<PathBuf, Substitute>>,
    wanted_checks: Vec<VideoRef>,
}

impl VideoState {
    pub(crate) fn start(profile: Option<PathBuf>) -> Self {
        let worker = profile
            .clone()
            .and_then(|profile| videos::Worker::start(profile).ok());
        let checks = profile
            .clone()
            .and_then(|profile| videos::Checker::checks(profile).ok());
        let mut state = Self {
            category: Category::default(),
            profile,
            worker,
            listed: Vec::new(),
            scanned: false,
            selected: None,
            rows: Vec::new(),
            importing: None,
            message: None,
            thumbs: None,
            thumb_cache: Lru::new(videos::THUMB_BUDGET, videos::THUMB_ENTRIES),
            wanted_thumbs: Vec::new(),
            checks,
            checked: Lru::new(usize::MAX, CHECKS),
            wanted_checks: Vec::new(),
        };
        state.rescan();
        state
    }

    fn rescan(&mut self) {
        if let Some(worker) = &self.worker
            && worker.submit(videos::Job::Scan).is_ok()
        {
            self.scanned = true;
        }
    }

    /// The listed file's poster, once made.
    pub(crate) fn list_thumb(&self, listed: &Listed) -> Option<Arc<gpui::RenderImage>> {
        self.peek(&ThumbKey::File(listed.clone()))
    }

    /// A cached poster; `Lru::get` would reorder, a render only reads.
    fn peek(&self, key: &ThumbKey) -> Option<Arc<gpui::RenderImage>> {
        self.thumb_cache
            .iter()
            .find(|(k, _)| *k == key)
            .and_then(|(_, v)| v.clone())
    }

    /// Every cached poster image, to find the ones an update evicted.
    fn thumb_images(&self) -> Vec<Arc<gpui::RenderImage>> {
        self.thumb_cache
            .iter()
            .filter_map(|(_, v)| v.clone())
            .collect()
    }
}

/// Releases the atlas sprites of posters no longer cached. Called outside
/// any window update, so `None` reaches every window.
fn release(
    before: Vec<Arc<gpui::RenderImage>>,
    after: &[Arc<gpui::RenderImage>],
    cx: &mut Context<Operator>,
) {
    for image in before {
        if !after.iter().any(|kept| Arc::ptr_eq(kept, &image)) {
            cx.drop_image(image, None);
        }
    }
}

/// `m:ss / m:ss` of a status, or just the position when the length is unknown.
fn position_text(position: Duration, duration: Option<Duration>) -> String {
    match duration {
        Some(duration) => format!("{} / {}", clock_text(position), clock_text(duration)),
        None => clock_text(position),
    }
}

impl Operator {
    /// The slide on the audience now (acknowledged), if its background is a video.
    /// The video of the slide the renderer acknowledged. A video that could
    /// not be played went out as black (a warning substituted it), so it has
    /// no transport.
    pub(crate) fn live_video(&self) -> Option<(VideoRef, Aspect, bool)> {
        let item = self.on_air.as_ref()?;
        let index = self.on_air_slide?;
        // Acknowledged, not requested: a refused video cue left the
        // previous scene on the audience, so it has no transport either.
        let LiveState::Confirmed(confirmed) = self.output.as_ref()?.live() else {
            return None;
        };
        if slides::slide_version(item.version, index) != Some(confirmed) {
            return None;
        }
        if !self
            .presented
            .is_some_and(|(_, presented, clean)| clean && presented == index)
        {
            return None;
        }
        let slide = item.slides.get(index)?;
        match background::plan(slide.background.as_ref()) {
            Plan::Video(video, aspect, repeat) => Some((video.clone(), aspect, repeat)),
            _ => None,
        }
    }

    /// Video pins wanted soon, most urgent first: a deferred cue, the live
    /// slide, the previewed slide, then their neighbours.
    fn wanted_checks(&self) -> Vec<VideoRef> {
        let mut centers: Vec<(&Item, usize)> = Vec::new();
        if let Some(live) = &self.live {
            centers.extend(self.deferred_send.map(|d| (live, d.index)));
            centers.extend(self.live_slide.map(|index| (live, index)));
        }
        if let Some(preview) = &self.preview {
            centers.push((preview, self.preview_slide));
        }
        let mut wanted = Vec::new();
        for offset in [0, 1, -1] {
            for (item, index) in &centers {
                let Some(slide) = index
                    .checked_add_signed(offset)
                    .and_then(|i| item.slides.get(i))
                else {
                    continue;
                };
                if let Plan::Video(video, ..) = background::plan(slide.background.as_ref())
                    && !wanted.contains(video)
                {
                    wanted.push(video.clone());
                }
            }
        }
        wanted.truncate(CHECKS / 2);
        wanted
    }

    /// Posters wanted: the previewed and live items' video tiles first,
    /// then the Videos list rows while that list is showing.
    fn wanted_video_thumbs(&self) -> Vec<ThumbKey> {
        let mut wanted = Vec::new();
        for item in self.preview.iter().chain(self.live.iter()) {
            for slide in item.slides.iter() {
                if let Plan::Video(video, aspect, _) = background::plan(slide.background.as_ref()) {
                    let key = ThumbKey::Pinned(video.clone(), aspect, images::TILE);
                    if !wanted.contains(&key) {
                        wanted.push(key);
                    }
                }
            }
        }
        if !self.collapsed && self.tab == 2 && self.video.category == Category::Videos {
            wanted.extend(
                self.video
                    .listed
                    .iter()
                    .take(LIST_THUMBS)
                    .map(|listed| ThumbKey::File(listed.clone())),
            );
        }
        wanted
    }

    /// A slide tile's poster for a video background.
    pub(crate) fn video_tile(
        &self,
        video: &VideoRef,
        aspect: Aspect,
    ) -> Option<Arc<gpui::RenderImage>> {
        self.video
            .peek(&ThumbKey::Pinned(video.clone(), aspect, images::TILE))
    }

    /// The cue background for a video slide: the playable path once its pin
    /// was checked, black with the substitution warning when it cannot play
    /// (the image rule), or `None` while a check or the FFmpeg lookup is
    /// still running (the cue is deferred and retried when it lands).
    pub(crate) fn video_background(
        &mut self,
        video: &VideoRef,
        aspect: Aspect,
        repeat: bool,
    ) -> Option<(PreparedBackground, Option<String>)> {
        let black = |why: Substitute| {
            Some((
                slides::color_background(background::BLACK),
                Some(why.warning(&video.name)),
            ))
        };
        match self.media_backend.availability {
            Availability::Checking => return None,
            Availability::Unavailable(_) => return black(Substitute::Unavailable),
            Availability::Ready(_) => {}
        }
        if self.profile.is_none() || self.video.checks.is_none() {
            return black(Substitute::Missing);
        }
        match self.video.checked.get(video).cloned() {
            Some(Ok(path)) => Some((
                PreparedBackground::Video {
                    version: videos::background_version(video, aspect, repeat),
                    source: Arc::new(sela::scene::VideoSource {
                        path,
                        aspect,
                        repeat,
                    }),
                },
                None,
            )),
            Some(Err(why)) => black(why),
            None => {
                // This one first; the others follow on the next poll.
                if let Some(checks) = &self.video.checks {
                    let mut wanted = vec![video.clone()];
                    wanted.extend(self.wanted_checks().into_iter().filter(|v| v != video));
                    checks.want(wanted.clone());
                    self.video.wanted_checks = wanted;
                }
                None
            }
        }
    }

    /// The previewed slide's video substitution, once checked.
    pub(crate) fn preview_video_warning(&self) -> Option<String> {
        let slide = self.preview.as_ref()?.slides.get(self.preview_slide)?;
        let Plan::Video(video, ..) = background::plan(slide.background.as_ref()) else {
            return None;
        };
        match self.media_backend.availability {
            Availability::Unavailable(_) => {
                return Some(Substitute::Unavailable.warning(&video.name));
            }
            Availability::Checking => return None,
            Availability::Ready(_) => {}
        }
        self.video
            .checked
            .iter()
            .find(|(k, _)| *k == video)
            .and_then(|(_, v)| v.as_ref().err())
            .map(|why| why.warning(&video.name))
    }

    /// Worker, thumbnail and check replies, then the next wanted work.
    /// Returns true when something visible changed.
    pub(crate) fn poll_videos(&mut self, cx: &mut Context<Self>) -> bool {
        let mut changed = false;
        let mut rescan = false;
        let mut retry = false;
        for _ in 0..4 {
            let Some(reply) = self.video.worker.as_ref().and_then(videos::Worker::poll) else {
                break;
            };
            changed = true;
            match reply {
                Err(error) => {
                    self.video.message = Some(error.message());
                    self.video.importing = None;
                    self.video.worker = None;
                    break;
                }
                Ok(videos::Reply::Videos(Ok(listed))) => {
                    if self
                        .video
                        .selected
                        .as_ref()
                        .is_some_and(|s| !listed.iter().any(|l| l.name == *s))
                    {
                        self.video.selected = None;
                    }
                    // Files that changed or left get new keys; their old
                    // posters go, and a missing video may be back.
                    let keep: Vec<Listed> = listed.clone();
                    let before = self.video.thumb_images();
                    self.video.thumb_cache.retain(|key| match key {
                        ThumbKey::File(file) => keep.contains(file),
                        ThumbKey::Pinned(..) => true,
                    });
                    release(before, &self.video.thumb_images(), cx);
                    self.video.checked.retain(|_| false);
                    self.video.wanted_checks.clear();
                    self.video.listed = listed;
                    retry = true;
                }
                Ok(videos::Reply::Videos(Err(error))) => {
                    self.video.message = Some(error.message());
                }
                Ok(videos::Reply::Imported(Ok(imported))) => {
                    self.video.importing = None;
                    self.video.message = Some(if imported.reused {
                        t!(Msg::VideoImportReused, name = videos::stem(&imported.name))
                    } else {
                        t!(Msg::VideoImported, name = videos::stem(&imported.name))
                    });
                    self.video.selected = Some(imported.name);
                    rescan = true;
                }
                Ok(videos::Reply::Imported(Err(error))) => {
                    self.video.importing = None;
                    self.video.message = Some(if error == videos::Error::Cancelled {
                        error.message()
                    } else {
                        t!(Msg::VideoImportFailed, error = error.message())
                    });
                }
            }
        }
        if rescan {
            self.video.rescan();
        }
        // Pin checks: the live cue may be waiting for one.
        for _ in 0..8 {
            let Some((video, result)) = self.video.checks.as_ref().and_then(|c| c.poll()) else {
                break;
            };
            changed = true;
            self.video.checked.insert(video, result, 0);
            retry = true;
        }
        if retry && self.deferred_send.is_some() {
            self.retry_deferred();
        }
        // Thumbnails start once FFmpeg is located.
        if self.video.thumbs.is_none()
            && let (Some(profile), Some(tools)) = (self.video.profile.clone(), self.media_tools())
        {
            self.video.thumbs = videos::Thumbnailer::thumbnails(profile, tools).ok();
        }
        for _ in 0..4 {
            let Some((key, thumb)) = self.video.thumbs.as_ref().and_then(|t| t.poll()) else {
                break;
            };
            changed = true;
            let extent = key.extent();
            let rendered = thumb
                .ok()
                .and_then(|rgba| preview_live::render_image(&rgba, extent));
            let bytes = (extent.width * extent.height * 4) as usize;
            let before = self.video.thumb_images();
            self.video.thumb_cache.insert(key, rendered, bytes);
            release(before, &self.video.thumb_images(), cx);
        }
        // Next wanted work, only when the lists changed.
        let checks = self.wanted_checks();
        let missing: Vec<VideoRef> = checks
            .into_iter()
            .filter(|v| !self.video.checked.contains(v))
            .collect();
        if missing != self.video.wanted_checks
            && let Some(checker) = &self.video.checks
        {
            checker.want(missing.clone());
            self.video.wanted_checks = missing;
        }
        let thumbs: Vec<ThumbKey> = self
            .wanted_video_thumbs()
            .into_iter()
            .filter(|k| !self.video.thumb_cache.contains(k))
            .collect();
        if thumbs != self.video.wanted_thumbs
            && let Some(thumbnailer) = &self.video.thumbs
        {
            thumbnailer.want(thumbs.clone());
            self.video.wanted_thumbs = thumbs;
        }
        changed
    }

    pub(crate) fn prompt_video_import(&mut self, cx: &mut Context<Self>) {
        if self.video.worker.is_none() {
            self.video.message = Some(t!(Msg::NoProfileVideoImport).into());
            return;
        }
        if self.video.importing.is_some() {
            return;
        }
        if self.media_tools().is_none() {
            self.video.message = Some(videos::Error::Unavailable.message());
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t!(Msg::ImportPrompt).into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.start_video_import(path);
                cx.notify();
            });
        })
        .detach();
    }

    /// Queues a video import: probed by FFmpeg, then copied into the
    /// profile on the videos worker, with Cancel in the footer meanwhile.
    pub(crate) fn start_video_import(&mut self, path: PathBuf) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (Some(worker), Some(tools)) = (&self.video.worker, self.media_tools()) else {
            self.video.message = Some(videos::Error::Unavailable.message());
            return;
        };
        match worker.submit(videos::Job::Import(path, tools)) {
            Ok(()) => {
                self.video.message = Some(t!(Msg::ImportingName, name = name));
                self.video.importing = Some(name);
            }
            Err(error) => self.video.message = Some(error.message()),
        }
    }

    pub(crate) fn cancel_video_import(&mut self) {
        if let Some(worker) = &self.video.worker
            && self.video.importing.is_some()
        {
            worker.cancel_imports();
            self.video.message = Some(t!(Msg::ImportCancelling).into());
        }
    }

    /// Play/Pause for the live video: pauses a playing video, plays one
    /// that is paused, primed or ended (the renderer's report decides).
    pub(crate) fn toggle_video_playback(&mut self) {
        let Some(output) = &self.output else {
            return;
        };
        let action = match output.media_pending().or_else(|| {
            output.media_status().map(|s| match s.state {
                PlayState::Playing => MediaAction::Play,
                _ => MediaAction::Pause,
            })
        }) {
            // The newest request or report flips.
            Some(MediaAction::Play) | Some(MediaAction::Restart) => MediaAction::Pause,
            Some(MediaAction::Pause) => MediaAction::Play,
            None => MediaAction::Pause,
        };
        self.send_media(action);
    }

    pub(crate) fn restart_video(&mut self) {
        self.send_media(MediaAction::Restart);
    }

    fn send_media(&mut self, action: MediaAction) {
        if self.live_video().is_none() {
            self.live_message = Some(t!(Msg::NoVideoLive).into());
            return;
        }
        let Some(output) = &mut self.output else {
            return;
        };
        if output.set_media(action, Instant::now()).is_err() {
            self.live_message = Some(t!(Msg::RefusalNotConnected).into());
        }
    }

    /// Live pane transport row, shown while the acknowledged live slide
    /// plays a video: Play/Pause, Restart and the renderer's reported
    /// position and state (what is presented, not what was requested).
    pub(crate) fn video_transport(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        self.live_video()?;
        let output = self.output.as_ref();
        let status = output.and_then(Supervisor::media_status);
        let pending = output.and_then(Supervisor::media_pending);
        let playing = match pending {
            Some(MediaAction::Pause) => false,
            Some(_) => true,
            None => status.is_none_or(|s| s.state == PlayState::Playing),
        };
        let state = match status.map(|s| s.state) {
            None | Some(PlayState::Primed) => t!(Msg::VideoStarting),
            Some(PlayState::Playing) => t!(Msg::VideoPlaying),
            Some(PlayState::Paused) => t!(Msg::VideoPaused),
            Some(PlayState::Ended) => t!(Msg::VideoEnded),
            Some(PlayState::Failed) => t!(Msg::VideoFailed),
        };
        let line = match status {
            Some(status) => format!(
                "{state} · {}",
                position_text(status.position, status.duration)
            ),
            None => state.to_owned(),
        };
        let rejected = output.and_then(Supervisor::media_rejected).map(|(_, e)| e);
        Some(
            div()
                .id("video-transport")
                .debug_selector(|| "video-transport".into())
                .role(Role::Group)
                .aria_label(t!(Msg::VideoTransport))
                .accessibility_id("video-transport")
                .h(px(PANE_HEADER_H))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .border_t_1()
                .border_color(rgb(BORDER))
                .child(
                    self.button(
                        Control::VideoPlayPause,
                        "video-play-pause",
                        if playing {
                            t!(Msg::VideoPause)
                        } else {
                            t!(Msg::VideoPlay)
                        },
                        cx,
                    )
                    .aria_toggled(if playing {
                        Toggled::True
                    } else {
                        Toggled::False
                    }),
                )
                .child(self.button(
                    Control::VideoRestart,
                    "video-restart",
                    t!(Msg::VideoRestart),
                    cx,
                ))
                .child(
                    div()
                        .id("video-position")
                        .debug_selector(|| "video-position".into())
                        .role(Role::Status)
                        .aria_label(line.clone())
                        .accessibility_id("video-position")
                        .min_w_0()
                        .truncate()
                        .text_size(px(TEXT_CAPTION))
                        .text_color(rgb(
                            if status.is_some_and(|s| s.state == PlayState::Failed) {
                                ERROR
                            } else {
                                MUTED
                            },
                        ))
                        .child(line),
                )
                .children(rejected.map(|error| {
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(TEXT_CAPTION))
                        .text_color(rgb(ERROR))
                        .child(t!(Msg::VideoCommandRefused, error = error))
                }))
                .into_any_element(),
        )
    }

    /// Media tab sidebar: the Videos and Images categories.
    pub(crate) fn media_sidebar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut entry = |control: Control, id: &'static str, msg: Msg, category: Category| {
            let active = self.video.category == category;
            self.button(control, id, t!(msg), cx)
                .aria_selected(active)
                .w_full()
                .when(active, |d| {
                    d.bg(rgb(SURFACE))
                        .text_color(rgb(TEXT))
                        .font_weight(FontWeight::MEDIUM)
                })
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(t!(Msg::MediaHeader)),
            )
            .child(
                div()
                    .id("media-categories")
                    .role(Role::TabList)
                    .aria_label(t!(Msg::MediaHeader))
                    .accessibility_id("media-categories")
                    .flex()
                    .flex_col()
                    .child(entry(
                        Control::MediaVideos,
                        "media-category-videos",
                        Msg::VideosHeader,
                        Category::Videos,
                    ))
                    .child(entry(
                        Control::MediaImages,
                        "media-category-images",
                        Msg::ImagesEmptyTitle,
                        Category::Images,
                    )),
            )
            .child(
                div()
                    .mt_3()
                    .font_weight(FontWeight::MEDIUM)
                    .child(t!(Msg::CollectionsHeader)),
            )
            .child(
                div()
                    .mt_2()
                    .text_color(rgb(MUTED))
                    .child(t!(Msg::NoCollections)),
            )
            .into_any_element()
    }

    /// The Videos list: a poster and the name without its extension per row
    /// (EW8-OBS-039 names media without extensions).
    pub(crate) fn video_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        while self.video.rows.len() < self.video.listed.len() {
            let index = self.video.rows.len() as isize;
            self.video.rows.push(
                cx.focus_handle()
                    .tab_index(VIDEO_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        if self.profile.is_none() {
            return resources::empty_detail(t!(Msg::VideosNoProfile), t!(Msg::ImagesNoProfileHint));
        }
        if self.video.listed.is_empty() {
            return resources::empty_detail(t!(Msg::VideosHeader), t!(Msg::VideosEmptyHint));
        }
        div()
            .id("video-list")
            .role(Role::List)
            .aria_label(t!(Msg::VideosHeader))
            .accessibility_id("video-list")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(self.video.listed.iter().enumerate().map(|(index, listed)| {
                let selected = self.video.selected.as_ref() == Some(&listed.name);
                let label = videos::stem(&listed.name).to_owned();
                let thumb = (index < LIST_THUMBS)
                    .then(|| self.video.list_thumb(listed))
                    .flatten();
                let select = move |this: &mut Self| {
                    this.video.selected = this.video.listed.get(index).map(|l| l.name.clone());
                };
                div()
                    .id(("video", index))
                    .debug_selector(move || format!("video-{index}"))
                    .role(Role::ListItem)
                    .aria_label(label.clone())
                    .aria_selected(selected)
                    .accessibility_id(format!("video-{index}"))
                    .track_focus(&self.video.rows[index])
                    .key_context("SelaControl")
                    .flex_shrink_0()
                    .h(px(ROW_THUMB_H + 8.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .cursor_pointer()
                    .bg(rgb(if selected { SELECTED_ROW } else { SURFACE }))
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        window.prevent_default();
                        select(this);
                        cx.notify();
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                return;
                            }
                            this.video.rows[index].focus(window, cx);
                            select(this);
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .debug_selector(move || format!("video-thumb-{index}"))
                            .w(px(ROW_THUMB_W))
                            .h(px(ROW_THUMB_H))
                            .flex_shrink_0()
                            .rounded(px(RADIUS_SMALL))
                            .overflow_hidden()
                            .bg(rgb(BLACK))
                            .children(thumb.map(|image| {
                                gpui::img(image)
                                    .size_full()
                                    .object_fit(gpui::ObjectFit::Cover)
                            })),
                    )
                    .child(div().min_w_0().truncate().child(label))
            }))
            .into_any_element()
    }

    /// Media tab footer while Videos is showing.
    pub(crate) fn video_footer(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let importing = self.video.importing.is_some();
        div()
            .h(px(STATUS_BAR_H))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(
                self.button(
                    Control::ImportVideo,
                    "import-video",
                    t!(Msg::ImportVideo),
                    cx,
                )
                .when(importing, |d| mark_disabled(d).text_color(rgb(DISABLED))),
            )
            .when(importing, |d| {
                d.child(self.button(
                    Control::CancelVideoImport,
                    "cancel-video-import",
                    t!(Msg::ImportCancel),
                    cx,
                ))
            })
            .children(self.video.message.clone().map(|message| {
                div()
                    .id("video-message")
                    .debug_selector(|| "video-message".into())
                    .role(Role::Status)
                    .aria_label(message.clone())
                    .accessibility_id("video-message")
                    .px_2()
                    .min_w_0()
                    .truncate()
                    .text_size(px(TEXT_CAPTION))
                    .text_color(rgb(MUTED))
                    .child(message)
            }))
            .into_any_element()
    }
}
