use super::*;
use sela::preparation::cue::LIVE_DEPTH;

/// Thumbnails kept for tiles at once.
const MAX_THUMBNAILS: usize = 32;

/// What an item's slides were projected from: the song revision and the
/// stored arrangement choice. Items with the same key show the same slides,
/// so only they can share a face resolution; Preview and Live may hold two
/// arrangements of one revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Projected {
    version: Version,
    chosen: Option<VariantId>,
}

/// A revision's default projection: no stored choice, so the first
/// arrangement.
impl From<Version> for Projected {
    fn from(version: Version) -> Self {
        Self {
            version,
            chosen: None,
        }
    }
}

impl Item {
    fn projected(&self) -> Projected {
        Projected {
            version: self.version,
            chosen: self.chosen,
        }
    }
}

/// GPUI draws BGRA; the fitted background is RGBA.
fn render_image(rgba: &[u8], extent: Extent) -> Option<Arc<gpui::RenderImage>> {
    let mut bgra = rgba.to_vec();
    bgra.as_chunks_mut::<4>()
        .0
        .iter_mut()
        .for_each(|px| px.swap(0, 2));
    let buffer = image::RgbaImage::from_raw(extent.width, extent.height, bgra)?;
    Some(Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        buffer,
    )])))
}

const TILE_BOX_H: f32 = 94.;
const TILE_BOX_W: f32 = 168. - 8.;
/// Room left for text inside the tile's border and padding.
const TILE_TEXT_H: f32 = TILE_BOX_H - 8.;
const TILE_TEXT_W: f32 = TILE_BOX_W - 12.;
const TILE_LINE_HEIGHT: f32 = 1.3;
/// Average glyph advance in em, for the width fit of unshaped tile text.
const TILE_GLYPH_EM: f32 = 0.6;
const TILE_MIN_TEXT: f32 = 2.;

/// Text size that fits every line of `text` inside the tile, the same
/// whole-page rule the audience applies (height by line count, width by the
/// longest line), never above the regular tile size.
fn tile_text_size(text: &str) -> f32 {
    let lines = text.lines().count().max(1) as f32;
    let widest = text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    TEXT_TILE
        .min(TILE_TEXT_H / (lines * TILE_LINE_HEIGHT))
        .min(TILE_TEXT_W / (widest * TILE_GLYPH_EM))
        .max(TILE_MIN_TEXT)
}

/// Slide thumbnail: the image background when `thumb` has landed, else the
/// slide's fill color (black while an image is still loading), under the
/// slide text; not a rendered audience frame.
pub(crate) fn tile(
    slide: &Slide,
    index: usize,
    page: Option<slides::Page>,
    border: u32,
    thumb: Option<Arc<gpui::RenderImage>>,
) -> gpui::Div {
    let fill = match background::plan(slide.background.as_ref()) {
        Plan::Color([r, g, b]) => u32::from_be_bytes([0, r, g, b]),
        Plan::Image(..) => BLACK,
    };
    div()
        .w(px(168.))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap_1()
        .p_1()
        .rounded(px(RADIUS))
        .child(
            div()
                .h(px(94.))
                .rounded(px(RADIUS_SMALL))
                .border_2()
                .border_color(rgb(border))
                .bg(rgb(fill))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .relative()
                .px_1()
                .debug_selector(move || format!("tile-box-{index}"))
                .text_size(px(tile_text_size(&slide.text)))
                .line_height(gpui::relative(TILE_LINE_HEIGHT))
                .text_color(rgb(DARK_TEXT))
                .children(thumb.map(|image| {
                    gpui::img(image)
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .object_fit(gpui::ObjectFit::Cover)
                }))
                .child(
                    div()
                        .debug_selector(move || format!("tile-text-{index}"))
                        .flex()
                        .flex_col()
                        .items_center()
                        .whitespace_nowrap()
                        .children(slide.text.lines().map(|line| {
                            // An empty line keeps its height in the stack.
                            div().text_center().child(if line.is_empty() {
                                " ".to_owned()
                            } else {
                                line.to_owned()
                            })
                        })),
                ),
        )
        .child(
            div()
                .text_size(px(TEXT_CAPTION))
                .text_color(rgb(CANVAS_TEXT))
                .truncate()
                .child(slide_caption(index, slide, page)),
        )
}

