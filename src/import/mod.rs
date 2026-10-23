//! Parsers for untrusted song files. Each parser turns one file's bytes into
//! a `ParsedSong` or a `Rejected` reason; none touches storage, the UI thread
//! or the network. The import framework (M1-14) maps a `ParsedSong` onto
//! `storage::Song` and owns staging, duplicates and the commit transaction.
pub mod batch;
pub mod chordpro;
pub mod ew_library;
pub mod ew_rtf;
pub mod openlyrics;
pub mod opensong;
pub mod song;
pub mod worker;

/// Largest song file any parser accepts, checked before parsing.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;
/// Most sections (labelled groups of slides) one song may produce.
pub const MAX_SECTIONS: usize = crate::arrangement::MAX_SECTIONS;
/// Most slides one song may produce across all sections.
pub const MAX_SLIDES: usize = crate::arrangement::MAX_SECTIONS;
/// Most entries in a parsed presentation order.
pub const MAX_ORDER: usize = crate::arrangement::MAX_OCCURRENCES;
/// Most warnings kept per file.
pub const MAX_WARNINGS: usize = 64;

/// One song as written in its source file, before it gets storage IDs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedSong {
    pub title: String,
    pub authors: Vec<String>,
    pub copyright: String,
    /// CCLI song number, ASCII digits only (`storage::song_number_ok`).
    pub song_number: String,
    pub administrator: String,
    /// Sections in file order.
    pub sections: Vec<ParsedSection>,
    /// Presentation order as indices into `sections`; empty means file order.
    /// Parsers resolve source names here and warn about unknown references.
    pub order: Vec<usize>,
    pub warnings: Vec<Warning>,
}

/// A labelled group of slides, such as OpenLyrics `<verse name="v1">`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedSection {
    /// Display label in Sela's vocabulary (`labels::parse`), such as "Verse 1"
    /// or "Chorus"; a source label outside it is kept verbatim.
    pub label: String,
    /// Slide texts with `\n` line breaks; each becomes one `storage::Section`.
    pub slides: Vec<String>,
}

/// Something the parser dropped or guessed. Shown per file in the staged
/// preview, so nothing is lost silently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    UnknownElement(String),
    UnknownDirective(String),
    /// The order named a section the file does not define.
    MissingOrderReference(String),
    /// No title in the file; the caller substitutes the file name.
    MissingTitle,
    EmptySection(String),
    ChordsRemoved,
    /// A field longer than the song model allows was cut.
    Truncated(String),
    /// Text that is not valid in its field, such as a non-numeric CCLI number.
    InvalidField(String),
    /// The section, slide or order limit was reached and the rest was dropped.
    LimitReached,
    /// An attribute the song model has no place for, as `element/@name`
    /// (OpenLyrics `lines/@repeat`, `verse/@lang`); reported once per file.
    IgnoredAttribute(String),
}

/// Why a file produced no song. Other files in the same batch still import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejected {
    TooLarge,
    InvalidUtf8,
    /// Not well formed for its format; the string names the first problem.
    Malformed(String),
    /// A DOCTYPE or entity declaration (XXE and expansion bombs).
    Entities,
    TooDeep,
    /// Well formed, but not this format (for example the wrong root element).
    WrongFormat,
    NoLyrics,
}

pub type Parsed = Result<ParsedSong, Rejected>;

/// Byte budget for section labels and slide text, priced like
/// `arrangement::SourceSnapshot` (24 bytes per slide). The rest of
/// `arrangement::MAX_TEXT_BYTES` covers the capped metadata fields.
pub const SLIDE_BYTES: usize = crate::arrangement::MAX_TEXT_BYTES - 16 * 1024;
/// Song model field limits (`storage::Song::validate`).
pub const MAX_TITLE: usize = 1024;
pub const MAX_TEXT_FIELD: usize = 4096;
pub const MAX_LABEL: usize = crate::arrangement::MAX_NAME_BYTES;

/// `text` cut at a grapheme boundary to at most `max` bytes.
pub fn cut(text: &str, max: usize) -> &str {
    use unicode_segmentation::UnicodeSegmentation;
    if text.len() <= max {
        return text;
    }
    let end = text
        .grapheme_indices(true)
        .map(|(at, g)| at + g.len())
        .take_while(|&end| end <= max)
        .last()
        .unwrap_or(0);
    &text[..end]
}

impl ParsedSong {
    /// Records a warning, keeping at most `MAX_WARNINGS`.
    pub fn warn(&mut self, warning: Warning) {
        if self.warnings.len() < MAX_WARNINGS {
            self.warnings.push(warning);
        }
    }

