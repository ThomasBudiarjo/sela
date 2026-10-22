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
  without regard to case. An unknown name is `MissingOrderReference` and is
  skipped. A name whose section is empty is skipped quietly, because the
  section already has `EmptySection`. An empty presentation means file order.
- Any other element with text, such as `<key>` or `<hymn_number>`, is
  `UnknownElement` with its name: Sela has no field for it. Empty elements
  are ignored.

Lyric lines are typed by their first character:

| Line starts with | Meaning |
| --- | --- |
| `[` | A section tag up to `]`, such as `[V1]`. An empty tag counts as `[V]`. |
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
  lyric lines under that tag stay in the tag's own section. A tag whose lines
  are all numbered gives no `EmptySection`.
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
  kept, as is a stray `]`. On a line that had a chord, runs of spaces become
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
chord survives. Failures found by these runs are stored as seeds in
`proptest-regressions.txt` in the fixture folders and replay first on every
`cargo test`.

## EasyWorship song words

Not implemented yet.
