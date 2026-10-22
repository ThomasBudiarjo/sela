//! Translation-independent identity of verses and translations (M4-03).
//!
//! A verse is identified by its book in the 66-book canon, its chapter and
//! its verse number *as the translation numbers them*. Translations do not
//! share one versification: a chapter may have more or fewer verses, or be
//! missing (see `Inventory`), so a verse identity is only meaningful together
//! with the translation that holds it.
use super::books::Book;
use crate::i18n::Language;
use std::collections::BTreeMap;

/// Highest chapter or verse number a translation may use. Keys pack three
/// decimal fields, so both stay below 1000.
pub const MAX_NUMBER: u16 = 999;
/// Longest translation id (`KJV`, `WEB`, `TB-1974`), in ASCII characters.
pub const MAX_ID: usize = 16;
/// Longest stored translation name, rights statement or source, in bytes.
pub const MAX_META: usize = 4096;

/// One verse position: book, chapter and verse, both numbers 1-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VerseId {
    pub book: Book,
    pub chapter: u16,
    pub verse: u16,
}

impl VerseId {
    /// `None` unless both numbers are in `1..=MAX_NUMBER`.
    pub fn new(book: Book, chapter: u16, verse: u16) -> Option<VerseId> {
        let ok = |n: u16| (1..=MAX_NUMBER).contains(&n);
        (ok(chapter) && ok(verse)).then_some(VerseId {
            book,
            chapter,
            verse,
        })
    }

    /// Storage key in canonical order: `BBCCCVVV` as a decimal number.
    pub fn key(self) -> i64 {
        i64::from(self.book.number()) * 1_000_000
            + i64::from(self.chapter) * 1_000
            + i64::from(self.verse)
    }

    pub fn from_key(key: i64) -> Option<VerseId> {
        let book = Book::from_number(u8::try_from(key / 1_000_000).ok()?)?;
        let chapter = u16::try_from(key / 1_000 % 1_000).ok()?;
        let verse = u16::try_from(key % 1_000).ok()?;
        VerseId::new(book, chapter, verse)
    }
}

/// Everything shown or checked about an installed translation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationInfo {
    /// Short label, also the file name: ASCII letters, digits and `-`.
    pub id: String,
    pub name: String,
    /// BCP 47 primary language subtag such as `en` or `id`; empty if unknown.
    pub language: String,
    /// The rights statement as the source gives it.
    pub rights: String,
    /// Where the text came from (URL, file name).
    pub source: String,
    /// Whether Sela may copy the text out (schedule snapshots, exports).
    /// Unknown imported rights are treated as restricted.
    pub redistributable: bool,
    /// Shipped with Sela and rebuilt from the bundle when damaged.
    pub bundled: bool,
}

impl TranslationInfo {
    /// The UI language whose book names suit this translation's text:
    /// Indonesian for `id`, else English.
    pub fn book_language(&self) -> Language {
        if self.language.eq_ignore_ascii_case("id") || self.language.eq_ignore_ascii_case("ind") {
            Language::Indonesian
        } else {
            Language::English
        }
    }
}

/// Most verses one schedule item may pin.
pub const MAX_SNAPSHOT_VERSES: usize = 2000;
/// Longest verse text in a snapshot (the importers' cap).
pub const MAX_SNAPSHOT_VERSE_BYTES: usize = 4096;

/// A schedule item's own copy of selected verses, pinned like a song
/// revision: it plays the same text later even if the translation file is
/// replaced, damaged or removed. `redistributable` travels with it so a
/// later export can leave restricted text out (docs/scripture.md).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub translation: String,
    pub name: String,
    pub language: String,
    pub redistributable: bool,
    /// Canonical order, each verse once.
    pub verses: Vec<(VerseId, String)>,
}

impl Snapshot {
    /// The selected verses of a translation, sorted and deduplicated.
    pub fn new(info: &TranslationInfo, verses: &[(VerseId, String)]) -> Snapshot {
        let mut verses = verses.to_vec();
        verses.sort_by_key(|(id, _)| *id);
        verses.dedup_by_key(|(id, _)| *id);
        Snapshot {
            translation: info.id.clone(),
            name: info.name.clone(),
            language: info.language.clone(),
            redistributable: info.redistributable,
            verses,
        }
    }