    /// Reports `MissingTitle`. The caller substitutes the file name on it,
    /// so it goes first and survives a full warning list.
    pub fn warn_missing_title(&mut self) {
        if !self.warnings.contains(&Warning::MissingTitle) {
            self.warnings.insert(0, Warning::MissingTitle);
            self.warnings.truncate(MAX_WARNINGS);
        }
    }

    /// Call after cutting a title the file did have. A title cut to nothing
    /// or to spaces (a single grapheme longer than `MAX_TITLE`, such as one
    /// letter with hundreds of combining marks) is a missing title: it is
    /// cleared and `MissingTitle` reported, so the caller substitutes the
    /// file name instead of accepting a song `Song::validate` refuses
    /// (SEC-09).
    pub fn keep_title_usable(&mut self) {
        if self.title.trim().is_empty() {
            self.title.clear();
            self.warn_missing_title();
        }
    }

    /// Records a warning once per song.
    fn warn_once(&mut self, warning: Warning) {
        if !self.warnings.contains(&warning) {
            self.warn(warning);
        }
    }

    /// Brings a parsed song inside the song model's limits, so every accepted
    /// file can be stored (`song::to_song`): fields over their limit are cut
    /// at a grapheme boundary with `Truncated(field)`; slides past the text
    /// budget, and order entries past `arrangement::MAX_OCCURRENCES` expanded
    /// slides, are dropped with one `LimitReached`. A song whose first slide
    /// alone is over the budget is `TooLarge`, as in OpenLyrics.
    pub fn fit_to_model(&mut self) -> Result<(), Rejected> {
        let fit = |text: &mut String, max: usize, field: &str, song: &mut ParsedSong| {
            if text.len() > max {
                let kept = cut(text, max).len();
                text.truncate(kept);
                song.warn_once(Warning::Truncated(field.to_string()));
            }
        };
        let mut title = std::mem::take(&mut self.title);
        let named = !title.is_empty();
        fit(&mut title, MAX_TITLE, "title", self);
        self.title = title;
        if named {
            self.keep_title_usable();
        }
        let mut copyright = std::mem::take(&mut self.copyright);
        fit(&mut copyright, MAX_TEXT_FIELD, "copyright", self);
        self.copyright = copyright;
        let mut administrator = std::mem::take(&mut self.administrator);
        fit(
            &mut administrator,
            crate::storage::MAX_ADMINISTRATOR,
            "administrator",
            self,
        );
        self.administrator = administrator;

        let mut joined = 0;
        let mut kept = 0;
        let mut authors_cut = false;
        for author in &mut self.authors {
            let separator = if kept == 0 {
                0
            } else {
                song::AUTHOR_SEPARATOR.len()
            };
            if joined + separator + author.len() > MAX_TEXT_FIELD {
                if kept == 0 {
                    let room = cut(author, MAX_TEXT_FIELD).len();
                    author.truncate(room);
                    kept = 1;
                }
                authors_cut = true;
                break;
            }
            joined += separator + author.len();
            kept += 1;
        }
        if authors_cut {
            self.authors.truncate(kept);
            self.warn_once(Warning::Truncated("authors".into()));
        }

        let mut labels_cut = false;
        for section in &mut self.sections {
            if section.label.len() > MAX_LABEL {
                let room = cut(&section.label, MAX_LABEL).len();
                section.label.truncate(room);
                labels_cut = true;
            }
        }
        if labels_cut {
            self.warn_once(Warning::Truncated("section label".into()));
        }

        // Slides are dropped from the end only, so every kept section keeps
        // its index and the order needs no remapping.
        let mut bytes = 0usize;
        let mut over_budget = None;
        'budget: for (at, section) in self.sections.iter_mut().enumerate() {
            for (index, slide) in section.slides.iter().enumerate() {
                let cost = section.label.len() + slide.len() + 24;
                if bytes + cost > SLIDE_BYTES {
                    section.slides.truncate(index);
                    over_budget = Some(if index > 0 { at + 1 } else { at });
                    break 'budget;
                }
                bytes += cost;
            }
        }
        if let Some(kept) = over_budget {
            self.sections.truncate(kept);
            self.order.retain(|&i| i < kept);
            self.warn_once(Warning::LimitReached);
        }
        if self.sections.is_empty() {
            return Err(Rejected::TooLarge);
        }

        let mut expanded = 0;
        let mut entries = 0;
        for &index in &self.order {
            let count = self.sections.get(index).map_or(0, |s| s.slides.len());
            if expanded + count > crate::arrangement::MAX_OCCURRENCES {
                break;
            }
            expanded += count;
            entries += 1;
        }
        if entries < self.order.len() {
            self.order.truncate(entries);
            self.warn_once(Warning::LimitReached);
        }
        Ok(())
    }
}

