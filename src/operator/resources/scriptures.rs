//! Scriptures tab (M4-03/M4-04): translation picker in the left column, a
//! reference/keyword box with book completion, the verse list with single,
//! consecutive (Shift) and nonconsecutive (Ctrl) selection, Preview of the
//! selected verses as paginated slides, Go Live through the ordered cue
//! path, and Import of user OSIS/USFX/Zefania files.
//!
//! Every Bible read runs on the scripture worker (`sela::scripture::worker`);
//! this module only submits commands, applies replies whose generation is
//! still current, and keeps the previous list, selection and Preview when a
//! reference is malformed or missing from the translation (VAL-BIBLE-009).
//! Layout follows EasyWorship's Resources tab placement (EW8-OBS-010 row of
//! tabs); the tab's inner layout is an UNOBSERVED best guess recorded in
//! docs/reference-observations.md.
use super::*;
use gpui::{ListAlignment, ListState, WeakEntity, list};
use sela::scripture::{
    self, Book, ParseError, Reference, Snapshot, VerseId,
    bundled::{self, Bundle},
    passage::{self, Located, Missing},
    worker::{self, CatalogNote, Command, Damage, Entry, ImportError, Reply, State},
};
use std::collections::BTreeSet;

actions!(
    operator,
    [
        AcceptScriptureCompletion,
        ClearScriptureSearch,
        NextVerse,
        PreviousVerse,
        ExtendNextVerse,
        ExtendPreviousVerse,
        FirstVerse,
        LastVerse,
        SelectAllVerses
    ]
);

/// Focus band of the tab, after the image rows' band.
const TRANSLATION_TAB_INDEX: isize = 2500;
const MODE_TAB_INDEX: isize = 2540;
pub(crate) const REFERENCE_TAB_INDEX: isize = 2542;
const CLEAR_TAB_INDEX: isize = 2543;
const SUGGESTION_TAB_INDEX: isize = 2544;
const LIST_TAB_INDEX: isize = 2550;
const IMPORT_TAB_INDEX: isize = 2560;
const ADD_TAB_INDEX: isize = 2561;
/// Book suggestions shown under the box.
const SUGGESTIONS: usize = 6;
/// Search starts at two characters (owner decision, docs/plan.md).
const MIN_QUERY_CHARS: usize = 2;
/// Width of the verse number column.
const NUMBER_W: f32 = 52.;
const HIT_NUMBER_W: f32 = 120.;

pub(crate) fn bind_scripture_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(
            "tab",
            AcceptScriptureCompletion,
            Some("SelaScriptureReference"),
        ),
        KeyBinding::new(
            "escape",
            ClearScriptureSearch,
            Some("SelaScriptureReference"),
        ),
        KeyBinding::new("down", NextVerse, Some("SelaScriptureList")),
        KeyBinding::new("up", PreviousVerse, Some("SelaScriptureList")),
        KeyBinding::new("shift-down", ExtendNextVerse, Some("SelaScriptureList")),
        KeyBinding::new("shift-up", ExtendPreviousVerse, Some("SelaScriptureList")),
        KeyBinding::new("home", FirstVerse, Some("SelaScriptureList")),
        KeyBinding::new("end", LastVerse, Some("SelaScriptureList")),
        KeyBinding::new("ctrl-a", SelectAllVerses, Some("SelaScriptureList")),
    ]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Reference,
    Keyword,
}

/// What the selection becomes when a passage lands.
#[derive(Clone, Debug)]
enum Select {
    /// The referenced verses (a typed reference).
    Range(VerseId, VerseId),
    /// The same verses as before (a translation switch).
    Keep(Vec<VerseId>),
}

#[derive(Clone, Debug)]
struct PendingPassage {
    generation: u64,
    translation: String,
    reference: Reference,
    select: Select,
    /// Whether the landed passage replaces Preview (an operator lookup)
    /// or only fills the list (text applied by a catalog refresh).
    preview: bool,
}

/// A line under the box: an error in red, or a hint in muted text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Note {
    pub(crate) text: String,
    pub(crate) error: bool,
}

pub(crate) struct ScriptureTab {
    /// Translations installed on first use; tests swap in synthetic ones.
    pub(crate) bundles: Vec<Bundle>,
    worker: Option<worker::Worker>,
    started: bool,
    /// Why the tab cannot work at all (no profile, worker failed).
    pub(crate) failure: Option<String>,
    pub(crate) translations: Vec<Entry>,
    pub(crate) catalog_pending: bool,
    /// The first catalog reply has been applied: later ones (after an
    /// import) refresh the list of translations only.
    catalog_loaded: bool,
    /// Id of the translation the list shows.
    pub(crate) current: Option<String>,
    pub(crate) mode: Mode,
    pub(crate) input: Entity<TextInput>,
    pub(crate) note: Option<Note>,
    /// Footer status: text and whether it reports a failure.
    pub(crate) message: Option<(String, bool)>,
    /// The reference the list shows, once it has loaded.
    pub(crate) reference: Option<Reference>,
    /// Rows of the verse list: a passage's whole chapters or search hits.
    pub(crate) rows: Arc<Vec<(VerseId, String)>>,
    pub(crate) hits: bool,
    pub(crate) more: bool,
    pub(crate) selection: BTreeSet<VerseId>,
    anchor: Option<usize>,
    pub(crate) cursor: Option<usize>,
    pub(crate) suggestions: Vec<Book>,
    generations: sela::search::Generations,
    pending: Option<PendingPassage>,
    /// A newer request waiting for the one in flight.
    queued: Option<Command>,
    in_flight: bool,
    pub(crate) importing: Option<String>,
    /// The Import Bible… file dialog is open; a second click asks nothing.
    pub(crate) import_prompt: bool,
    /// Scripture items of a schedule file being opened, waiting for its
    /// songs to be imported into the library.
    pub(crate) opening: Option<Vec<sela::schedule_file::ScriptureItem>>,
    /// A scripture schedule entry went to Preview without a `cx`; its faces
    /// resolve on the next poll.
    resolve_pending: bool,
    pub(crate) list_state: ListState,
    pub(crate) list_focus: FocusHandle,
    mode_focus: [FocusHandle; 2],
    clear_focus: FocusHandle,
    suggestion_focus: Vec<FocusHandle>,
    translation_focus: Vec<FocusHandle>,
    import_focus: FocusHandle,
    pub(crate) add_focus: FocusHandle,
    _observer: Subscription,
}

