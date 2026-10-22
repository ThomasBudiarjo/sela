//! The 66-book Protestant canon with English and Indonesian (LAI Terjemahan
//! Baru) names, aliases and KJV chapter counts. Name resolution rules are in
//! docs/scripture.md.
use crate::i18n::Language;

/// One canonical book, numbered 1 (Genesis) to 66 (Revelation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Book(u8);

struct Entry {
    osis: &'static str,
    english: &'static str,
    indonesian: &'static str,
    chapters: u16,
    /// The first entry is the abbreviation shown when book names are abbreviated.
    english_aliases: &'static [&'static str],
    indonesian_aliases: &'static [&'static str],
}

const fn book(
    osis: &'static str,
    english: &'static str,
    indonesian: &'static str,
    chapters: u16,
    english_aliases: &'static [&'static str],
    indonesian_aliases: &'static [&'static str],
) -> Entry {
    Entry {
        osis,
        english,
        indonesian,
        chapters,
        english_aliases,
        indonesian_aliases,
    }
}

#[rustfmt::skip]
const BOOKS: [Entry; 66] = [
    book("Gen", "Genesis", "Kejadian", 50, &["Gen", "Ge", "Gn"], &["Kej"]),
    book("Exod", "Exodus", "Keluaran", 40, &["Exod", "Ex", "Exo"], &["Kel"]),
    book("Lev", "Leviticus", "Imamat", 27, &["Lev", "Le", "Lv"], &["Im"]),
    book("Num", "Numbers", "Bilangan", 36, &["Num", "Nu", "Nm", "Nb"], &["Bil"]),
    book("Deut", "Deuteronomy", "Ulangan", 34, &["Deut", "De", "Dt"], &["Ul"]),
    book("Josh", "Joshua", "Yosua", 24, &["Josh", "Jos", "Jsh"], &["Yos"]),
    book("Judg", "Judges", "Hakim-hakim", 21, &["Judg", "Jdg", "Jg", "Jdgs"], &["Hak", "Hakim"]),
    book("Ruth", "Ruth", "Rut", 4, &["Ruth", "Ru", "Rth"], &["Rut"]),
    book("1Sam", "1 Samuel", "1 Samuel", 31, &["1 Sam", "1 Sa", "1 Sm"], &["1 Sam"]),
    book("2Sam", "2 Samuel", "2 Samuel", 24, &["2 Sam", "2 Sa", "2 Sm"], &["2 Sam"]),
    book("1Kgs", "1 Kings", "1 Raja-raja", 22, &["1 Kgs", "1 Ki", "1 Kin"], &["1 Raj", "1 Raja"]),
    book("2Kgs", "2 Kings", "2 Raja-raja", 25, &["2 Kgs", "2 Ki", "2 Kin"], &["2 Raj", "2 Raja"]),
    book("1Chr", "1 Chronicles", "1 Tawarikh", 29, &["1 Chr", "1 Ch", "1 Chron"], &["1 Taw"]),
    book("2Chr", "2 Chronicles", "2 Tawarikh", 36, &["2 Chr", "2 Ch", "2 Chron"], &["2 Taw"]),
    book("Ezra", "Ezra", "Ezra", 10, &["Ezra", "Ezr"], &["Ezr"]),
    book("Neh", "Nehemiah", "Nehemia", 13, &["Neh", "Ne"], &["Neh"]),
    book("Esth", "Esther", "Ester", 10, &["Esth", "Es"], &["Est"]),
    book("Job", "Job", "Ayub", 42, &["Job", "Jb"], &["Ayb"]),
    book("Ps", "Psalms", "Mazmur", 150, &["Ps", "Psalm", "Psa", "Pss", "Psm"], &["Mzm", "Maz"]),
    book("Prov", "Proverbs", "Amsal", 31, &["Prov", "Pr", "Prv", "Pro"], &["Ams"]),
    book("Eccl", "Ecclesiastes", "Pengkhotbah", 12, &["Eccl", "Ecc", "Ec", "Qoh"], &["Pkh", "Peng"]),
    book("Song", "Song of Solomon", "Kidung Agung", 8, &["Song", "Song of Songs", "SS", "Sg", "Cant", "Canticles"], &["Kid", "KA"]),
    book("Isa", "Isaiah", "Yesaya", 66, &["Isa", "Is"], &["Yes"]),
    book("Jer", "Jeremiah", "Yeremia", 52, &["Jer", "Je", "Jr"], &["Yer"]),
    book("Lam", "Lamentations", "Ratapan", 5, &["Lam", "La"], &["Rat"]),
    book("Ezek", "Ezekiel", "Yehezkiel", 48, &["Ezek", "Eze", "Ezk"], &["Yeh"]),
    book("Dan", "Daniel", "Daniel", 12, &["Dan", "Da", "Dn"], &["Dan"]),
    book("Hos", "Hosea", "Hosea", 14, &["Hos", "Ho"], &["Hos"]),
    book("Joel", "Joel", "Yoel", 3, &["Joel", "Jl"], &["Yl"]),
    book("Amos", "Amos", "Amos", 9, &["Amos", "Am"], &["Am"]),
    book("Obad", "Obadiah", "Obaja", 1, &["Obad", "Ob"], &["Ob"]),
    book("Jonah", "Jonah", "Yunus", 4, &["Jonah", "Jon", "Jnh"], &["Yun"]),
    book("Mic", "Micah", "Mikha", 7, &["Mic", "Mi"], &["Mi"]),
    book("Nah", "Nahum", "Nahum", 3, &["Nah", "Na"], &["Nah"]),
    book("Hab", "Habakkuk", "Habakuk", 3, &["Hab", "Hb"], &["Hab"]),
    book("Zeph", "Zephaniah", "Zefanya", 3, &["Zeph", "Zep", "Zp"], &["Zef"]),
    book("Hag", "Haggai", "Hagai", 2, &["Hag", "Hg"], &["Hag"]),
    book("Zech", "Zechariah", "Zakharia", 14, &["Zech", "Zec", "Zc"], &["Za", "Zak"]),
    book("Mal", "Malachi", "Maleakhi", 4, &["Mal", "Ml"], &["Mal"]),
    book("Matt", "Matthew", "Matius", 28, &["Matt", "Mt", "Mat"], &["Mat", "Mt"]),
    book("Mark", "Mark", "Markus", 16, &["Mark", "Mk", "Mrk", "Mar"], &["Mrk", "Mr"]),
    book("Luke", "Luke", "Lukas", 24, &["Luke", "Lk", "Luk"], &["Luk"]),
    book("John", "John", "Yohanes", 21, &["John", "Jn", "Jhn", "Joh"], &["Yoh"]),
    book("Acts", "Acts", "Kisah Para Rasul", 28, &["Acts", "Ac", "Act"], &["Kis", "Kisah"]),
    book("Rom", "Romans", "Roma", 16, &["Rom", "Ro", "Rm"], &["Rm", "Rom"]),
    book("1Cor", "1 Corinthians", "1 Korintus", 16, &["1 Cor", "1 Co"], &["1 Kor"]),
    book("2Cor", "2 Corinthians", "2 Korintus", 13, &["2 Cor", "2 Co"], &["2 Kor"]),
    book("Gal", "Galatians", "Galatia", 6, &["Gal", "Ga"], &["Gal"]),
    book("Eph", "Ephesians", "Efesus", 6, &["Eph", "Ephes"], &["Ef"]),
    book("Phil", "Philippians", "Filipi", 4, &["Phil", "Php", "Pp"], &["Flp"]),
    book("Col", "Colossians", "Kolose", 4, &["Col"], &["Kol"]),
    book("1Thess", "1 Thessalonians", "1 Tesalonika", 5, &["1 Thess", "1 Thes", "1 Th"], &["1 Tes"]),
    book("2Thess", "2 Thessalonians", "2 Tesalonika", 3, &["2 Thess", "2 Thes", "2 Th"], &["2 Tes"]),
    book("1Tim", "1 Timothy", "1 Timotius", 6, &["1 Tim", "1 Ti"], &["1 Tim"]),
    book("2Tim", "2 Timothy", "2 Timotius", 4, &["2 Tim", "2 Ti"], &["2 Tim"]),
    book("Titus", "Titus", "Titus", 3, &["Titus", "Tit"], &["Tit"]),
    book("Phlm", "Philemon", "Filemon", 1, &["Phlm", "Philem", "Phm"], &["Flm"]),
    book("Heb", "Hebrews", "Ibrani", 13, &["Heb"], &["Ibr"]),
    book("Jas", "James", "Yakobus", 5, &["Jas", "Jm"], &["Yak"]),
    book("1Pet", "1 Peter", "1 Petrus", 5, &["1 Pet", "1 Pe", "1 Pt"], &["1 Ptr", "1 Pet"]),
    book("2Pet", "2 Peter", "2 Petrus", 3, &["2 Pet", "2 Pe", "2 Pt"], &["2 Ptr", "2 Pet"]),
    book("1John", "1 John", "1 Yohanes", 5, &["1 John", "1 Jn", "1 Jhn", "1 Joh"], &["1 Yoh"]),
    book("2John", "2 John", "2 Yohanes", 1, &["2 John", "2 Jn", "2 Jhn", "2 Joh"], &["2 Yoh"]),
    book("3John", "3 John", "3 Yohanes", 1, &["3 John", "3 Jn", "3 Jhn", "3 Joh"], &["3 Yoh"]),
    book("Jude", "Jude", "Yudas", 1, &["Jude", "Jde"], &["Yud"]),
    book("Rev", "Revelation", "Wahyu", 22, &["Rev", "Re", "Rv", "Revelations"], &["Why", "Wah"]),
];

