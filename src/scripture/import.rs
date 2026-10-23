//! Parsers for untrusted Bible files: OSIS, USFX and Zefania XML (M4-03).
//!
//! Each turns one file's bytes into a `ParsedBible` (verses in canonical
//! order plus metadata and warnings) or a `Rejected` reason. Nothing here
//! touches storage, the UI thread or the network. The hardening matches the
//! song importers (`crate::import`): a size cap checked first, DTDs and
//! entity declarations refused, a linear nesting-depth scan before the DOM is
//! built, a node cap, and caps on verse count, verse length and warnings.
//! Only the 66-book canon is kept; other books are skipped with a warning.
use super::books::Book;
use super::model::{MAX_META, MAX_NUMBER, VerseId};
use roxmltree::Node;
use std::collections::HashMap;

/// Largest Bible file accepted, checked before parsing.
pub const MAX_FILE_BYTES: usize = 48 * 1024 * 1024;
/// Deepest element nesting accepted.
pub const MAX_DEPTH: usize = 64;
/// Most XML nodes (elements, text, comments) one file may produce.
pub const MAX_NODES: u32 = 3_000_000;
/// Most verses kept; the KJV canon has 31,102.
pub const MAX_VERSES: usize = 40_000;
/// Longest verse text kept, in bytes; longer text is cut at a character.
pub const MAX_VERSE_BYTES: usize = 4096;
/// Most warnings kept per file.
pub const MAX_WARNINGS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Osis,
    Usfx,
    Zefania,
}

/// One translation as written in its source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedBible {
    pub format: Format,
    /// The file's own title; empty when it has none.
    pub name: String,
    /// The file's own short id or work id, if any (not yet validated).
    pub abbreviation: String,
    pub language: String,
    /// The rights statement as written; empty when the file has none.
    pub rights: String,
    /// Verses in canonical order, each once, text with whitespace collapsed.
    pub verses: Vec<(VerseId, String)>,
    pub warnings: Vec<Warning>,
}

/// Something the parser dropped or joined. Shown after an import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// A book outside the 66-book canon, or an unknown code.
    SkippedBook(String),
    /// A verse or chapter number that is missing or out of range.
    BadNumber(String),
    /// One element covering several verses (`Gen.1.1 Gen.1.2`); its text is
    /// stored under the first.
    MergedVerses(String),
    EmptyVerse(VerseId),
    Truncated(VerseId),
    /// `MAX_VERSES` reached; the rest of the file was dropped.
    LimitReached,
}

/// Why a file produced no translation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejected {
    TooLarge,
    InvalidUtf8,
    /// Not well formed; the string names the first problem.
    Malformed(String),
    /// A DOCTYPE or entity declaration (XXE and expansion bombs).
    Entities,
    TooDeep,
    /// Well formed, but not OSIS, USFX or Zefania.
    WrongFormat,
    /// No canonical verse with text.
    NoVerses,
}

/// Parses an OSIS, USFX or Zefania file, recognized by its root element.
pub fn parse(bytes: &[u8]) -> Result<ParsedBible, Rejected> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(Rejected::TooLarge);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| Rejected::InvalidUtf8)?;
    check_depth(text)?;
    let options = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_NODES,
        ..roxmltree::ParsingOptions::default()
    };
    let document = roxmltree::Document::parse_with_options(text, options).map_err(reject)?;
    let root = document.root_element();
    let parsed = match root.tag_name().name().to_ascii_lowercase().as_str() {
        "osis" => osis(root),
        "usfx" => usfx(root),
        "xmlbible" => zefania(root),
        _ => return Err(Rejected::WrongFormat),
    };
    if parsed.verses.is_empty() {
        return Err(Rejected::NoVerses);
    }
    Ok(parsed)
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
/// counts fewer open elements than roxmltree would recurse into (the same
/// scan as the song importers): comments, CDATA and processing instructions
/// are skipped, quoted attribute values may hold `>`.
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Enter,
    Leave,
}

