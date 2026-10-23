use super::*;
use crate::text_input::TextInput;
use crate::ui::theme::*;
use gpui::{
    CursorStyle, DragMoveEvent, Empty, Entity, Focusable, FontWeight, KeyDownEvent, MouseButton,
    PathPromptOptions, Pixels, Point, Role, ScrollHandle, ScrollStrategy, SharedString,
    Subscription, Task, Toggled, UniformListScrollHandle, anchored, deferred, point, uniform_list,
};
use sela::{
    arrangement::VariantId,
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
    schedule::{EntryId, Pin, Schedule},
    schedule_file,
    slides::{self, Sizing, Slide},
    song_order::{SongOrder, SongSort},
    storage::{self, MAX_ITEMS, Reply, Version, Worker},
    t,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

mod assets;
mod autosave;
mod controls;
mod dialogs;
mod frame_log;
mod layout;
mod menus;
mod navigation;
mod options;
mod polling;
mod preview_live;
mod resources;
mod schedule;
mod schedule_files;
pub(super) mod song_import;
mod update_items;

use assets::{Assets, PREFLIGHT_DIALOG_W};
pub(super) use autosave::AutosaveSetup;
pub(super) use controls::Control;
use controls::{
    ControlSet, DIALOG_TAB_INDEX, IMAGE_TAB_INDEX, SCHEDULE_TAB_INDEX, SLIDE_TAB_INDEX,
    SONG_TAB_INDEX, mark_disabled,
};
use dialogs::DismissDialog;
pub(super) use dialogs::{Dialog, Then};
pub(crate) use menus::{BarMenu, TopMenu};
use menus::{
    BarMenuLeft, BarMenuRight, MenuConfirm, MenuDismiss, MenuNext, MenuPrevious, OpenEditMenu,
    OpenFileMenu, OpenHelpMenu, OpenLiveMenu, OpenProfilesMenu, OpenSongMenu, OpenViewMenu,
    SongMenu,
};
use menus::{MenuCloseSub, MenuOpenSub};
#[cfg(test)]
pub(super) use menus::{SONG_MENU, SongItem, TOP_MENUS};
pub(super) use options::{MonitorSlot, SettingsConfig};
use options::{RecentChange, SettingsState};
use resources::{
    ClearSongSearch, DeleteSongRow, ExtendFirstSongRow, ExtendLastSongRow, ExtendNextSongRow,
    ExtendPreviousSongRow, FirstSongRow, LastSongRow, NextSongRow, PageDownSongRow, PageUpSongRow,
    PreviousSongRow, SelectAllSongRows, SongSearch, SongSelection, TAB_IDS, TABS,
};

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
    /// Per slide: its page when the section was split by overflow.
    pub(super) pages: Arc<[Option<slides::Page>]>,
    /// The song's arrangements, for the schedule entry's choice.
    pub(super) arrangements: Arc<[(VariantId, String)]>,
    /// The entry's stored choice these slides were built for.
    pub(super) chosen: Option<VariantId>,
    /// The song's copyright strip (EW8-OBS-063); `None` when the song has no
    /// copyright text. Its text and the slides that draw it follow the
    /// Options › Song copyright settings when each cue is built.
    pub(super) copyright: Option<Arc<slides::CopyrightStrip>>,
    /// Per-slide face resolution, `None` until the background resolver lands.
    /// Until then the current live scene stays and a deferred cue waits.
    pub(super) resolved: Option<Arc<[fonts::Resolved]>>,
    /// The theme the slides were built with (M1-08): its rendered-look key
    /// (0 for the built-in look), whole-value stamp and shown name.
    pub(super) theme_key: u64,
    pub(super) theme_stamp: u64,
    pub(super) theme_name: String,
    /// The song named a theme the library no longer has.
    pub(super) theme_missing: bool,
    /// The library's themes were not loaded (or failed to load) when the
    /// item was built: it shows the default look and captured nothing.
    pub(super) theme_unloaded: bool,
    /// The song's library theme when the item was loaded.
    pub(super) song_theme: Option<sela::theme::ThemeId>,
}

