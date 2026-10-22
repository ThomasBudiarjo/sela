//! Song importers: OpenLyrics, OpenSong, ChordPro and EasyWorship RTF words.
//!
//! On top of each parser's own invariant tests (in its module), every input
//! here must also give the same result twice, stay inside the import limits,
//! and, when accepted, convert into a song the library can store
//! (`import::song::to_song`): an accepted file that can never be saved fails
//! the import only after the operator reviewed it.
use super::{config, corpus, files, mutated, within};
use crate::arrangement::MAX_OCCURRENCES;
use crate::import::{
    self, MAX_FILE_BYTES, MAX_ORDER, MAX_SECTIONS, MAX_SLIDES, MAX_WARNINGS, Parsed, ParsedSection,
    ParsedSong, Rejected, Warning, chordpro, ew_rtf, openlyrics, opensong,
};
use crate::storage::song_number_ok;
use proptest::prelude::*;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
enum Format {
    OpenLyrics,
    OpenSong,
    ChordPro,
    EwRtf,
}

impl Format {
    fn parse(self, bytes: &[u8]) -> Parsed {
        match self {
            Self::OpenLyrics => openlyrics::parse(bytes),
            Self::OpenSong => opensong::parse(bytes),
            Self::ChordPro => chordpro::parse(bytes),
            Self::EwRtf => ew_rtf::parse(bytes),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::OpenLyrics => "openlyrics",
            Self::OpenSong => "opensong",
            Self::ChordPro => "chordpro",
            Self::EwRtf => "ew_rtf",
        }
    }

    /// Rejections the format can give for some input.
    fn may_reject(self, rejected: &Rejected) -> bool {
        match self {
            Self::OpenLyrics | Self::OpenSong => true,
            Self::ChordPro => matches!(
                rejected,
                Rejected::NoLyrics | Rejected::InvalidUtf8 | Rejected::TooLarge
            ),
            Self::EwRtf => matches!(
                rejected,
                Rejected::NoLyrics
                    | Rejected::TooLarge
                    | Rejected::TooDeep
                    | Rejected::WrongFormat
                    | Rejected::Malformed(_)
            ),
        }
    }

    /// Seed files: the importer fixtures plus the retained fuzz corpus.
    fn seeds(self) -> Vec<Vec<u8>> {
        files(&format!("import/{}", self.name()))
            .into_iter()
            .chain(corpus(self.name()))
            .map(|(_, bytes)| bytes)
            .collect()
    }
}

/// How many slides `to_song` creates for the parsed order.
fn expanded_order(song: &ParsedSong) -> usize {
    song.order
        .iter()
        .filter_map(|&i| song.sections.get(i))
        .map(|s| s.slides.len())
        .sum()
}

