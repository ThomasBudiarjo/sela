//! OpenLyrics 0.8/0.9 XML song parser (M1-14). See docs/import.md for the
//! field mapping and what is reported instead of imported.
use super::{
    MAX_LABEL, MAX_ORDER, MAX_SECTIONS, MAX_SLIDES, MAX_TEXT_FIELD, MAX_TITLE, Parsed,
    ParsedSection, ParsedSong, Rejected, SLIDE_BYTES, Warning, cut, decode, label_for_code,
    song::AUTHOR_SEPARATOR,
};
use crate::storage::{MAX_ADMINISTRATOR, song_number_ok};
use roxmltree::Node;
use std::collections::{HashMap, HashSet};

const NAMESPACE: &str = "http://openlyrics.info/namespace/2009/song";
/// Element nesting cap, checked before roxmltree parses: its element parser
/// recurses per level, so a deeply nested file would overflow the stack.
/// Real songs nest about six levels (song, lyrics, verse, lines, chord).
const MAX_DEPTH: usize = 32;
/// Node cap passed to roxmltree; a large real song has a few thousand.
const MAX_NODES: u32 = 50_000;

pub fn parse(bytes: &[u8]) -> Parsed {
    let text = decode(bytes)?;
    check_depth(&text)?;
    let options = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_NODES,
        ..roxmltree::ParsingOptions::default()
    };
    let document = roxmltree::Document::parse_with_options(&text, options).map_err(reject)?;
    let root = document.root_element();
    if name(root) != Some("song") {
        return Err(Rejected::WrongFormat);
    }
    let mut reader = Reader::default();
    for child in elements(root) {
        match name(child) {
            Some("properties") => reader.properties(child),
            Some("lyrics") => reader.lyrics(child),
            _ => reader.not_imported(child),
        }
    }
    reader.finish()
}

fn reject(error: roxmltree::Error) -> Rejected {
    match error {
        roxmltree::Error::DtdDetected | roxmltree::Error::EntityReferenceLoop(_) => {
            Rejected::Entities
        }
        roxmltree::Error::NodesLimitReached => Rejected::TooLarge,
        other => Rejected::Malformed(other.to_string()),
    }
}

/// Rejects nesting deeper than `MAX_DEPTH` with a linear scan that never
/// counts fewer open elements than roxmltree would recurse into: comments,
/// CDATA and processing instructions are skipped as roxmltree skips them,
/// quoted attribute values may hold `>`, and anything else after `<` counts.
fn check_depth(text: &str) -> Result<(), Rejected> {
    let bytes = text.as_bytes();
    let past = |from: usize, end: &[u8]| {
        bytes[from.min(bytes.len())..]
            .windows(end.len())
            .position(|w| w == end)
            .map_or(bytes.len(), |at| from + at + end.len())
    };
    let mut depth = 0usize;
    let mut at = 0;
    while let Some(offset) = bytes[at..].iter().position(|&b| b == b'<') {
        at += offset;
        let rest = &bytes[at..];
        if rest.starts_with(b"<!--") {
            at = past(at + 4, b"-->");
        } else if rest.starts_with(b"<![CDATA[") {
            at = past(at + 9, b"]]>");
        } else if rest.starts_with(b"<?") {
            at = past(at + 2, b"?>");
        } else if rest.starts_with(b"</") || rest.starts_with(b"<!") {
            depth -= usize::from(rest[1] == b'/' && depth > 0);
            at = past(at + 2, b">");
        } else {
            let mut quote = None;
            let mut end = at + 1;
            let mut empty = false;
            while let Some(&b) = bytes.get(end) {
                if b == b'<' {
                    break;
                }
                match quote {
                    Some(q) if b == q => quote = None,
                    Some(_) => {}
                    None if b == b'"' || b == b'\'' => quote = Some(b),
                    None if b == b'>' => {
                        empty = bytes[end - 1] == b'/';
                        end += 1;
                        break;
                    }
                    None => {}
                }
                end += 1;
            }
            if !empty {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(Rejected::TooDeep);
                }
            }
            at = end;
        }
    }
    Ok(())
}

