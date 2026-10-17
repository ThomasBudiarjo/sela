//! Provisional song-to-slide projection and text cue construction. No GPUI types.
//! One slide per section occurrence of the chosen arrangement (default: the
//! first), else per stored section. A section whose text overflows the
//! minimum font size becomes several slides (`paginate`). Themes are not
//! implemented.
use crate::{
    arrangement::{Variant, VariantId},
    background::{self, Background},
    fonts::Resolved,
    format::{Align, Size, SlideFormat, VAlign},
    scene::{
        ContentVersion, Extent, OutlineStyle, OwnedText, PrepareError, PreparedBackground,
        PreparedCue, RendererCapabilities, ShadowStyle, TextStyle,
    },
    storage::{Section, Song, Version},
};

/// Reference canvas for point-valued formats (fixed size, outline size, shadow
/// offset and blur): values are defined at 1080 lines and scale with the
/// output height (Sela's rule, not observed EasyWorship behavior).
pub const REFERENCE_HEIGHT: u32 = 1080;
/// Renderer text inset on each side, matching the audience preparer.
const INSET: u32 = 32;
const SLIDE_BITS: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slide {
    pub label: String,
    pub text: String,
    pub format: SlideFormat,
    /// Resolved background: the section's own, else the song master; `None`
    /// is black.
    pub background: Option<Background>,
}

/// Text sizing across one item's slides. Auto slides are always resized to fit
/// ("Resize text to fit element"); `Normalized` also uses the smallest fitted
/// size for every Auto slide. Fixed-size slides keep their size and never
/// shrink the others. EasyWorship's default is unobserved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sizing {
    #[default]
    PerSlide,
    Normalized,
}

/// One audience slide of a section that was split because its text does not
/// fit at the minimum font size: page `number` of `of` (both 1-based).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub number: u16,
    pub of: u16,
}

/// A song's audience slides plus, per slide, its page when the section was
/// split. Both vectors always have the same length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projection {
    pub slides: Vec<Slide>,
    pub pages: Vec<Option<Page>>,
}

/// Minimum font size, in 1080-line reference pixels, a paginated slide may
/// need. Pagination uses the reference canvas rather than the output size, so
/// breaks are identical for every output and in the editor preview.
const MIN_FONT: f32 = 48.;
/// Lines that fit the reference text area at `MIN_FONT`.
pub const PAGE_LINES: usize = {
    let area = (REFERENCE_HEIGHT - 2 * INSET) as f32;
    (area / (MIN_FONT * LINE_HEIGHT)) as usize
};
/// Per-page byte budget, under the audience preparer's text limit.
const PAGE_BYTES: usize = MAX_TEXT_BYTES - 96;

/// Splits slide text that does not fit at the minimum font size into pages:
/// whole lines, balanced so no page is a one-line orphan. A single line over
/// the byte budget is cut at grapheme cluster boundaries, so a base letter
/// never loses its combining marks. Pure and deterministic: the same text
/// always gives the same pages.
pub fn paginate(text: &str) -> Vec<String> {
    use unicode_segmentation::UnicodeSegmentation;
    let mut lines: Vec<&str> = Vec::new();
    for line in text.split('\n') {
        if line.len() <= PAGE_BYTES {
            lines.push(line);
            continue;
        }
        let mut start = 0;
        let mut end = 0;
        for (offset, cluster) in line.grapheme_indices(true) {
            if offset + cluster.len() - start > PAGE_BYTES && end > start {
                lines.push(&line[start..end]);
                start = end;
            }
            end = offset + cluster.len();
        }
        lines.push(&line[start..]);
    }
    let mut pages = lines.len().div_ceil(PAGE_LINES).max(1);
    loop {
        let (base, extra) = (lines.len() / pages, lines.len() % pages);
        let mut out = Vec::with_capacity(pages);
        let mut at = 0;
        for page in 0..pages {
            let take = base + usize::from(page < extra);
            out.push(lines[at..at + take].join("\n"));
            at += take;
        }
        if pages >= lines.len().max(1) || out.iter().all(|p| p.len() <= PAGE_BYTES) {
            return out
                .into_iter()
                .map(|page| page.trim_matches('\n').to_owned())
                .collect();
        }
        pages += 1;
    }
}

