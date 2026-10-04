//! Provisional native authoring window. All durable I/O belongs to storage::Worker.
use crate::{
    ActivateControl, FocusNext, FocusPrevious, Quit,
    text_input::{Redo, TextInput, Undo},
};
use gpui::{prelude::*, *};
use sela::storage::{Command, Error, Id, Reply, Section, Song, Version, Worker};
use std::{path::PathBuf, time::Duration};

actions!(song_library, [Save]);

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

pub fn open(path: PathBuf, cx: &mut App) -> Result<(), String> {
    let bounds = Bounds::centered(None, size(px(980.), px(760.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(720.), px(440.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Sela — Song editor".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| Library::new(path, cx));
            let weak = view.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                weak.update(cx, |view, cx| view.may_close(cx))
                    .unwrap_or(true)
            });
            view.read(cx).fields[0]
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
            view
        },
    )
    .map_err(|_| "Cannot open song library window".to_string())?;
    Ok(())
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
        sections: vec![Section {
            label: "Verse 1".into(),
            lyrics: String::new(),
        }],
    }
}

const HISTORY_LIMIT: usize = 64;
const HISTORY_BYTES: usize = 8 * 1024 * 1024;

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
                .sections
                .iter()
                .map(|s| std::mem::size_of::<Section>() + s.label.len() + s.lyrics.len())
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

