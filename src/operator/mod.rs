use super::*;
use crate::text_input::{self, TextInput};
use crate::ui::theme::*;
use gpui::{
    CursorStyle, Empty, Entity, Focusable, FontWeight, MouseButton, PathPromptOptions, Pixels,
    Point, Role, SharedString, Subscription, Task, Toggled, anchored, deferred, point,
};
use sela::{
    background::{self, Plan},
    delivery::{Epoch, LiveState},
    fonts,
    i18n::{self, Language, Msg},
    images,
    masks::{Layer, Mask, Masks, UnmaskIntent},
    output::{Launch, Refusal, Status, Supervisor},
    preparation::{
        PreparationEvent, Preparer,
        cue::{CueEvent, CueJob, CuePreparer, Lane, Ticket},
    },
    scene::{ContentVersion, Extent, PrepareError, PreparedBackground, RendererCapabilities},
    schedule::{EntryId, Schedule},
    slides::{self, Sizing, Slide},
    storage::{self, MAX_ITEMS, Reply, Version, Worker},
    t,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

mod assets;
mod controls;
mod dialogs;
mod layout;
mod menus;
mod options;
mod polling;
mod preview_live;
mod resources;
mod schedule;

use assets::{Assets, PREFLIGHT_DIALOG_W};
pub(super) use controls::Control;
use controls::{
    ControlSet, DIALOG_TAB_INDEX, IMAGE_TAB_INDEX, SAVED_TAB_INDEX, SCHEDULE_TAB_INDEX,
    SLIDE_TAB_INDEX, SONG_TAB_INDEX, mark_disabled,
};
pub(super) use dialogs::{Dialog, Then};
pub(crate) use menus::{BarMenu, TopMenu};
use menus::{
    BarMenuLeft, BarMenuRight, MenuConfirm, MenuDismiss, MenuNext, MenuPrevious, OpenEditMenu,
    OpenFileMenu, OpenHelpMenu, OpenLiveMenu, OpenProfilesMenu, OpenSongMenu, OpenViewMenu,
    SongMenu,
};
#[cfg(test)]
pub(super) use menus::{SONG_MENU, SongItem, TOP_MENUS};
use options::SettingsState;
pub(super) use options::{MonitorSlot, SettingsConfig};
use resources::{ClearSongSearch, NextSongRow, PreviousSongRow, SongSearch, TAB_IDS, TABS};

const POLL: Duration = Duration::from_millis(16);
const LABELS: usize = 8;
const LOGO_TIMEOUT: Duration = Duration::from_secs(5);
/// Background prefetch size while Live output is off, so Preview can warn
/// about a missing image before anything goes live.
const NOMINAL_EXTENT: Extent = Extent {
    width: 1920,
    height: 1080,
};

/// Show-control button state. Lit only once the renderer acknowledged it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Indicator {
    Off,
    /// Requested but not yet acknowledged, or acknowledged but being turned off.
    Pending,
    On,
}

fn layer_name(layer: Layer) -> &'static str {
    t!(match layer {
        Layer::None => Msg::LayerNone,
        Layer::Clear => Msg::LayerClear,
        Layer::Black => Msg::LayerBlack,
        Layer::Logo => Msg::LayerLogo,
    })
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
    pub(super) slides: Arc<[Slide]>,
    /// Per-slide face resolution, `None` until the background resolver lands.
    /// Until then the current live scene stays and a deferred cue waits.
    pub(super) resolved: Option<Arc<[fonts::Resolved]>>,
}

/// A live cue being prepared off the UI thread.
pub(super) struct PendingCue {
    pub(super) ticket: Ticket,
    pub(super) index: usize,
    pub(super) item: Version,
    pub(super) generation: u64,
    pub(super) sizing: Sizing,
    pub(super) extent: Extent,
    pub(super) label: String,
    pub(super) warning: Option<String>,
}

/// A cue held back until fonts or a background land. It is submitted only
/// while it is still the newest request for the same live item and session.
#[derive(Clone, Copy, Debug)]
pub(super) struct DeferredSend {
    pub(super) index: usize,
    pub(super) generation: u64,
    pub(super) item: Version,
    pub(super) session: u64,
}