/// The arrangement `chosen` names when the song has it, else the first one.
pub fn variant_of(song: &Song, chosen: Option<VariantId>) -> Option<&Variant> {
    chosen
        .and_then(|id| song.variants.iter().find(|v| v.id == id))
        .or_else(|| song.variants.first())
}

/// The slides of the first arrangement.
pub fn slides(song: &Song) -> Vec<Slide> {
    project(song, None).slides
}

/// The chosen arrangement's order if it has occurrences and all of them
/// resolve; otherwise stored section order, so no lyrics are hidden. Each
/// section that overflows the minimum font size becomes consecutive pages.
pub fn project(song: &Song, chosen: Option<VariantId>) -> Projection {
    let ordered = variant_of(song, chosen).and_then(|variant| {
        variant
            .occurrences
            .iter()
            .map(|o| song.sections.iter().find(|s| s.id == o.section))
            .collect::<Option<Vec<_>>>()
            .filter(|sections| !sections.is_empty())
    });
    let sections = ordered.unwrap_or_else(|| song.sections.iter().collect());
    let mut out = Projection {
        slides: Vec::new(),
        pages: Vec::new(),
    };
    for section in sections {
        let slide = section_slide(section, song.master.as_ref());
        let split = paginate(&slide.text);
        if split.len() < 2 {
            out.slides.push(slide);
            out.pages.push(None);
            continue;
        }
        let of = u16::try_from(split.len()).unwrap_or(u16::MAX);
        for (index, text) in split.into_iter().enumerate() {
            out.slides.push(Slide {
                text,
                ..slide.clone()
            });
            out.pages.push(Some(Page {
                number: u16::try_from(index + 1).unwrap_or(u16::MAX),
                of,
            }));
        }
    }
    out
}

/// How many audience slides the section's text becomes.
pub fn page_count(slide_text: &str) -> usize {
    paginate(slide_text).len()
}

/// One section's slide: trailing spaces and surrounding blank lines dropped.
pub fn section_slide(section: &Section, master: Option<&Background>) -> Slide {
    Slide {
        background: background::resolve(section.background.as_ref(), master).cloned(),
        label: section.label.clone(),
        text: section
            .lyrics
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_matches('\n')
            .to_owned(),
        format: section.format.clone(),
    }
}