impl Book {
    /// Genesis to Revelation in canonical order.
    pub fn all() -> impl Iterator<Item = Book> {
        (1..=BOOKS.len() as u8).map(Book)
    }

    /// The book with canonical number `number` (1 to 66).
    pub fn from_number(number: u8) -> Option<Book> {
        (1..=BOOKS.len() as u8)
            .contains(&number)
            .then_some(Book(number))
    }

    pub fn number(self) -> u8 {
        self.0
    }

    fn entry(self) -> &'static Entry {
        &BOOKS[usize::from(self.0) - 1]
    }

    pub fn osis(self) -> &'static str {
        self.entry().osis
    }

    pub fn name(self, language: Language) -> &'static str {
        match language {
            Language::English => self.entry().english,
            Language::Indonesian => self.entry().indonesian,
        }
    }

    pub fn abbreviation(self, language: Language) -> &'static str {
        match language {
            Language::English => self.entry().english_aliases[0],
            Language::Indonesian => self.entry().indonesian_aliases[0],
        }
    }

    /// Chapter count in KJV versification.
    pub fn chapters(self) -> u16 {
        self.entry().chapters
    }

    fn names(self) -> impl Iterator<Item = &'static str> {
        let entry = self.entry();
        [entry.english, entry.indonesian, entry.osis]
            .into_iter()
            .chain(entry.english_aliases.iter().copied())
            .chain(entry.indonesian_aliases.iter().copied())
    }
}

