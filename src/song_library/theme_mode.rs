//! Theme editor (M1-08): the Song Editor in theme mode. The draft is a
//! sample song (EW8-OBS-043 shows sample lyrics on the Layouts canvas) whose
//! one slide carries the theme's text format and background, so the Format
//! pane's Text and Slide tabs, undo/redo, Cancel and the off-thread preview
//! all work unchanged. OK and Apply save the theme to the library; nothing
//! here reaches Live. The title field is the theme name; Theme Elements
//! (copyright, EW8-OBS-032) and margins (Text › Layout, EW8-OBS-030) are the
//! theme-only controls. Reset returns the draft to the built-in look as one
//! undo step.
use super::*;
use sela::format::{MAX_MARGIN, Margins};
use sela::theme::{AspectIssue, Theme, ThemeId};
use std::path::Path;

/// Editor button indexes of the theme-only controls.
pub(crate) const RESET: usize = 30;
pub(crate) const COPYRIGHT: usize = 31;
/// Copyright text the sample song carries while the theme shows the element.
const SAMPLE_COPYRIGHT: &str = "© Sample Publisher";
/// Theme thumbnails: the preview raster box-filtered by 8 (160x90).
const THUMB_SCALE: u32 = 8;
const THUMB: Extent = Extent {
    width: PREVIEW.width / THUMB_SCALE,
    height: PREVIEW.height / THUMB_SCALE,
};
/// Aspect warnings in the editor use a nominal 16:9 output.
const NOMINAL: Extent = Extent {
    width: 1920,
    height: 1080,
};
const MARGIN_TAB: isize = 1100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Side {
    Left,
    Right,
    Top,
    Bottom,
}
const SIDES: [Side; 4] = [Side::Left, Side::Right, Side::Top, Side::Bottom];

impl Side {
    fn get(self, m: Margins) -> u16 {
        match self {
            Side::Left => m.left,
            Side::Right => m.right,
            Side::Top => m.top,
            Side::Bottom => m.bottom,
        }
    }
    fn set(self, m: &mut Margins, value: u16) {
        match self {
            Side::Left => m.left = value,
            Side::Right => m.right = value,
            Side::Top => m.top = value,
            Side::Bottom => m.bottom = value,
        }
    }
    fn label(self) -> &'static str {
        t!(match self {
            Side::Left => Msg::MarginLeft,
            Side::Right => Msg::MarginRight,
            Side::Top => Msg::MarginTop,
            Side::Bottom => Msg::MarginBottom,
        })
    }
    fn id(self) -> &'static str {
        match self {
            Side::Left => "theme-margin-left",
            Side::Right => "theme-margin-right",
            Side::Top => "theme-margin-top",
            Side::Bottom => "theme-margin-bottom",
        }
    }
}

/// Without margins the renderer keeps a fixed 32 px inset; at 1080 lines
/// that is 32 reference points, which the fields show.
const DEFAULT_MARGIN: u16 = 32;

pub(super) struct ThemeEdit {
    pub(super) id: ThemeId,
    margins: [Entity<TextInput>; 4],
    shown: [Option<String>; 4],
    subscriptions: Vec<Subscription>,
    /// Warnings for the draft theme (font, image, contrast, aspect), from
    /// the same off-thread check as the Themes tab.
    pub(super) warnings: Vec<String>,
    warned: Option<u64>,
    warn_task: Option<Task<()>>,
}

/// Opens a theme editor window for `theme` (a new or library theme).
pub(crate) fn open_theme(path: PathBuf, theme: Theme, cx: &mut App) -> Result<(), String> {
    let bounds = Bounds::centered(None, size(px(1180.), px(740.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(760.), px(460.))),
            titlebar: Some(TitlebarOptions {
                title: Some(theme_window_title(&theme.name).into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            let window_id = window.window_handle().window_id();
            let view = cx.new(|cx| {
                let mut library = Library::new(path, cx);
                library.window = Some(window_id);
                library.start_theme(theme, cx);
                library
            });
            let weak = view.downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                weak.update(cx, |view, cx| view.may_close_in(window, cx))
                    .unwrap_or(true)
            });
            view.update(cx, |view, cx| {
                view.fields[0].read(cx).focus_handle(cx).focus(window, cx)
            });
            view
        },
    )
    .map_err(|_| "Cannot open theme editor window".to_string())?;
    Ok(())
}

