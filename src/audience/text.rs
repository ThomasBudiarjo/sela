//! Explicit-font CPU text layers. Run only in preparation, never a UI/frame
//! callback. The three masks are coverage (1 byte/px) over the cue's extent:
//! fill (glyphs, underline, synthetic bold), outline and shadow. Colors and
//! opacities are applied where the masks are blended, not here.
use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache,
    UnderlineStyle, Wrap,
};
use sela::{
    fonts,
    format::{Align as HAlign, VAlign},
    scene::{PreparedCue, TextStyle},
    slides::REFERENCE_HEIGHT,
};

/// Scene's owned-text bound; the auto fit stays below 288 px.
pub const MAX_FONT_SIZE: f32 = 512.0;
/// Same bound as every other source the preparer accepts.
pub const MAX_FONT_FILE: usize = 8 * 1024 * 1024;
const MAX_LINES: usize = 32;
const MAX_TEXT_BYTES: usize = 4096;
const MAX_SIDE: u32 = 4096;
const MAX_AREA: usize = 16 * 1024 * 1024;
/// Renderer text inset on each side, matching the slide fit.
const INSET: u32 = 32;
const LINE_HEIGHT: f32 = 1.3;
/// Ink threshold that seeds dilation; antialiased edges grow from it.
const SEED: u8 = 128;
/// Chamfer distance units: 1/4 px, orthogonal 3, diagonal 4.
const INF: u16 = u16::MAX;
const ORTHO: u16 = 12;
const DIAG: u16 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextError {
    InvalidFont,
    MissingGlyph,
    Overflow,
    Bounds,
}

/// Fill, outline and shadow coverage over the cue's extent.
pub struct Layers {
    pub fill: Vec<u8>,
    pub outline: Vec<u8>,
    pub shadow: Vec<u8>,
}

impl Layers {
    /// Interleave into one RGBA coverage texture (r=fill, g=outline, b=shadow).
    pub fn coverage(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.fill.len() * 4);
        for index in 0..self.fill.len() {
            out.extend([
                self.fill[index],
                self.outline[index],
                self.shadow[index],
                255,
            ]);
        }
        out
    }
}

/// The cue's three masks. Alignment, underline and synthetic styles come from
/// the cue's own style; outline and shadow are scaled from reference-canvas
/// points by the cue's height and may extend into the inset, clipped only at
/// the canvas edge.
pub fn layers(cue: &PreparedCue) -> Result<Layers, TextError> {
    let extent = cue.extent();
    let (width, height) = (extent.width as usize, extent.height as usize);
    let len = width
        .checked_mul(height)
        .filter(|len| *len <= MAX_AREA)
        .ok_or(TextError::Bounds)?;
    let empty = || vec![0u8; len];
    let mut fill = empty();
    let mut outline = empty();
    let mut shadow = empty();
    if let Some(text) = cue.text() {
        let style = text.style();
        let size = f32::from(text.font_size());
        let (area_width, area_height) = (
            extent
                .width
                .checked_sub(2 * INSET)
                .ok_or(TextError::Bounds)?,
            extent
                .height
                .checked_sub(2 * INSET)
                .ok_or(TextError::Bounds)?,
        );
        let mut area = glyph_coverage(
            text.font(),
            text.face_index(),
            text.content(),
            area_width,
            area_height,
            size,
            &style,
        )?;
        if style.synth_bold {
            dilate(
                &mut area,
                area_width as usize,
                area_height as usize,
                size / 48.,
            );
        }
        let (rows, row) = (area_height as usize, area_width as usize);
        for y in 0..rows {
            let start = (y + INSET as usize) * width + INSET as usize;
            fill[start..start + row].copy_from_slice(&area[y * row..(y + 1) * row]);
        }
        if let Some(outline_style) = style.outline {
            let radius =
                f32::from(outline_style.size) * extent.height as f32 / REFERENCE_HEIGHT as f32;
            outline.copy_from_slice(&fill);
            dilate(&mut outline, width, height, radius);
        }
        if let Some(shadow_style) = style.shadow {
            let scale = extent.height as f32 / REFERENCE_HEIGHT as f32;
            let distance = f32::from(shadow_style.offset) * scale;
            let theta = f32::from(shadow_style.angle).to_radians();
            let (dx, dy) = (
                (distance * theta.cos()).round() as i32,
                (distance * -theta.sin()).round() as i32,
            );
            let blur = (f32::from(shadow_style.blur) * scale).round() as i32;
            shadow_of(
                &fill,
                &outline,
                &mut shadow,
                (width, height),
                (dx, dy, blur),
            );
        }
    }
    Ok(Layers {
        fill,
        outline,
        shadow,
    })
}

