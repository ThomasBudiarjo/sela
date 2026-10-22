//! Stage (confidence) output content: what performers see on their own
//! monitor. EasyWorship 8 calls this output "Foldback" (EW8-OBS-054/056).
//! Framework-independent: no GPUI, renderer or clock-source types.
//!
//! Privacy rule: a [`StagePanel`] travels only to a stage renderer. The
//! audience renderer refuses any cue that carries one (see
//! `scene::PreparedCue::for_stage`), so stage notes, the next slide and the stage
//! message can never be drawn on the audience output.
use crate::scene::Extent;

/// Each panel text field, the same bound as the renderer's text raster.
pub const MAX_FIELD_BYTES: usize = 4096;
/// Lines per field, the same bound as the renderer's text raster.
pub const MAX_FIELD_LINES: usize = 32;
/// The stage message is one short line typed by the operator.
pub const MAX_MESSAGE_CHARS: usize = 160;
/// Dark, low-glare stage background (Sela's choice; EW unobserved).
pub const BACKGROUND: [u8; 3] = [16, 17, 20];
/// Content id of every stage cue; the revision counts panels sent in one
/// operator run, so a stage version never collides with a slide version.
pub const STAGE_ID: u128 = u128::from_le_bytes(*b"sela-stage-panel");

/// Everything one stage frame shows except the clock, which the stage
/// renderer draws from its own wall clock so a stalled operator UI never
/// stops it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StagePanel {
    /// Item title and slide name of the current slide.
    pub current_label: String,
    /// Current slide text, as the audience sees it (without the copyright strip).
    pub current: String,
    /// "Next" line: the next slide's name, the next schedule item, or the end.
    pub next_label: String,
    /// Next slide text; empty when the next thing is not a slide of this item.
    pub next: String,
    /// Operator message to the stage only; empty when none is shown.
    pub message: String,
    /// Draw the time of day in the header.
    pub show_clock: bool,
    /// Draw the next-slide area; off gives the current slide the whole body.
    pub show_next: bool,
}

impl StagePanel {
    /// Field sizes and line counts within the renderer's text bounds, and a
    /// one-line message.
    pub fn is_valid(&self) -> bool {
        let field = |text: &str| {
            text.len() <= MAX_FIELD_BYTES && text.split('\n').count() <= MAX_FIELD_LINES
        };
        [
            &self.current_label,
            &self.current,
            &self.next_label,
            &self.next,
        ]
        .into_iter()
        .all(|text| field(text))
            && self.message.chars().count() <= MAX_MESSAGE_CHARS
            && !self.message.contains('\n')
            && !self.current_label.contains('\n')
            && !self.next_label.contains('\n')
    }
}

/// What follows the current slide, decided only from the live item's own
/// slide snapshot and the schedule order, so it is deterministic: a repeated
/// chorus is "next" exactly where the arrangement repeats it, and editing the
/// song or its arrangement after Go Live changes nothing until the item is
/// sent live again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Upcoming<'a> {
    /// The next slide of the same item (index into its slides).
    Slide { index: usize, text: &'a str },
    /// The live item's last slide is showing; the schedule continues with this
    /// item (its slides are not loaded, so only the title is known).
    Item { title: &'a str },
    /// Nothing follows: the last slide of the last item, or a library song
    /// that is not in the schedule.
    End,
}

/// `slides` are the live item's slide texts; `next_item` is the title of the
/// schedule entry after the live one, if the live item is a schedule entry
/// and one follows. An index past the end is treated as the last slide.
pub fn upcoming<'a>(slides: &[&'a str], index: usize, next_item: Option<&'a str>) -> Upcoming<'a> {
    let following = index.checked_add(1);
    match following.and_then(|at| Some((at, *slides.get(at)?))) {
        Some((index, text)) => Upcoming::Slide { index, text },
        None => match next_item {
            Some(title) => Upcoming::Item { title },
            None => Upcoming::End,
        },
    }
}

