# Security and data safety (M5-02)

This note is Sela's threat model, the list of its unsafe code, and the record
of the fuzzing, dependency and license audits (M5-02, with the notices part of
M5-07). It states what was checked and how, what was fixed, and which risks
are accepted. It is not a qualification: Windows runs, physical hardware and
the installer remain open (see "Open items").

Last audit: 2026-10-22, stream `fuzz-audit`, Linux container, Rust 1.98.1,
cargo-deny 0.20.2, RustSec advisory database fetched that day.

## Assets and attackers

What must stay safe, in order:

1. **The live output.** A hostile or damaged file must never blank, freeze or
   replace what the audience sees, and must never stall the operator UI.
2. **The library and schedules.** No accepted input may corrupt the song
   library, make a committed revision unrecoverable, or plant content that
   changes later saves.
3. **The operator's machine.** No input may run code, read files outside what
   the operator chose, or reach the network.
4. **Licensing.** What Sela ships must be redistributable under
   AGPL-3.0-or-later with correct notices.

Attackers and accidents considered: a song, schedule, image or library file
from another church, a download or a USB stick; a damaged profile after a
crash, disk error or hand edit; a crashed or mismatched audience process; a
malicious or broken program placed where FFmpeg is expected; a dependency
update that adds a vulnerable, unmaintained or incompatibly licensed crate.
Out of scope: an attacker who already runs code as the operator's user, and
physical access to the machine.

Sela has **no network listener and no network client** (plan.md, Mission
2026-10 plan). GPUI carries an `http_client` trait and `url`, but Sela never
installs an HTTP client, so GPUI cannot fetch anything. `deny.toml` bans
`openssl-sys`, `native-tls`, `reqwest` and `hyper`, so a TLS or HTTP stack
cannot arrive silently through an update.

## Entry points

