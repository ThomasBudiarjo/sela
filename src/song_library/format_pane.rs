//! Song Editor Format pane: the Text › Style subset of EasyWorship's Format
//! inspector (EW8-OBS-027..029). Formatting is per slide (EW8-OBS-033); every
//! change is one whole-document undo step, a whole slider or dial drag
//! included. Original GPUI code on GPUI primitives only.
use super::*;
use sela::format::{
    Align, MAX_FIXED_SIZE, MAX_OPACITY, MAX_OUTLINE_SIZE, MAX_SHADOW_BLUR, MAX_SHADOW_OFFSET,
    Outline, Shadow, Size, SlideFormat, VAlign,
};
use std::{cell::RefCell, rc::Rc};

pub(super) const WIDTH: f32 = 284.;
const TAB: isize = 1000;
/// EW8-OBS-029 theme values; a slide that turns Outline or Shadow on starts
/// from these.
pub(super) const OUTLINE: Outline = Outline {
    enabled: true,
    color: [0, 0, 0],
    size: 7,
    opacity: 100,
};
pub(super) const SHADOW: Shadow = Shadow {
    enabled: true,
    color: [0, 0, 0],
    angle: 315,
    offset: 18,
    blur: 9,
    opacity: 90,
};
const WHITE: [u8; 3] = [255, 255, 255];
/// "Do not auto size text" starts from the fitted size (EW8-OBS-028); this
/// stands in until the preview of the current slide has landed.
pub(super) const FALLBACK_SIZE: u16 = 72;
/// A▾/A▴ step in 1080-reference px. EW's step is unobserved (Sela choice).
pub(super) const SIZE_STEP: u16 = 2;
const BUNDLED: &str = "DejaVu Sans";
const BORDER: u32 = 0xcbd0d6;
const ACCENT: u32 = 0x536aca;
const MUTED: u32 = 0xa5a9af;

actions!(
    format_pane,
    [MenuUp, MenuDown, MenuConfirm, MenuDismiss, StepDown, StepUp]
);

pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", MenuUp, Some("SelaMenu")),
        KeyBinding::new("down", MenuDown, Some("SelaMenu")),
        KeyBinding::new("enter", MenuConfirm, Some("SelaMenu")),
        KeyBinding::new("escape", MenuDismiss, Some("SelaMenu")),
        KeyBinding::new("left", StepDown, Some("SelaSlider")),
        KeyBinding::new("down", StepDown, Some("SelaSlider")),
        KeyBinding::new("right", StepUp, Some("SelaSlider")),
        KeyBinding::new("up", StepUp, Some("SelaSlider")),
    ]);
}

/// Numeric settings with a number field (and a slider or dial, except Size).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Num {
    Size,
    OutlineSize,
    OutlineOpacity,
    Angle,
    Offset,
    Blur,
    ShadowOpacity,
}
pub(super) const NUMS: [Num; 7] = [
    Num::Size,
    Num::OutlineSize,
    Num::OutlineOpacity,
    Num::Angle,
    Num::Offset,
    Num::Blur,
    Num::ShadowOpacity,
];

impl Num {
    fn index(self) -> usize {
        self as usize
    }
    pub(super) fn range(self) -> (u16, u16) {
        match self {
            Num::Size => (1, MAX_FIXED_SIZE),
            Num::OutlineSize => (1, MAX_OUTLINE_SIZE.into()),
            Num::OutlineOpacity | Num::ShadowOpacity => (0, MAX_OPACITY.into()),
            Num::Angle => (0, 359),
            Num::Offset => (0, MAX_SHADOW_OFFSET.into()),
            Num::Blur => (0, MAX_SHADOW_BLUR.into()),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Num::Size => "Size",
            Num::OutlineSize => "Outline size",
            Num::OutlineOpacity => "Outline opacity",
            Num::Angle => "Shadow angle",
            Num::Offset => "Shadow offset",
            Num::Blur => "Shadow blur",
            Num::ShadowOpacity => "Shadow opacity",
        }
    }
    /// The shown value: the slide's own, else what the setting would turn on
    /// with (the fitted size, EW8-OBS-029 theme values).
    pub(super) fn get(self, f: &SlideFormat, fitted: u16) -> u16 {
        let o = f.outline.unwrap_or(OUTLINE);
        let s = f.shadow.unwrap_or(SHADOW);
        match self {
            Num::Size => match f.size {
                Some(Size::Fixed(n)) => n,
                _ => fitted,
            },
            Num::OutlineSize => o.size.into(),
            Num::OutlineOpacity => o.opacity.into(),
            Num::Angle => s.angle,
            Num::Offset => s.offset.into(),
            Num::Blur => s.blur.into(),
            Num::ShadowOpacity => s.opacity.into(),
        }
    }
    /// Callers keep `v` inside `range`.
    pub(super) fn set(self, f: &mut SlideFormat, v: u16) {
        let byte = v.min(u16::from(u8::MAX)) as u8;
        match self {
            Num::Size => f.size = Some(Size::Fixed(v)),
            Num::OutlineSize | Num::OutlineOpacity => {
                let mut o = f.outline.unwrap_or(OUTLINE);
                if self == Num::OutlineSize {
                    o.size = byte;
                } else {
                    o.opacity = byte;
                }
                f.outline = Some(o);
            }
            _ => {
                let mut s = f.shadow.unwrap_or(SHADOW);
                match self {
                    Num::Angle => s.angle = v,
                    Num::Offset => s.offset = byte,
                    Num::Blur => s.blur = byte,
                    _ => s.opacity = byte,
                }
                f.shadow = Some(s);
            }
        }
    }
    /// Size edits need "Do not auto size text"; outline and shadow values
    /// need their effect turned on, as in EW's disabled controls.
    pub(super) fn enabled(self, f: &SlideFormat) -> bool {
        match self {
            Num::Size => matches!(f.size, Some(Size::Fixed(_))),
            Num::OutlineSize | Num::OutlineOpacity => f.outline.is_some_and(|o| o.enabled),
            _ => f.shadow.is_some_and(|s| s.enabled),
        }
    }
    fn step(self, value: u16, up: bool) -> u16 {
        let (lo, hi) = self.range();
        match (self, up) {
            (Num::Angle, true) => (value + 1) % 360,
            (Num::Angle, false) => (value + 359) % 360,
            (_, true) => value.saturating_add(1).min(hi),
            (_, false) => value.saturating_sub(1).max(lo),
        }
    }
    /// Layout-order tab index of the slider (or dial) and the number field.
    fn tabs(self) -> (isize, isize) {
        let base = match self {
            Num::Size => 1,
            Num::OutlineSize => 17,
            Num::OutlineOpacity => 19,
            Num::Angle => 23,
            Num::Offset => 25,
            Num::Blur => 27,
            Num::ShadowOpacity => 29,
        };
        (TAB + base, TAB + base + 1)
    }
}