pub(super) fn theme_window_title(name: &str) -> String {
    let name = name.trim();
    let name = if name.is_empty() {
        t!(Msg::Untitled)
    } else {
        name
    };
    t!(Msg::ThemeEditorWindowTitle, title = name)
}

/// The sample song a theme is edited as: one slide of sample lyrics with
/// the theme's format and background, the name as its title.
pub(super) fn sample(theme: &Theme) -> Song {
    Song {
        title: theme.name.clone(),
        authors: String::new(),
        copyright: if theme.show_copyright {
            SAMPLE_COPYRIGHT.into()
        } else {
            String::new()
        },
        license: String::new(),
        administrator: String::new(),
        song_number: String::new(),
        variants: Vec::new(),
        sections: vec![Section {
            id: SectionId::allocate(),
            label: String::new(),
            lyrics: layout_sample().to_owned(),
            format: theme.format.clone(),
            background: theme.background.clone(),
        }],
        master: None,
    }
}

impl Library {
    pub(super) fn start_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        let margins = std::array::from_fn(|i| {
            cx.new(|cx| {
                let mut input = TextInput::new("", false, 4, MARGIN_TAB + i as isize, cx)
                    .expect("empty text is valid");
                input.set_flow(false);
                input.use_document_history(|_, _| {});
                input
            })
        });
        self.theme = Some(ThemeEdit {
            id: theme.id,
            margins,
            shown: Default::default(),
            subscriptions: Vec::new(),
            warnings: Vec::new(),
            warned: None,
            warn_task: None,
        });
        self.fields[0].update(cx, |f, _| f.set_placeholder(t!(Msg::ThemeNamePlaceholder)));
        self.format = true;
        self.pane.tab = format_pane::Tab::Text;
        self.begin(sample(&theme), None, cx);
        self.set_status(t!(Msg::ThemeEditing));
    }

    /// The theme the draft describes, or `None` outside theme mode.
    pub(super) fn draft_theme(&self, song: &Song) -> Option<Theme> {
        let edit = self.theme.as_ref()?;
        let slide = song.sections.first();
        Some(Theme {
            id: edit.id,
            name: song.title.trim().to_owned(),
            format: slide.map(|s| s.format.clone()).unwrap_or_default(),
            background: slide
                .and_then(|s| s.background.clone())
                .or_else(|| song.master.clone()),
            show_copyright: !song.copyright.trim().is_empty(),
        })
    }

    /// Theme-mode handling of an editor button; `false` leaves it to the
    /// song editor. Song-only commands do nothing here.
    pub(super) fn theme_button_action(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.theme.is_none() {
            return false;
        }
        match index {
            1 | 2 => self.save_theme(cx),
            15 => {
                self.save_theme(cx);
                self.close_after_save = matches!(self.pending, Some(Pending::SaveTheme(_)));
            }
            RESET => self.reset_theme(cx),
            COPYRIGHT => self.toggle_copyright(cx),
            0 | 3 | 7 | 8 | 9 | 12 | 13 | 20..=28 => {}
            _ => return false,
        }
        let _ = window;
        true
    }

    fn save_theme(&mut self, cx: &mut Context<Self>) {
        let song = self.current(cx);
        let Some(theme) = self.draft_theme(&song) else {
            return;
        };
        if self.fields[0].read(cx).error().is_some() || !sela::theme::name_ok(&theme.name) {
            self.reject_status(t!(Msg::ThemeNameInvalid, max = sela::theme::MAX_NAME));
        } else if !theme.is_valid() || theme.encode().len() > sela::theme::MAX_ENCODED {
            self.reject_status(t!(Msg::FormatOutOfRange));
        } else {
            self.submit(Command::SaveTheme(theme), Pending::SaveTheme(song), cx);
        }
    }

    /// Back to the built-in look: no format, no background, copyright
    /// shown. The name and sample text stay. One undo step.
    fn reset_theme(&mut self, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let mut song = self.current(cx);
        for section in &mut song.sections {
            section.format = Default::default();
            section.background = None;
        }
        song.master = None;
        song.copyright = SAMPLE_COPYRIGHT.into();
        self.record(song);
        self.load_fields(cx);
        self.pane.reload();
        self.set_status(t!(Msg::ThemeResetDone));
    }

    fn toggle_copyright(&mut self, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let mut song = self.current(cx);
        song.copyright = if song.copyright.trim().is_empty() {
            SAMPLE_COPYRIGHT.into()
        } else {
            String::new()
        };
        self.record(song);
        self.load_fields(cx);
    }

    fn commit_margin(&mut self, side: Side, cx: &mut Context<Self>) {
        let Some(edit) = &self.theme else {
            return;
        };
        let index = side as usize;
        let text = edit.margins[index].read(cx).text().trim().to_owned();
        if edit.shown[index].as_deref() == Some(text.as_str()) || self.locked() {
            return;
        }
        match text.parse::<u16>() {
            Ok(value) if value <= MAX_MARGIN => {
                let format = self.pane_format();
                let mut margins = format.margins.unwrap_or(Margins {
                    left: DEFAULT_MARGIN,
                    right: DEFAULT_MARGIN,
                    top: DEFAULT_MARGIN,
                    bottom: DEFAULT_MARGIN,
                });
                if format.margins.is_none() || side.get(margins) != value {
                    side.set(&mut margins, value);
                    self.apply_format(cx, |f| f.margins = Some(margins));
                }
            }
            _ => self.reject_status(t!(Msg::MarginInvalid, max = MAX_MARGIN)),
        }
        if let Some(edit) = &mut self.theme {
            edit.shown[index] = None;
        }
        cx.notify();
    }

    fn clear_margins(&mut self, cx: &mut Context<Self>) {
        if self.pane_format().margins.is_some() {
            self.apply_format(cx, |f| f.margins = None);
        }
    }

    /// Keeps the margin fields in line with the draft and refreshes the
    /// draft theme's warnings off the UI thread when it changed.
    pub(super) fn sync_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = &self.theme else {
            return;
        };
        if edit.subscriptions.is_empty() {
            let handles: Vec<_> = edit
                .margins
                .iter()
                .map(|m| m.read(cx).focus_handle(cx))
                .collect();
            let subscriptions = SIDES
                .iter()
                .zip(handles)
                .map(|(&side, handle)| {
                    cx.on_focus_out(&handle, window, move |s, _, _, cx| {
                        s.commit_margin(side, cx)
                    })
                })
                .collect();
            if let Some(edit) = &mut self.theme {
                edit.subscriptions = subscriptions;
            }
        }
        let margins = self.pane_format().margins;
        let locked = self.locked();
        let song = self.current(cx);
        let wanted = self.draft_theme(&song);
        let Some(edit) = &mut self.theme else {
            return;
        };
        for side in SIDES {
            let index = side as usize;
            let value = margins.map_or(DEFAULT_MARGIN, |m| side.get(m)).to_string();
            let input = edit.margins[index].clone();
            if edit.shown[index].as_deref() != Some(value.as_str()) {
                input.update(cx, |f, cx| {
                    let _ = f.set_text(&value, cx);
                });
                edit.shown[index] = Some(value);
            }
            input.update(cx, |f, _| f.set_read_only(locked));
        }
        let Some(theme) = wanted else {
            return;
        };
        let stamp = theme.stamp();
        if edit.warned == Some(stamp) || edit.warn_task.is_some() {
            return;
        }
        let profile = self.backdrops.profile.clone();
        let check = cx
            .background_executor()
            .spawn(async move { theme_preview(&theme, profile.as_deref(), NOMINAL).1 });
        edit.warn_task = Some(cx.spawn(async move |this, cx| {
            let warnings = check.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(edit) = &mut this.theme {
                    edit.warn_task = None;
                    edit.warned = Some(stamp);
                    edit.warnings = warnings;
                }
                cx.notify();
            });
        }));
    }

    /// Slide tab, theme mode: Theme Elements (EW8-OBS-032), copyright only.
    pub(super) fn theme_elements(&self, cx: &mut Context<Self>) -> Option<Div> {
        self.theme.as_ref()?;
        let shown = !self.draft.copyright.trim().is_empty();
        Some(
            div()
                .flex()
                .flex_col()
                .child(heading(t!(Msg::ThemeElementsHeader)))
                .child(
                    div()
                        .id("theme-copyright")
                        .debug_selector(|| "theme-copyright".into())
                        .role(Role::CheckBox)
                        .aria_label(t!(Msg::ThemeShowCopyright))
                        .aria_toggled(if shown { Toggled::True } else { Toggled::False })
                        .accessibility_id("theme-copyright")
                        .track_focus(&self.buttons[COPYRIGHT])
                        .key_context("SelaControl")
                        .flex()
                        .items_center()
                        .gap_2()
                        .py_1()
                        .cursor_pointer()
                        .rounded(px(4.))
                        .focus(|d| d.bg(rgb(theme::FOCUS_BG)))
                        .on_mouse_down(MouseButton::Left, |_, w, _| w.prevent_default())
                        .on_action(cx.listener(|s, _: &ActivateControl, w, cx| {
                            w.prevent_default();
                            s.action(COPYRIGHT, w, cx);
                        }))
                        .on_click(cx.listener(|s, e: &ClickEvent, w, cx| {
                            if matches!(e, ClickEvent::Keyboard(_)) {
                                return;
                            }
                            s.action(COPYRIGHT, w, cx);
                        }))
                        .child(
                            div()
                                .size(px(14.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(3.))
                                .border_1()
                                .border_color(rgb(if shown {
                                    theme::ACCENT
                                } else {
                                    theme::BORDER
                                }))
                                .bg(rgb(if shown { theme::ACCENT } else { 0xffffff }))
                                .text_size(px(10.))
                                .text_color(rgb(0xffffff))
                                .child(if shown { "✓" } else { "" }),
                        )
                        .child(div().text_size(px(12.)).child(t!(Msg::ThemeShowCopyright))),
                ),
        )
    }

    /// Text tab, theme mode: Text › Layout margins (EW8-OBS-030), in
    /// 1080-line points.
    pub(super) fn theme_margins(&self, cx: &mut Context<Self>) -> Option<Div> {
        let edit = self.theme.as_ref()?;
        let set = self.pane_format().margins.is_some();
        Some(
            div()
                .flex()
                .flex_col()
                .child(heading(t!(Msg::ThemeMarginsHeader)))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .children(SIDES.iter().map(|&side| {
                            div()
                                .id(side.id())
                                .debug_selector(move || side.id().into())
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .w(px(46.))
                                        .text_size(px(12.))
                                        .text_color(rgb(theme::MUTED))
                                        .child(side.label()),
                                )
                                .child(
                                    div()
                                        .w(px(64.))
                                        .h(px(26.))
                                        .pl_1()
                                        .flex()
                                        .items_center()
                                        .rounded(px(4.))
                                        .border_1()
                                        .border_color(rgb(theme::BORDER))
                                        .bg(rgb(0xffffff))
                                        .on_action(cx.listener(
                                            move |s, _: &text_input::Enter, _, cx| {
                                                s.commit_margin(side, cx)
                                            },
                                        ))
                                        .child(edit.margins[side as usize].clone()),
                                )
                        })),
                )
                .child(
                    div()
                        .mt_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.))
                        .text_color(rgb(theme::MUTED))
                        .child(if set {
                            t!(Msg::MarginsSet).to_owned()
                        } else {
                            t!(Msg::MarginsDefault).to_owned()
                        })
                        .when(set, |d| {
                            d.child(
                                div()
                                    .id("theme-margins-default")
                                    .debug_selector(|| "theme-margins-default".into())
                                    .px_1()
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_color(rgb(theme::ACCENT))
                                    .hover(|d| d.bg(rgb(theme::HOVER)))
                                    .on_mouse_down(MouseButton::Left, |_, w, _| w.prevent_default())
                                    .on_click(cx.listener(|s, _, _, cx| s.clear_margins(cx)))
                                    .child(t!(Msg::MarginsUseDefault)),
                            )
                        }),
                ),
        )
    }
}