Every place untrusted bytes enter Sela, with its controls and tests.
"Fuzz" names the suite in `src/fuzz/` (see [Fuzzing](#fuzzing)).

| Entry point | Code | Controls | Tests |
| --- | --- | --- | --- |
| OpenLyrics, OpenSong, ChordPro song files | `import::{openlyrics,opensong,chordpro}` | 1 MiB file cap before parsing; UTF-8 only; XML with DTDs refused (`Entities`, no entity is ever resolved, no XXE), a linear depth scan before roxmltree (its parser recurses), node limits; section, slide, order and warning caps; text budget and field limits so every accepted song stores (`ParsedSong::fit_to_model`) | Module tests and proptests; fuzz `songs` (mutated fixtures, token soups, written-song round trips, 32 worst-case 1 MiB inputs with a time bound) |
| EasyWorship song words (RTF) | `import::ew_rtf` | 1 MiB cap; group depth 64; `\binN` bounded by the input; raw high bytes refused; same caps and text budget | Module tests; fuzz `songs` |
| EasyWorship library databases | not built yet (stream `ew-library-import`) | Required: open read-only on a private copy with the SQLite hardening below; bound row counts and blob sizes; never run its schema | To add with the importer |
| Scripture references | `scripture::parse`, `scripture::complete` | Linear tokenizer, saturating numbers, chapter checks per book | Module tests; fuzz `scripture` (any text, token soup, every spelling round trip, 1 MiB inputs) |
| Scripture modules (OSIS, Zefania, USFX) | not built yet (stream `scripture`) | Required: the song-XML controls (no DTD, depth scan, node and size limits) | Add a `src/fuzz/` target with the importer |
| `.selaschedule` files | `schedule_file::decode` | 8 MiB cap before parsing; serde_json recursion limit (128); version, item (32) and title (1 KiB) caps; every embedded song passes `Song::validate`; strict hex and binary codecs | Module tests; fuzz `schedule` (generated files round trip, mutated files reopen identically, worst cases) |
| Slide format, background and song payload codecs (schedule files and the library) | `format::SlideFormat::decode`, `background::Background::decode`, `storage::Song::decode` | Size caps checked first; strict decoding (no trailing bytes, no unknown tags) | Fuzz `codecs` (anything that decodes re-encodes to the same bytes) |
| Library database and backups | `storage` | Defensive SQLite and an untrusted schema on every connection; backup and restore refuse foreign triggers and views; verified copy before publication; 256 MiB budget; blob length limit | `storage` tests incl. `restore_refuses_a_library_with_triggers_or_views` |
| `settings.txt` | `settings::{load,parse,peek_layout}` | 1 MiB read cap; a damaged file gives defaults and is renamed aside; LF, CRLF and bare CR end lines; values with control characters are ignored on load and left out on save | Module tests; fuzz `settings` (saved settings load back unchanged, mutated files, worst cases) |
| Images (PNG, JPEG) | `images::inspect`, `scene::decode_image`, `images::downscale` | 8 MiB file cap (48 MiB for photos that are shrunk); header-only preflight before any pixel buffer: 16,384 px edges, 64 MiB decoded budget, decoder `total_bytes` check for 16-bit PNG, APNG refused; decoder allocation limits; Exif orientation; ICC ignored | Module tests; fuzz `images` (mutated PNG/JPEG with CRCs repaired, declared-dimension bombs) |
| Fonts | `fonts` (installed fonts), `ttf-parser` | 8 MiB per file, 64 faces per collection, 8,192 faces, 32 MiB cache; scanning on workers only | Module tests; ttf-parser is fuzzed upstream (see accepted advisories) |
| FFmpeg subprocess | `media::ffmpeg` | Runs `ffmpeg`/`ffprobe` as child processes, never linked; absolute paths only; refuses a GPL or nonfree build; 5 s probe timeout with kill; input must be an absolute local path, URL schemes, UNC shares and device paths refused; `-protocol_whitelist file`; `-nostdin` | Module tests; fuzz `media` (input paths, build output) |
| Audience process pipe | `transport::Frame` | Local anonymous pipes to a child Sela process, no listener; fixed header, version and kind checks; resource bodies capped at 64 MiB + 160 B and checked before allocation; strict payload decoding; renderer capabilities re-checked | Module and process tests; fuzz `transport` (mutated frames, arbitrary streams) |
| Archives and portable bundles | not built yet (M1-13, stream `bundle-backup-restore`) | Required: refuse absolute paths, `..`, drive and device names and links on extraction (no zip-slip); cap entry count, entry size, total size and compression ratio; versioned manifest; never execute or open embedded files directly | Add a fuzz target with the bundle code |
| Documents (PowerPoint through installed PowerPoint) | not built yet (M4) | Required: automate PowerPoint in its own process with a timeout, macros disabled, local files only; PDF and web pages are out of scope | With the feature |
| Browser surfaces and web content | none | Sela embeds no web view and renders no HTML; web pages are out of scope (plan.md). A future web surface must be sandboxed (plan.md, later scope) | n/a |
| Provider credentials and secrets | none | Sela stores no account, token or key today (offline, no providers). A scripture or licensing provider must keep credentials in the OS store, never in the profile, settings or support exports | With the provider |
| Diagnostics and support export | not built yet (M5-05) | Required: opt-in, preview, redaction of lyrics, notes, user paths and secrets; no telemetry | With M5-05 |
| Install and update | not built yet (M5-07) | Unsigned per-user Inno Setup installer built locally; no auto-update and no network at start; FFmpeg found by absolute path under the install folder (no PATH or DLL search order lookup); notices and source pointer shipped (below) | S08 with M5-07 |

## Unsafe code

`#![deny(unsafe_code)]` is set on the library and the binary crate, so new
unsafe code fails to compile without an explicit `#[allow(unsafe_code)]` and
an entry here.

| Location | What | Why it is sound |
| --- | --- | --- |
| `src/timer.rs` (`raise_resolution`, Windows only) | One call to `timeBeginPeriod(1)` from `winmm` | Takes a plain integer, no pointers or memory preconditions; the 1 ms timer request lasts until the process exits. Needed so Black/Clear/Logo receipts are not delayed by the 15.6 ms default tick. |

Dependencies contain unsafe code (GPUI, wgpu, the windows crates, image
codecs, SQLite through rusqlite, cosmic-text). They are not audited line by
line here; they are pinned exactly (Cargo.lock, `=` versions, git `rev`s),
checked against RustSec on every audit, and their input-facing parts are
exercised through Sela's own fuzz suites.

## Fuzzing

`src/fuzz/` (a `#[cfg(test)]` module of the library, so it runs with
`cargo test`) holds seeded property and fuzz suites for every parser and
decoder of untrusted input listed above:

| Suite | Targets | Invariants |
| --- | --- | --- |
| `songs` | the four song parsers | same result twice; size, section, slide, order and warning caps; no empty slide or CR; an empty title is always reported; every accepted song converts and passes `Song::validate`; generated OpenLyrics, OpenSong and ChordPro files read back exactly; 1 MiB worst cases within 20 s (debug build); oversized files refused within 0.5 s |
| `scripture` | reference parser, completion | valid reference or typed error; accepted references re-parse from their canonical spelling; every name and abbreviation in both languages reads back; completion bounded and unique |
| `schedule` | `.selaschedule` decoding | opened files hold only valid songs and known arrangements, save again and reopen identically; generated files round trip; refusals are typed |
| `codecs` | slide format, background, song payload | whatever decodes is valid and re-encodes to the same bytes; valid values round trip |
| `settings` | `settings.txt` | load, save and load gives the same settings; no duplicates; at most 10 recent files; names with control characters never saved as another name |
| `images` | header preflight and bounded decode | declared dimensions bounded before any pixel buffer; decoded size matches the buffer and stays in budget; typed errors only |
| `media` | FFmpeg input arguments and build output | only absolute local paths without a URL scheme reach `-i`, after the protocol whitelist; GPL and nonfree builds refused |
| `transport` | audience pipe frames | every byte stream gives frames or typed errors; accepted commands stay within renderer capabilities |

How they run:

- Each suite has a fixed RNG seed (so a run is reproducible) and 128-512
  cases by default; `PROPTEST_RNG_SEED` and `PROPTEST_CASES` change them.
- A failure is minimized by proptest and saved in
  `tests/fixtures/fuzz/<suite>.proptest-regressions`, which replays first on
  every run. Those files are committed.
- `tests/fixtures/fuzz/<target>/` is the retained corpus: hand-written
  adversarial files and minimized cases of real findings. Each suite replays
  it, and the song, schedule, settings and image suites also mutate it (the
  song suite with the importer fixtures too) as seeds. Add a file there for
  every new finding.
- A longer campaign: `scripts/audit.sh --fuzz 20000`, or
  `PROPTEST_CASES=20000 PROPTEST_RNG_SEED=<n> cargo test --lib fuzz::`.

Campaign on 2026-10-22: the default run, 8,192 cases with the fixed seeds,
and 20,000 cases each with seeds 11, 22 and 33 all pass after the fixes below
(about 30 s per 20,000-case run on the 4-CPU container).

**cargo-fuzz** (libFuzzer, coverage guided) needs a nightly toolchain, which
this project does not use. It is a later option: the suite functions take
`&[u8]`, so each can become a `fuzz_target!` without changes, seeded from
`tests/fixtures/fuzz/`. Until then the suites above are the fuzzing.

## Findings and fixes

Each fix has a regression test that fails without it (checked by reverting
the fix, 2026-10-22).

| ID | Finding | Fix | Regression tests |
| --- | --- | --- | --- |
| SEC-01 | OpenSong, ChordPro and EasyWorship words had no text budget: a large file (up to 1 MiB of lyrics) parsed fine but could never be stored, so the import would fail after the operator reviewed it. | Shared `ParsedSong::fit_to_model` (240 KiB budget, `LimitReached`, `TooLarge` for one oversized slide) | `import::tests::large_files_are_cut_to_the_text_budget_and_stay_storable`; fuzz `songs` worst cases |
| SEC-02 | OpenSong and ChordPro kept titles, authors, copyrights and section labels of any length, which `Song::validate` refuses. | Same function cuts them at a grapheme boundary with `Truncated(field)` | `import::tests::long_metadata_is_cut_at_a_grapheme_boundary_and_reported` |
| SEC-03 | An OpenSong or ChordPro order could expand to more than 512 slides, which no arrangement can hold. | Same function drops order entries past the cap with `LimitReached` | `import::tests::the_order_is_cut_at_the_arrangement_slide_limit` |
| SEC-04 | With 64 other warnings, OpenSong and ChordPro dropped `MissingTitle`, so the file name was never substituted and the song could not be stored. | `ParsedSong::warn_missing_title` puts it first in every parser | `import::tests::missing_title_survives_a_full_warning_list`; corpus `full-warning-list-no-title.*` |
| SEC-05 | `settings.txt` with a bare CR (old line ends, an editor's stray CR) put the CR into a recent path, which then never opened; saving silently rewrote paths and monitor names that contained control characters into other names. | Bare CR ends a line; such values are ignored on load and left out on save | `settings::tests::a_bare_cr_ends_a_line_and_never_joins_a_path`, `values_with_control_characters_are_left_out_not_rewritten`; fuzz `settings` |
| SEC-06 | `settings.txt` was read whole, so a damaged multi-gigabyte file stalled or exhausted startup (`peek_layout` runs before the window opens). | 1 MiB read cap; larger files are unreadable and renamed aside | `settings::tests::an_oversized_file_is_unreadable_and_kept_aside` |
| SEC-07 | A restored backup is an untrusted SQLite file; one with a trigger or view was accepted, and its trigger would run on every later save. Connections ran with SQLite's default trusted schema. | Defensive mode and untrusted schema on every connection; backup and restore refuse triggers and views | `storage::tests::restore_refuses_a_library_with_triggers_or_views` |

No panic, hang, stack overflow or memory-safety issue was found in any
parser; every finding above is a data-safety defect (a file accepted but not
storable, or a setting changed on save).

## Dependency audit

`deny.toml` holds the policy and `scripts/audit.sh` runs it with a pinned
cargo-deny (0.20.2, release binary checked against its SHA-256) for the
Windows, Linux and macOS targets. The audit fails on any advisory not listed
with a reason, a yanked crate, a license outside the list, a banned crate, an
unknown registry or git source, an unpinned direct dependency (every registry
dependency must be an exact `=` version and every git dependency a `rev`),
or out-of-date notices.

Result on 2026-10-22: `advisories ok, bans ok, licenses ok, sources ok`,
pin check and notices check pass (`audit: PASS`).

- **Licenses.** Allowed: 0BSD, AGPL-3.0-or-later (Sela), Apache-2.0
  (with and without the LLVM exception), BSD-2-Clause, BSD-3-Clause, BSL-1.0,
  CC0-1.0, ISC, MIT, MIT-0, MPL-2.0, Unicode-3.0, Unlicense, Zlib, plus
  `bzip2-1.0.6` for `libbz2-rs-sys` only. All are compatible with
  distributing the whole under AGPL-3.0-or-later. GPL-2.0-only is not
  allowed; `self_cell` offers it only as an alternative to Apache-2.0, which
  applies. MPL-2.0 crates (`cbindgen` at build time, `dwrote`, `option-ext`)
  stay under MPL for their own files.
- **Advisories.** No vulnerability. Six "unmaintained" notices are accepted
  in `deny.toml`, each with a reason: `paste` (RUSTSEC-2024-0436, build-time
  proc-macro), `rustybuzz` (RUSTSEC-2026-0206, SVG text in GPUI's usvg),
  `ttf-parser` (RUSTSEC-2026-0192, required by GPUI and cosmic-text, also used
  directly by `fonts`), and the zbus split crates `zvariant`, `zbus_names`,
  `zvariant_derive` (RUSTSEC-2026-0370/0371/0372, Linux desktop integration
  only). None has a safe upgrade inside the pinned GPUI revision. Revisit them
  on every GPUI bump; migrating Sela's own font checks to `skrifa` only helps
  once GPUI and cosmic-text move.
- **Duplicates.** 41 crates appear in two to four versions (warnings, for
  example `hashbrown`, `windows-sys`, `syn`), all through GPUI, wgpu and image.
  Accepted; they cost build time, not safety.
- **Sources.** crates.io, the `zed-industries` GitHub organization (GPUI and
  its forks of font-kit, scap and xim-rs) and proptest's pinned git revision.

## Third-party notices

`THIRD-PARTY-NOTICES` (repository root) is generated by
`scripts/third_party_notices.py` from Cargo.lock for `x86_64-pc-windows-msvc`
(the installer's target): 429 crates compiled into the binary (normal
dependencies; build-only and dev-only crates are not distributed), each with
its license expression, source and the license files from its package, copied
verbatim and deduplicated (240 distinct texts). The 21 crates that ship no
license file get the standard SPDX texts (`scripts/notices/spdx/`, SPDX
license list 3.27.0) with their authors named. The file also reproduces the
GPUI/Zed Apache-2.0 notice (`LICENSE-GPUI-APACHE`), the DejaVu fonts license
for the embedded fonts, and the FFmpeg LGPL entry with its source pointer.

The output depends only on Cargo.lock, the pinned crate sources and this
repository (no dates, sorted), so it is reproducible; `scripts/audit.sh`
fails when it is out of date. Regenerate after any dependency change and
review the diff. The installer (M5-07) must ship it next to `LICENSE`.

Open license item (L): for the bundled FFmpeg, the installer must ship
FFmpeg's own license files and either the corresponding source or a written
offer; the notices give the source pointer only. Decide with M5-07.

## Accepted residual risks

- **Schedule file memory.** An 8 MiB `.selaschedule` of tiny JSON values
  (for example `[0,0,0,...]`) expands to roughly 16-32 times its size in
  serde_json values (up to about 270 MB) before the item cap rejects it. It
  happens on the schedule worker, never on the UI thread, and is bounded.
  Revisit if low-memory machines are declared (M5-01).
- **FFmpeg demuxers.** FFmpeg parses media files with C code. Sela runs it as
  a separate process with local files only, so a crash ends that process and
  the live scene stays; a memory-safety exploit inside FFmpeg would run as the
  operator. A media file's own playlist may still name other local files
  (`-protocol_whitelist file`). Keep FFmpeg updated with the pinned release.
- **SQLite C code.** The bundled SQLite parses the library and restored
  backups. Hardened as above and kept current through rusqlite pins.
- **Unmaintained crates** listed under Dependency audit.
- **Installed fonts** are parsed by ttf-parser (and the system shaper in
  GPUI); a hostile font installed system-wide is outside Sela's control.

## Open items

- Windows: run the fuzz suites (`cargo test --lib fuzz::`, which also
  covers the Windows-only UNC and device path refusals in `media::ffmpeg`)
  and `scripts/audit.sh` under Git Bash (parity-pending PP-FUZZ-01).
- Interruption and disk-full tests across every durable write boundary
  (imports, migrations, backup restore, cache publication): partly covered by
  existing storage process-abort tests; not audited as a whole yet.
- Secret storage and export permissions: Sela stores no secrets today;
  revisit with provider credentials (scripture providers, M4).
- Fuzz targets for the EasyWorship library importer, scripture modules and
  bundles when those land (see Entry points).
- Installer: ship `THIRD-PARTY-NOTICES`, decide the FFmpeg source offer,
  S08 install/upgrade checks (M5-07, PP-FUZZ-02).
