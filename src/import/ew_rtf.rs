//! EasyWorship 6-8 song words (`SongWords.db` `word.words`, an RTF string) to
//! labelled sections of slides (M4-07). The observed format, both dialects
//! and the decoding rules are in docs/import.md, "EasyWorship song words".
use std::collections::BTreeMap;

use super::{
    MAX_FILE_BYTES, MAX_SECTIONS, MAX_SLIDES, Parsed, ParsedSection, ParsedSong, Rejected, Warning,
};

/// Deepest group nesting accepted. EW8 words nest three levels.
pub const MAX_DEPTH: usize = 64;

/// Parses one song's words. Metadata comes from `Songs.db`, so only
/// `sections` and `warnings` are filled.
pub fn parse(words: &[u8]) -> Parsed {
    if words.len() > MAX_FILE_BYTES {
        return Err(Rejected::TooLarge);
    }
    let mut song = ParsedSong::default();
    let document = Reader::new(words, &mut song).read()?;
    let drafts = match document.dialect {
        Dialect::Ew8 => ew8_sections(&document.paragraphs),
        Dialect::Legacy => legacy_sections(&document.paragraphs),
    };
    finish(drafts, &mut song)?;
    song.fit_to_model()?;
    Ok(song)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dialect {
    /// Written by EW 6+: hidden label paragraphs and slide markers.
    Ew8,
    /// Generic RichEdit RTF: slides separated by blank lines.
    Legacy,
}

/// A paragraph property; the last of `\pard`, `\sdparawysiwghidden` or
/// `\sdslidemarker` in effect at `\par` wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParagraphKind {
    Lyric,
    Label,
    SlideMarker,
}

struct Paragraph {
    /// Text with `\line` breaks as `\n`.
    text: String,
    kind: ParagraphKind,
}

