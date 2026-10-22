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

Not implemented yet.
