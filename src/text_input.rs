// Original Sela field; GPUI input/element scaffolding adapted from Zed's Apache
// example at a846890. See docs/text-input.md and LICENSE-GPUI-APACHE.
use gpui::{prelude::*, *};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

const HARD_BYTES: usize = 256 * 1024;
const HARD_LINES: usize = 4096;
const HISTORY_BYTES: usize = 2 * 1024 * 1024;
const ROW: f32 = 22.;
actions!(
    sela_text_input,
    [
        Left,
        Right,
        SelectLeft,
        SelectRight,
        Up,
        Down,
        SelectUp,
        SelectDown,
        Home,
        End,
        SelectAll,
        Backspace,
        Delete,
        Copy,
        Cut,
        Paste,
        Enter,
        Undo,
        Redo
    ]
);

/// Register once per application. All bindings are confined to the field context.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("left", Left, Some("SelaTextInput")),
        KeyBinding::new("right", Right, Some("SelaTextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("SelaTextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("SelaTextInput")),
        KeyBinding::new("up", Up, Some("SelaTextInput")),
        KeyBinding::new("down", Down, Some("SelaTextInput")),
        KeyBinding::new("shift-up", SelectUp, Some("SelaTextInput")),
        KeyBinding::new("shift-down", SelectDown, Some("SelaTextInput")),
        KeyBinding::new("home", Home, Some("SelaTextInput")),
        KeyBinding::new("end", End, Some("SelaTextInput")),
        KeyBinding::new("backspace", Backspace, Some("SelaTextInput")),
        KeyBinding::new("delete", Delete, Some("SelaTextInput")),
        KeyBinding::new("enter", Enter, Some("SelaTextInput")),
    ]);
    for modifier in ["ctrl", "cmd"] {
        cx.bind_keys([
            KeyBinding::new(&format!("{modifier}-a"), SelectAll, Some("SelaTextInput")),
            KeyBinding::new(&format!("{modifier}-c"), Copy, Some("SelaTextInput")),
            KeyBinding::new(&format!("{modifier}-x"), Cut, Some("SelaTextInput")),
            KeyBinding::new(&format!("{modifier}-v"), Paste, Some("SelaTextInput")),
            KeyBinding::new(&format!("{modifier}-z"), Undo, Some("SelaTextInput")),
            KeyBinding::new(&format!("{modifier}-shift-z"), Redo, Some("SelaTextInput")),
        ]);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputError {
    ByteLimit,
    LineLimit,
    Newline,
    InvalidRange,
}
#[derive(Clone)]
struct Snapshot {
    text: String,
    anchor: usize,
    head: usize,
}
#[derive(Default)]
struct Buffer {
    text: String,
    anchor: usize,
    head: usize,
    marked: Option<Range<usize>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    edits: u64,
    external_history: bool,
}
fn from16(text: &str, offset: usize) -> usize {
    let mut count = 0;
    for (byte, ch) in text.char_indices() {
        if count + ch.len_utf16() > offset {
            return byte;
        }
        count += ch.len_utf16();
    }
    text.len()
}
fn range16(text: &str, range: Range<usize>) -> Result<Range<usize>, InputError> {
    if range.start > range.end {
        return Err(InputError::InvalidRange);
    }
    Ok(from16(text, range.start)..from16(text, range.end))
}
fn to16(text: &str, byte: usize) -> usize {
    text[..byte].encode_utf16().count()
}
fn grapheme_floor(text: &str, byte: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .take_while(|i| *i <= byte)
        .last()
        .unwrap_or(0)
}
fn rows(text: &str) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            result.push(
                start..if i > start && text.as_bytes()[i - 1] == b'\r' {
                    i - 1
                } else {
                    i
                },
            );
            start = i + 1;
        }
    }
    result.push(start..text.len());
    result
}
fn trim_history(stack: &mut Vec<Snapshot>) {
    while stack.len() > 32 || stack.iter().map(|s| s.text.len()).sum::<usize>() > HISTORY_BYTES {
        stack.remove(0);
    }
}
impl Buffer {
    fn selection(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            anchor: self.anchor,
            head: self.head,
        }
    }
    fn replace(
        &mut self,
        explicit: Option<Range<usize>>,
        text: &str,
        preedit: Option<Option<Range<usize>>>,
        multiline: bool,
        max: usize,
    ) -> Result<(), InputError> {
        let range = match explicit {
            Some(r) => range16(&self.text, r)?,
            None => self.marked.clone().unwrap_or(self.selection()),
        };
        if !multiline && text.contains(['\r', '\n']) {
            return Err(InputError::Newline);
        }
        let len = self.text.len() - range.len();
        if text.len() > max.saturating_sub(len) {
            return Err(InputError::ByteLimit);
        }
        let lines = self.text[..range.start]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            + text.bytes().filter(|b| *b == b'\n').count()
            + self.text[range.end..]
                .bytes()
                .filter(|b| *b == b'\n')
                .count()
            + 1;
        if lines > HARD_LINES {
            return Err(InputError::LineLimit);
        }
        let relative = match &preedit {
            Some(Some(r)) => Some(range16(text, r.clone())?),
            _ => None,
        };
        if !self.external_history {
            self.undo.push(self.snapshot());
            trim_history(&mut self.undo);
            self.redo.clear();
        }
        self.text.replace_range(range.clone(), text);
        self.marked = preedit
            .and_then(|_| (!text.is_empty()).then_some(range.start..range.start + text.len()));
        let selected = relative
            .map(|r| range.start + r.start..range.start + r.end)
            .unwrap_or(range.start + text.len()..range.start + text.len());
        self.anchor = selected.start;
        self.head = selected.end;
        self.edits += 1;
        Ok(())
    }
    fn boundary(&self, forward: bool) -> usize {
        if forward {
            self.text
                .grapheme_indices(true)
                .find(|(i, _)| *i > self.head)
                .map(|(i, _)| i)
                .unwrap_or(self.text.len())
        } else {
            self.text
                .grapheme_indices(true)
                .rev()
                .find(|(i, _)| *i < self.head)
                .map(|(i, _)| i)
                .unwrap_or(0)
        }
    }
    fn deletion_range(&self, forward: bool) -> Range<usize> {
        let selected = self.selection();
        if !selected.is_empty() {
            return selected;
        }
        let floor = grapheme_floor(&self.text, self.head);
        if floor != self.head {
            return floor..self.boundary(true);
        }
        if forward {
            self.head..self.boundary(true)
        } else {
            self.boundary(false)..self.head
        }
    }
    fn history(&mut self, redo: bool) {
        let next = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        if let Some(s) = next {
            let old = self.snapshot();
            let stack = if redo { &mut self.undo } else { &mut self.redo };
            stack.push(old);
            trim_history(stack);
            self.text = s.text;
            self.anchor = s.anchor;
            self.head = s.head;
            self.marked = None;
            self.edits += 1;
        }
    }
}

