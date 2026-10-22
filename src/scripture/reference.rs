//! Scripture reference parsing (M4-04). The accepted syntax and the book
//! resolution rule are in docs/scripture.md.
use super::books::{self, Book};

/// A chapter and verse inside one book.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Verse {
    pub chapter: u16,
    pub verse: u16,
}

/// The part of a book a reference covers, inclusive at both ends. A single
/// chapter or verse has equal ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Span {
    Chapters { first: u16, last: u16 },
    Verses { start: Verse, end: Verse },
}

/// A book and a span whose chapters exist in that book (KJV versification)
/// and whose end does not come before its start. Verse numbers are not
/// checked against a translation here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reference {
    book: Book,
    span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Nothing but whitespace was typed.
    Empty,
    /// The book text, as typed, names no book.
    UnknownBook(String),
    /// A book alone, without a chapter.
    MissingChapter(Book),
    ChapterNotFound {
        book: Book,
        chapter: u16,
    },
    /// The end of the range comes before its start.
    InvalidRange,
    /// Text that is not a reference: no book name, stray characters, a
    /// missing number or verse 0.
    Malformed,
}

impl Reference {
    pub fn new(book: Book, span: Span) -> Result<Reference, ParseError> {
        let (first, last) = match span {
            Span::Chapters { first, last } => (first, last),
            Span::Verses { start, end } => (start.chapter, end.chapter),
        };
        if let Some(chapter) = [first, last]
            .into_iter()
            .find(|chapter| !(1..=book.chapters()).contains(chapter))
        {
            return Err(ParseError::ChapterNotFound { book, chapter });
        }
        let ordered = match span {
            Span::Chapters { first, last } => first <= last,
            Span::Verses { start, end } => {
                if start.verse == 0 || end.verse == 0 {
                    return Err(ParseError::Malformed);
                }
                start <= end
            }
        };
        if !ordered {
            return Err(ParseError::InvalidRange);
        }
        Ok(Reference { book, span })
    }

    pub fn book(&self) -> Book {
        self.book
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Clone, Copy)]
enum Token {
    Number(u16),
    Separator,
    Dash,
}

/// Parses a typed reference such as `John 3:16-4:2`, `1 Kor 13`, `Mzm 23.1`
/// or `kisah para rasul 2:1-4`.
pub fn parse(input: &str) -> Result<Reference, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ParseError::Empty);
    }
    let first_letter = input
        .find(char::is_alphabetic)
        .ok_or(ParseError::Malformed)?;
    let (lead, rest) = input.split_at(first_letter);
    if !lead
        .chars()
        .all(|c| c.is_ascii_digit() || c.is_whitespace() || c == '.')
    {
        return Err(ParseError::Malformed);
    }
    let split = first_letter
        + rest
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or(rest.len());
    let (name, locator) = input.split_at(split);
    let words = &name[first_letter..];
    // A hyphen belongs to names such as Hakim-hakim, never before the chapter.
    if words.trim_end().ends_with('-')
        || !words
            .chars()
            .all(|c| c.is_alphabetic() || c.is_whitespace() || matches!(c, '.' | '-' | '\''))
    {
        return Err(ParseError::Malformed);
    }
    let book =
        books::resolve(name).ok_or_else(|| ParseError::UnknownBook(name.trim().to_owned()))?;
    if locator.is_empty() {
        return Err(ParseError::MissingChapter(book));
    }
    let span = match tokens(locator)?[..] {
        [Token::Number(chapter)] => Span::Chapters {
            first: chapter,
            last: chapter,
        },
        [Token::Number(first), Token::Dash, Token::Number(last)] => Span::Chapters { first, last },
        [
            Token::Number(chapter),
            Token::Separator,
            Token::Number(verse),
        ] => {
            let verse = Verse { chapter, verse };
            Span::Verses {
                start: verse,
                end: verse,
            }
        }
        [
            Token::Number(chapter),
            Token::Separator,
            Token::Number(first),
            Token::Dash,
            Token::Number(last),
        ] => Span::Verses {
            start: Verse {
                chapter,
                verse: first,
            },
            end: Verse {
                chapter,
                verse: last,
            },
        },
        [
            Token::Number(start_chapter),
            Token::Separator,
            Token::Number(start_verse),
            Token::Dash,
            Token::Number(end_chapter),
            Token::Separator,
            Token::Number(end_verse),
        ] => Span::Verses {
            start: Verse {
                chapter: start_chapter,
                verse: start_verse,
            },
            end: Verse {
                chapter: end_chapter,
                verse: end_verse,
            },
        },
        _ => return Err(ParseError::Malformed),
    };
    Reference::new(book, span)
}

