//! Bible modules: the OSIS, USFX and Zefania importers
//! (`scripture::import::parse`, untrusted files up to 48 MiB), the bundle
//! decoder (`scripture::bundled::decode`, deflate then text) and installed
//! translation files (`scripture::store::open`, SQLite files in the profile
//! that may have been replaced or damaged).
use super::{config, corpus, mutated, within};
use crate::scripture::bundled::{self, Bundle};
use crate::scripture::import::{
    self, MAX_FILE_BYTES, MAX_VERSE_BYTES, MAX_VERSES, MAX_WARNINGS, ParsedBible, Rejected,
};
use crate::scripture::model::MAX_META;
use crate::scripture::{Book, TranslationInfo, VerseId, store};
use proptest::prelude::*;
use std::borrow::Cow;
use std::time::Duration;

fn info(id: &str, parsed: &ParsedBible) -> TranslationInfo {
    TranslationInfo {
        id: id.into(),
        name: parsed.name.clone(),
        language: parsed.language.clone(),
        rights: parsed.rights.clone(),
        source: "fuzz.xml".into(),
        redistributable: false,
        bundled: false,
    }
}

/// The verse rules every accepted translation meets, whatever its source.
fn check_verses(what: &str, verses: &[(VerseId, String)]) {
    assert!(!verses.is_empty(), "{what}: no verses");
    assert!(verses.len() <= MAX_VERSES, "{what}: verse count");
    assert!(
        verses.windows(2).all(|pair| pair[0].0 < pair[1].0),
        "{what}: verses out of order or repeated"
    );
    for (id, text) in verses {
        assert!(
            !text.is_empty() && text.len() <= MAX_VERSE_BYTES,
            "{what}: verse {id:?} is {} bytes",
            text.len()
        );
        assert!(
            !text.chars().any(char::is_control),
            "{what}: control character in {id:?}"
        );
        assert_eq!(text.trim(), text, "{what}: untrimmed {id:?}");
        assert!(!text.contains("  "), "{what}: uncollapsed {id:?}");
    }
}

/// `store`: also write the result as an installed translation and open it,
/// as the scripture worker does after an import.
fn check(bytes: &[u8], store: bool) {
    let parsed = within(Duration::from_secs(10), "bible import", || {
        import::parse(bytes)
    });
    assert_eq!(import::parse(bytes), parsed, "same input, same result");
    let parsed = match parsed {
        Err(Rejected::TooLarge) => return,
        Err(_) => {
            assert!(bytes.len() <= MAX_FILE_BYTES, "oversized file not refused");
            return;
        }
        Ok(parsed) => parsed,
    };
    assert!(bytes.len() <= MAX_FILE_BYTES, "accepted an oversized file");
    check_verses("import", &parsed.verses);
    assert!(parsed.warnings.len() <= MAX_WARNINGS, "warnings");
    for field in [
        &parsed.name,
        &parsed.abbreviation,
        &parsed.language,
        &parsed.rights,
    ] {
        assert!(field.len() <= MAX_META, "metadata length");
    }
    if store {
        let dir = tempfile::tempdir().unwrap();
        let path = store::path_for(dir.path(), "FUZZ");
        store::write(&path, &info("FUZZ", &parsed), "origin", &parsed.verses)
            .expect("an accepted file can be installed");
        let translation = store::open(&path).expect("an installed file opens");
        assert_eq!(translation.verse_count, parsed.verses.len());
        let (first, last) = (parsed.verses[0].0, parsed.verses.last().unwrap().0);
        let passage = translation.passage(first, last).unwrap();
        let expected = parsed.verses.len().min(store::MAX_PASSAGE + 1);
        assert_eq!(passage.len(), expected);
        assert_eq!(passage[..], parsed.verses[..expected]);
    }
}

