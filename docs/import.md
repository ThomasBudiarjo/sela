# Song import parsers

Parsers for untrusted song files live in `src/import/`. Each one turns a
single file's bytes into a `ParsedSong` (sections of slides, a presentation
order, metadata and warnings) or a `Rejected` reason. They never touch the
library, the UI thread or the network. The import framework (M1-14) owns
staging, the staged preview, duplicate policy and the commit transaction, and
maps a `ParsedSong` onto `storage::Song`.

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

Not imported and not reported: the attributes `author/@type`,
`author/@lang`, `title/@lang`, `title/@original`, `verse/@lang`,
`verse/@translit`, `lines/@part`, `lines/@break` and `lines/@repeat`.
This is an open loss: `Warning` has no attribute variant yet, and adding one
is a framework decision (M1-14), because the staged preview names every
warning kind.

Limits: 128 sections, 128 slides, and an order of at most 512 slides after
expansion. Slide text and labels share a budget of 240 KiB, priced like the
arrangement snapshot (24 bytes per slide on top of the text). Anything past a
limit is dropped with one `LimitReached`.

### Storage mapping

`to_song` makes one `storage::Section` per slide, carrying its section's
label, with IDs from `SectionId::allocate`. A non-empty order becomes the
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
