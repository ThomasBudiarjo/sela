//! Provisional song-to-slide projection and text cue construction. No GPUI types.
//! One slide per stored section, in stored order. Arrangement selection,
//! pagination, themes and reference-qualified fitting are not implemented.
use crate::{
    scene::{
        ContentVersion, Extent, PrepareError, PreparedBackground, PreparedCue, RendererCapabilities,
    },
    storage::{Song, Version},
};

/// Bundled DejaVu Sans (see `tests/fixtures/DejaVuSans.LICENSE`). Provisional
/// until a theme/font ticket chooses product typography.
pub const FONT: &[u8] = include_bytes!("../tests/fixtures/DejaVuSans.ttf");
const FONT_VERSION: ContentVersion = ContentVersion {
    id: 0x53454c41_44656a61_56755361_6e730001,
    revision: 1,
};
/// Renderer text inset on each side, matching the audience preparer.
const INSET: u32 = 32;
const BACKGROUND: [u8; 4] = [0, 0, 0, 255];
const SLIDE_BITS: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slide {
    pub label: String,
    pub text: String,
}

pub fn slides(song: &Song) -> Vec<Slide> {
    song.sections
        .iter()
        .map(|section| Slide {
            label: section.label.clone(),
            text: section
                .lyrics
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
                .trim_matches('\n')
                .to_owned(),
        })
        .collect()
}

/// Identity of slide `index` of one immutable song revision.
pub fn slide_version(song: Version, index: usize) -> Option<ContentVersion> {
    let revision = u64::try_from(song.revision).ok().filter(|r| *r > 0)?;
    let index = u64::try_from(index).ok().filter(|i| *i < 1 << SLIDE_BITS)?;
    Some(ContentVersion {
        id: u128::from_le_bytes(song.id.0),
        revision: revision.checked_mul(1 << SLIDE_BITS)? | index,
    })
}

/// Inverse of `slide_version` for the same song revision.
pub fn slide_index(song: Version, version: ContentVersion) -> Option<usize> {
    let index = (version.revision & ((1 << SLIDE_BITS) - 1)) as usize;
    (slide_version(song, index)? == version).then_some(index)
}

/// White centered text on black, sized so every line fits the inset area.
pub fn cue(
    version: ContentVersion,
    slide: &Slide,
    extent: Extent,
    caps: RendererCapabilities,
) -> Result<PreparedCue, PrepareError> {
    let text = if slide.text.is_empty() {
        None
    } else {
        let size = font_size(&slide.text, extent).ok_or(PrepareError::InvalidScene)?;
        Some((slide.text.clone(), FONT_VERSION, FONT.into(), size))
    };
    PreparedCue::from_owned(
        version,
        extent,
        PreparedBackground::Color(BACKGROUND),
        text,
        caps,
    )
}

// Audience text preparer bounds (`audience::text`); a cue outside them would
// only be rejected after delivery.
const MAX_FONT_SIZE: u16 = 96;
const MAX_LINES: usize = 32;
const MAX_TEXT_BYTES: usize = 4096;
const MAX_TEXT_AREA: u32 = 4096;
const LINE_HEIGHT: f32 = 1.3;

