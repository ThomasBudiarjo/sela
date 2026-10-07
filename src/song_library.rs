//! Provisional native authoring window. All durable I/O belongs to storage::Worker.
//! Layout and Words behavior follow EW8-OBS-021..026; see docs/song-library.md.
use crate::{
    ActivateControl, FocusNext, FocusPrevious, Quit,
    text_input::{self, Redo, TextInput, Undo},
};
use gpui::{prelude::*, *};
use sela::arrangement::{Occurrence, OccurrenceId, SectionId};
use sela::background::{Aspect, ImageRef, Plan, plan};
use sela::images::Substitute;
use sela::scene::{ContentVersion, Extent, PreparedBackground, RendererCapabilities};
use sela::storage::{Command, Error, Id, Reply, Section, Song, Version, Worker};
use std::{
    collections::{HashMap, VecDeque},
    ops::Range,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

mod format_pane;

actions!(song_library, [Save, SplitSection, SelectAllSlides]);

/// Call after `text_input::bind_keys`: at equal context depth the later
/// binding wins, so Ctrl+A in a Words cell selects every slide
/// (EW8-OBS-034) while other fields keep their own Select All.
pub fn bind_keys(cx: &mut App) {
    format_pane::bind_keys(cx);
    for modifier in ["ctrl", "cmd"] {
        let keys = format!("{modifier}-a");
        cx.bind_keys([
            KeyBinding::new(&keys, SelectAllSlides, Some("SongWords > SelaTextInput")),
            KeyBinding::new(&keys, SelectAllSlides, Some("SongWords")),
        ]);
    }
}

pub fn default_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")));
    base.map(|p| p.join("sela/library.sqlite"))
}

/// Bumped whenever an editor window commits a save or a delete, so other
/// windows (the operator's Songs list) reload their catalog from storage.
#[derive(Default)]
pub struct LibraryChanged(pub u64);

impl Global for LibraryChanged {}

fn library_changed(cx: &mut App) {
    cx.default_global::<LibraryChanged>().0 += 1;
}

pub fn open(path: PathBuf, cx: &mut App) -> Result<(), String> {
    open_song(path, None, cx)
}

/// Opens an editor window; with `version`, it loads that saved revision once
/// its own storage worker has opened the library (never on the UI thread).
pub fn open_song(path: PathBuf, version: Option<Version>, cx: &mut App) -> Result<(), String> {
    let bounds = Bounds::centered(None, size(px(1180.), px(740.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(760.), px(460.))),
            titlebar: Some(TitlebarOptions {
                title: Some(window_title("").into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| {
                let mut library = Library::new(path, cx);
                library.open_version = version;
                library
            });
            let weak = view.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                weak.update(cx, |view, cx| view.may_close(cx))
                    .unwrap_or(true)
            });
            // EW8-OBS-023: a new song starts with the caret in slide 1's label.
            view.update(cx, |view, cx| view.focus_cell(0, 0, 0, window, cx));
            view
        },
    )
    .map_err(|_| "Cannot open song library window".to_string())?;
    Ok(())
}

fn window_title(title: &str) -> String {
    let title = title.trim();
    format!(
        "Song Editor - {}",
        if title.is_empty() { "Untitled" } else { title }
    )
}

enum Pending {
    Open,
    Catalog,
    Load(Version),
    Save(Song),
    Delete,
}

fn blank() -> Song {
    Song {
        title: String::new(),
        authors: String::new(),
        copyright: String::new(),
        license: String::new(),
        variants: Vec::new(),
        sections: vec![Section {
            id: SectionId::allocate(),
            label: String::new(),
            lyrics: String::new(),
            format: Default::default(),
            background: None,
        }],
        master: None,
    }
}

const HISTORY_LIMIT: usize = 64;
const HISTORY_BYTES: usize = 8 * 1024 * 1024;
const LABEL: usize = 0;
const LYRICS: usize = 1;
/// Preview raster size. The audience fits text per extent, so this matches
/// the live layout proportionally at a quarter of 1080p's pixels.
const PREVIEW: Extent = Extent {
    width: 1280,
    height: 720,
};
/// Layouts tab canvas text (EW shows sample lyrics there, EW8-OBS-043).
const LAYOUT_SAMPLE: &str = "Sample text line one\nSample text line two\nSample text line three";
/// EW8-OBS-024: the Slides tab narrows the left pane to a thumbnail column.
const SLIDES_PANE: f32 = 264.;
const THUMBNAIL_WIDTH: f32 = 204.;
const THUMBNAIL_SCALE: u32 = 4;
/// Slides-tab thumbnail raster: 225 KiB each, at most one per slide (128)
/// plus one landing render, so the cache stays under 30 MiB.
const THUMBNAIL: Extent = Extent {
    width: PREVIEW.width / THUMBNAIL_SCALE,
    height: PREVIEW.height / THUMBNAIL_SCALE,
};

#[derive(Clone)]
struct Document {
    song: Song,
    section: usize,
}
impl Document {
    fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.song.title.len()
            + self.song.authors.len()
            + self.song.copyright.len()
            + self.song.license.len()
            + self
                .song
                .variants
                .iter()
                .map(|v| {
                    std::mem::size_of_val(v)
                        + v.name.len()
                        + v.occurrences.len() * std::mem::size_of::<sela::arrangement::Occurrence>()
                })
                .sum::<usize>()
            + self
                .song
                .sections
                .iter()
                .map(|s| {
                    std::mem::size_of::<Section>()
                        + s.label.len()
                        + s.lyrics.len()
                        + s.format.font.as_ref().map_or(0, String::len)
                        + s.background
                            .as_ref()
                            .and_then(|b| b.image_ref())
                            .map_or(0, |i| i.name.len())
                })
                .sum::<usize>()
    }
}
#[derive(Default)]
struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
}
impl History {
    fn trim(&mut self) {
        while self.undo.len() + self.redo.len() > HISTORY_LIMIT
            || self
                .undo
                .iter()
                .chain(&self.redo)
                .map(Document::bytes)
                .sum::<usize>()
                > HISTORY_BYTES
        {
            if !self.undo.is_empty() {
                self.undo.remove(0);
            } else if !self.redo.is_empty() {
                self.redo.remove(0);
            } else {
                break;
            }
        }
    }
    fn record(&mut self, before: Document) {
        self.redo.clear();
        self.undo.push(before);
        self.trim();
    }
    fn step(&mut self, current: Document, redo: bool) -> Option<Document> {
        let next = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        }?;
        if redo {
            self.undo.push(current);
        } else {
            self.redo.push(current);
        }
        self.trim();
        Some(next)
    }
}

/// Label-kind palette: EW8-OBS-024 hues as the ink, with light tints for
/// Sela's light finish. Unknown labels use the Verse hue, as observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Kind {
    ink: u32,
    bar: u32,
    border: u32,
}
const VERSE: Kind = Kind {
    ink: 0x314686,
    bar: 0xe3e8f5,
    border: 0xb7c2e3,
};
fn kind(label: &str) -> Kind {
    let label = label.trim().to_lowercase();
    let starts = |words: &[&str]| words.iter().any(|w| label.starts_with(w));
    if starts(&["chorus", "pre-chorus", "pre chorus", "prechorus"]) {
        Kind {
            ink: 0x5b3146,
            bar: 0xf3e5eb,
            border: 0xdbbccb,
        }
    } else if starts(&["bridge", "tag"]) {
        Kind {
            ink: 0x511c51,
            bar: 0xf0e2f0,
            border: 0xd5b6d5,
        }
    } else if starts(&["ending", "end"]) {
        Kind {
            ink: 0x511c1c,
            bar: 0xf4e2e2,
            border: 0xdbb8b8,
        }
    } else if starts(&["intro"]) {
        Kind {
            ink: 0x315b46,
            bar: 0xe1f1e8,
            border: 0xb5d8c5,
        }
    } else {
        VERSE
    }
}

/// EW8-OBS-022: a labeled slide starts a group; unlabeled slides join the
/// preceding one. Sela derives groups from labels only, so an unlabeled first
/// slide still forms the first group.
fn groups<'a>(labels: impl IntoIterator<Item = &'a str>) -> Vec<Range<usize>> {
    let mut groups: Vec<Range<usize>> = Vec::new();
    for (i, label) in labels.into_iter().enumerate() {
        match groups.last_mut() {
            Some(group) if label.is_empty() => group.end = i + 1,
            _ => groups.push(i..i + 1),
        }
    }
    groups
}

