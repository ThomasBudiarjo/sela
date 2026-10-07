//! Slide backgrounds (EW8-OBS-037..043). No GPUI types.
//!
//! A slide's background is an override: `None` on a section follows the
//! song's master, and a song without a master renders black. EasyWorship's
//! master is its theme's Master layout; until themes exist (M1-08) the song
//! master stands in for it.
use crate::images;

/// Encoded size bound, checked by storage before decoding.
pub const MAX_ENCODED: usize = 320;

const CODEC: u8 = 1;

/// EW Aspect Ratio list order (EW8-OBS-040).
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub enum Aspect {
    /// The whole image, letterboxed in black.
    Maintain,
    /// The image distorted to the output.
    Stretch,
    /// The output covered, edges cropped. The owner's default for a new
    /// image (EW shows Auto + Stretch).
    #[default]
    Zoom,
}

/// A profile image by file name, pinned to the bytes chosen in the editor so
/// a later replacement is detected instead of silently shown.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ImageRef {
    pub name: String,
    pub sha256: [u8; 32],
}

/// EW fill list None, Color Fill, Media Fill (EW8-OBS-038); Gradient Fill is
/// not supported yet.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Fill {
    /// No fill: black.
    None,
    Color([u8; 3]),
    /// Media Fill; black until an image is chosen (EW8-OBS-038).
    Media(Option<ImageRef>),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Background {
    pub fill: Fill,
    /// Used by Media fills only, kept across fill changes.
    pub aspect: Aspect,
}

impl Background {
    pub fn color(rgb: [u8; 3]) -> Self {
        Self {
            fill: Fill::Color(rgb),
            aspect: Aspect::default(),
        }
    }

    pub fn image(image: ImageRef) -> Self {
        Self {
            fill: Fill::Media(Some(image)),
            aspect: Aspect::default(),
        }
    }

    pub fn is_valid(&self) -> bool {
        match &self.fill {
            Fill::Media(Some(image)) => images::valid_name(&image.name),
            _ => true,
        }
    }

    /// The image this background needs prepared, if any.
    pub fn image_ref(&self) -> Option<&ImageRef> {
        match &self.fill {
            Fill::Media(Some(image)) => Some(image),
            _ => None,
        }
    }

    /// Canonical bytes: codec, fill tag and payload, aspect.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![CODEC];
        match &self.fill {
            Fill::None => out.push(0),
            Fill::Color(rgb) => {
                out.push(1);
                out.extend_from_slice(rgb);
            }
            Fill::Media(None) => out.push(2),
            Fill::Media(Some(image)) => {
                out.push(3);
                out.push(image.name.len() as u8);
                out.extend_from_slice(image.name.as_bytes());
                out.extend_from_slice(&image.sha256);
            }
        }
        out.push(self.aspect as u8);
        out
    }

    /// Strict inverse of `encode`: unknown codec, tags or aspect, trailing
    /// bytes and unusable image names are rejected.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_ENCODED {
            return None;
        }
        let mut r = Reader(bytes);
        if r.u8()? != CODEC {
            return None;
        }
        let fill = match r.u8()? {
            0 => Fill::None,
            1 => Fill::Color(r.take(3)?.try_into().ok()?),
            2 => Fill::Media(None),
            3 => {
                let len = usize::from(r.u8()?);
                let name = std::str::from_utf8(r.take(len)?).ok()?.to_owned();
                let sha256 = r.take(32)?.try_into().ok()?;
                Fill::Media(Some(ImageRef { name, sha256 }))
            }
            _ => return None,
        };
        let aspect = match r.u8()? {
            0 => Aspect::Maintain,
            1 => Aspect::Stretch,
            2 => Aspect::Zoom,
            _ => return None,
        };
        let background = Self { fill, aspect };
        (r.0.is_empty() && background.is_valid()).then_some(background)
    }
}

/// The background a slide shows: its own, else the master, else black.
pub fn resolve<'a>(
    slide: Option<&'a Background>,
    master: Option<&'a Background>,
) -> Option<&'a Background> {
    slide.or(master)
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Option<&[u8]> {
        let (head, rest) = self.0.split_at_checked(len)?;
        self.0 = rest;
        Some(head)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn image(name: &str) -> Background {
        Background {
            fill: Fill::Media(Some(ImageRef {
                name: name.into(),
                sha256: [7; 32],
            })),
            aspect: Aspect::Maintain,
        }
    }

    #[test]
    fn every_fill_and_aspect_round_trips() {
        let mut all = vec![
            Background {
                fill: Fill::None,
                aspect: Aspect::Stretch,
            },
            Background::color([1, 2, 3]),
            Background {
                fill: Fill::Media(None),
                aspect: Aspect::Zoom,
            },
            image("Sunrise (2).jpg"),
            image("思源 Ünïcode.png"),
        ];
        let longest = format!("{}.png", "a".repeat(251));
        all.push(image(&longest));
        for background in all {
            let bytes = background.encode();
            assert!(bytes.len() <= MAX_ENCODED);
            assert_eq!(Background::decode(&bytes), Some(background));
        }
        assert_eq!(Aspect::default(), Aspect::Zoom);
    }

    #[test]
    fn decode_rejects_unknown_truncated_trailing_and_unsafe_names() {
        let bytes = image("a.png").encode();
        assert_eq!(Background::decode(&[]), None);
        let mut codec = bytes.clone();
        codec[0] = 2;
        assert_eq!(Background::decode(&codec), None);
        for len in 0..bytes.len() {
            assert_eq!(Background::decode(&bytes[..len]), None, "{len}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(Background::decode(&trailing), None);
        assert_eq!(Background::decode(&[1, 4, 0]), None);
        assert_eq!(Background::decode(&[1, 0, 3]), None);
        assert_eq!(Background::decode(&vec![1; MAX_ENCODED + 1]), None);
        for name in ["../a.png", "a.gif", ".a.png", "a\\b.png", " a.png", ""] {
            let bad = image(name);
            assert!(!bad.is_valid(), "{name}");
            assert_eq!(Background::decode(&bad.encode()), None, "{name}");
        }
    }

    #[test]
    fn slide_overrides_master_and_none_falls_back() {
        let master = Background::color([0, 0, 255]);
        let slide = image("a.png");
        assert_eq!(resolve(Some(&slide), Some(&master)), Some(&slide));
        assert_eq!(resolve(None, Some(&master)), Some(&master));
        assert_eq!(resolve(None, None), None);
        let none = Background {
            fill: Fill::None,
            aspect: Aspect::Zoom,
        };
        assert_eq!(resolve(Some(&none), Some(&master)), Some(&none));
    }
}
