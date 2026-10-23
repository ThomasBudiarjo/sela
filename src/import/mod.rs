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

impl ParsedSong {
    /// Records a warning, keeping at most `MAX_WARNINGS`.
    pub fn warn(&mut self, warning: Warning) {
        if self.warnings.len() < MAX_WARNINGS {
            self.warnings.push(warning);
        }
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
}