const XML_TOKENS: &[&str] = &[
    "<osis>",
    "<osisText>",
    "</osisText>",
    "<div type=\"book\" osisID=\"John\">",
    "</div>",
    "<chapter osisID=\"John.3\">",
    "</chapter>",
    "<verse osisID=\"John.3.16\">",
    "<verse osisID=\"John.3.16 John.3.17\">",
    "<verse osisID=\"Gen.0.0\">",
    "<verse osisID=\"Bible.KJV:Gen.1.1\" sID=\"x\"/>",
    "<verse eID=\"x\"/>",
    "</verse>",
    "<note>n</note>",
    "<title>t</title>",
    "<usfx>",
    "<book id=\"GEN\">",
    "<book id=\"TOB\">",
    "</book>",
    "<c id=\"1\"/>",
    "<c id=\"1000\"/>",
    "<v id=\"1\"/>",
    "<v id=\"1-2\"/>",
    "<ve/>",
    "<f>foot</f>",
    "<XMLBIBLE>",
    "<BIBLEBOOK bnumber=\"1\">",
    "<BIBLEBOOK bnumber=\"67\">",
    "</BIBLEBOOK>",
    "<CHAPTER cnumber=\"1\">",
    "</CHAPTER>",
    "<VERS vnumber=\"1\">",
    "<VERS vnumber=\"65535\">",
    "</VERS>",
    "<NOTE>",
    "<BR/>",
    "<!--",
    "-->",
    "<![CDATA[",
    "]]>",
    "<?pi x?>",
    "<!DOCTYPE x>",
    "&amp;",
    "&#x0;",
    "&#xD800;",
    "&x;",
    "\"",
    "<",
    ">",
    " ",
    "\t",
    "\u{a0}",
    "\u{301}",
    "\u{1f600}",
    "x",
];

fn seeds() -> Vec<Vec<u8>> {
    corpus("bibles").into_iter().map(|(_, b)| b).collect()
}

/// Valid verses in canonical order, as an import or a bundle holds them.
fn verses() -> impl Strategy<Value = Vec<(VerseId, String)>> {
    // Words of printable characters joined by single spaces: what an
    // importer keeps after collapsing whitespace and dropping controls.
    let text =
        proptest::collection::vec("[a-zA-Z0-9,.;:!?'\u{e9}\u{8056}\u{1f600}&<>\"]{1,8}", 1..6)
            .prop_map(|words| words.join(" "));
    let verse = (1u8..=66, 1u16..=150, 1u16..=176, text);
    proptest::collection::vec(verse, 1..12).prop_map(|verses| {
        let mut verses: Vec<(VerseId, String)> = verses
            .into_iter()
            .filter_map(|(book, chapter, verse, text)| {
                Some((
                    VerseId::new(Book::from_number(book)?, chapter, verse)?,
                    text,
                ))
            })
            .collect();
        verses.sort_by_key(|(id, _)| *id);
        verses.dedup_by_key(|(id, _)| *id);
        verses
    })
}

fn usfx(verses: &[(VerseId, String)]) -> String {
    let mut text = String::from("<usfx>");
    let mut book = None;
    let mut chapter = None;
    for (id, verse) in verses {
        if book != Some(id.book) {
            if book.is_some() {
                text += "</book>";
            }
            text += &format!("<book id=\"{}\">", id.book.usfm());
            book = Some(id.book);
            chapter = None;
        }
        if chapter != Some(id.chapter) {
            text += &format!("<c id=\"{}\"/>", id.chapter);
            chapter = Some(id.chapter);
        }
        let escaped = verse
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        text += &format!("<v id=\"{}\"/>{escaped}<ve/>", id.verse);
    }
    text + "</book></usfx>"
}

fn bundle(bytes: Vec<u8>) -> Bundle {
    Bundle {
        id: "FUZZ",
        sha256: bundled::sha256(&bytes),
        bytes: Cow::Owned(bytes),
    }
}