/// Top-left fill coverage at an exact size, with no inset or effects:
/// diagnostics only. Same rejection rules as `layers`.
pub fn fill(
    font: &[u8],
    face_index: u32,
    text: &str,
    width: u32,
    height: u32,
    font_size: f32,
) -> Result<Vec<u8>, TextError> {
    glyph_coverage(
        font,
        face_index,
        text,
        width,
        height,
        font_size,
        &TextStyle {
            align: HAlign::Left,
            valign: VAlign::Top,
            ..TextStyle::default()
        },
    )
}

/// Glyph and underline coverage in an exact box, aligned and vertically placed
/// by the style. Overflow rules are identical in every placement, so no
/// alignment can hide oversized text.
fn glyph_coverage(
    font: &[u8],
    face_index: u32,
    text: &str,
    width: u32,
    height: u32,
    font_size: f32,
    style: &TextStyle,
) -> Result<Vec<u8>, TextError> {
    if font.len() > MAX_FONT_FILE
        || text.len() > MAX_TEXT_BYTES
        || text.split('\n').count() > MAX_LINES
        || !font_size.is_finite()
        || !(1.0..=MAX_FONT_SIZE).contains(&font_size)
        || width == 0
        || height == 0
        || width > MAX_SIDE
        || height > MAX_SIDE
    {
        return Err(TextError::Bounds);
    }
    let len = (width as usize)
        .checked_mul(height as usize)
        .filter(|len| *len <= MAX_AREA)
        .ok_or(TextError::Bounds)?;
    // Reject oversized collections BEFORE fontdb's per-face allocation.
    if font.get(..4) == Some(b"ttcf") && fonts::collection_faces(font).is_none() {
        return Err(TextError::InvalidFont);
    }
    if ttf_parser::Face::parse(font, face_index).is_err() {
        return Err(TextError::InvalidFont);
    }
    // Load the whole file once, then keep only the requested face so family
    // matching in a collection with duplicate names stays unambiguous.
    let mut loaded = cosmic_text::fontdb::Database::new();
    loaded.load_font_data(font.to_vec());
    let info = loaded
        .faces()
        .find(|face| face.index == face_index)
        .ok_or(TextError::InvalidFont)?
        .clone();
    drop(loaded);
    let mut db = cosmic_text::fontdb::Database::new();
    let id = db.push_face_info(info.clone());
    let family = info
        .families
        .first()
        .ok_or(TextError::InvalidFont)?
        .0
        .clone();
    let weight = info.weight;
    let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
    fonts.get_font(id, weight).ok_or(TextError::InvalidFont)?;
    let line_height = font_size * LINE_HEIGHT;
    let mut buffer = Buffer::new_empty(Metrics::new(font_size, line_height));
    buffer.set_size(Some(width as f32), Some(height as f32));
    buffer.set_wrap(Wrap::None);
    let mut attrs = Attrs::new();
    attrs.family = Family::Name(&family);
    attrs.weight = weight;
    attrs.style = if style.italic {
        Style::Italic
    } else {
        Style::Normal
    };
    // An italic request on a non-italic face makes cosmic-text synthesize the
    // slant; underline spans come from the face's own metrics.
    attrs.text_decoration.underline = if style.underline {
        UnderlineStyle::Single
    } else {
        UnderlineStyle::None
    };
    buffer.set_text(text, &attrs, Shaping::Advanced, None);
    if buffer.lines.len() > MAX_LINES {
        return Err(TextError::Bounds);
    }
    let align = match style.align {
        HAlign::Left => Align::Left,
        HAlign::Center => Align::Center,
        HAlign::Right => Align::Right,
    };
    for line in &mut buffer.lines {
        line.set_align(Some(align));
    }
    buffer.shape_until_scroll(&mut fonts, false);
    // layout_runs is viewport-filtered; validate every logical line first.
    let mut top = 0.0;
    for index in 0..buffer.lines.len() {
        for line in buffer
            .line_layout(&mut fonts, index)
            .ok_or(TextError::InvalidFont)?
        {
            for glyph in &line.glyphs {
                if glyph.glyph_id == 0 || glyph.font_id != id {
                    return Err(TextError::MissingGlyph);
                }
            }
            let line_h = line.line_height_opt.unwrap_or(line_height);
            let ink_height = line.max_ascent + line.max_descent;
            if !line.w.is_finite()
                || line.w > width as f32
                || top + line_h > height as f32
                || ink_height > line_h
            {
                return Err(TextError::Overflow);
            }
            top += line_h;
        }
    }
    let offset = match style.valign {
        VAlign::Top => 0.,
        VAlign::Middle => ((height as f32 - top) / 2.).floor().max(0.),
        VAlign::Bottom => (height as f32 - top).floor().max(0.),
    } as i32;
    let mut alpha = vec![0; len];
    buffer.draw(
        &mut fonts,
        &mut SwashCache::new(),
        Color::rgb(255, 255, 255),
        |x, y, w, h, color| {
            let a = u32::from(color.a());
            if a == 0 {
                return;
            }
            // The line box above is the overflow authority; the fake-italic
            // shear and antialias fringe may cross the area edge, and clip
            // there like any effect clips at the canvas edge.
            let x = x.max(0) as usize;
            let y = (y + offset).max(0) as usize;
            let right = x.saturating_add(w as usize).min(width as usize);
            let bottom = y.saturating_add(h as usize).min(height as usize);
            for row in y..bottom {
                for col in x..right {
                    let dst = &mut alpha[row * width as usize + col];
                    *dst = (a + (u32::from(*dst) * (255 - a) + 127) / 255) as u8;
                }
            }
        },
    );
    Ok(alpha)
}