/// Fits from the bundled font's unshaped advances with a 5% margin for
/// shaping differences; `None` when the renderer would reject the text,
/// including a glyph the font lacks.
fn font_size(text: &str, extent: Extent) -> Option<u16> {
    if text.len() > MAX_TEXT_BYTES || text.split('\n').count() > MAX_LINES {
        return None;
    }
    let area = |side: u32| {
        side.checked_sub(2 * INSET)
            .filter(|s| (1..=MAX_TEXT_AREA).contains(s))
    };
    let width = area(extent.width)? as f32;
    let height = area(extent.height)? as f32;
    let face = ttf_parser::Face::parse(FONT, 0).ok()?;
    let em = f32::from(face.units_per_em());
    let mut widest: f32 = 0.;
    for line in text.lines() {
        let mut advance = 0u32;
        for c in line.chars() {
            let glyph = face.glyph_index(c)?;
            advance += u32::from(face.glyph_hor_advance(glyph).unwrap_or(0));
        }
        widest = widest.max(advance as f32 / em);
    }
    let lines = text.lines().count().max(1) as f32;
    let size = (height / (lines * LINE_HEIGHT))
        .min(width / (widest * 1.05).max(f32::EPSILON))
        .min(height / 6.)
        .min(f32::from(MAX_FONT_SIZE))
        .floor();
    (size >= 1.).then_some(size as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        arrangement::SectionId,
        storage::{Id, Section},
    };

    fn song(sections: &[(&str, &str)]) -> Song {
        Song {
            title: "Signal Hymn".into(),
            authors: String::new(),
            copyright: String::new(),
            license: String::new(),
            variants: Vec::new(),
            sections: sections
                .iter()
                .map(|(label, lyrics)| Section {
                    id: SectionId::allocate(),
                    label: (*label).into(),
                    lyrics: (*lyrics).into(),
                })
                .collect(),
        }
    }
    const CAPS: RendererCapabilities = RendererCapabilities {
        max_texture_dimension: 4096,
    };
    const EXTENT: Extent = Extent {
        width: 1920,
        height: 1080,
    };

    #[test]
    fn sections_become_trimmed_slides_in_order() {
        let slides = slides(&song(&[
            ("Verse 1", "Line one  \r\nLine two\r\n\r\n"),
            ("Chorus", ""),
        ]));
        assert_eq!(
            slides,
            [
                Slide {
                    label: "Verse 1".into(),
                    text: "Line one\nLine two".into()
                },
                Slide {
                    label: "Chorus".into(),
                    text: String::new()
                }
            ]
        );
    }

    #[test]
    fn slide_versions_round_trip_and_reject_other_revisions() {
        let song = Version {
            id: Id([7; 16]),
            revision: 3,
        };
        let version = slide_version(song, 5).unwrap();
        assert_eq!(slide_index(song, version), Some(5));
        assert_ne!(slide_version(song, 4), Some(version));
        let other = Version {
            revision: 4,
            ..song
        };
        assert_eq!(slide_index(other, version), None);
        assert_eq!(slide_version(song, 1 << 16), None);
        assert_eq!(
            slide_version(
                Version {
                    revision: 0,
                    ..song
                },
                0
            ),
            None
        );
    }

    #[test]
    fn cue_carries_text_font_and_extent() {
        let slide = Slide {
            label: "Verse".into(),
            text: "Original test line\nSecond line".into(),
        };
        let version = ContentVersion { id: 9, revision: 1 };
        let cue = cue(version, &slide, EXTENT, CAPS).unwrap();
        assert_eq!(cue.extent(), EXTENT);
        assert_eq!(cue.version(), version);
        let text = cue.text().unwrap();
        assert_eq!(text.content(), slide.text);
        assert_eq!(text.font(), FONT);
        assert!(matches!(
            cue.background(),
            PreparedBackground::Color([0, 0, 0, 255])
        ));
    }

    #[test]
    fn font_size_fits_lines_and_width() {
        let small = font_size("one line", EXTENT).unwrap();
        assert!(f32::from(small) <= (1080. - 64.) / 6.);
        let large = Extent {
            width: 2560,
            height: 1600,
        };
        assert_eq!(font_size("one line", large), Some(MAX_FONT_SIZE));
        let narrow = font_size(&"i".repeat(60), EXTENT).unwrap();
        let wide = font_size(&"W".repeat(60), EXTENT).unwrap();
        assert!(wide < narrow, "{wide} {narrow}");
        let many = vec!["line"; 32].join("\n");
        let fitted = font_size(&many, EXTENT).unwrap();
        assert!(f32::from(fitted) * LINE_HEIGHT * 32. <= 1080. - 64.);
        assert_eq!(font_size(&vec!["line"; 33].join("\n"), EXTENT), None);
        assert_eq!(font_size(&"x".repeat(4097), EXTENT), None);
        assert_eq!(font_size("\u{4e2d}", EXTENT), None, "no glyph");
        assert_eq!(
            font_size(
                "x",
                Extent {
                    width: 4161,
                    height: 1080
                }
            ),
            None
        );
        assert_eq!(
            font_size(
                "x",
                Extent {
                    width: 64,
                    height: 400
                }
            ),
            None
        );
    }

    #[test]
    fn empty_slide_is_background_only_and_tiny_surface_is_rejected() {
        let blank = Slide {
            label: "Blank".into(),
            text: String::new(),
        };
        let version = ContentVersion { id: 9, revision: 2 };
        assert!(cue(version, &blank, EXTENT, CAPS).unwrap().text().is_none());
        let words = Slide {
            label: "Verse".into(),
            text: "words".into(),
        };
        let tiny = Extent {
            width: 60,
            height: 60,
        };
        assert_eq!(
            cue(version, &words, tiny, CAPS).err(),
            Some(PrepareError::InvalidScene)
        );
    }
}