/// A bundle that decodes passes the store's own checks.
fn check_bundle(bytes: Vec<u8>) {
    let decoded = within(Duration::from_secs(10), "bundle decode", || {
        bundled::decode(&bundle(bytes.clone()))
    });
    if let Ok((info, verses)) = decoded {
        assert!(info.bundled);
        assert!(crate::scripture::model::valid_id(&info.id));
        check_verses("bundle", &verses);
        let dir = tempfile::tempdir().unwrap();
        let path = store::path_for(dir.path(), &info.id);
        store::write(&path, &info, "origin", &verses).expect("a decoded bundle installs");
    }
}

fn sample_info() -> TranslationInfo {
    TranslationInfo {
        id: "SYN".into(),
        name: "Synthetic".into(),
        language: "en".into(),
        rights: "Public domain".into(),
        source: "synthetic".into(),
        redistributable: true,
        bundled: true,
    }
}

proptest! {
    #![proptest_config(config(0x5e1a_0b01, 256, "tests/fixtures/fuzz/bibles.proptest-regressions"))]

    #[test]
    fn mutated_bibles_keep_the_import_invariants(bytes in mutated(seeds(), XML_TOKENS)) {
        check(&bytes, false);
    }

    #[test]
    fn token_soups_keep_the_import_invariants(
        tokens in proptest::collection::vec(proptest::sample::select(XML_TOKENS), 0..60),
        root in proptest::sample::select(&["<osis><osisText>", "<usfx>", "<XMLBIBLE>"][..]),
    ) {
        check(format!("{root}{}", tokens.concat()).as_bytes(), false);
    }

    #[test]
    fn arbitrary_bytes_keep_the_import_invariants(
        bytes in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        check(&bytes, false);
    }

    #[test]
    fn generated_usfx_reads_back_exactly(verses in verses()) {
        let parsed = import::parse(usfx(&verses).as_bytes()).unwrap();
        prop_assert_eq!(&parsed.verses, &verses);
        prop_assert!(parsed.warnings.is_empty());
    }

    #[test]
    fn mutated_bundles_decode_to_installable_translations(
        verses in verses(),
        edits in super::edits(&["\n", "\t", "sela-bible 1\n", "id\tX\n", "43003016\t"]),
    ) {
        // Edits on the inflated text exercise the bundle parser; edits on
        // the deflated bytes exercise inflate (both with a matching hash).
        let deflated = bundled::encode(&sample_info(), &verses);
        let text = miniz_oxide::inflate::decompress_to_vec(&deflated).unwrap();
        let edited = super::apply(text, &edits);
        check_bundle(miniz_oxide::deflate::compress_to_vec(&edited, 1));
        check_bundle(super::apply(deflated, &edits));
    }
}

proptest! {
    // Each case writes and opens a SQLite file, so fewer cases.
    #![proptest_config(config(0x5e1a_0b02, 48, "tests/fixtures/fuzz/bibles-store.proptest-regressions"))]

    #[test]
    fn accepted_bibles_install_and_read_back(bytes in mutated(seeds(), XML_TOKENS)) {
        check(&bytes, true);
    }

    #[test]
    fn damaged_translation_files_open_valid_or_not_at_all(
        verses in verses(),
        flips in proptest::collection::vec((any::<proptest::sample::Index>(), 1u8..=255), 1..8),
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = store::path_for(dir.path(), "SYN");
        store::write(&path, &sample_info(), "origin", &verses).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        for (at, mask) in &flips {
            let i = at.index(bytes.len());
            bytes[i] ^= mask;
        }
        std::fs::write(&path, &bytes).unwrap();
        let opened = within(Duration::from_secs(10), "store open", || store::open(&path));
        if let Ok(translation) = opened {
            // What opens is exactly what was written: the content hash
            // covers every verse, and damage elsewhere is refused or repaired.
            prop_assert_eq!(translation.verse_count, verses.len());
            let (first, last) = (verses[0].0, verses.last().unwrap().0);
            prop_assert_eq!(translation.passage(first, last).unwrap(), verses.clone());
        }
    }
}