struct Library {
    focus: FocusHandle,
    fields: [Entity<TextInput>; 6],
    field_edits: [u64; 6],
    buttons: [FocusHandle; 18],
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
                let mut input = TextInput::new(
                    "",
                    i == 5,
                    [1024, 4096, 4096, 4096, 256, 256 * 1024][i],
                    20 + i as isize,
                    cx,
                )
                .expect("empty text is valid");
                input.use_document_history(move |text, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.field_edit(i, text);
                        cx.notify();
                    });
                });
                input
            })
        });
        let (worker, pending, status) = match Worker::open(path) {
            Ok(worker) => (Some(worker), Some(Pending::Open), "Opening library…".into()),
            Err(e) => (None, None, error_message(e).into()),
        };
        let mut this = Self {
            focus: cx.focus_handle(),
            fields,
            field_edits: [0; 6],
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
            draft: blank(),
            baseline: blank(),
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
        };
        this.load_fields(cx);
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
        this
    }

    fn current(&self, cx: &App) -> Song {
        let mut song = self.draft.clone();
        song.title = self.fields[0].read(cx).text().into();
        song.authors = self.fields[1].read(cx).text().into();
        song.copyright = self.fields[2].read(cx).text().into();
        song.license = self.fields[3].read(cx).text().into();
        if let Some(section) = song.sections.get_mut(self.section) {
            section.label = self.fields[4].read(cx).text().into();
            section.lyrics = self.fields[5].read(cx).text().into();
        }
        song
    }
    fn dirty(&self, cx: &App) -> bool {
        self.current(cx) != self.baseline
    }
    // Called synchronously by the field. Never read the borrowed field here.
    fn field_edit(&mut self, index: usize, text: &str) {
        let mut song = self.draft.clone();
        let value = match index {
            0 => &mut song.title,
            1 => &mut song.authors,
            2 => &mut song.copyright,
            3 => &mut song.license,
            4 => match song.sections.get_mut(self.section) {
                Some(s) => &mut s.label,
                None => return,
            },
            _ => match song.sections.get_mut(self.section) {
                Some(s) => &mut s.lyrics,
                None => return,
            },
        };
        *value = text.into();
        if song != self.draft
            && (self.status.starts_with("A title is required")
                || self.status.starts_with("An input was rejected")
                || self.status.starts_with("Song is invalid"))
        {
            self.status = "Draft changed · validate with Save or OK".into();
        }
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
    fn history(&mut self, redo: bool, cx: &mut Context<Self>) {
        if self.pending.is_some() {
            return;
        }
        self.record(self.current(cx));
        if let Some(next) = self.history.step(
            Document {
                song: self.draft.clone(),
                section: self.section,
            },
            redo,
        ) {
            self.draft = next.song;
            self.section = next.section;
            self.confirm_delete = false;
            self.load_fields(cx);
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
        if self.pending.is_some() {
            return false;
        }
        if self.dirty(cx) {
            self.status = "Save or discard your edits before changing songs.".into();
            cx.notify();
            return false;
        }
        true
    }
    fn load_fields(&mut self, cx: &mut Context<Self>) {
        let empty = Section {
            label: String::new(),
            lyrics: String::new(),
        };
        let section = self.draft.sections.get(self.section).unwrap_or(&empty);
        for (field, value) in self.fields.iter().zip([
            &self.draft.title,
            &self.draft.authors,
            &self.draft.copyright,
            &self.draft.license,
            &section.label,
            &section.lyrics,
        ]) {
            field
                .update(cx, |field, cx| field.set_text(value, cx))
                .expect("validated editable song");
        }
    }
    fn begin(&mut self, song: Song, version: Option<Version>, cx: &mut Context<Self>) {
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
                if self.close_after_save {
                    self.committed_close = true;
                    self.close_after_save = false;
                } else {
                    self.refresh(cx);
                }
            }
            (Some(Pending::Delete), Ok(Reply::Deleted)) => {
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
        cx.notify();
    }
    fn action(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.is_some() {
            return;
        }
        self.record(self.current(cx));
        match index {
            12 => {
                self.inspector = !self.inspector;
                self.show_catalog = false;
            }
            13 => self.show_catalog = !self.show_catalog,
            14 => self.slides = false,
            17 => self.slides = true,
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
            }
            1 | 2 => {
                let song = self.current(cx);
                if self
                    .fields
                    .iter()
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
            5..=8 => {
                let song = self.current(cx);
                // Preserve a blank title while authoring, but bound total section payload.
                let mut bounded = song.clone();
                if bounded.title.trim().is_empty() {
                    bounded.title = "Untitled".into();
                }
                if bounded.validate().is_err() {
                    self.status = "Song too large. Shorten this section before switching.".into();
                } else {
                    let before = Document {
                        song: song.clone(),
                        section: self.section,
                    };
                    let mut candidate = song;
                    let mut section = self.section;
                    match index {
                        5 => section = section.saturating_sub(1),
                        6 => {
                            section = (section + 1).min(candidate.sections.len().saturating_sub(1))
                        }
                        7 if candidate.sections.len() < 128 => {
                            candidate.sections.push(Section {
                                label: format!("Section {}", candidate.sections.len() + 1),
                                lyrics: String::new(),
                            });
                            section = candidate.sections.len() - 1;
                        }
                        8 if !candidate.sections.is_empty() => {
                            candidate.sections.remove(section);
                            section = section.min(candidate.sections.len().saturating_sub(1));
                        }
                        _ => (),
                    }
                    if candidate == before.song && section == before.section {
                        if index == 7 {
                            self.status = "At most 128 sections. Original unchanged.".into();
                            cx.notify();
                        }
                        return;
                    }
                    let mut bounded = candidate.clone();
                    if bounded.title.trim().is_empty() {
                        bounded.title = "Untitled".into();
                    }
                    if bounded.validate().is_err() {
                        self.status =
                            "Section change exceeds the song limit. Original unchanged.".into();
                        cx.notify();
                        return;
                    }
                    if candidate != before.song {
                        self.history.record(before);
                        self.confirm_delete = false;
                    }
                    self.draft = candidate;
                    self.section = section;
                    self.load_fields(cx);
                }
            }
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

    fn select_section(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.pending.is_some() || index >= self.draft.sections.len() {
            return;
        }
        let song = self.current(cx);
        let mut bounded = song.clone();
        if bounded.title.trim().is_empty() {
            bounded.title = "Untitled".into();
        }
        if bounded.validate().is_err() {
            self.status = "Song too large. Shorten this section before switching.".into();
        } else {
            self.record(song.clone());
            self.draft = song;
            self.section = index;
            self.load_fields(cx);
        }
        cx.notify();
    }
    fn button(&self, index: usize, label: &str, cx: &mut Context<Self>) -> impl IntoElement {
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
    }
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
        let busy = self.pending.is_some();
        let dirty = self.dirty(cx);
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
            .on_action(cx.listener(|s, _: &Undo, _, cx| s.history(false, cx)))
            .on_action(cx.listener(|s, _: &Redo, _, cx| s.history(true, cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf4f4f3))
            .text_color(rgb(0x292c30))
            .font_family("DejaVu Sans")
            .text_size(px(13.))
            .child(
                div()
                    .id("editor-toolbar")
                    .overflow_x_scroll()
                    .h(px(44.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(0xdcdedc))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Song editor"))
                    .when(!busy, |d| {
                        d.child(self.button(13, "Library", cx))
                            .child(self.button(0, "New", cx))
                            .child(self.button(1, "Save", cx))
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
                            .child(self.button(4, "Discard edits", cx))
                    })
                    .when(f32::from(window.viewport_size().width) >= 900., |d| {
                        d.child(if dirty {
                            "Unsaved"
                        } else {
                            "Saved / unchanged"
                        })
                    })
                    .child(div().flex_1())
                    .when(!busy, |d| d.child(self.button(12, "Inspector", cx))),
            )
            .child(div().px_3().py_2().text_size(px(12.)).child(format!(
                "{} · {}",
                if dirty { "Unsaved" } else { "Unchanged" },
                self.status
            )))
            .when(self.confirm_close && !busy, |d| {
                d.child(
                    div()
                        .px_3()
                        .py_2()
                        .bg(rgb(0xfff1dc))
                        .flex()
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
                    .when(self.show_catalog, |d| {
                        d.child(
                            div()
                                .w(px(220.))
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
                    .child(if busy {
                        div()
                            .flex_1()
                            .p_6()
                            .child("Loading or saving…")
                            .into_any_element()
                    } else {
                        div()
                            .id("song-form")
                            .w(px(
                                (f32::from(window.viewport_size().width) * 0.46).max(310.)
                            ))
                            .flex_shrink_0()
                            .min_w_0()
                            .p_4()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .child("Title")
                                    .child(self.fields[0].clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_shrink_0()
                                    .child(
                                        div()
                                            .border_b_2()
                                            .border_color(rgb(if !self.slides {
                                                0x536aca
                                            } else {
                                                0xf4f4f3
                                            }))
                                            .child(self.button(14, "Words", cx)),
                                    )
                                    .child(
                                        div()
                                            .border_b_2()
                                            .border_color(rgb(if self.slides {
                                                0x536aca
                                            } else {
                                                0xf4f4f3
                                            }))
                                            .child(self.button(17, "Slides", cx)),
                                    ),
                            )
                            .when(self.slides, |d| {
                                d.child(
                                    div().text_size(px(11.)).text_color(rgb(0x646971)).child(
                                        "Draft sections · first 4 lines · not rendered slides",
                                    ),
                                )
                            })
                            .children(self.draft.sections.iter().enumerate().map(|(i, section)| {
                                div()
                                    .id(("draft-section", i))
                                    .track_focus(&self.section_focus[i])
                                    .key_context("SelaControl")
                                    .flex_shrink_0()
                                    .p_2()
                                    .border_b_1()
                                    .border_color(rgb(0xdcdedc))
                                    .bg(rgb(if i == self.section {
                                        0xe3e7f3
                                    } else {
                                        0xfafaf9
                                    }))
                                    .focus(|d| d.bg(rgb(0xdce3fa)))
                                    .cursor_pointer()
                                    .on_action(cx.listener(move |s, _: &ActivateControl, w, cx| {
                                        w.prevent_default();
                                        s.select_section(i, cx);
                                    }))
                                    .on_click(cx.listener(move |s, event, w, cx| {
                                        if matches!(event, ClickEvent::Keyboard(_)) {
                                            return;
                                        }
                                        s.section_focus[i].focus(w, cx);
                                        s.select_section(i, cx);
                                    }))
                                    .child(format!("{} · {}", i + 1, section.label))
                                    .when(self.slides, |d| {
                                        d.child(
                                            div()
                                                .mt_2()
                                                .p_2()
                                                .bg(rgb(0x202226))
                                                .text_color(rgb(0xfafaf9))
                                                .children(section.lyrics.split('\n').take(4).map(
                                                    |line| {
                                                        div().child(
                                                            line.trim_end_matches('\r').to_owned(),
                                                        )
                                                    },
                                                )),
                                        )
                                    })
                            }))
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(format!(
                                        "Section {} of {}",
                                        if self.draft.sections.is_empty() {
                                            0
                                        } else {
                                            self.section + 1
                                        },
                                        self.draft.sections.len()
                                    ))
                                    .child(self.button(5, "Previous", cx))
                                    .child(self.button(6, "Next", cx)),
                            )
                            .when(!self.draft.sections.is_empty(), |d| {
                                d.child(
                                    div()
                                        .flex_shrink_0()
                                        .child("Section label")
                                        .child(self.fields[4].clone()),
                                )
                                .when(!self.slides, |d| {
                                    d.child(
                                        div()
                                            .flex_shrink_0()
                                            .child("Lyrics · line breaks are preserved")
                                            .child(self.fields[5].clone()),
                                    )
                                })
                            })
                            .into_any_element()
                    })
                    .when(!busy && !self.show_catalog, |d| {
                        d.child(
                            div()
                                .id("draft-preview")
                                .flex_1()
                                .min_w_0()
                                .overflow_y_scroll()
                                .p_4()
                                .border_l_1()
                                .border_color(rgb(0xdcdedc))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(0x646971))
                                        .child("Local draft preview · not audience pagination"),
                                )
                                .when(!self.inspector, |d| {
                                    d.child(
                                        div()
                                            .mt_4()
                                            .p_4()
                                            .bg(rgb(0x202226))
                                            .text_color(rgb(0xfafaf9))
                                            .child(self.fields[4].read(cx).text().to_string())
                                            .child(div().mt_3().children(
                                                self.fields[5].read(cx).text().split('\n').map(
                                                    |line| {
                                                        div().min_h(px(20.)).child(
                                                            line.trim_end_matches('\r').to_owned(),
                                                        )
                                                    },
                                                ),
                                            )),
                                    )
                                })
                                .when(self.inspector, |d| {
                                    d.child(
                                        div().child("Inspector · song information").children(
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
                    }),
            )
            .when(!busy, |d| {
                d.child(
                    div()
                        .h(px(40.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .px_3()
                        .child(self.button(7, "+ Add section", cx))
                        .child(self.button(8, "Remove", cx))
                        .child(div().flex_1())
                        .child(self.button(15, "OK", cx))
                        .child(self.button(16, "Cancel", cx)),
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
    fn field(cx: &mut VisualTestContext, view: &Entity<Library>, index: usize, text: &str) {
        cx.update(|w, cx| {
            view.update(cx, |v, cx| {
                if (1..=3).contains(&index) {
                    v.inspector = true;
                    v.show_catalog = false;
                    cx.notify();
                }
                v.fields[index]
                    .update(cx, |f, cx| f.set_text(text, cx))
                    .unwrap();
                v.fields[index].read(cx).focus_handle(cx).focus(w, cx);
            })
        });
    }
    fn action(cx: &mut VisualTestContext, view: &Entity<Library>, index: usize) {
        cx.update(|w, cx| view.update(cx, |v, cx| v.action(index, w, cx)));
    }

    fn edit(cx: &mut VisualTestContext, view: &Entity<Library>, index: usize, text: &str) {
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
        let input = view.read_with(cx, |v, _| v.fields[index].clone());
        cx.update(|w, cx| {
            input.update(cx, |f, cx| {
                let end = f.text().encode_utf16().count();
                f.replace_text_in_range(Some(0..end), text, w, cx);
                f.focus_handle(cx).focus(w, cx);
            })
        });
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
        edit(&mut cx, &view, 5, "Asymmetric first line");
        action(&mut cx, &view, 7);
        edit(&mut cx, &view, 5, "Distinct second section");
        let count = view.read_with(&cx, |v, _| v.history.undo.len());
        action(&mut cx, &view, 17);
        action(&mut cx, &view, 12);
        action(&mut cx, &view, 13);
        cx.update(|_, cx| view.update(cx, |v, cx| v.select_section(0, cx)));
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
    fn blank_title_section_navigation_preserves_draft_but_cannot_save(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = fixture(cx, dir.path().join("library.sqlite"));
        edit(&mut cx, &view, 0, "   ");
        edit(&mut cx, &view, 5, "First section");
        action(&mut cx, &view, 7);
        edit(&mut cx, &view, 5, "Second section");
        let draft = view.read_with(&cx, |v, cx| v.current(cx));
        let count = view.read_with(&cx, |v, _| v.history.undo.len());
        cx.update(|_, cx| view.update(cx, |v, cx| v.select_section(0, cx)));
        assert_eq!(view.read_with(&cx, |v, _| v.section), 0);
        assert_eq!(
            view.read_with(&cx, |v, cx| v.fields[5].read(cx).text().to_owned()),
            "First section"
        );
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), draft);
        assert_eq!(view.read_with(&cx, |v, _| v.history.undo.len()), count);
        action(&mut cx, &view, 15);
        assert!(view.read_with(&cx, |v, _| v.pending.is_none()
            && !v.committed_close
            && v.version.is_none()
            && v.status.contains("title")));
    }

    #[gpui::test]
    fn chronological_document_history_across_navigation_and_structures(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let (mut cx, view) = fixture(cx, dir.path().join("library.sqlite"));
        edit(&mut cx, &view, 0, "Café 😀");
        edit(&mut cx, &view, 5, "First e\u{301}\r\n\n");
        let first = view.read_with(&cx, |v, cx| v.current(cx));
        action(&mut cx, &view, 7);
        edit(&mut cx, &view, 4, "Chorus B");
        edit(&mut cx, &view, 5, "Second asymmetric 😀\nlast");
        let two = view.read_with(&cx, |v, cx| v.current(cx));
        action(&mut cx, &view, 5);
        let count = view.read_with(&cx, |v, _| v.history.undo.len());
        cx.simulate_keystrokes("left shift-right");
        action(&mut cx, &view, 6);
        action(&mut cx, &view, 5);
        assert_eq!(view.read_with(&cx, |v, _| v.history.undo.len()), count);
        edit(&mut cx, &view, 5, "First changed");
        action(&mut cx, &view, 8);
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).sections[0].clone()),
            two.sections[1]
        );
        // Control-focus and field-focus both resolve the same semantic document action.
        cx.update(|w, cx| view.read(cx).buttons[8].clone().focus(w, cx));
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx).sections.len()), 2);
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), two);
        assert_eq!(view.read_with(&cx, |v, _| v.section), 0);
        cx.simulate_keystrokes("ctrl-shift-z ctrl-shift-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx).sections.len()), 1);
        cx.simulate_keystrokes("ctrl-z ctrl-z");
        action(&mut cx, &view, 6);
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).sections[1].lyrics.clone()),
            ""
        );
        cx.simulate_keystrokes("ctrl-z ctrl-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), first);
        edit(&mut cx, &view, 1, "Branch author");
        assert!(view.read_with(&cx, |v, _| v.history.redo.is_empty()));
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
        edit(&mut cx, &view, 5, "Saved lyrics");
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
        edit(&mut cx, &view, 5, "Conflict draft");
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
        assert!(view.read_with(&cx, |v, cx| v.current(cx) == blank()
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
        edit(&mut cx, &view, 5, "Retain me 😀");
        let original = view.read_with(&cx, |v, cx| v.current(cx));
        let count = view.read_with(&cx, |v, _| v.history.undo.len());
        action(&mut cx, &view, 1);
        wait(&mut cx, &view);
        assert_eq!(view.read_with(&cx, |v, _| v.history.undo.len()), count);
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), original);
        cx.simulate_keystrokes("ctrl-z ctrl-shift-z");
        assert_eq!(view.read_with(&cx, |v, cx| v.current(cx)), original);
        // Real Remove is invoked at control focus, not a soon-hidden lyric field.
        cx.update(|w, cx| view.read(cx).buttons[8].clone().focus(w, cx));
        action(&mut cx, &view, 8);
        assert!(view.read_with(&cx, |v, cx| v.current(cx).sections.is_empty()));
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
                label: n.to_string(),
                lyrics: format!("unique {n}"),
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
        field(&mut cx, &view, 0, "Original Café");
        field(&mut cx, &view, 1, "Original author");
        field(&mut cx, &view, 5, "First\r\nline e\u{301}\n");
        action(&mut cx, &view, 7);
        field(&mut cx, &view, 4, "Chorus");
        field(&mut cx, &view, 5, "Different second section");
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
                    label: "Verse 1".into(),
                    lyrics: "First\r\nline e\u{301}\n".into()
                },
                Section {
                    label: "Chorus".into(),
                    lyrics: "Different second section".into()
                },
            ]
        );
        field(&mut cx, &view, 0, "Unsaved");
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
}
