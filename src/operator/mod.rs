use super::*;
use crate::text_input::{self, TextInput};
use crate::ui::theme::*;
use gpui::{
    CursorStyle, Empty, Entity, Focusable, FontWeight, MouseButton, PathPromptOptions, Pixels,
    Point, SharedString, Subscription, Task, anchored, deferred, point,
};
use sela::{
    background::{self, Plan},
    delivery::{Epoch, LiveState},
    fonts, images,
    masks::{Layer, Mask, Masks},
    output::{Launch, Refusal, Status, Supervisor},
    preparation::{PreparationEvent, Preparer},
    scene::{ContentVersion, Extent, PrepareError, PreparedBackground, RendererCapabilities},
    schedule::{EntryId, Schedule},
    slides::{self, Sizing, Slide},
    storage::{self, MAX_ITEMS, Reply, Version, Worker},
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

mod controls;
mod dialogs;
mod layout;
mod menus;
mod options;
mod polling;
mod preview_live;
mod resources;
mod schedule;

pub(super) use controls::Control;
use controls::{
    ControlSet, DIALOG_TAB_INDEX, IMAGE_TAB_INDEX, SAVED_TAB_INDEX, SCHEDULE_TAB_INDEX,
    SLIDE_TAB_INDEX, SONG_TAB_INDEX,
};
pub(super) use dialogs::{Dialog, Then};
use menus::{MenuConfirm, MenuDismiss, MenuNext, MenuPrevious, OpenSongMenu, SongMenu};
#[cfg(test)]
pub(super) use menus::{SONG_MENU, SongItem};
use options::SettingsState;
pub(super) use options::{MonitorSlot, SettingsConfig};
use resources::TABS;

const POLL: Duration = Duration::from_millis(16);
const LABELS: usize = 8;
const LOGO_TIMEOUT: Duration = Duration::from_secs(5);
const NO_LOGO: &str = "No logo set · Media → select an image → Use As Logo Background";
/// Background prefetch size while Live output is off, so Preview can warn
/// about a missing image before anything goes live.
const NOMINAL_EXTENT: Extent = Extent {
    width: 1920,
    height: 1080,
};
const PREPARING_BACKGROUND: &str = "Preparing background…";

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
            .rounded(px(RADIUS))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE))
            .shadow_md()
            .text_size(px(TEXT_SMALL))
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
    /// Per-slide face resolution, `None` until the background resolver lands.
    /// Until then the current live scene stays and a deferred cue waits.
    pub(super) resolved: Option<Arc<[fonts::Resolved]>>,
}

pub(crate) enum Request {
    Open,
    Catalog,
    Song(Version),
    DeleteSong(Version, String),
    SaveSchedule(storage::Schedule),
    ScheduleCatalog,
    OpenSchedule(Version),
}

pub(super) struct Operator {
    pub(super) focus: FocusHandle,
    pub(super) controls: ControlSet,
    pub(super) ratios: [f32; 3],
    pub(super) collapsed: bool,
    pub(super) tab: usize,
    drag: Option<(usize, Point<gpui::Pixels>, f32)>,
    activation: Option<Subscription>,
    pub(super) library_error: Option<String>,
    new_menu: bool,
    library: Option<PathBuf>,
    worker: Option<Worker>,
    request: Option<Request>,
    catalog_stale: bool,
    pub(super) catalog: Vec<(Version, String)>,
    pub(super) song_rows: Vec<FocusHandle>,
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
    pub(super) settings: SettingsState,
    image_worker: Option<images::Worker>,
    pub(super) images: Vec<String>,
    image_rows: Vec<FocusHandle>,
    pub(super) selected_image: Option<String>,
    image_menu: Option<Point<Pixels>>,
    pub(super) media_message: Option<String>,
    /// Not persisted yet.
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
    pub(super) song_menu: Option<SongMenu>,
    song_menu_focus: FocusHandle,
    /// Song row bounds from the last prepaint; a keyboard-opened menu is
    /// placed under its row.
    song_bounds: Rc<RefCell<Vec<gpui::Bounds<Pixels>>>>,
    /// Songs footer status: text and whether it reports a failure.
    pub(super) song_message: Option<(String, bool)>,
    /// A library double-click waiting for its song to load.
    live_on_load: Option<Version>,
    /// Song version whose face resolution is running on a background thread.
    font_job: Option<Version>,
    /// The in-flight resolution task; cleared when its result lands.
    font_task: Option<Task<()>>,
    /// A Go Live/Next/Previous cue deferred until the live item's fonts or
    /// its slide background land.
    deferred_send: Option<usize>,
    /// Slide backgrounds fitted to the output extent (M1-05h).
    pub(super) backgrounds: images::BackgroundCache,
    /// At most one background job with the image worker, so an import still
    /// finds a free slot in its two-job queue.
    pub(super) background_job: Option<images::BackgroundKey>,
    /// One background catalog scan feeding the shared font store.
    _font_scan: Option<Task<()>>,
    _library_changed: Subscription,
    _poller: Option<Task<()>>,
}