/// Text of a file after the size check, with one UTF-8 BOM removed and
/// CRLF/CR normalised to LF.
pub fn decode(bytes: &[u8]) -> Result<String, Rejected> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(Rejected::TooLarge);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| Rejected::InvalidUtf8)?;
    Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}

/// The Sela label for a section code shared by OpenLyrics and OpenSong, such
/// as `v1` -> "Verse 1", `c` -> "Chorus", `p2` -> "Pre-Chorus 2". Unknown
/// codes return `None` so the caller keeps the source name.
pub fn label_for_code(code: &str) -> Option<String> {
    let mut chars = code.trim().chars();
    let kind = match chars.next()?.to_ascii_lowercase() {
        'v' => "Verse",
        'c' => "Chorus",
        'b' => "Bridge",
        'p' => "Pre-Chorus",
        'e' => "Ending",
        'i' => "Intro",
        't' => "Tag",
        'o' | 'm' => "Misc",
        _ => return None,
    };
    let rest = chars.as_str();
    if rest.is_empty() {
        return Some(kind.to_string());
    }
    // OpenLyrics part suffixes ("v1a", "v1b") share their verse's label; a
    // word such as "Coda" is not a code.
    let number = rest.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let number: u32 = number.parse().ok()?;
    Some(format!("{kind} {number}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_strips_bom_and_normalises_newlines() {
        assert_eq!(decode(b"\xEF\xBB\xBFa\r\nb\rc\n").unwrap(), "a\nb\nc\n");
        assert_eq!(decode(b"\xff"), Err(Rejected::InvalidUtf8));
        assert_eq!(
            decode(&vec![b'a'; MAX_FILE_BYTES + 1]),
            Err(Rejected::TooLarge)
        );
    }

    #[test]
    fn codes_map_to_sela_labels() {
        assert_eq!(label_for_code("v1").as_deref(), Some("Verse 1"));
        assert_eq!(label_for_code("C").as_deref(), Some("Chorus"));
        assert_eq!(label_for_code("p2").as_deref(), Some("Pre-Chorus 2"));
        assert_eq!(label_for_code("v1a").as_deref(), Some("Verse 1"));
        assert_eq!(label_for_code("x1"), None);
        assert_eq!(label_for_code("Coda"), None);
        assert_eq!(label_for_code("verse"), None);
        assert_eq!(label_for_code("va"), None);
        assert_eq!(label_for_code("v1.5"), None);
        assert_eq!(label_for_code(""), None);
        for code in ["v2", "c", "b1", "p", "e", "i", "t", "o"] {
            let label = label_for_code(code).unwrap();
            assert!(crate::labels::parse(&label).is_some(), "{label}");
        }
    }

    fn storable(parsed: &ParsedSong) -> bool {
        let mut named = parsed.clone();
        if named.title.is_empty() {
            named.title = "file name".into();
        }
        song::to_song(&named).is_ok()
    }

    fn slide_text(song: &ParsedSong) -> usize {
        song.sections
            .iter()
            .flat_map(|s| &s.slides)
            .map(String::len)
            .sum()
    }

    // M5-02 fuzz finding: OpenSong, ChordPro and EasyWorship words had no
    // text budget, so a large file parsed fine but could never be stored
    // (`Song::validate` caps a song at 256 KiB) and failed after review.
    #[test]
    fn large_files_are_cut_to_the_text_budget_and_stay_storable() {
        // 100 slides of 200 lines each: about 320 KiB of text, under the
        // slide count limit but over the 240 KiB text budget.
        let slide = " word word word\n".repeat(200);
        let opensong = format!(
            "<song><title>T</title><lyrics>[V1]\n{}</lyrics></song>",
            format!("{slide}||\n").repeat(100)
        );
        let chordpro = format!("{{title: T}}\n{}", format!("{slide}\n").repeat(100));
        let ew_rtf = format!(
            "{{\\rtf1 {}}}",
            format!("{}\\par ", "word word word\\par ".repeat(200)).repeat(100)
        );
        for parsed in [
            opensong::parse(opensong.as_bytes()),
            chordpro::parse(chordpro.as_bytes()),
            ew_rtf::parse(ew_rtf.as_bytes()),
        ] {
            let parsed = parsed.unwrap();
            assert!(parsed.warnings.contains(&Warning::LimitReached));
            assert!(slide_text(&parsed) <= SLIDE_BYTES);
            assert!(storable(&parsed), "{:?}", parsed.warnings);
        }
        // One slide over the whole budget cannot be cut sensibly.
        let one_slide = format!("{{title: T}}\n{}", "word ".repeat(60_000));
        assert_eq!(
            chordpro::parse(one_slide.as_bytes()),
            Err(Rejected::TooLarge)
        );
    }

    // M5-02 fuzz finding: OpenSong and ChordPro kept titles, authors and
    // copyrights of any length, which `Song::validate` refuses.
    #[test]
    fn long_metadata_is_cut_at_a_grapheme_boundary_and_reported() {
        let long = "e\u{301}".repeat(3000);
        let chordpro = format!(
            "{{title: {long}}}\n{{copyright: {long}}}\n{{artist: {long}}}\n{{artist: B}}\nA line\n"
        );
        let parsed = chordpro::parse(chordpro.as_bytes()).unwrap();
        assert_eq!(parsed.title.len(), MAX_TITLE - MAX_TITLE % 3);
        assert!(parsed.copyright.len() <= MAX_TEXT_FIELD);
        assert!(parsed.copyright.ends_with("e\u{301}"), "no split grapheme");
        assert_eq!(parsed.authors.len(), 1);
        for field in ["title", "copyright", "authors"] {
            assert!(parsed.warnings.contains(&Warning::Truncated(field.into())));
        }
        assert!(storable(&parsed));
        let opensong = format!(
            "<song><title>{long}</title><copyright>{long}</copyright><lyrics>[{}]\n a\n</lyrics></song>",
            "L".repeat(300)
        );
        let parsed = opensong::parse(opensong.as_bytes()).unwrap();
        assert!(parsed.title.len() <= MAX_TITLE && parsed.sections[0].label.len() == MAX_LABEL);
        assert!(
            parsed
                .warnings
                .contains(&Warning::Truncated("section label".into()))
        );
        assert!(storable(&parsed));
    }

    // SEC-09 (fuzz-audit review): a title of one grapheme over the title
    // limit was cut to "" with no MissingTitle, so the file name was never
    // substituted and the accepted song could not be stored.
    #[test]
    fn a_title_cut_to_nothing_is_reported_missing() {
        let grapheme = format!("a{}", "\u{301}".repeat(600));
        assert!(grapheme.len() > MAX_TITLE && cut(&grapheme, MAX_TITLE).is_empty());
        for title in [grapheme.clone(), format!("  {grapheme}")] {
            for parsed in [
                chordpro::parse(format!("{{title: {title}}}\nA line\n").as_bytes()),
                opensong::parse(
                    format!("<song><title>{title}</title><lyrics>[V1]\n a\n</lyrics></song>")
                        .as_bytes(),
                ),
                openlyrics::parse(
                    format!(
                        "<song xmlns=\"http://openlyrics.info/namespace/2009/song\" version=\"0.9\">\
                         <properties><titles><title>{title}</title></titles></properties>\
                         <lyrics><verse name=\"v1\"><lines>a</lines></verse></lyrics></song>"
                    )
                    .as_bytes(),
                ),
            ] {
                let parsed = parsed.unwrap();
                assert_eq!(parsed.title, "");
                assert_eq!(parsed.warnings[0], Warning::MissingTitle, "{:?}", parsed.warnings);
                assert!(parsed.warnings.contains(&Warning::Truncated("title".into())));
                assert!(storable(&parsed));
            }
        }
    }

    // M5-02 fuzz finding: an OpenSong or ChordPro order could expand into
    // more slides than an arrangement holds, so the song never stored.
    #[test]
    fn the_order_is_cut_at_the_arrangement_slide_limit() {
        let slides = "a\n\n".repeat(10);
        let text = format!("{{soc}}\n{slides}{{eoc}}\n{}", "{chorus}\n".repeat(100));
        let parsed = chordpro::parse(text.as_bytes()).unwrap();
        let expanded: usize = parsed
            .order
            .iter()
            .map(|&i| parsed.sections[i].slides.len())
            .sum();
        assert!(expanded <= crate::arrangement::MAX_OCCURRENCES);
        assert!(parsed.warnings.contains(&Warning::LimitReached));
        assert!(storable(&parsed));
    }

    // M5-02 fuzz finding: once 64 other warnings filled the list, OpenSong
    // and ChordPro dropped MissingTitle, so the caller never substituted the
    // file name and the song failed to store with an empty title.
    #[test]
    fn missing_title_survives_a_full_warning_list() {
        let directives: String = (0..70).map(|n| format!("{{unknown{n}: x}}\n")).collect();
        let elements: String = (0..70).map(|n| format!("<extra{n}>x</extra{n}>")).collect();
        for parsed in [
            chordpro::parse(format!("{directives}A line\n").as_bytes()),
            opensong::parse(
                format!("<song>{elements}<lyrics>[V1]\n a\n</lyrics></song>").as_bytes(),
            ),
        ] {
            let parsed = parsed.unwrap();
            assert_eq!(parsed.warnings.len(), MAX_WARNINGS);
            assert_eq!(parsed.warnings[0], Warning::MissingTitle);
        }
    }
}
