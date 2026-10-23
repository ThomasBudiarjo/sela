# Scriptures

Status (2026-10-22, stream `scripture`): M4-03 and M4-04 are
**implemented-unqualified**. The behavior below is checked by unit tests
(`src/scripture/*`), GPUI tests (`src/tests/scripture.rs`, including the
Import Bible… button and its file prompt, the catalog refresh after an
import, translation switches and the `.selaschedule` v5 save/reopen) and the
Linux/X11 native script `scripts/scripture-x11.py` (typing, Go Live, the
malformed note, the WEB switch, keyword search, Import Bible… through the
portal dialog with the "(restricted)" row label, Add to Schedule, Ctrl+S /
Ctrl+O of the `.selaschedule` v5 file through the portal dialogs with the
item going live from its snapshot after its translation file was deleted;
with `--kind dual` also the per-slide reference strip on a 1920x1080
audience). A damaged or restricted translation row says so in words
("(damaged)", "(restricted)", EN/ID) in both its visible and accessible
label, tested in GPUI; only its red colour is untested. Windows runs, EasyWorship
8.0.49 observations and physical-output checks are still open
(docs/parity-pending.md, Stream: scripture).

| Part | Where |
| --- | --- |
| Reference parser, book names and completion | `src/scripture/reference.rs`, `books.rs` (below) |
| Verse and translation identity, versification inventory, schedule snapshot | `src/scripture/model.rs` |
| OSIS, USFX and Zefania importers | `src/scripture/import.rs` |
| One SQLite file per translation, integrity checks, keyword index | `src/scripture/store.rs` |
| Bundled KJV and WEB | `src/scripture/bundled.rs`, `assets/scripture/*.selabible.z`, `examples/scripture_bundle.rs` |
| Provider thread (install, passages, search, import) | `src/scripture/worker.rs` |
| Pagination and reference labels | `src/scripture/passage.rs` |
| Scriptures tab (GPUI) | `src/operator/resources/scriptures.rs` |
| Schedule items and `.selaschedule` version 5 | `src/schedule.rs`, `src/schedule_file.rs`, `src/operator/schedule_files.rs` |

## Bundled translations and provenance

Owner decision (docs/plan.md, Mission 2026-10): bundle the public-domain KJV
and WEB. ebible.org is blocked from the build container, so both texts come
from the GitHub repository `seven1m/open-bibles`, whose README lists every
file with its licence ("Repository of public domain and freely licensed
bibles"). Pinned commit `f257a3559025c3f873b48a75019f53a9354ed7de` (HEAD of
`master` on 2026-10-22).

| Id | Source file (raw URL at the pinned commit) | README licence | Licence statement inside the file | Source SHA-256 | Bundle SHA-256 (`assets/scripture/`) |
| --- | --- | --- | --- | --- | --- |
| KJV | `https://raw.githubusercontent.com/seven1m/open-bibles/f257a3559025c3f873b48a75019f53a9354ed7de/eng-kjv.osis.xml` (OSIS, 10,096,487 bytes) | Public Domain | `<rights type="x-copyright">public domain</rights>` | `eeeae647fc28360ce47f9c0d5cc3b397b7fdd9913fe53dc9f44eb6deee50e253` | `kjv.selabible.z` 1,309,571 bytes, `b9ce7933e081f6d869d2c8f1c6f5395c2951f9efd0acbd2a2f6b96ecb5922e8d` |
| WEB | `https://raw.githubusercontent.com/seven1m/open-bibles/f257a3559025c3f873b48a75019f53a9354ed7de/eng-web.usfx.xml` (USFX, 6,210,746 bytes) | Public Domain | Preface: "Because the World English Bible is in the Public Domain (not copyrighted), it can be freely copied, distributed, and redistributed ... reserve the name 'World English Bible' for faithful copies of this translation." | `5ffa2626f170a109a4a96afc90775c06f0821cb4ba81ed34e63663e085708d68` | `web.selabible.z` 1,297,127 bytes, `aaccbee472ce797f985c9587fcc284bcfe9f8b8f32e44a948a17b9cf36ba4686` |

Notes:

- The KJV file describes itself as "the standardized text of 1769"
  (publisher eBible.org). It is public domain except in the United Kingdom,
  where the King James Version is under Crown patent; the bundled rights line
  says so. Its deuterocanon (15 books) is not bundled.
- The WEB bundle keeps the verse text only: footnotes, cross references,
  headings, Psalm titles and the deuterocanon are left out, which keeps it a
  faithful copy of the translation's verse text under the name "World English
  Bible". The WEB numbers some verses differently from the KJV (it has no
  Luke 17:36, Acts 8:37, 15:34, 24:7 as separate verses and has the
  doxology at Romans 14:24-26 instead of 16:25-27): 31,098 verses against
  the KJV's 31,102.