fn heading(title: &'static str) -> Div {
    div()
        .mt_3()
        .mb_1()
        .pb_1()
        .border_b_1()
        .border_color(rgb(0xe4e5e3))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::MUTED))
        .child(title)
}

/// Mean relative luminance of straight RGBA pixels over black, sampled.
fn mean_luminance(rgba: &[u8]) -> f32 {
    let mut sum = 0.;
    let mut count = 0u32;
    for pixel in rgba.as_chunks::<4>().0.iter().step_by(7) {
        let alpha = f32::from(pixel[3]) / 255.;
        let over = |c: u8| (f32::from(c) * alpha).round() as u8;
        sum += sela::theme::luminance([over(pixel[0]), over(pixel[1]), over(pixel[2])]);
        count += 1;
    }
    if count == 0 { 0. } else { sum / count as f32 }
}

/// A theme's thumbnail (sample lyrics over its background, 160x90, rendered
/// exactly like the audience) and its warnings: a font that falls back, a
/// missing or changed image, low contrast, an image aspect that does not
/// match `output`. Reads and decodes files: off the UI thread only.
pub(crate) fn theme_preview(
    theme: &Theme,
    profile: Option<&Path>,
    output: Extent,
) -> (Option<Arc<RenderImage>>, Vec<String>) {
    let mut warnings = Vec::new();
    let resolved = sela::fonts::shared().resolve(&theme.format);
    if resolved.warning().is_some()
        && let Some(name) = theme.format.font.as_deref()
    {
        warnings.push(t!(Msg::ThemeWarnFont, name = name));
    }
    let black = || sela::slides::color_background(sela::background::BLACK);
    let (background, luminance) = match plan(theme.background.as_ref()) {
        Plan::Color(rgb) => (
            sela::slides::color_background(rgb),
            sela::theme::luminance(rgb),
        ),
        Plan::Image(image, aspect) => {
            let decoded = profile
                .ok_or(Substitute::Missing)
                .and_then(|profile| sela::images::decode_background(profile, image));
            match decoded {
                Ok((from, rgba)) => {
                    if let Some(issue) = sela::theme::aspect_issue(from, output, aspect) {
                        warnings.push(match issue {
                            AspectIssue::Distorted(percent) => {
                                t!(Msg::ThemeWarnStretch, percent = percent)
                            }
                            AspectIssue::Bars => t!(Msg::ThemeWarnBars).to_owned(),
                            AspectIssue::Cropped(percent) => {
                                t!(Msg::ThemeWarnCrop, percent = percent)
                            }
                        });
                    }
                    match sela::background::fit(&rgba, from, PREVIEW, aspect) {
                        Some(fitted) => {
                            let luminance = mean_luminance(&fitted);
                            (
                                PreparedBackground::Image {
                                    version: sela::images::background_version(image, aspect),
                                    extent: PREVIEW,
                                    rgba: fitted.into(),
                                },
                                luminance,
                            )
                        }
                        None => {
                            warnings.push(Substitute::Invalid.warning(&image.name));
                            (black(), 0.)
                        }
                    }
                }
                Err(why) => {
                    warnings.push(why.warning(&image.name));
                    (black(), 0.)
                }
            }
        }
    };
    if let Some(ratio) = sela::theme::low_contrast(theme, luminance) {
        warnings.push(t!(Msg::ThemeWarnContrast, ratio = format!("{ratio:.1}")));
    }
    let slide = sela::slides::Slide {
        label: String::new(),
        text: layout_sample().to_owned(),
        format: theme.format.clone(),
        background: theme.background.clone(),
    };
    let image = pixels(&slide, &resolved, background).and_then(|(bgra, _)| {
        let (scale, width) = (THUMB_SCALE as usize, PREVIEW.width as usize);
        let n = (scale * scale) as u32;
        let mut small = Vec::with_capacity(bgra.len() / (scale * scale));
        for y in 0..THUMB.height as usize {
            for x in 0..THUMB.width as usize {
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
        image_from(&small, THUMB)
    });
    (image, warnings)
}

#[cfg(test)]
impl Library {
    /// The theme the draft would save, `None` outside theme mode.
    pub(crate) fn probe_theme(&self, cx: &App) -> Option<Theme> {
        self.draft_theme(&self.current(cx))
    }
    pub(crate) fn probe_theme_warnings(&self) -> Option<Vec<String>> {
        let edit = self.theme.as_ref()?;
        edit.warned.map(|_| edit.warnings.clone())
    }
    /// An editor button: OK (15), Apply (1), Cancel (16), Undo (5), Redo
    /// (6), Reset (`RESET`), Copyright (`COPYRIGHT`).
    pub(crate) fn probe_action(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.action(index, window, cx);
    }
    /// Types `value` into a margin field (0 left .. 3 bottom) and commits.
    pub(crate) fn probe_margin(&mut self, side: usize, value: &str, cx: &mut Context<Self>) {
        let Some(edit) = &self.theme else {
            return;
        };
        let input = edit.margins[side].clone();
        input.update(cx, |f, cx| {
            let _ = f.set_text(value, cx);
        });
        self.commit_margin(SIDES[side], cx);
    }
    pub(crate) fn probe_status(&self) -> String {
        self.status.clone()
    }
    pub(crate) fn probe_close_prompt(&self) -> bool {
        self.confirm_close
    }
}
