//! Photos over the decode budget are shrunk at import (owner decision): the
//! stored copy fits the output size, keeps the aspect and is re-encoded in
//! the source's format. The source file is only read. Policy: `docs/images.md`.
use super::{Error, MAX_DECODED_BYTES, MAX_EDGE, MAX_FILE_BYTES};
use crate::scene::Extent;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, imageops::FilterType};
use std::io::Cursor;

/// Largest declared pixel count that is decoded for shrinking: 48 MP
/// (8000 × 6000). A 16-bit PNG buffer is twice RGBA, so a decode peaks at a
/// few hundred MiB on the import thread, never on the UI thread.
pub(super) const MAX_SOURCE_PIXELS: u64 = 48_000_000;
/// Largest source file read for a photo that will be shrunk.
pub(super) const MAX_SOURCE_BYTES: usize = 48 * 1024 * 1024;
const JPEG_QUALITY: u8 = 90;

pub(super) struct Shrunk {
    pub bytes: Vec<u8>,
    /// The source's displayed size, after its Exif orientation.
    pub from: Extent,
    pub extent: Extent,
}

/// The largest size inside `fit` that also keeps the decoded copy inside the
/// decode budget, never larger than the source and never below one pixel.
fn target(from: Extent, fit: Extent) -> Extent {
    let budget_pixels = (MAX_DECODED_BYTES / 4) as f64;
    let (w, h) = (f64::from(from.width), f64::from(from.height));
    let scale = (f64::from(fit.width.max(1)) / w)
        .min(f64::from(fit.height.max(1)) / h)
        .min((budget_pixels / (w * h)).sqrt())
        .min(1.0);
    Extent {
        width: ((w * scale).floor() as u32).max(1),
        height: ((h * scale).floor() as u32).max(1),
    }
}

/// Decodes `bytes` (already inspected: PNG or JPEG within `MAX_SOURCE_PIXELS`),
/// applies the Exif orientation, shrinks to fit `fit` and re-encodes.
pub(super) fn fit(
    bytes: &[u8],
    fit: Extent,
    cancel: &mut dyn FnMut() -> bool,
) -> Result<Shrunk, Error> {
    let format = image::guess_format(bytes).map_err(|_| Error::Unsupported)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_EDGE);
    limits.max_image_height = Some(MAX_EDGE);
    limits.max_alloc = Some(MAX_SOURCE_PIXELS * 8);
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| Error::Corrupt)?;
    let orientation = decoder.orientation().map_err(|_| Error::Corrupt)?;
    let mut decoded = DynamicImage::from_decoder(decoder).map_err(|error| match error {
        image::ImageError::Limits(_) => Error::Dimensions,
        _ => Error::Corrupt,
    })?;
    if cancel() {
        return Err(Error::Cancelled);
    }
    decoded.apply_orientation(orientation);
    let from = Extent {
        width: decoded.width(),
        height: decoded.height(),
    };
    let to = target(from, fit);
    let resized = if to == from {
        decoded
    } else {
        // The 8-bit copy is dropped first: only one full-size buffer lives
        // at a time.
        let eight_bit = match format {
            image::ImageFormat::Jpeg => DynamicImage::ImageRgb8(decoded.into_rgb8()),
            _ => DynamicImage::ImageRgba8(decoded.into_rgba8()),
        };
        eight_bit.resize_exact(to.width, to.height, FilterType::Triangle)
    };
    if cancel() {
        return Err(Error::Cancelled);
    }
    let mut out = Vec::new();
    match format {
        image::ImageFormat::Jpeg => {
            let rgb = resized.into_rgb8();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).write_image(
                &rgb,
                to.width,
                to.height,
                ExtendedColorType::Rgb8,
            )
        }
        _ => {
            let rgba = resized.into_rgba8();
            image::codecs::png::PngEncoder::new(&mut out).write_image(
                &rgba,
                to.width,
                to.height,
                ExtendedColorType::Rgba8,
            )
        }
    }
    .map_err(|_| Error::Corrupt)?;
    if out.len() > MAX_FILE_BYTES {
        return Err(Error::TooLarge);
    }
    Ok(Shrunk {
        bytes: out,
        from,
        extent: to,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn extent(width: u32, height: u32) -> Extent {
        Extent { width, height }
    }

    #[test]
    fn target_keeps_aspect_fits_the_box_and_never_upscales() {
        assert_eq!(
            target(extent(6000, 4000), extent(1920, 1080)),
            extent(1620, 1080)
        );
        assert_eq!(
            target(extent(4000, 6000), extent(1920, 1080)),
            extent(720, 1080)
        );
        // A huge output still lands inside the decode budget.
        let big = target(extent(6000, 4000), extent(16384, 16384));
        assert!(u64::from(big.width) * u64::from(big.height) * 4 <= MAX_DECODED_BYTES as u64);
        // A box larger than the source does not enlarge it (when in budget).
        assert_eq!(
            target(extent(4096, 4097), extent(8000, 8000)),
            extent(4095, 4096)
        );
        assert_eq!(
            target(extent(1, 20000), extent(1920, 1080)),
            extent(1, 1080)
        );
    }
}