impl ScriptureTab {
    pub(crate) fn new(cx: &mut Context<Operator>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(
                "",
                false,
                sela::search::MAX_QUERY_BYTES,
                REFERENCE_TAB_INDEX,
                cx,
            )
            .expect("empty text is valid")
        });
        let observer = cx.observe(&input, |this, _, cx| this.scripture_text_changed(cx));
        let focus =
            |cx: &mut Context<Operator>, index| cx.focus_handle().tab_index(index).tab_stop(true);
        Self {
            bundles: bundled::shipped(),
            worker: None,
            started: false,
            failure: None,
            translations: Vec::new(),
            catalog_pending: false,
            catalog_loaded: false,
            current: None,
            mode: Mode::Reference,
            input,
            note: None,
            message: None,
            reference: None,
            rows: Arc::default(),
            hits: false,
            more: false,
            selection: BTreeSet::new(),
            anchor: None,
            cursor: None,
            suggestions: Vec::new(),
            generations: Default::default(),
            pending: None,
            queued: None,
            in_flight: false,
            importing: None,
            import_prompt: false,
            opening: None,
            resolve_pending: false,
            list_state: ListState::new(0, ListAlignment::Top, px(200.)),
            list_focus: focus(cx, LIST_TAB_INDEX),
            mode_focus: [focus(cx, MODE_TAB_INDEX), focus(cx, MODE_TAB_INDEX + 1)],
            clear_focus: focus(cx, CLEAR_TAB_INDEX),
            suggestion_focus: (0..SUGGESTIONS as isize)
                .map(|i| focus(cx, SUGGESTION_TAB_INDEX + i))
                .collect(),
            translation_focus: Vec::new(),
            import_focus: focus(cx, IMPORT_TAB_INDEX),
            add_focus: focus(cx, ADD_TAB_INDEX),
            _observer: observer,
        }
    }

    /// The entry of the shown translation.
    pub(crate) fn current_entry(&self) -> Option<&Entry> {
        let id = self.current.as_deref()?;
        self.translations.iter().find(|e| e.info.id == id)
    }

    fn inventory_of(&self, id: &str) -> Option<Arc<sela::scripture::Inventory>> {
        self.translations
            .iter()
            .find(|e| e.info.id == id)
            .and_then(|e| match &e.state {
                State::Ready { inventory, .. } => Some(inventory.clone()),
                State::Damaged(_) => None,
            })
    }

    /// The selected rows in canonical order.
    pub(crate) fn selected_verses(&self) -> Vec<(VerseId, String)> {
        self.rows
            .iter()
            .filter(|(id, _)| self.selection.contains(id))
            .cloned()
            .collect()
    }

    /// Drops the shown rows, selection and any passage or search still on
    /// its way, so no reply for the previous translation lands.
    fn forget_shown(&mut self) {
        self.generations.advance();
        self.pending = None;
        self.queued = None;
        self.reference = None;
        let hits = self.mode == Mode::Keyword;
        self.set_rows(Vec::new(), hits);
        self.selection.clear();
    }

    fn set_rows(&mut self, rows: Vec<(VerseId, String)>, hits: bool) {
        self.rows = Arc::new(rows);
        self.hits = hits;
        self.list_state.reset(self.rows.len());
        self.anchor = None;
        self.cursor = None;
    }

    fn submit(&mut self, command: Command) {
        let Some(worker) = &self.worker else {
            return;
        };
        if self.in_flight {
            self.queued = Some(command);
            return;
        }
        match worker.submit(command) {
            Ok(()) => self.in_flight = true,
            Err(command) => {
                self.queued = Some(command);
                self.message = Some((t!(Msg::ScriptureBusy).into(), true));
            }
        }
    }
}

/// The label of a translation row, shown and spoken: its id and name,
/// then "(damaged)" for an unusable file or "(restricted)" for text whose
/// rights do not allow redistribution (an import), in the UI language.
pub(crate) fn translation_label(entry: &Entry) -> String {
    let id = &entry.info.id;
    let name = if entry.info.name.is_empty() || entry.info.name == *id {
        id.clone()
    } else {
        format!("{id} · {}", entry.info.name)
    };
    let tag = if matches!(entry.state, State::Damaged(_)) {
        Some(t!(Msg::ScriptureDamagedTag))
    } else if !entry.info.redistributable {
        Some(t!(Msg::ScriptureRestricted))
    } else {
        None
    };
    match tag {
        Some(tag) => format!("{name} ({tag})"),
        None => name,
    }
}

/// The EN/ID wording of a reference the parser refused.
pub(crate) fn parse_note(error: &ParseError) -> Option<Note> {
    let language = i18n::language();
    let (text, error) = match error {
        ParseError::Empty => return None,
        ParseError::UnknownBook(name) => (t!(Msg::ScriptureErrUnknownBook, name = name), true),
        ParseError::MissingChapter(book) => (
            t!(Msg::ScriptureErrMissingChapter, book = book.name(language)),
            false,
        ),
        ParseError::ChapterNotFound { book, chapter } => (
            t!(
                Msg::ScriptureErrChapterNotFound,
                book = book.name(language),
                chapter = chapter
            ),
            true,
        ),
        ParseError::InvalidRange => (t!(Msg::ScriptureErrInvalidRange).into(), true),
        ParseError::Malformed => (t!(Msg::ScriptureErrMalformed).into(), true),
    };
    Some(Note { text, error })
}

/// The EN/ID wording of a reference the translation does not have.
pub(crate) fn missing_note(missing: Missing, translation: &str) -> Note {
    let language = i18n::language();
    let text = match missing {
        Missing::Book(book) => t!(
            Msg::ScriptureErrBookMissing,
            book = book.name(language),
            translation = translation
        ),
        Missing::Chapter(book, chapter) => t!(
            Msg::ScriptureErrChapterMissing,
            book = book.name(language),
            chapter = chapter,
            translation = translation
        ),
        Missing::Verse {
            book,
            chapter,
            count,
        } => t!(
            Msg::ScriptureErrVerseMissing,
            book = book.name(language),
            chapter = chapter,
            count = count,
            translation = translation
        ),
    };
    Note { text, error: true }
}

/// The EN/ID wording of a translation file's problem; only the detail in
/// parentheses (a check's name, the system's reason) stays untranslated.
pub(crate) fn store_error(error: &sela::scripture::store::Error) -> String {
    use sela::scripture::store::Error;
    match error {
        Error::NotFound => t!(Msg::ScriptureWhyMissing).into(),
        Error::Io(detail) => t!(Msg::ScriptureWhyIo, detail = detail),
        Error::Unsupported => t!(Msg::ScriptureWhyUnsupported).into(),
        Error::Corrupt(detail) => t!(Msg::ScriptureWhyCorrupt, detail = detail),
        Error::Invalid(detail) => t!(Msg::ScriptureWhyInvalid, detail = detail),
    }
}

pub(crate) fn damage(why: &Damage) -> String {
    use sela::scripture::bundled::Error;
    match why {
        Damage::Store(error) => store_error(error),
        Damage::Bundle(Error::Checksum) => t!(Msg::ScriptureWhyChecksum).into(),
        Damage::Bundle(Error::Inflate) => t!(Msg::ScriptureWhyInflate).into(),
        Damage::Bundle(Error::Format(detail)) => t!(Msg::ScriptureWhyBundle, detail = detail),
        Damage::FileName => t!(Msg::ScriptureWhyFileName).into(),
    }
}

/// A failed passage or search, phrased for the note under the box.
fn read_error(error: &worker::Error, translation: &str) -> String {
    match error {
        worker::Error::Unknown(id) => t!(Msg::ScriptureNotInstalled, translation = id),
        worker::Error::Unavailable(why) => t!(
            Msg::ScriptureDamaged,
            translation = translation,
            error = damage(why)
        ),
        worker::Error::TooLong => t!(
            Msg::ScriptureErrTooLong,
            max = sela::scripture::store::MAX_PASSAGE
        ),
        worker::Error::Store(error) => t!(
            Msg::ScriptureDamaged,
            translation = translation,
            error = store_error(error)
        ),
    }
}