/// The invariants every parse result must keep. Panics with the input's
/// format and a description; proptest shrinks and records the input.
fn check(format: Format, input: &[u8], parsed: &Parsed) {
    let what = format.name();
    assert_eq!(
        &format.parse(input),
        parsed,
        "{what}: same input, same result"
    );
    let song = match parsed {
        Err(rejected) => {
            assert!(
                format.may_reject(rejected),
                "{what}: unexpected rejection {rejected:?}"
            );
            if input.len() > MAX_FILE_BYTES {
                assert_eq!(rejected, &Rejected::TooLarge, "{what}: size limit");
            }
            return;
        }
        Ok(song) => song,
    };
    assert!(
        input.len() <= MAX_FILE_BYTES,
        "{what}: accepted an oversized file"
    );
    assert!(song.sections.len() <= MAX_SECTIONS, "{what}: sections");
    let slides: usize = song.sections.iter().map(|s| s.slides.len()).sum();
    assert!(slides <= MAX_SLIDES, "{what}: slides");
    assert!(song.order.len() <= MAX_ORDER, "{what}: order length");
    assert!(
        song.order.iter().all(|&i| i < song.sections.len()),
        "{what}: order index out of range"
    );
    assert!(song.warnings.len() <= MAX_WARNINGS, "{what}: warnings");
    assert!(song_number_ok(&song.song_number), "{what}: song number");
    // EasyWorship words carry no metadata: the title comes from Songs.db.
    if !matches!(format, Format::EwRtf) {
        assert!(
            !song.title.trim().is_empty() || song.warnings.contains(&Warning::MissingTitle),
            "{what}: an empty title must be reported as MissingTitle"
        );
    }
    for slide in song.sections.iter().flat_map(|s| &s.slides) {
        assert!(!slide.trim().is_empty(), "{what}: empty slide {slide:?}");
        assert!(!slide.contains('\r'), "{what}: CR in slide {slide:?}");
    }
    // The caller's rule: a missing title becomes the file name.
    let mut named = song.clone();
    if named.title.trim().is_empty() {
        named.title = "untitled.file".into();
    }
    assert!(
        expanded_order(song) <= MAX_OCCURRENCES,
        "{what}: expanded order"
    );
    match import::song::to_song(&named) {
        Ok(stored) => assert_eq!(stored.validate(), Ok(()), "{what}: stored song"),
        Err(error) => panic!(
            "{what}: an accepted song cannot be stored ({error:?}); title {} bytes, \
             authors {:?} bytes, copyright {} bytes, administrator {} bytes, {} sections, \
             text {} bytes, labels max {} bytes",
            song.title.len(),
            song.authors.iter().map(String::len).collect::<Vec<_>>(),
            song.copyright.len(),
            song.administrator.len(),
            song.sections.len(),
            song.sections
                .iter()
                .flat_map(|s| &s.slides)
                .map(String::len)
                .sum::<usize>(),
            song.sections
                .iter()
                .map(|s| s.label.len())
                .max()
                .unwrap_or(0),
        ),
    }
}

fn run(format: Format, input: &[u8]) {
    let parsed = within(Duration::from_secs(5), format.name(), || {
        format.parse(input)
    });
    check(format, input, &parsed);
}

const XML_TOKENS: &[&str] = &[
    "<song>",
    "</song>",
    "<song xmlns=\"http://openlyrics.info/namespace/2009/song\">",
    "<lyrics>",
    "</lyrics>",
    "<verse name=\"v1\">",
    "<verse name=\"c\">",
    "<verse>",
    "</verse>",
    "<lines>",
    "</lines>",
    "<br/>",
    "<chord name=\"G\"/>",
    "<comment>x</comment>",
    "<title>",
    "</title>",
    "<properties>",
    "</properties>",
    "<verseOrder>v1 c v1</verseOrder>",
    "<presentation>V1 C V1</presentation>",
    "<author>",
    "</author>",
    "<copyright>",
    "<ccli>12</ccli>",
    "<!--",
    "-->",
    "<![CDATA[",
    "]]>",
    "<?pi x?>",
    "<!DOCTYPE song>",
    "&amp;",
    "&#x41;",
    "&x;",
    "\"",
    "'",
    ">",
    "<",
    "[V1]\n",
    "[C]\n",
    "||",
    "|",
    ".G  C\n",
    "1 ",
    "\n",
    " ",
    "\u{3000}",
    "\u{1f600}",
    "\u{301}",
    "x",
];

const CHORDPRO_TOKENS: &[&str] = &[
    "{title: T}\n",
    "{t:",
    "{soc}\n",
    "{eoc}\n",
    "{sov: Name}\n",
    "{eov}\n",
    "{sob label=\"B\"}\n",
    "{start_of_tab}\n",
    "{end_of_tab}\n",
    "{chorus}\n",
    "{meta: artist A}\n",
    "{subtitle: S}\n",
    "{ccli: 12}\n",
    "{copyright: C}\n",
    "{",
    "}",
    "[G]",
    "[*Coda]",
    "[",
    "]",
    "# c\n",
    "\n",
    "\n\n",
    " ",
    "word ",
    "\u{1f600}",
    ":",
];

