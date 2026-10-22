//! Image header preflight (`images::inspect`) and the bounded decoder every
//! preparation uses (`scene::decode_image`). Images come from anywhere the
//! operator browses, so their headers and pixel data are untrusted.
use super::{config, corpus, mutated, within};
use crate::images::{self, Error, MAX_DECODED_BYTES, MAX_EDGE, downscale::MAX_SOURCE_PIXELS};
use crate::scene::{self, Extent, MAX_SCENE_BYTES, PrepareError, RendererCapabilities};
use image::ImageEncoder;
use proptest::prelude::*;
use std::io::Cursor;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const CAPS: RendererCapabilities = RendererCapabilities {
    max_texture_dimension: 8192,
};

/// The dimensions the header declares, read independently of `inspect`.
fn declared(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

fn check(bytes: &[u8]) {
    for downscalable in [false, true] {
        let inspected = within(Duration::from_secs(5), "inspect", || {
            images::inspect(bytes, downscalable)
        });
        match inspected {
            Ok(found) => {
                let (w, h) = declared(bytes).expect("an inspected image has dimensions");
                assert!(w > 0 && h > 0 && w <= MAX_EDGE && h <= MAX_EDGE, "{w}x{h}");
                let pixels = u64::from(w) * u64::from(h);
                if downscalable {
                    assert!(pixels <= MAX_SOURCE_PIXELS);
                } else {
                    assert!(pixels * 4 <= MAX_DECODED_BYTES as u64);
                    assert!(!found.over_budget);
                }
            }
            Err(Error::Unsupported | Error::Corrupt | Error::Dimensions | Error::Animated) => {}
            Err(other) => panic!("inspect does no I/O, but gave {other:?}"),
        }
    }
    let never = AtomicBool::new(false);
    let decoded = within(Duration::from_secs(10), "decode", || {
        scene::decode_image(bytes, CAPS, MAX_SCENE_BYTES, &never)
    });
    match decoded {
        Ok((extent, rgba)) => {
            assert!(extent.width > 0 && extent.height > 0);
            assert!(extent.width <= CAPS.max_texture_dimension);
            assert!(extent.height <= CAPS.max_texture_dimension);
            assert_eq!(
                rgba.len(),
                extent.width as usize * extent.height as usize * 4
            );
            assert!(rgba.len() <= MAX_SCENE_BYTES);
        }
        Err(
            PrepareError::UnsupportedImage
            | PrepareError::InvalidImage
            | PrepareError::InvalidScene
            | PrepareError::TooLarge,
        ) => {}
        Err(other) => panic!("decode gave {other:?}"),
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Recomputes every PNG chunk CRC, so a mutation reaches the chunk's parser
/// instead of stopping at the checksum.
fn with_valid_crcs(mut bytes: Vec<u8>) -> Vec<u8> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return bytes;
    }
    let mut at = 8;
    while at + 12 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let Some(end) = (at + 8)
            .checked_add(len)
            .filter(|end| end + 4 <= bytes.len())
        else {
            break;
        };
        let crc = crc32(&bytes[at + 4..end]);
        bytes[end..end + 4].copy_from_slice(&crc.to_be_bytes());
        at = end + 4;
    }
    bytes
}

fn png(width: u32, height: u32, color: image::ExtendedColorType) -> Vec<u8> {
    let bytes = (color.bits_per_pixel() as usize).div_ceil(8);
    let pixels: Vec<u8> = (0..width as usize * height as usize * bytes)
        .map(|i| (i * 37 % 251) as u8)
        .collect();
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded)
        .write_image(&pixels, width, height, color)
        .unwrap();
    encoded
}

fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let pixels: Vec<u8> = (0..width as usize * height as usize * 3)
        .map(|i| (i * 13 % 251) as u8)
        .collect();
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 80)
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgb8)
        .unwrap();
    encoded
}

fn seeds() -> Vec<Vec<u8>> {
    use image::ExtendedColorType as C;
    let mut seeds = vec![
        png(3, 2, C::Rgba8),
        png(5, 3, C::Rgb8),
        png(4, 4, C::L8),
        png(2, 3, C::La8),
        png(3, 3, C::Rgba16),
        jpeg(8, 8),
        jpeg(17, 9),
    ];
    seeds.extend(corpus("images").into_iter().map(|(_, b)| b));
    seeds
}

const TOKENS: &[&str] = &[
    "IHDR",
    "IDAT",
    "IEND",
    "acTL",
    "fcTL",
    "eXIf",
    "iCCP",
    "PLTE",
    "tRNS",
    "\u{0}\u{0}\u{0}\u{0}",
    "\u{0}\u{0}\u{ff}\u{ff}",
    "\u{7f}\u{ff}\u{ff}\u{ff}",
    "\u{ff}\u{d8}",
    "\u{ff}\u{e1}",
    "\u{ff}\u{c0}",
    "\u{ff}\u{c2}",
    "\u{ff}\u{c4}",
    "\u{ff}\u{da}",
    "\u{ff}\u{d9}",
    "Exif\u{0}\u{0}II*\u{0}",
];

proptest! {
    #![proptest_config(config(0x5e1a_0601, 256, "tests/fixtures/fuzz/images.proptest-regressions"))]

    #[test]
    fn mutated_images_decode_within_bounds_or_fail(bytes in mutated(seeds(), TOKENS)) {
        check(&with_valid_crcs(bytes.clone()));
        check(&bytes);
    }

    #[test]
    fn declared_dimensions_are_bounded_before_any_pixel_buffer(
        width in prop_oneof![1u32..64, 8000u32..9000, 16_000u32..17_000, Just(u32::MAX), Just(0u32)],
        height in prop_oneof![1u32..64, 4000u32..5000, 16_000u32..17_000, Just(u32::MAX), Just(0u32)],
        depth in prop_oneof![Just(8u8), Just(16u8)],
    ) {
        // A one-pixel RGBA PNG whose IHDR declares another size.
        let mut bytes = png(1, 1, image::ExtendedColorType::Rgba8);
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        bytes[24] = depth;
        check(&with_valid_crcs(bytes));
    }
}

#[test]
fn retained_corpus_replays_cleanly() {
    for (path, bytes) in corpus("images") {
        within(Duration::from_secs(20), &path.display().to_string(), || {
            check(&bytes);
            check(&with_valid_crcs(bytes.clone()));
        });
    }
}

#[test]
fn a_decoded_image_keeps_its_declared_or_oriented_extent() {
    for bytes in seeds() {
        if let Ok((extent, _)) =
            scene::decode_image(&bytes, CAPS, MAX_SCENE_BYTES, &AtomicBool::new(false))
        {
            let (w, h) = declared(&bytes).unwrap();
            assert!(
                extent
                    == Extent {
                        width: w,
                        height: h
                    }
                    || extent
                        == Extent {
                            width: h,
                            height: w
                        }
            );
        }
    }
}
