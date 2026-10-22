//! Selected verses as audience slides and reference labels (M4-04).
//!
//! Verses always go out in canonical order whatever order they were picked
//! in. Text is word-wrapped into lines (the renderer fits lines, it does not
//! wrap them) and a passage is split between verses into slides of at most
//! `LAYOUT.lines` lines; a single verse longer than that is split between
//! lines, and its continuation slides repeat no verse number. EasyWorship
//! 8.0.49's own splitting ("Automatically Flow Scripture on Multiple
//! Slides", minimum font size, EW8-OBS-056) is unobserved, so the layout is
//! a best guess (docs/reference-observations.md, Scriptures best guesses).
use super::books::Book;
use super::model::{Inventory, VerseId};
use super::reference::{Reference, Span};
use crate::i18n::Language;

/// Where a reference lands in one translation: the whole chapters it
/// touches (what the verse list shows) and the referenced verses inside
/// them (what gets selected).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Located {
    pub chapters: (VerseId, VerseId),
    pub selected: (VerseId, VerseId),
}

/// Why a parsed reference does not exist in a translation (VAL-BIBLE-009).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    /// The translation lacks the whole book (a New Testament only text).
    Book(Book),
    /// The translation lacks the chapter (another versification).
    Chapter(Book, u16),
    /// The chapter has only `count` verses here.
    Verse {
        book: Book,
        chapter: u16,
        count: u16,
    },
}

/// The whole chapters a reference touches in a translation, checking only
/// that the book and those chapters exist there (a translation switch keeps
/// the selected verses and reports the ones the new translation lacks).
pub fn chapters(
    reference: &Reference,
    inventory: &Inventory,
) -> Result<(VerseId, VerseId), Missing> {
    let book = reference.book();
    if !inventory.has_book(book) {
        return Err(Missing::Book(book));
    }
    let (first, last) = match reference.span() {
        Span::Chapters { first, last } => (first, last),
        Span::Verses { start, end } => (start.chapter, end.chapter),
    };
    for chapter in [first, last] {
        inventory
            .last_verse(book, chapter)
            .ok_or(Missing::Chapter(book, chapter))?;
    }
    let final_count = inventory.last_verse(book, last).unwrap_or(1);
    VerseId::new(book, first, 1)
        .zip(VerseId::new(book, last, final_count))
        .ok_or(Missing::Chapter(book, first))
}

/// Checks a reference against a translation's own verse inventory.
pub fn locate(reference: &Reference, inventory: &Inventory) -> Result<Located, Missing> {
    let book = reference.book();
    if !inventory.has_book(book) {
        return Err(Missing::Book(book));
    }
    let last = |chapter: u16| {
        inventory
            .last_verse(book, chapter)
            .ok_or(Missing::Chapter(book, chapter))
    };
    let at =
        |chapter, verse| VerseId::new(book, chapter, verse).ok_or(Missing::Chapter(book, chapter));
    let (first, final_chapter, start, end) = match reference.span() {
        Span::Chapters { first, last: end } => (first, end, (first, 1), (end, last(end)?)),
        Span::Verses { start, end } => (
            start.chapter,
            end.chapter,
            (start.chapter, start.verse),
            (end.chapter, end.verse),
        ),
    };
    let first_count = last(first)?;
    let final_count = last(final_chapter)?;
    for (chapter, verse, count) in [(start.0, start.1, first_count), (end.0, end.1, final_count)] {
        if verse > count {
            return Err(Missing::Verse {
                book,
                chapter,
                count,
            });
        }
    }
    Ok(Located {
        chapters: (at(first, 1)?, at(final_chapter, final_count)?),
        selected: (at(start.0, start.1)?, at(end.0, end.1)?),
    })
}

/// One audience slide of a passage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassageSlide {
    /// Word-wrapped lines joined by `\n`.
    pub text: String,
    /// The verses whose text is on this slide, in order.
    pub verses: Vec<VerseId>,
}

/// Slide shape: line width in characters and lines per slide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub line_chars: usize,
    pub lines: usize,
}

/// About 44 characters a line and six lines a slide: large, readable text
/// on a 16:9 output once the renderer fits it.
pub const LAYOUT: Layout = Layout {
    line_chars: 44,
    lines: 6,
};

