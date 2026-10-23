//! Installed fonts (`fonts::Catalog::scan`, `Fonts::resolve`). Font files
//! come from the system font folders, where any program or user may have
//! installed one, so their tables and names are untrusted.
use super::{apply, config, edits};
use crate::fonts::{BUNDLED, Catalog, Fonts};
use crate::format::SlideFormat;
use proptest::prelude::*;
use std::sync::Arc;

/// A minimal TrueType font: `head`, `hhea`, `maxp` and a `name` table with
/// the given (name id, language, text) records on the Windows Unicode BMP
/// platform. Enough for ttf-parser to parse the face and read its names.
fn font_with_names(names: &[(u16, u16, String)]) -> Vec<u8> {
    let mut head = vec![0u8; 54];
    head[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    let mut hhea = vec![0u8; 36];
    hhea[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    hhea[34..36].copy_from_slice(&1u16.to_be_bytes());
    let mut maxp = 0x0000_5000u32.to_be_bytes().to_vec();
    maxp.extend_from_slice(&1u16.to_be_bytes());

    let mut storage = Vec::new();
    let mut records = Vec::new();
    for (name_id, language, text) in names {
        let encoded: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let encoded = &encoded[..encoded.len().min(4096)];
        for value in [3u16, 1, *language, *name_id] {
            records.extend_from_slice(&value.to_be_bytes());
        }
        records.extend_from_slice(&(encoded.len() as u16).to_be_bytes());
        records.extend_from_slice(&(storage.len() as u16).to_be_bytes());
        storage.extend_from_slice(encoded);
    }
    let mut name = Vec::new();
    name.extend_from_slice(&0u16.to_be_bytes());
    name.extend_from_slice(&(names.len() as u16).to_be_bytes());
    name.extend_from_slice(&((6 + records.len()) as u16).to_be_bytes());
    name.extend(records);
    name.extend(storage);

    let tables: [(&[u8; 4], Vec<u8>); 4] = [
        (b"head", head),
        (b"hhea", hhea),
        (b"maxp", maxp),
        (b"name", name),
    ];
    let mut font = 0x0001_0000u32.to_be_bytes().to_vec();
    font.extend_from_slice(&(tables.len() as u16).to_be_bytes());
    font.extend_from_slice(&[0, 64, 0, 2, 0, 0]);
    let mut offset = 12 + 16 * tables.len();
    let mut body = Vec::new();
    for (tag, data) in &tables {
        font.extend_from_slice(*tag);
        font.extend_from_slice(&0u32.to_be_bytes());
        font.extend_from_slice(&(offset as u32).to_be_bytes());
        font.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut padded = data.clone();
        padded.resize(data.len().div_ceil(4) * 4, 0);
        offset += padded.len();
        body.extend(padded);
    }
    font.extend(body);
    font
}

/// Every family the catalog lists can be chosen in the Format pane: the
/// slide format that names it is valid, and resolving it does not panic.
fn check_catalog(files: &[(&str, Vec<u8>)]) -> Catalog {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in files {
        std::fs::write(dir.path().join(name), bytes).unwrap();
    }
    let catalog = Catalog::scan([dir.path().to_path_buf()]);
    let families: Vec<String> = catalog.families().to_vec();
    let fonts = Fonts::new();
    fonts.install(Arc::new(Catalog::scan([dir.path().to_path_buf()])));
    for family in &families {
        let format = SlideFormat {
            font: Some(family.clone()),
            ..SlideFormat::default()
        };
        assert!(
            format.is_valid(),
            "listed family {family:?} cannot be chosen"
        );
        let _ = fonts.resolve(&format);
    }
    catalog
}

fn name_records() -> impl Strategy<Value = Vec<(u16, u16, String)>> {
    proptest::collection::vec(
        (
            prop_oneof![Just(1u16), Just(16u16), Just(2u16)],
            prop_oneof![Just(0x409u16), Just(0x421u16), any::<u16>()],
            prop_oneof![
                "[A-Za-z][A-Za-z0-9 ]{0,30}",
                any::<String>(),
                "[ \\t\\n\\x00\\x7f\\u{85}\\u{3000}]{0,4}[A-Za-z]{1,8}[ \\t\\n\\x00\\x7f\\u{85}]{0,4}",
                "[a-z]{200,300}",
            ],
        ),
        0..4,
    )
}

proptest! {
    #![proptest_config(config(0x5e1a_0901, 128, "tests/fixtures/fuzz/fonts.proptest-regressions"))]

    #[test]
    fn listed_font_families_can_always_be_chosen(names in name_records()) {
        let catalog = check_catalog(&[("named.ttf", font_with_names(&names))]);
        prop_assert!(catalog.families().len() <= 1);
    }
}

proptest! {
    #![proptest_config(config(0x5e1a_0902, 48, "tests/fixtures/fuzz/fonts-mutated.proptest-regressions"))]

    #[test]
    fn mutated_font_files_never_break_the_scan(
        edits in edits(&["head", "name", "glyf", "loca", "cmap", "\u{0}\u{0}\u{ff}\u{ff}", "ttcf"]),
    ) {
        let small = font_with_names(&[(1, 0x409, "Fuzz Sans".into())]);
        check_catalog(&[
            ("bundled.ttf", apply(BUNDLED.to_vec(), &edits)),
            ("small.ttf", apply(small, &edits)),
            ("small.ttc", apply(b"ttcf\0\x01\0\0\0\0\0\x01\0\0\0\x10".to_vec(), &edits)),
        ]);
    }
}

#[test]
fn the_synthetic_font_parses_and_lists_its_family() {
    let catalog = check_catalog(&[(
        "named.ttf",
        font_with_names(&[(1, 0x409, "Fuzz Sans".into())]),
    )]);
    assert_eq!(catalog.families(), ["Fuzz Sans"]);
}

// M5-02 fuzz finding SEC-08: a font whose family name holds control
// characters was listed in the Format pane but its slide format was invalid,
// so choosing it could never be saved. Such names are now skipped; another
// usable name record of the same face still lists it.
#[test]
fn a_family_name_with_control_characters_is_not_listed() {
    let hostile = font_with_names(&[(1, 0x409, "\u{7f}Evil\nSans".into())]);
    let mixed = font_with_names(&[
        (16, 0x409, "Bad\u{0}Name".into()),
        (1, 0x409, "Good Sans".into()),
    ]);
    let catalog = check_catalog(&[("hostile.ttf", hostile), ("mixed.ttf", mixed)]);
    assert_eq!(catalog.families(), ["Good Sans"]);
    assert_eq!(catalog.excluded, 1);
}
