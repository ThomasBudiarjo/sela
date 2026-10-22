//! OpenSong XML song parser (M4-07). A `<song>` root holds metadata elements
//! and a `<lyrics>` text whose lines are typed by their first character; the
//! line rules are in docs/import.md ("OpenSong and ChordPro").
use std::collections::HashMap;

use super::{
    MAX_ORDER, MAX_SECTIONS, MAX_SLIDES, Parsed, ParsedSection, ParsedSong, Rejected, Warning,
    decode, label_for_code,
};

/// roxmltree parses nested elements recursively, so nesting is bounded by a
/// scan of the text before the parse instead of a check on the tree.
const MAX_DEPTH: usize = 32;
const NODES_LIMIT: u32 = 100_000;

pub fn parse(bytes: &[u8]) -> Parsed {
    let text = decode(bytes)?;
    if nesting_depth(&text) > MAX_DEPTH {
        return Err(Rejected::TooDeep);
    }
    let options = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: NODES_LIMIT,
        entity_resolver: None,
    };
    let doc = roxmltree::Document::parse_with_options(&text, options).map_err(|e| match e {
        roxmltree::Error::DtdDetected => Rejected::Entities,
        roxmltree::Error::NodesLimitReached => Rejected::TooLarge,
        e => Rejected::Malformed(e.to_string()),
    })?;
    let root = doc.root_element();
    if root.tag_name().name() != "song" {
        return Err(Rejected::WrongFormat);
    }

    let mut song = ParsedSong::default();
    let mut presentation = String::new();
    let mut lyrics = String::new();
    for field in root.children().filter(|n| n.is_element()) {
        let value: String = field
            .descendants()
            .filter_map(|n| n.is_text().then(|| n.text()).flatten())
            .collect();
        match field.tag_name().name() {
            "title" => song.title = value.trim().to_string(),
            "author" => song.authors = split_authors(&value),
            "copyright" => song.copyright = value.trim().to_string(),
            "ccli" if crate::storage::song_number_ok(value.trim()) => {
                song.song_number = value.trim().to_string();
            }
            "ccli" => warn(&mut song, Warning::InvalidField("ccli".into())),
            "presentation" => presentation = value,
            "lyrics" => lyrics = value,
            name if !value.trim().is_empty() => {
                warn(&mut song, Warning::UnknownElement(name.to_string()));
            }
            _ => {}
        }
    }

    let names = build_sections(&lyrics, &mut song);
    if song.sections.is_empty() {
        return Err(Rejected::NoLyrics);
    }
    'order: for token in presentation.split_whitespace() {
        match names.get(&token.to_ascii_uppercase()) {
            Some(Target::Sections(indices)) => {
                for &index in indices {
                    if song.order.len() >= MAX_ORDER {
                        warn(&mut song, Warning::LimitReached);
                        break 'order;
                    }
                    song.order.push(index);
                }
            }
            Some(Target::Reported) => {}
            None => warn(&mut song, Warning::MissingOrderReference(token.to_string())),
        }
    }
    if song.title.is_empty() {
        warn(&mut song, Warning::MissingTitle);
    }
    Ok(song)
}

/// What a presentation name plays.
enum Target {
    /// These sections, in order: one, or a bare tag's numbered verses.
    Sections(Vec<usize>),
    /// Nothing: the name's sections were already reported as `EmptySection`
    /// or cut by `LimitReached`.
    Reported,
}

/// One `[tag]` group, or one verse number under a tag, before limits apply.
struct Block {
    /// The name a presentation order uses, such as "V1".
    name: String,
    /// Lines of each slide; `||` starts the next slide.
    slides: Vec<Vec<String>>,
    /// The text before the first tag, which has no name of its own when it
    /// is empty.
    lead: bool,
    /// Indices into the block list of this tag's numbered verses.
    numbered: Vec<usize>,
}

