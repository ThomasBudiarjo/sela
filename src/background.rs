//! Slide backgrounds (EW8-OBS-037..043). No GPUI types.
//!
//! A slide's background is an override: `None` on a section follows the
//! song's master, and a song without a master renders black. EasyWorship's
//! master is its theme's Master layout; until themes exist (M1-08) the song
//! master stands in for it.
use crate::{images, scene::Extent};
use image::{GenericImage, GenericImageView, ImageBuffer, Rgba, RgbaImage, imageops};

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

pub const BLACK: [u8; 3] = [0, 0, 0];

/// What a resolved background needs before it can become cue pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan<'a> {
    Color([u8; 3]),
    /// A profile image, decoded and fitted off the UI thread.
    Image(&'a ImageRef, Aspect),
}

/// No background, no fill and an empty Media Fill are all black.
pub fn plan(background: Option<&Background>) -> Plan<'_> {
    match background {
        Some(Background {
            fill: Fill::Color(rgb),
            ..
        }) => Plan::Color(*rgb),
        Some(Background {
            fill: Fill::Media(Some(image)),
            aspect,
        }) => Plan::Image(image, *aspect),
        _ => Plan::Color(BLACK),
    }
}

/// Straight RGBA `src` of `from` pixels fitted to exactly `to` pixels, so the
/// renderer composites it 1:1. Zoom covers `to` and crops the overflow evenly;
/// Stretch scales each axis independently; Maintain shows the whole image
/// centered on opaque black bars. `None` for empty extents or a length that
/// does not match `from`.
pub fn fit(src: &[u8], from: Extent, to: Extent, aspect: Aspect) -> Option<Vec<u8>> {
    if [from.width, from.height, to.width, to.height].contains(&0)
        || src.len() as u64 != u64::from(from.width) * u64::from(from.height) * 4
    {
        return None;
    }
    let source = ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(from.width, from.height, src)?;
    let (fw, fh) = (f64::from(from.width), f64::from(from.height));
    let (tw, th) = (f64::from(to.width), f64::from(to.height));
    let fitted = match aspect {
        Aspect::Stretch => scaled(&source, to.width, to.height),
        Aspect::Zoom => {
            let scale = (tw / fw).max(th / fh);
            let w = ((tw / scale).round() as u32).clamp(1, from.width);
            let h = ((th / scale).round() as u32).clamp(1, from.height);
            let crop =
                imageops::crop_imm(&source, (from.width - w) / 2, (from.height - h) / 2, w, h);
            scaled(&*crop, to.width, to.height)
        }
        Aspect::Maintain => {
            let scale = (tw / fw).min(th / fh);
            let w = ((fw * scale).round() as u32).clamp(1, to.width);
            let h = ((fh * scale).round() as u32).clamp(1, to.height);
            let inner = scaled(&source, w, h);
            let mut out = RgbaImage::from_pixel(to.width, to.height, Rgba([0, 0, 0, 255]));
            imageops::replace(
                &mut out,
                &inner,
                i64::from((to.width - w) / 2),
                i64::from((to.height - h) / 2),
            );
            out
        }
    };
    Some(fitted.into_raw())
}

/// A filtered resize, or an exact copy at the same size.
fn scaled<I: GenericImageView<Pixel = Rgba<u8>>>(view: &I, width: u32, height: u32) -> RgbaImage {
    if view.dimensions() == (width, height) {
        let mut out = RgbaImage::new(width, height);
        out.copy_from(view, 0, 0)
            .expect("same-size copy is in bounds");
        out
    } else {
        imageops::resize(view, width, height, imageops::FilterType::Triangle)
    }
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

    const R: [u8; 4] = [255, 0, 0, 255];
    const G: [u8; 4] = [0, 255, 0, 255];
    const B: [u8; 4] = [0, 0, 255, 255];
    const K: [u8; 4] = [0, 0, 0, 255];

    fn extent(width: u32, height: u32) -> Extent {
        Extent { width, height }
    }

    fn raster(rows: &[&[[u8; 4]]]) -> Vec<u8> {
        rows.iter().flat_map(|row| row.concat()).collect()
    }

    fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * width + x) * 4) as usize;
        rgba[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn zoom_crops_the_center_evenly_on_either_axis() {
        let wide = raster(&[&[R, G, G, B], &[R, G, G, B]]);
        assert_eq!(
            fit(&wide, extent(4, 2), extent(2, 2), Aspect::Zoom).unwrap(),
            raster(&[&[G, G], &[G, G]])
        );
        // Tall source, square output: the outer rows are cropped and the
        // uniform center scales up without bleeding.
        let tall = raster(&[&[R, R], &[G, G], &[G, G], &[B, B]]);
        let zoom = fit(&tall, extent(2, 4), extent(4, 4), Aspect::Zoom).unwrap();
        assert!(zoom.as_chunks::<4>().0.iter().all(|p| *p == G));
    }

    #[test]
    fn stretch_fills_the_output_and_distorts() {
        let half = [R, R, R, R, B, B, B, B];
        let src = raster(&[&half, &half]);
        let out = fit(&src, extent(8, 2), extent(16, 8), Aspect::Stretch).unwrap();
        assert_eq!(out.len(), 16 * 8 * 4);
        for y in [0, 7] {
            for x in [0, 1] {
                assert_eq!(pixel(&out, 16, x, y), R, "{x},{y}");
                assert_eq!(pixel(&out, 16, 15 - x, y), B, "{x},{y}");
            }
        }
    }

    #[test]
    fn maintain_shows_the_whole_image_between_black_bars() {
        let wide = raster(&[&[R, G, G, B], &[R, G, G, B]]);
        assert_eq!(
            fit(&wide, extent(4, 2), extent(4, 4), Aspect::Maintain).unwrap(),
            raster(&[&[K; 4], &[R, G, G, B], &[R, G, G, B], &[K; 4]])
        );
        let tall = raster(&[&[R, B], &[R, B], &[R, B], &[R, B]]);
        let row = [K, K, K, R, B, K, K, K];
        assert_eq!(
            fit(&tall, extent(2, 4), extent(8, 4), Aspect::Maintain).unwrap(),
            raster(&[&row, &row, &row, &row])
        );
        // Straight alpha passes through; the renderer composites it over black.
        let clear = [9, 9, 9, 0];
        assert_eq!(
            fit(&clear, extent(1, 1), extent(1, 1), Aspect::Maintain).unwrap(),
            clear
        );
    }

    #[test]
    fn fit_rejects_empty_extents_and_wrong_lengths() {
        assert_eq!(fit(&[], extent(0, 1), extent(1, 1), Aspect::Zoom), None);
        assert_eq!(fit(&R, extent(1, 1), extent(1, 0), Aspect::Zoom), None);
        assert_eq!(fit(&R, extent(2, 1), extent(1, 1), Aspect::Stretch), None);
        assert_eq!(
            fit(&[R, R].concat(), extent(1, 1), extent(1, 1), Aspect::Zoom),
            None
        );
    }

    #[test]
    fn plan_maps_absent_and_empty_fills_to_black() {
        let image = image("a.png");
        assert_eq!(plan(None), Plan::Color(BLACK));
        assert_eq!(
            plan(Some(&Background::color([1, 2, 3]))),
            Plan::Color([1, 2, 3])
        );
        for fill in [Fill::None, Fill::Media(None)] {
            let background = Background {
                fill,
                aspect: Aspect::Zoom,
            };
            assert_eq!(plan(Some(&background)), Plan::Color(BLACK));
        }
        assert_eq!(
            plan(Some(&image)),
            Plan::Image(image.image_ref().unwrap(), Aspect::Maintain)
        );
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