/// USFM/USFX book codes in canonical order (Genesis to Revelation).
const USFM: [&str; 66] = [
    "GEN", "EXO", "LEV", "NUM", "DEU", "JOS", "JDG", "RUT", "1SA", "2SA", "1KI", "2KI", "1CH",
    "2CH", "EZR", "NEH", "EST", "JOB", "PSA", "PRO", "ECC", "SNG", "ISA", "JER", "LAM", "EZK",
    "DAN", "HOS", "JOL", "AMO", "OBA", "JON", "MIC", "NAM", "HAB", "ZEP", "HAG", "ZEC", "MAL",
    "MAT", "MRK", "LUK", "JHN", "ACT", "ROM", "1CO", "2CO", "GAL", "EPH", "PHP", "COL", "1TH",
    "2TH", "1TI", "2TI", "TIT", "PHM", "HEB", "JAS", "1PE", "2PE", "1JN", "2JN", "3JN", "JUD",
    "REV",
];

impl Book {
    /// The book with this OSIS id (`Gen`, `1Cor`), ignoring ASCII case.
    /// Books outside the 66-book canon (`Tob`, `1Macc`) have none.
    pub fn from_osis(id: &str) -> Option<Book> {
        Book::all().find(|book| book.osis().eq_ignore_ascii_case(id))
    }

    /// The USFM/USFX code (`GEN`, `1CO`).
    pub fn usfm(self) -> &'static str {
        USFM[usize::from(self.0) - 1]
    }

    /// The book with this USFM/USFX code, ignoring ASCII case.
    pub fn from_usfm(code: &str) -> Option<Book> {
        Book::all().find(|book| book.usfm().eq_ignore_ascii_case(code))
    }
}

