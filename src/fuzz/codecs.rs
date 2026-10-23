//! Binary codecs read back from the library database and from schedule
//! files: slide formats, backgrounds and the song text payload. A schedule
//! file from another machine, or a damaged database row, reaches them with
//! arbitrary bytes.
use super::{config, corpus};
use crate::background::{Aspect, Background, Fill, ImageRef};
use crate::format::{
    Align, MAX_FIXED_SIZE, MAX_OPACITY, MAX_OUTLINE_SIZE, MAX_SHADOW_BLUR, MAX_SHADOW_OFFSET,
    Outline, Shadow, Size, SlideFormat, VAlign,
};
use crate::storage::{MAX_SONG_BYTES, Song};
use proptest::prelude::*;

/// Strict codecs: whatever decodes is valid and encodes to the same bytes.
fn check_format(bytes: &[u8]) {
    if let Some(format) = SlideFormat::decode(bytes) {
        assert!(format.is_valid() && !format.is_default(), "{format:?}");
        assert_eq!(format.encode(), bytes, "canonical encoding");
    }
}

fn check_background(bytes: &[u8]) {
    if let Some(background) = Background::decode(bytes) {
        assert!(background.is_valid(), "{background:?}");
        assert_eq!(background.encode(), bytes, "canonical encoding");
    }
}

fn check_song_payload(bytes: &[u8]) {
    if let Ok(song) = Song::decode(bytes) {
        assert!(bytes.len() <= MAX_SONG_BYTES);
        assert_eq!(song.encode(), bytes, "canonical encoding");
    }
}

pub(crate) fn slide_formats() -> impl Strategy<Value = SlideFormat> {
    let rgb = any::<[u8; 3]>();
    (
        proptest::option::of("[A-Za-z][A-Za-z0-9 \u{e9}\u{8056}]{0,40}"),
        (
            proptest::option::of(any::<bool>()),
            proptest::option::of(any::<bool>()),
            proptest::option::of(any::<bool>()),
        ),
        proptest::option::of(prop_oneof![
            Just(Size::Auto),
            (1..=MAX_FIXED_SIZE).prop_map(Size::Fixed)
        ]),
        proptest::option::of(rgb),
        proptest::option::of(prop_oneof![
            Just(Align::Left),
            Just(Align::Center),
            Just(Align::Right)
        ]),
        proptest::option::of(prop_oneof![
            Just(VAlign::Top),
            Just(VAlign::Middle),
            Just(VAlign::Bottom)
        ]),
        proptest::option::of(
            (any::<bool>(), rgb, 1..=MAX_OUTLINE_SIZE, 0..=MAX_OPACITY).prop_map(
                |(enabled, color, size, opacity)| Outline {
                    enabled,
                    color,
                    size,
                    opacity,
                },
            ),
        ),
        proptest::option::of(
            (
                any::<bool>(),
                rgb,
                0u16..360,
                0..=MAX_SHADOW_OFFSET,
                0..=MAX_SHADOW_BLUR,
                0..=MAX_OPACITY,
            )
                .prop_map(|(enabled, color, angle, offset, blur, opacity)| Shadow {
                    enabled,
                    color,
                    angle,
                    offset,
                    blur,
                    opacity,
                }),
        ),
    )
        .prop_map(
            |(font, (bold, italic, underline), size, color, align, valign, outline, shadow)| {
                SlideFormat {
                    font: font.map(|f| f.trim_end().to_string()),
                    bold,
                    italic,
                    underline,
                    size,
                    color,
                    align,
                    valign,
                    outline,
                    shadow,
                    margins: None,
                }
            },
        )
}

pub(crate) fn backgrounds() -> impl Strategy<Value = Background> {
    let fill = prop_oneof![
        Just(Fill::None),
        any::<[u8; 3]>().prop_map(Fill::Color),
        Just(Fill::Media(None)),
        (
            "[a-zA-Z0-9 _()\u{e9}\u{8056}-]{1,60}",
            any::<[u8; 32]>(),
            0usize..3
        )
            .prop_map(|(stem, sha256, ext)| Fill::Media(Some(ImageRef {
                name: format!("{}.{}", stem.trim(), ["png", "jpg", "JPEG"][ext]),
                sha256,
            }))),
    ];
    let aspect = prop_oneof![
        Just(Aspect::Maintain),
        Just(Aspect::Stretch),
        Just(Aspect::Zoom)
    ];
    (fill, aspect)
        .prop_map(|(fill, aspect)| Background { fill, aspect })
        .prop_filter("a usable image name", Background::is_valid)
}

proptest! {
    #![proptest_config(config(0x5e1a_0301, 512, "tests/fixtures/fuzz/codecs.proptest-regressions"))]

    #[test]
    fn arbitrary_bytes_decode_strictly(bytes in proptest::collection::vec(any::<u8>(), 0..400)) {
        check_format(&bytes);
        check_background(&bytes);
        check_song_payload(&bytes);
    }

    #[test]
    fn mutated_encodings_decode_strictly(
        format in slide_formats(),
        background in backgrounds(),
        edits in super::edits(&["\u{0}", "\u{1}", "\u{ff}", "\u{80}"]),
    ) {
        check_format(&super::apply(format.encode(), &edits));
        check_background(&super::apply(background.encode(), &edits));
    }

    #[test]
    fn valid_values_round_trip(format in slide_formats(), background in backgrounds()) {
        if format.is_default() {
            // The default is never stored, so it never decodes.
            prop_assert_eq!(SlideFormat::decode(&format.encode()), None);
        } else {
            prop_assert_eq!(SlideFormat::decode(&format.encode()), Some(format));
        }
        prop_assert_eq!(Background::decode(&background.encode()), Some(background));
    }

    #[test]
    fn song_payload_framing_decodes_strictly(
        strings in proptest::collection::vec(any::<String>(), 0..12),
        edits in super::edits(&["\u{0}\u{0}\u{0}\u{0}", "\u{ff}\u{ff}\u{ff}\u{7f}"]),
    ) {
        let mut bytes = Vec::new();
        for s in &strings {
            bytes.extend_from_slice(&(s.len() as u32).to_le_bytes());
            bytes.extend_from_slice(s.as_bytes());
        }
        check_song_payload(&bytes);
        check_song_payload(&super::apply(bytes, &edits));
    }
}

#[test]
fn retained_corpus_replays_cleanly() {
    for (_, bytes) in corpus("codecs") {
        check_format(&bytes);
        check_background(&bytes);
        check_song_payload(&bytes);
    }
}
