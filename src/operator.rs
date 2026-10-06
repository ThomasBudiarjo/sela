use super::*;
use crate::text_input::{self, TextInput};
use gpui::{
    CursorStyle, Empty, Entity, Focusable, FontWeight, MouseButton, PathPromptOptions, Pixels,
    Point, SharedString, Subscription, Task,
};
use sela::{
    delivery::{Epoch, LiveState},
    fonts, images,
    masks::{Layer, Mask, Masks},
    output::{Launch, Refusal, Status, Supervisor},
    preparation::{PreparationEvent, Preparer},
    scene::{ContentVersion, Extent, PrepareError, RendererCapabilities},
    schedule::{EntryId, Schedule},
    slides::{self, Sizing, Slide},
    storage::{self, MAX_ITEMS, Reply, Version, Worker},
};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
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
const MASK_ON: u32 = 0xf6dfdc;
const DISABLED: u32 = 0xb0b3b8;

pub(super) const LIVE_OUTPUT: usize = 11;
pub(super) const GO_LIVE: usize = 12;
pub(super) const PREVIOUS: usize = 13;
pub(super) const NEXT: usize = 14;
pub(super) const LOGO: usize = 15;
pub(super) const BLACK: usize = 16;
pub(super) const CLEAR: usize = 17;
pub(super) const IMPORT_IMAGE: usize = 18;
pub(super) const USE_AS_LOGO: usize = 19;
pub(super) const IMAGE_MENU_LOGO: usize = 20;
pub(super) const NORMALIZE: usize = 21;
pub(super) const OPEN_SCHEDULE: usize = 22;
pub(super) const SAVE_SCHEDULE: usize = 23;
pub(super) const ADD_TO_SCHEDULE: usize = 24;
pub(super) const MOVE_UP: usize = 25;
pub(super) const MOVE_DOWN: usize = 26;
pub(super) const REMOVE_ITEM: usize = 27;
pub(super) const DIALOG_CONFIRM: usize = 28;
pub(super) const DIALOG_CANCEL: usize = 29;
pub(super) const NEW_SCHEDULE: usize = 30;
pub(super) const ITEM_MENU_REMOVE: usize = 31;
const CONTROLS: usize = 32;
const SONG_TAB_INDEX: isize = 100;
const SCHEDULE_TAB_INDEX: isize = 500;
const SAVED_TAB_INDEX: isize = 700;
const DIALOG_TAB_INDEX: isize = 900;
const SLIDE_TAB_INDEX: isize = 1000;
const IMAGE_TAB_INDEX: isize = 2000;
const POLL: Duration = Duration::from_millis(16);
const LABELS: usize = 8;
const LOGO_TIMEOUT: Duration = Duration::from_secs(5);
const NO_LOGO: &str = "No logo set · Media → select an image → Use As Logo Background";

/// Show-control button state. Lit only once the renderer acknowledged it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Indicator {
    Off,
    /// Requested but not yet acknowledged, or acknowledged but being turned off.
    Pending,
    On,
}

fn layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::None => "None",
        Layer::Clear => "Clear",
        Layer::Black => "Black",
        Layer::Logo => "Logo",
    }
}

/// Builds the audience child command for a fresh session epoch.
pub(super) type Launcher = Arc<dyn Fn(Epoch) -> Launch + Send + Sync>;

/// Keyboard order follows control indices, with room to place the New menu's
/// second item next to the first and dialog buttons after their rows.
fn tab_position(index: usize) -> isize {
    match index {
        NEW_SCHEDULE => 2 * 11 + 1,
        DIALOG_CONFIRM => DIALOG_TAB_INDEX + 1,
        DIALOG_CANCEL => DIALOG_TAB_INDEX + 2,
        _ => 2 * (index as isize + 1),
    }
}

#[derive(Clone, Copy)]
struct Split(usize);

/// A library song dragged towards the Schedule.
#[derive(Clone)]
struct SongDrag {
    version: Version,
    title: String,
}

/// A schedule entry dragged to a new position.
#[derive(Clone, Copy)]
struct EntryDrag(EntryId);

struct DragLabel(SharedString);

impl Render for DragLabel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded(px(4.))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE))
            .shadow_md()
            .text_size(px(12.))
            .text_color(rgb(TEXT))
            .child(self.0.clone())
    }
}

#[derive(Clone)]
pub(super) struct Item {
    pub(super) version: Version,
    /// The schedule entry this was previewed from; `None` for a library song.
    pub(super) entry: Option<EntryId>,
    pub(super) title: String,
    pub(super) slides: Vec<Slide>,
}

/// What follows once unsaved schedule changes are discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Then {
    Open,
    New,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Dialog {
    SaveAs,
    Open,
    Unsaved(Then),
    Remove(EntryId),
}