type ContentObserver = Box<dyn FnMut(&str, &mut App)>;
/// Typed or composed text, whether it is still composing, whether it would
/// replace the whole text; `true` consumes it.
type TypingHook = Box<dyn FnMut(&str, bool, bool, &mut Window, &mut App) -> bool>;

/// Borderless cell of a larger document (the song editor's Words list).
#[derive(Clone, Copy)]
struct Flow {
    bold: bool,
}

pub struct TextInput {
    focus: FocusHandle,
    buffer: Buffer,
    document_edit: Option<ContentObserver>,
    typing: Option<TypingHook>,
    flow: Option<Flow>,
    placeholder: Option<&'static str>,
    multiline: bool,
    max: usize,
    tab_index: isize,
    error: Option<InputError>,
    read_only: bool,
    layout: Vec<(Range<usize>, ShapedLine)>,
    bounds: Option<Bounds<Pixels>>,
    scroll: Point<Pixels>,
    dragging: bool,
    reveal: bool,
}
impl TextInput {
    pub fn new(
        text: &str,
        multiline: bool,
        max_bytes: usize,
        tab_index: isize,
        cx: &mut Context<Self>,
    ) -> Result<Self, InputError> {
        let mut input = Self {
            focus: cx.focus_handle().tab_index(tab_index).tab_stop(true),
            buffer: Buffer::default(),
            document_edit: None,
            typing: None,
            flow: None,
            placeholder: None,
            multiline,
            max: max_bytes.min(HARD_BYTES),
            tab_index,
            error: None,
            read_only: false,
            layout: Vec::new(),
            bounds: None,
            scroll: point(px(0.), px(0.)),
            dragging: false,
            reveal: true,
        };
        input.set_text(text, cx)?;
        Ok(input)
    }
    pub fn text(&self) -> &str {
        &self.buffer.text
    }
    pub fn edit_count(&self) -> u64 {
        self.buffer.edits
    }
    /// Selected byte range; empty at the cursor. Always on grapheme boundaries.
    #[allow(dead_code)] // Standalone input_check has no section splitting.
    pub fn selection(&self) -> Range<usize> {
        self.buffer.selection()
    }
    pub fn error(&self) -> Option<&InputError> {
        self.error.as_ref()
    }
    /// Synchronous mutation gate, including retained native IME handlers before
    /// redraw. Programmatic set_text remains available for acknowledged loads.
    #[allow(dead_code)] // Standalone input_check has no pending document operations.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }
    /// Synchronous content-only callback; must not read/update this field or edit
    /// while its owner is borrowed. Loads/selection/errors never call it.
    /// Undo/Redo bubble to the enclosing owner; standalone history is disabled.
    #[allow(dead_code)] // Standalone input_check compiles the same component, without an owner.
    pub fn use_document_history(&mut self, on_edit: impl FnMut(&str, &mut App) + 'static) {
        self.document_edit = Some(Box::new(on_edit));
        self.buffer.external_history = true;
        self.buffer.undo.clear();
        self.buffer.redo.clear();
    }
    /// Grey italic hint while the field is empty and nothing is composed.
    #[allow(dead_code)] // Standalone input_check has no hints.
    pub fn set_placeholder(&mut self, placeholder: &'static str) {
        self.placeholder = Some(placeholder);
    }
    /// Document-cell mode: no border, height follows the rows, wheel scrolling
    /// goes to the enclosing list. Up on the first row, Down on the last row,
    /// Enter in a single-line cell and Backspace at the start are not
    /// consumed, so the owner can move between cells.
    #[allow(dead_code)] // Standalone input_check has no document cells.
    pub fn set_flow(&mut self, bold: bool) {
        self.flow = Some(Flow { bold });
    }
    /// Platform text input (typing, IME) is offered to `hook` first; a
    /// consumed change is not applied. The hook runs while this field is
    /// borrowed, so it must defer any access to it. Key actions (Backspace,
    /// Paste, …) are not offered: owners capture those actions instead.
    #[allow(dead_code)] // Standalone input_check has no owner.
    pub fn intercept_typing(
        &mut self,
        hook: impl FnMut(&str, bool, bool, &mut Window, &mut App) -> bool + 'static,
    ) {
        self.typing = Some(Box::new(hook));
    }
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.buffer.anchor = 0;
        self.move_to(self.text().len(), true, cx);
    }
    fn offer_typing(
        &mut self,
        range: &Option<Range<usize>>,
        text: &str,
        composing: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.read_only {
            return false;
        }
        let replaced = match range {
            Some(r) => range16(self.text(), r.clone()).ok(),
            None => Some(
                self.buffer
                    .marked
                    .clone()
                    .unwrap_or(self.buffer.selection()),
            ),
        };
        let whole = replaced == Some(0..self.text().len());
        self.typing
            .as_mut()
            .is_some_and(|hook| hook(text, composing, whole, window, cx))
    }
    /// Collapses the selection to `byte` (clamped, grapheme floor).
    #[allow(dead_code)] // Standalone input_check never moves the cursor itself.
    pub fn set_cursor(&mut self, byte: usize, cx: &mut Context<Self>) {
        self.move_to(byte.min(self.text().len()), false, cx);
    }
    /// Validated atomic load; resets history and selection, increments edit count.
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) -> Result<(), InputError> {
        let result = self.buffer.replace(
            Some(0..self.buffer.text.encode_utf16().count()),
            text,
            None,
            self.multiline,
            self.max,
        );
        self.error = result.as_ref().err().cloned();
        if result.is_ok() {
            self.buffer.undo.clear();
            self.buffer.redo.clear();
            self.buffer.anchor = 0;
            self.buffer.head = 0;
            self.reveal = true;
        }
        cx.notify();
        result
    }
    fn edit(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        preedit: Option<Option<Range<usize>>>,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let before = self
            .document_edit
            .as_ref()
            .map(|_| self.buffer.text.clone());
        self.error = self
            .buffer
            .replace(range, text, preedit, self.multiline, self.max)
            .err();
        if let Some(on_edit) = self.document_edit.as_mut()
            && before.as_deref() != Some(&self.buffer.text)
        {
            on_edit(&self.buffer.text, cx);
        }
        self.reveal = true;
        cx.notify();
    }
    fn move_to(&mut self, index: usize, select: bool, cx: &mut Context<Self>) {
        let index = grapheme_floor(self.text(), index);
        self.buffer.head = index;
        if !select {
            self.buffer.anchor = index;
        }
        self.reveal = true;
        cx.notify();
    }
    fn horizontal(&mut self, forward: bool, select: bool, cx: &mut Context<Self>) {
        let r = self.buffer.selection();
        let index = if !select && !r.is_empty() {
            if forward { r.end } else { r.start }
        } else {
            self.buffer.boundary(forward)
        };
        self.move_to(index, select, cx);
    }
    fn row_index(&self, byte: usize) -> usize {
        self.layout
            .iter()
            .rposition(|(r, _)| r.start <= byte)
            .unwrap_or(0)
    }
    fn vertical(&mut self, down: bool, select: bool, cx: &mut Context<Self>) {
        let row = self.row_index(self.buffer.head);
        let edge = if down {
            row + 1 >= self.layout.len()
        } else {
            row == 0
        };
        if self.flow.is_some() && edge && !select {
            cx.propagate();
            return;
        }
        if let Some((r, line)) = self.layout.get(row) {
            let x = line.x_for_index(self.buffer.head.min(r.end) - r.start);
            let next = if down {
                (row + 1).min(self.layout.len() - 1)
            } else {
                row.saturating_sub(1)
            };
            let (r, line) = &self.layout[next];
            self.move_to(r.start + line.closest_index_for_x(x), select, cx);
        }
    }
    fn hit(&self, p: Point<Pixels>) -> usize {
        let Some(b) = self.bounds else { return 0 };
        let row = (((p.y - b.top() + self.scroll.y) / px(ROW)).floor() as usize)
            .min(self.layout.len().saturating_sub(1));
        let byte = self
            .layout
            .get(row)
            .map(|(r, l)| r.start + l.closest_index_for_x(p.x - b.left() + self.scroll.x))
            .unwrap_or(0);
        grapheme_floor(self.text(), byte)
    }
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let r = self.buffer.selection();
        if !r.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.buffer.text[r].to_owned()));
        }
    }
}
impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let r = range16(self.text(), r).ok()?;
        *actual = Some(to16(self.text(), r.start)..to16(self.text(), r.end));
        Some(self.text()[r].to_owned())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let r = self.buffer.selection();
        Some(UTF16Selection {
            range: to16(self.text(), r.start)..to16(self.text(), r.end),
            reversed: self.buffer.head < self.buffer.anchor,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked
            .as_ref()
            .map(|r| to16(self.text(), r.start)..to16(self.text(), r.end))
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.marked = None;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.offer_typing(&r, text, false, window, cx) {
            return;
        }
        self.edit(r, text, None, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.offer_typing(&r, text, true, window, cx) {
            return;
        }
        self.edit(r, text, Some(selection), cx);
    }
    fn set_selected_text_range(&mut self, r: Range<usize>, _: &mut Window, cx: &mut Context<Self>) {
        match range16(self.text(), r) {
            Ok(r) => {
                self.buffer.anchor = r.start;
                self.buffer.head = r.end;
                self.reveal = true;
            }
            Err(e) => self.error = Some(e),
        }
        cx.notify();
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.text().encode_utf16().count())
    }
    fn bounds_for_range(
        &mut self,
        r: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let r = range16(self.text(), r).ok()?;
        let b = self.bounds?;
        let row = self.row_index(r.start);
        let (lr, l) = self.layout.get(row)?;
        Some(Bounds::new(
            point(
                b.left() + l.x_for_index(r.start.min(lr.end) - lr.start) - self.scroll.x,
                b.top() + px(row as f32 * ROW) - self.scroll.y,
            ),
            size(
                (l.x_for_index(r.end.min(lr.end) - lr.start)
                    - l.x_for_index(r.start.min(lr.end) - lr.start))
                .max(px(1.)),
                px(ROW),
            ),
        ))
    }
    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(to16(self.text(), self.hit(p)))
    }
}

