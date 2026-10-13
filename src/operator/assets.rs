//! Schedule preflight, the Go Live check and the import progress notes
//! (M1-11b). Files are checked on `preflight::Worker` and `images::Worker`;
//! nothing here touches the disk on the UI thread. Policy: `docs/images.md`.
use super::*;
use sela::{
    background::ImageRef,
    images::Substitute,
    preflight::{self, ImageIssue, LogoIssue},
    storage::Song,
};

pub(crate) const PREFLIGHT_DIALOG_W: f32 = 560.;
/// First tab index of the Locate… buttons: after the dialog's own buttons.
const LOCATE_TAB_INDEX: isize = DIALOG_TAB_INDEX + 10;

/// A Go Live waiting for its item's images to be checked.
struct HeldGoLive {
    generation: u64,
    item: Item,
    index: usize,
}

#[derive(Default)]
pub(crate) struct Assets {
    pub(crate) worker: Option<preflight::Worker>,
    /// The opened schedule's songs, kept for re-checks after a relink.
    songs: Vec<Song>,
    /// Generation of the schedule check in flight or shown.
    generation: u64,
    pub(crate) checking: bool,
    pub(crate) report: Option<preflight::Report>,
    held: Option<HeldGoLive>,
    pub(crate) held_problems: Vec<(ImageRef, Substitute)>,
    locate_rows: Vec<FocusHandle>,
    /// Last Locate… outcome, shown in the report; true when it failed.
    pub(crate) note: Option<(String, bool)>,
    /// A dialog opened from `poll` takes focus on the next render.
    focus_dialog: bool,
    /// File name of the import in flight.
    pub(crate) importing: Option<String>,
    /// What Sela did to the last imported file, one sentence each.
    pub(crate) import_notes: Vec<String>,
}

fn attention(report: &preflight::Report) -> bool {
    !report.items.is_empty() || report.logo.is_some()
}

fn problem_label(problem: Substitute) -> &'static str {
    t!(match problem {
        Substitute::Missing => Msg::PreflightProblemMissing,
        Substitute::Changed => Msg::PreflightProblemChanged,
        Substitute::TooLarge => Msg::PreflightProblemTooLarge,
        Substitute::Invalid => Msg::PreflightProblemCorrupt,
    })
}

fn size_label(extent: Extent) -> String {
    format!("{}×{}", extent.width, extent.height)
}

/// One sentence per thing Sela did to the file, for the Media footer.
pub(crate) fn import_notes(imported: &images::Imported) -> Vec<String> {
    let mut notes = Vec::new();
    if let Some(from) = imported.downscaled_from {
        notes.push(t!(
            Msg::ImportNoteDownscaled,
            from = size_label(from),
            to = size_label(imported.extent)
        ));
    }
    if imported.reused {
        notes.push(t!(Msg::ImportNoteReused).into());
    }
    if imported.oriented {
        notes.push(t!(Msg::ImportNoteOriented).into());
    }
    if imported.icc_ignored {
        notes.push(t!(Msg::ImportNoteIcc).into());
    }
    notes
}

impl Assets {
    pub(crate) fn start(profile: Option<&Path>) -> Self {
        Self {
            worker: profile.and_then(|p| preflight::Worker::start(p.to_path_buf()).ok()),
            ..Self::default()
        }
    }

    pub(crate) fn needs_review(&self) -> bool {
        self.report.as_ref().is_some_and(attention)
    }
}

impl Operator {
    /// A schedule was opened: remember its songs and check them.
    pub(crate) fn start_schedule_check(&mut self, songs: Vec<Song>) {
        self.assets.songs = songs;
        self.assets.report = None;
        self.assets.note = None;
        self.recheck_schedule();
    }

    fn recheck_schedule(&mut self) {
        let assets = &mut self.assets;
        if let Some(worker) = &mut assets.worker
            && let Ok(generation) = worker.check(assets.songs.clone())
        {
            assets.generation = generation;
            assets.checking = true;
        }
    }

    /// Opens a dialog from `poll`, which has no window: it takes focus on the
    /// next render. Refuses while another dialog is open.
    fn show_dialog_later(&mut self, dialog: Dialog) -> bool {
        if self.dialog.is_some() {
            return false;
        }
        self.dialog = Some(dialog);
        self.dialog_message = None;
        self.item_menu = None;
        self.song_menu = None;
        self.bar_menu = None;
        self.new_menu = false;
        self.assets.focus_dialog = true;
        true
    }