- Rebuild a bundle with
  `cargo run --locked --example scripture_bundle -- SOURCE ID NAME LANGUAGE RIGHTS SOURCE-URL OUTPUT`,
  which runs the same importer the app uses on user files; then update the
  SHA-256 constant in `bundled.rs` (the test
  `the_shipped_bundles_are_intact_public_domain_texts` checks the counts,
  66 books, the rights text and John 3:16).
- Bundle format: deflate (`miniz_oxide` 0.8.9, already in the dependency
  tree through `png`, now a direct dependency) of UTF-8 text: `sela-bible 1`,
  `key<TAB>value` metadata lines, an empty line, then `BBCCCVVV<TAB>text`
  lines in canonical order. `decode` checks the SHA-256 first, inflates with
  a 16 MiB cap and rejects bad keys, order, empty or oversized verses.
- No copyrighted translation is bundled or used as a fixture (in particular
  not the Indonesian TB). Tests use synthetic text.

## Installed translations

`<profile>/Scripture/<ID>.selabible` is a SQLite file per translation
(`PRAGMA application_id` 0x53424942 "SBIB", `user_version` 1):

- `meta(key, value)`: id, name, language, rights, source, redistributable,
  bundled, verse count, `content` (SHA-256 over every verse's key, length
  and text) and `origin` (the bundle's or the imported file's SHA-256).
- `verses(id INTEGER PRIMARY KEY, text)` keyed by `VerseId::key`
  (`BBCCCVVV`, canonical order).
- `verse_index`: FTS5 over the verses (external content, `unicode61
  remove_diacritics 2`). Keyword search quotes every term
  (`search::expression`), the last one as a prefix, all terms must match;
  hits come in canonical order, 200 at most, with "more" reported.

A file is written to `<ID>.selabible.partial`, synced and renamed, so a
reader never sees half a translation. `store::open` refuses a file before
reading anything from it unless all of these hold: application id and
format version, `PRAGMA quick_check`, the metadata, verse count and content
hash. A damaged FTS index alone (quick_check failing only because of it, or
FTS5 `integrity-check` failing) is dropped and rebuilt in place.

The worker (`scripture::worker::Worker`, its own thread, bounded command
queue of 4) starts the first time the Scriptures tab is shown. Its catalog
step installs each bundled translation that is missing, damaged or built
from another bundle (its `origin` differs), from the bundle; an imported
file that fails the checks is listed as damaged and refuses passages
("re-import it"). On the Linux X11 run, installing both bundles took 1.1 s
in a debug build. Replies carry the caller's generation; the tab applies
only the newest.

## Versification and canon

Every translation keeps its own numbering: `Inventory` is built from the
verses actually present (per book, the highest verse of each chapter). A
reference is checked against it (`passage::locate`, VAL-BIBLE-009):

| Case | Result |
| --- | --- |
| Book missing (a New Testament only text) | "Genesis is not in YLT" |
| Chapter missing (another chapter division) | "John 4 is not in SYB" |
| Verse past the chapter's end | "John 3 has 36 verses in KJV" |
| Every referenced verse is a gap (WEB Acts 8:37) | "Not in WEB: Acts 8:37" |

On any of these, and on every `ParseError`, the list, selection and Preview
stay as they were. A translation switch keeps the selected verses: the new
translation's chapters are loaded (`passage::chapters`, which only needs the
chapters to exist) and the verses it lacks are named ("Not in WEB: 3 John
1:15"); if it lacks the book or chapter, the switch is refused and the old
translation stays shown. With nothing selected, the referenced verses are
selected in the new translation; a reference typed but not yet loaded, and
the box's text after a Reference/Keyword switch (which forgets the shown
reference), are looked up in the new translation instead, and nothing still
on its way for the old translation can land afterwards. The parser still uses KJV chapter counts, so a
translation with more chapters in a book than the KJV cannot reach them by
reference (none bundled; open).

## Importing user Bibles

Import Bible… (Scriptures footer) reads one OSIS, USFX or Zefania XML file
(recognized by the root element `osis`, `usfx` or `XMLBIBLE`) on the worker:

- Hardening as for song files: 48 MiB cap checked before reading, DTDs and
  entities refused, a linear nesting scan (64 levels) before the DOM, a
  3,000,000 node cap, 40,000 verses, 4 KiB per verse (cut with a warning),
  64 warnings.
- OSIS: milestone (`sID`/`eID`) and container verses, `osisID` with or
  without a work prefix; merged ids (`Gen.1.1 Gen.1.2`) are stored under the
  first with a warning; notes, titles, variant readings and figures dropped.
- USFX: `<book id>`, `<c id>`, `<v id>`/`<ve/>` milestones; footnotes,
  cross references, headings, Psalm titles (`d`) and front/back matter
  dropped.
- Zefania: `BIBLEBOOK bnumber`, `CHAPTER cnumber`, `VERS vnumber`; notes,
  cross references, captions and prologs dropped.
- Only the 66-book canon is kept; other books are skipped with one warning
  each. A verse split over paragraphs is joined. Text is whitespace-collapsed.
