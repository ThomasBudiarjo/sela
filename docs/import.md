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

Not implemented yet.

## EasyWorship song words

Not implemented yet.
