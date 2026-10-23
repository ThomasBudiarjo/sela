//! Stage renderer composition (M3-01/M3-02). Runs only on the stage
//! process's preparation and clock workers, never in a redraw.
//!
//! One RGBA coverage texture carries three independently coloured layers, so
//! the existing compositor shader draws the stage without new GPU code:
//! r (fill, white) holds the current slide and the clock, g (the outline
//! layer, sky blue) the next label, divider and next slide, b (the shadow
//! layer, amber) the current label and the stage message. No dilation or
//! blur is applied; the regions never overlap (`stage::layout`).
use super::{
    compositor::Blend,
    text::{self, TextError},
};
use sela::{
    format::{Align, VAlign},
    scene::{Extent, OutlineStyle, PreparedStage, ShadowStyle, TextStyle},
    stage::{self, Region},
};

/// Next slide text and its label (Sela's choice of colour; EW unobserved).
const NEXT: [u8; 3] = [125, 211, 252];
/// Current label and the stage message.
const NOTE: [u8; 3] = [251, 191, 36];
/// Divider coverage: a dim line in the next colour.
const DIVIDER: u8 = 96;
/// The next slide is drawn at most this share of the current slide's size.
const NEXT_SHARE: f32 = 0.6;
/// Shaped-text fallbacks when the advance estimate is slightly too large.
const RETRIES: usize = 6;

/// Which text a field holds: slide text in the live slide's face, or panel
/// chrome (labels, clock, message) in the face `stage::chrome_face` picks.
#[derive(Clone, Copy)]
enum Ink {
    Slide,
    Chrome,
}

#[derive(Clone, Copy)]
enum Channel {
    Fill = 0,
    Next = 1,
    Note = 2,
}

/// The three stage colours as compositor layers, all fully opaque.
pub fn blend() -> Blend {
    Blend::from_style(&TextStyle {
        color: [255, 255, 255],
        outline: Some(OutlineStyle {
            color: NEXT,
            size: 1,
            opacity: 100,
        }),
        shadow: Some(ShadowStyle {
            color: NOTE,
            angle: 0,
            offset: 0,
            blur: 0,
            opacity: 100,
        }),
        ..TextStyle::default()
    })
}

/// Everything but the clock, rastered once per stage cue.
pub struct Base {
    pub extent: Extent,
    pub coverage: Vec<u8>,
    pub clock: Option<Region>,
}

/// Coverage for every panel field except the clock. The current and next
/// slides come in the live slide's face; the labels, clock and message in
/// the bundled DejaVu Sans unless only the live face draws them
/// (`stage::chrome_face`). A character the chosen face lacks is drawn as a
/// stand-in (`stage::covered`), and a label, the message or the next slide
/// that still cannot be drawn is left blank, so nothing but the current
/// slide itself can reject the cue (the stage then keeps its previous
/// scene). The current slide is the one the audience already drew in this
/// face.
pub fn base(stage: &PreparedStage, extent: Extent) -> Result<Base, TextError> {
    let panel = stage.panel();
    let layout = stage::layout(extent, panel).ok_or(TextError::Bounds)?;
    let len = (extent.width as usize)
        .checked_mul(extent.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|len| *len <= 64 * 1024 * 1024)
        .ok_or(TextError::Bounds)?;
    let mut coverage = vec![0u8; len];
    for alpha in coverage.iter_mut().skip(3).step_by(4) {
        *alpha = 255;
    }
    let mut canvas = Canvas {
        coverage: &mut coverage,
        extent,
        stage,
    };
    // Optional fields: a failure leaves that area blank.
    let _ = canvas.draw(
        Ink::Chrome,
        Channel::Note,
        &panel.current_label,
        layout.label,
        (Align::Left, VAlign::Middle),
        None,
    );
    if let Some(region) = layout.message {
        let _ = canvas.draw(
            Ink::Chrome,
            Channel::Note,
            &panel.message,
            region,
            (Align::Center, VAlign::Middle),
            None,
        );
    }
    let current = canvas.draw(
        Ink::Slide,
        Channel::Fill,
        &panel.current,
        layout.current,
        (Align::Center, VAlign::Middle),
        None,
    )?;
    if let Some(region) = layout.divider {
        canvas.fill(Channel::Next, region, DIVIDER);
    }
    if let Some(region) = layout.next_label {
        let _ = canvas.draw(
            Ink::Chrome,
            Channel::Next,
            &panel.next_label,
            region,
            (Align::Left, VAlign::Middle),
            None,
        );
    }
    if let Some(region) = layout.next {
        let cap = current.map(|size| (f32::from(size) * NEXT_SHARE).max(8.) as u16);
        let _ = canvas.draw(
            Ink::Slide,
            Channel::Next,
            &panel.next,
            region,
            (Align::Center, VAlign::Top),
            cap,
        );
    }
    Ok(Base {
        extent,
        coverage,
        clock: layout.clock,
    })
}