fn import_error(error: &ImportError) -> String {
    use sela::scripture::import::Rejected;
    match error {
        ImportError::Unreadable(detail) => t!(Msg::ScriptureWhyUnreadable, detail = detail),
        ImportError::TooMany => t!(Msg::ScriptureRejectTooMany).into(),
        ImportError::Store(error) => store_error(error),
        ImportError::Rejected(rejected) => match rejected {
            Rejected::TooLarge => t!(Msg::ScriptureRejectTooLarge).into(),
            Rejected::InvalidUtf8 => t!(Msg::ScriptureRejectEncoding).into(),
            Rejected::Malformed(detail) => t!(Msg::ScriptureRejectMalformed, detail = detail),
            Rejected::Entities => t!(Msg::ScriptureRejectEntities).into(),
            Rejected::TooDeep => t!(Msg::ScriptureRejectDeep).into(),
            Rejected::WrongFormat => t!(Msg::ScriptureRejectFormat).into(),
            Rejected::NoVerses => t!(Msg::ScriptureRejectNoVerses).into(),
        },
    }
}

impl Operator {
    /// Preview of a scripture schedule entry: its pinned verses, labelled
    /// with the installed translation's chapter ends when it is installed.
    pub(crate) fn preview_snapshot(&mut self, entry: EntryId, snapshot: &Snapshot) {
        let inventory = self
            .scripture
            .inventory_of(&snapshot.translation)
            .map(|inventory| (*inventory).clone())
            .unwrap_or_else(|| snapshot.inventory());
        let mut item = passage_item(&snapshot.info(), &inventory, &snapshot.verses);
        item.entry = Some(entry);
        if self
            .preview
            .as_ref()
            .is_some_and(|p| (p.version, p.entry) == (item.version, item.entry))
        {
            return;
        }
        self.preview = Some(item);
        self.preview_slide = 0;
        // Faces resolve off the UI thread on the next poll.
        self.scripture.resolve_pending = true;
    }

    /// Add to Schedule from the Scriptures tab: a snapshot of the selected
    /// verses, appended like a song (best guess, EW unobserved).
    pub(crate) fn add_scripture_to_schedule(&mut self) {
        let tab = &self.scripture;
        let (Some(entry), verses) = (tab.current_entry(), tab.selected_verses()) else {
            return;
        };
        if verses.is_empty() {
            self.scripture.message = Some((t!(Msg::ScriptureSelectVerses).into(), true));
            return;
        }
        let snapshot = Snapshot::new(&entry.info, &verses);
        let Some(title) = self.scripture_item().map(|item| item.title) else {
            return;
        };
        match self
            .schedule
            .insert_scripture(usize::MAX, snapshot, title.clone())
        {
            Ok(_) => {
                self.scripture.message =
                    Some((t!(Msg::ScriptureAddedToSchedule, reference = title), false));
                self.schedule_message = None;
            }
            Err(sela::schedule::Error::Full) => {
                self.schedule_message = Some(t!(Msg::ScheduleFull, max = MAX_ITEMS));
            }
            Err(_) => {
                self.scripture.message = Some((t!(Msg::ScriptureSelectVerses).into(), true));
            }
        }
    }

    /// Starts the worker and asks for the catalog the first time the tab is
    /// shown; the bundled translations are installed then, off the UI thread.
    pub(crate) fn ensure_scripture_started(&mut self) {
        if self.scripture.started {
            return;
        }
        self.scripture.started = true;
        let Some(profile) = self.profile.clone() else {
            self.scripture.failure = Some(t!(Msg::ScriptureNoProfile).into());
            return;
        };
        match worker::Worker::start(profile.join("Scripture"), self.scripture.bundles.clone()) {
            Ok(worker) => {
                self.scripture.worker = Some(worker);
                self.request_scripture_catalog();
            }
            Err(error) => {
                self.scripture.failure =
                    Some(t!(Msg::ScriptureUnavailable, error = error.to_string()));
            }
        }
    }

    fn request_scripture_catalog(&mut self) {
        let Some(worker) = &self.scripture.worker else {
            return;
        };
        if worker.submit(Command::Catalog).is_ok() {
            self.scripture.catalog_pending = true;
        } else {
            self.scripture.message = Some((t!(Msg::ScriptureBusy).into(), true));
        }
    }

    pub(crate) fn poll_scripture(&mut self, cx: &mut Context<Self>) -> bool {
        let mut changed = false;
        if std::mem::take(&mut self.scripture.resolve_pending) {
            self.resolve_fonts(cx);
            changed = true;
        }
        for _ in 0..worker::QUEUE * 2 {
            let Some(reply) = self
                .scripture
                .worker
                .as_ref()
                .and_then(worker::Worker::poll)
            else {
                break;
            };
            changed = true;
            self.apply_scripture_reply(reply, cx);
        }
        if self
            .scripture
            .worker
            .as_ref()
            .is_some_and(worker::Worker::stopped)
        {
            self.scripture.worker = None;
            self.scripture.failure = Some(t!(
                Msg::ScriptureUnavailable,
                error = t!(Msg::ScriptureWhyStopped)
            ));
            changed = true;
        }
        changed
    }