/// Trims an operator message to one line within the bound: line breaks
/// become spaces, surrounding whitespace is dropped, long text is cut at a
/// character boundary.
pub fn clean_message(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.chars().take(MAX_MESSAGE_CHARS).collect()
}

/// Clock text for a local time of day: 24-hour "HH:MM" (EW's Foldback
/// preview showed "02:35"; 12/24-hour choice and seconds are UNOBSERVED).
pub fn clock_text(hour: u32, minute: u32) -> String {
    format!("{:02}:{:02}", hour.min(23), minute.min(59))
}

/// A pixel box on the stage surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    pub fn bottom(&self) -> u32 {
        self.y + self.height
    }
    pub fn right(&self) -> u32 {
        self.x + self.width
    }
    pub fn overlaps(&self, other: &Region) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// Where each part of the panel is drawn. Sela's layout (EW's foldback layout
/// controls are UNOBSERVED beyond a clock and a "next line" strip in the
/// Options preview, EW8-OBS-056): a header with the current label on the
/// left and the clock on the right, an optional message band, the current
/// slide large, then a divider, the next label and the next slide smaller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub label: Region,
    pub clock: Option<Region>,
    pub message: Option<Region>,
    pub current: Region,
    pub divider: Option<Region>,
    pub next_label: Option<Region>,
    pub next: Option<Region>,
}

impl Layout {
    /// Every drawn region, for bounds and overlap checks.
    pub fn regions(&self) -> Vec<Region> {
        let mut all = vec![self.label, self.current];
        all.extend(self.clock);
        all.extend(self.message);
        all.extend(self.divider);
        all.extend(self.next_label);
        all.extend(self.next);
        all
    }
}

/// Smallest surface the stage layout supports; smaller surfaces are refused.
pub const MIN_EXTENT: Extent = Extent {
    width: 320,
    height: 180,
};

/// The panel's regions on `extent`, or `None` below `MIN_EXTENT`. Regions
/// never overlap and stay inside the surface.
pub fn layout(extent: Extent, panel: &StagePanel) -> Option<Layout> {
    if extent.width < MIN_EXTENT.width || extent.height < MIN_EXTENT.height {
        return None;
    }
    let (w, h) = (extent.width, extent.height);
    let share = |side: u32, part: f32| ((side as f32) * part).round() as u32;
    let margin = share(h, 0.03).max(8);
    let gap = (margin / 2).max(4);
    let inner = w - 2 * margin;
    let header = share(h, 0.09).max(16);
    let (label, clock) = if panel.show_clock {
        let clock_width = share(inner, 0.3);
        (
            Region {
                x: margin,
                y: margin,
                width: inner - clock_width - gap,
                height: header,
            },
            Some(Region {
                x: margin + inner - clock_width,
                y: margin,
                width: clock_width,
                height: header,
            }),
        )
    } else {
        (
            Region {
                x: margin,
                y: margin,
                width: inner,
                height: header,
            },
            None,
        )
    };
    let mut top = margin + header + gap;
    let message = (!panel.message.is_empty()).then(|| {
        let band = Region {
            x: margin,
            y: top,
            width: inner,
            height: share(h, 0.08).max(14),
        };
        top = band.bottom() + gap;
        band
    });
    let body = h - margin - top;
    if !panel.show_next {
        return Some(Layout {
            label,
            clock,
            message,
            current: Region {
                x: margin,
                y: top,
                width: inner,
                height: body,
            },
            divider: None,
            next_label: None,
            next: None,
        });
    }
    let current = Region {
        x: margin,
        y: top,
        width: inner,
        height: share(body, 0.58),
    };
    let rule = (h / 360).max(2);
    let divider = Region {
        x: margin,
        y: current.bottom() + gap,
        width: inner,
        height: rule,
    };
    let next_label = Region {
        x: margin,
        y: divider.bottom() + gap,
        width: inner,
        height: share(h, 0.05).max(12),
    };
    let next_top = next_label.bottom() + gap / 2;
    let next = Region {
        x: margin,
        y: next_top,
        width: inner,
        height: (h - margin).saturating_sub(next_top).max(1),
    };
    Some(Layout {
        label,
        clock,
        message,
        current,
        divider: Some(divider),
        next_label: Some(next_label),
        next: Some(next),
    })
}