/// Box of ink at or above the seed threshold, as exclusive bounds.
fn ink_box(mask: &[u8], width: usize, height: usize) -> (usize, usize, usize, usize) {
    let mut box_ = (width, height, 0, 0);
    for y in 0..height {
        for x in 0..width {
            if mask[y * width + x] >= SEED {
                box_.0 = box_.0.min(x);
                box_.1 = box_.1.min(y);
                box_.2 = box_.2.max(x + 1);
                box_.3 = box_.3.max(y + 1);
            }
        }
    }
    box_
}

/// Grow ink outward by `radius` px with a chamfer distance transform, working
/// only inside the ink box expanded by the radius. Existing coverage is kept
/// where it is denser, so antialiased edges never lose ink.
fn dilate(mask: &mut [u8], width: usize, height: usize, radius: f32) {
    if !radius.is_finite() || radius < 0. {
        return;
    }
    let (left, top, right, bottom) = ink_box(mask, width, height);
    if left >= right || top >= bottom {
        return;
    }
    let reach = radius as i32 + 3;
    let (wx, wy) = (
        (left as i32 - reach).max(0) as usize,
        (top as i32 - reach).max(0) as usize,
    );
    let (hx, hy) = (
        (right as i32 + reach).min(width as i32) as usize,
        (bottom as i32 + reach).min(height as i32) as usize,
    );
    let (window_width, window_height) = (hx - wx, hy - wy);
    let at = |x: usize, y: usize| (y - wy) * window_width + (x - wx);
    let mut dist = vec![INF; window_width * window_height];
    for y in wy..hy {
        for x in wx..hx {
            if mask[y * width + x] >= SEED {
                dist[at(x, y)] = 0;
            }
        }
    }
    // `dist` is passed per call so each borrow ends before the pass writes.
    let neighbor = |dist: &[u16], x: usize, y: usize| -> u16 {
        if x >= wx && x < hx && y >= wy && y < hy {
            dist[at(x, y)]
        } else {
            INF
        }
    };
    for y in wy..hy {
        for x in wx..hx {
            if dist[at(x, y)] == 0 {
                continue;
            }
            let here = dist[at(x, y)];
            let candidates = [
                neighbor(&dist, x.wrapping_sub(1), y).saturating_add(ORTHO),
                neighbor(&dist, x, y.wrapping_sub(1)).saturating_add(ORTHO),
                neighbor(&dist, x.wrapping_sub(1), y.wrapping_sub(1)).saturating_add(DIAG),
                neighbor(&dist, x + 1, y.wrapping_sub(1)).saturating_add(DIAG),
            ];
            dist[at(x, y)] = candidates.iter().copied().fold(here, u16::min);
        }
    }
    for y in (wy..hy).rev() {
        for x in (wx..hx).rev() {
            if dist[at(x, y)] == 0 {
                continue;
            }
            let here = dist[at(x, y)];
            let candidates = [
                neighbor(&dist, x + 1, y).saturating_add(ORTHO),
                neighbor(&dist, x, y + 1).saturating_add(ORTHO),
                neighbor(&dist, x + 1, y + 1).saturating_add(DIAG),
                neighbor(&dist, x.wrapping_sub(1), y + 1).saturating_add(DIAG),
            ];
            dist[at(x, y)] = candidates.iter().copied().fold(here, u16::min);
        }
    }
    for y in wy..hy {
        for x in wx..hx {
            let distance = f32::from(dist[at(x, y)]);
            // Chamfer units are twelfths of a pixel (ORTHO per step).
            let coverage = if distance >= f32::from(INF) {
                0.
            } else {
                (radius + 0.5 - distance / 12.).clamp(0., 1.)
            };
            let alpha = (coverage * 255.).round() as u8;
            let dst = &mut mask[y * width + x];
            if alpha > *dst {
                *dst = alpha;
            }
        }
    }
}