/// The three color settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pick {
    Text,
    Outline,
    Shadow,
}
impl Pick {
    pub(super) fn get(self, f: &SlideFormat) -> [u8; 3] {
        match self {
            Pick::Text => f.color.unwrap_or(WHITE),
            Pick::Outline => f.outline.unwrap_or(OUTLINE).color,
            Pick::Shadow => f.shadow.unwrap_or(SHADOW).color,
        }
    }
    pub(super) fn set(self, f: &mut SlideFormat, color: [u8; 3]) {
        match self {
            Pick::Text => f.color = Some(color),
            Pick::Outline => {
                f.outline = Some(Outline {
                    color,
                    ..f.outline.unwrap_or(OUTLINE)
                })
            }
            Pick::Shadow => {
                f.shadow = Some(Shadow {
                    color,
                    ..f.shadow.unwrap_or(SHADOW)
                })
            }
        }
    }
    fn enabled(self, f: &SlideFormat) -> bool {
        match self {
            Pick::Text => true,
            Pick::Outline => f.outline.is_some_and(|o| o.enabled),
            Pick::Shadow => f.shadow.is_some_and(|s| s.enabled),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Menu {
    Font,
    Size,
    Color(Pick),
    Outline,
    Shadow,
}

/// Focusable pane buttons, in layout order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ctl {
    Font,
    Size,
    SizeDown,
    SizeUp,
    Color,
    Bold,
    Italic,
    Underline,
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
    OutlineType,
    OutlineColor,
    ShadowMode,
    ShadowColor,
}
const CTLS: usize = 18;
impl Ctl {
    fn tab(self) -> isize {
        TAB + match self {
            Ctl::Font => 0,
            Ctl::Size => 2,
            Ctl::SizeDown => 3,
            Ctl::SizeUp => 4,
            Ctl::Color => 5,
            Ctl::Bold => 6,
            Ctl::Italic => 7,
            Ctl::Underline => 8,
            Ctl::Left => 9,
            Ctl::Center => 10,
            Ctl::Right => 11,
            Ctl::Top => 12,
            Ctl::Middle => 13,
            Ctl::Bottom => 14,
            Ctl::OutlineType => 15,
            Ctl::OutlineColor => 16,
            Ctl::ShadowMode => 21,
            Ctl::ShadowColor => 22,
        }
    }
}

#[derive(Clone)]
pub(super) struct Open {
    pub(super) menu: Menu,
    pub(super) highlight: usize,
    restore: Option<FocusHandle>,
}

type Handler = Rc<dyn Fn(&mut Library, &mut Window, &mut Context<Library>)>;

pub(super) struct Pane {
    pub(super) nums: [Entity<TextInput>; 7],
    /// Value last loaded into each number field; `None` reloads it.
    shown: [Option<String>; 7],
    pub(super) hex: Entity<TextInput>,
    hex_shown: Option<String>,
    buttons: [FocusHandle; CTLS],
    sliders: [FocusHandle; 7],
    pub(super) menu_focus: FocusHandle,
    pub(super) menu: Option<Open>,
    /// A click outside closed this menu at this position; the same click
    /// on its trigger must not reopen it.
    dismissed: Option<(Menu, Point<Pixels>)>,
    /// Mouse-down position of the pane click being handled.
    click_at: Option<Point<Pixels>>,
    /// The slider or dial being dragged and the document before the drag.
    pub(super) drag: Option<(Num, Document)>,
    /// Slider and dial bounds from the last paint, by `Num` index.
    pub(super) tracks: Rc<RefCell<[Option<Bounds<Pixels>>; 7]>>,
    fonts: UniformListScrollHandle,
    subscriptions: Vec<Subscription>,
}

impl Pane {
    pub(super) fn new(cx: &mut Context<Library>) -> Self {
        let field = |max: usize, tab: isize, cx: &mut Context<Library>| {
            cx.new(|cx| {
                let mut input =
                    TextInput::new("", false, max, tab, cx).expect("empty text is valid");
                input.set_flow(false);
                // Undo/Redo bubble to the document; a value only enters it
                // on commit.
                input.use_document_history(|_, _| {});
                input
            })
        };
        Self {
            nums: std::array::from_fn(|i| field(8, NUMS[i].tabs().1, cx)),
            shown: Default::default(),
            hex: field(16, TAB + 40, cx),
            hex_shown: None,
            buttons: std::array::from_fn(|_| cx.focus_handle()),
            sliders: std::array::from_fn(|i| {
                cx.focus_handle().tab_stop(true).tab_index(NUMS[i].tabs().0)
            }),
            menu_focus: cx.focus_handle(),
            menu: None,
            dismissed: None,
            click_at: None,
            drag: None,
            tracks: Rc::new(RefCell::new([None; 7])),
            fonts: UniformListScrollHandle::new(),
            subscriptions: Vec::new(),
        }
    }
    pub(super) fn inputs(&self) -> impl Iterator<Item = &Entity<TextInput>> {
        self.nums.iter().chain(std::iter::once(&self.hex))
    }
    /// Reload every field from the draft on the next render.
    pub(super) fn reload(&mut self) {
        self.shown = Default::default();
        self.hex_shown = None;
    }
}

/// `#RRGGBB` (the `#` optional, any case).
pub(super) fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let digits = text.trim().strip_prefix('#').unwrap_or(text.trim());
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

pub(super) fn hex(color: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}

fn rgb_of(color: [u8; 3]) -> Rgba {
    rgb(u32::from_be_bytes([0, color[0], color[1], color[2]]))
}

/// Slider value at `x` across a track starting at `left`, rounded and
/// clamped to the setting's range.
pub(super) fn slider_value(num: Num, x: f32, left: f32, width: f32) -> u16 {
    let (lo, hi) = num.range();
    let fraction = if width > 0. {
        ((x - left) / width).clamp(0., 1.)
    } else {
        0.
    };
    (f32::from(lo) + fraction * f32::from(hi - lo)).round() as u16
}

/// Dial angle for a pointer offset from the dial center in screen
/// coordinates (y down), in the renderer's convention: degrees
/// counterclockwise from the right, so 315 points down-right.
pub(super) fn dial_angle(dx: f32, dy: f32) -> u16 {
    if dx == 0. && dy == 0. {
        return 0;
    }
    let degrees = (-dy).atan2(dx).to_degrees().rem_euclid(360.).round() as u16;
    degrees % 360
}

/// Screen offset of the dial's indicator at `angle`, `radius` from center.
pub(super) fn dial_point(angle: u16, radius: f32) -> (f32, f32) {
    let theta = f32::from(angle).to_radians();
    (radius * theta.cos(), -radius * theta.sin())
}

fn hsl(hue: f32, saturation: f32, lightness: f32) -> [u8; 3] {
    let c = (1. - (2. * lightness - 1.).abs()) * saturation;
    let h = hue / 60.;
    let x = c * (1. - (h % 2. - 1.).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let m = lightness - c / 2.;
    [r, g, b].map(|v| ((v + m) * 255.).round().clamp(0., 255.) as u8)
}

/// Swatch rows: a grey ramp from white to black above a hue × lightness grid
/// (EW8-OBS-028 layout; Sela's own colors).
pub(super) fn palette() -> Vec<Vec<[u8; 3]>> {
    const COLUMNS: usize = 12;
    let mut rows = vec![
        (0..COLUMNS)
            .map(|i| {
                let v = (255. * (1. - i as f32 / (COLUMNS - 1) as f32)).round() as u8;
                [v, v, v]
            })
            .collect::<Vec<_>>(),
    ];
    for lightness in [0.85, 0.7, 0.55, 0.4, 0.25] {
        rows.push(
            (0..COLUMNS)
                .map(|i| hsl(i as f32 * 360. / COLUMNS as f32, 0.8, lightness))
                .collect(),
        );
    }
    rows
}

/// Font trigger text and whether it can open: the list waits for the
/// background scan.
pub(super) fn font_label(
    catalog: Option<&sela::fonts::Catalog>,
    f: &SlideFormat,
) -> (String, bool) {
    match catalog {
        None => ("Loading fonts…".into(), false),
        Some(_) => (f.font.clone().unwrap_or_else(|| BUNDLED.into()), true),
    }
}

fn label(text: &'static str) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(0x646971))
        .child(text)
}

fn section(title: &'static str) -> Div {
    div()
        .mt_3()
        .mb_1()
        .pb_1()
        .border_b_1()
        .border_color(rgb(0xe4e5e3))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(0x646971))
        .child(title)
}

fn inert_box(content: impl IntoElement) -> Div {
    div()
        .flex_shrink_0()
        .h(px(26.))
        .min_w(px(26.))
        .px_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .border_1()
        .border_color(rgb(0xe1e3e5))
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(content)
}

/// Three bars aligned like the text they set.
fn align_icon(align: Align) -> Div {
    let bar = |w: f32| div().w(px(w)).h(px(2.)).bg(rgb(0x4a4f57));
    div()
        .w(px(14.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .map(|d| match align {
            Align::Left => d.items_start(),
            Align::Center => d.items_center(),
            Align::Right => d.items_end(),
        })
        .child(bar(14.))
        .child(bar(9.))
        .child(bar(14.))
}

/// Two bars inside a frame, at its top, middle or bottom.
fn valign_icon(valign: VAlign) -> Div {
    let bar = |w: f32| div().w(px(w)).h(px(2.)).bg(rgb(0x4a4f57));
    div()
        .size(px(14.))
        .border_1()
        .border_color(rgb(0x9a9ea5))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(1.))
        .py(px(1.))
        .map(|d| match valign {
            VAlign::Top => d.justify_start(),
            VAlign::Middle => d.justify_center(),
            VAlign::Bottom => d.justify_end(),
        })
        .child(bar(8.))
        .child(bar(5.))
}

impl Library {
    /// The slide the pane shows: the caret (or selected) slide.
    pub(super) fn pane_format(&self) -> SlideFormat {
        self.draft
            .sections
            .get(self.section)
            .map(|s| s.format.clone())
            .unwrap_or_default()
    }

    /// Slides a format change applies to.
    pub(super) fn format_targets(&self) -> Range<usize> {
        let index = self
            .section
            .min(self.draft.sections.len().saturating_sub(1));
        index..(index + 1).min(self.draft.sections.len())
    }

    /// Fitted size of the current slide in 1080-reference px, once the
    /// preview of exactly this slide has landed.
    pub(super) fn fitted(&self, cx: &App) -> Option<u16> {
        let key = slide_key(&self.preview_slide(cx)?);
        self.preview
            .as_ref()
            .filter(|p| p.key == key)
            .and_then(|p| p.fitted)
    }

    /// One formatting change on every target slide as one undo step.
    /// Invalid results are refused and leave the draft unchanged.
    pub(super) fn apply_format(
        &mut self,
        cx: &mut Context<Self>,
        change: impl Fn(&mut SlideFormat),
    ) -> bool {
        if self.locked() {
            return false;
        }
        self.drag_end(cx);
        let mut song = self.current(cx);
        for index in self.format_targets() {
            change(&mut song.sections[index].format);
            if !song.sections[index].format.is_valid() {
                self.status = "That format value is out of range. Slide unchanged.".into();
                cx.notify();
                return false;
            }
        }
        if !valid_draft(&song) {
            self.status = "The song would exceed its limits. Slide unchanged.".into();
            cx.notify();
            return false;
        }
        self.clear_rejection();
        self.record(song);
        cx.notify();
        true
    }

    fn commit_num(&mut self, num: Num, cx: &mut Context<Self>) {
        let i = num.index();
        let text = self.pane.nums[i].read(cx).text().trim().to_owned();
        let format = self.pane_format();
        let fitted = self.fitted(cx).unwrap_or(FALLBACK_SIZE);
        if self.locked() || !num.enabled(&format) || self.pane.shown[i].as_deref() == Some(&text) {
            self.pane.shown[i] = None;
            cx.notify();
            return;
        }
        let (lo, hi) = num.range();
        match text.parse::<u16>() {
            Ok(value) if (lo..=hi).contains(&value) => {
                if value != num.get(&format, fitted) {
                    self.apply_format(cx, |f| num.set(f, value));
                }
            }
            _ => {
                self.status = format!(
                    "{} must be a whole number from {lo} to {hi}. Slide unchanged.",
                    num.label()
                );
            }
        }
        self.pane.shown[i] = None;
        cx.notify();
    }

    fn step_num(&mut self, num: Num, up: bool, cx: &mut Context<Self>) {
        let format = self.pane_format();
        if !num.enabled(&format) {
            return;
        }
        let value = num.step(num.get(&format, FALLBACK_SIZE), up);
        self.apply_format(cx, |f| num.set(f, value));
    }

    pub(super) fn step_size(&mut self, up: bool, cx: &mut Context<Self>) {
        let base = match self.pane_format().size {
            Some(Size::Fixed(n)) => n,
            _ => self.fitted(cx).unwrap_or(FALLBACK_SIZE),
        };
        let size = if up {
            base.saturating_add(SIZE_STEP).min(MAX_FIXED_SIZE)
        } else {
            base.saturating_sub(SIZE_STEP).max(1)
        };
        self.apply_format(cx, |f| f.size = Some(Size::Fixed(size)));
    }

    pub(super) fn toggle_style(&mut self, ctl: Ctl, cx: &mut Context<Self>) {
        let format = self.pane_format();
        let on = |flag: Option<bool>| Some(flag != Some(true));
        match ctl {
            Ctl::Bold => {
                let value = on(format.bold);
                self.apply_format(cx, |f| f.bold = value)
            }
            Ctl::Italic => {
                let value = on(format.italic);
                self.apply_format(cx, |f| f.italic = value)
            }
            _ => {
                let value = on(format.underline);
                self.apply_format(cx, |f| f.underline = value)
            }
        };
    }

    /// Menu rows: label, enabled, checked.
    fn menu_rows(&self, menu: Menu) -> Vec<(String, bool, bool)> {
        let format = self.pane_format();
        match menu {
            Menu::Font => {
                let families = sela::fonts::shared().catalog();
                std::iter::once((format!("{BUNDLED} (bundled)"), true, format.font.is_none()))
                    .chain(families.iter().flat_map(|c| c.families()).map(|name| {
                        (
                            name.clone(),
                            true,
                            format.font.as_deref() == Some(name.as_str()),
                        )
                    }))
                    .collect()
            }
            Menu::Size => {
                let fixed = matches!(format.size, Some(Size::Fixed(_)));
                vec![
                    ("Do not auto size text".into(), true, fixed),
                    ("Resize text to fit element".into(), true, !fixed),
                    ("Reset Size".into(), true, false),
                ]
            }
            Menu::Outline => {
                let on = format.outline.is_some_and(|o| o.enabled);
                vec![
                    ("None".into(), true, !on),
                    ("Outer".into(), true, on),
                    ("Center".into(), false, false),
                    ("Inner".into(), false, false),
                ]
            }
            Menu::Shadow => {
                let on = format.shadow.is_some_and(|s| s.enabled);
                vec![("None".into(), true, !on), ("Enabled".into(), true, on)]
            }
            Menu::Color(_) => Vec::new(),
        }
    }

    pub(super) fn open_menu(&mut self, menu: Menu, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let dismissed = self.pane.dismissed.take();
        if let (Some((closed, down)), Some(at)) = (dismissed, self.pane.click_at)
            && closed == menu
            && down == at
        {
            return;
        }
        if self.pane.menu.as_ref().is_some_and(|o| o.menu == menu) {
            self.close_menu(window, cx);
            return;
        }
        let highlight = self
            .menu_rows(menu)
            .iter()
            .position(|(_, _, checked)| *checked)
            .unwrap_or(0);
        let restore = self
            .pane
            .menu
            .take()
            .map_or_else(|| window.focused(cx), |open| open.restore);
        self.pane.menu = Some(Open {
            menu,
            highlight,
            restore,
        });
        if let Menu::Color(_) = menu {
            self.pane.hex_shown = None;
            self.pane.hex.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            self.pane.menu_focus.focus(window, cx);
        }
        if menu == Menu::Font {
            self.pane
                .fonts
                .scroll_to_item(highlight, ScrollStrategy::Center);
        }
        cx.notify();
    }

    pub(super) fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.pane.menu.take() {
            if let Some(restore) = open.restore {
                restore.focus(window, cx);
            }
            cx.notify();
        }
    }

    fn move_highlight(&mut self, down: bool, cx: &mut Context<Self>) {
        let Some(open) = self.pane.menu.clone() else {
            return;
        };
        let rows = self.menu_rows(open.menu);
        let mut index = open.highlight;
        for _ in 0..rows.len() {
            index = if down {
                (index + 1).min(rows.len().saturating_sub(1))
            } else {
                index.saturating_sub(1)
            };
            if rows.get(index).is_some_and(|r| r.1) {
                break;
            }
        }
        if let Some(open) = self.pane.menu.as_mut() {
            open.highlight = index;
        }
        if open.menu == Menu::Font {
            self.pane
                .fonts
                .scroll_to_item(index, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    /// Applies a menu row and closes the menu.
    pub(super) fn choose(
        &mut self,
        menu: Menu,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.menu_rows(menu).get(index).is_some_and(|r| r.1) {
            return;
        }
        match (menu, index) {
            (Menu::Font, 0) => {
                self.apply_format(cx, |f| f.font = None);
            }
            (Menu::Font, i) => {
                let Some(name) = sela::fonts::shared()
                    .catalog()
                    .and_then(|c| c.families().get(i - 1).cloned())
                else {
                    return;
                };
                self.apply_format(cx, |f| f.font = Some(name.clone()));
            }
            (Menu::Size, 0) => {
                let size = match self.pane_format().size {
                    Some(Size::Fixed(n)) => n,
                    _ => self
                        .fitted(cx)
                        .unwrap_or(FALLBACK_SIZE)
                        .clamp(1, MAX_FIXED_SIZE),
                };
                self.apply_format(cx, |f| f.size = Some(Size::Fixed(size)));
            }
            (Menu::Size, 1) => {
                self.apply_format(cx, |f| f.size = Some(Size::Auto));
            }
            (Menu::Size, _) => {
                self.apply_format(cx, |f| f.size = None);
            }
            (Menu::Outline, 0) => {
                self.apply_format(cx, |f| {
                    f.outline = f.outline.map(|o| Outline {
                        enabled: false,
                        ..o
                    })
                });
            }
            (Menu::Outline, _) => {
                self.apply_format(cx, |f| {
                    f.outline = Some(Outline {
                        enabled: true,
                        ..f.outline.unwrap_or(OUTLINE)
                    })
                });
            }
            (Menu::Shadow, 0) => {
                self.apply_format(cx, |f| {
                    f.shadow = f.shadow.map(|s| Shadow {
                        enabled: false,
                        ..s
                    })
                });
            }
            (Menu::Shadow, _) => {
                self.apply_format(cx, |f| {
                    f.shadow = Some(Shadow {
                        enabled: true,
                        ..f.shadow.unwrap_or(SHADOW)
                    })
                });
            }
            (Menu::Color(_), _) => {}
        }
        self.close_menu(window, cx);
    }

    pub(super) fn pick_color(
        &mut self,
        pick: Pick,
        color: [u8; 3],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_format(cx, |f| pick.set(f, color));
        self.close_menu(window, cx);
    }

    fn commit_hex(&mut self, pick: Pick, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.pane.hex.read(cx).text().to_owned();
        match parse_hex(&text) {
            Some(color) => self.pick_color(pick, color, window, cx),
            None => {
                self.status = "Enter a color as #RRGGBB. Slide unchanged.".into();
                self.pane.hex_shown = None;
                cx.notify();
            }
        }
    }

    pub(super) fn drag_start(&mut self, num: Num, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.locked() || !num.enabled(&self.pane_format()) {
            return;
        }
        self.drag_end(cx);
        self.record(self.current(cx));
        self.pane.drag = Some((
            num,
            Document {
                song: self.draft.clone(),
                section: self.section,
            },
        ));
        self.drag_to(position, cx);
    }

    /// Follows the pointer without history; the drag becomes one undo step
    /// when it ends.
    pub(super) fn drag_to(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(num) = self.pane.drag.as_ref().map(|(n, _)| *n) else {
            return;
        };
        let Some(bounds) = self.pane.tracks.borrow()[num.index()] else {
            return;
        };
        let value = if num == Num::Angle {
            let center = bounds.center();
            dial_angle(
                f32::from(position.x - center.x),
                f32::from(position.y - center.y),
            )
        } else {
            slider_value(
                num,
                f32::from(position.x),
                f32::from(bounds.origin.x),
                f32::from(bounds.size.width),
            )
        };
        let mut song = self.draft.clone();
        for index in self.format_targets() {
            num.set(&mut song.sections[index].format, value);
        }
        if song != self.draft
            && song.sections.iter().all(|s| s.format.is_valid())
            && valid_draft(&song)
        {
            self.draft = song;
            cx.notify();
        }
    }

    pub(super) fn drag_end(&mut self, cx: &mut Context<Self>) {
        if let Some((_, before)) = self.pane.drag.take() {
            if before.song != self.draft {
                self.history.record(before);
                self.confirm_delete = false;
            }
            cx.notify();
        }
    }

    /// Brings the number fields (and an open color field) in line with the
    /// shown slide, and installs the focus-out commits once a window exists.
    pub(super) fn sync_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pane.subscriptions.is_empty() {
            for num in NUMS {
                let handle = self.pane.nums[num.index()].read(cx).focus_handle(cx);
                self.pane.subscriptions.push(cx.on_focus_out(
                    &handle,
                    window,
                    move |s, _, _, cx| s.commit_num(num, cx),
                ));
            }
            // Focus leaving the open menu (Tab, or a click the capture
            // listener did not see) closes it.
            let hex = self.pane.hex.read(cx).focus_handle(cx);
            for handle in [self.pane.menu_focus.clone(), hex] {
                self.pane
                    .subscriptions
                    .push(cx.on_focus_out(&handle, window, |s, _, w, cx| {
                        let hex = s.pane.hex.read(cx).focus_handle(cx);
                        if s.pane.menu.is_some()
                            && !s.pane.menu_focus.is_focused(w)
                            && !hex.is_focused(w)
                        {
                            s.pane.menu = None;
                            cx.notify();
                        }
                    }));
            }
        }
        let format = self.pane_format();
        let fitted = self.fitted(cx).unwrap_or(FALLBACK_SIZE);
        let locked = self.locked();
        for num in NUMS {
            let i = num.index();
            let value = num.get(&format, fitted).to_string();
            let input = self.pane.nums[i].clone();
            if self.pane.shown[i].as_deref() != Some(value.as_str()) {
                input.update(cx, |f, cx| {
                    let _ = f.set_text(&value, cx);
                });
                self.pane.shown[i] = Some(value);
            }
            input.update(cx, |f, _| f.set_read_only(locked || !num.enabled(&format)));
        }
        self.pane.hex.update(cx, |f, _| f.set_read_only(locked));
        if let Some(Open {
            menu: Menu::Color(pick),
            ..
        }) = self.pane.menu
        {
            let value = hex(pick.get(&format));
            if self.pane.hex_shown.as_deref() != Some(value.as_str()) {
                self.pane.hex.update(cx, |f, cx| {
                    let _ = f.set_text(&value, cx);
                });
                self.pane.hex_shown = Some(value);
            }
        }
    }

    fn pane_button(
        &self,
        ctl: Ctl,
        content: impl IntoElement,
        pressed: bool,
        enabled: bool,
        cx: &mut Context<Self>,
        on: impl Fn(&mut Library, &mut Window, &mut Context<Library>) + 'static,
    ) -> Stateful<Div> {
        let on: Handler = Rc::new(on);
        let base = div()
            .id(("format-control", ctl as usize))
            .debug_selector(move || format!("format-{ctl:?}"))
            .flex_shrink_0()
            .h(px(26.))
            .min_w(px(26.))
            .px_1()
            .flex()
            .items_center()
            .justify_center()
            .gap_1()
            .rounded(px(4.))
            .border_1()
            .text_size(px(12.));
        if !enabled {
            return base
                .border_color(rgb(0xe1e3e5))
                .text_color(rgb(MUTED))
                .opacity(0.6)
                .child(content);
        }
        let activate = on.clone();
        base.track_focus(
            &self.pane.buttons[ctl as usize]
                .clone()
                .tab_stop(true)
                .tab_index(ctl.tab()),
        )
        .key_context("SelaControl")
        .cursor_pointer()
        .border_color(rgb(if pressed { 0x8fa1e0 } else { BORDER }))
        .bg(rgb(if pressed { 0xdce3fa } else { 0xffffff }))
        .text_color(rgb(if pressed { 0x2f4f99 } else { 0x292c30 }))
        .hover(|d| d.border_color(rgb(0x8fa1e0)))
        .focus(|d| d.border_color(rgb(ACCENT)))
        // Keep the caret (and a whole-song selection) in Words.
        .on_mouse_down(MouseButton::Left, |_, w, _| w.prevent_default())
        .on_action(cx.listener(move |s, _: &ActivateControl, w, cx| {
            w.prevent_default();
            activate(s, w, cx)
        }))
        .on_click(cx.listener(move |s, e: &ClickEvent, w, cx| {
            let ClickEvent::Mouse(click) = e else {
                return;
            };
            s.pane.click_at = Some(click.down.position);
            on(s, w, cx);
            s.pane.click_at = None;
        }))
        .child(content)
    }

    /// A dropdown trigger; a click that just dismissed its own menu does not
    /// reopen it.
    fn trigger(
        &self,
        ctl: Ctl,
        menu: Menu,
        content: impl IntoElement,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.pane_button(
            ctl,
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_between()
                .gap_1()
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(content),
                )
                .child(div().text_color(rgb(0x7a7f86)).child("▾")),
            false,
            enabled,
            cx,
            move |s, w, cx| s.open_menu(menu, w, cx),
        )
    }

    fn popover(&self, content: impl IntoElement, cx: &mut Context<Self>) -> Div {
        div().absolute().top(relative(1.)).left_0().child(
            deferred(
                anchored()
                    .offset(point(px(0.), px(4.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .id("format-popover")
                            .occlude()
                            .key_context("SelaMenu")
                            .track_focus(&self.pane.menu_focus)
                            .on_action(
                                cx.listener(|s, _: &MenuUp, _, cx| s.move_highlight(false, cx)),
                            )
                            .on_action(
                                cx.listener(|s, _: &MenuDown, _, cx| s.move_highlight(true, cx)),
                            )
                            .on_action(cx.listener(|s, _: &MenuConfirm, w, cx| {
                                if let Some(open) = s.pane.menu.clone() {
                                    s.choose(open.menu, open.highlight, w, cx);
                                }
                            }))
                            .on_action(cx.listener(|s, _: &MenuDismiss, w, cx| s.close_menu(w, cx)))
                            // Capture phase: the clicked element still
                            // takes focus afterwards if it accepts it.
                            .on_mouse_down_out(cx.listener(|s, e: &MouseDownEvent, w, cx| {
                                if let Some(open) = s.pane.menu.take() {
                                    s.pane.dismissed = Some((open.menu, e.position));
                                    if let Some(restore) = open.restore {
                                        restore.focus(w, cx);
                                    }
                                    cx.notify();
                                }
                            }))
                            .p_1()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(0xd5d8dc))
                            .bg(rgb(0xffffff))
                            .shadow_md()
                            .text_size(px(12.))
                            .child(content),
                    ),
            )
            .with_priority(1),
        )
    }

    fn menu_list(&self, menu: Menu, cx: &mut Context<Self>) -> AnyElement {
        let highlight = self.pane.menu.as_ref().map_or(0, |o| o.highlight);
        let rows = self.menu_rows(menu);
        let row = move |i: usize,
                        (text, enabled, checked): (String, bool, bool),
                        cx: &mut Context<Self>| {
            div()
                .id(("format-menu-row", i))
                .debug_selector(move || format!("format-menu-{i}"))
                .h(px(26.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .rounded(px(4.))
                .when(i == highlight && enabled, |d| d.bg(rgb(0xe8ecfb)))
                .text_color(rgb(if enabled { 0x292c30 } else { MUTED }))
                .child(div().w(px(10.)).child(if checked { "●" } else { "" }))
                .child(text)
                .when(enabled, |d| {
                    d.cursor_pointer()
                        .hover(|d| d.bg(rgb(0xeef0f3)))
                        .on_click(cx.listener(move |s, _, w, cx| s.choose(menu, i, w, cx)))
                })
        };
        if menu == Menu::Font {
            let count = rows.len();
            let current = self.pane_format().font;
            return div()
                .w(px(280.))
                .h(px(320.))
                .child(
                    uniform_list(
                        "format-fonts",
                        count,
                        cx.processor(move |s: &mut Library, range: Range<usize>, _, cx| {
                            let highlight = s.pane.menu.as_ref().map_or(0, |o| o.highlight);
                            let families = sela::fonts::shared().catalog();
                            range
                                .filter_map(|i| {
                                    let name = if i == 0 {
                                        BUNDLED.to_owned()
                                    } else {
                                        families.as_ref()?.families().get(i - 1)?.clone()
                                    };
                                    let checked = if i == 0 {
                                        current.is_none()
                                    } else {
                                        current.as_deref() == Some(name.as_str())
                                    };
                                    Some(
                                        div()
                                            .id(("format-font", i))
                                            .debug_selector(move || format!("format-font-{i}"))
                                            .h(px(28.))
                                            .px_2()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .rounded(px(4.))
                                            .cursor_pointer()
                                            .when(i == highlight, |d| d.bg(rgb(0xe8ecfb)))
                                            .when(checked, |d| d.font_weight(FontWeight::SEMIBOLD))
                                            .hover(|d| d.bg(rgb(0xeef0f3)))
                                            .on_click(cx.listener(move |s, _, w, cx| {
                                                s.choose(Menu::Font, i, w, cx)
                                            }))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .child(if i == 0 {
                                                        format!("{BUNDLED} (bundled)")
                                                    } else {
                                                        name.clone()
                                                    }),
                                            )
                                            // EW8-OBS-028: a sample in the
                                            // family itself. Only visible rows
                                            // render, so GPUI loads only those.
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .max_w(px(130.))
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .text_size(px(15.))
                                                    .font_family(SharedString::from(name.clone()))
                                                    .child(name),
                                            ),
                                    )
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(&self.pane.fonts)
                    .size_full(),
                )
                .into_any_element();
        }
        div()
            .min_w(px(200.))
            .flex()
            .flex_col()
            .when(menu == Menu::Size, |d| {
                d.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0x646971))
                        .child("Auto Sizing"),
                )
            })
            .children(rows.into_iter().enumerate().map(|(i, r)| {
                let separator = menu == Menu::Size && i == 2;
                div()
                    .when(separator, |d| {
                        d.mt_1().pt_1().border_t_1().border_color(rgb(0xe4e5e3))
                    })
                    .child(row(i, r, cx))
            }))
            .into_any_element()
    }

    fn color_popover(&self, pick: Pick, cx: &mut Context<Self>) -> AnyElement {
        let current = pick.get(&self.pane_format());
        div()
            .w(px(268.))
            .p_1()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .text_size(px(12.))
                    .child(
                        div()
                            .border_b_2()
                            .border_color(rgb(ACCENT))
                            .child("Swatches"),
                    )
                    .child(div().text_color(rgb(MUTED)).child("Spectrum"))
                    .child(div().text_color(rgb(MUTED)).child("Values")),
            )
            .children(palette().into_iter().enumerate().map(|(r, row)| {
                div()
                    .flex()
                    .gap(px(2.))
                    .when(r == 0, |d| d.mb_1())
                    .children(row.into_iter().enumerate().map(|(c, color)| {
                        div()
                            .id(("format-swatch", r * 16 + c))
                            .size(px(18.))
                            .rounded(px(3.))
                            .border_1()
                            .border_color(rgb(if color == current { ACCENT } else { 0xd5d8dc }))
                            .when(color == current, |d| d.border_2())
                            .bg(rgb_of(color))
                            .cursor_pointer()
                            .on_click(
                                cx.listener(move |s, _, w, cx| s.pick_color(pick, color, w, cx)),
                            )
                    }))
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(label("Hex Value"))
                    .child(
                        div()
                            .w(px(96.))
                            .h(px(26.))
                            .px_1()
                            .flex()
                            .items_center()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(0xffffff))
                            .on_action(cx.listener(move |s, _: &text_input::Enter, w, cx| {
                                s.commit_hex(pick, w, cx)
                            }))
                            .child(self.pane.hex.clone()),
                    ),
            )
            .into_any_element()
    }

    /// A trigger with its popover when open.
    fn dropdown(&self, menu: Menu, trigger: Stateful<Div>, cx: &mut Context<Self>) -> Div {
        let open = self.pane.menu.as_ref().is_some_and(|o| o.menu == menu);
        div()
            .relative()
            .flex_1()
            .min_w_0()
            .child(trigger.w_full())
            .when(open, |d| {
                let content = match menu {
                    Menu::Color(pick) => self.color_popover(pick, cx),
                    _ => self.menu_list(menu, cx),
                };
                d.child(self.popover(content, cx))
            })
    }

    fn color_dropdown(&self, ctl: Ctl, pick: Pick, cx: &mut Context<Self>) -> Div {
        let format = self.pane_format();
        let (color, enabled) = (pick.get(&format), pick.enabled(&format));
        div().w(px(64.)).flex_shrink_0().child(
            self.dropdown(
                Menu::Color(pick),
                self.trigger(
                    ctl,
                    Menu::Color(pick),
                    div()
                        .w(px(32.))
                        .h(px(14.))
                        .rounded(px(2.))
                        .border_1()
                        .border_color(rgb(0x9a9ea5))
                        .bg(rgb_of(color)),
                    enabled,
                    cx,
                ),
                cx,
            ),
        )
    }

    fn number(&self, num: Num, enabled: bool, cx: &mut Context<Self>) -> Div {
        let arrow = move |up: bool, cx: &mut Context<Self>| {
            div()
                .id(("format-spin", num.index() * 2 + usize::from(up)))
                .h(px(11.))
                .w(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(8.))
                .text_color(rgb(0x7a7f86))
                .child(if up { "▲" } else { "▼" })
                .when(enabled, |d| {
                    d.cursor_pointer()
                        .hover(|d| d.text_color(rgb(ACCENT)))
                        .on_mouse_down(MouseButton::Left, |_, w, _| w.prevent_default())
                        .on_click(cx.listener(move |s, _, _, cx| s.step_num(num, up, cx)))
                })
        };
        div()
            .w(px(64.))
            .flex_shrink_0()
            .h(px(26.))
            .pl_1()
            .flex()
            .items_center()
            .rounded(px(4.))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(0xffffff))
            .when(!enabled, |d| d.opacity(0.5))
            .on_action(cx.listener(move |s, _: &text_input::Enter, _, cx| s.commit_num(num, cx)))
            .on_action(cx.listener(move |s, _: &text_input::Up, _, cx| {
                s.commit_num(num, cx);
                s.step_num(num, true, cx)
            }))
            .on_action(cx.listener(move |s, _: &text_input::Down, _, cx| {
                s.commit_num(num, cx);
                s.step_num(num, false, cx)
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(self.pane.nums[num.index()].clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(arrow(true, cx))
                    .child(arrow(false, cx)),
            )
    }

    fn track_bounds(&self, num: Num) -> impl IntoElement {
        let tracks = self.pane.tracks.clone();
        let i = num.index();
        canvas(
            move |bounds, _, _| tracks.borrow_mut()[i] = Some(bounds),
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
    }

    fn slider(&self, num: Num, enabled: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let (lo, hi) = num.range();
        let value = num.get(&self.pane_format(), FALLBACK_SIZE);
        let fraction = f32::from(value - lo) / f32::from(hi - lo);
        let fill = if enabled { ACCENT } else { 0xc4c8ce };
        div()
            .id(("format-slider", num.index()))
            .debug_selector(move || format!("format-slider-{num:?}"))
            .flex_1()
            .h(px(22.))
            .relative()
            .flex()
            .items_center()
            .rounded(px(4.))
            .when(enabled, |d| {
                d.track_focus(&self.pane.sliders[num.index()])
                    .key_context("SelaSlider")
                    .cursor_pointer()
                    .focus(|d| d.bg(rgb(0xeef1fb)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |s, e: &MouseDownEvent, w, cx| {
                            w.prevent_default();
                            s.drag_start(num, e.position, cx)
                        }),
                    )
                    .on_action(
                        cx.listener(move |s, _: &StepDown, _, cx| s.step_num(num, false, cx)),
                    )
                    .on_action(cx.listener(move |s, _: &StepUp, _, cx| s.step_num(num, true, cx)))
            })
            .child(self.track_bounds(num))
            .child(
                div()
                    .w_full()
                    .h(px(4.))
                    .rounded_full()
                    .bg(rgb(0xdfe2e6))
                    .child(
                        div()
                            .h_full()
                            .w(relative(fraction))
                            .rounded_full()
                            .bg(rgb(fill)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(fraction))
                    .ml(px(-6.))
                    .size(px(12.))
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(fill))
                    .bg(rgb(0xffffff)),
            )
    }

    fn dial(&self, enabled: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let angle = Num::Angle.get(&self.pane_format(), 0);
        let (dx, dy) = dial_point(angle, 8.);
        let ink = if enabled { ACCENT } else { 0xc4c8ce };
        div()
            .id("format-dial")
            .debug_selector(|| "format-dial".into())
            .flex_shrink_0()
            .size(px(28.))
            .relative()
            .rounded_full()
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(0xf6f7f8))
            .when(enabled, |d| {
                d.track_focus(&self.pane.sliders[Num::Angle.index()])
                    .key_context("SelaSlider")
                    .cursor_pointer()
                    .focus(|d| d.border_color(rgb(ACCENT)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|s, e: &MouseDownEvent, w, cx| {
                            w.prevent_default();
                            s.drag_start(Num::Angle, e.position, cx)
                        }),
                    )
                    .on_action(
                        cx.listener(|s, _: &StepDown, _, cx| s.step_num(Num::Angle, false, cx)),
                    )
                    .on_action(cx.listener(|s, _: &StepUp, _, cx| s.step_num(Num::Angle, true, cx)))
            })
            .child(self.track_bounds(Num::Angle))
            .child(
                div()
                    .absolute()
                    .left(px(13. + dx - 3.))
                    .top(px(13. + dy - 3.))
                    .size(px(6.))
                    .rounded_full()
                    .bg(rgb(ink)),
            )
    }

    fn slider_row(&self, num: Num, enabled: bool, cx: &mut Context<Self>) -> Div {
        div()
            .mt_1()
            .flex()
            .flex_col()
            .child(label(match num {
                Num::OutlineSize => "Size",
                Num::Offset => "Offset",
                Num::Blur => "Blur",
                _ => "Opacity",
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(self.slider(num, enabled, cx))
                    .child(self.number(num, enabled, cx)),
            )
    }

    pub(super) fn format_pane(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.sync_pane(window, cx);
        let format = self.pane_format();
        let catalog = sela::fonts::shared().catalog();
        let (font, font_ready) = font_label(catalog.as_deref(), &format);
        let fixed = matches!(format.size, Some(Size::Fixed(_)));
        let outline = format.outline.is_some_and(|o| o.enabled);
        let shadow = format.shadow.is_some_and(|s| s.enabled);
        let style = |ctl: Ctl, content: Div, pressed: bool, cx: &mut Context<Self>| {
            self.pane_button(ctl, content, pressed, true, cx, move |s, _, cx| {
                s.toggle_style(ctl, cx)
            })
        };
        let align = |ctl: Ctl, value: Align, cx: &mut Context<Self>| {
            self.pane_button(
                ctl,
                align_icon(value),
                format.align.unwrap_or(Align::Center) == value,
                true,
                cx,
                move |s, _, cx| {
                    s.apply_format(cx, |f| f.align = Some(value));
                },
            )
        };
        let valign = |ctl: Ctl, value: VAlign, cx: &mut Context<Self>| {
            self.pane_button(
                ctl,
                valign_icon(value),
                format.valign.unwrap_or(VAlign::Middle) == value,
                true,
                cx,
                move |s, _, cx| {
                    s.apply_format(cx, |f| f.valign = Some(value));
                },
            )
        };
        let size_box = if fixed {
            div()
                .flex_1()
                .flex()
                .gap_1()
                .child(self.number(Num::Size, true, cx).flex_1())
                .child(div().w(px(30.)).child(self.dropdown(
                    Menu::Size,
                    self.trigger(Ctl::Size, Menu::Size, "", true, cx),
                    cx,
                )))
        } else {
            div().flex_1().flex().child(self.dropdown(
                Menu::Size,
                self.trigger(Ctl::Size, Menu::Size, "Auto", true, cx),
                cx,
            ))
        };
        div()
            .id("format-pane")
            // Runs after a clicked field focuses itself (bubble order), so
            // only clicks on chrome or disabled controls keep the caret.
            .on_mouse_down(MouseButton::Left, |_, w, _| w.prevent_default())
            .w(px(WIDTH))
            .flex_shrink_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(0xfbfbfa))
            .border_l_1()
            .border_color(rgb(0xdcdedc))
            // EW8-OBS-027 element tabs; only Text is built.
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .px_2()
                    .pt_1()
                    .gap_3()
                    .border_b_1()
                    .border_color(rgb(0xdcdedc))
                    .text_size(px(12.))
                    .child(div().py_1().text_color(rgb(MUTED)).child("Style"))
                    .child(
                        div()
                            .py_1()
                            .border_b_2()
                            .border_color(rgb(ACCENT))
                            .child("Text"),
                    )
                    .child(div().py_1().text_color(rgb(MUTED)).child("Arrange")),
            )
            .child(
                div()
                    .id("format-pane-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_3()
                    .pb_3()
                    .child(
                        div()
                            .mt_2()
                            .mx_auto()
                            .flex()
                            .w(px(150.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .text_size(px(12.))
                            .child(
                                div()
                                    .flex_1()
                                    .py(px(3.))
                                    .flex()
                                    .justify_center()
                                    .bg(rgb(0xdce3fa))
                                    .text_color(rgb(0x2f4f99))
                                    .child("Style"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .py(px(3.))
                                    .flex()
                                    .justify_center()
                                    .text_color(rgb(MUTED))
                                    .child("Layout"),
                            ),
                    )
                    .child(section("FONT"))
                    .child(self.dropdown(
                        Menu::Font,
                        self.trigger(Ctl::Font, Menu::Font, font, font_ready, cx),
                        cx,
                    ))
                    .child(
                        div()
                            .mt_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(div().w(px(40.)).child(label("Size")))
                            .child(size_box)
                            .child(self.pane_button(
                                Ctl::SizeDown,
                                "A▾",
                                false,
                                true,
                                cx,
                                |s, _, cx| s.step_size(false, cx),
                            ))
                            .child(self.pane_button(
                                Ctl::SizeUp,
                                "A▴",
                                false,
                                true,
                                cx,
                                |s, _, cx| s.step_size(true, cx),
                            )),
                    )
                    .child(
                        div()
                            .mt_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(div().w(px(40.)).child(label("Color")))
                            .child(self.color_dropdown(Ctl::Color, Pick::Text, cx)),
                    )
                    .child(
                        div()
                            .mt_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(style(
                                Ctl::Bold,
                                div().font_weight(FontWeight::BOLD).child("B"),
                                format.bold == Some(true),
                                cx,
                            ))
                            .child(style(
                                Ctl::Italic,
                                div().italic().child("I"),
                                format.italic == Some(true),
                                cx,
                            ))
                            .child(style(
                                Ctl::Underline,
                                div().underline().child("U"),
                                format.underline == Some(true),
                                cx,
                            ))
                            .child(inert_box("X²"))
                            .child(inert_box("X₂"))
                            .child(div().flex_1())
                            .child(inert_box("⇤"))
                            .child(inert_box("⇥")),
                    )
                    .child(
                        div()
                            .mt_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(align(Ctl::Left, Align::Left, cx))
                            .child(align(Ctl::Center, Align::Center, cx))
                            .child(align(Ctl::Right, Align::Right, cx))
                            .child(div().w(px(10.)))
                            .child(valign(Ctl::Top, VAlign::Top, cx))
                            .child(valign(Ctl::Middle, VAlign::Middle, cx))
                            .child(valign(Ctl::Bottom, VAlign::Bottom, cx)),
                    )
                    .child(section("OUTLINE"))
                    .child(self.dropdown(
                        Menu::Outline,
                        self.trigger(
                            Ctl::OutlineType,
                            Menu::Outline,
                            if outline { "Outer" } else { "None" },
                            true,
                            cx,
                        ),
                        cx,
                    ))
                    .child(
                        div()
                            .mt_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(self.color_dropdown(Ctl::OutlineColor, Pick::Outline, cx))
                            .child(inert_box("Round ▾").flex_1().justify_between()),
                    )
                    .child(self.slider_row(Num::OutlineSize, outline, cx))
                    .child(self.slider_row(Num::OutlineOpacity, outline, cx))
                    .child(section("SHADOW"))
                    .child(self.dropdown(
                        Menu::Shadow,
                        self.trigger(
                            Ctl::ShadowMode,
                            Menu::Shadow,
                            if shadow { "Enabled" } else { "None" },
                            true,
                            cx,
                        ),
                        cx,
                    ))
                    .child(div().mt_1().flex().child(self.color_dropdown(
                        Ctl::ShadowColor,
                        Pick::Shadow,
                        cx,
                    )))
                    .child(
                        div()
                            .mt_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(40.)).child(label("Angle")))
                            .child(self.dial(shadow, cx))
                            .child(self.number(Num::Angle, shadow, cx)),
                    )
                    .child(self.slider_row(Num::Offset, shadow, cx))
                    .child(self.slider_row(Num::Blur, shadow, cx))
                    .child(self.slider_row(Num::ShadowOpacity, shadow, cx)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn hex_round_trips_and_rejects_malformed_values() {
        assert_eq!(parse_hex("#ff0080"), Some([255, 0, 128]));
        assert_eq!(parse_hex(" 00FF00 "), Some([0, 255, 0]));
        for bad in ["", "#fff", "#12345g", "#1234567", "1234+6"] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
        assert_eq!(hex([255, 0, 128]), "#FF0080");
        assert_eq!(parse_hex(&hex([1, 2, 3])), Some([1, 2, 3]));
    }

    #[test]
    fn slider_maps_position_to_range_and_clamps() {
        assert_eq!(slider_value(Num::OutlineSize, 100., 100., 200.), 1);
        assert_eq!(slider_value(Num::OutlineSize, 300., 100., 200.), 50);
        assert_eq!(slider_value(Num::OutlineSize, 500., 100., 200.), 50);
        assert_eq!(slider_value(Num::ShadowOpacity, 0., 100., 200.), 0);
        assert_eq!(slider_value(Num::ShadowOpacity, 200., 100., 200.), 50);
        assert_eq!(slider_value(Num::Blur, 150., 100., 0.), 0);
    }

    #[test]
    fn dial_uses_the_renderer_angle_convention() {
        assert_eq!(dial_angle(10., 0.), 0);
        assert_eq!(dial_angle(0., -10.), 90);
        assert_eq!(dial_angle(-10., 0.), 180);
        assert_eq!(dial_angle(10., 10.), 315, "down-right, the EW theme shadow");
        assert_eq!(dial_angle(0., 0.), 0);
        for angle in [0, 45, 90, 180, 270, 315, 359] {
            let (dx, dy) = dial_point(angle, 10.);
            assert_eq!(dial_angle(dx, dy), angle);
        }
    }

    #[test]
    fn values_default_to_the_theme_and_set_keeps_the_rest() {
        let mut f = SlideFormat::default();
        assert_eq!(Num::OutlineSize.get(&f, 0), 7);
        assert_eq!(Num::Angle.get(&f, 0), 315);
        assert_eq!(Num::Size.get(&f, 64), 64);
        assert!(!Num::Size.enabled(&f) && !Num::Blur.enabled(&f));
        Num::Blur.set(&mut f, 20);
        assert_eq!(f.shadow, Some(Shadow { blur: 20, ..SHADOW }));
        Pick::Outline.set(&mut f, [9, 8, 7]);
        assert_eq!(
            f.outline,
            Some(Outline {
                color: [9, 8, 7],
                ..OUTLINE
            })
        );
        assert_eq!(Pick::Text.get(&f), WHITE);
        assert_eq!(Num::Angle.step(0, false), 359);
        assert_eq!(Num::Angle.step(359, true), 0);
        assert_eq!(Num::OutlineSize.step(50, true), 50);
        assert_eq!(Num::OutlineSize.step(1, false), 1);
        for num in NUMS {
            let (lo, hi) = num.range();
            for v in [lo, hi] {
                let mut f = SlideFormat::default();
                num.set(&mut f, v);
                assert!(f.is_valid(), "{num:?} {v}");
            }
        }
    }

    #[test]
    fn palette_has_a_grey_ramp_over_a_hue_grid() {
        let rows = palette();
        assert_eq!(rows.len(), 6);
        assert!(rows.iter().all(|r| r.len() == 12));
        assert_eq!(rows[0][0], [255, 255, 255]);
        assert_eq!(rows[0][11], [0, 0, 0]);
        assert!(rows[0].iter().all(|c| c[0] == c[1] && c[1] == c[2]));
        let red = rows[3][0];
        assert!(red[0] > red[1] && red[1] == red[2], "{red:?}");
    }

    #[test]
    fn font_label_waits_for_the_scan() {
        let format = SlideFormat {
            font: Some("Arial".into()),
            ..Default::default()
        };
        assert_eq!(font_label(None, &format), ("Loading fonts…".into(), false));
        let catalog = sela::fonts::Catalog::scan_with(Vec::<PathBuf>::new(), 0);
        assert_eq!(font_label(Some(&catalog), &format), ("Arial".into(), true));
        assert_eq!(
            font_label(Some(&catalog), &SlideFormat::default()),
            ("DejaVu Sans".into(), true)
        );
    }
}