impl Block {
    fn new(name: String) -> Self {
        Block {
            name,
            slides: vec![Vec::new()],
            lead: false,
            numbered: Vec::new(),
        }
    }

    fn push(&mut self, text: &str) {
        if !text.contains('|') {
            self.line(text);
            return;
        }
        for (i, page) in text.split("||").enumerate() {
            if i > 0 {
                self.slides.push(Vec::new());
            }
            for part in page.split('|').map(str::trim).filter(|p| !p.is_empty()) {
                self.line(part);
            }
        }
    }

    fn line(&mut self, text: &str) {
        if let Some(lines) = self.slides.last_mut() {
            lines.push(text.trim().to_string());
        }
    }

    fn slides(&self) -> Vec<String> {
        self.slides
            .iter()
            .map(|lines| {
                let start = lines.iter().position(|l| !l.is_empty());
                let end = lines.iter().rposition(|l| !l.is_empty());
                match (start, end) {
                    (Some(start), Some(end)) => lines[start..=end].join("\n"),
                    _ => String::new(),
                }
            })
            .filter(|slide| !slide.is_empty())
            .collect()
    }
}

/// Splits the lyrics into `song.sections` and returns what each source name
/// (upper case) plays.
fn build_sections(lyrics: &str, song: &mut ParsedSong) -> HashMap<String, Target> {
    let mut lead = Block::new("V".into());
    lead.lead = true;
    let mut blocks = vec![lead];
    let mut tag = 0;
    let mut numbered: HashMap<u32, usize> = HashMap::new();
    for line in lyrics.lines() {
        let digits = line.bytes().take_while(u8::is_ascii_digit).count();
        match (line.as_bytes().first(), line[..digits].parse::<u32>()) {
            (Some(b'['), _) => {
                let (name, rest) = line[1..].split_once(']').unwrap_or((&line[1..], ""));
                let name = name.trim();
                let name = if name.is_empty() { "V" } else { name };
                blocks.push(Block::new(name.to_string()));
                tag = blocks.len() - 1;
                numbered.clear();
                if !rest.trim().is_empty() {
                    blocks[tag].push(rest);
                }
            }
            (Some(b'.'), _) => warn(song, Warning::ChordsRemoved),
            (Some(b';'), _) => {}
            (_, Ok(number)) => {
                let index = *numbered.entry(number).or_insert_with(|| {
                    blocks.push(Block::new(numbered_name(&blocks[tag].name, number)));
                    let index = blocks.len() - 1;
                    blocks[tag].numbered.push(index);
                    index
                });
                blocks[index].push(&line[digits..]);
            }
            _ => blocks[tag].push(line),
        }
    }

    // `None` for a block that is empty or after the limit cut.
    let mut section_of: Vec<Option<usize>> = Vec::new();
    let mut slide_count = 0;
    for block in &blocks {
        let label = label_for(&block.name);
        let mut slides = block.slides();
        if slides.is_empty() {
            if !block.lead && block.numbered.is_empty() {
                warn(song, Warning::EmptySection(label));
            }
            section_of.push(None);
            continue;
        }
        let room = MAX_SLIDES - slide_count;
        if song.sections.len() >= MAX_SECTIONS || room == 0 {
            warn(song, Warning::LimitReached);
            break;
        }
        if slides.len() > room {
            slides.truncate(room);
            warn(song, Warning::LimitReached);
        }
        slide_count += slides.len();
        song.sections.push(ParsedSection { label, slides });
        section_of.push(Some(song.sections.len() - 1));
    }

    let mut names = HashMap::new();
    for (block, section) in blocks.iter().zip(&section_of) {
        let sections: Vec<usize> = match section {
            Some(index) => vec![*index],
            None => block
                .numbered
                .iter()
                .filter_map(|&i| section_of.get(i).copied().flatten())
                .collect(),
        };
        let target = if !sections.is_empty() {
            Target::Sections(sections)
        } else if block.lead {
            continue;
        } else {
            Target::Reported
        };
        let entry = names
            .entry(block.name.to_ascii_uppercase())
            .or_insert(Target::Reported);
        if matches!(entry, Target::Reported) {
            *entry = target;
        }
    }
    names
}