/// The size cap `cue` should use for Auto slides of this item, if any.
/// `resolved` must be parallel to `slides`; Fixed-size slides are excluded
/// because their size does not come from the fit.
pub fn size_cap(
    slides: &[Slide],
    resolved: &[Resolved],
    extent: Extent,
    sizing: Sizing,
) -> Option<u16> {
    match sizing {
        Sizing::PerSlide => None,
        Sizing::Normalized => slides
            .iter()
            .zip(resolved)
            .filter(|(s, _)| !s.text.is_empty() && !matches!(s.format.size, Some(Size::Fixed(_))))
            .filter_map(|(s, r)| fit_size(&s.text, r, extent))
            .min(),
    }
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

/// The slide's formatted text over a prepared background: the resolved face,
/// a fitted or fixed size, and the style `fit_size` keeps inside the inset.
/// `resolved` must come from the same `SlideFormat` (bundled resolution for
/// the default look, `fonts::Fonts` for an installed family). An image
/// background must already be fitted to `extent` (`background::fit`) so the
/// renderer composites it 1:1.
pub fn cue(
    version: ContentVersion,
    slide: &Slide,
    resolved: &Resolved,
    extent: Extent,
    caps: RendererCapabilities,
    cap: Option<u16>,
    background: PreparedBackground,
) -> Result<PreparedCue, PrepareError> {
    if let PreparedBackground::Image { extent: image, .. } = &background
        && *image != extent
    {
        return Err(PrepareError::InvalidImage);
    }
    let text = if slide.text.is_empty() {
        None
    } else {
        // A Fixed size that would not fit is refused like other unshowable
        // text; the raster still checks the shaped glyphs exactly.
        let size = match slide.format.size {
            Some(Size::Fixed(fixed)) => {
                let fits =
                    fit_size(&slide.text, resolved, extent).ok_or(PrepareError::InvalidScene)?;
                let scaled = (f32::from(fixed) * extent.height as f32 / REFERENCE_HEIGHT as f32)
                    .round()
                    .max(1.) as u16;
                (scaled <= fits)
                    .then_some(scaled)
                    .ok_or(PrepareError::InvalidScene)?
            }
            _ => {
                let size =
                    fit_size(&slide.text, resolved, extent).ok_or(PrepareError::InvalidScene)?;
                cap.map_or(size, |cap| size.min(cap.max(1)))
            }
        };
        Some(OwnedText {
            content: slide.text.clone(),
            font_version: resolved.face().version(),
            font: resolved.face().bytes_arc(),
            face_index: resolved.face().index(),
            font_size: size,
            style: style_of(&slide.format, resolved),
        })
    };
    PreparedCue::from_owned(version, extent, background, text, caps)
}

/// A color plan's opaque cue background; image plans are prepared by the
/// image worker.
pub fn color_background(rgb: [u8; 3]) -> PreparedBackground {
    PreparedBackground::Color([rgb[0], rgb[1], rgb[2], 255])
}

/// Format plus resolution folded into the cue's style. `None` format fields
/// keep today's look: white, centered both ways, no underline or effects.
fn style_of(format: &SlideFormat, resolved: &Resolved) -> TextStyle {
    TextStyle {
        color: format.color.unwrap_or([255, 255, 255]),
        align: format.align.unwrap_or(Align::Center),
        valign: format.valign.unwrap_or(VAlign::Middle),
        underline: format.underline == Some(true),
        italic: format.italic == Some(true),
        synth_bold: resolved.synth_bold(),
        synth_italic: resolved.synth_italic(),
        outline: format.outline.filter(|o| o.enabled).map(|o| OutlineStyle {
            color: o.color,
            size: o.size,
            opacity: o.opacity,
        }),
        shadow: format.shadow.filter(|s| s.enabled).map(|s| ShadowStyle {
            color: s.color,
            angle: s.angle,
            offset: s.offset,
            blur: s.blur,
            opacity: s.opacity,
        }),
    }
}

// Audience text preparer bounds (`audience::text`); a cue outside them would
// only be rejected after delivery.
const MAX_FONT_SIZE: u16 = 288;
const MAX_LINES: usize = 32;
const MAX_TEXT_BYTES: usize = 4096;
const MAX_TEXT_AREA: u32 = 4096;
const LINE_HEIGHT: f32 = 1.3;
/// Margins for effects the advances do not cover: synthetic bold dilation
/// (size/48 per side) and the fake-italic shear (tan 14 degrees of ascent).
const SYNTH_BOLD_EM: f32 = 1. / 48. * 2.;
const SYNTH_ITALIC_EM: f32 = 0.25;

/// Fits from the resolved face's unshaped advances with a 5% margin for
/// shaping differences plus synthetic-style margins; `None` when the renderer
/// would reject the text, including a glyph the font lacks.
fn fit_size(text: &str, resolved: &Resolved, extent: Extent) -> Option<u16> {
    if text.len() > MAX_TEXT_BYTES || text.split('\n').count() > MAX_LINES {
        return None;
    }
    let area = |side: u32| {
        side.checked_sub(2 * INSET)
            .filter(|s| (1..=MAX_TEXT_AREA).contains(s))
    };
    let width = area(extent.width)? as f32;
    let height = area(extent.height)? as f32;
    let face = ttf_parser::Face::parse(resolved.face().bytes(), resolved.face().index()).ok()?;
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
    let synth = if resolved.synth_bold() {
        SYNTH_BOLD_EM
    } else {
        0.
    } + if resolved.synth_italic() {
        SYNTH_ITALIC_EM
    } else {
        0.
    };
    let width_em = widest * 1.05 + synth;
    let lines = text.lines().count().max(1) as f32;
    let size = (height / (lines * LINE_HEIGHT))
        .min(width / width_em.max(f32::EPSILON))
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
        fonts,
        format::{Shadow, Size},
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
                    format: Default::default(),
                    background: None,
                })
                .collect(),
            master: None,
        }
    }
    fn black() -> PreparedBackground {
        color_background(background::BLACK)
    }
    fn resolved(slides: &[Slide]) -> Vec<Resolved> {
        slides
            .iter()
            .map(|s| Resolved::bundled(&s.format))
            .collect()
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
                    text: "Line one\nLine two".into(),
                    format: Default::default(),
                    background: None,
                },
                Slide {
                    label: "Chorus".into(),
                    text: String::new(),
                    format: Default::default(),
                    background: None,
                }
            ]
        );
    }

    #[test]
    fn slides_resolve_their_own_background_else_the_master() {
        use crate::background::{Background, tests::image};
        let mut song = song(&[("A", "one"), ("B", "two")]);
        song.sections[1].background = Some(image("own.png"));
        let backgrounds = |song: &Song| {
            slides(song)
                .into_iter()
                .map(|s| s.background)
                .collect::<Vec<_>>()
        };
        assert_eq!(backgrounds(&song), [None, Some(image("own.png"))]);
        song.master = Some(Background::color([0, 0, 255]));
        assert_eq!(
            backgrounds(&song),
            [Some(Background::color([0, 0, 255])), Some(image("own.png"))]
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
    fn cue_carries_text_font_style_and_extent() {
        let slide = Slide {
            label: "Verse".into(),
            text: "Original test line\nSecond line".into(),
            format: Default::default(),
            background: None,
        };
        let version = ContentVersion { id: 9, revision: 1 };
        let resolved = Resolved::bundled(&slide.format);
        let basic = cue(version, &slide, &resolved, EXTENT, CAPS, None, black()).unwrap();
        assert_eq!(basic.extent(), EXTENT);
        assert_eq!(basic.version(), version);
        let text = basic.text().unwrap();
        assert_eq!(text.content(), slide.text);
        assert_eq!(text.font(), fonts::BUNDLED);
        assert_eq!(text.face_index(), 0);
        assert_eq!(text.style(), TextStyle::default());
        assert!(matches!(
            basic.background(),
            PreparedBackground::Color([0, 0, 0, 255])
        ));
        // The EW-like bold default resolves to the bundled Bold face.
        let bold = SlideFormat {
            bold: Some(true),
            ..Default::default()
        };
        let bold_slide = Slide {
            format: bold.clone(),
            ..slide.clone()
        };
        let bold_cue = cue(
            version,
            &bold_slide,
            &Resolved::bundled(&bold),
            EXTENT,
            CAPS,
            None,
            black(),
        )
        .unwrap();
        assert_eq!(bold_cue.text().unwrap().font(), fonts::BUNDLED_BOLD);
        // A fitted image passes through; any other size would be rescaled
        // by the renderer, so it is refused.
        let image = |extent: Extent| PreparedBackground::Image {
            version,
            extent,
            rgba: vec![7; extent.width as usize * extent.height as usize * 4].into(),
        };
        let over = cue(
            version,
            &slide,
            &resolved,
            EXTENT,
            CAPS,
            None,
            image(EXTENT),
        )
        .unwrap();
        assert!(matches!(
            over.background(),
            PreparedBackground::Image { extent, .. } if *extent == EXTENT
        ));
        let small = Extent {
            width: 960,
            height: 540,
        };
        assert_eq!(
            cue(version, &slide, &resolved, EXTENT, CAPS, None, image(small)).err(),
            Some(PrepareError::InvalidImage)
        );
        let blue = cue(
            version,
            &slide,
            &resolved,
            EXTENT,
            CAPS,
            None,
            color_background([0, 0, 255]),
        )
        .unwrap();
        assert!(matches!(
            blue.background(),
            PreparedBackground::Color([0, 0, 255, 255])
        ));
    }

    #[test]
    fn styled_cue_resolves_the_whole_format() {
        let format = crate::format::tests::full();
        let slide = Slide {
            label: "Verse".into(),
            text: "Original refrain".into(),
            format: format.clone(),
            background: None,
        };
        let resolved = Resolved::bundled(&format);
        let version = ContentVersion { id: 9, revision: 4 };
        let styled = cue(version, &slide, &resolved, EXTENT, CAPS, None, black()).unwrap();
        let text = styled.text().unwrap();
        // The fixture asks for bold, and a real bundled Bold face exists.
        assert_eq!(text.font(), fonts::BUNDLED_BOLD);
        let style = text.style();
        assert_eq!(style.color, [255, 128, 0]);
        assert_eq!(style.align, Align::Right);
        assert_eq!(style.valign, VAlign::Bottom);
        assert!(style.underline && !style.italic && !style.synth_italic && !style.synth_bold);
        assert_eq!(
            style.outline,
            Some(OutlineStyle {
                color: [0, 0, 0],
                size: 7,
                opacity: 100,
            })
        );
        assert_eq!(
            style.shadow,
            Some(ShadowStyle {
                color: [0, 0, 0],
                angle: 315,
                offset: 18,
                blur: 9,
                opacity: 90,
            })
        );
        // Fixed 78 on the reference canvas is 78 px at 1080 lines.
        assert_eq!(text.font_size(), 78);
        // A disabled shadow stays absent even when the format carries it.
        let off = SlideFormat {
            shadow: Some(Shadow {
                enabled: false,
                ..format.shadow.unwrap()
            }),
            ..format
        };
        let off_slide = Slide {
            format: off.clone(),
            ..slide.clone()
        };
        let cue = cue(
            version,
            &off_slide,
            &Resolved::bundled(&off),
            EXTENT,
            CAPS,
            None,
            black(),
        )
        .unwrap();
        assert_eq!(cue.text().unwrap().style().shadow, None);
    }

    #[test]
    fn fixed_size_scales_with_height_and_is_refused_when_unshowable() {
        let format = SlideFormat {
            size: Some(Size::Fixed(78)),
            ..Default::default()
        };
        let slide = Slide {
            label: "Verse".into(),
            text: "A much longer line of original lyrics\nand a second one".into(),
            format: format.clone(),
            background: None,
        };
        let resolved = Resolved::bundled(&format);
        let version = ContentVersion { id: 9, revision: 5 };
        // 78 px does not fit 48 wide letters at 1080 lines: refused.
        let unshowable = Slide {
            text: "W".repeat(48),
            ..slide.clone()
        };
        assert_eq!(
            cue(version, &unshowable, &resolved, EXTENT, CAPS, None, black()).err(),
            Some(PrepareError::InvalidScene)
        );
        // Shorter text fits, and the size scales with the output height.
        let fits = Slide {
            text: "Refrain".into(),
            ..slide.clone()
        };
        for (height, expected) in [(1080, 78), (720, 52), (1600, 116)] {
            let extent = Extent {
                width: 1920,
                height,
            };
            let cue = cue(version, &fits, &resolved, extent, CAPS, None, black()).unwrap();
            assert_eq!(cue.text().unwrap().font_size(), expected, "{height}");
        }
        // The Auto fit keeps using the resolved face and the cap.
        let auto = Slide {
            format: Default::default(),
            ..slide
        };
        let auto_resolved = Resolved::bundled(&auto.format);
        let cue = cue(
            version,
            &auto,
            &auto_resolved,
            EXTENT,
            CAPS,
            Some(60),
            black(),
        )
        .unwrap();
        assert_eq!(cue.text().unwrap().font_size(), 60);
    }

    #[test]
    fn fit_size_fits_lines_width_and_synthetic_styles() {
        let plain = Resolved::bundled(&SlideFormat::default());
        let small = fit_size("one line", &plain, EXTENT).unwrap();
        assert_eq!(small, ((1080. - 64.) / 6.) as u16);
        let large = Extent {
            width: 2560,
            height: 1600,
        };
        assert_eq!(fit_size("one line", &plain, large), Some(256));
        let uhd = Extent {
            width: 3840,
            height: 2160,
        };
        assert_eq!(fit_size("one line", &plain, uhd), Some(MAX_FONT_SIZE));
        let narrow = fit_size(&"i".repeat(60), &plain, EXTENT).unwrap();
        let wide = fit_size(&"W".repeat(60), &plain, EXTENT).unwrap();
        assert!(wide < narrow, "{wide} {narrow}");
        let many = vec!["line"; 32].join("\n");
        let fitted = fit_size(&many, &plain, EXTENT).unwrap();
        assert!(f32::from(fitted) * LINE_HEIGHT * 32. <= 1080. - 64.);
        assert_eq!(fit_size(&vec!["line"; 33].join("\n"), &plain, EXTENT), None);
        assert_eq!(fit_size(&"x".repeat(4097), &plain, EXTENT), None);
        assert_eq!(fit_size("\u{4e2d}", &plain, EXTENT), None, "no glyph");
        assert_eq!(
            fit_size(
                "x",
                &plain,
                Extent {
                    width: 4161,
                    height: 1080
                }
            ),
            None
        );
        assert_eq!(
            fit_size(
                "x",
                &plain,
                Extent {
                    width: 64,
                    height: 400
                }
            ),
            None
        );
        // Synthetic styles reserve margin, so a width-bound styled fit never
        // out-sizes the plain fit. Height-capped fits meet the same cap, and
        // `bold_face` below shows a real bold face fits at the plain size.
        let styled = Resolved::bundled(&SlideFormat {
            bold: Some(true),
            italic: Some(true),
            ..Default::default()
        });
        let text = "W".repeat(60);
        let plain_fit = fit_size(&text, &plain, EXTENT).unwrap();
        let styled_fit = fit_size(&text, &styled, EXTENT).unwrap();
        assert!(styled_fit < plain_fit, "{text}");
        assert!(styled_fit > 0);
        // A bold face fits at the plain size: the margin is for synthesis.
        let bold_face = Resolved::bundled(&SlideFormat {
            bold: Some(true),
            ..Default::default()
        });
        assert_eq!(
            fit_size("Signal", &bold_face, EXTENT),
            fit_size("Signal", &plain, EXTENT)
        );
    }

    #[test]
    fn empty_slide_is_background_only_and_tiny_surface_is_rejected() {
        let blank = Slide {
            label: "Blank".into(),
            text: String::new(),
            format: Default::default(),
            background: None,
        };
        let version = ContentVersion { id: 9, revision: 2 };
        assert!(
            cue(
                version,
                &blank,
                &Resolved::bundled(&blank.format),
                EXTENT,
                CAPS,
                Some(40),
                black()
            )
            .unwrap()
            .text()
            .is_none()
        );
        let words = Slide {
            label: "Verse".into(),
            text: "words".into(),
            format: Default::default(),
            background: None,
        };
        let tiny = Extent {
            width: 60,
            height: 60,
        };
        assert_eq!(
            cue(
                version,
                &words,
                &Resolved::bundled(&words.format),
                tiny,
                CAPS,
                Some(40),
                black()
            )
            .err(),
            Some(PrepareError::InvalidScene)
        );
    }

    #[test]
    fn first_arrangement_orders_slides_and_falls_back_to_sections() {
        use crate::arrangement::{Occurrence, OccurrenceId, Variant, VariantId};
        let mut song = song(&[("Verse 1", "v1"), ("Chorus", "c"), ("Verse 2", "v2")]);
        assert_eq!(texts(&slides(&song)), ["v1", "c", "v2"]);
        let [v1, c, v2] = [0, 1, 2].map(|i| song.sections[i].id);
        let occurrence = |section| Occurrence {
            id: OccurrenceId(SectionId::allocate().0),
            section,
        };
        let variant = |name: &str, order: &[SectionId]| Variant {
            id: VariantId(SectionId::allocate().0),
            name: name.into(),
            occurrences: order.iter().copied().map(occurrence).collect(),
        };
        song.variants = vec![
            variant("Sunday", &[v1, c, v2, c, c]),
            variant("Short", &[v2]),
        ];
        let ordered = slides(&song);
        assert_eq!(texts(&ordered), ["v1", "c", "v2", "c", "c"]);
        assert_eq!(ordered[3].label, "Chorus");
        song.variants[0].occurrences.clear();
        assert_eq!(
            texts(&slides(&song)),
            ["v1", "c", "v2"],
            "empty arrangement"
        );
        song.variants[0] = variant("Broken", &[v2, SectionId::allocate()]);
        assert_eq!(
            texts(&slides(&song)),
            ["v1", "c", "v2"],
            "unresolved section"
        );
    }

    fn texts(slides: &[Slide]) -> Vec<&str> {
        slides.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn normalized_sizing_uses_the_smallest_auto_fitted_size() {
        let slides = slides(&song(&[
            ("Verse 1", "Short"),
            ("Blank", ""),
            (
                "Verse 2",
                "A much longer line of original lyrics\nand a second one",
            ),
        ]));
        let base_resolved = resolved(&slides);
        assert_eq!(
            size_cap(&slides, &base_resolved, EXTENT, Sizing::PerSlide),
            None
        );
        let cap = size_cap(&slides, &base_resolved, EXTENT, Sizing::Normalized).unwrap();
        let sizes: Vec<u16> = slides
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.text.is_empty())
            .map(|(i, s)| {
                let version = ContentVersion {
                    id: 9,
                    revision: i as u64,
                };
                let per_slide =
                    cue(version, s, &base_resolved[i], EXTENT, CAPS, None, black()).unwrap();
                let normalized = cue(
                    version,
                    s,
                    &base_resolved[i],
                    EXTENT,
                    CAPS,
                    Some(cap),
                    black(),
                )
                .unwrap();
                assert!(
                    normalized.text().unwrap().font_size() <= per_slide.text().unwrap().font_size()
                );
                normalized.text().unwrap().font_size()
            })
            .collect();
        assert_eq!(sizes, [cap, cap]);
        assert_eq!(
            cap,
            fit_size(&slides[2].text, &base_resolved[2], EXTENT).unwrap()
        );
        assert!(fit_size(&slides[0].text, &base_resolved[0], EXTENT).unwrap() > cap);
        // Slides the renderer would reject never shrink the others to nothing.
        let mut with_bad = slides.clone();
        with_bad.push(Slide {
            label: "Bad".into(),
            text: "\u{4e2d}".into(),
            format: Default::default(),
            background: None,
        });
        let bad_resolved = resolved(&with_bad);
        assert_eq!(
            size_cap(&with_bad, &bad_resolved, EXTENT, Sizing::Normalized),
            Some(cap)
        );
        assert_eq!(
            size_cap(
                &slides[1..2],
                &base_resolved[1..2],
                EXTENT,
                Sizing::Normalized
            ),
            None
        );
        // A Fixed-size slide is excluded from the cap but keeps its own size.
        let mut with_fixed = slides.clone();
        with_fixed[0].format.size = Some(Size::Fixed(20));
        let fixed_resolved = resolved(&with_fixed);
        assert_eq!(
            size_cap(&with_fixed, &fixed_resolved, EXTENT, Sizing::Normalized),
            Some(cap)
        );
        let version = ContentVersion { id: 9, revision: 1 };
        let fixed_cue = cue(
            version,
            &with_fixed[0],
            &fixed_resolved[0],
            EXTENT,
            CAPS,
            Some(cap.min(1)),
            black(),
        )
        .unwrap();
        assert_eq!(fixed_cue.text().unwrap().font_size(), 20);
    }

    fn numbered(count: usize) -> String {
        (1..=count)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn text_that_fits_is_one_page_and_overflow_splits_into_balanced_pages() {
        assert_eq!(PAGE_LINES, 16);
        assert_eq!(paginate(""), [""]);
        assert_eq!(paginate(&numbered(16)).len(), 1);
        let pages = paginate(&numbered(17));
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].lines().count(), 9);
        assert_eq!(pages[1].lines().count(), 8);
        let joined = pages.join("\n");
        assert_eq!(joined, numbered(17), "no line lost or reordered");
        assert_eq!(paginate(&numbered(33)).len(), 3);
    }

    #[test]
    fn pagination_is_deterministic_and_every_page_is_showable() {
        let text = numbered(40);
        assert_eq!(paginate(&text), paginate(&text));
        for page in paginate(&text) {
            assert!(page.lines().count() <= PAGE_LINES);
            assert!(page.len() <= MAX_TEXT_BYTES);
            assert!(page.lines().count() <= MAX_LINES);
        }
        let wide = vec!["x".repeat(600); 12].join("\n");
        let pages = paginate(&wide);
        assert!(pages.len() > 1, "byte budget forces a split");
        assert!(pages.iter().all(|p| p.len() <= PAGE_BYTES));
        assert_eq!(pages.join("\n"), wide);
    }

    #[test]
    fn a_long_line_is_cut_between_grapheme_clusters_not_inside_them() {
        let cluster = "e\u{301}\u{323}";
        let line = cluster.repeat(PAGE_BYTES);
        let pages = paginate(&line);
        assert!(pages.len() > 1);
        for page in &pages {
            assert!(page.len() <= PAGE_BYTES);
            assert!(!page.starts_with('\u{301}') && !page.starts_with('\u{323}'));
            assert_eq!(page.matches('e').count(), page.matches('\u{301}').count());
            assert_eq!(page.matches('e').count(), page.matches('\u{323}').count());
        }
        assert_eq!(pages.concat(), line);
        // CJK and emoji ZWJ sequences stay whole too.
        let family = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}";
        let pages = paginate(&family.repeat(PAGE_BYTES / 4));
        assert!(
            pages
                .iter()
                .all(|p| p.matches('\u{1f468}').count() * 2 == p.matches('\u{200d}').count())
        );
    }

    #[test]
    fn projection_follows_the_chosen_arrangement_and_marks_pages() {
        use crate::arrangement::{Occurrence, OccurrenceId, Variant, VariantId};
        let long = numbered(20);
        let mut song = song(&[
            ("Verse 1", "v1"),
            ("Chorus", long.as_str()),
            ("Verse 2", "v2"),
        ]);
        let [v1, c, v2] = [0, 1, 2].map(|i| song.sections[i].id);
        let variant = |name: &str, order: &[SectionId]| Variant {
            id: VariantId(SectionId::allocate().0),
            name: name.into(),
            occurrences: order
                .iter()
                .map(|section| Occurrence {
                    id: OccurrenceId(SectionId::allocate().0),
                    section: *section,
                })
                .collect(),
        };
        let full = variant("Full", &[v1, c, v2, c]);
        let short = variant("Short", &[v2, v1]);
        song.variants = vec![full.clone(), short.clone()];
        let default = project(&song, None);
        assert_eq!(default.slides.len(), default.pages.len());
        let labels: Vec<_> = default.slides.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Verse 1", "Chorus", "Chorus", "Verse 2", "Chorus", "Chorus"]
        );
        assert_eq!(default.pages[0], None);
        assert_eq!(default.pages[1], Some(Page { number: 1, of: 2 }));
        assert_eq!(default.pages[2], Some(Page { number: 2, of: 2 }));
        let chosen = project(&song, Some(short.id));
        assert_eq!(texts(&chosen.slides), ["v2", "v1"]);
        assert_eq!(chosen.pages, [None, None]);
        let unknown = project(&song, Some(VariantId(SectionId::allocate().0)));
        assert_eq!(unknown, default, "an unknown arrangement falls back");
        assert_eq!(slides(&song), default.slides);
    }
}