enum Request {
    Open,
    Catalog,
    Song(Version),
    SaveSchedule(storage::Schedule),
    ScheduleCatalog,
    OpenSchedule(Version),
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
    /// Operator mask intent; survives Live output off/on (EW8-OBS-017).
    pub(super) masks: Masks,
    /// Layer handed to this session's supervisor.
    sent_mask: Option<Layer>,
    /// A Live double-click unmasks only after its slide left the cue slot,
    /// so the previous slide is never revealed in between.
    unmask_after_cue: bool,
    pub(super) logo: Option<images::Logo>,
    logo_preparer: Option<(RendererCapabilities, Preparer)>,
    logo_requested: Option<ContentVersion>,
    logo_sent: Option<ContentVersion>,
    pub(super) logo_error: Option<String>,
    profile: Option<PathBuf>,
    image_worker: Option<images::Worker>,
    pub(super) images: Vec<String>,
    image_rows: Vec<FocusHandle>,
    pub(super) selected_image: Option<String>,
    image_menu: Option<Point<Pixels>>,
    pub(super) media_message: Option<String>,
    /// Not persisted; Sela has no settings store yet.
    pub(super) sizing: Sizing,
    pub(super) size_cap: Option<(Version, Extent, Sizing, Option<u16>)>,
    pub(super) schedule: Schedule,
    pub(super) selected_entry: Option<EntryId>,
    schedule_rows: Vec<FocusHandle>,
    pub(super) schedule_message: Option<String>,
    /// One schedule storage job waiting for the worker.
    queued: Option<(storage::Command, Request)>,
    /// `None` while the saved list is loading.
    pub(super) saved_schedules: Option<Vec<(Version, String)>>,
    saved_rows: Vec<FocusHandle>,
    pub(super) dialog: Option<Dialog>,
    dialog_return: Option<FocusHandle>,
    pub(super) dialog_message: Option<String>,
    pub(super) title_input: Option<Entity<TextInput>>,
    item_menu: Option<(EntryId, Point<Pixels>)>,
    /// A library double-click waiting for its song to load.
    live_on_load: Option<Version>,
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
        let profile = library
            .as_deref()
            .and_then(Path::parent)
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf);
        let mut media_message = None;
        let image_worker = profile.clone().and_then(|profile| {
            let worker = images::Worker::start(profile).ok();
            if worker.is_none() {
                media_message = Some("Image worker could not start".into());
            }
            worker
        });
        if let Some(worker) = &image_worker {
            // A fresh worker has room for both jobs.
            let _ = worker.submit(images::Job::LoadLogo);
            let _ = worker.submit(images::Job::Scan);
        }
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
                    .tab_index(tab_position(index))
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
            masks: Masks::default(),
            sent_mask: None,
            unmask_after_cue: false,
            logo: None,
            logo_preparer: None,
            logo_requested: None,
            logo_sent: None,
            logo_error: None,
            profile,
            image_worker,
            images: Vec::new(),
            image_rows: Vec::new(),
            selected_image: None,
            image_menu: None,
            media_message,
            sizing: Sizing::default(),
            size_cap: None,
            schedule: Schedule::default(),
            selected_entry: None,
            schedule_rows: Vec::new(),
            schedule_message: None,
            queued: None,
            saved_schedules: None,
            saved_rows: Vec::new(),
            dialog: None,
            dialog_return: None,
            dialog_message: None,
            title_input: None,
            item_menu: None,
            live_on_load: None,
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
                        || self.controls[IMPORT_IMAGE..=NORMALIZE]
                            .iter()
                            .any(|f| f.is_focused(window))
                        || self.controls[ADD_TO_SCHEDULE].is_focused(window)
                        || self.song_rows.iter().any(|f| f.is_focused(window))
                        || self.image_rows.iter().any(|f| f.is_focused(window)))
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
            LOGO => self.toggle_mask(Mask::Logo),
            BLACK => self.toggle_mask(Mask::Black),
            CLEAR => self.toggle_mask(Mask::Clear),
            IMPORT_IMAGE => self.prompt_import(cx),
            USE_AS_LOGO => self.use_as_logo(),
            IMAGE_MENU_LOGO => {
                self.image_menu = None;
                self.controls[USE_AS_LOGO].focus(window, cx);
                self.use_as_logo();
            }
            NORMALIZE if !self.collapsed && self.tab == 0 => {
                self.sizing = match self.sizing {
                    Sizing::PerSlide => Sizing::Normalized,
                    Sizing::Normalized => Sizing::PerSlide,
                };
                // Applies to the live slide now, so its size never changes
                // only on the next navigation.
                if let Some(index) = self.live_slide
                    && matches!(
                        self.output.as_ref().map(Supervisor::status),
                        Some(Status::Connected { .. })
                    )
                {
                    self.send(index);
                }
            }
            OPEN_SCHEDULE => self.open_schedule(window, cx),
            SAVE_SCHEDULE => self.save_schedule(window, cx),
            ADD_TO_SCHEDULE if !self.collapsed && self.tab == 0 => {
                match self
                    .selected_song
                    .filter(|_| self.selected_entry.is_none())
                    .and_then(|v| self.catalog.iter().find(|(c, _)| *c == v).cloned())
                {
                    Some((version, title)) => self.add_entry(usize::MAX, version, title),
                    None => self.schedule_message = Some("Select a song in Songs first".into()),
                }
            }
            MOVE_UP | MOVE_DOWN => match self.selected_entry {
                Some(id) => {
                    self.schedule.move_by(id, index == MOVE_UP);
                }
                None => self.schedule_message = Some("Select a schedule item first".into()),
            },
            REMOVE_ITEM => match self.selected_entry {
                Some(id) => self.open_dialog(Dialog::Remove(id), window, cx),
                None => self.schedule_message = Some("Select a schedule item first".into()),
            },
            ITEM_MENU_REMOVE => {
                if let Some((id, _)) = self.item_menu.take() {
                    self.open_dialog(Dialog::Remove(id), window, cx);
                }
            }
            NEW_SCHEDULE if self.new_menu => {
                self.new_menu = false;
                self.controls[9].focus(window, cx);
                self.guard(Then::New, window, cx);
            }
            DIALOG_CONFIRM => match self.dialog {
                Some(Dialog::SaveAs) => {
                    let title = self
                        .title_input
                        .as_ref()
                        .map(|input| input.read(cx).text().trim().to_owned())
                        .unwrap_or_default();
                    if title.is_empty() {
                        self.dialog_message = Some("Enter a title for the schedule".into());
                    } else if self.queue_save(title) {
                        self.close_dialog(window, cx);
                    }
                }
                Some(Dialog::Remove(id)) => {
                    self.close_dialog(window, cx);
                    self.remove_entry(id);
                }
                Some(Dialog::Unsaved(then)) => {
                    self.close_dialog(window, cx);
                    self.proceed(then, window, cx);
                }
                Some(Dialog::Open) | None => return,
            },
            DIALOG_CANCEL if self.dialog.is_some() => self.close_dialog(window, cx),
            _ => return,
        }
        cx.notify();
    }

    fn add_entry(&mut self, at: usize, version: Version, title: String) {
        self.schedule_message = match self.schedule.insert(at, version, title) {
            Ok(_) => None,
            Err(_) => Some(format!("The schedule is full ({MAX_ITEMS} items)")),
        };
    }

    /// Never touches Live: a removed live item stays on screen (see `live_lines`).
    fn remove_entry(&mut self, id: EntryId) {
        if self.schedule.remove(id).is_some() && self.selected_entry == Some(id) {
            self.selected_entry = None;
        }
    }

    fn select_entry(&mut self, id: EntryId) {
        if let Some(entry) = self.schedule.get(id) {
            self.selected_entry = Some(id);
            self.selected_song = Some(entry.version);
            self.live_on_load = None;
            self.schedule_message = None;
        }
    }

    /// Down/Up. Stops at either end; provisional (EasyWorship unobserved).
    fn step_schedule(&mut self, forward: bool) {
        match self.schedule.neighbor(self.selected_entry, forward) {
            Some(id) => self.select_entry(id),
            None if self.schedule.entries().is_empty() => {
                self.schedule_message = Some("The schedule is empty".into());
            }
            None => {}
        }
    }

    fn remove_selected(&mut self) {
        match self.selected_entry {
            Some(id) => self.remove_entry(id),
            None => self.schedule_message = Some("Select a schedule item first".into()),
        }
    }

    fn open_dialog(&mut self, dialog: Dialog, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_none() {
            self.dialog_return = window.focused(cx);
        }
        self.dialog = Some(dialog);
        self.dialog_message = None;
        self.item_menu = None;
        self.new_menu = false;
        if dialog == Dialog::SaveAs {
            let input = self.title_input.get_or_insert_with(|| {
                cx.new(|cx| {
                    TextInput::new("", false, 1024, DIALOG_TAB_INDEX, cx)
                        .expect("empty text is valid")
                })
            });
            input.update(cx, |input, cx| {
                let _ = input.set_text("", cx);
            });
            input.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            // The non-destructive choice, so Enter never discards by accident.
            self.controls[DIALOG_CANCEL].focus(window, cx);
        }
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog = None;
        self.dialog_message = None;
        match self.dialog_return.take() {
            Some(handle) => handle.focus(window, cx),
            None => self.focus.focus(window, cx),
        }
    }

    /// Runs `then` now, or asks first when the schedule has unsaved changes.
    fn guard(&mut self, then: Then, window: &mut Window, cx: &mut Context<Self>) {
        if self.schedule.is_dirty() {
            self.open_dialog(Dialog::Unsaved(then), window, cx);
        } else {
            self.proceed(then, window, cx);
        }
    }

    fn proceed(&mut self, then: Then, window: &mut Window, cx: &mut Context<Self>) {
        match then {
            Then::Open => {
                if self.worker.is_none() {
                    self.schedule_message = Some("No song library open".into());
                } else if self.queue(
                    storage::Command::ScheduleCatalog(None),
                    Request::ScheduleCatalog,
                ) {
                    self.saved_schedules = None;
                    self.open_dialog(Dialog::Open, window, cx);
                }
            }
            Then::New => {
                self.schedule.clear();
                self.selected_entry = None;
                self.schedule_message = None;
            }
            Then::Quit => window.remove_window(),
        }
    }

    fn open_schedule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.guard(Then::Open, window, cx);
    }

    fn save_schedule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.worker.is_none() {
            self.schedule_message = Some("No song library open".into());
            return;
        }
        match self.schedule.title().map(str::to_owned) {
            Some(title) => {
                self.queue_save(title);
            }
            None => self.open_dialog(Dialog::SaveAs, window, cx),
        }
    }

    fn queue_save(&mut self, title: String) -> bool {
        let snapshot = self.schedule.snapshot(&title);
        let command = storage::Command::SaveSchedule(self.schedule.saved(), snapshot.clone());
        let queued = self.queue(command, Request::SaveSchedule(snapshot));
        if queued {
            self.schedule_message = Some("Saving…".into());
        }
        queued
    }

    fn queue(&mut self, command: storage::Command, request: Request) -> bool {
        if self.queued.is_some() || self.schedule_request() {
            self.schedule_message =
                Some("Wait for the schedule to finish saving or opening".into());
            return false;
        }
        self.queued = Some((command, request));
        true
    }

    fn schedule_request(&self) -> bool {
        matches!(
            self.request,
            Some(Request::SaveSchedule(_) | Request::ScheduleCatalog | Request::OpenSchedule(_))
        )
    }

    fn open_saved(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((version, title)) = self
            .saved_schedules
            .as_ref()
            .and_then(|list| list.get(index))
            .cloned()
        else {
            return;
        };
        if self.queue(
            storage::Command::Schedule(version),
            Request::OpenSchedule(version),
        ) {
            self.schedule_message = Some(format!("Opening “{title}”…"));
            self.close_dialog(window, cx);
        }
        cx.notify();
    }

    /// Quit and window-close guard: unsaved schedule changes are confirmed
    /// first, and a save in progress finishes before the window closes.
    pub(super) fn may_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let saving = matches!(self.request, Some(Request::SaveSchedule(_)))
            || matches!(self.queued, Some((_, Request::SaveSchedule(_))));
        if saving {
            self.schedule_message = Some("Saving the schedule… try again when it is saved".into());
        } else if self.schedule.is_dirty() {
            self.open_dialog(Dialog::Unsaved(Then::Quit), window, cx);
        } else {
            return true;
        }
        cx.notify();
        false
    }

    /// Double-click in Songs goes straight to Live (documented), from the
    /// first slide (provisional).
    fn song_to_live(&mut self, index: usize) {
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

    pub(super) fn poll(&mut self, cx: &mut Context<Self>) {
        let mut changed = self.poll_library() | self.poll_images();
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
        changed |= self.sync_show(Instant::now());
        if changed {
            cx.notify();
        }
    }

    fn submit_image_job(&mut self, job: images::Job) {
        let Some(worker) = &self.image_worker else {
            self.media_message = Some("No profile folder; images are unavailable".into());
            return;
        };
        if let Err(error) = worker.submit(job) {
            self.media_message = Some(error.to_string());
        }
    }

    fn poll_images(&mut self) -> bool {
        let mut changed = false;
        let mut rescan = false;
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
                }
                Ok(images::Reply::Images(Ok(names))) => {
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
                    self.media_message = Some(format!("Images unavailable: {error}"));
                }
                Ok(images::Reply::Imported(Ok(name))) => {
                    self.media_message = Some(format!("Imported {name}"));
                    self.selected_image = Some(name);
                    rescan = true;
                }
                Ok(images::Reply::Imported(Err(error))) => {
                    self.media_message = Some(format!("Import failed: {error}"));
                }
                Ok(images::Reply::Logo(Ok(logo))) => {
                    self.logo = logo;
                    self.logo_error = None;
                    self.logo_requested = None;
                    self.logo_sent = None;
                }
                // The previous logo, if any, stays in use.
                Ok(images::Reply::Logo(Err(error))) => {
                    self.logo_error = Some(format!("Logo unavailable: {error}"));
                }
            }
        }
        if rescan {
            self.submit_image_job(images::Job::Scan);
        }
        changed
    }

    fn prompt_import(&mut self, cx: &mut Context<Self>) {
        if self.image_worker.is_none() {
            self.media_message = Some("No profile folder; images cannot be imported".into());
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.submit_image_job(images::Job::Import(path));
                cx.notify();
            });
        })
        .detach();
    }

    fn use_as_logo(&mut self) {
        match self.selected_image.clone() {
            Some(name) => {
                self.media_message = None;
                self.submit_image_job(images::Job::UseAsLogo(name));
            }
            None => self.media_message = Some("Select an image first".into()),
        }
    }

    pub(super) fn toggle_mask(&mut self, mask: Mask) {
        if mask == Mask::Logo && !self.masks.is_on(Mask::Logo) && self.logo.is_none() {
            self.live_message = Some(NO_LOGO.into());
            return;
        }
        self.masks.toggle(mask);
        self.unmask_after_cue = false;
        self.live_message = None;
        self.sync_show(Instant::now());
    }

    /// EW8-OBS-014: a single click on a Live slide applies it. EW8-OBS-016: a
    /// double-click also turns a single mask off; clearing combined masks is
    /// provisional (EW8-OBS-020), as is keeping a mask on a single click.
    fn click_live(&mut self, index: usize, double: bool) {
        if double && self.masks.any() {
            self.masks.live_double_click();
            self.unmask_after_cue = true;
        }
        // The first click of a double-click already sent this slide.
        if !(double && self.live_slide == Some(index)) {
            self.send(index);
        }
        self.sync_show(Instant::now());
    }

    fn logo_ready(&self) -> bool {
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
    fn wire_layer(&self) -> Layer {
        match self.masks.layer() {
            Layer::Logo if !self.logo_ready() => Layer::Black,
            layer => layer,
        }
    }

    pub(super) fn indicator(&self, mask: Mask) -> Indicator {
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

    /// Prepares the logo for the current surface off the UI thread and hands
    /// it to the supervisor. Returns true when visible state changed.
    fn sync_logo(&mut self, now: Instant) -> bool {
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
                        self.logo_error = Some("Logo preparation could not start".into());
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
                    self.logo_error = Some(format!("Logo unavailable: {error}"));
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
                    self.logo_error = Some(format!("Logo unavailable: {error}"));
                }
            }
        }
        changed
    }

    fn sync_show(&mut self, now: Instant) -> bool {
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

    fn poll_library(&mut self) -> bool {
        let Some(worker) = &mut self.worker else {
            return false;
        };
        let mut changed = false;
        let mut live_now = false;
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
                        entry: self.selected_entry,
                        title: song.title.clone(),
                        slides: slides::slides(&song),
                    });
                    self.preview_slide = 0;
                    if self.live_on_load == Some(version) && self.selected_entry.is_none() {
                        self.live_on_load = None;
                        live_now = true;
                    }
                }
                (Some(Request::SaveSchedule(snapshot)), Ok(Reply::Saved(version))) => {
                    self.schedule_message = Some(format!("Saved “{}”", snapshot.title));
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
                                format!("Opened “{title}”")
                            }
                            Err(_) => "The saved schedule could not be read".into(),
                        });
                }
                (Some(Request::SaveSchedule(_)), Err(storage::Error::Conflict)) => {
                    self.schedule_message = Some(
                        "Not saved: this schedule was saved elsewhere since it was opened".into(),
                    );
                }
                (
                    Some(
                        Request::SaveSchedule(_)
                        | Request::ScheduleCatalog
                        | Request::OpenSchedule(_),
                    ),
                    Err(error),
                ) => {
                    self.schedule_message = Some(format!("Schedule storage failed ({error:?})"));
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
                    Err(error) if schedule => {
                        self.schedule_message =
                            Some(format!("Schedule storage failed ({error:?})"));
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
        changed
    }

    fn select_song(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some((version, _)) = self.catalog.get(index) {
            self.selected_song = Some(*version);
            self.selected_entry = None;
            self.live_on_load = None;
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
        // Masks do carry over (EW8-OBS-017); a fresh renderer starts unmasked.
        self.sent_mask = self.output.as_ref().map(|_| Layer::None);
        self.unmask_after_cue = false;
        self.logo_requested = None;
        self.logo_sent = None;
        self.sync_show(Instant::now());
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
        // Bundled-face resolution is pure; installed-family catalog resolution
        // and its loading gate are a separate slice.
        let resolved: Vec<_> = item
            .slides
            .iter()
            .map(|slide| fonts::Resolved::bundled(&slide.format))
            .collect();
        let cap = match self.size_cap {
            Some((v, e, s, cap)) if (v, e, s) == (item.version, extent, self.sizing) => cap,
            _ => {
                let cap = slides::size_cap(&item.slides, &resolved, extent, self.sizing);
                self.size_cap = Some((item.version, extent, self.sizing, cap));
                cap
            }
        };
        let result = slides::cue(version, slide, &resolved[index], extent, caps, cap)
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

    /// Live pane status, top to bottom. Mask lines reflect acknowledged state.
    pub(super) fn live_lines(&self) -> Vec<(String, u32)> {
        let output = self.output.as_ref();
        let connected = matches!(
            output.map(Supervisor::status),
            Some(Status::Connected { .. })
        );
        let sending = self.live_index(output.and_then(|o| o.wanted().or(o.in_flight())));
        let mut lines = vec![(self.output_line(), DARK_MUTED)];
        let slide = match self.on_screen() {
            Some(label) => label.to_owned(),
            None if connected => "nothing confirmed".into(),
            None => "unknown".into(),
        };
        match output.and_then(Supervisor::mask) {
            Some(layer) if layer != Layer::None => {
                lines.push((format!("Mask: {}", layer_name(layer)), DARK_TEXT));
                lines.push((format!("Under mask: {slide}"), DARK_MUTED));
            }
            _ => {
                let color = if self.on_screen().is_some() {
                    DARK_TEXT
                } else {
                    DARK_MUTED
                };
                lines.push((format!("On screen: {slide}"), color));
            }
        }
        if self
            .live
            .as_ref()
            .and_then(|item| item.entry)
            .is_some_and(|id| self.schedule.get(id).is_none())
        {
            lines.push(("Live item is no longer in the schedule".into(), DARK_MUTED));
        }
        if let Some(layer) = output.and_then(Supervisor::mask_pending) {
            lines.push((format!("Sending mask: {}…", layer_name(layer)), SENDING));
        }
        if self.masks.any() && output.is_none() {
            lines.push((
                format!(
                    "Mask armed: {} · shown when Live output is on",
                    layer_name(self.masks.layer())
                ),
                DARK_MUTED,
            ));
        }
        if self.masks.is_on(Mask::Logo) {
            if let Some(error) = &self.logo_error {
                lines.push((error.clone(), DARK_ERROR));
            } else if connected && !self.logo_ready() {
                lines.push(("Preparing logo · covering with Black".into(), SENDING));
            }
        }
        if sending.is_some() {
            lines.push(("Sending…".into(), SENDING));
        }
        if let Some((_, error)) = output.and_then(|o| o.rejected()) {
            lines.push((format!("Last cue not shown: {error}"), DARK_ERROR));
        }
        if let Some((layer, error)) = output.and_then(|o| o.mask_rejected()) {
            lines.push((
                format!("Mask {} not shown: {error}", layer_name(layer)),
                DARK_ERROR,
            ));
        }
        if let Some((_, error)) = output.and_then(|o| o.logo_rejected()) {
            lines.push((format!("Logo not shown: {error}"), DARK_ERROR));
        }
        if let Some(message) = &self.live_message {
            lines.push((message.clone(), DARK_ERROR));
        }
        lines
    }

    fn live_pane(&mut self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let output = self.output.as_ref();
        let confirmed = output.and_then(|o| match o.live() {
            LiveState::Confirmed(version) => Some(version),
            LiveState::Unknown => None,
        });
        let confirmed = self.live_index(confirmed);
        let sending = self.live_index(output.and_then(|o| o.wanted().or(o.in_flight())));
        let lines = self.live_lines();
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
                        let selected =
                            self.selected_song == Some(*version) && self.selected_entry.is_none();
                        let drag = SongDrag {
                            version: *version,
                            title: title.clone(),
                        };
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
                            .on_click(cx.listener(
                                move |this, event: &gpui::ClickEvent, window, cx| {
                                    if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                        return;
                                    }
                                    this.song_rows[index].focus(window, cx);
                                    if event.click_count() >= 2 {
                                        this.song_to_live(index);
                                        cx.notify();
                                    } else {
                                        this.select_song(index, cx);
                                    }
                                },
                            ))
                            .on_drag(drag, |drag, _, _, cx| {
                                cx.new(|_| DragLabel(drag.title.clone().into()))
                            })
                            .child(if title.trim().is_empty() {
                                "Untitled".to_string()
                            } else {
                                title.clone()
                            })
                    }),
            )
            .into_any_element()
    }

    fn schedule_pane(&mut self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.schedule.entries().len();
        while self.schedule_rows.len() < count {
            let index = self.schedule_rows.len() as isize;
            self.schedule_rows.push(
                cx.focus_handle()
                    .tab_index(SCHEDULE_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        let confirmed = self.output.as_ref().and_then(|o| match o.live() {
            LiveState::Confirmed(version) => Some(version),
            LiveState::Unknown => None,
        });
        // Marked only once the renderer acknowledged a slide of this entry.
        let on_screen = self
            .live_index(confirmed)
            .and(self.live.as_ref())
            .and_then(|item| item.entry);
        let header = match self.schedule.title() {
            Some(title) => format!("Schedule · {title}"),
            None => "Schedule".into(),
        };
        let idle = self.selected_entry.is_none();
        let rows = self
            .schedule
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let id = entry.id;
                let title = entry.title.clone();
                let selected = self.selected_entry == Some(id);
                div()
                    .id(("schedule-item", index))
                    .debug_selector(move || format!("schedule-item-{index}"))
                    .track_focus(&self.schedule_rows[index])
                    .key_context("SelaControl")
                    .flex_shrink_0()
                    .h(px(28.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .cursor_pointer()
                    .bg(rgb(if selected { 0xe3e7f3 } else { SURFACE }))
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(0xdce3fa)))
                    .drag_over::<SongDrag>(|s, _, _, _| s.bg(rgb(0xdce3fa)))
                    .drag_over::<EntryDrag>(|s, _, _, _| s.bg(rgb(0xdce3fa)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        window.prevent_default();
                        this.select_entry(id);
                        cx.notify();
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_))
                                || event.is_right_click()
                            {
                                return;
                            }
                            this.schedule_rows[index].focus(window, cx);
                            this.select_entry(id);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                            this.schedule_rows[index].focus(window, cx);
                            this.select_entry(id);
                            this.item_menu = Some((id, event.position));
                            cx.notify();
                        }),
                    )
                    .on_drag(EntryDrag(id), {
                        let title = title.clone();
                        move |_, _, _, cx| cx.new(|_| DragLabel(title.clone().into()))
                    })
                    // Dropping on a row inserts a song before it, or moves the
                    // dragged entry into its place (provisional).
                    .on_drop(cx.listener(move |this, drag: &SongDrag, _, cx| {
                        this.add_entry(index, drag.version, drag.title.clone());
                        cx.notify();
                    }))
                    .on_drop(cx.listener(move |this, drag: &EntryDrag, _, cx| {
                        this.schedule.move_to(drag.0, index);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(18.))
                            .flex_shrink_0()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(format!("{}", index + 1)),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(title))
                    .when(on_screen == Some(id), |d| {
                        d.child(
                            div()
                                .debug_selector(|| "schedule-live-marker".into())
                                .flex_shrink_0()
                                .text_size(px(11.))
                                .text_color(rgb(ON_SCREEN))
                                .child("● Live"),
                        )
                    })
            })
            .collect::<Vec<_>>();
        div()
            .debug_selector(|| "Schedule".into())
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
                    .gap_2()
                    .px_3()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(CHROME))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(header),
                    )
                    .when(self.schedule.is_dirty(), |d| {
                        d.child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(11.))
                                .text_color(rgb(MUTED))
                                .child("Unsaved"),
                        )
                    }),
            )
            .child(
                div()
                    .id("schedule-list")
                    .debug_selector(|| "schedule-list".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .bg(rgb(SURFACE))
                    .drag_over::<SongDrag>(|s, _, _, _| s.bg(rgb(0xeef1fa)))
                    .on_drop(cx.listener(|this, drag: &SongDrag, _, cx| {
                        this.add_entry(usize::MAX, drag.version, drag.title.clone());
                        cx.notify();
                    }))
                    .on_drop(cx.listener(|this, drag: &EntryDrag, _, cx| {
                        this.schedule.move_to(drag.0, usize::MAX);
                        cx.notify();
                    }))
                    .when(rows.is_empty(), |d| {
                        d.items_center()
                            .justify_center()
                            .p_3()
                            .text_color(rgb(MUTED))
                            .text_center()
                            .child("Drag songs here or use Add to Schedule")
                    })
                    .children(rows),
            )
            .children(self.schedule_message.clone().map(|message| {
                div()
                    .debug_selector(|| "schedule-message".into())
                    .flex_shrink_0()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .truncate()
                    .child(message)
            }))
            .child(
                div()
                    .h(px(30.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_1()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(CHROME))
                    .children(
                        [
                            (MOVE_UP, "schedule-up", "Up"),
                            (MOVE_DOWN, "schedule-down", "Down"),
                            (REMOVE_ITEM, "schedule-remove", "Remove"),
                        ]
                        .map(|(index, id, label)| {
                            self.button(index, id, label, cx)
                                .when(idle, |d| d.text_color(rgb(DISABLED)))
                        }),
                    ),
            )
    }

    fn dialog_overlay(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let dialog = self.dialog?;
        let heading = match dialog {
            Dialog::SaveAs => "Save schedule as".to_string(),
            Dialog::Open => "Open schedule".into(),
            Dialog::Unsaved(Then::Open) => "Open another schedule without saving changes?".into(),
            Dialog::Unsaved(Then::New) => "Start a new schedule without saving changes?".into(),
            Dialog::Unsaved(Then::Quit) => "Quit without saving schedule changes?".into(),
            Dialog::Remove(id) => match self.schedule.get(id) {
                Some(entry) => format!("Remove “{}” from the schedule?", entry.title),
                None => "Remove this item from the schedule?".into(),
            },
        };
        let confirm = match dialog {
            Dialog::SaveAs => Some("Save"),
            Dialog::Open => None,
            Dialog::Unsaved(_) => Some("Discard changes"),
            Dialog::Remove(_) => Some("Remove"),
        };
        let body = match dialog {
            Dialog::SaveAs => self.title_input.clone().map(IntoElement::into_any_element),
            Dialog::Open => Some(self.saved_list(cx)),
            _ => None,
        };
        Some(
            div()
                .debug_selector(|| "schedule-dialog".into())
                .occlude()
                .absolute()
                .top(px(44.))
                .left(px(8.))
                .w(px(380.))
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .rounded(px(6.))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(SURFACE))
                .shadow_md()
                // Enter in the title field saves; the field itself ignores it.
                .capture_action(cx.listener(|this, _: &text_input::Enter, window, cx| {
                    if this.dialog == Some(Dialog::SaveAs) {
                        cx.stop_propagation();
                        this.activate(DIALOG_CONFIRM, window, cx);
                    }
                }))
                .child(div().font_weight(FontWeight::MEDIUM).child(heading))
                .children(body)
                .children(self.dialog_message.clone().map(|message| {
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(ERROR))
                        .child(message)
                }))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_1()
                        .children(confirm.map(|label| {
                            self.button(DIALOG_CONFIRM, "dialog-confirm", label, cx)
                                .text_color(rgb(TEXT))
                        }))
                        .child(self.button(
                            DIALOG_CANCEL,
                            "dialog-cancel",
                            if matches!(dialog, Dialog::Remove(_)) {
                                "Keep"
                            } else {
                                "Cancel"
                            },
                            cx,
                        )),
                )
                .into_any_element(),
        )
    }

    fn saved_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(list) = &self.saved_schedules else {
            return div()
                .text_color(rgb(MUTED))
                .child("Loading saved schedules…")
                .into_any_element();
        };
        if list.is_empty() {
            return div()
                .text_color(rgb(MUTED))
                .child("No saved schedules")
                .into_any_element();
        }
        while self.saved_rows.len() < list.len() {
            let index = self.saved_rows.len() as isize;
            self.saved_rows.push(
                cx.focus_handle()
                    .tab_index(SAVED_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        div()
            .id("saved-schedules")
            .max_h(px(240.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .border_1()
            .border_color(rgb(BORDER))
            .children(list.iter().enumerate().map(|(index, (_, title))| {
                div()
                    .id(("saved", index))
                    .debug_selector(move || format!("saved-{index}"))
                    .track_focus(&self.saved_rows[index])
                    .key_context("SelaControl")
                    .flex_shrink_0()
                    .h(px(28.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(0xdce3fa)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        window.prevent_default();
                        this.open_saved(index, window, cx);
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if !matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                this.open_saved(index, window, cx);
                            }
                        }),
                    )
                    .child(div().truncate().child(title.clone()))
            }))
            .into_any_element()
    }

    fn image_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        while self.image_rows.len() < self.images.len() {
            let index = self.image_rows.len() as isize;
            self.image_rows.push(
                cx.focus_handle()
                    .tab_index(IMAGE_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        if self.profile.is_none() {
            return empty_detail(
                "Images · no profile",
                "Launch Sela without arguments to use the default profile.",
            );
        }
        if self.images.is_empty() {
            return empty_detail(
                "Images",
                "No images yet. Use Import image… to add a PNG or JPEG.",
            );
        }
        let logo = self.logo.as_ref().map(|logo| logo.name.as_str());
        div()
            .id("image-list")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(self.images.iter().enumerate().map(|(index, name)| {
                let selected = self.selected_image.as_ref() == Some(name);
                let label = if logo == Some(name.as_str()) {
                    format!("{name} · Logo")
                } else {
                    name.clone()
                };
                let select = move |this: &mut Self| {
                    this.selected_image = this.images.get(index).cloned();
                };
                div()
                    .id(("image", index))
                    .debug_selector(move || format!("image-{index}"))
                    .track_focus(&self.image_rows[index])
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
                        select(this);
                        cx.notify();
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_))
                                || event.is_right_click()
                            {
                                return;
                            }
                            this.image_rows[index].focus(window, cx);
                            select(this);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                            this.image_rows[index].focus(window, cx);
                            select(this);
                            this.image_menu = Some(event.position);
                            cx.notify();
                        }),
                    )
                    .child(div().truncate().child(label))
            }))
            .into_any_element()
    }

    fn mask_button(
        &self,
        index: usize,
        id: &'static str,
        label: &'static str,
        mask: Mask,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let state = self.indicator(mask);
        let disabled = mask == Mask::Logo && self.logo.is_none() && !self.masks.is_on(mask);
        self.button(index, id, label, cx)
            .border_1()
            .border_color(rgb(if state == Indicator::Pending {
                SENDING
            } else {
                CHROME
            }))
            .when(state == Indicator::On, |d| {
                d.bg(rgb(MASK_ON))
                    .text_color(rgb(ON_SCREEN))
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(disabled, |d| d.text_color(rgb(DISABLED)))
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
            .key_context("Sela SelaShow")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Quit, window, cx| {
                if this.may_close(window, cx) {
                    window.remove_window();
                }
            }))
            .on_action(cx.listener(|this, _: &SaveSchedule, window, cx| {
                this.save_schedule(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenSchedule, window, cx| {
                this.open_schedule(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NextScheduleItem, _, cx| {
                this.step_schedule(true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PreviousScheduleItem, _, cx| {
                this.step_schedule(false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &RemoveScheduleItem, _, cx| {
                this.remove_selected();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleBlack, _, cx| {
                this.toggle_mask(Mask::Black);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleLogo, _, cx| {
                this.toggle_mask(Mask::Logo);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleClear, _, cx| {
                this.toggle_mask(Mask::Clear);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &GoLive, _, cx| {
                this.go_live();
                cx.notify();
            }))
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
                    .child(self.button(OPEN_SCHEDULE, "open-schedule", "Open", cx))
                    .child(self.button(SAVE_SCHEDULE, "save-schedule", "Save", cx))
                    .children(
                        ["Web", "Remote"]
                            .map(|label| div().px_1().text_color(rgb(0x92969c)).child(label)),
                    )
                    .child(div().flex_1())
                    .child(
                        self.button(GO_LIVE, "go-live", "Go Live", cx)
                            .text_color(rgb(TEXT)),
                    )
                    .child(
                        div()
                            .debug_selector(|| "Alerts".into())
                            .px_1()
                            .text_color(rgb(0x92969c))
                            .child("Alerts"),
                    )
                    .child(self.mask_button(LOGO, "mask-logo", "Logo", Mask::Logo, cx))
                    .child(self.mask_button(BLACK, "mask-black", "Black", Mask::Black, cx))
                    .child(self.mask_button(CLEAR, "mask-clear", "Clear", Mask::Clear, cx))
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
                    .child(self.schedule_pane(left, cx))
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
                            .child(div().flex_1().min_h_0().child(self.live_pane(right, cx)))
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
                } else if self.tab == 2 {
                    self.image_list(cx)
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
                    let normalized = self.sizing == Sizing::Normalized;
                    d.child(
                        div()
                            .h(px(30.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_1()
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .child(self.button(8, "open-library", "+ New Song", cx))
                            .child(
                                self.button(
                                    ADD_TO_SCHEDULE,
                                    "add-to-schedule",
                                    "Add to Schedule",
                                    cx,
                                )
                                .when(
                                    self.selected_song.is_none() || self.selected_entry.is_some(),
                                    |d| d.text_color(rgb(DISABLED)),
                                ),
                            )
                            .child(div().flex_1())
                            .child(
                                self.button(
                                    NORMALIZE,
                                    "normalize-text",
                                    if normalized {
                                        "Normalize text size across slides: On"
                                    } else {
                                        "Normalize text size across slides: Off"
                                    },
                                    cx,
                                )
                                .when(normalized, |d| d.text_color(rgb(TEXT))),
                            ),
                    )
                })
                .when(self.tab == 2, |d| {
                    d.child(
                        div()
                            .h(px(30.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_1()
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .child(self.button(IMPORT_IMAGE, "import-image", "Import image…", cx))
                            .child(
                                self.button(
                                    USE_AS_LOGO,
                                    "use-as-logo",
                                    "Use As Logo Background",
                                    cx,
                                )
                                .when(self.selected_image.is_none(), |d| {
                                    d.text_color(rgb(DISABLED))
                                }),
                            )
                            .children(self.media_message.clone().map(|message| {
                                div()
                                    .debug_selector(|| "media-message".into())
                                    .px_2()
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED))
                                    .truncate()
                                    .child(message)
                            }))
                            .children(self.logo_error.clone().map(|error| {
                                div()
                                    .px_2()
                                    .text_size(px(11.))
                                    .text_color(rgb(ERROR))
                                    .truncate()
                                    .child(error)
                            })),
                    )
                })
            })
            .when_some(
                self.image_menu.filter(|_| self.tab == 2 && !self.collapsed),
                |d, position| {
                    d.child(
                        div()
                            .occlude()
                            .absolute()
                            .left(position.x)
                            .top(position.y)
                            .bg(rgb(SURFACE))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .shadow_md()
                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                this.image_menu = None;
                                cx.notify();
                            }))
                            .child(self.button(
                                IMAGE_MENU_LOGO,
                                "image-menu-logo",
                                "Use As Logo Background",
                                cx,
                            )),
                    )
                },
            )
            .when(self.new_menu, |d| {
                d.child(
                    div()
                        .occlude()
                        .absolute()
                        .top(px(40.))
                        .left(px(8.))
                        .bg(rgb(SURFACE))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .shadow_md()
                        .child(self.button(10, "new-song-menu", "New Song", cx))
                        .child(self.button(NEW_SCHEDULE, "new-schedule-menu", "New Schedule", cx)),
                )
            })
            .when_some(self.item_menu, |d, (_, position)| {
                d.child(
                    div()
                        .occlude()
                        .absolute()
                        .left(position.x)
                        .top(position.y)
                        .bg(rgb(SURFACE))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .shadow_md()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.item_menu = None;
                            cx.notify();
                        }))
                        .child(self.button(
                            ITEM_MENU_REMOVE,
                            "item-menu-remove",
                            "Remove From Schedule",
                            cx,
                        )),
                )
            })
            .children(self.dialog_overlay(cx))
    }
}
