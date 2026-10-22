# Scripture references

The reference parser lives in `src/scripture/`. It turns the text an operator
types into the Scriptures tab's reference box into a `Reference` (a book and a
span of chapters or verses) or a `ParseError` the tab can phrase. It knows
the canon and chapter counts but no Bible text, so it never touches storage,
the UI thread or the network.

## Books

`src/scripture/books.rs` holds the 66-book Protestant canon in canonical
order (Genesis is 1, Revelation is 66). Each book has:

- its OSIS id (`Gen`, `1Cor`, `Phlm`);
- its English name and its Indonesian name from LAI Terjemahan Baru
  (`John` and `Yohanes`, `Song of Solomon` and `Kidung Agung`);
- English and Indonesian aliases. The first alias of each language is the
  abbreviation shown when book names are abbreviated (`Book::abbreviation`),
  for EasyWorship's "Abbreviate Book Names" option (EW8-OBS-056);
- its chapter count in KJV versification (1,189 chapters in all).

A translation whose versification differs (for example a Hebrew-numbered
Joel or Malachi) is the Scriptures tab's concern: the tab checks verses
against the loaded translation.

## Syntax

A reference is a book name followed by a locator. Case does not matter.
Whitespace may be added or left out anywhere except inside a number
(`john3:16`, `John 3 : 16 - 4 : 2`). A space never joins numbers, so
`John 1 2` and `Ps 11 9` are malformed rather than John 12 or Psalm 119. A
space is not a chapter and verse separator either, because EasyWorship's
handling of `John 3 16` is unobserved.

| Form | Example | Result |
| --- | --- | --- |
| Whole chapter | `John 3` | chapters 3 to 3 |
| Chapter range | `John 3-4` | chapters 3 to 4 |
| Verse | `John 3:16` | 3:16 to 3:16 |
| Verse range | `John 3:16-18` | 3:16 to 3:18 |
| Cross-chapter range | `John 3:16-4:2` | 3:16 to 4:2 |

- `:` and `.` both separate chapter and verse (`John 3.16`, `Mzm 23.1-6`).
  Indonesian references use `:`, and `.` appears in European-style
  references. A dot inside the book name is an abbreviation dot
  (`Gen. 1:1`), because the book name ends at the first digit after its
  first letter.
- `-`, en dash and em dash separate the two ends of a range. After `C:V-`, a
  single number is a verse in chapter `C`, so `John 3:16-4` is verses 16 to
  4 of chapter 3 and therefore an invalid range.
- A numbered book may be written with or without a space (`1Cor`, `1 Cor`,
  `1 Kor`, `1Kor 13:4`, `1Cor13:4`).
- A book name may contain spaces, dots, hyphens and apostrophes
  (`Kisah Para Rasul`, `Hakim-hakim`, `1 Raja-raja`). That punctuation is
  ignored wherever it appears when the name is matched, so `Jo-hn 3` is
  John 3. A hyphen right before the chapter is not part of the name, so
  `John -3` is malformed.

Not supported, and reported as an error rather than guessed:

- Roman numerals and words for numbered books (`I Corinthians`,
  `First John`): unknown book.
- Comma or semicolon lists (`John 3:16,18`): malformed. Non-consecutive
  verses are a selection in the tab, not one reference.
- A whole chapter range ending in a verse (`John 3-4:2`): malformed.
- A verse number alone for one-chapter books (`Jude 5`): read as chapter 5,
  so chapter not found. Write `Jude 1:5`. EasyWorship's behavior here is
  unobserved.

## Book name resolution

The book text is compared as a key: letters and digits only, lowercased, so
`1 Cor.`, `1cor` and `1 COR` are one key. Every name, OSIS id and alias in
either language is a candidate. The rule, used by both `parse` and
`complete`:

1. A book with a name or alias exactly equal to the key wins.
2. Otherwise the first book in canonical order with a name or alias that
   starts with the key wins.

No two books share a name or alias key (a unit test checks the table), so
rule 1 picks at most one book. `complete(prefix, limit)` lists the rule 1
book first, then every rule 2 book in canonical order, up to `limit`; its
first entry is always the book `parse` would choose. An empty prefix lists
the canon from Genesis.

Examples:

| Typed | Resolves to | Why |
| --- | --- | --- |
| `Am` | Amos | exact alias, although Amsal (Proverbs) comes first |
| `Jo` | Joshua | Joshua, Job, Joel, Jonah, John start with `jo` |
| `Ju` | Judges | before Jude |
| `Ph` | Philippians | before Philemon |
| `Phil` | Philippians | exact alias |
| `Yo` | Joshua (Yosua) | before Yoel and Yohanes |
| `Ma` | Psalms (Mazmur) | before Malachi, Matthew and Mark |
| `1 The` | 1 Thessalonians | only match |
| `1 K` | 1 Kings | before 1 Korintus (1 Corinthians) |

The rule does not prefer a language, so a prefix shared across languages can
pick the other language's book (`Ma` gives Mazmur, not Malachi). A preference
for the loaded translation's language is an open option for the Scriptures
tab. EasyWorship 8.0.49's own completion order and accept key are not
observed yet (VAL-BIBLE-010); canonical order is the best guess.

## Errors

| Input | Error |
| --- | --- |
| empty or whitespace | `Empty` |
| `Hezekiah 1:1` | `UnknownBook("Hezekiah")` |
| `John` | `MissingChapter(John)` |
| `John 30:1`, `John 0:1`, `John 20-22` | `ChapterNotFound { book, chapter }` |
| `John 3:18-16`, `John 4:2-3:16`, `John 4-3` | `InvalidRange` |
| `3:16`, `John 3:`, `John 3:16a`, `John 3:0` | `Malformed` |

Chapters are checked before order, so `John 30:1-3:1` reports chapter 30.
Overlong numbers saturate at 65,535 and fail as a missing chapter.

`John 3:99` parses: the parser does not know verse counts. The Scriptures
tab validates every verse against the loaded translation and reports a verse
that translation lacks (VAL-BIBLE-009).
