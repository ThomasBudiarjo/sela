use super::*;

impl Operator {
    pub(crate) fn poll(&mut self, cx: &mut Context<Self>) {
        let mut changed = self.poll_library(cx) | self.poll_images() | self.poll_settings();
        changed |= self.poll_cues();
        if let Some(output) = &mut self.output {
            changed |= output.poll(Instant::now());
            if let Status::Connected { extent, .. } = output.status()
                && self.output_extent != Some(extent)
            {
                self.output_extent = Some(extent);
                // Same session, new surface: re-render the current intent.
                if let Some(index) = self.requested_slide() {
                    self.cancel_cues();
                    self.dispatch(index, self.request_generation);
                }
                changed = true;
            }
        }
        self.prefetch_backgrounds();
        changed |= self.sync_show(Instant::now());
        if changed {
            cx.notify();
        }
    }

    pub(crate) fn poll_images(&mut self) -> bool {
        let mut changed = false;
        let mut rescan = false;
        let mut retry = false;
        // Two replies are buffered at most; bound the loop anyway.
        for _ in 0..4 {
            let Some(reply) = self.image_worker.as_ref().and_then(images::Worker::poll) else {
                break;
            };
            changed = true;
            match reply {
                Err(error) => {
                    self.media_message = Some(error.to_string());
                    self.image_worker = None;
                    // Waiting cues now substitute black instead.
                    self.background_job = None;
                    retry = true;
                    break;
                }
                Ok(images::Reply::Background(key, fitted)) => {
                    if self.background_job.as_ref() == Some(&key) {
                        self.background_job = None;
                    }
                    self.backgrounds.insert(key, fitted);
                    retry = true;
                }
                Ok(images::Reply::Images(Ok(names))) => {
                    // The folder changed: a missing image may be back.
                    self.backgrounds.forget_substitutes();
                    if self
                        .selected_image
                        .as_ref()
                        .is_some_and(|s| !names.contains(s))
                    {
                        self.selected_image = None;
                    }
                    self.images = names;
                }
                Ok(images::Reply::Images(Err(error))) => {
                    self.media_message =
                        Some(t!(Msg::ImagesUnavailable, error = error.to_string()));
                }
                Ok(images::Reply::Imported(Ok(name))) => {
                    self.media_message = Some(t!(Msg::ImportedImage, name = name));
                    self.selected_image = Some(name);
                    rescan = true;
                }
                Ok(images::Reply::Imported(Err(error))) => {
                    self.media_message = Some(t!(Msg::ImportFailed, error = error.to_string()));
                }
                Ok(images::Reply::Logo(Ok(logo))) => {
                    self.logo = logo;
                    self.logo_error = None;
                    self.logo_requested = None;
                    self.logo_sent = None;
                }
                // The previous logo, if any, stays in use.
                Ok(images::Reply::Logo(Err(error))) => {
                    self.logo_error = Some(t!(Msg::LogoUnavailable, error = error.to_string()));
                }
            }
        }
        if rescan {
            self.submit_image_job(images::Job::Scan);
        }
        if retry {
            self.retry_deferred();
        }
        changed
    }