struct Document {
    dialect: Dialect,
    paragraphs: Vec<Paragraph>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Destination {
    Text,
    FontTable,
    Skip,
}

/// State that RTF scopes to a group and restores at `}`.
#[derive(Clone, Copy)]
struct Group {
    kind: ParagraphKind,
    destination: Destination,
    /// `\ucN`: fallback characters that follow each `\uN`.
    uc: u32,
    /// `\fN`; `None` is the document default font.
    font: Option<i32>,
}

const ROOT: Group = Group {
    kind: ParagraphKind::Lyric,
    destination: Destination::Text,
    uc: 1,
    font: None,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CodePage {
    Cp1252,
    Cp1251,
}

/// Destinations without `\*` whose text is not lyrics.
const SKIPPED: &[&[u8]] = &[
    b"colortbl",
    b"stylesheet",
    b"info",
    b"pict",
    b"object",
    b"header",
    b"headerl",
    b"headerr",
    b"headerf",
    b"footer",
    b"footerl",
    b"footerr",
    b"footerf",
    b"footnote",
    b"annotation",
    b"pntext",
    b"pntxta",
    b"pntxtb",
    b"listtable",
    b"listoverridetable",
    b"revtbl",
    b"filetbl",
    b"fldinst",
    b"nonshppict",
    b"xe",
    b"tc",
    b"txe",
];

/// Character formatting the importer drops; each is reported once.
const FORMATTING: &[&[u8]] = &[b"b", b"i", b"ul", b"strike"];

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    stack: Vec<Group>,
    song: &'a mut ParsedSong,
    dialect: Dialect,
    /// `\ansicpgN`; `Err` names a declaration this parser cannot decode.
    code_page: Result<CodePage, String>,
    default_font: i32,
    /// The `\fN` of the font table entry being read.
    font_entry: Option<i32>,
    /// Font table entries as (`\fN`, `\fcharsetM`).
    fonts: BTreeMap<i32, i32>,
    /// `\uN` fallback characters still to skip.
    skip: u32,
    high_surrogate: Option<u16>,
    text: String,
    paragraphs: Vec<Paragraph>,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], song: &'a mut ParsedSong) -> Self {
        Self {
            bytes,
            pos: 0,
            stack: Vec::new(),
            song,
            dialect: Dialect::Legacy,
            code_page: Ok(CodePage::Cp1252),
            default_font: 0,
            font_entry: None,
            fonts: BTreeMap::new(),
            skip: 0,
            high_surrogate: None,
            text: String::new(),
            paragraphs: Vec::new(),
        }
    }

    fn read(mut self) -> Result<Document, Rejected> {
        self.pos = self
            .bytes
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(self.bytes.len());
        if !self.bytes[self.pos..].starts_with(b"{\\rtf") {
            return Err(Rejected::WrongFormat);
        }
        while let Some(&byte) = self.bytes.get(self.pos) {
            self.pos += 1;
            match byte {
                b'{' => {
                    if self.stack.len() == MAX_DEPTH {
                        return Err(Rejected::TooDeep);
                    }
                    self.stack.push(self.stack.last().copied().unwrap_or(ROOT));
                    self.skip = 0;
                }
                b'}' => {
                    self.skip = 0;
                    if self.stack.len() == 1 {
                        return self.end();
                    }
                    self.stack.pop();
                }
                b'\\' => self.control()?,
                b'\r' | b'\n' | 0 => {}
                0x80.. if self.group().destination == Destination::Text => {
                    return Err(Rejected::Malformed(format!(
                        "byte 0x{byte:02X} at offset {} is not 7-bit RTF",
                        self.pos - 1
                    )));
                }
                _ if self.fallback() => {}
                b'\t' => self.literal(' '),
                _ => self.literal(char::from(byte)),
            }
        }
        Err(Rejected::Malformed("unclosed group".into()))
    }

    fn end(mut self) -> Result<Document, Rejected> {
        if !self.text.is_empty() || self.high_surrogate.is_some() {
            self.end_paragraph();
        }
        let rest = &self.bytes[self.pos..];
        if !rest.iter().all(|&b| b.is_ascii_whitespace() || b == 0) {
            return Err(Rejected::Malformed("text after the document".into()));
        }
        Ok(Document {
            dialect: self.dialect,
            paragraphs: self.paragraphs,
        })
    }

    fn group(&mut self) -> &mut Group {
        self.stack.last_mut().expect("inside the document group")
    }

    /// Consumes one `\uN` fallback character if any are due.
    fn fallback(&mut self) -> bool {
        let due = self.skip > 0;
        self.skip = self.skip.saturating_sub(1);
        due
    }

    fn control(&mut self) -> Result<(), Rejected> {
        let Some(&first) = self.bytes.get(self.pos) else {
            return Err(Rejected::Malformed("backslash at the end".into()));
        };
        if !first.is_ascii_alphabetic() {
            self.pos += 1;
            return self.symbol(first);
        }
        let start = self.pos;
        while self
            .bytes
            .get(self.pos)
            .is_some_and(u8::is_ascii_alphabetic)
        {
            self.pos += 1;
        }
        let word = &self.bytes[start..self.pos];
        let param = self.param();
        if self.bytes.get(self.pos) == Some(&b' ') {
            self.pos += 1;
        }
        if word == b"bin" {
            let length = usize::try_from(param.unwrap_or(0)).unwrap_or(0);
            self.pos = self.pos.saturating_add(length).min(self.bytes.len());
        } else if !self.fallback() {
            self.word(word, param);
        }
        Ok(())
    }

    fn param(&mut self) -> Option<i32> {
        let negative = self.bytes.get(self.pos) == Some(&b'-')
            && self.bytes.get(self.pos + 1).is_some_and(u8::is_ascii_digit);
        if negative {
            self.pos += 1;
        }
        let mut value: Option<i64> = None;
        while let Some(&digit) = self.bytes.get(self.pos).filter(|b| b.is_ascii_digit()) {
            self.pos += 1;
            let digit = i64::from(digit - b'0');
            value = Some(value.unwrap_or(0).saturating_mul(10).saturating_add(digit));
        }
        let value = if negative { -value? } else { value? };
        Some(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
    }

    fn symbol(&mut self, symbol: u8) -> Result<(), Rejected> {
        if symbol == b'\'' {
            let byte = self
                .bytes
                .get(self.pos..self.pos + 2)
                .and_then(|hex| std::str::from_utf8(hex).ok())
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                .ok_or_else(|| Rejected::Malformed("bad \\' escape".into()))?;
            self.pos += 2;
            if !self.fallback() {
                let c = self.decode(byte);
                self.literal(c);
            }
            return Ok(());
        }
        if symbol == b'*' {
            self.group().destination = Destination::Skip;
            return Ok(());
        }
        if self.fallback() {
            return Ok(());
        }
        match symbol {
            b'\\' | b'{' | b'}' => self.literal(char::from(symbol)),
            b'~' => self.literal('\u{A0}'),
            b'_' => self.literal('\u{2011}'),
            b'\r' | b'\n' => self.end_paragraph(),
            _ => {}
        }
        Ok(())
    }

    fn word(&mut self, word: &[u8], param: Option<i32>) {
        let destination = self.group().destination;
        match word {
            b"par" | b"sect" | b"page" => self.end_paragraph(),
            b"line" => self.literal('\n'),
            b"tab" | b"cell" | b"emspace" | b"enspace" | b"qmspace" => self.literal(' '),
            b"emdash" => self.literal('\u{2014}'),
            b"endash" => self.literal('\u{2013}'),
            b"bullet" => self.literal('\u{2022}'),
            b"lquote" => self.literal('\u{2018}'),
            b"rquote" => self.literal('\u{2019}'),
            b"ldblquote" => self.literal('\u{201C}'),
            b"rdblquote" => self.literal('\u{201D}'),
            b"u" => {
                if let Some(n) = param {
                    // RTF writes code units above 32767 as negative 16-bit values.
                    let unit = if n < 0 { n.saturating_add(0x10000) } else { n };
                    self.unicode(u16::try_from(unit).unwrap_or(0xFFFD));
                }
                self.skip = self.group().uc;
            }
            b"uc" => self.group().uc = u32::try_from(param.unwrap_or(1)).unwrap_or(0),
            b"pard" => self.group().kind = ParagraphKind::Lyric,
            b"sdparawysiwghidden" => {
                self.dialect = Dialect::Ew8;
                self.group().kind = ParagraphKind::Label;
            }
            b"sdslidemarker" => {
                self.dialect = Dialect::Ew8;
                self.group().kind = ParagraphKind::SlideMarker;
            }
            b"sdeasyworship" => self.dialect = Dialect::Ew8,
            b"ansicpg" => {
                self.code_page = match param {
                    Some(1252) => Ok(CodePage::Cp1252),
                    Some(1251) => Ok(CodePage::Cp1251),
                    _ => Err(format!("\\ansicpg{}", param.unwrap_or(0))),
                }
            }
            b"mac" | b"pc" | b"pca" => {
                self.code_page = Err(format!("\\{}", String::from_utf8_lossy(word)));
            }
            b"deff" => self.default_font = param.unwrap_or(0),
            b"fonttbl" => self.group().destination = Destination::FontTable,
            b"f" if destination == Destination::FontTable => self.font_entry = param,
            b"f" => self.group().font = param,
            b"plain" => self.group().font = None,
            b"fcharset" if destination == Destination::FontTable => {
                if let (Some(font), Some(charset)) = (self.font_entry, param) {
                    self.fonts.insert(font, charset);
                }
            }
            w if destination == Destination::Text
                && FORMATTING.contains(&w)
                && param != Some(0) =>
            {
                let name = format!("\\{}", String::from_utf8_lossy(w));
                warn_once(self.song, Warning::UnknownElement(name));
            }
            w if SKIPPED.contains(&w) => self.group().destination = Destination::Skip,
            _ => {}
        }
    }

    /// A `\'hh` byte in the code page of the current font's `\fcharset`, or
    /// the document's `\ansicpg` when the font declares none.
    fn decode(&mut self, byte: u8) -> char {
        let font = self.group().font.unwrap_or(self.default_font);
        let code_page = match self.fonts.get(&font).copied() {
            None | Some(1) => self.code_page.clone(),
            Some(0) => Ok(CodePage::Cp1252),
            Some(204) => Ok(CodePage::Cp1251),
            Some(other) => Err(format!("\\fcharset{other}")),
        };
        let code_page = code_page.unwrap_or_else(|declared| {
            warn_once(self.song, Warning::UnknownDirective(declared));
            CodePage::Cp1252
        });
        code_page.decode(byte)
    }

    fn unicode(&mut self, unit: u16) {
        if self.group().destination != Destination::Text {
            return;
        }
        match unit {
            0xD800..=0xDBFF => {
                self.literal_surrogate();
                self.high_surrogate = Some(unit);
            }
            0xDC00..=0xDFFF => match self.high_surrogate.take() {
                Some(high) => {
                    let code =
                        0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(unit) - 0xDC00);
                    self.literal(char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER));
                }
                None => self.literal(char::REPLACEMENT_CHARACTER),
            },
            _ => {
                self.literal(char::from_u32(u32::from(unit)).unwrap_or(char::REPLACEMENT_CHARACTER))
            }
        }
    }

    /// A high surrogate not followed by a low one becomes U+FFFD.
    fn literal_surrogate(&mut self) {
        if self.high_surrogate.take().is_some() {
            self.literal(char::REPLACEMENT_CHARACTER);
        }
    }

    fn literal(&mut self, c: char) {
        if self.group().destination != Destination::Text {
            return;
        }
        self.literal_surrogate();
        if c == '\n' || !c.is_control() {
            self.text.push(c);
        }
    }

    fn end_paragraph(&mut self) {
        if self.group().destination != Destination::Text {
            return;
        }
        self.literal_surrogate();
        let kind = self.group().kind;
        self.paragraphs.push(Paragraph {
            text: std::mem::take(&mut self.text),
            kind,
        });
    }
}