pub(crate) enum Request {
    Open,
    Catalog,
    Search,
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
    pub(super) worker: Option<Worker>,
    request: Option<Request>,
    catalog_stale: bool,
    pub(super) catalog: Vec<(Version, String)>,
    pub(super) search: SongSearch,
    pub(super) search_input: Entity<TextInput>,
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
    /// A Live double-click under a mask, armed for exactly one submission
    /// (`request_generation`). Only that submission's successful Applied
    /// receipt lifts the masks; any newer explicit request, mask toggle, item
    /// change or output toggle cancels it, and failure or substitution kills
    /// it, so no other slide is ever revealed.
    pub(super) unmask: UnmaskIntent,
    /// Submission id of the newest explicit operator cue request: unique,
    /// monotonic, never reused across output sessions and independent of
    /// `ContentVersion`. Retries of a deferred or refreshed request reuse it.
    pub(super) request_generation: u64,
    /// The newest presented cue: (submission, slide, clean outcome).
    presented: Option<(u64, usize, bool)>,
    /// Counts output start/stop cycles; deferred cues never cross one.
    output_session: u64,
    pub(super) logo: Option<images::Logo>,
    logo_preparer: Option<(RendererCapabilities, Preparer)>,
    logo_requested: Option<ContentVersion>,
    logo_sent: Option<ContentVersion>,
    pub(super) logo_error: Option<String>,
    profile: Option<PathBuf>,
    pub(super) settings: SettingsState,
    image_worker: Option<images::Worker>,
    /// Preflight worker and report, Go Live hold, import notes (M1-11b).
    pub(super) assets: Assets,
    pub(super) images: Vec<String>,
    image_rows: Vec<FocusHandle>,
    pub(super) selected_image: Option<String>,
    image_menu: Option<Point<Pixels>>,
    pub(super) media_message: Option<String>,
    /// Not persisted yet.
    pub(super) sizing: Sizing,
    pub(super) size_cap: Option<(Version, Extent, Sizing, Option<u16>)>,
    /// Builds cues on its own thread; created on first use.
    pub(super) cue_preparer: Option<CuePreparer>,
    /// Live cues submitted to cue_preparer, oldest first, bounded by the
    /// preparer's live depth. Their slide and label apply only on landing.
    pub(super) pending_cues: VecDeque<PendingCue>,
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
    /// Open main-menu-bar popup, if any (File/Edit/Live/Profiles/View/Help).
    pub(super) bar_menu: Option<BarMenu>,
    bar_menu_focus: FocusHandle,
    /// Focus to restore when the menu bar closes (EW8-OBS-010 Esc behavior).
    bar_return: Option<FocusHandle>,
    /// Set when a label mouse-down opened a menu; the click completing that
    /// same press must not immediately close it again.
    bar_opened_with_mouse: Option<(TopMenu, Instant)>,
    /// Menu-label bounds from the last prepaint; popups anchor below them.
    bar_bounds: Rc<RefCell<Vec<gpui::Bounds<Pixels>>>>,
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
    pub(super) deferred_send: Option<DeferredSend>,
    /// Slide backgrounds fitted to the output extent (M1-05h).
    pub(super) backgrounds: images::BackgroundCache,
    /// At most one background job with the image worker, so an import still
    /// finds a free slot in its two-job queue.
    pub(super) background_job: Option<images::BackgroundKey>,
    /// One background catalog scan feeding the shared font store.
    _font_scan: Option<Task<()>>,
    _library_changed: Subscription,
    _search_observer: Subscription,
    pub(super) _search_waker: Option<Task<()>>,
    _poller: Option<Task<()>>,
    _receipt_waker: Option<Task<()>>,
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
        let assets = Assets::start(profile.as_deref());
        let mut media_message = None;
        let image_worker = profile.clone().and_then(|profile| {
            let worker = images::Worker::start(profile).ok();
            if worker.is_none() {
                media_message = Some(t!(Msg::ImageWorkerFailed).into());
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
            KeyBinding::new("down", NextSongRow, Some("SelaSongRow")),
            KeyBinding::new("up", PreviousSongRow, Some("SelaSongRow")),
            KeyBinding::new("down", MenuNext, Some("SelaSongMenu")),
            KeyBinding::new("up", MenuPrevious, Some("SelaSongMenu")),
            KeyBinding::new("enter", MenuConfirm, Some("SelaSongMenu")),
            KeyBinding::new("space", MenuConfirm, Some("SelaSongMenu")),
            KeyBinding::new("escape", MenuDismiss, Some("SelaSongMenu")),
            KeyBinding::new("escape", ClearSongSearch, Some("SelaSongSearch")),
        ]);
        // Main menu bar (M1-02/M1-03): Alt+mnemonics open the six menus
        // (EW8-OBS-010; only Alt+L observed, the rest are first-letter best
        // guesses). `!SelaTextInput` keeps plain typing from opening menus.
        cx.bind_keys([
            KeyBinding::new("alt-f", OpenFileMenu, Some("Sela && !SelaTextInput")),
            KeyBinding::new("alt-e", OpenEditMenu, Some("Sela && !SelaTextInput")),
            KeyBinding::new("alt-l", OpenLiveMenu, Some("Sela && !SelaTextInput")),
            KeyBinding::new("alt-p", OpenProfilesMenu, Some("Sela && !SelaTextInput")),
            KeyBinding::new("alt-v", OpenViewMenu, Some("Sela && !SelaTextInput")),
            KeyBinding::new("alt-h", OpenHelpMenu, Some("Sela && !SelaTextInput")),
            KeyBinding::new("down", MenuNext, Some("SelaMenuBar")),
            KeyBinding::new("up", MenuPrevious, Some("SelaMenuBar")),
            KeyBinding::new("tab", MenuNext, Some("SelaMenuBar")),
            KeyBinding::new("shift-tab", MenuPrevious, Some("SelaMenuBar")),
            KeyBinding::new("right", BarMenuRight, Some("SelaMenuBar")),
            KeyBinding::new("left", BarMenuLeft, Some("SelaMenuBar")),
            KeyBinding::new("enter", MenuConfirm, Some("SelaMenuBar")),
            KeyBinding::new("space", MenuConfirm, Some("SelaMenuBar")),
            KeyBinding::new("escape", MenuDismiss, Some("SelaMenuBar")),
        ]);
        // An editor window saved or deleted a song: reload Songs off the UI
        // thread through the storage worker.
        let library_changed =
            cx.observe_global::<crate::song_library::LibraryChanged>(|this, cx| {
                this.catalog_stale = true;
                cx.notify();
            });
        let search_input = cx.new(|cx| {
            TextInput::new(
                "",
                false,
                sela::search::MAX_QUERY_BYTES,
                resources::SEARCH_TAB_INDEX,
                cx,
            )
            .expect("empty text is valid")
        });
        let search_observer = cx.observe(&search_input, |this, _, cx| this.search_text_changed(cx));
        let executor = cx.background_executor().clone();
        let poller = cx.spawn(async move |this, cx| {
            loop {
                executor.timer(POLL).await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        });
        // Renderer receipts poll immediately instead of on the next timer tick.
        let receipt_waker = cx.spawn(async move |this, cx| {
            loop {
                sela::transport::inbound().wait().await;
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
            search: SongSearch::default(),
            search_input,
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
            unmask: UnmaskIntent::default(),
            request_generation: 0,
            presented: None,
            output_session: 0,
            logo: None,
            logo_preparer: None,
            logo_requested: None,
            logo_sent: None,
            logo_error: None,
            profile,
            settings,
            image_worker,
            assets,
            images: Vec::new(),
            image_rows: Vec::new(),
            selected_image: None,
            image_menu: None,
            media_message,
            sizing: Sizing::default(),
            size_cap: None,
            cue_preparer: None,
            pending_cues: VecDeque::new(),
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
            bar_menu: None,
            bar_menu_focus: cx.focus_handle(),
            bar_return: None,
            bar_opened_with_mouse: None,
            bar_bounds: Rc::default(),
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
            _search_observer: search_observer,
            _search_waker: None,
            _poller: Some(poller),
            _receipt_waker: Some(receipt_waker),
        }
    }
}

fn storage_message(error: storage::Error) -> String {
    t!(Msg::LibraryUnavailable, error = format!("{error:?}"))
}

fn shown_title(title: &str) -> String {
    if title.trim().is_empty() {
        t!(Msg::Untitled).to_string()
    } else {
        title.to_string()
    }
}

fn slide_name(index: usize, slide: &Slide) -> String {
    if slide.label.trim().is_empty() {
        t!(Msg::SlideN, index = index + 1)
    } else {
        slide.label.clone()
    }
}
