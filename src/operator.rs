use super::*;
use gpui::{CursorStyle, Empty, FontWeight, MouseButton, Point, SharedString, Subscription, Task};
use sela::{
    delivery::{Epoch, LiveState},
    output::{Launch, Refusal, Status, Supervisor},
    scene::{ContentVersion, Extent},
    slides::{self, Slide},
    storage::{self, Reply, Version, Worker},
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

const TABS: [&str; 5] = ["Songs", "Scriptures", "Media", "Presentations", "Themes"];

// Sela-owned neutral chrome. Content/output state must not share selection colors.
const CHROME: u32 = 0xf4f4f3;
const SURFACE: u32 = 0xfafaf9;
const BORDER: u32 = 0xdcdedc;
const TEXT: u32 = 0x292c30;
const MUTED: u32 = 0x646971;
const HOVER: u32 = 0xe8e9e7;
const PRESSED: u32 = 0xdedfdc;
const ACCENT: u32 = 0x536aca;
const CANVAS: u32 = 0x202226;
const ON_SCREEN: u32 = 0xd0453b;
const SENDING: u32 = 0xd99a2b;
const ERROR: u32 = 0xb33232;
const DARK_TEXT: u32 = 0xf2f2f2;
const DARK_MUTED: u32 = 0x9ea3ab;
const DARK_ERROR: u32 = 0xf08a80;

pub(super) const LIVE_OUTPUT: usize = 11;
pub(super) const GO_LIVE: usize = 12;
pub(super) const PREVIOUS: usize = 13;
pub(super) const NEXT: usize = 14;
const CONTROLS: usize = 15;
const SONG_TAB_INDEX: isize = 100;
const SLIDE_TAB_INDEX: isize = 1000;
const POLL: Duration = Duration::from_millis(16);
const LABELS: usize = 8;

/// Builds the audience child command for a fresh session epoch.
pub(super) type Launcher = Arc<dyn Fn(Epoch) -> Launch + Send + Sync>;

#[derive(Clone, Copy)]
struct Split(usize);

#[derive(Clone)]
pub(super) struct Item {
    pub(super) version: Version,
    pub(super) title: String,
    pub(super) slides: Vec<Slide>,
}

enum Request {
    Open,
    Catalog,
    Song(Version),
}

pub(super) struct Operator {
    pub(super) focus: FocusHandle,
    pub(super) controls: [FocusHandle; CONTROLS],
    pub(super) ratios: [f32; 3],
    pub(super) collapsed: bool,
    pub(super) tab: usize,
    drag: Option<(usize, Point<gpui::Pixels>, f32)>,
    activation: Option<Subscription>,
    library_error: Option<String>,
    new_menu: bool,
    library: Option<PathBuf>,
    worker: Option<Worker>,
    request: Option<Request>,
    catalog_stale: bool,
    pub(super) catalog: Vec<(Version, String)>,
    song_rows: Vec<FocusHandle>,
    slide_rows: Vec<FocusHandle>,
    pub(super) selected_song: Option<Version>,
    pub(super) preview: Option<Item>,
    pub(super) preview_slide: usize,
    pub(super) live: Option<Item>,
    pub(super) live_slide: Option<usize>,
    launcher: Launcher,
    pub(super) output: Option<Supervisor>,
    output_extent: Option<Extent>,
    labels: VecDeque<(ContentVersion, String)>,
    pub(super) live_message: Option<String>,
    _poller: Option<Task<()>>,
}

impl Operator {
    pub(super) fn new(
        library: Option<PathBuf>,
        launcher: Launcher,
        cx: &mut Context<Self>,
    ) -> Self {
        let (worker, request, library_error) = match library.clone().map(Worker::open) {
            Some(Ok(worker)) => (Some(worker), Some(Request::Open), None),
            Some(Err(error)) => (None, None, Some(storage_message(error))),
            None => (None, None, None),
        };
        let executor = cx.background_executor().clone();
        let poller = cx.spawn(async move |this, cx| {
            loop {
                executor.timer(POLL).await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            focus: cx.focus_handle(),
            controls: std::array::from_fn(|index| {
                cx.focus_handle()
                    .tab_index(index as isize + 1)
                    .tab_stop(true)
            }),
            ratios: [0.24, 0.62, 0.62],
            collapsed: false,
            tab: 0,
            drag: None,
            activation: None,
            library_error,
            new_menu: false,
            library,
            worker,
            request,
            catalog_stale: false,
            catalog: Vec::new(),
            song_rows: Vec::new(),
            slide_rows: Vec::new(),
            selected_song: None,
            preview: None,
            preview_slide: 0,
            live: None,
            live_slide: None,
            launcher,
            output: None,
            output_extent: None,
            labels: VecDeque::with_capacity(LABELS),
            live_message: None,
            _poller: Some(poller),
        }
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        match index {
            0 => {
                self.ratios = [0.24, 0.62, 0.62];
                self.collapsed = false;
                self.drag = None;
            }
            9 => self.new_menu = !self.new_menu,
            1 => window.dispatch_action(Box::new(Quit), cx),
            2 => {
                self.collapsed = !self.collapsed;
                self.drag = None;
                if self.collapsed
                    && (self.controls[3..=10].iter().any(|f| f.is_focused(window))
                        || self.song_rows.iter().any(|f| f.is_focused(window)))
                {
                    self.controls[2].focus(window, cx);
                }
            }
            3..=7 if !self.collapsed => self.tab = index - 3,
            8 | 10 => {
                self.new_menu = false;
                if index == 10 {
                    self.controls[9].focus(window, cx);
                }
                self.library_error = self
                    .library
                    .clone()
                    .or_else(crate::song_library::default_path)
                    .ok_or_else(|| {
                        "No user data directory. Launch with --library DATABASE_PATH.".to_string()
                    })
                    .and_then(|path| crate::song_library::open(path, cx))
                    .err();
            }
            LIVE_OUTPUT => self.toggle_output(),
            GO_LIVE => self.go_live(),
            PREVIOUS => self.step_live(false),
            NEXT => self.step_live(true),
            _ => return,
        }
        cx.notify();
    }

    pub(super) fn poll(&mut self, cx: &mut Context<Self>) {
        let mut changed = self.poll_library();
        if let Some(output) = &mut self.output {
            changed |= output.poll(Instant::now());
            if let Status::Connected { extent, .. } = output.status()
                && self.output_extent != Some(extent)
            {
                self.output_extent = Some(extent);
                // Same session, new surface: re-render the current intent.
                if let Some(index) = self.live_slide {
                    self.send(index);
                }
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
    }

    fn poll_library(&mut self) -> bool {
        let Some(worker) = &mut self.worker else {
            return false;
        };
        let mut changed = false;
        if let Some(reply) = worker.poll() {
            changed = true;
            match (self.request.take(), reply) {
                (Some(Request::Open), Ok(Reply::Opened)) => self.catalog_stale = true,
                (Some(Request::Catalog), Ok(Reply::Catalog(catalog))) => {
                    self.catalog = catalog;
                    self.library_error = None;
                }
                (Some(Request::Song(version)), Ok(Reply::Song(song)))
                    if self.selected_song == Some(version) =>
                {
                    self.preview = Some(Item {
                        version,
                        title: song.title.clone(),
                        slides: slides::slides(&song),
                    });
                    self.preview_slide = 0;
                }
                (_, Err(error)) => self.library_error = Some(storage_message(error)),
                _ => {}
            }
        }
        if self.request.is_none() {
            let wanted = self
                .selected_song
                .filter(|v| self.preview.as_ref().is_none_or(|p| p.version != *v));
            let request = if let Some(version) = wanted {
                Some((storage::Command::Song(version), Request::Song(version)))
            } else if self.catalog_stale {
                self.catalog_stale = false;
                Some((storage::Command::Catalog(None), Request::Catalog))
            } else {
                None
            };
            if let Some((command, request)) = request {
                match worker.submit(command) {
                    Ok(_) => self.request = Some(request),
                    Err(error) => {
                        self.library_error = Some(storage_message(error));
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    fn select_song(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some((version, _)) = self.catalog.get(index) {
            self.selected_song = Some(*version);
            cx.notify();
        }
    }

    fn select_slide(&mut self, index: usize, go_live: bool, cx: &mut Context<Self>) {
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

    fn toggle_output(&mut self) {
        if self.output.take().is_some() {
            self.live_message = Some("Live output is off".into());
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
    }

    fn go_live(&mut self) {
        let Some(item) = self.preview.clone() else {
            self.live_message = Some("Select a song to preview first".into());
            return;
        };
        if item.slides.is_empty() {
            self.live_message = Some("The previewed song has no slides".into());
            return;
        }
        let index = self.preview_slide.min(item.slides.len() - 1);
        if self.ready_for_cue() {
            self.live = Some(item);
            self.send(index);
        }
    }

    fn step_live(&mut self, forward: bool) {
        let (Some(count), Some(index)) = (
            self.live.as_ref().map(|item| item.slides.len()),
            self.live_slide,
        ) else {
            self.live_message = Some("Nothing is live".into());
            return;
        };
        let target = if forward {
            index + 1
        } else {
            index.wrapping_sub(1)
        };
        // Boundary behavior across schedule items is unobserved; stop here.
        if target < count {
            self.send(target);
        }
    }

    fn ready_for_cue(&mut self) -> bool {
        let message = match self.output.as_ref().map(Supervisor::status) {
            None => "Turn on Live output first",
            Some(Status::Starting) => "Live output is still starting",
            Some(Status::Lost(_)) => "Output state unknown · turn Live off and on",
            Some(Status::Connected { .. }) => return true,
        };
        self.live_message = Some(message.into());
        false
    }

    fn send(&mut self, index: usize) {
        if !self.ready_for_cue() {
            return;
        }
        let (Some(output), Some(item)) = (&mut self.output, &self.live) else {
            return;
        };
        let Status::Connected { extent, caps } = output.status() else {
            return;
        };
        let Some((slide, version)) = item
            .slides
            .get(index)
            .zip(slides::slide_version(item.version, index))
        else {
            return;
        };
        let result = slides::cue(version, slide, extent, caps)
            .map_err(|error| error.to_string())
            .and_then(|cue| {
                output
                    .present(Arc::new(cue), Instant::now())
                    .map_err(|refusal| match refusal {
                        Refusal::NotConnected => "Live output is not connected".into(),
                        Refusal::WrongExtent => "Output size changed; try again".into(),
                    })
            });
        match result {
            Ok(()) => {
                let label = format!("{} · {}", item.title, slide_name(index, slide));
                if self.labels.len() == LABELS {
                    self.labels.pop_front();
                }
                self.labels.retain(|(v, _)| *v != version);
                self.labels.push_back((version, label));
                self.live_slide = Some(index);
                self.live_message = None;
            }
            Err(message) => self.live_message = Some(message),
        }
    }

    /// What the renderer has acknowledged, by label, if anything.
    pub(super) fn on_screen(&self) -> Option<&str> {
        let LiveState::Confirmed(version) = self.output.as_ref()?.live() else {
            return None;
        };
        self.labels
            .iter()
            .find(|(v, _)| *v == version)
            .map(|(_, label)| label.as_str())
    }

    fn live_index(&self, version: Option<ContentVersion>) -> Option<usize> {
        slides::slide_index(self.live.as_ref()?.version, version?)
    }

    pub(super) fn output_line(&self) -> String {
        match self.output.as_ref().map(Supervisor::status) {
            None => "Live output off".into(),
            Some(Status::Starting) => "Starting live output…".into(),
            Some(Status::Connected { extent, .. }) => {
                format!("Live output {}×{}", extent.width, extent.height)
            }
            Some(Status::Lost(loss)) => {
                format!("{loss} · output state unknown · turn Live off and on")
            }
        }
    }

    fn button(
        &self,
        index: usize,
        id: &'static str,
        label: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        control(id, label)
            .track_focus(&self.controls[index])
            .key_context("SelaControl")
            // Background distinguishes keyboard focus from the selected-tab underline.
            .focus(|d| d.bg(rgb(0xdce3fa)).text_color(rgb(0x253c91)))
            .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                if this.controls[index].is_focused(window) {
                    // GPUI also synthesizes keyboard clicks for on_click; use only
                    // this semantic route for keyboard activation.
                    window.prevent_default();
                    this.activate(index, window, cx);
                }
            }))
            .on_click(cx.listener(move |this, event, window, cx| {
                if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                    return;
                }
                this.controls[index].focus(window, cx);
                this.activate(index, window, cx);
            }))
    }

    pub(super) fn dimensions(&self, width: f32, height: f32) -> [f32; 4] {
        let available = (width - 12.).max(0.);
        let minimum = 150_f32.min(available / 3.);
        let left =
            (available * self.ratios[0]).clamp(minimum, (available - 2. * minimum).max(minimum));
        let boundary = (available * self.ratios[1])
            .clamp(left + minimum, (available - minimum).max(left + minimum));
        let body = (height - 76.).max(0.);
        let upper = if self.collapsed {
            (height - 70.).max(0.)
        } else {
            (body * self.ratios[2]).clamp(
                140_f32.min(body / 2.),
                (body - 140_f32.min(body / 2.)).max(0.),
            )
        };
        [left, boundary - left, available - boundary, upper]
    }

    fn splitter(&self, axis: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let horizontal = axis == 2;
        div()
            .id(match axis {
                0 => "split-schedule",
                1 => "split-live",
                _ => "split-resources",
            })
            .debug_selector(|| {
                match axis {
                    0 => "split-schedule",
                    1 => "split-live",
                    _ => "split-resources",
                }
                .into()
            })
            .flex_shrink_0()
            .when(horizontal, |d| {
                d.h(px(6.)).w_full().cursor(CursorStyle::ResizeUpDown)
            })
            .when(!horizontal, |d| {
                d.w(px(6.)).h_full().cursor(CursorStyle::ResizeLeftRight)
            })
            .bg(rgb(CHROME))
            .flex()
            .items_center()
            .justify_center()
            .hover(|d| d.bg(rgb(0xdde2f2)))
            .child(
                div()
                    .bg(rgb(BORDER))
                    .when(horizontal, |d| d.h(px(1.)).w_full())
                    .when(!horizontal, |d| d.w(px(1.)).h_full()),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, _| {
                    window.prevent_default();
                    this.drag = Some((axis, event.position, this.ratios[axis]));
                }),
            )
            .on_drag(Split(axis), |_, _, _, cx| cx.new(|_| Empty))
            .on_drag_move::<Split>(cx.listener(
                move |this, event: &gpui::DragMoveEvent<Split>, window, cx| {
                    if let Some((active, start, ratio)) = this.drag {
                        if active != axis || active != event.drag(cx).0 {
                            return;
                        }
                        let viewport = window.viewport_size();
                        let extent = if horizontal {
                            f32::from(viewport.height) - 76.
                        } else {
                            f32::from(viewport.width) - 12.
                        };
                        let delta = if horizontal {
                            event.event.position.y - start.y
                        } else {
                            event.event.position.x - start.x
                        };
                        if extent > 0. {
                            this.ratios[axis] = (ratio + f32::from(delta) / extent).clamp(0., 1.);
                            // Normalize stored proportions to the actual bounded geometry.
                            let d = this
                                .dimensions(f32::from(viewport.width), f32::from(viewport.height));
                            if horizontal {
                                this.ratios[2] = d[3] / extent;
                            } else {
                                this.ratios[0] = d[0] / extent;
                                this.ratios[1] = (d[0] + d[1]) / extent;
                            }
                            cx.notify();
                        }
                    }
                },
            ))
    }

    fn preview_pane(&mut self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
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
            Some(item) if count > 0 => format!(
                "{} · Slide {} of {count}",
                item.title,
                self.preview_slide + 1
            ),
            Some(item) => format!("{} · no slides", item.title),
            None => "Nothing selected".into(),
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
                        .track_focus(&self.slide_rows[index])
                        .key_context("SelaControl")
                        .focus(|d| d.bg(rgb(0x2c3550)))
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
        slide_pane("Preview", width, footer, None, tiles)
    }

    fn live_pane(&self, width: f32) -> impl IntoElement {
        let output = self.output.as_ref();
        let confirmed = output.and_then(|o| match o.live() {
            LiveState::Confirmed(version) => Some(version),
            LiveState::Unknown => None,
        });
        let confirmed = self.live_index(confirmed);
        let sending = self.live_index(output.and_then(|o| o.wanted().or(o.in_flight())));
        let rejected = output.and_then(|o| o.rejected());
        let mut lines = vec![(self.output_line(), DARK_MUTED)];
        lines.push(match self.on_screen() {
            Some(label) => (format!("On screen: {label}"), DARK_TEXT),
            None if matches!(
                output.map(Supervisor::status),
                Some(Status::Connected { .. })
            ) =>
            {
                ("On screen: nothing confirmed".into(), DARK_MUTED)
            }
            None => ("On screen: unknown".into(), DARK_MUTED),
        });
        if sending.is_some() {
            lines.push(("Sending…".into(), SENDING));
        }
        if let Some((_, error)) = rejected {
            lines.push((format!("Last cue not shown: {error}"), DARK_ERROR));
        }
        if let Some(message) = &self.live_message {
            lines.push((message.clone(), DARK_ERROR));
        }
        let footer = match (&self.live, self.live_slide) {
            (Some(item), Some(index)) => format!(
                "{} · Slide {} of {}",
                item.title,
                index + 1,
                item.slides.len()
            ),
            (Some(item), None) => format!("{} · not sent", item.title),
            _ => "Nothing live".into(),
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
                })
                .collect::<Vec<_>>()
        });
        slide_pane("Live", width, footer, Some(lines), tiles)
    }

    fn song_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        while self.song_rows.len() < self.catalog.len() {
            let index = self.song_rows.len() as isize;
            self.song_rows.push(
                cx.focus_handle()
                    .tab_index(SONG_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        if self.library.is_none() {
            return empty_detail(
                "Songs · no library open",
                "Launch Sela without arguments to use the default library.",
            );
        }
        if self.catalog.is_empty() {
            return empty_detail(
                "Songs · offline library",
                "No songs yet. Use + New Song to create one.",
            );
        }
        div()
            .id("song-list")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(
                self.catalog
                    .iter()
                    .enumerate()
                    .map(|(index, (version, title))| {
                        let selected = self.selected_song == Some(*version);
                        div()
                            .id(("song", index))
                            .debug_selector(move || format!("song-{index}"))
                            .track_focus(&self.song_rows[index])
                            .key_context("SelaControl")
                            .flex_shrink_0()
                            .h(px(28.))
                            .px_3()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(BORDER))
                            .cursor_pointer()
                            .bg(rgb(if selected { 0xe3e7f3 } else { SURFACE }))
                            .hover(|d| d.bg(rgb(HOVER)))
                            .focus(|d| d.bg(rgb(0xdce3fa)))
                            .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                                window.prevent_default();
                                this.select_song(index, cx);
                            }))
                            .on_click(cx.listener(move |this, event, window, cx| {
                                if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                    return;
                                }
                                this.song_rows[index].focus(window, cx);
                                this.select_song(index, cx);
                            }))
                            .child(if title.trim().is_empty() {
                                "Untitled".to_string()
                            } else {
                                title.clone()
                            })
                    }),
            )
            .into_any_element()
    }
}

fn storage_message(error: storage::Error) -> String {
    format!("Song library unavailable ({error:?})")
}

fn slide_name(index: usize, slide: &Slide) -> String {
    if slide.label.trim().is_empty() {
        format!("Slide {}", index + 1)
    } else {
        slide.label.clone()
    }
}

fn control(id: &'static str, label: impl Into<SharedString>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(|| id.into())
        .px_3()
        .h(px(26.))
        .flex()
        .items_center()
        .rounded(px(4.))
        .cursor_pointer()
        .text_color(rgb(MUTED))
        .hover(|d| d.bg(rgb(HOVER)).text_color(rgb(TEXT)))
        .active(|d| d.bg(rgb(PRESSED)))
        .child(label.into())
}

fn empty_detail(title: impl Into<SharedString>, text: &'static str) -> gpui::AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(div().font_weight(FontWeight::MEDIUM).child(title.into()))
        .child(
            div()
                .mt_2()
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .child(text),
        )
        .into_any_element()
}

/// Text-only slide thumbnail; not a rendered audience frame.
fn tile(slide: &Slide, index: usize, border: u32) -> gpui::Div {
    div()
        .w(px(168.))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap_1()
        .p_1()
        .rounded(px(4.))
        .child(
            div()
                .h(px(94.))
                .rounded(px(3.))
                .border_2()
                .border_color(rgb(border))
                .bg(rgb(0x000000))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .px_1()
                .text_size(px(9.))
                .text_color(rgb(0xf2f2f2))
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
                .text_size(px(11.))
                .text_color(rgb(0xb2b6be))
                .truncate()
                .child(format!("{} · {}", index + 1, slide_name(index, slide))),
        )
}

fn slide_pane(
    title: &'static str,
    width: f32,
    footer: String,
    status: Option<Vec<(String, u32)>>,
    tiles: Option<Vec<gpui::Stateful<gpui::Div>>>,
) -> impl IntoElement {
    let empty = tiles.as_ref().is_none_or(Vec::is_empty);
    div()
        .debug_selector(|| title.into())
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .h(px(32.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .px_3()
                .font_weight(FontWeight::MEDIUM)
                .border_b_1()
                .border_color(rgb(BORDER))
                .bg(rgb(CHROME))
                .child(title),
        )
        .children(status.map(|lines| {
            div()
                .debug_selector(move || format!("{title}-status"))
                .flex_shrink_0()
                .px_3()
                .py_1()
                .bg(rgb(CANVAS))
                .border_b_1()
                .border_color(rgb(0x34373d))
                .text_size(px(12.))
                .children(
                    lines
                        .into_iter()
                        .map(|(line, color)| div().truncate().text_color(rgb(color)).child(line)),
                )
        }))
        .child(
            div()
                .id(title)
                .flex_1()
                .min_h_0()
                .p_2()
                .overflow_y_scroll()
                .bg(rgb(CANVAS))
                .text_size(px(13.))
                .text_color(rgb(0xb2b6be))
                .when(empty, |d| {
                    d.flex()
                        .items_center()
                        .justify_center()
                        .child(if title == "Live" {
                            "Nothing live"
                        } else {
                            "Nothing selected"
                        })
                })
                .when_some(tiles.filter(|t| !t.is_empty()), |d, tiles| {
                    d.flex().flex_wrap().gap_2().children(tiles)
                }),
        )
        .child(
            div()
                .h(px(26.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .px_3()
                .border_t_1()
                .border_color(rgb(BORDER))
                .bg(rgb(CHROME))
                .text_size(px(11.))
                .text_color(rgb(MUTED))
                .truncate()
                .child(footer),
        )
}

impl Render for Operator {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |this, window, _| {
                if window.is_window_active() {
                    // Songs saved in the editor window appear on return.
                    this.catalog_stale = true;
                } else {
                    this.drag = None;
                }
            }));
        }
        let viewport = window.viewport_size();
        let [left, middle, right, upper] =
            self.dimensions(viewport.width.into(), viewport.height.into());
        let live_label = if self.output.is_some() {
            "Live ● On"
        } else {
            "Live ○ Off"
        };
        div()
            .key_context("Sela")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &Quit, window, _| window.remove_window()))
            .on_action(cx.listener(|_, _: &FocusNext, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrevious, window, cx| window.focus_prev(cx)))
            .capture_any_mouse_up(cx.listener(|this, _, _, _| this.drag = None))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(CHROME))
            .text_color(rgb(TEXT))
            .font_family("DejaVu Sans")
            .text_size(px(13.))
            .child(
                div()
                    .h(px(40.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(self.button(9, "new-menu", "New ▾", cx))
                    .children(
                        ["Open", "Save", "Web", "Remote"]
                            .map(|label| div().px_1().text_color(rgb(0x92969c)).child(label)),
                    )
                    .child(div().flex_1())
                    .child(
                        self.button(GO_LIVE, "go-live", "Go Live", cx)
                            .text_color(rgb(TEXT)),
                    )
                    .children(["Alerts", "Logo", "Black", "Clear"].map(|label| {
                        div()
                            .debug_selector(move || label.into())
                            .px_1()
                            .text_color(rgb(0x92969c))
                            .child(label)
                    }))
                    .child(
                        self.button(LIVE_OUTPUT, "live-output", live_label, cx)
                            .when(self.output.is_some(), |d| d.text_color(rgb(ON_SCREEN))),
                    ),
            )
            .child(
                div()
                    .h(px(upper))
                    .flex_shrink_0()
                    .flex()
                    .child(pane("Schedule", "Schedule is empty", left, false))
                    .child(self.splitter(0, cx))
                    .child(self.preview_pane(middle, cx))
                    .child(self.splitter(1, cx))
                    .child(
                        div()
                            .w(px(right))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(div().flex_1().min_h_0().child(self.live_pane(right)))
                            .child(
                                div()
                                    .h(px(32.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .border_t_1()
                                    .border_color(rgb(BORDER))
                                    .child(self.button(PREVIOUS, "live-previous", "‹ Previous", cx))
                                    .child(self.button(NEXT, "live-next", "Next ›", cx)),
                            ),
                    ),
            )
            .when(!self.collapsed, |d| d.child(self.splitter(2, cx)))
            .child(
                div()
                    .h(px(30.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_3()
                    .gap_3()
                    .child(div().font_weight(FontWeight::MEDIUM).child("Resources"))
                    .child(self.button(
                        2,
                        "collapse-resources",
                        if self.collapsed {
                            "Restore resources"
                        } else {
                            "Collapse resources"
                        },
                        cx,
                    ))
                    .child(self.button(1, "quit", "Quit · Ctrl+Q", cx))
                    .child(self.button(0, "reset-layout", "Reset layout", cx))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(self.output_line()),
                    ),
            )
            .when(!self.collapsed, |d| {
                let detail = if self.tab == 0 {
                    self.song_list(cx)
                } else {
                    empty_detail(
                        format!("{} library is empty", TABS[self.tab]),
                        "This resource library is not implemented.",
                    )
                };
                d.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
                            div()
                                .h(px(32.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .px_3()
                                .border_b_1()
                                .border_color(rgb(BORDER))
                                .children(TABS.iter().enumerate().map(|(index, label)| {
                                    self.button(index + 3, label, *label, cx)
                                        .h_full()
                                        .rounded_none()
                                        .border_b_2()
                                        .border_color(rgb(if self.tab == index {
                                            ACCENT
                                        } else {
                                            CHROME
                                        }))
                                        .bg(rgb(if self.tab == index { SURFACE } else { CHROME }))
                                        .when(self.tab == index, |d| {
                                            d.text_color(rgb(TEXT)).font_weight(FontWeight::MEDIUM)
                                        })
                                })),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .bg(rgb(SURFACE))
                                .child(
                                    div()
                                        .w(px(left))
                                        .flex_shrink_0()
                                        .p_3()
                                        .border_r_1()
                                        .border_color(rgb(BORDER))
                                        .bg(rgb(CHROME))
                                        .child(
                                            div()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Collections"),
                                        )
                                        .child(
                                            div()
                                                .mt_2()
                                                .text_color(rgb(MUTED))
                                                .child("No collections"),
                                        ),
                                )
                                .child(
                                    div()
                                        .debug_selector(|| "library-detail".into())
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .child(div().flex_1().min_h_0().child(detail))
                                        .children(self.library_error.clone().map(|error| {
                                            div().px_3().py_1().text_color(rgb(ERROR)).child(error)
                                        })),
                                ),
                        ),
                )
                .when(self.tab == 0, |d| {
                    d.child(
                        div()
                            .h(px(30.))
                            .flex_shrink_0()
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .child(self.button(8, "open-library", "+ New Song", cx)),
                    )
                })
            })
            .when(self.new_menu, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(40.))
                        .left(px(8.))
                        .bg(rgb(SURFACE))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .shadow_md()
                        .child(self.button(10, "new-song-menu", "New Song", cx)),
                )
            })
    }
}

fn pane(title: &'static str, text: &'static str, width: f32, dark: bool) -> impl IntoElement {
    div()
        .debug_selector(|| title.into())
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .h(px(32.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .px_3()
                .font_weight(FontWeight::MEDIUM)
                .border_b_1()
                .border_color(rgb(BORDER))
                .bg(rgb(CHROME))
                .child(title),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .p_3()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.))
                .bg(rgb(if dark { CANVAS } else { SURFACE }))
                .text_color(rgb(if dark { 0xb2b6be } else { MUTED }))
                .child(text),
        )
}