/// Shift the union of fill and outline by the shadow offset and blur it with
/// three box passes, all inside a box expanded by the reach, clipped at the
/// canvas edges. Ink never escapes the box, so the edges read as zero.
fn shadow_of(
    fill: &[u8],
    outline: &[u8],
    out: &mut [u8],
    (width, height): (usize, usize),
    (dx, dy, blur): (i32, i32, i32),
) {
    let mut box_ = (width, height, 0, 0);
    let mut union = |mask: &[u8]| {
        for y in 0..height {
            let row = &mask[y * width..(y + 1) * width];
            let first = row.iter().position(|a| *a != 0);
            let last = row.iter().rposition(|a| *a != 0);
            if let (Some(first), Some(last)) = (first, last) {
                box_.0 = box_.0.min(first);
                box_.2 = box_.2.max(last + 1);
                if first <= last {
                    box_.1 = box_.1.min(y);
                    box_.3 = box_.3.max(y + 1);
                }
            }
        }
    };
    union(fill);
    union(outline);
    let (left, top, right, bottom) = box_;
    if left >= right || top >= bottom {
        return;
    }
    let reach = dx.unsigned_abs() as usize + blur.max(0) as usize + 2;
    let vertical = dy.unsigned_abs() as usize + blur.max(0) as usize + 2;
    let (wx, wy) = (left.saturating_sub(reach), top.saturating_sub(vertical));
    let (hx, hy) = ((right + reach).min(width), (bottom + vertical).min(height));
    let value = |x: i32, y: i32| -> u8 {
        if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
            return 0;
        }
        let at = y as usize * width + x as usize;
        fill[at].max(outline[at])
    };
    for y in wy..hy {
        for x in wx..hx {
            out[y * width + x] = value(x as i32 - dx, y as i32 - dy);
        }
    }
    let radius = blur.max(0) as usize;
    if radius > 0 {
        for _ in 0..3 {
            box_blur(out, width, (wx, wy, hx, hy), radius, true);
            box_blur(out, width, (wx, wy, hx, hy), radius, false);
        }
    }
}