impl Item {
    /// The theme state `Operator::wanted_theme` compares: stamp and notes.
    pub(super) fn theme_view(&self) -> (u64, bool, bool) {
        (self.theme_stamp, self.theme_missing, self.theme_unloaded)
    }

    /// Position of the arrangement these slides follow: the stored choice
    /// when the song has it, else the first.
    pub(super) fn chosen_index(&self) -> usize {
        self.chosen
            .and_then(|id| self.arrangements.iter().position(|(v, _)| *v == id))
            .unwrap_or(0)
    }
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
    /// Held only because the preparer's live lane was full; retried as soon
    /// as it drains, so rapid presses are never dropped.
    pub(super) busy: bool,
}

pub(crate) enum Request {
    Open,
    Catalog,
    Search,
    Song(Version),
    DeleteSong(Version, String),
    /// Reading the songs a schedule file save embeds.
    SaveSchedule(schedule_files::SaveJob),
    /// Resolving an opened schedule file's songs against the library.
    ImportSchedule(PathBuf, Vec<Option<VariantId>>),
    /// Loading the old and new revision of each outdated entry's song;
    /// `versions` names the songs in reply order.
    UpdateScan {
        pairs: Vec<(EntryId, Version, Version)>,
        versions: Vec<Version>,
    },
    /// A song import's duplicate check or commit (`song_import`), for the
    /// import job that asked; a reply for an older job is dropped.
    SongImport(u64),
    /// Loading a restored schedule's songs for the open-time file check
    /// (M1-11b); the schedule document it was for.
    RecoveryCheck(u64),
    /// A theme load or write (M1-08).
    Themes(resources::ThemeJob),
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
    pub(super) new_menu: bool,
    library: Option<PathBuf>,
    pub(super) worker: Option<Worker>,
    request: Option<Request>,
    catalog_stale: bool,
    pub(super) catalog: Vec<(Version, String)>,
    pub(super) search: SongSearch,
    pub(super) search_input: Entity<TextInput>,
    /// Focus handles by row index, created for rows as they first scroll
    /// into view. Only the active visible row is a tab stop.
    pub(super) song_rows: HashMap<usize, FocusHandle>,
    pub(super) song_scroll: UniformListScrollHandle,
    /// Rows built by the last layout of the virtualized Songs list.
    pub(super) song_visible: Range<usize>,
    pub(super) song_selection: SongSelection,
    /// A keyboard move to this row waits for it to be built before focusing.
    pub(super) song_focus_pending: Option<usize>,
    /// Order of the loaded Songs list; a settings change reloads it.
    pub(super) song_order: SongOrder,
    /// Thumb drag of the Songs scrollbar is in progress.
    pub(super) song_scrolling: bool,
    pub(super) frame_log: Option<frame_log::FrameLog>,
    /// Test probe: slide index and copyright strip text of each cue handed
    /// to the renderer, newest last (songs-browser).
    #[cfg(test)]
    pub(super) presented_footers: Vec<(usize, Option<String>)>,
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
    /// File › Import › Songs: worker, staged preview and commit (M1-14).
    pub(super) song_import: song_import::SongImport,
    pub(super) images: Vec<String>,
    pub(super) image_rows: Vec<FocusHandle>,
    pub(super) selected_image: Option<String>,
    pub(super) image_menu: Option<Point<Pixels>>,
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
    /// The focused entry: what Preview shows and Go Live applies.
    pub(super) selected_entry: Option<EntryId>,
    /// Multi-selection, drag and update-review state of the Schedule pane.
    pub(super) view: schedule::ScheduleView,
    pub(super) schedule_rows: Vec<FocusHandle>,
    pub(super) schedule_message: Option<String>,
    /// One schedule storage job waiting for the worker.
    queued: Option<(storage::Command, Request)>,
    /// Next step of a schedule file job; goes to the storage worker ahead of
    /// `queued` so a half-finished open or save is never interleaved.
    followup: Option<(storage::Command, Request)>,
    /// Reads and writes `.selaschedule` files off the UI thread.
    file_worker: Option<schedule_file::Worker>,
    file_job: Option<schedule_files::FileJob>,
    /// A native Open or Save dialog is showing.
    file_prompt: Option<Task<()>>,
    /// Guarded command that continues once a save from the unsaved prompt
    /// lands; started from `render`, which has the window.
    resume: Option<Then>,
    /// Text of the schedule file error dialog.
    pub(super) file_error: Option<String>,
    /// Window title last handed to the platform.
    pub(crate) window_title: String,
    pub(super) dialog: Option<Dialog>,
    dialog_return: Option<FocusHandle>,
    /// Ancestor of every modal dialog control; Tab wraps inside it.
    pub(super) dialog_scope: FocusHandle,
    pub(super) dialog_message: Option<String>,
    pub(super) item_menu: Option<(EntryId, Point<Pixels>)>,
    /// Where keyboard focus returns when the New, Schedule item or image
    /// popup menu closes (SC2-FOCUS-02).
    popup_origin: Option<FocusHandle>,
    /// A pointer press outside the New menu closed it at this position; the
    /// same press on the New button must not reopen it.
    new_menu_closed_at: Option<Point<Pixels>>,
    new_menu_swallow_click: bool,
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
    /// Songs footer status: text and whether it reports a failure.
    pub(super) song_message: Option<(String, bool)>,
    /// Themes tab state and the library's themes (M1-08).
    pub(super) themes: resources::ThemeState,
    /// A library double-click waiting for its song to load.
    live_on_load: Option<Version>,
    /// A PageDown/PageUp to another schedule item waiting for its preview.
    pub(super) live_entry_on_load: Option<EntryId>,
    /// Keyboard focus of the Live pane (context `SelaLivePane`).
    pub(super) live_focus: FocusHandle,
    /// Song version whose face resolution is running on a background thread.
    font_job: Option<Version>,
    /// The in-flight resolution task; cleared when its result lands.
    font_task: Option<Task<()>>,
    /// A Go Live/Next/Previous cue deferred until the live item's fonts or
    /// its slide background land.
    pub(super) deferred_send: Option<DeferredSend>,
    /// The newest cue failed to prepare; cleared by the next request.
    pub(super) cue_failed: bool,
    /// Slide backgrounds fitted to the output extent (M1-05h).
    pub(super) backgrounds: images::BackgroundCache,
    /// At most one background job with the image worker, so an import still
    /// finds a free slot in its two-job queue.
    pub(super) background_job: Option<images::BackgroundKey>,
    /// Decoded tile thumbnails by key; `None` marks one that failed to load.
    pub(super) tile_images: Vec<(images::BackgroundKey, Option<Arc<gpui::RenderImage>>)>,
    /// One background catalog scan feeding the shared font store.
    _font_scan: Option<Task<()>>,
    _library_changed: Subscription,
    _search_observer: Subscription,
    pub(super) _search_waker: Option<Task<()>>,
    _poller: Option<Task<()>>,
    _receipt_waker: Option<Task<()>>,
    /// Schedule autosave and recovery (M1-12).
    pub(super) autosave: autosave::AutosaveState,
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
        let autosave = autosave::AutosaveState::start(profile.as_deref(), cx);
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
            KeyBinding::new("shift-down", ExtendNextSongRow, Some("SelaSongRow")),
            KeyBinding::new("shift-up", ExtendPreviousSongRow, Some("SelaSongRow")),
            KeyBinding::new("home", FirstSongRow, Some("SelaSongRow")),
            KeyBinding::new("ctrl-home", FirstSongRow, Some("SelaSongRow")),
            KeyBinding::new("end", LastSongRow, Some("SelaSongRow")),
            KeyBinding::new("ctrl-end", LastSongRow, Some("SelaSongRow")),
            KeyBinding::new("shift-home", ExtendFirstSongRow, Some("SelaSongRow")),
            KeyBinding::new("shift-end", ExtendLastSongRow, Some("SelaSongRow")),
            // Deeper than the global PageDown (Go Live): in the list it pages.
            KeyBinding::new("pagedown", PageDownSongRow, Some("SelaSongRow")),
            KeyBinding::new("pageup", PageUpSongRow, Some("SelaSongRow")),
            KeyBinding::new("ctrl-a", SelectAllSongRows, Some("SelaSongRow")),
            KeyBinding::new("delete", DeleteSongRow, Some("SelaSongRow")),
            KeyBinding::new("down", MenuNext, Some("SelaSongMenu")),
            KeyBinding::new("up", MenuPrevious, Some("SelaSongMenu")),
            KeyBinding::new("right", MenuOpenSub, Some("SelaSongMenu")),
            KeyBinding::new("left", MenuCloseSub, Some("SelaSongMenu")),
            KeyBinding::new("escape", DismissDialog, Some("SelaDialog")),
            KeyBinding::new("enter", MenuConfirm, Some("SelaSongMenu")),
            KeyBinding::new("space", MenuConfirm, Some("SelaSongMenu")),
            KeyBinding::new("escape", MenuDismiss, Some("SelaSongMenu")),
            // New, Schedule item and image popups (SC2-FOCUS-02).
            KeyBinding::new("down", MenuNext, Some("SelaPopupMenu")),
            KeyBinding::new("up", MenuPrevious, Some("SelaPopupMenu")),
            KeyBinding::new("escape", MenuDismiss, Some("SelaPopupOpen")),
            KeyBinding::new("escape", ClearSongSearch, Some("SelaSongSearch")),
        ]);
        song_import::bind_keys(cx);
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
                this.themes.stale = true;
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
        let schedule_writer = config.schedule_writer.clone();
        let layout = config.initial_layout.unwrap_or_default();
        let settings = SettingsState::start(profile.as_deref(), config, cx);
        Self {
            focus: cx.focus_handle(),
            controls: ControlSet::new(cx),
            ratios: layout.fractions(),
            collapsed: layout.collapsed,
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
            song_rows: HashMap::new(),
            song_scroll: UniformListScrollHandle::new(),
            song_visible: 0..0,
            song_selection: SongSelection::default(),
            song_focus_pending: None,
            song_order: SongOrder::default(),
            song_scrolling: false,
            frame_log: frame_log::FrameLog::from_env(),
            #[cfg(test)]
            presented_footers: Vec::new(),
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
            song_import: Default::default(),
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
            view: Default::default(),
            schedule_rows: Vec::new(),
            schedule_message: None,
            queued: None,
            followup: None,
            file_worker: schedule_file::Worker::start(schedule_writer).ok(),
            file_job: None,
            file_prompt: None,
            resume: None,
            file_error: None,
            window_title: String::new(),
            dialog: None,
            dialog_return: None,
            dialog_scope: cx.focus_handle(),
            dialog_message: None,
            item_menu: None,
            popup_origin: None,
            new_menu_closed_at: None,
            new_menu_swallow_click: false,
            song_menu: None,
            song_menu_focus: cx.focus_handle(),
            bar_menu: None,
            bar_menu_focus: cx.focus_handle(),
            bar_return: None,
            bar_opened_with_mouse: None,
            bar_bounds: Rc::default(),
            song_message: None,
            themes: resources::ThemeState::new(cx),
            live_on_load: None,
            live_entry_on_load: None,
            live_focus: cx.focus_handle(),
            font_job: None,
            font_task: None,
            deferred_send: None,
            cue_failed: false,
            backgrounds: images::BackgroundCache::new(images::BACKGROUND_BUDGET),
            background_job: None,
            tile_images: Vec::new(),
            _font_scan: font_scan,
            _library_changed: library_changed,
            _search_observer: search_observer,
            _search_waker: None,
            _poller: Some(poller),
            _receipt_waker: Some(receipt_waker),
            autosave,
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

/// Tile caption and accessible name: number, label and, for a split section,
/// which page of how many.
fn slide_caption(index: usize, slide: &Slide, page: Option<slides::Page>) -> String {
    let name = slide_name(index, slide);
    match page {
        Some(page) => t!(
            Msg::SlideCaptionPaged,
            index = index + 1,
            name = name,
            number = page.number,
            count = page.of
        ),
        None => format!("{} · {}", index + 1, name),
    }
}

fn slide_name(index: usize, slide: &Slide) -> String {
    if slide.label.trim().is_empty() {
        t!(Msg::SlideN, index = index + 1)
    } else {
        slide.label.clone()
    }
}