const RTF_TOKENS: &[&str] = &[
    "{",
    "}",
    "\\par ",
    "\\line ",
    "\\pard ",
    "\\sdslidemarker ",
    "\\sdparawysiwghidden ",
    "\\sdeasyworship2 ",
    "\\u-10179?",
    "\\u8364 ",
    "\\uc2 ",
    "\\uc0 ",
    "\\'e9",
    "\\'",
    "\\*",
    "{\\fonttbl{\\f0 Tahoma;}}",
    "\\f1 ",
    "\\fcharset204 ",
    "\\ansicpg1251 ",
    "\\bin2 ",
    "\\bin99999999 ",
    "\\b ",
    "\\",
    "\\u99999999999 ",
    "\\uc99999999999 ",
    "Verse 1",
    "\r\n",
];

proptest! {
    #![proptest_config(config(0x5e1a_0101, 256, "tests/fixtures/fuzz/songs.proptest-regressions"))]

    #[test]
    fn openlyrics_mutations_keep_the_import_invariants(
        bytes in mutated(Format::OpenLyrics.seeds(), XML_TOKENS),
    ) {
        run(Format::OpenLyrics, &bytes);
    }

    #[test]
    fn opensong_mutations_keep_the_import_invariants(
        bytes in mutated(Format::OpenSong.seeds(), XML_TOKENS),
    ) {
        run(Format::OpenSong, &bytes);
    }

    #[test]
    fn chordpro_mutations_keep_the_import_invariants(
        bytes in mutated(Format::ChordPro.seeds(), CHORDPRO_TOKENS),
    ) {
        run(Format::ChordPro, &bytes);
    }

    #[test]
    fn ew_rtf_mutations_keep_the_import_invariants(
        bytes in mutated(Format::EwRtf.seeds(), RTF_TOKENS),
    ) {
        run(Format::EwRtf, &bytes);
    }

    #[test]
    fn arbitrary_bytes_keep_the_import_invariants(
        bytes in proptest::collection::vec(any::<u8>(), 0..1024),
    ) {
        for format in [Format::OpenLyrics, Format::OpenSong, Format::ChordPro, Format::EwRtf] {
            run(format, &bytes);
        }
    }
}

/// Words for generated lyrics: no markup characters of any format.
fn line() -> impl Strategy<Value = String> {
    proptest::collection::vec("[a-zA-Z0-9',.!?\u{e9}\u{8056}]{1,8}", 1..5)
        .prop_map(|words| words.join(" "))
}

fn slides() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(
        proptest::collection::vec(line(), 1..4).prop_map(|lines| lines.join("\n")),
        1..4,
    )
}

/// Distinct section codes with the label each maps to, and slides.
fn coded_sections() -> impl Strategy<Value = Vec<(String, ParsedSection)>> {
    proptest::sample::subsequence(
        vec![
            ("v1", "Verse 1"),
            ("v2", "Verse 2"),
            ("v3", "Verse 3"),
            ("c", "Chorus"),
            ("c2", "Chorus 2"),
            ("b", "Bridge"),
            ("p", "Pre-Chorus"),
            ("e", "Ending"),
        ],
        1..6,
    )
    .prop_flat_map(|codes| {
        let n = codes.len();
        (Just(codes), proptest::collection::vec(slides(), n))
    })
    .prop_map(|(codes, slides)| {
        codes
            .into_iter()
            .zip(slides)
            .map(|((code, label), slides)| {
                (
                    code.to_string(),
                    ParsedSection {
                        label: label.to_string(),
                        slides,
                    },
                )
            })
            .collect()
    })
}

