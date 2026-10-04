//! Provisional native authoring window. All durable I/O belongs to storage::Worker.
use crate::{ActivateControl, FocusNext, FocusPrevious, Quit, text_input::TextInput};
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
                title: Some("Sela — Song library".into()),
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

struct Library {
    focus: FocusHandle,
    fields: [Entity<TextInput>; 6],
    field_edits: [u64; 6],
    buttons: [FocusHandle; 12],
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
    section: usize,
    status: String,
    confirm_close: bool,
    confirm_delete: bool,
}

impl Library {
    fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let fields = std::array::from_fn(|i| {
            cx.new(|cx| {
                TextInput::new(
                    "",
                    i == 5,
                    [1024, 4096, 4096, 4096, 256, 256 * 1024][i],
                    20 + i as isize,
                    cx,
                )
                .expect("empty text is valid")
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
            section: 0,
            status,
            confirm_close: false,
            confirm_delete: false,
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
                self.begin(song, Some(v), cx);
                self.cursor = None;
                self.refresh(cx);
            }
            (Some(Pending::Delete), Ok(Reply::Deleted)) => {
                self.begin(blank(), None, cx);
                self.cursor = None;
                self.refresh(cx);
            }
            (_, Err(e)) => {
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
        match index {
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
                    self.draft = song;
                    match index {
                        5 => self.section = self.section.saturating_sub(1),
                        6 => {
                            self.section =
                                (self.section + 1).min(self.draft.sections.len().saturating_sub(1))
                        }
                        7 if self.draft.sections.len() < 128 => {
                            self.draft.sections.push(Section {
                                label: format!("Section {}", self.draft.sections.len() + 1),
                                lyrics: String::new(),
                            });
                            self.section = self.draft.sections.len() - 1;
                        }
                        8 if !self.draft.sections.is_empty() => {
                            self.draft.sections.remove(self.section);
                            self.section = self
                                .section
                                .min(self.draft.sections.len().saturating_sub(1));
                        }
                        _ => (),
                    }
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
    fn button(&self, index: usize, label: &str, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(("library-button", index))
            .track_focus(&self.buttons[index])
            .key_context("SelaControl")
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf4f4f3))
            .text_color(rgb(0x292c30))
            .font_family("DejaVu Sans")
            .text_size(px(13.))
            .child(
                div()
                    .h(px(44.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(0xdcdedc))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Song library"),
                    )
                    .child(if dirty {
                        "Unsaved changes"
                    } else {
                        "No unsaved changes"
                    })
                    .when(!busy, |d| {
                        d.child(self.button(0, "New", cx))
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
                    }),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_size(px(12.))
                    .child(self.status.clone()),
            )
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
                    .child(
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
                                                .on_click(cx.listener(move |s, event, w, cx| {
                                                    if matches!(event, ClickEvent::Keyboard(_)) {
                                                        return;
                                                    }
                                                    s.row_focus[i].focus(w, cx);
                                                    s.select(version, cx);
                                                }))
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
                    .child(if busy {
                        div()
                            .flex_1()
                            .p_6()
                            .child("Loading or saving…")
                            .into_any_element()
                    } else {
                        div()
                            .id("song-form")
                            .flex_1()
                            .min_w_0()
                            .p_4()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .children(
                                ["Title", "Authors", "Copyright", "License identifier"]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, label)| {
                                        div()
                                            .flex_shrink_0()
                                            .child(div().mb_1().text_size(px(12.)).child(label))
                                            .child(self.fields[i].clone())
                                    }),
                            )
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
                                    .child(self.button(6, "Next", cx))
                                    .child(self.button(7, "Add section", cx))
                                    .child(self.button(8, "Remove", cx)),
                            )
                            .when(!self.draft.sections.is_empty(), |d| {
                                d.child(
                                    div()
                                        .flex_shrink_0()
                                        .child("Section label")
                                        .child(self.fields[4].clone()),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .child("Lyrics · line breaks are preserved")
                                        .child(self.fields[5].clone()),
                                )
                            })
                            .into_any_element()
                    }),
            )
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
        latest.authors = "metadata\nnot supported by this editor".into();
        let incompatible = other.save_song(None, latest).unwrap();
        cx.update(|_, cx| view.update(cx, |v, cx| v.select(incompatible, cx)));
        wait(&mut cx, &view);
        assert_eq!(
            view.read_with(&cx, |v, cx| v.current(cx).title),
            "Shared original"
        );
        assert!(view.read_with(&cx, |v, _| v.status.contains("Original unchanged")));
    }
}
