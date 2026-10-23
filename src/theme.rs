//! Song themes (M1-08): a named look (text format, background, copyright
//! element) that songs and schedule items render with. No GPUI types.
//!
//! Inheritance is field by field and fixed (docs/themes.md):
//!
//! - Text format: the slide's own field, else the theme's field, else the
//!   built-in default (white, centered, auto size, 32 px inset).
//! - Background: the slide's own, else the song master (M1-05h), else the
//!   theme background, else black (EW8-OBS-037..041: the Master is a layout
//!   of the song's theme; Sela keeps the song master in between).
//! - Copyright strip: drawn when the song has copyright text and the theme
//!   shows the copyright element (EW8-OBS-032 Theme Elements, EW8-OBS-063).
//!
//! Which theme: a schedule item's theme snapshot, else the song's library
//! theme, else the library default, else the built-in theme.
use crate::{
    background::{Aspect, Background},
    format::SlideFormat,
    scene::Extent,
    storage::Song,
};
use sha2::{Digest, Sha256};

/// Longest theme name, in bytes.
pub const MAX_NAME: usize = 128;
/// Encoded size bound, checked by storage and schedule files before decoding.
pub const MAX_ENCODED: usize = 1024;
/// Themes a library holds; a save past it is refused.
pub const MAX_THEMES: usize = 256;
/// WCAG 2 contrast below which text is flagged (the large-text AA level;
/// lyrics are large text).
pub const MIN_CONTRAST: f32 = 3.0;
/// Image and output aspect ratios closer than this (relative) count as equal.
pub const ASPECT_TOLERANCE: f32 = 0.02;

const CODEC: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThemeId(pub [u8; 16]);

impl ThemeId {
    /// The built-in theme; never stored in a library.
    pub const BUILT_IN: Self = Self([0; 16]);

    /// CPU-only allocation (time, process, counter); never the built-in id.
    pub fn allocate() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let mut hash = Sha256::new();
        hash.update(b"sela-theme");
        hash.update(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .to_le_bytes(),
        );
        hash.update(std::process::id().to_le_bytes());
        hash.update(COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
        let mut id: [u8; 16] = hash.finalize()[..16].try_into().unwrap();
        if id == [0; 16] {
            id[0] = 1;
        }
        Self(id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Theme {
    pub id: ThemeId,
    pub name: String,
    /// The theme's text look; every unset field keeps the built-in default.
    pub format: SlideFormat,
    /// The theme's Master background; `None` is black.
    pub background: Option<Background>,
    /// Theme Elements › copyright (EW8-OBS-032).
    pub show_copyright: bool,
}

impl Theme {
    /// Sela's look before themes existed: the theme every song had until
    /// M1-08, so an untouched library renders exactly as before.
    pub fn built_in() -> Self {
        Self {
            id: ThemeId::BUILT_IN,
            name: "Default".into(),
            format: SlideFormat::default(),
            background: None,
            show_copyright: true,
        }
    }

    pub fn is_built_in(&self) -> bool {
        self.id == ThemeId::BUILT_IN
    }

    /// A new library theme with the built-in look.
    pub fn new(name: &str) -> Self {
        Self {
            id: ThemeId::allocate(),
            name: name.to_owned(),
            ..Self::built_in()
        }
    }

    pub fn is_valid(&self) -> bool {
        name_ok(&self.name)
            && self.format.is_valid()
            && self.background.as_ref().is_none_or(Background::is_valid)
    }

    /// Renders like the built-in theme (whatever its name or id).
    pub fn is_neutral(&self) -> bool {
        self.format.is_default() && self.background.is_none() && self.show_copyright
    }

    /// Identity of the rendered look: 0 for a neutral theme (so items that
    /// never had a theme keep their old projection key), else a stable hash
    /// of format, background and elements. Names and ids do not matter.
    pub fn key(&self) -> u64 {
        if self.is_neutral() {
            return 0;
        }
        let mut hash = Sha256::new();
        hash.update(self.format.encode());
        hash.update(
            self.background
                .as_ref()
                .map(Background::encode)
                .unwrap_or_default(),
        );
        hash.update([u8::from(self.show_copyright)]);
        u64::from_le_bytes(hash.finalize()[..8].try_into().unwrap()).max(1)
    }

    /// Identity of the whole theme value (id and name included), for telling
    /// apart two snapshots that render alike.
    pub fn stamp(&self) -> u64 {
        let digest = Sha256::digest(self.encode());
        u64::from_le_bytes(digest[..8].try_into().unwrap())
    }

    /// Canonical bytes: codec, id, name, format and background blobs (empty
    /// for none), element flags.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![CODEC];
        out.extend_from_slice(&self.id.0);
        out.push(self.name.len().min(usize::from(u8::MAX)) as u8);
        out.extend_from_slice(self.name.as_bytes());
        let format = if self.format.is_default() {
            Vec::new()
        } else {
            self.format.encode()
        };
        out.push(format.len() as u8);
        out.extend_from_slice(&format);
        let background = self
            .background
            .as_ref()
            .map(Background::encode)
            .unwrap_or_default();
        out.extend_from_slice(&(background.len() as u16).to_le_bytes());
        out.extend_from_slice(&background);
        out.push(u8::from(self.show_copyright));
        out
    }

    /// Strict inverse of `encode`: unknown codec or flags, trailing bytes and
    /// invalid values are rejected.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_ENCODED {
            return None;
        }
        let mut rest = bytes;
        let mut take = |len: usize| -> Option<&[u8]> {
            let (head, tail) = rest.split_at_checked(len)?;
            rest = tail;
            Some(head)
        };
        if take(1)? != [CODEC] {
            return None;
        }
        let id = ThemeId(take(16)?.try_into().ok()?);
        let len = usize::from(take(1)?[0]);
        let name = std::str::from_utf8(take(len)?).ok()?.to_owned();
        let len = usize::from(take(1)?[0]);
        let format = match take(len)? {
            [] => SlideFormat::default(),
            bytes => SlideFormat::decode(bytes)?,
        };
        let len = usize::from(u16::from_le_bytes(take(2)?.try_into().ok()?));
        let background = match take(len)? {
            [] => None,
            bytes => Some(Background::decode(bytes)?),
        };
        let show_copyright = match take(1)? {
            [0] => false,
            [1] => true,
            _ => return None,
        };
        let theme = Self {
            id,
            name,
            format,
            background,
            show_copyright,
        };
        (rest.is_empty() && theme.is_valid()).then_some(theme)
    }