fn order_of(len: usize) -> impl Strategy<Value = Vec<usize>> {
    proptest::collection::vec(0..len, 0..8)
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn openlyrics_file(title: &str, sections: &[(String, ParsedSection)], order: &[usize]) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<song xmlns=\"http://openlyrics.info/namespace/2009/song\" version=\"0.9\">\n<properties><titles><title>{}</title></titles>",
        xml_escape(title)
    );
    if !order.is_empty() {
        let codes: Vec<&str> = order.iter().map(|&i| sections[i].0.as_str()).collect();
        xml += &format!("<verseOrder>{}</verseOrder>", codes.join(" "));
    }
    xml += "</properties>\n<lyrics>\n";
    for (code, section) in sections {
        xml += &format!("<verse name=\"{code}\">");
        for slide in &section.slides {
            let lines: Vec<String> = slide.split('\n').map(xml_escape).collect();
            xml += &format!("<lines>{}</lines>", lines.join("<br/>"));
        }
        xml += "</verse>\n";
    }
    xml + "</lyrics>\n</song>\n"
}

fn opensong_file(title: &str, sections: &[(String, ParsedSection)], order: &[usize]) -> String {
    let codes: Vec<String> = order
        .iter()
        .map(|&i| sections[i].0.to_uppercase())
        .collect();
    let mut lyrics = String::new();
    for (code, section) in sections {
        lyrics += &format!("[{}]\n", code.to_uppercase());
        for (i, slide) in section.slides.iter().enumerate() {
            if i > 0 {
                lyrics += " ||\n";
            }
            for line in slide.split('\n') {
                lyrics += &format!(" {line}\n");
            }
        }
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<song><title>{}</title><presentation>{}</presentation><lyrics>{}</lyrics></song>\n",
        xml_escape(title),
        codes.join(" "),
        xml_escape(&lyrics)
    )
}

fn chordpro_file(title: &str, sections: &[ParsedSection]) -> String {
    let mut text = format!("{{title: {title}}}\n");
    for section in sections {
        text += &format!("{{start_of_verse: {}}}\n", section.label);
        text += &section.slides.join("\n\n");
        text += "\n{end_of_verse}\n\n";
    }
    text
}

proptest! {
    #![proptest_config(config(0x5e1a_0102, 128, "tests/fixtures/fuzz/songs-round-trip.proptest-regressions"))]

    #[test]
    fn openlyrics_written_songs_read_back(
        title in line(),
        (sections, order) in coded_sections().prop_flat_map(|s| {
            let n = s.len();
            (Just(s), order_of(n))
        }),
    ) {
        let xml = openlyrics_file(&title, &sections, &order);
        let parsed = openlyrics::parse(xml.as_bytes()).unwrap();
        check(Format::OpenLyrics, xml.as_bytes(), &Ok(parsed.clone()));
        let expected: Vec<ParsedSection> = sections.into_iter().map(|(_, s)| s).collect();
        prop_assert_eq!(&parsed.title, &title);
        prop_assert_eq!(&parsed.sections, &expected);
        prop_assert_eq!(&parsed.order, &order);
        prop_assert_eq!(&parsed.warnings, &Vec::<Warning>::new());
    }

    #[test]
    fn opensong_written_songs_read_back(
        title in line(),
        (sections, order) in coded_sections().prop_flat_map(|s| {
            let n = s.len();
            (Just(s), order_of(n))
        }),
    ) {
        let xml = opensong_file(&title, &sections, &order);
        let parsed = opensong::parse(xml.as_bytes()).unwrap();
        check(Format::OpenSong, xml.as_bytes(), &Ok(parsed.clone()));
        let expected: Vec<ParsedSection> = sections.into_iter().map(|(_, s)| s).collect();
        prop_assert_eq!(&parsed.title, &title);
        prop_assert_eq!(&parsed.sections, &expected);
        prop_assert_eq!(&parsed.order, &order);
        prop_assert_eq!(&parsed.warnings, &Vec::<Warning>::new());
    }

    #[test]
    fn chordpro_written_songs_read_back(
        title in line(),
        sections in proptest::collection::vec(
            ("[A-Z][a-z]{1,8}( [0-9])?", slides())
                .prop_map(|(label, slides)| ParsedSection { label, slides }),
            1..6,
        ),
    ) {
        let text = chordpro_file(&title, &sections);
        let parsed = chordpro::parse(text.as_bytes()).unwrap();
        check(Format::ChordPro, text.as_bytes(), &Ok(parsed.clone()));
        prop_assert_eq!(&parsed.title, &title);
        prop_assert_eq!(&parsed.sections, &sections);
        prop_assert_eq!(&parsed.order, &Vec::<usize>::new());
        prop_assert_eq!(&parsed.warnings, &Vec::<Warning>::new());
    }
}

