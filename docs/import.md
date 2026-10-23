# Song import parsers

Parsers for untrusted song files live in `src/import/`. Each one turns a
single file's bytes into a `ParsedSong` (sections of slides, a presentation
order, metadata and warnings) or a `Rejected` reason. They never touch the
library, the UI thread or the network. The import framework (M1-14, see
[Import framework](#import-framework-m1-14) below) owns staging, the staged
preview, duplicate policy and the commit transaction, and maps a
`ParsedSong` onto `storage::Song`.

Shared rules (`src/import/mod.rs`):

- Files over `MAX_FILE_BYTES` (1 MiB) are rejected before parsing.
- One UTF-8 BOM is removed and CRLF/CR become LF. Invalid UTF-8 is rejected,
  except where a format defines its own encoding (EasyWorship RTF).
- A section is a labelled group of slides. Each slide becomes one
  `storage::Section` (`import::song::to_song`). Only a section's first slide
  keeps the label; the others get an empty label and join its group, as in
  EasyWorship's editor (EW8-OBS-022).
- Section codes shared by OpenLyrics and OpenSong map to Sela labels with
  `label_for_code` (`v1` is "Verse 1", `c` is "Chorus").
- Anything a parser drops or guesses is reported as a `Warning`.

## OpenLyrics

`import::openlyrics::parse` reads OpenLyrics 0.8 and 0.9 song files.
`import::song::to_song` turns its result into a `storage::Song`. The format
reference is the OpenLyrics data format page at docs.openlyrics.org, read on
2026-10-22. The `version` attribute is not checked, because 0.8 and 0.9 share
every element mapped here. OpenLyrics 0.7 `<line>` elements are read as one
line each.

### Rejected files

- Any DOCTYPE gives `Entities`, including an empty `<!DOCTYPE song>`.
  roxmltree runs with `allow_dtd: false` and no entity resolver, so it never
  opens a file or a URL for an entity. An undeclared reference such as
  `&x;` gives `Malformed`.
- Nesting deeper than 32 elements gives `TooDeep`. A linear scan checks the
  depth before roxmltree parses, because roxmltree's element parser recurses
  once per level. Without the scan, a 1 MiB file of open tags overflowed the
  test thread's stack.
- More than 50,000 XML nodes give `TooLarge` (roxmltree `nodes_limit`).
- A file that is not well formed gives `Malformed`, with roxmltree's message
  and position.
- A root other than `<song>` in the namespace
  `http://openlyrics.info/namespace/2009/song` gives `WrongFormat`. An
  OpenSong file also has a `<song>` root, but without that namespace.
- A file without verse text gives `NoLyrics`. A file whose first slide alone
  is over the text budget gives `TooLarge`.

### Field mapping

| OpenLyrics | Sela | Rule |
| --- | --- | --- |
| First non-empty `<title>` | Title | Decision: file order wins, because the spec names no primary title. Other titles are reported as `UnknownElement("title")`. A title of only spaces, Unicode spaces included, counts as none. No title gives `MissingTitle`, which is kept even when the warning list is full. |
| Each `<author>` | Authors | Duplicates are dropped. `to_song` joins them with `"; "` in the one-line Authors field (names may contain commas). |
| `<copyright>` | Copyright | |
| `<ccliNo>` | Song Number | ASCII digits only, at most 12. Anything else gives `InvalidField("ccliNo")` and is dropped. |
| `<publisher>` | Administrator | Decision: the closest EasyWorship song property. |
| `<verseOrder>` | Arrangement | See the order rules below. |
| `<verse name>` | Section label | XML whitespace in the name collapses to one space. Names that follow the code convention (a letter, digits, an optional part letter) go through `label_for_code`. Other names stay verbatim, so `bridge` stays "bridge". A missing or blank name gives an empty label and `InvalidField("verse name")`. |
| Each `<lines>` | One slide | See the text rules below. |

`<copyright>`, `<ccliNo>`, `<publisher>` and `<verseOrder>` hold one value.
The first one in the file is kept and each later one is reported as
`UnknownElement(name)`.

Fields over the song model's limits are cut at a grapheme boundary with
`Truncated(field)`. The limits are 1,024 bytes for the title, 4,096 for the
authors joined and for the copyright, 256 for the publisher, and 256 for a
section label.

Order rules:

- Names match case-insensitively.
- A name selects every verse with that name, in file order. Translations of a
  verse share its name (`lang` differs), so all of them follow each other.
- An unknown name gives `MissingOrderReference(name)`. A name of a verse that
  had no text, or that was dropped at a limit, is skipped without that
  warning, because `EmptySection` or `LimitReached` already reports it.
- Without a `<verseOrder>` the song has no arrangement and plays in stored
  order, like a song made in the editor.

Text rules inside `<lines>`:

- `<br/>` is a line break, and so is the end of a 0.7 `<line>`. Other XML
  whitespace (space, tab, CR, LF) collapses to one space and each line is
  trimmed. Unicode spaces such as U+3000 and U+00A0 stay as written.
- Blank lines at the start and end of a slide are dropped. Blank lines between
  text lines stay.
- `<chord>` markup is removed and its text kept, with one `ChordsRemoved`.
- `<comment>` is dropped. It holds performance notes that OpenLyrics does not
  display either.
- `<tag>` and any other element keep their text. Their markup is reported as
  `UnknownElement(name)`.
- A verse whose slides are all empty gives `EmptySection(label)`.

Not imported, reported once each as `UnknownElement(name)`: `<format>`,
`<instrument>`, and every property not listed above, such as `keywords`,
`themes`, `songbooks`, `releaseDate`, `tempo`, `key` and `comments`.

Not imported, reported once each as `IgnoredAttribute("element/@name")`
(added by the import framework, 2026-10-22): every attribute of `<title>`,
`<author>` and `<lines>`, and every attribute of `<verse>` except `name`,
such as `author/@type`, `author/@lang`, `title/@lang`, `title/@original`,
`verse/@lang`, `verse/@translit`, `lines/@part`, `lines/@break` and
`lines/@repeat`. A repeated chorus (`lines/@repeat`) therefore plays once
unless the `verseOrder` repeats it. Attributes of `<song>` itself (version,
creating application, dates) describe the file, not the song, and are not
reported.

Limits: 128 sections, 128 slides, and an order of at most 512 slides after
expansion. Slide text and labels share a budget of 240 KiB, priced like the
arrangement snapshot (24 bytes per slide on top of the text). Anything past a
limit is dropped with one `LimitReached`.

### Storage mapping

`to_song` makes one `storage::Section` per slide, with IDs from
`SectionId::allocate`. The first slide of a section carries its label and the
others have an empty label (see the shared rules). A non-empty order becomes the
song's only arrangement, "Arrangement 1". Each order entry expands into all
slides of its section. License and backgrounds stay empty. The result passes
`Song::validate`, or `to_song` returns `Error::Invalid`. The caller must
substitute a title first when the parser reported `MissingTitle`.

### Licensing (L check, 2026-10-22)

The OpenLyrics specification states no license. The
`openlyrics/openlyrics` repository has no LICENSE file and its README has no
license statement. GitHub's license API returns 404 for it. The
documentation's Sphinx configuration says "2009-2021, Martin Zibricky, Raoul
Snyman, Samuel Mehrbrodt and others". The project calls the format a free,
open XML standard. Sela implements the format from the data format page and
copies none of the specification's text, schemas or example songs. Every
fixture in `tests/fixtures/import/openlyrics/` and its lyrics were written
for Sela.

## OpenSong and ChordPro

`opensong::parse` and `chordpro::parse` (M4-07) produce a `ParsedSong` only.
Chords are removed in both formats and reported once as `ChordsRemoved`;
Sela's song model has no chord data, so nothing keeps them as hidden data.
Every warning is reported once per file, however often its cause repeats.
A song over the limits keeps its first `MAX_SECTIONS` sections, its first
`MAX_SLIDES` slides and its first `MAX_ORDER` order entries, with
`LimitReached`. A file with no lyric text is `Rejected::NoLyrics`. A missing
title is `Warning::MissingTitle` with an empty title, and the caller
substitutes the file name. A CCLI number that is not ASCII digits
(`storage::song_number_ok`) is dropped with `InvalidField("ccli")`.

### OpenSong

The file is XML with a `<song>` root (another root is `WrongFormat`). It is
parsed with roxmltree with DTDs disallowed, so any `<!DOCTYPE` is
`Rejected::Entities` and no entity is ever resolved. More than 100,000 nodes
is `TooLarge`. roxmltree descends nested elements recursively, so a scan of
the text rejects nesting deeper than 32 elements as `TooDeep` before the
parse; without it, a deeply nested file overflows the stack.

Metadata, from direct children of `<song>`:

- `<title>`, `<copyright>` and `<ccli>` are trimmed.
- `<author>` is split into names at `,` `;` `&` `/` `|` and at the word
  "and" (any case). A surname-first name such as "Smith, John" therefore
  becomes two names; the staged preview shows them for correction. The
  Indonesian "dan" is not a separator.
- `<presentation>` is a whitespace-separated list of section names, matched
  without regard to case. A bare tag whose lines are all numbered, such as
  `V` for `[V]` with lines `1 ...` and `2 ...`, plays its numbered verses in
  order (V1, V2). A name no section answers to is `MissingOrderReference` and
  is skipped; this includes `V` when the only `V` is the empty text before
  the first tag. A name whose sections are all empty or cut by the limits is
  skipped quietly, because `EmptySection` or `LimitReached` already reports
  it. An empty presentation means file order.
- Any other element with text, such as `<key>` or `<hymn_number>`, is
  `UnknownElement` with its name: Sela has no field for it. Empty elements
  are ignored.

Lyric lines are typed by their first character:

| Line starts with | Meaning |
| --- | --- |
| `[` | A section tag up to `]`, such as `[V1]`. An empty tag counts as `[V]`. Text after the `]` is the tag's first lyric line, so nothing on the line is lost. |
| `.` | A chord line. It is removed. |
| `;` | A comment. It is dropped without a warning. |
| a digit | A numbered line, such as `1 text` under `[V]` (see below). |
| anything else | A lyric line. It is trimmed, which removes OpenSong's leading space. |

- Tag labels: a tag that is one letter and optional digits (`V1`, `C`, `B2`,
  `P`, `T`) maps through `label_for_code`. Any other tag, such as `[Coda]`,
  is kept verbatim as the label. `label_for_code` reads only the first
  letter, so `Coda` would otherwise become "Chorus".
- Numbered verses: under one tag, lines that start with the number N form a
  section named tag plus N, in order of first appearance. `[V]` with lines
  `1 ...` and `2 ...` gives "V1" (Verse 1) and "V2" (Verse 2). Unnumbered
  lyric lines under that tag stay in the tag's own section, which comes
  before the numbered verses wherever those lines sit, and the bare tag name
  then plays only that section. A tag whose lines are all numbered gives no
  `EmptySection`.
- Text before the first tag forms a section named `V` ("Verse").
- `||` in a lyric line ends the slide; the rest of the line starts the next
  one. A single `|` breaks the line in two. Empty pieces left by either are
  dropped, so a line that is only `|` adds nothing.
- Blank lines at the start and end of a slide are trimmed. Blank lines inside
  a slide are kept.
- A tag with no lyric lines is `EmptySection` and produces no section.
- A tag defined twice gives two sections; the presentation order uses the
  first.

Not handled: encodings other than UTF-8 (`InvalidUtf8`), lines such as `---`
(kept as lyric text), and OpenSong set files (another root, so
`WrongFormat`).

### ChordPro

The file is UTF-8 text, one line at a time. A line that starts with `#` is a
comment and is ignored. A trimmed line that starts with `{` is a directive:
the name runs to the first `:` or space, and the rest is its value. A missing
closing `}` is tolerated. Directive names are matched without regard to case.
An empty `{}` is ignored; a directive with a value but no name, such as
`{: x}`, is `UnknownDirective` with the whole line.

| Directive | Result |
| --- | --- |
| `title`, `t` | The title. The first one wins. |
| `subtitle`, `st` | The authors when the file has no `artist`, `composer` or `lyricist`; otherwise dropped with `UnknownDirective("subtitle")`. |
| `artist`, `composer`, `lyricist` | One author each, with duplicates removed. Values are not split. |
| `copyright`, `ccli` | Those fields. The first copyright wins. |
| `meta` | `{meta: artist Name}` acts as `{artist: Name}` for the keys above. Other keys are `UnknownDirective(key)`. |
| `comment`, `c`, `comment_italic`, `ci`, `comment_box`, `cb`, `highlight` | Dropped. They never become lyric text. |
| `start_of_chorus`/`soc` ... `end_of_chorus`/`eoc` | A chorus section. |
| `start_of_verse`/`sov` ... `end_of_verse`/`eov` | A verse section. |
| `start_of_bridge`/`sob` ... `end_of_bridge`/`eob` | A bridge section. |
| `start_of_tab`/`sot`, `start_of_grid`/`sog` and their ends | Their lines are skipped; a non-blank one gives `ChordsRemoved`. |
| `chorus` | Repeats the last chorus in the order. Before any chorus it is `MissingOrderReference("chorus")`. |
| anything else | `UnknownDirective(name)`, including known formatting directives such as `key`, `capo` and `new_page`. |

- Sections: outside an environment, blank lines separate verses, labelled
  "Verse 1", "Verse 2" and so on. Inside an environment, blank lines separate
  slides of one section. Starting an environment ends the current one; an
  end directive ends whichever is open, and an unclosed one ends at the end
  of the file.
- Labels: `{soc: Refrain}` or `{soc label="Refrain"}` names the section.
  Otherwise choruses are "Chorus", "Chorus 2", bridges "Bridge", "Bridge 2"
  and verses continue the verse count. Named sections still advance the
  count.
- Order: when the file has a `{chorus}`, the order lists every section in
  file order with the repeats inserted. Without one it is empty (file order).
- An environment that ends with no lyric lines is `EmptySection`.
- Inline chords: brackets nest, and each matched pair is removed with what
  is inside it. `[G]Amazing [C]grace` is "Amazing grace", `grace[G]` is
  "grace" and `x[A[B]]y` is "xy". An unmatched `[` and the text after it are
  kept, as is a stray `]`. Chord names have no spaces, so brackets around
  spaced words are lyrics: `Open [G line [C] more] end` is "Open [G line
  more] end", not "Open end". An annotation such as `[*To Coda]` is removed
  like a chord. On a line that had a chord, runs of spaces become
  one space. A line that held only chords is dropped, so it never separates
  two verses.
- Lyric lines are trimmed at both ends.

Not handled: several songs in one file (`{new_song}` is an unknown
directive, so the songs merge), conditional directive selectors such as
`{title-guitar}`, and encodings other than UTF-8.

### Tests

Unit tests in each module use the original fixtures in
`tests/fixtures/import/opensong/` and `tests/fixtures/import/chordpro/`. Each
module also runs bounded proptest cases (256 per property) on arbitrary bytes
and on token soups of format syntax, and checks the limits and that no
chord survives. OpenSong also runs a soup of XML markup (comments, CDATA,
processing instructions, quoted `>` and self-closing tags) against
roxmltree's own depth, and checks that `TooDeep` starts exactly at the cap. Failures found by these runs are stored as seeds in
`proptest-regressions.txt` in the fixture folders and replay first on every
`cargo test`.

## EasyWorship song words

`ew_rtf::parse` turns one `word.words` value from EasyWorship's
`SongWords.db` into sections. The value is an RTF string. The importer
framework reads the row and maps the `Songs.db` metadata, so the parser fills
only `sections` and `warnings`. A missing `word` row is the framework's case
(VAL-IMPORT-009): the parser never sees one.

### Observed format

Observed on 2026-10-22 in a read-only temp copy of the local EW 8.0.49
`SongWords.db` (one song, four slides). Only control-word structure was
recorded, never lyric text:

- Header `{\rtf1\ansi\deff0\sdeasyworship2`, then `{\fonttbl{\f0 Tahoma;}}`
  and `{\colortbl ;}`. The text is ASCII and uses CRLF between groups.
- One group per paragraph:
  `{\pard ... \sdewparatemplatestyle101 \plain \sdewtemplatestyle101 \fs156 {\*\sdfsreal ..}{\*\sdfsdef ..} \sdfsauto <text>\par}`.
- A label paragraph adds `\sdparawysiwghidden\sdlistlevel-1` and uses style
  102 instead of 101.
- A slide break is an empty paragraph flagged `\sdslidemarker`. It sits
  between slides, including before a label paragraph that starts a new group.
- The sample parses into two sections: one label with slides of 3, 2 and 1
  lines, then one label with a 1-line slide. That matches its four
  `slide_uids`. Both labels are custom (not in `labels::parse`).

The older dialect comes from the research on the EW demo package (mission
research `ew-import-and-ux.md`, Part A). It is generic RichEdit RTF with
`\fcharset0` font tables and `{\*\pnseclvlN}` list destinations, and it has no
EasyWorship control words. Slides are separated by blank lines, and a block
may start with a label line such as `Verse 1`.

### Structure

- **Dialect.** Words that contain `\sdeasyworship`, `\sdparawysiwghidden` or
  `\sdslidemarker` use the EW8 rules. All other words use the legacy rules.
- **EW8.** A label paragraph starts a section. Its whitespace is collapsed,
  and the text is kept as written, custom labels included. `\sdslidemarker`
  ends a slide. Later slides join the section until the next label
  (EW8-OBS-022). Slides before the first label get the label "". A label that
  follows lyrics without a marker starts a new slide, because a label is
  always its slide's first line. Text inside a marker paragraph starts the
  next slide. Blank lines stay inside their slide (EW8-OBS-023). A lyric line
  that reads "Chorus" stays lyrics.
- **Legacy.** Blocks are separated by blank lines. If a block's first line
  parses with `labels::parse` ("Verse 2", "CHORUS", "Pre-Chorus 1"), it
  becomes the label, kept as written. A block without a label joins the
  previous labelled section. If there is none, it becomes "Verse N", numbered
  1, 2, ... across the generated labels only. A label alone on its block
  labels the blocks that follow it.
- **Slides.** Every line is trimmed, and leading and trailing blank lines are
  removed. A slide left empty is dropped. So is a section left with no slides,
  with one `EmptySection(label)` warning per section. Words without any
  slide are `Rejected::NoLyrics`. Hitting `MAX_SECTIONS` or `MAX_SLIDES` stops
  the parse with `LimitReached`.

### Decoding

- **Input.** The parser takes bytes, checks `MAX_FILE_BYTES`, and does not
  use `decode()`. After leading whitespace the input must start with `{\rtf`,
  or it is `WrongFormat`.
- **Raw bytes.** A byte of 0x80 or above in lyric text is
  `Rejected::Malformed`. RTF is 7-bit, and the words come from an SQLite TEXT
  column, so a raw byte could be UTF-8 or a code page. Rejecting that one song
  is better than guessing and importing mojibake. Raw bytes inside skipped
  destinations, such as a font name, are ignored.
- **Code pages.** `\'hh` decodes in the code page of the current font's
  `\fcharset`: 0 is Windows-1252, 204 is Windows-1251, and 1 or none falls back
  to the document's `\ansicpg`. `\ansicpg` supports 1252 (the default) and
  1251. Any other declaration (`\ansicpg1253`, `\fcharset161`, `\mac`, the
  DBCS pages such as 932) decodes as 1252 and adds one
  `UnknownDirective` warning that names it. The warning appears only when a
  `\'hh` actually needed that page. Bytes the page does not define become
  U+FFFD.
- **Unicode.** `\uN` adds 65536 to a negative N. Surrogate pairs combine,
  and a lone surrogate becomes U+FFFD. `\ucN` is scoped to its group and
  defaults to 1. It sets how many fallback units follow each `\uN`, and
  those are skipped. A text character, a `\'hh`, or a control word or symbol
  each count as one unit, and a brace ends the skip.
- **Groups.** Nesting deeper than `ew_rtf::MAX_DEPTH` (64) is `TooDeep`. An
  unmatched `}`, an unclosed group, a trailing backslash, a bad `\'` escape,
  or text after the closing brace is `Malformed`. Trailing whitespace and NUL
  bytes are allowed.
- **Skipped.** `\*` destinations, the font, color and style tables, `\info`,
  `\pict`, headers and footers, footnotes, `\pntext`, the list tables and
  `\fldinst` are skipped. `\fldrslt` text is kept. `\binN` skips N raw bytes.
- **Text.** `\par`, `\sect`, `\page` and a backslash before a newline end a
  paragraph, and `\line` is a line break. `\tab`, `\cell` and a raw tab become
  a space. The quote, dash and bullet words map to their characters.
  `\~` is U+00A0, `\_` is U+2011, `\-` is dropped, and so are control
  characters.
- **Formatting.** `\b`, `\i`, `\ul` and `\strike` are dropped, with one
  `UnknownElement` warning each. Font size, alignment and the EW style
  numbers are dropped without a warning, because the theme owns them.

### Tests and open items

The synthetic fixtures are in `tests/fixtures/import/ew_rtf/` (original text).
The tests in `src/import/ew_rtf.rs` cover:

- Both dialects, including custom labels and labels without numbers.
- Blank lines inside an EW8 slide, empty slides and sections.
- `\'hh` in 1252 and 1251 with font charsets, unsupported pages, and `\uN`
  with `\ucN` and surrogates.
- Destinations and `\bin`, malformed braces, depth, size and slide limits,
  and worst-case 1 MiB inputs that must parse in linear time.
- Bounded proptests (VAL-IMPORT-017, 256 cases each): arbitrary bytes, RTF
  token soup, and EW8 and legacy round trips. Their failures persist to
  `tests/fixtures/import/ew_rtf/proptest-regressions.txt`, which proptest
  replays.

Still unobserved in EW 8.0.49:

- How EW8 writes non-ASCII lyrics (`\uN`, `\'hh` or raw text). The only
  sample is ASCII.
- Where per-slide formatting is saved.
- Whether EW8 shows a legacy block's "Verse 1" first line as a label. Sela's
  "Verse N" numbering of unlabelled legacy blocks is Sela's own choice.
- Whether a label paragraph ever follows lyrics without a marker.

## EasyWorship library (M4-07)

Status: **implemented-unqualified** (ew-library-import stream, 2026-10-23).
Linux evidence only: unit tests in `src/import/ew_library.rs` and
`src/import/worker.rs`, GPUI tests in `src/tests/ew_library_import.rs` and
`scripts/ew-import-x11.py`. `scripts/ew-import-windows.py` is NOT RUN (no
Windows host). Every schema detail below is an **UNOBSERVED best guess**
from public sources (the open-source OpenLP EasyWorship importer's
description of `Songs.db`/`SongWords.db` and the one read-only look at the
owner's `SongWords.db` recorded under "Observed format" above). The checks
against a copy of the owner's real EW 8.0.49 library are PP-EWL-01..08 in
docs/parity-pending.md (Stream: ew-library-import); the guesses are
EW8-OBS-EWL-1..7 in docs/reference-observations.md.

### Entry point and folder

File › Import ▸ **EasyWorship Library…**, after Song Folder… (UNOBSERVED
placement, EW8-OBS-EWL-1). It opens the native folder chooser (one folder,
`prompt_for_paths` with `directories: true, multiple: false`; asynchronous,
the UI never waits). The chosen folder, or the first of these folders below
it, must hold `Songs.db` with `SongWords.db` beside it
(`ew_library::DATA_FOLDERS`): `.`, `Data`, `Databases/Data`,
`v6.1/Databases/Data`, `Default/v6.1/Databases/Data`. So the EasyWorship
profile folder (`...\EasyWorship\Default`), its `v6.1` folder, `Databases`
or `Data` may be chosen. Names compare without ASCII case (a library copied
from Windows). The disabled state, the preview dialog, the staged rows,
duplicates, commit and cancel are the import framework's below.

### Read-only copies

The user's library is never handed to SQLite. On the `sela-import` thread
(`import::worker`, `Source::EasyWorship`), `ew_library::Library::open`:

1. finds the two databases;
2. copies each one, plus its `-wal` and `-journal` when present, into a new
   private temporary folder (`tempfile`, `sela-ew-library-*`), reading the
   sources through the same regular-file open as song files (`O_NONBLOCK` on
   Unix); at most 256 MiB per file (`MAX_DB_BYTES`, checked from the size
   and while copying); the cancel flag is checked between 1 MiB chunks;
3. compares each source's size and modification time before and after the
   copy, and fails with "The library changed while it was copied. Close
   EasyWorship and import again." when one changed or a journal appeared
   (EasyWorship saving while Sela copies);
4. opens the copies read-write (so SQLite can replay a copied `-wal` or roll
   back a copied hot journal in the copy only) with `trusted_schema = OFF`,
   a 2 MiB value limit (`SQLITE_LIMIT_LENGTH`), the `UTF8_U_CI` collation
   registered, and `query_only = ON` before any query of the songs;
5. checks the schema: table `song` with a `title` column, table `word` with
   `song_id` and `words`.

The copies are deleted when the job ends (the `Library` is dropped). Tests
hash every source file (SHA-256, size, mtime) before and after, including a
library whose rows live only in an open `-wal` (`ew_fixture::write_databases`
with `wal: true`).

### `UTF8_U_CI`

EasyWorship's schema declares this collation on its text columns (public
sources); SQLite refuses any statement that sorts or compares such a column
until a collation of that name exists ("no such collation sequence", tested).
Sela registers it as Unicode lower case, then code point order
(`ew_library::utf8_u_ci`). EW's exact rules are UNOBSERVED (PP-EWL-03); only
the preview order depends on them: songs are listed by
`title COLLATE UTF8_U_CI`, then rowid.

### Field mapping (UNOBSERVED column names)

| `Songs.db` `song` column | Sela | Rule |
| --- | --- | --- |
| `rowid` | row name `Songs.db #N` | `word.song_id` refers to it. |
| `title` | Title | Line breaks become spaces; trimmed; cut at 1,024 bytes with `Truncated("title")`. Empty: "Untitled" with `MissingTitle`. |
| `author` | Authors | One line, kept as one author (no splitting), cut at 4,096 bytes. |
| `copyright` | Copyright | Trimmed, cut at 4,096 bytes. |
| `administrator` | Administrator | One line, cut at 256 bytes. |
| `vendor_id` | Song Number (CCLI) | Integer or digits, at most 12; anything else gives `InvalidField("vendor_id")` and is dropped. |
| `description` | none | Sela songs have no description field: `IgnoredAttribute("song/description")` when not empty. |
| `tags` | none | No keyword field yet: `IgnoredAttribute("song/tags")` when not empty. |
| `song_uid` | `Staged::source_id` | Duplicate detection inside one import (below); not stored. |
| other columns | none | Not read (layout, revision, presentation ids). |

Only `title` is required; missing optional columns read as empty. A value
that is not valid UTF-8 is decoded with replacement characters and gives
`InvalidField(<column>)`. Words: the newest `word` row (highest rowid) for the
song's rowid, parsed by `ew_rtf::parse` ("EasyWorship song words" above);
older rows for the same song give `IgnoredAttribute("word (older rows)")`.

### Failures

- No `word` row, or a NULL `words`: that song's row says "No words for this
  song in SongWords.db" (`FileError::MissingWords`); the other songs import
  (VAL-IMPORT-009).
- Words the RTF parser refuses (not RTF, malformed, too deep) or over 1 MiB
  (or over the 2 MiB SQLite value limit): that song's row with the parser's
  reason, under the EasyWorship format.
- A library that cannot be read at all is one failed row named after the
  chosen folder (`FileError::Library`): no `Songs.db` ("No EasyWorship library
  here..."), `Songs.db` without `SongWords.db`, over 256 MiB, changed during
  the copy, unreadable, not a database or not the expected tables ("Not an
  EasyWorship library: ..."). Import stays disabled.
- At most 500 songs per import (`batch::MAX_FILES`); the rest are counted
  ("Songs not read, over the limit of 500 per import: N") and are not
  staged. The 32 MiB staged-text budget applies as for files. Importing the
  remainder of a larger library is open (backlog M4-07).

### Duplicates and re-import

The framework's policy applies unchanged (title and words key against the
library and inside the import). In addition, a later row whose `song_uid`
equals an earlier row's is a duplicate inside the import ("Same song as
Songs.db #N in this import", default Skip), even when its title or words
differ (`batch_duplicates`, tested). Importing the same library again finds
every song in the library with the same words, so each defaults to Skip and
Import shows (0). Sela does not store the `song_uid`, so an EW song edited
after an import is matched by title (Import as copy by default, or Replace).
How EW itself treats a re-import is UNOBSERVED (PP-EWL-05).

### Tests

- U (`src/import/ew_library.rs`, synthetic `Songs.db`/`SongWords.db` built
  by `src/import/ew_fixture.rs`): metadata and words mapping; sources hashed
  before and after, copies deleted; rows only in a `-wal`; collation order
  and SQLite's refusal without it; missing word row and NULL words; older
  word rows; truncation, invalid CCLI, missing title, invalid UTF-8; words
  over the parser and value limits; folder search and case-insensitive
  names; not-a-library refusals; a source changing during the copy;
  cancel; an oversized database.
- I (`src/import/worker.rs`): a library job staged in title order with a
  missing-words row and sources unchanged; a folder without a library is
  one failed row; the 500-song limit and cancel.
- `operator::song_import::tests`: `song_uid` duplicates inside an import;
  every new error has EN text.
- GPUI (`src/tests/ew_library_import.rs`): menu, folder chooser options,
  preview rows, warnings, the uid duplicate, commit with metadata read back
  from the library, re-import as duplicates, cancel, sources unchanged; a
  folder without a library.
- Native: `scripts/ew-import-x11.py` (PASS on Linux/Xvfb) and
  `scripts/ew-import-windows.py` (NOT RUN: no Windows host). Both build the
  library with `scripts/ew_library_fixture.py`.

## Import framework (M1-14)

Status: **implemented-unqualified** (import-framework stream, 2026-10-22).
Linux evidence only: GPUI tests and `scripts/song-import-x11.py`. Windows
(`scripts/song-import-windows.py`) and every EasyWorship comparison are open
(docs/parity-pending.md, "Stream: import-framework").

### Entry point and chooser

File › Import ▸ Songs… (files, several at once) and Song Folder… (one or
more folders) in the operator menu bar; EasyWorship Library… (one folder)
is described in "EasyWorship library" above. The Import ▸ group is flattened into
the File popup like New ▸, after Close Schedule. Where EasyWorship 8.0.49
offers library import is **UNOBSERVED**: its observed File menu
(EW8-OBS-053) has no Import row and its Songs context menu (EW8-OBS-061)
none either; the Song Editor's Import ▾ (EW8-OBS-021) imports into one open
song. See docs/reference-observations.md, "Song import — UNOBSERVED best
guesses". The native chooser is GPUI's `prompt_for_paths` (asynchronous; the
UI never waits on it). All three items are disabled while an import runs.

### Which files, which parser (`import::batch`)

- `.cho`, `.chopro`, `.chordpro`, `.crd`, `.pro`: ChordPro.
- `.xml`, or no extension with content that starts with `<`: OpenLyrics when
  the root is `<song>` in the OpenLyrics namespace, otherwise OpenSong. A
  well-formed file with another root is "not a song file".
- A chosen file of any other type is listed as unsupported. Inside a chosen
  folder, other files are only counted ("Not song files, skipped: N"),
  because song folders hold pictures and notes too. A file without an
  extension is the exception, in a folder too: OpenSong songs have none, so
  it is read, and one whose content is not a song gets a row "Not an
  OpenLyrics, OpenSong or ChordPro song file". Folders are walked three
  levels deep without following symbolic links; hidden entries are skipped;
  only regular files are candidates (a named pipe, socket or device is
  counted as not a song file, and a chosen one is refused from its metadata
  before it is opened; on Unix the open itself does not wait
  (`O_NONBLOCK`) and the opened file's type is checked again, so a pipe
  swapped in after the first check cannot block the worker either); at most 20,000
  directory entries are looked at per chosen folder, and a folder cut there
  is named in the preview ("Folder has more than 20000 entries; files after
  that were not looked at: …"). Which entries fall past the budget depends
  on the file system's listing order.
- Limits: 500 files per import (the rest are counted as not read), 1 MiB
  per file (checked from the file size before reading, and again while
  reading), 32 MiB of staged song text (later files get "Not read: one
  import holds at most 32 MiB of song text", distinct from the 500-file
  message), and the library's own song limits
  (`Song::validate`, `storage::MAX_SONG_BYTES`), which give "larger than the
  library allows".
- A missing title becomes the file name (without extension); the
  `MissingTitle` warning stays and the preview says so.

### The `sela-import` worker (`import::worker`)

One long-lived thread, started on first use. Its job channel holds one job
and its event channel `EVENT_DEPTH` (16) results; when the UI does not read,
the worker blocks instead of queueing more (tested). Events carry the job
number, so results of a cancelled job are dropped. Cancel is checked before
every file; `Done { canceled }` ends a job, or `Failed` when the thread ended
without finishing it (a panic): the operator closes the preview, says "The
import stopped unexpectedly …" and starts a new thread for the next import
(pr1-review). The UI handles at most 64
events per 16 ms poll. Nothing in the worker touches the library.

### Staged preview

A modal dialog (`Dialog::ImportSongs`, `src/operator/song_import.rs`) that
opens while files are read and lists every file in order: the song title, a
format chip, the file name and slide count, every warning as text, and for
a failed file its error in red. A summary line counts ready songs, files
that cannot be imported and duplicates. Nothing is saved while the dialog is
open. Keyboard: focus starts on Cancel; Tab cycles the per-row choice groups,
Import and Cancel inside the dialog; Left/Right (or Up/Down) change the
focused group's choice; a row focused from the keyboard scrolls into view;
Esc is Cancel. Accessibility: the dialog is a Dialog named "Import Songs",
the list a List, each row a ListItem named by the song title (or the file
name) with the path as its description, each choice group a RadioGroup
("What to do with <title>") of RadioButtons with their toggle state, the
summary and progress lines Status nodes.

### Duplicates (UNOBSERVED best guess)

After reading, the operator asks the storage worker (`find_duplicates`) for
library songs with the same title, compared case-insensitively with
punctuation and spacing ignored (`batch::title_key`; accents count). A
match with the same words (`batch::lyrics_key`, a SHA-256 of the
normalized lyrics in slide order, labels and metadata ignored) wins over a
title-only match. Inside one import, a later file with the same title and
words points at the first one.

| Kind | Default | Choices |
| --- | --- | --- |
| Not a duplicate | Import | Import, Skip |
| Same title and words as a library song | Skip | Skip, Import as copy, Replace |
| Same title, other words | Import as copy | Skip, Import as copy, Replace |
| Same song earlier in this import | Skip | Skip, Import as copy |

"All duplicates" sets one choice on every row that offers it. Import as
copy adds a new song with the same title. Replace saves the file's song as
a new revision of the library song (same song id): schedules keep the
revision they pinned until Update items in Schedule, and the old revision
stays in history. Replace overwrites words, labels and metadata, drops
that song's backgrounds, master background and per-slide formatting
(the file has none), and replaces its arrangement variants with the file's
order (none when the file has no order). The dialog's Replace note says
so in EN and ID.
Two rows replacing the same song are refused before anything is saved.

### Commit and cancel

Import (n) sends every accepted song to the storage worker as one
`Command::CommitImport`. `Repository::commit_import` validates all songs,
then writes them in ONE immediate transaction through `write_song`, the
same path as `save_song` (revision, section ids, formats, metadata, search
index row, Date modified). Any failure rolls the whole batch back and the
preview stays open with "Nothing was imported: …" for a retry. A replaced
song that changed since the check fails with `Conflict` (also a full
rollback). This closes M1-07's import index maintenance: imported words are
searchable at once, accent-folded (`unicode61 remove_diacritics 2`).

Cancel works in every phase: while reading, the worker stops before its next
file and its late results are dropped; while checking or reviewing, the
staged songs are discarded; while committing, a commit that has not reached
the storage worker is dropped, and one that has is cancelled through its
`storage::Cancel` flag, which `commit_import` checks before every song and
before the commit, so it rolls back. If the commit had already finished,
the footer says so ("finished before it could be cancelled"). Source files
are only read, never written (tested by content and modification time).

While the preview is reading, checking, in review or committing, Quit
(Ctrl+Q, which still reaches the main window behind a dialog) and the
window's close request are refused with "Finish or cancel the import before
quitting." in the dialog (on its own line, so an import error such as a
failed duplicate check stays visible), so an Unsaved prompt can never
replace the preview and strand the import (UNOBSERVED, EW8-OBS-IMP-7). This
also holds for a quit that resumes later, after Don't Save while a settings
write was pending or after a Save, when the preview was opened in the
meantime: the quit is dropped and the operator quits again once the import
is finished or cancelled. Any other
dialog that replaces the preview cancels the import first. A chooser that
returns while another dialog is open drops its choice with "Close the open
dialog, then import again."

After a commit the dialog closes, the Songs footer reports
"Imported: n · Skipped: n · Not importable: n", the Songs list reloads, the
imported songs become the Songs selection, Preview loads the first one and
keyboard focus moves to its row. Live is never touched.

### Tests

- `import::batch` (format choice, unsupported and hostile files, library
  limit, title from the file name, duplicate keys, bounded folder walk, file
  limit), `import::worker` (ordered results with partial failure, cancel
  before the next file, back-pressure, a panicked thread reported as
  `Failed`), `storage` (one transaction with the
  index and Replace, failure injection by trigger, Conflict, cancel, invalid
  batch, duplicate lookup through the worker),
  `operator::song_import::tests` (batch duplicates, choices, every warning
  and error has text), `import::openlyrics` (attributes reported).
- GPUI (`src/tests/song_import.rs`): folder import with preview, commit,
  selection, focus, accented search, a repeated OpenSong label, preview and
  an acknowledged Live slide; keyboard File menu and choice groups with an
  unsupported file and Esc; cancel while reading (worker held by the test
  pace hook); cancel before the commit reaches storage; a failing save
  (SQLite trigger) rolling back the batch, then a retry; duplicate choices
  (bulk, Replace, copy, double Replace refused).
- Native: `scripts/song-import-x11.py` (PASS on Linux/Xvfb) and
  `scripts/song-import-windows.py` (NOT RUN: no Windows host).
- Fixtures: `tests/fixtures/import/batch/` (original synthetic text).

Open: the EasyWorship observations listed in parity-pending; Windows run;
cancel during a long commit is covered by the storage flag test and the
queued-commit GPUI test, not by a native run; per-file read timeouts (a hung
network share would hold the worker, never the UI); `.txt`/EasyWorship
lyrics files and SongSelect formats are not imported; several songs in one
ChordPro file still merge (see ChordPro above).