    /// The text color the theme draws, built-in white when unset.
    pub fn text_color(&self) -> [u8; 3] {
        self.format.color.unwrap_or([255, 255, 255])
    }
}

/// A theme name: trimmed non-empty, at most `MAX_NAME` bytes, no controls.
pub fn name_ok(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= MAX_NAME && !name.chars().any(char::is_control)
}

/// The song as it renders under `theme`: every slide's format laid over the
/// theme's, and the theme background as the master when the song has none.
/// The song's own data is never changed, so removing a theme restores it.
pub fn apply(song: &Song, theme: &Theme) -> Song {
    let mut out = song.clone();
    if theme.is_neutral() {
        return out;
    }
    for section in &mut out.sections {
        section.format = section.format.over(&theme.format);
    }
    if out.master.is_none() {
        out.master = theme.background.clone();
    }
    out
}

/// The copyright strip text under `theme` (`slides::copyright_strip`), or
/// `None` when the theme hides the copyright element.
pub fn copyright_strip(song: &Song, theme: &Theme) -> Option<String> {
    theme
        .show_copyright
        .then(|| crate::slides::copyright_strip(song))
        .flatten()
}

/// The song's copyright strip (`slides::CopyrightStrip`) under `theme`, or
/// `None` when the theme hides the copyright element; the Copyright Info
/// options still shape its text when a cue is built.
pub fn copyright_of(song: &Song, theme: &Theme) -> Option<crate::slides::CopyrightStrip> {
    theme
        .show_copyright
        .then(|| crate::slides::CopyrightStrip::of(song))
        .flatten()
}

/// Library themes plus the chosen default song theme.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThemeSet {
    /// Sorted by name (case-insensitive), then id.
    pub themes: Vec<Theme>,
    /// The library default song theme; `None` is the built-in theme.
    pub default: Option<ThemeId>,
}

/// Which theme a song resolved to and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub theme: Theme,
    /// The song named a theme the library no longer has; the default was
    /// used instead (shown as a warning, never silently).
    pub missing: Option<ThemeId>,
}