/// Repeats `unit` until the input is just under the 1 MiB import limit.
fn fill(prefix: &str, unit: &str, suffix: &str) -> Vec<u8> {
    let room = MAX_FILE_BYTES - prefix.len() - suffix.len();
    let mut text = String::with_capacity(MAX_FILE_BYTES);
    text += prefix;
    text += &unit.repeat(room / unit.len());
    text += suffix;
    assert!(text.len() <= MAX_FILE_BYTES);
    text.into_bytes()
}

const OPENLYRICS_HEAD: &str = "<song xmlns=\"http://openlyrics.info/namespace/2009/song\"><properties><titles><title>T</title></titles></properties><lyrics><verse name=\"v1\"><lines>";
const OPENLYRICS_TAIL: &str = "</lines></verse></lyrics></song>";

/// Worst-case inputs at the size limit: each must finish within the bound
/// and keep the invariants (bounded time and memory, no stack overflow).
fn worst_cases() -> Vec<(Format, &'static str, Vec<u8>)> {
    vec![
        (
            Format::OpenLyrics,
            "long slide",
            fill(OPENLYRICS_HEAD, "word ", OPENLYRICS_TAIL),
        ),
        (
            Format::OpenLyrics,
            "line breaks",
            fill(OPENLYRICS_HEAD, "a<br/>", OPENLYRICS_TAIL),
        ),
        (
            Format::OpenLyrics,
            "many verses",
            fill(
                "<song xmlns=\"http://openlyrics.info/namespace/2009/song\"><lyrics>",
                "<verse name=\"v1\"><lines>a</lines></verse>",
                "</lyrics></song>",
            ),
        ),
        (Format::OpenLyrics, "open tags", fill("", "<a>", "")),
        (
            Format::OpenLyrics,
            "unclosed comment",
            fill("<song><!--", "-", ""),
        ),
        (
            Format::OpenLyrics,
            "quoted attribute",
            fill("<song a=\"", ">", ""),
        ),
        (
            Format::OpenLyrics,
            "long title",
            fill(
                "<song xmlns=\"http://openlyrics.info/namespace/2009/song\"><properties><titles><title>",
                "x",
                "</title></titles></properties><lyrics><verse name=\"v1\"><lines>a</lines></verse></lyrics></song>",
            ),
        ),
        (
            Format::OpenSong,
            "long lyrics",
            fill(
                "<song><title>T</title><lyrics>[V1]\n",
                " word word word\n",
                "</lyrics></song>",
            ),
        ),
        (
            Format::OpenSong,
            "many tags",
            fill(
                "<song><title>T</title><lyrics>",
                "[V1]\n a\n",
                "</lyrics></song>",
            ),
        ),
        (
            Format::OpenSong,
            "numbered lines",
            fill("<song><lyrics>[V]\n", "1 a\n2 b\n", "</lyrics></song>"),
        ),
        (
            Format::OpenSong,
            "slide breaks",
            fill("<song><lyrics>[V1]\n a", "||a", "</lyrics></song>"),
        ),
        (
            Format::OpenSong,
            "long presentation",
            fill(
                "<song><lyrics>[V1]\n a\n</lyrics><presentation>",
                "V1 ",
                "</presentation></song>",
            ),
        ),
        (
            Format::OpenSong,
            "long title",
            fill(
                "<song><title>",
                "x",
                "</title><lyrics>[V1]\n a\n</lyrics></song>",
            ),
        ),
        (Format::OpenSong, "open tags", fill("", "<a>", "")),
        (
            Format::ChordPro,
            "long verse",
            fill("{title: T}\n", "word word word\n", ""),
        ),
        (Format::ChordPro, "chords", fill("", "[G]", "a")),
        (Format::ChordPro, "open brackets", fill("", "[", "a")),
        (Format::ChordPro, "nested brackets", fill("", "[[", "a")),
        (Format::ChordPro, "many verses", fill("", "a\n\n", "")),
        (
            Format::ChordPro,
            "chorus repeats",
            fill("{soc}\na\n{eoc}\n", "{chorus}\n", ""),
        ),
        (Format::ChordPro, "directives", fill("a\n", "{x}\n", "")),
        (
            Format::ChordPro,
            "long title",
            fill("{title: ", "x", "}\na\n"),
        ),
        (
            Format::ChordPro,
            "long copyright",
            fill("{copyright: ", "x", "}\na\n"),
        ),
        (Format::EwRtf, "open groups", fill("{\\rtf1 ", "{", "a")),
        (
            Format::EwRtf,
            "paragraphs",
            fill("{\\rtf1 ", "a\\par ", "}"),
        ),
        (
            Format::EwRtf,
            "slide markers",
            fill("{\\rtf1 \\sdeasyworship2 ", "a\\sdslidemarker\\par ", "}"),
        ),
        (
            Format::EwRtf,
            "unicode escapes",
            fill("{\\rtf1 ", "\\u8364 ", "}"),
        ),
        (Format::EwRtf, "hex escapes", fill("{\\rtf1 ", "\\'e9", "}")),
        (
            Format::EwRtf,
            "control words",
            fill("{\\rtf1 ", "\\b ", "a}"),
        ),
        (
            Format::EwRtf,
            "long control word",
            fill("{\\rtf1 \\", "a", "}"),
        ),
        (
            Format::EwRtf,
            "long number",
            fill("{\\rtf1 \\u", "9", " a}"),
        ),
        (
            Format::EwRtf,
            "skipped groups",
            fill("{\\rtf1 a", "{\\*\\x b}", "}"),
        ),
    ]
}

