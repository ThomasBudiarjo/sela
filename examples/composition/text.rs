//! Explicit-font CPU diagnostic. Run only in preparation, never a UI/frame callback.
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextError {
    InvalidFont,
    MissingGlyph,
    Overflow,
    Bounds,
}

/// White-text alpha coverage, row-major, with no margin or automatic wrapping.
pub fn raster(
    font: &[u8],
    text: &str,
    width: u32,
    height: u32,
    font_size: f32,
) -> Result<Vec<u8>, TextError> {
    if font.len() > 2 * 1024 * 1024
        || text.len() > 4096
        || text.split('\n').count() > 32
        || !font_size.is_finite()
        || !(1.0..=96.0).contains(&font_size)
        || width == 0
        || height == 0
        || width > 4096
        || height > 4096
    {
        return Err(TextError::Bounds);
    }
    let len = (width as usize)
        .checked_mul(height as usize)
        .filter(|len| *len <= 16 * 1024 * 1024)
        .ok_or(TextError::Bounds)?;
    // Reject collections BEFORE fontdb's collection-sized face allocation.
    if !matches!(font.get(..4), Some(b"\0\x01\0\0" | b"OTTO" | b"true"))
        || ttf_parser::Face::parse(font, 0).is_err()
    {
        return Err(TextError::InvalidFont);
    }
    let mut db = cosmic_text::fontdb::Database::new();
    db.load_font_data(font.to_vec());
    let mut faces = db.faces();
    let face = faces.next().ok_or(TextError::InvalidFont)?;
    let id = face.id;
    let weight = face.weight;
    let family = face
        .families
        .first()
        .ok_or(TextError::InvalidFont)?
        .0
        .clone();
    if faces.next().is_some() {
        return Err(TextError::InvalidFont);
    }
    drop(faces);
    let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
    fonts.get_font(id, weight).ok_or(TextError::InvalidFont)?;
    let line_height = font_size * 1.3;
    let mut buffer = Buffer::new_empty(Metrics::new(font_size, line_height));
    buffer.set_size(Some(width as f32), Some(height as f32));
    buffer.set_wrap(Wrap::None);
    buffer.set_text(
        text,
        &Attrs::new().family(Family::Name(&family)).weight(weight),
        Shaping::Advanced,
        None,
    );
    if buffer.lines.len() > 32 {
        return Err(TextError::Bounds);
    }
    buffer.shape_until_scroll(&mut fonts, false);
    // layout_runs is viewport-filtered; validate every logical line first.
    let mut top = 0.0;
    for i in 0..buffer.lines.len() {
        for line in buffer
            .line_layout(&mut fonts, i)
            .ok_or(TextError::InvalidFont)?
        {
            for glyph in &line.glyphs {
                if glyph.glyph_id == 0 || glyph.font_id != id {
                    return Err(TextError::MissingGlyph);
                }
            }
            let h = line.line_height_opt.unwrap_or(line_height);
            let ink_height = line.max_ascent + line.max_descent;
            if !line.w.is_finite()
                || line.w > width as f32
                || top + h > height as f32
                || ink_height > h
            {
                return Err(TextError::Overflow);
            }
            top += h;
        }
    }
    let mut alpha = vec![0; len];
    let mut overflow = false;
    buffer.draw(
        &mut fonts,
        &mut SwashCache::new(),
        Color::rgb(255, 255, 255),
        |x, y, w, h, color| {
            let a = u32::from(color.a());
            if a == 0 {
                return;
            }
            let right = i64::from(x) + i64::from(w);
            let bottom = i64::from(y) + i64::from(h);
            if x < 0 || y < 0 || right > i64::from(width) || bottom > i64::from(height) {
                overflow = true;
                return;
            }
            for row in y as usize..bottom as usize {
                for col in x as usize..right as usize {
                    let dst = &mut alpha[row * width as usize + col];
                    *dst = (a + (u32::from(*dst) * (255 - a) + 127) / 255) as u8;
                }
            }
        },
    );
    if overflow {
        Err(TextError::Overflow)
    } else {
        Ok(alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FONT: &[u8] = include_bytes!("../../tests/fixtures/DejaVuSans.ttf");

    #[test]
    fn unicode_and_explicit_lines_have_distinct_coverage() {
        let plain = raster(FONT, "Signal", 320, 100, 24.0).unwrap();
        let unicode = raster(FONT, "Café a\u{301} سلام", 320, 100, 24.0).unwrap();
        assert_ne!(plain, unicode);
        assert!(unicode.iter().any(|a| *a > 0 && *a < 255));
        assert!(unicode[60 * 320..].iter().all(|a| *a == 0));
        let lines = raster(FONT, "Signal\nBeacon", 320, 100, 24.0).unwrap();
        assert!(plain[32 * 320..].iter().all(|a| *a == 0));
        assert!(lines[32 * 320..64 * 320].iter().any(|a| *a != 0));
        assert_eq!(lines.len(), 32000);
    }

    #[test]
    fn rejects_missing_glyph_and_all_overflow_directions() {
        assert_eq!(
            raster(FONT, "\u{10ffff}", 320, 100, 24.0),
            Err(TextError::MissingGlyph)
        );
        assert_eq!(
            raster(FONT, "Signal", 10, 100, 24.0),
            Err(TextError::Overflow)
        );
        assert_eq!(
            raster(FONT, "Signal\nBeacon", 320, 40, 24.0),
            Err(TextError::Overflow)
        );
        // A negative left bearing is ink overflow even when the advance fits.
        assert_eq!(raster(FONT, "j", 320, 100, 48.0), Err(TextError::Overflow));
    }

    #[test]
    fn rejects_input_bounds_and_collections() {
        for (w, h, size) in [
            (0, 100, 24.0),
            (4097, 1, 24.0),
            (1, 1, f32::NAN),
            (1, 1, 97.0),
        ] {
            assert_eq!(raster(FONT, "A", w, h, size), Err(TextError::Bounds));
        }
        assert_eq!(
            raster(FONT, &"A".repeat(4097), 320, 100, 24.0),
            Err(TextError::Bounds)
        );
        assert_eq!(
            raster(FONT, &"\n".repeat(32), 320, 100, 1.0),
            Err(TextError::Bounds)
        );
        assert_eq!(
            raster(&vec![0; 2 * 1024 * 1024 + 1], "A", 320, 100, 24.0),
            Err(TextError::Bounds)
        );
        assert_eq!(
            raster(b"ttcf\0\0\0\0\xff\xff\xff\xff", "A", 320, 100, 24.0),
            Err(TextError::InvalidFont)
        );
        assert_eq!(
            raster(b"OTTOgarbage", "A", 320, 100, 24.0),
            Err(TextError::InvalidFont)
        );
    }
}