/// `[V]` with lines numbered 1 and 2 holds verses "V1" and "V2".
fn numbered_name(tag: &str, number: u32) -> String {
    if tag.len() == 1 && tag.as_bytes()[0].is_ascii_alphabetic() {
        format!("{tag}{number}")
    } else {
        format!("{tag} {number}")
    }
}

/// A tag that is one letter and optional digits ("V1", "C", "B2") is a code
/// for `label_for_code`; any other tag ("Coda", "Outro 2") is kept as written,
/// because `label_for_code` reads only the first letter.
fn label_for(name: &str) -> String {
    let mut chars = name.chars();
    let is_code =
        chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_digit());
    is_code
        .then(|| label_for_code(name))
        .flatten()
        .unwrap_or_else(|| name.to_string())
}

/// OpenSong keeps every writer in one `<author>` text. Names are split at
/// `,` `;` `&` `/` `|` and at the word "and"; "Smith, John" therefore becomes
/// two names, which the staged preview shows for correction.
fn split_authors(text: &str) -> Vec<String> {
    let mut authors = Vec::new();
    for piece in text.split([',', ';', '&', '/', '|']) {
        let words: Vec<&str> = piece.split_whitespace().collect();
        for name in words.split(|word| word.eq_ignore_ascii_case("and")) {
            if !name.is_empty() {
                authors.push(name.join(" "));
            }
        }
    }
    authors
}

fn warn(song: &mut ParsedSong, warning: Warning) {
    if !song.warnings.contains(&warning) {
        song.warn(warning);
    }
}