/// Renders one slide exactly as the audience preparer would, off the UI
/// thread: resolved faces (installed, or the bundled fallback), fitted size,
/// styled layers over the cue background. The image is `None` when the
/// audience would reject the text (overflow, missing glyph, unfittable fixed
/// size); the warning is set when a named family fell back to the bundled
/// face.
fn render_preview(slide: &sela::slides::Slide, backdrops: &Backdrops) -> Preview {
    let resolved = sela::fonts::shared().resolve(&slide.format);
    let (background, substituted) = backdrops.prepare(slide);
    let raster = pixels(slide, &resolved, background);
    let warning = [substituted.as_deref(), resolved.warning()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    Preview {
        key: slide_key(slide),
        fitted: raster.as_ref().and_then(|(_, size)| *size),
        image: raster.and_then(|(bgra, _)| image_from(&bgra, PREVIEW)),
        warning: (!warning.is_empty()).then_some(warning),
    }
}

/// The preview raster box-filtered down by `THUMBNAIL_SCALE`, so thumbnails
/// keep the audience proportions (the text inset is fixed in pixels, so
/// fitting at thumbnail size directly would lay out differently).
fn render_thumbnail(
    slide: &sela::slides::Slide,
    backdrops: &Backdrops,
) -> Option<Arc<RenderImage>> {
    let resolved = sela::fonts::shared().resolve(&slide.format);
    let (background, _) = backdrops.prepare(slide);
    let (bgra, _) = pixels(slide, &resolved, background)?;
    let (scale, width) = (THUMBNAIL_SCALE as usize, PREVIEW.width as usize);
    let n = (scale * scale) as u32;
    let mut small = Vec::with_capacity(bgra.len() / (scale * scale));
    for y in 0..THUMBNAIL.height as usize {
        for x in 0..THUMBNAIL.width as usize {
            let mut sums = [0u32; 4];
            for dy in 0..scale {
                let row = ((y * scale + dy) * width + x * scale) * 4;
                for pixel in bgra[row..row + scale * 4].as_chunks::<4>().0 {
                    for (sum, byte) in sums.iter_mut().zip(pixel) {
                        *sum += u32::from(*byte);
                    }
                }
            }
            small.extend(sums.map(|sum| ((sum + n / 2) / n) as u8));
        }
    }
    image_from(&small, THUMBNAIL)
}

/// Editor backgrounds fitted at `PREVIEW` size, by image and aspect. Four
/// entries (about 14 MiB) cover a song alternating a few images without
/// decoding again per thumbnail.
const BACKDROPS: usize = 4;

type Backdrop = ((ImageRef, Aspect), Arc<[u8]>);

/// Background preparation for previews and thumbnails, shared by their
/// background tasks; the UI thread never locks it.
struct Backdrops {
    profile: Option<PathBuf>,
    /// Least recently fitted first.
    fitted: Mutex<VecDeque<Backdrop>>,
}

impl Backdrops {
    fn new(profile: Option<PathBuf>) -> Self {
        Self {
            profile,
            fitted: Mutex::new(VecDeque::new()),
        }
    }

    /// The slide's prepared background, and the substitution warning when its
    /// image cannot be shown (black, as the audience would). Reads and
    /// decodes files: off the UI thread only.
    fn prepare(&self, slide: &sela::slides::Slide) -> (PreparedBackground, Option<String>) {
        let (image, aspect) = match plan(slide.background.as_ref()) {
            Plan::Color(rgb) => return (sela::slides::color_background(rgb), None),
            Plan::Image(image, aspect) => (image, aspect),
        };
        let key = (image.clone(), aspect);
        let cached = {
            let fitted = self.fitted.lock().unwrap_or_else(|e| e.into_inner());
            fitted
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        };
        let fitted = match cached {
            Some(rgba) => Ok(rgba),
            None => self
                .profile
                .as_deref()
                .ok_or(Substitute::Missing)
                .and_then(|profile| {
                    sela::images::fitted_background(profile, image, aspect, PREVIEW)
                })
                .inspect(|rgba| {
                    let mut fitted = self.fitted.lock().unwrap_or_else(|e| e.into_inner());
                    fitted.retain(|(k, _)| *k != key);
                    fitted.push_back((key.clone(), rgba.clone()));
                    while fitted.len() > BACKDROPS {
                        fitted.pop_front();
                    }
                }),
        };
        match fitted {
            Ok(rgba) => (
                PreparedBackground::Image {
                    version: sela::images::background_version(image, aspect),
                    extent: PREVIEW,
                    rgba,
                },
                None,
            ),
            Err(why) => (
                sela::slides::color_background(sela::background::BLACK),
                Some(why.warning(&image.name)),
            ),
        }
    }
}

/// One slide's preview pixels, as BGRA: the cue's fill, outline and shadow
/// coverage layers blended in linear light over the background color or
/// fitted image, the CPU twin of the audience compositor's shader. Also
/// returns the laid-out font size in 1080-reference px (`None` without text).
fn pixels(
    slide: &sela::slides::Slide,
    resolved: &sela::fonts::Resolved,
    background: PreparedBackground,
) -> Option<(Vec<u8>, Option<u16>)> {
    let cue = sela::slides::cue(
        ContentVersion { id: 0, revision: 0 },
        slide,
        resolved,
        PREVIEW,
        RendererCapabilities {
            max_texture_dimension: 4096,
        },
        None,
        background,
    )
    .ok()?;
    let coverage = crate::audience::text::layers(&cue).ok()?.coverage();
    let blend = cue
        .text()
        .map_or(crate::audience::compositor::Blend::plain(), |text| {
            crate::audience::compositor::Blend::from_style(&text.style())
        });
    let size = cue.text().map(|text| {
        (f32::from(text.font_size()) * sela::slides::REFERENCE_HEIGHT as f32
            / PREVIEW.height as f32)
            .round() as u16
    });
    match *cue.background() {
        PreparedBackground::Color(background) => Some((
            crate::audience::compositor::blend_pixels(background, &coverage, &blend),
            size,
        )),
        PreparedBackground::Image { ref rgba, .. } => Some((
            crate::audience::compositor::blend_over(rgba, &coverage, &blend)?,
            size,
        )),
    }
}

type SlideKey = (
    String,
    sela::format::SlideFormat,
    Option<sela::background::Background>,
);

/// Preview and thumbnail cache key: two slides with the same text but
/// different formats or backgrounds render differently.
fn slide_key(slide: &sela::slides::Slide) -> SlideKey {
    (
        slide.text.clone(),
        slide.format.clone(),
        slide.background.clone(),
    )
}

fn image_from(bgra: &[u8], extent: Extent) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(extent.width, extent.height, bgra.to_vec())?;
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AllKey {
    Copy,
    Cut,
    Paste,
    Delete,
    Enter,
}

#[derive(Clone, Copy)]
enum Nav {
    Up,
    Down,
    Enter,
    Back,
}

/// One finished preview: the slide it rendered (text + format key), its
/// raster (`None` when the audience would reject the slide) and the fallback
/// warning from face resolution, if any.
struct Preview {
    key: SlideKey,
    /// Laid-out size in 1080-reference px, the start of "Do not auto size
    /// text" (EW8-OBS-028).
    fitted: Option<u16>,
    image: Option<Arc<RenderImage>>,
    warning: Option<String>,
}

pub(crate) struct Library {
    focus: FocusHandle,
    /// Title, authors, copyright, license.
    fields: [Entity<TextInput>; 4],
    field_edits: [u64; 4],
    /// Label and lyrics cell per section; always parallel to `draft.sections`.
    cells: Vec<[Entity<TextInput>; 2]>,
    buttons: [FocusHandle; 20],
    subscriptions: Vec<Subscription>,
    task: Option<Task<()>>,
    worker: Option<Worker>,
    pending: Option<Pending>,
    catalog: Vec<(Version, String)>,
    row_focus: Vec<FocusHandle>,
    cursor: Option<Id>,
    version: Option<Version>,
    draft: Song,
    baseline: Song,
    history: History,
    section: usize,
    status: String,
    confirm_close: bool,
    confirm_delete: bool,
    inspector: bool,
    show_catalog: bool,
    slides: bool,
    close_after_save: bool,
    committed_close: bool,
    section_focus: [FocusHandle; 128],
    words_scroll: ScrollHandle,
    /// Latest finished preview (`None` image = rejected).
    preview: Option<Preview>,
    /// At most one raster in flight; a newer slide starts when it lands.
    preview_task: Option<Task<()>>,
    /// Slides-tab thumbnails by slide key (text + format, `None` =
    /// rejected), pruned to the current slides.
    thumbnails: HashMap<SlideKey, Option<Arc<RenderImage>>>,
    /// One thumbnail renders at a time, in slide order.
    thumbnail_task: Option<Task<()>>,
    /// Slide background images from the library's profile folder.
    backdrops: Arc<Backdrops>,
    title: String,
    /// The toolbar's Format toggle docks the pane right of the preview.
    format: bool,
    pane: format_pane::Pane,
    /// Ctrl+A whole-song selection: every cell's text is selected, format
    /// changes apply to every slide and typing replaces the song text.
    all: bool,
    /// The cell holding the caret when `all` began; leaving it ends `all`.
    all_focus: Option<(usize, usize)>,
    /// Edit Slide Layouts: the Layouts tab shows sample text over the song
    /// master, and the Slide pane edits the master (EW8-OBS-043).
    layouts: bool,
    /// A saved revision to load once the catalog has loaded (Edit Song…).
    open_version: Option<Version>,
}

impl Library {
    fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        // Provisional document shortcuts, scoped to this window; field actions bubble.
        for modifier in ["ctrl", "cmd"] {
            cx.bind_keys([
                KeyBinding::new(&format!("{modifier}-z"), Undo, Some("SongLibrary")),
                KeyBinding::new(&format!("{modifier}-shift-z"), Redo, Some("SongLibrary")),
            ]);
        }
        let fields = std::array::from_fn(|i| {
            let owner = cx.weak_entity();
            cx.new(|cx| {
                let mut input =
                    TextInput::new("", false, [1024, 4096, 4096, 4096][i], 20 + i as isize, cx)
                        .expect("empty text is valid");
                if i == 0 {
                    input.set_placeholder("Title");
                }
                input.use_document_history(move |text, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.field_edit(i, text);
                        cx.notify();
                    });
                });
                input
            })
        });
        // The profile is the library's folder, as for the operator.
        let profile = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(PathBuf::from);
        let (worker, pending, status) = match Worker::open(path) {
            Ok(worker) => (Some(worker), Some(Pending::Open), "Opening library…".into()),
            Err(e) => (None, None, error_message(e).into()),
        };
        let initial = blank();
        let mut this = Self {
            focus: cx.focus_handle(),
            fields,
            field_edits: [0; 4],
            cells: Vec::new(),
            buttons: std::array::from_fn(|i| {
                cx.focus_handle().tab_stop(true).tab_index(i as isize + 1)
            }),
            subscriptions: Vec::new(),
            task: None,
            worker,
            pending,
            catalog: Vec::new(),
            row_focus: Vec::new(),
            cursor: None,
            version: None,
            draft: initial.clone(),
            baseline: initial,
            history: History::default(),
            section: 0,
            status,
            confirm_close: false,
            confirm_delete: false,
            inspector: false,
            show_catalog: false,
            slides: false,
            close_after_save: false,
            committed_close: false,
            section_focus: std::array::from_fn(|i| {
                cx.focus_handle().tab_stop(true).tab_index(300 + i as isize)
            }),
            words_scroll: ScrollHandle::new(),
            preview: None,
            preview_task: None,
            thumbnails: HashMap::new(),
            thumbnail_task: None,
            backdrops: Arc::new(Backdrops::new(profile)),
            title: String::new(),
            format: false,
            pane: format_pane::Pane::new(cx),
            all: false,
            all_focus: None,
            layouts: false,
            open_version: None,
        };
        this.load_fields(cx);
        this.sync_input_lock(cx);
        for (index, field) in this.fields.iter().enumerate() {
            this.subscriptions
                .push(cx.observe(field, move |this, field, cx| {
                    let count = field.read(cx).edit_count();
                    if this.field_edits[index] != count {
                        this.field_edits[index] = count;
                        cx.notify();
                    }
                }));
        }
        let executor = cx.background_executor().clone();
        this.task = Some(cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(40)).await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        }));
        // One background scan feeds the shared catalog that this editor's
        // preview renders resolve against; the operator scans too, and
        // whichever window opens first wins.
        if sela::fonts::shared().catalog().is_none() {
            let scan = cx
                .background_executor()
                .spawn(async move { sela::fonts::Catalog::scan_system() });
            cx.spawn(async move |this, cx| {
                let catalog = scan.await;
                sela::fonts::shared().install_catalog(catalog);
                let _ = this.update(cx, |this, cx| this.fonts_scanned(cx));
            })
            .detach();
        }
        this
    }

    fn cell(index: usize, locked: bool, cx: &mut Context<Self>) -> [Entity<TextInput>; 2] {
        std::array::from_fn(|part| {
            let owner = cx.weak_entity();
            cx.new(|cx| {
                let mut input = TextInput::new(
                    "",
                    part == LYRICS,
                    [256, 256 * 1024][part],
                    400 + (2 * index + part) as isize,
                    cx,
                )
                .expect("empty text is valid");
                input.set_flow(part == LABEL);
                // EW8-OBS-022 placeholders.
                input.set_placeholder(["label", "song"][part]);
                input.set_read_only(locked);
                let typing = owner.clone();
                input.use_document_history(move |text, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.cell_edit(index, part, text);
                        cx.notify();
                    });
                });
                // During a whole-song selection, typing over the selection
                // replaces the song text (owner-reported EW behavior). The
                // replacement reloads cells, so it runs after this one.
                let me = cx.entity_id();
                input.intercept_typing(move |text, composing, whole, window, cx| {
                    let Some(this) = typing.upgrade() else {
                        return false;
                    };
                    let (all, removed) = {
                        let this = this.read(cx);
                        let cells = this.cells.iter().flatten();
                        (this.all, !cells.map(Entity::entity_id).any(|id| id == me))
                    };
                    if !all && !removed {
                        return false;
                    }
                    let (owner, text) = (typing.clone(), text.to_owned());
                    window.defer(cx, move |w, cx| {
                        if removed {
                            // The platform keeps sending keys to a removed
                            // cell until the next paint; they belong to the
                            // cell that has focus now. A composition restarts
                            // there, so its updates are dropped.
                            let target = owner.upgrade().filter(|_| !composing).and_then(|this| {
                                let this = this.read(cx);
                                let (i, p) = this.focused_cell(w, cx)?;
                                Some(this.cells[i][p].clone())
                            });
                            if let Some(cell) = target {
                                cell.update(cx, |c, cx| {
                                    c.replace_text_in_range(None, &text, w, cx)
                                });
                            }
                            return;
                        }
                        let _ = owner.update(cx, |this, cx| {
                            if !whole {
                                this.exit_all(cx);
                            } else if composing {
                                // The IME keeps composing into the emptied
                                // first slide; its commit is the next step.
                                this.replace_all("", w, cx);
                            } else {
                                this.replace_all(&text, w, cx);
                            }
                        });
                    });
                    removed || whole
                });
                input
            })
        })
    }

    fn current(&self, cx: &App) -> Song {
        let mut song = self.draft.clone();
        song.title = self.fields[0].read(cx).text().into();
        song.authors = self.fields[1].read(cx).text().into();
        song.copyright = self.fields[2].read(cx).text().into();
        song.license = self.fields[3].read(cx).text().into();
        for (section, cell) in song.sections.iter_mut().zip(&self.cells) {
            section.label = cell[LABEL].read(cx).text().into();
            section.lyrics = cell[LYRICS].read(cx).text().into();
        }
        song
    }
    fn dirty(&self, cx: &App) -> bool {
        self.current(cx) != self.baseline
    }
    fn locked(&self) -> bool {
        self.pending.is_some() || self.committed_close
    }
    fn clear_rejection(&mut self) {
        if self.status.starts_with("A title is required")
            || self.status.starts_with("An input was rejected")
            || self.status.starts_with("Song is invalid")
            || self.status.ends_with("Slide unchanged.")
        {
            self.status = "Draft changed · validate with Apply or OK".into();
        }
    }
    // Called synchronously by the field. Never read the borrowed field here.
    fn field_edit(&mut self, index: usize, text: &str) {
        let mut song = self.draft.clone();
        *[
            &mut song.title,
            &mut song.authors,
            &mut song.copyright,
            &mut song.license,
        ][index] = text.into();
        self.clear_rejection();
        self.record(song);
    }
    // Called synchronously by the cell. Never read the borrowed cell here.
    fn cell_edit(&mut self, index: usize, part: usize, text: &str) {
        let mut song = self.draft.clone();
        let Some(section) = song.sections.get_mut(index) else {
            return;
        };
        *if part == LABEL {
            &mut section.label
        } else {
            &mut section.lyrics
        } = text.into();
        self.section = index;
        self.clear_rejection();
        self.record(song);
    }
    fn record(&mut self, song: Song) {
        if song != self.draft {
            self.history.record(Document {
                song: self.draft.clone(),
                section: self.section,
            });
            self.draft = song;
            self.confirm_delete = false;
        }
    }
    fn history(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        self.drag_end(cx);
        self.exit_all(cx);
        self.record(self.current(cx));
        if let Some(next) = self.history.step(
            Document {
                song: self.draft.clone(),
                section: self.section,
            },
            redo,
        ) {
            let focused = self.focused_cell(window, cx);
            let thumbnail = self.section_focus.iter().position(|f| f.is_focused(window));
            self.draft = next.song;
            self.section = next.section;
            self.confirm_delete = false;
            self.load_fields(cx);
            self.pane.reload();
            // The caret follows the restored slide instead of staying in a
            // cell that now holds a different slide.
            if focused.is_some_and(|(i, _)| i != self.section)
                || thumbnail.is_some_and(|i| i != self.section)
            {
                let index = self.section;
                self.focus_cell(index, LYRICS, usize::MAX, window, cx);
            }
            self.status = if redo {
                "Document redone"
            } else {
                "Document undone"
            }
            .into();
            cx.notify();
        }
    }
    fn may_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.pending.is_some() {
            self.status = "Wait for the pending operation before closing.".into();
            cx.notify();
            return false;
        }
        if self.dirty(cx) {
            self.confirm_close = true;
            self.status = "Unsaved changes. Save, discard, or keep editing.".into();
            cx.notify();
            return false;
        }
        true
    }
    fn may_replace(&mut self, cx: &mut Context<Self>) -> bool {
        if self.locked() {
            return false;
        }
        if self.dirty(cx) {
            self.status = "Save or discard your edits before changing songs.".into();
            cx.notify();
            return false;
        }
        true
    }
    /// Brings every input in line with `draft`. Cells stay index-parallel to
    /// sections; an unchanged input keeps its caret.
    fn load_fields(&mut self, cx: &mut Context<Self>) {
        fn load(input: &Entity<TextInput>, value: &str, cx: &mut App) {
            if input.read(cx).text() != value || input.read(cx).error().is_some() {
                let caret = input.read(cx).selection().end;
                input.update(cx, |input, cx| {
                    input.set_text(value, cx).expect("validated editable song");
                    input.set_cursor(caret, cx);
                });
            }
        }
        for (field, value) in self.fields.iter().zip([
            &self.draft.title,
            &self.draft.authors,
            &self.draft.copyright,
            &self.draft.license,
        ]) {
            load(field, value, cx);
        }
        let locked = self.locked();
        self.cells.truncate(self.draft.sections.len());
        while self.cells.len() < self.draft.sections.len() {
            let cell = Self::cell(self.cells.len(), locked, cx);
            self.cells.push(cell);
        }
        for (cell, section) in self.cells.iter().zip(&self.draft.sections) {
            load(&cell[LABEL], &section.label, cx);
            load(&cell[LYRICS], &section.lyrics, cx);
        }
        self.section = self
            .section
            .min(self.draft.sections.len().saturating_sub(1));
    }
    fn begin(&mut self, song: Song, version: Option<Version>, cx: &mut Context<Self>) {
        self.exit_all(cx);
        self.layouts = false;
        self.pane.drag = None;
        self.pane.reload();
        self.draft = song.clone();
        self.baseline = song;
        self.history = History::default();
        self.version = version;
        self.section = 0;
        self.confirm_delete = false;
        self.confirm_close = false;
        self.load_fields(cx);
        cx.notify();
    }
    fn sync_input_lock(&self, cx: &mut Context<Self>) {
        let locked = self.locked();
        for field in self.fields.iter().chain(self.cells.iter().flatten()) {
            field.update(cx, |f, _| f.set_read_only(locked));
        }
        // Unlocking leaves disabled pane fields to the next `sync_pane`.
        if locked {
            for field in self.pane.inputs() {
                field.update(cx, |f, _| f.set_read_only(true));
            }
        }
    }
    fn submit(&mut self, command: Command, pending: Pending, cx: &mut Context<Self>) {
        if self.pending.is_some() {
            return;
        }
        let result = self
            .worker
            .as_mut()
            .ok_or(Error::Closed)
            .and_then(|w| w.submit(command));
        match result {
            Ok(_) => {
                self.pending = Some(pending);
                self.status = "Working…".into();
            }
            Err(e) => self.status = error_message(e).into(),
        }
        self.sync_input_lock(cx);
        cx.notify();
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.submit(Command::Catalog(self.cursor), Pending::Catalog, cx);
    }
    fn select(&mut self, version: Version, cx: &mut Context<Self>) {
        if self.may_replace(cx) {
            self.submit(Command::Song(version), Pending::Load(version), cx);
        }
    }
    fn poll(&mut self, cx: &mut Context<Self>) {
        let Some(reply) = self.worker.as_mut().and_then(Worker::poll) else {
            return;
        };
        let pending = self.pending.take();
        match (pending, reply) {
            (Some(Pending::Open), Ok(Reply::Opened)) => self.refresh(cx),
            (Some(Pending::Catalog), Ok(Reply::Catalog(items))) => {
                self.row_focus = (0..items.len())
                    .map(|i| cx.focus_handle().tab_stop(true).tab_index(100 + i as isize))
                    .collect();
                self.catalog = items;
                self.status =
                    "Library ready · committed changes stay offline on this computer".into();
                if let Some(version) = self.open_version.take() {
                    self.select(version, cx);
                }
            }
            (Some(Pending::Load(v)), Ok(Reply::Song(song))) => {
                // Storage can retain documents outside this provisional field policy.
                // Never partially load or silently alter such a document.
                if editable(&song) {
                    self.begin(song, Some(v), cx);
                    self.status = "Loaded saved revision".into();
                } else {
                    self.status = "This song exceeds the editor's line limits or has multiline metadata. Original unchanged.".into();
                }
            }
            (Some(Pending::Save(song)), Ok(Reply::Saved(v))) => {
                // Save/duplicate advance only the durable baseline and identity.
                self.baseline = song;
                self.version = Some(v);
                self.confirm_delete = false;
                self.confirm_close = false;
                self.cursor = None;
                library_changed(cx);
                if self.close_after_save {
                    self.committed_close = !self.dirty(cx);
                    self.close_after_save = false;
                    if !self.committed_close {
                        self.status = "Saved revision; newer edits remain unsaved.".into();
                    }
                } else {
                    self.refresh(cx);
                }
            }
            (Some(Pending::Delete), Ok(Reply::Deleted)) => {
                library_changed(cx);
                self.begin(blank(), None, cx);
                self.cursor = None;
                self.refresh(cx);
            }
            (_, Err(e)) => {
                self.close_after_save = false;
                self.status = error_message(e).into();
            }
            _ => self.status = "Unexpected storage reply. Close and reopen the library.".into(),
        }
        self.sync_input_lock(cx);
        cx.notify();
    }
    fn action(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        self.record(self.current(cx));
        match index {
            12 => {
                self.inspector = !self.inspector;
                self.show_catalog = false;
            }
            13 => self.show_catalog = !self.show_catalog,
            18 => {
                self.format = !self.format;
                if !self.format {
                    self.pane.menu = None;
                }
            }
            // EW8-OBS-036: back to Words ends the whole-song selection.
            14 => {
                self.exit_all(cx);
                self.layouts = false;
                self.slides = false;
            }
            17 => {
                self.layouts = false;
                self.slides = true;
            }
            19 => self.edit_layouts(false, window, cx),
            15 => {
                self.action(1, window, cx);
                self.close_after_save = matches!(self.pending, Some(Pending::Save(_)));
            }
            16 => {
                if self.may_close(cx) {
                    window.remove_window();
                }
            }
            0 if self.may_replace(cx) => {
                self.begin(blank(), None, cx);
                self.status = "New song · not saved".into();
                self.focus_cell(0, LABEL, 0, window, cx);
            }
            1 | 2 => {
                let song = self.current(cx);
                if self
                    .fields
                    .iter()
                    .chain(self.cells.iter().flatten())
                    .any(|field| field.read(cx).error().is_some())
                {
                    self.status =
                        "An input was rejected. Correct the highlighted field before saving."
                            .into();
                } else if song.validate().is_err() {
                    self.status =
                        "A title is required; the complete song must fit within 256 KiB.".into();
                } else {
                    let previous = if index == 2 { None } else { self.version };
                    self.submit(
                        Command::SaveSong(previous, song.clone()),
                        Pending::Save(song),
                        cx,
                    );
                }
            }
            3 if self.version.is_some() && !self.dirty(cx) => {
                if self.confirm_delete {
                    self.submit(
                        Command::DeleteSong(self.version.unwrap()),
                        Pending::Delete,
                        cx,
                    );
                } else {
                    self.confirm_delete = true;
                    self.status = "Click Confirm delete to remove this library entry. Schedule snapshots remain.".into();
                }
            }
            4 => {
                self.begin(self.baseline.clone(), self.version, cx);
                self.status = "Edits discarded".into();
            }
            5 | 6 => self.history(index == 6, window, cx),
            7 | 8 => self.restructure(index == 7, window, cx),
            9 if self.may_replace(cx) => {
                self.cursor = if self.catalog.len() == 128 {
                    self.catalog.last().map(|(v, _)| v.id)
                } else {
                    None
                };
                self.refresh(cx);
            }
            10 if self.confirm_close => window.remove_window(),
            11 => {
                self.confirm_close = false;
                self.confirm_delete = false;
                self.status = "Continue editing".into();
            }
            _ => self.status = "Save or discard edits before deleting a saved song.".into(),
        }
        cx.notify();
    }

    /// `+` appends an empty unlabeled slide and puts the caret in its label
    /// (EW8-OBS-023); `−` removes the current slide (Sela control, no EW
    /// counterpart observed). Atomic and one undo step.
    fn restructure(&mut self, add: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.exit_all(cx);
        let song = self.current(cx);
        let mut candidate = song.clone();
        let mut section = self.section;
        if add {
            if candidate.sections.len() >= 128 {
                self.status = "At most 128 slides. Original unchanged.".into();
                return;
            }
            candidate.sections.push(Section {
                id: SectionId::allocate(),
                label: String::new(),
                lyrics: String::new(),
                format: Default::default(),
                background: None,
            });
            section = candidate.sections.len() - 1;
        } else {
            if candidate.sections.is_empty() {
                return;
            }
            candidate.sections.remove(section);
            section = section.min(candidate.sections.len().saturating_sub(1));
        }
        if !valid_draft(&candidate) {
            self.status =
                "Slide is used by an arrangement or the song exceeds limits. Original unchanged."
                    .into();
            return;
        }
        self.history.record(Document {
            song,
            section: self.section,
        });
        self.confirm_delete = false;
        self.draft = candidate;
        self.section = section;
        self.load_fields(cx);
        if add {
            self.focus_cell(section, LABEL, 0, window, cx);
        }
    }

    /// Ctrl+Enter in the lyrics: the text from the caret on becomes a new
    /// unlabeled slide right after this one, the newline just before the
    /// caret is dropped and the caret moves to the new slide's start
    /// (EW8-OBS-023). Arrangements get the new section after every
    /// occurrence of the split one, so no lyrics leave the output. One undo
    /// step restores the original.
    fn split_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        self.exit_all(cx);
        let Some(index) = self
            .focused_cell(window, cx)
            .filter(|(_, part)| *part == LYRICS && !self.slides)
            .map(|(i, _)| i)
        else {
            self.status = "Place the cursor in the lyrics to split the slide.".into();
            cx.notify();
            return;
        };
        let at = self.cells[index][LYRICS].read(cx).selection().start;
        let song = self.current(cx);
        self.record(song.clone());
        let section = &song.sections[index];
        let (head, tail) = section.lyrics.split_at(at);
        let head = head
            .strip_suffix('\n')
            .map_or(head, |h| h.strip_suffix('\r').unwrap_or(h));
        let new = Section {
            id: SectionId::allocate(),
            label: String::new(),
            lyrics: tail.into(),
            format: Default::default(),
            background: None,
        };
        let mut candidate = song.clone();
        candidate.sections[index].lyrics = head.into();
        candidate.sections.insert(index + 1, new.clone());
        for variant in &mut candidate.variants {
            let mut occurrences = Vec::with_capacity(variant.occurrences.len() + 1);
            for occurrence in &variant.occurrences {
                occurrences.push(*occurrence);
                if occurrence.section == section.id {
                    occurrences.push(Occurrence {
                        id: OccurrenceId(SectionId::allocate().0),
                        section: new.id,
                    });
                }
            }
            variant.occurrences = occurrences;
        }
        if !valid_draft(&candidate) {
            self.status =
                "Splitting would exceed the song's section limits. Original unchanged.".into();
            cx.notify();
            return;
        }
        self.history.record(Document {
            song,
            section: index,
        });
        self.confirm_delete = false;
        self.draft = candidate;
        self.section = index + 1;
        self.load_fields(cx);
        self.focus_cell(index + 1, LYRICS, 0, window, cx);
        self.status = "Slide split · Undo restores it".into();
        cx.notify();
    }

    /// Backspace at the start of an unlabeled slide joins it to the previous
    /// slide, the inverse of Ctrl+Enter. Provisional: EW's merge is unobserved.
    /// Its occurrences leave every arrangement; one undo step restores them.
    fn merge(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.exit_all(cx);
        let song = self.current(cx);
        self.record(song.clone());
        let mut candidate = song.clone();
        let removed = candidate.sections.remove(index);
        let previous = &mut candidate.sections[index - 1];
        let caret = previous.lyrics.len();
        if !removed.lyrics.is_empty() {
            if !previous.lyrics.is_empty() {
                previous.lyrics.push('\n');
            }
            previous.lyrics.push_str(&removed.lyrics);
        }
        for variant in &mut candidate.variants {
            variant.occurrences.retain(|o| o.section != removed.id);
        }
        if !valid_draft(&candidate) || !editable(&candidate) {
            self.status = "Joining would exceed the slide limits. Original unchanged.".into();
            cx.notify();
            return;
        }
        self.history.record(Document {
            song,
            section: index,
        });
        self.confirm_delete = false;
        self.draft = candidate;
        self.section = index - 1;
        self.load_fields(cx);
        self.focus_cell(index - 1, LYRICS, caret, window, cx);
        self.status = "Slides joined · Undo restores them".into();
        cx.notify();
    }

    /// Ctrl+A in Words or Slides: every label and lyrics cell selected, the
    /// caret at the end of the last slide (EW8-OBS-034); format changes then
    /// apply to every slide (EW8-OBS-035).
    fn select_all_slides(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked() || self.layouts || self.cells.is_empty() {
            return;
        }
        // EW shows the Slide pane while every slide is selected
        // (EW8-OBS-035); Sela keeps the Text tab reachable so text formats
        // still apply to every slide (owner-reported).
        self.pane.tab = format_pane::Tab::Slide;
        self.pane.menu = None;
        let last = self.cells.len() - 1;
        if !self.slides {
            self.focus_cell(last, LYRICS, usize::MAX, window, cx);
        }
        for cell in self.cells.iter().flatten() {
            cell.update(cx, |c, cx| c.select_all(cx));
        }
        self.all = true;
        self.all_focus = (!self.slides).then_some((last, LYRICS));
        self.status = "All slides selected · formatting applies to every slide".into();
        cx.notify();
    }

    /// Ends the whole-song selection; cells still wholly selected collapse
    /// to their end, a cell the operator already moved in keeps its caret.
    fn exit_all(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.all) {
            return;
        }
        self.all_focus = None;
        for cell in self.cells.iter().flatten() {
            let (selection, len) = {
                let c = cell.read(cx);
                (c.selection(), c.text().len())
            };
            if len > 0 && selection == (0..len) {
                cell.update(cx, |c, cx| c.set_cursor(len, cx));
            }
        }
        cx.notify();
    }

    /// The song as plain text: each slide's label line (if any) above its
    /// lyrics, slides separated by a blank line.
    fn song_text(&self, cx: &App) -> String {
        self.current(cx)
            .sections
            .iter()
            .map(|s| match s.label.as_str() {
                "" => s.lyrics.clone(),
                label => format!("{label}\n{}", s.lyrics),
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Typing, pasting or deleting over the whole-song selection: the song
    /// becomes one unlabeled slide holding `text`, keeping slide 1's
    /// identity, format and background; arrangements keep only that slide. One undo
    /// step restores the original. EW's exact result is unobserved.
    fn replace_all(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked() || !self.all {
            return;
        }
        self.exit_all(cx);
        let song = self.current(cx);
        let (id, format, background) = song.sections.first().map_or_else(
            || (SectionId::allocate(), Default::default(), None),
            |s| (s.id, s.format.clone(), s.background.clone()),
        );
        let mut candidate = song.clone();
        candidate.sections = vec![Section {
            id,
            label: String::new(),
            lyrics: text.into(),
            format,
            background,
        }];
        for variant in &mut candidate.variants {
            variant.occurrences.retain(|o| o.section == id);
        }
        if !valid_draft(&candidate) || !editable(&candidate) {
            self.status = "That text does not fit in one slide. Song unchanged.".into();
            cx.notify();
            return;
        }
        self.history.record(Document {
            song,
            section: self.section,
        });
        self.confirm_delete = false;
        self.draft = candidate;
        self.section = 0;
        self.load_fields(cx);
        self.pane.reload();
        self.focus_cell(0, LYRICS, usize::MAX, window, cx);
        self.status = "Song text replaced · Undo restores it".into();
        cx.notify();
    }

    /// Key edits a cell would apply to its own selection apply to the whole
    /// song instead while it is selected; otherwise they reach the cell.
    fn all_key(&mut self, edit: AllKey, window: &mut Window, cx: &mut Context<Self>) {
        if !self.all || self.locked() {
            return;
        }
        cx.stop_propagation();
        let text = match edit {
            AllKey::Copy | AllKey::Cut => {
                cx.write_to_clipboard(ClipboardItem::new_string(self.song_text(cx)));
                if edit == AllKey::Copy {
                    return;
                }
                String::new()
            }
            AllKey::Delete => String::new(),
            AllKey::Enter => "\n".into(),
            AllKey::Paste => match cx.read_from_clipboard().and_then(|c| c.text()) {
                Some(text) => text,
                None => return,
            },
        };
        self.replace_all(&text, window, cx);
    }

    /// Cell-to-cell movement for keys the cell did not consume. Down/Up cross
    /// label and lyrics (EW8-OBS-023); Enter in a label goes to its lyrics
    /// (provisional, EW8-OBS-026).
    fn navigate(
        &mut self,
        index: usize,
        part: usize,
        nav: Nav,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.locked() || index >= self.cells.len() {
            return;
        }
        let unlabeled = self.cells[index][LABEL].read(cx).text().is_empty();
        match (part, nav) {
            (LABEL, Nav::Down | Nav::Enter) => self.focus_cell(index, LYRICS, 0, w, cx),
            (LABEL, Nav::Up) if index > 0 => self.focus_cell(index - 1, LYRICS, usize::MAX, w, cx),
            (LYRICS, Nav::Up) => self.focus_cell(index, LABEL, usize::MAX, w, cx),
            (LYRICS, Nav::Down) if index + 1 < self.cells.len() => {
                self.focus_cell(index + 1, LABEL, 0, w, cx)
            }
            (_, Nav::Back) if index > 0 && unlabeled => self.merge(index, w, cx),
            (LYRICS, Nav::Back) => self.focus_cell(index, LABEL, usize::MAX, w, cx),
            _ => {}
        }
    }

    fn focused_cell(&self, window: &Window, cx: &App) -> Option<(usize, usize)> {
        self.cells.iter().enumerate().find_map(|(i, cell)| {
            cell.iter()
                .position(|c| c.read(cx).focus_handle(cx).is_focused(window))
                .map(|part| (i, part))
        })
    }

    /// Caret at `byte` (clamped) in one cell; that slide becomes current. In
    /// the Slides tab the cells are not rendered, so its thumbnail takes focus
    /// instead and shortcuts keep reaching the editor.
    fn focus_cell(
        &mut self,
        index: usize,
        part: usize,
        byte: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(cell) = self.cells.get(index).map(|c| c[part].clone()) else {
            return;
        };
        cell.update(cx, |c, cx| c.set_cursor(byte, cx));
        if self.slides {
            self.section_focus[index].focus(window, cx);
            self.section = index;
            cx.notify();
            return;
        }
        cell.read(cx).focus_handle(cx).focus(window, cx);
        self.section = index;
        let group = (1..=index)
            .filter(|i| !self.cells[*i][LABEL].read(cx).text().is_empty())
            .count();
        self.words_scroll.scroll_to_item(group);
        cx.notify();
    }

    fn select_section(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked() || index >= self.draft.sections.len() {
            return;
        }
        self.exit_all(cx);
        if self.slides {
            self.section = index;
            cx.notify();
        } else {
            self.focus_cell(index, LYRICS, usize::MAX, window, cx);
        }
    }

    /// The current slide as the audience would show it: the caret cell's text
    /// with the section's stored format and resolved background.
    fn preview_slide(&self, cx: &App) -> Option<sela::slides::Slide> {
        if self.layouts {
            return Some(sela::slides::section_slide(
                &Section {
                    id: SectionId(Id([0; 16])),
                    label: String::new(),
                    lyrics: LAYOUT_SAMPLE.into(),
                    format: Default::default(),
                    background: None,
                },
                self.draft.master.as_ref(),
            ));
        }
        let cell = self.cells.get(self.section)?;
        let stored = self.draft.sections.get(self.section);
        let section = Section {
            id: SectionId(Id([0; 16])),
            label: String::new(),
            lyrics: cell[LYRICS].read(cx).text().into(),
            format: stored.map_or_else(Default::default, |s| s.format.clone()),
            background: stored.and_then(|s| s.background.clone()),
        };
        Some(sela::slides::section_slide(
            &section,
            self.draft.master.as_ref(),
        ))
    }

    /// Latest-wins preview preparation off the UI thread.
    fn ensure_preview(&mut self, cx: &mut Context<Self>) {
        let Some(slide) = self.preview_slide(cx) else {
            return;
        };
        let key = slide_key(&slide);
        if self.preview_task.is_some() || self.preview.as_ref().is_some_and(|p| p.key == key) {
            return;
        }
        let backdrops = self.backdrops.clone();
        let raster = cx
            .background_executor()
            .spawn(async move { render_preview(&slide, &backdrops) });
        self.preview_task = Some(cx.spawn(async move |this, cx| {
            let preview = raster.await;
            let _ = this.update(cx, |this, cx| {
                this.preview_task = None;
                if let Some(Preview {
                    image: Some(old), ..
                }) = this.preview.replace(preview)
                {
                    cx.drop_image(old, None);
                }
                cx.notify();
            });
        }));
    }

    /// The shared catalog landed: previews rendered with the bundled fallback
    /// are stale, so drop them and render again with resolved faces.
    fn fonts_scanned(&mut self, cx: &mut Context<Self>) {
        if let Some(Preview {
            image: Some(old), ..
        }) = self.preview.take()
        {
            cx.drop_image(old, None);
        }
        self.preview = None;
        self.thumbnails.retain(|_, image| {
            if let Some(old) = image.take() {
                cx.drop_image(old, None);
            }
            false
        });
        if !self.locked() {
            self.ensure_preview(cx);
            self.ensure_thumbnails(cx);
        }
        cx.notify();
    }
    /// Fills the Slides-tab thumbnail cache off the UI thread, one slide at a
    /// time, and drops thumbnails whose key no longer appears in the draft.
    fn ensure_thumbnails(&mut self, cx: &mut Context<Self>) {
        if self.thumbnails.is_empty() && (!self.slides || self.locked()) {
            return;
        }
        let keys: Vec<_> = self
            .draft
            .sections
            .iter()
            .map(|s| slide_key(&sela::slides::section_slide(s, self.draft.master.as_ref())))
            .collect();
        let stale: Vec<_> = self
            .thumbnails
            .keys()
            .filter(|k| !keys.contains(k))
            .cloned()
            .collect();
        for key in stale {
            if let Some(Some(old)) = self.thumbnails.remove(&key) {
                cx.drop_image(old, None);
            }
        }
        if !self.slides || self.locked() || self.thumbnail_task.is_some() {
            return;
        }
        let Some(slide) = self
            .draft
            .sections
            .iter()
            .map(|s| sela::slides::section_slide(s, self.draft.master.as_ref()))
            .find(|slide| !self.thumbnails.contains_key(&slide_key(slide)))
        else {
            return;
        };
        let key = slide_key(&slide);
        let backdrops = self.backdrops.clone();
        let raster = cx
            .background_executor()
            .spawn(async move { render_thumbnail(&slide, &backdrops) });
        self.thumbnail_task = Some(cx.spawn(async move |this, cx| {
            let image = raster.await;
            let _ = this.update(cx, |this, cx| {
                this.thumbnail_task = None;
                if let Some(Some(old)) = this.thumbnails.insert(key, image) {
                    cx.drop_image(old, None);
                }
                cx.notify();
            });
        }));
    }

    /// The Layouts tab list: the theme selector stands for the song until
    /// themes exist (M1-08), and the one layout card is the Master
    /// (EW8-OBS-043), shown with the master's thumbnail.
    fn layout_list(&self) -> Div {
        let master = sela::slides::section_slide(
            &Section {
                id: SectionId(Id([0; 16])),
                label: String::new(),
                lyrics: LAYOUT_SAMPLE.into(),
                format: Default::default(),
                background: None,
            },
            self.draft.master.as_ref(),
        );
        let thumbnail = self
            .preview
            .as_ref()
            .filter(|p| p.key == slide_key(&master))
            .and_then(|p| p.image.clone());
        div()
            .flex_1()
            .min_h_0()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .px_1()
                    .text_size(px(12.))
                    .text_color(rgb(0x646971))
                    .child("Song master · until themes exist"),
            )
            .child(
                div()
                    .debug_selector(|| "layout-master".into())
                    .w(px(THUMBNAIL_WIDTH))
                    .rounded(px(4.))
                    .overflow_hidden()
                    .border_2()
                    .border_color(rgb(0x536aca))
                    .child(
                        div()
                            .h(px(THUMBNAIL_WIDTH * 9. / 16.))
                            .bg(rgb(0x000000))
                            .children(thumbnail.map(|t| img(t).size_full())),
                    )
                    .child(
                        div()
                            .h(px(24.))
                            .px_2()
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .bg(rgb(0xdce3fa))
                            .text_color(rgb(0x2f4f99))
                            .child("Master"),
                    ),
            )
    }

    fn thumbnail_row(&self, index: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let section = &self.draft.sections[index];
        let selected = self.all || index == self.section;
        let labeled = !section.label.is_empty();
        let kind = kind(&section.label);
        let thumbnail = self.thumbnails.get(&slide_key(&sela::slides::section_slide(
            section,
            self.draft.master.as_ref(),
        )));
        div()
            .id(("draft-section", index))
            .track_focus(&self.section_focus[index])
            .key_context("SelaControl")
            .flex_shrink_0()
            .flex()
            .gap_1()
            .p_1()
            .border_2()
            .rounded(px(6.))
            .border_color(rgb(if selected { 0x536aca } else { 0xfbfbfa }))
            .when(selected, |d| d.bg(rgb(0xdce3fa)))
            .focus(|d| d.border_color(rgb(0x8fa1e0)))
            .cursor_pointer()
            .on_action(cx.listener(move |s, _: &ActivateControl, w, cx| {
                w.prevent_default();
                s.select_section(index, w, cx);
            }))
            .on_click(cx.listener(move |s, event, w, cx| {
                if matches!(event, ClickEvent::Keyboard(_)) {
                    return;
                }
                s.section_focus[index].focus(w, cx);
                s.select_section(index, w, cx);
            }))
            .child(
                div()
                    .w(px(20.))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .text_size(px(12.))
                    .text_color(rgb(if selected { 0x2f4f99 } else { 0x7a7f86 }))
                    .child((index + 1).to_string()),
            )
            .child(
                div()
                    .w(px(THUMBNAIL_WIDTH))
                    .flex_shrink_0()
                    .rounded(px(4.))
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(if labeled { kind.border } else { 0xdcdedc }))
                    .child(
                        div()
                            .relative()
                            .h(px(THUMBNAIL_WIDTH * 9. / 16.))
                            .bg(rgb(0x000000))
                            .children(
                                thumbnail
                                    .and_then(|t| t.clone())
                                    .map(|t| img(t).size_full()),
                            )
                            .when(matches!(thumbnail, Some(None)), |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .p_2()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(11.))
                                        .text_color(rgb(0x9a9ea5))
                                        .child("Cannot be shown"),
                                )
                            }),
                    )
                    // EW8-OBS-024 caption bar: label-kind fill, or grey
                    // italic "Slide N" when unlabeled.
                    .child(
                        div()
                            .h(px(24.))
                            .px_2()
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .map(|d| {
                                if labeled {
                                    d.bg(rgb(kind.bar))
                                        .text_color(rgb(kind.ink))
                                        .child(section.label.clone())
                                } else {
                                    d.bg(rgb(0xeeeeec))
                                        .text_color(rgb(0x8a8f96))
                                        .italic()
                                        .child(format!("Slide {}", index + 1))
                                }
                            }),
                    ),
            )
    }

    fn control(
        &self,
        index: usize,
        label: &str,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !enabled {
            return inert(label).into_any_element();
        }
        div()
            .id(("library-button", index))
            .track_focus(&self.buttons[index])
            .key_context("SelaControl")
            .flex_shrink_0()
            .px_2()
            .py_1()
            .rounded(px(4.))
            .text_size(px(12.))
            .cursor_pointer()
            .hover(|d| d.bg(rgb(0xe8e9e7)))
            .focus(|d| d.bg(rgb(0xdce3fa)))
            .on_action(cx.listener(move |s, _: &ActivateControl, w, cx| {
                w.prevent_default();
                s.action(index, w, cx);
            }))
            .on_click(cx.listener(move |s, e, w, cx| {
                if matches!(e, ClickEvent::Keyboard(_)) {
                    return;
                }
                s.buttons[index].focus(w, cx);
                s.action(index, w, cx);
            }))
            .child(label.to_owned())
            .into_any_element()
    }
    fn button(&self, index: usize, label: &str, cx: &mut Context<Self>) -> AnyElement {
        self.control(index, label, true, cx)
    }
    fn framed(&self, index: usize, label: &str, enabled: bool, cx: &mut Context<Self>) -> Div {
        div()
            .flex_shrink_0()
            .min_w(px(76.))
            .border_1()
            .rounded(px(4.))
            .border_color(rgb(if enabled { 0xcbd0d6 } else { 0xe1e3e5 }))
            .bg(rgb(0xffffff))
            .flex()
            .justify_center()
            .child(self.control(index, label, enabled, cx))
    }

    /// During a whole-song selection, cell edit keys act on the song (they
    /// are captured before the focused cell sees them) and caret movement or
    /// a click ends the selection.
    fn whole_song_keys(&self, list: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        fn edit<A: Action>(
            list: Stateful<Div>,
            key: AllKey,
            cx: &mut Context<Library>,
        ) -> Stateful<Div> {
            list.capture_action(cx.listener(move |s, _: &A, w, cx| s.all_key(key, w, cx)))
        }
        fn exit<A: Action>(list: Stateful<Div>, cx: &mut Context<Library>) -> Stateful<Div> {
            list.capture_action(cx.listener(|s, _: &A, _, cx| s.exit_all(cx)))
        }
        let list = edit::<text_input::Copy>(list, AllKey::Copy, cx);
        let list = edit::<text_input::Cut>(list, AllKey::Cut, cx);
        let list = edit::<text_input::Paste>(list, AllKey::Paste, cx);
        let list = edit::<text_input::Delete>(list, AllKey::Delete, cx);
        let list = edit::<text_input::Backspace>(list, AllKey::Delete, cx);
        let list = edit::<text_input::Enter>(list, AllKey::Enter, cx);
        let list = exit::<text_input::Left>(list, cx);
        let list = exit::<text_input::Right>(list, cx);
        let list = exit::<text_input::Up>(list, cx);
        let list = exit::<text_input::Down>(list, cx);
        let list = exit::<text_input::SelectLeft>(list, cx);
        let list = exit::<text_input::SelectRight>(list, cx);
        let list = exit::<text_input::SelectUp>(list, cx);
        let list = exit::<text_input::SelectDown>(list, cx);
        let list = exit::<text_input::Home>(list, cx);
        let list = exit::<text_input::End>(list, cx);
        list.capture_any_mouse_down(cx.listener(|s, _, _, cx| s.exit_all(cx)))
    }

    fn slide_row(&self, index: usize, kind: Kind, cx: &mut Context<Self>) -> Div {
        let selected = self.all || index == self.section;
        let labeled = !self.cells[index][LABEL].read(cx).text().is_empty();
        let nav = |part: usize, cx: &mut Context<Self>| {
            div()
                .on_action(cx.listener(move |s, _: &text_input::Up, w, cx| {
                    s.navigate(index, part, Nav::Up, w, cx)
                }))
                .on_action(cx.listener(move |s, _: &text_input::Down, w, cx| {
                    s.navigate(index, part, Nav::Down, w, cx)
                }))
                .on_action(cx.listener(move |s, _: &text_input::Enter, w, cx| {
                    s.navigate(index, part, Nav::Enter, w, cx)
                }))
                .on_action(cx.listener(move |s, _: &text_input::Backspace, w, cx| {
                    s.navigate(index, part, Nav::Back, w, cx)
                }))
        };
        div()
            .flex()
            .when(index > 0, |d| d.mt(px(1.)))
            .child(
                div()
                    .id(("slide-number", index))
                    .w(px(30.))
                    .flex_shrink_0()
                    .pt(px(2.))
                    .pr(px(6.))
                    .flex()
                    .justify_end()
                    .text_size(px(12.))
                    .text_color(rgb(if selected { 0x2f4f99 } else { 0x7a7f86 }))
                    .bg(rgb(if selected { 0xdce3fa } else { 0xf1f1ef }))
                    .cursor_pointer()
                    .on_click(cx.listener(move |s, _, w, cx| s.select_section(index, w, cx)))
                    .child((index + 1).to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        nav(LABEL, cx)
                            .px_1()
                            .when(labeled, |d| {
                                d.bg(rgb(kind.bar)).border_l_2().border_color(rgb(kind.ink))
                            })
                            .child(self.cells[index][LABEL].clone()),
                    )
                    .child(
                        nav(LYRICS, cx)
                            .px_1()
                            .pb_1()
                            .child(self.cells[index][LYRICS].clone()),
                    ),
            )
    }
}