// Each argument is a distinct pane region; a struct would only rename them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn slide_pane(
    id: &'static str,
    title: String,
    width: f32,
    footer: String,
    status: Option<Vec<(String, u32)>>,
    toolbar: Option<gpui::AnyElement>,
    tiles: Option<Vec<gpui::Stateful<gpui::Div>>>,
    focus: Option<FocusHandle>,
) -> impl IntoElement {
    let empty = tiles.as_ref().is_none_or(Vec::is_empty);
    let list_name = title.clone();
    let status_id = SharedString::from(format!("{id}-status"));
    div()
        .debug_selector(|| id.into())
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .h(px(PANE_HEADER_H))
                .flex_shrink_0()
                .flex()
                .items_center()
                .px(px(PANE_PAD_X))
                .font_weight(FontWeight::MEDIUM)
                .border_b_1()
                .border_color(rgb(BORDER))
                .bg(rgb(CHROME))
                .child(title),
        )
        .children(toolbar)
        .children(status.map(|lines| {
            let spoken = lines
                .iter()
                .map(|(line, _)| line.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            div()
                .id(status_id.clone())
                .role(Role::Group)
                .aria_label(spoken)
                .accessibility_id(status_id.clone())
                .debug_selector(move || format!("{id}-status"))
                .flex_shrink_0()
                .px(px(PANE_PAD_X))
                .py_1()
                .bg(rgb(CANVAS))
                .border_b_1()
                .border_color(rgb(CANVAS_BORDER))
                .text_size(px(TEXT_SMALL))
                .children(
                    lines
                        .into_iter()
                        .map(|(line, color)| div().truncate().text_color(rgb(color)).child(line)),
                )
        }))
        .child(
            div()
                .id(id)
                .role(Role::List)
                .aria_label(list_name)
                .accessibility_id(id)
                .flex_1()
                .min_h_0()
                .p(px(SPACE_S))
                .overflow_y_scroll()
                .bg(rgb(CANVAS))
                .text_size(px(TEXT_BASE))
                .text_color(rgb(CANVAS_TEXT))
                .when_some(focus, |d, handle| {
                    d.track_focus(&handle).key_context("SelaLivePane")
                })
                .when(empty, |d| {
                    d.flex()
                        .items_center()
                        .justify_center()
                        .child(if id == "Live" {
                            t!(Msg::NothingLive)
                        } else {
                            t!(Msg::NothingSelected)
                        })
                })
                .when_some(tiles.filter(|t| !t.is_empty()), |d, tiles| {
                    d.flex().flex_wrap().gap(px(ROW_GAP)).children(tiles)
                }),
        )
        .child(
            div()
                .h(px(PANE_FOOTER_H))
                .flex_shrink_0()
                .flex()
                .items_center()
                .px(px(PANE_PAD_X))
                .border_t_1()
                .border_color(rgb(BORDER))
                .bg(rgb(CHROME))
                .text_size(px(TEXT_CAPTION))
                .text_color(rgb(MUTED))
                .truncate()
                .child(footer),
        )
}

impl Operator {
    /// Double-click in Songs goes straight to Live (documented), from the
    /// first slide (provisional).
    pub(crate) fn song_to_live(&mut self, index: usize) {
        let Some((version, _)) = self.songs().get(index).cloned() else {
            return;
        };
        // A library Go Live is a newer explicit request even while its song is
        // still loading; an older double-click must not unmask the previous item.
        self.unmask.cancel();
        self.selected_song = Some(version);
        self.clear_entry_selection();
        if self
            .preview
            .as_ref()
            .is_some_and(|p| p.version == version && p.entry.is_none())
        {
            self.live_on_load = None;
            self.preview_slide = 0;
            self.go_live();
        } else {
            self.live_on_load = Some(version);
        }
    }

    pub(crate) fn background_extent(&self) -> Extent {
        self.output_extent.unwrap_or(NOMINAL_EXTENT)
    }

    /// What an import fits to: the connected output, else the configured
    /// audience monitor, else the nominal size.
    pub(crate) fn import_extent(&self) -> Extent {
        self.output_extent
            .or_else(|| self.settings.audience_extent())
            .unwrap_or(NOMINAL_EXTENT)
    }