/// Splits verses into slides. `numbers` prefixes each verse with its number
/// (EW "Show verse numbers", on by default, EW8-OBS-056); a verse in another
/// chapter than the one before it gets `chapter:verse`. Verses flow on from
/// line to line; a slide break falls between verses unless one verse alone
/// needs more than a slide.
pub fn paginate(verses: &[(VerseId, String)], layout: Layout, numbers: bool) -> Vec<PassageSlide> {
    let width = layout.line_chars.max(8);
    let max_lines = layout.lines.max(1);
    let mut sorted: Vec<&(VerseId, String)> = verses.iter().collect();
    sorted.sort_by_key(|(id, _)| *id);
    sorted.dedup_by_key(|(id, _)| *id);
    let mut pieces: Vec<(VerseId, String)> = Vec::new();
    let mut previous: Option<VerseId> = None;
    for (id, text) in sorted {
        let prefix = match previous {
            _ if !numbers => String::new(),
            Some(p) if p.book != id.book || p.chapter != id.chapter => {
                format!("{}:{} ", id.chapter, id.verse)
            }
            _ => format!("{} ", id.verse),
        };
        previous = Some(*id);
        pieces.push((*id, format!("{prefix}{text}")));
    }
    let mut slides: Vec<PassageSlide> = Vec::new();
    let mut group: Vec<&(VerseId, String)> = Vec::new();
    let joined = |group: &[&(VerseId, String)]| {
        group
            .iter()
            .map(|(_, piece)| piece.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let flush = |group: &mut Vec<&(VerseId, String)>, slides: &mut Vec<PassageSlide>| {
        if group.is_empty() {
            return;
        }
        slides.push(PassageSlide {
            text: wrap(&joined(group), width).join("\n"),
            verses: group.iter().map(|(id, _)| *id).collect(),
        });
        group.clear();
    };
    for piece in &pieces {
        let mut candidate = group.clone();
        candidate.push(piece);
        if wrap(&joined(&candidate), width).len() <= max_lines {
            group = candidate;
            continue;
        }
        flush(&mut group, &mut slides);
        let lines = wrap(&piece.1, width);
        if lines.len() <= max_lines {
            group.push(piece);
            continue;
        }
        // One verse longer than a slide continues on the next ones.
        for chunk in lines.chunks(max_lines) {
            slides.push(PassageSlide {
                text: chunk.join("\n"),
                verses: vec![piece.0],
            });
        }
    }
    flush(&mut group, &mut slides);
    slides
}

/// Greedy word wrap to lines of at most `width` characters; a word longer
/// than a line is broken inside it.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut length = 0usize;
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        loop {
            let separator = usize::from(length > 0);
            if length + separator + word.len() <= width {
                if separator == 1 {
                    line.push(' ');
                }
                line.extend(word.iter());
                length += separator + word.len();
                break;
            }
            if length > 0 {
                lines.push(std::mem::take(&mut line));
                length = 0;
                continue;
            }
            let rest = word.split_off(width);
            lines.push(word.iter().collect());
            word = rest;
            if word.is_empty() {
                break;
            }
        }
    }
    if length > 0 {
        lines.push(line);
    }
    lines
}