    fn apply_scripture_reply(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Catalog { entries, notes } => {
                self.scripture.catalog_pending = false;
                self.scripture.translations = entries;
                self.scripture.translation_focus = (0..self.scripture.translations.len())
                    .map(|i| {
                        cx.focus_handle()
                            .tab_index(TRANSLATION_TAB_INDEX + i as isize)
                            .tab_stop(true)
                    })
                    .collect();
                let ready = |e: &&Entry| matches!(e.state, State::Ready { .. });
                let before = self.scripture.current.clone();
                let keep = self
                    .scripture
                    .current_entry()
                    .filter(ready)
                    .map(|e| e.info.id.clone());
                self.scripture.current = keep.or_else(|| {
                    self.scripture
                        .translations
                        .iter()
                        .find(ready)
                        .map(|e| e.info.id.clone())
                });
                if let Some(note) = notes.first() {
                    self.scripture.message = Some(match note {
                        CatalogNote::Rebuilt(id) => {
                            (t!(Msg::ScriptureRebuilt, translation = id), false)
                        }
                        CatalogNote::Failed(id, why) => (
                            t!(Msg::ScriptureDamaged, translation = id, error = damage(why)),
                            true,
                        ),
                        CatalogNote::Folder(why) => (
                            t!(
                                Msg::ScriptureUnavailable,
                                error = t!(Msg::ScriptureWhyFolder, detail = why)
                            ),
                            true,
                        ),
                    });
                } else if self.scripture.translations.is_empty() {
                    self.scripture.message = Some((t!(Msg::ScriptureNoTranslations).into(), true));
                }
                if !std::mem::replace(&mut self.scripture.catalog_loaded, true) {
                    // Text typed while the first catalog loaded applies now.
                    self.scripture_text_changed(cx);
                } else if self.scripture.current != before {
                    // The shown translation became unusable: its rows go,
                    // Preview and Live stay as they are.
                    self.scripture.forget_shown();
                    if before.is_none() {
                        // There was no usable translation, so nothing was
                        // shown: the box's text is looked up in the one an
                        // import made usable. Only the list fills; Preview
                        // still changes only on an operator action.
                        self.scripture_text_applied(cx);
                    }
                }
                // Any other refresh (after an import, whether it succeeded
                // or not) leaves the list, the selection and Preview alone:
                // only an operator action changes Preview.
            }
            Reply::Passage {
                generation,
                translation,
                result,
            } => {
                self.scripture.in_flight = false;
                let pending = self
                    .scripture
                    .pending
                    .take_if(|p| p.generation == generation);
                if self.scripture.generations.is_current(generation)
                    && let Some(pending) = pending
                {
                    match result {
                        Ok(verses) => self.passage_landed(pending, verses, cx),
                        Err(error) => {
                            self.scripture.note = Some(Note {
                                text: read_error(&error, &translation),
                                error: true,
                            });
                        }
                    }
                }
                self.pump_scripture();
            }
            Reply::Search {
                generation,
                translation,
                result,
            } => {
                self.scripture.in_flight = false;
                if self.scripture.generations.is_current(generation)
                    && self.scripture.mode == Mode::Keyword
                {
                    match result {
                        Ok((hits, more)) => {
                            self.scripture.set_rows(hits, true);
                            self.scripture.more = more;
                            self.scripture.selection.clear();
                            self.scripture.note = None;
                        }
                        Err(error) => {
                            self.scripture.note = Some(Note {
                                text: read_error(&error, &translation),
                                error: true,
                            });
                        }
                    }
                }
                self.pump_scripture();
            }
            Reply::Imported(result) => {
                self.scripture.importing = None;
                self.scripture.message = Some(match result {
                    Ok(imported) => (
                        t!(
                            Msg::ScriptureImported,
                            translation = imported.info.id,
                            count = imported.verses,
                            notes = imported.warnings.len()
                        ),
                        false,
                    ),
                    Err(error) => (
                        t!(Msg::ScriptureImportFailed, error = import_error(&error)),
                        true,
                    ),
                });
                self.request_scripture_catalog();
            }
        }
    }

    fn pump_scripture(&mut self) {
        if let Some(command) = self.scripture.queued.take() {
            self.scripture.submit(command);
        }
    }

    fn passage_landed(
        &mut self,
        pending: PendingPassage,
        verses: Vec<(VerseId, String)>,
        cx: &mut Context<Self>,
    ) {
        let tab = &mut self.scripture;
        let present: BTreeSet<VerseId> = verses.iter().map(|(id, _)| *id).collect();
        let selection: BTreeSet<VerseId> = match &pending.select {
            Select::Range(from, to) => present.range(*from..=*to).copied().collect(),
            Select::Keep(ids) => ids
                .iter()
                .filter(|id| present.contains(id))
                .copied()
                .collect(),
        };
        if selection.is_empty() {
            // Every referenced verse is a gap in this translation's
            // numbering (or none was selected): keep what is shown.
            let language = tab
                .translations
                .iter()
                .find(|e| e.info.id == pending.translation)
                .map(|e| e.info.book_language())
                .unwrap_or_default();
            let verses = match &pending.select {
                Select::Range(from, to) => {
                    let start = passage::label(&[*from], &Default::default(), language);
                    match (from == to, from.chapter == to.chapter) {
                        (true, _) => start,
                        (false, true) => format!("{start}-{}", to.verse),
                        (false, false) => format!("{start}-{}:{}", to.chapter, to.verse),
                    }
                }
                Select::Keep(ids) => passage::label(ids, &Default::default(), language),
            };
            tab.note = Some(Note {
                text: t!(
                    Msg::ScriptureMissingVerses,
                    translation = pending.translation,
                    verses = verses
                ),
                error: true,
            });
            return;
        }
        let previous = tab.current.clone();
        let old_inventory = previous.as_deref().and_then(|id| tab.inventory_of(id));
        tab.current = Some(pending.translation.clone());
        tab.reference = Some(pending.reference);
        tab.note = None;
        tab.more = false;
        tab.set_rows(verses, false);
        tab.selection = selection;
        if let Select::Keep(ids) = &pending.select {
            let missing: Vec<VerseId> = ids
                .iter()
                .filter(|id| !present.contains(id))
                .copied()
                .collect();
            if !missing.is_empty() {
                let language = tab
                    .current_entry()
                    .map(|e| e.info.book_language())
                    .unwrap_or_default();
                let inventory = old_inventory.unwrap_or_default();
                tab.note = Some(Note {
                    text: t!(
                        Msg::ScriptureMissingVerses,
                        translation = pending.translation,
                        verses = passage::label(&missing, &inventory, language)
                    ),
                    error: false,
                });
            }
        }
        let first = tab
            .selection
            .first()
            .and_then(|first| tab.rows.iter().position(|(id, _)| id == first));
        tab.cursor = first;
        tab.anchor = first;
        if let Some(index) = first {
            // Rows are not measured yet after a reset, so place the first
            // selected verse near the top with one verse of context above.
            tab.list_state.scroll_to(gpui::ListOffset {
                item_ix: index.saturating_sub(1),
                offset_in_item: px(0.),
            });
        }
        if pending.preview {
            self.preview_scripture(cx);
        }
    }

    /// Typed text changed (or the mode, translation or catalog did).
    pub(crate) fn scripture_text_changed(&mut self, cx: &mut Context<Self>) {
        let text = self.scripture.input.read(cx).text().to_string();
        match self.scripture.mode {
            Mode::Reference => self.reference_typed(&text, true),
            Mode::Keyword => self.keyword_typed(&text),
        }
        cx.notify();
    }

    /// Looks the box's text up again without touching Preview (a keyword
    /// search never does).
    fn scripture_text_applied(&mut self, cx: &mut Context<Self>) {
        let text = self.scripture.input.read(cx).text().to_string();
        match self.scripture.mode {
            Mode::Reference => self.reference_typed(&text, false),
            Mode::Keyword => self.keyword_typed(&text),
        }
        cx.notify();
    }

    fn reference_typed(&mut self, text: &str, preview: bool) {
        let tab = &mut self.scripture;
        // Newer text supersedes any passage still loading for older text.
        tab.generations.advance();
        tab.pending = None;
        tab.queued = None;
        let trimmed = text.trim();
        tab.suggestions = if !trimmed.is_empty()
            && trimmed.chars().any(char::is_alphabetic)
            && !trimmed.chars().any(|c| c.is_ascii_digit())
        {
            scripture::complete(trimmed, SUGGESTIONS)
        } else {
            Vec::new()
        };
        let reference = match scripture::parse(text) {
            Ok(reference) => reference,
            // The previous list, selection and Preview stay.
            Err(error) => {
                tab.note = parse_note(&error);
                return;
            }
        };
        let Some(id) = tab.current.clone() else {
            return;
        };
        self.load_reference(reference, id, None, preview);
    }

    /// Locates `reference` in translation `id` and asks for its chapters;
    /// `keep` re-selects those verses instead of the referenced ones.
    fn load_reference(
        &mut self,
        reference: Reference,
        id: String,
        keep: Option<Vec<VerseId>>,
        preview: bool,
    ) -> bool {
        let tab = &mut self.scripture;
        let Some(inventory) = tab.inventory_of(&id) else {
            return false;
        };
        // A switch keeps the selected verses, so only the chapters must
        // exist there; a typed reference must exist verse for verse.
        let found = match keep {
            Some(ids) => passage::chapters(&reference, &inventory)
                .map(|chapters| (chapters, Select::Keep(ids))),
            None => passage::locate(&reference, &inventory).map(|located: Located| {
                (
                    located.chapters,
                    Select::Range(located.selected.0, located.selected.1),
                )
            }),
        };
        let ((from, to), select) = match found {
            Ok(found) => found,
            Err(missing) => {
                tab.note = Some(missing_note(missing, &id));
                return false;
            }
        };
        tab.note = None;
        let generation = tab.generations.advance();
        tab.pending = Some(PendingPassage {
            generation,
            translation: id.clone(),
            reference,
            select,
            preview,
        });
        tab.submit(Command::Passage {
            generation,
            translation: id,
            from,
            to,
        });
        true
    }

    fn keyword_typed(&mut self, text: &str) {
        let tab = &mut self.scripture;
        tab.suggestions.clear();
        let query = text.trim();
        // Every keystroke supersedes an older answer.
        let generation = tab.generations.advance();
        if query.chars().count() < MIN_QUERY_CHARS {
            tab.set_rows(Vec::new(), true);
            tab.more = false;
            tab.selection.clear();
            tab.note = (!query.is_empty()).then(|| Note {
                text: t!(Msg::ScriptureSearchShort).into(),
                error: false,
            });
            return;
        }
        tab.note = None;
        let Some(id) = tab.current.clone() else {
            return;
        };
        tab.submit(Command::Search {
            generation,
            translation: id,
            query: query.to_owned(),
        });
    }

    pub(crate) fn set_scripture_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.scripture.mode == mode {
            return;
        }
        self.scripture.mode = mode;
        self.scripture.note = None;
        // The shown reference belongs to the list it loaded; after a mode
        // switch the list shows hits (or nothing), so a translation switch
        // must not reload it with a hit selected (EW8 UNOBSERVED).
        self.scripture.reference = None;
        self.scripture.suggestions.clear();
        self.scripture.input.update(cx, |input, cx| {
            let _ = input.set_text("", cx);
        });
        if mode == Mode::Keyword {
            self.scripture.set_rows(Vec::new(), true);
            self.scripture.selection.clear();
        }
        cx.notify();
    }

    /// Picks another translation. A shown reference is looked up in it
    /// first; if the translation lacks it, nothing changes but the note
    /// (mismatched canon or versification).
    pub(crate) fn choose_translation(&mut self, id: String, cx: &mut Context<Self>) {
        if self.scripture.current.as_deref() == Some(id.as_str()) {
            return;
        }
        let Some(entry) = self.scripture.translations.iter().find(|e| e.info.id == id) else {
            return;
        };
        if let State::Damaged(why) = &entry.state {
            self.scripture.note = Some(Note {
                text: t!(Msg::ScriptureDamaged, translation = id, error = damage(why)),
                error: true,
            });
            cx.notify();
            return;
        }
        // A typed reference still loading is retyped in the new translation
        // rather than replaced by the shown one.
        let typed_pending = self
            .scripture
            .pending
            .as_ref()
            .is_some_and(|p| matches!(p.select, Select::Range(..)));
        match (self.scripture.mode, self.scripture.reference) {
            (Mode::Reference, Some(reference)) if !typed_pending => {
                let keep: Vec<VerseId> = self.scripture.selection.iter().copied().collect();
                // With nothing selected the referenced verses are selected
                // there, as if typed again.
                let keep = (!keep.is_empty()).then_some(keep);
                self.load_reference(reference, id, keep, true);
            }
            _ => {
                // Nothing for the old translation may land after this: the
                // rows and selection go, and the box's text (reference or
                // keyword) is looked up in the new translation.
                self.scripture.forget_shown();
                self.scripture.current = Some(id);
                self.scripture_text_changed(cx);
            }
        }
        cx.notify();
    }

    /// Replaces the book part of the typed text with a suggestion.
    pub(crate) fn accept_suggestion(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(book) = self.scripture.suggestions.get(index).copied() else {
            return false;
        };
        let language = self
            .scripture
            .current_entry()
            .map(|e| e.info.book_language())
            .unwrap_or_else(i18n::language);
        let text = format!("{} ", book.name(language));
        self.scripture.input.update(cx, |input, cx| {
            let _ = input.set_text(&text, cx);
            input.set_cursor(text.len(), cx);
        });
        let focus = self.scripture.input.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        self.scripture_text_changed(cx);
        true
    }

    pub(crate) fn clear_scripture_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.scripture.input.update(cx, |input, cx| {
            let _ = input.set_text("", cx);
        });
        self.scripture.note = None;
        let focus = self.scripture.input.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        self.scripture_text_changed(cx);
    }

    /// A click on row `index`: plain selects it alone, Shift extends from
    /// the anchor (consecutive), Ctrl/Cmd toggles it (nonconsecutive). A
    /// double-click sends the selection live.
    pub(crate) fn click_verse(
        &mut self,
        index: usize,
        modifiers: gpui::Modifiers,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        let tab = &mut self.scripture;
        let Some((id, _)) = tab.rows.get(index).cloned() else {
            return;
        };
        if count >= 2 {
            if !tab.selection.contains(&id) {
                tab.selection = BTreeSet::from([id]);
                tab.anchor = Some(index);
            }
            tab.cursor = Some(index);
            self.preview_scripture(cx);
            self.go_live_requested();
            cx.notify();
            return;
        }
        if modifiers.shift {
            let anchor = tab.anchor.unwrap_or(index);
            let (from, to) = (anchor.min(index), anchor.max(index));
            tab.selection = tab.rows[from..=to].iter().map(|(id, _)| *id).collect();
        } else if modifiers.secondary() {
            if !tab.selection.remove(&id) {
                tab.selection.insert(id);
            }
            tab.anchor = Some(index);
        } else {
            tab.selection = BTreeSet::from([id]);
            tab.anchor = Some(index);
        }
        tab.cursor = Some(index);
        self.preview_scripture(cx);
        cx.notify();
    }

    fn move_verse(&mut self, step: isize, extend: bool, cx: &mut Context<Self>) {
        let tab = &mut self.scripture;
        let count = tab.rows.len();
        if count == 0 {
            return;
        }
        let target = match (tab.cursor, step) {
            (_, isize::MIN) => 0,
            (_, isize::MAX) => count - 1,
            (Some(cursor), step) => cursor.saturating_add_signed(step).min(count - 1),
            (None, _) => 0,
        };
        let id = tab.rows[target].0;
        if extend {
            let anchor = tab.anchor.unwrap_or(target);
            let (from, to) = (anchor.min(target), anchor.max(target));
            tab.selection = tab.rows[from..=to].iter().map(|(id, _)| *id).collect();
        } else {
            tab.selection = BTreeSet::from([id]);
            tab.anchor = Some(target);
        }
        tab.cursor = Some(target);
        tab.list_state.scroll_to_reveal_item(target);
        self.preview_scripture(cx);
        cx.notify();
    }

    /// Preview shows the selected verses as slides. An empty selection
    /// leaves Preview alone.
    pub(crate) fn preview_scripture(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.scripture_item() else {
            return;
        };
        if self
            .preview
            .as_ref()
            .is_some_and(|p| p.version == item.version)
        {
            return;
        }
        self.selected_song = None;
        self.clear_entry_selection();
        self.live_on_load = None;
        self.live_entry_on_load = None;
        self.preview = Some(item);
        self.preview_slide = 0;
        self.resolve_fonts(cx);
    }

    /// The selected verses as a Preview/Live item: slides of at most
    /// `passage::LAYOUT` lines, each with its own reference and the
    /// translation id as its footer.
    pub(crate) fn scripture_item(&self) -> Option<Item> {
        let tab = &self.scripture;
        let entry = tab.current_entry()?;
        let State::Ready { inventory, .. } = &entry.state else {
            return None;
        };
        let verses = tab.selected_verses();
        if verses.is_empty() {
            return None;
        }
        Some(passage_item(&entry.info, inventory, &verses))
    }

    /// `true` when the typed text could still become the first suggestion
    /// (Tab accepts it).
    fn completion_pending(&self) -> bool {
        self.scripture.mode == Mode::Reference && !self.scripture.suggestions.is_empty()
    }

    fn scripture_button(
        &self,
        focus: &FocusHandle,
        id: &'static str,
        label: impl Into<SharedString>,
        on: impl Fn(&mut Operator, &mut Window, &mut Context<Operator>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let label = label.into();
        let on = Rc::new(on);
        let keyboard = on.clone();
        let handle = focus.clone();
        let clicked = focus.clone();
        crate::operator::controls::chrome_button(id, label.clone())
            .role(Role::Button)
            .aria_label(label)
            .accessibility_id(id)
            .track_focus(focus)
            .key_context("SelaControl")
            .focus(|d| d.bg(rgb(FOCUS_BG)).text_color(rgb(FOCUS_TEXT)))
            .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                if handle.is_focused(window) {
                    window.prevent_default();
                    keyboard(this, window, cx);
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, event, window, cx| {
                if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                    return;
                }
                clicked.focus(window, cx);
                on(this, window, cx);
                cx.notify();
            }))
    }

    pub(crate) fn prompt_scripture_import(&mut self, cx: &mut Context<Self>) {
        if self.scripture.importing.is_some()
            || self.scripture.import_prompt
            || self.scripture.worker.is_none()
        {
            return;
        }
        self.scripture.import_prompt = true;
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t!(Msg::ScriptureImportPrompt).into()),
        });
        cx.spawn(async move |this, cx| {
            // Picked, cancelled or failed: the dialog is closed either way.
            let path = match paths.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.scripture.import_prompt = false;
                if let Some(path) = path {
                    this.start_scripture_import(path);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_scripture_import(&mut self, path: PathBuf) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(worker) = &self.scripture.worker else {
            return;
        };
        match worker.submit(Command::Import(path)) {
            Ok(()) => {
                self.scripture.message = Some((t!(Msg::ScriptureImporting, name = name), false));
                self.scripture.importing = Some(name);
            }
            Err(_) => self.scripture.message = Some((t!(Msg::ScriptureBusy).into(), true)),
        }
    }

    /// Detail area: mode switch, box, suggestions, note and the verse list.
    pub(crate) fn scripture_detail(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.ensure_scripture_started();
        let tab = &self.scripture;
        if let Some(failure) = &tab.failure {
            return empty_detail(failure.clone(), t!(Msg::ScriptureEmptyHint));
        }
        let (placeholder, label) = match tab.mode {
            Mode::Reference => (
                t!(Msg::ScriptureReferencePlaceholder),
                t!(Msg::ScriptureReferenceLabel),
            ),
            Mode::Keyword => (
                t!(Msg::ScriptureKeywordPlaceholder),
                t!(Msg::ScriptureKeywordLabel),
            ),
        };
        self.scripture.input.update(cx, |input, _| {
            input.set_placeholder(placeholder);
            input.set_accessible("scripture-reference", label);
        });
        let tab = &self.scripture;
        let mode_button = |this: &Self, mode: Mode, cx: &mut Context<Self>| {
            let (index, id, label) = match mode {
                Mode::Reference => (
                    0,
                    "scripture-mode-reference",
                    t!(Msg::ScriptureModeReference),
                ),
                Mode::Keyword => (1, "scripture-mode-keyword", t!(Msg::ScriptureModeKeyword)),
            };
            let on = this.scripture.mode == mode;
            this.scripture_button(
                &this.scripture.mode_focus[index],
                id,
                label,
                move |this, _, cx| this.set_scripture_mode(mode, cx),
                cx,
            )
            .aria_toggled(if on { Toggled::True } else { Toggled::False })
            .when(on, |d| {
                d.bg(rgb(SELECTED_ROW))
                    .text_color(rgb(TEXT))
                    .font_weight(FontWeight::MEDIUM)
            })
        };
        let has_text = !tab.input.read(cx).text().is_empty();
        let suggestions = tab.suggestions.clone();
        let note = tab.note.clone();
        let language = tab
            .current_entry()
            .map(|e| e.info.book_language())
            .unwrap_or_else(i18n::language);
        let search_row = div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .px(px(PANE_PAD_X))
            .py_1()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .id("scripture-mode")
                    .role(Role::Group)
                    .aria_label(t!(Msg::ScriptureModeLabel))
                    .accessibility_id("scripture-mode")
                    .flex()
                    .flex_shrink_0()
                    .gap_1()
                    .child(mode_button(self, Mode::Reference, cx))
                    .child(mode_button(self, Mode::Keyword, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .key_context("SelaScriptureReference")
                    .on_action(
                        cx.listener(|this, _: &AcceptScriptureCompletion, window, cx| {
                            if !this.completion_pending() || !this.accept_suggestion(0, window, cx)
                            {
                                cx.propagate();
                            }
                        }),
                    )
                    .on_action(cx.listener(|this, _: &ClearScriptureSearch, window, cx| {
                        if this.scripture.input.read(cx).text().is_empty() {
                            cx.propagate();
                        } else {
                            this.clear_scripture_search(window, cx);
                        }
                    }))
                    .child(self.scripture.input.clone()),
            )
            .when(has_text, |d| {
                d.child(
                    self.scripture_button(
                        &self.scripture.clear_focus,
                        "scripture-clear",
                        "\u{00d7}",
                        |this, window, cx| this.clear_scripture_search(window, cx),
                        cx,
                    )
                    .aria_label(t!(Msg::ScriptureClear)),
                )
            });
        let suggestion_row = (!suggestions.is_empty()).then(|| {
            div()
                .id("scripture-suggestions")
                .role(Role::List)
                .aria_label(t!(Msg::ScriptureSuggestions))
                .accessibility_id("scripture-suggestions")
                .flex_shrink_0()
                .flex()
                .flex_wrap()
                .gap_1()
                .px(px(PANE_PAD_X))
                .py_1()
                .children(suggestions.iter().enumerate().map(|(index, book)| {
                    let id: &'static str = SUGGESTION_IDS[index.min(SUGGESTION_IDS.len() - 1)];
                    self.scripture_button(
                        &self.scripture.suggestion_focus[index],
                        id,
                        book.name(language),
                        move |this, window, cx| {
                            this.accept_suggestion(index, window, cx);
                        },
                        cx,
                    )
                    .role(Role::ListItem)
                    .border_1()
                    .border_color(rgb(BORDER))
                    .when(index == 0, |d| d.text_color(rgb(TEXT)))
                }))
        });
        let note_row = note.map(|note| {
            div()
                .id("scripture-note")
                .debug_selector(|| "scripture-note".into())
                .role(Role::Status)
                .aria_label(note.text.clone())
                .accessibility_id("scripture-note")
                .flex_shrink_0()
                .px(px(PANE_PAD_X))
                .py_1()
                .text_size(px(TEXT_SMALL))
                .text_color(rgb(if note.error { ERROR } else { MUTED }))
                .child(note.text)
        });
        let body = self.verse_list(cx);
        let more = (self.scripture.hits && self.scripture.more).then(|| {
            div()
                .id("scripture-more")
                .debug_selector(|| "scripture-more".into())
                .role(Role::Status)
                .accessibility_id("scripture-more")
                .flex_shrink_0()
                .px(px(PANE_PAD_X))
                .py_1()
                .border_t_1()
                .border_color(rgb(BORDER))
                .text_size(px(TEXT_CAPTION))
                .text_color(rgb(MUTED))
                .child(t!(Msg::ScriptureMoreResults, count = worker::MAX_HITS))
        });
        div()
            .id("scripture-detail")
            .debug_selector(|| "scripture-detail".into())
            .size_full()
            .flex()
            .flex_col()
            .child(search_row)
            .children(suggestion_row)
            .children(note_row)
            .child(body)
            .children(more)
            .into_any_element()
    }

    fn verse_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let tab = &self.scripture;
        let empty = if tab.rows.is_empty() {
            Some(if tab.translations.is_empty() && tab.catalog_pending {
                (t!(Msg::ScriptureLoading), "")
            } else if tab.hits
                && tab.input.read(cx).text().trim().chars().count() >= MIN_QUERY_CHARS
            {
                (t!(Msg::ScriptureNoResults), "")
            } else {
                (t!(Msg::ScriptureEmptyTitle), t!(Msg::ScriptureEmptyHint))
            })
        } else {
            None
        };
        let content = match empty {
            Some((title, hint)) => div()
                .id("scripture-empty")
                .debug_selector(|| "scripture-empty".into())
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().font_weight(FontWeight::MEDIUM).child(title))
                .child(
                    div()
                        .mt_2()
                        .text_size(px(TEXT_SMALL))
                        .text_color(rgb(MUTED))
                        .child(hint),
                )
                .into_any_element(),
            None => {
                let rows = tab.rows.clone();
                let selection = tab.selection.clone();
                let cursor = tab.cursor;
                let hits = tab.hits;
                let weak: WeakEntity<Operator> = cx.entity().downgrade();
                let focus = tab.list_focus.clone();
                let (language, inventory) = tab
                    .current_entry()
                    .map(|e| {
                        let inventory = match &e.state {
                            State::Ready { inventory, .. } => inventory.clone(),
                            State::Damaged(_) => Arc::default(),
                        };
                        (e.info.book_language(), inventory)
                    })
                    .unwrap_or_else(|| (i18n::language(), Arc::default()));
                let count = rows.len();
                list(tab.list_state.clone(), move |index, window, _cx| {
                    let Some((id, text)) = rows.get(index).cloned() else {
                        return Empty.into_any_element();
                    };
                    let selected = selection.contains(&id);
                    let focused = cursor == Some(index) && focus.is_focused(window);
                    let number = if hits {
                        passage::label(&[id], &inventory, language)
                    } else {
                        format!("{}:{}", id.chapter, id.verse)
                    };
                    let weak = weak.clone();
                    let focus = focus.clone();
                    div()
                        .id(("verse", index))
                        .debug_selector(move || format!("verse-{index}"))
                        .role(Role::ListItem)
                        .aria_label(format!("{number} {text}"))
                        .aria_selected(selected)
                        .aria_position_in_set(index)
                        .aria_size_of_set(count)
                        .accessibility_id(format!("verse-{index}"))
                        .w_full()
                        .flex()
                        .gap_2()
                        .px(px(PANE_PAD_X))
                        .py_1()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .cursor_pointer()
                        .bg(rgb(if focused {
                            FOCUS_BG
                        } else if selected {
                            SELECTED_ROW
                        } else {
                            SURFACE
                        }))
                        .hover(|d| d.bg(rgb(HOVER)))
                        .on_click(move |event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_))
                                || event.is_right_click()
                            {
                                return;
                            }
                            focus.focus(window, cx);
                            let modifiers = event.modifiers();
                            let clicks = event.click_count();
                            let _ = weak.update(cx, |this, cx| {
                                this.click_verse(index, modifiers, clicks, cx);
                            });
                        })
                        .child(
                            div()
                                .w(px(if hits { HIT_NUMBER_W } else { NUMBER_W }))
                                .flex_shrink_0()
                                .text_size(px(TEXT_SMALL))
                                .text_color(rgb(if selected { TEXT } else { MUTED }))
                                .font_weight(if selected {
                                    FontWeight::MEDIUM
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child(number),
                        )
                        .child(div().flex_1().min_w_0().child(text))
                        .into_any_element()
                })
                .size_full()
                .into_any_element()
            }
        };
        div()
            .id("scripture-verses")
            .role(Role::List)
            .aria_label(t!(Msg::ScriptureVerseList))
            .accessibility_id("scripture-verses")
            .debug_selector(|| "scripture-verses".into())
            .track_focus(&self.scripture.list_focus)
            .key_context("SelaControl SelaScriptureList")
            .flex_1()
            .min_h_0()
            .on_action(cx.listener(|this, _: &NextVerse, _, cx| this.move_verse(1, false, cx)))
            .on_action(cx.listener(|this, _: &PreviousVerse, _, cx| this.move_verse(-1, false, cx)))
            .on_action(cx.listener(|this, _: &ExtendNextVerse, _, cx| this.move_verse(1, true, cx)))
            .on_action(
                cx.listener(|this, _: &ExtendPreviousVerse, _, cx| this.move_verse(-1, true, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FirstVerse, _, cx| this.move_verse(isize::MIN, false, cx)),
            )
            .on_action(
                cx.listener(|this, _: &LastVerse, _, cx| this.move_verse(isize::MAX, false, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectAllVerses, _, cx| {
                let tab = &mut this.scripture;
                tab.selection = tab.rows.iter().map(|(id, _)| *id).collect();
                this.preview_scripture(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ActivateControl, window, cx| {
                if this.scripture.list_focus.is_focused(window) {
                    window.prevent_default();
                    // Enter/Space select the cursor row alone.
                    this.move_verse(0, false, cx);
                }
            }))
            .child(content)
            .into_any_element()
    }

    /// Left column: installed translations; the shown one is selected.
    pub(crate) fn scripture_sidebar(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let tab = &self.scripture;
        let rows: Vec<(usize, Entry)> = tab.translations.iter().cloned().enumerate().collect();
        let current = tab.current.clone();
        let count = rows.len();
        div()
            .id("scripture-translations")
            .role(Role::List)
            .aria_label(t!(Msg::ScriptureTranslationsHeader))
            .accessibility_id("scripture-translations")
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(t!(Msg::ScriptureTranslationsHeader)),
            )
            .when(rows.is_empty(), |d| {
                d.child(
                    div()
                        .mt_1()
                        .text_size(px(TEXT_SMALL))
                        .text_color(rgb(MUTED))
                        .child(if tab.catalog_pending {
                            t!(Msg::ScriptureLoading)
                        } else {
                            t!(Msg::ScriptureNoTranslations)
                        }),
                )
            })
            .children(rows.into_iter().map(|(index, entry)| {
                let id = entry.info.id.clone();
                let selected = current.as_deref() == Some(id.as_str());
                let damaged = matches!(entry.state, State::Damaged(_));
                // The same words are shown and spoken, so a damaged or
                // restricted translation is never marked by colour alone.
                let name = translation_label(&entry);
                let handle = self.scripture.translation_focus.get(index).cloned();
                let chosen = id.clone();
                let keyboard = id.clone();
                let accessibility = format!("translation-{id}");
                div()
                    .id(SharedString::from(accessibility.clone()))
                    .debug_selector(move || accessibility.clone())
                    .role(Role::ListItem)
                    .aria_label(name.clone())
                    .aria_selected(selected)
                    .aria_position_in_set(index)
                    .aria_size_of_set(count)
                    .accessibility_id(format!("translation-{id}"))
                    .when_some(handle, |d, handle| d.track_focus(&handle))
                    .key_context("SelaControl")
                    .px_2()
                    .py_1()
                    .rounded(px(RADIUS))
                    .cursor_pointer()
                    .truncate()
                    .text_size(px(TEXT_SMALL))
                    .text_color(rgb(if damaged {
                        ERROR
                    } else if selected {
                        TEXT
                    } else {
                        MUTED
                    }))
                    .when(selected, |d| {
                        d.bg(rgb(SELECTED_ROW)).font_weight(FontWeight::MEDIUM)
                    })
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)).text_color(rgb(FOCUS_TEXT)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        window.prevent_default();
                        this.choose_translation(keyboard.clone(), cx);
                    }))
                    .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                        if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                            return;
                        }
                        this.choose_translation(chosen.clone(), cx);
                    }))
                    .child(name)
            }))
            .into_any_element()
    }

    /// Status bar: Import Bible…, Add to Schedule, the selection count and
    /// the latest message.
    pub(crate) fn scripture_footer(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let tab = &self.scripture;
        let selected = tab.selection.len();
        let message = tab.message.clone();
        let importing = tab.importing.is_some() || tab.import_prompt || tab.worker.is_none();
        div()
            .id("scripture-footer")
            .h(px(STATUS_BAR_H))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(
                self.scripture_button(
                    &self.scripture.import_focus,
                    "scripture-import",
                    t!(Msg::ScriptureImport),
                    |this, _, cx| this.prompt_scripture_import(cx),
                    cx,
                )
                .when(importing, |d| mark_disabled(d).text_color(rgb(DISABLED))),
            )
            .child(
                self.scripture_button(
                    &self.scripture.add_focus,
                    "scripture-add",
                    t!(Msg::AddToSchedule),
                    |this, _, _| this.add_scripture_to_schedule(),
                    cx,
                )
                .when(selected == 0, |d| {
                    mark_disabled(d).text_color(rgb(DISABLED))
                }),
            )
            .children(message.map(|(message, failed)| {
                div()
                    .id("scripture-message")
                    .debug_selector(|| "scripture-message".into())
                    .role(Role::Status)
                    .aria_label(message.clone())
                    .accessibility_id("scripture-message")
                    .px_2()
                    .min_w_0()
                    .text_size(px(TEXT_CAPTION))
                    .text_color(rgb(if failed { ERROR } else { MUTED }))
                    .truncate()
                    .child(message)
            }))
            .child(div().flex_1())
            .when(selected > 0, |d| {
                let count = t!(Msg::ScriptureSelectionCount, count = selected);
                d.child(
                    div()
                        .id("scripture-selection-count")
                        .debug_selector(|| "scripture-selection-count".into())
                        .role(Role::Status)
                        .aria_label(count.clone())
                        .accessibility_id("scripture-selection-count")
                        .px_3()
                        .text_size(px(TEXT_CAPTION))
                        .text_color(rgb(MUTED))
                        .child(count),
                )
            })
            .into_any_element()
    }
}