#[test]
fn worst_case_inputs_finish_within_their_time_bound() {
    let mut failed = Vec::new();
    for (format, name, input) in worst_cases() {
        let case = format!("{} {name}", format.name());
        let outcome = std::panic::catch_unwind(|| {
            let parsed = within(Duration::from_secs(20), &case, || format.parse(&input));
            check(format, &input, &parsed);
        });
        if outcome.is_err() {
            failed.push(case);
        }
    }
    assert!(failed.is_empty(), "worst cases failed: {failed:?}");
}

#[test]
fn oversized_inputs_are_refused_before_parsing() {
    let input = vec![b'a'; MAX_FILE_BYTES + 1];
    for format in [
        Format::OpenLyrics,
        Format::OpenSong,
        Format::ChordPro,
        Format::EwRtf,
    ] {
        let parsed = within(Duration::from_millis(500), format.name(), || {
            format.parse(&input)
        });
        assert_eq!(parsed, Err(Rejected::TooLarge), "{}", format.name());
    }
}

#[test]
fn retained_corpus_replays_cleanly() {
    let formats = [
        Format::OpenLyrics,
        Format::OpenSong,
        Format::ChordPro,
        Format::EwRtf,
    ];
    for format in formats {
        let seeds = files(&format!("import/{}", format.name()))
            .into_iter()
            .chain(corpus(format.name()));
        for (path, bytes) in seeds {
            // Every format sees every file: a file of one format is hostile
            // input to the others.
            for other in formats {
                let parsed = within(Duration::from_secs(5), &path.display().to_string(), || {
                    other.parse(&bytes)
                });
                check(other, &bytes, &parsed);
            }
        }
    }
}