/// The reference for a set of verses, in canonical order: consecutive
/// verses join into ranges (across a chapter end when `inventory` shows the
/// chapter ended), other verses of the same chapter follow after a comma,
/// another chapter or book after a semicolon. `John 3:16-18`,
/// `John 3:16, 18`, `John 3:36-4:2`, `John 3:16; Romans 5:8`.
pub fn label(ids: &[VerseId], inventory: &Inventory, language: Language) -> String {
    let mut ids = ids.to_vec();
    ids.sort();
    ids.dedup();
    let follows = |a: VerseId, b: VerseId| {
        a.book == b.book
            && ((a.chapter == b.chapter && a.verse + 1 == b.verse)
                || (a.chapter + 1 == b.chapter
                    && b.verse == 1
                    && inventory.last_verse(a.book, a.chapter) == Some(a.verse)))
    };
    let mut runs: Vec<(VerseId, VerseId)> = Vec::new();
    for id in ids {
        match runs.last_mut() {
            Some((_, end)) if follows(*end, id) => *end = id,
            _ => runs.push((id, id)),
        }
    }
    let mut out = String::new();
    let mut last: Option<VerseId> = None;
    for (start, end) in runs {
        let range = if end == start {
            None
        } else if end.chapter == start.chapter {
            Some(end.verse.to_string())
        } else {
            Some(format!("{}:{}", end.chapter, end.verse))
        };
        match last {
            Some(previous) if previous.book == start.book && previous.chapter == start.chapter => {
                out.push_str(&format!(", {}", start.verse));
            }
            Some(previous) if previous.book == start.book => {
                out.push_str(&format!("; {}:{}", start.chapter, start.verse));
            }
            Some(_) => out.push_str(&format!(
                "; {} {}:{}",
                start.book.name(language),
                start.chapter,
                start.verse
            )),
            None => out.push_str(&format!(
                "{} {}:{}",
                start.book.name(language),
                start.chapter,
                start.verse
            )),
        }
        if let Some(range) = range {
            out.push('-');
            out.push_str(&range);
        }
        last = Some(end);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripture::Book;

    fn v(book: u8, chapter: u16, verse: u16) -> VerseId {
        VerseId::new(Book::from_number(book).unwrap(), chapter, verse).unwrap()
    }

    fn inventory() -> Inventory {
        let mut inventory = Inventory::default();
        for verse in 1..=36 {
            inventory.add(v(43, 3, verse));
        }
        for verse in 1..=54 {
            inventory.add(v(43, 4, verse));
        }
        inventory
    }

    #[test]
    fn labels_join_runs_and_cross_chapter_ranges() {
        let inv = inventory();
        let en = Language::English;
        assert_eq!(label(&[v(43, 3, 16)], &inv, en), "John 3:16");
        assert_eq!(
            label(&[v(43, 3, 18), v(43, 3, 16), v(43, 3, 17)], &inv, en),
            "John 3:16-18"
        );
        assert_eq!(
            label(&[v(43, 3, 16), v(43, 3, 18)], &inv, en),
            "John 3:16, 18"
        );
        assert_eq!(
            label(
                &[v(43, 3, 35), v(43, 3, 36), v(43, 4, 1), v(43, 4, 2)],
                &inv,
                en
            ),
            "John 3:35-4:2"
        );
        // Without knowing chapter 3 ends at 36, 3:35 and 4:1 do not join.
        assert_eq!(
            label(&[v(43, 3, 35), v(43, 4, 1)], &inv, en),
            "John 3:35; 4:1"
        );
        assert_eq!(
            label(&[v(43, 3, 16), v(45, 5, 8)], &inv, en),
            "John 3:16; Romans 5:8"
        );
        assert_eq!(
            label(&[v(43, 3, 16), v(43, 3, 17)], &inv, Language::Indonesian),
            "Yohanes 3:16-17"
        );
        assert_eq!(label(&[], &inv, en), "");
    }

    #[test]
    fn references_are_located_in_the_translations_own_versification() {
        let inv = inventory();
        let reference = |text| crate::scripture::parse(text).unwrap();
        assert_eq!(
            locate(&reference("John 3:16-18"), &inv),
            Ok(Located {
                chapters: (v(43, 3, 1), v(43, 3, 36)),
                selected: (v(43, 3, 16), v(43, 3, 18)),
            })
        );
        assert_eq!(
            locate(&reference("John 3:36-4:2"), &inv),
            Ok(Located {
                chapters: (v(43, 3, 1), v(43, 4, 54)),
                selected: (v(43, 3, 36), v(43, 4, 2)),
            })
        );
        assert_eq!(
            locate(&reference("John 4"), &inv),
            Ok(Located {
                chapters: (v(43, 4, 1), v(43, 4, 54)),
                selected: (v(43, 4, 1), v(43, 4, 54)),
            })
        );
        let john = Book::from_number(43).unwrap();
        assert_eq!(
            locate(&reference("John 3:37"), &inv),
            Err(Missing::Verse {
                book: john,
                chapter: 3,
                count: 36
            })
        );
        assert_eq!(
            locate(&reference("John 3:1-4:60"), &inv),
            Err(Missing::Verse {
                book: john,
                chapter: 4,
                count: 54
            })
        );
        assert_eq!(
            locate(&reference("John 5:1"), &inv),
            Err(Missing::Chapter(john, 5))
        );
        assert_eq!(
            locate(&reference("John 2-3"), &inv),
            Err(Missing::Chapter(john, 2))
        );
        assert_eq!(
            locate(&reference("Gen 1:1"), &inv),
            Err(Missing::Book(Book::from_number(1).unwrap()))
        );
        // A switch only needs the chapters: John 3:30-4:60 still lists both.
        assert_eq!(
            chapters(&reference("John 3:30-4:60"), &inv),
            Ok((v(43, 3, 1), v(43, 4, 54)))
        );
        assert_eq!(
            chapters(&reference("John 5"), &inv),
            Err(Missing::Chapter(john, 5))
        );
        assert_eq!(
            chapters(&reference("Gen 1"), &inv),
            Err(Missing::Book(Book::from_number(1).unwrap()))
        );
    }

    fn verses(list: &[(VerseId, &str)]) -> Vec<(VerseId, String)> {
        list.iter().map(|(id, t)| (*id, t.to_string())).collect()
    }

    fn flat(slide: &PassageSlide) -> String {
        slide.text.replace('\n', " ")
    }

    #[test]
    fn short_passages_fit_one_slide_with_verse_numbers_in_canonical_order() {
        let slides = paginate(
            &verses(&[
                (v(43, 3, 18), "Third."),
                (v(43, 3, 16), "First."),
                (v(43, 4, 1), "Next chapter."),
            ]),
            LAYOUT,
            true,
        );
        assert_eq!(slides.len(), 1);
        assert_eq!(flat(&slides[0]), "16 First. 18 Third. 4:1 Next chapter.");
        assert_eq!(slides[0].verses, [v(43, 3, 16), v(43, 3, 18), v(43, 4, 1)]);
        let plain = paginate(&verses(&[(v(43, 3, 16), "First.")]), LAYOUT, false);
        assert_eq!(plain[0].text, "First.");
    }

    #[test]
    fn words_wrap_into_lines_that_fit() {
        assert_eq!(wrap("aa bb cc", 5), ["aa bb", "cc"]);
        assert_eq!(wrap("abcdefgh ij", 3), ["abc", "def", "gh", "ij"]);
        assert_eq!(wrap("  ", 5), Vec::<String>::new());
        assert_eq!(wrap("ünï cödé", 3), ["ünï", "cöd", "é"]);
    }

    #[test]
    fn long_passages_split_between_verses_and_long_verses_between_lines() {
        let layout = Layout {
            line_chars: 40,
            lines: 4,
        };
        // Each verse wraps to two lines, so two verses fill a slide.
        let sentence = "word ".repeat(12); // 59 characters
        let list: Vec<(VerseId, String)> = (1..=6)
            .map(|n| (v(19, 119, n), sentence.trim().to_owned()))
            .collect();
        let slides = paginate(&list, layout, true);
        assert_eq!(slides.len(), 3, "two verses per slide");
        for slide in &slides {
            let lines: Vec<&str> = slide.text.lines().collect();
            assert!(lines.len() <= 4);
            assert!(lines.iter().all(|l| l.chars().count() <= 40));
        }
        assert_eq!(slides[1].verses, [v(19, 119, 3), v(19, 119, 4)]);
        assert!(slides[1].text.starts_with("3 word"));

        let long = "x".repeat(10) + &" longword".repeat(80);
        let slides = paginate(&verses(&[(v(17, 8, 9), &long)]), layout, true);
        assert!(slides.len() >= 4);
        assert!(slides.iter().all(|s| s.verses == [v(17, 8, 9)]));
        assert!(slides[0].text.starts_with("9 xxx"));
        assert!(!slides[1].text.starts_with('9'));
        let joined: Vec<String> = slides.iter().map(flat).collect();
        assert_eq!(joined.join(" "), format!("9 {long}"), "nothing lost");
        assert!(paginate(&[], layout, true).is_empty());
    }
}