/// Books matching a typed book name or prefix, best first: the book that has
/// the typed text as an exact name or alias, then every book with a name or
/// alias starting with it, in canonical order. Case, spaces, dots and hyphens
/// are ignored, so `1cor`, `1 Cor.` and `1 COR` are the same text.
pub fn complete(prefix: &str, limit: usize) -> Vec<Book> {
    let typed = key(prefix).collect::<String>();
    let exact = Book::all().find(|book| book.names().any(|name| key(name).eq(typed.chars())));
    let prefixed = Book::all()
        .filter(|book| Some(*book) != exact && book.names().any(|name| starts_with(name, &typed)));
    exact.into_iter().chain(prefixed).take(limit).collect()
}

/// The book a typed name resolves to: the first `complete` result.
pub(super) fn resolve(name: &str) -> Option<Book> {
    complete(name, 1).first().copied()
}

fn key(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
}

fn starts_with(name: &str, typed: &str) -> bool {
    let mut name = key(name);
    typed.chars().all(|c| name.next() == Some(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_and_alias_resolves_to_its_own_book() {
        for book in Book::all() {
            for name in book.names() {
                assert_eq!(resolve(name), Some(book), "{name}");
            }
        }
    }

    #[test]
    fn the_canon_has_1189_chapters_929_of_them_in_the_old_testament() {
        let total: u16 = Book::all().map(Book::chapters).sum();
        let old: u16 = Book::all().take(39).map(Book::chapters).sum();
        assert_eq!((total, old), (1189, 929));
    }

    #[test]
    fn books_are_numbered_1_to_66() {
        assert_eq!(Book::from_number(0), None);
        assert_eq!(Book::from_number(67), None);
        assert_eq!(Book::from_number(1).map(Book::osis), Some("Gen"));
        assert_eq!(Book::from_number(66).map(Book::osis), Some("Rev"));
        assert_eq!(Book::all().count(), 66);
    }

    #[test]
    fn osis_and_usfm_codes_round_trip_for_every_book() {
        for book in Book::all() {
            assert_eq!(Book::from_osis(book.osis()), Some(book));
            assert_eq!(Book::from_usfm(book.usfm()), Some(book));
            assert_eq!(Book::from_usfm(&book.usfm().to_lowercase()), Some(book));
        }
        assert_eq!(Book::from_osis("Tob"), None);
        assert_eq!(Book::from_usfm("FRT"), None);
        assert_eq!(Book::from_usfm("JHN").map(Book::osis), Some("John"));
        assert_eq!(Book::from_usfm("PHM").map(Book::osis), Some("Phlm"));
    }

    #[test]
    fn names_and_abbreviations_follow_the_language() {
        let john = Book::from_number(43).unwrap();
        assert_eq!(john.name(Language::English), "John");
        assert_eq!(john.name(Language::Indonesian), "Yohanes");
        assert_eq!(john.abbreviation(Language::English), "John");
        assert_eq!(john.abbreviation(Language::Indonesian), "Yoh");
        let song = Book::from_number(22).unwrap();
        assert_eq!(song.name(Language::Indonesian), "Kidung Agung");
        assert_eq!(song.abbreviation(Language::Indonesian), "Kid");
    }

    fn numbers(books: Vec<Book>) -> Vec<u8> {
        books.into_iter().map(Book::number).collect()
    }

    #[test]
    fn completion_lists_prefix_matches_in_canonical_order() {
        assert_eq!(numbers(complete("Jo", 10)), [6, 18, 29, 32, 43]);
        assert_eq!(numbers(complete("ju", 10)), [7, 65]);
        assert_eq!(numbers(complete("Ph", 10)), [50, 57]);
        assert_eq!(numbers(complete("Ma", 10)), [19, 39, 40, 41]);
        assert_eq!(numbers(complete("Yo", 10)), [6, 29, 43]);
        assert_eq!(numbers(complete("1 The", 10)), [52]);
        assert_eq!(numbers(complete("1k", 10)), [11, 46]);
    }

    #[test]
    fn completion_puts_an_exact_alias_before_earlier_prefix_matches() {
        assert_eq!(numbers(complete("Am", 10)), [30, 20]);
        assert_eq!(numbers(complete("AMS", 10)), [20]);
    }

    #[test]
    fn completion_honours_the_limit_and_unknown_text() {
        assert_eq!(numbers(complete("Jo", 2)), [6, 18]);
        assert_eq!(numbers(complete("", 3)), [1, 2, 3]);
        assert_eq!(complete("Hezekiah", 5), []);
        assert_eq!(complete("Jo", 0), []);
    }
}