fn tokens(locator: &str) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut chars = locator.chars().filter(|c| !c.is_whitespace()).peekable();
    while let Some(c) = chars.next() {
        tokens.push(match c {
            ':' | '.' => Token::Separator,
            '-' | '\u{2013}' | '\u{2014}' => Token::Dash,
            '0'..='9' => {
                // Overlong numbers saturate so they still fail as a missing
                // chapter (or a verse the translation lacks), not as malformed.
                let mut number = c.to_digit(10).unwrap_or(0) as u16;
                while let Some(digit) = chars.peek().and_then(|c| c.to_digit(10)) {
                    number = number.saturating_mul(10).saturating_add(digit as u16);
                    chars.next();
                }
                Token::Number(number)
            }
            _ => return Err(ParseError::Malformed),
        });
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (canonical number, chapters, English name, English abbreviation,
    /// Indonesian name, Indonesian abbreviation), written independently of
    /// the book table.
    const CANON: [(u8, u16, &str, &str, &str, &str); 66] = [
        (1, 50, "Genesis", "Gen", "Kejadian", "Kej"),
        (2, 40, "Exodus", "Ex", "Keluaran", "Kel"),
        (3, 27, "Leviticus", "Lev", "Imamat", "Im"),
        (4, 36, "Numbers", "Num", "Bilangan", "Bil"),
        (5, 34, "Deuteronomy", "Deut", "Ulangan", "Ul"),
        (6, 24, "Joshua", "Josh", "Yosua", "Yos"),
        (7, 21, "Judges", "Judg", "Hakim-hakim", "Hak"),
        (8, 4, "Ruth", "Ru", "Rut", "Rut"),
        (9, 31, "1 Samuel", "1 Sam", "1 Samuel", "1 Sam"),
        (10, 24, "2 Samuel", "2 Sam", "2 Samuel", "2 Sam"),
        (11, 22, "1 Kings", "1 Kgs", "1 Raja-raja", "1 Raj"),
        (12, 25, "2 Kings", "2 Kgs", "2 Raja-raja", "2 Raj"),
        (13, 29, "1 Chronicles", "1 Chr", "1 Tawarikh", "1 Taw"),
        (14, 36, "2 Chronicles", "2 Chr", "2 Tawarikh", "2 Taw"),
        (15, 10, "Ezra", "Ezr", "Ezra", "Ezr"),
        (16, 13, "Nehemiah", "Neh", "Nehemia", "Neh"),
        (17, 10, "Esther", "Esth", "Ester", "Est"),
        (18, 42, "Job", "Jb", "Ayub", "Ayb"),
        (19, 150, "Psalms", "Ps", "Mazmur", "Mzm"),
        (20, 31, "Proverbs", "Prov", "Amsal", "Ams"),
        (21, 12, "Ecclesiastes", "Eccl", "Pengkhotbah", "Pkh"),
        (22, 8, "Song of Songs", "Song", "Kidung Agung", "Kid"),
        (23, 66, "Isaiah", "Isa", "Yesaya", "Yes"),
        (24, 52, "Jeremiah", "Jer", "Yeremia", "Yer"),
        (25, 5, "Lamentations", "Lam", "Ratapan", "Rat"),
        (26, 48, "Ezekiel", "Ezek", "Yehezkiel", "Yeh"),
        (27, 12, "Daniel", "Dan", "Daniel", "Dan"),
        (28, 14, "Hosea", "Hos", "Hosea", "Hos"),
        (29, 3, "Joel", "Jl", "Yoel", "Yl"),
        (30, 9, "Amos", "Am", "Amos", "Am"),
        (31, 1, "Obadiah", "Obad", "Obaja", "Ob"),
        (32, 4, "Jonah", "Jon", "Yunus", "Yun"),
        (33, 7, "Micah", "Mic", "Mikha", "Mi"),
        (34, 3, "Nahum", "Nah", "Nahum", "Nah"),
        (35, 3, "Habakkuk", "Hab", "Habakuk", "Hab"),
        (36, 3, "Zephaniah", "Zeph", "Zefanya", "Zef"),
        (37, 2, "Haggai", "Hag", "Hagai", "Hag"),
        (38, 14, "Zechariah", "Zech", "Zakharia", "Za"),
        (39, 4, "Malachi", "Mal", "Maleakhi", "Mal"),
        (40, 28, "Matthew", "Matt", "Matius", "Mat"),
        (41, 16, "Mark", "Mk", "Markus", "Mrk"),
        (42, 24, "Luke", "Lk", "Lukas", "Luk"),
        (43, 21, "John", "Jn", "Yohanes", "Yoh"),
        (44, 28, "Acts", "Ac", "Kisah Para Rasul", "Kis"),
        (45, 16, "Romans", "Rom", "Roma", "Rm"),
        (46, 16, "1 Corinthians", "1 Cor", "1 Korintus", "1 Kor"),
        (47, 13, "2 Corinthians", "2 Cor", "2 Korintus", "2 Kor"),
        (48, 6, "Galatians", "Gal", "Galatia", "Gal"),
        (49, 6, "Ephesians", "Eph", "Efesus", "Ef"),
        (50, 4, "Philippians", "Phil", "Filipi", "Flp"),
        (51, 4, "Colossians", "Col", "Kolose", "Kol"),
        (52, 5, "1 Thessalonians", "1 Thess", "1 Tesalonika", "1 Tes"),
        (53, 3, "2 Thessalonians", "2 Thess", "2 Tesalonika", "2 Tes"),
        (54, 6, "1 Timothy", "1 Tim", "1 Timotius", "1 Tim"),
        (55, 4, "2 Timothy", "2 Tim", "2 Timotius", "2 Tim"),
        (56, 3, "Titus", "Tit", "Titus", "Tit"),
        (57, 1, "Philemon", "Phlm", "Filemon", "Flm"),
        (58, 13, "Hebrews", "Heb", "Ibrani", "Ibr"),
        (59, 5, "James", "Jas", "Yakobus", "Yak"),
        (60, 5, "1 Peter", "1 Pet", "1 Petrus", "1 Ptr"),
        (61, 3, "2 Peter", "2 Pet", "2 Petrus", "2 Ptr"),
        (62, 5, "1 John", "1 Jn", "1 Yohanes", "1 Yoh"),
        (63, 1, "2 John", "2 Jn", "2 Yohanes", "2 Yoh"),
        (64, 1, "3 John", "3 Jn", "3 Yohanes", "3 Yoh"),
        (65, 1, "Jude", "Jde", "Yudas", "Yud"),
        (66, 22, "Revelation", "Rev", "Wahyu", "Why"),
    ];

    fn book(number: u8) -> Book {
        Book::from_number(number).unwrap()
    }

    fn verses(book_number: u8, start: (u16, u16), end: (u16, u16)) -> Reference {
        let verse = |(chapter, verse)| Verse { chapter, verse };
        Reference::new(
            book(book_number),
            Span::Verses {
                start: verse(start),
                end: verse(end),
            },
        )
        .unwrap()
    }

    fn chapters(book_number: u8, first: u16, last: u16) -> Reference {
        Reference::new(book(book_number), Span::Chapters { first, last }).unwrap()
    }

    fn book_of(input: &str) -> Result<u8, ParseError> {
        parse(input).map(|reference| reference.book().number())
    }

    #[test]
    fn every_book_resolves_by_english_and_indonesian_names_and_abbreviations() {
        for (number, _, english, english_abbr, indonesian, indonesian_abbr) in CANON {
            for name in [english, english_abbr, indonesian, indonesian_abbr] {
                let mut spellings = vec![
                    name.to_owned(),
                    name.to_lowercase(),
                    name.to_uppercase(),
                    format!("{name}."),
                ];
                if name.starts_with(|c: char| c.is_ascii_digit()) {
                    spellings.push(name.replacen(' ', "", 1));
                }
                for spelling in spellings {
                    assert_eq!(
                        parse(&format!("{spelling} 1:1")),
                        Ok(verses(number, (1, 1), (1, 1))),
                        "{spelling}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_book_accepts_its_last_chapter_and_rejects_the_next() {
        for (number, last, english, _, _, _) in CANON {
            assert_eq!(
                parse(&format!("{english} {last}")),
                Ok(chapters(number, last, last))
            );
            assert_eq!(
                parse(&format!("{english} {}", last + 1)),
                Err(ParseError::ChapterNotFound {
                    book: book(number),
                    chapter: last + 1
                }),
                "{english}"
            );
        }
    }

    #[test]
    fn numbered_books_parse_with_or_without_a_space() {
        for input in [
            "1Cor 13:4",
            "1 Cor 13:4",
            "1 Kor 13:4",
            "1Kor 13:4",
            "1Cor13:4",
            "1 korintus13:4",
            "1. Cor 13:4",
        ] {
            assert_eq!(parse(input), Ok(verses(46, (13, 4), (13, 4))), "{input}");
        }
        assert_eq!(parse("2Raja-raja 5:1"), Ok(verses(12, (5, 1), (5, 1))));
        assert_eq!(parse("3 Yoh 1:4"), Ok(verses(64, (1, 4), (1, 4))));
    }

    #[test]
    fn each_reference_form_parses() {
        assert_eq!(parse("John 3"), Ok(chapters(43, 3, 3)));
        assert_eq!(parse("John 3-4"), Ok(chapters(43, 3, 4)));
        assert_eq!(parse("John 3:16"), Ok(verses(43, (3, 16), (3, 16))));
        assert_eq!(parse("John 3:16-18"), Ok(verses(43, (3, 16), (3, 18))));
        assert_eq!(parse("John 3:16-4:2"), Ok(verses(43, (3, 16), (4, 2))));
        assert_eq!(parse("John 3:16-3:18"), Ok(verses(43, (3, 16), (3, 18))));
        assert_eq!(parse("John 3:16-16"), Ok(verses(43, (3, 16), (3, 16))));
        assert_eq!(
            parse("Yoh 3:16\u{2013}18"),
            Ok(verses(43, (3, 16), (3, 18)))
        );
    }

    #[test]
    fn a_dot_separates_chapter_and_verse_like_a_colon() {
        assert_eq!(parse("John 3.16"), Ok(verses(43, (3, 16), (3, 16))));
        assert_eq!(parse("Mzm 23.1-6"), Ok(verses(19, (23, 1), (23, 6))));
        assert_eq!(parse("Gen. 1.1-2.3"), Ok(verses(1, (1, 1), (2, 3))));
    }

    #[test]
    fn case_and_whitespace_do_not_matter() {
        for input in [
            "john 3:16-4:2",
            "JOHN 3:16-4:2",
            "  jOhN   3 : 16 - 4 : 2  ",
            "John3:16-4:2",
            "john\t3:16 -4:2",
            "YOHANES 3:16-4:2",
        ] {
            assert_eq!(parse(input), Ok(verses(43, (3, 16), (4, 2))), "{input:?}");
        }
        assert_eq!(
            parse("kisah  para   rasul 2:1-4"),
            Ok(verses(44, (2, 1), (2, 4)))
        );
        assert_eq!(parse("song of solomon 2"), Ok(chapters(22, 2, 2)));
        assert_eq!(parse("KIDUNG AGUNG 2"), Ok(chapters(22, 2, 2)));
    }

    #[test]
    fn verses_beyond_the_chapter_parse_for_the_translation_to_check() {
        assert_eq!(parse("John 3:99"), Ok(verses(43, (3, 99), (3, 99))));
    }

    #[test]
    fn chapters_that_do_not_exist_are_not_found() {
        let not_found = |chapter| {
            Err(ParseError::ChapterNotFound {
                book: book(43),
                chapter,
            })
        };
        assert_eq!(parse("John 30:1"), not_found(30));
        assert_eq!(parse("John 0:1"), not_found(0));
        assert_eq!(parse("John 3:16-30:1"), not_found(30));
        assert_eq!(parse("John 20-22"), not_found(22));
        assert_eq!(parse("John 99999999:1"), not_found(u16::MAX));
    }

    #[test]
    fn ranges_that_go_backwards_are_invalid() {
        for input in [
            "John 3:18-16",
            "John 4:2-3:16",
            "John 4-3",
            "John 3:18-3:16",
        ] {
            assert_eq!(parse(input), Err(ParseError::InvalidRange), "{input}");
        }
    }

    #[test]
    fn unknown_books_report_the_typed_name() {
        for (input, name) in [
            ("Hezekiah 1:1", "Hezekiah"),
            ("  Xyz  3", "Xyz"),
            ("I Corinthians 13", "I Corinthians"),
        ] {
            assert_eq!(
                parse(input),
                Err(ParseError::UnknownBook(name.to_owned())),
                "{input}"
            );
        }
    }

    #[test]
    fn empty_input_and_a_book_alone_are_reported() {
        assert_eq!(parse(""), Err(ParseError::Empty));
        assert_eq!(parse(" \t "), Err(ParseError::Empty));
        assert_eq!(parse("John"), Err(ParseError::MissingChapter(book(43))));
        assert_eq!(parse(" 1 kor "), Err(ParseError::MissingChapter(book(46))));
    }

    #[test]
    fn text_that_is_not_a_reference_is_malformed() {
        for input in [
            "3:16",
            "John 3:",
            "John :16",
            "John 3::16",
            "John 3:16-",
            "John -3",
            "John - 3",
            "John 3-4:2",
            "John 3:16,18",
            "John 3:16a",
            "John 3:16-4:2-5",
            "John 3:0",
            "John 3:0-4",
            ":John 3:16",
            "Jo#hn 3:16",
        ] {
            assert_eq!(parse(input), Err(ParseError::Malformed), "{input}");
        }
    }

    #[test]
    fn ambiguous_prefixes_resolve_to_an_exact_alias_then_canonical_order() {
        assert_eq!(book_of("Jo 1:1"), Ok(6));
        assert_eq!(book_of("Ju 1:1"), Ok(7));
        assert_eq!(book_of("Ph 1:1"), Ok(50));
        assert_eq!(book_of("Ma 1:1"), Ok(19));
        assert_eq!(book_of("Yo 1:1"), Ok(6));
        assert_eq!(book_of("Am 1:1"), Ok(30));
        assert_eq!(book_of("Phil 1:1"), Ok(50));
        assert_eq!(book_of("Joh 1:1"), Ok(43));
        assert_eq!(book_of("1 The 1:1"), Ok(52));
    }
}