    /// Bounds a snapshot read from a file must meet.
    pub fn valid(&self) -> bool {
        valid_id(&self.translation)
            && self.name.len() <= MAX_META
            && self.language.len() <= MAX_META
            && !self.verses.is_empty()
            && self.verses.len() <= MAX_SNAPSHOT_VERSES
            && self.verses.windows(2).all(|pair| pair[0].0 < pair[1].0)
            && self
                .verses
                .iter()
                .all(|(_, text)| !text.is_empty() && text.len() <= MAX_SNAPSHOT_VERSE_BYTES)
    }

    /// The translation as far as the snapshot knows it.
    pub fn info(&self) -> TranslationInfo {
        TranslationInfo {
            id: self.translation.clone(),
            name: self.name.clone(),
            language: self.language.clone(),
            rights: String::new(),
            source: String::new(),
            redistributable: self.redistributable,
            bundled: false,
        }
    }

    /// Content identity: the same verses of the same translation are the
    /// same item (a stable Preview/Live identity and an unchanged schedule
    /// pin). Never collides with a song's random id in practice.
    pub fn version(&self) -> crate::storage::Version {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"sela-scripture\0");
        hash.update(self.translation.as_bytes());
        for (id, text) in &self.verses {
            hash.update(id.key().to_le_bytes());
            hash.update((text.len() as u64).to_le_bytes());
            hash.update(text.as_bytes());
        }
        let digest = hash.finalize();
        let mut id = [0u8; 16];
        id.copy_from_slice(&digest[..16]);
        crate::storage::Version {
            id: crate::storage::Id(id),
            revision: 1,
        }
    }

    /// The verses' own inventory: enough to label them when the
    /// translation is not installed.
    pub fn inventory(&self) -> Inventory {
        let mut inventory = Inventory::default();
        for (id, _) in &self.verses {
            inventory.add(*id);
        }
        inventory
    }
}

/// `true` for an id Sela can use as a file name on every platform.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && !id.starts_with(['-', '_'])
}

/// A translation id derived from free text (a work id, a file name): ASCII
/// letters and digits upper-cased, other characters dropped, cut to
/// `MAX_ID`; `None` when nothing usable is left.
pub fn derive_id(text: &str) -> Option<String> {
    let id: String = text
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .take(MAX_ID)
        .collect();
    (!id.is_empty()).then_some(id)
}

/// Which verses a translation has: per book, the verse count of each
/// chapter (index 0 is chapter 1; 0 means the chapter is missing). Built
/// from the verses themselves, so it is the translation's own versification.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    books: BTreeMap<Book, Vec<u16>>,
}

impl Inventory {
    pub fn add(&mut self, id: VerseId) {
        let chapters = self.books.entry(id.book).or_default();
        let index = usize::from(id.chapter) - 1;
        if chapters.len() <= index {
            chapters.resize(index + 1, 0);
        }
        chapters[index] = chapters[index].max(id.verse);
    }

    pub fn has_book(&self, book: Book) -> bool {
        self.books.contains_key(&book)
    }