/// The local name of an OpenLyrics element; `None` for text, comments and
/// elements of another namespace.
fn name<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    (node.is_element() && node.tag_name().namespace() == Some(NAMESPACE))
        .then(|| node.tag_name().name())
}

fn elements<'a, 'input>(node: Node<'a, 'input>) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children().filter(Node::is_element)
}

/// XML whitespace (not Unicode spaces such as U+3000) collapsed to single
/// spaces and trimmed, as line breaks are explicit `<br/>` elements.
fn collapse(text: &str) -> String {
    text.split([' ', '\t', '\n', '\r'])
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn text_of(node: Node) -> String {
    let raw: String = node
        .descendants()
        .filter(Node::is_text)
        .filter_map(|n| n.text())
        .collect();
    collapse(&raw)
}

/// A verse name that follows the OpenLyrics section code convention: one
/// letter, an optional number and an optional part letter (`v1`, `c`, `v1a`).
fn is_code(name: &str) -> bool {
    let mut chars = name.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    let part = chars
        .as_str()
        .trim_start_matches(|c: char| c.is_ascii_digit());
    part.len() <= 1 && part.chars().all(|c| c.is_ascii_alphabetic())
}

#[derive(Default)]
struct Reader {
    song: ParsedSong,
    titles: Vec<String>,
    authors: Vec<String>,
    copyright: Option<String>,
    ccli: Option<String>,
    publisher: Option<String>,
    verse_order: Option<String>,
    /// Indices into `song.sections` by lowercased source name, in file order.
    names: HashMap<String, Vec<usize>>,
    /// Lowercased names of verses without text or dropped at a limit; the
    /// order skips them without a missing-reference warning.
    skipped: HashSet<String>,
    slides: usize,
    bytes: usize,
}

impl Reader {
    fn warn(&mut self, warning: Warning) {
        if !self.song.warnings.contains(&warning) {
            self.song.warn(warning);
        }
    }

    fn not_imported(&mut self, node: Node) {
        self.warn(Warning::UnknownElement(node.tag_name().name().to_string()));
    }

    fn properties(&mut self, node: Node) {
        for child in elements(node) {
            match name(child) {
                Some("titles") => {
                    for title in elements(child) {
                        match name(title) {
                            Some("title") => {
                                self.ignored_attributes(title, "title", &[]);
                                self.titles.push(text_of(title));
                            }
                            _ => self.not_imported(title),
                        }
                    }
                }
                Some("authors") => {
                    for author in elements(child) {
                        match name(author) {
                            Some("author") => {
                                self.ignored_attributes(author, "author", &[]);
                                self.authors.push(text_of(author));
                            }
                            _ => self.not_imported(author),
                        }
                    }
                }
                Some("copyright") => self.once(child, |r| &mut r.copyright),
                Some("ccliNo") => self.once(child, |r| &mut r.ccli),
                Some("publisher") => self.once(child, |r| &mut r.publisher),
                Some("verseOrder") => self.once(child, |r| &mut r.verse_order),
                _ => self.not_imported(child),
            }
        }
    }

    /// Reports each attribute of `node` outside `mapped` once per file as
    /// `IgnoredAttribute("element/@name")`, such as `lines/@repeat` or
    /// `verse/@lang`: the song model has no place for them.
    fn ignored_attributes(&mut self, node: Node, element: &str, mapped: &[&str]) {
        for attribute in node.attributes() {
            if !mapped.contains(&attribute.name()) {
                self.warn(Warning::IgnoredAttribute(format!(
                    "{element}/@{}",
                    attribute.name()
                )));
            }
        }
    }

    /// Keeps the first occurrence of a single-valued property and reports
    /// any later one as not imported.
    fn once(&mut self, node: Node, slot: fn(&mut Self) -> &mut Option<String>) {
        if slot(self).is_some() {
            self.not_imported(node);
        } else {
            *slot(self) = Some(text_of(node));
        }
    }

    fn lyrics(&mut self, node: Node) {
        for child in elements(node) {
            match name(child) {
                Some("verse") => self.verse(child),
                _ => self.not_imported(child),
            }
        }
    }

    fn verse(&mut self, node: Node) {
        self.ignored_attributes(node, "verse", &["name"]);
        let source = collapse(node.attribute("name").unwrap_or_default());
        if source.is_empty() {
            self.warn(Warning::InvalidField("verse name".into()));
        }
        let label = is_code(&source)
            .then(|| label_for_code(&source))
            .flatten()
            .unwrap_or_else(|| source.clone());
        let label = self.fit(label, MAX_LABEL, "section label");
        let mut slides = Vec::new();
        for child in elements(node) {
            match name(child) {
                Some("lines") => {
                    self.ignored_attributes(child, "lines", &[]);
                    let text = self.lines(child);
                    if !text.is_empty() {
                        slides.push(text);
                    }
                }
                _ => self.not_imported(child),
            }
        }
        if slides.is_empty() {
            self.warn(Warning::EmptySection(label));
            self.skipped.insert(source.to_lowercase());
            return;
        }
        let mut kept = Vec::new();
        for slide in slides {
            let cost = label.len() + slide.len() + 24;
            if self.song.sections.len() >= MAX_SECTIONS
                || self.slides >= MAX_SLIDES
                || self.bytes + cost > SLIDE_BYTES
            {
                self.warn(Warning::LimitReached);
                break;
            }
            self.slides += 1;
            self.bytes += cost;
            kept.push(slide);
        }
        if kept.is_empty() {
            self.skipped.insert(source.to_lowercase());
            return;
        }
        self.names
            .entry(source.to_lowercase())
            .or_default()
            .push(self.song.sections.len());
        self.song.sections.push(ParsedSection {
            label,
            slides: kept,
        });
    }

    /// One slide's text: `<br/>` breaks lines, chords and formatting tags
    /// keep their text, comments are dropped.
    fn lines(&mut self, node: Node) -> String {
        let mut lines = vec![String::new()];
        self.inline(node, &mut lines);
        let lines: Vec<String> = lines.iter().map(|line| collapse(line)).collect();
        let first = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);
        let last = lines
            .iter()
            .rposition(|l| !l.is_empty())
            .map_or(0, |i| i + 1);
        lines[first..last.max(first)].join("\n")
    }

    fn inline(&mut self, node: Node, lines: &mut Vec<String>) {
        for child in node.children() {
            if child.is_text() {
                let line = lines.last_mut().expect("starts with one line");
                line.push_str(child.text().unwrap_or_default());
                continue;
            }
            if !child.is_element() {
                continue;
            }
            match name(child) {
                Some("br") => lines.push(String::new()),
                // OpenLyrics 0.7 wrote each line as an element.
                Some("line") => {
                    self.inline(child, lines);
                    lines.push(String::new());
                }
                Some("comment") => {}
                Some("chord") => {
                    self.warn(Warning::ChordsRemoved);
                    self.inline(child, lines);
                }
                _ => {
                    self.not_imported(child);
                    self.inline(child, lines);
                }
            }
        }
    }

    /// `text` cut at a grapheme boundary to `max` bytes, with a warning.
    fn fit(&mut self, text: String, max: usize, field: &str) -> String {
        if text.len() <= max {
            return text;
        }
        self.warn(Warning::Truncated(field.to_string()));
        cut(&text, max).to_string()
    }

    fn finish(mut self) -> Parsed {
        if self.song.sections.is_empty() {
            return Err(if self.song.warnings.contains(&Warning::LimitReached) {
                Rejected::TooLarge
            } else {
                Rejected::NoLyrics
            });
        }
        let mut titles = std::mem::take(&mut self.titles);
        // `Song::validate` trims Unicode spaces, which `collapse` keeps.
        titles.retain(|t| !t.trim().is_empty());
        match titles.len() {
            // The caller substitutes a title on this warning, so it must
            // survive a full warning list.
            0 => self.song.warn_missing_title(),
            1 => {}
            _ => self.warn(Warning::UnknownElement("title".into())),
        }
        if let Some(title) = titles.into_iter().next() {
            self.song.title = self.fit(title, MAX_TITLE, "title");
        }
        let mut joined = 0;
        for author in std::mem::take(&mut self.authors) {
            if author.is_empty() || self.song.authors.contains(&author) {
                continue;
            }
            let separator = if joined == 0 {
                0
            } else {
                AUTHOR_SEPARATOR.len()
            };
            if joined + separator + author.len() > MAX_TEXT_FIELD {
                self.warn(Warning::Truncated("authors".into()));
                break;
            }
            joined += separator + author.len();
            self.song.authors.push(author);
        }
        if let Some(copyright) = self.copyright.take() {
            self.song.copyright = self.fit(copyright, MAX_TEXT_FIELD, "copyright");
        }
        if let Some(publisher) = self.publisher.take() {
            self.song.administrator = self.fit(publisher, MAX_ADMINISTRATOR, "publisher");
        }
        match self.ccli.take() {
            Some(number) if song_number_ok(&number) => self.song.song_number = number,
            Some(_) => self.warn(Warning::InvalidField("ccliNo".into())),
            None => {}
        }
        if let Some(order) = self.verse_order.take() {
            self.resolve(&order);
        }
        Ok(self.song)
    }

    /// Each order name becomes every section with that name, in file order
    /// (translations of a verse share its name).
    fn resolve(&mut self, order: &str) {
        let mut slides = 0;
        for entry in order.split(' ').filter(|e| !e.is_empty()) {
            let entry = entry.to_lowercase();
            let Some(sections) = self.names.get(&entry).cloned() else {
                if !self.skipped.contains(&entry) {
                    self.warn(Warning::MissingOrderReference(entry));
                }
                continue;
            };
            for index in sections {
                let count = self.song.sections[index].slides.len();
                if slides + count > MAX_ORDER {
                    self.warn(Warning::LimitReached);
                    return;
                }
                slides += count;
                self.song.order.push(index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{MAX_FILE_BYTES, MAX_WARNINGS, song::to_song};
    use proptest::prelude::*;
    use std::time::{Duration, Instant};

    const FIELDS: &[u8] = include_bytes!("../../tests/fixtures/import/openlyrics/fields.xml");
    const LOSSES: &[u8] = include_bytes!("../../tests/fixtures/import/openlyrics/losses.xml");
    const UNICODE: &[u8] = include_bytes!("../../tests/fixtures/import/openlyrics/unicode.xml");

    fn section(label: &str, slides: &[&str]) -> ParsedSection {
        ParsedSection {
            label: label.into(),
            slides: slides.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn song_with(properties: &str, lyrics: &str) -> String {
        format!(
            r#"<song xmlns="{NAMESPACE}" version="0.9"><properties>{properties}</properties><lyrics>{lyrics}</lyrics></song>"#
        )
    }

    fn sorted(warnings: Vec<Warning>) -> Vec<String> {
        let mut out: Vec<String> = warnings.iter().map(|w| format!("{w:?}")).collect();
        out.sort();
        out
    }

    #[test]
    fn every_supported_field_is_mapped() {
        let song = parse(FIELDS).unwrap();
        assert_eq!(
            song,
            ParsedSong {
                title: "Lantern by the Road".into(),
                authors: vec!["Rina Kartika".into(), "Daniel Hartono".into()],
                copyright: "2026 Sela Test Fixtures".into(),
                song_number: "1234567".into(),
                administrator: "Fixture Music Admin".into(),
                sections: vec![
                    section(
                        "Verse 1",
                        &["A lantern hangs beside the road\nand burns the whole night long"]
                    ),
                    section(
                        "Verse 2",
                        &["The travellers who pass it by\nwalk home along its glow"]
                    ),
                    section(
                        "Chorus",
                        &["Keep the light, keep the light\nuntil the morning comes"]
                    ),
                    section(
                        "Bridge",
                        &[
                            "When the wind is cold\nthe flame still holds",
                            "When the road is lost\nthe light is found"
                        ]
                    ),
                ],
                order: vec![0, 2, 1, 2, 3, 2],
                // "words"/"music" has no place in the one-line Authors field.
                warnings: vec![Warning::IgnoredAttribute("author/@type".into())],
            }
        );
    }

    #[test]
    fn content_that_is_not_imported_is_reported_once() {
        let song = parse(LOSSES).unwrap();
        assert_eq!(song.title, "River Stones");
        assert_eq!(song.authors, ["Maya Sutanto"]);
        assert_eq!(song.song_number, "");
        assert_eq!(
            song.sections,
            [
                section(
                    "Verse 1",
                    &[
                        "Stones lie quiet\nunder the running water",
                        "Smooth and round\n\nafter a thousand years"
                    ]
                ),
                section(
                    "Verse 1",
                    &["Batu diam di dasar\ndi bawah air yang mengalir"]
                ),
                section("Chorus", &["Carry me, carry me down"]),
                section("bridge", &["Down to the sea"]),
            ]
        );
        assert_eq!(song.order, [0, 1, 2, 0, 1, 3]);
        let unknown = |name: &str| Warning::UnknownElement(name.into());
        let attribute = |name: &str| Warning::IgnoredAttribute(name.into());
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![
                unknown("title"),
                unknown("keywords"),
                unknown("format"),
                unknown("tag"),
                unknown("sparkle"),
                unknown("instrument"),
                Warning::InvalidField("ccliNo".into()),
                Warning::ChordsRemoved,
                Warning::EmptySection("Verse 2".into()),
                Warning::MissingOrderReference("v3".into()),
                // Dropped attributes, once each (docs/import.md).
                attribute("title/@lang"),
                attribute("author/@type"),
                attribute("author/@lang"),
                attribute("lines/@part"),
                attribute("lines/@break"),
                attribute("verse/@lang"),
                attribute("verse/@translit"),
                attribute("lines/@repeat"),
            ])
        );
    }

    #[test]
    fn indonesian_accented_and_cjk_text_is_kept_exactly() {
        let song = parse(UNICODE).unwrap();
        assert_eq!(song.title, "Cahaya Pagi Hari");
        assert_eq!(song.authors, ["Ayu Wulandari", "José Peña"]);
        assert_eq!(song.copyright, "© 2026 Sela Test Fixtures");
        let slides: Vec<&str> = song
            .sections
            .iter()
            .flat_map(|s| &s.slides)
            .map(String::as_str)
            .collect();
        assert_eq!(
            slides,
            [
                "Pagi ini kubuka jendela lebar-lebar\nCahaya-Mu masuk, hatiku jadi tenang",
                "Terima kasih, ya Tuhan, untuk hari yang baru\nKasih-Mu tak pernah berubah",
                "Canción de la mañana, señor del corazón\nNoël à minuit, déjà là, Ångström naïve",
                "晨光照进窗内\u{3000}心里充满平安\n주님의 빛이 아침을 엽니다\nCafe\u{301} with a combining accent",
            ]
        );
        assert_eq!(song.warnings, []);
    }

    #[test]
    fn hostile_files_are_rejected_within_a_bounded_time() {
        let nest = |depth: usize| {
            song_with(
                "",
                &format!(
                    r#"<verse name="v1"><lines>{}x{}</lines></verse>"#,
                    "<tag>".repeat(depth),
                    "</tag>".repeat(depth)
                ),
            )
            .into_bytes()
        };
        assert!(parse(&nest(MAX_DEPTH - 4)).is_ok(), "at the cap");
        let mut huge = nest(0);
        huge.resize(MAX_FILE_BYTES + 1, b' ');
        let mut invalid =
            song_with("", r#"<verse name="v1"><lines>ok</lines></verse>"#).into_bytes();
        invalid.splice(10..10, [0xC3, 0x28]);
        let many_nodes = song_with(
            "",
            &format!(
                r#"<verse name="v1"><lines>{}</lines></verse>"#,
                "<br/>".repeat(60_000)
            ),
        );
        let deepest = "<a>".repeat(MAX_FILE_BYTES / 3);
        let file = |name: &str| {
            std::fs::read(format!(
                "{}/tests/fixtures/import/openlyrics/{name}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap()
        };
        let malformed = Rejected::Malformed(String::new());
        let cases = [
            ("unclosed tag", file("malformed-unclosed.xml"), malformed),
            (
                "entity to a local file",
                file("xxe-file.xml"),
                Rejected::Entities,
            ),
            ("entity to a URL", file("xxe-url.xml"), Rejected::Entities),
            (
                "billion laughs",
                file("billion-laughs.xml"),
                Rejected::Entities,
            ),
            (
                "doctype without declarations",
                format!("<!DOCTYPE song>{}", song_with("", "")).into_bytes(),
                Rejected::Entities,
            ),
            (
                "one level past the cap",
                nest(MAX_DEPTH - 3),
                Rejected::TooDeep,
            ),
            (
                "a megabyte of open tags",
                deepest.into_bytes(),
                Rejected::TooDeep,
            ),
            (
                "too many nodes",
                many_nodes.into_bytes(),
                Rejected::TooLarge,
            ),
            ("over the size cap", huge, Rejected::TooLarge),
            ("invalid UTF-8", invalid, Rejected::InvalidUtf8),
        ];
        for (case, bytes, expected) in cases {
            let start = Instant::now();
            let result = parse(&bytes);
            let elapsed = start.elapsed();
            assert!(elapsed < Duration::from_secs(5), "{case}: took {elapsed:?}");
            match result {
                Err(rejected) => assert_eq!(
                    std::mem::discriminant(&rejected),
                    std::mem::discriminant(&expected),
                    "{case}: {rejected:?}"
                ),
                Ok(song) => panic!("{case}: imported {song:?}"),
            }
        }
    }

    #[test]
    fn markup_inside_comments_cdata_and_attributes_does_not_count_as_nesting() {
        let tags = "<a><a><a><a><a><a><a><a><a><a>".repeat(4);
        let lyrics = format!(
            r#"<!-- {tags} --><verse name="v1"><lines><chord root="G" x="{}"><chord root="C"><chord root="D">deep</chord></chord></chord><![CDATA[ {tags}]]></lines></verse>"#,
            ">".repeat(40)
        );
        let song = parse(song_with("", &lyrics).as_bytes()).unwrap();
        assert_eq!(
            song.sections,
            [section("Verse 1", &[&format!("deep {tags}")])]
        );
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![Warning::MissingTitle, Warning::ChordsRemoved])
        );
    }

    #[test]
    fn other_formats_and_songs_without_lyrics_are_rejected() {
        let opensong = b"<song><title>Plain</title><lyrics>[V1]\n Words</lyrics></song>";
        assert_eq!(parse(opensong), Err(Rejected::WrongFormat));
        assert_eq!(parse(b"<html><body/></html>"), Err(Rejected::WrongFormat));
        let titled = "<titles><title>Only a title</title></titles>";
        assert_eq!(
            parse(song_with(titled, "").as_bytes()),
            Err(Rejected::NoLyrics)
        );
        let blank = r#"<verse name="v1"><lines> <br/> <comment>hidden</comment> </lines></verse>"#;
        assert_eq!(
            parse(song_with(titled, blank).as_bytes()),
            Err(Rejected::NoLyrics)
        );
        assert!(matches!(parse(b""), Err(Rejected::Malformed(_))));
    }

    #[test]
    fn limits_keep_the_first_entries_and_warn() {
        let verses: String = (1..=MAX_SECTIONS + 2)
            .map(|n| format!(r#"<verse name="v{n}"><lines>line {n}</lines></verse>"#))
            .collect();
        let order = format!("<verseOrder>{}</verseOrder>", "v1 ".repeat(MAX_ORDER + 5));
        let song = parse(song_with(&order, &verses).as_bytes()).unwrap();
        assert_eq!(song.sections.len(), MAX_SECTIONS);
        assert_eq!(
            song.sections.last().unwrap().label,
            format!("Verse {MAX_SECTIONS}")
        );
        assert_eq!(song.order, vec![0; MAX_ORDER]);
        assert_eq!(
            sorted(song.warnings.clone()),
            sorted(vec![Warning::LimitReached, Warning::MissingTitle])
        );
        let named = ParsedSong {
            title: "At every limit".into(),
            ..song
        };
        assert!(to_song(&named).is_ok(), "the capped song is storable");

        let order = "<verseOrder>v1 v129</verseOrder>";
        let song = parse(song_with(order, &verses).as_bytes()).unwrap();
        assert_eq!(song.order, [0]);
        assert!(
            !song
                .warnings
                .iter()
                .any(|w| matches!(w, Warning::MissingOrderReference(_)))
        );

        let big = format!(
            r#"<verse name="v1"><lines>{}</lines></verse>"#,
            "x".repeat(SLIDE_BYTES)
        );
        assert_eq!(
            parse(song_with("", &big).as_bytes()),
            Err(Rejected::TooLarge)
        );

        let long = format!(
            "<titles><title>{}é</title></titles><publisher>{}</publisher><ccliNo>1234567890123</ccliNo>",
            "t".repeat(MAX_TITLE - 1),
            "p".repeat(MAX_ADMINISTRATOR + 1)
        );
        let lyrics = r#"<verse name="v1"><lines>x</lines></verse>"#;
        let song = parse(song_with(&long, lyrics).as_bytes()).unwrap();
        assert_eq!(song.title, "t".repeat(MAX_TITLE - 1));
        assert_eq!(song.administrator, "p".repeat(MAX_ADMINISTRATOR));
        assert_eq!(song.song_number, "");
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![
                Warning::Truncated("title".into()),
                Warning::Truncated("publisher".into()),
                Warning::InvalidField("ccliNo".into()),
            ])
        );
    }

    #[test]
    fn verse_names_outside_the_code_convention_stay_verbatim() {
        let lyrics = r#"<verse name="custom"><lines>a</lines></verse><verse name="V2"><lines>b</lines></verse><verse name="c1b"><lines>c</lines></verse><verse name="x9"><lines>d</lines></verse>"#;
        let order = "<verseOrder>V2 CUSTOM</verseOrder>";
        let song = parse(song_with(order, lyrics).as_bytes()).unwrap();
        let labels: Vec<&str> = song.sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["custom", "Verse 2", "Chorus 1", "x9"]);
        assert_eq!(song.order, [1, 0]);
    }

    #[test]
    fn titles_of_only_unicode_spaces_count_as_missing() {
        let lyrics = r#"<verse name="v1"><lines>words</lines></verse>"#;
        for title in ["&#xA0;", "&#x3000;", "\u{3000}", " \u{a0} "] {
            let properties = format!("<titles><title>{title}</title></titles>");
            let song = parse(song_with(&properties, lyrics).as_bytes()).unwrap();
            assert_eq!(song.warnings, [Warning::MissingTitle], "{title:?}");
        }
        let unknown: String = (0..MAX_WARNINGS + 6)
            .map(|n| format!("<extra{n}/>"))
            .collect();
        let song = parse(song_with(&unknown, lyrics).as_bytes()).unwrap();
        assert_eq!(song.warnings.len(), MAX_WARNINGS);
        assert!(
            song.warnings.contains(&Warning::MissingTitle),
            "kept when the warning list is full"
        );
    }

    #[test]
    fn a_long_order_of_unknown_names_resolves_within_a_bounded_time() {
        let mut lyrics = String::from(r#"<verse name="v1"><lines>only words</lines></verse>"#);
        for n in 0..20_000 {
            lyrics.push_str(&format!(r#"<verse name="e{n}"/>"#));
        }
        let mut order = String::from("v1");
        let room = MAX_FILE_BYTES - song_with("<verseOrder></verseOrder>", &lyrics).len();
        while order.len() + 3 <= room {
            order.push_str(" zz");
        }
        let bytes = song_with(&format!("<verseOrder>{order}</verseOrder>"), &lyrics);
        assert!(bytes.len() <= MAX_FILE_BYTES);
        let start = Instant::now();
        let song = parse(bytes.as_bytes()).unwrap();
        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_secs(5), "took {elapsed:?}");
        assert_eq!(song.order, [0]);
    }

    #[test]
    fn verse_names_are_collapsed_and_a_missing_name_is_reported() {
        let lyrics = r#"<verse><lines>a</lines></verse><verse name="  "><lines>b</lines></verse><verse name="last&#10;one"><lines>c</lines></verse>"#;
        let song = parse(song_with("", lyrics).as_bytes()).unwrap();
        let labels: Vec<&str> = song.sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["", "", "last one"]);
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![
                Warning::MissingTitle,
                Warning::InvalidField("verse name".into()),
            ])
        );
    }

    #[test]
    fn repeated_single_properties_keep_the_first_and_are_reported() {
        let properties = "<titles><title>Twice</title></titles>\
            <copyright>First holder</copyright><copyright>Second holder</copyright>\
            <ccliNo>111</ccliNo><ccliNo>222</ccliNo>\
            <publisher>First admin</publisher><publisher>Second admin</publisher>\
            <verseOrder>v1</verseOrder><verseOrder>v2 v1</verseOrder>";
        let lyrics =
            r#"<verse name="v1"><lines>a</lines></verse><verse name="v2"><lines>b</lines></verse>"#;
        let song = parse(song_with(properties, lyrics).as_bytes()).unwrap();
        assert_eq!(song.copyright, "First holder");
        assert_eq!(song.song_number, "111");
        assert_eq!(song.administrator, "First admin");
        assert_eq!(song.order, [0]);
        let unknown = |name: &str| Warning::UnknownElement(name.into());
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![
                unknown("copyright"),
                unknown("ccliNo"),
                unknown("publisher"),
                unknown("verseOrder"),
            ])
        );
    }

    #[test]
    fn openlyrics_0_7_line_elements_become_line_breaks() {
        let lyrics = "<verse name=\"v1\"><lines>\n  <line>first line</line>\n  <line>second <chord name=\"G\"/>line</line>\n</lines></verse>";
        let song = parse(song_with("", lyrics).as_bytes()).unwrap();
        assert_eq!(
            song.sections,
            [section("Verse 1", &["first line\nsecond line"])]
        );
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![Warning::MissingTitle, Warning::ChordsRemoved])
        );
    }

    /// A parsed song is always storable once the caller names it after a
    /// `MissingTitle`.
    fn check(bytes: &[u8]) {
        let Ok(mut song) = parse(bytes) else {
            return;
        };
        let slides: usize = song.sections.iter().map(|s| s.slides.len()).sum();
        assert!(song.sections.len() <= MAX_SECTIONS && slides <= MAX_SLIDES);
        assert!(song.order.iter().all(|&i| i < song.sections.len()));
        assert!(
            song.sections
                .iter()
                .flat_map(|s| &s.slides)
                .all(|t| !t.is_empty())
        );
        assert!(song_number_ok(&song.song_number));
        if song.warnings.contains(&Warning::MissingTitle) {
            song.title = "file name".into();
        }
        assert_eq!(to_song(&song).map(|_| ()), Ok(()));
    }

    const INSERTS: [&[u8]; 6] = [b"<br/>", b"<", b">", b"</lines>", b"&", b"<chord>"];

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
            check(&bytes);
        }

        #[test]
        fn mutated_documents_never_panic(
            edits in proptest::collection::vec(
                (any::<prop::sample::Index>(), any::<u8>(), 0u8..5),
                1..24,
            )
        ) {
            for fixture in [FIELDS, LOSSES, UNICODE] {
                let mut bytes = fixture.to_vec();
                for (at, byte, kind) in &edits {
                    let at = at.index(bytes.len() + 1);
                    match kind {
                        0 if at < bytes.len() => {
                            bytes.remove(at);
                        }
                        1 => bytes.insert(at, *byte),
                        2 if at < bytes.len() => bytes[at] = *byte,
                        3 => {
                            let insert = INSERTS[usize::from(*byte) % INSERTS.len()];
                            bytes.splice(at..at, insert.iter().copied());
                        }
                        _ => bytes.truncate(at),
                    }
                }
                check(&bytes);
            }
        }
    }
}