/// Visits `root` and its descendants in document order without recursion.
/// `visit(node, Enter)` returning false skips the node's subtree (and its
/// `Leave`); text nodes get only `Enter`.
fn walk<'a, 'i>(root: Node<'a, 'i>, mut visit: impl FnMut(Node<'a, 'i>, Step) -> bool) {
    let mut node = root;
    let mut entering = true;
    loop {
        if entering {
            let descend = visit(node, Step::Enter) && node.is_element();
            if descend {
                if let Some(child) = node.first_child() {
                    node = child;
                    continue;
                }
                visit(node, Step::Leave);
            }
        } else {
            visit(node, Step::Leave);
        }
        if node == root {
            return;
        }
        match node.next_sibling() {
            Some(sibling) => {
                node = sibling;
                entering = true;
            }
            None => {
                let Some(parent) = node.parent() else { return };
                node = parent;
                entering = false;
            }
        }
    }
}

fn local(node: Node) -> String {
    node.tag_name().name().to_ascii_lowercase()
}

/// Leading ASCII digits as a number in `1..=MAX_NUMBER`.
fn number(text: &str) -> Option<u16> {
    let digits: &str = &text[..text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len())];
    digits
        .parse::<u16>()
        .ok()
        .filter(|n| (1..=MAX_NUMBER).contains(n))
}