struct FieldElement(Entity<TextInput>);
impl IntoElement for FieldElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for FieldElement {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let input = self.0.read(cx);
        style.size.height = px(if input.flow.is_some() {
            rows(input.text()).len() as f32 * ROW
        } else if input.multiline {
            220.
        } else {
            ROW
        })
        .into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let style = window.text_style();
        self.0.update(cx, |input, _| {
            input.layout = rows(input.text())
                .into_iter()
                .map(|r| {
                    let mut runs = Vec::new();
                    let marked = input.buffer.marked.clone();
                    let mut start = r.start;
                    for end in marked
                        .iter()
                        .flat_map(|m| [m.start, m.end])
                        .filter(|i| *i > r.start && *i < r.end)
                        .chain(std::iter::once(r.end))
                    {
                        runs.push(TextRun {
                            len: end - start,
                            font: style.font(),
                            color: style.color,
                            background_color: None,
                            strikethrough: None,
                            underline: marked
                                .as_ref()
                                .filter(|m| start >= m.start && start < m.end)
                                .map(|_| UnderlineStyle {
                                    color: Some(style.color),
                                    thickness: px(1.),
                                    wavy: false,
                                }),
                        });
                        start = end;
                    }
                    let line = window.text_system().shape_line(
                        input.text()[r.clone()].to_owned().into(),
                        px(13.),
                        &runs,
                        None,
                    );
                    (r, line)
                })
                .collect();
            if input
                .bounds
                .is_none_or(|previous| previous.size != bounds.size)
            {
                input.reveal = true;
            }
            input.bounds = Some(bounds);
            if input.reveal {
                let row = input.row_index(input.buffer.head);
                let (r, line) = &input.layout[row];
                let x = line.x_for_index(input.buffer.head.min(r.end) - r.start);
                let y = px(row as f32 * ROW);
                input.scroll.x = input
                    .scroll
                    .x
                    .min(x)
                    .max(x - bounds.size.width + px(3.))
                    .max(px(0.));
                input.scroll.y = input
                    .scroll
                    .y
                    .min(y)
                    .max(y + px(ROW) - bounds.size.height)
                    .max(px(0.));
                input.reveal = false;
            }
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.0.read(cx).focus.clone();
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.0.clone()), cx);
        self.0.update(cx, |input, cx| {
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                let selected = input.buffer.selection();
                for (row, (r, line)) in input.layout.iter().enumerate() {
                    let origin = point(
                        bounds.left() - input.scroll.x,
                        bounds.top() + px(row as f32 * ROW) - input.scroll.y,
                    );
                    if origin.y + px(ROW) < bounds.top() || origin.y > bounds.bottom() {
                        continue;
                    }
                    if !selected.is_empty() && selected.start <= r.end && selected.end > r.start {
                        let a = line.x_for_index(selected.start.max(r.start).min(r.end) - r.start);
                        let b = line.x_for_index(selected.end.min(r.end) - r.start);
                        window.paint_quad(fill(
                            Bounds::new(
                                origin + point(a, px(0.)),
                                size((b - a).max(px(3.)), px(ROW)),
                            ),
                            rgba(0x4078c840),
                        ));
                    }
                    let _ = line.paint(origin, px(ROW), TextAlign::Left, None, window, cx);
                    if input.focus.is_focused(window) && row == input.row_index(input.buffer.head) {
                        let x = line.x_for_index(input.buffer.head.min(r.end) - r.start);
                        window.paint_quad(fill(
                            Bounds::new(origin + point(x, px(0.)), size(px(1.), px(ROW))),
                            rgb(0x2b65b0),
                        ));
                    }
                }
            });
        });
    }
}
impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.is_focused(window);
        let flow = self.flow;
        let placeholder = self
            .placeholder
            .filter(|_| self.buffer.text.is_empty() && self.buffer.marked.is_none());
        div()
            .id(("text-input", self.tab_index as usize))
            .key_context("SelaTextInput")
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .w_full()
            .relative()
            .when(flow.is_none(), |d| {
                d.p(px(7.))
                    .border_1()
                    .rounded(px(3.))
                    .border_color(rgb(if self.error.is_some() {
                        0xc33b3b
                    } else if focus {
                        0x5485ba
                    } else {
                        0xcbd0d6
                    }))
                    .bg(rgb(0xffffff))
            })
            .when(flow.is_some(), |d| d.px(px(2.)))
            .when(flow.is_some_and(|f| f.bold), |d| {
                d.font_weight(FontWeight::BOLD)
            })
            .text_color(rgb(0x242a31))
            .text_size(px(13.))
            .line_height(px(ROW))
            .on_action(cx.listener(|s, _: &Left, _, cx| s.horizontal(false, false, cx)))
            .on_action(cx.listener(|s, _: &Right, _, cx| s.horizontal(true, false, cx)))
            .on_action(cx.listener(|s, _: &SelectLeft, _, cx| s.horizontal(false, true, cx)))
            .on_action(cx.listener(|s, _: &SelectRight, _, cx| s.horizontal(true, true, cx)))
            .on_action(cx.listener(|s, _: &Up, _, cx| s.vertical(false, false, cx)))
            .on_action(cx.listener(|s, _: &Down, _, cx| s.vertical(true, false, cx)))
            .on_action(cx.listener(|s, _: &SelectUp, _, cx| s.vertical(false, true, cx)))
            .on_action(cx.listener(|s, _: &SelectDown, _, cx| s.vertical(true, true, cx)))
            .on_action(cx.listener(|s, _: &Home, _, cx| {
                let r = rows(s.text());
                let i = r
                    .iter()
                    .rposition(|r| r.start <= s.buffer.head)
                    .unwrap_or(0);
                s.move_to(r[i].start, false, cx)
            }))
            .on_action(cx.listener(|s, _: &End, _, cx| {
                let r = rows(s.text());
                let i = r
                    .iter()
                    .rposition(|r| r.start <= s.buffer.head)
                    .unwrap_or(0);
                s.move_to(r[i].end, false, cx)
            }))
            .on_action(cx.listener(|s, _: &SelectAll, _, cx| s.select_all(cx)))
            .on_action(cx.listener(|s, _: &Backspace, _, cx| {
                if s.flow.is_some() && s.buffer.head == 0 && s.buffer.anchor == 0 {
                    cx.propagate();
                    return;
                }
                let r = s.buffer.deletion_range(false);
                s.edit(
                    Some(to16(s.text(), r.start)..to16(s.text(), r.end)),
                    "",
                    None,
                    cx,
                )
            }))
            .on_action(cx.listener(|s, _: &Delete, _, cx| {
                let r = s.buffer.deletion_range(true);
                s.edit(
                    Some(to16(s.text(), r.start)..to16(s.text(), r.end)),
                    "",
                    None,
                    cx,
                )
            }))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(|s, _: &Cut, w, cx| {
                if !s.buffer.selection().is_empty() {
                    s.copy(&Copy, w, cx);
                    let r = s.buffer.selection();
                    s.edit(
                        Some(to16(s.text(), r.start)..to16(s.text(), r.end)),
                        "",
                        None,
                        cx,
                    )
                }
            }))
            .on_action(cx.listener(|s, _: &Paste, _, cx| {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    s.edit(None, &text, None, cx)
                }
            }))
            .on_action(cx.listener(|s, _: &Enter, _, cx| {
                if s.multiline {
                    s.edit(None, "\n", None, cx)
                } else if s.flow.is_some() {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|s, _: &Undo, _, cx| {
                if s.read_only {
                    return;
                }
                if s.document_edit.is_some() {
                    cx.propagate();
                    return;
                }
                s.buffer.history(false);
                s.error = None;
                s.reveal = true;
                cx.notify()
            }))
            .on_action(cx.listener(|s, _: &Redo, _, cx| {
                if s.read_only {
                    return;
                }
                if s.document_edit.is_some() {
                    cx.propagate();
                    return;
                }
                s.buffer.history(true);
                s.error = None;
                s.reveal = true;
                cx.notify()
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, e: &MouseDownEvent, w, cx| {
                    s.focus.focus(w, cx);
                    s.dragging = true;
                    s.move_to(s.hit(e.position), e.modifiers.shift, cx)
                }),
            )
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                if e.pressed_button != Some(MouseButton::Left) {
                    s.dragging = false;
                }
                if s.dragging {
                    s.move_to(s.hit(e.position), true, cx)
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|s, _, _, _| s.dragging = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|s, _, _, _| s.dragging = false),
            )
            .on_scroll_wheel(cx.listener(|s, e: &ScrollWheelEvent, _, cx| {
                if s.flow.is_some() {
                    return;
                }
                let d = e.delta.pixel_delta(px(ROW));
                let h = px(s.layout.len() as f32 * ROW);
                if let Some(b) = s.bounds {
                    s.scroll.y = (s.scroll.y - d.y)
                        .max(px(0.))
                        .min((h - b.size.height).max(px(0.)));
                    let width = s
                        .layout
                        .iter()
                        .map(|(_, l)| l.width)
                        .fold(px(0.), Pixels::max);
                    s.scroll.x = (s.scroll.x - d.x)
                        .max(px(0.))
                        .min((width - b.size.width).max(px(0.)));
                }
                cx.stop_propagation();
                cx.notify()
            }))
            .child(FieldElement(cx.entity()))
            .children(placeholder.map(|text| {
                div()
                    .absolute()
                    .top(px(if flow.is_some() { 0. } else { 7. }))
                    .left(px(if flow.is_some() { 2. } else { 8. }))
                    .italic()
                    .text_color(rgb(0x9ea3ab))
                    .child(text)
            }))
            .children(self.error.as_ref().map(|e| {
                div()
                    .text_color(rgb(0xb33232))
                    .child(format!("Input rejected: {e:?}"))
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use std::{cell::Cell, rc::Rc};
    actions!(field_test, [ParentEnter]);
    struct Probe {
        input: Entity<TextInput>,
        root: FocusHandle,
        activated: Rc<Cell<usize>>,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let activated = self.activated.clone();
            div()
                .key_context("FieldTestRoot")
                .track_focus(&self.root)
                .size_full()
                .on_action(move |_: &ParentEnter, _, _| activated.set(activated.get() + 1))
                .child(self.input.clone())
                .child(div().child(format!("{}", self.input.read(cx).edit_count())))
        }
    }
    fn fixture(
        cx: &mut TestAppContext,
        multiline: bool,
        text: &str,
    ) -> (
        VisualTestContext,
        Entity<TextInput>,
        FocusHandle,
        Rc<Cell<usize>>,
    ) {
        let activated = Rc::new(Cell::new(0));
        let window = cx.update(|cx| {
            bind_keys(cx);
            cx.bind_keys([KeyBinding::new("enter", ParentEnter, Some("FieldTestRoot"))]);
            cx.open_window(Default::default(), |window, cx| {
                let input =
                    cx.new(|cx| TextInput::new(text, multiline, HARD_BYTES, 0, cx).unwrap());
                input.read(cx).focus.clone().focus(window, cx);
                cx.new(|cx| Probe {
                    input,
                    root: cx.focus_handle(),
                    activated: activated.clone(),
                })
            })
            .unwrap()
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        let (input, root) = window
            .root(&mut visual)
            .unwrap()
            .read_with(&visual, |p, _| (p.input.clone(), p.root.clone()));
        (visual, input, root, activated)
    }
    #[gpui::test]
    fn field_context_consumes_enter_and_clipboard_policy(cx: &mut TestAppContext) {
        let (mut cx, input, root, activated) = fixture(cx, false, "Title");
        cx.simulate_keystrokes("enter");
        assert_eq!(activated.get(), 0);
        assert_eq!(input.read_with(&cx, |i, _| i.text().to_owned()), "Title");
        cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("one\r\ntwo".into())));
        cx.simulate_keystrokes("ctrl-a ctrl-v");
        assert_eq!(
            input.read_with(&cx, |i, _| i.error.clone()),
            Some(InputError::Newline)
        );
        cx.update(|w, cx| root.focus(w, cx));
        cx.simulate_keystrokes("enter");
        assert_eq!(activated.get(), 1);
        cx.simulate_keystrokes("ctrl-a backspace");
        assert_eq!(input.read_with(&cx, |i, _| i.text().to_owned()), "Title");
    }
    #[gpui::test]
    fn native_callbacks_geometry_and_scroll(cx: &mut TestAppContext) {
        let text = format!(
            "prefix 😀e\u{301}\n{}\n",
            (0..30).map(|i| format!("Row {i}\n")).collect::<String>()
        );
        let (mut cx, input, _, _) = fixture(cx, true, &text);
        cx.simulate_resize(size(px(300.), px(400.)));
        cx.update(|w, cx| {
            input.update(cx, |i, cx| {
                let end = i.text().encode_utf16().count();
                i.set_selected_text_range(end..end, w, cx);
            })
        });
        cx.run_until_parked();
        cx.update(|w, cx| {
            input.update(cx, |i, cx| {
                assert!(i.scroll.y > px(0.));
                let end = i.text_length_utf16(w, cx).unwrap();
                let b = i
                    .bounds_for_range(end..end, i.bounds.unwrap(), w, cx)
                    .unwrap();
                assert!(b.top() >= i.bounds.unwrap().top());
                assert!(b.bottom() <= i.bounds.unwrap().bottom());
                assert_eq!(i.character_index_for_point(b.origin, w, cx), Some(end));
                i.scroll.x = px(25.);
                i.scroll.y = px(ROW);
                let b = i.bounds.unwrap();
                let (r, l) = &i.layout[1];
                let index = r.start;
                let p = point(b.left() + l.x_for_index(0) - i.scroll.x, b.top());
                assert_eq!(
                    i.character_index_for_point(p, w, cx),
                    Some(to16(i.text(), index))
                );
                let ime = i
                    .bounds_for_range(to16(i.text(), index)..to16(i.text(), index), b, w, cx)
                    .unwrap();
                assert_eq!(ime.origin, p);
                i.replace_and_mark_text_in_range(Some(7..11), "😀e\u{301}", Some(2..4), w, cx);
                assert_eq!(i.buffer.selection(), 11..14);
                i.unmark_text(w, cx);
                assert!(i.buffer.marked.is_none());
            })
        });
    }
    #[::core::prelude::v1::test]
    fn composition_relative_selection_and_precedence() {
        let mut b = Buffer::default();
        b.replace(None, "prefix tail", None, true, 100).unwrap();
        b.anchor = 7;
        b.head = 11;
        b.replace(None, "😀e\u{301}", Some(Some(2..4)), true, 100)
            .unwrap();
        assert_eq!(b.selection(), 11..14);
        b.replace(Some(0..6), "X", None, true, 100).unwrap();
        assert_eq!(b.text, "X 😀e\u{301}");
    }
    #[::core::prelude::v1::test]
    fn limits_atomic_and_ranges() {
        let mut b = Buffer::default();
        b.replace(None, "😀", None, true, 4).unwrap();
        let old = b.text.clone();
        assert_eq!(
            b.replace(None, "x", None, true, 4),
            Err(InputError::ByteLimit)
        );
        assert_eq!(b.text, old);
        assert_eq!(
            b.replace(Some(Range { start: 2, end: 1 }), "", None, true, 4),
            Err(InputError::InvalidRange)
        );
        assert_eq!(range16("a😀b", 2..99).unwrap(), 1..6);
        b.replace(Some(0..2), "abcd", None, true, 4).unwrap();
        assert_eq!(b.text, "abcd");
        assert_eq!(
            b.replace(Some(1..2), "😀", None, true, 6),
            Err(InputError::ByteLimit)
        );
        b.replace(Some(1..2), "😀", None, true, 7).unwrap();
        assert_eq!(b.text, "a😀cd");
        let before = b.snapshot();
        let edits = b.edits;
        assert_eq!(
            b.replace(
                Some(1..3),
                "ab",
                Some(Some(Range { start: 2, end: 1 })),
                true,
                7
            ),
            Err(InputError::InvalidRange)
        );
        assert_eq!(b.text, before.text);
        assert_eq!(
            b.selection(),
            before.anchor.min(before.head)..before.anchor.max(before.head)
        );
        assert_eq!(b.edits, edits);
    }
    #[::core::prelude::v1::test]
    fn grapheme_and_newline_policy() {
        let mut b = Buffer::default();
        b.replace(None, "a😀e\u{301}", None, true, 100).unwrap();
        assert_eq!(b.boundary(false), 5);
        b.anchor = b.boundary(false);
        b.replace(None, "", None, true, 100).unwrap();
        assert_eq!(b.text, "a😀");
        assert_eq!(
            b.replace(None, "x\r\ny", None, false, 100),
            Err(InputError::Newline)
        );
        assert_eq!(rows("a\r\n\n"), vec![0..1, 3..3, 4..4]);
        assert_eq!(rows(""), vec![0..0]);
    }
    #[::core::prelude::v1::test]
    fn history_bounded() {
        let mut b = Buffer::default();
        for _ in 0..50 {
            b.replace(None, "a", None, true, 100).unwrap();
        }
        assert_eq!(b.undo.len(), 32);
        b.history(false);
        assert_eq!(b.text.len(), 49);
        b.history(true);
        assert_eq!(b.text.len(), 50);
        for _ in 0..40 {
            b.history(false);
        }
        assert_eq!(b.text.len(), 18);
    }
    #[::core::prelude::v1::test]
    fn line_and_history_byte_budgets_and_interior_grapheme() {
        let mut b = Buffer::default();
        assert_eq!(
            b.replace(None, &"\n".repeat(HARD_LINES), None, true, HARD_BYTES),
            Err(InputError::LineLimit)
        );
        assert!(b.text.is_empty());
        b.replace(None, "e\u{301}😀", None, true, 100).unwrap();
        b.head = 1;
        b.anchor = 1;
        assert_eq!(b.deletion_range(false), 0..3);
        assert_eq!(b.deletion_range(true), 0..3);
        b.head = 3;
        b.anchor = 3;
        assert_eq!(b.deletion_range(true), 3..7);
        let text = "a".repeat(HARD_BYTES);
        let mut b = Buffer::default();
        for _ in 0..40 {
            b.replace(Some(0..b.text.len()), &text, None, true, HARD_BYTES)
                .unwrap();
        }
        assert!(b.undo.iter().map(|s| s.text.len()).sum::<usize>() <= HISTORY_BYTES);
        assert_eq!(b.undo.len(), 8);
    }
    #[gpui::test]
    fn document_owner_is_synchronous_content_only_without_field_history(cx: &mut TestAppContext) {
        let (mut cx, input, _, _) = fixture(cx, true, "original");
        let edits = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let captured = edits.clone();
        cx.update(|_, cx| {
            input.update(cx, |i, _| {
                i.use_document_history(move |text, _| captured.borrow_mut().push(text.to_owned()))
            })
        });
        cx.update(|w, cx| {
            input.update(cx, |i, cx| {
                i.replace_text_in_range(Some(0..8), "😀e\u{301}\r\n", w, cx);
                i.replace_text_in_range(Some(0..6), "B", w, cx);
                assert_eq!(edits.borrow().as_slice(), ["😀e\u{301}\r\n", "B"]);
                assert!(i.buffer.undo.is_empty() && i.buffer.redo.is_empty());
                i.replace_text_in_range(Some(Range { start: 2, end: 1 }), "bad", w, cx);
                assert_eq!(edits.borrow().len(), 2);
                i.set_text("loaded", cx).unwrap();
                assert_eq!(edits.borrow().len(), 2);
            })
        });
        cx.simulate_keystrokes("left shift-right ctrl-z ctrl-shift-z");
        assert_eq!(input.read_with(&cx, |i, _| i.text().to_owned()), "loaded");
        assert_eq!(edits.borrow().len(), 2);
    }

    #[gpui::test]
    fn multiline_enter_vertical_direction_and_load_failure(cx: &mut TestAppContext) {
        let (mut cx, input, _, activated) = fixture(cx, true, "a😀e\u{301}\r\n\n");
        cx.simulate_keystrokes("end shift-left");
        cx.update(|w, cx| {
            input.update(cx, |i, cx| {
                let selected = i.selected_text_range(false, w, cx).unwrap();
                assert!(selected.reversed);
                assert_eq!(selected.range, 3..5);
            })
        });
        cx.simulate_keystrokes("backspace down enter");
        assert_eq!(activated.get(), 0);
        assert_eq!(
            input.read_with(&cx, |i, _| i.text().to_owned()),
            "a😀\r\n\n\n"
        );
        cx.update(|_, cx| {
            input.update(cx, |i, cx| {
                let old = i.text().to_owned();
                let count = i.edit_count();
                assert_eq!(
                    i.set_text(&"x".repeat(HARD_BYTES + 1), cx),
                    Err(InputError::ByteLimit)
                );
                assert_eq!(i.text(), old);
                assert_eq!(i.edit_count(), count);
                i.set_text("original\r\nbytes\n", cx).unwrap();
                assert_eq!(i.text(), "original\r\nbytes\n");
                assert!(i.buffer.undo.is_empty());
            })
        });
    }
}
