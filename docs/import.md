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
  `storage::Section`, so a two-slide verse yields two sections with the same
  label.
- Section codes shared by OpenLyrics and OpenSong map to Sela labels with
  `label_for_code` (`v1` is "Verse 1", `c` is "Chorus").
- Anything a parser drops or guesses is reported as a `Warning`.

## OpenLyrics

Not implemented yet.

## OpenSong and ChordPro

Not implemented yet.

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