    /// Prepares the logo for the current surface off the UI thread and hands
    /// it to the supervisor. Returns true when visible state changed.
    pub(crate) fn sync_logo(&mut self, now: Instant) -> bool {
        let (Some(output), Some(logo)) = (&mut self.output, &self.logo) else {
            return false;
        };
        let Status::Connected { extent, caps } = output.status() else {
            return false;
        };
        let version = images::logo_version(logo.resource.version, extent);
        let mut changed = false;
        if self.logo_requested != Some(version) {
            if self.logo_preparer.as_ref().is_none_or(|(c, _)| *c != caps) {
                match Preparer::new(caps) {
                    Ok(preparer) => self.logo_preparer = Some((caps, preparer)),
                    Err(_) => {
                        self.logo_requested = Some(version);
                        self.logo_error = Some(t!(Msg::LogoPrepFailed).into());
                        return true;
                    }
                }
            }
            let (_, preparer) = self.logo_preparer.as_mut().expect("created above");
            match preparer.request(images::logo_spec(logo, extent), now, LOGO_TIMEOUT) {
                Ok(_) => self.logo_requested = Some(version),
                // Retried on the next poll.
                Err(PrepareError::Busy) => {}
                Err(error) => {
                    self.logo_requested = Some(version);
                    self.logo_error = Some(t!(Msg::LogoUnavailable, error = error.to_string()));
                    changed = true;
                }
            }
        }
        if let Some((_, preparer)) = &mut self.logo_preparer
            && let Some(event) = preparer.poll(now)
        {
            changed = true;
            match event {
                PreparationEvent::Ready { cue, .. } if cue.version() == version => {
                    // A refusal means the surface changed; the next poll re-requests.
                    if output.set_logo(cue, now).is_ok() {
                        self.logo_sent = Some(version);
                        self.logo_error = None;
                    }
                }
                PreparationEvent::Ready { .. } => {}
                PreparationEvent::Failed { error, .. } => {
                    self.logo_error = Some(t!(Msg::LogoUnavailable, error = error.to_string()));
                }
            }
        }
        changed
    }

    pub(crate) fn sync_show(&mut self, now: Instant) -> bool {
        let changed = self.sync_logo(now);
        let layer = self.wire_layer();
        let Some(output) = &mut self.output else {
            return changed;
        };
        if self.unmask_after_cue {
            if output.wanted().is_some() || output.in_flight().is_some() {
                return changed;
            }
            self.unmask_after_cue = false;
        }
        if self.sent_mask != Some(layer) && output.set_mask(layer, now).is_ok() {
            self.sent_mask = Some(layer);
            return true;
        }
        changed
    }