- The id comes from the file (`identifier`, OSIS work id) or the file name,
  upper-cased ASCII, and never replaces another translation (`SYA-2`).
  Imported rights are unknown to Sela, so the translation is marked
  restricted (`redistributable = false`). The flag is recorded, not yet
  enforced: a schedule item pinned from it still carries its verse text in
  the `.selaschedule` file (see Schedule items).
- After an import (whether it succeeded or failed) the catalog is refreshed
  for the new row only: the verse list, its selection, Preview and the
  schedule selection stay as the operator left them. The one exception:
  when no translation was usable before (nothing was shown), the refresh
  looks the box's text up in the newly usable translation and fills the
  list and selection, still without touching Preview.
- Import Bible… opens one file dialog at a time: while it is open, and
  while an import runs, the button is disabled and asks for nothing.

## Scriptures tab

Placement follows the EasyWorship Resources tab row (EW8-OBS-010: Songs,
Scriptures, Media, Presentations, Themes); the inside of the tab is an
UNOBSERVED best guess (docs/reference-observations.md, "Scriptures —
UNOBSERVED best guesses"):

- Left column: the installed translations (shown one selected; a damaged one
  in red). Clicking one switches translation as above.
- Top row: Reference / Keyword switch, the box, a clear button. Reference
  mode parses as you type; a book alone shows up to six book suggestions
  (`complete`), clicking one or pressing Tab takes the first; the hint "Type
  a chapter after John" is muted, errors are red. Keyword mode searches from
  two characters (owner decision).
- Verse list: the whole chapters around the reference, the referenced verses
  selected and scrolled into view; search hits show their reference. Click
  selects one verse, Shift+click a consecutive range, Ctrl+click toggles a
  nonconsecutive verse, double-click sends the selection live; Up/Down,
  Shift+Up/Down, Home/End and Ctrl+A in the focused list.
- Footer: Import Bible…, Add to Schedule, the latest message, the selection
  count.
- Typing in the box never triggers live shortcuts (`SelaShow &&
  !SelaTextInput`); Page Down from the verse list is Go Live as elsewhere.

Every string is in `src/i18n.rs` (English and Indonesian), every control has
an AccessKit id and role (`scripture-reference`, `scripture-mode-*`,
`scripture-suggestion-N`, `scripture-note`, `scripture-verses`, `verse-N`,
`translation-<ID>`, `scripture-import`, `scripture-add`, `scripture-message`,
`scripture-selection-count`). Problems reported by the worker (a damaged or
unreadable file, a failed bundle install, a stopped reader) are phrased in
the UI language; only a technical detail in parentheses (a check's name, the
system's error text) stays untranslated.

## Preview, Go Live and pagination

The selection becomes a Preview item at once (no file work: the verses are
already in the list), in canonical order whatever order they were picked
in. `passage::paginate` word-wraps the text into lines of 44 characters
(the renderer fits lines; it does not wrap) and fills slides of 6 lines,
breaking between verses; a verse longer than a slide continues on the next
slides without repeating its number. Verse numbers prefix each verse
(`chapter:verse` after a chapter change). Each slide's footer is the
two-line strip "<slide reference>" over "<translation name> (<ID>)", drawn by
the existing copyright-strip path (it needs an output at least 360 px high).
Go Live, Next/Previous and masks go through the existing ordered cue path:
the slides are prepared off the UI thread, Live changes only on the
renderer's receipt, and a failed preparation keeps the current scene. The
item title is "John 3:16-18 (KJV)" and its identity is the content hash of
translation and verses, so re-selecting the same verses is the same item.

## Schedule items

Add to Schedule pins a `Snapshot` of the selected verses (translation id,
name, language, redistribution flag, verse text): the item plays the same
text later even if the translation file is replaced, damaged or removed,
like a pinned song revision. Selecting the item previews its verses at once
(no library read). `.selaschedule` version 5 stores such items in place
between song items (`"kind":"scripture"`, verses as `[key, text]` pairs);
a schedule without scripture items is still written as version 4 (the
themes item snapshots), so an older Sela opens it, and an older Sela refuses
a version 5 file as newer. (Developed as version 4; renumbered to 5 at
integration on 2026-10-23 because the integration branch already used
version 4 for the themes item snapshot.) Scripture items keep the built-in
look: Apply to Item skips them and none captures a theme. Autosave
revisions (recovery format version 3, docs/autosave.md) embed the same
snapshot, so a crash-restored scripture item plays its pinned verses.
A snapshot is checked on read (canonical keys, order, sizes, its content
matching its pin). Restricted (imported) text is stored with
`redistributable: false`; the portable bundle export (M1-13) must leave such
text out (open).

## Reference parser

The parser turns the text typed into the reference box into a `Reference`
(a book and a span of chapters or verses) or a `ParseError` the tab phrases
in English or Indonesian. It knows the canon and KJV chapter counts but no
Bible text, so it never touches storage, the UI thread or the network.

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
