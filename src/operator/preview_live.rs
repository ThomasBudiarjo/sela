use super::*;
use sela::preparation::cue::LIVE_DEPTH;

/// Text-only slide thumbnail over the slide's fill color (image fills show
/// black); not a rendered audience frame.
pub(crate) fn tile(slide: &Slide, index: usize, border: u32) -> gpui::Div {
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
                .px_1()
                .text_size(px(TEXT_TILE))
                .text_color(rgb(DARK_TEXT))
                .children(
                    slide
                        .text
                        .lines()
                        .take(6)
                        .map(|line| div().text_center().child(line.to_owned())),
                ),
        )
        .child(
            div()
                .text_size(px(TEXT_CAPTION))
                .text_color(rgb(CANVAS_TEXT))
                .truncate()
                .child(format!("{} · {}", index + 1, slide_name(index, slide))),
        )
}

pub(crate) fn slide_pane(
    id: &'static str,
    title: String,
    width: f32,
    footer: String,
    status: Option<Vec<(String, u32)>>,
    tiles: Option<Vec<gpui::Stateful<gpui::Div>>>,
) -> impl IntoElement {
    let empty = tiles.as_ref().is_none_or(Vec::is_empty);
    let list_name = title.clone();
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
        .children(status.map(|lines| {
            div()
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
        let Some((version, _)) = self.catalog.get(index).cloned() else {
            return;
        };
        self.selected_song = Some(version);
        self.selected_entry = None;
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

    /// Hands the most urgent missing background to the image worker. Wanted
    /// entries are refreshed least urgent first, so eviction spares them.
    pub(crate) fn prefetch_backgrounds(&mut self) {
        if self.background_job.is_some() || self.image_worker.is_none() {
            return;
        }
        let mut missing = None;
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
    pub(crate) fn unresolved(&self) -> Option<Version> {
        let wants = |item: &Item| item.resolved.is_none().then_some(item.version);
        self.live
            .as_ref()
            .and_then(wants)
            .or(self.preview.as_ref().and_then(wants))
    }

    /// Resolve the unresolved item's formats off the UI thread. The landing
    /// callback re-derives the next want, so a busy job is simply skipped.
    pub(crate) fn resolve_fonts(&mut self, cx: &mut Context<Self>) {
        let Some(version) = self.unresolved() else {
            return;
        };
        if self.font_job.is_some() {
            return;
        }
        let formats: Vec<_> = self
            .preview
            .iter()
            .chain(self.live.iter())
            .find(|item| item.version == version)
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
        self.font_job = Some(version);
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
                operator.fonts_resolved(version, resolved, cx)
            });
        }));
    }

    /// A background resolution landed: attach by version, retry the deferred
    /// cue, then derive the next wanting item. `pub(super)` for the
    /// late-landing transition test in `tests`.
    pub(crate) fn fonts_resolved(
        &mut self,
        version: Version,
        resolved: Vec<fonts::Resolved>,
        cx: &mut Context<Self>,
    ) {
        self.font_job = None;
        self.font_task = None;
        let resolved: Arc<[fonts::Resolved]> = resolved.into();
        let mut replaced = false;
        for item in [&mut self.live, &mut self.preview].into_iter().flatten() {
            if item.version == version {
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
                .is_some_and(|live| live.version == version)
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

    fn defer(&mut self, index: usize, generation: u64, item: Version) {
        self.deferred_send = Some(DeferredSend {
            index,
            generation,
            item,
            session: self.output_session,
        });
    }

    pub(crate) fn toggle_output(&mut self) {
        self.cancel_cues();
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
    }

    pub(crate) fn go_live(&mut self) {
        let Some(item) = self.preview.clone() else {
            self.live_message = Some(t!(Msg::LiveSelectSongFirst).into());
            return;
        };
        if item.slides.is_empty() {
            self.live_message = Some(t!(Msg::LivePreviewNoSlides).into());
            return;
        }
        let index = self.preview_slide.min(item.slides.len() - 1);
        if self.ready_for_cue() {
            if self.live.as_ref().map(|live| live.version) != Some(item.version) {
                self.cancel_cues();
                self.unmask.cancel();
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
            self.defer(index, generation, item.version);
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
                        self.defer(index, generation, item.version);
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
            Err(PrepareError::Busy) => self.live_message = Some(t!(Msg::CueBusy).into()),
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
                    self.unmask.drop_submission(pending.generation);
                    self.live_message = Some(t!(Msg::CueFailed, error = error.to_string()));
                }
            }
        }
        changed
    }

    /// Everything submitted for Live so far is stale (new item, session or
    /// surface); its results are dropped even if they finish later.
    pub(crate) fn cancel_cues(&mut self) {
        self.pending_cues.clear();
        self.deferred_send = None;
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
    }

    /// The slide the operator last asked for: the newest cue in preparation,
    /// else the one on screen.
    pub(crate) fn requested_slide(&self) -> Option<usize> {
        self.pending_cues
            .back()
            .map(|p| p.index)
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
                    tile(slide, index, if selected { ACCENT } else { CANVAS })
                        .id(("preview-slide", index))
                        .debug_selector(move || format!("preview-slide-{index}"))
                        .role(Role::ListItem)
                        .aria_label(format!("{} · {}", index + 1, slide_name(index, slide)))
                        .aria_selected(selected)
                        .accessibility_id(format!("preview-slide-{index}"))
                        .track_focus(&self.slide_rows[index])
                        .key_context("SelaControl")
                        .focus(|d| d.bg(rgb(CANVAS_FOCUS)))
                        .cursor_pointer()
                        .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                            window.prevent_default();
                            this.select_slide(index, false, cx);
                        }))
                        .on_click(
                            cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                                if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                    return;
                                }
                                this.slide_rows[index].focus(window, cx);
                                // Documented (EasyWorship 7 guide): double-click sends to output.
                                this.select_slide(index, event.click_count() >= 2, cx);
                            }),
                        )
                })
                .collect::<Vec<_>>()
        });
        let warning = self.preview_warning().map(|w| vec![(w, DARK_ERROR)]);
        slide_pane(
            "Preview",
            t!(Msg::PanePreview).to_string(),
            width,
            footer,
            warning,
            tiles,
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
        if sending.is_some() {
            lines.push((t!(Msg::Sending).into(), SENDING));
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
                    tile(slide, index, border)
                        .id(("live-slide", index))
                        .debug_selector(move || format!("live-slide-{index}"))
                        .role(Role::ListItem)
                        .aria_label(format!("{} · {}", index + 1, slide_name(index, slide)))
                        .aria_selected(Some(index) == confirmed)
                        .accessibility_id(format!("live-slide-{index}"))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_))
                                || event.is_right_click()
                            {
                                return;
                            }
                            this.click_live(index, event.click_count() >= 2);
                            cx.notify();
                        }))
                })
                .collect::<Vec<_>>()
        });
        slide_pane(
            "Live",
            t!(Msg::PaneLive).to_string(),
            width,
            footer,
            Some(lines),
            tiles,
        )
    }
}