impl Operator {
    pub(super) fn with_settings(
        library: Option<PathBuf>,
        launcher: Launcher,
        config: SettingsConfig,
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
        // One background scan feeds the shared catalog that both this
        // operator's face resolution and the song editor's preview renders
        // use. Whoever opens first scans; a second window sees the catalog.
        let font_scan = if fonts::shared().catalog().is_none() {
            let scan = cx
                .background_executor()
                .spawn(async move { fonts::Catalog::scan_system() });
            Some(cx.spawn(async move |this, cx| {
                let catalog = scan.await;
                fonts::shared().install_catalog(catalog);
                let _ = this.update(cx, |this, cx| this.fonts_scanned(cx));
            }))
        } else {
            None
        };
        // Song row and Songs menu keys, deeper than the `SelaShow` bindings
        // so Up/Down move the menu highlight instead of the schedule.
        cx.bind_keys([
            KeyBinding::new("shift-f10", OpenSongMenu, Some("SelaSongRow")),
            KeyBinding::new("menu", OpenSongMenu, Some("SelaSongRow")),
            KeyBinding::new("down", MenuNext, Some("SelaSongMenu")),
            KeyBinding::new("up", MenuPrevious, Some("SelaSongMenu")),
            KeyBinding::new("enter", MenuConfirm, Some("SelaSongMenu")),
            KeyBinding::new("space", MenuConfirm, Some("SelaSongMenu")),
            KeyBinding::new("escape", MenuDismiss, Some("SelaSongMenu")),
        ]);
        // An editor window saved or deleted a song: reload Songs off the UI
        // thread through the storage worker.
        let library_changed =
            cx.observe_global::<crate::song_library::LibraryChanged>(|this, cx| {
                this.catalog_stale = true;
                cx.notify();
            });
        let executor = cx.background_executor().clone();
        let poller = cx.spawn(async move |this, cx| {
            loop {
                executor.timer(POLL).await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        });
        let settings = SettingsState::start(profile.as_deref(), config, cx);
        Self {
            focus: cx.focus_handle(),
            controls: ControlSet::new(cx),
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
            settings,
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
            song_menu: None,
            song_menu_focus: cx.focus_handle(),
            song_bounds: Rc::default(),
            song_message: None,
            live_on_load: None,
            font_job: None,
            font_task: None,
            deferred_send: None,
            backgrounds: images::BackgroundCache::new(images::BACKGROUND_BUDGET),
            background_job: None,
            _font_scan: font_scan,
            _library_changed: library_changed,
            _poller: Some(poller),
        }
    }
}

fn storage_message(error: storage::Error) -> String {
    format!("Song library unavailable ({error:?})")
}

fn shown_title(title: &str) -> &str {
    if title.trim().is_empty() {
        "Untitled"
    } else {
        title
    }
}

fn slide_name(index: usize, slide: &Slide) -> String {
    if slide.label.trim().is_empty() {
        format!("Slide {}", index + 1)
    } else {
        slide.label.clone()
    }
}