/// Deepest element nesting in `text`, counted the way roxmltree descends:
/// comments, CDATA and processing instructions are skipped, and a `>` inside
/// a quoted attribute value does not end its tag. Malformed text gets some
/// count and is rejected by the parse that follows.
fn nesting_depth(text: &str) -> usize {
    let bytes = text.as_bytes();
    let (mut i, mut depth, mut deepest) = (0, 0usize, 0);
    while let Some(offset) = bytes[i..].iter().position(|&b| b == b'<') {
        i += offset;
        let rest = &bytes[i..];
        let skip_past = |end: &[u8]| {
            rest.windows(end.len())
                .position(|w| w == end)
                .map_or(bytes.len(), |p| i + p + end.len())
        };
        if rest.starts_with(b"<!--") {
            i = skip_past(b"-->");
        } else if rest.starts_with(b"<![CDATA[") {
            i = skip_past(b"]]>");
        } else if rest.starts_with(b"<?") {
            i = skip_past(b"?>");
        } else if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
            i = skip_past(b">");
        } else {
            let mut quote = None;
            let mut end = bytes.len();
            for (j, &b) in rest.iter().enumerate().skip(1) {
                match (quote, b) {
                    (None, b'"' | b'\'') => quote = Some(b),
                    (Some(q), _) if q == b => quote = None,
                    (None, b'>') => {
                        end = i + j;
                        break;
                    }
                    _ => {}
                }
            }
            if end == bytes.len() || bytes[end - 1] != b'/' {
                depth += 1;
                deepest = deepest.max(depth);
            }
            i = (end + 1).min(bytes.len());
        }
    }
    deepest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::MAX_FILE_BYTES;
    use proptest::prelude::*;
    use proptest::test_runner::FileFailurePersistence;

    const MORNING_RIVER: &[u8] =
        include_bytes!("../../tests/fixtures/import/opensong/morning-river.xml");
    const NUMBERED: &[u8] =
        include_bytes!("../../tests/fixtures/import/opensong/numbered-verses.xml");

    fn song_xml(inner: &str) -> Vec<u8> {
        format!("<song><title>T</title>{inner}</song>").into_bytes()
    }

    fn lyrics(text: &str) -> ParsedSong {
        parse(&song_xml(&format!("<lyrics>{text}</lyrics>"))).unwrap()
    }

    fn section(label: &str, slides: &[&str]) -> ParsedSection {
        ParsedSection {
            label: label.into(),
            slides: slides.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn sorted(warnings: Vec<Warning>) -> Vec<String> {
        let mut out: Vec<String> = warnings.iter().map(|w| format!("{w:?}")).collect();
        out.sort();
        out
    }

    #[test]
    fn a_tagged_song_keeps_sections_order_and_metadata() {
        let song = parse(MORNING_RIVER).unwrap();
        assert_eq!(song.title, "Morning River");
        assert_eq!(song.authors, ["Ana Lestari", "Ben Hart"]);
        assert_eq!(song.copyright, "2026 Sela Test Fixtures");
        assert_eq!(song.song_number, "7654321");
        assert_eq!(
            song.sections,
            [
                section(
                    "Verse 1",
                    &["The morning river runs so clear\nIt sings that daybreak has come near"]
                ),
                section(
                    "Chorus",
                    &["Rise up, rise up, the light is here\nRise up and sing"]
                ),
                section(
                    "Verse 2",
                    &["The evening lantern glows so warm\nIt keeps us safe from every storm"]
                ),
            ]
        );
        assert_eq!(song.order, [0, 1, 2, 1]);
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![
                Warning::ChordsRemoved,
                Warning::UnknownElement("key".into())
            ])
        );
    }

    #[test]
    fn numbered_lines_become_verses_and_bars_split_slides_and_lines() {
        let song = parse(NUMBERED).unwrap();
        assert_eq!(song.authors, ["Dewi Hartono", "Sam Okoro", "Lee Park"]);
        assert_eq!(
            song.sections,
            [
                section(
                    "Verse 1",
                    &["First verse opens the day\nWe walk the lane together"]
                ),
                section(
                    "Verse 2",
                    &["Second verse closes the night\nWe rest beneath the stars"]
                ),
                section("Chorus", &["Sing it out", "Sing again\nHold on\nhold fast"]),
            ]
        );
        assert_eq!(song.order, [0, 2, 1, 2]);
        assert_eq!(song.warnings, [Warning::ChordsRemoved]);
    }

    #[test]
    fn tags_map_to_sela_labels_and_custom_tags_stay_verbatim() {
        let song = lyrics("[B]\n a\n[P2]\n b\n[T]\n c\n[Coda]\n d\n[X1]\n e");
        let labels: Vec<&str> = song.sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["Bridge", "Pre-Chorus 2", "Tag", "Coda", "X1"]);
    }

    #[test]
    fn lyrics_before_any_tag_form_a_verse() {
        let song = lyrics("\n Opening words\n[C]\n Chorus words");
        assert_eq!(
            song.sections,
            [
                section("Verse", &["Opening words"]),
                section("Chorus", &["Chorus words"])
            ]
        );
    }

    #[test]
    fn comments_chords_and_blank_edges_are_dropped() {
        let song = lyrics("[V1]\n;note to the band\n\n.G  C\n Line one\n\n Line two\n\n");
        assert_eq!(
            song.sections,
            [section("Verse 1", &["Line one\n\nLine two"])]
        );
        assert_eq!(song.warnings, [Warning::ChordsRemoved]);
    }

    #[test]
    fn an_unknown_order_tag_is_reported_and_skipped() {
        let xml =
            song_xml("<presentation>V1 Z9 c</presentation><lyrics>[V1]\n a\n[C]\n b</lyrics>");
        let song = parse(&xml).unwrap();
        assert_eq!(song.order, [0, 1]);
        assert_eq!(song.warnings, [Warning::MissingOrderReference("Z9".into())]);
    }

    #[test]
    fn an_order_name_with_no_section_is_reported_even_if_a_hidden_block_had_it() {
        let xml = song_xml("<presentation>V C</presentation><lyrics>[V1]\n a\n[C]\n b</lyrics>");
        let song = parse(&xml).unwrap();
        assert_eq!(song.order, [1]);
        assert_eq!(song.warnings, [Warning::MissingOrderReference("V".into())]);
    }

    #[test]
    fn a_bare_tag_in_the_order_plays_its_numbered_verses() {
        let xml = song_xml(
            "<presentation>V C V2</presentation><lyrics>[V]\n1 a\n2 b\n[C]\n c</lyrics>",
        );
        let song = parse(&xml).unwrap();
        assert_eq!(
            song.sections,
            [
                section("Verse 1", &["a"]),
                section("Verse 2", &["b"]),
                section("Chorus", &["c"])
            ]
        );
        assert_eq!(song.order, [0, 1, 2, 1]);
        assert!(song.warnings.is_empty());
    }

    #[test]
    fn unnumbered_lines_under_a_numbered_tag_come_first_and_own_the_bare_name() {
        let xml = song_xml("<presentation>V2 V</presentation><lyrics>[V]\n1 a\n plain\n2 b</lyrics>");
        let song = parse(&xml).unwrap();
        assert_eq!(
            song.sections,
            [
                section("Verse", &["plain"]),
                section("Verse 1", &["a"]),
                section("Verse 2", &["b"])
            ]
        );
        assert_eq!(song.order, [2, 0]);
    }

    #[test]
    fn text_after_a_tag_is_a_lyric_line() {
        let song = lyrics("[V1] Sung first | then this\n a\n[C]x");
        assert_eq!(
            song.sections,
            [
                section("Verse 1", &["Sung first\nthen this\na"]),
                section("Chorus", &["x"])
            ]
        );
    }

    #[test]
    fn an_empty_tag_is_reported_and_not_an_order_error() {
        let xml = song_xml(
            "<presentation>V1 C V1</presentation><lyrics>[V1]\n a\n[C]\n;only a note</lyrics>",
        );
        let song = parse(&xml).unwrap();
        assert_eq!(song.sections, [section("Verse 1", &["a"])]);
        assert_eq!(song.order, [0, 0]);
        assert_eq!(song.warnings, [Warning::EmptySection("Chorus".into())]);
    }

    #[test]
    fn missing_title_bad_ccli_and_unmapped_fields_are_warnings() {
        let xml =
            b"<song><ccli>CCLI 123</ccli><hymn_number>12</hymn_number><lyrics> a</lyrics></song>";
        let song = parse(xml).unwrap();
        assert_eq!(song.title, "");
        assert_eq!(song.song_number, "");
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![
                Warning::InvalidField("ccli".into()),
                Warning::UnknownElement("hymn_number".into()),
                Warning::MissingTitle,
            ])
        );
    }

    #[test]
    fn crlf_bom_and_non_ascii_text_parse_like_plain_lf() {
        let lf = "<song><title>Lagu Pagi</title><lyrics>[V1]\n Kasih-Mu ajaib, señor\n 光の歌\n[C]\n Haleluya</lyrics></song>";
        let mut crlf = b"\xEF\xBB\xBF".to_vec();
        crlf.extend_from_slice(lf.replace('\n', "\r\n").as_bytes());
        let song = parse(lf.as_bytes()).unwrap();
        assert_eq!(parse(&crlf).unwrap(), song);
        assert_eq!(song.sections[0].slides, ["Kasih-Mu ajaib, señor\n光の歌"]);
    }

    #[test]
    fn hostile_or_foreign_files_are_rejected() {
        let xxe = b"<?xml version=\"1.0\"?><!DOCTYPE song [<!ENTITY x SYSTEM \"file:///C:/Windows/win.ini\">]><song><lyrics>&x;</lyrics></song>";
        assert_eq!(parse(xxe), Err(Rejected::Entities));
        let laughs = b"<!DOCTYPE song [<!ENTITY a \"aaaa\"><!ENTITY b \"&a;&a;&a;&a;\">]><song><lyrics>&b;</lyrics></song>";
        assert_eq!(parse(laughs), Err(Rejected::Entities));
        assert!(matches!(
            parse(b"<song><lyrics> a</song>"),
            Err(Rejected::Malformed(_))
        ));
        assert!(matches!(parse(b""), Err(Rejected::Malformed(_))));
        assert_eq!(
            parse(b"<openlyrics><lyrics> a</lyrics></openlyrics>"),
            Err(Rejected::WrongFormat)
        );
        assert_eq!(
            parse(&song_xml("<lyrics>[V1]\n.G\n;x</lyrics>")),
            Err(Rejected::NoLyrics)
        );
        assert_eq!(
            parse(b"<song><title>T</title></song>"),
            Err(Rejected::NoLyrics)
        );
        assert_eq!(parse(b"\xff<song/>"), Err(Rejected::InvalidUtf8));
    }

    #[test]
    fn oversized_deep_and_node_heavy_files_are_rejected() {
        let mut big = song_xml("<lyrics> a</lyrics>");
        big.resize(MAX_FILE_BYTES + 1, b' ');
        assert_eq!(parse(&big), Err(Rejected::TooLarge));

        let deep = format!(
            "<song>{}{}</song>",
            "<a>".repeat(100_000),
            "</a>".repeat(100_000)
        );
        assert_eq!(parse(deep.as_bytes()), Err(Rejected::TooDeep));
        let at_cap = format!(
            "<song><lyrics> a</lyrics>{}{}</song>",
            "<a x='>'>".repeat(MAX_DEPTH - 1),
            "</a>".repeat(MAX_DEPTH - 1)
        );
        assert!(parse(at_cap.as_bytes()).is_ok());
        let commented = format!(
            "<song><!--{}--><lyrics> a</lyrics></song>",
            "<a>".repeat(100)
        );
        assert!(parse(commented.as_bytes()).is_ok());

        let wide = format!("<song><lyrics> a</lyrics>{}</song>", "<a/>".repeat(150_000));
        assert_eq!(parse(wide.as_bytes()), Err(Rejected::TooLarge));
    }

    #[test]
    fn section_slide_and_order_limits_hold() {
        let many: String = (0..MAX_SECTIONS + 5)
            .map(|i| format!("[V{i}]\n line\n"))
            .collect();
        let song = lyrics(&many);
        assert_eq!(song.sections.len(), MAX_SECTIONS);
        assert_eq!(song.warnings, [Warning::LimitReached]);

        let slides = format!("[C]\n{}", " a ||".repeat(MAX_SLIDES + 5));
        let song = lyrics(&slides);
        assert_eq!(song.sections[0].slides.len(), MAX_SLIDES);
        assert_eq!(song.warnings, [Warning::LimitReached]);

        let order = format!(
            "<presentation>{}</presentation><lyrics> a</lyrics>",
            "V ".repeat(MAX_ORDER + 5)
        );
        let song = parse(&song_xml(&order)).unwrap();
        assert_eq!(song.order.len(), MAX_ORDER);
        assert_eq!(song.warnings, [Warning::LimitReached]);
    }

    fn check_invariants(song: &ParsedSong) {
        assert!(!song.sections.is_empty());
        assert!(song.sections.len() <= MAX_SECTIONS);
        let slides: usize = song.sections.iter().map(|s| s.slides.len()).sum();
        assert!(slides <= MAX_SLIDES);
        assert!(
            song.sections
                .iter()
                .all(|s| s.slides.iter().all(|t| !t.is_empty()))
        );
        assert!(song.order.len() <= MAX_ORDER);
        assert!(song.order.iter().all(|&i| i < song.sections.len()));
        assert!(crate::storage::song_number_ok(&song.song_number));
    }

    const LYRIC_TOKENS: &[&str] = &[
        "\n", "\n", "\n", "[V1]", "[C]", "[V]", "[", "]", "[Coda]", ".G  C", ";", " ", " words",
        " kata", " 光", "1 ", "2 ", "3", "|", "||", "\r\n", "x", "&amp;", "]]",
    ];

    const XML_TOKENS: &[&str] = &[
        "<a>",
        "<a>",
        "</a>",
        "<a/>",
        "<a x='>'>",
        "<a y=\"/\">",
        "<b />",
        "<!--",
        "-->",
        "<![CDATA[",
        "]]>",
        "<?p",
        "?>",
        "'",
        "\"",
        "<",
        ">",
        "/",
        "text",
        "&amp;",
        "<lyrics> a</lyrics>",
    ];
    const OPENS: &[&str] = &["<a>", "<a x='>'>", "<a y=\"a/\">", "<a\n>"];
    const FILLERS: &[&str] = &[
        "",
        "<!--<a><a>-->",
        "<![CDATA[<a></b>]]>",
        "<?pi <a>?>",
        "t &gt; u",
    ];

    fn element_depth(xml: &str) -> Option<usize> {
        let doc = roxmltree::Document::parse(xml).ok()?;
        doc.descendants()
            .filter(|n| n.is_element())
            .map(|n| n.ancestors().filter(|a| a.is_element()).count())
            .max()
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
                "tests/fixtures/import/opensong/proptest-regressions.txt",
            ))),
            ..ProptestConfig::default()
        })]

        #[test]
        fn markup_soup_never_reaches_the_parser_too_deep(
            tokens in proptest::collection::vec(proptest::sample::select(XML_TOKENS), 0..200),
        ) {
            let xml = format!("<song>{}</song>", tokens.concat());
            let result = parse(xml.as_bytes());
            if let Ok(song) = &result {
                check_invariants(song);
            }
            // The scan counts elements that can hold children; an empty
            // `<a/>` may sit one level below the deepest of them.
            match element_depth(&xml) {
                Some(depth) if depth > MAX_DEPTH + 1 => {
                    prop_assert_eq!(result, Err(Rejected::TooDeep));
                }
                Some(depth) if depth <= MAX_DEPTH => {
                    prop_assert_ne!(result, Err(Rejected::TooDeep));
                }
                _ => {}
            }
        }

        #[test]
        fn nesting_is_cut_exactly_at_the_cap(
            levels in proptest::collection::vec(
                (proptest::sample::select(OPENS), proptest::sample::select(FILLERS)),
                0..2 * MAX_DEPTH,
            ),
        ) {
            let opened: String = levels.iter().map(|(open, filler)| format!("{open}{filler}")).collect();
            let xml = format!(
                "<song><lyrics> a</lyrics>{opened}{}</song>",
                "</a>".repeat(levels.len())
            );
            let result = parse(xml.as_bytes());
            if levels.len() + 1 > MAX_DEPTH {
                prop_assert_eq!(result, Err(Rejected::TooDeep));
            } else {
                prop_assert!(result.is_ok(), "{result:?}");
            }
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
            if let Ok(song) = parse(&bytes) {
                check_invariants(&song);
            }
        }

        #[test]
        fn arbitrary_lyrics_keep_the_song_invariants(
            tokens in proptest::collection::vec(proptest::sample::select(LYRIC_TOKENS), 0..300),
            order in "[VCB0-9 ]{0,40}",
        ) {
            let xml = song_xml(&format!(
                "<presentation>{order}</presentation><lyrics>{}</lyrics>",
                tokens.concat()
            ));
            match parse(&xml) {
                Ok(song) => check_invariants(&song),
                Err(rejected) => prop_assert_eq!(rejected, Rejected::NoLyrics),
            }
        }
    }
}