    pub(crate) fn poll_library(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(worker) = &mut self.worker else {
            return false;
        };
        let mut changed = false;
        let mut live_now = false;
        let mut resolve_now = false;
        if let Some(reply) = worker.poll() {
            changed = true;
            match (self.request.take(), reply) {
                (Some(Request::Open), Ok(Reply::Opened)) => self.catalog_stale = true,
                (Some(Request::Catalog), Ok(Reply::Catalog(catalog))) => {
                    // A library selection follows its song to a revision saved
                    // since, so Preview shows the edit; Live keeps its item.
                    if self.selected_entry.is_none()
                        && let Some(selected) = self.selected_song
                        && let Some((head, _)) = catalog
                            .iter()
                            .find(|(v, _)| v.id == selected.id && *v != selected)
                    {
                        self.selected_song = Some(*head);
                    }
                    self.catalog = catalog;
                    self.library_error = None;
                }
                (Some(Request::DeleteSong(version, title)), Ok(Reply::Deleted)) => {
                    self.catalog.retain(|(v, _)| *v != version);
                    // Preview keeps the slides it already prepared (provisional).
                    if self.selected_song == Some(version) && self.selected_entry.is_none() {
                        self.selected_song = None;
                    }
                    self.song_message =
                        Some((t!(Msg::DeletedSong, title = shown_title(&title)), false));
                    self.catalog_stale = true;
                }
                (Some(Request::DeleteSong(_, title)), Err(error)) => {
                    let title = shown_title(&title);
                    self.song_message = Some((
                        match error {
                            storage::Error::Conflict => t!(Msg::DeleteConflict, title = title),
                            error => t!(
                                Msg::DeleteStorageFailed,
                                title = title,
                                error = format!("{error:?}")
                            ),
                        },
                        true,
                    ));
                    self.catalog_stale = true;
                }
                (Some(Request::Song(version)), Ok(Reply::Song(song)))
                    if self.selected_song == Some(version) =>
                {
                    self.preview = Some(Item {
                        version,
                        entry: self.selected_entry,
                        title: song.title.clone(),
                        slides: slides::slides(&song).into(),
                        resolved: None,
                    });
                    self.preview_slide = 0;
                    if self.live_on_load == Some(version) && self.selected_entry.is_none() {
                        self.live_on_load = None;
                        live_now = true;
                    }
                    // The item's faces resolve off the UI thread before any
                    // of its slides can go live (M1-05g2b).
                    resolve_now = true;
                }
                (Some(Request::SaveSchedule(snapshot)), Ok(Reply::Saved(version))) => {
                    self.schedule_message = Some(t!(Msg::SavedSchedule, title = snapshot.title));
                    self.schedule.mark_saved(version, snapshot);
                }
                (Some(Request::ScheduleCatalog), Ok(Reply::ScheduleCatalog(mut list))) => {
                    list.sort_by_key(|a| a.1.to_lowercase());
                    self.saved_schedules = Some(list);
                }
                (Some(Request::OpenSchedule(version)), Ok(Reply::Schedule(stored, songs))) => {
                    let title = stored.title.clone();
                    let titles = songs.into_iter().map(|song| song.title).collect();
                    self.schedule_message =
                        Some(match self.schedule.open(version, stored, titles) {
                            Ok(()) => {
                                self.selected_entry = None;
                                t!(Msg::OpenedSchedule, title = title)
                            }
                            Err(_) => t!(Msg::ScheduleUnreadable).into(),
                        });
                }
                (Some(Request::SaveSchedule(_)), Err(storage::Error::Conflict)) => {
                    self.schedule_message = Some(t!(Msg::ScheduleConflict).into());
                }
                (
                    Some(
                        Request::SaveSchedule(_)
                        | Request::ScheduleCatalog
                        | Request::OpenSchedule(_),
                    ),
                    Err(error),
                ) => {
                    self.schedule_message =
                        Some(t!(Msg::ScheduleStorageFailed, error = format!("{error:?}")));
                    if self.dialog == Some(Dialog::Open) {
                        self.saved_schedules = Some(Vec::new());
                    }
                }
                (_, Err(error)) => self.library_error = Some(storage_message(error)),
                _ => {}
            }
        }
        if self.request.is_none() {
            let wanted = self.selected_song.filter(|v| {
                self.preview
                    .as_ref()
                    .is_none_or(|p| (p.version, p.entry) != (*v, self.selected_entry))
            });
            let request = if let Some(job) = self.queued.take() {
                Some(job)
            } else if let Some(version) = wanted {
                Some((storage::Command::Song(version), Request::Song(version)))
            } else if self.catalog_stale {
                self.catalog_stale = false;
                Some((storage::Command::Catalog(None), Request::Catalog))
            } else {
                None
            };
            if let Some((command, request)) = request {
                let schedule = matches!(
                    request,
                    Request::SaveSchedule(_) | Request::ScheduleCatalog | Request::OpenSchedule(_)
                );
                match worker.submit(command) {
                    Ok(_) => self.request = Some(request),
                    Err(error) if matches!(request, Request::DeleteSong(..)) => {
                        self.song_message =
                            Some((t!(Msg::DeleteQueueFull, error = format!("{error:?}")), true));
                        changed = true;
                    }
                    Err(error) if schedule => {
                        self.schedule_message =
                            Some(t!(Msg::ScheduleStorageFailed, error = format!("{error:?}")));
                        changed = true;
                    }
                    Err(error) => {
                        self.library_error = Some(storage_message(error));
                        changed = true;
                    }
                }
            }
        }
        if live_now {
            self.go_live();
        }
        if resolve_now {
            self.resolve_fonts(cx);
        }
        changed
    }

    /// A Songs reload is wanted or in flight.
    #[cfg(test)]
    pub(crate) fn catalog_refresh_pending(&self) -> bool {
        self.catalog_stale || matches!(self.request, Some(Request::Catalog))
    }
}
