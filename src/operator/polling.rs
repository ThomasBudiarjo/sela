use super::*;

impl Operator {
    pub(crate) fn poll(&mut self, cx: &mut Context<Self>) {
        let mut changed = self.poll_library(cx)
            | self.poll_images()
            | self.poll_preflight()
            | self.poll_settings()
            | self.poll_schedule_files()
            | self.poll_song_import();
        changed |= self.poll_cues();
        if let Some(output) = &mut self.output {
            changed |= output.poll(Instant::now());
            if let Status::Connected { extent, .. } = output.status()
                && self.output_extent != Some(extent)
            {
                self.output_extent = Some(extent);
                // Same session, new surface: rebuild the newest request.
                self.refresh_live_request();
                changed = true;
            }
        }
        self.prefetch_backgrounds();
        changed |= self.sync_show(Instant::now());
        if changed {
            cx.notify();
        }
    }

    /// Test hook: pretend the settled surface extent changed, so the next
    /// `poll` takes the surface-refresh path.
    #[cfg(test)]
    pub(crate) fn resurface(&mut self) {
        self.output_extent = None;
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
                    self.assets.importing = None;
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
                    if key.2 == images::TILE {
                        self.store_thumbnail(key, fitted);
                    } else {
                        self.backgrounds.insert(key, fitted);
                        retry = true;
                    }
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
                Ok(images::Reply::Imported(Ok(imported))) => {
                    self.assets.importing = None;
                    self.assets.import_notes = assets::import_notes(&imported);
                    self.media_message = Some(t!(Msg::ImportedImage, name = imported.name));
                    self.selected_image = Some(imported.name);
                    rescan = true;
                }
                Ok(images::Reply::Imported(Err(error))) => {
                    self.assets.importing = None;
                    self.assets.import_notes.clear();
                    self.media_message = Some(if error == images::Error::Cancelled {
                        error.to_string()
                    } else {
                        t!(Msg::ImportFailed, error = error.to_string())
                    });
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
        // The double-clicked request lifts its mask only on the successful
        // Applied receipt carrying its own submission id. A rejection keeps
        // the mask over the retained scene and the intent dies; an older
        // confirmation of the same slide never counts.
        if let Some(submission) = self.unmask.armed() {
            let Some(output) = &self.output else {
                return changed;
            };
            if self.unmask.settle(output.submission(submission)) {
                self.masks.live_double_click();
            }
        }
        let layer = self.wire_layer();
        let Some(output) = &mut self.output else {
            return changed;
        };
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
        let mut opened_songs = None;
        let mut to_write = None;
        let mut to_open = None;
        let mut scanned = None;
        let mut rescan = false;
        let mut deleted_at = None;
        let mut import_reply = None;
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
                    rescan = std::mem::take(&mut self.view.scan_wanted);
                }
                (Some(Request::Search), Ok(Reply::Search(results))) => {
                    // A stale generation changes nothing and is not shown.
                    changed = self.search.apply(
                        results,
                        self.selected_entry.is_some(),
                        &mut self.selected_song,
                    );
                }
                (Some(Request::Search), Err(storage::Error::Canceled)) => {}
                (Some(Request::DeleteSong(version, title)), Ok(Reply::Deleted)) => {
                    let shown = self
                        .search
                        .hits()
                        .unwrap_or(&self.catalog)
                        .iter()
                        .position(|(v, _)| *v == version);
                    if self.selected_song == Some(version) && self.selected_entry.is_none() {
                        // Preview keeps the slides it already prepared
                        // (provisional); the selection moves to a neighbour.
                        self.selected_song = None;
                        deleted_at = shown;
                    }
                    self.catalog.retain(|(v, _)| *v != version);
                    self.search.forget(version);
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
                    let chosen = schedule::entry_arrangement(&self.schedule, self.selected_entry);
                    let projection = slides::project(&song, chosen);
                    self.preview = Some(Item {
                        version,
                        entry: self.selected_entry,
                        title: song.title.clone(),
                        slides: projection.slides.into(),
                        pages: projection.pages.into(),
                        arrangements: song
                            .variants
                            .iter()
                            .map(|v| (v.id, v.name.clone()))
                            .collect(),
                        chosen,
                        copyright: slides::copyright_strip(&song).map(Into::into),
                        resolved: None,
                    });
                    self.preview_slide = 0;
                    if self.live_on_load == Some(version) && self.selected_entry.is_none() {
                        self.live_on_load = None;
                        live_now = true;
                    }
                    if self.live_entry_on_load.is_some()
                        && self.live_entry_on_load == self.selected_entry
                    {
                        self.live_entry_on_load = None;
                        live_now = true;
                    }
                    // The item's faces resolve off the UI thread before any
                    // of its slides can go live (M1-05g2b).
                    resolve_now = true;
                }
                (Some(Request::SaveSchedule(job)), Ok(Reply::Songs(songs))) => {
                    to_write = Some((job, songs));
                }
                (Some(Request::ImportSchedule(path, arrangements)), Ok(Reply::Imported(items))) => {
                    to_open = Some((path, items, arrangements));
                }
                (Some(Request::UpdateScan { pairs, versions }), Ok(Reply::Songs(songs))) => {
                    scanned = Some((pairs, versions, songs));
                }
                (
                    Some(
                        Request::SaveSchedule(_)
                        | Request::ImportSchedule(..)
                        | Request::UpdateScan { .. },
                    ),
                    Err(error),
                ) => {
                    self.schedule_message =
                        Some(t!(Msg::ScheduleStorageFailed, error = format!("{error:?}")));
                }
                (Some(Request::SongImport), reply) => import_reply = Some(reply),
                (_, Err(error)) => self.library_error = Some(storage_message(error)),
                _ => {}
            }
        }
        if self.request.is_none() {
            let wanted = self.selected_song.filter(|v| {
                self.preview.as_ref().is_none_or(|p| {
                    (p.version, p.entry, p.chosen)
                        != (
                            *v,
                            self.selected_entry,
                            schedule::entry_arrangement(&self.schedule, self.selected_entry),
                        )
                })
            });
            let request = if let Some(job) = self.followup.take() {
                Some(job)
            } else if let Some(job) = self.queued.take() {
                Some(job)
            } else if let Some(command) = self.song_import.take_storage() {
                Some((command, Request::SongImport))
            } else if let Some(version) = wanted {
                Some((storage::Command::Song(version), Request::Song(version)))
            } else if let Some(command) = self.search.take_command() {
                Some((command, Request::Search))
            } else if self.catalog_stale {
                self.catalog_stale = false;
                self.search.refresh();
                Some((storage::Command::Library(self.song_order), Request::Catalog))
            } else {
                None
            };
            if let Some((command, request)) = request {
                let schedule = matches!(
                    request,
                    Request::SaveSchedule(_)
                        | Request::ImportSchedule(..)
                        | Request::UpdateScan { .. }
                );
                match worker.submit(command) {
                    Ok(cancel) => {
                        if matches!(request, Request::SongImport) {
                            self.song_import.hold_cancel(cancel);
                        }
                        self.request = Some(request);
                    }
                    Err(error) if matches!(request, Request::SongImport) => {
                        import_reply = Some(Err(error));
                        changed = true;
                    }
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
        if let Some(reply) = import_reply {
            self.song_import_reply(reply);
        }
        if let Some(index) = deleted_at {
            let remaining = self.songs().len();
            if remaining > 0 {
                let neighbour = index.min(remaining - 1);
                self.select_song(neighbour, cx);
                // The confirmation returned focus to the deleted row; the
                // neighbour takes it over, which also covers a deleted last
                // row that is no longer rendered.
                self.reveal_song(neighbour);
            } else {
                self.song_selection.clear();
            }
        }
        if let Some((pairs, versions, songs)) = scanned {
            self.finish_update_scan(pairs, versions, songs);
            changed = true;
        }
        if rescan {
            self.begin_update_scan();
        }
        if let Some((job, songs)) = to_write {
            self.write_schedule(job, songs);
            changed = true;
        }
        if let Some((path, items, arrangements)) = to_open {
            opened_songs = self.finish_open(path, items, arrangements);
            changed = true;
        }
        if let Some(songs) = opened_songs {
            self.start_schedule_check(songs);
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