    pub fn books(&self) -> impl Iterator<Item = Book> + '_ {
        self.books.keys().copied()
    }

    /// Highest verse number in the chapter; `None` when the chapter is missing.
    pub fn last_verse(&self, book: Book, chapter: u16) -> Option<u16> {
        let chapters = self.books.get(&book)?;
        let count = *chapters.get(usize::from(chapter).checked_sub(1)?)?;
        (count > 0).then_some(count)
    }

    /// Highest chapter number the book has here.
    pub fn last_chapter(&self, book: Book) -> Option<u16> {
        u16::try_from(self.books.get(&book)?.len()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(n: u8) -> Book {
        Book::from_number(n).unwrap()
    }

    #[test]
    fn keys_sort_canonically_and_round_trip() {
        let a = VerseId::new(book(43), 3, 16).unwrap();
        let b = VerseId::new(book(43), 4, 1).unwrap();
        let c = VerseId::new(book(44), 1, 1).unwrap();
        assert_eq!(a.key(), 43_003_016);
        assert!(a.key() < b.key() && b.key() < c.key());
        assert!(a < b && b < c);
        for id in [a, b, c] {
            assert_eq!(VerseId::from_key(id.key()), Some(id));
        }
        assert_eq!(VerseId::from_key(67_001_001), None);
        assert_eq!(VerseId::from_key(43_000_001), None);
        assert_eq!(VerseId::new(book(1), 0, 1), None);
        assert_eq!(VerseId::new(book(1), 1, 1000), None);
    }

    #[test]
    fn ids_are_file_name_safe() {
        assert!(valid_id("KJV"));
        assert!(valid_id("TB-1974"));
        assert!(!valid_id(""));
        assert!(!valid_id("../x"));
        assert!(!valid_id("a b"));
        assert!(!valid_id("-x"));
        assert!(!valid_id("ABCDEFGHIJKLMNOPQ"));
        assert_eq!(derive_id("Bible.en.kjv").as_deref(), Some("BIBLEENKJV"));
        assert_eq!(derive_id("éé"), None);
    }

    #[test]
    fn the_inventory_is_the_translations_own_versification() {
        let mut inventory = Inventory::default();
        for (chapter, verse) in [(1, 1), (1, 2), (3, 1), (3, 15)] {
            inventory.add(VerseId::new(book(64), chapter, verse).unwrap());
        }
        assert_eq!(inventory.last_verse(book(64), 1), Some(2));
        assert_eq!(inventory.last_verse(book(64), 2), None, "missing chapter");
        assert_eq!(inventory.last_verse(book(64), 3), Some(15));
        assert_eq!(inventory.last_verse(book(64), 0), None);
        assert_eq!(inventory.last_chapter(book(64)), Some(3));
        assert!(!inventory.has_book(book(1)));
        assert_eq!(inventory.books().collect::<Vec<_>>(), [book(64)]);
    }

    #[test]
    fn snapshots_are_sorted_bounded_and_identified_by_content() {
        let info = TranslationInfo {
            id: "SYN".into(),
            name: "Synthetic".into(),
            language: "en".into(),
            rights: "r".into(),
            source: "s".into(),
            redistributable: true,
            bundled: true,
        };
        let a = VerseId::new(book(43), 3, 16).unwrap();
        let b = VerseId::new(book(43), 3, 18).unwrap();
        let snapshot = Snapshot::new(
            &info,
            &[(b, "Two".into()), (a, "One".into()), (b, "Two".into())],
        );
        assert_eq!(snapshot.verses, [(a, "One".into()), (b, "Two".into())]);
        assert!(snapshot.valid());
        assert_eq!(snapshot.version(), snapshot.clone().version());
        let mut other = snapshot.clone();
        other.verses[0].1 = "Changed".into();
        assert_ne!(other.version(), snapshot.version());
        let mut other = snapshot.clone();
        other.translation = "OTHER".into();
        assert_ne!(other.version(), snapshot.version());
        assert_eq!(snapshot.inventory().last_verse(book(43), 3), Some(18));
        assert_eq!(snapshot.info().id, "SYN");
        let mut bad = snapshot.clone();
        bad.verses.reverse();
        assert!(!bad.valid());
        bad.verses.clear();
        assert!(!bad.valid());
        let mut bad = snapshot.clone();
        bad.translation = "../x".into();
        assert!(!bad.valid());
        let mut bad = snapshot;
        bad.verses[0].1 = "x".repeat(MAX_SNAPSHOT_VERSE_BYTES + 1);
        assert!(!bad.valid());
    }

    #[test]
    fn indonesian_translations_use_indonesian_book_names() {
        let mut info = TranslationInfo {
            id: "X".into(),
            name: "X".into(),
            language: "id".into(),
            rights: String::new(),
            source: String::new(),
            redistributable: false,
            bundled: false,
        };
        assert_eq!(info.book_language(), Language::Indonesian);
        info.language = "en".into();
        assert_eq!(info.book_language(), Language::English);
    }
}