impl ThemeSet {
    pub fn get(&self, id: ThemeId) -> Option<&Theme> {
        self.themes.iter().find(|t| t.id == id)
    }

    /// The default song theme (EW8-OBS-019 "Set As Default Song Theme").
    pub fn default_theme(&self) -> Theme {
        self.default
            .and_then(|id| self.get(id))
            .cloned()
            .unwrap_or_else(Theme::built_in)
    }

    /// The theme a song with library theme `song_theme` renders with.
    pub fn resolve(&self, song_theme: Option<ThemeId>) -> Resolution {
        match song_theme {
            Some(id) if id == ThemeId::BUILT_IN => Resolution {
                theme: Theme::built_in(),
                missing: None,
            },
            Some(id) => match self.get(id) {
                Some(theme) => Resolution {
                    theme: theme.clone(),
                    missing: None,
                },
                None => Resolution {
                    theme: self.default_theme(),
                    missing: Some(id),
                },
            },
            None => Resolution {
                theme: self.default_theme(),
                missing: None,
            },
        }
    }

    /// Sorts by name, then id, as the library lists them.
    pub fn sort(&mut self) {
        self.themes.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
    }
}

/// WCAG 2 relative luminance of an sRGB color.
pub fn luminance(rgb: [u8; 3]) -> f32 {
    let channel = |c: u8| {
        let c = f32::from(c) / 255.;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
}

/// WCAG 2 contrast ratio between two luminances (1.0 to 21.0).
pub fn contrast(a: f32, b: f32) -> f32 {
    let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

/// The contrast between the theme's text and a background of mean
/// luminance `background`, when it is below `MIN_CONTRAST` and no enabled
/// outline separates the text. An outline of enough contrast to the
/// background counts as separation.
pub fn low_contrast(theme: &Theme, background: f32) -> Option<f32> {
    let ratio = contrast(luminance(theme.text_color()), background);
    let outlined = theme.format.outline.is_some_and(|o| {
        o.enabled && o.opacity >= 50 && contrast(luminance(o.color), background) >= MIN_CONTRAST
    });
    (ratio < MIN_CONTRAST && !outlined).then_some(ratio)
}

/// What fitting an image of `image` pixels to an `output` canvas does to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AspectIssue {
    /// Stretch distorts it by this many percent.
    Distorted(u16),
    /// Maintain adds black bars.
    Bars,
    /// Zoom crops this many percent of the image.
    Cropped(u16),
}

/// The visible effect of a mismatched image aspect ratio, or `None` when the
/// ratios agree within `ASPECT_TOLERANCE`.
pub fn aspect_issue(image: Extent, output: Extent, aspect: Aspect) -> Option<AspectIssue> {
    if [image.width, image.height, output.width, output.height].contains(&0) {
        return None;
    }
    let ratio = |e: Extent| e.width as f32 / e.height as f32;
    let (from, to) = (ratio(image), ratio(output));
    let off = (from / to).max(to / from);
    if off - 1. <= ASPECT_TOLERANCE {
        return None;
    }
    let percent = ((off - 1.) * 100.).round().min(f32::from(u16::MAX)) as u16;
    Some(match aspect {
        Aspect::Stretch => AspectIssue::Distorted(percent),
        Aspect::Maintain => AspectIssue::Bars,
        Aspect::Zoom => AspectIssue::Cropped(((1. - 1. / off) * 100.).round() as u16),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        arrangement::SectionId,
        format::{Align, Margins, Outline, Size},
        storage::{Id, Section},
    };

    pub(crate) fn styled() -> Theme {
        Theme {
            id: ThemeId([7; 16]),
            name: "Evening Navy".into(),
            format: SlideFormat {
                font: Some("Fixture Sans".into()),
                bold: Some(true),
                color: Some([255, 230, 120]),
                align: Some(Align::Left),
                size: Some(Size::Fixed(64)),
                margins: Some(Margins {
                    left: 120,
                    right: 120,
                    top: 60,
                    bottom: 60,
                }),
                ..Default::default()
            },
            background: Some(Background::color([10, 20, 70])),
            show_copyright: false,
        }
    }

    fn song() -> Song {
        Song {
            title: "Theme Fixture".into(),
            authors: "Original Author".into(),
            copyright: "Original Copyright".into(),
            license: String::new(),
            administrator: String::new(),
            song_number: String::new(),
            variants: Vec::new(),
            sections: vec![
                Section {
                    id: SectionId(Id([1; 16])),
                    label: "Verse".into(),
                    lyrics: "plain slide".into(),
                    format: SlideFormat::default(),
                    background: None,
                },
                Section {
                    id: SectionId(Id([2; 16])),
                    label: "Chorus".into(),
                    lyrics: "own look".into(),
                    format: SlideFormat {
                        italic: Some(true),
                        color: Some([255, 255, 255]),
                        ..Default::default()
                    },
                    background: Some(Background::color([90, 0, 0])),
                },
            ],
            master: None,
        }
    }

    #[test]
    fn themes_round_trip_and_reject_bad_bytes() {
        for theme in [Theme::built_in(), styled(), Theme::new("Ünïcode 主题")] {
            let bytes = theme.encode();
            assert!(bytes.len() <= MAX_ENCODED);
            assert_eq!(Theme::decode(&bytes), Some(theme));
        }
        let bytes = styled().encode();
        for len in 0..bytes.len() {
            assert_eq!(Theme::decode(&bytes[..len]), None, "{len}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(Theme::decode(&trailing), None);
        let mut codec = bytes.clone();
        codec[0] = 2;
        assert_eq!(Theme::decode(&codec), None);
        let mut flag = bytes.clone();
        *flag.last_mut().unwrap() = 2;
        assert_eq!(Theme::decode(&flag), None);
        for name in ["", "   ", "tab\tname"] {
            let bad = Theme {
                name: name.into(),
                ..styled()
            };
            assert!(!bad.is_valid());
            assert_eq!(Theme::decode(&bad.encode()), None, "{name:?}");
        }
        let long = Theme {
            name: "n".repeat(MAX_NAME + 1),
            ..styled()
        };
        assert!(!long.is_valid());
        let longest = Theme {
            name: "n".repeat(MAX_NAME),
            format: crate::format::tests::full(),
            background: Some(Background::image(crate::background::ImageRef {
                name: format!("{}.png", "a".repeat(200)),
                sha256: [3; 32],
            })),
            ..styled()
        };
        assert!(longest.encode().len() <= MAX_ENCODED);
        assert_eq!(Theme::decode(&longest.encode()), Some(longest));
    }

    #[test]
    fn slide_then_master_then_theme_then_default() {
        let theme = styled();
        let themed = apply(&song(), &theme);
        // Slide 1 has nothing of its own: the theme's fields and background.
        assert_eq!(themed.sections[0].format, theme.format);
        assert_eq!(themed.master, theme.background);
        let slide = crate::slides::slides(&themed);
        assert_eq!(slide[0].background, theme.background);
        // Slide 2 keeps every field it set and its own background; the rest
        // comes from the theme.
        assert_eq!(themed.sections[1].format.italic, Some(true));
        assert_eq!(themed.sections[1].format.color, Some([255, 255, 255]));
        assert_eq!(themed.sections[1].format.font, theme.format.font);
        assert_eq!(slide[1].background, Some(Background::color([90, 0, 0])));
        // A song master beats the theme background.
        let mut mastered = song();
        mastered.master = Some(Background::color([0, 90, 0]));
        let themed = apply(&mastered, &theme);
        assert_eq!(themed.master, Some(Background::color([0, 90, 0])));
        // The built-in theme changes nothing at all.
        assert_eq!(apply(&song(), &Theme::built_in()), song());
        // Lyrics, labels and ids are never touched.
        let themed = apply(&song(), &theme);
        for (a, b) in themed.sections.iter().zip(&song().sections) {
            assert_eq!((a.id, &a.label, &a.lyrics), (b.id, &b.label, &b.lyrics));
        }
    }

    #[test]
    fn copyright_element_follows_the_theme() {
        assert_eq!(
            copyright_strip(&song(), &Theme::built_in()).as_deref(),
            Some("Theme Fixture - Original Author\nOriginal Copyright")
        );
        assert_eq!(copyright_strip(&song(), &styled()), None);
        let mut bare = song();
        bare.copyright.clear();
        assert_eq!(copyright_strip(&bare, &Theme::built_in()), None);
    }

    #[test]
    fn keys_track_the_rendered_look_only() {
        assert_eq!(Theme::built_in().key(), 0);
        assert_eq!(
            Theme::new("Plain copy").key(),
            0,
            "neutral themes share key 0"
        );
        let a = styled();
        let renamed = Theme {
            name: "Other name".into(),
            id: ThemeId([9; 16]),
            ..a.clone()
        };
        assert_eq!(a.key(), renamed.key());
        assert_ne!(a.key(), 0);
        let recolored = Theme {
            background: Some(Background::color([10, 20, 71])),
            ..a.clone()
        };
        assert_ne!(a.key(), recolored.key());
        let hidden = Theme {
            show_copyright: false,
            ..Theme::built_in()
        };
        assert_ne!(hidden.key(), 0);
    }

    #[test]
    fn resolution_prefers_song_theme_then_default_and_reports_missing() {
        let mut set = ThemeSet {
            themes: vec![styled(), Theme::new("Second")],
            default: None,
        };
        assert!(set.resolve(None).theme.is_built_in());
        assert_eq!(set.resolve(Some(styled().id)).theme, styled());
        set.default = Some(set.themes[1].id);
        assert_eq!(set.resolve(None).theme, set.themes[1]);
        let gone = ThemeId([42; 16]);
        let resolution = set.resolve(Some(gone));
        assert_eq!(resolution.missing, Some(gone));
        assert_eq!(resolution.theme, set.themes[1]);
        assert!(set.resolve(Some(ThemeId::BUILT_IN)).theme.is_built_in());
        set.default = Some(gone);
        assert!(set.default_theme().is_built_in(), "a vanished default");
        set.sort();
        assert_eq!(set.themes[0].name, "Evening Navy");
    }

    #[test]
    fn contrast_follows_wcag_and_outlines_separate() {
        assert!((contrast(luminance([255, 255, 255]), luminance([0, 0, 0])) - 21.).abs() < 0.01);
        assert!((contrast(luminance([0, 0, 0]), luminance([0, 0, 0])) - 1.).abs() < 0.01);
        let white_on_yellow = Theme {
            background: Some(Background::color([255, 255, 0])),
            ..Theme::built_in()
        };
        let ratio = low_contrast(&white_on_yellow, luminance([255, 255, 0])).unwrap();
        assert!(ratio < 1.2, "{ratio}");
        assert_eq!(low_contrast(&Theme::built_in(), luminance([0, 0, 0])), None);
        let outlined = Theme {
            format: SlideFormat {
                outline: Some(Outline {
                    enabled: true,
                    color: [0, 0, 0],
                    size: 7,
                    opacity: 100,
                }),
                ..Default::default()
            },
            ..white_on_yellow.clone()
        };
        assert_eq!(low_contrast(&outlined, luminance([255, 255, 0])), None);
        let disabled = Theme {
            format: SlideFormat {
                outline: Some(Outline {
                    enabled: false,
                    color: [0, 0, 0],
                    size: 7,
                    opacity: 100,
                }),
                ..Default::default()
            },
            ..white_on_yellow
        };
        assert!(low_contrast(&disabled, luminance([255, 255, 0])).is_some());
    }

    #[test]
    fn aspect_issues_name_the_visible_effect() {
        let hd = Extent {
            width: 1920,
            height: 1080,
        };
        let four_three = Extent {
            width: 1024,
            height: 768,
        };
        assert_eq!(aspect_issue(hd, hd, Aspect::Stretch), None);
        assert_eq!(
            aspect_issue(
                Extent {
                    width: 1280,
                    height: 720
                },
                hd,
                Aspect::Zoom
            ),
            None
        );
        assert_eq!(
            aspect_issue(four_three, hd, Aspect::Stretch),
            Some(AspectIssue::Distorted(33))
        );
        assert_eq!(
            aspect_issue(four_three, hd, Aspect::Maintain),
            Some(AspectIssue::Bars)
        );
        assert_eq!(
            aspect_issue(four_three, hd, Aspect::Zoom),
            Some(AspectIssue::Cropped(25))
        );
        // A 16:9 image on a 4:3 (landscape) output.
        assert_eq!(
            aspect_issue(hd, four_three, Aspect::Zoom),
            Some(AspectIssue::Cropped(25))
        );
        assert_eq!(
            aspect_issue(
                Extent {
                    width: 0,
                    height: 1
                },
                hd,
                Aspect::Zoom
            ),
            None
        );
    }
}