    /// Called from `render`, where the window is available.
    pub(crate) fn focus_pending_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.assets.focus_dialog) && self.dialog.is_some() {
            self.dialog_return = window.focused(cx);
            // The non-destructive choice, so Enter never confirms by accident.
            self.controls[Control::DialogCancel].focus(window, cx);
        }
    }

    pub(crate) fn review_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.assets.report.is_some() {
            self.open_dialog(Dialog::Preflight, window, cx);
        }
    }

    pub(crate) fn poll_preflight(&mut self) -> bool {
        let mut changed = false;
        // Two replies are buffered at most; bound the loop anyway.
        for _ in 0..4 {
            let Some(reply) = self
                .assets
                .worker
                .as_ref()
                .and_then(preflight::Worker::poll)
            else {
                break;
            };
            changed = true;
            match reply {
                Err(_) => {
                    // The worker is gone: Go Live is not held up by it.
                    self.assets.worker = None;
                    self.assets.checking = false;
                    if let Some(held) = self.assets.held.take() {
                        self.go_live_item(held.item, held.index);
                    }
                    break;
                }
                Ok(preflight::Reply::Checked(generation, report))
                    if generation == self.assets.generation =>
                {
                    self.assets.checking = false;
                    let items = report.items.len();
                    let attention = attention(&report);
                    self.assets.report = Some(report);
                    if attention {
                        if items > 0 {
                            self.schedule_message =
                                Some(t!(Msg::PreflightAttention, count = items));
                        }
                        self.show_dialog_later(Dialog::Preflight);
                    }
                }
                Ok(preflight::Reply::Checked(..)) => {}
                Ok(preflight::Reply::Relinked(image, result)) => {
                    self.assets.note = Some(match result {
                        Ok(_) => (t!(Msg::PreflightRelinked, name = image.name), false),
                        Err(error) => (
                            t!(
                                Msg::PreflightRelinkFailed,
                                name = image.name,
                                error = error.to_string()
                            ),
                            true,
                        ),
                    });
                    if result.is_ok() {
                        // The file is back: shown backgrounds and the list
                        // must be rebuilt, and the report refreshed.
                        self.backgrounds.forget_substitutes();
                        if let Some(worker) = &self.image_worker {
                            let _ = worker.submit(images::Job::Scan);
                        }
                        self.recheck_schedule();
                    }
                }
                Ok(preflight::Reply::ImagesChecked(generation, problems)) => {
                    if self.assets.held.as_ref().map(|h| h.generation) != Some(generation) {
                        continue;
                    }
                    if problems.is_empty() {
                        if let Some(held) = self.assets.held.take() {
                            self.go_live_item(held.item, held.index);
                        }
                    } else {
                        self.assets.held_problems = problems;
                        self.live_message = None;
                        if !self.show_dialog_later(Dialog::GoLiveMissing) {
                            self.hold_go_live_back();
                        }
                    }
                }
            }
        }
        changed
    }

    /// Go Live, after the image of the slide being sent was checked. Returns
    /// true when the request was held for that check; the reply resumes it.
    pub(crate) fn hold_go_live_for_check(&mut self, item: &Item, index: usize) -> bool {
        let mut refs: Vec<ImageRef> = Vec::new();
        if let Some(slide) = item.slides.get(index)
            && let Plan::Image(image, _) = background::plan(slide.background.as_ref())
        {
            refs.push(image.clone());
        }
        let Some(worker) = &mut self.assets.worker else {
            return false;
        };
        if refs.is_empty() {
            return false;
        }
        // A full queue is not a reason to refuse Go Live: a missing image
        // already shows black with its warning.
        let Ok(generation) = worker.check_images(refs) else {
            return false;
        };
        self.assets.held = Some(HeldGoLive {
            generation,
            item: item.clone(),
            index,
        });
        self.assets.held_problems.clear();
        self.live_message = Some(t!(Msg::PreflightChecking).into());
        true
    }

    /// The operator chose Go Live anyway: the usual cue, with its black
    /// substitute and warning.
    pub(crate) fn go_live_anyway(&mut self) {
        self.assets.held_problems.clear();
        if let Some(held) = self.assets.held.take() {
            self.go_live_item(held.item, held.index);
        }
    }

    /// The warning was declined (or could not be shown): the audience keeps
    /// the current scene.
    pub(crate) fn hold_go_live_back(&mut self) {
        self.assets.held = None;
        self.assets.held_problems.clear();
        self.live_message = Some(t!(Msg::GoLiveHeld).into());
    }

    pub(crate) fn go_live_missing_body(&self) -> gpui::AnyElement {
        div()
            .debug_selector(|| "go-live-missing".into())
            .flex()
            .flex_col()
            .gap_1()
            .text_size(px(TEXT_SMALL))
            .text_color(rgb(MUTED))
            .children(
                self.assets
                    .held_problems
                    .iter()
                    .take(4)
                    .map(|(image, problem)| {
                        div().child(t!(
                            Msg::GoLiveMissingBody,
                            name = image.name,
                            problem = problem_label(*problem)
                        ))
                    }),
            )
            .into_any_element()
    }

    pub(crate) fn preflight_body(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(report) = self.assets.report.clone() else {
            return div()
                .text_size(px(TEXT_SMALL))
                .text_color(rgb(MUTED))
                .child(t!(Msg::PreflightChecking))
                .into_any_element();
        };
        let images: usize = report.items.iter().map(|item| item.images.len()).sum();
        while self.assets.locate_rows.len() < images {
            let n = self.assets.locate_rows.len() as isize;
            self.assets.locate_rows.push(
                cx.focus_handle()
                    .tab_index(LOCATE_TAB_INDEX + n)
                    .tab_stop(true),
            );
        }
        let caption = |text: String, color: u32| {
            div()
                .text_size(px(TEXT_CAPTION))
                .text_color(rgb(color))
                .child(text)
        };
        let mut next = 0;
        let mut list = div()
            .id("preflight-list")
            .debug_selector(|| "preflight-list".into())
            .role(Role::List)
            .aria_label(t!(Msg::PreflightHeading))
            .accessibility_id("preflight-list")
            .max_h(px(280.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        for item in &report.items {
            let mut group = div().flex().flex_col().gap_1().child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .child(shown_title(&item.title)),
            );
            for issue in &item.images {
                let row = self.preflight_row(next, issue, &item.title, cx);
                next += 1;
                group = group.child(row);
            }
            for font in &item.fonts {
                group = group.child(caption(
                    t!(
                        Msg::PreflightFont,
                        family = font.family,
                        title = shown_title(&item.title)
                    ),
                    MUTED,
                ));
            }
            list = list.child(group);
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(caption(t!(Msg::PreflightIntro).into(), MUTED))
            .child(if report.items.is_empty() && report.logo.is_none() {
                caption(t!(Msg::PreflightAllClear).into(), MUTED).into_any_element()
            } else {
                list.into_any_element()
            })
            .children(report.logo.as_ref().map(|logo| {
                let error = match logo {
                    LogoIssue::Choice(error) | LogoIssue::Image { problem: error, .. } => error,
                };
                caption(t!(Msg::PreflightLogo, error = error.to_string()), ERROR)
            }))
            .children(
                (report.unchecked_images > 0 || report.unchecked_fonts > 0)
                    .then(|| caption(t!(Msg::PreflightUnchecked).into(), MUTED)),
            )
            .children(self.assets.note.clone().map(|(text, failed)| {
                caption(text, if failed { ERROR } else { MUTED })
                    .debug_selector(|| "preflight-note".into())
            }))
            .into_any_element()
    }

    fn preflight_row(
        &self,
        n: usize,
        issue: &ImageIssue,
        title: &str,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let image = issue.image.clone();
        let problem = problem_label(issue.problem);
        let label = t!(Msg::PreflightLocate);
        let run = {
            let image = image.clone();
            move |this: &mut Self, cx: &mut Context<Self>| this.locate(image.clone(), cx)
        };
        let focus = self.assets.locate_rows[n].clone();
        div()
            .id(("preflight-row", n))
            .debug_selector(move || format!("preflight-row-{n}"))
            .role(Role::ListItem)
            .aria_label(format!(
                "{} · {problem} · {}",
                image.name,
                t!(Msg::PreflightUsedBy, title = title)
            ))
            .accessibility_id(format!("preflight-row-{n}"))
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(image.name.clone()),
            )
            .child(
                div()
                    .debug_selector(move || format!("preflight-problem-{n}"))
                    .flex_shrink_0()
                    .text_size(px(TEXT_CAPTION))
                    .text_color(rgb(ERROR))
                    .child(problem),
            )
            .child(
                div()
                    .id(("locate", n))
                    .debug_selector(move || format!("locate-{n}"))
                    .role(Role::Button)
                    .aria_label(label)
                    .accessibility_id(format!("locate-{n}"))
                    .track_focus(&focus)
                    .key_context("SelaControl")
                    .flex_shrink_0()
                    .px_3()
                    .h(px(CONTROL_H))
                    .flex()
                    .items_center()
                    .rounded(px(RADIUS))
                    .cursor_pointer()
                    .text_color(rgb(MUTED))
                    .hover(|d| d.bg(rgb(HOVER)).text_color(rgb(TEXT)))
                    .active(|d| d.bg(rgb(PRESSED)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)).text_color(rgb(FOCUS_TEXT)))
                    .on_action(cx.listener({
                        let focus = focus.clone();
                        let run = run.clone();
                        move |this, _: &ActivateControl, window, cx| {
                            if focus.is_focused(window) {
                                window.prevent_default();
                                run(this, cx);
                            }
                        }
                    }))
                    .on_click(cx.listener(move |this, event, window, cx| {
                        if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                            return;
                        }
                        focus.focus(window, cx);
                        run(this, cx);
                    }))
                    .child(label),
            )
            .into_any_element()
    }

    /// Asks for the original file and relinks it if its bytes match.
    pub(crate) fn locate(&mut self, image: ImageRef, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t!(Msg::PreflightLocatePrompt).into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.relink(image, path);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn relink(&mut self, image: ImageRef, source: PathBuf) {
        let name = image.name.clone();
        let result = match &mut self.assets.worker {
            Some(worker) => worker.relink(image, source),
            None => Err(images::Error::Disconnected),
        };
        if let Err(error) = result {
            self.assets.note = Some((
                t!(
                    Msg::PreflightRelinkFailed,
                    name = name,
                    error = error.to_string()
                ),
                true,
            ));
        }
    }

    /// Cancels the import in flight; the worker replies `Cancelled` and the
    /// library is left as it was.
    pub(crate) fn cancel_import(&mut self) {
        if self.assets.importing.is_none() {
            return;
        }
        if let Some(worker) = &self.image_worker {
            worker.cancel_imports();
        }
        self.media_message = Some(t!(Msg::ImportCancelling).into());
    }
}