    /// Backgrounds wanted soon, most urgent first: a deferred cue, the live
    /// slide, the previewed slide, then both neighbours of each. Capped at
    /// what the cache holds for this extent, so prefetching never evicts its
    /// own work.
    pub(crate) fn wanted_backgrounds(&self) -> Vec<images::BackgroundKey> {
        let extent = self.background_extent();
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
                if let Plan::Image(image, aspect) = background::plan(slide.background.as_ref()) {
                    let key = (image.clone(), aspect, extent);
                    if !wanted.contains(&key) {
                        wanted.push(key);
                    }
                }
            }
        }
        wanted.truncate(self.backgrounds.capacity(extent));
        wanted
    }

    /// Tile thumbnails wanted for the previewed and live items: the image
    /// backgrounds of their slides at tile size, bounded.
    pub(crate) fn wanted_thumbnails(&self) -> Vec<images::BackgroundKey> {
        let mut wanted = Vec::new();
        for item in self.preview.iter().chain(self.live.iter()) {
            for slide in item.slides.iter() {
                if let Plan::Image(image, aspect) = background::plan(slide.background.as_ref()) {
                    let key = (image.clone(), aspect, images::TILE);
                    if !wanted.contains(&key) {
                        wanted.push(key);
                    }
                    if wanted.len() >= MAX_THUMBNAILS {
                        return wanted;
                    }
                }
            }
        }
        wanted
    }

    /// The decoded tile thumbnail for a slide's image background, once the
    /// image worker has delivered it.
    pub(crate) fn thumbnail(&self, slide: &Slide) -> Option<Arc<gpui::RenderImage>> {
        let Plan::Image(image, aspect) = background::plan(slide.background.as_ref()) else {
            return None;
        };
        let key = (image.clone(), aspect, images::TILE);
        self.tile_images
            .iter()
            .find(|(k, _)| *k == key)
            .and_then(|(_, rendered)| rendered.clone())
    }

    /// Keeps a landed thumbnail (a failed one is remembered as `None` so it is
    /// not requested again) and drops those no longer wanted.
    pub(crate) fn store_thumbnail(&mut self, key: images::BackgroundKey, fitted: images::Fitted) {
        let rendered = fitted.ok().and_then(|rgba| render_image(&rgba, key.2));
        let wanted = self.wanted_thumbnails();
        self.tile_images.retain(|(k, _)| wanted.contains(k));
        self.tile_images.retain(|(k, _)| *k != key);
        self.tile_images.push((key, rendered));
    }

    /// Hands the most urgent missing background to the image worker. Wanted
    /// entries are refreshed least urgent first, so eviction spares them.
    pub(crate) fn prefetch_backgrounds(&mut self) {
        if self.background_job.is_some() || self.image_worker.is_none() {
            return;
        }
        let mut missing = None;
        // Tile thumbnails are the least urgent, so they are tried first and
        // any missing full-size background overwrites them.
        for key in self.wanted_thumbnails().into_iter().rev() {
            if !self.tile_images.iter().any(|(k, _)| *k == key) {
                missing = Some(key);
            }
        }
        for key in self.wanted_backgrounds().into_iter().rev() {
            if self.backgrounds.get(&key).is_none() {
                missing = Some(key);
            }
        }
        let (Some(key), Some(worker)) = (missing, &self.image_worker) else {
            return;
        };
        // A full queue (an import is running) is retried on the next poll.
        if worker.submit(images::Job::Background(key.clone())).is_ok() {
            self.background_job = Some(key);
        }
    }

    /// The previewed slide's background substitution, if its image could
    /// not be prepared (owner rule: black plus this warning).
    pub(crate) fn preview_warning(&self) -> Option<String> {
        let slide = self.preview.as_ref()?.slides.get(self.preview_slide)?;
        let Plan::Image(image, aspect) = background::plan(slide.background.as_ref()) else {
            return None;
        };
        if self.image_worker.is_none() {
            return Some(images::Substitute::Missing.warning(&image.name));
        }
        match self
            .backgrounds
            .peek(&(image.clone(), aspect, self.background_extent()))
        {
            Some(Err(why)) => Some(why.warning(&image.name)),
            _ => None,
        }
    }

    pub(crate) fn toggle_mask(&mut self, mask: Mask) {
        if mask == Mask::Logo && !self.masks.is_on(Mask::Logo) && self.logo.is_none() {
            self.live_message = Some(t!(Msg::NoLogoSet).into());
            return;
        }
        self.masks.toggle(mask);
        self.unmask.cancel();
        self.live_message = None;
        self.sync_show(Instant::now());
    }

    /// EW8-OBS-014: a single click on a Live slide applies it. EW8-OBS-016: a
    /// double-click also turns a single mask off; clearing combined masks is
    /// provisional (EW8-OBS-020), as is keeping a mask on a single click.
    pub(crate) fn click_live(&mut self, index: usize, double: bool) {
        // The first click of a double-click already sent this slide.
        let latest = self
            .deferred_send
            .map(|d| d.index)
            .or_else(|| self.requested_slide());
        if double && latest == Some(index) {
            self.unmask.cancel();
        } else {
            self.send(index);
        }
        if double && self.masks.any() {
            let generation = self.request_generation;
            let waiting = self
                .pending_cues
                .back()
                .is_some_and(|p| p.generation == generation && p.index == index)
                || self
                    .deferred_send
                    .is_some_and(|d| d.generation == generation && d.index == index);
            // The mask lifts only on the Applied receipt that carries this
            // exact submission (`sync_show`), whichever stage it is in.
            if waiting || self.presented == Some((generation, index, true)) {
                self.unmask.arm(generation);
            }
        }
        self.sync_show(Instant::now());
    }

    pub(crate) fn logo_ready(&self) -> bool {
        let (Some(output), Some(logo)) = (&self.output, &self.logo) else {
            return false;
        };
        let Status::Connected { extent, .. } = output.status() else {
            return false;
        };
        let version = images::logo_version(logo.resource.version, extent);
        self.logo_sent == Some(version)
            && (output.logo() == Some(version) || output.logo_pending() == Some(version))
    }

    /// The layer to send. A Logo intent covers with Black until the logo for
    /// this surface is with the renderer, so text never shows in between.
    pub(crate) fn wire_layer(&self) -> Layer {
        match self.masks.layer() {
            Layer::Logo if !self.logo_ready() => Layer::Black,
            layer => layer,
        }
    }

    pub(crate) fn indicator(&self, mask: Mask) -> Indicator {
        let output = self.output.as_ref();
        let confirmed = output.and_then(Supervisor::mask);
        let synced = confirmed == Some(self.masks.layer())
            && output.is_some_and(|o| o.mask_pending().is_none());
        let layer = match mask {
            Mask::Black => Layer::Black,
            Mask::Clear => Layer::Clear,
            Mask::Logo => Layer::Logo,
        };
        match (self.masks.is_on(mask), synced) {
            (true, true) => Indicator::On,
            (true, false) => Indicator::Pending,
            (false, false) if confirmed == Some(layer) && self.wire_layer() != layer => {
                Indicator::Pending
            }
            _ => Indicator::Off,
        }
    }

    /// The next item wanting face resolution: the live item first, then the
    /// preview item. Items are resolved whole, one job at a time.
    pub(crate) fn unresolved(&self) -> Option<Projected> {
        let wants = |item: &Item| item.resolved.is_none().then(|| item.projected());
        self.live
            .as_ref()
            .and_then(wants)
            .or(self.preview.as_ref().and_then(wants))
    }

    /// Resolve the unresolved item's formats off the UI thread. The landing
    /// callback re-derives the next want, so a busy job is simply skipped.
    pub(crate) fn resolve_fonts(&mut self, cx: &mut Context<Self>) {
        let Some(projected) = self.unresolved() else {
            return;
        };
        if self.font_job.is_some() {
            return;
        }
        let formats: Vec<_> = self
            .preview
            .iter()
            .chain(self.live.iter())
            .find(|item| item.projected() == projected)
            .map(|item| {
                item.slides
                    .iter()
                    .map(|slide| slide.format.clone())
                    .collect()
            })
            .unwrap_or_default();
        if formats.is_empty() {
            return;
        }
        self.font_job = Some(projected.version);
        let executor = cx.background_executor().clone();
        self.font_task = Some(cx.spawn(async move |this, cx| {
            let resolved = executor
                .spawn(async move {
                    let fonts = fonts::shared();
                    formats
                        .iter()
                        .map(|format| fonts.resolve(format))
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |operator, cx| {
                operator.fonts_resolved(projected, resolved, cx)
            });
        }));
    }

    /// A background resolution landed: attach it to the items it was made
    /// for, retry the deferred cue, then derive the next wanting item.
    /// `pub(super)` for the late-landing transition test in `tests`.
    pub(crate) fn fonts_resolved(
        &mut self,
        projected: impl Into<Projected>,
        resolved: Vec<fonts::Resolved>,
        cx: &mut Context<Self>,
    ) {
        let projected = projected.into();
        self.font_job = None;
        self.font_task = None;
        let resolved: Arc<[fonts::Resolved]> = resolved.into();
        let mut replaced = false;
        for item in [&mut self.live, &mut self.preview].into_iter().flatten() {
            // Cues index the resolution by slide, so it must be parallel.
            if item.projected() == projected && item.slides.len() == resolved.len() {
                replaced = item.resolved.is_some();
                item.resolved = Some(resolved.clone());
            }
        }
        if replaced {
            // Improved resolution (the catalog landed after a bundled
            // fallback): sizes re-fit, so drop the cap and refresh the cue.
            self.size_cap = None;
            if self
                .live
                .as_ref()
                .is_some_and(|live| live.projected() == projected)
            {
                self.refresh_live_request();
            }
        } else {
            self.retry_deferred();
        }
        self.resolve_fonts(cx);
        cx.notify();
    }

    /// The background catalog scan landed: formats that named a family fell
    /// back to bundled before the catalog existed, so resolve them again.
    pub(crate) fn fonts_scanned(&mut self, cx: &mut Context<Self>) {
        let named = |item: &Item| {
            item.slides.iter().any(|slide| {
                slide
                    .format
                    .font
                    .as_deref()
                    .is_some_and(|name| !name.trim().is_empty())
            })
        };
        for item in [&mut self.live, &mut self.preview].into_iter().flatten() {
            if named(item) {
                item.resolved = None;
            }
        }
        self.resolve_fonts(cx);
        cx.notify();
    }

    pub(crate) fn select_slide(&mut self, index: usize, go_live: bool, cx: &mut Context<Self>) {
        if self
            .preview
            .as_ref()
            .is_some_and(|item| index < item.slides.len())
        {
            self.preview_slide = index;
            if go_live {
                self.go_live();
            }
            cx.notify();
        }
    }

    /// Submits the deferred cue if it is still the newest request for the
    /// live item and this output session; anything older is dropped.
    pub(crate) fn retry_deferred(&mut self) {
        let Some(deferred) = self.deferred_send.take() else {
            return;
        };
        let current = self.live.as_ref().map(|live| live.version) == Some(deferred.item)
            && self.output_session == deferred.session;
        if current {
            self.dispatch(deferred.index, deferred.generation);
        }
    }

    fn defer(&mut self, index: usize, generation: u64, item: Version, busy: bool) {
        self.deferred_send = Some(DeferredSend {
            index,
            generation,
            item,
            session: self.output_session,
            busy,
        });
    }

    pub(crate) fn toggle_output(&mut self) {
        self.cancel_cues();
        self.assets.drop_held();
        self.output_session += 1;
        if self.output.take().is_some() {
            self.live_message = Some(t!(Msg::LiveOutputOff).into());
        } else {
            let launcher = self.launcher.clone();
            self.output = Some(Supervisor::start(
                move |epoch| launcher(epoch),
                Instant::now(),
            ));
            self.live_message = None;
        }
        // A new session never replays earlier intent; Go Live must be repeated.
        self.output_extent = None;
        self.live_slide = None;
        // Masks do carry over (EW8-OBS-017); a fresh renderer starts unmasked.
        self.sent_mask = self.output.as_ref().map(|_| Layer::None);
        self.unmask.cancel();
        self.logo_requested = None;
        self.logo_sent = None;
        self.sync_show(Instant::now());
        self.follow_live_output();
    }

    pub(crate) fn go_live(&mut self) {
        // Refused or not, this is the newest explicit request.
        self.unmask.cancel();
        self.assets.drop_held();
        let Some(item) = self.preview.clone() else {
            self.live_message = Some(t!(Msg::LiveSelectSongFirst).into());
            return;
        };
        if item.slides.is_empty() {
            self.live_message = Some(t!(Msg::LivePreviewNoSlides).into());
            return;
        }
        let index = self.preview_slide.min(item.slides.len() - 1);
        if !self.ready_for_cue() {
            return;
        }
        // A slide with an image waits for their check; a missing or damaged
        // file asks before anything goes live, so the audience never gets a
        // black slide the operator did not choose.
        if self.hold_go_live_for_check(&item, index) {
            return;
        }
        self.go_live_item(item, index);
    }

    /// The Go Live cue itself, once any file check has passed or been waived.
    pub(crate) fn go_live_item(&mut self, item: Item, index: usize) {
        if self.ready_for_cue() {
            if self.live.as_ref().map(|live| live.version) != Some(item.version) {
                self.cancel_cues();
            }
            self.live = Some(item);
            self.send(index);
        }
    }

    pub(crate) fn step_live(&mut self, forward: bool) {
        let (Some(count), Some(index)) = (
            self.live.as_ref().map(|item| item.slides.len()),
            self.requested_slide(),
        ) else {
            self.live_message = Some(t!(Msg::LiveNothingLive).into());
            return;
        };
        let target = if forward {
            index + 1
        } else {
            index.wrapping_sub(1)
        };
        // Boundary behavior across schedule items is unobserved; stop here.
        if target < count {
            self.unmask.cancel();
            self.send(target);
        }
    }

    pub(crate) fn ready_for_cue(&mut self) -> bool {
        let message = match self.output.as_ref().map(Supervisor::status) {
            None => t!(Msg::TurnOnLiveFirst),
            Some(Status::Starting) => t!(Msg::LiveStillStarting),
            Some(Status::Lost(_)) => t!(Msg::OutputStateUnknown),
            Some(Status::Connected { .. }) => return true,
        };
        self.live_message = Some(message.into());
        false
    }

    /// Builds the cue on the preparation worker; `poll_cues` presents it. Font
    /// and background lookups are cheap map reads; fitting, sizing and cue
    /// assembly never run here.
    pub(crate) fn send(&mut self, index: usize) {
        // A newer explicit request supersedes any double-click unmask intent,
        // including one already queued with the renderer.
        self.request_generation += 1;
        self.unmask.cancel();
        self.cue_failed = false;
        self.dispatch(index, self.request_generation);
    }

    /// Submits (or defers) the cue for request `generation`; retries of the
    /// same request pass the generation through unchanged.
    pub(crate) fn dispatch(&mut self, index: usize, generation: u64) {
        if !self.ready_for_cue() {
            return;
        }
        let (Some(Status::Connected { extent, caps }), Some(item)) = (
            self.output.as_ref().map(Supervisor::status),
            self.live.clone(),
        ) else {
            return;
        };
        // This request supersedes any cue still waiting on fonts or an image.
        self.deferred_send = None;
        let Some((slide, version)) = item
            .slides
            .get(index)
            .zip(slides::slide_version(item.version, index))
        else {
            return;
        };
        // Installed faces resolve off the UI thread. Until they land, the
        // current live scene stays; the deferred cue is retried by
        // `fonts_resolved` (M1-05g2b).
        let Some(resolved) = item.resolved.clone() else {
            self.defer(index, generation, item.version, false);
            self.live_message = Some(t!(Msg::ResolvingFonts).into());
            return;
        };
        // Image backgrounds are fitted by the image worker; until this one
        // lands the current scene stays and the cue is retried on landing.
        // An image that cannot be prepared is shown as black, with a warning.
        let (background, warning) = match background::plan(slide.background.as_ref()) {
            Plan::Color(rgb) => (slides::color_background(rgb), None),
            Plan::Image(image, aspect) => {
                let fitted = if self.image_worker.is_none() {
                    Some(Err(images::Substitute::Missing))
                } else {
                    self.backgrounds.get(&(image.clone(), aspect, extent))
                };
                match fitted {
                    Some(Ok(rgba)) => (
                        PreparedBackground::Image {
                            version: images::background_version(image, aspect),
                            extent,
                            rgba,
                        },
                        None,
                    ),
                    Some(Err(why)) => (
                        slides::color_background(background::BLACK),
                        Some(why.warning(&image.name)),
                    ),
                    None => {
                        self.defer(index, generation, item.version, false);
                        self.live_message = Some(t!(Msg::PreparingBackground).into());
                        return;
                    }
                }
            }
        };
        let label = format!("{} · {}", item.title, slide_name(index, slide));
        let job = CueJob {
            item: item.version,
            index,
            version,
            slides: item.slides.clone(),
            resolved,
            extent,
            caps,
            sizing: self.sizing,
            background,
            footer: item.copyright.clone(),
        };
        let sizing = job.sizing;
        if self.cue_preparer.is_none() {
            self.cue_preparer = CuePreparer::new().ok();
        }
        let submitted = match &self.cue_preparer {
            Some(preparer) => preparer.submit(Lane::Live, job),
            None => Err(PrepareError::Disconnected),
        };
        match submitted {
            Ok(ticket) => self.pending_cues.push_back(PendingCue {
                ticket,
                index,
                generation,
                item: item.version,
                sizing,
                extent,
                label,
                warning,
            }),
            Err(PrepareError::Busy) => {
                // Kept in order: `poll_cues` resubmits it once the lane drains.
                self.defer(index, generation, item.version, true);
                self.live_message = Some(t!(Msg::CueBusy).into());
            }
            Err(error) => {
                self.live_message = Some(t!(Msg::CueFailed, error = error.to_string()));
            }
        }
    }

    /// Presents cues that finished preparing. A failed cue leaves the current
    /// scene alone and only reports; results from cancelled requests never
    /// surface because the preparer fences them.
    pub(crate) fn poll_cues(&mut self) -> bool {
        let mut changed = false;
        for _ in 0..=LIVE_DEPTH {
            let Some(event) = self.cue_preparer.as_ref().and_then(CuePreparer::poll) else {
                break;
            };
            changed = true;
            let Some(at) = self
                .pending_cues
                .iter()
                .position(|p| p.ticket == event.ticket())
            else {
                continue;
            };
            let Some(pending) = self.pending_cues.remove(at) else {
                continue;
            };
            match event {
                CueEvent::Ready { cue, size_cap, .. } => {
                    self.size_cap = Some((pending.item, pending.extent, pending.sizing, size_cap));
                    let Some(output) = self.output.as_mut() else {
                        self.unmask.drop_submission(pending.generation);
                        continue;
                    };
                    match output.present_submission(cue, pending.generation, Instant::now()) {
                        Ok(()) => {
                            let version = slides::slide_version(pending.item, pending.index);
                            if let Some(version) = version {
                                if self.labels.len() == LABELS {
                                    self.labels.pop_front();
                                }
                                self.labels.retain(|(v, _)| *v != version);
                                self.labels.push_back((version, pending.label));
                            }
                            self.live_slide = Some(pending.index);
                            let clean = pending.warning.is_none();
                            self.presented = Some((pending.generation, pending.index, clean));
                            if !clean {
                                // A substituted background keeps the mask on.
                                self.unmask.drop_submission(pending.generation);
                            }
                            self.live_message = pending.warning;
                        }
                        Err(refusal) => {
                            self.unmask.drop_submission(pending.generation);
                            self.live_message = Some(match refusal {
                                Refusal::NotConnected => t!(Msg::RefusalNotConnected).into(),
                                Refusal::WrongExtent => t!(Msg::RefusalWrongExtent).into(),
                            });
                        }
                    }
                }
                CueEvent::Failed { error, .. } => {
                    self.cue_failed = true;
                    self.unmask.drop_submission(pending.generation);
                    self.live_message = Some(t!(Msg::CueFailed, error = error.to_string()));
                }
            }
        }
        if self.deferred_send.is_some_and(|d| d.busy) && self.pending_cues.len() < LIVE_DEPTH {
            self.retry_deferred();
            changed = true;
        }
        changed
    }

    /// Everything submitted for Live so far is stale (new item, session or
    /// surface); its results are dropped even if they finish later.
    pub(crate) fn cancel_cues(&mut self) {
        self.pending_cues.clear();
        self.deferred_send = None;
        self.assets.drop_held();
        if let Some(preparer) = &self.cue_preparer {
            preparer.cancel(Lane::Live);
        }
    }

    /// The newest live request and the generation that owns it: a deferred
    /// send first (it is newer than anything in preparation), then the newest
    /// cue in preparation, then the slide on screen.
    fn latest_request(&self) -> Option<(usize, u64)> {
        if let Some(deferred) = self.deferred_send {
            return Some((deferred.index, deferred.generation));
        }
        if let Some(pending) = self.pending_cues.back() {
            return Some((pending.index, pending.generation));
        }
        self.live_slide
            .map(|index| (index, self.request_generation))
    }

    /// Same session, new parameters (surface or faces): rebuild the newest
    /// request. It keeps its own slide AND its own generation, so a
    /// double-click unmask intent can only ever match that exact request; if
    /// the intent's request is gone, the intent dies here.
    pub(crate) fn refresh_live_request(&mut self) {
        let Some((index, generation)) = self.latest_request() else {
            return;
        };
        self.cancel_cues();
        self.dispatch(index, generation);
        self.unmask.retain_only(generation);
        // Without a cue carrying the id, the `live_slide` fallback could later
        // present another slide under it.
        let carried = self
            .deferred_send
            .is_some_and(|d| d.generation == generation)
            || self.pending_cues.iter().any(|p| p.generation == generation);
        if !carried {
            self.unmask.drop_submission(generation);
        }
    }

    /// The slide the operator last asked for: the newest cue in preparation,
    /// else the one on screen.
    pub(crate) fn requested_slide(&self) -> Option<usize> {
        self.deferred_send
            .map(|d| d.index)
            .or_else(|| self.pending_cues.back().map(|p| p.index))
            .or(self.live_slide)
    }
    /// What the renderer has acknowledged, by label, if anything.
    pub(crate) fn on_screen(&self) -> Option<&str> {
        let LiveState::Confirmed(version) = self.output.as_ref()?.live() else {
            return None;
        };
        self.labels
            .iter()
            .find(|(v, _)| *v == version)
            .map(|(_, label)| label.as_str())
    }

    pub(crate) fn live_index(&self, version: Option<ContentVersion>) -> Option<usize> {
        slides::slide_index(self.live.as_ref()?.version, version?)
    }

    pub(crate) fn output_line(&self) -> String {
        match self.output.as_ref().map(Supervisor::status) {
            None => t!(Msg::OutputOff).into(),
            Some(Status::Starting) => t!(Msg::OutputStarting).into(),
            Some(Status::Connected { extent, .. }) => t!(
                Msg::OutputConnected,
                width = extent.width,
                height = extent.height
            ),
            Some(Status::Lost(loss)) => t!(Msg::OutputLost, loss = loss),
        }
    }

    pub(crate) fn preview_pane(&mut self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.preview.as_ref().map_or(0, |item| item.slides.len());
        while self.slide_rows.len() < count {
            let index = self.slide_rows.len() as isize;
            self.slide_rows.push(
                cx.focus_handle()
                    .tab_index(SLIDE_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        let footer = match &self.preview {
            Some(item) if count > 0 => t!(
                Msg::PreviewFooter,
                title = item.title,
                index = self.preview_slide + 1,
                count = count
            ),
            Some(item) => t!(Msg::PreviewNoSlides, title = item.title),
            None => t!(Msg::NothingSelected).into(),
        };
        let tiles = self.preview.as_ref().map(|item| {
            item.slides
                .iter()
                .enumerate()
                .map(|(index, slide)| {
                    let selected = index == self.preview_slide;
                    let page = item.pages.get(index).copied().flatten();
                    tile(
                        slide,
                        index,
                        page,
                        if selected { ACCENT } else { CANVAS },
                        self.thumbnail(slide),
                    )
                    .id(("preview-slide", index))
                    .debug_selector(move || format!("preview-slide-{index}"))
                    .role(Role::ListItem)
                    .aria_label(slide_caption(index, slide, page))
                    .aria_selected(selected)
                    .accessibility_id(format!("preview-slide-{index}"))
                    .track_focus(&self.slide_rows[index])
                    .key_context("SelaControl SelaPreviewSlide")
                    .focus(|d| d.bg(rgb(CANVAS_FOCUS)))
                    .cursor_pointer()
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        window.prevent_default();
                        this.select_slide(index, false, cx);
                    }))
                    .on_click(cx.listener(
                        move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                return;
                            }
                            this.slide_rows[index].focus(window, cx);
                            // Documented (EasyWorship 7 guide): double-click sends to output.
                            this.select_slide(index, event.click_count() >= 2, cx);
                        },
                    ))
                })
                .collect::<Vec<_>>()
        });
        let mut notes = Vec::new();
        if let Some(page) = self
            .preview
            .as_ref()
            .and_then(|item| item.pages.get(self.preview_slide).copied().flatten())
        {
            notes.push((
                t!(Msg::SlideSplitNote, number = page.number, count = page.of),
                DARK_MUTED,
            ));
        }
        notes.extend(self.preview_warning().map(|w| (w, DARK_ERROR)));
        let warning = (!notes.is_empty()).then_some(notes);
        let toolbar = self.arrangement_button(cx);
        slide_pane(
            "Preview",
            t!(Msg::PanePreview).to_string(),
            width,
            footer,
            warning,
            toolbar,
            tiles,
            None,
        )
    }

    /// Live pane status, top to bottom. Mask lines reflect acknowledged state.
    pub(crate) fn live_lines(&self) -> Vec<(String, u32)> {
        let output = self.output.as_ref();
        let connected = matches!(
            output.map(Supervisor::status),
            Some(Status::Connected { .. })
        );
        let sending = self.live_index(output.and_then(|o| o.wanted().or(o.in_flight())));
        let mut lines = vec![(self.output_line(), DARK_MUTED)];
        let slide = match self.on_screen() {
            Some(label) => label.to_owned(),
            None if connected => t!(Msg::NothingConfirmed).into(),
            None => t!(Msg::UnknownState).into(),
        };
        match output.and_then(Supervisor::mask) {
            Some(layer) if layer != Layer::None => {
                lines.push((t!(Msg::MaskLine, layer = layer_name(layer)), DARK_TEXT));
                lines.push((t!(Msg::UnderMask, slide = slide), DARK_MUTED));
            }
            _ => {
                let color = if self.on_screen().is_some() {
                    DARK_TEXT
                } else {
                    DARK_MUTED
                };
                lines.push((t!(Msg::OnScreen, slide = slide), color));
            }
        }
        if self
            .live
            .as_ref()
            .and_then(|item| item.entry)
            .is_some_and(|id| self.schedule.get(id).is_none())
        {
            lines.push((t!(Msg::LiveItemGone).into(), DARK_MUTED));
        }
        if let Some(layer) = output.and_then(Supervisor::mask_pending) {
            lines.push((t!(Msg::SendingMask, layer = layer_name(layer)), SENDING));
        }
        if self.masks.any() && output.is_none() {
            lines.push((
                t!(Msg::MaskArmed, layer = layer_name(self.masks.layer())),
                DARK_MUTED,
            ));
        }
        if self.masks.is_on(Mask::Logo) {
            if let Some(error) = &self.logo_error {
                lines.push((error.clone(), DARK_ERROR));
            } else if connected && !self.logo_ready() {
                lines.push((t!(Msg::PreparingLogo).into(), SENDING));
            }
        }
        if !self.pending_cues.is_empty() || self.deferred_send.is_some() {
            lines.push((t!(Msg::IndicatorPreparing).into(), DARK_PREPARING));
        }
        if sending.is_some() {
            lines.push((t!(Msg::Sending).into(), SENDING));
        }
        if self.cue_failed || output.is_some_and(|o| o.rejected().is_some()) {
            lines.push((t!(Msg::IndicatorFailed).into(), DARK_ERROR));
        }
        if matches!(output.map(Supervisor::status), Some(Status::Lost(_))) {
            lines.insert(
                1,
                (t!(Msg::IndicatorDisconnected).into(), DARK_DISCONNECTED),
            );
        }
        if let Some((_, error)) = output.and_then(|o| o.rejected()) {
            lines.push((t!(Msg::CueNotShown, error = error), DARK_ERROR));
        }
        if let Some((layer, error)) = output.and_then(|o| o.mask_rejected()) {
            lines.push((
                t!(Msg::MaskNotShown, layer = layer_name(layer), error = error),
                DARK_ERROR,
            ));
        }
        if let Some((_, error)) = output.and_then(|o| o.logo_rejected()) {
            lines.push((t!(Msg::LogoNotShown, error = error), DARK_ERROR));
        }
        lines.extend(self.stage_lines());
        if let Some(message) = &self.live_message {
            lines.push((message.clone(), DARK_ERROR));
        }
        lines
    }

    pub(crate) fn live_pane(&mut self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let output = self.output.as_ref();
        let confirmed = output.and_then(|o| match o.live() {
            LiveState::Confirmed(version) => Some(version),
            LiveState::Unknown => None,
        });
        let confirmed = self.live_index(confirmed);
        let sending = self.live_index(output.and_then(|o| o.wanted().or(o.in_flight())));
        let lines = self.live_lines();
        let footer = match (&self.live, self.live_slide) {
            (Some(item), Some(index)) => t!(
                Msg::PreviewFooter,
                title = item.title,
                index = index + 1,
                count = item.slides.len()
            ),
            (Some(item), None) => t!(Msg::LiveNotSent, title = item.title),
            _ => t!(Msg::NothingLive).into(),
        };
        let tiles = self.live.as_ref().map(|item| {
            item.slides
                .iter()
                .enumerate()
                .map(|(index, slide)| {
                    let border = if Some(index) == confirmed {
                        ON_SCREEN
                    } else if Some(index) == sending {
                        SENDING
                    } else {
                        CANVAS
                    };
                    let page = item.pages.get(index).copied().flatten();
                    tile(slide, index, page, border, self.thumbnail(slide))
                        .id(("live-slide", index))
                        .debug_selector(move || format!("live-slide-{index}"))
                        .role(Role::ListItem)
                        .aria_label(slide_caption(index, slide, page))
                        .aria_selected(Some(index) == confirmed)
                        .accessibility_id(format!("live-slide-{index}"))
                        .cursor_pointer()
                        .on_click(
                            cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                                if matches!(event, gpui::ClickEvent::Keyboard(_))
                                    || event.is_right_click()
                                {
                                    return;
                                }
                                this.live_focus.focus(window, cx);
                                this.click_live(index, event.click_count() >= 2);
                                cx.notify();
                            }),
                        )
                })
                .collect::<Vec<_>>()
        });
        slide_pane(
            "Live",
            t!(Msg::PaneLive).to_string(),
            width,
            footer,
            Some(lines),
            None,
            tiles,
            Some(self.live_focus.clone()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};

    struct Host(String);

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let slide = Slide {
                label: "Verse".into(),
                text: self.0.clone(),
                format: Default::default(),
                background: None,
            };
            div().p_4().child(tile(&slide, 0, None, CANVAS, None))
        }
    }

    fn lines(count: usize) -> String {
        (1..=count)
            .map(|n| format!("Line {n} of the page"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_tile(
        cx: &mut TestAppContext,
        text: String,
    ) -> (gpui::Bounds<Pixels>, gpui::Bounds<Pixels>) {
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| cx.new(|_| Host(text)))
                .unwrap()
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        (
            visual.debug_bounds("tile-box-0").expect("box"),
            visual.debug_bounds("tile-text-0").expect("text"),
        )
    }

    #[test]
    fn tile_text_size_shrinks_with_lines_and_width() {
        assert_eq!(tile_text_size("one"), TEXT_TILE);
        assert!(tile_text_size(&lines(14)) < tile_text_size(&lines(4)));
        assert!(tile_text_size(&"w".repeat(80)) < TEXT_TILE);
        assert!(tile_text_size(&lines(14)) * TILE_LINE_HEIGHT * 14. <= TILE_TEXT_H + 0.01);
    }

    #[gpui::test]
    fn tall_paginated_page_fits_the_whole_tile(cx: &mut TestAppContext) {
        let (frame, text) = render_tile(cx, lines(14));
        assert!(
            text.top() >= frame.top() && text.bottom() <= frame.bottom(),
            "all 14 lines inside the tile: text {text:?} box {frame:?}"
        );
        assert!(text.left() >= frame.left() && text.right() <= frame.right());
    }

    #[gpui::test]
    fn short_text_keeps_the_regular_size(cx: &mut TestAppContext) {
        let (frame, text) = render_tile(cx, "Glory\nto the King".into());
        assert!(text.bottom() <= frame.bottom() && text.size.height < frame.size.height / 2.);
    }
}