/// Line height used by the renderer's text raster (multiple of the size).
pub const LINE_HEIGHT: f32 = 1.3;
const MAX_SIZE: f32 = 288.;

/// Largest font size, by the face's advance widths, at which `text` fits
/// `region` without wrapping (one line per `\n`). `None` when the face lacks
/// a glyph, the text is too long or nothing fits. The renderer still checks
/// the shaped glyphs and steps down if the estimate is too large.
pub fn fit(text: &str, font: &[u8], face_index: u32, region: Region) -> Option<u16> {
    if text.is_empty()
        || text.len() > MAX_FIELD_BYTES
        || text.split('\n').count() > MAX_FIELD_LINES
        || region.width == 0
        || region.height == 0
    {
        return None;
    }
    let face = ttf_parser::Face::parse(font, face_index).ok()?;
    let em = f32::from(face.units_per_em());
    let mut widest: f32 = 0.;
    for line in text.split('\n') {
        let mut advance = 0u32;
        for c in line.chars() {
            let glyph = face.glyph_index(c)?;
            advance += u32::from(face.glyph_hor_advance(glyph).unwrap_or(0));
        }
        widest = widest.max(advance as f32 / em);
    }
    let lines = text.split('\n').count().max(1) as f32;
    let size = (region.height as f32 / (lines * LINE_HEIGHT))
        .min(region.width as f32 / (widest * 1.05).max(f32::EPSILON))
        .min(MAX_SIZE)
        .floor();
    (size >= 4.).then_some(size as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HD: Extent = Extent {
        width: 1920,
        height: 1080,
    };

    fn panel() -> StagePanel {
        StagePanel {
            current_label: "Signal Hymn · Verse 1".into(),
            current: "First original line\nSecond original line".into(),
            next_label: "Next · Chorus".into(),
            next: "Original refrain".into(),
            message: String::new(),
            show_clock: true,
            show_next: true,
        }
    }

    #[test]
    fn next_is_the_following_slide_of_the_same_item() {
        let slides = ["Verse", "Chorus", "Verse two", "Chorus"];
        assert_eq!(
            upcoming(&slides, 0, Some("Later")),
            Upcoming::Slide {
                index: 1,
                text: "Chorus"
            }
        );
        // A repeated chorus is next exactly where the arrangement repeats it,
        // not the first slide with the same text.
        assert_eq!(
            upcoming(&slides, 2, None),
            Upcoming::Slide {
                index: 3,
                text: "Chorus"
            }
        );
    }

    #[test]
    fn last_slide_points_at_the_next_item_or_the_end() {
        let slides = ["Verse", "Chorus"];
        assert_eq!(
            upcoming(&slides, 1, Some("Closing Song")),
            Upcoming::Item {
                title: "Closing Song"
            }
        );
        assert_eq!(upcoming(&slides, 1, None), Upcoming::End);
        // An index past the end (a shrunk snapshot) never invents a slide.
        assert_eq!(upcoming(&slides, 7, None), Upcoming::End);
        // An item without slides goes straight to what follows it.
        assert_eq!(
            upcoming(&[], 0, Some("Empty next")),
            Upcoming::Item {
                title: "Empty next"
            }
        );
    }

    #[test]
    fn messages_are_one_bounded_line() {
        assert_eq!(clean_message("  Wrap up\n  now  "), "Wrap up now");
        let long = "é".repeat(400);
        let cleaned = clean_message(&long);
        assert_eq!(cleaned.chars().count(), MAX_MESSAGE_CHARS);
        assert!(
            StagePanel {
                message: cleaned,
                ..panel()
            }
            .is_valid()
        );
        assert!(
            !StagePanel {
                message: "a\nb".into(),
                ..panel()
            }
            .is_valid()
        );
        assert!(
            !StagePanel {
                current: "x\n".repeat(40),
                ..panel()
            }
            .is_valid()
        );
    }

    #[test]
    fn clock_is_zero_padded_24_hour() {
        assert_eq!(clock_text(2, 35), "02:35");
        assert_eq!(clock_text(23, 5), "23:05");
        assert_eq!(clock_text(0, 0), "00:00");
        assert_eq!(clock_text(99, 99), "23:59");
    }

    fn inside(region: Region, extent: Extent) -> bool {
        region.width > 0
            && region.height > 0
            && region.right() <= extent.width
            && region.bottom() <= extent.height
    }

    #[test]
    fn layout_regions_stay_inside_and_never_overlap() {
        let extents = [
            HD,
            Extent {
                width: 1280,
                height: 1024,
            },
            Extent {
                width: 3840,
                height: 1080,
            },
            MIN_EXTENT,
            Extent {
                width: 2560,
                height: 1600,
            },
        ];
        for extent in extents {
            for message in ["", "Two minutes left"] {
                for show_clock in [false, true] {
                    for show_next in [false, true] {
                        let panel = StagePanel {
                            message: message.into(),
                            show_clock,
                            show_next,
                            ..panel()
                        };
                        let layout = layout(extent, &panel).unwrap();
                        let regions = layout.regions();
                        for (i, a) in regions.iter().enumerate() {
                            assert!(inside(*a, extent), "{extent:?} {a:?}");
                            for b in &regions[i + 1..] {
                                assert!(!a.overlaps(b), "{extent:?} {a:?} {b:?}");
                            }
                        }
                        assert_eq!(layout.clock.is_some(), show_clock);
                        assert_eq!(layout.message.is_some(), !message.is_empty());
                        assert_eq!(layout.next.is_some(), show_next);
                        // The current slide always gets the largest share.
                        if let Some(next) = layout.next {
                            assert!(layout.current.height > next.height);
                        }
                    }
                }
            }
        }
        assert_eq!(
            layout(
                Extent {
                    width: 319,
                    height: 400
                },
                &panel()
            ),
            None
        );
    }

    #[test]
    fn a_message_takes_room_from_the_body_not_the_header() {
        let quiet = layout(HD, &panel()).unwrap();
        let loud = layout(
            HD,
            &StagePanel {
                message: "Wrap up".into(),
                ..panel()
            },
        )
        .unwrap();
        assert_eq!(quiet.label, loud.label);
        assert_eq!(quiet.clock, loud.clock);
        assert!(loud.current.y > quiet.current.y);
        assert!(loud.current.height < quiet.current.height);
    }

    #[test]
    fn fit_respects_width_height_lines_and_glyphs() {
        let font = crate::fonts::BUNDLED;
        let region = Region {
            x: 0,
            y: 0,
            width: 1000,
            height: 300,
        };
        let one = fit("Original refrain", font, 0, region).unwrap();
        let two = fit("Original refrain\nOriginal refrain", font, 0, region).unwrap();
        assert!(two < one, "{two} {one}");
        assert!(f32::from(two) * LINE_HEIGHT * 2. <= 300.);
        let wide = fit(&"W".repeat(40), font, 0, region).unwrap();
        let narrow = fit(&"i".repeat(40), font, 0, region).unwrap();
        assert!(wide < narrow);
        assert_eq!(fit("", font, 0, region), None);
        assert_eq!(fit("\u{4e2d}", font, 0, region), None, "no glyph");
        assert_eq!(fit("x", b"not a font", 0, region), None);
        assert_eq!(fit(&"x\n".repeat(40), font, 0, region), None);
    }
}