/// Stable ids of the suggestion buttons.
const SUGGESTION_IDS: [&str; SUGGESTIONS] = [
    "scripture-suggestion-0",
    "scripture-suggestion-1",
    "scripture-suggestion-2",
    "scripture-suggestion-3",
    "scripture-suggestion-4",
    "scripture-suggestion-5",
];

/// Builds the Preview/Live item for verses of one translation.
pub(crate) fn passage_item(
    info: &sela::scripture::TranslationInfo,
    inventory: &sela::scripture::Inventory,
    verses: &[(VerseId, String)],
) -> Item {
    let language = info.book_language();
    let ids: Vec<VerseId> = verses.iter().map(|(id, _)| *id).collect();
    let pages = passage::paginate(verses, passage::LAYOUT, true);
    let title = format!(
        "{} ({})",
        passage::label(&ids, inventory, language),
        info.id
    );
    let slides: Vec<Slide> = pages
        .iter()
        .map(|page| Slide {
            label: passage::label(&page.verses, inventory, language),
            text: page.text.clone(),
            format: Default::default(),
            background: None,
        })
        .collect();
    // The strip is two lines (`slides::copyright_strip` shape): this
    // slide's own reference over the translation's name.
    let name = if info.name.trim().is_empty() {
        info.id.clone()
    } else {
        format!("{} ({})", info.name, info.id)
    };
    let footers: Arc<[Arc<str>]> = slides
        .iter()
        .map(|slide| Arc::from(format!("{}\n{name}", slide.label)))
        .collect();
    Item {
        version: Snapshot::new(info, verses).version(),
        entry: None,
        title,
        pages: slides.iter().map(|_| None).collect(),
        slides: slides.into(),
        arrangements: Arc::default(),
        chosen: None,
        copyright: None,
        resolved: None,
        footers: Some(footers),
    }
}