/// The base coverage with `clock` drawn right-aligned in the header, in the
/// bundled face (`stage::chrome_face`; left blank if it cannot be drawn).
pub fn with_clock(base: &Base, stage: &PreparedStage, clock: &str) -> Vec<u8> {
    let mut coverage = base.coverage.clone();
    if let Some(region) = base.clock {
        let _ = Canvas {
            coverage: &mut coverage,
            extent: base.extent,
            stage,
        }
        .draw(
            Ink::Chrome,
            Channel::Fill,
            clock,
            region,
            (Align::Right, VAlign::Middle),
            None,
        );
    }
    coverage
}

/// The stage process's local time of day. Reads the system time zone, so it
/// runs on a worker, never in a redraw.
pub fn local_clock() -> String {
    use chrono::Timelike;
    let now = chrono::Local::now();
    stage::clock_text(now.hour(), now.minute())
}

struct Canvas<'a> {
    coverage: &'a mut [u8],
    extent: Extent,
    stage: &'a PreparedStage,
}

impl Canvas<'_> {
    /// Rasters `text` fitted into `region` (at most `cap`) into `channel`;
    /// returns the size used, `None` for empty text.
    fn draw(
        &mut self,
        ink: Ink,
        channel: Channel,
        text: &str,
        region: Region,
        (align, valign): (Align, VAlign),
        cap: Option<u16>,
    ) -> Result<Option<u16>, TextError> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        let (live, live_index) = (self.stage.font(), self.stage.face_index());
        let (font, face) = match ink {
            Ink::Slide => (live, live_index),
            Ink::Chrome => match stage::chrome_face(text, live, live_index) {
                stage::ChromeFace::Live => (live, live_index),
                stage::ChromeFace::Bundled => (sela::fonts::BUNDLED, 0),
            },
        };
        let text = stage::covered(text, font, face).ok_or(TextError::InvalidFont)?;
        let text = text.as_ref();
        let mut size = stage::fit(text, font, face, region).ok_or(TextError::MissingGlyph)?;
        if let Some(cap) = cap {
            size = size.min(cap.max(4));
        }
        let style = TextStyle {
            align,
            valign,
            ..TextStyle::default()
        };
        for _ in 0..RETRIES {
            match text::glyph_coverage(
                font,
                face,
                text,
                region.width,
                region.height,
                f32::from(size),
                &style,
            ) {
                Ok(mask) => {
                    self.blit(channel, region, &mask);
                    return Ok(Some(size));
                }
                Err(TextError::Overflow) if size > 6 => size = (size * 9 / 10).max(4),
                Err(error) => return Err(error),
            }
        }
        Err(TextError::Overflow)
    }

    fn blit(&mut self, channel: Channel, region: Region, mask: &[u8]) {
        let width = self.extent.width as usize;
        let row = region.width as usize;
        for y in 0..region.height as usize {
            let start = ((region.y as usize + y) * width + region.x as usize) * 4;
            for (x, value) in mask[y * row..(y + 1) * row].iter().enumerate() {
                let dst = &mut self.coverage[start + x * 4 + channel as usize];
                *dst = (*dst).max(*value);
            }
        }
    }

    fn fill(&mut self, channel: Channel, region: Region, value: u8) {
        let width = self.extent.width as usize;
        for y in region.y..region.bottom() {
            for x in region.x..region.right() {
                let at = (y as usize * width + x as usize) * 4 + channel as usize;
                self.coverage[at] = value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sela::{
        scene::{ContentVersion, PreparedCue, RendererCapabilities},
        stage::StagePanel,
    };
    use std::sync::Arc;

    const EXTENT: Extent = Extent {
        width: 960,
        height: 540,
    };

    fn cue(panel: StagePanel) -> PreparedCue {
        cue_in(panel, sela::fonts::BUNDLED)
    }

    /// A stage cue whose live slide face is `font`.
    fn cue_in(panel: StagePanel, font: &[u8]) -> PreparedCue {
        PreparedCue::for_stage(
            ContentVersion { id: 1, revision: 1 },
            EXTENT,
            stage::BACKGROUND,
            panel,
            (ContentVersion { id: 9, revision: 1 }, Arc::from(font), 0),
            RendererCapabilities {
                max_texture_dimension: 4096,
            },
        )
        .unwrap()
    }

    fn panel() -> StagePanel {
        StagePanel {
            current_label: "Signal Hymn · Verse 1".into(),
            current: "First original line\nSecond original line".into(),
            next_label: "Next · Chorus".into(),
            next: "Original refrain".into(),
            message: "Two minutes".into(),
            show_clock: true,
            show_next: true,
        }
    }

    /// Ink per channel inside a region.
    fn ink(coverage: &[u8], region: Region, channel: Channel) -> u64 {
        let mut total = 0;
        for y in region.y..region.bottom() {
            for x in region.x..region.right() {
                let at = (y as usize * EXTENT.width as usize + x as usize) * 4 + channel as usize;
                total += u64::from(coverage[at]);
            }
        }
        total
    }

    #[test]
    fn each_field_lands_in_its_region_and_colour() {
        let cue = cue(panel());
        let stage = cue.stage().unwrap();
        let base = base(stage, EXTENT).unwrap();
        let layout = stage::layout(EXTENT, stage.panel()).unwrap();
        assert!(ink(&base.coverage, layout.current, Channel::Fill) > 0);
        assert_eq!(ink(&base.coverage, layout.current, Channel::Next), 0);
        assert!(ink(&base.coverage, layout.next.unwrap(), Channel::Next) > 0);
        assert_eq!(ink(&base.coverage, layout.next.unwrap(), Channel::Fill), 0);
        assert!(ink(&base.coverage, layout.message.unwrap(), Channel::Note) > 0);
        assert!(ink(&base.coverage, layout.label, Channel::Note) > 0);
        // The clock region stays empty until a clock is drawn.
        let clock = base.clock.unwrap();
        assert_eq!(ink(&base.coverage, clock, Channel::Fill), 0);
        let timed = with_clock(&base, stage, "02:35");
        assert!(ink(&timed, clock, Channel::Fill) > 0);
        assert_eq!(
            ink(&timed, layout.current, Channel::Fill),
            ink(&base.coverage, layout.current, Channel::Fill),
            "the clock never touches the slide text"
        );
    }

    /// One channel of `coverage` inside `region`, row by row.
    fn pixels(coverage: &[u8], region: Region, channel: Channel) -> Vec<u8> {
        let mut out = Vec::new();
        for y in region.y..region.bottom() {
            for x in region.x..region.right() {
                let at = (y as usize * EXTENT.width as usize + x as usize) * 4 + channel as usize;
                out.push(coverage[at]);
            }
        }
        out
    }

    #[test]
    fn chrome_stays_in_the_bundled_face_whatever_the_song_face() {
        // Review finding (stage-display round 2): the clock, labels and
        // message followed the live song's face, so a decorative or symbol
        // face made them unreadable. A different live face (Bold here)
        // changes the slide text only.
        let regular = cue(panel());
        let bold = cue_in(panel(), sela::fonts::BUNDLED_BOLD);
        let (regular, bold) = (regular.stage().unwrap(), bold.stage().unwrap());
        let layout = stage::layout(EXTENT, regular.panel()).unwrap();
        let (a, b) = (base(regular, EXTENT).unwrap(), base(bold, EXTENT).unwrap());
        for (region, channel) in [
            (layout.label, Channel::Note),
            (layout.message.unwrap(), Channel::Note),
            (layout.next_label.unwrap(), Channel::Next),
        ] {
            assert_eq!(
                pixels(&a.coverage, region, channel),
                pixels(&b.coverage, region, channel)
            );
        }
        let clock = a.clock.unwrap();
        assert_eq!(
            pixels(&with_clock(&a, regular, "02:35"), clock, Channel::Fill),
            pixels(&with_clock(&b, bold, "02:35"), clock, Channel::Fill),
        );
        assert_ne!(
            pixels(&a.coverage, layout.current, Channel::Fill),
            pixels(&b.coverage, layout.current, Channel::Fill),
            "the slide itself is drawn in the live face"
        );
        assert_ne!(
            pixels(&a.coverage, layout.next.unwrap(), Channel::Next),
            pixels(&b.coverage, layout.next.unwrap(), Channel::Next),
        );
    }

    #[test]
    fn empty_next_and_message_leave_their_areas_blank() {
        let cue = cue(StagePanel {
            next: String::new(),
            message: String::new(),
            show_clock: false,
            ..panel()
        });
        let stage = cue.stage().unwrap();
        let base = base(stage, EXTENT).unwrap();
        let layout = stage::layout(EXTENT, stage.panel()).unwrap();
        assert!(layout.message.is_none() && base.clock.is_none());
        assert_eq!(ink(&base.coverage, layout.next.unwrap(), Channel::Next), 0);
        assert!(ink(&base.coverage, layout.next_label.unwrap(), Channel::Next) > 0);
    }

    #[test]
    fn a_glyph_the_face_lacks_never_costs_the_current_slide() {
        // Review finding (stage-display): an uncovered next slide used to
        // reject the whole panel, leaving performers a slide behind.
        let latin = cue(StagePanel {
            next: "\u{4e2d}\u{6587} \u{1F600}".into(),
            next_label: "Next · \u{4e2d}".into(),
            message: "\u{4e2d}".into(),
            ..panel()
        });
        let stage = latin.stage().unwrap();
        let drawn = base(stage, EXTENT).expect("the current slide is drawn");
        let layout = stage::layout(EXTENT, stage.panel()).unwrap();
        assert!(ink(&drawn.coverage, layout.current, Channel::Fill) > 0);
        // Stand-ins are drawn where the face lacks a glyph.
        assert!(ink(&drawn.coverage, layout.next.unwrap(), Channel::Next) > 0);
        // An uncovered current slide is drawn with stand-ins too.
        let cjk = cue(StagePanel {
            current: "\u{4e2d}\u{6587}".into(),
            ..panel()
        });
        let stage = cjk.stage().unwrap();
        let drawn = base(stage, EXTENT).unwrap();
        let layout = stage::layout(EXTENT, stage.panel()).unwrap();
        assert!(ink(&drawn.coverage, layout.current, Channel::Fill) > 0);
    }

    #[test]
    fn a_next_area_that_cannot_be_drawn_is_left_blank() {
        // One line too long to fit at any legible size: the next area stays
        // blank and the current slide is still drawn.
        let cue = cue(StagePanel {
            next: "x".repeat(stage::MAX_FIELD_BYTES),
            ..panel()
        });
        let stage = cue.stage().unwrap();
        let drawn = base(stage, EXTENT).expect("the panel still builds");
        let layout = stage::layout(EXTENT, stage.panel()).unwrap();
        assert!(ink(&drawn.coverage, layout.current, Channel::Fill) > 0);
        assert_eq!(ink(&drawn.coverage, layout.next.unwrap(), Channel::Next), 0);
    }

    #[test]
    fn long_text_steps_down_until_it_fits() {
        let cue = cue(StagePanel {
            current: ["WWWW MMMM WWWW MMMM"; 12].join("\n"),
            next: "W".repeat(120),
            ..panel()
        });
        assert!(base(cue.stage().unwrap(), EXTENT).is_ok());
    }

    #[test]
    fn local_clock_has_the_hh_mm_shape() {
        let text = local_clock();
        assert_eq!(text.len(), 5);
        assert_eq!(&text[2..3], ":");
    }
}