fn warn_once(song: &mut ParsedSong, warning: Warning) {
    if !song.warnings.contains(&warning) {
        song.warn(warning);
    }
}

impl CodePage {
    fn decode(self, byte: u8) -> char {
        let unit = match (self, byte) {
            (_, 0..=0x7F) => u32::from(byte),
            (Self::Cp1252, 0x80..=0x9F) => u32::from(CP1252_80[usize::from(byte - 0x80)]),
            (Self::Cp1252, _) => u32::from(byte),
            (Self::Cp1251, 0xC0..) => 0x410 + u32::from(byte - 0xC0),
            (Self::Cp1251, _) => u32::from(CP1251_80[usize::from(byte - 0x80)]),
        };
        char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)
    }
}

/// Windows-1252 0x80..=0x9F (0xA0..=0xFF equal Latin-1); undefined bytes are
/// U+FFFD.
const CP1252_80: [u16; 32] = [
    0x20AC, 0xFFFD, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0xFFFD, 0x017D, 0xFFFD, 0xFFFD, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0xFFFD, 0x017E, 0x0178,
];

/// Windows-1251 0x80..=0xBF (0xC0..=0xFF are U+0410..=U+044F).
const CP1251_80: [u16; 64] = [
    0x0402, 0x0403, 0x201A, 0x0453, 0x201E, 0x2026, 0x2020, 0x2021, 0x20AC, 0x2030, 0x0409, 0x2039,
    0x040A, 0x040C, 0x040B, 0x040F, 0x0452, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0xFFFD, 0x2122, 0x0459, 0x203A, 0x045A, 0x045C, 0x045B, 0x045F, 0x00A0, 0x040E, 0x045E, 0x0408,
    0x00A4, 0x0490, 0x00A6, 0x00A7, 0x0401, 0x00A9, 0x0404, 0x00AB, 0x00AC, 0x00AD, 0x00AE, 0x0407,
    0x00B0, 0x00B1, 0x0406, 0x0456, 0x0491, 0x00B5, 0x00B6, 0x00B7, 0x0451, 0x2116, 0x0454, 0x00BB,
    0x0458, 0x0405, 0x0455, 0x0457,
];

/// A section being built; it may still end up with no slides.
struct Draft {
    label: String,
    slides: Vec<String>,
    dropped_empty: bool,
}

impl Draft {
    fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            slides: Vec::new(),
            dropped_empty: false,
        }
    }
}