/// Whitespace collapsed to single spaces and trimmed, control characters
/// dropped.
fn collapse(text: &str) -> String {
    text.split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Metadata text: collapsed and cut to `MAX_META` bytes.
fn meta(text: &str) -> String {
    let mut text = collapse(text);
    cut(&mut text, MAX_META);
    text
}

/// Cuts `text` to at most `max` bytes at a character boundary. A cut that
/// lands after a space drops it, so text stays trimmed (SEC-11).
fn cut(text: &mut String, max: usize) -> bool {
    if text.len() <= max {
        return false;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    let trimmed = text.trim_end().len();
    text.truncate(trimmed);
    true
}

fn all_text(node: Node) -> String {
    let mut out = String::new();
    for text in node.descendants().filter(Node::is_text) {
        if out.len() > MAX_META * 2 {
            break;
        }
        out.push_str(text.text().unwrap_or_default());
    }
    meta(&out)
}

/// Verse text assembly shared by the three formats.
#[derive(Default)]
struct Collector {
    verses: Vec<(VerseId, String)>,
    index: HashMap<VerseId, usize>,
    current: Option<VerseId>,
    buffer: String,
    warnings: Vec<Warning>,
    skipped: Vec<String>,
    full: bool,
}

impl Collector {
    fn warn(&mut self, warning: Warning) {
        if self.warnings.len() < MAX_WARNINGS {
            self.warnings.push(warning);
        }
    }

    fn skip_book(&mut self, code: &str) {
        if !self.skipped.iter().any(|s| s == code) && self.skipped.len() < MAX_WARNINGS {
            self.skipped.push(code.to_owned());
            self.warn(Warning::SkippedBook(code.to_owned()));
        }
    }

    fn start(&mut self, id: VerseId) {
        self.finish();
        self.current = Some(id);
    }

    fn text(&mut self, text: &str) {
        // Room for the cap plus collapsible whitespace; the rest is cut anyway.
        if self.current.is_some() && self.buffer.len() <= MAX_VERSE_BYTES * 2 {
            self.buffer.push_str(text);
        }
    }

    fn space(&mut self) {
        if self.current.is_some() {
            self.buffer.push(' ');
        }
    }

    fn finish(&mut self) {
        let Some(id) = self.current.take() else {
            return;
        };
        let mut text = collapse(&std::mem::take(&mut self.buffer));
        if text.is_empty() {
            if !self.index.contains_key(&id) {
                self.warn(Warning::EmptyVerse(id));
            }
            return;
        }
        if let Some(&at) = self.index.get(&id) {
            // A verse split across paragraphs or milestones continues.
            let joined = &mut self.verses[at].1;
            joined.push(' ');
            joined.push_str(&text);
            if cut(joined, MAX_VERSE_BYTES) {
                self.warn(Warning::Truncated(id));
            }
            return;
        }
        if self.verses.len() >= MAX_VERSES {
            if !self.full {
                self.full = true;
                self.warn(Warning::LimitReached);
            }
            return;
        }
        if cut(&mut text, MAX_VERSE_BYTES) {
            self.warn(Warning::Truncated(id));
        }
        self.index.insert(id, self.verses.len());
        self.verses.push((id, text));
    }

    fn done(mut self, format: Format) -> ParsedBible {
        self.finish();
        self.verses.sort_by_key(|(id, _)| *id);
        ParsedBible {
            format,
            name: String::new(),
            abbreviation: String::new(),
            language: String::new(),
            rights: String::new(),
            verses: self.verses,
            warnings: self.warnings,
        }
    }
}

/// An OSIS id such as `Gen.1.1` or `Bible.KJV:John.3.16`.
fn osis_verse(collector: &mut Collector, id: &str) -> Option<VerseId> {
    let id = id.rsplit(':').next().unwrap_or(id);
    let mut parts = id.split('.');
    let (book, chapter, verse) = (parts.next()?, parts.next(), parts.next());
    let Some(book) = Book::from_osis(book) else {
        collector.skip_book(book);
        return None;
    };
    let found = chapter
        .and_then(number)
        .zip(verse.and_then(number))
        .and_then(|(chapter, verse)| VerseId::new(book, chapter, verse));
    if found.is_none() {
        collector.warn(Warning::BadNumber(id.to_owned()));
    }
    found
}

fn osis(root: Node) -> ParsedBible {
    let mut collector = Collector::default();
    let text_node = root
        .children()
        .find(|n| n.is_element() && local(*n) == "osistext");
    let work = root
        .descendants()
        .find(|n| n.is_element() && local(*n) == "work");
    let child = |name: &str| {
        work.and_then(|w| w.children().find(|n| n.is_element() && local(*n) == name))
            .map(all_text)
            .unwrap_or_default()
    };
    let (name, rights, work_language) = (child("title"), child("rights"), child("language"));
    let language = text_node
        .and_then(|n| n.attribute(("http://www.w3.org/XML/1998/namespace", "lang")))
        .map(meta)
        .filter(|l| !l.is_empty())
        .unwrap_or(work_language);
    let abbreviation = text_node
        .and_then(|n| n.attribute("osisIDWork"))
        .map(|w| meta(w.rsplit('.').next().unwrap_or(w)))
        .unwrap_or_default();
    walk(root, |node, step| {
        if node.is_text() {
            collector.text(node.text().unwrap_or_default());
            return false;
        }
        if !node.is_element() {
            return false;
        }
        let name = local(node);
        match (name.as_str(), step) {
            ("header" | "note" | "title" | "rdg" | "figure" | "milestone", Step::Enter) => false,
            ("verse", Step::Enter) => {
                if node.attribute("eID").is_some() {
                    collector.finish();
                } else if let Some(ids) = node.attribute("osisID") {
                    let mut ids = ids.split_whitespace();
                    let first = ids.next().unwrap_or_default();
                    if ids.next().is_some() {
                        collector.warn(Warning::MergedVerses(meta(first)));
                    }
                    match osis_verse(&mut collector, first) {
                        Some(id) => collector.start(id),
                        None => collector.finish(),
                    }
                }
                true
            }
            ("verse", Step::Leave) => {
                // A container verse ends with its element; a milestone
                // (`sID`) stays open until its `eID`.
                if node.attribute("sID").is_none() && node.has_children() {
                    collector.finish();
                }
                true
            }
            ("chapter", Step::Enter) if node.attribute("eID").is_some() => {
                collector.finish();
                true
            }
            ("chapter" | "div", Step::Leave) => {
                collector.finish();
                true
            }
            ("l" | "lb" | "lg" | "p", _) => {
                collector.space();
                true
            }
            _ => true,
        }
    });
    let mut parsed = collector.done(Format::Osis);
    parsed.name = name;
    parsed.rights = rights;
    parsed.language = language;
    parsed.abbreviation = abbreviation;
    parsed
}

/// USFX/USFM codes of front and back matter, skipped without a warning.
const PERIPHERALS: &[&str] = &[
    "FRT", "BAK", "OTH", "INT", "CNC", "GLO", "TDX", "NDX", "XXA", "XXB", "XXC", "XXD", "XXE",
    "XXF", "XXG", "CVR",
];

fn usfx(root: Node) -> ParsedBible {
    let mut collector = Collector::default();
    let mut book: Option<Book> = None;
    let mut chapter: Option<u16> = None;
    let mut language = String::new();
    walk(root, |node, step| {
        if node.is_text() {
            collector.text(node.text().unwrap_or_default());
            return false;
        }
        if !node.is_element() {
            return false;
        }
        let name = local(node);
        match (name.as_str(), step) {
            ("languagecode", Step::Enter) => {
                language = all_text(node);
                false
            }
            ("book", Step::Enter) => {
                collector.finish();
                let code = node.attribute("id").unwrap_or_default();
                book = Book::from_usfm(code);
                chapter = None;
                if book.is_none() && !PERIPHERALS.iter().any(|p| p.eq_ignore_ascii_case(code)) {
                    collector.skip_book(code);
                }
                book.is_some()
            }
            ("book", Step::Leave) => {
                collector.finish();
                book = None;
                true
            }
            ("c", Step::Enter) => {
                collector.finish();
                let id = node.attribute("id").unwrap_or_default();
                chapter = number(id);
                if chapter.is_none() {
                    collector.warn(Warning::BadNumber(meta(id)));
                }
                false
            }
            ("v", Step::Enter) => {
                collector.finish();
                let id = node.attribute("id").unwrap_or_default();
                let verse = book
                    .zip(chapter)
                    .zip(number(id))
                    .and_then(|((b, c), v)| VerseId::new(b, c, v));
                match verse {
                    Some(verse) => {
                        if id.contains(['-', ',']) {
                            collector.warn(Warning::MergedVerses(meta(id)));
                        }
                        collector.start(verse);
                    }
                    None => collector.warn(Warning::BadNumber(meta(id))),
                }
                false
            }
            ("ve", Step::Enter) => {
                collector.finish();
                false
            }
            (
                "f" | "fe" | "x" | "h" | "toc" | "id" | "ide" | "rem" | "s" | "ms" | "mr" | "r"
                | "d" | "cl" | "cp" | "vp" | "fig" | "ca" | "va" | "periph" | "restore"
                | "milestone",
                Step::Enter,
            ) => false,
            ("p" | "q" | "b" | "li" | "pi" | "m" | "nb" | "qc" | "qr" | "qm", _) => {
                collector.space();
                true
            }
            _ => true,
        }
    });
    let mut parsed = collector.done(Format::Usfx);
    parsed.language = language;
    parsed
}

fn zefania(root: Node) -> ParsedBible {
    let mut collector = Collector::default();
    let mut book: Option<Book> = None;
    let mut chapter: Option<u16> = None;
    let information = root
        .children()
        .find(|n| n.is_element() && local(*n) == "information");
    let field = |name: &str| {
        information
            .and_then(|i| i.children().find(|n| n.is_element() && local(*n) == name))
            .map(all_text)
            .unwrap_or_default()
    };
    let mut name = meta(root.attribute("biblename").unwrap_or_default());
    if name.is_empty() {
        name = field("title");
    }
    let (abbreviation, language, rights) =
        (field("identifier"), field("language"), field("rights"));
    walk(root, |node, step| {
        if node.is_text() {
            collector.text(node.text().unwrap_or_default());
            return false;
        }
        if !node.is_element() {
            return false;
        }
        let element = local(node);
        match (element.as_str(), step) {
            (
                "information" | "note" | "xref" | "caption" | "prolog" | "remark" | "media" | "div",
                Step::Enter,
            ) => false,
            ("biblebook", Step::Enter) => {
                collector.finish();
                let number = node.attribute("bnumber").unwrap_or_default();
                book = number.parse::<u8>().ok().and_then(Book::from_number);
                chapter = None;
                if book.is_none() {
                    let label = node.attribute("bname").unwrap_or(number);
                    collector.skip_book(&meta(label));
                }
                book.is_some()
            }
            ("chapter", Step::Enter) => {
                collector.finish();
                let id = node.attribute("cnumber").unwrap_or_default();
                chapter = number(id);
                if chapter.is_none() {
                    collector.warn(Warning::BadNumber(meta(id)));
                }
                chapter.is_some()
            }
            ("vers", Step::Enter) => {
                collector.finish();
                let id = node.attribute("vnumber").unwrap_or_default();
                let verse = book
                    .zip(chapter)
                    .zip(number(id))
                    .and_then(|((b, c), v)| VerseId::new(b, c, v));
                match verse {
                    Some(verse) => {
                        collector.start(verse);
                        true
                    }
                    None => {
                        collector.warn(Warning::BadNumber(meta(id)));
                        false
                    }
                }
            }
            ("vers" | "chapter" | "biblebook", Step::Leave) => {
                collector.finish();
                true
            }
            ("br", _) => {
                collector.space();
                true
            }
            _ => true,
        }
    });
    let mut parsed = collector.done(Format::Zefania);
    parsed.name = name;
    parsed.abbreviation = abbreviation;
    parsed.language = language;
    parsed.rights = rights;
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(book: u8, chapter: u16, verse: u16) -> VerseId {
        VerseId::new(Book::from_number(book).unwrap(), chapter, verse).unwrap()
    }

    fn texts(parsed: &ParsedBible) -> Vec<(i64, &str)> {
        parsed
            .verses
            .iter()
            .map(|(id, text)| (id.key(), text.as_str()))
            .collect()
    }

    const OSIS_MILESTONES: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<osis xmlns="http://www.bibletechnologies.net/2003/OSIS/namespace">
 <osisText osisIDWork="Bible.en.syn" xml:lang="en">
  <header><work osisWork="Bible.en.syn"><title>Synthetic Test Bible</title>
   <rights type="x-copyright">Synthetic text for tests</rights></work></header>
  <div type="book" osisID="John">
   <title type="main">John</title>
   <chapter osisRef="John.3" sID="John.3.s" n="3"/>
   <title type="psalm">A heading that is not verse text</title>
   <p><verse osisID="John.3.16" sID="v16" n="16"/>First <transChange type="added">added</transChange>
   words.<note>a footnote</note><verse eID="v16"/>
   <verse osisID="John.3.17" sID="v17"/><q who="x" sID="q1" marker=""/>Second
   line<lb/>continues<verse eID="v17"/></p>
   <chapter eID="John.3.s"/>
   <chapter osisID="John.4">
    <verse osisID="John.4.1">Container verse</verse>
    <verse osisID="John.4.2 John.4.3">Merged verses</verse>
   </chapter>
  </div>
  <div type="book" osisID="Tob"><chapter osisID="Tob.1"><verse osisID="Tob.1.1">skipped</verse></chapter></div>
 </osisText>
</osis>"#;

    #[test]
    fn osis_milestone_and_container_verses() {
        let parsed = parse(OSIS_MILESTONES.as_bytes()).unwrap();
        assert_eq!(parsed.format, Format::Osis);
        assert_eq!(parsed.name, "Synthetic Test Bible");
        assert_eq!(parsed.rights, "Synthetic text for tests");
        assert_eq!(parsed.language, "en");
        assert_eq!(parsed.abbreviation, "syn");
        assert_eq!(
            texts(&parsed),
            [
                (43_003_016, "First added words."),
                (43_003_017, "Second line continues"),
                (43_004_001, "Container verse"),
                (43_004_002, "Merged verses"),
            ]
        );
        assert!(
            parsed
                .warnings
                .contains(&Warning::SkippedBook("Tob".into()))
        );
        assert!(
            parsed
                .warnings
                .contains(&Warning::MergedVerses("John.4.2".into()))
        );
    }

    const USFX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<usfx><languageCode>eng</languageCode>
<book id="FRT"><p>Preface text</p></book>
<book id="PSA"><h>Psalms</h><c id="23"/><d>A synthetic title.</d>
<q><v id="1"/>First line,</q><q level="2">second line.<ve/></q>
<p><v id="2"/>Has a note<f caller="+"><fr>23:2 </fr><ft>note text</ft></f> and a <x>cross ref</x>end.<ve/></p>
</book>
<book id="TOB"><c id="1"/><p><v id="1"/>skipped<ve/></p></book>
<book id="JHN"><c id="1"/><p><v id="1-2"/>Joined verse.<ve/><v id="0"/>bad<ve/></p></book>
</usfx>"#;

    #[test]
    fn usfx_milestones_skip_notes_titles_and_peripherals() {
        let parsed = parse(USFX.as_bytes()).unwrap();
        assert_eq!(parsed.format, Format::Usfx);
        assert_eq!(parsed.language, "eng");
        assert_eq!(
            texts(&parsed),
            [
                (19_023_001, "First line, second line."),
                (19_023_002, "Has a note and a end."),
                (43_001_001, "Joined verse."),
            ]
        );
        assert_eq!(
            parsed.warnings,
            [
                Warning::SkippedBook("TOB".into()),
                Warning::MergedVerses("1-2".into()),
                Warning::BadNumber("0".into()),
            ]
        );
    }

    const ZEFANIA: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<XMLBIBLE biblename="Synthetic Zefania">
<INFORMATION><title>Ignored title</title><identifier>SZEF</identifier>
<language>IND</language><rights>Synthetic rights</rights></INFORMATION>
<BIBLEBOOK bnumber="1" bname="Kejadian"><CHAPTER cnumber="1">
<CAPTION>Heading</CAPTION>
<VERS vnumber="1">Pada mulanya<NOTE>catatan</NOTE> <STYLE css="x">Allah</STYLE><BR/>menciptakan.</VERS>
<VERS vnumber="2">Kedua — “kutipan” ünïcode</VERS>
<VERS vnumber="x">bad</VERS>
</CHAPTER></BIBLEBOOK>
<BIBLEBOOK bnumber="67" bname="Tobit"><CHAPTER cnumber="1"><VERS vnumber="1">skip</VERS></CHAPTER></BIBLEBOOK>
</XMLBIBLE>"#;

    #[test]
    fn zefania_containers_keep_unicode_and_drop_notes() {
        let parsed = parse(ZEFANIA.as_bytes()).unwrap();
        assert_eq!(parsed.format, Format::Zefania);
        assert_eq!(parsed.name, "Synthetic Zefania");
        assert_eq!(parsed.abbreviation, "SZEF");
        assert_eq!(parsed.language, "IND");
        assert_eq!(parsed.rights, "Synthetic rights");
        assert_eq!(
            texts(&parsed),
            [
                (1_001_001, "Pada mulanya Allah menciptakan."),
                (1_001_002, "Kedua — “kutipan” ünïcode"),
            ]
        );
        assert_eq!(
            parsed.warnings,
            [
                Warning::BadNumber("x".into()),
                Warning::SkippedBook("Tobit".into())
            ]
        );
    }

    #[test]
    fn hostile_files_are_rejected_before_or_while_parsing() {
        let big = vec![b' '; MAX_FILE_BYTES + 1];
        assert_eq!(parse(&big), Err(Rejected::TooLarge));
        assert_eq!(parse(b"<osis>\xff</osis>"), Err(Rejected::InvalidUtf8));
        let doctype = r#"<?xml version="1.0"?><!DOCTYPE x [<!ENTITY a "aaaa">]><usfx>&a;</usfx>"#;
        assert_eq!(parse(doctype.as_bytes()), Err(Rejected::Entities));
        let deep = format!(
            "<usfx>{}{}</usfx>",
            "<p>".repeat(MAX_DEPTH + 1),
            "</p>".repeat(MAX_DEPTH + 1)
        );
        assert_eq!(parse(deep.as_bytes()), Err(Rejected::TooDeep));
        assert!(matches!(
            parse(b"<usfx><book id=\"GEN\">"),
            Err(Rejected::Malformed(_))
        ));
        assert_eq!(parse(b"<song/>"), Err(Rejected::WrongFormat));
        assert_eq!(
            parse(b"<usfx><book id=\"GEN\"><c id=\"1\"/></book></usfx>"),
            Err(Rejected::NoVerses)
        );
    }

    #[test]
    fn verse_count_and_length_are_capped() {
        let long = "word ".repeat(MAX_VERSE_BYTES);
        let mut many = String::from("<usfx><book id=\"PSA\">");
        for chapter in 1..=45u16 {
            many.push_str(&format!("<c id=\"{chapter}\"/>"));
            for verse in 1..=999u16 {
                many.push_str(&format!("<v id=\"{verse}\"/>v<ve/>"));
            }
        }
        many.push_str(&format!(
            "<c id=\"46\"/><v id=\"1\"/>{long}<ve/></book></usfx>"
        ));
        let parsed = parse(many.as_bytes()).unwrap();
        assert_eq!(parsed.verses.len(), MAX_VERSES);
        assert!(parsed.warnings.contains(&Warning::LimitReached));

        let one =
            format!("<usfx><book id=\"GEN\"><c id=\"1\"/><v id=\"1\"/>{long}<ve/></book></usfx>");
        let parsed = parse(one.as_bytes()).unwrap();
        assert!(parsed.verses[0].1.len() <= MAX_VERSE_BYTES);
        assert_eq!(parsed.warnings, [Warning::Truncated(id(1, 1, 1))]);
    }

    // SEC-11 (fuzz-audit): a verse cut at `MAX_VERSE_BYTES` right after a
    // space kept it, so the stored text was not trimmed as documented.
    #[test]
    fn a_verse_cut_at_the_limit_ends_without_a_space() {
        for words in ["a ".repeat(MAX_VERSE_BYTES), "ab ".repeat(MAX_VERSE_BYTES)] {
            let one = format!(
                "<usfx><book id=\"GEN\"><c id=\"1\"/><v id=\"1\"/>{words}<ve/>\
                 <v id=\"1\"/>{words}<ve/></book></usfx>"
            );
            let parsed = parse(one.as_bytes()).unwrap();
            let text = &parsed.verses[0].1;
            assert!(text.len() <= MAX_VERSE_BYTES);
            assert_eq!(text.trim_end(), text);
        }
    }

    #[test]
    fn split_verses_are_joined_and_empty_ones_reported() {
        let text = r#"<osis><osisText><div type="book" osisID="Gen">
<verse osisID="Gen.1.1" sID="a"/>One<verse eID="a"/>
<verse osisID="Gen.1.1" sID="b"/>more<verse eID="b"/>
<verse osisID="Gen.1.2" sID="c"/>  <verse eID="c"/>
<verse osisID="Gen.x.2" sID="d"/>bad<verse eID="d"/>
</div></osisText></osis>"#;
        let parsed = parse(text.as_bytes()).unwrap();
        assert_eq!(texts(&parsed), [(1_001_001, "One more")]);
        assert_eq!(
            parsed.warnings,
            [
                Warning::EmptyVerse(id(1, 1, 2)),
                Warning::BadNumber("Gen.x.2".into())
            ]
        );
    }
}