/// One separable box pass over a window, reading values outside the window as
/// zero (the caller confines all ink inside it).
fn box_blur(
    buf: &mut [u8],
    width: usize,
    (wx, wy, hx, hy): (usize, usize, usize, usize),
    radius: usize,
    horizontal: bool,
) {
    let n = (2 * radius + 1) as u32;
    // The pass runs along one axis; `outer` walks the fixed cross axis.
    let (start, end) = if horizontal { (wx, hx) } else { (wy, hy) };
    let span = end - start;
    let mut line = vec![0u8; span];
    for outer in if horizontal { wy..hy } else { wx..hx } {
        let at = |axis: usize| -> u8 {
            let (x, y) = if horizontal {
                (axis, outer)
            } else {
                (outer, axis)
            };
            buf[y * width + x]
        };
        let mut sum = 0u32;
        for axis in start.saturating_sub(radius)..start + radius + 1 {
            sum += u32::from(at(axis.min(end - 1)));
        }
        for (offset, slot) in line.iter_mut().enumerate() {
            let axis = start + offset;
            *slot = ((sum + n / 2) / n) as u8;
            let add = axis + radius + 1;
            let remove = axis.checked_sub(radius).filter(|r| *r >= start);
            sum = sum
                .saturating_add(u32::from(if add < end { at(add) } else { 0 }))
                .saturating_sub(u32::from(remove.map_or(0, at)));
        }
        for (offset, value) in line.iter().enumerate() {
            let (x, y) = if horizontal {
                (start + offset, outer)
            } else {
                (outer, start + offset)
            };
            buf[y * width + x] = *value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sela::{
        format::{Align as HAlign, Outline, Shadow, SlideFormat, VAlign},
        scene::{ContentVersion, Extent, OutlineStyle, OwnedText, PreparedBackground, ShadowStyle},
    };

    const FONT: &[u8] = include_bytes!("../../tests/fixtures/DejaVuSans.ttf");
    const CAPS: sela::scene::RendererCapabilities = sela::scene::RendererCapabilities {
        max_texture_dimension: 4096,
    };

    /// Mirrors fonts::tests::collection: a valid two-face collection over the
    /// bundled regular font (not visible from this crate's tests).
    fn collection() -> Vec<u8> {
        const HEADER: usize = 20;
        let mut ttc = Vec::with_capacity(HEADER + FONT.len());
        ttc.extend_from_slice(b"ttcf");
        ttc.extend_from_slice(&[0, 1, 0, 0]);
        ttc.extend_from_slice(&2u32.to_be_bytes());
        ttc.extend_from_slice(&(HEADER as u32).to_be_bytes());
        ttc.extend_from_slice(&(HEADER as u32).to_be_bytes());
        ttc.extend_from_slice(FONT);
        let tables = u16::from_be_bytes([FONT[4], FONT[5]]);
        for record in 0..usize::from(tables) {
            let at = HEADER + 12 + record * 16 + 8;
            let offset = u32::from_be_bytes(ttc[at..at + 4].try_into().unwrap());
            ttc[at..at + 4].copy_from_slice(&(offset + HEADER as u32).to_be_bytes());
        }
        ttc
    }

    fn styled(text: &str, style: TextStyle, extent: Extent, size: u16) -> PreparedCue {
        sela::scene::PreparedCue::from_owned(
            ContentVersion { id: 1, revision: 1 },
            extent,
            PreparedBackground::Color([0, 0, 0, 255]),
            Some(OwnedText {
                content: text.into(),
                font_version: ContentVersion { id: 0, revision: 0 },
                font: FONT.as_ref().into(),
                face_index: 0,
                font_size: size,
                style,
            }),
            CAPS,
        )
        .unwrap()
    }

    /// Ink bounds of a mask as (left, top, right, bottom), exclusive.
    fn bbox(mask: &[u8], width: usize) -> Option<(usize, usize, usize, usize)> {
        let height = mask.len() / width;
        let mut bounds = (width, height, 0, 0);
        for y in 0..height {
            for x in 0..width {
                if mask[y * width + x] != 0 {
                    bounds.0 = bounds.0.min(x);
                    bounds.1 = bounds.1.min(y);
                    bounds.2 = bounds.2.max(x + 1);
                    bounds.3 = bounds.3.max(y + 1);
                }
            }
        }
        (bounds.0 < bounds.2).then_some(bounds)
    }

    #[test]
    fn unicode_and_explicit_lines_have_distinct_coverage() {
        let plain = fill(FONT, 0, "Signal", 320, 100, 24.0).unwrap();
        let unicode = fill(FONT, 0, "Café a\u{301} سلام", 320, 100, 24.0).unwrap();
        assert_ne!(plain, unicode);
        assert!(unicode.iter().any(|a| *a > 0 && *a < 255));
        assert!(unicode[60 * 320..].iter().all(|a| *a == 0));
        let lines = fill(FONT, 0, "Signal\nBeacon", 320, 100, 24.0).unwrap();
        assert!(plain[32 * 320..].iter().all(|a| *a == 0));
        assert!(lines[32 * 320..64 * 320].iter().any(|a| *a != 0));
        assert_eq!(lines.len(), 32000);
    }

    #[test]
    fn placement_follows_alignment_without_relaxing_overflow() {
        let cases = [
            (HAlign::Left, VAlign::Top),
            (HAlign::Center, VAlign::Middle),
            (HAlign::Right, VAlign::Bottom),
        ];
        for (align, valign) in cases {
            let style = TextStyle {
                align,
                valign,
                ..TextStyle::default()
            };
            let coverage = glyph_coverage(FONT, 0, "Signal", 320, 100, 24.0, &style).unwrap();
            let (width, height) = (320usize, 100usize);
            let (left, top, right, bottom) = bbox(&coverage, width).unwrap();
            match (align, valign) {
                // Top-aligned ink starts after the ascent-to-cap-height gap.
                (HAlign::Left, VAlign::Top) => assert!(left <= 2 && top <= 8, "{left} {top}"),
                (HAlign::Center, VAlign::Middle) => assert!(
                    left.abs_diff(width - 1 - (right - 1)) <= 3
                        && top.abs_diff(height - 1 - (bottom - 1)) <= 12,
                    "{left} {top}"
                ),
                (HAlign::Right, VAlign::Bottom) => {
                    assert!(
                        right >= width - 2 && bottom >= height - 8,
                        "{right} {bottom}"
                    )
                }
                _ => {}
            }
            if (align, valign) != (HAlign::Left, VAlign::Top) {
                assert_ne!(coverage, fill(FONT, 0, "Signal", 320, 100, 24.0).unwrap());
            }
        }
        // No placement hides overflow, and centering still gives "j" its
        // negative left bearing room.
        let centered = TextStyle::default();
        assert!(glyph_coverage(FONT, 0, "j", 320, 100, 48.0, &centered).is_ok());
        let mut overflow = centered;
        overflow.valign = VAlign::Top;
        assert_eq!(
            glyph_coverage(FONT, 0, "Signal\nBeacon", 320, 40, 24.0, &overflow),
            Err(TextError::Overflow)
        );
        assert_eq!(
            glyph_coverage(FONT, 0, "Signal", 10, 100, 24.0, &overflow),
            Err(TextError::Overflow)
        );
    }

    #[test]
    fn rejects_missing_glyph_and_all_overflow_directions() {
        assert_eq!(
            fill(FONT, 0, "\u{10ffff}", 320, 100, 24.0),
            Err(TextError::MissingGlyph)
        );
        assert_eq!(
            fill(FONT, 0, "Signal", 10, 100, 24.0),
            Err(TextError::Overflow)
        );
        assert_eq!(
            fill(FONT, 0, "Signal\nBeacon", 320, 40, 24.0),
            Err(TextError::Overflow)
        );
        // A negative left bearing clips at the area edge like the fake-italic
        // shear; the advance-fit authority above still rejects oversized text.
        assert!(fill(FONT, 0, "j", 320, 100, 48.0).is_ok());
    }

    #[test]
    fn rejects_input_bounds_and_bad_collections() {
        for (width, height, size) in [
            (0, 100, 24.0),
            (4097, 1, 24.0),
            (1, 1, f32::NAN),
            (1, 1, 513.0),
        ] {
            assert_eq!(
                fill(FONT, 0, "A", width, height, size),
                Err(TextError::Bounds)
            );
        }
        assert_eq!(
            fill(FONT, 0, &"A".repeat(4097), 320, 100, 24.0),
            Err(TextError::Bounds)
        );
        assert_eq!(
            fill(FONT, 0, &"\n".repeat(32), 320, 100, 1.0),
            Err(TextError::Bounds)
        );
        assert_eq!(
            fill(&vec![0; MAX_FONT_FILE + 1], 0, "A", 320, 100, 24.0),
            Err(TextError::Bounds)
        );
        assert_eq!(
            fill(b"ttcf\0\0\0\0\xff\xff\xff\xff", 0, "A", 320, 100, 24.0),
            Err(TextError::InvalidFont)
        );
        assert_eq!(
            fill(b"OTTOgarbage", 0, "A", 320, 100, 24.0),
            Err(TextError::InvalidFont)
        );
        // A face index the font does not have is refused, not wrapped around.
        assert_eq!(
            fill(FONT, 1, "A", 320, 100, 24.0),
            Err(TextError::InvalidFont)
        );
        // A bounded collection renders both faces, even with duplicate names.
        let ttc = collection();
        for index in 0..2 {
            assert!(
                fill(&ttc, index, "Signal", 320, 100, 24.0).is_ok(),
                "collection face {index}"
            );
        }
        assert_eq!(
            fill(&ttc, 2, "A", 320, 100, 24.0),
            Err(TextError::InvalidFont)
        );
        let mut too_many = ttc.clone();
        too_many[8..12].copy_from_slice(&65u32.to_be_bytes());
        assert_eq!(
            fill(&too_many, 0, "A", 320, 100, 24.0),
            Err(TextError::InvalidFont)
        );
    }

    #[test]
    fn layers_follow_the_cue_style_and_effect_geometry() {
        let extent = Extent {
            width: 1280,
            height: 720,
        };
        let plain = layers(&styled("Signal", TextStyle::default(), extent, 96)).unwrap();
        assert_eq!(plain.fill.len(), 1280 * 720);
        assert!(plain.outline.iter().all(|a| *a == 0));
        assert!(plain.shadow.iter().all(|a| *a == 0));
        let fill_box = bbox(&plain.fill, 1280).unwrap();
        // Default style centers inside the 32 px inset.
        assert!(fill_box.0.abs_diff(1280 - 1 - (fill_box.2 - 1)) <= 4);
        assert!(fill_box.1 > 32 && fill_box.3 < 720 - 32);

        // Outline: the dilated fill grows by the scaled radius, inward and out.
        let outlined = layers(&styled(
            "Signal",
            TextStyle {
                outline: Some(OutlineStyle {
                    color: [0, 0, 0],
                    size: 7,
                    opacity: 100,
                }),
                ..TextStyle::default()
            },
            extent,
            96,
        ))
        .unwrap();
        // 7 reference points at 720 lines: 4.67 px radius.
        let outline_box = bbox(&outlined.outline, 1280).unwrap();
        let growth = outline_box.2 - outline_box.0 - (fill_box.2 - fill_box.0);
        assert!((7..=11).contains(&growth), "outline grew {growth}");
        assert!(outline_box.0 < fill_box.0 && outline_box.2 > fill_box.2);
        assert!(outline_box.1 < fill_box.1 && outline_box.3 > fill_box.3);
        // Outline covers the fill and never exceeds the canvas.
        for index in 0..plain.fill.len() {
            assert!(outlined.fill[index] >= plain.fill[index]);
            assert!(outlined.outline[index] >= outlined.fill[index]);
        }

        // Shadow at 315 degrees offsets down-right, blurred.
        let shadowed = layers(&styled(
            "Signal",
            TextStyle {
                shadow: Some(ShadowStyle {
                    color: [0, 0, 0],
                    angle: 315,
                    offset: 18,
                    blur: 9,
                    opacity: 90,
                }),
                ..TextStyle::default()
            },
            extent,
            96,
        ))
        .unwrap();
        let shadow_box = bbox(&shadowed.shadow, 1280).unwrap();
        // 18 reference points at 720 lines: 12 px on both axes after cos/sin.
        let (dx, dy) = (
            shadow_box.0 as i64 - fill_box.0 as i64,
            shadow_box.1 as i64 - fill_box.1 as i64,
        );
        assert!((7..=14).contains(&dx.abs()), "dx {dx}");
        assert!((7..=14).contains(&dy.abs()), "dy {dy}");
        assert!(shadow_box.2 > fill_box.2 + 7, "spread right");
        assert!(shadow_box.3 > fill_box.3 + 7, "spread down");
        assert!(
            shadowed.shadow.iter().any(|a| *a > 0 && *a < 255),
            "blurred"
        );
        // Zero degrees shift right only; the vertical edges blur by reach.
        let flat = layers(&styled(
            "Signal",
            TextStyle {
                shadow: Some(ShadowStyle {
                    color: [0, 0, 0],
                    angle: 0,
                    offset: 18,
                    blur: 0,
                    opacity: 100,
                }),
                ..TextStyle::default()
            },
            extent,
            96,
        ))
        .unwrap();
        let flat_box = bbox(&flat.shadow, 1280).unwrap();
        assert!(
            (flat_box.1 as i64 - fill_box.1 as i64).abs() <= 1
                && (flat_box.3 as i64 - fill_box.3 as i64).abs() <= 1,
            "no vertical shift"
        );
        assert!(
            (12..=14).contains(&(flat_box.0 as i64 - fill_box.0 as i64).abs()),
            "horizontal shift"
        );

        // A shadow larger than the inset is clipped at the canvas edge.
        let mut clipped = TextStyle {
            align: HAlign::Left,
            valign: VAlign::Top,
            shadow: Some(ShadowStyle {
                color: [0, 0, 0],
                angle: 135,
                offset: 100,
                blur: 50,
                opacity: 80,
            }),
            ..TextStyle::default()
        };
        clipped.align = HAlign::Left;
        let near_edge = layers(&styled("Signal", clipped, extent, 40)).unwrap();
        assert_eq!(near_edge.shadow.len(), 1280 * 720);
        assert!(bbox(&near_edge.shadow, 1280).is_some());
    }

    #[test]
    fn synthetic_bold_italic_and_underline_change_the_fill() {
        let extent = Extent {
            width: 1280,
            height: 720,
        };
        let size = 96;
        let plain = layers(&styled("Signal Hero", TextStyle::default(), extent, size)).unwrap();
        let box_of = |layers: &Layers| bbox(&layers.fill, 1280).unwrap();
        let plain_box = box_of(&plain);

        let bold = layers(&styled(
            "Signal Hero",
            TextStyle {
                synth_bold: true,
                ..TextStyle::default()
            },
            extent,
            size,
        ))
        .unwrap();
        let bold_box = box_of(&bold);
        let radius = f32::from(size) / 48.;
        let grew = bold_box.2 - bold_box.0 - (plain_box.2 - plain_box.0);
        assert!(
            (grew as f32) >= radius && grew <= radius.ceil() as usize * 2 + 2,
            "bold grew {grew} at radius {radius}"
        );

        let italic = layers(&styled(
            "Signal Hero",
            TextStyle {
                italic: true,
                synth_italic: true,
                ..TextStyle::default()
            },
            extent,
            size,
        ))
        .unwrap();
        let italic_box = box_of(&italic);
        assert!(italic_box.2 > plain_box.2, "shear reaches right");
        assert!(
            italic_box.0 <= plain_box.0 + 2,
            "shear keeps the origin: {} vs {}",
            italic_box.0,
            plain_box.0
        );

        // A descender like "g" reaches below the underline bar, so the box
        // only grows for text without descenders.
        let plain_caps =
            box_of(&layers(&styled("BEACON", TextStyle::default(), extent, size)).unwrap());
        let underlined = layers(&styled(
            "BEACON",
            TextStyle {
                underline: true,
                ..TextStyle::default()
            },
            extent,
            size,
        ))
        .unwrap();
        let underline_box = box_of(&underlined);
        let added = underline_box.3 - plain_caps.3;
        assert!((1..=12).contains(&added), "underline added {added} rows");
        // The underline bar spans the line width, below the glyph ink.
        let bottom_row = &underlined.fill[(underline_box.3 - 1) * 1280..underline_box.3 * 1280];
        let bar = bottom_row.iter().filter(|a| **a > 0).count();
        assert!(bar >= (underline_box.2 - underline_box.0) * 9 / 10);
        assert!(underline_box.1 == plain_caps.1, "underline is below only");
    }

    #[test]
    fn a_slide_cue_renders_its_whole_resolved_format() {
        let format = SlideFormat {
            bold: Some(true),
            italic: Some(true),
            underline: Some(true),
            color: Some([255, 128, 0]),
            align: Some(HAlign::Right),
            valign: Some(VAlign::Bottom),
            outline: Some(Outline {
                enabled: true,
                color: [0, 0, 0],
                size: 7,
                opacity: 100,
            }),
            shadow: Some(Shadow {
                enabled: true,
                color: [0, 0, 0],
                angle: 315,
                offset: 18,
                blur: 9,
                opacity: 90,
            }),
            ..Default::default()
        };
        let slide = sela::slides::Slide {
            label: "Verse".into(),
            text: "Original refrain".into(),
            format: format.clone(),
            background: None,
        };
        let resolved = sela::fonts::Resolved::bundled(&format);
        let extent = Extent {
            width: 1920,
            height: 1080,
        };
        let version = ContentVersion { id: 9, revision: 1 };
        let cue = sela::slides::cue(version, &slide, &resolved, extent, CAPS, None).unwrap();
        let layers = layers(&cue).unwrap();
        let text = cue.text().unwrap();
        assert_eq!(text.style().color, [255, 128, 0]);
        assert_eq!(text.style().align, HAlign::Right);
        assert!(text.style().underline && text.style().synth_italic);
        // Right/bottom placement plus outline and shadow toward the inset;
        // the bottom keeps the inset plus the descent gap below the ink.
        let fill_box = bbox(&layers.fill, 1920).unwrap();
        let outline_box = bbox(&layers.outline, 1920).unwrap();
        let shadow_box = bbox(&layers.shadow, 1920).unwrap();
        assert!(fill_box.2 > 1920 - 40, "right aligned: {fill_box:?}");
        assert!(fill_box.3 > 1080 - 60, "bottom aligned: {fill_box:?}");
        assert!(outline_box.0 < fill_box.0 && outline_box.3 > fill_box.3);
        assert!(shadow_box.3 > outline_box.3 && shadow_box.2 > outline_box.2);
    }
}