/// EW8-OBS-022: a label paragraph starts a group; `\sdslidemarker` ends a
/// slide and the following slides join the group until the next label.
/// Blank lines stay inside their slide (EW8-OBS-023).
fn ew8_sections(paragraphs: &[Paragraph]) -> Vec<Draft> {
    let mut drafts: Vec<Draft> = Vec::new();
    let mut lines: Vec<&str> = Vec::new();
    for paragraph in paragraphs {
        match paragraph.kind {
            ParagraphKind::Label => {
                let label = paragraph.text.split_whitespace().collect::<Vec<_>>();
                if label.is_empty() {
                    continue;
                }
                // A label is its slide's first line, so one after lyrics opens
                // a slide. Blank lines before it are dropped so that a run of
                // labels does not rescan them.
                if lines.iter().any(|line| !line.trim().is_empty()) {
                    close_slide(&mut drafts, &mut lines);
                } else {
                    lines.clear();
                }
                drafts.push(Draft::new(label.join(" ")));
            }
            ParagraphKind::SlideMarker => {
                close_slide(&mut drafts, &mut lines);
                lines.push(&paragraph.text);
            }
            ParagraphKind::Lyric => lines.push(&paragraph.text),
        }
    }
    close_slide(&mut drafts, &mut lines);
    drafts
}

fn close_slide(drafts: &mut Vec<Draft>, lines: &mut Vec<&str>) {
    let text = slide_text(lines.drain(..));
    if drafts.is_empty() {
        drafts.push(Draft::new(""));
    }
    let draft = drafts.last_mut().expect("pushed above");
    if text.is_empty() {
        draft.dropped_empty = true;
    } else {
        draft.slides.push(text);
    }
}

/// Lines trimmed, without leading and trailing blank lines.
fn slide_text<'a>(paragraphs: impl Iterator<Item = &'a str>) -> String {
    let lines: Vec<&str> = paragraphs
        .flat_map(|p| p.split('\n'))
        .map(str::trim)
        .collect();
    let first = lines.iter().position(|line| !line.is_empty());
    let last = lines.iter().rposition(|line| !line.is_empty());
    match (first, last) {
        (Some(first), Some(last)) => lines[first..=last].join("\n"),
        _ => String::new(),
    }
}

/// Slides are separated by blank lines. A block whose first line is a label
/// in `labels::parse` starts a section; other blocks join the previous
/// labelled section, or become "Verse N" when there is none to join.
fn legacy_sections(paragraphs: &[Paragraph]) -> Vec<Draft> {
    let lines: Vec<&str> = paragraphs
        .iter()
        .flat_map(|p| p.text.split('\n'))
        .map(str::trim)
        .collect();
    let mut drafts: Vec<Draft> = Vec::new();
    let mut joinable = false;
    let mut verses = 0;
    for block in lines
        .split(|line| line.is_empty())
        .filter(|b| !b.is_empty())
    {
        let body = if crate::labels::parse(block[0]).is_some() {
            drafts.push(Draft::new(block[0]));
            joinable = true;
            &block[1..]
        } else {
            if !joinable {
                verses += 1;
                drafts.push(Draft::new(format!("Verse {verses}")));
            }
            block
        };
        if let (Some(draft), false) = (drafts.last_mut(), body.is_empty()) {
            draft.slides.push(body.join("\n"));
        }
    }
    drafts
}