/// Lets the operator's tests read and drive an editor window it opened.
#[cfg(test)]
impl Library {
    /// The loaded revision, the current document and whether storage is busy.
    pub(crate) fn probe(&self, cx: &App) -> (Option<Version>, Song, bool) {
        (self.version, self.current(cx), self.pending.is_some())
    }
    pub(crate) fn probe_poll(&mut self, cx: &mut Context<Self>) {
        self.poll(cx);
    }
    pub(crate) fn probe_title_field(&self) -> Entity<TextInput> {
        self.fields[0].clone()
    }
    /// The footer Apply button: saves without closing.
    pub(crate) fn probe_apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.action(1, window, cx);
    }
}

/// Visible but unavailable control (EW8-OBS-021 layout, not yet implemented).
fn inert(label: &str) -> Div {
    div()
        .flex_shrink_0()
        .px_2()
        .py_1()
        .text_size(px(12.))
        .text_color(rgb(0xa5a9af))
        .child(label.to_owned())
}

/// Saved songs need a title; a draft only needs to fit while authoring.
fn valid_draft(song: &Song) -> bool {
    let mut bounded = song.clone();
    if bounded.title.trim().is_empty() {
        bounded.title = "Untitled".into();
    }
    bounded.validate().is_ok()
}

fn editable(song: &Song) -> bool {
    [&song.title, &song.authors, &song.copyright, &song.license]
        .iter()
        .all(|s| !s.contains(['\r', '\n']))
        && song.sections.iter().all(|s| {
            !s.label.contains(['\r', '\n'])
                && s.lyrics.bytes().filter(|b| *b == b'\n').count() < 4096
        })
}

