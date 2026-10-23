//! Typed scripture references (`scripture::reference::parse`) and book
//! completion. The operator types these, and imported scripture files will
//! carry them, so any text must give a valid reference or a typed error.
use super::{config, corpus, within};
use crate::i18n::Language;
use crate::scripture::{self as reference, Book, ParseError, Reference, Span, Verse};
use proptest::prelude::*;
use std::time::Duration;

/// A reference as an operator would type it.
fn typed(name: &str, span: Span, separator: char, dash: &str) -> String {
    match span {
        Span::Chapters { first, last } if first == last => format!("{name} {first}"),
        Span::Chapters { first, last } => format!("{name} {first}{dash}{last}"),
        Span::Verses { start, end } if start == end => {
            format!("{name} {}{separator}{}", start.chapter, start.verse)
        }
        Span::Verses { start, end } if start.chapter == end.chapter => format!(
            "{name} {}{separator}{}{dash}{}",
            start.chapter, start.verse, end.verse
        ),
        Span::Verses { start, end } => format!(
            "{name} {}{separator}{}{dash}{}{separator}{}",
            start.chapter, start.verse, end.chapter, end.verse
        ),
    }
}

fn check(input: &str) {
    let parsed = within(Duration::from_secs(2), "reference", || {
        reference::parse(input)
    });
    assert_eq!(reference::parse(input), parsed, "same input, same result");
    match parsed {
        Ok(found) => {
            // Whatever was accepted is a valid reference in its own right,
            // and its canonical spelling reads back as the same reference.
            assert_eq!(Reference::new(found.book(), found.span()), Ok(found));
            let canonical = typed(found.book().name(Language::English), found.span(), ':', "-");
            assert_eq!(reference::parse(&canonical), Ok(found), "{canonical}");
        }
        Err(ParseError::ChapterNotFound { book, chapter }) => {
            assert!(chapter == 0 || chapter > book.chapters(), "{input:?}");
        }
        Err(ParseError::UnknownBook(name)) => assert!(!name.is_empty()),
        Err(_) => {}
    }
}

const TOKENS: &[&str] = &[
    "John",
    "Yoh",
    "1",
    "2",
    "3",
    "16",
    "150",
    "0",
    "65535",
    "99999999",
    " ",
    ":",
    ".",
    "-",
    "\u{2013}",
    "\u{2014}",
    "Kor",
    "1 Kor",
    "Song of Songs",
    "Hakim-hakim",
    "kisah para rasul",
    "Ps",
    "Mzm",
    "'",
    "\u{3000}",
    "\u{a0}",
    "\u{301}",
    "\u{345}",
    "\u{130}",
    "ǅ",
    "x",
    ",",
    "\t",
    "\u{1f600}",
];

fn spans(book: Book) -> impl Strategy<Value = Span> {
    let chapters = book.chapters();
    prop_oneof![
        (1..=chapters, 1..=chapters).prop_map(|(a, b)| Span::Chapters {
            first: a.min(b),
            last: a.max(b),
        }),
        (1..=chapters, 1..=chapters, 1u16..=176, 1u16..=176).prop_map(|(a, b, v, w)| {
            let start = Verse {
                chapter: a.min(b),
                verse: v,
            };
            let end = Verse {
                chapter: a.max(b),
                verse: w,
            };
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            Span::Verses { start, end }
        }),
    ]
}

fn books_and_spans() -> impl Strategy<Value = (Book, Span)> {
    (1u8..=66)
        .prop_map(|n| Book::from_number(n).unwrap())
        .prop_flat_map(|book| (Just(book), spans(book)))
}

proptest! {
    #![proptest_config(config(0x5e1a_0201, 512, "tests/fixtures/fuzz/scripture.proptest-regressions"))]

    #[test]
    fn any_text_gives_a_valid_reference_or_an_error(input in any::<String>()) {
        check(&input);
    }

    #[test]
    fn reference_token_soup_gives_a_valid_reference_or_an_error(
        tokens in proptest::collection::vec(proptest::sample::select(TOKENS), 0..12),
    ) {
        check(&tokens.concat());
    }

    #[test]
    fn every_spelling_of_a_reference_reads_back(
        (book, span) in books_and_spans(),
        language in prop_oneof![Just(Language::English), Just(Language::Indonesian)],
        abbreviated in any::<bool>(),
        separator in prop_oneof![Just(':'), Just('.')],
        dash in prop_oneof![Just("-"), Just("\u{2013}"), Just(" - ")],
        case in 0u8..3,
    ) {
        let name = if abbreviated { book.abbreviation(language) } else { book.name(language) };
        let name = match case {
            0 => name.to_string(),
            1 => name.to_lowercase(),
            _ => name.to_uppercase(),
        };
        let input = typed(&name, span, separator, dash);
        prop_assert_eq!(reference::parse(&input), Reference::new(book, span), "{}", input);
    }

    #[test]
    fn completion_is_bounded_and_ordered(prefix in any::<String>(), limit in 0usize..80) {
        let found = reference::complete(&prefix, limit);
        prop_assert!(found.len() <= limit.min(66));
        let mut unique = found.clone();
        unique.sort();
        unique.dedup();
        prop_assert_eq!(unique.len(), found.len(), "no book twice");
    }
}

#[test]
fn long_inputs_finish_within_their_time_bound() {
    let mib = 1024 * 1024;
    for input in [
        "J".repeat(mib),
        format!("John {}", "1".repeat(mib)),
        format!("John 1:1{}", "-".repeat(mib)),
        format!("{}John 1", "1 ".repeat(mib / 2)),
        format!("John {}", "1:".repeat(mib / 2)),
        "\u{301}".repeat(mib / 2),
    ] {
        within(Duration::from_secs(5), "long reference", || check(&input));
    }
}

#[test]
fn retained_corpus_replays_cleanly() {
    for (path, bytes) in corpus("scripture") {
        for line in String::from_utf8_lossy(&bytes).lines() {
            within(Duration::from_secs(2), &path.display().to_string(), || {
                check(line)
            });
        }
    }
}