fn finish(drafts: Vec<Draft>, song: &mut ParsedSong) -> Result<(), Rejected> {
    let mut slides = 0;
    for mut draft in drafts {
        if draft.dropped_empty || draft.slides.is_empty() {
            song.warn(Warning::EmptySection(draft.label.clone()));
        }
        if draft.slides.is_empty() {
            continue;
        }
        if song.sections.len() == MAX_SECTIONS || slides == MAX_SLIDES {
            song.warn(Warning::LimitReached);
            break;
        }
        let room = MAX_SLIDES - slides;
        let cut = draft.slides.len() > room;
        draft.slides.truncate(room);
        slides += draft.slides.len();
        song.sections.push(ParsedSection {
            label: draft.label,
            slides: draft.slides,
        });
        if cut {
            song.warn(Warning::LimitReached);
            break;
        }
    }
    if song.sections.is_empty() {
        return Err(Rejected::NoLyrics);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::SLIDE_BYTES;
    use proptest::prelude::*;
    use proptest::test_runner::FileFailurePersistence;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/tests/fixtures/import/ew_rtf/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn section(label: &str, slides: &[&str]) -> ParsedSection {
        ParsedSection {
            label: label.into(),
            slides: slides.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn sections(words: &str) -> Vec<ParsedSection> {
        let song = parse(words.as_bytes()).unwrap();
        assert_eq!(song.warnings, Vec::<Warning>::new(), "{words}");
        song.sections
    }

    /// One paragraph in the shape observed in a local EW 8.0.49
    /// `SongWords.db` (docs/import.md).
    fn ew8_paragraph(flag: &str, text: &str) -> String {
        format!(
            "{{\\pard{flag}\\qc\\qdef\\sdewparatemplatestyle101\\plain\\sdewtemplatestyle101\\fs156{{\\*\\sdfsreal 77.76}}\\sdfsauto {text}\\par}}\r\n"
        )
    }

    fn ew8_words(sections: &[ParsedSection]) -> String {
        let mut words = String::from(
            "{\\rtf1\\ansi\\deff0\\sdeasyworship2\r\n{\\fonttbl{\\f0 Tahoma;}}\r\n{\\colortbl ;}\r\n",
        );
        for (s, section) in sections.iter().enumerate() {
            if s > 0 {
                words += &ew8_paragraph("\\sdslidemarker", "");
            }
            words += &ew8_paragraph(
                "\\sdparawysiwghidden\\sdlistlevel-1",
                &escape(&section.label),
            );
            for (i, slide) in section.slides.iter().enumerate() {
                if i > 0 {
                    words += &ew8_paragraph("\\sdslidemarker", "");
                }
                for line in slide.split('\n') {
                    words += &ew8_paragraph("", &escape(line));
                }
            }
        }
        words + "}"
    }

    fn escape(text: &str) -> String {
        let mut out = String::new();
        for c in text.chars() {
            match c {
                '\\' | '{' | '}' => {
                    out.push('\\');
                    out.push(c);
                }
                ' '..='~' => out.push(c),
                '\u{A0}'..='\u{FF}' => out += &format!("\\'{:02x}", u32::from(c)),
                _ => {
                    let mut units = [0; 2];
                    for unit in c.encode_utf16(&mut units) {
                        out += &format!("\\u{}?", *unit as i16);
                    }
                }
            }
        }
        out
    }

    #[test]
    fn ew8_hidden_labels_and_slide_markers_make_sections() {
        let song = parse(&fixture("ew8_labels.rtf")).unwrap();
        assert_eq!(
            song.sections,
            [
                section(
                    "Verse 1",
                    &[
                        "Morning breaks on quiet hills\nRivers carry songs of light\nEvery valley lifts its voice",
                        "Open wide the ancient gates\nLet the weary come and rest",
                        "Sing again",
                    ],
                ),
                section("Chorus", &["Lift your hands and lift your hearts"]),
                section("Coda (soft)", &["Hush now, the night is still"]),
                section(
                    "Verse 2",
                    &["Lanterns glow along the road\n\nChorus\nhomeward bound"]
                ),
            ]
        );
        assert_eq!(song.warnings, Vec::<Warning>::new());
        assert_eq!(song.title, "");
    }

    #[test]
    fn ew8_slides_before_any_label_are_unlabelled() {
        let words = format!(
            "{{\\rtf1\\sdeasyworship2 {}{}{}{}{}}}",
            ew8_paragraph("", "Chorus"),
            ew8_paragraph("", ""),
            ew8_paragraph("", "first words"),
            ew8_paragraph("\\sdslidemarker", ""),
            ew8_paragraph("", "second words"),
        );
        assert_eq!(
            sections(&words),
            [section("", &["Chorus\n\nfirst words", "second words"])]
        );
    }

    #[test]
    fn ew8_label_after_lyrics_without_a_marker_starts_a_slide() {
        let words = format!(
            "{{\\rtf1 {}{}{}}}",
            ew8_paragraph("", "opening line"),
            ew8_paragraph("\\sdparawysiwghidden", "  Bridge   2 "),
            ew8_paragraph("", "bridge line"),
        );
        assert_eq!(
            sections(&words),
            [
                section("", &["opening line"]),
                section("Bridge 2", &["bridge line"])
            ]
        );
    }

    #[test]
    fn ew8_empty_slides_are_dropped_with_a_warning() {
        let words = format!(
            "{{\\rtf1\\sdeasyworship2 {}{}{}{}{}{}{}{}{}{}{}{}}}",
            ew8_paragraph("\\sdparawysiwghidden", "Verse 1"),
            ew8_paragraph("", "a"),
            ew8_paragraph("\\sdslidemarker", ""),
            ew8_paragraph("", "  "),
            ew8_paragraph("\\sdslidemarker", ""),
            ew8_paragraph("", "b"),
            ew8_paragraph("\\sdslidemarker", ""),
            ew8_paragraph("\\sdparawysiwghidden", "Chorus"),
            ew8_paragraph("\\sdslidemarker", ""),
            ew8_paragraph("\\sdparawysiwghidden", "Verse 2"),
            ew8_paragraph("", "c"),
            ew8_paragraph("\\sdslidemarker", ""),
        );
        let song = parse(words.as_bytes()).unwrap();
        assert_eq!(
            song.sections,
            [section("Verse 1", &["a", "b"]), section("Verse 2", &["c"])]
        );
        assert_eq!(
            song.warnings,
            [
                Warning::EmptySection("Verse 1".into()),
                Warning::EmptySection("Chorus".into()),
                Warning::EmptySection("Verse 2".into()),
            ]
        );
    }

    #[test]
    fn words_without_any_text_have_no_lyrics() {
        let words = format!(
            "{{\\rtf1 {}{}}}",
            ew8_paragraph("\\sdparawysiwghidden", "Verse 1"),
            ew8_paragraph("\\sdslidemarker", ""),
        );
        assert_eq!(parse(words.as_bytes()), Err(Rejected::NoLyrics));
        assert_eq!(parse(b"{\\rtf1 \\par \\par }"), Err(Rejected::NoLyrics));
        assert_eq!(parse(b"{\\rtf1}"), Err(Rejected::NoLyrics));
    }

    #[test]
    fn legacy_words_split_on_blank_lines_and_detect_label_lines() {
        let song = parse(&fixture("legacy_blank_lines.rtf")).unwrap();
        assert_eq!(
            song.sections,
            [
                section(
                    "Verse 1",
                    &[
                        "Morning breaks on quiet hills\nRivers carry songs of light",
                        "Open wide the ancient gates\nLet the weary come and rest",
                    ],
                ),
                section("Chorus 1", &["Lift your hands and lift your hearts"]),
                section("Verse 2", &["Lanterns glow along the road\nChorus"]),
            ]
        );
        assert_eq!(song.warnings, Vec::<Warning>::new());
    }

    #[test]
    fn legacy_blocks_without_a_label_become_numbered_verses() {
        assert_eq!(
            sections("{\\rtf1 one a\\par one b\\par\\par two\\par \\par\\par three}"),
            [
                section("Verse 1", &["one a\none b"]),
                section("Verse 2", &["two"]),
                section("Verse 3", &["three"]),
            ]
        );
        assert_eq!(
            sections(
                "{\\rtf1 opening\\par\\par CHORUS\\par sing\\par\\par sing more\\par\\par Interlude\\par}"
            ),
            [
                section("Verse 1", &["opening"]),
                section("CHORUS", &["sing", "sing more", "Interlude"]),
            ]
        );
    }

    #[test]
    fn legacy_label_alone_on_its_block_labels_the_next_block() {
        assert_eq!(
            sections("{\\rtf1 Pre-Chorus 2\\par\\par rising\\par}"),
            [section("Pre-Chorus 2", &["rising"])]
        );
        let song =
            parse(b"{\\rtf1 Tag\\par\\par Verse 3\\par words\\par\\par Ending\\par}").unwrap();
        assert_eq!(song.sections, [section("Verse 3", &["words"])]);
        assert_eq!(
            song.warnings,
            [
                Warning::EmptySection("Tag".into()),
                Warning::EmptySection("Ending".into())
            ]
        );
    }

    #[test]
    fn cp1252_hex_escapes_decode() {
        assert_eq!(
            sections("{\\rtf1\\ansi caf\\'e9 \\'93quoted\\'94 \\'80 \\'9f\\'81}"),
            [section(
                "Verse 1",
                &["caf\u{e9} \u{201c}quoted\u{201d} \u{20ac} \u{178}\u{fffd}"]
            )]
        );
    }

    #[test]
    fn declared_cp1251_and_font_charsets_decode() {
        assert_eq!(
            sections("{\\rtf1\\ansi\\ansicpg1251 \\'cf\\'f0\\'e8\\'a8}"),
            [section("Verse 1", &["\u{41f}\u{440}\u{438}\u{401}"])]
        );
        assert_eq!(
            sections(
                "{\\rtf1\\ansi\\ansicpg1251\\deff0{\\fonttbl{\\f0\\fnil\\fcharset0 Arial;}{\\f1\\fnil\\fcharset204 Arial Cyr;}}\\f1\\'cf {\\f0\\'cf} \\'cf\\plain\\'cf}"
            ),
            [section("Verse 1", &["\u{41f} \u{cf} \u{41f}\u{cf}"])]
        );
    }

    #[test]
    fn undecodable_code_pages_fall_back_to_cp1252_with_a_warning() {
        let song = parse(b"{\\rtf1\\ansi\\ansicpg1253 \\'e1\\'e1}").unwrap();
        assert_eq!(song.sections, [section("Verse 1", &["\u{e1}\u{e1}"])]);
        assert_eq!(
            song.warnings,
            [Warning::UnknownDirective("\\ansicpg1253".into())]
        );
        let song =
            parse(b"{\\rtf1{\\fonttbl{\\f3\\fcharset161 Greek;}}\\f3 \\'e1 plain ascii}").unwrap();
        assert_eq!(song.sections, [section("Verse 1", &["\u{e1} plain ascii"])]);
        assert_eq!(
            song.warnings,
            [Warning::UnknownDirective("\\fcharset161".into())]
        );
        let song = parse(b"{\\rtf1\\ansicpg932 only ascii}").unwrap();
        assert_eq!(song.warnings, Vec::<Warning>::new());
    }

    #[test]
    fn unicode_escapes_skip_their_fallback_characters() {
        assert_eq!(
            sections(
                "{\\rtf1 \\u8364?5 \\u-32682?\\u-10179?\\u-8704? {\\uc2\\u233 xy}\\u233 ? \\uc0\\u233 z\\uc1\\u233\\'e9!}"
            ),
            [section(
                "Verse 1",
                &["\u{20ac}5 \u{8056}\u{1f600} \u{e9}\u{e9} \u{e9}z\u{e9}!"]
            )]
        );
        assert_eq!(
            sections("{\\rtf1 a\\u-8704?b\\u-10179?c\\u-10179?}"),
            [section("Verse 1", &["a\u{fffd}b\u{fffd}c\u{fffd}"])]
        );
    }

    #[test]
    fn control_symbols_and_special_characters_map_to_text() {
        assert_eq!(
            sections(
                "{\\rtf1 \\{a\\}\\\\b\\~c\\_d\\-e\\emdash\\endash\\lquote x\\rquote\\ldblquote y\\rdblquote\\bullet\\tab z\tq\r\nr\\line s\\\r\nt}"
            ),
            [section(
                "Verse 1",
                &[
                    "{a}\\b\u{a0}c\u{2011}de\u{2014}\u{2013}\u{2018}x\u{2019}\u{201c}y\u{201d}\u{2022} z qr\ns\nt"
                ]
            )]
        );
    }

    #[test]
    fn ignorable_destinations_and_tables_are_not_lyrics() {
        assert_eq!(
            sections(
                "{\\rtf1{\\fonttbl{\\f0 Arial;}}{\\colortbl;\\red1\\green2\\blue3;}{\\stylesheet{\\s0 Normal;}}{\\info{\\title T}{\\author A}}{\\*\\generator Writer;}{\\*\\unknowndest{nested} text}\\unknownword kept {\\pict\\bin3 {{{}{\\field{\\*\\fldinst HYPERLINK}{\\fldrslt shown}}}"
            ),
            [section("Verse 1", &["kept shown"])]
        );
    }

    #[test]
    fn dropped_character_formatting_is_reported_once_each() {
        let song = parse(b"{\\rtf1 \\b bold\\b0  \\i it\\i0  \\b again\\b0  \\ul0 plain}").unwrap();
        assert_eq!(
            song.sections,
            [section("Verse 1", &["bold it again plain"])]
        );
        assert_eq!(
            song.warnings,
            [
                Warning::UnknownElement("\\b".into()),
                Warning::UnknownElement("\\i".into())
            ]
        );
    }

    #[test]
    fn malformed_words_are_rejected() {
        for words in [
            "{\\rtf1 open",
            "{\\rtf1 {inner}",
            "{\\rtf1 a}}",
            "{\\rtf1 a} b",
            "{\\rtf1 a\\",
            "{\\rtf1 \\'zz}",
            "{\\rtf1 \\'e}",
        ] {
            assert!(
                matches!(parse(words.as_bytes()), Err(Rejected::Malformed(_))),
                "{words}"
            );
        }
        assert!(matches!(
            parse("{\\rtf1 caf\u{e9}}".as_bytes()),
            Err(Rejected::Malformed(_))
        ));
        assert_eq!(
            sections("{\\rtf1{\\fonttbl{\\f0 Caf\u{e9};}} ok}\r\n\0"),
            [section("Verse 1", &["ok"])]
        );
    }

    #[test]
    fn other_formats_deep_nesting_and_size_are_rejected() {
        assert_eq!(parse(b""), Err(Rejected::WrongFormat));
        assert_eq!(parse(b"Verse 1\nwords"), Err(Rejected::WrongFormat));
        assert_eq!(parse(b"{\\urtf1 a}"), Err(Rejected::WrongFormat));
        let nested = |depth: usize| {
            format!(
                "{{\\rtf1 {}a{}}}",
                "{".repeat(depth - 1),
                "}".repeat(depth - 1)
            )
        };
        assert_eq!(sections(&nested(MAX_DEPTH)), [section("Verse 1", &["a"])]);
        assert_eq!(
            parse(nested(MAX_DEPTH + 1).as_bytes()),
            Err(Rejected::TooDeep)
        );
        let mut huge = b"{\\rtf1 ".to_vec();
        huge.resize(MAX_FILE_BYTES + 1, b'a');
        assert_eq!(parse(&huge), Err(Rejected::TooLarge));
    }

    #[test]
    fn largest_words_stop_at_the_slide_limit() {
        let mut words = String::from("{\\rtf1\\sdeasyworship2 ");
        while words.len() < MAX_FILE_BYTES - 32 {
            words += "x\\par{\\sdslidemarker\\par}";
        }
        words += "x}";
        let song = parse(words.as_bytes()).unwrap();
        let slides: usize = song.sections.iter().map(|s| s.slides.len()).sum();
        assert_eq!(slides, MAX_SLIDES);
        assert_eq!(song.warnings, [Warning::LimitReached]);

        let mut words = String::from("{\\rtf1 ");
        while words.len() < MAX_FILE_BYTES - 32 {
            words += "x\\par\\par ";
        }
        words += "}";
        let song = parse(words.as_bytes()).unwrap();
        assert_eq!(song.sections.len(), MAX_SECTIONS);
        assert_eq!(
            song.sections[MAX_SECTIONS - 1].label,
            format!("Verse {MAX_SECTIONS}")
        );
        assert_eq!(song.warnings, [Warning::LimitReached]);
    }

    fn assert_fast(started: std::time::Instant) {
        let elapsed = started.elapsed();
        assert!(elapsed < std::time::Duration::from_secs(10), "{elapsed:?}");
    }

    #[test]
    fn largest_font_tables_decode_in_linear_time() {
        let started = std::time::Instant::now();
        let mut fonts = String::from("{\\rtf1{\\fonttbl");
        let mut last = 0;
        while fonts.len() < MAX_FILE_BYTES / 2 {
            last += 1;
            fonts += &format!("\\f{last}\\fcharset204 ");
        }
        fonts += &format!("}}\\f{last} ");
        // Each escape decodes to two bytes; the slide stays inside the text
        // budget, so the song is accepted (`fit_to_model`).
        let mut escapes = 0;
        while fonts.len() < MAX_FILE_BYTES - 8 && escapes < (SLIDE_BYTES - 64) / 2 {
            fonts += "\\'e9";
            escapes += 1;
        }
        fonts += "}";
        let song = parse(fonts.as_bytes()).unwrap();
        assert!(song.sections[0].slides[0].starts_with("\u{439}\u{439}"));
        assert_eq!(song.sections[0].slides[0].chars().count(), escapes);
        assert_fast(started);
    }

    #[test]
    fn largest_label_runs_parse_in_linear_time() {
        let started = std::time::Instant::now();
        let mut labels = String::from("{\\rtf1 ");
        while labels.len() < MAX_FILE_BYTES / 2 {
            labels += "\\par ";
        }
        while labels.len() < MAX_FILE_BYTES - 32 {
            labels += "{\\sdparawysiwghidden x\\par}";
        }
        labels += "y}";
        let song = parse(labels.as_bytes()).unwrap();
        assert_eq!(song.sections, [section("x", &["y"])]);
        assert_fast(started);
    }

    fn assert_bounded(input: &[u8], parsed: &Parsed) {
        let Ok(song) = parsed else { return };
        assert!(!song.sections.is_empty());
        assert!(song.sections.len() <= MAX_SECTIONS);
        assert!(song.warnings.len() <= crate::import::MAX_WARNINGS);
        let slides: Vec<&String> = song.sections.iter().flat_map(|s| &s.slides).collect();
        assert!(slides.len() <= MAX_SLIDES);
        for slide in &slides {
            assert!(
                !slide.is_empty() && slide.trim() == slide.as_str(),
                "{slide:?}"
            );
        }
        let text: usize = slides.iter().map(|s| s.len()).sum();
        assert!(
            text <= 2 * input.len(),
            "{text} bytes of text from {}",
            input.len()
        );
    }

    fn rtf_soup() -> impl Strategy<Value = String> {
        const TOKENS: &[&str] = &[
            "{",
            "}",
            "\\par ",
            "\\line ",
            "\\pard ",
            "\\sdslidemarker ",
            "\\sdparawysiwghidden ",
            "\\sdeasyworship2 ",
            "\\u-10179?",
            "\\u-8704?",
            "\\u8364 ",
            "\\uc2 ",
            "\\uc0 ",
            "\\'e9",
            "\\'",
            "\\*",
            "\\fonttbl ",
            "\\f1 ",
            "\\fcharset204 ",
            "\\ansicpg1251 ",
            "\\bin2 ",
            "\\b ",
            "\\",
            "Verse 1",
            "Chorus",
        ];
        let token = prop_oneof![
            prop::sample::select(TOKENS).prop_map(str::to_string),
            "[a-z \\-0-9]{0,6}",
        ];
        proptest::collection::vec(token, 0..200)
            .prop_map(|tokens| format!("{{\\rtf1 {}", tokens.concat()))
    }

    fn line() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9,.!?' \u{e9}\u{f1}\u{8056}\u{1f600}{}\\\\]{1,24}"
            .prop_map(|line| line.trim().to_string())
            .prop_filter("non-empty", |line| !line.is_empty())
    }

    fn ew8_song() -> impl Strategy<Value = Vec<ParsedSection>> {
        let label = prop_oneof![
            Just("Verse 1".to_string()),
            Just("Chorus".to_string()),
            Just("Pre-Chorus 2".to_string()),
            "[A-Za-z\u{e9}]{1,8}( [A-Za-z0-9()]{1,6})?",
        ];
        let slide = proptest::collection::vec(line(), 1..4).prop_map(|lines| lines.join("\n"));
        let section = (label, proptest::collection::vec(slide, 1..4))
            .prop_map(|(label, slides)| ParsedSection { label, slides });
        proptest::collection::vec(section, 1..6)
    }

    fn legacy_song() -> impl Strategy<Value = Vec<ParsedSection>> {
        let label = prop_oneof![
            (1u32..20).prop_map(|n| format!("Verse {n}")),
            Just("Chorus".to_string()),
            Just("Chorus 2".to_string()),
            Just("Bridge".to_string()),
            Just("Intro".to_string()),
            Just("Ending".to_string()),
        ];
        let word =
            prop::sample::select(vec!["light", "river", "morning", "grace", "stone", "open"]);
        let line = proptest::collection::vec(word, 2..5).prop_map(|words| words.join(" "));
        let slide = proptest::collection::vec(line, 1..4).prop_map(|lines| lines.join("\n"));
        let section = (label, proptest::collection::vec(slide, 1..4))
            .prop_map(|(label, slides)| ParsedSection { label, slides });
        proptest::collection::vec(section, 1..6)
    }

    fn legacy_words(sections: &[ParsedSection]) -> String {
        let mut words = String::from(
            "{\\rtf1\\ansi\\ansicpg1252\\deff0{\\fonttbl{\\f0\\fnil\\fcharset0 Arial;}}\r\n\\pard\\li0\\fi0 ",
        );
        for section in sections {
            words += &format!("\\plain\\f0 {}\\par\r\n", section.label);
            for (i, slide) in section.slides.iter().enumerate() {
                if i > 0 {
                    words += "\\par\r\n";
                }
                for line in slide.split('\n') {
                    words += &format!("\\plain\\f0 {line}\\par\r\n");
                }
            }
            words += "\\par\r\n";
        }
        words + "}"
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
                "tests/fixtures/import/ew_rtf/proptest-regressions.txt",
            ))),
            ..ProptestConfig::default()
        })]

        #[test]
        fn arbitrary_bytes_are_bounded(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
            let mut input = b"{\\rtf1 ".to_vec();
            input.extend(&bytes);
            assert_bounded(&bytes, &parse(&bytes));
            assert_bounded(&input, &parse(&input));
        }

        #[test]
        fn rtf_token_soup_is_bounded(words in rtf_soup(), close in any::<bool>()) {
            let words = if close { words + "}" } else { words };
            assert_bounded(words.as_bytes(), &parse(words.as_bytes()));
        }

        #[test]
        fn ew8_words_round_trip(song in ew8_song()) {
            let words = ew8_words(&song);
            let parsed = parse(words.as_bytes()).unwrap();
            prop_assert_eq!(parsed.sections, song);
            prop_assert_eq!(parsed.warnings, Vec::<Warning>::new());
        }

        #[test]
        fn legacy_words_round_trip(song in legacy_song()) {
            let words = legacy_words(&song);
            let parsed = parse(words.as_bytes()).unwrap();
            prop_assert_eq!(parsed.sections, song);
            prop_assert_eq!(parsed.warnings, Vec::<Warning>::new());
        }
    }
}