/// Size bound for the worst cases below: large enough to show quadratic or
/// exponential behavior, small enough for a debug build (the real limit is
/// 48 MiB; `oversized_files_are_refused_before_parsing` covers past it).
const WORST: usize = 8 * 1024 * 1024;

fn fill(prefix: &str, unit: &str, suffix: &str) -> Vec<u8> {
    let room = WORST - prefix.len() - suffix.len();
    format!("{prefix}{}{suffix}", unit.repeat(room / unit.len())).into_bytes()
}

#[test]
fn worst_case_bibles_finish_within_their_time_bound() {
    let osis = "<osis><osisText><div type=\"book\" osisID=\"Gen\"><chapter osisID=\"Gen.1\">";
    for (name, input) in [
        ("open tags", fill("<usfx>", "<p>", "")),
        (
            "nested within the depth limit",
            fill("<usfx>", "<p></p>", "</usfx>"),
        ),
        ("unclosed comment", fill("<usfx><!--", "-", "")),
        ("quoted attribute", fill("<usfx a=\"", ">", "")),
        // A 4-byte unit puts the 4 KiB verse cut right after a space, so
        // `check_verses` sees whether the cut drops it (SEC-11).
        (
            "long verse",
            fill(
                "<usfx><book id=\"GEN\"><c id=\"1\"/><v id=\"1\"/>",
                "and ",
                "<ve/></book></usfx>",
            ),
        ),
        (
            "many milestones",
            fill(
                "<usfx><book id=\"GEN\"><c id=\"1\"/>",
                "<v id=\"1\"/>a<ve/>",
                "</book></usfx>",
            ),
        ),
        (
            "many chapters",
            fill("<usfx><book id=\"PSA\">", "<c id=\"1\"/>", "</book></usfx>"),
        ),
        (
            "skipped books",
            fill(
                "<usfx>",
                "<book id=\"TOB\"><c id=\"1\"/><v id=\"1\"/>a</book>",
                "</usfx>",
            ),
        ),
        (
            "merged osis ids",
            fill(
                &format!("{osis}<verse osisID=\""),
                "Gen.1.1 ",
                "\">a</verse></chapter></div></osisText></osis>",
            ),
        ),
        (
            "osis verses",
            fill(
                osis,
                "<verse osisID=\"Gen.1.1\">a</verse>",
                "</chapter></div></osisText></osis>",
            ),
        ),
        (
            "zefania notes",
            fill(
                "<XMLBIBLE><BIBLEBOOK bnumber=\"1\"><CHAPTER cnumber=\"1\"><VERS vnumber=\"1\">",
                "<NOTE>n</NOTE>",
                "</VERS></CHAPTER></BIBLEBOOK></XMLBIBLE>",
            ),
        ),
        (
            "character references",
            fill(
                "<usfx><book id=\"GEN\"><c id=\"1\"/><v id=\"1\"/>",
                "&#x41;",
                "<ve/></book></usfx>",
            ),
        ),
    ] {
        within(Duration::from_secs(20), name, || check(&input, false));
    }
}

#[test]
fn oversized_files_are_refused_before_parsing() {
    let input = vec![b' '; MAX_FILE_BYTES + 1];
    let parsed = within(Duration::from_millis(500), "oversized", || {
        import::parse(&input)
    });
    assert_eq!(parsed, Err(Rejected::TooLarge));
}

#[test]
fn shipped_bundles_meet_the_bundle_invariants() {
    for shipped in bundled::shipped() {
        let (info, verses) = bundled::decode(&shipped).unwrap();
        assert_eq!(info.id, shipped.id);
        check_verses(shipped.id, &verses);
    }
}

#[test]
fn retained_corpus_replays_cleanly() {
    for (path, bytes) in corpus("bibles") {
        let _ = path;
        check(&bytes, true);
    }
}