fn error_message(e: Error) -> &'static str {
    match e {
        Error::Conflict => {
            "Changed elsewhere. Your draft is retained; duplicate it to save a separate copy, or discard and reopen."
        }
        Error::Locked => {
            "Library is busy in another process. Wait, then retry; your draft is retained."
        }
        Error::Full => "Storage is full. Free space and retry; your draft is retained.",
        Error::Unsupported => {
            "Unsupported library version or foreign database. Use a supported library; original was not replaced."
        }
        Error::Corrupt => {
            "Library is corrupt. Preserve the original and restore a known-good backup."
        }
        Error::Io => {
            "Library could not be read or written. Check folder permissions and free space, then reopen or retry."
        }
        Error::Invalid => "Song is invalid or exceeds storage limits. Correct the draft and retry.",
        Error::Missing => "Saved revision is missing. Refresh the library.",
        Error::Exists => {
            "Backup destination already exists. Preserve it and the library; recovery requires a fresh destination."
        }
        _ => {
            "Library is unavailable. Close and reopen; do not discard an unsaved draft without copying it."
        }
    }
}

impl Render for Library {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.committed_close {
            window.remove_window();
        }
        if let Some(cell) = self.focused_cell(window, cx).filter(|_| !self.slides) {
            self.section = cell.0;
            // Moving into another cell ends the whole-song selection; focus
            // leaving the cells (the Format pane) keeps it.
            if self.all && Some(cell) != self.all_focus {
                self.exit_all(cx);
            }
        }
        let title = self.fields[0].read(cx).text().to_owned();
        if title != self.title {
            window.set_window_title(&window_title(&title));
            self.title = title;
        }
        self.ensure_preview(cx);
        self.ensure_thumbnails(cx);
        let busy = self.pending.is_some();
        let dirty = self.dirty(cx);
        let viewport = window.viewport_size();
        let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
        let catalog_width = if self.show_catalog { 220. } else { 0. };
        let words_width = if self.slides {
            SLIDES_PANE
        } else {
            ((width - catalog_width) * 0.34).max(330.)
        };
        let show_format = self.format && !busy && !self.show_catalog;
        let format_width = if show_format { format_pane::WIDTH } else { 0. };
        let pane = (width - catalog_width - words_width - format_width - 48.).max(160.);
        let slide_width = pane.min((height - 230.).max(90.) * 16. / 9.);
        let labels: Vec<String> = self
            .cells
            .iter()
            .map(|c| c[LABEL].read(cx).text().to_owned())
            .collect();
        let groups = groups(labels.iter().map(String::as_str));
        let empty = self.preview_slide(cx).is_some_and(|s| s.text.is_empty());
        let rejected = self.preview.as_ref().is_some_and(|p| p.image.is_none());
        let image = self.preview.as_ref().and_then(|p| p.image.clone());
        let format_pane = show_format.then(|| self.format_pane(window, cx));
        div()
            .key_context("Sela SongLibrary")
            .track_focus(&self.focus)
            .on_action(cx.listener(|s, _: &Quit, w, cx| {
                if s.may_close(cx) {
                    w.remove_window();
                }
            }))
            .on_action(cx.listener(|_, _: &FocusNext, w, cx| w.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrevious, w, cx| w.focus_prev(cx)))
            .on_action(cx.listener(|s, _: &Save, w, cx| s.action(1, w, cx)))
            .on_action(cx.listener(|s, _: &SplitSection, w, cx| s.split_section(w, cx)))
            .on_action(cx.listener(|s, _: &Undo, w, cx| s.history(false, w, cx)))
            .on_action(cx.listener(|s, _: &Redo, w, cx| s.history(true, w, cx)))
            .on_action(cx.listener(|s, _: &SelectAllSlides, w, cx| s.select_all_slides(w, cx)))
            // A slider or dial drag follows the pointer anywhere in the
            // window and ends on release, even outside it.
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(MouseButton::Left) {
                    s.drag_to(e.position, cx);
                } else {
                    s.drag_end(cx);
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|s, _, _, cx| s.drag_end(cx)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|s, _, _, cx| s.drag_end(cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf4f4f3))
            .text_color(rgb(0x292c30))
            .font_family("DejaVu Sans")
            .text_size(px(13.))
            // EW8-OBS-021 toolbar: Title and document tools at left, insert
            // groups, then Format/Animate/Presentation at right.
            .child(
                div()
                    .id("editor-toolbar")
                    .overflow_x_scroll()
                    .h(px(68.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(rgb(0xdcdedc))
                    .child(
                        div()
                            .w(px(250.))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(self.fields[0].clone())
                            .child(
                                div()
                                    .flex()
                                    .when(!busy, |d| {
                                        d.child(self.button(0, "New", cx))
                                            .child(self.button(5, "Undo", cx))
                                            .child(self.button(6, "Redo", cx))
                                    }),
                            ),
                    )
                    .child(div().w(px(1.)).h(px(40.)).mx_2().bg(rgb(0xdcdedc)))
                    .children(["Text", "Scripture", "Shape", "Media"].map(inert))
                    .child(div().flex_1())
                    .child(if busy {
                        inert("Format").into_any_element()
                    } else {
                        div()
                            .border_b_2()
                            .border_color(rgb(if self.format { 0x536aca } else { 0xf4f4f3 }))
                            .child(self.button(18, "Format", cx))
                            .into_any_element()
                    })
                    .children(["Animate", "Presentation"].map(inert))
                    .child(div().w(px(1.)).h(px(40.)).mx_2().bg(rgb(0xdcdedc)))
                    .when(!busy, |d| {
                        d.child(self.button(13, "Library", cx))
                            .child(self.button(12, "Inspector", cx))
                    }),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_size(px(12.))
                    .text_color(rgb(0x646971))
                    .child(format!(
                        "{} · {}",
                        if dirty { "Unsaved" } else { "Unchanged" },
                        self.status
                    )),
            )
            .when(self.confirm_close && !busy, |d| {
                d.child(
                    div()
                        .px_3()
                        .py_2()
                        .bg(rgb(0xfff1dc))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child("Close without saving?")
                        .child(self.button(10, "Discard and close", cx))
                        .child(self.button(11, "Keep editing", cx)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .border_t_1()
                    .border_color(rgb(0xdcdedc))
                    .when(self.show_catalog, |d| {
                        d.child(
                            div()
                                .w(px(catalog_width))
                                .flex_shrink_0()
                                .border_r_1()
                                .border_color(rgb(0xdcdedc))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .p_3()
                                        .text_color(rgb(0x646971))
                                        .child("Saved songs · ID order"),
                                )
                                .when(!busy, |d| {
                                    d.child(
                                        div()
                                            .px_2()
                                            .flex()
                                            .flex_wrap()
                                            .child(self.button(2, "Duplicate", cx))
                                            .child(self.button(
                                                3,
                                                if self.confirm_delete {
                                                    "Confirm delete"
                                                } else {
                                                    "Delete"
                                                },
                                                cx,
                                            ))
                                            .child(self.button(4, "Discard edits", cx)),
                                    )
                                })
                                .child(
                                    div()
                                        .id("library-list")
                                        .flex_1()
                                        .min_h_0()
                                        .overflow_y_scroll()
                                        .children(self.catalog.iter().enumerate().map(
                                            |(i, (version, title))| {
                                                let version = *version;
                                                div()
                                                    .id(("song-row", i))
                                                    .track_focus(&self.row_focus[i])
                                                    .key_context("SelaControl")
                                                    .px_3()
                                                    .py_2()
                                                    .cursor_pointer()
                                                    .bg(rgb(if self.version == Some(version) {
                                                        0xe3e7f3
                                                    } else {
                                                        0xf4f4f3
                                                    }))
                                                    .hover(|d| d.bg(rgb(0xe8e9e7)))
                                                    .focus(|d| d.bg(rgb(0xdce3fa)))
                                                    .on_action(cx.listener(
                                                        move |s, _: &ActivateControl, w, cx| {
                                                            w.prevent_default();
                                                            s.select(version, cx);
                                                        },
                                                    ))
                                                    .on_click(cx.listener(
                                                        move |s, event, w, cx| {
                                                            if matches!(
                                                                event,
                                                                ClickEvent::Keyboard(_)
                                                            ) {
                                                                return;
                                                            }
                                                            s.row_focus[i].focus(w, cx);
                                                            s.select(version, cx);
                                                        },
                                                    ))
                                                    .child(title.clone())
                                            },
                                        )),
                                )
                                .when(!busy, |d| {
                                    d.child(self.button(
                                        9,
                                        if self.catalog.len() == 128 {
                                            "Next page"
                                        } else {
                                            "Refresh / first page"
                                        },
                                        cx,
                                    ))
                                }),
                        )
                    })
                    .child(
                        div()
                            .w(px(words_width))
                            .flex_shrink_0()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .bg(rgb(0xfbfbfa))
                            .child(
                                div()
                                    .flex()
                                    .flex_shrink_0()
                                    .px_2()
                                    .pt_1()
                                    .border_b_1()
                                    .border_color(rgb(0xdcdedc))
                                    .children([(14, "Words", !self.slides && !self.layouts), (17, "Slides", self.slides && !self.layouts)].map(
                                        |(index, label, active)| {
                                            div()
                                                .border_b_2()
                                                .border_color(rgb(if active {
                                                    0x536aca
                                                } else {
                                                    0xfbfbfa
                                                }))
                                                .child(self.button(index, label, cx))
                                        },
                                    ))
                                    // EW8-OBS-043: "Layouts ⊗" beside Words and Slides.
                                    .when(self.layouts, |d| {
                                        d.child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .border_b_2()
                                                .border_color(rgb(0x536aca))
                                                .child(div().px_2().text_size(px(12.)).child("Layouts"))
                                                .child(self.button(19, "⊗", cx)),
                                        )
                                    }),
                            )
                            .child(if busy {
                                div()
                                    .flex_1()
                                    .p_6()
                                    .child("Loading or saving…")
                                    .into_any_element()
                            } else if self.layouts {
                                self.layout_list().into_any_element()
                            } else if self.slides {
                                div()
                                    .id("slide-list")
                                    .key_context("SongWords")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .p_2()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .children(
                                        (0..self.draft.sections.len())
                                            .map(|i| self.thumbnail_row(i, cx)),
                                    )
                                    .into_any_element()
                            } else {
                                self.whole_song_keys(div().id("words").key_context("SongWords"), cx)
                                    .track_scroll(&self.words_scroll)
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .p_2()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .children(groups.iter().map(|group| {
                                        let label = &labels[group.start];
                                        let kind = kind(label);
                                        div()
                                            .flex_shrink_0()
                                            .border_1()
                                            .rounded(px(4.))
                                            .overflow_hidden()
                                            .bg(rgb(0xffffff))
                                            .border_color(rgb(if label.is_empty() {
                                                0xdcdedc
                                            } else {
                                                kind.border
                                            }))
                                            .children(
                                                group.clone().map(|i| self.slide_row(i, kind, cx)),
                                            )
                                    }))
                                    .into_any_element()
                            })
                            .child(
                                div()
                                    .h(px(34.))
                                    .flex_shrink_0()
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .border_t_1()
                                    .border_color(rgb(0xdcdedc))
                                    .when(!busy, |d| {
                                        d.child(self.button(7, "+", cx))
                                            .child(self.button(8, "−", cx))
                                    }),
                            ),
                    )
                    .when(!busy && !self.show_catalog, |d| {
                        d.child(
                            div()
                                .id("draft-preview")
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .bg(rgb(0xebebe9))
                                .border_l_1()
                                .border_color(rgb(0xdcdedc))
                                .when(!self.inspector, |d| {
                                    d.child(
                                        div()
                                            .flex_1()
                                            .min_h_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                div()
                                                    .id("slide-preview")
                                                    .relative()
                                                    .w(px(slide_width))
                                                    .h(px(slide_width * 9. / 16.))
                                                    .bg(rgb(0x000000))
                                                    .border_1()
                                                    .border_color(rgb(0xc9ccd0))
                                                    .on_click(cx.listener(|s, e: &ClickEvent, w, cx| {
                                                        if e.click_count() == 2 {
                                                            // No canvas editing yet: edit in Words.
                                                            s.exit_all(cx);
                                                            s.slides = false;
                                                            let index = s.section;
                                                            s.focus_cell(index, LYRICS, usize::MAX, w, cx);
                                                        }
                                                    }))
                                                    .children(image.map(|image| {
                                                        img(image).size_full()
                                                    }))
                                                    .when(empty || rejected, |d| {
                                                        d.child(
                                                            div()
                                                                .absolute()
                                                                .inset_0()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .p_4()
                                                                .text_color(rgb(0x9a9ea5))
                                                                .child(if rejected {
                                                                    "This slide cannot be shown: a line is too long or uses a character the bundled font lacks."
                                                                } else {
                                                                    // EW8-OBS-023 empty-slide hint.
                                                                    "Double click to edit song"
                                                                }),
                                                        )
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .h(px(26.))
                                            .flex_shrink_0()
                                            .px_3()
                                            .flex()
                                            .items_center()
                                            .text_size(px(11.))
                                            .text_color(rgb(0x646971))
                                            .gap_1()
                                            .child(if self.layouts {
                                                "Master layout · audience layout preview".to_owned()
                                            } else {
                                                format!(
                                                    "Slide {} of {} · audience layout preview",
                                                    (self.section + 1).min(self.cells.len()),
                                                    self.cells.len()
                                                )
                                            })
                                            // A named family that fell back to
                                            // the bundled face (M1-05g2b).
                                            .children(
                                                self.preview
                                                    .as_ref()
                                                    .and_then(|p| p.warning.clone())
                                                    .map(|warning| {
                                                        div()
                                                            .text_color(rgb(0xb36b00))
                                                            .child(warning)
                                                    }),
                                            ),
                                    )
                                })
                                .when(self.inspector, |d| {
                                    d.child(
                                        div().p_4().child("Inspector · song information").children(
                                            ["Authors", "Copyright", "License identifier"]
                                                .iter()
                                                .enumerate()
                                                .map(|(i, label)| {
                                                    div()
                                                        .mt_3()
                                                        .child(*label)
                                                        .child(self.fields[i + 1].clone())
                                                }),
                                        ),
                                    )
                                }),
                        )
                    })
                    .children(format_pane),
            )
            // EW8-OBS-021 footer.
            .when(!busy, |d| {
                d.child(
                    div()
                        .h(px(46.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .border_t_1()
                        .border_color(rgb(0xdcdedc))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_size(px(12.))
                                .text_color(rgb(0xa5a9af))
                                .child(
                                    div()
                                        .size(px(13.))
                                        .border_1()
                                        .rounded(px(2.))
                                        .border_color(rgb(0xcbd0d6)),
                                )
                                .child("Apply changes to items in schedule"),
                        )
                        .child(div().flex_1())
                        .child(self.framed(1, "Apply", dirty, cx))
                        .child(self.framed(15, "OK", true, cx))
                        .child(self.framed(16, "Cancel", true, cx)),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use sela::storage::Repository;

    fn fixture(cx: &mut TestAppContext, path: PathBuf) -> (VisualTestContext, Entity<Library>) {
        let window = cx.update(|cx| {
            crate::bind_operator_keys(cx);
            cx.open_window(Default::default(), |window, cx| {
                let view = cx.new(|cx| Library::new(path, cx));
                view.read(cx).focus.clone().focus(window, cx);
                view
            })
            .unwrap()
        });
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let view = window.root(&mut cx).unwrap();
        wait(&mut cx, &view);
        (cx, view)
    }
    fn wait(cx: &mut VisualTestContext, view: &Entity<Library>) {
        let start = std::time::Instant::now();
        while view.read_with(cx, |v, _| v.pending.is_some()) {
            cx.update(|_, cx| view.update(cx, |v, cx| v.poll(cx)));
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn input(cx: &VisualTestContext, view: &Entity<Library>, index: usize) -> Entity<TextInput> {
        view.read_with(cx, |v, _| v.fields[index].clone())
    }
    fn cell(
        cx: &VisualTestContext,
        view: &Entity<Library>,
        index: usize,
        part: usize,
    ) -> Entity<TextInput> {
        view.read_with(cx, |v, _| v.cells[index][part].clone())
    }
    fn show_inspector(cx: &mut VisualTestContext, view: &Entity<Library>, index: usize) {
        if (1..=3).contains(&index) {
            cx.update(|_, cx| {
                view.update(cx, |v, cx| {
                    v.inspector = true;
                    v.show_catalog = false;
                    cx.notify();
                })
            });
            cx.run_until_parked();
        }
    }
    /// Programmatic load: bypasses the document callback, like a late reload.
    fn field(cx: &mut VisualTestContext, view: &Entity<Library>, index: usize, text: &str) {
        show_inspector(cx, view, index);
        let input = input(cx, view, index);
        cx.update(|w, cx| {
            input.update(cx, |f, cx| f.set_text(text, cx)).unwrap();
            input.read(cx).focus_handle(cx).focus(w, cx);
        });
    }
    /// Native replacement of the whole text, as typing would report it.
    fn replace(cx: &mut VisualTestContext, input: Entity<TextInput>, text: &str) {
        cx.update(|w, cx| {
            input.update(cx, |f, cx| {
                let end = f.text().encode_utf16().count();
                f.replace_text_in_range(Some(0..end), text, w, cx);
                f.focus_handle(cx).focus(w, cx);
            })
        });
    }
    fn edit(cx: &mut VisualTestContext, view: &Entity<Library>, index: usize, text: &str) {
        show_inspector(cx, view, index);
        let input = input(cx, view, index);
        replace(cx, input, text);
    }
    fn type_cell(
        cx: &mut VisualTestContext,
        view: &Entity<Library>,
        index: usize,
        part: usize,
        text: &str,
    ) {
        let input = cell(cx, view, index, part);
        replace(cx, input, text);
    }
    fn caret(
        cx: &mut VisualTestContext,
        view: &Entity<Library>,
        index: usize,
        part: usize,
        byte: usize,
    ) {
        cx.update(|w, cx| view.update(cx, |v, cx| v.focus_cell(index, part, byte, w, cx)));
        cx.run_until_parked();
    }
    fn focused(cx: &mut VisualTestContext, view: &Entity<Library>) -> Option<(usize, usize)> {
        cx.update(|w, cx| view.read(cx).focused_cell(w, cx))
    }
    fn action(cx: &mut VisualTestContext, view: &Entity<Library>, index: usize) {
        cx.update(|w, cx| view.update(cx, |v, cx| v.action(index, w, cx)));
    }

    #[gpui::test]
    fn persisted_ids_and_arrangements_survive_editor_save_duplicate_and_undo(
        cx: &mut TestAppContext,
    ) {
        use sela::arrangement::{Occurrence, OccurrenceId, Variant, VariantId};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite");
        let mut original = blank();
        original.title = "Duplicate labels".into();
        original.sections[0].label = "Verse 1".into();
        original.sections[0].lyrics = "First original".into();
        original.sections.push(Section {
            id: SectionId::allocate(),
            label: original.sections[0].label.clone(),
            lyrics: "Chorus asymmetric\r\n".into(),
            format: Default::default(),
            background: None,
        });
        original.variants = vec![Variant {
            id: VariantId(Id([5; 16])),
            name: "Retained".into(),
            occurrences: [0, 1, 1]
                .into_iter()
                .enumerate()
                .map(|(i, n)| Occurrence {
                    id: OccurrenceId(Id([i as u8; 16])),
                    section: original.sections[n].id,
                })
                .collect(),
        }];
        let first = Repository::open(&path)
            .unwrap()
            .save_song(None, original.clone())
            .unwrap();
        let (mut cx, view) = fixture(cx, path.clone());
        cx.update(|_, cx| view.update(cx, |v, cx| v.select(first, cx)));
        wait(&mut cx, &view);
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), original);
        type_cell(&mut cx, &view, 0, LYRICS, "First edited");
        let edited = view.read_with(&cx, |v, cx| v.current(cx));
        action(&mut cx, &view, 1);
        wait(&mut cx, &view);
        let second = view.read_with(&cx, |v, _| v.version.unwrap());
        assert_eq!(
            Repository::open(&path).unwrap().song(second).unwrap(),
            edited
        );
        cx.update(|w, cx| view.read(cx).buttons[8].clone().focus(w, cx));
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), original);
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), edited);
        let count = view.read_with(&cx, |v, _| v.history.undo.len());
        action(&mut cx, &view, 8); // referenced slide cannot be removed
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), edited);
        assert_eq!(view.read_with(&cx, |v, _| v.history.undo.len()), count);
        action(&mut cx, &view, 7);
        let added = view.read_with(&cx, |v, cx| v.current(cx));
        assert!(!edited.sections.iter().any(|s| s.id == added.sections[2].id));
        assert_eq!(added.sections[2].label, "", "EW appends an unlabeled slide");
        assert_eq!(focused(&mut cx, &view), Some((2, LABEL)));
        cx.update(|w, cx| view.read(cx).buttons[8].clone().focus(w, cx));
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), edited);
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), added);
        assert_eq!(view.read_with(&cx, |v, _| v.section), 2);
        action(&mut cx, &view, 8); // unreferenced new slide can be removed
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), edited);
        action(&mut cx, &view, 2);
        wait(&mut cx, &view);
        let duplicate = view.read_with(&cx, |v, _| v.version.unwrap());
        assert_ne!(duplicate.id, first.id);
        let repo = Repository::open(&path).unwrap();
        assert_eq!(repo.song(duplicate).unwrap(), edited);
        assert_eq!(repo.song(first).unwrap(), original);
    }

    #[gpui::test]
    fn ctrl_enter_splits_without_label_and_backspace_joins(cx: &mut TestAppContext) {
        use sela::arrangement::{Variant, VariantId};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite");
        let mut original = blank();
        original.title = "Split hymn".into();
        original.sections[0].label = "Verse 1".into();
        original.sections[0].lyrics = "Line one\r\nLine two\nLine three".into();
        original.sections.push(Section {
            id: SectionId::allocate(),
            label: "Chorus".into(),
            lyrics: "Refrain".into(),
            format: Default::default(),
            background: None,
        });
        let (verse, chorus) = (original.sections[0].id, original.sections[1].id);
        original.variants = vec![Variant {
            id: VariantId(Id([5; 16])),
            name: "Sunday".into(),
            occurrences: [verse, chorus, verse]
                .into_iter()
                .enumerate()
                .map(|(i, section)| Occurrence {
                    id: OccurrenceId(Id([i as u8 + 1; 16])),
                    section,
                })
                .collect(),
        }];
        let first = Repository::open(&path)
            .unwrap()
            .save_song(None, original.clone())
            .unwrap();
        let (mut cx, view) = fixture(cx, path.clone());
        cx.update(|_, cx| view.update(cx, |v, cx| v.select(first, cx)));
        wait(&mut cx, &view);

        // Outside the lyrics (title, a label) Ctrl+Enter changes nothing.
        field(&mut cx, &view, 0, "Split hymn");
        cx.simulate_keystrokes("ctrl-enter");
        caret(&mut cx, &view, 0, LABEL, 0);
        cx.simulate_keystrokes("ctrl-enter");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), original);
        assert!(view.read_with(&cx, |v, _| v.status.contains("cursor in the lyrics")));

        // Caret at the start of "Line two".
        caret(&mut cx, &view, 0, LYRICS, 10);
        cx.simulate_keystrokes("ctrl-enter");
        let split = view.read_with(&cx, |v, cx| v.current(cx));
        let labels: Vec<_> = split.sections.iter().map(|s| s.label.as_str()).collect();
        let lyrics: Vec<_> = split.sections.iter().map(|s| s.lyrics.as_str()).collect();
        assert_eq!(labels, ["Verse 1", "", "Chorus"]);
        assert_eq!(lyrics, ["Line one", "Line two\nLine three", "Refrain"]);
        let new = split.sections[1].id;
        assert!(![verse, chorus].contains(&new));
        let order: Vec<_> = split.variants[0]
            .occurrences
            .iter()
            .map(|o| o.section)
            .collect();
        assert_eq!(order, [verse, new, chorus, verse, new]);
        assert_eq!(
            sela::slides::slides(&split)
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>(),
            [
                "Line one",
                "Line two\nLine three",
                "Refrain",
                "Line one",
                "Line two\nLine three"
            ]
        );
        assert_eq!(view.read_with(&cx, |v, _| v.section), 1);
        assert_eq!(focused(&mut cx, &view), Some((1, LYRICS)));
        assert_eq!(
            cell(&cx, &view, 1, LYRICS).read_with(&cx, |c, _| c.selection()),
            0..0
        );
        // The unlabeled slide joins Verse 1's group; Chorus starts its own.
        assert_eq!(groups(labels.iter().copied()), [0..2, 2..3]);

        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), original);
        assert_eq!(view.read_with(&cx, |v, _| v.section), 0);
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), split);
        action(&mut cx, &view, 1);
        wait(&mut cx, &view);
        let saved = view.read_with(&cx, |v, _| v.version.unwrap());
        assert_eq!(Repository::open(&path).unwrap().song(saved).unwrap(), split);

        // Backspace at the start of the unlabeled slide joins it back.
        caret(&mut cx, &view, 1, LYRICS, 0);
        cx.simulate_keystrokes("backspace");
        let joined = view.read_with(&cx, |v, cx| v.current(cx));
        assert_eq!(joined.sections.len(), 2);
        assert_eq!(joined.sections[0].lyrics, "Line one\nLine two\nLine three");
        assert_eq!(joined.variants, original.variants);
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
        assert_eq!(
            cell(&cx, &view, 0, LYRICS).read_with(&cx, |c, _| c.selection()),
            8..8
        );
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), split);
        // A labeled slide is never joined; Backspace only leaves its lyrics.
        caret(&mut cx, &view, 2, LYRICS, 0);
        cx.simulate_keystrokes("backspace");
        assert_eq!(focused(&mut cx, &view), Some((2, LABEL)));
        cx.simulate_keystrokes("home backspace");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), split);

        // At the 128-section limit nothing is split and no undo step is added.
        cx.update(|_, cx| {
            view.update(cx, |v, cx| {
                let mut full = v.current(cx);
                full.variants.clear();
                while full.sections.len() < 128 {
                    full.sections.push(Section {
                        id: SectionId::allocate(),
                        label: "Filler".into(),
                        lyrics: String::new(),
                        format: Default::default(),
                        background: None,
                    });
                }
                v.begin(full, None, cx);
            })
        });
        let full = view.read_with(&cx, |v, cx| v.current(cx));
        caret(&mut cx, &view, 0, LYRICS, 0);
        cx.simulate_keystrokes("ctrl-enter");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), full);
        assert!(view.read_with(&cx, |v, _| v.history.undo.is_empty()
            && v.status.contains("limits")));
    }

    #[gpui::test]
    fn words_cells_navigate_and_new_song_focuses_first_label(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = fixture(cx, dir.path().join("library.sqlite"));
        action(&mut cx, &view, 0);
        cx.run_until_parked();
        assert_eq!(focused(&mut cx, &view), Some((0, LABEL)));
        cx.simulate_input("Verse 1");
        cx.simulate_keystrokes("enter");
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
        cx.simulate_input("Line one");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("Line two");
        // Enter is a line break, not a split (EW8-OBS-023).
        cx.simulate_keystrokes("enter enter");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx).sections.len()), 1);
        cx.simulate_input("after blank");
        cx.simulate_keystrokes("ctrl-enter");
        cx.simulate_input("Second");
        cx.simulate_keystrokes("up");
        assert_eq!(focused(&mut cx, &view), Some((1, LABEL)));
        cx.simulate_input("Chorus");
        cx.simulate_keystrokes("up");
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
        cx.simulate_keystrokes("down");
        assert_eq!(focused(&mut cx, &view), Some((1, LABEL)));
        cx.simulate_keystrokes("down");
        assert_eq!(focused(&mut cx, &view), Some((1, LYRICS)));
        let song = view.read_with(&cx, |v, cx| v.current(cx));
        let parts: Vec<_> = song
            .sections
            .iter()
            .map(|s| (s.label.as_str(), s.lyrics.as_str()))
            .collect();
        assert_eq!(
            parts,
            [
                ("Verse 1", "Line one\nLine two\n\nafter blank"),
                ("Chorus", "Second")
            ]
        );
        assert_eq!(view.read_with(&cx, |v, _| v.section), 1);
        cx.simulate_keystrokes("ctrl-z");
        let label = view.read_with(&cx, |v, cx| v.current(cx).sections[1].label.clone());
        assert!(label.len() < 6 && "Chorus".starts_with(&label));
        // Title lives in the toolbar and names the window.
        edit(&mut cx, &view, 0, "Amazing");
        cx.run_until_parked();
        assert_eq!(view.read_with(&cx, |v, _| v.title.clone()), "Amazing");
    }

    #[test]
    fn label_kinds_and_groups_follow_observed_palette() {
        assert_eq!(kind("Verse 2"), VERSE);
        assert_eq!(kind("etsaer"), VERSE);
        assert_eq!(kind("Chorus"), kind("Pre-Chorus"));
        assert_eq!(kind("Bridge"), kind("tag"));
        assert_ne!(kind("Chorus"), VERSE);
        assert_ne!(kind("Bridge"), kind("Chorus"));
        assert_ne!(kind("Ending"), kind("Intro"));
        assert_eq!(kind("Ending").ink, 0x511c1c);
        assert_eq!(kind("Intro").ink, 0x315b46);
        assert_eq!(groups(["", "", "Chorus", "", "Tag"]), [0..2, 2..4, 4..5]);
        assert_eq!(groups(["Verse", "Chorus"]), [0..1, 1..2]);
        assert!(groups([]).is_empty());
    }

    fn slide(text: &str) -> sela::slides::Slide {
        sela::slides::Slide {
            label: String::new(),
            text: text.into(),
            format: Default::default(),
            background: None,
        }
    }

    #[test]
    fn preview_matches_audience_raster_and_rejects_unshowable_text() {
        let ink = |image: &RenderImage, channel: usize| {
            image
                .as_bytes(0)
                .unwrap()
                .chunks(4)
                .filter(|p| p[channel] > 128)
                .count()
        };
        let blank = render_preview(&slide(""), &Backdrops::new(None))
            .image
            .unwrap();
        assert_eq!(ink(&blank, 0), 0);
        let text = render_preview(&slide("Amazing grace\nhow sweet"), &Backdrops::new(None))
            .image
            .unwrap();
        let size = text.size(0);
        assert_eq!((size.width.0, size.height.0), (1280, 720));
        assert!(ink(&text, 0) > 1000);
        // Styled previews blend the format's color: yellow text has green
        // and red ink but no blue.
        let styled = sela::slides::Slide {
            format: sela::format::SlideFormat {
                bold: Some(true),
                color: Some([255, 255, 0]),
                ..Default::default()
            },
            ..slide("Amazing grace\nhow sweet")
        };
        let yellow = render_preview(&styled, &Backdrops::new(None))
            .image
            .unwrap();
        assert_eq!(ink(&yellow, 0), 0, "yellow fill has no blue");
        assert!(ink(&yellow, 1) > 1000, "yellow fill has green");
        assert!(
            render_preview(&slide("\u{e000}"), &Backdrops::new(None))
                .image
                .is_none(),
            "missing glyph"
        );
        // The fitted size is reported in 1080-reference px; a fixed size
        // round-trips through the 720-high preview.
        assert_eq!(
            render_preview(&slide(""), &Backdrops::new(None)).fitted,
            None
        );
        let fitted = render_preview(&slide("Amazing grace\nhow sweet"), &Backdrops::new(None))
            .fitted
            .unwrap();
        assert!(fitted > 40, "{fitted}");
        let fixed = sela::slides::Slide {
            format: sela::format::SlideFormat {
                size: Some(sela::format::Size::Fixed(78)),
                ..Default::default()
            },
            ..slide("Amazing grace")
        };
        assert_eq!(
            render_preview(&fixed, &Backdrops::new(None)).fitted,
            Some(78)
        );
    }

    #[gpui::test]
    fn preview_follows_the_caret_slide_off_thread(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = fixture(cx, dir.path().join("library.sqlite"));
        type_cell(&mut cx, &view, 0, LYRICS, "First slide");
        action(&mut cx, &view, 7);
        type_cell(&mut cx, &view, 1, LYRICS, "");
        cx.update(|_, cx| view.update(cx, |v, cx| v.ensure_preview(cx)));
        cx.run_until_parked();
        assert!(view.read_with(&cx, |v, _| {
            v.preview
                .as_ref()
                .is_some_and(|p| p.key.0.is_empty() && p.image.is_some())
        }));
        caret(&mut cx, &view, 0, LYRICS, 0);
        cx.update(|_, cx| view.update(cx, |v, cx| v.ensure_preview(cx)));
        cx.run_until_parked();
        assert!(view.read_with(&cx, |v, _| {
            v.preview_task.is_none()
                && v.preview
                    .as_ref()
                    .is_some_and(|p| p.key.0 == "First slide" && p.image.is_some())
        }));
        type_cell(&mut cx, &view, 0, LYRICS, "\u{e000}");
        cx.update(|_, cx| view.update(cx, |v, cx| v.ensure_preview(cx)));
        cx.run_until_parked();
        assert!(view.read_with(&cx, |v, _| {
            v.preview.as_ref().is_some_and(|p| p.image.is_none())
        }));
    }

    #[gpui::test]
    fn preview_warns_when_the_named_family_falls_back(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite");
        let mut repository = Repository::open(&path).unwrap();
        repository
            .save_song(
                None,
                Song {
                    title: "Fallback Hymn".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    variants: Vec::new(),
                    sections: vec![Section {
                        id: SectionId::allocate(),
                        label: "Verse 1".into(),
                        lyrics: "Falling back line".into(),
                        format: sela::format::SlideFormat {
                            font: Some("Sela Missing Family".into()),
                            ..Default::default()
                        },
                        background: None,
                    }],
                    master: None,
                },
            )
            .unwrap();
        let (mut cx, view) = fixture(cx, path);
        let version = view.read_with(&cx, |v, _| v.catalog[0].0);
        cx.update(|_, cx| view.update(cx, |v, cx| v.select(version, cx)));
        wait(&mut cx, &view);
        cx.update(|_, cx| view.update(cx, |v, cx| v.ensure_preview(cx)));
        cx.run_until_parked();
        // The bundled fallback still renders, and the warning names the
        // family and the fallback (M1-05g2b).
        assert!(view.read_with(&cx, |v, _| {
            v.preview.as_ref().is_some_and(|p| {
                p.image.is_some()
                    && p.warning.as_deref().is_some_and(|w| {
                        w.contains("Sela Missing Family") && w.contains("DejaVu Sans")
                    })
            })
        }));
    }

    /// A solid PNG in the profile's images folder, pinned by hash.
    fn profile_image(
        profile: &std::path::Path,
        name: &str,
        width: u32,
        height: u32,
        rgb: [u8; 3],
    ) -> ImageRef {
        use sha2::Digest;
        let images = sela::images::images_dir(profile);
        std::fs::create_dir_all(&images).unwrap();
        let [r, g, b] = rgb;
        image::RgbaImage::from_pixel(width, height, image::Rgba([r, g, b, 255]))
            .save(images.join(name))
            .unwrap();
        let bytes = std::fs::read(images.join(name)).unwrap();
        ImageRef {
            name: name.into(),
            sha256: sha2::Sha256::digest(&bytes).into(),
        }
    }

    /// RGB at (x, y) of a BGRA preview raster.
    fn rgb_at(image: &RenderImage, x: usize, y: usize) -> [u8; 3] {
        let bytes = image.as_bytes(0).unwrap();
        let at = (y * PREVIEW.width as usize + x) * 4;
        [bytes[at + 2], bytes[at + 1], bytes[at]]
    }

    #[test]
    fn preview_composites_over_the_fitted_image_or_substitutes_black() {
        use sela::background::Background;
        let dir = tempfile::tempdir().unwrap();
        let blue = [30, 110, 210];
        // 4:1, so Maintain letterboxes top and bottom on the 16:9 preview.
        let wide = profile_image(dir.path(), "wide.png", 400, 100, blue);
        let backdrops = Backdrops::new(Some(dir.path().into()));
        let with = |background| sela::slides::Slide {
            background: Some(background),
            ..slide("Amazing grace")
        };
        let zoom = render_preview(&with(Background::image(wide.clone())), &backdrops);
        assert_eq!(zoom.warning, None);
        let image = zoom.image.unwrap();
        assert_eq!(rgb_at(&image, 2, 2), blue, "Zoom covers the corner");
        assert!(
            image
                .as_bytes(0)
                .unwrap()
                .chunks(4)
                .any(|p| p[..3] == [255; 3]),
            "white text over the image"
        );
        let maintain = Background {
            aspect: Aspect::Maintain,
            ..Background::image(wide.clone())
        };
        let image = render_preview(&with(maintain), &backdrops).image.unwrap();
        assert_eq!(rgb_at(&image, 2, 2), [0; 3], "black bar");
        assert_eq!(rgb_at(&image, 2, 360), blue, "image between the bars");
        assert_eq!(backdrops.fitted.lock().unwrap().len(), 2);

        let missing = ImageRef {
            name: "gone.png".into(),
            ..wide.clone()
        };
        let preview = render_preview(&with(Background::image(missing)), &backdrops);
        assert_eq!(rgb_at(&preview.image.unwrap(), 2, 2), [0; 3]);
        assert_eq!(
            preview.warning.as_deref(),
            Some("Background image ‘gone.png’ is missing; showing black")
        );
        let red = render_preview(&with(Background::color([200, 0, 0])), &backdrops);
        assert_eq!(rgb_at(&red.image.unwrap(), 2, 2), [200, 0, 0]);
        // Without a profile folder no image can be found.
        let none = render_preview(&with(Background::image(wide)), &Backdrops::new(None));
        assert!(none.warning.unwrap().contains("is missing"));
        // Thumbnails composite over the same background.
        let thumbnail = render_thumbnail(&with(Background::color(blue)), &backdrops).unwrap();
        assert_eq!(
            thumbnail.as_bytes(0).unwrap()[..3],
            [blue[2], blue[1], blue[0]]
        );
    }

    #[test]
    fn editor_backdrops_keep_at_most_four_fitted_images() {
        let dir = tempfile::tempdir().unwrap();
        let backdrops = Backdrops::new(Some(dir.path().into()));
        for n in 0..6u8 {
            let image = profile_image(dir.path(), &format!("{n}.png"), 16, 9, [n, n, n]);
            let slide = sela::slides::Slide {
                background: Some(sela::background::Background::image(image)),
                ..slide("")
            };
            assert_eq!(backdrops.prepare(&slide).1, None);
        }
        let fitted = backdrops.fitted.lock().unwrap();
        assert_eq!(fitted.len(), BACKDROPS);
        assert_eq!(fitted.front().unwrap().0.0.name, "2.png", "oldest evicted");
        assert!(fitted.iter().all(|(_, rgba)| rgba.len() == 1280 * 720 * 4));
    }

    #[test]
    fn thumbnail_is_the_preview_box_filtered() {
        let text = slide("Amazing grace\nhow sweet the sound");
        let (full, small) = (
            render_preview(&text, &Backdrops::new(None)).image.unwrap(),
            render_thumbnail(&text, &Backdrops::new(None)).unwrap(),
        );
        let size = small.size(0);
        assert_eq!((size.width.0, size.height.0), (320, 180));
        let sum = |image: &RenderImage| {
            image
                .as_bytes(0)
                .unwrap()
                .chunks(4)
                .map(|p| u64::from(p[1]))
                .sum::<u64>()
        };
        let (full, small) = (sum(&full), sum(&small) * 16);
        assert!(small > 0);
        assert!(full.abs_diff(small) * 200 < full, "{full} vs {small}");
        assert!(render_thumbnail(&slide("\u{e000}"), &Backdrops::new(None)).is_none());
    }

    #[gpui::test]
    fn slides_tab_thumbnails_render_off_thread_and_prune_edits(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = fixture(cx, dir.path().join("library.sqlite"));
        type_cell(&mut cx, &view, 0, LYRICS, "First");
        action(&mut cx, &view, 7);
        type_cell(&mut cx, &view, 1, LYRICS, "Second");
        let fill = |cx: &mut VisualTestContext| {
            for _ in 0..4 {
                cx.update(|_, cx| view.update(cx, |v, cx| v.ensure_thumbnails(cx)));
                cx.run_until_parked();
            }
        };
        let keys = |cx: &mut VisualTestContext| {
            view.read_with(cx, |v, _| {
                let mut keys: Vec<_> = v
                    .thumbnails
                    .iter()
                    .map(|(k, i)| (k.0.clone(), i.is_some()))
                    .collect();
                keys.sort();
                keys
            })
        };
        fill(&mut cx);
        assert!(keys(&mut cx).is_empty(), "Words tab renders no thumbnails");
        action(&mut cx, &view, 17);
        fill(&mut cx);
        assert_eq!(
            keys(&mut cx),
            [("First".into(), true), ("Second".into(), true)]
        );
        cx.simulate_keystrokes("tab");
        action(&mut cx, &view, 14);
        type_cell(&mut cx, &view, 0, LYRICS, "\u{e000}");
        fill(&mut cx);
        assert_eq!(keys(&mut cx), [("Second".into(), true)]);
        assert!(view.read_with(&cx, |v, _| v.thumbnail_task.is_none()));
        action(&mut cx, &view, 17);
        fill(&mut cx);
        assert_eq!(
            keys(&mut cx),
            [("Second".into(), true), ("\u{e000}".into(), false)]
        );
        // + in the Slides tab focuses the new thumbnail, so Ctrl+Z still
        // reaches the editor.
        action(&mut cx, &view, 7);
        assert!(cx.update(|w, cx| view.read(cx).section_focus[2].is_focused(w)));
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(view.read_with(&cx, |v, _| v.draft.sections.len()), 2);
        assert!(cx.update(|w, cx| {
            let v = view.read(cx);
            v.section == 0 && v.section_focus[0].is_focused(w)
        }));
    }

    #[gpui::test]
    fn editor_modes_section_selection_and_ok_failure_retain_document(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite");
        let (mut cx, view) = fixture(cx, path.clone());
        assert!(view.read_with(&cx, |v, _| !v.inspector && !v.show_catalog && !v.slides));
        action(&mut cx, &view, 15);
        assert!(view.read_with(&cx, |v, _| v.pending.is_none()
            && !v.committed_close
            && !v.close_after_save
            && v.status.contains("title")));
        edit(&mut cx, &view, 0, "Original mode fixture");
        type_cell(&mut cx, &view, 0, LYRICS, "Asymmetric first line");
        action(&mut cx, &view, 7);
        type_cell(&mut cx, &view, 1, LYRICS, "Distinct second section");
        let count = view.read_with(&cx, |v, _| v.history.undo.len());
        action(&mut cx, &view, 17);
        action(&mut cx, &view, 12);
        action(&mut cx, &view, 13);
        cx.update(|w, cx| view.update(cx, |v, cx| v.select_section(0, w, cx)));
        assert_eq!(view.read_with(&cx, |v, _| v.section), 0);
        assert_eq!(view.read_with(&cx, |v, _| v.history.undo.len()), count);
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).sections[0].lyrics.clone()),
            "Asymmetric first line"
        );
        action(&mut cx, &view, 1);
        wait(&mut cx, &view);
        let version = view.read_with(&cx, |v, _| v.version.unwrap());
        let mut other = Repository::open(&path).unwrap();
        let mut song = other.song(version).unwrap();
        song.title = "Other writer".into();
        other.save_song(Some(version), song).unwrap();
        edit(&mut cx, &view, 0, "Retained conflict draft");
        action(&mut cx, &view, 15);
        assert!(view.read_with(&cx, |v, _| v.close_after_save && !v.committed_close));
        action(&mut cx, &view, 16);
        assert!(view.read_with(&cx, |v, _| v.pending.is_some()));
        wait(&mut cx, &view);
        assert!(view.read_with(&cx, |v, cx| !v.committed_close
            && !v.close_after_save
            && v.dirty(cx)
            && v.status.starts_with("Changed elsewhere")
            && v.current(cx).title == "Retained conflict draft"));
    }

    #[gpui::test]
    fn pending_operations_reject_native_edits_before_redraw(cx: &mut TestAppContext) {
        for operation in ["ok", "load", "delete", "failed-save"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("library.sqlite");
            let mut repo = Repository::open(&path).unwrap();
            let mut original = blank();
            original.title = "Original".into();
            original.sections[0].lyrics = "Original verse".into();
            let first = repo.save_song(None, original.clone()).unwrap();
            let mut other = original.clone();
            other.title = "Different load target".into();
            let second = repo.save_song(None, other.clone()).unwrap();
            let (mut cx, view) = fixture(cx, path);
            cx.update(|_, cx| view.update(cx, |v, cx| v.select(first, cx)));
            wait(&mut cx, &view);
            if operation == "failed-save" {
                repo.save_song(Some(first), other.clone()).unwrap();
            }
            let input = cell(&cx, &view, 0, LYRICS);
            // One update: no redraw can detach the old input handler between
            // submission and the direct native replacement/preedit callbacks.
            cx.update(|w, cx| {
                view.update(cx, |v, cx| match operation {
                    "load" => v.select(second, cx),
                    "delete" => {
                        v.action(3, w, cx);
                        v.action(3, w, cx);
                    }
                    _ => v.action(15, w, cx),
                });
                let history = view.read(cx).history.undo.len();
                let edits = input.read(cx).edit_count();
                input.update(cx, |f, cx| {
                    f.replace_text_in_range(Some(0..0), "Late commit", w, cx);
                    f.replace_and_mark_text_in_range(Some(0..0), "Late preedit", Some(0..2), w, cx);
                });
                assert_eq!(input.read(cx).text(), "Original verse", "{operation}");
                assert_eq!(input.read(cx).edit_count(), edits);
                assert_eq!(view.read(cx).history.undo.len(), history);
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while view.read(cx).pending.is_some() {
                    view.update(cx, |v, cx| v.poll(cx));
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(1));
                }
                let expected = match operation {
                    "load" => other.clone(),
                    "delete" => view.read(cx).baseline.clone(),
                    _ => original.clone(),
                };
                assert_eq!(view.read(cx).current(cx), expected);
                assert_eq!(view.read(cx).committed_close, operation == "ok");
                // Completion-to-close also has no redraw yet. Other terminal
                // replies, including storage errors, must re-enable editing.
                let input = view.read(cx).cells[0][LYRICS].clone();
                input.update(cx, |f, cx| {
                    f.replace_text_in_range(Some(0..0), "After", w, cx)
                });
                if operation == "ok" {
                    view.update(cx, |v, cx| {
                        v.history(false, w, cx);
                        v.action(7, w, cx);
                    });
                    assert_eq!(view.read(cx).current(cx), original);
                    assert_eq!(repo.song(view.read(cx).version.unwrap()).unwrap(), original);
                } else {
                    assert!(input.read(cx).text().starts_with("After"));
                    assert!(view.read(cx).dirty(cx));
                }
            });
        }
    }

    #[gpui::test]
    fn ok_requires_current_document_to_match_saved_snapshot(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = fixture(cx, dir.path().join("library.sqlite"));
        edit(&mut cx, &view, 0, "Submitted title");
        cx.update(|w, cx| {
            view.update(cx, |v, cx| v.action(15, w, cx));
            // Deliberately bypass the native input lock via programmatic load:
            // auto-close still must independently compare with the saved snapshot.
            let title = view.read(cx).fields[0].clone();
            title
                .update(cx, |f, cx| f.set_text("Newer title", cx))
                .unwrap();
        });
        wait(&mut cx, &view);
        assert!(view.read_with(&cx, |v, cx| !v.committed_close
            && !v.close_after_save
            && v.dirty(cx)
            && v.baseline.title == "Submitted title"
            && v.current(cx).title == "Newer title"));
        edit(&mut cx, &view, 0, "Still editable");
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).title),
            "Still editable"
        );
    }

    #[gpui::test]
    fn blank_title_slide_selection_preserves_draft_but_cannot_save(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = fixture(cx, dir.path().join("library.sqlite"));
        edit(&mut cx, &view, 0, "   ");
        type_cell(&mut cx, &view, 0, LYRICS, "First section");
        action(&mut cx, &view, 7);
        type_cell(&mut cx, &view, 1, LYRICS, "Second section");
        let draft = view.read_with(&cx, |v, cx| v.current(cx));
        let count = view.read_with(&cx, |v, _| v.history.undo.len());
        cx.update(|w, cx| view.update(cx, |v, cx| v.select_section(0, w, cx)));
        assert_eq!(view.read_with(&cx, |v, _| v.section), 0);
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), draft);
        assert_eq!(view.read_with(&cx, |v, _| v.history.undo.len()), count);
        action(&mut cx, &view, 15);
        assert!(view.read_with(&cx, |v, _| v.pending.is_none()
            && !v.committed_close
            && v.version.is_none()
            && v.status.contains("title")));
    }

    #[gpui::test]
    fn chronological_document_history_across_cells_and_structures(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = fixture(cx, dir.path().join("library.sqlite"));
        edit(&mut cx, &view, 0, "Café 😀");
        type_cell(&mut cx, &view, 0, LYRICS, "First e\u{301}\r\n\n");
        let first = view.read_with(&cx, |v, cx| v.current(cx));
        action(&mut cx, &view, 7);
        type_cell(&mut cx, &view, 1, LABEL, "Chorus B");
        type_cell(&mut cx, &view, 1, LYRICS, "Second asymmetric 😀\nlast");
        let two = view.read_with(&cx, |v, cx| v.current(cx));
        let count = view.read_with(&cx, |v, _| v.history.undo.len());
        // Caret moves and slide selection are not document steps.
        caret(&mut cx, &view, 0, LYRICS, 0);
        cx.simulate_keystrokes("right shift-right");
        cx.update(|w, cx| view.update(cx, |v, cx| v.select_section(1, w, cx)));
        assert_eq!(view.read_with(&cx, |v, _| v.history.undo.len()), count);
        type_cell(&mut cx, &view, 0, LYRICS, "First changed");
        cx.update(|w, cx| view.update(cx, |v, cx| v.select_section(0, w, cx)));
        action(&mut cx, &view, 8);
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).sections[0].clone()),
            two.sections[1]
        );
        // Toolbar Undo/Redo and the shortcut resolve the same document history.
        cx.update(|w, cx| view.read(cx).buttons[8].clone().focus(w, cx));
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx).sections.len()), 2);
        action(&mut cx, &view, 5);
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), two);
        assert_eq!(
            view.read_with(&cx, |v, _| v.section),
            0,
            "slide of the undone edit"
        );
        action(&mut cx, &view, 6);
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx).sections.len()), 1);
        cx.simulate_keystrokes("ctrl-z ctrl-z ctrl-z ctrl-z ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), first);
        edit(&mut cx, &view, 1, "Branch author");
        assert!(view.read_with(&cx, |v, _| v.history.redo.is_empty()));
        cx.simulate_keystrokes("ctrl-z");
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).authors),
            "Branch author"
        );
        action(&mut cx, &view, 4);
        assert!(view.read_with(&cx, |v, _| v.history.undo.is_empty()
            && v.history.redo.is_empty()));
    }

    #[gpui::test]
    fn save_duplicate_undo_keep_baseline_version_and_failed_save_history(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite");
        let (mut cx, view) = fixture(cx, path.clone());
        edit(&mut cx, &view, 0, "Saved title");
        type_cell(&mut cx, &view, 0, LYRICS, "Saved lyrics");
        action(&mut cx, &view, 1);
        // Busy undo cannot modify an in-flight payload.
        cx.simulate_keystrokes("ctrl-z");
        wait(&mut cx, &view);
        let saved = view.read_with(&cx, |v, _| v.version.unwrap());
        cx.simulate_keystrokes("ctrl-z");
        assert!(view.read_with(&cx, |v, cx| v.dirty(cx)
            && v.version == Some(saved)
            && v.baseline.sections[0].lyrics == "Saved lyrics"));
        cx.update(|_, cx| view.update(cx, |v, cx| assert!(!v.may_close(cx))));
        cx.simulate_keystrokes("ctrl-shift-z");
        assert!(!view.read_with(&cx, |v, cx| v.dirty(cx)));
        // Saving from an undone position must preserve the future branch.
        cx.simulate_keystrokes("ctrl-z");
        action(&mut cx, &view, 1);
        wait(&mut cx, &view);
        let undone_saved = view.read_with(&cx, |v, _| v.version.unwrap());
        assert_eq!(view.read_with(&cx, |v, _| v.history.redo.len()), 1);
        cx.simulate_keystrokes("ctrl-shift-z");
        assert!(view.read_with(&cx, |v, cx| v.dirty(cx) && v.version == Some(undone_saved)));
        action(&mut cx, &view, 1);
        wait(&mut cx, &view);
        action(&mut cx, &view, 2);
        wait(&mut cx, &view);
        let duplicate = view.read_with(&cx, |v, _| v.version.unwrap());
        assert_ne!(duplicate.id, saved.id);
        cx.simulate_keystrokes("ctrl-z");
        assert!(view.read_with(&cx, |v, cx| v.dirty(cx) && v.version == Some(duplicate)));
        cx.simulate_keystrokes("ctrl-shift-z");
        let mut other = Repository::open(&path).unwrap();
        let baseline = other.song(duplicate).unwrap();
        other.save_song(Some(duplicate), baseline.clone()).unwrap();
        type_cell(&mut cx, &view, 0, LYRICS, "Conflict draft");
        let before = view.read_with(&cx, |v, _| v.history.undo.len());
        action(&mut cx, &view, 1);
        wait(&mut cx, &view);
        assert_eq!(view.read_with(&cx, |v, _| v.history.undo.len()), before);
        assert!(
            view.read_with(&cx, |v, _| v.status.starts_with("Changed elsewhere")
                && v.baseline == baseline
                && v.version == Some(duplicate))
        );
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), baseline);
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).sections[0].lyrics.clone()),
            "Conflict draft"
        );
        // Validation failures retain both stacks too.
        edit(&mut cx, &view, 0, "");
        action(&mut cx, &view, 1);
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).title),
            "Saved title"
        );
        action(&mut cx, &view, 4);
        cx.update(|_, cx| view.update(cx, |v, cx| v.select(saved, cx)));
        wait(&mut cx, &view);
        assert!(view.read_with(&cx, |v, _| v.history.undo.is_empty()
            && v.history.redo.is_empty()));
        edit(&mut cx, &view, 1, "New boundary");
        cx.simulate_keystrokes("ctrl-z");
        action(&mut cx, &view, 0);
        assert!(view.read_with(&cx, |v, cx| v.current(cx) == v.baseline
            && v.current(cx).title.is_empty()
            && v.current(cx).sections.len() == 1
            && v.current(cx).sections[0].lyrics.is_empty()
            && v.current(cx).sections[0].id != baseline.sections[0].id
            && v.history.undo.is_empty()
            && v.history.redo.is_empty()
            && v.version.is_none()));
    }

    #[test]
    fn combined_history_count_and_byte_budget() {
        let mut history = History::default();
        for n in 0..100 {
            let mut song = blank();
            song.title = n.to_string();
            history.record(Document { song, section: 0 });
        }
        assert_eq!(history.undo.len(), HISTORY_LIMIT);
        assert_eq!(history.undo[0].song.title, "36");
        let mut song = blank();
        song.sections[0].lyrics = "😀".repeat(64 * 1024);
        for _ in 0..100 {
            history.record(Document {
                song: song.clone(),
                section: 0,
            });
        }
        assert!(history.undo.len() < HISTORY_LIMIT);
        assert!(history.undo.iter().map(Document::bytes).sum::<usize>() <= HISTORY_BYTES);
        for _ in 0..100 {
            history.step(
                Document {
                    song: song.clone(),
                    section: 0,
                },
                false,
            );
        }
        assert!(history.undo.is_empty());
        assert!(history.redo.iter().map(Document::bytes).sum::<usize>() <= HISTORY_BYTES);
        history.record(Document {
            song: blank(),
            section: 0,
        });
        assert!(history.redo.is_empty());
    }

    #[gpui::test]
    fn structural_limits_zero_sections_and_unavailable_save_are_atomic(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = fixture(cx, dir.path().to_path_buf()); // DB open fails, not a writable file.
        edit(&mut cx, &view, 0, "T");
        type_cell(&mut cx, &view, 0, LYRICS, "Retain me 😀");
        let original = view.read_with(&cx, |v, cx| v.current(cx));
        let count = view.read_with(&cx, |v, _| v.history.undo.len());
        action(&mut cx, &view, 1);
        wait(&mut cx, &view);
        assert_eq!(view.read_with(&cx, |v, _| v.history.undo.len()), count);
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), original);
        cx.simulate_keystrokes("ctrl-z ctrl-shift-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), original);
        cx.update(|w, cx| view.read(cx).buttons[8].clone().focus(w, cx));
        action(&mut cx, &view, 8);
        assert!(
            view.read_with(&cx, |v, cx| v.current(cx).sections.is_empty()
                && v.cells.is_empty())
        );
        cx.run_until_parked();
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), original);
        cx.simulate_keystrokes("ctrl-shift-z");
        action(&mut cx, &view, 7);
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx).sections.len()), 1);

        let mut full = blank();
        full.title = "T".into();
        full.sections[0].lyrics = "x".repeat(256 * 1024 - 32);
        full.validate().unwrap();
        cx.update(|_, cx| view.update(cx, |v, cx| v.begin(full.clone(), None, cx)));
        edit(&mut cx, &view, 1, "a");
        cx.simulate_keystrokes("ctrl-z");
        let redo = view.read_with(&cx, |v, _| v.history.redo.len());
        action(&mut cx, &view, 7);
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), full);
        assert_eq!(view.read_with(&cx, |v, _| v.history.redo.len()), redo);
        assert_eq!(view.read_with(&cx, |v, _| v.section), 0);
        // A rejected native edit must not become an undo entry or truncate redo.
        edit(&mut cx, &view, 0, &"😀".repeat(257));
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), full);
        assert_eq!(view.read_with(&cx, |v, _| v.history.redo.len()), redo);
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx).authors), "a");

        let mut many = blank();
        many.title = "128 sections".into();
        many.sections = (0..128)
            .map(|n| Section {
                id: SectionId::allocate(),
                label: n.to_string(),
                lyrics: format!("unique {n}"),
                format: Default::default(),
                background: None,
            })
            .collect();
        cx.update(|_, cx| view.update(cx, |v, cx| v.begin(many.clone(), None, cx)));
        edit(&mut cx, &view, 0, "Changed");
        cx.simulate_keystrokes("ctrl-z");
        action(&mut cx, &view, 7);
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), many);
        assert_eq!(view.read_with(&cx, |v, _| v.history.redo.len()), 1);
    }

    #[gpui::test]
    fn save_reopen_sections_duplicate_and_close_guard(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/library.sqlite");
        let (mut cx, view) = fixture(cx, path.clone());
        edit(&mut cx, &view, 0, "Original Café");
        edit(&mut cx, &view, 1, "Original author");
        type_cell(&mut cx, &view, 0, LYRICS, "First\r\nline e\u{301}\n");
        action(&mut cx, &view, 7);
        type_cell(&mut cx, &view, 1, LABEL, "Chorus");
        type_cell(&mut cx, &view, 1, LYRICS, "Different second section");
        cx.simulate_keystrokes("ctrl-s");
        cx.update(|_, cx| {
            view.update(cx, |v, cx| {
                assert!(!v.may_close(cx), "pending save must not be abandoned")
            })
        });
        wait(&mut cx, &view);
        let first = view.read_with(&cx, |v, cx| {
            assert!(!v.dirty(cx));
            v.version.unwrap()
        });
        let repo = Repository::open(&path).unwrap();
        let saved = repo.song(first).unwrap();
        assert_eq!(saved.title, "Original Café");
        assert_eq!(
            saved.sections,
            vec![
                Section {
                    id: saved.sections[0].id,
                    label: String::new(),
                    lyrics: "First\r\nline e\u{301}\n".into(),
                    format: Default::default(),
                    background: None,
                },
                Section {
                    id: saved.sections[1].id,
                    label: "Chorus".into(),
                    lyrics: "Different second section".into(),
                    format: Default::default(),
                    background: None,
                },
            ]
        );
        edit(&mut cx, &view, 0, "Unsaved");
        cx.simulate_keystrokes("ctrl-q");
        assert!(view.read_with(&cx, |v, _| v.confirm_close));
        action(&mut cx, &view, 0);
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx).title), "Unsaved");
        action(&mut cx, &view, 4);
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), saved);
        action(&mut cx, &view, 2);
        wait(&mut cx, &view);
        let duplicate = view.read_with(&cx, |v, _| v.version.unwrap());
        assert_ne!(first.id, duplicate.id);
        assert_eq!(repo.song(duplicate).unwrap(), saved);
        action(&mut cx, &view, 3);
        assert_eq!(
            repo.catalog(None).unwrap().len(),
            2,
            "delete requires confirmation"
        );
        action(&mut cx, &view, 3);
        wait(&mut cx, &view);
        assert_eq!(repo.heads(false, None).unwrap(), vec![first]);
        assert_eq!(
            repo.song(duplicate).unwrap(),
            saved,
            "historical content retained"
        );
        assert!(view.read_with(&cx, |v, _| v.history.undo.is_empty()
            && v.history.redo.is_empty()));
    }

    #[gpui::test]
    fn stale_write_and_invalid_load_keep_the_editable_draft(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite");
        let (mut cx, view) = fixture(cx, path.clone());
        field(&mut cx, &view, 0, "Shared original");
        cx.simulate_keystrokes("ctrl-s");
        wait(&mut cx, &view);
        let original = view.read_with(&cx, |v, _| v.version.unwrap());
        let mut other = Repository::open(&path).unwrap();
        let mut latest = other.song(original).unwrap();
        latest.title = "Other writer".into();
        let newer = other.save_song(Some(original), latest.clone()).unwrap();
        field(&mut cx, &view, 0, "My unsaved edit");
        cx.simulate_keystrokes("ctrl-s");
        wait(&mut cx, &view);
        assert!(view.read_with(&cx, |v, cx| v.dirty(cx)
            && v.status.starts_with("Changed elsewhere")));
        assert_eq!(other.song(newer).unwrap(), latest);
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).title),
            "My unsaved edit"
        );
        action(&mut cx, &view, 4);
        edit(&mut cx, &view, 1, "Future author");
        cx.simulate_keystrokes("ctrl-z");
        latest.authors = "metadata\nnot supported by this editor".into();
        let incompatible = other.save_song(None, latest).unwrap();
        cx.update(|_, cx| view.update(cx, |v, cx| v.select(incompatible, cx)));
        wait(&mut cx, &view);
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).title),
            "Shared original"
        );
        assert!(view.read_with(&cx, |v, _| v.status.contains("Original unchanged")));
        assert_eq!(view.read_with(&cx, |v, _| v.history.redo.len()), 1);
        // Pending load hid fields; restore a rendered contextual focus explicitly.
        cx.update(|w, cx| view.read(cx).focus.clone().focus(w, cx));
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).authors),
            "Future author"
        );
    }

    /// Two slides, the caret in slide 1's lyrics, the Format pane open.
    fn format_fixture(
        cx: &mut TestAppContext,
        path: PathBuf,
    ) -> (VisualTestContext, Entity<Library>) {
        let (mut cx, view) = fixture(cx, path);
        cx.simulate_resize(size(px(1400.), px(900.)));
        field(&mut cx, &view, 0, "Formatted");
        type_cell(&mut cx, &view, 0, LYRICS, "One");
        action(&mut cx, &view, 7);
        type_cell(&mut cx, &view, 1, LYRICS, "Two");
        caret(&mut cx, &view, 0, LYRICS, 0);
        action(&mut cx, &view, 18);
        cx.run_until_parked();
        (cx, view)
    }
    fn click(cx: &mut VisualTestContext, selector: &str) {
        let bounds = cx
            .debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
            .unwrap_or_else(|| panic!("{selector} is not rendered"));
        cx.simulate_click(bounds.center(), Default::default());
        cx.run_until_parked();
    }
    fn formats(cx: &VisualTestContext, view: &Entity<Library>) -> Vec<sela::format::SlideFormat> {
        view.read_with(cx, |v, _| {
            v.draft.sections.iter().map(|s| s.format.clone()).collect()
        })
    }
    fn undo_len(cx: &VisualTestContext, view: &Entity<Library>) -> usize {
        view.read_with(cx, |v, _| v.history.undo.len())
    }

    #[gpui::test]
    fn format_pane_applies_to_the_caret_slide_as_one_undo_step(cx: &mut TestAppContext) {
        use format_pane::OUTLINE;
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = format_fixture(cx, dir.path().join("library.sqlite"));
        let count = undo_len(&cx, &view);
        click(&mut cx, "format-Bold");
        let f = formats(&cx, &view);
        assert_eq!((f[0].bold, f[1].bold), (Some(true), None));
        assert_eq!(undo_len(&cx, &view), count + 1);
        // Pane buttons keep the caret in Words (EW8-OBS-035).
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(formats(&cx, &view)[0].bold, None);
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(formats(&cx, &view)[0].bold, Some(true));

        click(&mut cx, "format-OutlineType");
        assert!(view.read_with(&cx, |v, _| v.pane.menu.is_some()));
        // Center and Inner are listed but unavailable.
        click(&mut cx, "format-menu-2");
        assert_eq!(formats(&cx, &view)[0].outline, None);
        click(&mut cx, "format-menu-1");
        assert_eq!(formats(&cx, &view)[0].outline, Some(OUTLINE));
        assert!(view.read_with(&cx, |v, _| v.pane.menu.is_none()));
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));

        // The pane follows the caret slide.
        caret(&mut cx, &view, 1, LYRICS, 0);
        click(&mut cx, "format-Right");
        let f = formats(&cx, &view);
        assert_eq!(
            (f[0].align, f[1].align),
            (None, Some(sela::format::Align::Right))
        );
        assert_eq!(f[1].bold, None);

        // Nothing applies while storage work is pending.
        view.update(&mut cx, |v, _| v.pending = Some(Pending::Catalog));
        let before = formats(&cx, &view);
        cx.update(|_, cx| view.update(cx, |v, cx| v.toggle_style(format_pane::Ctl::Bold, cx)));
        assert_eq!(formats(&cx, &view), before);
    }

    #[gpui::test]
    fn menus_open_by_keyboard_and_a_dismissing_click_does_not_reopen(cx: &mut TestAppContext) {
        use sela::format::Size;
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = format_fixture(cx, dir.path().join("library.sqlite"));
        let fitted = view
            .read_with(&cx, |v, cx| v.fitted(cx))
            .expect("preview landed");
        click(&mut cx, "format-Size");
        // Auto is checked; Up then Enter picks "Do not auto size text",
        // which starts from the fitted size (EW8-OBS-028).
        cx.simulate_keystrokes("up enter");
        assert_eq!(formats(&cx, &view)[0].size, Some(Size::Fixed(fitted)));
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
        click(&mut cx, "format-Size");
        cx.simulate_keystrokes("escape");
        assert!(view.read_with(&cx, |v, _| v.pane.menu.is_none()));
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
        click(&mut cx, "format-Size");
        click(&mut cx, "format-Size");
        assert!(view.read_with(&cx, |v, _| v.pane.menu.is_none()));
        click(&mut cx, "format-SizeUp");
        assert_eq!(
            formats(&cx, &view)[0].size,
            Some(Size::Fixed(fitted + format_pane::SIZE_STEP))
        );
    }

    #[gpui::test]
    fn number_fields_refuse_invalid_values_and_hex_sets_the_color(cx: &mut TestAppContext) {
        use sela::format::Size;
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = format_fixture(cx, dir.path().join("library.sqlite"));
        cx.update(|w, cx| view.update(cx, |v, cx| v.choose(format_pane::Menu::Size, 0, w, cx)));
        cx.run_until_parked();
        let size = formats(&cx, &view)[0].size;
        let field = view.read_with(&cx, |v, _| v.pane.nums[0].clone());
        let count = undo_len(&cx, &view);
        for bad in ["9999", "0", "big"] {
            replace(&mut cx, field.clone(), bad);
            cx.simulate_keystrokes("enter");
            cx.run_until_parked();
            assert_eq!(formats(&cx, &view)[0].size, size, "{bad}");
            assert!(view.read_with(&cx, |v, _| v.status.starts_with("Size must be")));
            let Some(Size::Fixed(n)) = size else {
                unreachable!()
            };
            assert_eq!(
                field.read_with(&cx, |f, _| f.text().to_owned()),
                n.to_string()
            );
        }
        assert_eq!(undo_len(&cx, &view), count);
        replace(&mut cx, field.clone(), "40");
        cx.simulate_keystrokes("enter");
        assert_eq!(formats(&cx, &view)[0].size, Some(Size::Fixed(40)));
        // Up in the field steps the value.
        cx.simulate_keystrokes("up");
        assert_eq!(formats(&cx, &view)[0].size, Some(Size::Fixed(41)));
        assert_eq!(undo_len(&cx, &view), count + 2);

        caret(&mut cx, &view, 0, LYRICS, 0);
        click(&mut cx, "format-Color");
        let hex = view.read_with(&cx, |v, _| v.pane.hex.clone());
        assert_eq!(hex.read_with(&cx, |f, _| f.text().to_owned()), "#FFFFFF");
        replace(&mut cx, hex.clone(), "#12345g");
        cx.simulate_keystrokes("enter");
        assert_eq!(formats(&cx, &view)[0].color, None);
        assert!(view.read_with(&cx, |v, _| v.status.starts_with("Enter a color")));
        replace(&mut cx, hex, "#ff0000");
        cx.simulate_keystrokes("enter");
        assert_eq!(formats(&cx, &view)[0].color, Some([255, 0, 0]));
        assert!(view.read_with(&cx, |v, _| v.pane.menu.is_none()));
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
    }

    #[gpui::test]
    fn slider_and_dial_drags_are_one_undo_step_and_save_round_trips(cx: &mut TestAppContext) {
        use format_pane::{OUTLINE, SHADOW};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite");
        let (mut cx, view) = format_fixture(cx, path.clone());
        // Disabled until Outline is on: a press changes nothing.
        let track = cx.debug_bounds("format-slider-OutlineSize").unwrap();
        cx.simulate_mouse_down(track.center(), MouseButton::Left, Default::default());
        cx.simulate_mouse_up(track.center(), MouseButton::Left, Default::default());
        assert_eq!(formats(&cx, &view)[0].outline, None);
        click(&mut cx, "format-OutlineType");
        click(&mut cx, "format-menu-1");
        let count = undo_len(&cx, &view);
        let track = cx.debug_bounds("format-slider-OutlineSize").unwrap();
        let y = track.center().y;
        cx.simulate_mouse_down(
            point(track.left(), y),
            MouseButton::Left,
            Default::default(),
        );
        assert_eq!(formats(&cx, &view)[0].outline.unwrap().size, 1);
        cx.simulate_mouse_move(track.center(), Some(MouseButton::Left), Default::default());
        cx.simulate_mouse_move(
            point(track.right() + px(80.), y),
            Some(MouseButton::Left),
            Default::default(),
        );
        assert_eq!(formats(&cx, &view)[0].outline.unwrap().size, 50, "clamped");
        cx.simulate_mouse_up(
            point(track.right() + px(80.), y),
            MouseButton::Left,
            Default::default(),
        );
        cx.run_until_parked();
        assert_eq!(
            undo_len(&cx, &view),
            count + 1,
            "one step for the whole drag"
        );
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(formats(&cx, &view)[0].outline, Some(OUTLINE));
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(formats(&cx, &view)[0].outline.unwrap().size, 50);

        click(&mut cx, "format-ShadowMode");
        click(&mut cx, "format-menu-1");
        assert_eq!(formats(&cx, &view)[0].shadow, Some(SHADOW));
        let dial = cx.debug_bounds("format-dial").unwrap();
        let up = point(dial.center().x, dial.top());
        cx.simulate_mouse_down(up, MouseButton::Left, Default::default());
        cx.simulate_mouse_up(up, MouseButton::Left, Default::default());
        cx.run_until_parked();
        assert_eq!(formats(&cx, &view)[0].shadow.unwrap().angle, 90);

        cx.simulate_keystrokes("ctrl-s");
        wait(&mut cx, &view);
        let version = view.read_with(&cx, |v, _| v.version.unwrap());
        let saved = Repository::open(&path).unwrap().song(version).unwrap();
        assert_eq!(
            saved
                .sections
                .iter()
                .map(|s| s.format.clone())
                .collect::<Vec<_>>(),
            formats(&cx, &view)
        );
        assert_eq!(saved.sections[1].format, Default::default());
    }

    fn all(cx: &VisualTestContext, view: &Entity<Library>) -> bool {
        view.read_with(cx, |v, _| v.all)
    }
    fn lyrics(cx: &VisualTestContext, view: &Entity<Library>) -> Vec<(String, String)> {
        view.read_with(cx, |v, cx| {
            v.current(cx)
                .sections
                .into_iter()
                .map(|s| (s.label, s.lyrics))
                .collect()
        })
    }
    fn wholly_selected(cx: &VisualTestContext, view: &Entity<Library>) -> bool {
        view.read_with(cx, |v, cx| {
            v.cells.iter().flatten().all(|c| {
                let c = c.read(cx);
                c.selection() == (0..c.text().len())
            })
        })
    }

    #[gpui::test]
    fn ctrl_a_selects_every_slide_and_formats_apply_to_all_in_one_step(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = format_fixture(cx, dir.path().join("library.sqlite"));
        // In a metadata field Ctrl+A stays a field selection.
        field(&mut cx, &view, 0, "Formatted");
        cx.simulate_keystrokes("ctrl-a");
        assert!(!all(&cx, &view));
        caret(&mut cx, &view, 0, LYRICS, 0);

        cx.simulate_keystrokes("ctrl-a");
        cx.run_until_parked();
        assert!(all(&cx, &view));
        assert!(wholly_selected(&cx, &view));
        // EW8-OBS-034: the caret ends in the last slide.
        assert_eq!(focused(&mut cx, &view), Some((1, LYRICS)));
        // EW8-OBS-035: the pane turns to the Slide tab; Text stays reachable.
        assert_eq!(
            view.read_with(&cx, |v, _| v.pane.tab),
            format_pane::Tab::Slide
        );
        click(&mut cx, "format-TabText");
        assert!(all(&cx, &view), "the tab click keeps the selection");
        let count = undo_len(&cx, &view);
        click(&mut cx, "format-Bold");
        let f = formats(&cx, &view);
        assert_eq!((f[0].bold, f[1].bold), (Some(true), Some(true)));
        assert_eq!(undo_len(&cx, &view), count + 1);
        assert!(all(&cx, &view), "pane clicks keep the selection");
        assert_eq!(lyrics(&cx, &view).len(), 2);
        cx.simulate_keystrokes("ctrl-z");
        let f = formats(&cx, &view);
        assert_eq!((f[0].bold, f[1].bold), (None, None));
        assert!(!all(&cx, &view));

        // Slides tab: the thumbnail list takes Ctrl+A too; Words ends it
        // (EW8-OBS-036).
        action(&mut cx, &view, 17);
        let thumbnail = view.read_with(&cx, |v, _| v.section_focus[0].clone());
        cx.update(|w, cx| thumbnail.focus(w, cx));
        cx.simulate_keystrokes("ctrl-a");
        assert!(all(&cx, &view));
        click(&mut cx, "format-TabText");
        click(&mut cx, "format-Italic");
        let f = formats(&cx, &view);
        assert_eq!((f[0].italic, f[1].italic), (Some(true), Some(true)));
        action(&mut cx, &view, 14);
        assert!(!all(&cx, &view));
        click(&mut cx, "format-Underline");
        let f = formats(&cx, &view);
        assert_eq!(
            f.iter().filter(|f| f.underline == Some(true)).count(),
            1,
            "only the caret slide again"
        );
    }

    #[gpui::test]
    fn typing_over_the_whole_song_replaces_it_as_one_undo_step(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = format_fixture(cx, dir.path().join("library.sqlite"));
        type_cell(&mut cx, &view, 0, LABEL, "Verse 1");
        caret(&mut cx, &view, 0, LYRICS, 0);
        click(&mut cx, "format-Bold");
        let original = lyrics(&cx, &view);
        let first = view.read_with(&cx, |v, _| v.draft.sections[0].id);

        cx.simulate_keystrokes("ctrl-a ctrl-c");
        assert!(all(&cx, &view), "Copy keeps the selection");
        assert_eq!(
            cx.update(|_, cx| cx.read_from_clipboard().and_then(|c| c.text())),
            Some("Verse 1\nOne\n\nTwo".to_owned())
        );
        assert_eq!(lyrics(&cx, &view), original);

        let count = undo_len(&cx, &view);
        cx.simulate_input("N");
        cx.run_until_parked();
        assert_eq!(lyrics(&cx, &view), [(String::new(), "N".to_owned())]);
        view.read_with(&cx, |v, _| {
            assert_eq!(v.draft.sections[0].id, first);
            assert_eq!(v.draft.sections[0].format.bold, Some(true));
        });
        assert!(!all(&cx, &view));
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
        assert_eq!(undo_len(&cx, &view), count + 1);
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(lyrics(&cx, &view), original);
        cx.simulate_keystrokes("ctrl-shift-z");
        cx.simulate_input("e");
        assert_eq!(lyrics(&cx, &view)[0].1, "Ne", "typing continues normally");
        // Keys the platform still sends to slide 2's removed cell before
        // the next paint reach the focused cell.
        cx.simulate_keystrokes("ctrl-z ctrl-z ctrl-a");
        let removed = cell(&cx, &view, 1, LYRICS);
        for text in ["N", "ew"] {
            cx.update(|w, cx| {
                removed.update(cx, |c, cx| c.replace_text_in_range(None, text, w, cx))
            });
        }
        cx.run_until_parked();
        assert_eq!(lyrics(&cx, &view), [(String::new(), "New".to_owned())]);
        for _ in 0..4 {
            if lyrics(&cx, &view).len() == 1 {
                cx.simulate_keystrokes("ctrl-z");
            }
        }
        assert_eq!(lyrics(&cx, &view), original);

        cx.simulate_keystrokes("ctrl-a backspace");
        cx.run_until_parked();
        assert_eq!(lyrics(&cx, &view), [(String::new(), String::new())]);
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(lyrics(&cx, &view), original);

        cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("A\nB".into())));
        cx.simulate_keystrokes("ctrl-a ctrl-v");
        cx.run_until_parked();
        assert_eq!(lyrics(&cx, &view), [(String::new(), "A\nB".to_owned())]);
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(lyrics(&cx, &view), original);

        // Caret movement or another cell ends the selection without edits.
        cx.simulate_keystrokes("ctrl-a left");
        assert!(!all(&cx, &view));
        assert!(!wholly_selected(&cx, &view));
        cx.simulate_input("x");
        assert_eq!(lyrics(&cx, &view).len(), 2);
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(lyrics(&cx, &view), original);
        cx.simulate_keystrokes("ctrl-a");
        caret(&mut cx, &view, 0, LYRICS, 0);
        assert!(!all(&cx, &view));
        cx.simulate_keystrokes("enter");
        assert_eq!(lyrics(&cx, &view)[0].1, "\nOne");

        // Nothing applies while storage work is pending.
        cx.simulate_keystrokes("ctrl-z ctrl-a");
        view.update(&mut cx, |v, _| v.pending = Some(Pending::Catalog));
        cx.update(|w, cx| view.update(cx, |v, cx| v.replace_all("gone", w, cx)));
        assert_eq!(lyrics(&cx, &view), original);
    }

    fn backgrounds(
        cx: &VisualTestContext,
        view: &Entity<Library>,
    ) -> Vec<Option<sela::background::Background>> {
        view.read_with(cx, |v, _| {
            v.draft
                .sections
                .iter()
                .map(|s| s.background.clone())
                .collect()
        })
    }
    fn master(
        cx: &VisualTestContext,
        view: &Entity<Library>,
    ) -> Option<sela::background::Background> {
        view.read_with(cx, |v, _| v.draft.master.clone())
    }
    /// Picks a Fill ▾ row by its label.
    fn fill(cx: &mut VisualTestContext, view: &Entity<Library>, label: &str) {
        click(cx, "format-Fill");
        let row = view
            .read_with(cx, |v, _| v.menu_rows(format_pane::Menu::Fill))
            .iter()
            .position(|r| r.0 == label)
            .unwrap_or_else(|| panic!("no {label} row"));
        click(cx, &format!("format-menu-{row}"));
    }

    #[gpui::test]
    fn slide_tab_sets_color_image_and_aspect_as_undo_steps(cx: &mut TestAppContext) {
        use sela::background::{Background, Fill};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite");
        let sky = profile_image(dir.path(), "Sky.png", 160, 90, [40, 90, 200]);
        let (mut cx, view) = format_fixture(cx, path.clone());
        click(&mut cx, "format-TabSlide");
        assert_eq!(backgrounds(&cx, &view), [None, None]);
        let rows = view.read_with(&cx, |v, _| v.menu_rows(format_pane::Menu::Fill));
        assert_eq!(
            rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
            [
                "Master",
                "None",
                "Color Fill",
                "Gradient Fill",
                "Media Fill"
            ]
        );
        assert!(rows[0].2, "a new slide follows the master");
        assert!(!rows[3].1, "Gradient Fill is listed but unavailable");
        assert!(cx.debug_bounds("format-master-note").is_some());

        let count = undo_len(&cx, &view);
        fill(&mut cx, &view, "Color Fill");
        assert_eq!(
            backgrounds(&cx, &view),
            [Some(Background::color(format_pane::COLOR_FILL)), None],
            "EW's first Color Fill is blue, on the caret slide only"
        );
        assert_eq!(undo_len(&cx, &view), count + 1);
        assert_eq!(focused(&mut cx, &view), Some((0, LYRICS)));
        click(&mut cx, "format-BackgroundColor");
        let hex = view.read_with(&cx, |v, _| v.pane.hex.clone());
        assert_eq!(hex.read_with(&cx, |f, _| f.text().to_owned()), "#0000FF");
        replace(&mut cx, hex, "#00ff00");
        cx.simulate_keystrokes("enter");
        assert_eq!(
            backgrounds(&cx, &view)[0],
            Some(Background::color([0, 255, 0]))
        );

        // Media Fill starts without an image (EW8-OBS-038).
        fill(&mut cx, &view, "Media Fill");
        assert_eq!(
            backgrounds(&cx, &view)[0].as_ref().map(|b| b.fill.clone()),
            Some(Fill::Media(None))
        );
        click(&mut cx, "format-SelectMedia");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(&cx, |v, _| v.pane.media.names.clone()),
            Some(Ok(vec!["Sky.png".to_owned()]))
        );
        assert!(
            view.read_with(&cx, |v, _| v
                .pane
                .media
                .thumbs
                .get("Sky.png")
                .is_some_and(Option::is_some)),
            "thumbnail decoded off the UI thread"
        );
        click(&mut cx, "media-item-0");
        cx.run_until_parked();
        assert_eq!(
            backgrounds(&cx, &view)[0],
            Some(Background::image(sky.clone()))
        );
        assert!(
            view.read_with(&cx, |v, _| v.pane.menu.is_none()),
            "one click chooses and closes"
        );

        click(&mut cx, "format-Aspect");
        click(&mut cx, "format-menu-0");
        assert_eq!(
            backgrounds(&cx, &view)[0].as_ref().map(|b| b.aspect),
            Some(Aspect::Maintain)
        );
        // Choosing an image again resets the aspect to Zoom.
        click(&mut cx, "format-SelectMedia");
        cx.run_until_parked();
        click(&mut cx, "media-item-0");
        cx.run_until_parked();
        assert_eq!(
            backgrounds(&cx, &view)[0],
            Some(Background::image(sky.clone()))
        );
        assert_eq!(undo_len(&cx, &view), count + 6);

        cx.simulate_keystrokes("ctrl-z ctrl-z");
        assert_eq!(
            backgrounds(&cx, &view)[0]
                .as_ref()
                .map(|b| (b.image_ref().cloned(), b.aspect)),
            Some((Some(sky.clone()), Aspect::Zoom)),
            "undo restores the image chosen before Maintain"
        );
        cx.simulate_keystrokes("ctrl-shift-z");
        assert_eq!(
            backgrounds(&cx, &view)[0].as_ref().map(|b| b.aspect),
            Some(Aspect::Maintain)
        );
        fill(&mut cx, &view, "Master");
        assert_eq!(backgrounds(&cx, &view), [None, None]);
        cx.simulate_keystrokes("ctrl-z");

        // Import… copies the file into the profile and sets it.
        let source = dir.path().join("Dawn.png");
        image::RgbaImage::from_pixel(32, 18, image::Rgba([220, 120, 40, 255]))
            .save(&source)
            .unwrap();
        click(&mut cx, "format-SelectMedia");
        cx.run_until_parked();
        click(&mut cx, "media-import");
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response(|_| Some(vec![source.clone()]));
        cx.run_until_parked();
        let dawn = backgrounds(&cx, &view)[0]
            .clone()
            .and_then(|b| b.image_ref().cloned());
        assert_eq!(dawn.as_ref().map(|i| i.name.as_str()), Some("Dawn.png"));
        assert!(
            sela::images::images_dir(dir.path())
                .join("Dawn.png")
                .is_file()
        );

        cx.simulate_keystrokes("ctrl-s");
        wait(&mut cx, &view);
        let version = view.read_with(&cx, |v, _| v.version.unwrap());
        let saved = Repository::open(&path).unwrap().song(version).unwrap();
        assert_eq!(
            saved
                .sections
                .iter()
                .map(|s| s.background.clone())
                .collect::<Vec<_>>(),
            backgrounds(&cx, &view)
        );
    }

    #[gpui::test]
    fn ctrl_a_and_the_master_reach_slides_and_cancel_discards(cx: &mut TestAppContext) {
        use sela::background::Background;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite");
        let (mut cx, view) = format_fixture(cx, path.clone());
        click(&mut cx, "format-TabSlide");
        fill(&mut cx, &view, "Color Fill");
        caret(&mut cx, &view, 0, LYRICS, 0);
        cx.simulate_keystrokes("ctrl-a");
        // Different backgrounds show blank (EW8-OBS-042).
        assert_eq!(
            view.read_with(&cx, |v, _| v.pane_background()),
            format_pane::Shown::Mixed
        );
        assert!(
            view.read_with(&cx, |v, _| v.menu_rows(format_pane::Menu::Fill))
                .iter()
                .all(|r| !r.2)
        );
        let count = undo_len(&cx, &view);
        fill(&mut cx, &view, "Color Fill");
        let blue = Some(Background::color(format_pane::COLOR_FILL));
        assert_eq!(backgrounds(&cx, &view), [blue.clone(), blue.clone()]);
        assert_eq!(undo_len(&cx, &view), count + 1, "one step for every slide");
        assert!(all(&cx, &view));
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(backgrounds(&cx, &view), [blue.clone(), None]);

        // Edit Slide Layouts edits the master; slides without their own
        // background follow it (EW8-OBS-043, owner decision).
        click(&mut cx, "format-EditLayouts");
        assert!(view.read_with(&cx, |v, _| v.layouts));
        assert!(cx.debug_bounds("layout-master").is_some());
        assert!(
            cx.debug_bounds("format-TabText").is_none(),
            "Slide pane alone"
        );
        let rows = view.read_with(&cx, |v, _| v.menu_rows(format_pane::Menu::Fill));
        assert_eq!(rows[0].0, "None", "no Master row on the master");
        assert!(rows[0].2);
        let count = undo_len(&cx, &view);
        fill(&mut cx, &view, "Color Fill");
        click(&mut cx, "format-BackgroundColor");
        let hex = view.read_with(&cx, |v, _| v.pane.hex.clone());
        replace(&mut cx, hex, "#802040");
        cx.simulate_keystrokes("enter");
        let wine = Some(Background::color([0x80, 0x20, 0x40]));
        assert_eq!(master(&cx, &view), wine);
        assert_eq!(backgrounds(&cx, &view), [blue.clone(), None]);
        assert_eq!(undo_len(&cx, &view), count + 2);
        view.read_with(&cx, |v, cx| {
            let preview = v.preview_slide(cx).unwrap();
            assert_eq!(preview.text, LAYOUT_SAMPLE);
            assert_eq!(preview.background, wine);
            let slides: Vec<_> = v
                .draft
                .sections
                .iter()
                .map(|s| sela::slides::section_slide(s, v.draft.master.as_ref()).background)
                .collect();
            assert_eq!(slides, [blue.clone(), wine.clone()]);
        });
        cx.update(|w, cx| view.update(cx, |v, cx| v.action(19, w, cx)));
        assert!(!view.read_with(&cx, |v, _| v.layouts));
        caret(&mut cx, &view, 1, LYRICS, 0);
        cx.update(|_, cx| view.update(cx, |v, cx| v.ensure_preview(cx)));
        cx.run_until_parked();
        assert!(cx.debug_bounds("format-master-note").is_some());
        assert_eq!(
            view.read_with(&cx, |v, cx| v.preview_slide(cx).unwrap().background),
            wine
        );
        // Choosing None on the master is the same as no master.
        click(&mut cx, "format-EditLayouts");
        fill(&mut cx, &view, "None");
        assert_eq!(master(&cx, &view), None);
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(master(&cx, &view), wine);
        action(&mut cx, &view, 14);
        assert!(
            !view.read_with(&cx, |v, _| v.layouts),
            "Words closes Layouts"
        );

        // Cancel asks before discarding; nothing reaches the library.
        cx.update(|w, cx| view.update(cx, |v, cx| v.action(16, w, cx)));
        assert!(view.read_with(&cx, |v, _| v.confirm_close));
        assert!(
            Repository::open(&path)
                .unwrap()
                .catalog(None)
                .unwrap()
                .is_empty()
        );
    }

    #[gpui::test]
    fn a_changed_slide_image_previews_black_with_the_warning(cx: &mut TestAppContext) {
        use sela::background::Background;
        let dir = tempfile::tempdir().unwrap();
        let mut sky = profile_image(dir.path(), "Sky.png", 160, 90, [40, 90, 200]);
        sky.sha256[0] ^= 1;
        let (mut cx, view) = format_fixture(cx, dir.path().join("library.sqlite"));
        click(&mut cx, "format-TabSlide");
        cx.update(|_, cx| {
            view.update(cx, |v, cx| v.choose_image(sky.clone(), cx));
        });
        cx.run_until_parked();
        assert_eq!(backgrounds(&cx, &view)[0], Some(Background::image(sky)));
        cx.update(|_, cx| view.update(cx, |v, cx| v.ensure_preview(cx)));
        cx.run_until_parked();
        let (warning, corner) = view.read_with(&cx, |v, _| {
            let p = v.preview.as_ref().unwrap();
            (p.warning.clone(), p.image.as_ref().map(|i| rgb_at(i, 2, 2)))
        });
        assert_eq!(
            warning.as_deref(),
            Some("Background image ‘Sky.png’ changed since it was chosen; showing black")
        );
        assert_eq!(corner, Some([0; 3]));
        assert_eq!(
            cx.debug_bounds("format-media-name").map(|_| ()),
            Some(()),
            "the pane still names the image"
        );
    }
}
