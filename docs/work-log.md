# Work log and resume point

Use [backlog.md](backlog.md) for ticket checklists and test boundaries. This file
records execution state, not a second copy of the backlog. Dates use ISO format;
include timezone for timed hardware/rehearsal evidence.

## Current state

- **Video backgrounds and Live playback (video-playback, M2-02 video backgrounds, M2-03, M2-05 basic, 2026-10-23):** a Media Fill can be a profile video per slide or song master (`Fill::Video`, Repeating; Select Media… › Videos, keyboard Left/Right between Videos and Images; EW8-OBS-032/039); videos are imported (probe-checked copy) and listed with first-frame posters in Media › Videos; every video cue re-checks the video's pin off the UI thread, also after an earlier successful play (missing/changed/no FFmpeg → black plus the warning, the image rule; review round 1), the renderer decodes it itself, primes its first frame before the cue can be accepted (failure keeps the live scene) and plays it at frame timestamps under the lyrics, looping, shared across slides of the same video, running on under Black/Clear/Logo (best guess); the Live pane transport row (Play/Pause, Restart, renderer-reported "Playing · 0:03 / 0:08") sends wire kind 8 on the Safety lane and shows wire kind 9 reports; `.selaschedule` version 6 only for schedules holding a video. Unit, real-GPU, GPUI tests and `scripts/video-x11.py` PASS on `:123`; `scripts/video-windows.py` NOT RUN (no Windows host). A video's audio (M2-04 feasible part) plays in the renderer from a second FFmpeg decode into a bounded lock-free ring, held within 40 ms of the video clock, following the transport, never muted by masks (best guess); Windows opens the default device through `cpal` 0.18.2, other platforms a null output; the transport row adds "Sound on" / "No sound output" / "Sound failed". **Implemented-unqualified** (docs/media.md "Video backgrounds and playback", "Audio of video backgrounds"; EW8-OBS-VP-1..13 UNOBSERVED; PP-VP-01..13; no speakers or A/V measurement). Review round 1: per-cue pin checks and slide posters that follow them, a cancellable one-at-a-time Select Media… import that a closed editor cancels, Media › Videos rescans when opened, Videos-list posters follow the scroll position, unbound transport actions removed, upstream GPUI references in media.md. Review round 2: a stale pin check asked for again right after its reply no longer stalls the deferred cue (taking a reply ends the in-flight mark), `video_background` uses `Checked::fresh_for`, a test closes the editor window during an import, PP-VP-08 says which cases no native script covers. Next: PP-VP-12/13 on Windows, then the PP-VP observations.
- **EasyWorship library import (ew-library-import, M4-07 EasyWorship 6-8 child, 2026-10-23):** File › Import ▸ EasyWorship Library… (UNOBSERVED placement, EW8-OBS-EWL-1) takes one folder (the EW profile or any folder down to `Databases\Data`), copies `Songs.db`/`SongWords.db` (plus `-wal`/`-journal`) into a private temporary folder on the `sela-import` thread, refuses a source that changes during the copy, opens only the copies (`query_only`, `trusted_schema` off, 2 MiB values, `UTF8_U_CI` registered), maps title/author/copyright/administrator/CCLI (`vendor_id`), parses the newest `word` row with `ew_rtf`, and stages one row per song in title order into the framework's preview and one-transaction commit: a song without words is a failed row, a repeated `song_uid` is a duplicate inside the import, a re-import finds every song as a library duplicate (Skip); a library over 500 songs imports 500 per turn, songs not yet in Sela's library first, and word rows are read in one pass without assuming an index (review round 2). Every schema detail is an UNOBSERVED best guess (docs/import.md "EasyWorship library", PP-EWL-01..08). Unit, worker and GPUI tests (synthetic databases built in test code, sources hashed before/after) and `scripts/ew-import-x11.py` PASS; `scripts/ew-import-windows.py` NOT RUN (no Windows host). Next: PP-EWL observations on a copy of the owner's EW 8.0.49 library, then the Windows script.
- **Portable bundles, backup and restore (bundle-backup-restore, M1-13, S04, 2026-10-23):** File › Export Standalone Schedule… (the EW8-OBS-053 row, inert until now) writes a `.selapackage`: a strict ZIP subset with a version-1 manifest (every entry's size, SHA-256 and role), the schedule snapshots and the profile images byte for byte; fonts are listed, never copied; restricted scripture is left out and listed. File › Open Schedule opens one into the profile after validating the whole bundle (images never replace another image, case-insensitive; the schedule becomes a new file in Schedules) and the preflight report shows the package notes and Locate…. Profiles › Back Up Profile… / Restore Profile… (UNOBSERVED placement) write a `.selabackup` (verified library copy, settings with profile-relative recents, logo, images, schedules, imported translations; no lock, autosave or bundled translations; links never followed) and restore it only as a new folder through private staging and one rename. Traversal, absolute and device names, links, case conflicts, ZIP64/encryption, overlapping entries, bombs, checksum and manifest mismatches are refused before any change. U/I/fuzz (`portable`), GPUI (`src/tests/bundle_transfer.rs`), S04 (`tests/bundle_s04.rs`) and `scripts/bundle-x11.py` PASS; `scripts/bundle-windows.py` NOT RUN (no Windows host). M1-13 **implemented-unqualified**; EW8-OBS-BUN-1..7 UNOBSERVED (PP-BUN-01..08). Review round 1: a pipe given a bundle name is refused at once, a panicking job reports failure and frees the worker, an import finishing after Cancel is kept but not opened, exports are readable as the umask allows. Review round 2: a job panicking mid-write is tested to leave no temporary file or staging folder; library and file system failures get one localized reason each (no English `io::ErrorKind` text). Next: the Windows run and EW observations; profile switching (M1-12b) to open a restored profile from the menu; operator export options.
- **Song import framework (import-framework, M1-14 + M4-07 OpenSong/ChordPro, 2026-10-22):** File › Import ▸ Songs… / Song Folder… (UNOBSERVED placement, EW8-OBS-IMP-1) reads OpenLyrics, OpenSong and ChordPro files on the bounded `sela-import` thread, shows a staged preview with every warning (new `Warning::IgnoredAttribute` for `lines/@repeat`, `verse/@lang`, ...) and per-file error, checks library and in-batch duplicates (Skip / Import as copy / Replace, best guess), and commits all accepted songs in one transaction through the `save_song` path so the search index follows (closes M1-07's import maintenance). Cancel commits nothing in any phase. No schema change. GPUI tests (`src/tests/song_import.rs`) and `scripts/song-import-x11.py` PASS; `scripts/song-import-windows.py` NOT RUN (no Windows host). M1-14 **implemented-unqualified**; EW observations PP-IMPF-01..09 open; review rounds 1 and 2 fixed (quit/close, including a quit resumed after Don't Save or Save, no longer strands an open import; the quit note no longer hides import errors). Next: run the Windows script and the EW observations.
- **Sunday-core scrutiny round 2 and scenarios S01/S02 (scrutiny-r2, 2026-10-22):** seven confirmed defects fixed with regression tests that fail before their fix: show keys fired under the open Songs row menu (SC2-FOCUS-01), a failed Go Live of another item borrowed the old slide index for the footer, Next/Previous and a surface refresh (SC2-LIVE-01), Go Live on a duplicate entry let the original's pending cue reach the audience (SC2-LIVE-02), a library pick left a Live Page Down target behind (SC2-LIVE-03), an English-only window title (SC2-I18N-01), dialogs with no accessible role or name (SC2-A11Y-01) and a window title that reached the accessibility tree only on the next unrelated redraw (SC2-A11Y-02); plus the harness reading every AT-SPI role as "" (SC2-HARNESS-01). New `scripts/scenario-s0{1,2}-x11.py` PASS on Xvfb with the real audience; Windows counterparts NOT RUN. `.agents/setup` works on rustup 1.28 and adds mingw-w64, the Windows target, the E2E packages and ruff. Tickets stay **implemented-unqualified**/active; M1-15 is active. Review round 1 fixed three more (the editor close prompt asked "Close without saving?" although Yes saves, SC2-I18N-02; the Schedule item, image and New ▾ menus left show keys live, SC2-FOCUS-02; Normalize skipped a Go Live in preparation, SC2-LIVE-04) and made the harness window checks race-free; review round 2 fixed two minors (a stale New ▾ swallow-click ate the next Enter, SC2-FOCUS-03; Normalize sent a second cue instead of rebuilding the one in preparation and dropped a double-click unmask, SC2-LIVE-05). Next: the Windows runs (parity-pending "Stream: scrutiny-r2"), then the remaining unconfirmed suspicions in the scrutiny-r2 record.
- **Autosave, recovery and the profile lock (autosave-recovery, M1-12, 2026-10-22):** schedule revisions are committed to `<profile>/Recovery/` off the UI thread (checksummed files, temp + fsync + rename; 2 s quiet / 10 s maximum cadence; 20 revisions / 4 MiB retention), the first edit after a commit records an uncommitted marker, every close flushes and marks the run clean, and after an unclean exit a modal prompt restores the last committed revision (Restore default) while disclosing lost newer edits, damaged and interrupted autosaves. A second operator on the same profile shows "Sela is already running" and touches nothing (OS file lock). S03 passes with SIGKILL child processes and natively in `scripts/autosave-x11.py`; `autosave-windows.py` is NOT RUN (no Windows host). **Implemented-unqualified** (docs/autosave.md; EW8-OBS-071..077 UNOBSERVED; PP-AUTO-01..10). Review round 1 fixed: a close before the start-up scan's reply (or with the prompt unanswered) now records `offered` instead of acknowledging the offer unseen; an unwritable session after a crash still offers a read-only restore; markers only for unsaved work, withdrawn on undo/save and set at once during a running commit; Restore runs the missing-file check; local profile management split out to M1-12b (planned). Review round 2 fixed: a crash between a Save and the next commit now says the revision's file was saved again later (file mtime) instead of "never saved"; a close during a start-up open that then fails closes at once without flashing the prompt. Next: Windows run and EW crash/second-instance/Profiles observation.
- **Songs browser remainder (songs-browser, 2026-10-22):** Sort by Author, Copyright, Administrator and Song Number (EW8-OBS-064; empty keys last, Song Number numeric, persisted like Title/Date modified), slide-label search through a schema 7 `labels` FTS column (EW8-OBS-045/046; migrated by an index rebuild behind the verified `.schema6-backup`; label-only matches rank after every song with a title, lyric or metadata hit; review fix 2026-10-23), and Options › Main Output › Song › Copyright Info (Display Copyright Information, Show on first displayed slide only, License Info; EW8-OBS-055/056) applied to the next Live cue. New GPUI tests (`src/tests/songs_browser.rs`) also cover a late reply for an earlier query, deleting the selected search hit and arrow keys through a sorted virtual list. `scripts/songs-browser-x11.py` PASSES on Xvfb; `scripts/songs-browser-windows.py` is written but NOT RUN (no Windows host). M1-04, M1-07 and the M1-05 copyright follow-up are **implemented-unqualified**; 10 EW/Windows items under parity-pending "Stream: songs-browser". Next: PP-SB-01..10, Media/other resource tabs (M1-04), M1-05t Song tab fonts.
- **Themes (M1-08, themes stream, 2026-10-22):** songs and schedule items render with a theme (`sela::theme`: text format with margins, background, copyright element), resolved field by field (slide → song master → theme → built-in; item snapshot → song theme → library default → built-in). Schema 8 (themes, song themes, default; developed as schema 7, renumbered at integration 2026-10-23 because schema 7 is the songs-browser index reshape) behind a verified `.schema7-backup`; `.selaschedule` v4 stores each item's theme snapshot; margins reach the audience as cue insets. The operator Themes tab lists themes with rendered thumbnails and font/image/contrast/aspect warnings, sets the default and applies a theme to songs or schedule items without touching Live; the theme editor is the Song Editor in theme mode (Format pane reuse, undo, Reset Look, Cancel). **Implemented-unqualified** (docs/themes.md); review round 1 (2026-10-23) fixed substitutes frozen into schedule items, non-atomic Apply to Song, the native live-edit check, thumbnail release, editor re-insert and margin accessibility; review round 2 (2026-10-23) releases thumbnails from the operator window's atlas, lets Apply to Song reach a previewed schedule item, names a theme deleted elsewhere, retries a lock-blocked theme load and keeps editor edits for a theme deleted meanwhile; 16 GPUI tests and `scripts/themes-x11.py` pass, `scripts/themes-windows.py` not run (no Windows host), EW Themes tab unobserved (PP-TH-01..10), follow-ups M1-08a/M1-08b planned. Next: Windows run and EW observation.
- **FFmpeg base and decode pipeline (media-core, 2026-10-22):** `scripts/fetch-ffmpeg` (+ `.ps1`) fetch the pinned BtbN LGPL 8.1 shared build (linux64/win64, size and SHA-256 checked) into the gitignored dev folder; the operator locates FFmpeg once on its own thread at startup, logs the accepted version or each refusal, and shows a non-blocking amber "Video and audio unavailable" status for a missing, GPL (Ubuntu's `/usr/bin`) or nonfree build while Live output and the safety controls keep working; `media::decode::Decoder` (framework-free) probes with ffprobe and decodes to raw RGBA frames with per-frame timestamps over a pipe into a bounded queue (backpressure, cancel, end, stall watchdog, crash/corrupt/unsupported classification; process group on Unix, Job Object on Windows), and VAL-MEDIA-038 is checked at runtime. 25 integration tests in `tests/media_decode.rs`, 7 GPUI tests, `media-backend-x11.py` PASS on `:115` (review round 1: frame budget enforced, stderr-join hang fixed, unique default content ids, `first_frame` → `TimedOut`, Indonesian narrow-header layout; round 2: hover hints with the full warning/output text, `fetch-ffmpeg.ps1` usage exit 2 run under pwsh 7.4.6 on Linux, Linux children die with a SIGKILLed Sela). M2-01/M2-02 **implemented-unqualified** (Windows runs PP-MC-03 not run, no video shown yet). Next: the video-playback stream consumes `Decoder`/`Operator::media_tools()`.
- **Scriptures (scripture stream, 2026-10-22):** M4-03/M4-04 implemented-unqualified. Bundled public-domain KJV and WEB (GitHub seven1m/open-bibles at a pinned commit, SHA-256s in docs/scripture.md) install off the UI thread into per-translation SQLite files with integrity checks and FTS keyword search; OSIS/USFX/Zefania import; the Scriptures tab has a translation picker, reference box with EN/ID errors and completion, keyword search from two characters, consecutive/nonconsecutive selection, readable pagination with per-slide reference footers, Go Live through the cue path and pinned schedule snapshots (`.selaschedule` version 5; developed as version 4, renumbered at integration 2026-10-23 because version 4 is the themes item snapshot). GPUI `tests::scripture` (15) and `scripture-x11.py` (xvfb and `--kind dual`: Import Bible… through the portal dialog, the 1080p reference strip, and since review round 2 `.selaschedule` v5 (then v4) save/reopen through the portal dialogs) PASS on Linux after review rounds 1-2 (a catalog refresh that replaced Preview; typed text lost on a switch or after the first usable import; double Import dialogs; translation rows now say "(damaged)"/"(restricted)" in words); `scripture-windows.py` and EW observations (PP-BIB-01..10) open.
- **Stage (Foldback) output (stage-display, M3-01/M3-02 + stage clock, 2026-10-22):** a second renderer process `sela --stage` with its own supervisor, epoch and receipts; stage panels (current slide, deterministic next slide/next item/end, header label, private stage message) travel only as wire kind 7 STAGE, which the audience renderer refuses (and the stage refuses slides/logos); the stage process draws its own local "HH:MM" clock. Options › Foldback (enable, Automatic or named monitor never defaulting onto the audience's, clock, next), Live › Show Stage Output and Live › Stage Message…; either output can fail without touching the other. Fourteen GPUI tests (after review round 1: the stage follows only what was handed to the audience, one Show Stage Output restarts a lost stage, the stage is drawn in the live slide's face with stand-ins instead of refusing a panel, fake-clock tests of the clock worker; round 2: clock, labels and message stay in bundled DejaVu Sans unless only the song face draws them, a failed or gone clock worker ends the stage process, the UI-thread stage encode is measured against an 8 ms budget), U tests, `stage-display-x11.py` PASS on Xvfb `:118` and on a three-monitor Xorg dummy `:118` (production placement). **Implemented-unqualified** (`docs/stage-display.md`; Windows script and EW observations open, PP-STG-01..09). Next: run `stage-display-windows.py` and the EW Foldback observations.
- **Data safety, fuzzing and dependency audit (fuzz-audit, M5-02 slice + M5-07 notices, 2026-10-22; review rounds 1-2 2026-10-23):** seeded property/fuzz suites in `src/fuzz/` for every parser and decoder of untrusted input, brought up to date with the integration branch (merged at 2523643): four song importers and File › Import staging, OSIS/USFX/Zefania modules, bundles and installed translation files, scripture references, `.selaschedule` v1-v5 (themes, scripture items), slide/background/theme/song codecs, autosave revisions and session, `settings.txt` (copyright and stage keys), images, fonts, FFmpeg input paths, ffprobe reports and showinfo lines, the audience and stage pipes; retained corpus in `tests/fixtures/fuzz/`; SEC-01..13 fixed with regression tests (round 1: empty title after a grapheme cut, Duration overflow panics in probe/decode, untrimmed cut verses, unhardened scripture SQLite with triggers/views, unbounded settings read in the already-running notice; plus the two unhardened storage connections); unsafe inventory U-01..U-04 under `deny(unsafe_code)`; `deny.toml` + `scripts/audit.sh` PASS; reproducible `THIRD-PARTY-NOTICES` (exceptions and every `AND` term now required); round 2: copy call sites, the SEC-11 worst case and session files checked, `unsafe_code` denied for every target. M5-02 and M5-07 **active** (slices implemented-unqualified). Next: Windows run (PP-FUZZ-01), state-machine fuzzing, durable-write interruption audit.
- **Shared Linux E2E harness and parity checklist (e2e-harness, 2026-10-22):** `scripts/native_x11.py` drives the real operator and audience on a private Xvfb (`xvfb` kind, audience in its `window` placement beside the operator) or a private two-monitor Xorg dummy display (`dual` kind, real `secondary` placement), with private profiles and D-Bus sessions, AT-SPI control ids, the GTK portal file dialogs and tagged process cleanup. `live-output-x11.py`, `schedule-x11.py` and `song-editor-x11.py` (ports of the Windows scripts) PASS on `:100`; GPUI now needs lavapipe Vulkan (`VK_DRIVER_FILES=/dev/null` breaks the operator). Linux finding: on the dual display the audience window takes the X input focus (backlog M1-10). `docs/e2e-linux.md` holds setup, API and conventions for every `<feature>-x11.py`; `docs/parity-pending.md` is the single checklist of EasyWorship observations and Windows runs the owner still has to do, with one section per upcoming stream. M0-03 stays **implemented-unqualified**. Baseline fix f05e27a (Linux-only E0282 in a `src/audience/text.rs` test fixture) restored the Linux test build before this slice. Next: the feature streams (scrutiny-r2 first) use the harness and append to parity-pending.md.
- **Parallel wave 2 merged (2026-10-22):** the three remaining sunday-core scrutiny round 1 fixes (arrangement rendering, Songs focus and editor-prompt modality, window layout startup) plus EasyWorship-independent cores pulled forward from later features: OpenLyrics, OpenSong, ChordPro and EasyWorship song-words parsers onto one `import::ParsedSong` shape with `import::song::to_song`, the EN/ID scripture reference parser, and the FFmpeg locator with its LGPL build check. No importer, Scriptures or media UI exists yet. All native re-runs for the round 1 fixes pass, after a mixed-DPI fix found by the new spare-monitor layout run (the operator window reopened 20% smaller on a 100% monitor beside a 125% primary). The fixes stay **implemented-unqualified** (physical projector, Narrator and older hardware remain); the cores are partial slices of M1-14, M4-07, M4-04 and M2-01 (see the wave-2 records at the end). Next: scrutiny round 2 of sunday-core.
- **Song metadata fields and the song editor close modal (song-metadata-fields, 2026-10-18):** songs carry Administrator and Song Number (schema 6, `song_metadata` side table) and a stored `modified` stamp that Sort by Date modified now uses (it survives an index rebuild). The Song Editor Inspector shows Authors, Copyright, License, Administrator and Song number (CCLI); a non-numeric or over-long value is refused with a visible message and saves nothing. A song with copyright text draws a two-line strip ("Title - Author" over the copyright) at the bottom of every slide on the audience (EW8-OBS-063); the Songs search matches author, copyright, license, administrator and song number. The editor's Yes/No/Cancel close prompt is now a focus-trapped modal. **Implemented-unqualified** (docs/storage.md schema 6, docs/search.md). Next: sort by Author/Copyright/Administrator/Song Number, slide-label search, "show on first slide only" option.

- **Songs resource browser and modal focus scopes (M1-03/M1-04 remainder, 2026-10-18):** the Songs list is virtualized (`uniform_list` plus a drawn scrollbar) for the full library and for search results, with Shift/Ctrl click and keyboard multi-select, Ctrl+End/Home/Page keys, Sort by Title/Date modified (+Ascending/Descending, persisted), Delete with Yes/No (default No), a ">200 results" hint, and every dialog in one centred modal backdrop with a Tab trap and Esc returning focus. **Implemented-unqualified** (docs/search.md). Next: Author/Copyright/Administrator/Song Number sort, slide-label search, song editor close prompt as a true modal.

- **Arrangements and pagination (M1-06d, 2026-10-19):** the song editor has an
  Arrangement tab (add/remove/reorder references, new/delete/make-active
  arrangements, undoable); each schedule item has its own arrangement (Preview
  pane button; saved in `.selaschedule` version 2, v1 still opens); overflowing
  sections paginate into balanced audience pages with editor/Preview feedback.
  **Implemented-unqualified** (docs/arrangement.md#m1-06d).

- **Songs search box (M1-07b, 2026-10-13):** the Songs tab has a search box
  under the list: two or more characters filter through `Command::Search`
  (one character shows the full list and sends no query), typing is a text
  field so Black/Clear/Logo and Page Down never fire, Esc and the clear button
  restore the list, no results shows "No songs found", and saves re-run the
  query. Stale replies are dropped by `search::Generations`. The box is the UIA
  Edit `song-search`, so `scripts/a11y-windows.py` PASSES VAL-A11Y-001 in full.
  20k-song keystroke-to-paint p95 81.5-83.8 ms. **Implemented-unqualified**
  (docs/search.md). Next: slide-label search, Songs list virtualization.
- **Accessibility basics (M1-16-a11y, 2026-10-08):** every critical
  main-window control exposes a stable element id, an AccessKit role and a
  localized (EN/ID) name through UI Automation; Black/Clear/Logo and Live
  output report ToggleState, tabs/rows/tiles report selected, menus report
  Expanded and disabled state. The 31-stop Tab order is documented
  (25 tabs to the first song row, unchanged) and walked natively; Songs-row
  Down/Up moves the song selection. `scripts/uia-names-windows.ps1` dumps
  the UIA tree; `scripts/a11y-windows.py` PASSES the full VAL-A11Y-001…005
  native check. **Implemented-unqualified** (Narrator qualification and the
  pending library search box remain; see operator-shell.md M1-16-a11y).
- **Main menu bar (M1-02d, 2026-10-08):** the operator window has the
  observed EasyWorship menu bar (File, Edit, Live, Profiles, View, Help)
  above the toolbar. Menus open by Alt+mnemonic and mouse (hover switches,
  a dismissing press is swallowed); Esc restores the previous focus; items
  dispatch the same commands as the toolbar/shortcuts and unbuilt items are
  disabled. The toolbar Options button is gone (Options is Edit › Options).
  Nine new GPUI tests, the new `scripts/menu-bar-windows.py` and all six
  baseline native scripts pass. **Implemented-unqualified**
  (VAL-SHELL-011; disabled-item features, mnemonic confirmation and screen
  readers remain). Next: enable items with their features, M1-07 search box.
- **UI language switch (M1-16 language slice, 2026-10-08):** `src/i18n.rs`
  holds every operator, menu, dialog and Song Editor string as a `Msg` enum
  with exhaustive English and Indonesian tables and `{name}` placeholders;
  Options › Main Output › General has a Language radio (English / Bahasa
  Indonesia) that repaints all windows at runtime and persists
  (`language=id` in settings.txt, English is the omitted default). Song
  content, schedule titles and the audience output never go through the
  table. GPUI tests, the new `scripts/i18n-windows.py` and all four baseline
  native scripts pass. **Implemented-unqualified** (screen reader and
  full-UI Indonesian review remain, VAL-L10N-*). Next: remaining Options
  tabs, M1-07 search box, M1-11b preflight UI.
- **Parallel wave 1 merged (2026-10-07):** three worktrees merged into
  local `main` with their slice commits:
  - M1-05i: the operator Songs row menu (New Song…, Edit Song…, Delete with
    confirmation, Refresh; Songs reload after editor saves).
  - M1-07a: the schema 5 FTS5 search index with a 20k database benchmark.
    There is no search UI yet.
  - M1-11a: hardened still-image import (size, dimension and decoded-memory
    caps, Exif orientation, cancellable copy) and the `preflight` API
    (missing or changed images and fonts, hash-checked relink). There is no
    operator UI yet.

  Combined checks and all four Windows native scripts pass (operator menu,
  Song Editor, live output, schedule). All three slices are
  **implemented-unqualified**; M1-07 and M1-11 stay **active**. Next:
  M1-11b preflight UI, the M1-07 search box after a W04 EW search
  observation, or another wave (see the wave-1 integration record).
- **Slide backgrounds edited (M1-05h3, 2026-10-07):** the Format pane has a
  Slide tab with the Background section (Fill ▾, color swatch, Select Media…
  with thumbnails and Import…, Aspect Ratio ▾) and Edit Slide Layouts, a
  Layouts tab that edits the song master. Changes apply to the caret slide
  or to every slide after Ctrl+A, one undo step each. GPUI tests and the
  Windows editor script pass. **M1-05h is implemented-unqualified**; open:
  Linux/macOS, screen reader, EW confirmation of a Master edit reaching
  slides and of the Ctrl+A Text-tab deviation. New tickets M1-05p–s and a
  video-background item in M2-02. Next: M1-05i (operator song menu).
- **Slide backgrounds render (M1-05h2, 2026-10-07):** color and image
  backgrounds reach the audience, the editor preview and thumbnails. Images
  are decoded, hash-checked and fitted (Zoom/Stretch/Maintain) off the UI
  thread and cached in the operator (96 MiB, at least 3); a cue waits with
  "Preparing background…" until its image is ready; a missing or changed
  image shows black with a warning in Live and Preview. GPUI tests, the
  DX12/Vulkan readback check and the Windows live-output script pass.
  M1-05h stays **active**; next: h3, the Slide pane editing UI.
- **Slide background model (M1-05h1, 2026-10-07):** `background::Background`
  (None/Color/Media fill, Maintain/Stretch/Zoom), `Section.background`,
  `Song.master` and schema 4 behind a verified `.schema3-backup`; slides
  carry the resolved background. Nothing renders or edits it yet. M1-05h
  stays **active**; next: h2 rendering and preparation.
- **Slide backgrounds observed (RUN-W06I, 2026-10-07):** EW8-OBS-037–043:
  backgrounds are per slide and fall back to the theme's Master layout
  (Edit Slide Layouts opens a Layouts tab); fills None/Color/Gradient/Media;
  Aspect Maintain/Stretch/Zoom; Ctrl+A applies a fill to all slides.
  M1-05h is **active** with the approved spec (slide → song master → black,
  Zoom default, black bars, black plus a warning for a missing image).
  Next: h1, the background model and schema 4.
- **Whole-song selection (M1-05g3b, 2026-10-07):** Ctrl+A in Words or Slides
  selects every slide; every Format pane change then applies to all slides
  as one undo step; typing, Paste or Backspace over it replaces the song with
  one slide (one undo step); Copy copies the whole text. GPUI tests and the
  Windows native script pass. M1-05g is now **implemented-unqualified**
  (IME, Linux/macOS and EW confirmation of the replace result open).
  Deferred formatting work is ticketed as M1-05j–o. Next: M1-05h per-song
  background, in the owner-approved order.
- **Song Editor Format pane (M1-05g3a, 2026-10-07):** the toolbar Format
  toggle docks the Text › Style pane (font list with samples, size, colors,
  B/I/U, alignment, outline and shadow with sliders and an angle dial).
  Changes apply to the caret slide as one undo step. GPUI tests and the
  Windows native script pass. M1-05g stays **active**; next is g3b, Ctrl+A
  whole-song selection with replace-typing.
- **Ctrl+A formatting observed (RUN-W06H, 2026-10-07):** EW8-OBS-034–036:
  Ctrl+A selects every slide and a format change (Italic, font) applies to
  all of them. M1-05g stays **active**; next is g3a (Format pane), then g3b
  (Ctrl+A whole-song selection) per the approved spec.
- **Styled slides render (M1-05g2, 2026-10-06):** M1-05g stays **active**;
  g1 and g2 are done, g3 is next. Every per-slide `SlideFormat` field stored
  in g1 now renders on one shared path (audience, editor preview, Slides
  thumbnails): installed fonts through the shared off-thread catalog
  (`fonts::shared()`) with bundled DejaVu fallback and an editor warning,
  B/I/U (real or synthesized), color, H/V alignment, auto or fixed size,
  Outer outline and shadow. The operator gates Go Live/Next/Previous on
  background font resolution without replacing the live scene. Windows DX12
  native checks pass. Next: g3, the Song Editor Format pane (Text › Style
  subset), per-slide apply, Ctrl+A whole-song selection with replace-typing,
  undo, GPUI tests and native script.
- **Per-slide formats stored (M1-05g1, 2026-10-06):** slice 1 adds
  `format::SlideFormat` on every `storage::Section` and schema 3
  (`section_formats`) behind a verified `.schema2-backup`.
- **EasyWorship Format pane observed (RUN-W06G, 2026-10-06):** the owner's
  restarted 8.0.49 accepted toolbar clicks, so the Format inspector was recorded
  (EW8-OBS-027–033, superseding unverified EW8-OBS-026): Text › Style (font,
  Auto/fixed size, color, B/I/U, super/subscript, indent, alignment, outline,
  shadow, bullets), Text › Layout (margins, auto sizing with normalize, wrap,
  capitalization), Style, Arrange and the Slide pane (background, for M1-05h).
  Formatting applies **per slide** in EW. Owner chose to stay on EW8 rather
  than switch the reference to EW7. Next: M1-05g after the owner picks the
  formatting scope.
- **Song Editor Slides tab (M1-05f, 2026-10-06):** Slides shows rendered
  thumbnails (the preview raster box-filtered to 320×180 off the UI thread,
  bounded text-keyed cache) with label-kind caption bars or grey italic
  "Slide N", a narrowed pane and a blue selection frame (EW8-OBS-024). Windows
  native check passes. Implemented-unqualified. Next: M1-05g per-song text
  formatting (schema change with verified backup), then M1-05h background.
- **EasyWorship-style Song Editor (M1-05e, 2026-10-06):** RUN-W06E observed the
  8.0.49 Song Editor (EW8-OBS-021–026). The editor now has the Title in the
  toolbar, EW's tool order (unbuilt tools disabled), Words as one list of
  slides with inline label/lyrics cells and label-kind group colors, Ctrl+Enter
  splitting into an unlabeled slide, Backspace joining, `+`, Apply/OK/Cancel,
  and a real audience-raster slide preview rendered off the UI thread. Windows
  native check passes. Implemented-unqualified. Next, in the owner-approved
  order: M1-05f Slides tab thumbnails, M1-05g per-song text formatting, M1-05h
  song background.
- **Basic schedule (M1-09a, 2026-10-06):** Schedule items pin song revisions;
  add by button or drag, remove (confirmed, or Ctrl+Delete), reorder by drag or
  Up/Down, Down/Up select, Ctrl+S/Ctrl+O save and open, unsaved guard on Open,
  New Schedule, Ctrl+Q and window close. The live item keeps its identity
  across reorder/removal and no cue is sent. Library double-click goes to Live.
  Windows native keyboard check passes. Implemented-unqualified. The approved
  2026-10-06 spec is complete; next: choose the next dependency-ready ticket
  (M1-09 remaining items, W04 observation, or M1-04/M1-08).
- **Masks and picture logo (M0-08a + M1-10c, 2026-10-06):** Logo/Black/Clear
  toolbar buttons and Ctrl+L/B/C (Page Down = Go Live) drive the real audience;
  indicators follow renderer acknowledgments; Media → Images imports a picture
  and sets it as the logo. Windows DX12 native checks pass. Both
  implemented-unqualified.
- **Better slide output (M1-06c, 2026-10-06):** first-arrangement order, text
  up to 288px, "Normalize text size across slides" toggle, Ctrl+Enter section
  split with undo. Implemented-unqualified.
- **EasyWorship masks observed (M0-02, 2026-10-06):** RUN-W03 recorded the
  single-song W02/W03 static cases (EW8-OBS-014–020): Black and Logo replace
  each other, Clear stacks, masks survive Go Live/navigation/Live off, Live
  double-click unmasks. Implemented in M0-08a and M1-10c.
- **Operator live output (M1-10b, 2026-10-06):** Live on starts `sela --audience`
  on the secondary monitor; Songs → Preview slide → Go Live shows white text on
  the audience, and Live marks only renderer-acknowledged slides. Windows native
  keyboard check passes. Next: M1-10 safety buttons with M0-08, schedule items
  (M1-09), slide shortcuts after W02/W04 observation, hotplug with the owner.
- **Windows host available (2026-10-06):** Windows 11 Home, i7-14650HX, Intel UHD
  + RTX 4060 Laptop, three displays at mixed scale. M0-01 is **done**: locked
  Windows build/clippy/fmt/tests pass and the native operator window passes the
  Win32 smoke. Windows exposed and fixed a backup durability bug (M1-01).
  M0-04 DX12 audience spike passed on physical displays (stalls, preparation,
  exit/kill, mixed-DPI moves, fullscreen, both GPUs) while the owner was gaming;
  hotplug/device loss/present pacing remain open. M0-07 composition/video
  readback passed on DX12 for Intel and NVIDIA. The idle-machine rerun of the
  audience spike (with the `WGPU_POWER_PREF` adapter matrix) passed. Next:
  hotplug with the owner present. M0-02 is active: the first installed
  observation of unlicensed EasyWorship 8.0.49 recorded the main layout.
- All three Oracle findings fixed: M1-05 pending native-input ownership, M0-07d
  preparation backpressure ordering, and M0-06 disconnected receipt invalidation.
  Regression/native checks passed; separate local fix commits, no push authorized.
- Latest continuation merged and checked: **M1-06b** backed-up arrangement/
  section-ID persistence and lossless editor round trips; **M0-06c/M0-07d** owned
  resource transport and native worker-prepared text/image composition. Individual
  local commits retained. Owner permits Windows/physical qualification at the end;
  available automated/native checks continue before commits. No gate is waived.
- Application: native GPUI technical preview builds and renders on Linux software
  OpenGL, with persistent offline song authoring; no service schedule UI or live
  lyric presentation yet. The Linux preview is reinstalled and native-tested.
  Opt-in winit/wgpu diagnostics cover independent animation, owned text/image
  native cue delivery/receipts and separate explicit-font/image GPU readback.
- Latest parallel wave is merged into local `main`, retaining each subticket:
  **M1-05c** document undo, **M1-01b** verified backup/restore and process-abort
  tests, **M1-06a** immutable arrangement domain, **M0-06b** bounded static-color
  native pipes. **M1-02c/M1-05d** correct toolbar/editor placement from documented
  official references; no Painter. Parent regression fixes and combined checks
  are committed. These slices are implemented-unqualified, not completed parents.
- Next concrete slices: M1-06 arrangement controls and explicit missing-reference
  repair, followed by reference-directed pagination/fitting. Storage schema 2 and
  editor undo already retain explicit IDs and variants; do not regenerate them.
  Renderer can proceed separately with resized-output preparation/retention and
  production preparation-intent/pre-resolved-safety coordination before wiring
  live controls. Native preparation is currently FIFO diagnostic, not a proven
  safety-priority path. No mask-policy guesses or service-ready claims.
- The owner permits UI implementation before physical renderer qualification;
  this changes sequencing, not reference, hardware or service-ready claims.
- M0-07a/b/c/d text/GPU/video/native-resource slices are locally merged. Native
  GPU-ready bindings and receipts are checked on software GL; resize, transitions,
  production performance and physical qualification remain open.
- M0-02 reference ledger, M0-03 native harness and M0-04 audience spike were
  developed in separate worktrees and merged into local `main`, with ticket
  commits and an M0-04 integration/failure-check checkpoint.
- Resume M0-04 on Windows DX12 with two physical displays, DPI/topology changes,
  hotplug and physical capture; run the M0-02 installed-reference runbook alongside.
  No Windows runner is connected. These required checks block gate closure.
- Remaining M1 UI slices may proceed under the approved exception; no M0-10
  physical Windows qualification or service-ready release gate has closed.
- Setup and application changes are local only; no push/publication authorized.
- Parallel opportunity: M0-02 reference observation when a lawful EasyWorship
  8.0.49 installation on Windows is available.
- Known qualification needs: real Windows GPU/displays; EasyWorship reference;
  later physical audio/capture/controllers/mobile devices and permitted provider
  accounts. None is assumed available merely because this repo is in an orb.
- Native E2E: bounded X11 PID-scoped focus/resize/key/exit checks passed. Windows
  accessibility/physical display qualification remains open; see native runbook.

## Ticket status index

Only tickets that have started or changed state need entries. Unlisted tickets
remain `planned`. Update the current state above when switching work.

| Ticket | State | Completed slice / remaining work |
| --- | --- | --- |
| PLAN-001 | done | 63 implementation tickets, 326 ticket checklist items, test/compatibility matrices and commit/resume rules. Documentation verified; delivered in this local planning commit. No application implementation. |
| M0-01 | done | Pinned native GPUI window, lockfile, CI and instructions; Linux and Windows 11 build/test/native launch/resize/quit verified (2026-10-06). |
| M0-02 | active | Public-source ledger (SRC-01–11) and runbook; installed runs on unlicensed 8.0.49: W01 partial (layout, unlicensed output, song editor) and W02/W03 single-song static cases (selection, Go Live, masks, shortcuts, logo); W06 partial (Song Editor layout, Format pane, Ctrl+A formatting, slide backgrounds and the Master layout, RUN-W06E–I). RUN-W07A (2026-10-08): W04 search rule, key/page semantics, Options, schedule menu, song Info/copyright, Sort by, Delete (EW8-OBS-044–066). Open: multi-item W02, combined-mask double-click, motion/audio, label jump keys, Master edit reach, W05–W06. |
| M0-03 | implemented-unqualified | Three real Operator action/focus tests and repeated native X11 smoke; shared Linux X11 harness `scripts/native_x11.py` with three ported feature scripts passing (e2e-harness, 2026-10-22; `docs/e2e-linux.md`). Fixed clocks, failure injection and Windows/accessibility checks open. |
| M0-04 | implemented-unqualified | Separate audience process measured under UI stalls, synthetic preparation and operator exit/kill on Linux and Windows DX12 physical displays, incl. mixed-DPI moves, fullscreen, both GPUs, and an idle-machine rerun; hotplug, device loss, present pacing open. |
| M0-05 | implemented-unqualified | CPU snapshots, bounded workers and native resource shaping/upload checked; production intent coordination and hardware qualification open. |
| M0-06 | implemented-unqualified | Ordered bounded native owned-resource IPC/receipts and failure/expiry retention, passed on Linux GL and Windows DX12; production safety coordination/supervision and reference mask semantics open. |
| M0-07 | implemented-unqualified | Explicit-font/image native worker preparation/submission plus static/FFV1 GPU readback; resized output, transitions, performance and physical qualification open. |
| M0-08 | implemented-unqualified | M0-08a mask layer and logo slot: `masks` (None/Black/Logo plus independent Clear), Safety-lane masks and Cue-lane logo over the delivery wire (kinds 5 MASK, 6 LOGO), per-lane ordering, audience Black/Clear/Logo rendering. Operator side is M1-10c. Native hardware capture, reference mask semantics for combined masks (EW8-OBS-020) and video logo (M2-03) open. |
| M1-01 | implemented-unqualified | Schema 5 (search index, M1-07a) after schema 4 (backgrounds, M1-05h1) and schema 3 (formats, M1-05g1); schema-2 section/arrangement persistence, verified backup-gated migration/fresh restore and process-abort tests; remaining schemas, destructive migrations/recovery UI and power-loss qualification open. |
| M1-02 | implemented-unqualified | Separate-pane contemporary shell, documented-reference toolbar correction and M1-02d menu bar (EW order, Alt+mnemonic and mouse access, action parity, native script passed); layout persistence (splitters, collapsed, window placement; native PASS) done; modes/installed-reference/mixed-DPI checks open. |
| M1-03 | implemented-unqualified | Contextual keyboard access to shell and menu commands mapped to GPUI actions/key contexts with native checks; text-entry/modal/selection/live command ownership open. scrutiny-r2 (2026-10-22): the Songs row menu keeps show keys inert (SC2-FOCUS-01, GPUI + `scenario-s02-x11.py`), and so do the Schedule item, image and New ▾ menus (SC2-FOCUS-02, review round 1); a press on New ▾ released elsewhere no longer swallows the next Enter (SC2-FOCUS-03, review round 2); Windows run open. |
| M1-04 | implemented-unqualified | Songs: virtualized list, multi-select, Sort by Title/Author/Copyright/Administrator/Song Number/Date modified persisted (songs-browser 2026-10-22, schema 7 sort keys), Delete with Yes/No, search box, stale-reply/delete/keyboard GPUI tests, Linux `songs-browser-x11.py` PASS. Open: Media and other tabs, grid sizing, collections, Windows `songs-browser-windows.py` (NOT RUN), EW sort-rule observations (PP-SB-01..03). |
| M1-05 | implemented-unqualified | Persistent metadata/section authoring, full document undo/redo, receipt-gated OK; M1-05e EW-observed Words layout with off-thread rendered preview; M1-05f rendered Slides thumbnails (Windows native check); M1-05g implemented-unqualified (g1 per-slide format storage, g2 styled rendering with the operator font gate, Windows DX12 native check passed; g3a Format pane and g3b Ctrl+A whole-song selection with replace-typing, GPUI and Windows native checks). M1-05h implemented-unqualified (h0 RUN-W06I observed, h1 model and schema 4, h2 rendering with the operator background gate and Windows native check, h3 Slide pane editing with the master's Layouts tab and Windows native check), M1-05i implemented-unqualified (operator song menu, GPUI tests and Windows native script), M1-05j–o (planned format controls and tabs), M1-05p–s (planned: gradient, opacity/rotate/flip, Theme Elements/layouts, Auto aspect and image tiles), drag selection across cells, IME/accessibility qualification open. M1-05 song metadata (song-metadata-fields, 2026-10-18): implemented-unqualified (schema 6, Administrator/Song Number inputs and Inspector rows, audience copyright strip, metadata search, Date modified stamp, editor close modal; native `song-metadata-windows.py` PASS). scrutiny-r2 (2026-10-22): the editor close prompt is an AccessKit Dialog named with its question (SC2-A11Y-01), now "Save changes before closing?" to match Yes = save (SC2-I18N-02); `scenario-s0{1,2}-x11.py` create and edit songs natively. Copyright display follow-up (songs-browser, 2026-10-22): implemented-unqualified (Options › Song Copyright Info: Display, first displayed slide only, License Info line; GPUI tests and Linux script; Windows run, EW observation PP-SB-05..08 and M1-05t fonts open). |
| M1-06 | implemented-unqualified | Stable IDs, immutable domain, backed-up migration, editor data/undo persistence; M1-06d adds the editor Arrangement tab, per-schedule-item arrangement (`.selaschedule` v2), deterministic overflow pagination with feedback; reference repair UI and EW observation open. |
| M1-07 | active | M1-07a implemented-unqualified: schema 5 FTS5 index maintained in the song transaction, escaped prefix queries, bm25 ranking with deterministic ties, generation-tagged search, repair/rebuild, 20k database benchmark (pooled p95 75.9 ms; one-letter prefix p95 109 ms). M1-07b implemented-unqualified: Songs search box (2-character minimum, generation-guarded, typing never fires shortcuts, clear/Esc, no-results, selection and save refresh) and the accessibility Edit; 20k keystroke-to-paint p95 81.5-83.8 ms (3 runs). Slide-label search implemented-unqualified (songs-browser 2026-10-22: schema 7 `labels` column, label weight 0.5, label-only hits sorted last since the 2026-10-23 review fix, Linux VM A/B benchmarks in the work log). Older-hardware, Windows and Narrator runs open. Import index maintenance proven by M1-14 `commit_import` through the save path (import-framework, 2026-10-22). |
| M1-08 | implemented-unqualified | themes stream (2026-10-22): theme model and inheritance, schema 8 (developed as 7, renumbered at integration 2026-10-23) with verified backup, `.selaschedule` v4 item snapshots, margins as cue insets, Themes tab (thumbnails, warnings, default, apply to song/item), Song Editor theme mode (reuse of Format pane, undo, Reset Look, Cancel), missing font/theme notes in Preview; lib/bin/integration tests, GPUI `tests::themes` (16 after review round 2) and `themes-x11.py` PASS. Open: `themes-windows.py` run (no Windows host), EW Themes tab/editor observation (PP-TH-01..06, -08..-10; TH-BG-01..12), R-level render checkbox, M1-08a (Layouts theme selector / EW8-OBS-057 Master scope), M1-08b (Copy to Theme ▸ Song Theme), Text › Layout placement of margins (M1-05k), free copyright placement, screen reader. |
| M1-09 | active | Schedule files implemented-unqualified (`.selaschedule` Save/Save As/Open/Open Recent/Close, atomic worker I/O, hidden revisions; see work-log record "schedule-files"). M1-09a implemented-unqualified: framework-free `schedule` (session-local entry IDs pinning song revisions, 32-item cap, move/remove/neighbor, dirty tracking), Schedule pane with drag/drop, save/open dialogs and unsaved guard, Live identity by entry, renderer-acknowledged Live marker; Windows native check passed. schedule-remainder (implemented-unqualified): duplicate, Shift/Ctrl multi-select, group move, drag reorder with autoscroll/Esc cancel, reviewable Update items. Open: EW observation of multi-select/drag, schedule item types beyond songs, persisted item IDs, native drag run, qualification. scrutiny-r2 (2026-10-22): localized idle/untitled window title (SC2-I18N-01) that reaches the accessibility tree at once (SC2-A11Y-02), unsaved prompt announced as a dialog (SC2-A11Y-01); S01 save/close/reopen native PASS on Linux. |
| M1-10 | active | M1-10a: production `sela --audience` renderer mode (moved compositor/text, centered text, settled surface-extent frame, non-activating monitor-covering window, scene retained after controller loss), Windows DX12 smoke on two monitors. M1-10b: operator output supervisor, Songs list, section slides in Preview, Go Live/double-click, Previous/Next, Live shows renderer-acknowledged slide, Windows keyboard-driven native check. M1-10c (with M0-08a): operator Logo/Black/Clear toggles with acknowledged indicators, picture logo from the profile image folder, show-control key context, Windows native checks passed. M1-09a schedule items now exist. Navigation remainder (page/arrow/label keys, indicators, tile thumbnails) implemented-unqualified, see preview-live-navigation record. Open: preparation off the UI thread, reference-observed behavior (EW8-OBS-020), hotplug, latency, physical qualification. scrutiny-r2 (2026-10-22): a failed Go Live of another item no longer borrows the old slide index (SC2-LIVE-01), Go Live on a duplicate entry drops the original's pending cues (SC2-LIVE-02), a library pick cancels a waiting Live Page Down (SC2-LIVE-03), Normalize applies to a Go Live still in preparation (SC2-LIVE-04) by rebuilding it in place, keeping a double-click unmask (SC2-LIVE-05, review round 2); S02 masked Go Live native PASS on Linux. |
| M1-11 | active | M1-11a implemented-unqualified: hardened PNG/JPEG import (8 MiB, 16384 px, 64 MiB decoded, header bomb guard, Exif orientation applied in the shared decoder, ICC ignored, APNG refused, identical re-import reused, same name different bytes numbered, cancellable temporary-and-link copy, side-specific errors), `preflight::check`/`Worker` (missing/changed images, missing fonts, logo) and hash-checked `images::relink`. M1-11b implemented-unqualified: preflight report on schedule open, Locate…/relink, Go Live missing-file warning, import Cancel and notes, photos over the decode budget downscaled at import (source untouched). Open: thumbnails pipeline, EW media observation, native Locate… chooser, Linux/macOS. |
| M1-12 | implemented-unqualified | Autosave revisions in `<profile>/Recovery/` (checksummed, atomic publish, worker thread, injected clock/fs faults), uncommitted marker (unsaved work only, withdrawn when untrue), close flush (`offered` when the recovery offer is unresolved), recovery prompt with loss disclosure (read-only when the session cannot be written), 20/4 MiB rotation, OS profile lock and second-instance notice; S03 SIGKILL tests, 18 GPUI tests and `autosave-x11.py` PASS (autosave-recovery, 2026-10-23, review round 2). Open: `autosave-windows.py`, EW observation (EW8-OBS-071..078), real disk/ACL faults, power loss, older-hardware hitch (P/H). |
| M1-12b | planned | Local profile management (Profiles > Default / Profiles Manager…: create, switch, rename); split from M1-12 in its review; EW8-OBS-077, PP-AUTO-10. |
| M1-13 | implemented-unqualified | bundle-backup-restore (2026-10-23): strict ZIP subset + manifest v1 (`src/bundle`), standalone schedule export/open with preflight notes, profile backup and restore into a new folder, `sela-bundle` worker with Cancel and quit hold; U/I/fuzz/GPUI/S04 tests and `bundle-x11.py` PASS. Open: `bundle-windows.py` (NOT RUN, no Windows host; real second machine and NTFS case-insensitive volume), EW observations PP-BUN-01..08, menu-driven switch to a restored profile (M1-12b), real disk-full/read-only volumes, theme-snapshot images in preflight, operator export options (embed images on/off). Review round 1 (2026-10-23): pipe-safe open, panic-safe worker, too-late Cancel never replaces the schedule, shareable export permissions, localized library errors. Review round 2: mid-write panic cleanup tested, localized file system and per-failure library reasons; open item "Localize image errors". |
| M1-14 | implemented-unqualified | OpenLyrics parser core plus the import framework (import-framework, 2026-10-22): File › Import ▸ Songs… / Song Folder… (UNOBSERVED placement), bounded `sela-import` worker, staged preview with warnings (new `Warning::IgnoredAttribute`) and per-file errors, duplicate policy Skip / Import as copy / Replace (best guess), one-transaction commit through the save path, cancel in every phase; GPUI tests and `song-import-x11.py` PASS. Open: Windows `song-import-windows.py` (not run), EW observations PP-IMPF-01..09. Review round 1 fixes landed (quit guard, folder-budget note, non-regular files, byte-limit message, Replace warning, Cancelling-path tests); round 2: resumed quits guarded, separate quit note, `O_NONBLOCK` open, end-to-end truncation and walk tests, native quit checks. |
| M1-15 | active | scrutiny-r2 (2026-10-22): S01 and S02 automated as `scripts/scenario-s0{1,2}-x11.py` (PASS on Xvfb with the real audience process) and `scripts/scenario-s0{1,2}-windows.py` (NOT RUN, no Windows host). Themes (S01 uses a slide Color Fill), S03/S04, the 90-minute rehearsal and hardware remain open. bundle-backup-restore (2026-10-23): S04 automated (`tests/bundle_s04.rs`, `bundle-x11.py` PASS; `bundle-windows.py` NOT RUN). |
| M1-16 | implemented-unqualified | Developer-local Linux install prerequisite; settings slice (versioned store + Options dialog with Output monitor) implemented-unqualified; language slice (i18n tables, all operator/editor strings migrated, Options language radio, runtime switch + persistence, native check) implemented-unqualified; a11y slice (AccessKit ids/roles/localized names, toggle/selected/expanded/disabled states, documented Tab order, Songs-row arrows, UIA dump + VAL-A11Y native scripts pass) implemented-unqualified; Windows installer, Narrator qualification and dependency gates remain open. scrutiny-r2 (2026-10-22): dialogs announced (SC2-A11Y-01), title reaches AT-SPI/UIA at once (SC2-A11Y-02), localized title (SC2-I18N-01), `.agents/setup` works with rustup 1.28 and installs mingw-w64, the Windows target, the E2E harness packages and ruff; Narrator and Windows runs open. |
| M2-01 | implemented-unqualified | media-core (2026-10-22): pinned LGPL 8.1 linux64/win64 builds with verified fetch scripts, locator + license check, startup locate on its own thread with log line and non-blocking "Video and audio unavailable" status (GPL/nonfree/missing), subprocess isolation (process group / Job Object; Linux children also die with a crashed Sela via `PR_SET_PDEATHSIG`) and CPU frames bounded by count and bytes (enforced). Open: Windows runs (PP-MC-03), macOS children outliving a crashed Sela, hardware decode, GPU/audio budgets, format matrix qualification, installer notices. |
| M2-02 | implemented-unqualified | media-core (2026-10-22): framework-free supervised decoder (ffprobe metadata, raw RGBA frames with showinfo timestamps, bounded queue/backpressure, cancel, EOS, stall watchdog, crash/corrupt/unsupported/no-video, timestamp repair, `VideoFrame::background()`, `first_frame`), VAL-MEDIA-038 runtime check; 25 integration tests in `tests/media_decode.rs` (review round 2). video-playback (2026-10-23): video slide/master backgrounds presented by the renderer (first-frame gate, timestamps, loop, shared playback, text over video, failure keeps the scene), pin checks with black substitution (re-checked for every cue since review round 1), Media › Videos import and list; U/I/R/GPUI tests and `video-x11.py` PASS. Open: videos in the Go Live missing-file dialog and preflight, Windows runs (PP-MC-03, PP-VP-12), preload, seek, hardware decode, EW observations PP-VP-01..11. |
| M2-03 | implemented-unqualified | video-playback (2026-10-23): Live transport row Play/Pause/Restart (wire kind 8 MEDIA, Safety lane after masks), renderer reports (wire kind 9 STATUS, presented frame time) shown in the row, Repeating loop / play-once, masks never pause a video (best guess). Open: observed rules (EW8-OBS-VP-4..10), seek/stop/start position, masks waiting behind an in-flight transport command (shared Safety lane), keyboard routes once observed, audio transport (M2-04), Windows run (PP-VP-12). |
| M2-04 | implemented-unqualified (feasible part) | video-playback (2026-10-23): audio of background videos — second supervised FFmpeg decode (`media::audio`, f32le) into a 500 ms lock-free SPSC ring; output callback with atomics only keeps within 40 ms of the video master clock (skip ahead / silence), follows Pause/Play/Restart/loop/end, masks never mute (best guess EW8-OBS-VP-13); Windows default device via `cpal` 0.18.2, null output elsewhere; STATUS carries the audio state, shown in the transport row. Open: real speakers and A/V measurement, Windows run (PP-VP-12/13), volume/mute/fades/routing, device loss and rate change, audio-only items and background audio. |
| M2-05 | active | video-playback basic slice (2026-10-23, implemented-unqualified): latest-wins poster and pin-check workers with in-flight deduplication, poster LRU (16 MiB / 256), identity keys and rescan invalidation with atlas release; review round 1: list posters follow the scrolled-in rows, slide posters follow their pin checks; review round 2: taking a reply ends its key's in-flight mark, so a key asked for again at once is queued. Open: a virtualized Videos list, next-cue preload, shared preparation scheduler, GPU/disk budgets, storm tests and profiling. |
| M3-01 | implemented-unqualified | stage-display (2026-10-22): audience + stage outputs as separate processes/supervisors, routing by cue kind (STAGE kind 7; each renderer refuses the other), Options › Foldback monitor never defaulting onto the audience (no free monitor = no stage), failure isolation both ways (GPUI + X11 kill, triple-monitor Xorg placement). Open: alternate/stream/house outputs (M3-03), per-output themes, mixed DPI/refresh, hotplug/topology, Windows/physical routes. |
| M3-02 | implemented-unqualified | stage-display (2026-10-22): stage composition (label, current, next/next item/end, message, clock), Options clock/next switches, last-slide/repeated-chorus/empty-next/arrangement-change tests; review round 1 (2026-10-23): panel follows only what the audience was handed, drawn in the live slide's face with stand-ins for missing glyphs; round 2: chrome (label, clock, next label, message) in bundled DejaVu Sans unless only the song face draws it, stage encode budget (`examples/stage_frame_cost.rs`). Open: EW Foldback audit (EW8-OBS-STG-1..7 best guesses, PP-STG-01..09), safe-area/font settings, chords/notes, viewing-distance check, Windows run. |
| M3-05 | implemented-unqualified (stage clock only) | Owner scope (Mission 2026-10): only the stage clock; countdowns, service timers and audience clocks are out of scope by that decision. stage-display (2026-10-22): renderer-drawn local "HH:MM" clock on its own worker, independent of the operator UI; fake-clock tests of the worker tick and a clock upload failure that ends the stage process (2026-10-23); round 2: the clock and labels never follow a decorative song face, and a gone clock worker also ends the stage process. Open: midnight/wall-clock-correction/suspension tests, EW clock audit (PP-STG-04), Windows run. |
| M4-03 | implemented-unqualified | Scripture providers (scripture stream 2026-10-22, review rounds 1-2 2026-10-23): bundled PD KJV/WEB with provenance, per-translation SQLite with integrity checks and index rebuild, versification inventory, OSIS/USFX/Zefania import (restricted, labelled in words); Windows run open. |
| M4-04 | implemented-unqualified | Parser plus Scriptures tab (scripture stream 2026-10-22, review rounds 1-2 2026-10-23): reference/keyword search, completion, selection, pagination, Go Live, schedule snapshots (native save/reopen on Linux); EW observation (PP-BIB), scripture theme, Options › Scripture and the Windows run open. |
| M4-07 | active | Parser cores implemented-unqualified: OpenSong, ChordPro, EasyWorship RTF words. OpenSong and ChordPro hooked into the M1-14 import framework with staged preview and commit, Linux native PASS (import-framework, 2026-10-22); Windows runs and EW comparison open. EasyWorship 6-8 library importer implemented-unqualified (ew-library-import, 2026-10-23): File › Import ▸ EasyWorship Library…, private `query_only` copies with `UTF8_U_CI`, metadata mapping, missing `word` rows, `song_uid` duplicates, re-import as duplicates, sources hashed before/after; GPUI tests and `ew-import-x11.py` PASS. Libraries over 500 songs import in turns (songs not yet in Sela's library first, review round 2). Open: every schema detail UNOBSERVED (PP-EWL-01..08 against a copy of the owner's EW 8.0.49 library), `ew-import-windows.py` NOT RUN, a run on a real library over 500 songs, description/tags fields. |
| M5-02 | active | fuzz-audit slice (2026-10-22, review rounds 1-2 2026-10-23) implemented-unqualified: threat model and unsafe inventory U-01..U-04 (docs/security.md), seeded fuzz suites with a retained corpus for every untrusted-input parser of the integration branch (incl. Bible modules, bundles, translation files, autosave revisions, themes, v5 schedules, FFmpeg reports, stage frames), SEC-01..13 fixed with regression tests, deny.toml + scripts/audit.sh passing with six documented unmaintained notices. Open: state-machine sequence fuzzing, durable-write interruption/disk-full audit, export permissions and secrets (M5-05, providers), a fuzz target for the future EW-library importer, Windows run (PP-FUZZ-01). |
| M5-07 | active | Notices slice (fuzz-audit, 2026-10-22, review rounds 1-2 2026-10-23) implemented-unqualified: reproducible THIRD-PARTY-NOTICES from Cargo.lock (scripts/third_party_notices.py, unit checks in scripts/test_third_party_notices.py, both run by scripts/audit.sh). Packages, checksums, FFmpeg source offer, installer and S08 open (PP-FUZZ-02). |

## Session records

### M0-01 — Native bootstrap — 2026-10-04 (Asia/Jakarta)

- Scope: one Rust package, pinned GPUI/platform and stderr diagnostics, minimal
  original light window, contextual Quit action, close lifecycle, lockfile,
  Linux checks/Windows build CI, setup dependencies and build instructions.
- Upstream pin, inspected API paths and Apache/GPL boundary are recorded in
  `docs/gpui-bootstrap.md`; no Zed application components/assets copied.
- Verification: `cargo build --locked -j 8` passed; `target/debug/sela --version`
  printed `Sela 0.1.0 (technical preview)`; `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -j 8 -- -D warnings`, `git diff --check`
  passed. `cargo test --locked -j 8` passed with **zero tests**, not feature coverage;
  M0-03 adds meaningful action/native checks.
- Initial compile exposed the two-argument close callback and a focus-handle
  borrow; both corrected against pinned GPUI before final checks.
- Native N: Xvfb 1600x1000, Openbox, xcompmgr, Debian 12 x64, Mesa llvmpipe
  software GL. `DISPLAY=:99 xdotool ... key --clearmodifiers ctrl+q` and
  `... key --clearmodifiers alt+F4` closed the window; service recorded clean exit.
  Root `import` captures cropped with ImageMagick were inspected at 960x600 and
  720x440: readable title, limitation text and Quit control, no clipping.
  Evidence: `.amp/in/artifacts/m0-01-bootstrap.png`, `m0-01-small.png`.
- Failure evidence: software Vulkan configured but produced black captures;
  native GL test used `VK_DRIVER_FILES=/dev/null`. Direct-window captures also
  failed/hung; root captures succeeded. No claim that Vulkan/native GPUs pass.
- Setup: `bash -n .agents/setup`, `shellcheck .agents/setup` passed; installation
  11.903s; locked fetch warm run 3s, subsequent run 0.932s. No secrets required.
- Unrun: Windows CI (not pushed/dispatched), Windows build/native launch,
  hardware, accessibility and EasyWorship installed observation. `list_runners`
  returned no connected runners. No milestone gate waived.
- Delivery: local M0-01 implementation checkpoint, not an installer/release.
- Next: integrate M0-02 research and run M0-03/M0-04 isolated worktrees; obtain
  Windows dual-display/reference access for required qualification.

### M0-01 — Partial: orb prerequisites — 2026-10-04 (Asia/Jakarta)

- Scope: `.agents/setup`, `rust-toolchain.toml`, `.gitignore`, README setup notes;
  no application manifest, service, credentials or renderer selection.
- Upstream: Zed commit
  [`a846890`](https://github.com/zed-industries/zed/commit/a84689073d296dfd39987bc7dd478e43ef76d83a),
  `rust-toolchain.toml`, `script/linux`, and GPUI/Linux/WGPU manifests. This
  inspected revision is provenance, not yet Sela's GPUI dependency pin.
- Decisions: pin Rust 1.98.1 with rustfmt/clippy; install only missing Debian
  prerequisites; persist cargo discovery for login shells; no resume script
  because nothing needs authentication or service repair. Native package list
  must be revalidated against the eventual application feature set.
- Verification on Debian 12 x86_64 orb:
  - `bash -n .agents/setup` and `shellcheck .agents/setup`: passed.
  - `time .agents/setup`: first install 15.885s. Replaced deprecated rustup
    auto-install with explicit `rustup toolchain install --no-self-update --no-update`.
    Final script warm runs: 0.409s and 0.531s, no package downloads.
  - `RUSTUP_HOME="$fresh" time .agents/setup` with a disposable empty rustup
    directory: final-script fresh toolchain installation passed in 12.55s;
    temporary toolchain removed afterward.
  - `env -i HOME="$HOME" USER="$(id -un)" PATH=/usr/local/bin:/usr/bin:/bin
    /bin/bash -lc 'command -v cargo rustc; rustc --version'`: proxies found in
    `/home/user/.cargo/bin`, Rust 1.98.1. Rust 2024 stdin compile/run in the same
    clean login-shell environment printed `Rust compile/run smoke passed`.
    Initial smoke-command shell quoting failed; corrected command passed.
  - `pkg-config --modversion fontconfig wayland-client xkbcommon-x11`: 2.14.1,
    1.21.0, 1.5.0. `vulkaninfo --summary`: Mesa llvmpipe CPU device; not physical
    GPU/display qualification.
  - `grep -Fc '# Sela orb Rust environment' "$HOME/.bash_profile"`: one hook
    after repeated runs. `git check-ignore .amp/portals/test.json target/test`:
    both ignored. Documentation verifier and `git diff --check`: passed.
- Not run: GPUI build/render, app tests, Windows build/native launch, hardware
  checks; no application exists. Software Vulkan detection is not app execution.
- Delivery: local partial commit `chore(M0-01): prepare Rust and GPUI orb prerequisites`;
  no push or project-setting change. Future-orb activation awaits authorization.
- Resume: finish M0-01 upstream application-pattern review, dependency pin,
  manifest/window, CI and Windows verification. No full parent checkbox is
  checked because this setup slice does not satisfy an entire combined item.

### PLAN-001 — Executable roadmap and checkpoint discipline — 2026-10-04

- Request: detailed tickets from zero through release, resumable checklists,
  explicit test boundaries, comprehensive EasyWorship feature/layout/UX coverage,
  optimization and feasible E2E, with commits per ticket or partial ticket.
- Scope: repository documentation only; no GitHub issues, pushes or app changes.
- Decisions: track build-specific reference evidence separately from public
  feature descriptions; leave provider/legal and physical hardware gates explicit.
  Keep native E2E conditional, without weakening manual/hardware acceptance.
- Sources: existing `docs/plan.md`, root `AGENTS.md`, official EasyWorship feature
  page and search results linked in `docs/backlog.md`; no installed reference
  observation performed and no parity claimed.
- Changes: `docs/backlog.md`, `docs/work-log.md`, root guidance and README/plan links.
- Verification: `git diff --check` passed. Inline Python structure check passed:
  63 unique tickets, 326 ticket checklist items, dependency graph acyclic, all
  ticket references and local Markdown links valid. Reproducible final check below.
- Not run: app/unit/UI/E2E/hardware tests; this is documentation-only and there is
  no runnable application. Reference installation and provider rights remain unverified.
- Delivery: local commit `docs(PLAN-001): add executable backlog and checkpoint rules`;
  no push, GitHub issues, release or deployment performed.
- Resume: start M0-01 by inspecting/pinning upstream GPUI patterns and toolchain;
  no application ticket has been started. Do not jump to the full UI or database.

Documentation verification (run from repository root; Python 3 standard library):

```sh
git diff --check
python3 - <<'PY'
from pathlib import Path
import re
text = Path('docs/backlog.md').read_text()
parts = re.split(r'^### (M\d-\d{2}) — [^\n]+\n', text, flags=re.M)
tickets = dict(zip(parts[1::2], parts[2::2]))
assert len(tickets) == len(parts[1::2]), 'duplicate IDs'
graph = {}
for key, body in tickets.items():
    assert 'Acceptance:' in body, key
    assert len(re.findall(r'^- \[[ x]\]', body, re.M)) >= 5, key
    header = re.search(r'Depends on: (.*?)Tests:', body, re.S)
    assert header, key
    graph[key] = re.findall(r'M\d-\d{2}', header[1])
    assert all(dep in tickets for dep in graph[key]), key
visited, active = set(), set()
def visit(key):
    assert key not in active, ('cycle', key)
    if key in visited:
        return
    active.add(key)
    for dep in graph[key]:
        visit(dep)
    active.remove(key)
    visited.add(key)
for key in tickets:
    visit(key)
assert all(ref in tickets for ref in re.findall(r'\bM\d-\d{2}\b', text))
for path in [Path('README.md'), Path('AGENTS.md'), *Path('docs').glob('*.md')]:
    source = path.read_text()
    assert all(line == line.rstrip() for line in source.splitlines()), path
    for link in re.findall(r'\]\(([^)]+)\)', source):
        if '://' in link:
            continue
        filename, _, fragment = link.partition('#')
        target = path.parent / filename if filename else path
        assert target.is_file(), (path, link)
        if fragment:
            headings = re.findall(r'^#+ (.+)$', target.read_text(), re.M)
            anchors = [re.sub(r'[^\w\- ]', '', h.lower()).replace(' ', '-') for h in headings]
            assert fragment in anchors, (path, link)
count = sum(len(re.findall(r'^- \[[ x]\]', body, re.M)) for body in tickets.values())
print(f'PASS: {len(tickets)} unique tickets; {count} ticket checklist items')
print('PASS: dependencies acyclic; references, local links/anchors and whitespace valid')
PY
```

## Copy this record when beginning a ticket or partial slice

```text
### <ticket ID> — <slice title> — <date/timezone>

- State: active | blocked | implemented-unqualified | done | deferred
- Scope / checklist items addressed:
- Dependency and reference observation IDs:
- Decisions / upstream GPUI commit and paths (when applicable):
- Files / durable evidence paths:
- Verification: exact commands + decisive results + environment/build/fixture.
- UI evidence: inspected screenshots/states or interaction/accessibility results.
- Not run: checks and concrete reason; do not report these as passing.
- Remaining checklist / blockers / smallest unblock action:
- Next action: exact command, file/function or observation to resume with.
- Delivery: local partial/completed commit; pushed/published only if actually done.
```

Include the log/checklist update in the same commit as its implementation slice.
Use `git log --grep='<ticket ID>'` to find commits; no need to amend a commit merely
to put its own hash in this file. A blocked partial slice may be committed with
failing checks disclosed, but cannot be marked done or qualified.

### M0-02 — Partial: public-source ledger and Windows runbook — 2026-10-04

- State: **blocked** for installed observation; documentation checkpoint complete,
  not parent-ticket completion. Independently assigned alongside active M0-01;
  the current-state/index above describes the bootstrap checkpoint, not a claim
  that this parallel research has not started.
- Scope: `docs/reference-observations.md` and this appended record only. No
  README/backlog/plan gate changes, UI implementation, or completed M0-02 boxes.
- Evidence: ledger EW8-OBS-001–009, source register SRC-01–04, observation runs
  W01–W06 and separate Sela acceptance handoff G01 in
  [reference-observations.md](reference-observations.md).
- Sources: full official update, Quick Start, EasyWorship 7 shortcuts and Screen
  Setup pages fetched via `read_web_page(forceRefetch=true)` on 2026-10-04.
  Official listing explicitly says “Build 8.0.49 · Released Jun 30, 2026”.
  Historical guide says Preview navigation does not advance live output and
  Live double-click can exit Logo/Black/Clear; v7 shortcut guide assigns Ctrl+C
  both editor-copy and show text-toggle. These are documented-only, not observed
  8.0.49 behavior. Screen Setup describes extended desktop routing, not pacing.
- Decisions: preserve unknown mask precedence/restore/Go Live/transport/output
  scope and focus dispatch. Runbook separates reference compatibility from
  Sela's acknowledged-state, private-selection and output-independence contracts.
  No upstream GPUI pattern review needed: no code or GPUI implementation added.
- Verification: `git diff --check` passed; Python standard-library link/whitespace
  check below passed for both owned files (local links/anchors, unique ledger IDs,
  nine records). All four external source URLs returned full page content in the
  live fetches above. No proprietary installer/assets downloaded or copied.
- Not run: installed reference N/H/L entitlement confirmation, Windows/native
  app tests, physical display/GPU/audio, timing, E2E or failure injection. No
  lawful installed reference or Windows runner is available; public source access
  does not substitute for them. Zero installed observations/artifacts/passes.
- Remaining/next action: obtain authorized Windows operator with lawful 8.0.49,
  physical operator/audience displays and capture; execute W01 then W02/W03 static
  cases and append results/artifacts. Continue focus/routing/full inventory;
  G01 requires the Sela spike and M0-03/M0-08/M0-09 prerequisites. No gates waived.
- Delivery: local `docs(M0-02): checkpoint reference ledger and Windows runbook`
  commit on `ticket/m0-02-reference`; no push, publication or additional agents.

Reproduce this slice's documentation check from repository root:

```sh
git diff --check
python3 - <<'PY'
from pathlib import Path
import re
for path in map(Path, ['docs/reference-observations.md', 'docs/work-log.md']):
    source = path.read_text()
    assert all(line == line.rstrip() for line in source.splitlines()), path
    for link in re.findall(r'\]\(([^)]+)\)', source):
        if '://' in link:
            continue
        filename, _, fragment = link.partition('#')
        target = path.parent / filename if filename else path
        assert target.is_file(), (path, link)
        if fragment:
            headings = re.findall(r'^#+ (.+)$', target.read_text(), re.M)
            anchors = [re.sub(r'[^\w\- ]', '', h.lower()).replace(' ', '-') for h in headings]
            assert fragment in anchors, (path, link)
ledger = Path('docs/reference-observations.md').read_text()
ids = re.findall(r'^\| (EW8-OBS-\d{3}) \|', ledger, re.M)
assert len(ids) == len(set(ids)) == 9
print('PASS: two owned files; local links/anchors and whitespace; nine unique ledger IDs')
PY
```

### M0-03 — Bounded operator harness — 2026-10-04 (Asia/Jakarta)

- State: **implemented-unqualified**, bounded local checkpoint, not full parent
  completion. Dependency M0-01 Linux bootstrap executed; its Windows gate remains
  open. Scope: actual Operator action/focus tests, X11 smoke and native runbook.
  No Cargo manifest/lock changes, audience spike or speculative operator features.
- Files: `src/main.rs` (shared binding setup only), `src/tests.rs`,
  `scripts/native-smoke.sh`, `docs/native-testing.md`, M0-03 backlog boxes and
  this appended record. Upstream pin/path/license and accessibility findings are
  recorded in the native runbook; inspected the existing pinned cargo checkout.
- U/I: `CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo test --locked -j 4`
  passed **3 tests**, 0 failed/ignored. Actual Operator tree, production binding,
  retained focus, input dispatch, blur/refocus, unbound key, wrong context and
  explicit direct-action contrast. Capture observer does not replace the Quit
  handler; upstream test platform quit is no-op, so only native proves exit.
- Checks: `cargo fmt --all -- --check`,
  `CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo clippy --locked --all-targets -j 4 -- -D warnings`,
  `CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo build --locked -j 4`,
  `bash -n scripts/native-smoke.sh`, `shellcheck scripts/native-smoke.sh`,
  `git diff --check`: passed. Cargo serialized shared build locks normally.
- Native: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh /home/user/workspace/repo/target/debug/sela .amp/in/artifacts`
  passed; immediately repeated without artifact argument, passed. Each run
  launched twice, verified PID-window focus, Ctrl+J survival, 720x440 resize,
  Ctrl+Q and Alt+F4 exit status 0 and window removal. Only test PID/windows used;
  existing supervised Xvfb/Openbox/xcompmgr services were not changed/stopped.
- Environment: Debian 12 x64 orb, Xvfb :99 1600x1000, Mesa llvmpipe software GL;
  Vulkan black-capture limitation remains. Binary SHA-256 at verification:
  `ac83cb51fbbe72b477b359d74c59d3facff6566be9de92c834a38912792ce88c`.
- UI evidence: root captured then cropped, inspected
  `.amp/in/artifacts/m0-03-native-small.png` (720x440): readable title,
  limitation text and Quit control, no text clipping. Native stderr retained in
  `.amp/in/artifacts/m0-03-native.log`; temporary root/profile/runtime cleaned.
  Artifacts are local ignored evidence, not committed assets.
- Failure checks: `DISPLAY=:99 scripts/native-smoke.sh /tmp/sela-no-such-binary`
  returned nonzero with missing-binary diagnostic and no leftover smoke directory.
  Explicit window XSendEvent input trial failed (BadWindow/quit timeout), cleaned
  its process, and was replaced with focus-verified XTEST; two final passes above.
  Concurrent focus-changing automation remains unsafe; coordinate native input.
- Not run: Windows build/native UIA/Narrator, physical GPU/displays, Wayland,
  performance, installed EasyWorship compatibility. No suitable Windows/physical
  runner/reference in this orb. Manual equivalent and non-skipping policy are in
  `docs/native-testing.md`; upstream AccessKit plumbing is not Sela accessibility
  qualification. Wrong-context negative is headless only, not a fabricated UI.
- Remaining: first M0-03 combined fixture/clock/failure checklist stays open:
  conventions and temporary XDG dirs exist, but clocked domain and I/O fixtures
  do not yet exist. Extend them when actual domain adapters arrive; no test-only
  production flags. Integrator should rerun tests/native smoke after merging,
  then execute Windows/manual accessibility fallback on a real runner.
- Delivery: local `feat(M0-03): checkpoint bounded GPUI and X11 harness` commit;
  no push/publication. Full ticket qualification remains open.

### M0-04 — Linux audience process spike — 2026-10-04 UTC

- State: **implemented-unqualified** parallel slice; Windows parent gate open.
- Scope: opt-in `examples/output_spike.rs`, example dev dependencies/lockfile,
  `scripts/output-spike.py`, `docs/output-spike.md`, M0-04 checklist only. No
  `src/main.rs` or native harness changes; no production renderer/scene protocol.
- Upstream: bootstrap GPUI pin and exact winit 0.30.12 example/wgpu 29.0.4
  API rustdoc source paths and license boundaries recorded in output-spike note.
- Chosen boundary: independently supervised audience process owns winit/wgpu GL
  surface/event loop; monotonic triangle motion, ~16.667ms pacing, bounded 2048
  telemetry queue off frame path, explicit native close, 25s self deadline.
- Verification (CARGO_TARGET_DIR=/home/user/workspace/repo/target):
  `cargo build --locked --example output_spike -j 4`,
  `cargo test --locked --example output_spike -j 4` (one test passed),
  `cargo clippy --locked --example output_spike -j 4 -- -D warnings`,
  `rustfmt --edition 2024 --check examples/output_spike.rs`, `git diff --check`;
  `python3 scripts/output-spike.py` passed on shared :99 Xvfb/Openbox/xcompmgr,
  Debian12 llvmpipe Mesa22.3.6 GL. Initial missing display handle adapter error
  fixed against wgpu documented owned-display API; no shared service changed.
- N/I/P: actual 100/500/2000ms GPUI callbacks stalled; 6/30/118 audience calls
  inside exact intervals, queued 100ms action only started after stall. Clean
  operator exit and second operator SIGKILL had 218/72 subsequent calls.
  621 calls, interval p50/p95/max 16.870/19.793/47.308ms. Native resize and close
  passed; all owned processes reaped. Not physical scanout or 60Hz qualification.
- Durable timestamp table, failure boundaries, commands and decision in
  `docs/output-spike.md`; review artifacts `.amp/in/artifacts/output-spike/`
  raw logs/summary/root capture pairs. Inspected cropped root stall geometry
  visibly moved; no direct application capture. Minimal diagnostic controls,
  not EasyWorship parity. Bottom help at small window is a known layout limit.
- Not run: slow preparation, forced surface/device loss, Windows DX12/native
  mixed-DPI dual physical displays/fullscreen/hotplug, physical scanout/cue
  latency, text-over-video, production lifecycle or memory/CPU/GPU soak. Orb
  software display cannot replace these gates; parent not done.
- Next: integrate this local slice; obtain Windows hardware and owning two-process
  supervisor, run backend dx12 with physical capture; extend failure/preparation
  tests before M0-10 decision. Local coherent M0-04 commit only; no push.
- Final rerun after telemetry 1KiB cap/suboptimal surface-release ordering:
  same build/test/clippy/fmt/diff commands passed; `python3 scripts/output-spike.py
  .amp/in/artifacts/output-spike-final` passed, 619 calls, stall calls 6/30/117/6,
  post-exit/kill 214/70; p50/p95/max 16.854/18.152/49.197ms. Prior measurement
  artifacts retained; final exact 2s/exit/kill timestamps appended in spike note.

### M0-04 — Integration and delayed preparation — 2026-10-04 (Asia/Jakarta)

- Scope: integrated M0-02/M0-03/M0-04 worktrees in local main, retained both
  work-log records when resolving the append conflict; no implementation conflict.
  Added single in-flight delayed background task and busy/completed feedback.
  Uses pinned GPUI background executor/entity task/update patterns from
  `crates/gpui/examples/testing.rs`; no actual decoder or production scene protocol.
- Corrected driver title-only targeting and hard-coded orb checkout/environment:
  now PID-scoped, verifies focus on every key, uses private temporary runtime,
  inherits explicit DISPLAY/backend choice, closes/reaps only owned processes.
  Captures root to temporary storage and retains cropped owned windows only.
  Strengthened interval assertions (complete ordered stalls, boundaries included,
  ≥500ms stall-gap rejection as diagnostic only), actual clean-exit status and
  one worker/busy rejection/control action entirely within preparation interval.
- `cargo fmt --all -- --check`, `cargo build --locked --all-targets -j 8`,
  `cargo test --locked --all-targets -j 8` (**4 passed**),
  `cargo clippy --locked --all-targets -j 8 -- -D warnings`,
  `uvx ruff check scripts/output-spike.py`, Bash syntax/ShellCheck, and
  `git diff --check`: passed. CI commands now include example tests; remote
  workflows have not been pushed or run. Formatter changes were reread.
- Native integrated baseline: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  scripts/native-smoke.sh /home/user/workspace/repo/target/debug/sela .amp/in/artifacts`
  passed after merge and again after preparation changes. Rendered resized
  bootstrap inspected: no clipped labels. No fake web UI or browser evidence.
- `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/output-spike.py
  .amp/in/artifacts/output-preparation` passed on Debian12 Xvfb/Openbox/xcompmgr,
  Mesa22.3.6 llvmpipe GL, debug build. 898 present calls; p50/p95/max intervals
  16.828/17.757/46.039ms; 118 inside 2s UI stall; 237/76 after clean exit/SIGKILL.
  During 2s background delay the 100ms UI handler completed before worker finish,
  repeated preparation was rejected, and audience maximum gap was 41.877ms.
  Exact correlation times are in `docs/output-spike.md` and artifact summary.json.
- Inspected preparation pending/completed screenshots and two-frame comparisons
  during UI stall/after exit: readable controls/help and changed triangle position.
  Artifact directory `.amp/in/artifacts/output-preparation/`; captures/logs are
  local evidence, not physical scanout or hardware performance qualification.
- Failure checks: subprocess runs with missing DISPLAY, nonexistent target binary
  and invalid X server all returned nonzero within 15s, produced no success
  summary, and left no `sela-output-*` runtime directories. Initial Ruff findings
  (imports/mode/file scope/style) corrected with formatter and scoped file lifetime.
- Remaining blockers: Windows build/native launch/DX12 physical dual-monitor
  mixed-DPI/fullscreen/hotplug/recovery; lawful installed EasyWorship 8.0.49
  observations; software Vulkan black-capture finding. Production command,
  scene, text/video and data/UI tickets have not been implemented or qualified.
  M0-10 entry-to-M1 gate is not waived; usable/installed Sunday app is not delivered.
- Next concrete action: connect an authorized Windows runner with physical output
  and reference access; execute `docs/reference-observations.md` W01–W05 and the
  Windows spike sequence in `docs/output-spike.md`, recording actual evidence.
- Delivery: local ticket commits plus worktree merges and this M0-04 checkpoint;
  no push, installer, signed package, publication or deployment.
- Final identical-source repeat passed: 883 calls; 117 during 2s stall;
  post-exit/kill 241/75; preparation max gap 54.505ms. Summary retained as
  `.amp/in/artifacts/output-preparation/repeat-summary.json`; transient duplicate
  captures removed. Native setup commands are now explicit in the runbook.

### M0-05 — CPU preparation checkpoint — 2026-10-04 UTC

- State: **implemented-unqualified** partial slice. Framework-independent owned
  scene/version/resource types and one bounded worker, no new UI or production
  renderer/backend selection. Detailed ownership/budgets/limitations are in
  [preparation.md](preparation.md). Direct pinned image/hash/font dependencies
  were already transitive; lock update adds only root dependency entries.
- Worker reads and hashes capped source files, decodes static PNG/JPEG, parses
  supplied font face 0, returns read-only snapshots. Acceptance supersedes older
  work; saturation/invalid input does not. Exact deadlines reject even queued
  results; cancellation and teardown do not wait on blocking OS calls.
- U/I: `cargo test --locked --all-targets -j 8` passed **14 tests** (10 new
  preparation tests, 3 GPUI tests, 1 spike test). Asymmetric RGBA/alpha and JPEG
  normalization, immutable font/text/image snapshots, missing/changed/corrupt
  resources, invalid dimensions, budgets/overflow, worker saturation/failure,
  queued stale results, exact deadline, recovery and nonblocking cancellation/drop.
- `cargo clippy --locked --all-targets -j 8 -- -D warnings`,
  `cargo fmt --all -- --check`, `git diff --check` and existing inline documentation
  checks passed. Initial clippy array-chunk warning corrected with clippy's fix;
  reran tests and strict clippy. Formatter/fix output reread. Font SHA-256 matched
  fixture README; full redistribution notices retained (trailing spaces removed).
- Not run/claimed: rendering/shaping/glyph coverage, GPU upload/readiness,
  APNG fixture regression, measured RSS, Windows/hardware or physical display
  latency. These modules are not yet wired to the native spike. U/I resource
  readiness is not presentation acknowledgment. Parent combined acceptance boxes
  remain open where downstream renderer work is needed.
- Delivery: local `feat(M0-05): checkpoint bounded CPU scene preparation` commit;
  no push or installer. Next: M0-06 bounded ordered submission and acknowledgment
  state; keep safety capacity tests separate from unobserved mask semantics.

### M0-06 — Ordered delivery model checkpoint — 2026-10-04 UTC

- State: **implemented-unqualified** partial slice. `src/delivery.rs` separates
  requested/accepted/applied/unknown state, one global sequence per fresh epoch,
  FIFO normal/safety admission with one outstanding slot each, renderer-side
  rejection and idempotent pending/current duplicates. No IPC or native rendering
  callback is implemented. Full contract and integration obligations:
  [delivery.md](delivery.md).
- Deadline is checked before the callback, not after changing output. Receipt
  timeout/disconnect invalidates the session, clears requests and never replays
  them. Last-confirmed history is not labelled live after disconnect. Ambiguous
  stale receipts require newer confirmation or mark output unknown.
- Verification: `cargo test --locked --all-targets -j 8`: **22 passed** (18 library,
  3 GPUI, 1 spike). Eight new delivery tests cover all 24 receipt permutations,
  both lane orders, 1000 rejected admissions per lane order, duplicate/reordered
  commands, render failure retaining prior Arc, exact deadline/no callback,
  restart/foreign/unsent receipts, renderer overload and real missing-file worker
  recovery. `for run in $(seq 1 20); do cargo test --locked --lib -j 8 --quiet;
  done` passed all 20 runs (18 tests/run); no randomized schedule claim.
- `cargo build --locked --all-targets -j 8`, strict all-target clippy,
  `cargo fmt --all -- --check`, `git diff --check` and inline documentation
  checks passed. Formatted code and entire test module reread/reviewed.
- Not run/claimed: actual safety-control semantics or timing, renderer IPC,
  preparation-intent coordination, receipt-to-GPU-frame integration, physical
  scanout, Windows/reference/hardware. Reserved lane currently holds a prepared
  replacement snapshot, not an invented Black/Clear/Logo state machine. Combined
  parent boxes remain open for those integration requirements.
- Delivery: local `feat(M0-06): checkpoint ordered delivery and acknowledgment model`
  commit, no push/installer. Next: M0-07 explicit-font composition/readback;
  integrate resource readiness before applying this model to the native spike.

### M0-07a — Explicit-font CPU text raster — 2026-10-04 UTC

- State: **implemented-unqualified** parallel CPU diagnostic slice on
  `ticket/m0-07-text`, based on local main 71dc754, not origin/main. Owns only
  `examples/composition/text.rs`, Cargo dev dependency/root lock entry,
  `docs/composition-text.md` and this appended record. Parent boxes are left to
  integration; no main example, src, other modules or backlog edits.
- API: supplied single-face font bytes and original UTF-8 text to bounded
  row-major white alpha; Advanced shaping, explicit lines, no wrap/margin,
  all-line logical and raster ink overflow rejection, source-over mark coverage.
  Collection headers rejected before fontdb allocation; empty supplied-only DB.
  Per-job caches dropped; not a hard memory/time bound or adversarial sandbox.
- Upstream commits/paths, dependency/fixture notices, bounds and exact harness
  commands are in [composition-text.md](composition-text.md). No GPUI code added.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --example text_check -j 4` **3 passed**;
  `cargo clippy --locked --example text_check -j 4 -- -D warnings`,
  `cargo run --locked --example text_check -j 4`,
  `rustfmt --edition 2024 --check examples/composition/text.rs examples/text_check.rs`
  passed. Initial iterator move/borrow error fixed and checks rerun. Temporary
  harness and `/tmp` PGM removed before commit; module reread after formatting.
- Inspected ignored `.amp/in/artifacts/m0-07a-text.png`: readable Café, acute a,
  Arabic سلام and explicit Signal beacon second line; no clipped ink. Tests also
  reject missing glyph, both logical overflow axes, left ink bearing, malformed/
  collection fonts and oversized inputs. `git diff --check` passed.
- Not run: GPU/native upload/readback or golden/transition/video, Windows,
  physical GPU/displays, reference compatibility, performance/RSS. CPU checks
  do not qualify parent M0-07 or waive M0-10. Local subticket commit only, no push.
- Next: integrator includes this module in composition example and reruns example
  tests/clippy plus GPU readback; then execute Windows/reference/hardware gates.

### M0-07b — Offscreen static GPU composition/readback — 2026-10-04 UTC

- State: **implemented-unqualified** subticket on `ticket/m0-07-compose`, based
  on local main `71dc754` M0-05/M0-06 contracts. Assigned GPU module/docs only;
  parent owns permanent common example/checklist and parallel explicit-font mask.
- Files: `examples/composition/gpu.rs`, `docs/composition-gpu.md`, this one
  appended record. No manifest/src/text/backlog/main-example changes. Original
  embedded WGSL full-screen triangle loads nearest sRGB image and linear R8
  coverage, composites straight-alpha image over opaque black and white glyphs
  in linear light, writes sRGB RGBA8, strips padded GPU readback rows.
- Decisions/upstream: exact pinned wgpu29.0.4 MIT/Apache-2.0 API paths,
  coordinate/color policy, per-resource 64MiB/8192/device caps, 10s poll + 1s
  callback wait and remaining default-error/driver-memory boundaries documented
  in [composition-gpu.md](composition-gpu.md). No GPUI patterns or new deps.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --example compose_gpu_check -j 4` (2 GPU-free tests),
  `cargo test --locked --all-targets -j 4` (**24 passed**),
  `cargo clippy --locked --example compose_gpu_check -j 4 -- -D warnings`,
  `rustfmt --edition 2024 --check examples/composition/gpu.rs examples/compose_gpu_check.rs`,
  `git diff --check`: passed. Initial v29 API compile errors and Clippy findings
  corrected, final checks rerun; automatic chunk fix rolled back before explicit
  reference-comparison correction. Formatted code reread.
- R/GPU: `runtime=$(mktemp -d /tmp/sela-compose-runtime.XXXXXX)`;
  `XDG_RUNTIME_DIR="$runtime" cargo run --locked --example compose_gpu_check -j 4`;
  `rmdir "$runtime"`: passed repeatedly using actual Vulkan llvmpipe LLVM15.0.6
  CPU adapter, Mesa22.3.6, Debian12 x64 debug orb, no surface/native service
  changes. Initial successful run without XDG directory had runtime warnings;
  private-runtime runs did not. No GL fallback or global environment mutation.
- Real readback checked asymmetric 3x2 RGBA orientation/nonaligned rows, both
  contain/cover aspect axes and crop boundaries, transparent background, zero/
  half/full coverage (~188 half-white, not 128), midtone linear-light color,
  input rejection and recovery. Retained `Compositor::check_readback()` allows
  parent to repeat all pixel checks; normal unit tests never request GPU.
- Evidence: ignored `.amp/in/artifacts/m0-07b-gpu.png` (321x180) inspected via
  media tool: centered four-color image, black sidebars, white synthetic H and
  half-coverage bar visible. Synthetic mask only, not font/shaping evidence.
  Temporary harness removed before commit; full reproduction is in module note.
- Not run: explicit-font integration, Windows/DX12, physical GPU/display or
  scanout, native E2E, forced loss/OOM/timeout, cut/fade/video, performance/RSS or
  installed-reference checks. Software offscreen pixels do not qualify them;
  parent M0-07 remains open. Budget is per resource, not total resident memory.
- Next: integrator includes GPU module in common example, calls diagnostic
  `check_readback()` only opt-in and feeds CPU worker's exact output-size alpha
  mask into `render`; rerun all-target checks and inspect actual font frames.
- Delivery: local `feat(M0-07b): add offscreen GPU composition and readback`
  commit; no push/publication/backend selection or waived qualification.

### M0-07 — Parallel integration and actual-font GPU frames — 2026-10-04 UTC

- Merged text/GPU worktrees via separate merge commits; only conflict was two
  appended work-log records, both retained. Reviewed both implementations and
  dependency diff. Added permanent `composition_spike` example linking the actual
  M0-05 preparer, explicit-font worker and GPU composition; no fake web UI or
  optimistic M0-06 rendering acknowledgment. README and module handoffs now link
  [composition-spike.md](composition-spike.md) instead of requiring scratch code.
- `cargo test --locked --all-targets -j 8`: **27 passed**, 0 failed/ignored.
  `cargo clippy --locked --all-targets -j 8 -- -D warnings`,
  `cargo build --locked --all-targets -j 8`, `cargo fmt --all -- --check`, diff and
  inline documentation checks passed. Initial format-macro compile typo fixed;
  Clippy's const-expression chunk autofix produced invalid syntax and rolled
  itself back; corrected the const generic braces manually and reran strict checks.
  Final code/formatter output reread.
- R: `XDG_RUNTIME_DIR="$runtime" target/debug/examples/composition_spike
  .amp/in/artifacts/composition vulkan` passed twice, with private mktemp runtime
  removed afterward. Full per-pixel GPU checks plus actual white-font mask and
  uncovered-background checks. Both source image/font files removed before
  rendering. Overflow fixture rejected, never silently clipped/uploaded.
- Inspected all 641×360 color/contain/cover images: readable Latin accents,
  combining mark and connected Arabic; correct contain sidebars/cover filling,
  no glyph boxes/clipping/row corruption. Latest retained representative frame
  inspected again after measured repeat. Evidence in `.amp/in/artifacts/composition/`.
- P diagnostic only: Debian12 x64, 16-vCPU Xeon 2.60GHz, debug, Vulkan CPU adapter
  llvmpipe LLVM15.0.6 Mesa22.3.6. `/usr/bin/time -v` measured one full binary run:
  wall 0.51s, max RSS 115540KiB, no swaps; preparation/raster 42.226ms,
  color/contain/cover allocation/upload/readback 5.271/77.773/12.595ms.
  No frame-time/SLO/physical-GPU claim; note records variability and exact boundary.
- N regression: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh
  "$PWD/target/debug/sela"` passed focus/unbound key/resize/Ctrl+Q/WM-close/exit.
  CLI failure checks via `subprocess.run(..., timeout=10)` passed for missing
  arguments and unsupported backend, nonzero with diagnostic and no output dir.
- Remaining: native audience GPU-ready scene/IPC/receipt integration; video
  surface/copy/sync spike; observed cut/fade/interruption/masks; goldens, resource
  recovery, production memory/performance, Windows/hardware. Refreshed Amp
  `list_runners`: **none connected**. Lawful installed 8.0.49 and physical Windows
  dual-display checks are still required; M0-10/full-M1 gate is not waived.
- Delivery: local integration checkpoint, worktree commits retained; no push,
  installer or service-ready app. Resume partial M0-07 integration above. Connect
  an authorized Windows/reference environment to unblock observed transitions,
  M0-08 and the hardware gate; do not label static PNGs a usable worship app.

### Sequencing approval and next slices — 2026-10-04 UTC

- Owner cannot currently provide hardware and explicitly authorized continuing
  operator UI first while renderer testing remains unavailable. Recorded the
  exception in plan/backlog; no release or parity gate marked done/deferred.
- M0-07c scope: bounded FFmpeg child decode of an original lossless FFV1 fixture,
  complete RGBA frames to existing wgpu compositor, ordered frame/copy checks,
  overload/cancel/decoder failure, exact limits and licensing/interop findings.
- M1-02a scope: original native GPUI Schedule/Preview/Live over Resources shell,
  empty/disconnected/disabled states, resource tabs, bounded splitters and compact
  window. Historical Quick Start Guide supports pane families/resource tabs only;
  exact 8.0.49 dimensions/focus/modes remain unverified. Initial geometry is
  reversible development policy, not a new claim of compatibility.
- Upstream review completed for pinned GPUI div drag/focus, canvas, window bounds
  observation and test mouse APIs; current upstream comparison matched the pin.
  Keep framework Apache APIs, not GPL Zed application UI components.
- Independent worktrees will own video example/modules versus operator source/
  tests/native driver. Merge with local ticket commits and combined checks;
  serialize native focus-changing tests. No push/publication authorization.

### M1-02a — Empty operator shell — 2026-10-04 UTC

- State: **implemented-unqualified**, early-UI exception; local main `115f545`
  base on `ticket/m1-02-shell`, not origin/main. No physical/reference gate waived.
- Scope: `src/main.rs`, original `src/operator.rs`, `src/tests.rs`,
  `scripts/operator-shell.py`, `docs/operator-shell.md`, this appended record.
  No library/contracts, manifests, examples, plan/backlog/README changes.
- Separate Schedule/Preview/Live over full-width Resources; provisional 24/38/38,
  five truthful empty tabs, collections, collapse/restore/reset, three bounded
  proportional splitters. Live disconnected; safety/Go Live have no handlers.
  Retained root focus and semantic clickable/Ctrl+Q Quit; no storage/import claims.
- Pinned upstream Apache framework paths/licenses and historical-reference limits
  are recorded in the shell note. Persistence/other modes remain parent work.
- Verification: shared CARGO_TARGET_DIR; `cargo test --locked --all-targets -j 4`
  **29 passed**; bin build, strict all-target clippy, fmt check, Ruff and diff check
  passed. Two new actual-tree tests cover tabs, collapse/restore/reset, compact
  bounds, every drag/release, focus, cross-axis isolation and subminimum fallback.
- Native: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/operator-shell.py`
  and existing native-smoke passed on shared Xvfb/Openbox/xcompmgr software GL;
  owned app reaped, shared services unchanged. Installed x11-utils for xwininfo.
  Corrected decoration-offset crops/input and drag cross-axis bug after visual
  inspection; regression assertion added. Initial restore test reused the old
  button position after collapse; corrected to query its new bounds.
- Inspected `.amp/in/artifacts/operator-shell/{normal,compact,scriptures,collapsed,
  drag-minimum}.png`: readable 1280x800/720x440, true tab/restore states, bounded
  minimum drag. Details/exact repeat commands in `docs/operator-shell.md`.
- Unrun: Windows, hardware, mixed DPI, installed 8.0.49, accessibility and
  performance; no parity or audience qualification claim. Next: parent merges,
  reruns combined tests/native checks, maintains persistence/modes/qualification.
- Delivery: local `feat(M1-02a)` commit only; no push/publication.

### M0-07c — software decoder/GPU interop — 2026-10-04 UTC

- Implemented-unqualified diagnostic in `examples/video_spike.rs` and
  `examples/video/decoder.rs`; details, limits, source/legal review, exact
  reproduction and remaining work in [video-spike.md](video-spike.md).
- Original private 12-frame 320×180/12fps FFV1 BGRA Matroska decoded off caller
  into capacity-one complete RGBA queue. Supervisor kills/reaps on cancellation,
  Drop/deadline; polling does not join. Explicit local format/codec/protocol,
  output-size forcing, 1920×1080/120-frame/32MiB/30s limits; no new dependencies.
- Checks with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --example video_spike -j 8`: **9 passed**;
  `cargo clippy --locked --example video_spike -j 8 -- -D warnings`,
  `cargo build --locked --example video_spike -j 8`, `cargo fmt --all -- --check`
  passed. Initial Clippy constant-chunk warning fixed; final files reread.
- R: private mktemp XDG runtime, `target/debug/examples/video_spike
  .amp/in/artifacts/video` (shared target absolute path) passed twice on Vulkan
  llvmpipe/Mesa22.3.6, FFmpeg5.1.9-0+deb12u1 GPL build. All original frame bytes,
  order, actual-font white glyph/uncovered GPU pixels, missing/corrupt errors,
  deterministic queue-saturation deadline/cancel, child reaping checked.
  Ordinary tests need neither FFmpeg nor GPU; static Unix child tests additionally
  cover partial EOF, bounds, watchdog and asynchronous Drop.
- First/last PNGs inspected under `.amp/in/artifacts/video`: readable actual text,
  asymmetric green stripe, moving pink rectangle. No native X11 used. No native
  hardware/Windows, original PTS/audio/zero-copy, frame-SLO or hostile-input
  sandbox claim. Encoder fixture setup remains synchronous without a watchdog;
  production lifecycle/resource qualification remains open.
- Next: parent merge and combined validation/backlog update; carry unresolved
  native surface/fence/PTS/audio/device/memory work forward. Local coherent
  `feat(M0-07c)` commit only; no push or waived release gates.

### M0-07c — Parent integration review and failure fixes — 2026-10-04 UTC

- Merged the shell and decoder worktrees locally, preserving both ticket commits
  and both appended records. Reviewed actual implementations before integration.
- Fixed terminal decoder failures exposing buffered frames, completion racing the
  final poll, and the fixture encoder's unbounded exit wait. Added real-child
  success/failure final-buffer and encoder timeout/error regressions.
- `cargo test --locked --all-targets -j 8`: **40 passed** (11 video tests).
  `cargo clippy --locked --all-targets -j 8 -- -D warnings`,
  `cargo build --locked --all-targets -j 8`, `cargo fmt --all -- --check` passed.
  Formatted implementation and diff reread. Runtime private mktemp directory:
  `XDG_RUNTIME_DIR="$runtime" target/debug/examples/video_spike
  .amp/in/artifacts/video` passed all 12 exact frame/readback and failure checks.
- Parent inspected both first/last PNGs: readable actual text, left green stripe,
  rectangle moves from left to right across the captured endpoints, no corruption.
  Actual ordering/bytes verified by the executable, not inferred from screenshots.
- Remains diagnostic-only: no native playback, audio, original PTS, physical
  Windows/GPU qualification or hostile-process sandbox. Kernel metadata/spawn
  latency and inherited stdout descendants are outside the watchdog contract.
- Delivery: local ticket fix checkpoint; no push or release. Continue operator
  work under the approved sequencing exception; native renderer integration and
  observed transitions remain open.

### M1-02a — Shell integration and native evidence — 2026-10-04 UTC

- Parent reviewed and merged original worktree commit. Fixed collapsed layout's
  unused six-pixel gutter; added all three lower drag-boundary assertions, alongside
  existing upper-bound tests. Native driver uses repo-relative target by default
  and distinguishes asserted input/exit checks from visually inspected state.
- `cargo test --locked --all-targets -j 8`: **40 passed**, including five operator
  tests. All-target strict Clippy/build, fmt, Ruff and diff checks passed.
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/operator-shell.py` and
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh
  "$PWD/target/debug/sela"` passed on Debian12 Xvfb/Openbox software GL.
- Parent inspected normal, compact, Scriptures, collapsed and drag-minimum PNGs
  under `.amp/in/artifacts/operator-shell/`: correct selected label, full collapsed
  height, minimum 150px Schedule, no clipped controls or pane overlap. Automated
  GPUI tests assert state/bounds; captured images alone do not prove transitions.
- Persisted installed xwininfo and FFmpeg diagnostic prerequisites in `.agents/setup`.
  `bash -n .agents/setup`, `shellcheck .agents/setup`, two setup runs passed (2s/1s);
  no service restart. These setup changes are only local, not active for future
  project orbs until shipped. README/backlog now reflect the merged diagnostics.
- New owner direction: retain EasyWorship layout 1:1 as the compatibility target,
  modern Zed visual styling, not the dated Painter concept. Begin M1-02b styling
  only; preserve placement/behavior and original assets. Zed HEAD matches our pin;
  inspect One Light/settings and button/tab source as design observations only.
- Windows/reference/mixed-DPI and persistent geometry/modes remain open. No
  service-ready/installable release claim, no push. Next: implement styling,
  capture all affected states again, then continue remaining operator tickets.

### M1-02b — Direct GPUI styling checkpoint — 2026-10-04 UTC

- Implemented original neutral chrome, typography hierarchy, thin visual dividers
  with unchanged drag hit areas, subtle hover/pressed buttons and selected tabs.
  No pane/control-region reorder, invented menu or live behavior. Provenance and
  license boundary are in `docs/operator-shell.md`; no Zed components/assets copied.
- Owner clarified design references as Codex/T3code/Notion/Zed and prohibited
  further Painter use. Recorded in AGENTS/plan. Early generated studies are not
  specifications or accepted designs; future iteration is actual GPUI only.
- `cargo test --locked --all-targets -j 8`: **40 passed**; strict all-target
  Clippy, fmt, bin build, Ruff and diff checks passed. Native shell driver and
  native-smoke passed with `DISPLAY=:99 VK_DRIVER_FILES=/dev/null`.
- Inspected actual normal/compact/Scriptures/collapsed/minimum-drag/hover captures
  under `.amp/in/artifacts/operator-shell/`. Layout and labels fit both sizes;
  hover gives subtle feedback, all empty/disconnected claims remain explicit.
  Direct compact-image review rejected an automated visual-analysis false alarm:
  Preview label really is centered in its taller canvas; Live has bottom controls.
- This is not owner design approval, full feature parity or a usable worship
  release. Persistent layout, modes, accessible controls and content authoring
  remain. Local styling commit only; no push/publication.
- Next independent slices: M1-01a durable song/schedule repository as prerequisite
  to real operator content, and M1-03a keyboard access to existing shell controls.
  Both proceed under the UI-first sequencing exception; no output gate waived.

### M1-01a — Durable song and schedule repository — 2026-10-04 UTC

- State: **implemented-unqualified** prerequisite on `ticket/m1-01-store`, based
  on this thread's local main `c937670`, not origin/main. Scope limited to new
  `src/storage.rs`, `src/lib.rs` export, Cargo dependency/lock, `docs/storage.md`
  and this appended record. No operator/main/tests/backlog/plan/setup edits.
- Read project guidance, plan/backlog/work-log before implementation; proceed
  under owner's explicit UI/prerequisite sequencing exception. No parent done
  claim, physical/reference gate waiver or fabricated theme/asset schema.
- API and schema evidence: [storage.md](storage.md). Stable random 128-bit IDs,
  duplicate titles, bounded lossless Unicode song payload, immutable revision
  history, tombstone deletes and ordered FK-backed schedule snapshots. Expected
  head conflicts and whole schedule transaction rollback are explicit.
- Initial empty schema 0→1 transaction only; unsupported/newer/foreign/corrupt
  files rejected without reset/replacement. Dependency pin rusqlite 0.40.2,
  bundled SQLite 3.53.2, limits. Authoritative rustdoc and downloaded dependency
  license/source notices inspected; no GPUI patterns or live contracts changed.
- One background connection/thread per Worker; async open, one outstanding
  bounded command/result including unconsumed completion, nonblocking submit/
  poll/drop. Pre-start cancel does no command I/O; started transactions return
  actual commit/failure, never a fabricated canceled success. Errors omit paths,
  SQLite messages and content. Synchronous Repository is documented worker-only.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target` on Debian
  12 x64 orb: `cargo test --locked --all-targets -j4` **48 passed**, zero failed/
  ignored; `cargo clippy --locked --all-targets -j4 -- -D warnings`,
  `cargo fmt --all -- --check`, `git diff --check` passed. Final code/format and
  lock diff reviewed. No existing dependency version upgraded; eight new
  dependency packages plus unified hashbrown feature added by Cargo.
- Eight new U/I tests use disposable databases for reopen/Unicode/CRLF/LF/empty
  sections/duplicate titles; snapshot survival after edit/delete; stale revision
  and FK failure byte-unchanged rollback; injected second-item trigger failure;
  initial migration rollback; byte-preserved newer/foreign/corrupt files; real
  competing lock (~100ms timeout), actual max_page_count disk-full rollback/
  reopen; pre-start cancel, race-legal actual result, queue busy and nonjoining
  Drop. No skipped DB checks; test fixtures removed by TempDir.
- Not run: GPUI editor/native E2E, Windows, installed reference, physical GPU/
  displays, crash/power-loss recovery, host-volume exhaustion or performance/
  RSS qualification. No UI integration exists yet; no suitable physical/reference
  environment. SQLite page-limit FULL is simulated capacity, not physical disk.
- Remaining parent: arrangements/themes/assets/font references, destructive
  upgrade backups, history retention/search, autosave/close outcome recovery,
  stable cross-revision occurrence selection. Kernel/SQLite non-lock I/O has no
  hard deadline and memory limits are payload/cache policies, not hard RSS.
- Next: integrator cherry-picks this local ticket commit, reruns combined tests,
  then wires Worker open/poll/SaveSong/Heads to the actual GPUI editor with explicit
  busy/conflict/error/dirty-close states. Poll writes before intentional close;
  reconcile heads after unexpected Drop. Local per-ticket commit only; no push.

### M1-03a — Existing shell keyboard access — 2026-10-04 UTC

- State: **implemented-unqualified** bounded subticket on `ticket/m1-03-focus`,
  based on LOCAL main `c937670`, not origin/main. Assigned UI-first exception;
  no parent completion or qualification gate changed. Owns only `src/main.rs`,
  `src/operator.rs`, `src/tests.rs`, `scripts/operator-shell.py`,
  `docs/operator-shell.md` and this appended session record. No storage changes.
- Retained eight control focus handles, semantic contextual Tab/Shift+Tab and
  Enter/Space actions, click focus, wrapping traversal, distinct focus background
  versus selected-tab underline. Hidden tabs leave the rendered traversal tree;
  collapse/reset focus surviving controls. Root Ctrl+Q preserved from children.
  No disconnected live handlers or invented safety/navigation bindings.
- Reversible Sela accessibility policy, not installed EasyWorship observation.
  EasyWorship 1:1 target/neutral styling/geometry retained; no Painter. Reviewed
  current Zed HEAD (`git ls-remote https://github.com/zed-industries/zed HEAD`)
  matching pin `a84689073d296dfd39987bc7dd478e43ef76d83a`; Apache GPUI example
  `crates/gpui/examples/tab_stop.rs` and window/div/interactive focus APIs listed
  in shell note. No GPL application UI copied. Explicitly rejects synthesized
  keyboard clicks so semantic keys cannot double-activate or bypass bindings.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --all-targets -j 4`: **42 passed**, seven actual operator
  tree/input tests. `cargo clippy --locked --all-targets -j 4 -- -D warnings`,
  `cargo fmt --all -- --check`, `uvx ruff check scripts/operator-shell.py`,
  `git diff --check`: passed. `cargo fmt --all` / `uvx ruff format
  scripts/operator-shell.py` output reread. Initial new exact-child drag-focus
  assertion failed: GPUI defaults refocused the tracked ancestor on mouse down.
  Suppressed only splitter default focus, retaining both prior root and child;
  reran all tests with exact focus plus containment assertions, no weakening.
- Native: own `cargo build --locked --bin sela -j 4` immediately before
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null CARGO_TARGET_DIR=/home/user/workspace/repo/target
  python3 scripts/operator-shell.py`; passed, including native focus/selection
  pixel assertions, size, focus, survival and Ctrl+Q clean exit. Then
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh
  /home/user/workspace/repo/target/debug/sela`: passed input/resize/Ctrl+Q/WM-close.
  Debian12 x64 debug/Xvfb/Openbox/xcompmgr software GL; services left running.
- Inspected actual `.amp/in/artifacts/operator-shell/` captures:
  `focus-scriptures-unselected.png` separates focus from Songs selection;
  `focus-media-selected.png` proves Space-selected Media; `keyboard-collapsed.png`
  has focused Restore and no tabs (Enter did not toggle twice);
  `keyboard-reset.png` restores Resources while retaining Media selection.
  Artifacts are ignored local evidence, not physical/accessibility qualification.
- Not run: installed 8.0.49, Windows/UIA/Narrator/screen-reader, mixed DPI,
  physical GPU/displays or performance; orb software display cannot qualify
  them. Text-input/modal/multi-selection/live-navigation ownership remains future
  work, not invented here. Backlog/plan intentionally untouched by ownership.
- Delivery: local `feat(M1-03a)` subticket commit only, no push/publication.
  Next: integrator merge and rerun combined checks; obtain authorized Windows
  reference/accessibility environment before claiming focus compatibility.

### M1-01a / M1-03a — Parent review and merged verification — 2026-10-04 UTC

- Merged both worktrees with separate local merge commits, retaining subticket
  commits. Only appended work-log conflict; both records preserved. Reviewed
  storage API/schema/migration/worker/test implementation and actual focus diff.
- Strengthened snapshot regression with distinct lyrics under duplicate titles;
  ordered B/A/A content and revisions must survive edit/delete/reopen. Identical
  payload fixtures could not detect accidental reordered song resolution.
- Combined `cargo test --locked --all-targets -j 8`: **50 passed**; strict
  all-target Clippy, fmt and bin build passed. Native shell driver and native-smoke
  rerun from main with `DISPLAY=:99 VK_DRIVER_FILES=/dev/null`: passed focus/
  selection pixel assertions, input, resize, keyboard and WM exits.
- Parent inspected four actual keyboard captures: unselected Scriptures focus,
  Media activation, collapsed Restore focus, reset with retained Media selection.
  Existing artifact directory under main contains reproducible native evidence.
- No direct DB/file work added to UI; storage is not yet connected. Domain
  payload bounds and transaction outcomes reviewed; initial schema only, no
  destructive upgrade or host-disk/power-loss qualification claimed.
- Next M1-05a scope: persistent native field entities using Apache GPUI input
  callbacks, Unicode/grapheme selection, bounded edits, clipboard/IME, multiline
  lyric layout/scroll/caret. Then wire editor save/load through the existing
  bounded storage Worker. No synthetic key-to-string web substitute or Painter.
- Upstream input example inspected through Librarian: single-line only; native
  UTF16 ranges and marked replacement require careful conversion. Existing
  example preedit selection math must not be copied unchanged. GPUI API paths,
  ownership and licensing must be documented with the implementation.
- Local checkpoint only; no push/install/release claim. Windows/reference gates
  remain open; they do not stop approved provisional operator implementation.

### M1-05a — Native text-entry component — 2026-10-04 UTC

- State: **implemented-unqualified**, assigned large bounded component slice on
  `ticket/m1-05-input` in `/home/user/workspace/sela-input`, based on LOCAL main
  `3523641`, not origin/main. Read guidance/plan/backlog/work log; owner's UI-first
  prerequisite approval applies. No Painter, parent M1-05 completion or waived
  physical/reference gates. No main/operator/storage/lib/backlog/plan/README edits.
- Owns `src/text_input.rs`, permanent native `examples/input_check.rs`,
  `docs/text-input.md`, `LICENSE-GPUI-APACHE`, direct pinned unicode-segmentation
  manifest/root lock entry and this appended record. No dependency upgrades;
  1.13.3 was already locked. Original buffer/history/logical-row layout; Apache
  GPUI native handler/custom Element scaffolding attribution/modifications retained.
- Personally read pinned/current Zed a846890 GPUI examples/input.rs/tab_stop.rs,
  src/input.rs/text_system.rs/window.rs; current HEAD verified with
  `git ls-remote https://github.com/zed-industries/zed HEAD`. No GPL editor/ui code
  or assets copied. API, policies and license boundary in text-input note.
- Retained Focusable fields; validated atomic load; explicit errors/counter;
  UTF8/UTF16 clamp/reject policy, fixed composition-relative endpoints,
  marked underline/unmark notification, anchor/head direction, grapheme movement/
  collapsed deletion, copy/cut/paste, contextual Enter without root activation,
  bounded snapshots, real multiline logical rows including trailing empty rows,
  horizontal/vertical clipped scrolling, caret reveal and scroll-aware IME/hit
  geometry. No storage/network. Single-line rejects rather than flattens newlines;
  original Unicode/LF/CRLF preserved, new Enter inserts LF.
- Verification (`CARGO_TARGET_DIR=/home/user/workspace/repo/target`):
  `cargo test --locked --all-targets -j4`: **58 passed**, 8 new module tests,
  zero failed/ignored; `cargo clippy --locked --all-targets -j4 -- -D warnings`,
  `cargo build --locked --example input_check -j4`,
  `cargo fmt --all -- --check`, `git diff --check`: passed, repeated after final
  cut/resize fixes. Formatted code reread. `cmp LICENSE-GPUI-APACHE` against pinned
  upstream license passed. Initial imported GPUI test macro recursion and paint
  borrow errors corrected; no suppressions or skipped checks.
- Tests: nonzero-prefix surrogate+combining IME relative selection and explicit
  precedence; malformed range/selection atomicity and surrogate clamp; byte budget
  on both sides with prefix/suffix; line/history count+byte limits; grapheme deletion
  including scalar-interior caret; CRLF/blank/trailing rows; native callbacks for
  scrolled hit/range geometry and trailing empty caret; reversed selection/vertical
  movement; load rollback; clipboard newline policy and real parent action leak
  negative. Headless callbacks are not an actual IME service session.
- Native: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null` bounded inline Python subprocess/
  xdotool driver against shared target's `debug/examples/input_check`; full run
  passed PID-focus, typing/Tab/Enter, clipboard/errors, undo/redo, horizontal/
  vertical scroll, scrolled edit, mouse drag, WM close exit 0. Final binary repeat
  passed native cut/paste Unicode roundtrip, undo/redo, 420×440 resize/selection,
  close exit 0. Private temp runtime/root captures removed, owned apps reaped;
  existing Xvfb/Openbox services untouched. First driver failed only because it
  wrongly required Alt+F4 survival; corrected exit assertion then reran.
- Inspected actual local ignored `.amp/in/artifacts/input-check/` initial, newline/
  byte-error, redo, horizontal-scroll, scrolled-edit, mouse-selection, cut-empty,
  cut-paste-roundtrip and compact-selection PNGs: readable, correct content/error,
  clipboard preserves 47-byte Unicode/CRLF fixture, empty/trailing caret, clipped
  long text, revealed edited row, selection aligned to visible rows. Summary
  JSON separates asserted focus/exit from visual content evidence. Exact manual
  replay and binary commands are in text-input note; no scratch driver retained.
- Not run: actual platform IME/dead keys/CJK/dictation, Windows/macOS/Wayland,
  screen reader/accessibility/mixed DPI/bidi geometry, physical GPU/reference,
  measured performance/RSS or editor DB/save/close integration. No qualification,
  performance or parity claim. Snapshot composition is not coalesced; no sticky
  vertical column, blinking caret, word/double-click selection or soft wrap claim.
- Next: parent imports module and retains fields, registers contextual bindings,
  observes counter/text/error, validates combined storage envelope and owns save/
  close Worker flow. Rerun combined checks/native fixture; execute actual platform
  IME/accessibility and installed reference checks on authorized native runners.
- Delivery: coherent local `feat(M1-05a)` subticket commit only; no push/publication.

### M1-05b — Persistent native song authoring — 2026-10-04 UTC

- Implemented-unqualified. Original separate authoring window launched from
  Songs or explicit `--library` profile, retained native fields, labeled sections,
  create/edit/duplicate/tombstone delete, validation and bounded ID-paged catalog.
  SQLite and parent-directory creation stay on the bounded storage worker.
  Inline dirty/pending-close guards cover Ctrl+Q and WM close; operator Quit now
  closes only itself, never bypassing another editor's unsaved guard.
- Immutable expected-head saves preserve concurrent-writer conflicts; late
  replies cannot replace newer edits because busy controls are temporarily hidden.
  Incompatible stored metadata/line counts reject before changing any field.
  Documented original provisional UX and missing full-document undo; no live
  connection, installed-reference parity, release, or performance claim.
- Upstream GPUI close/focus/spawn contracts and licensing references are in
  `docs/song-library.md`; no Painter or Zed application components used.
- `cargo test --locked --all-targets -j 8`: **70 passed**, zero failed/ignored
  (includes shared input tests in both app/example). Strict all-target Clippy,
  fmt and app/all-target build passed. Added tests check different sections,
  Unicode, pending/dirty close, duplicate/delete history, stale write retention,
  atomic incompatible load, 128-entry catalog boundary/current revision/tombstone,
  and worker parent creation/failure without replacing existing bytes.
- Native commands, serial on existing Xvfb/Openbox/software GL:
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/song-library.py`,
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/operator-shell.py`,
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh "$PWD/target/debug/sela"`:
  all passed. Song driver independently decodes SQLite bytes after real native
  title/metadata/two-section multiline entry; verifies blank-title rejection,
  dirty WM close, keep-editing, discard-close, same-profile restart and clean exit.
  Extended shell check opens the editor by keyboard, types a draft, closes only
  the operator, asserts the editor survives, and exercises its dirty-close guard
  before final exit. Its profile is disposable; no real default data is touched.
- Inspected actual `.amp/in/artifacts/song-library/` saved/reopened-chorus,
  validation, unsaved-close, keep-editing, compact and compact-scrolled captures.
  Metadata and asymmetric lyrics match; controls fit at 720×440; scrolling exposes
  section/lyrics without overlap. Content crossing scroll viewport edges is
  intentionally clipped. Status validation is inline, not a native popup.
- Initial integration-test macro import/borrow and strict lint errors corrected.
  First extended native driver assumed focus stayed on a removed Keep-editing
  button; screenshot showed Title focus restored. Corrected test to focus Title
  and traverse both close choices; full repeat passed. No failure suppressed.
  The multi-window driver's first fixed launcher coordinate missed after a
  splitter drag; using the real tab order exercised the launcher reliably.
- Still unrun: Windows install/IME/accessibility/high-DPI, physical GPU/output,
  installed EasyWorship editor comparison and measured performance. Catalog is
  not search, editor is not a service scheduler; backups/autosave remain open.
  Next: developer-local install and launch/save/reopen of the installed binary,
  then arrangement/pagination or resource-search prerequisites. Commit is local;
  no push/publication authorized.

### M1-16a — Locally installed Linux developer preview — 2026-10-04 UTC

- Partial prerequisite under the owner's request for installable/useful progress;
  does not close M1-16's Windows scope, M1-12 dependency, M0-10 or Sunday gate.
  Source app remains the M1-05b technical preview. Added reproducible README
  instructions, ignored generated Cargo install root, and a foreground orb
  Desktop launcher which preserves supplied display/Wayland/audio variables.
- `cargo install --path . --locked --offline --debug --bin sela --root "$PWD/.amp/install" -j 8`:
  passed without sudo using cached dependencies. Installed executable reports
  `Sela 0.1.0 (technical preview)`; `ldd` output asserted to contain no `not found`.
  SHA256: `6638e4f6dbde5ef9de37beeddbe855d32e5030e3ba28a554fd6038b79069c2dd`.
  This is a local unoptimized ELF needing setup libraries/fonts, not a portable,
  signed, published or Windows package. Initial uncached setup requires network.
- `SELA_BINARY="$PWD/.amp/install/bin/sela" DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/song-library.py`:
  passed complete native input/independent SQLite/save/reopen/dirty-close/resize
  flow on the installed executable. Inspected its actual reopened-chorus capture:
  all original metadata, Chorus section identity and both distinct lines present,
  readable with no overlap. Same retained native artifact path as M1-05b.
- `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh "$PWD/.amp/install/bin/sela"`
  and `DISPLAY=:99 scripts/native-smoke.sh "$PWD/scripts/run-orb-preview.sh"`:
  both passed input/focus/resize, Ctrl+Q and WM close, clean exit/window removal.
  `bash -n scripts/run-orb-preview.sh`, `shellcheck scripts/run-orb-preview.sh`,
  and launcher `--version`: passed. Launcher deliberately uses orb software GL;
  it is not a recommendation for untested physical hardware.
- Bounded inline Python executed two additional disposable-root cycles of the
  same offline Cargo install command, installed `--version`, then
  `cargo uninstall --root <temporary-root>/installed sela`. Binary removed each
  time and a sibling original SQLite retention fixture's SHA256 stayed identical.
  Temporary roots removed; main `.amp/install/bin/sela` remains installed. This
  verifies filesystem retention only, not an actual Sela schema upgrade or S08.
- Unrun: Windows/macOS installer, clean-machine dependency packaging, production
  upgrade/rollback/signing, real platform IME/accessibility and physical output.
  No performance or complete parity claim. Song authoring is usable; schedule,
  arrangement/theme authoring, connected live output and recovery remain open.
- Next concrete implementation: M1-05c bounded whole-document undo for section
  changes, preserving field editing and saved revision/dirty invariants; inspect
  reference semantics when access is available. Then M1-06 arrangement/pagination.
  Local per-subticket commits only; no push, publication or deployment authorized.

### M1-05c — Bounded whole-document song undo/redo — 2026-10-04 UTC

- State: **implemented-unqualified**, assigned worktree `/home/user/workspace/sela-undo`
  on `ticket/m1-05-undo`, based on LOCAL main `2b62c77`, not origin/main. Read
  AGENTS/plan/backlog/work log first; owner authorized this non-layout functionality.
  Owns song_library/text_input, their notes, native song driver and this appended
  record only. No Painter, storage/model/renderer/main/Cargo/backlog or header/index
  changes; no new visible controls or reference-parity claim.
- One chronological whole-Song/section snapshot owner: 64 retained snapshots and
  8 MiB accounted bytes combined across undo/redo. Metadata/lyrics/native preedit
  and Add/Remove share the timeline; navigation/selection/no-ops leave it alone.
  New edits truncate redo. Original Unicode/emoji/combining/LF/CRLF preserved.
  Post-Add validation makes structural payload failures atomic; cap/no-op actions
  no longer reload fields. Current/baseline/allocator/transient memory is outside
  retention accounting; no measured RSS/latency qualification.
- Focused synchronous content callback seam avoids deferred/coalesced observer
  history and stale section association, with no field read during its borrow.
  Only owned fields disable local snapshot recording and bubble semantic Undo/
  Redo; standalone component behavior remains covered. WeakEntity lifetime and
  contextual actions follow inspected Apache GPUI APIs at current/pinned Zed
  `a84689073d296dfd39987bc7dd478e43ef76d83a`; exact paths/provenance in owner note.
- Save/Duplicate retain both stacks and selected section, advancing baseline and
  immutable Version only on successful receipt. Undo after Save is dirty; redo to
  baseline is clean. Save with a redo branch retains that branch. Busy undo/redo
  ignored. New/successful Load/Discard/successful Delete reset history. Failed
  validation/input/structural edits/submission/conflict/load retain history and
  durable state. Dirty/pending close guards unchanged. Carets/composition reset
  on document restoration; no selection restoration or edit coalescing claim.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --all-targets -j2`: **76 passed**, zero failed/ignored;
  `cargo clippy --locked --all-targets -j2 -- -D warnings`,
  `cargo fmt --all -- --check`, `uvx ruff check scripts/song-library.py`,
  Python AST parse and `git diff --check`: passed. Formatted/linter output reread.
  New tests cover cross-navigation asymmetric edits/add/remove/zero sections,
  Unicode, save/duplicate/redo boundaries, New/Load/Discard/Delete reset,
  invalid-load redo retention, dirty close, pending-ignore, conflict/unavailable/
  validation save failure, rejected native edits, 128-section/encoded-size caps,
  count/combined-byte eviction and synchronous content-only callbacks.
- Initial new tests had owned-index move/borrow compile errors (fixed with cloned
  expected values/focus handles). Zero-section direct helper kept focus on the
  removed lyric entity, unlike real Remove-button invocation; corrected the test
  to exercise retained control focus and reran. Clippy collapsed nested if and
  malformed-range fixture construction corrected; Ruff import/executable mode/
  explicit expected search failure check fixed. Final checks passed; no failures
  suppressed or native checks represented as executed.
- Native focus-changing checks intentionally **not run** on shared `:99` while
  parallel work proceeds. Extended `scripts/song-library.py` is lint/parse-checked
  but native replay pending: after parent merge, build merged bin with shared
  target and `-j2`, run serially with `SELA_BINARY="$CARGO_TARGET_DIR/debug/sela"
  DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/song-library.py`.
  Driver asserts save-undo dirty WM close and independent SQLite one-section
  removal/two-section undo restoration. Inspect saved-document-undo, removed-
  section, restored-section and existing compact/reopen PNGs. Owner note contains
  exact replay and manual navigation/branch/final-section checks.
- Remaining: native replay above, installed 8.0.49 semantics, Windows/platform IME/
  accessibility/hardware and measured performance. Parent consolidated checklists
  remain untouched. Next: integrate local commit and execute combined serial native
  replay; then continue arrangement/pagination without claiming parity or waiving
  qualification. Local coherent subticket commit only; no push/publication.

### M1-01b — Verified database backup / restore prerequisite — 2026-10-04 UTC

- **Implemented-unqualified** on `ticket/m1-01-backup`, independent worktree
  `/home/user/workspace/sela-backup`, based on LOCAL main `2b62c77`, not origin.
  Read guidance, plan/backlog/work log. Owned only storage implementation/tests,
  storage contract, existing rusqlite feature and this appended record; no UI,
  domain module/backlog/index/header edits, new crates or dependency upgrades.
- Added `BackupNew` / `RestoreNew` and `Copied` on capacity-one Worker;
  read-only restore input, fresh destination, no current-profile switching,
  online SQLite consistent committed snapshot including WAL. Private same-parent
  staging, supported-schema/quick-check/FK/history verification, explicit close
  and file sync precede atomic no-overwrite hard-link publication. Source and
  existing destination/aliases/SQLite sidecars are never overwritten.
- Budgets: 64 pages/step, 256MiB logical DB, cooperative five seconds,
  100ms lock timeout, 2MiB per connection cache target, 4096-byte paths,
  historical decode one bounded song/schedule at a time. No hard RSS/disk/OS-I/O
  deadline. `Locked` fails rather than indefinite retries. Copy cancellation
  differs from writes: sampled between steps/rows and before publication;
  cancellation that loses publication race reports actual outcome. Drop detaches.
- Exact provenance and scoped checklist in `docs/storage.md`: SQLite backup.html,
  c3ref/backup_finish.html, WAL sections 2–4 and pinned rusqlite 0.40.2 local
  src/backup.rs. Enabled backup feature only; Cargo.lock unchanged.
- Executed with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --lib storage::tests -j2`: **14 passed**;
  `cargo test --locked --all-targets -j2`: **74 passed**, zero failed/ignored;
  `cargo clippy --locked --all-targets -j2 -- -D warnings`,
  `cargo fmt --all -- --check`, `git diff --check`: passed. Initial Clippy
  single-element test-loop lint corrected without suppression; checks repeated.
- Four new executed tests cover Unicode original/edit/tombstone revisions and
  asymmetric B,A,B schedule backup/reopen/worker restore on live WAL; competing
  uncommitted writer/exclusive lock; content-corrupt verification failure;
  corrupt/newer restore input byte retention; source/destination/hardlink/sidecar
  collision and cancellation; bounded worker saturation. Actual child process
  abort after partial online-backup step leaves no output and unchanged source.
  Separate actual child abort after committed song/schedule plus cache-spilled
  uncommitted head/revision/item changes reopens with committed song and ordered
  schedule intact and uncommitted revision absent. Disposable data only.
- No power-loss guarantee: directory metadata is not synced; hard-link outcome
  is authoritative, cleanup best effort. Abort can leave private abandoned staging;
  post-publication/pre-ack process death means caller outcome unknown, inspect
  destination rather than overwrite. Trusted local directory/hard-link-capable
  filesystem required; hostile directory/sidecar races excluded. Windows ACLs,
  filesystem qualification, sync/volume failure injection, continuous-writer
  restart stress and measured large-library budgets remain unexecuted.
- Schema 1 has no actual destructive upgrade: its verified-backup-before-mutation
  migration gate remains open, not simulated or silently marked done. Parent
  M1-01/M1-12/M1-13 integration, rotation, recovery UI and portable assets open.
  Next: parent merge/consolidate checklist; future concrete migration must consume
  verified backup acknowledgment before mutating and execute its own rollback
  checks. Local per-subticket commit only; no push/publication/deployment.

### M1-06a — Bounded arrangement domain — 2026-10-04 UTC

- State: **implemented-unqualified** assigned parallel non-UI prerequisite on
  `ticket/m1-06-arrangement`, `/home/user/workspace/sela-arrangement`, from LOCAL
  main `2b62c77`, not origin/main. Read guidance/plan/backlog/work-log. Only
  `src/arrangement.rs`, library export, `docs/arrangement.md` and this append;
  no persisted Song/Section/schema, editor, Cargo, UI or renderer changes.
- Contract: explicit typed section/variant/occurrence IDs reuse opaque 128-bit
  storage Id vocabulary. Occurrence scope is variant-local; selection uses the
  full version/variant/occurrence tuple. Owned immutable revision snapshot shared
  by Arc; repeated occurrences borrow identical section content, never head data.
  Pure named-variant/edit/reorder/remove/retarget operations are all-or-error;
  final-position move and empty draft policy explicit. Limits/reference/name/ID
  validation and future non-destructive ID migration obligations in arrangement
  note. No legacy label/index ID conversion or provenance verification claim.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --lib -j2`: **33 passed**, including five new arrangement
  tests; `cargo clippy --locked --lib --tests -j2 -- -D warnings`,
  `cargo fmt --all -- --check`, `git diff --check`: passed. Formatter output
  reread. Initial shared-target reuse returned the baseline's 28 tests (zero
  arrangement tests); detected via filtered listing, touched this worktree's
  source timestamps and reran, observing actual compilation and all five tests.
  Only the recompiled 33-test result is arrangement evidence.
- U cases: scrambled source order, duplicate labels/distinct IDs, asymmetric
  V1/C/V2/C/C, pointer-shared choruses/distinct occurrences, both move directions,
  retarget/remove/insert and independent variants; missing/deleted references and
  internal final-reference corruption give no partial result; rejected edits
  retain original; empty policy, lossless Unicode/CRLF, exact source/arrangement
  byte boundaries, name bytes, section/variant/occurrence count boundaries.
- Not run/claimed: native arrangement E2E, Windows/reference/physical rendering,
  layout/pagination/font fitting, performance/RSS or persistence migration.
  There is no UI or persisted adapter to exercise; no GPUI pattern added.
  Parent M1-06 remains open. Source revision/content provenance is the future
  repository adapter's obligation; domain payload budgets are not hard RSS caps.
- Next: parent merge export/appended record, rerun combined checks with observed
  test names (shared target can reuse another worktree artifact); settle storage
  ID/arrangement migration and editor selection/undo before wiring controls.
  Local coherent M1-06a subticket commit only; no push/publication/deployment.

### M0-06b — Bounded native static-color cue pipe diagnostic — 2026-10-04 UTC

- Owner-authorized parallel worktree `/home/user/workspace/sela-native-cues`,
  branch `ticket/m0-06-native-cues`, based on **local main `2b62c77`**, not remote
  main. Read plan/backlog/work log and preparation/delivery/output/composition
  notes. Scope is backend diagnostic only; no Painter, main/operator layout,
  storage, manifest/lockfile or reference safety-policy changes. Scoped checklist,
  schema, API/license provenance and remaining work appended in `docs/delivery.md`.
- Implemented-unqualified static-color fallback, not full text/image delivery:
  fixed version-1 73-byte-max owned frames, no Rust memory layouts or file refs,
  worker-only stdin/stdout I/O, inbound 2/outbound 4 inline frame capacities;
  reconstruct immutable validated opaque-color cues, renderer-owned Ready after
  native surface/device setup, epoch/sequence/lane enforcement and preparation
  rejection without replacing prior scene. Native commit encodes a GPU pass,
  calls queue.submit and native present **before Applied**; no GPU wait/readback
  or pipe/file/decode/raster/logging in the frame callback. No Applied is emitted
  by non-GPU process test fixtures. No main UI controls are wired.
- Renderer-local receipt-read timestamp preserves inbound queue expiration;
  budgets are 1–5000ms, not synchronized process clocks or hard pipe-transit
  expiry. Controller timeout/disconnect remains Unknown and disables delivery;
  old child must be retired/reaped before a fresh epoch, with no automatic replay.
  Example normally closes at 24s; independent process-exit watchdog caps even
  stuck startup at 25s. Driver bounds startup/receipts/capture/retirement, escalates
  only owned children, records receipts and checks native cropped pixels.
- Checks with shared `CARGO_TARGET_DIR=/home/user/workspace/repo/target`, `-j2`:
  `cargo test --locked --all-targets -j2` **79 passed, 1 ignored child fixture**;
  three integration tests invoke that fixture as actual Rust child processes.
  Covers exchange/reject, independent renderer lane saturation/FIFO, sequence and
  epoch rejection, real killed/stalled child disconnect, exact injected deadline,
  fresh unknown/no replay. Six transport unit tests include all truncated frame
  lengths, malformed/oversize header/body fields, full owned image/font rejection
  instead of silent stripping, inbound queue expiry, prior-snapshot retention,
  receipt round trips and 1000 nonblocking writer-overload attempts. Existing
  Delivery/Preparer failure tests also passed unchanged.
- `cargo clippy --locked --all-targets -j2 -- -D warnings`,
  `cargo fmt --all -- --check`,
  `cargo build --locked --example native_cues -j2`, Python AST parse and
  `git diff --check`: passed. Initial alpha-malformation assertion accidentally
  rewrote 255 with 255; corrected the test to alpha 0 and reran. An intermediate
  shared-target build reused another worktree's Sela rlib despite correct local
  sources; touching local `src/lib.rs` forced a root-crate rebuild (no
  content or dependency change). Final all-target compile/tests/clippy passed
  together; parent should serialize merged Cargo/native qualification runs.
- Provisioned only own supervised `sela-cue-x11` Xvfb **:101**, 1024×768 (no WM
  or compositor), never touched reserved :99/focus. Executed twice:
  `DISPLAY=:101 VK_DRIVER_FILES=/dev/null CARGO_TARGET_DIR=/home/user/workspace/repo/target python3 scripts/native-cues.py`
  with `--out .amp/in/artifacts/native-cues` then
  `--out .amp/in/artifacts/native-cues-final`: passed. GL adapter startup logs
  identify Mesa22.3.6 llvmpipe LLVM15.0.6 CPU renderer. Final summary ties eight
  client-only PNGs to receipt sequences/timestamps and independently checks three
  interior RGB samples each. Red/green apply, invalid-alpha/extent failure retain
  red, stale sequence retains green, currently applied duplicate doesn't replace,
  retired epoch rejects and restarted output remains unconfirmed until a fresh
  explicit cue. Inspected actual red/retention/green/unconfirmed captures. This
  is software virtual native evidence, **not physical scanout/GPU completion**.
- Negative driver checks (missing DISPLAY/binary, invalid backend) and missing
  audience CLI args returned nonzero. Separate inline subprocess check left stdin
  open without commands: Ready arrived, standalone audience exited cleanly after
  **24.060s** without supervisor EOF. No stuck-driver watchdog fault injection,
  physical/Windows/macOS/reference checks, performance/RSS/latency budgets or
  operator focus regression executed; parent owns serialized post-merge native
  checks. Own display service stopped after checks; no push/publication.
- Next: versioned owned text/font/image schema with total/per-field budgets,
  renderer preparation/upload worker and GPU-ready ownership separated from
  offscreen readback, then receipt-correlated asymmetric resource failure tests.
  Preparation-intent coordinator, durable epochs, production supervision and
  device-loss handling remain open. Static-color slice is implemented-unqualified;
  M0-06/M0-07/M0-10 are not done and no Black/Clear/Logo semantics were invented.

### M1-02c — Reference-directed toolbar correction — 2026-10-04 UTC

- Implemented-unqualified on `ticket/m1-ui-reference`, LOCAL main `231fffb`
  (all four non-UI merges), not origin/main. Read instructions/plan/backlog/log.
  Owner explicitly requested actual UI correction; no Painter, dependency,
  storage, arrangement or renderer changes. Scoped checklist/provenance in
  `docs/operator-shell.md`, documented-only evidence D-UI-01 in reference ledger.
- Personally fetched/inspected official toolbar image: exact left/right action
  order, right group above Live, no bottom live controls or invented output
  handlers. Its build is unknown, not verified 8.0.49. Original clean GPUI chrome,
  preserved pane/splitter geometry; Reset/Quit moved to Resources chrome.
- Real New menu and bottom Songs + open blank authoring; companion M1-05d follows.
  New/menu join semantic traversal, menu overlays without reflow. Fixed removed
  menu-item focus after native replay caught failed operator close. No assertion
  dropped. Disabled output controls remain inert, no color-diagnostic wiring.
- Combined slice checks: `CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo
  test --locked --all-targets -j4`: **96 passed, 1 ignored child fixture** (three
  process tests invoke it); new test names actually observed. Root sources touched
  before initial build to defeat shared-worktree stale cache. Strict all-target
  Clippy (`-D warnings`), fmt, Ruff both drivers, diff check and bin build passed.
- Serial `DISPLAY=:99 VK_DRIVER_FILES=/dev/null CARGO_TARGET_DIR=/home/user/workspace/repo/target
  python3 scripts/operator-shell.py`, song-library driver and native-smoke against
  shared target binary passed. X11 services untouched. Actual normal/New menu/
  compact/focus/collapse/reset/drag/editor-guard PNGs inspected under
  `.amp/in/artifacts/operator-shell/`; screenshots are ignored local evidence.
- Windows/installed8.0.49/accessibility/DPI/physical GPU/display/performance unrun:
  no suitable reference/hardware environment; no parity or release qualification.
  Local subticket commit only, no push. Next: retain companion editor correction,
  parent reviews/reruns integration and consolidates backlog; exact reference
  interaction and audience qualification gates remain open.

### M1-05d — Reference-directed song editor correction — 2026-10-04 UTC

- Implemented-unqualified companion on `ticket/m1-ui-reference`, LOCAL main
  `231fffb`, retaining M1-05c chronological document history and all four non-UI
  merges. Owns song_library/source tests, native song driver, owner note, ledger
  and appended record only. No storage/arrangement/transport/scene/Cargo/native
  diagnostic/backlog changes; root-source touches only forced correct cache build.
- Personally read official Support7 articles (2024-09-03 and 2023-01-05) and
  inspected working historical editor screenshot. D-UI-02 explicitly documents
  those sources, not an installed8.0.49 finding. Apache GPUI upstream commit,
  inspected paths and license recorded; original UI/fixture text, no Painter.
- Title/Words/Slides/section selector left, draft preview or Inspector metadata
  right, Add/Remove bottom-left and OK/Cancel bottom-right. Explicit Library
  selector retains bounded worker catalog/loading/paging outside Words; existing
  New/Save/Duplicate/Delete/Discard/Previous/Next reachable. Compact Inspector
  clipping caught by visual review: omit redundant header status below900px,
  keep truthful status below toolbar; fixed footer and scrolling preserve access.
- Slides are four-line original text thumbnails; preview is logical draft text,
  not audience WYSIWYG/pagination/themes/arrangement or matching output. No fake
  unsupported formatting/media/output features. Native field entities retained.
- OK waits for committed Saved receipt, not submission; validation/conflict/error
  keep draft open; Cancel/WM/Ctrl+Q retain existing pending/dirty guards. Ordinary
  Save advances only durable baseline/Version and retains selection/history.
  New test exercises mode/navigation history invariants, invalid/conflicting OK
  and pending Cancel. Old history/failure tests remain, revealing Inspector
  before metadata-field focus instead of typing into a removed focus subtree.
- Final `CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo test --locked
  --all-targets -j4`: **96 passed, 1 ignored child fixture**, new actual test names
  and all five arrangement tests observed. Strict all-target Clippy `-D warnings`,
  fmt, Ruff both drivers, diff check, locked bin build passed. Serial native
  song-library/operator-shell drivers and native-smoke passed with DISPLAY=:99,
  VK_DRIVER_FILES=/dev/null and the same shared target. Shared services untouched.
- Native independent SQLite decoding verifies metadata/two asymmetric sections,
  save-undo dirty close, retained committed bytes on discard, structural undo
  across saves, reopen and actual OK-save/close; invalid OK also asserts survival
  and zero commits. This executes the previously pending M1-05c native replay.
  Coordinates were updated, not payload/undo/failure assertions removed.
- Inspected actual ignored `.amp/in/artifacts/song-library/` initial/draft/Slides,
  Inspector-filled, validation/validation-ok, unsaved-close, restored-section,
  reopened-chorus, compact/compact-inspector/compact-library-selector/scrolled
  PNGs, plus operator's independent editor-close guard. Original native crops,
  not reference/composited images. Commands and scoped checklist in owner note.
- Remaining: lawful installed8.0.49 behavior, Windows/real IME/accessibility/DPI,
  hardware/audience output and performance unavailable/unrun, no parity/release
  claim. Next: parent reviews these two local commits, reruns merged integration,
  consolidates backlog and schedules required native/reference qualifications;
  unsupported authoring/arrangement/pagination remains open. No push/publication.

### M1-05d — Integration regression: whitespace-title navigation — 2026-10-04 UTC

- Parent review found section-row selection rejected a whitespace-only unsaved
  title, unlike Add/Previous/Next. Treat whitespace as blank only in the temporary
  size-validation copy; retain exact draft bytes and require a real title on save.
- Added an asymmetric two-section GPUI regression proving selection actually
  changes, correct field content loads, document/history remain unchanged, and
  OK still refuses persistence/close. After correcting a test-only borrow lifetime,
  `cargo test --locked --bin sela blank_title_section_navigation -j4` failed at
  section 1 versus expected 0 before the fix. The same test passes after the fix.
- `cargo test --locked --all-targets -j4`: **97 passed, 1 ignored subprocess
  fixture**, invoked by three process tests. Strict all-target Clippy `-D warnings`
  and `cargo fmt --all -- --check` passed. Updated song guide entry paths and OK,
  Cancel and backend-only backup limits. No visual geometry change in this fix.
- `cargo install --path . --locked --offline --debug --bin sela --root
  "$PWD/.amp/install" -j4` succeeded. Installed `--version` printed
  `Sela 0.1.0 (technical preview)`; `DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  scripts/native-smoke.sh "$PWD/.amp/install/bin/sela"` passed both Ctrl+Q and
  WM-close scenarios. Full merged native/visual evidence is recorded below.
- Local subticket fix, not a push or Windows qualification. Next: complete
  parent integration checkpoint and retain the remaining parent-ticket gates.

### M0-06b — Native driver executable metadata — 2026-10-04 UTC

- Parent's combined Ruff check found EXE001: the shebang-bearing native cue
  driver lacked executable permission. Set its tracked mode to 0755; no Python
  content changed. `scripts/native-cues.py --help` and `uvx ruff check
  scripts/operator-shell.py scripts/song-library.py scripts/native-cues.py` now
  pass; `git diff --check` passes. The full native cue replay had already passed
  via `python3`; the mode change only makes direct invocation consistent.
- Local M0-06b follow-up; parent integration evidence follows. No push.

### Parallel wave — Parent integration and installed preview — 2026-10-04 UTC

- Four independently owned non-UI worktrees plus the reference-directed UI
  worktree are locally merged with per-subticket commits intact. Parent inspected
  storage publication/validation, arrangement identity/bounds, transport/native
  submit/receipt ordering, document callbacks/history and toolbar/editor changes.
  Resolved append-only work-log conflicts without dropping worker records;
  retained both library module exports. Main checkout owns combined verification.
- Shared target reuse had been detected in worker runs. Touched merged root
  sources before rebuild and confirmed real compilation plus new test names;
  did not treat stale zero-test/filter output as verification. Final
  `cargo test --locked --all-targets -j4`: **97 passed, 1 ignored child fixture**
  (three process tests explicitly invoke it). Strict all-target Clippy, fmt,
  all-target build and `git diff --check` passed. Final Ruff across all three
  native Python drivers passed after the recorded executable-mode correction.
- Serial Linux X11/software GL commands with `DISPLAY=:99 VK_DRIVER_FILES=/dev/null`:
  `python3 scripts/operator-shell.py`, `python3 scripts/song-library.py`,
  `scripts/native-smoke.sh "$PWD/target/debug/sela"`, and
  `python3 scripts/native-cues.py --backend gl --out .amp/in/artifacts/native-cues`
  all passed. Existing shared Xvfb/Openbox/xcompmgr services were not restarted.
  Native cue summary binds receipt sequences to sampled cropped pixel colors;
  failed/stale/retired commands retain state and fresh startup does not replay.
- Reinstalled via the exact offline debug Cargo install command above. Installed
  smoke passed; `DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  SELA_BINARY="$PWD/.amp/install/bin/sela" python3 scripts/song-library.py` passed:
  actual validation, independent SQLite payload checks, two-section editing,
  save/document/structural undo, dirty-close, reopen, resize and OK commit-close.
- Parent inspected actual `.amp/in/artifacts/operator-shell/{normal,compact}.png`,
  `song-library/{draft,slides,compact-inspector,compact-scrolled}.png` and native
  cue `render-failure-retains-red.png`. Toolbar grouping/placement, Words/Slides,
  draft preview, Inspector and fixed footer are legible without overlap. Compact
  form crops at its scroll boundary; scrolling reveals lyrics while footer stays
  reachable. These are native software-rendered captures, not physical evidence.
- Updated backlog evidence and README entry paths. The arrangement domain is
  not persisted; backup is a worker API, not a recovery UI; native transport is
  opaque-color-only, not text/image output or Black/Clear/Logo implementation.
  Official marketing/Support screenshots are documented evidence, not installed
  8.0.49 verification. No indexed search, service scheduler or live lyric UI yet.
- Unrun: Windows/package installer, installed reference, physical GPU/projector,
  mixed DPI, IME/accessibility qualification, destructive migration, power-loss
  recovery, measured large-workload performance and long hardware soak. No access
  or implementation prerequisite for these checks was invented. No parent ticket
  or milestone gate was closed by this wave.
- Delivery: all work is committed locally; nothing pushed, published or deployed.
  Preview launcher remains `scripts/run-orb-preview.sh`. Resume the explicit-ID
  persistence/migration prerequisite and renderer resource transport separately;
  obtain Windows/reference access when the owner is available.

### Continuation scope — 2026-10-04 UTC

- Owner asks to continue feasible implementation and postpone final Windows/
  hardware proof. Resume two independent prerequisites from local main: storage
  explicit-ID/arrangement migration with verified backup, and owned font/text/
  image transport with native readiness separated from frame submission.
- Storage owns repository/domain/editor data round trips and their tests;
  renderer owns scene/transport/native examples and their tests. No Painter or
  live-control semantics change. Reference unknowns remain explicit.
- Each worktree records/commits its own bounded slices. Parent merges locally,
  reviews combined contracts and runs integration/native checks. Shared Cargo
  target must be serialized and root-source compilation observed to avoid the
  previously detected stale-worktree artifact reuse. No push authorized.

### M1-06b — Backed-up stable-ID/arrangement persistence — 2026-10-04 UTC

- **Implemented-unqualified** in `/home/user/workspace/sela-persistence`, branch
  `ticket/m1-06-persistence`, based on LOCAL main **f161dc3**, not origin/main.
  Read AGENTS/plan/backlog/work-log/arrangement/storage before implementation.
  Owned storage/song-library, their three notes, native song driver and this
  append only; central backlog/current-state consolidation remains parent-owned.
  No Painter, visual geometry, renderer/scene/transport/delivery/main/library
  export/example/Cargo/dependency or real-profile writes. No push/publication.
- Schema 2 adds revision-keyed section-ID mappings and ordered named variants/
  occurrences with FK/uniqueness checks. Original schema-1 payload codec remains
  exact; migration does not rewrite payloads, old schedules, heads or tombstones.
  Legacy IDs assigned once with SQLite randomblob per immutable revision, never
  inferred from labels/indices/text or claimed continuous across old revisions.
  New source edits preserve section IDs and variants. Historical arrangements
  and schedule snapshots remain exact after edits/tombstones. Late missing
  references reject all, never silently drop a chorus or return a partial variant.
- Opening legacy profile holds IMMEDIATE writer reservation across a separate
  read-only online backup and migration. `<profile>.schema1-backup` must be
  verified/synced/published fresh before any upgrade DDL/mapping. Verification
  uses non-upgrading open; backup/RestoreNew retain input schema 1 or 2. Migration
  DDL/mappings/user_version are one transaction. Backup conflict/corruption/lock
  blocks mutation. Failed/aborted migration retains verified backup; deterministic
  backup name intentionally blocks blind retry with Exists. Preserve both files
  and restore to a fresh profile for recovery; no overwrite/rotation/recovery UI.
- Existing Song is the editor/repository source of truth, now Section.id plus
  Song.variants. SaveSong/Song/Schedule worker round-trip all data; added
  Repository/worker Arrangement(v) returns truthful immutable source pairing.
  Pure Song adapter still requires caller provenance. New/Add CPU-only SHA-256
  clock/PID/counter allocator does not read files/random devices; no cryptographic
  uniqueness claim. Undo/load/save never regenerate IDs. Duplicate retains
  song-local document IDs under a fresh song ID. Initial baseline shares one
  blank allocation; history bytes include variants. Referenced Remove fails
  atomically with status, no new controls/focus/action mappings.
- Preserve maximum 256KiB unarranged legacy codec payload; arrangement creation
  still obeys existing stricter text-plus-ID domain budget and may reject such a
  document without losing text. Existing capacity-one worker and bounded reads
  remain; added schedule arrangement completion overhead documented. No hard
  RSS/migration deadline, measured performance, or physical/fsync qualification.
- Executed final checks with actual root compilation/new test names observed:
  `flock /tmp/sela-cargo-continuation.lock bash -c 'set -e; touch src/*.rs; export CARGO_TARGET_DIR=/home/user/workspace/repo/target; cargo test --locked --all-targets -j4; cargo clippy --locked --all-targets -j4 -- -D warnings; cargo fmt --all -- --check'`:
  **104 passed, zero failed, one ignored subprocess fixture explicitly
  invoked by three process tests**. Six new storage and one GPUI test: legacy WAL/
  per-revision ID reload/old 2,1,2 schedules; backup gate conflict/corrupt/real
  competing writer/DDL rollback; actual child SIGABRT after inserted mapping
  rolls back schema/rows and retains backup; exact legacy payload max; schema2
  V1/C/V2/C/C/reorder/source provenance/late reference and insert failures/
  tombstone/backup/restore/worker round trips; deliberate late corrupt reference
  and mapping position never truncate/publish; duplicate-label editor load/save/
  duplicate/undo/redo/new Add identity/referenced and unreferenced Remove.
  Existing storage crash/disk-full/worker/history and arrangement bounds pass.
  `uvx ruff check scripts/song-library.py`, Python AST parse and `git diff --check`
  passed. Formatter output and edited notes reread. Shared lock released outside
  Cargo; no other worktree's root artifact accepted as evidence.
- Initial compile used unsupported rusqlite usize FromSql (changed to checked
  contiguous i64 positions). Initial test assumed byte-identical whole SQLite
  backup header (corrected to schema/payload equivalence and immutable backup
  bytes after verification), DDL error classification corrected to Corrupt.
  Random New IDs exposed independent blank baseline allocation/old blank equality:
  share initial allocation and assert fresh blank content/ID instead. Maximum
  legacy test needed two sections/minimal metadata to exercise stricter domain
  bound. Deliberately corrupt external fixture now explicitly disables FK before
  injecting a broken late reference. No failed expectation suppressed.
- Native song driver retains original independent payload decoder and now checks
  schema2 distinct section IDs and exact retention across removal/undo/save.
  Lint/parse checked only: parent owns :99 focus replay, not run or disturbed here.
  After merge, parent must rebuild merged root under shared lock/touch and execute
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null SELA_BINARY=/home/user/workspace/repo/target/debug/sela CARGO_TARGET_DIR=/home/user/workspace/repo/target python3 scripts/song-library.py`
  plus combined native operator checks. Status error text is the only visible
  wording change; no visual controls/arrangement UX introduced.
- Windows/hardlink/ACL/installed8.0.49/IME/accessibility/physical display/GPU,
  real volume/power-loss faults, large-library measurements and recovery chooser
  remain unrun/open. Next concrete action: parent local merge/review/native replay
  and central checklist consolidation, then arrangement controls/explicit repair
  and pagination. Local M1-06b commit carries requested Amp-Thread-ID trailer;
  no parent ticket marked done or qualification waived.

### M0-06c — Owned resource wire checkpoint — 2026-10-04 UTC

- Resumed renderer continuation on `ticket/m0-06-resources` from parent LOCAL
  main f161dc3, not origin/main. Read engineering/plan/backlog/work-log,
  delivery/composition/preparation notes. No main/UI/storage/dependency edits.
- Added version-2 bounded font/UTF-8/normalized-RGBA transport and immutable
  worker-only owned reconstruction; v1 color/receipt/Ready remains compatible.
  Exact schema, per-field/aggregate and channel memory ceiling in delivery.md.
  Timestamp precedes v2 body transfer and survives queued reconstruction.
- Serialized shared-target Cargo under `/tmp/sela-cargo-continuation.lock`,
  touching root sources: `cargo test --locked --lib transport -j4` **6 passed**,
  including actual image/font byte round trips, truncated/oversized/trailing
  resource frames and existing 1000-attempt overload/deadline tests. Compilation
  of this worktree was observed. Cargo fmt executed; native helper slice remains
  unstaged for the next subticket. All-target attempt exposed native helper-only
  compile/dead-code issues, being corrected before M0-07d verification.
- State: implemented-unqualified transport checkpoint, not native resource proof.
  Next: bounded native worker raster/upload, submit-only bindings, asymmetric
  text/image retained-state replay; Windows/physical/reference proof stays open.

### M0-07d — Worker-prepared native text/image submission — 2026-10-04 UTC

- Added bounded native worker (one executing/one waiting/one completion), owned
  reconstruction/font shaping/raster/upload outside event callbacks and shared
  `ReadyComposition` bindings. Existing offscreen `render` still performs diagnostic
  readback; native `submit_native` never invokes it. Upload staging flush and <=2s
  GPU completion wait occur only on worker; upload failure retires, no retry loop.
- GPU-ready admission retains RendererSession FIFO/epoch/duplicate/deadline
  enforcement; Applied only follows Queue::submit/native notify/present. Added
  ordered Busy rejection for saturated preparation. Resource deadline includes
  body transfer, queue, preparation/upload and result wait. Valid resource-size
  framing allowance corrected to +160 bytes (image+text metadata requires 133).
- Exact final verification, under shared-target flock with touched sources and
  actual compilation: `cargo test --locked --all-targets -j4` **105 passed,
  1 ignored child fixture** (four process tests execute it); strict
  `cargo clippy --locked --all-targets -j4 -- -D warnings`,
  `cargo fmt --all -- --check`, `cargo build --locked --example native_cues -j4`,
  `uvx ruff check scripts/native-cues.py`, `git diff --check` all passed.
  Eight transport tests include source/text/image/aggregate byte boundaries,
  malformed/truncated/trailing framing, queued v2 expiry, exact owned font/image
  bytes and 1000 overload rejections retaining revision23. New real-child test
  checks actual UTF-8/font/asymmetric RGBA across pipes; no fake Applied there.
- `DISPLAY=:102 VK_DRIVER_FILES=/dev/null CARGO_TARGET_DIR=/home/user/workspace/repo/target
  python3 scripts/native-cues.py --backend gl --out
  .amp/in/artifacts/native-resources-final` passed twice during final development;
  last pass includes fatal-upload retirement code. Dedicated supervised
  `sela-resources-xvfb`, 1024×768, Mesa22.3.6/llvmpipe LLVM15 CPU GL, was stopped
  afterward; no shared :99 service/input/focus touched. Fifteen native client
  PNGs, receipt timestamps, pixel samples and full RGB hashes plus adapter logs
  persist in that ignored artifact directory. Actual text/image and expired
  retention captures inspected: readable Signal café/Beacon, six asymmetric tiles,
  no clipped glyph/row corruption. Driver checks six colors, real white coverage,
  identical retained RGB after missing glyph, overflow, invalid font, oversized
  text, stale sequence and 1ms expiry; retired resource epoch/startup no replay.
- State: implemented-unqualified diagnostic resource slice, not main operator
  wiring, installed-reference parity or physical output. Explicit channel/CPU/GPU
  ceilings and stricter native text limits/provenance in owner notes. Resize keeps
  owned resources but requires new-extent preparation; visible resized retention
  unqualified. Upload/device-loss/watchdog fault injection, measured memory/frame
  pacing, Windows/DX12/Metal, physical GPU/display, mixed DPI and reference remain
  unrun. No dependency/GPUI/Painter/storage/editor changes or push.
- Parent next: merge local resource subtickets, preserve append-only log records,
  consolidate central backlog/current state, touch merged root sources under
  flock and rerun integration plus serial native/operator regressions. Continue
  production intent/pre-resolved safety coordinator and resize preparation before
  live-control wiring; do not close parent M0 gates from software-native captures.

### M0-07d — Parent native capture correction — 2026-10-04 UTC

- Merged native replay on shared :99 with Openbox exposed a driver geometry bug:
  xdotool's reported origin shifted the crop downward by about 20px. The top-left
  expected red sample at (100,160) read the lower ochre tile; screenshot inspection
  also showed a bottom decoration strip. Worker :102 had no window manager, so
  its passing captures did not expose this. No renderer assertion was weakened.
- Use xwininfo absolute client origin/width/height, matching existing GPUI drivers.
  Ruff caught an initial capture-label shadow before execution; renamed the local
  field. `uvx ruff check scripts/native-cues.py` and `DISPLAY=:99
  VK_DRIVER_FILES=/dev/null python3 scripts/native-cues.py --backend gl --out
  .amp/in/artifacts/native-resources-integrated` now pass all 15 captures, pixel
  samples and retained full-image RGB hashes. `git diff --check` passes.
- Inspected corrected text-asymmetric-image and expired-retains-text-image PNGs:
  complete six tiles, equal-height rows, readable Signal café/Beacon, no decoration
  strip or shifted crop. Receipt sequence/pixel checks are executed evidence;
  images alone do not prove timing or physical scanout. No application behavior
  change. Local M0-07d follow-up, no push.

### M1-06b / M0-06c / M0-07d — Merged verification — 2026-10-04 UTC

- Merged both independent worktrees into LOCAL main, preserving all three
  implementation commits. Only conflict was append-only work-log records; both
  retained. Parent reviewed migration backup/transaction boundaries, historical
  payload/ID/arrangement resolution, editor history, bounded wire parsing,
  preparation queue ownership, GPU-ready bindings and native receipt ordering.
- Held `/tmp/sela-cargo-continuation.lock`, touched merged src/examples/test roots
  and observed actual compilation. `cargo test --locked --all-targets -j4`:
  **112 passed, zero failed, 1 ignored subprocess fixture**, explicitly invoked by
  four process tests. `cargo clippy --locked --all-targets -j4 -- -D warnings`,
  `cargo fmt --all -- --check`, `cargo build --locked --all-targets -j4`, Ruff on
  all three native Python drivers and `git diff --check` passed.
- Serial `DISPLAY=:99 VK_DRIVER_FILES=/dev/null` runs of
  `python3 scripts/song-library.py`, `python3 scripts/operator-shell.py` and
  `scripts/native-smoke.sh "$PWD/target/debug/sela"` passed. Song replay verifies
  schema2 distinct IDs and retention across structural removal/undo/save against
  independent SQLite reads, plus existing payload/close/reopen assertions.
- Native resources driver initially failed under Openbox; fixed and verified
  actual client coordinates as recorded immediately above. Final run: PASS,
  15 captures, actual text coverage/asymmetric pixels, identical retained RGB
  hashes after invalid font, missing glyph, overflow, oversized text, stale and
  expired cues, fresh restart without replay. Evidence in
  `.amp/in/artifacts/native-resources-integrated/summary.json` and its PNGs;
  text-asymmetric-image and expired-retains-text-image personally inspected.
- Shared GPU helper regression: private temporary XDG runtime, explicit Vulkan
  llvmpipe, `target/debug/examples/composition_spike <temporary-output> vulkan`
  passed pixel/color/contain/cover/overflow checks; `target/debug/examples/video_spike
  <temporary-output>` passed 12 exact ordered RGBA frames, actual-text GPU pixels,
  missing/corrupt, saturated deadline/cancel and retained-last-frame checks.
  Disposable outputs removed. These runs are checks, not performance qualification.
- Reinstalled with `cargo install --path . --locked --offline --debug --bin sela
  --root "$PWD/.amp/install" -j4`; installed `--version` prints
  `Sela 0.1.0 (technical preview)`. Installed native-smoke passed Ctrl+Q and WM
  close. No original profile opened/migrated during testing; fixtures are disposable.
- README/backlog/current-state and arrangement replay checklist updated. UI layout
  unchanged; no arrangement controls, pagination or Go Live wiring claimed.
  Windows/installed reference, real GPU/projector, mixed DPI, accessibility/IME,
  power-loss/volume faults, production budgets and physical soaks remain open,
  with final hardware verification postponed at the owner's request.
- Delivery: local commits and merges only, no push/publication/deployment.
  Resume arrangement UI/repair or renderer resize/safety coordination as above.

### M0-06 — Oracle disconnected receipt fix — 2026-10-05 UTC

- A matched Rejected(Disconnected) now invalidates the session, clears both
  pending lanes and exposes Unknown; prior confirmation remains diagnostic history
  only. Late acknowledgments cannot revive it and new sends require a fresh session.
- Regression first failed (Confirmed revision 3 instead of Unknown), then passed
  for both Cue/Safety orderings, two dispatched pending commands, late Applied
  and new sends. `cargo test --locked --lib delivery::tests -j4`: 9 passed;
  `cargo clippy --locked --lib -j4 -- -D warnings` and fmt passed.
- No UI/protocol shape change. Remaining Oracle fixes are pending native editor
  input and renderer backpressure; combined checks follow those changes.

### M1-05 — Oracle pending-input ownership fix — 2026-10-05 UTC

- Synchronously lock TextInput mutations when storage work is accepted (including
  initial open), before the next redraw can detach the native handler. Native
  replace/preedit and local undo/redo cannot change buffer/history while locked.
  Programmatic acknowledged loads remain possible. Unlock on completion/error,
  unless a committed OK is awaiting window removal; owner actions/history are
  also blocked during that final interval. OK independently compares the current
  document against the Saved snapshot before closing.
- Inspected pinned Zed a84689073d296dfd39987bc7dd478e43ef76d83a:
  crates/gpui/src/window.rs handle_input (5245–5266) and
  crates/gpui_linux/src/linux/x11/window.rs handle_ime_commit/preedit (1224–1247).
  Input-handler lifetime extends until redraw, so hidden controls are not a lock.
  Existing Apache GPUI APIs only, no new dependency or copied application code.
- GPUI regression first failed with late commit/preedit bytes in the field after
  OK submission. Now exercises OK, Load, Delete and conflicting Save, injecting
  both callbacks in the same App update before any redraw/poll, checking unchanged
  field/edit-count/history, actual durable reply, and unlock/close behavior.
  Separate test bypasses native lock with a programmatic load and proves OK cannot
  close over a mismatched snapshot. No layout/appearance changes.
- `cargo test --locked --all-targets -j4`: 117 passed, 1 ignored subprocess fixture
  (includes in-progress renderer regressions). Strict all-target Clippy,
  fmt and all-target build passed. Serial DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  song-library.py, operator-shell.py and native-smoke.sh target/debug/sela passed,
  including independent SQLite/undo/reopen/OK/dirty-close/focus checks. Native IME
  interleaving is simulated via its actual input-handler API; Windows/real IME
  device qualification remains open. Local commit only; renderer fix follows.

### M0-07d — Oracle preparation ordering fix and final checks — 2026-10-05 UTC

- Saturated preparation retains one bounded Frame and stops pipe intake until
  enqueued, rather than rejecting a later stamp before earlier preparation has
  completed. Preserves receiver timestamp/deadline; worker disconnect is fatal.
  Documented extra CPU ownership ceiling of 64MiB+168 bytes. Still FIFO diagnostic
  preparation, not production safety-priority or measured memory qualification.
- Deterministic queue-full regression withholds the worker receive, tests both
  Cue/Safety orders with asymmetric revisions 7/29, repeated blocked polls (no
  further pipe intake), and ordered Accepted/Applied. Separate test checks expired
  retained deadlines and disconnected preparation worker.
- `cargo test --locked --all-targets -j4`: 117 passed, zero failed, one ignored
  subprocess fixture explicitly invoked by four process tests.
  `cargo clippy --locked --all-targets -j4 -- -D warnings`,
  `cargo fmt --all -- --check`, `cargo build --locked --all-targets -j4` and
  `uvx ruff check scripts/native-cues.py scripts/song-library.py
  scripts/operator-shell.py` passed. Combined native editor/operator/smoke checks
  recorded above also passed with all three fixes present.
- `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/native-cues.py --backend
  gl --out .amp/in/artifacts/oracle-fixes-native`: PASS, 16 captures. New real-process
  case sends Cue 2 then Safety 3 without waiting for acknowledgments; both receive
  Accepted then Applied, terminal order is 2 then 3, final blue RGB is (31,53,179).
  Existing failure/expiry retention and fresh-session/no-replay assertions pass.
  Evidence: `.amp/in/artifacts/oracle-fixes-native/summary.json`; inspected
  `consecutive-lanes-blue.png`, uniform blue without old content/decorations.
- Reinstalled using `cargo install --path . --locked --offline --debug --bin sela
  --root "$PWD/.amp/install" -j4`; `.amp/install/bin/sela --version` prints
  `Sela 0.1.0 (technical preview)`. `DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  scripts/native-smoke.sh "$PWD/.amp/install/bin/sela"`: PASS for focus, resize,
  Ctrl+Q and WM close with clean exit/window removal. Temporary profile only.
- All three requested Oracle fixes are implemented and locally verified. Backlog
  evidence updated without closing parent qualification gates. Windows/real IME,
  installed reference, GPU/projector, production safety/performance and installer
  checks remain open. Next implementation slices remain arrangement controls/
  missing-reference repair or renderer resize/safety coordination. Local commits
  only; no push, publication or deployment.

### M0-01 — Windows build and native launch — 2026-10-06 (Asia/Jakarta, UTC+7)

- State: **done**. Closes the last M0-01 checklist item. Base `b553d9a` local main.
- Environment: Windows 11 Home Single Language, i7-14650HX, Intel UHD Graphics
  (driver 32.0.101.6790) + NVIDIA RTX 4060 Laptop (32.0.15.9159); displays
  DISPLAY5 primary 2048x1152 logical, DISPLAY1 1463x914, DISPLAY6 1920x1080.
  Installed via winget: rustup 1.29.1 (toolchain 1.98.1 from rust-toolchain.toml),
  VS 2022 Build Tools 17.14 VCTools + Windows 11 SDK 10.0.26100 + CMake, Python
  3.12.10 (user scope) and Ruff (pip --user). Exact commands in gpui-bootstrap.md.
- Windows-only defect found and fixed (M1-01 storage): `copy_new` flushed the
  staged backup through `File::open(..).sync_all()`. Windows `FlushFileBuffers`
  needs a write handle, so every verified backup and schema-1 migration returned
  `Error::Io` (six storage tests failed). Now opens with write access before
  `sync_all`. Also silenced the Windows-only `unused_mut` warning on the
  backup-directory builder. Windows staging directories inherit the parent ACL
  (no 0o700 equivalent); recorded as an open Windows hardening item, not fixed.
- Verification in plain PowerShell 7 from this checkout (fresh Windows target dir):
  `cargo build --locked --all-targets -j 16` passed (4m57s cold);
  `cargo fmt --all -- --check` and
  `cargo clippy --locked --all-targets -j 16 -- -D warnings` passed;
  `cargo test --locked --all-targets -j 16` **112 passed, 1 ignored subprocess
  fixture**, 0 failed (lib 52, bin 28, transport_process 4, composition 5,
  input_check 9, native_cues 7, output_spike 1, video_spike 6). Linux runs 117:
  five video decoder child-process tests are `cfg(unix)`. Before the fix: 46/52
  lib tests passed, six storage backup/migration tests failed with `Io`.
  `target\debug\sela.exe --version` printed `Sela 0.1.0 (technical preview)`;
  SHA-256 `221a734621f3c6c7ddf2cecd4f67bbb592062bb3e60021ddfea75e03b6b22c45`.
- Native N: new `scripts/native-smoke-windows.py` (+ `scripts/native_win.py`)
  `python scripts/native-smoke-windows.py target\debug\sela.exe .amp\in\artifacts\windows`
  PASS: PID-scoped window, verified foreground before every SendInput, survived
  Ctrl+J, DPI 120 (125%) client settled at 900x550 = 720x440 logical, smaller
  request refused (minimum held), 180 sampled colors, Ctrl+Q exit 0 after 2.1s,
  relaunch Alt+F4 exit 0, both windows removed. Ruff check passed after removing
  one unused noqa. Window was on the primary 2048x1152 display only.
- UI evidence: inspected `.amp/in/artifacts/windows/m0-01-windows-small.png`
  (900x550): readable Segoe UI toolbar, Schedule/Preview/Live, Resources tabs and
  "+ New Song", no clipping at the minimum size. Log shows GPUI DirectWrite with
  Segoe UI, RTX 4060 Direct3D 11.1 device; `0x887A002D` DXGI debug interface
  missing (optional Graphics Tools, debug-only) and a benign shutdown
  `window not found`. `.amp/in/` is now gitignored (it was only locally excluded).
- Not run: Windows CI (not pushed), Narrator/UIA semantics, IME, cross-monitor
  DPI moves, Intel-GPU operator rendering, song-library/operator-shell native
  drivers (X11-only, not yet ported), EasyWorship reference (not installed).
- Next: M0-04 Windows run of `examples/output_spike.rs` on a second physical
  display with DX12; then M0-07 composition/video/native cue checks on DX12.
- Delivery: local `fix(M1-01)` and `feat(M0-01)` commits; no push.

### M0-04 / M0-07 — Windows DX12 physical-display and GPU readback — 2026-10-06 (UTC+7)

- State: both **implemented-unqualified**; no checklist box closed.
  Evidence detail and tables: output-spike.md (Windows section),
  composition-spike.md and video-spike.md. Same host as the M0-01 record.
- Owner was playing a game throughout. Timing is under uncontrolled GPU load
  and possible concurrent input; treat it as feasibility, not qualification.
- Changes: `examples/output_spike.rs` opt-in `SELA_SPIKE_MONITOR`,
  `SELA_SPIKE_FULLSCREEN`, `SELA_SPIKE_SECONDS` (default 25s, clamp 1..600) and
  wgpu `WGPU_POWER_PREF`, plus monitor/scale/surface telemetry; defaults keep
  the Linux path unchanged. `composition/gpu.rs` and `native_cues.rs` also honor
  `WGPU_POWER_PREF`; `video_spike` gained the optional backend argument.
  New `scripts/output-spike-windows.py`; `native_win.py` skips winit's visible
  0x0 helper window and falls back to AttachThreadInput for foreground.
  `scripts/native-cues.py` is now cross-platform: reader/writer threads replace
  select/nonblocking pipes, Win32 BitBlt capture on Windows, and a single
  raw-RGB sampler for both platforms. Its Linux path was not re-executed.
- Driver failures before the pass: (1) targeted winit's 0x0 helper window, so
  moves/captures/Alt+F4 hit the wrong HWND (65-byte PNGs, no scale events);
  fixed by skipping zero-area windows and matching the audience title within
  the PID. (2) Twice the operator never became foreground; added the
  AttachThreadInput fallback and a 1s settle before fullscreen-phase input.
- `python scripts/output-spike-windows.py` PASS (main, fullscreen, adapter
  matrix). Main: 949 presents, p50/p95/p99/max 17.03/17.61/18.36/48.25ms; 2s stall
  118 presents with a 17.97ms maximum gap; preparation gap 18.05ms; 224/60
  presents after exit/kill. Moves 175%→125%→175% emitted scale 1.75/1.25/1.75,
  client 1120x630/800x450/1120x630, maximum gaps 17.73/18.10ms. Fullscreen
  2560x1600 on the laptop panel, 2s stall gap 17.89ms. Matrix: low=Intel UHD,
  high=RTX 4060 on all three monitors, p50≈17.03ms, max ≤18.11ms (run with the
  earlier `SELA_SPIKE_POWER` name; rerun pending with `WGPU_POWER_PREF`).
- Inspected stall-a/stall-b (triangle moved during the stall), fullscreen-stall,
  after-kill under `.amp/in/artifacts/output-spike-windows/`.
- M0-07: `composition_spike <out> dx12` PASS with WGPU_POWER_PREF low (Intel)
  and high (NVIDIA); `video_spike <out> dx12` PASS on both, plus `vulkan` on NVIDIA,
  with FFmpeg 7.1.1 gyan.dev full build. Inspected `image-contain.png`.
- Findings: wgpu default DX12 present mode was Mailbox (latency 2) with
  timer pacing. That cannot match 165/180Hz scanout, so production must choose
  the present mode and pacing explicitly. Present-call timing is not scanout.
- Checks after changes: ruff on all seven Python scripts, `cargo fmt --check`,
  strict all-target clippy, `cargo test --locked --all-targets` 112 passed /
  1 ignored. All passed.
- Not run: `native-cues.py --backend dx12` and the matrix rerun (both open
  windows that can take focus from the owner's game), display hotplug (needs the
  owner to unplug a monitor), device loss/TDR, sleep/resume, DXGI frame
  statistics, idle-machine soak, Linux re-execution of the refactored cue driver.
- Next: when the machine is idle run
  `python scripts/native-cues.py --backend dx12 --out .amp\in\artifacts\native-cues-windows`
  and `python scripts/output-spike-windows.py`; then a hotplug run with
  `SELA_SPIKE_SECONDS=120` while the owner unplugs/replugs monitor 2.
- Delivery: local `feat(M0-04)` and `feat(M0-07)` commits; no push.

### M0-04 — Idle-machine Windows rerun — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**; no checklist box closed.
- Owner stopped gaming. Same host and debug build as above.
- `python scripts/output-spike-windows.py .amp\in\artifacts\output-spike-windows-idle`
  PASS (main, fullscreen, `WGPU_POWER_PREF` matrix). Main 944 calls, p50/p95/
  p99/max 17.04/17.73/18.24/23.39ms; 2s stall max gap 18.20ms; preparation
  18.64ms; 218 calls after exit, 60 after kill; scale 1.75→1.25→1.75.
  Fullscreen 2560x1600 p50/p99/max 17.02/18.33/18.84ms. Matrix low=Intel UHD,
  high=RTX 4060 on all three monitors. Present mode again Mailbox latency 2.
  Detail in output-spike.md.
- Not run: hotplug (owner must unplug a monitor), device loss/TDR, sleep/resume,
  DXGI frame statistics, multi-hour soak.
- Next: hotplug with `SELA_SPIKE_SECONDS=120` while the owner unplugs and
  replugs monitor 2.

### M0-06 / M0-07d — Windows DX12 native cue driver — 2026-10-06 (UTC+7)

- State: both **implemented-unqualified**; no checklist box closed.
- `cargo build --locked --example native_cues` then
  `python scripts/native-cues.py --backend dx12 --out .amp\in\artifacts\native-cues-windows`
  PASS on the RTX 4060 (DX12, driver 32.0.15.9159). First Windows execution of
  the cross-platform driver. All 16 receipt-correlated captures matched their
  expected sample colors, including failure retention of the text/image scene,
  old-epoch rejection and the unconfirmed restarted session. Detail in
  delivery.md.
- Not run: Intel adapter for this driver, Linux re-execution of the refactored
  driver, physical scanout timing.

### M0-02 — First installed observation, partial W01 — 2026-10-06 (UTC+7)

- State: **active** (was blocked). Owner installed the free, unlicensed
  EasyWorship 8 (registry 8.0.49, executable FileVersion 8.0.49.0).
- New `scripts/reference_win.py`: lists one image's windows and captures a
  single HWND with PrintWindow, plus focused click/type helpers. Captures never
  include other applications.
- Recorded RUN-W01-2026-10-06 and EW8-OBS-010–013 in reference-observations.md:
  main-window layout (menu row, toolbar groups, Schedule/Preview/Live with
  Preview Output/Live Output strips and "Slide N of M", Songs library), the
  unlicensed output windows on the laptop panel with a "NOT LICENSED FOR LIVE
  PROJECTION" logo, and the song editor structure and dirty-cancel prompt.
- Failed: synthetic clicks inside the song editor did not move focus, so no
  fixture songs exist and W02 (Go Live, Preview/Live, next/previous) did not run.
  Draft discarded with No; nothing saved in the EasyWorship profile.
- Checks: `python -m ruff check scripts/reference_win.py` passed.
- Not run: About dialog capture, W02–W06, masks, keyboard/focus, routing.
- Next: owner-assisted fixture creation (or an import file), then W02.

### M1-10a — Application audience renderer mode — 2026-10-06 (UTC+7)

- State: **active** parent; no checklist box closed. Owner request to integrate
  the UI with the engine is treated as approval to start M1-10 before M1-03,
  M1-09 and M0-08 finish (2026-10-04 sequencing exception).
- Moved `examples/composition/{gpu,text}.rs` to `src/audience/{compositor,text}.rs`
  and the native cue renderer into `src/audience.rs`; `examples/native_cues.rs`
  is now a thin wrapper, and the composition/video examples include the moved
  files. cosmic-text, winit, wgpu and pollster became normal dependencies.
- `text::raster_aligned` adds centered lines (horizontal and vertical).
- Transport frame kind 4 SURFACE (epoch, width, height): the audience reports
  its settled drawable extent after Ready and after each resize. Reports wait
  for 250ms without resize, because Windows emits transient sizes on monitor
  arrival and a cue built for one would fail.
- `sela --audience EPOCH_HEX <INDEX|secondary|window> [backend]` opens a
  borderless window covering the chosen monitor. It is created without
  activation and is not winit fullscreen: winit's `set_fullscreen` (and
  fullscreen-at-creation) force the window active on Windows and took keyboard
  focus from the operator. The window is sized again after arrival because a
  monitor with another scale factor rescaled it (3584x2240 on a 2560x1600 panel).
  With `retain_on_disconnect` the applied scene stays on screen after the
  controller pipes close, until the window is closed.
- New `scripts/audience-smoke-windows.py`: Ready then one settled surface
  report, client equals surface, window rect equals monitor rect, foreground
  unchanged, centered text cue Accepted then Applied, no foreign pixels over the
  output (composed-screen capture of the audience client only), retained after
  stdin close, WM_CLOSE exit 0.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` and `cargo build --locked --all-targets` pass.
  `python -m ruff check scripts/audience-smoke-windows.py` and `ruff format` pass.
  `python scripts/native-cues.py --backend dx12 --out .amp\in\artifacts\native-cues-windows-m1-10a`
  PASS. `python scripts/audience-smoke-windows.py --monitor secondary` (laptop
  panel 2560x1600, 168 DPI) and `--monitor 0` (primary 2560x1440, 120 DPI) PASS,
  0 foreign pixels; evidence in `.amp\in\artifacts\audience-smoke-windows-*`.
- Not run: Linux re-execution of the moved renderer, Intel adapter, hotplug of
  the audience monitor, physical scanout timing, taskbar behavior with other
  taskbar settings.
- Next: M1-10b operator supervisor that spawns `sela --audience` with a fresh
  epoch, Go Live from preview slides, Live pane showing confirmed/unknown/
  disconnected state, next/previous.

### M1-10b — Operator live output and acknowledged Live — 2026-10-06 (UTC+7)

- State: **active** parent, implemented-unqualified slice; no checklist box
  closed (safety buttons, preparing state off the UI thread, observed 8.0.49
  behavior, masked Go Live and latency remain open).
- New `src/output.rs` `Supervisor`: spawns one audience child per session off the UI
  thread with a fresh SHA-256 epoch; Starting → Connected{extent, caps} or
  Lost (spawn failure, 15s startup timeout, exit, protocol or delivery error).
  One cue in flight (3s acknowledgment) plus one wanted cue that coalesces to the
  latest; rejected cues keep the prior confirmed state; children are killed and
  reaped off-thread on drop. `Delivery::is_connected` added.
- New `src/slides.rs`: one slide per section, versions `revision << 16 | index`,
  white centered DejaVu Sans on black. Font size fitted from glyph advances
  (5% margin) within the audience text limits (96px, 32 lines, 4096 bytes,
  4096px area); missing glyphs fail before delivery.
- `src/operator.rs`: storage-worker catalog in Songs (refreshed on activation),
  Preview slide tiles (click, Enter/Space, double-click = Go Live per SRC-02,
  documented-only), Live ○/● toggle, Go Live, Previous/Next stopping at the ends,
  Live pane with output line, "On screen", "Sending…" and rejection lines, red
  confirmed / amber sending borders; resend on settled surface change. No replay
  into a new session. `sela --operator-library PATH`; `SELA_AUDIENCE_MONITOR`,
  `SELA_AUDIENCE_BACKEND`.
- Tests: `tests/output_process.rs` (fake audience child: apply, reject, resize,
  wrong epoch, exit, hang, silent), supervisor and slide unit tests, operator
  tests `go_live_requires_preview_and_output`,
  `go_live_shows_renderer_acknowledged_slide`,
  `rejected_or_lost_output_is_never_shown_as_live`, updated keyboard traversal,
  and `audience::tests::operator_slide_cues_fit_the_audience_text_preparer`.
- Native: new `scripts/live-output-windows.py`, `examples/seed_library.rs`,
  `native_win.press`. The first run failed correctly: the real renderer rejected
  a 256px cue (limit 96px); Live showed "Last cue not shown" and "nothing
  confirmed", audience stayed black. Fixed the fit, added the cross-check test.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (144 passed, 3 ignored fixtures),
  `cargo build --locked` and `cargo build --locked --example seed_library` pass.
  `python -m ruff check scripts` passes; `ruff format` applied to the new script
  only (five older scripts are not ruff-formatted; unchanged).
  `python scripts/live-output-windows.py` PASS three times (`--out` default,
  `-r1`, `-r2`) on the laptop panel 2560x1600 @168 DPI, DX12, RTX 4060 Laptop:
  operator kept the foreground, Go Live/Next/Previous observed 171–188 ms after
  the key at capture granularity, Next stopped at the end, Live off and Ctrl+Q
  ended the child. `slides::cue` on the UI thread: p50 0.10 ms, p95 0.11 ms, max
  0.79 ms (200 runs, 2560x1600, dev profile, throwaway example not committed).
- Not run: primary-monitor output with the operator on the same monitor, Linux/
  macOS native, Intel adapter, audience hotplug, installed 8.0.49 W02/W03,
  physical scanout latency, screen reader.
- Next: M0-08 mask semantics then Black/Clear/Logo buttons; M1-09 schedule items
  as Live sources; W02 observation (needs owner-assisted fixtures) for slide
  shortcuts, selection and double-click; move cue preparation to a worker if
  themes or images make it measurable.

### M0-02 — W02/W03 single-song observation — 2026-10-06 (UTC+7)

- State: **active**. Scope: the static W02/W03 cases reachable with one song,
  as planned in the approved 2026-10-06 spec (Phase 1).
- Sources: added SRC-05–SRC-11 (help.easyworship.com Toolbar, Live Area,
  Preview Area, Helpful Shortcut Keys, Editing Images, Editing Videos; the
  Adding And Editing Songs article). SRC-04 was already Screen Setup, so the
  spec's "SRC-04 to SRC-10" became SRC-05–SRC-11.
- New `scripts/reference-w03-windows.py` (probe/press/seq/run). Input only to
  the EasyWorship main window after a foreground check; audience evidence is
  the Live Output window's screen rectangle captured until stable; toggle
  indicators read from a PrintWindow toolbar crop; JSONL row and contact
  sheet per case.
- Fixtures: owner's song "waeqwe" (4 slides, placeholder text). Generated
  original logo and green-stripe background images; imported via Media →
  Images → +, logo set with "Use As Logo Background", background applied with
  Copy to Theme ▸ Song Theme and "Set As Default Song Theme".
- First run RUN-W03-20261006-132625 was invalid for masks: the Media tab stayed
  active, so the image item was previewed and live. Kept as evidence that Clear
  does nothing on a live image item. Rerun RUN-W03-20261006-133553 (165 rows)
  on the song is the source of EW8-OBS-014–019; EW8-OBS-020 lists what did
  not run.
- Findings: Black and Logo replace each other, Clear stacks with either; Go
  Live, Preview double-click and Live ‹ › keep masks; Live double-click clears
  a single mask and applies that slide; Live off keeps mask state; Ctrl+B/L/C
  and Page Down match the buttons with Preview focus; Live slide single-click
  applies immediately; › stops at the last slide.
- Checks: `python -m ruff check scripts/reference-w03-windows.py` and
  `python -m ruff format --check` pass. No Rust changes.
- Not run: combined-mask Live double-click, masked Live single-click, profile
  reopen, motion/audio, alerts, multiple outputs, multi-item schedule cases,
  W04 focus contexts, transition timing. The EasyWorship profile now holds the
  two images, a "sela-w03-background" default song theme and the logo setting.
- Next: Phase 2, M0-08a `src/masks.rs` (cover None/Black/Logo plus Clear flag)
  and M1-10c operator toggles with a picture logo.

### M0-08a — Mask layer and logo slot — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Scope: Phase 2a–2c of the approved
  2026-10-06 spec; operator UI, logo source and native checks are M1-10c.
- Added `src/masks.rs` (cover None/Black/Logo plus independent Clear, from
  EW8-OBS-015–017; combined-mask Live double-click is provisional per
  EW8-OBS-020). `Payload{Scene,Logo,Mask}` in `delivery`; masks on the Safety
  lane, logo on the Cue lane. Wire kinds 5 MASK (v1, 30 bytes) and 6 LOGO
  (v2 owned resource). Supervisor mask/logo slots with coalescing and pump
  order mask, logo, cue. Audience: masks bypass the preparation worker,
  background-only bind group for Clear, Black clear pass, retained logo
  bindings, wrong-extent slides keep the last frame.
- Decision: `RendererSession` orders sequences per lane. Otherwise a mask sent
  while a slide is still in preparation would make that slide Stale. Updated
  `reordered_commands_and_duplicates_never_reapply_old_content` to assert
  same-lane reordering and added `lanes_are_ordered_independently`.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (159 passed, 3 ignored fixtures) and
  `cargo build --locked` pass on Windows 11.
- Not run: native capture of Black/Clear/Logo frames, operator indicator
  agreement, Linux/macOS, video logo (deferred to M2-03).
- Next: M1-10c operator toggles (acknowledged indicators, Live pane mask lines,
  Live double-click unmask, Ctrl+B/L/C and PageDown in a show-control key
  context), Media Images list and "Use as Logo Background", native checks.
### M1-10c — Operator masks and picture logo — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Scope: Phase 2d–2f of the approved
  2026-10-06 spec (operator toggles, picture logo, native checks).
- Added `src/images.rs`: profile `Resources/Images/` beside the library,
  PNG/JPEG copy-import with a free " (N)" name, 8 MiB / 4096-file caps,
  SHA-256 `ResourceRef`, `logo.txt` for the chosen logo, and a bounded worker
  (Scan/Import/UseAsLogo/LoadLogo) so no file work runs on the UI thread.
- Operator: Logo/Black/Clear toolbar buttons with Off/Pending/On indicators from
  supervisor acknowledgments; `ToggleBlack/ToggleLogo/ToggleClear/GoLive`
  actions bound to Ctrl+B/L/C and Page Down in `SelaShow && !SelaTextInput`;
  Live pane mask lines; Live single-click applies, double-click unmasks after
  its cue leaves the slot; mask intent survives Live off/on (EW8-OBS-017).
  The logo is prepared off-thread per surface extent and resent on new session
  and resize; Logo intent goes out as Black until the logo is with the renderer.
  Media tab lists images with Import image… and Use As Logo Background (button
  and right-click menu).
- Decisions: text-field exclusion for show keys is Sela policy (W04 open);
  combined-mask Live double-click and single-click under a mask are provisional
  (EW8-OBS-020); logo uses Contain, diverging from EW8-OBS-019 "fills the
  output" until a non-matching aspect ratio is observed; video logo is M2-03.
- Checks (Windows 11, RTX 4060 Laptop): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 73, bin 47 + 1 ignored fixture,
  integration suites all pass), `cargo build --locked`,
  `python -m ruff check scripts` pass. `ruff format --check` passes for
  `live-output-windows.py`; `native_win.py` and `native-cues.py` were already
  unformatted at HEAD and were left unformatted.
- Native: `cargo build --locked --example seed_library` then
  `python scripts/live-output-windows.py --out .amp\in\artifacts\live-output-windows-masks{,-r1,-r2}`
  PASS three times (2560x1600 secondary, DX12): Black/Clear/Logo frames and
  lit buttons agree, each second press restores the identical slide frame,
  Logo returns after Live off/on, 171–188 ms per change (capture polling).
  `cargo build --locked --example native_cues` then
  `python scripts/native-cues.py --backend dx12 --out .amp\in\artifacts\native-cues-windows-masks`
  PASS: Clear keeps the image background without text, Black, Logo without a
  logo is RenderFailed and stays black, duplicate mask Applied without redraw,
  unmask byte-identical to the retained text/image frame. Captures inspected.
- Not run: native file picker (logo seeded into the profile; GPUI test covers
  the prompt), Linux/macOS, UIA names, installed observation of EW8-OBS-020,
  physical scanout timing.
- Next: Phase 3 M1-06c — raise the text size limit, resize-to-fit/normalize,
  Ctrl+Enter split with undo, arrangement order on output.

### M1-06c — Better slide output — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Scope: Phase 3 of the approved
  2026-10-06 spec.
- `slides::slides` follows the first arrangement (falls back to section order
  when there is none, it is empty, or a section does not resolve).
  Text limit 96 → 288px in `audience::text` and `slides.rs`.
  `slides::Sizing{PerSlide,Normalized}` and `size_cap`; `cue` takes a cap.
- Operator: "Normalize text size across slides: Off/On" in the Songs footer
  (control 21, after Clear in tab order); cap cached per song revision, extent
  and setting; toggling resends the live slide; not persisted.
  `scripts/live-output-windows.py` tab counts updated (19 tabs to the first song,
  9 back to Live output).
- Song editor: Ctrl+Enter (`SplitSection`, `SongLibrary` context) splits the
  section at the lyrics cursor; new section ID, copied label; arrangements get
  the new section after each occurrence of the split one; one undo step.
  `TextInput::selection()` added.
- Decisions: always resize to fit (the fixed-size mode needs a theme font
  size); normalize off by default; copied label and mid-line split are
  provisional; SRC-12 added for EasyWorship's auto-size settings.
- Checks (Windows 11): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 75, bin 49 + 2 ignored: fake
  audience fixture and timing; all other suites pass), `cargo build --locked`,
  `python -m ruff check scripts`, `ruff format --check scripts/live-output-windows.py`.
- Measured: `cargo test --locked --release --bin sela large_text_preparation_time -- --ignored --nocapture`:
  2560x1600 p50 0.6–2.7 ms (max 3.3), 3840x2160 at 288px p50 0.7–3.5 ms
  (max 4.2); dev profile p50 3.7–29.9 ms (max 32.7).
- Native: `python scripts/live-output-windows.py --out .amp\in\artifacts\live-output-windows-m1-06c`
  PASS (2560x1600 DX12); captures inspected: the verse fills the width at the
  new size; the operator footer shows the Normalize toggle.
- Not run: native Ctrl+Enter and Normalize (no Windows song-editor driver),
  EasyWorship 8.0.49 observation of auto-size and split, Linux/macOS.
- Next: Phase 4 M1-09a — Schedule pane items pinned to song revisions, add/
  remove/reorder, Save/Open with Ctrl+S/Ctrl+O, live item identity.

### M1-09a — Basic schedule — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Scope: Phase 4 of the approved
  2026-10-06 spec (early start with the same approval as M1-10; M1-04 and
  M1-08 have not started).
- Added `src/schedule.rs` (framework-free): entries with session-local
  `EntryId`s pinning song `Version`s, insert/push (full at 32 = storage
  `MAX_ITEMS`), remove, move_to/move_by, neighbor (stops at the ends), snapshot,
  `mark_saved` (edits during a save stay dirty), open (checks title count),
  clear; dirty compares entry versions with the last saved/opened baseline.
  `Repository::schedule_catalog` and `Command/Reply::ScheduleCatalog` list
  current schedule titles.
- Operator: Schedule pane (header, Unsaved, rows, "● Live" only after renderer
  acknowledgment, drag/drop from Songs and between rows, right-click Remove
  From Schedule, footer Up/Down/Remove, message line); Add to Schedule in the
  Songs footer; Open/Save toolbar buttons; New ▾ → New Schedule. Dialogs for
  save-as title, open list, remove confirmation and unsaved guard. Storage
  requests go through one queued worker job. Live identity is the entry, so
  reorder/removal never sends a cue; Live notes "Live item is no longer in the
  schedule". Library double-click goes to Live from the first slide. Window
  close (`on_window_should_close`) and Ctrl+Q use `may_close`. Actions
  SaveSchedule/OpenSchedule/Next/PreviousScheduleItem/RemoveScheduleItem bound
  to Ctrl+S/Ctrl+O/Down/Up/Ctrl+Delete in `SelaShow && !SelaTextInput`.
  Overlays (dialogs, item, New and Media image menus) now `occlude()` so clicks
  no longer fall through to rows behind them (found by a GPUI test).
- Decisions (provisional, unobserved in 8.0.49): Up/Down move buttons,
  Ctrl+Delete removes without confirmation, Down/Up stop at the ends, a song
  dropped on a row inserts before it, library double-click starts at slide 1,
  item IDs are not persisted. Library edits never update scheduled revisions.
- Tests: `schedule::tests`, `storage` `schedule_catalog_lists_current_titles`,
  GPUI `schedule_add_reorder_select_navigate_and_remove`,
  `drag_songs_into_the_schedule_reorder_and_cancel`,
  `live_item_identity_survives_reorder_duplicates_and_removal`,
  `save_and_reopen_pin_revisions_behind_an_unsaved_guard`,
  `quit_and_new_schedule_are_guarded`, `library_double_click_goes_straight_to_live`;
  two traversal tests updated for the new controls.
- Checks (Windows 11, RTX 4060 Laptop): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 82, bin 55 + 2 ignored, integration
  and example suites all pass), `cargo build --locked`,
  `python -m ruff check scripts`, `ruff format --check` on
  `schedule-windows.py` and `live-output-windows.py` pass.
- Native: `cargo build --locked --example seed_library` (now with
  `--second-song`), then
  `python scripts/live-output-windows.py --out .amp\in\artifacts\live-output-windows-m1-09a`
  PASS (tab counts 25 to the first song, 15 back to Live output) and
  `python scripts/schedule-windows.py --out .amp\in\artifacts\schedule-windows-m1-09a-r2`
  PASS (secondary 2560x1600, DX12): keyboard add/duplicate/select/remove/
  reorder, Ctrl+S save read back from SQLite, two items sent live (62/63 ms,
  capture polling), live item removal keeps the frame, WM_CLOSE and Ctrl+Q
  guarded, reopen with Ctrl+O. The first run failed on a missing `down` key in
  `native_win.py` (added Up/Down/Delete). Captures inspected; a doubled border
  around the title field was removed.
- Not run: native mouse drag/drop (GPUI tests only), EasyWorship 8.0.49
  observation of schedule shortcuts/menus/drop insertion, Linux/macOS, UIA.
- Next: the approved spec is complete. Choose the next dependency-ready ticket:
  remaining M1-09 items (duplicate, multi-move, autoscroll, explicit refresh),
  schedule-level Live navigation (M1-10), or W04 observation.

### M1-05e — EasyWorship Song Editor Words layout — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Owner request: make Song New/Edit match
  EasyWorship 8. Reverse engineering the EW binary was declined (EULA,
  copyright, original-code rule); the installed 8.0.49 editor was observed as a
  black box instead (RUN-W06E, EW8-OBS-021–026 in reference-observations.md,
  captures in `.amp/in/artifacts/reference/ew8-w06-editor/`, nothing saved in
  EW). Owner-approved plan: four phases, each its own commit: M1-05e Words and
  layout, M1-05f Slides tab, M1-05g formatting (follow the official help,
  provisional), M1-05h background; EW layout/behavior in Sela's light style.
- `src/song_library.rs` rewritten around per-slide label/lyrics cells
  (`TextInput` flow mode, index-parallel to `draft.sections`), keeping the
  whole-document history, storage worker, guards and action indices (5/6 are
  now toolbar Undo/Redo, 7/8 `+`/`−`, 1 is footer Apply). Ctrl+Enter no longer
  copies the label; Backspace at the start of an unlabeled slide joins it to the
  previous one and drops its occurrences (provisional). History steps move the
  caret to the restored slide; reloading a cell keeps its caret. New songs start
  unlabeled (EW) instead of "Verse 1". Window title "Song Editor - <title>".
- `src/text_input.rs`: flow mode (borderless, height follows rows, wheel goes
  to the list, Up/Down at the edges, single-line Enter and Backspace at 0
  propagate), placeholders, `set_cursor`; the unfocused 3 px caret-selection
  quad is no longer painted for empty selections.
- Preview: `audience::text_coverage` (extracted from `prepare_frame`, same
  inset/raster) plus `slides::section_slide`; the editor rasterizes 1280x720
  BGRA on the background executor, one job in flight, latest wins, and drops
  replaced images with `App::drop_image`. Upstream checked at the pinned
  `a846890`: `crates/gpui/src/assets.rs` (`RenderImage`),
  `elements/img.rs` (`ImageSource::Render`), `app.rs`/`window.rs`
  (`drop_image`), `elements/div.rs` (`ScrollHandle::scroll_to_item`).
- Tests: new GPUI `words_cells_navigate_and_new_song_focuses_first_label`,
  `ctrl_enter_splits_without_label_and_backspace_joins`,
  `preview_follows_the_caret_slide_off_thread`; unit
  `preview_matches_audience_raster_and_rejects_unshowable_text`,
  `label_kinds_and_groups_follow_observed_palette`; the existing editor tests
  were moved from the old label/lyrics fields to cells without weakening their
  failure assertions (one expectation changed: undo restores the slide where
  the undone edit happened).
- Checks (Windows 11): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 82, bin 59 + 2 ignored, integration
  and example suites pass), `cargo build --locked`, `python -m ruff check scripts`.
- Native: `python scripts/song-editor-windows.py` PASS (new; Unicode SendInput
  and a click helper added to `native_win.py`): "Song Editor - Untitled", caret
  in slide 1's label, Enter to lyrics, line breaks, Ctrl+Enter, Up to slide 2's
  label, preview ink (black 38882 / white 910 samples), split and join, title
  click + typing, Ctrl+S payload decoded from SQLite, window title follows,
  Ctrl+Q exits 0. Captures in `.amp/in/artifacts/song-editor-windows/`
  inspected; stray caret quads in unfocused cells were found and fixed.
- Not run: X11 `scripts/song-library.py` (still targets the M1-05d form; not
  ported), IME/UIA, Linux/macOS, EW confirmation of Enter-in-label, Backspace
  join and the separate group for an appended unlabeled slide (recorded as
  Sela deviations in song-library.md).
- Next: M1-05f Slides tab with rendered thumbnails and label-colored caption
  bars (EW8-OBS-025), reusing `render_preview` at thumbnail size with a bounded
  cache.

### M1-05f — Song Editor Slides tab thumbnails — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Phase 2 of the owner-approved Song Editor
  plan. Source: EW8-OBS-024 and the installed 8.0.49 capture
  `.amp/in/artifacts/reference/ew8-w06-editor/25-slides-tab.png` (an appended
  unlabeled slide shows a neutral bar with grey italic "Slide 9", so Sela uses a
  neutral bar for every unlabeled slide; unlabeled slides inside a labeled group
  were not observed).
- `src/song_library.rs`: `render_preview` split into `coverage` and
  `image_from`; `render_thumbnail` box-filters the 1280x720 coverage 4x to
  320x180 so thumbnails keep the audience layout. `ensure_thumbnails` renders
  one missing slide at a time on the background executor, only in Slides, and
  prunes the text-keyed cache to the current slides on every render (dropping
  images from the atlas): at most 129 x 225 KiB. `thumbnail_row` draws number,
  thumbnail and caption bar with the selection frame; the Slides pane is 264 px.
- Bug found natively and fixed: in Slides, `+` and undo focused a Words cell
  (or a removed thumbnail) that is not rendered, so Ctrl+Q stopped reaching
  the editor. `focus_cell` now focuses the slide's thumbnail in Slides and
  history moves a focused thumbnail to the restored slide. Preview double-click
  switches to Words (Sela substitute for EW canvas editing).
- Tests: unit `thumbnail_is_the_preview_box_filtered`; GPUI
  `slides_tab_thumbnails_render_off_thread_and_prune_edits` (no rendering in
  Words, order, pruning after a Words edit, rejected text, `+` then Ctrl+Z
  keeps focus in the editor).
- Checks (Windows 11): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 82, bin 61 + 2 ignored, other
  suites pass), `cargo build --locked`, `python -m ruff check scripts`.
- Native: `python scripts/song-editor-windows.py` PASS twice, now also
  clicking Slides (thumbnail samples black 16816 / white 47), selecting slide 1
  (preview follows), `+` (grey italic "Slide 3"), Ctrl+Z, Ctrl+Q exit 0.
  Captures `05-slides`, `06-slide-1-selected`, `07-unlabeled-slide`
  inspected. The driver clicks logical coordinates for this layout at 125%.
- Not run: IME/UIA, Linux/macOS, EW confirmation of Slides keyboard navigation
  and of captions for unlabeled slides inside a labeled group.
- Next: M1-05g per-song text formatting (font, auto/fixed size, color, B/I/U,
  shadow, outline, alignment), following the EW help as provisional
  (EW8-OBS-026), with a schema change behind the verified-backup gate.

### M0-02 / RUN-W06G — EasyWorship Format pane observation — 2026-10-06 (UTC+7)

- State: observation only, no code. Owner asked to retry observing the Format
  panel before M1-05g. RUN-W06G recorded EW8-OBS-027–033 in
  reference-observations.md; EW8-OBS-026 is superseded. Nothing was saved in
  EasyWorship (every editor session closed with Cancel → No).
- Retry findings: the editor opened by the driver ignored all toolbar input
  again (`mouse_event`, held `SendInput`, `PostMessage`, touch injection;
  `HTCLIENT` hit test, no overlay, UIA shows only the preview child). After the
  owner restarted EW and reopened the editor, toolbar clicks worked. Remaining
  synthetic-input quirks (toolbar hit area over the pane's tab row; B/I/U clicks
  reopening the font list) are recorded as tooling caveats, not behavior.
- Owner suggested switching the reference to EasyWorship 7; recommendation was
  to keep the pinned 8.0.49 (all prior observations and the compatibility
  target are EW8; EW7 would mix builds). The owner continued on EW8.
- Key result for M1-05g: formatting is per slide in 8.0.49 (Shadow → None on
  slide 1 left slide 3 unchanged and persisted across slide switches). The
  Slide pane (EW8-OBS-032) covers the M1-05h background controls.
- Not observed: applying formatting to all slides at once, Reset Styles,
  Masters, what Apply/OK saves, schedule/live rendering of a formatted song,
  click behavior of B/I/U and the font list (only read from captures).
- Next: owner decides M1-05g scope (per slide as observed, or per song first
  as a recorded deviation), then implement with the schema change behind the
  verified-backup gate.

### M1-05g1 — Per-slide format model and schema 3 — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**; M1-05g stays **active** (g2, g3 open).
- Owner decisions: formatting is per slide (EW8-OBS-033); Ctrl+A selects all
  slides so one change formats the whole song; typing during Ctrl+A replaces
  the whole song text like EW (undoable). The Ctrl+A behavior is
  owner-reported, not yet observed. Approved plan: g1 model/storage, g2
  rendering with installed fonts, g3 Format pane.
- `src/format.rs` (new, no GPUI): `SlideFormat` with optional font, B/I/U,
  `Size::{Auto, Fixed}`, color, `Align`, `VAlign`, `Outline`, `Shadow`;
  `is_valid` bounds (Sela's, not EW's); canonical versioned codec with strict
  decode. `storage::Section.format`; `Song::validate` rejects invalid formats.
- `src/storage.rs`: `SCHEMA3` `section_formats` (rows only for non-default
  formats, FK to `section_ids`); fresh profiles start at 3; schema 2 migrates
  behind a verified `.schema2-backup`, schema 1 straight to 3 behind its
  single `.schema1-backup`; reads reject bad rows as `Corrupt`; copy/restore
  accept schemas 1–3; unsupported-version tests moved to 4. Editor history
  byte accounting includes font names. Other `Section` literals gained a
  default format.
- Tests: 3 `format::tests`, 5 new storage tests (migration, gates, abort hook
  `SELA_ABORT_MIGRATION_SCHEMA3`, round trip per revision, invalid/corrupt
  rows); schema 1 migration test asserts schema 3 and a single backup.
- Checks (Windows 11): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 90, bin 61 + 2 ignored, other
  suites pass), `cargo build --locked`.
- Not run: native scripts (no UI or renderer change in this slice), migration
  of a real owner profile (none exists outside tests), Linux/macOS.
- Next: g2. Extend `PreparedText`/transport with the resolved format, load
  installed fonts off the UI thread with bounds, render fill/outline/shadow
  masks and blend them with colors in the compositor, and move the editor
  preview/thumbnails to the same path.

### M1-05g2a — Styled slide rendering core — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**; M1-05g stays **active** (g2b, g3 open).
- One rendering path for the whole `SlideFormat`: audience frame, editor
  preview and Slides thumbnails all go through `slides::cue` →
  `audience::text::layers` → blend (GPU shader or CPU twin). Point-valued
  effects (outline size, shadow offset/blur) scale by the 1080-line reference
  height so the audience and previews agree at any extent.
- `src/fonts.rs` (new, no GPUI): installed-font `Catalog::scan` with bounds
  (8 MiB files, 64-face collections, 8192 faces, 32 MiB loaded-face cache),
  SHA-256-tracked faces, `Resolved` (bundled DejaVu Sans regular + Bold from
  dejavu-fonts 2.37, license note in `tests/fixtures/README.md`).
  `Resolved::bundled` resolves without I/O; scanning stays off the UI thread.
- `src/scene.rs`: `OwnedText`/`TextStyle`/`OutlineStyle`/`ShadowStyle` with
  strict bounds; `PreparedCue::from_owned` validates face index and style.
- `src/slides.rs`: format-aware cue (Bold → bundled Bold face, else synth
  margins 1/24 em; Italic 0.25 em shear margin; underline; color; H/V
  alignment); auto-size fit and `Fixed` refuse-if-unshowable; `size_cap`
  resolves the worst-case size across slides for scrolling checks.
- `src/transport.rs`: text wire tag 2 carries the resolved format (face
  index, size, color, align, valign, B/I/U flags, outline, shadow); tag 1
  rejected with a compatible-version error.
- `src/audience/text.rs`: `layers` renders fill/outline/shadow coverage
  (RGBA, `Rgba8Unorm`) via cosmic-text: per-line advance/line-height overflow
  authority, alignment, synth B/I (shear, stroke double), underline through
  cosmic decorations, chamfer dilate (outline, units are 12ths of a pixel),
  axis-generic box blur (shadow), ink clipped only at the canvas edge.
- `src/audience/compositor.rs`: three-channel WGSL blend (shadow → outline →
  fill in linear light), 64-byte uniform, colored readback checks; exact CPU
  twin `blend_pixels` for the editor preview/thumbnails.
- `src/song_library.rs`: preview and thumbnails render via the same `layers` +
  CPU blend, cache key is now `(text, format)`; the thumbnail box filter
  averages all 16 preview pixels per output pixel (the old filter sampled
  one). `src/audience.rs` prepares frames from coverage + `Blend`; text work
  stays on the worker. `src/operator.rs` resolves bundled faces per send;
  installed-family resolution is deferred to g2b.
- Decisions: fake-italic shear and negative left bearings clip at the area
  edge like other effects (the advance-fit authority rejects oversized
  text; cosmic-text drops glyphs past the buffer width either way); EW
  parity for negative-bearing rejection was not observed, recorded here.
- Tests: fonts 6; scene/slides/transport suites updated for owned text and
  the wire tag; audience text 16 (styled fill/outline/shadow geometry, synth
  B/I/U, clip-vs-overflow, whole-format 1920×1080 render); compositor colored
  readback (fill blue / outline red / 50% green shadow, half-linear green
  187–189); song library styled preview/thumbnail ink.
- Checks (Windows 11): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 99, bin 65 + 2 ignored, output
  9 + 1 ignored, transport 4 + 1 ignored), `cargo build --locked`.
- Not run: native DX12 audience-window E2E with a styled cue (g2b, needs the
  operator format-resolution slice), installed-font scan against a real
  Windows font directory (g2b), Linux/macOS.
- Next: g2b. Operator resolves installed families via the catalog with a
  loading gate, seed one formatted song, extend the song-editor and
  live-output native scripts, run the Windows DX12 audience check, measure
  layer preparation timing, update `docs/composition-text.md`, backlog and
  work log, commit.

### M1-05g2b — Installed-font gate, native DX12 qualification — 2026-10-06 (UTC+7)

- State: M1-05g **g2 done** (g2a core + g2b integration); g3 (Format pane)
  stays open, so the ticket stays active.
- One shared font store: `fonts::shared()` (catalog + bounded face cache).
  Whichever window opens first (operator or song editor) scans the system
  font directories off the UI thread and installs the catalog; resolution
  can read files and stays on background threads.
- Operator gate: a previewed song's formats resolve as one background job
  (`Item.resolved`, live item first). A Go Live/Next/Previous that races the
  resolver keeps the current live scene, shows `Resolving fonts…` and
  retries when the faces land; a late catalog scan resets items that named a
  family (their earlier resolution was the bundled fallback), re-resolves
  and refreshes the live cue, and drops the stale size cap. `send` never
  resolves on the UI thread.
- Song editor: `pixels` resolves through the shared store (off the UI
  thread), preview and thumbnails agree with the audience, and the preview
  caption shows the fallback warning (`Font "X" unavailable · showing
  DejaVu Sans`, amber) while still rendering the bundled fallback.
- Seed: `seed_library --formatted-song` now seeds a single-song library
  with three styled slides (bold gold right/bottom + Outer outline +
  shadow at a fixed size, synth italic + underline centered, installed
  Arial bold italic) for the native check.
- Native scripts: `live-output-windows.py` gains a second operator phase
  on the formatted library — color-aware checks (gold pixels, margin-based
  right/bottom assertion, centered ink box), Next-stops-at-end, clean
  exit. `song-editor-windows.py` unchanged and re-run (regression with the
  editor scan).
- Checks (Windows 11, secondary monitor 2560x1600):
  `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --
  -D warnings`, `cargo test --locked --all-targets` (lib 99, bin 67 + 2
  ignored, other suites pass), `cargo build --locked`, `ruff check
  scripts`; `scripts/live-output-windows.py` **PASS** (both phases, gold
  box 590,1378-2512,1514, margins 48/86 vs 590/1378; italic centered;
  Arial visible; both operators exit 0 and end their children);
  `scripts/song-editor-windows.py` **PASS** (preview black=38854/white=931,
  thumbnails black=16784/white=74).
- Two native iterations before PASS, both check-bound, not render bugs:
  auto-fit fills the area width, so Right alignment is invisible with a
  fitted line — the gold slide uses `Fixed(120)` — and the "pushed right"
  assertion became a margin comparison (right/bottom margins smaller than
  left/top) instead of assuming a narrow block.
- Measured (`large_text_preparation_time`, ignored): 2560x1600 1/2/4
  lines p50 5.5/14.7/26.4 ms (max 6.8/15.7/28.4); 3840x2160 p50
  7.7/20.5/34.5 ms (max 8.4/21.7/36.2). Recorded in
  `docs/composition-text.md`.
- Not run: Linux/macOS, physical-projector/output-device checks beyond the
  secondary monitor, a real owner profile with installed-family formats.
- Next: g3. Enable the toolbar Format toggle as a right pane (Text › Style
  subset), apply to the caret/selected slide, Ctrl+A whole-song selection
  with replace-typing, whole-document history, GPUI tests, extend the
  native script.
- Docs fix (2026-10-06, follow-up commit): these two entries and the
  M1-05g2 section of `docs/composition-text.md` were first written through
  PowerShell double-quoted strings, which consumed every inline-code
  backtick (and turned some following letters into control characters).
  Restored from the source; write docs with the file tools or .NET
  `File.WriteAllText`, never through PowerShell string interpolation.

### M0-02 / RUN-W06H — Ctrl+A whole-song formatting observation — 2026-10-07 (UTC+7)

- State: observation only, no code. Phase 0 of the approved M1-05g3 spec
  (owner answers: EW-style font rows with samples, full sliders and angle
  dial now, observe first with the owner present). Recorded
  EW8-OBS-034–036 in reference-observations.md; captures in
  `.amp/in/artifacts/reference/ew8-w06h-format/`.
- Result: Ctrl+A in Words selects every slide's labels and lyrics with the
  caret at the end of the last slide; Italic then applied to every slide
  (Words and Slides thumbnails); the Slides tab shows every thumbnail
  selected. Leaving Words for Slides and back ended the selection. The
  owner ended the run and confirmed that after Ctrl+A every format choice
  (font and the rest) applies to all slides.
- Not observed (Sela choices recorded in the g3 entries): undo of a
  whole-song change, typing/paste/Backspace over the selection, Copy,
  slider/dial drags, Size step amount, Reset Size/Styles, what the pane
  shows for mixed values. A synthetic Italic click focused the font field
  instead (RUN-W06G quirk); the family became Agency FB on all slides
  without an agent choice, trigger unrecorded.
- The owner closed EasyWorship himself; the agent saved nothing.
- Next: g3a Format pane (per-slide apply, sliders, dial, undo), then g3b
  Ctrl+A whole-song selection.

### M1-05g3a — Song Editor Format pane — 2026-10-07 (UTC+7)

- State: g3a **implemented-unqualified**; g3b (Ctrl+A) open, so M1-05g
  stays active. Behavior and provenance are in
  [song-library.md](song-library.md#m1-05g3a--format-pane).
- Code: new `src/song_library/format_pane.rs` (pane state, controls,
  popovers, sliders, dial, `format_targets` = the caret slide today, so
  g3b only widens that range). `song_library.rs`: toolbar button 18 toggles
  the pane, which docks right of the preview; `Preview.fitted` carries the
  laid-out size (1080-reference px) from the same off-thread raster; root
  mouse move/up listeners end a drag anywhere; undo/redo end a drag first;
  `begin` drops one; pane fields lock with the editor. `main.rs` binds the
  `SelaMenu`/`SelaSlider` keys through `song_library::bind_keys`.
- Sela choices (EW unobserved): A▾/A▴ step 2 px; a whole drag is one undo
  step; out-of-range typed values are refused rather than clamped; the
  palette is Sela's own; a dismissing click on a trigger does not reopen
  its menu.
- Fixed during the slice: the hex field stayed read-only after the
  initial library open (`sync_input_lock` locked it, nothing unlocked
  it), so `sync_pane` now resets it each render; a click on pane chrome or
  a disabled control moved focus to the window root, so the pane root
  prevents the default focus change (fields still focus first in bubble
  order).
- Checks (Windows 11): `cargo fmt --all -- --check`, `cargo clippy
  --locked --all-targets -- -D warnings`, `cargo test --locked
  --all-targets` (lib 99, bin 77 + 2 ignored, other suites pass), `cargo
  build --locked`, `ruff check scripts`; `scripts/song-editor-windows.py`
  **PASS** (preview black=38854/white=931, thumbnails black=16784/white=74,
  red preview samples 4528, stored format `01 22 00 01 FF 00 00` for slide
  1 only). Captures `08-before-format` to `12-format-saved` inspected:
  popover under Color, red bold slide 1 in preview and thumbnail, slide 2
  unchanged, disabled outline/shadow values dimmed.
- Not run: Linux/macOS, screen reader/IME, a mouse drag of a slider in the
  native script (GPUI tests cover drags), a font choice from the installed
  list natively.
- Next: g3b. `SelectAllSlides` on Ctrl+A in Words and Slides, all-slides
  visual, exit on caret/click/tab/undo, replace-typing as one undo step
  (one unlabeled slide keeping slide 1's id and format, arrangements
  pruned), Copy of the whole text, tests, native script, deferred tickets.

### M1-05g3b — Whole-song selection and replace-typing — 2026-10-07 (UTC+7)

- State: g3b **implemented-unqualified**, so M1-05g moves to
  **implemented-unqualified** (not done). Behavior and provenance are in
  [song-library.md](song-library.md#m1-05g3b--whole-song-selection).
- Code: `text_input.rs` gains `intercept_typing` (platform text and IME
  input is offered to the owner first, with whether it replaces the whole
  text) and a public `select_all`. `song_library.rs`: `SelectAllSlides`
  bound to ctrl-a/cmd-a in `SongWords > SelaTextInput` and `SongWords`
  (Words list and Slides list); `all`/`all_focus` state;
  `select_all_slides`, `exit_all`, `song_text`, `replace_all`, `all_key`;
  `whole_song_keys` captures the cell edit keys and the caret keys on the
  Words list; render ends the mode when focus enters another cell; every
  structure, history, tab and song-open path calls `exit_all`.
  `format_pane.rs`: `format_targets` is every slide while `all`.
- Sela choices (EW unobserved): the replacement is one unlabeled slide with
  slide 1's id and format and arrangements pruned; Enter replaces with a
  line break; Copy/Cut use label-above-lyrics text with blank lines between
  slides; the Text pane stays available while all slides are selected (EW
  shows only the Slide pane, EW8-OBS-035).
- Fixed during the slice: the first native run stored only "H" of
  "Hallelujah". The replacement removes the cell being typed in (slide 2),
  and Windows keeps sending characters to that removed cell's input
  handler until the next paint. A removed cell now forwards committed text
  to the focused cell (outside the Library borrow, since that cell's own
  hook reads the Library); a GPUI test drives the removed cell directly.
- Checks (Windows 11): `cargo fmt --all -- --check`, `cargo clippy
  --locked --all-targets -- -D warnings`, `cargo test --locked
  --all-targets` (lib 99, bin 79 + 2 ignored, other suites pass), `cargo
  build --locked`, `ruff check scripts`; `scripts/song-editor-windows.py`
  **PASS** (earlier steps unchanged: preview black=38854/white=931,
  thumbnails 16784/74, red 4528; then stored formats `01 22 00 01 00 FF 00`
  and `01 20 00 00 FF 00` for both slides after Ctrl+A, and after typing
  over Ctrl+A one slide "Hallelujah" with slide 1's format). Captures
  `13-all-selected` (both thumbnails framed), `14-all-green` and
  `15-typed-over` (one unlabeled slide, green bold preview) inspected.
- Not run: Linux/macOS, IME composition over the selection, screen reader,
  a native mouse click ending the selection (GPUI test covers caret keys
  and focus change), EW confirmation of the replace result and undo.
- Next: M1-05h per-song background (color or Media image), following the
  RUN-W06G Slide pane observations (EW8-OBS-032).

### M0-02 / RUN-W06I — Slide pane background observation — 2026-10-07 (UTC+7)

- State: observation only, no code. Slice h0 of the approved M1-05h spec
  (`~/.factory/specs/2026-10-07-m1-05h-per-slide-backgrounds-with-a-song-master.md`).
  Recorded EW8-OBS-037–043 in reference-observations.md; captures in
  `.amp/in/artifacts/reference/ew8-w06i-background/`.
- Documented sources read first: help.easyworship.com "Edit a Song"
  (© 2026, build not stated) and the EasyWorship blog "Individual Song
  Settings" (2020-01-17, EW7 Inspector): a background can be set per
  slide, or for the whole song through the Masters tab or Apply to Theme.
  8.0.49 has no Masters tab (EW8-OBS-021); its Master is the theme layout
  behind Edit Slide Layouts (EW8-OBS-043).
- Owner decisions before the run (binding for M1-05h): per slide with a
  master; resolution slide → song master → black (theme slots in with
  M1-08); a new image defaults to Zoom, not EW's Auto + Stretch; Maintain
  bars are black; a missing or changed image renders black with a visible
  warning in Preview, never changing the live scene; color and image in
  this ticket, gradient (EW lists Gradient Fill) and video later.
- Result: per-slide background with a theme Master fallback; a new slide
  uses the Master, not the previous slide's choice; Ctrl+A applies a fill
  to every slide and shows blank values where slides differ; Select
  Media… is a category/search/thumbnail picker without import; choosing an
  image resets aspect to Auto + Stretch; Maintain letterboxes in black.
- Not observed: Master edits reaching slides (synthetic clicks ignored in
  the Layouts tab), whether a Master edit changes the theme for other
  songs, how the Layouts tab closes, Gradient Fill controls, None fill
  output, what Auto means, background undo, saved/live rendering.
- Privacy: the first capture used a screen grab of the editor rectangle
  while another application covered it; that file was deleted unread
  beyond the first view and later captures use `PrintWindow`, or a screen
  grab of a popup rectangle only.
- The agent closed the editor with Cancel → No; nothing was saved.
- Next: h1, `src/background.rs` model, `Song.master` and
  `Section.background`, schema 4 with a verified backup, slide resolution.

### M1-05h1 — Slide background model and schema 4 — 2026-10-07 (UTC+7)

- State: h1 **implemented-unqualified**; M1-05h stays active. Contract in
  [storage](storage.md#m1-05h1--slide-backgrounds--backed-up-schema-4).
- Code: new `src/background.rs` (`Background`, `Fill`, `Aspect`, `ImageRef`,
  codec, `resolve`). `storage.rs`: `Section.background`, `Song.master`,
  validation, `SCHEMA4` (`song_backgrounds`, `section_backgrounds`), one
  backup for any schema below 4, `SELA_ABORT_MIGRATION_SCHEMA4` test hook,
  load and save. `slides.rs`: `Slide.background`, `section_slide(section,
  master)`. `song_library.rs`: the preview slide and thumbnails resolve
  against the draft's master, the cache key includes the background, and
  the whole-song replacement keeps slide 1's background. Every other new
  section (+, Ctrl+Enter split, blank song) has none, so it follows the
  master like a new EW slide (EW8-OBS-041). `scripts/song-library.py` (Linux
  replay, stale since schema 3) now expects schema 4.
- Checks (Windows 11): `cargo fmt --all -- --check`, `cargo clippy --locked
  --all-targets -- -D warnings`, `cargo test --locked --all-targets` (lib
  108, bin 79 + 2 ignored, other suites pass), `cargo build --locked`,
  `ruff check scripts`; `scripts/song-editor-windows.py` **PASS** (unchanged
  samples: preview black=38854/white=931, thumbnails 16784/74, red 4528).
- Not run: `scripts/song-library.py` (Linux/X11 replay), Linux/macOS.
- Next: h2. `background::fit` (Maintain/Stretch/Zoom to the output extent,
  black bars), an images worker `Decode` job with a bounded cache in the
  operator, a "Preparing background…" gate, black plus a Preview warning for
  a missing or changed image, editor preview/thumbnail compositing over the
  fitted image, the native live-output check.

### M1-05h2 — Slide backgrounds render — 2026-10-07 (UTC+7)

- State: h2 **implemented-unqualified**; M1-05h stays active. Behavior in
  [song library](song-library.md#m1-05h2--slide-backgrounds-render).
- Code: `background.rs` `plan` (black for none, no fill or an empty Media
  Fill) and `fit` (Zoom crop, Stretch, Maintain on black bars; `image`
  Triangle resize, exact copy at equal size). `scene.rs` shares the bounded
  PNG/JPEG decode (`decode_image`) and `PreparedBackground::Image.rgba` is
  `Arc<[u8]>`. `images.rs`: `Substitute`, `decode_background` (hash pinned,
  16384-edge/64 MiB source budget), `fitted_background`,
  `background_version`, `BackgroundCache` (96 MiB, minimum 3, LRU,
  substitutes free), `Job::Background`/`Reply::Background`. `slides::cue`
  takes the prepared background and refuses an image of another size;
  `slides::color_background`. `operator.rs`: prefetch (deferred, live,
  preview, ±1; output size or 1920×1080 while off; capped at cache
  capacity), one background job at a time, the "Preparing background…"
  gate retried on landing, black plus warning in Live status and the
  Preview pane, substitutes forgotten on a rescan, color fills in tiles.
  `song_library.rs`: `Backdrops` (profile folder, four fitted 1280×720
  images) for the preview and thumbnails, caption warning.
  `compositor.rs`: `blend_over`, and `check_readback` compares it with the
  GPU over a fitted image. The existing "colored layer blend order" check
  expected blue for an uncovered pixel over black since g2a (never run
  since); it now expects black. `seed_library --background-song`;
  `live-output-windows.py` third phase (and `ruff format` of the file).
- Checks (Windows 11, RTX 4060 Laptop): `cargo fmt --check`, `cargo clippy
  --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets`
  (lib 116, bin 83 + 2 ignored, other suites pass), `cargo build --locked`,
  `ruff check scripts`, `ruff format --check scripts/live-output-windows.py`;
  `target/debug/examples/composition_spike <out> vulkan` and `dx12` **PASS**;
  `python scripts/live-output-windows.py --out
  .amp\in\artifacts\live-output-windows-m1-05h2` **PASS** on a 2560×1600
  audience (Zoom corners green, Maintain bars black with red/blue edges,
  missing image black with the warning visible in `operator-backdrop-missing`,
  master color; observed 829/391/469/485 ms in a debug build, comparable to
  text slides); `python scripts/song-editor-windows.py --out
  .amp\in\artifacts\song-editor-windows-m1-05h2` **PASS** (samples unchanged).
- Not run: Linux/macOS, a 4K output (cache minimum path is unit-tested only),
  a separate timing of the UI-thread frame encode of the fitted image, the
  editor with an image background natively (needs h3's UI or a seeded song
  opened in the editor; unit and GPUI tests cover it).
- Next: h3. Format pane Slide tab with the Background section (Fill, color,
  Select Media… with Import, Aspect), master editing, Ctrl+A, undo, GPUI
  tests, `song-editor-windows.py`; then the new tickets (gradient, opacity/
  rotate/flip, Theme Elements/Edit Slide Layouts, video backgrounds).

### M1-05h3 — Slide pane backgrounds — 2026-10-07 (UTC+7)

- State: h3 **implemented-unqualified**; **M1-05h implemented-unqualified**.
  Behavior in [song library](song-library.md#m1-05h3--slide-pane-backgrounds).
- Code: `format_pane.rs` gets `Tab` (Slide, Text), `FillChoice`, `Shown`
  (blank when Ctrl+A slides differ), `Menu::{Fill, Aspect, Media}`,
  `Pick::Background`, new `Ctl`s (tab order 990–996, before the Text tab's
  controls), `Media` (listing and up to 256 thumbnails on the background
  executor, one job at a time), `pane_background`, `apply_background`,
  `color_of`, `choose_fill`, `choose_image`, `pick_media`, `import_media`,
  `edit_layouts`, the Slide tab and media popover; the Text tab moved into
  `text_tab` unchanged. `song_library.rs`: `layouts` state, Layouts ⊗ tab
  (button 19) with the Master card and `LAYOUT_SAMPLE` preview, Words and
  Slides close it, Ctrl+A turns the pane to the Slide tab, history bytes
  count image names. `images.rs`: `image_ref` and `thumbnail`.
- Decisions: Sela's "Master" fill row stands for "follows the master" (EW
  has no reset, EW8-OBS-041); Ctrl+A shows the Slide tab like EW but keeps
  the Text tab reachable (deviation from EW8-OBS-035, keeps the
  owner-reported "every Format choice applies to all"; awaiting owner
  confirmation); a fill chosen while slides differ gives all of them the
  same background, while Aspect keeps each slide's image; Import… in the
  picker is Sela's (EW has none) and calls `images::import` from a
  background task, since the editor has no images worker; entering Layouts
  focuses Fill ▾ (the Words cells are not rendered, and without a focused
  element Ctrl+Z did nothing; the GPUI test caught it).
- Checks (Windows 11, RTX 4060 Laptop): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked
  --all-targets` (lib 117, bin 86 + 2 ignored, other suites pass), `cargo
  build --locked`, `python -m ruff check scripts`, `python -m ruff format
  --check scripts/song-editor-windows.py`; `python
  scripts/song-editor-windows.py --out
  .amp\in\artifacts\song-editor-windows-m1-05h3` **PASS** (earlier samples
  unchanged: preview black=38854/white=931, thumbnails 16784/74, red 4528;
  new: orange Color Fill 69457, Zoom sky 69388 with black 0, Maintain black
  41496, master in Layouts 67992, master on slide 2's thumbnail 8458; SQLite
  rows match). Captures 17 (picker), 19 (Maintain) and 20 (Layouts)
  inspected. Passed twice (before and after `cargo fmt` and the docs); one
  run in between failed at the first save with the label stored as
  " Verse 1" (a stray leading space, before any Format pane step); its
  captures were overwritten by the next, passing run, so the cause is
  unrecorded. Watch for it in later runs.
- Not run: `scripts/live-output-windows.py` (operator code unchanged since
  its h2 pass), Linux/macOS, screen reader, mouse-only use of the picker
  (GPUI tests click it; the native run uses the keyboard), a profile with
  many large images (thumbnail time unmeasured), EW confirmation of the
  unobserved Master behavior.
- GPUI references: pin `a84689073d`, `crates/gpui/src/app.rs`
  (`prompt_for_paths`), `crates/gpui/src/platform/test/platform.rs`,
  `crates/gpui/src/app/test_context.rs` (`simulate_path_prompt_response`),
  `crates/gpui/src/elements/uniform_list.rs`.
- Next: M1-05i, the operator Library right-click New Song…/Edit Song…/Delete
  (EW8-OBS-021). Ask the owner about the Ctrl+A Text-tab deviation and, in
  an EW session, whether a Master edit reaches slides without their own
  background.

### M1-07a — Search index backend and schema 5 — 2026-10-07 (UTC+7)

- State: **implemented-unqualified**, backend slice of M1-07 in parallel
  wave 1 (worktree `D:\Projects\sela-wt\m1-07`, branch
  `ticket/m1-07-search`, base `527030a`). No UI. Design, query rules and
  measurements in [search](search.md); schema in
  [storage](storage.md#m1-07a--search-index--backed-up-schema-5).
- Delivery note: the worker session was interrupted once and then stalled
  for about 100 minutes, so the parent stopped it and finished the slice from its uncommitted code (applied `cargo fmt`,
  renamed `Generations::next` to `advance` for clippy's
  `should_implement_trait`, ran the benchmark and wrote the docs). The
  worker's own benchmark run had failed with exit code 4. The likeliest
  cause is a killed command: the half-written temp database and journal it
  left matched an interrupted run, and the bench takes about 4 minutes.
- Code: `src/search.rs` (new: `expression`, `Hit`, `Results`, `IndexState`,
  `Generations`, `MAX_QUERY_BYTES`, `MAX_HITS`); `src/storage.rs` (schema 5
  `search_rows` and FTS5 `song_search`, the 4→5 migration behind
  `.schema4-backup`, `index_song`/`unindex_song` in the save and delete
  transactions, `search`, `check_search`, `rebuild_search`,
  `repair_search`, open-time index recovery, `Command::Search`,
  `RepairSearch`, `RebuildSearch`, `Reply::Search`, `Reply::SearchIndex`);
  `examples/search_bench.rs` (new); `src/lib.rs` exports `search`.
- Decisions (provisional, EW search unobserved):
  - Tokenizer `unicode61 remove_diacritics 2`. Punctuation splits words, so
    `kasihmu` does not find `Kasih-Mu`.
  - Every term is quoted text, terms are ANDed, and only the last term is a
    prefix.
  - Empty or punctuation-only queries return nothing without I/O.
  - At most 200 hits, with `truncated`.
  - bm25 weights: title 10, lyrics 1, metadata 2. Ties break by title
    ignoring ASCII case, then exact title, then id.
  - Labels are not indexed.
  - The index is derived and is rebuilt when damaged. Damage outside it
    keeps the library closed.
- Checks (Windows 11, i7-14650HX, 48 GB):
  - `cargo fmt --all -- --check` and `cargo clippy --locked --all-targets
    -j 8 -- -D warnings` pass.
  - `cargo test --locked --all-targets -j 8` passes: lib 134 (new: 13
    storage and 4 search tests), bin 86 + 2 ignored, output_process 9 + 1
    ignored, transport_process 4 + 1 ignored, composition_spike 11,
    input_check 9, native_cues 17 + 1 ignored, output_spike 1, video_spike
    12.
  - `cargo build --locked -j 8 --example seed_library` passes.
- Benchmark: `cargo run --locked --release -j 8 --example search_bench`
  (20,000 songs, seed `0x5e1a0007`), database query latency only.
  - Pooled 1,400 samples: p50 18.1 ms, p95 75.9 ms, max 119.8 ms. Within
    the provisional 30 ms target and 100 ms p95 ceiling.
  - Misses: the one-letter prefix `k` has p95 109.4 ms (over the ceiling).
    Broad queries (most common word, two common words, two-letter prefix)
    have p50 of 44–49 ms, above the target.
  - Other costs: open 451 ms (integrity and consistency checks on the
    storage worker), full check 388 ms, rebuild 1.46 s, `save_song` p50
    7.8 ms and p95 12.9 ms.
  - Full table in [search](search.md#benchmark-examplessearch_benchrs).
- Not run:
  - Search UI and end-to-end latency (no UI in this slice).
  - EW 8.0.49 search observation (W04).
  - Linux and macOS.
  - The older integrated-GPU rehearsal laptop.
  - A save-time baseline without the index.
  - Native scripts (no operator change).
- Next:
  - The operator Library search box (focus ownership so typing never fires
    live shortcuts, `Generations` for stale replies, empty, loading and
    error states) after observing EW search in W04.
  - Measure before choosing a fix for short-prefix cost: start the search
    at two characters, order very short prefixes by title, or add an FTS5
    prefix index.

### M1-11a — Image import hardening and preflight API — 2026-10-07

- State: M1-11a **implemented-unqualified**; M1-11 **active** (M1-11b
  operator UI next). Owner-approved parallel wave, branch
  `ticket/m1-11-images` from `527030a`. Design, limits and decisions in
  [images](images.md#m1-11a--import-hardening-and-preflight-api). No schema
  change, no operator or editor code change.
- Code: `images.rs` gets `MAX_FILE_BYTES`, `MAX_EDGE`, `MAX_DECODED_BYTES`,
  new `Error` variants (`SourceMissing`, `Dimensions`, `Animated`,
  `Corrupt`, `ReadDenied`, `ProfileDenied`, `DiskFull`, `Cancelled`,
  `Mismatch`, `NameTaken`) with recovery messages, header `inspect`
  (`PngDecoder::with_limits`, APNG, orientation, ICC), `import_checked`
  returning `Imported`, the temporary-and-hard-link copy (`link_new`, with a
  placeholder-and-rename fallback), `relink`/`Relinked`, `content_hash` and
  `Worker::cancel_imports`. `import(profile, source) -> Result<String, Error>`
  keeps its signature. `scene::decode_image` applies the Exif orientation
  (returns the displayed extent) and bounds the decoder buffer itself
  (`TooLarge`), since it now uses `into_decoder`. New `preflight.rs`:
  `check`, `Report`/`ItemReport`/`ImageIssue`/`FontIssue`/`LogoIssue`/`Use`
  and `Worker` (check generations, relink requests).
- Decisions: managed copies only; identical bytes under the same name (or a
  numbered variant) are reused instead of copied, other names are copied
  again; same name with different bytes gets " (N)"; APNG refused rather
  than first frame (EW unobserved); ICC ignored and sRGB assumed; Exif
  orientation applied everywhere the shared decoder is used (backgrounds,
  thumbnails, editor preview, logo); a relink never replaces a different
  file under the pinned name (`NameTaken`); the logo is not pinned, so
  preflight reports it only when unreadable; fonts unchecked (not missing)
  until the catalog is scanned.
- Checks (Windows 11): `cargo fmt --all -- --check` pass; `cargo clippy
  --locked --all-targets -j 6 -- -D warnings` pass; `cargo test --locked
  --all-targets -j 6` pass (lib 130 + 1 ignored, bin 86 + 2 ignored,
  output_process 9 + 1 ignored, transport_process 4 + 1 ignored, examples
  pass); `cargo build --locked -j 6 --example seed_library` pass. New tests
  present in the output: `images::tests::{import_reuses_identical_bytes_and_keeps_each_version_of_a_name,
  import_refuses_oversized_files_and_decompression_bombs,
  import_refuses_animated_png, import_and_decode_apply_exif_orientation,
  cancelled_import_leaves_no_file_even_mid_copy,
  worker_cancels_submitted_imports_only,
  relink_restores_only_the_pinned_bytes,
  io_failures_name_the_side_and_the_recovery}` and
  `preflight::tests::{preflight_reports_missing_and_changed_images_and_missing_fonts,
  preflight_reports_the_logo_and_unscanned_fonts,
  preflight_reports_a_catalogued_font_whose_file_is_gone,
  preflight_work_is_bounded, worker_checks_then_relinks_only_a_matching_file}`.
  `cargo test --locked --lib -j 6 -- images::tests::permission --ignored`
  pass (`icacls` deny write on the images folder gives `ProfileDenied` with
  no file left; deny read on the source gives `ReadDenied`). With the
  `apply_orientation` call removed, the orientation test failed (extent
  2×1 instead of 1×2), so it checks the behavior.
- Not run: the `cfg(unix)` `chmod` permission test (Windows host);
  Linux/macOS; native scripts (no operator change; the parent runs them at
  the wave merge); a large real camera photo or wide-gamut image; FAT/exFAT
  volume for the link fallback (only the hard-link path ran); decode time
  and memory measurement of a 64 MiB import.
- Next: M1-11b. Operator preflight when a schedule opens and before Live,
  the report view, Locate… (file prompt, `preflight::Request::Relink`),
  import Cancel (`Worker::cancel_imports`) and `Imported` notes; then the
  thumbnail pipeline item and an EW media-import observation.

### M1-05i — Operator song menu — 2026-10-07

- State: M1-05i **implemented-unqualified** on branch
  `ticket/m1-05i-song-menu` (base `527030a`). Behavior in
  [operator shell](operator-shell.md#m1-05i--operator-song-menu).
- Code: `operator.rs` adds `SongItem`/`SONG_MENU` (EW8-OBS-021 order; Update
  items in Schedule and Sort by ▸ disabled), `SongMenu`, the
  `deferred(anchored())` menu overlay with its own focus handle and
  `SelaSongMenu` key context, actions `OpenSongMenu`, `MenuNext`,
  `MenuPrevious`, `MenuConfirm`, `MenuDismiss` (bound in `Operator::new`;
  `main.rs` unchanged), `Dialog::DeleteSong`, `Request::DeleteSong` through
  the existing single storage job slot, a red/neutral `song-message` in the
  Songs footer, row bounds from `on_children_prepainted` for keyboard
  placement, and an observer of the new `LibraryChanged` global that marks
  Songs stale. `song_library.rs` adds `LibraryChanged` (bumped on editor
  save and delete), `open_song(path, Option<Version>, cx)` (`open` keeps its
  signature) and `open_version` (selected when the editor's catalog
  arrives), plus test-only probes.
- Decisions: Delete asks first with Keep focused (EW wording unobserved);
  the storage soft delete keeps schedule snapshots, and nothing is sent to
  the audience; Preview keeps a deleted song's slides; one delete at a time
  (a second one is refused with a message, not queued); Shift+F10 and the
  Menu key open the menu below the focused row with the first item
  highlighted, and the highlight wraps over enabled items. The EW
  observation was skipped by owner decision; these are provisional.
- Checks (Windows 11): `cargo fmt --all -- --check` OK; `cargo clippy
  --locked --all-targets -j 6 -- -D warnings` OK; `cargo test --locked
  --all-targets -j 6` OK (lib 117; bin 90 + 2 ignored, including the new
  `song_menu_items_order_disabled_items_and_dismissal`,
  `edit_song_opens_that_song_new_song_a_blank_one_and_a_save_refreshes_songs`,
  `deleting_a_scheduled_live_song_keeps_snapshots_and_the_live_scene`,
  `a_failed_delete_shows_why_and_keeps_the_row`; output_process 9 + 1,
  transport_process 4 + 1, composition_spike 11, input_check 9,
  native_cues 17 + 1, output_spike 1, video_spike 12); `cargo build
  --locked -j 6 --example seed_library` OK; `python -m ruff check scripts`
  OK; `python -m ruff format --check scripts/operator-menu-windows.py` OK
  (four older scripts fail `ruff format --check scripts`, unchanged).
- Not run: `scripts/operator-menu-windows.py` (written for this ticket, not
  executed in this session), `scripts/live-output-windows.py` and
  `scripts/schedule-windows.py` after the operator changes, Linux/macOS,
  screen reader, EW observation.
- GPUI references: pin `a84689073d`, `crates/gpui/src/elements/anchored.rs`,
  `crates/gpui/src/elements/deferred.rs`, `crates/gpui/examples/popover.rs`,
  `crates/gpui/src/elements/div.rs`, `crates/gpui/src/app.rs`,
  `crates/gpui/src/app/context.rs`, `crates/gpui/src/keymap/context.rs`,
  `crates/gpui/src/window.rs`, `crates/gpui_windows/src/events.rs` (VK_APPS
  is the `menu` key).
- Next: run `python scripts/operator-menu-windows.py --out
  .amp\in\artifacts\operator-menu-windows-m1-05i` on Windows and rerun
  `scripts/live-output-windows.py` and `scripts/schedule-windows.py`; in an
  EW session, observe menu keyboard access, the Delete confirmation, Update
  items in Schedule and Sort by ▸. (Native runs done in the wave-1
  integration record below.)

### Parallel wave 1 — Parent integration — 2026-10-07 (UTC+7)

- Scope: the owner-approved wave
  (`C:\Users\thoma\.factory\specs\2026-10-07-parallel-wave-1-m1-05i-m1-07-search-backend-m1-11-images-and-preflight.md`).
  - Worktrees under `D:\Projects\sela-wt\`, all based on `527030a`:
    - `m1-05i` (`ticket/m1-05i-song-menu`)
    - `m1-07` (`ticket/m1-07-search`)
    - `m1-11` (`ticket/m1-11-images`)
  - Each worktree used its own `target\`, so no build output was shared.
  - One background worker per stream. The parent merged and ran the native
    checks. EW observation was skipped by owner decision.
- Workers:
  - A (M1-05i) committed `4f3ff30`.
  - E (M1-11a) committed `65581f5`. It also edited `scene::decode_image`
    (orientation), outside its listed files; no other stream touched it.
  - B (M1-07a) was interrupted, then stalled for about 100 minutes with
    nothing committed. The parent stopped it and finished the slice from
    its code as `3cf8130` (see the M1-07a record).
- Merges (`--no-ff`, order B, E, A):
  - `a393301` M1-07a, clean.
  - `61515e2` M1-11a.
  - `f1245d4` M1-05i.

  The only conflicts were the appended session records at the end of this
  file, and both merges kept every record. The parent also set the M1-07
  status row to `active` (backend slice only). The first A merge commit
  was made with conflict markers still in this file (an edit failed and a
  commit in the same batch ran anyway); it was amended locally before any
  check or push.
- Checks on merged `main` (Windows 11, i7-14650HX, RTX 4060 Laptop):
  - `cargo fmt --all -- --check` and `cargo clippy --locked --all-targets
    -- -D warnings` pass.
  - `cargo test --locked --all-targets` passes:
    - lib 147 + 1 ignored (117 before the wave, plus B's 17 and E's 13;
      the ignored one is E's Windows permission test);
    - bin 90 + 2 ignored (A's 4 new);
    - output_process 9 + 1, transport_process 4 + 1, composition_spike 11,
      input_check 9, native_cues 17 + 1, output_spike 1, video_spike 12.

    The counts equal the branch sums, so no stale artifact was reused.
  - `cargo build --locked` and `cargo build --locked --example
    seed_library` pass.
  - `python -m ruff check scripts` and `git diff --check 527030a HEAD`
    pass.
- Native checks, run serially with the owner's keyboard and mouse idle:
  - `python scripts/operator-menu-windows.py --out
    .amp\in\artifacts\operator-menu-windows-m1-05i` **PASS** on the second
    run. The first run failed "Escape left the operator different from
    before the menu": opening the menu selects its row (the designed
    behavior), and the script's baseline had only focused the row, so
    Preview changed. The script now presses Space before the baseline.
    Second run: menu 64,794 pixels, Escape residue 0, Keep kept, Delete
    deleted, the saved schedule pins the deleted song, exit 0. Captures 00–08
    inspected: the confirmation names the song and says schedules keep
    their copy and live output does not change, with Keep focused; after
    the delete the schedule keeps the song and the footer confirms.
  - `python scripts/song-editor-windows.py --out
    .amp\in\artifacts\song-editor-windows-wave1` **PASS**. Every sample is
    identical to the M1-05h3 run.
  - `python scripts/live-output-windows.py --out
    .amp\in\artifacts\live-output-windows-wave1` **PASS** (masks, logo,
    formatted and background songs).
  - `python scripts/schedule-windows.py --out
    .amp\in\artifacts\schedule-windows-wave1` **PASS**.
- Lessons for the next wave:
  - Give workers long command timeouts for release benchmarks.
  - Have them commit a `wip` checkpoint before long runs.
  - In the parent, do not batch a commit after edits that might fail.
  - Use absolute paths in .NET file calls. `[IO.File]` resolves relative
    paths against the process directory, and one such call wrote an empty
    `src/search.rs` into the main checkout; it was found and removed before
    any commit.
- Not run: Linux/macOS, screen reader, EW observation (menu keys, Delete
  confirmation, search behavior, media import), the M1-15 rehearsal laptop.
- Worktrees and branches are kept for reference. Nothing is pushed.
- Next:
  - M1-11b: operator preflight report, Locate…, import Cancel.
  - M1-07b: Library search box. It needs the W04 EW search observation
    first.
  - Or a second wave pairing M1-11b with an independent backend ticket
    (for example M1-14 OpenLyrics core). M1-11b and the search box both
    touch `operator.rs`, so they should not run in parallel.

### M0-02 / RUN-W07A — Foundation and Sunday-core EW observation — 2026-10-08

- State: observation only, no code. Recorded EW8-OBS-044–066 in
  `reference-observations.md`. Captures (ignored) under
  `.amp/in/artifacts/ew-w07*/`.
- Decisions: library search is live from 1 character and word-prefix over
  title, labels and lyric words (Sela keeps the >=2 character FTS decision as
  a deviation); Left/Right/Home/End navigate slides, PageDown/PageUp navigate
  schedule items; Delete confirm defaults to No.
- UNOBSERVED with best guesses: label jump keys, Master edit reach, Update
  items dialog wording, Administrator/CCLI rendering, schedule multi-select
  and drag reorder, Preview Up/Down.
- Correction (fix-foundation-observation-coverage, docs only): the best guesses
  for schedule drag reorder (EW8-OBS-060) and Administrator/Song Number-CCLI
  (EW8-OBS-063) did not exist in the first commit; they were added afterwards,
  with the plain B/C/L-in-search and Ctrl+B/L/C-in-search rules (EW8-OBS-047),
  the label-jump pane and Live target (EW8-OBS-052), the Master edit
  cross-song/theme rule (EW8-OBS-057), the Options text lists (EW8-OBS-056) and
  ledger rows for EW8-OBS-044-066. No new EW run: the search-field check stays
  UNOBSERVED, so EW was not launched.
- Checks: EW closed via WM_CLOSE, `Get-Process EasyWorship` empty; the
  unsaved-schedule prompt was answered No; nothing saved.
- Next: consume the records in M1-07b (search), M1-02/M1-03 (keys), M5-06
  (Options).

### Foundation / operator-module-split-tokens — Operator module split and design tokens — 2026-10-08

- State: implemented-unqualified (refactor only, no behavior change intended).
- Scope: `src/operator.rs` (3399 lines) became `src/operator/` with `mod.rs`
  (struct, state, `new`), `controls.rs` (typed `Control` enum, `ControlSet`
  index-by-`Control`, `tab_position`, `activate`, `button`/`mask_button`),
  `layout.rs` (root `Render`, dimensions, splitters), `schedule.rs`,
  `preview_live.rs`, `polling.rs`, `menus.rs`, `dialogs/mod.rs`, and
  `resources/{mod,songs,media,scriptures,presentations,themes}.rs` (the last
  three are placeholders that render the existing empty state). Largest file
  is `preview_live.rs` at ~800 lines. New `src/ui/theme.rs` holds colors,
  type sizes, bar heights and radii; `operator/` has no `rgb(0x…)` literals
  (enforced by a unit test). `song_library.rs` and `format_pane.rs` use the
  same tokens for chrome; the Format pane's border moved from `0xcbd0d6` to
  the shared `BORDER` `0xdcdedc` (intentional, one-step lighter) and its muted
  text is `FAINT`.
- Decisions: methods moved as `impl Operator` blocks with `pub(crate)`
  visibility; fields stay private/`pub(super)` on the struct in `mod.rs` so
  children read them. `src/tests.rs` changed only to use `Control` instead of
  bare indices; no assertion changed.
- Upstream zed paths consulted: none new (no new GPUI patterns; module and
  token layout only). Pinned rev stays `a84689073d296dfd39987bc7dd478e43ef76d83a`.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets
  -- -D warnings`, `cargo test --locked --all-targets -- --test-threads=12` all
  exit 0 (bin 94 passed incl. 4 new `ui::theme` tests). Native, one at a time:
  `live-output`, `schedule`, `song-editor`, `operator-menu` scripts all exit 0
  with PASS/ok. Before/after: 13 `operator-*.png` captures from
  `live-output-windows.py` are byte-identical (SHA-256) between
  `.amp/in/artifacts/split-before` and `split-after/live-output`; the song
  editor Format pane capture was viewed and looks correct.
- Unexecuted: `ruff` not rerun on scripts (none changed); VAL-SHELL-007 sampled
  pixel table across song editor/Options (no Options dialog yet) is left to the
  validator.
- Next: later features add tokens to `ui/theme.rs` instead of inline `rgb()`.
### Foundation / settings-store-options-dialog — M1-16 settings slice — 2026-10-08

- State: implemented-unqualified (settings slice of M1-16 only).
- Scope: `src/settings.rs` (lib, framework-free): `Settings { output_monitor }`,
  text format `sela-settings 1` + `key=value`, unknown keys ignored, newer
  version or any parse error = unreadable. `load` returns defaults silently for
  a missing file; for an unreadable one it renames the file to
  `settings.txt.bad[N]` (never deleted) and returns one note. `save_with` writes
  `settings.tmp`, `sync_all`, then replaces via an injectable rename hook, so a
  failure before rename keeps the old file. `Worker` owns a thread with a
  bounded queue (8) and a polled reply channel; the writer is injectable.
  `MonitorInfo` + `monitor_arg`/`monitor_missing` map the saved device name to
  the `--audience` argument. Binary: `sela --list-monitors` (winit, run on a
  background executor thread from the operator) prints name/size/position/
  primary; `audience::Monitor::Named` resolves a device name and falls back to
  the first non-primary monitor when it is gone (never the primary).
  `src/operator/options.rs`: `SettingsState`, `MonitorSlot` (read by the
  launcher at spawn, `SELA_AUDIENCE_MONITOR` still wins), `poll_settings`,
  the Options overlay (sections Main Output, Alternate Output, Foldback,
  Service Intervals, Slide Labels, Advanced from EW8-OBS-054; only Main Output ›
  General with the monitor list is live, the rest are dimmed and labelled "not
  available"; tabs Song..Alerts likewise) using `ui/theme.rs` tokens only. New
  `Control::Options` (top-bar "Options…", tab stop after Save). OK applies the
  draft, queues the save on the worker, updates the slot and, if Live output is
  on, restarts it so the audience moves; Cancel discards. Saved monitor missing
  from the list: status-bar note, fallback to the default non-primary monitor,
  file not rewritten until OK with a changed choice.
- Decisions: hand-written line format instead of a new serde/toml crate (one
  key today); monitors identified by winit device name (`\\.\DISPLAYn`);
  Options is a top-bar button because the operator has no menu bar yet (EW has
  Edit › Options, EW8-OBS-053). Language setting waits for i18n.
- Upstream zed/gpui paths consulted: none new (existing overlay/focus patterns).
- Checks (all exit 0): `cargo fmt --all -- --check`; `cargo clippy --locked
  --all-targets -- -D warnings`; `cargo test --locked --all-targets --
  --test-threads=12` (lib 158, bin 99 incl. 5 `settings_ui` tests, 11 settings
  unit tests); `python -m ruff check scripts`. Native, one at a time:
  `scripts/options-windows.py` PASS (audience default on DISPLAY1; Options
  pick DISPLAY6 + OK moves the audience to DISPLAY6 and writes
  `output_monitor=\\.\DISPLAY6`; restart opens the audience on DISPLAY6;
  Cancel leaves settings.txt byte-identical; missing `DISPLAY99` shows the note,
  audience on DISPLAY1, file unchanged; truncated file starts with defaults and
  keeps `settings.txt.bad`); `live-output`, `schedule`, `operator-menu`,
  `song-editor` PASS. The new top-bar stop shifted tab counts: live-output
  `TABS_TO_FIRST_SONG` 25→26 and shift-tab 15→16, schedule shift-tab 11→12; tab
  order tests updated. Evidence: `.amp/in/artifacts/options-windows/`
  (`summary.json`, `01-options-open.png` … `06-corrupt-note.png`).
- Unexecuted: VAL-SHELL-007 pixel table comparing the song editor, main window
  and Options at fixed positions (`summary.json` only records that the border,
  chrome, selected-row and focus tokens appear in the Options capture; all use
  the shared tokens); screen-reader exposure; Indonesian strings (i18n not
  landed).
- Next: i18n language selector in Options, remaining Main Output tabs, Reassign
  on the output warning, menu-bar placement.

### Foundation / i18n-infrastructure — M1-16 language slice — 2026-10-08

- State: implemented-unqualified (language slice of M1-16 only).
- Scope: `src/i18n.rs` (lib, framework-free): a compile-time `Msg` enum (327
  messages) with exhaustive `english()`/`indonesian()` tables — key parity is
  structural — a thread-local active `Language` (UI thread only; every test
  thread defaults to English), `text()` falling back to English for an empty
  translation (never a raw key or blank label), and a `t!` macro:
  `t!(Msg::X)` for plain strings, `t!(Msg::X, name = value)` for `{name}`
  placeholders. `settings.rs` gains `language` (`language=id` written only
  when non-default; unknown codes ignored like unknown keys; file version
  stays 1). All operator chrome (toolbar, mask buttons, pane headers, status
  bar, resource tabs, context menus, schedule/song/Options dialogs, Live pane
  status lines, background-substitution warnings) and all Song Editor strings
  (window title, toolbar, tabs, placeholders, status messages, Format pane)
  render through the table. The Options dialog (Main Output › General) has a
  Language radio list (English / Bahasa Indonesia, labels constant across
  languages) above the monitor list; OK applies the draft: on a language
  change `i18n::set_language` + `cx.refresh_windows()` repaint every window
  immediately, no restart, and the worker persists the choice. The monitor
  restart-on-change path only runs when the monitor actually changed, so a
  language-only switch never touches the audience (VAL-I18N-004). The editor
  status `starts_with` rejection probe became a `status_is_rejection` flag so
  no logic matches translated text. Element ids and debug selectors stay
  static English (tests and scripts find controls in any language); the
  slide_pane and Options section/tab selectors got explicit stable slugs.
- Decisions: thread-local over a global atomic so parallel tests are isolated
  and the audience child process (separate process, default English) can never
  be affected; `Language::label()` is deliberately not translated; error
  payload text from lib `Display` impls (storage/images/output refusals)
  stays English inside `{error}` placeholders — full translation of those is
  VAL-L10N-003 territory. Indonesian glossary per mission directive: Jadwal,
  Pratinjau, Langsung, Lagu, Alkitab (tab) / Ayat (reference), Hitam/Kosong/
  Logo masks, Tayangkan for Go Live. Options dialog height 372→500 px for the
  new section; section labels truncate before touching the "not available"
  tag.
- Upstream zed/gpui paths consulted: none new (existing overlay/focus
  patterns).
- Checks (all exit 0): `cargo fmt --all -- --check`; `cargo clippy --locked
  --all-targets -- -D warnings`; `cargo test --locked --all-targets --
  --test-threads=12` (lib 164 incl. i18n parity/placeholder/fallback/
  substitution/code tests and the settings language round-trip; bin 100
  incl. `settings_ui::language_switches_at_runtime_and_persists`);
  `python -m ruff check scripts`. Native, one at a time:
  `scripts/i18n-windows.py` PASS (Go Live in English, switch to Indonesian
  without restart: settings.txt gains `language=id`, audience same process
  with byte-identical pixels; restart opens Indonesian with Bahasa Indonesia
  selected in Options; switching back clears the key and the next restart
  opens English; captures `01`–`10` viewed: all chrome Indonesian/English as
  expected, song title and lyrics unchanged, no clipped or overlapped labels);
  baselines `options-windows`, `live-output-windows`, `schedule-windows`,
  `song-editor-windows`, `operator-menu-windows` all PASS. Evidence:
  `.amp/in/artifacts/i18n-windows/` (`summary.json`, PNGs) and
  `*-after-i18n/` for the baselines.
- Unexecuted: Indonesian Song Editor native capture (editor strings are
  migrated and unit-tested in English; full-UI Indonesian review is
  VAL-L10N-002/004); screen-reader exposure of the new radio rows;
  plural/date/number rules (none needed by current messages).
- Next: remaining Main Output tabs, Reassign on the output warning, menu-bar
  placement (EW Edit › Options), M1-07 search box.

### Foundation / main-menu-bar — M1-02d menu bar — 2026-10-08

- State: implemented-unqualified (VAL-SHELL-011; disabled-item enablement,
  mnemonic confirmation and screen-reader exposure remain open).
- Scope: an EasyWorship 8.0.49 menu bar (File, Edit, Live, Profiles, View,
  Help; EW8-OBS-010/053, Profiles/View/Help contents from
  `ew-w07d-options/menu-*.png`) as a 28 px in-window row above the toolbar —
  GPUI app menus are macOS-only. Alt+F/E/L/P/V/H opens menus with the first
  enabled item highlighted; mouse clicks open/switch/close, hover switches
  while open, and a press that dismisses is swallowed like a Windows menu;
  Down/Up/Tab/Shift+Tab move the highlight (wrapping, skipping separators,
  headers and disabled rows), Left/Right switch menus, Enter/Space chooses,
  Esc restores the pre-menu focus. File/Edit/Live dispatch the existing
  commands (New Song/Schedule, Open/Save Schedule, Options, Go Live,
  Logo/Black/Clear, Show Live Output, Exit) through the same Operator methods
  as the toolbar and shortcuts; everything not yet built renders disabled and
  inert. The toolbar Options button is removed (EW has no equivalent; Options
  is Edit › Options). All strings via `src/i18n.rs` (42 new `Msg` entries,
  EN+ID; table guard 327→369). New actions: `OpenFile/Edit/Live/Profiles/
  View/HelpMenu`, `BarMenuLeft/Right`, plus the shared `MenuNext/…` set in the
  new `SelaMenuBar` context; `SelaShow` bindings now exclude `SelaMenuBar`.
- Decisions/deviations: File › New ▸ is flattened to an inert "New ▸" header
  row plus Song…/Schedule items — the GPUI popover supports one level;
  Alt+mnemonics other than Alt+L are unobserved first-letter best guesses
  (recorded as provisional in operator-shell.md); the bar is not a Tab stop
  so pane traversal counts were preserved except for the removed Options
  button (25 tabs to the first song; 15 shift-tabs slide→Live output; 14
  collapsed). Bug found by the native run: GPUI's `div` track_focus logic
  auto-focuses an ancestor (the root) on a bubble-phase mouse down, so a
  mouse-opened menu lost its keyboard focus — fixed with
  `window.prevent_default()` in the label's mouse-down listener plus
  `cx.stop_propagation()` in the dismissing down-out listener (GPUI
  `a84689073d` `div.rs::paint_mouse_listeners`, `window.rs` click synthesis).
- Upstream zed/gpui paths consulted: `crates/gpui/examples/popover.rs`,
  `crates/gpui/src/elements/{div,deferred,anchored}.rs`,
  `crates/gpui/src/window.rs`, `crates/gpui/src/keymap/context.rs`,
  `crates/gpui/src/app/test_context.rs`.
- Checks (all exit 0): `cargo fmt --all -- --check`; `cargo clippy --locked
  --all-targets -- -D warnings`; `cargo test --locked --all-targets --
  --test-threads=12` (lib 164; bin 109 incl. nine new `tests::menu_bar`
  tests: label order, per-menu contents against the captures, mouse
  open/hover-switch/swallow/dismiss, Alt+Esc focus restore, arrow/wrap
  navigation, dispatch parity, disabled inertness, text-field isolation);
  `python -m ruff check scripts` (menu-bar-windows.py also ruff-formatted;
  the other five unformatted scripts predate this change and CI does not
  gate on ruff format). Native, one at a time: `scripts/menu-bar-windows.py`
  PASS (Esc-restores-focus probe, all six menus captured via Alt and mouse,
  toggle/outside dismiss, disabled Open Recent inert, menu-driven Logo/
  Black/Clear with audience pixel checks and lit toolbar buttons, Show Live
  Output off/on ending and restarting the audience child, Edit › Options,
  File › Save As "Menu Set" / Open / Exit, clean exit 0;
  `.amp/in/artifacts/menu-bar-windows/summary.json` + PNGs; `menu-file`,
  `menu-live`, `menu-profiles` and `options-from-menu` captures viewed).
  Baselines `live-output-windows`, `schedule-windows`,
  `song-editor-windows`, `operator-menu-windows`, `options-windows` and
  `i18n-windows` updated (tab counts, menu-based Options opening, lit-pixel
  band offset by the menu row) and all PASS.
- Unexecuted: EW confirmation of the five unobserved mnemonics and of the
  flattened New ▸ submenu; screen-reader/UIA exposure of the bar; menu
  captures at the collapsed window width; physical multi-DPI run (this run
  was at 1.25 scale).
- Next: enable the disabled items with their features (Open Recent, Save
  As…, clipboard Edit items, Go Back, Profiles/View/Help), M1-07 search box,
  remaining Main Output tabs.

### M1-16-a11y — AccessKit roles/names and UIA verification — 2026-10-08 (Asia/Jakarta)

- Scope: stable element ids, AccessKit roles and localized names for every
  critical main-window control (menu bar/menus/items, toolbar, masks, Live
  output, Resources tabs, Songs/Schedule/saved-schedule/Media lists and
  rows, Preview/Live slide tiles, popups); ToggleState for Black/Clear/Logo
  (Mixed while pending) and Live output; selected/expanded/disabled states;
  a documented keyboard Tab order; Songs-row Down/Up selection movement;
  `scripts/uia-names-windows.ps1` (read-only UIA tree dump) and
  `scripts/a11y-windows.py` (VAL-A11Y-001…005 native driver).
- Decisions/deviations: the pinned GPUI rev (`a846890`) has `role`,
  `aria_label`, `aria_toggled`, `aria_selected`, `aria_expanded` but no
  `aria_disabled` — disabled state is set through
  `a11y_synthetic_children(|b| b.parent_node().set_disabled())`
  (`Control::mark_disabled`). GPUI's Windows backend never receives
  `WM_MOUSELEAVE`, so the native driver parks the pointer over inert
  in-window space before focus-fill probes (a stuck hover fill otherwise
  masks the focus fill; paint order makes hover win over focus — recorded
  as policy). The song catalog order is randomized per run (sorted by
  generated version id), so the new GPUI test derives expectations from the
  catalog instead of assuming titles.
- Upstream paths consulted: `crates/gpui/src/elements/div.rs`
  (focus/in_focus/hover style application order, track_focus a11y
  registration), `crates/gpui/src/accessibility.rs`, vendored
  `accesskit`/`accesskit_windows` 0.24 mappings (Role→ControlType, toggled,
  selected, expanded, disabled).
- Checks (all exit 0): `cargo fmt --all -- --check`; `cargo clippy --locked
  --all-targets -- -D warnings`; `cargo test --locked --all-targets`
  (327 tests: new `controls.rs` unit tests for role/toggle mapping, new
  GPUI test `song_row_arrows_move_the_song_selection_not_the_schedule`);
  `python -m ruff check scripts/a11y-windows.py`. Native:
  `scripts/a11y-windows.py` **PASS** — all expected id+role+name rows in
  English, Indonesian and restored English; File menu popup named items,
  disabled items and Expanded state; mask toggles in UIA with audience
  pixel proof; list selection reporting in both lists; a full 31-stop
  keyboard-only Tab walk with focus-fill probes, Enter/Space activation of
  Go Live/Previous/Next/Black/Clear/Logo verified on the audience, and a
  Shift+Tab back-step (`.amp/in/artifacts/a11y-windows/`, captures viewed).
  `uia-names-windows.ps1` smoke-dumped the live tree (menu, toggles,
  DISABLED flags, selected tabs/rows). Baselines live-output-windows,
  schedule-windows, menu-bar-windows and i18n-windows re-run and PASS.
- Unexecuted: Narrator/screen-reader qualification (M5-03); the library
  search box Edit expectation (blocked on the pending library-search-box
  feature, recorded in the summary); mixed-DPI and collapsed-window runs.
- Next: library-search-box (adds the last VAL-A11Y-001 expectation),
  accessibility-qualification slice (Narrator, M5-03), M1-07 search UI.

### cue-preparation-off-ui-thread — M0-05/M0-06 follow-through — 2026-10-08 (Asia/Jakarta)

- Scope: move `slides::cue()` assembly, size-cap fitting and cue construction
  off the UI thread onto a bounded `sela-prepare-cue` worker; stale/failed
  results never replace the scene; Black/Clear/Logo stay independent of it
  (VAL-PREP-001..005).
- Decisions: new `src/preparation/cue.rs` (`CuePreparer`, lanes Live ordered
  FIFO depth 4 with `Busy` backpressure, Preview newest-wins, generation
  fences). `Item.slides` became `Arc<[Slide]>` so jobs share slide text.
  `Operator::send` now submits a `CueJob`; `poll_cues` presents landed cues,
  sets `live_slide`/labels only then, and reports failures in the Live status
  without touching the audience. Stale = `cancel_cues()` on a new item, Live
  output toggle, surface change, font re-resolution. Safety lane = masks and
  the logo bypass this worker entirely (documented in `docs/delivery.md`).
  New Msg `CueBusy`/`CueFailed` (EN+ID). Background fitting was already on the
  image worker; face resolution already off-thread; both unchanged.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets
  -- -D warnings`, `cargo test --locked --all-targets -- --test-threads=12`,
  `python -m ruff check scripts` (all exit 0, results in the final gate run).
  New tests: 10 in `preparation::cue::tests`, 3 GPUI tests
  (`black_applies_within_the_bound_while_a_heavy_cue_prepares`,
  `failed_or_stale_preparation_never_replaces_the_current_scene`,
  `the_cue_queue_is_bounded_and_refuses_with_a_message`), ignored measurement
  `heavy_cue_ui_thread_time`. Native: `scripts/prep-windows.py` PASS with the
  release binary (Black on audience 156 ms, button lit 172 ms); it FAILS the
  250 ms bound with the debug binary (button lit 516 ms, tile painting cost);
  `scripts/live-output-windows.py` PASS. Measurements in `docs/delivery.md`.
- Unexecuted: the native check clicks nothing in the Schedule list (keyboard
  only); 24 MP image variant (image fitting is on the image worker, prepared
  before the cue and unchanged); physical multi-monitor qualification.
- Evidence: `.amp/in/artifacts/prep-windows-release/`, `.amp/in/artifacts/live-output-prep/`.
- Next: preview cue consumer for the Preview lane when preview thumbnails
  need prepared cues; release-build paint cost of 100-slide preview (virtualize tiles).

### fix-foundation-live-prep-barriers — M0-05/M0-06 scrutiny fix — 2026-10-08 (Asia/Jakarta)

- Scope: foundation scrutiny items #13/#14. (A) A Live double-click under
  Black/Clear/Logo no longer clears the mask before the clicked slide is
  presented; (B) deferred font/image cues are fenced.
- Decisions: `Operator.unmask_on_present` records the double-clicked slide
  while it sits in `pending_cues` or `deferred_send`; `poll_cues` lifts the
  mask (then the existing `unmask_after_cue` waits for the supervisor slot)
  only when that slide is presented. Failure, `Busy`, refusal, no output,
  image substitute, new item, output toggle, another request or a manual
  mask toggle clear it, so the mask stays on and the failed status shows.
  `deferred_send` is now `DeferredSend {index, item, output session}`; it is
  cleared by `cancel_cues` and by any newer `send`, and `retry_deferred`
  (font landing, image landing) sends only if item and session still match.
  A font re-resolution refresh only re-sends when the landed version is the
  live item. Cold-cache benchmark wording corrected: only the first sample
  is cold on the worker (docs/delivery.md, src/tests.rs).
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets
  -- -D warnings`, `cargo test --locked --all-targets -- --test-threads=12`,
  `python -m ruff check scripts` all exit 0. New GPUI tests:
  `live_double_click_under_black_never_reveals_the_previous_slide`,
  `failed_live_double_click_keeps_the_mask_and_reports`,
  `deferred_cue_dies_with_a_newer_request_or_a_new_session`. Native:
  `scripts/live-output-windows.py` PASS; release `scripts/prep-windows.py`
  PASS (Black on audience 141 ms, button lit 172 ms, bound 250 ms).
- Unexecuted: an image-background (uncached) variant of the deferral tests
  uses the font-deferral path, which shares `retry_deferred`; native check
  of a slow-prep double-click (no seed with a slow cue).
- Evidence: `.amp/in/artifacts/live-output-barriers/`,
  `.amp/in/artifacts/prep-windows-barriers/`.
- Next: none for this fix.

### Foundation fix / fix-foundation-settings-lifecycle — M1-16 settings slice — 2026-10-08

- State: implemented-unqualified (settings slice of M1-16).
- (A) Exit drain: `Worker` loop no longer breaks when the reply receiver is
  gone, so every queued `Save` is written after `Worker` drop. `SettingsState`
  counts accepted saves (`pending_saves`, decremented on `Saved`).
  `may_close` (Ctrl+Q, File > Exit, window close) and the unsaved-dialog Quit
  path call `settings_allow_close`: while saves are pending it returns false,
  shows "Saving settings…" in the settings note, and a window task polls every
  25 ms and removes the window once the count reaches 0. After 5 s it closes
  anyway and writes "Settings may not have been saved (write timed out)" to
  stderr. UI stays live (no blocking wait).
- (B) Load race: `loaded` flag (true at once when there is no worker). Until
  `Loaded` arrives Options shows "Loading settings…", monitor/language rows are
  inert and OK writes nothing. A `Loaded` that arrives with Options open
  reseeds the drafts from the persisted values; an `applied` flag stops a late
  `Loaded` from replacing an operator choice. New strings EN+ID:
  SavingSettings, LoadingSettings, SettingsCloseTimeout. `Worker::start_with`
  and `SettingsConfig.loader` inject a slow loader for tests.
- Tests: `worker_drains_every_queued_save_after_the_worker_is_dropped`,
  `quit_after_ok_waits_for_the_write_and_keeps_painting` (2 s writer),
  `quit_after_a_stuck_write_force_closes_after_the_bound`,
  `options_before_loaded_never_rewrites_the_saved_monitor` (DISPLAY6 + id),
  `options_open_during_load_rebase_on_loaded_and_keep_a_missing_monitor`
  (DISPLAY99), `late_loaded_never_overwrites_an_applied_choice`. The worker
  drain test was added with the fix rather than watched failing first.
- Checks: fmt, clippy `-D warnings`, `cargo test --locked --all-targets
  -- --test-threads=12`, `python -m ruff check scripts` all exit 0;
  `python scripts/options-windows.py` PASS (evidence
  `.amp/in/artifacts/options-lifecycle/`).
- Unexecuted: native quit-with-slow-writer (the real writer is fast; covered by
  GPUI tests with injected writer/clock).
- Next: none for this fix.


### fix-foundation-i18n-options-fit (VAL-I18N-005)
- Options sidebar widened 196->224 px, dialog 660->720 px, section title no longer truncated, the 'not available' tag moved under the label (still greyed/visible); tab order unchanged. Seven Live status literals in preview_live.rs moved to Msg keys (EN+ID; Live-prefixed keys for the three whose existing keys had different text); Msg::ALL count 386.
- Verified: scripts/i18n-windows.py PASS and scripts/options-windows.py PASS (captures .amp/in/artifacts/i18n-fit/04,08 show full labels in EN and ID); fmt, clippy -D warnings, cargo test --all-targets, ruff all clean.


### fix-foundation-spacing-tokens-storage-doc (VAL-SHELL-006, storage doc)
- src/ui/theme.rs gains SPACE_XS/S/M (4/8/12 px = GPUI 16 px-rem `*_1/_2/_3`), PANE_PAD_X, ROW_GAP, MENU_ITEM_PAD_X, MENU_ITEM_MARGIN_X, DIALOG_PAD, DIALOG_GAP; used in schedule.rs, preview_live.rs, resources/songs.rs, menus.rs (item row), dialogs/mod.rs and the Options dialog chrome. Values equal the previous utilities, so rendering is unchanged. Test `spacing_tokens_match_the_rendered_chrome`.
- docs/storage.md migration contract corrected: schema 0 -> 5; schema 1-4 -> 5 behind one `<profile>.schema{N}-backup`; versions above 5 Unsupported. No other current-tense 'schema 4 current' claims in docs/ (historical sections kept).
- Indonesian clip item: before/after captures show 'Presentasi' tab and the drop hint fit at 1280x800; the apparent clipping in 08-options-indonesian.png was the modal Options dialog covering them, so no layout change was needed.
- Verified: fmt, clippy -D warnings, cargo test --all-targets, ruff exit 0; i18n, live-output, schedule, song-editor, operator-menu, options native scripts exit 0. Before/after i18n captures (.amp/in/artifacts/spacing-before vs spacing-after-i18n) differ only in the bottom 8 px strip; audience captures byte-identical.
- Next: none for this fix.

### fix-foundation-unmask-intent-r2 (foundation scrutiny round 2)
- Rule: every explicit operator cue request (`send`: click, step, Go Live, Normalize) bumps `request_generation` and drops `unmask_on_present`; a Live double-click stores the current generation as its unmask intent. `PendingCue` and `DeferredSend` carry the generation; internal retries (`retry_deferred`, font re-fit, surface change) go through `dispatch(index, generation)` and keep it. The mask lifts only when the presented cue has exactly that generation and no substitute warning. The already-presented branch of the second click requires `presented == (generation, index, clean)`, so a substituted/failed first cue never unmasks. The second click also treats a deferred request as the latest, so it adds no duplicate cue. Failure/refusal paths drop the intent only for their own generation.
- Tests (src/tests.rs): `double_click_after_the_first_cue_was_substituted_keeps_the_mask`, `newer_deferred_request_cancels_the_double_click_unmask` (B lands keeps Black; C failing keeps Black), `same_request_retry_keeps_the_double_click_unmask`. The first two fail on the pre-fix source. Round-1 tests and the Black-latency test still pass.
- Docs: operator-shell.md unmask barrier rewritten.
- Verified: fmt, clippy -D warnings, cargo test --all-targets, ruff clean; scripts/live-output-windows.py PASS (.amp/in/artifacts/live-output-unmask-r2c; first two attempts failed with "window lost the foreground before 'tab'", an environment focus flake); release-binary scripts/prep-windows.py PASS (.amp/in/artifacts/prep-unmask-r2, Black observed 157 ms, lit 172 ms).
- Next: none for this fix.

### fix-foundation-quit-guard-r2 (foundation scrutiny round 2)
- Decision: the settings-close task is the single close request (`SettingsState.closing`). On completion (saves landed or 5 s bound, which sets `close_forced`) it detaches itself and re-runs `Operator::may_close` instead of `remove_window`, so a dirty schedule shows the Unsaved(Quit) prompt; if that prompt is already open it just ends. Cancel on the Unsaved(Quit) dialog calls `cancel_close_request` (drops task, clears forced flag). UI keeps painting; the wait stays on the executor.
- Tests (src/tests.rs settings_ui): `schedule_edit_during_the_settings_wait_prompts_instead_of_closing`, `second_quit_then_cancel_drops_the_close_request`, `stuck_writer_with_a_dirty_schedule_prompts_after_the_bound`; existing `quit_after_ok_waits_for_the_write_and_keeps_painting` and `quit_after_a_stuck_write_force_closes_after_the_bound` still pass.
- Verified: fmt, clippy -D warnings, cargo test --all-targets, ruff clean; scripts/options-windows.py PASS (.amp/in/artifacts/options-quit-r2); scripts/schedule-windows.py PASS (.amp/in/artifacts/schedule-quit-r2).
- Next: none for this fix.


### fix-foundation-unmask-intent-r3 (foundation scrutiny round 3)
- Scope: FOUNDATION-R3-PREP-01/02 — the two remaining paths where an unintended scene could appear under Black/Clear/Logo.
- Rule: (1) a surface/font refresh rebuilds the NEWEST live request — deferred send first, then the newest cue in preparation, then the slide on screen — with that request's own slide and generation together (`latest_request`/`refresh_live_request`); it no longer re-sends the previous slide under the deferred request's generation, and an unmask intent whose request is gone dies. (2) `unmask_after_cue: bool` became `unmask_on_applied: Option<ContentVersion>`: `poll_cues` no longer lifts the mask at supervisor acceptance (queueing); the mask lifts only when `sync_show` sees the renderer's Applied receipt confirm exactly that slide version (`LiveState::Confirmed`). A terminal rejection keeps the mask over the retained scene and just drops the intent; a re-submitted same version is waited out (`Supervisor::present` clears the stale rejection report so a repeated double-click retries and unmasks on success). `send` (any newer explicit request), mask toggles and output toggles drop the intent at any stage, including after queueing.
- Tests (src/tests.rs; fake audience child gained `reject-scene:<n>` and `slow-scene:<ms>` modes): `surface_refresh_keeps_the_masked_deferred_request` and `font_refresh_keeps_the_masked_deferred_request` (red pre-fix: the refresh re-sent the previous slide under the deferred generation and unmasked it), `newer_request_cancels_the_unmask_before_the_receipt` (red pre-fix: mask cleared at queue acceptance), `render_failed_double_click_keeps_the_mask_and_the_old_scene` (red pre-fix: the rejection let sync_show send Layer::None). All round-1/round-2 barrier tests, the mask truth table and the bounded-queue test unchanged and green.
- Docs: docs/operator-shell.md unmask-barrier paragraph rewritten (Applied-receipt rule, refresh semantics, rejection behavior); mission library/preparation.md round-3 section updated.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `python -m ruff check scripts` exit 0. `cargo test --locked --all-targets -- --test-threads=12`: lib 176/176 and bin 133/134 pass; the only failure is `black_applies_within_the_bound_while_a_heavy_cue_prepares` (Black receipt 125-184 ms vs the 100 ms bound under 12-thread load, on every full-suite attempt this session, also with the four new tests excluded). Environmental, not caused by this change: the same failure reproduces at the base commit 1f4b51f with the change stashed (125 ms), and the test passes standalone in ~20 ms five times running; it measures OS pipe/dispatch latency to the fake child, a path this change does not touch. Native: `scripts/live-output-windows.py` PASS (.amp/in/artifacts/live-output-unmask-r3). `scripts/prep-windows.py --binary target\release\sela.exe` FAILS the 250 ms bound on the laptop panel (Black observed 313-422 ms; round 2 measured 157/172 ms on this machine) — reproduced with the base-commit release binary (344 ms), while the same fixed binary PASSes on DISPLAY6 (observed 140 ms, lit 187 ms; .amp/in/artifacts/prep-diag-display6). The laptop panel's present path degraded when the display topology changed during this session (a new primary DISPLAY5 appeared; both GPUs report OK). Failed prep runs leak their sela.exe children; each was killed by PID before retrying.
- Unexecuted: a passing laptop-panel `prep-windows.py` run (panel-specific environmental regression; pending display reconfiguration or reboot). Evidence: .amp/in/artifacts/prep-unmask-r3, prep-unmask-r3b/c, prep-base-check, prep-diag-display6.
- Next: none for the code; re-run `scripts/prep-windows.py` and the full test gate once the laptop panel recovers.


### fix-foundation-black-latency (owner decision 2026-10-08; VAL-PREP-002/003)
- Scope: make the Black/Clear/Logo latency gates pass without relaxing bounds on the laptop panel and in the full suite. Measured first (timing table and numbers in docs/delivery.md "Black/Clear/Logo latency fix").
- Root causes: (1) unit test: unoptimized dependency code (gpui/cosmic-text at opt-level 0) contending across 12 in-process test threads; the fake child/IPC took ~1 ms. (2) native: Sela's audience path is 3-5 ms; the observed 313-422 ms came from the harness BitBlt of the 2560x1600 panel (109-157 ms per capture, a second capture when the first misses the black frame) and from leaked audience children of earlier failed runs.
- Changes: Cargo.toml `[profile.dev.package."*"] opt-level = 2` (cold rebuild of the dependency set ~12 min, cached afterwards); `src/audience.rs` wake-on-frame via EventLoopProxy, immediate redraw for accepted commands, idle frames not redrawn (1 s heartbeat); `src/timer.rs` timeBeginPeriod(1) in operator and audience; `src/transport.rs` `PipeWorkers::with_wake`, `Signal`, `inbound()`; `src/operator/mod.rs` receipt-wake poll task; `scripts/native_win.py` App.close kills the process tree (prep-windows.py failure paths use it); `scripts/prep-windows.py` records per-capture timings and wall-clock times.
- Tests added in src/transport.rs: `signal_ignores_notifications_until_armed_then_coalesces`, `with_wake_runs_after_each_inbound_frame_is_queued`.
- Upstream GPUI paths studied: see docs/delivery.md.
- Checks: fmt, clippy -D warnings, ruff exit 0; `cargo test --locked --all-targets -- --test-threads=12` exit 0 three consecutive times (Black receipt 13.3/18.1/13.3 ms); prep-windows.py release PASS 3/3 on what was reported as DISPLAY1 (CORRECTION: those runs used `--monitor secondary` and actually selected DISPLAY6; see fix-foundation-display1-latency-r4) and 3/3 on DISPLAY6 (evidence .amp/in/artifacts/final/); live-output-windows.py PASS on the third attempt (first two: "lost the foreground" flake). No sela.exe left after any run.
- Caveat: I could not reproduce the 313-422 ms laptop-panel failure after the first baseline (all 20+ baseline runs on the unfixed audience also passed at 109-157 ms), so the leaked-process and second-capture explanation is inferred from the capture timings, not observed failing.
- Next: none.

### fix-native-harness-foreground (test-harness reliability; no product change)
- Instrumented scripts/native_win.py: describe_window() (class/title/PID/image of GetForegroundWindow) and ensure_foreground(), used by chord/press/click/type_text. A theft is printed as '[native_win] foreground thief before <step>: ...' and recorded in THIEVES; the helper re-activates (SendInput nudge, SetForegroundWindow, AttachThreadInput fallback) for up to 2 s and only then fails, naming the current foreground window.
- Diagnosis: 15 consecutive runs (live-output x10, schedule x5) plus a11y and release prep all PASSED with zero thefts logged, so the flake did not reproduce with the audience window on DISPLAY1; Sela's audience is already non-activating (live-output asserts this). Thief unidentified; likely transient outside window (remote-control services/notifications). Any future occurrence now logs the thief and is retried.
- Verification: live-output-windows.py 10/10 PASS, schedule-windows.py 5/5 PASS, a11y-windows.py PASS, prep-windows.py --binary target\release\sela.exe PASS, ruff clean. Rust unchanged, cargo gate not rerun.


### fix-foundation-unmask-submission-r4 (foundation scrutiny round 4; FOUNDATION-R4-PREP-01)
- Scope: close the class behind rounds 1-3. ContentVersion is reusable, so an old confirmation of slide A satisfied a new double-click of A while B was in flight, and a same-version RenderFailed retry could unmask.
- Rule: every explicit cue request carries a unique monotonic submission id (`request_generation`) through `Supervisor::present_submission` into a `Submissions` ledger (src/masks.rs). The double-click `UnmaskIntent` is armed for exactly one id and settles only on that id's successful Applied receipt; Failed kills it; any newer request, mask toggle, item change or output toggle cancels it; internal retries re-use the id and re-presenting an id forgets its earlier receipt. `unmask_on_present`/`unmask_on_applied` and the version comparison in `sync_show` are gone. Bounded waits unchanged; B may complete under Black. The wire protocol is unchanged: the id is mapped to the existing stamp inside the supervisor.
- Tests: src/masks/model.rs (random 60 000 sequences + exhaustive up to 4 steps over the production Masks/UnmaskIntent/Submissions, version-matched negative control, two explicit sequences); src/tests.rs `double_click_a_while_b_is_in_flight_never_exposes_b_or_the_stale_a` and `same_version_render_failed_retry_never_unmasks` (fake child gained `slow-scene-at:<n>:<ms>`). The model initially found that a synthetic DoubleClick without its first click could arm for a request that was never made; a double-click is modeled as click + double-click, as the UI delivers it. No new dependency: the exhaustive/seeded generator is used instead of proptest.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `python -m ruff check scripts` exit 0; `cargo test --locked --all-targets -- --test-threads=12` exit 0 (lib 182, bin 136, all integration green, includes VAL-PREP-002 100 ms and all round-1..3 barrier tests). Native: `scripts/live-output-windows.py --out .amp\in\artifacts\live-output-r4` PASS.
- Unexecuted: `scripts/prep-windows.py` (release, laptop panel; no product timing path changed).
- Docs: docs/operator-shell.md barrier paragraph; mission library/preparation.md.
- Next: none for this fix.

### fix-foundation-display1-latency-r4 (foundation scrutiny round 4; FOUNDATION-R4-LATENCY-01, R4-FOREGROUND-02)
- Scope: qualify the Black latency bound on the laptop panel DISPLAY1 for real, and harden the harness's monitor and foreground reporting. No product code changed.
- Root cause: prep-windows.py defaulted to `--monitor secondary` = Sela's first-non-primary fallback. With DISPLAY5 primary the first non-primary monitor is DISPLAY6, so the earlier "DISPLAY1" passes were DISPLAY6 runs (1920x1080 captures). Sela's behavior is as documented (settings fallback rules and VAL-SETTINGS-002 unchanged); the script was wrong.
- Changes: `scripts/native_win.py` `window_monitor()` (device, bounds, window rect, DPI, client extent, covers_monitor), `chord` now goes through `ensure_foreground`; strict foreground assertions in live-output, schedule, operator-menu and reference-w03 scripts now include `describe_window()` of the foreground window; `scripts/prep-windows.py` defaults to `name:\\.\DISPLAY1`, writes `requested_monitor`/`audience_monitor` into summary.json and fails on a mismatch.
- Measured (release, DISPLAY1 verified, 2560x1600): observed/lit 203/219, 156/172, 188/204 ms, all under the unchanged 250 ms bound. DISPLAY6 62/78 ms. A `name:\\.\DISPLAY9` request fails with a mismatch message. The observation is one 156-203 ms full-panel BitBlt, so margin on DISPLAY1 is 30-95 ms; no product bottleneck found (audience path 3-5 ms), harness left capturing the whole panel so the real audience pixels are observed.
- Checks: see commit; `scripts/live-output-windows.py` PASS (.amp/in/artifacts/live-output-r4). Docs corrected in docs/delivery.md.
- Next: none.

### fix-foundation-unmask-golive-r5 (foundation scrutiny round 5; FOUNDATION-R5-PREP-03 plus adjudicated follow-ups)
- Bug: with Black on and a double-click unmask on Live slide B in flight, a Songs-tab Go Live of another song C whose load was still pending (`live_on_load`) did not cancel the armed intent, so B's Applied lifted Black over item A's slide although the latest explicit intent was Go Live C (pending or failed). Same class: `go_live()` returned early on an empty Preview or a slideless item without cancelling.
- Fix (src/operator/preview_live.rs): `song_to_live` cancels the intent right after the catalog lookup; `go_live` cancels first, so refused Go Live ends the older intent too; `refresh_live_request` drops the armed submission when no pending or deferred cue carries it after `dispatch`. B is not cancelled, no waits or queue bounds changed, B may finish under Black, masks persist across Go Live (EW8-OBS-017).
- Docs: src/masks.rs, src/output.rs and docs/operator-shell.md restated: re-presenting a request forgets a receipt already recorded for it, but an earlier still-in-flight presentation of the same request may still settle the intent and shows the same slide (R5-PREP-01/02 adjudicated not safety defects; the optional "latest terminal outcome wins" hardening was not adopted). The pending-`live_on_load` ordering is recorded as UNOBSERVED (EW8-OBS-066b, backlog M1-10), behavior unchanged.
- Model (src/masks/model.rs): oracle is now the model renderer scene (lift requires `shown == lifted request` and latest intent `Intent::Slide`), never the ledger; `Intent` enum, split `SecondClick`, `LibraryGoLive`/`LoadLands`/`LoadFails`/`RefusedGoLive`, `Satisfy::NoLibraryCancel` mutant caught by the random runs and by the directed sequence; exhaustive walk over 24 steps to depth 4. Directed tests added; `same_version_render_failed_retry_never_unmasks` unchanged.
- GPUI tests (src/tests.rs): `library_go_live_cancels_a_pending_double_click_unmask`, `failed_library_go_live_keeps_the_mask`, `refused_go_live_cancels_a_pending_double_click_unmask`, `slideless_go_live_cancels_a_pending_double_click_unmask`, `double_click_on_an_already_applied_slide_unmasks_without_a_new_cue`, `refresh_without_a_cue_drops_the_armed_request`. All six fail with the preview_live.rs changes stashed. `Operator::worker` and `request_generation` became `pub(super)` for them.
- Checks: see commit message for the gate results.
- Next: none for this fix.

### fix-foundation-shell-visual-ledger (foundation user-testing attempt 2; VAL-SHELL-007, VAL-REF-007, native baselines)
- Scope: (A) Song Editor chrome: the selected section paint was `FOCUS_BG` (220,227,250) in the Slides thumbnail row and the Words slide-number gutter, versus `SELECTED_ROW` (227,231,243) in the main window and Options (7/channel apart); the Words/Slides header had no fixed height (42 px vs 40 px). `src/song_library.rs` now paints both selected states with `theme::SELECTED_ROW` and the header is `theme::PANE_HEADER_H` tall (`items_end`, bottom border), so border, selected-row, focus-ring (`FOCUS_BG` on focus) and header height all come from `src/ui/theme.rs`. (B) `docs/reference-observations.md`: EW8-OBS-052, EW8-OBS-057 and EW8-OBS-066b status cells changed from UNOBSERVED to `unverified`; the UNOBSERVED/best-guess wording stays in the notes. All other ledger records for VAL-REF-001..006 already use observed/unverified with RUN-W07A (2026-10 run) and M1-03/M1-04/M1-10 owner mappings; the legacy documented-only/superseded/conflicting states are listed in the header vocabulary.
- (C) Native baselines on HEAD 3d28bc3: options-windows.py, i18n-windows.py and a11y-windows.py all PASSed here before the change (the validator's failures came from stale Tab counts and a mouse-seeded start with root-window focus). Removed the counts anyway: both scripts now enable Live output through the Live menu row located by UIA id (`a11y.show_live_output`, `bar-live-show`); i18n selects the first song by UIA id and confirms the Quit-unsaved dialog by id `dialog-confirm`; a11y re-Tabs until UIA focus is on a real control before the keyboard walk.
- New `scripts/chrome-windows.py` (VAL-SHELL-007): captures main, Options and a Song Editor at 1600x1000 client, samples the modal color near each token in a fixed region and the header border rows. Sampled table (client px, `.amp/in/artifacts/chrome-windows/summary.json`): border main (600,84) / options (600,187) / editor (300,119) all [220,222,220]; selected row main (700,708) / options (450,240) / editor (15,171) all [227,231,243]; focus main (1083,43) / options (1152,771) / editor (15,123) all [220,227,250]; header height 40/40/40 px (border rows 84-124, 187-227, 119-159). Max channel deltas 0/0/0.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `python -m ruff check scripts` exit 0; `cargo test --locked --all-targets -- --test-threads=12` exit 0. Native, one at a time: chrome-windows.py, options-windows.py, i18n-windows.py, a11y-windows.py, song-editor-windows.py all PASS. One chrome-windows.py attempt hit the harness "lost the foreground" flake (a browser stole focus) and was rerun.
- Unexecuted: live-output-windows.py (no operator/output code touched).
- Next: none for this fix.

### M1-07b — Songs search box (2026-10-13)
- State: **implemented-unqualified**. Fulfils VAL-SEARCH-001..010 and VAL-A11Y-001 (the Edit expectation moved here from the foundation a11y slice).
- Decisions: (1) `SongSearch` (src/operator/resources/song_search.rs) is a pure controller: trimmed query, `search::Generations`, one pending-submission slot (a newer query replaces the unsent one), newest accepted hits. Fewer than 2 characters (owner decision, deliberately unlike EW8-OBS-045's 1) clears the hits and sends nothing. (2) The single storage worker slot is shared: Search is a new `Request` variant ordered after queued jobs and Song loads and before the Catalog reload; a catalog reload (after a save or delete) re-runs the active query. (3) The box is the shared `TextInput` (`SelaTextInput` context, so the `SelaShow && !SelaTextInput` bindings never fire) wrapped in a `SelaSongSearch` context that binds Esc to `ClearSongSearch`; `TextInput::set_accessible` gives it Role `SearchInput` (UIA Edit), id `song-search` and a localized name. Tab index 499 (after the song rows, before the schedule) keeps the 25 tabs to the first song row. (4) A selected library song missing from new results is deselected; Preview keeps its content. (5) `watch_search`: a 2 ms task picks up the reply after a search is sent. (6) `SEARCH_SQL` ranks and limits inside the FTS scan before joining songs (about 20% faster, same results).
- Upstream GPUI paths studied: none new (existing `TextInput`, `track_focus`, `on_action`/`key_context` patterns; zed rev a846890 unchanged).
- Tests: src/operator/resources/song_search.rs (5 unit tests: no query below 2 characters, shortening drops late replies, newer query replaces the pending one and a stale reply is dropped, whitespace changes and refresh, refresh while unfiltered) and src/tests.rs `song_search_ui` (6 GPUI tests with the real storage worker: filter by title and words with zero `Command::Search` for 1 character; no-results state, clear button and Esc keep focus; b/c/l/B/C/L/Ctrl+B/Ctrl+L/Page Down/Right typed with output live change no mask and send no cue; selection kept or dropped without replacing Preview; results refresh after a save and after a lyric rewrite; a late older-generation reply never overwrites).
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `python -m ruff check scripts` exit 0; `cargo test --locked --all-targets -- --test-threads=12` exit 0. Native, one at a time: `scripts/search-windows.py` PASS (.amp/in/artifacts/search-m1-07b: 1 character unfiltered, `si`/`refrain`/`orig`/`zzqx`, clear button, Esc, selection dropped, typing b/c/l/shift+b/c/l/Page Down/Right with a song live leaves the masks Off and the audience digest unchanged; captures 02-filtered and 03-no-results viewed); `scripts/a11y-windows.py` PASS (.amp/in/artifacts/a11y-m1-07b: `song-search` is an Edit named "Search songs" / "Cari lagu" and the only Edit); `live-output-windows.py`, `schedule-windows.py`, `song-editor-windows.py`, `operator-menu-windows.py` PASS.
- Measured (docs/search.md): 20k corpus from `cargo run --locked --release --example search_bench -- --keep .amp\in\bench20k\library.sqlite`; `scripts/search-windows.py --binary target\release\sela.exe --latency-library <copy> --out .amp\in\artifacts\search-latency-runN`, three runs of 30 two-letter queries: p50 66.5/66.4/65.7 ms, p95 81.5/83.8/82.4 ms, max 84.8/84.7/89.2 ms. Machine: Windows 11 Home, i7-14650HX, 48 GB, NVMe. Database-only after the query change: pooled p95 61.9 ms, two-letter prefix p95 58.3 ms. An earlier full-window-capture harness read 136 ms p95 because each poll cost about 20 ms; it was replaced by a small-region capture.
- Unexecuted: Narrator pass (M5-03), older integrated-GPU laptop, DISPLAY1/DISPLAY5 runs (audience on the secondary monitor), Linux and macOS.
- Known gaps: slide labels are not searched (EW matches them); the unfiltered list still builds every row each frame (20k rows), so clearing a 20k query is slow; no truncation hint above 200 hits.
- Next: slide-label search and Songs list virtualization if wanted; the M1-10 key policy for Ctrl+B/L/C with the box focused stays the best guess in EW8-OBS-047.

### M1-11b — Preflight UI and photo downscale (2026-10-14)
- State: M1-11b **implemented-unqualified**; M1-11 stays **active** (thumbnails, EW media observation). Fulfils VAL-PREFLIGHT-001..006.
- Decisions: (1) Owner decision: a photo over the 64 MiB decode budget is downscaled at import to fit the output extent (aspect kept, never upscaled, same format, JPEG q90), the original is only read, and a note shows both sizes. The shrink path is bounded at 48 MP and 48 MiB of file; Exif orientation is baked into the copy. (2) Preflight runs when a saved schedule opens and shows a dialog that lists missing, changed, too-large and corrupt images with Locate…; Locate… → file prompt → `Request::Relink`, then the schedule is rechecked. (3) Go Live checks only the image of the slide being sent (`Request::Images`, does not supersede the schedule check); on a problem it asks, Cancel is the default and leaves the audience scene, Go Live anyway sends the usual black-with-warning cue. A later slide with a missing image does not interrupt the current one. (4) Corrupt is distinguished from Changed by decoding a hash-mismatched file (`images::verify`). (5) Import shows progress with Cancel (`Worker::cancel_imports`) and the notes in the Media footer.
- Code: `src/images.rs`, `src/images/downscale.rs`, `src/preflight.rs`, `src/operator/assets.rs` (new), Go Live split into `go_live`/`go_live_item` in `preview_live.rs`, `Dialog::Preflight`/`GoLiveMissing`, controls `CancelImport`/`ReviewFiles`, 30 EN+ID strings, `seed_library --vanished-schedule`.
- Upstream GPUI paths studied: none new (existing dialog, focus-handle and `prompt_for_paths` patterns; zed rev a846890 unchanged).
- Tests: lib `images::`/`preflight::` (30 passed incl. downscale and corrupt/changed/missing); `src/tests.rs` `preflight_ui` (4 GPUI tests). The existing image-background Go Live test no longer asserts the "Preparing background…" message because the check now runs first.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `python -m ruff check scripts` exit 0; `cargo test --locked --all-targets -- --test-threads=12` exit 0. Native, one at a time: `scripts/preflight-windows.py` PASS (.amp/in/artifacts/preflight-windows: report on open, Go Live warning, Cancel keeps the audience digest, Go Live anyway shows the slide; operator-report and operator-go-live-warning captures viewed); `live-output-windows.py`, `schedule-windows.py`, `menu-bar-windows.py` PASS. `a11y-windows.py` is flaky on this machine (the walk sometimes never gets UIA focus after the mouse seed): my build PASS, FAIL, FAIL; the unchanged HEAD build PASS, FAIL in the same runs, so it is the harness, not this change.
- Unexecuted: the native Locate… file chooser, native 24 MP import and Cancel import (covered by GPUI/unit tests), Indonesian visual pass of the new dialogs, Linux/macOS.
- Known gaps: report dialog is anchored top-left like the other popover dialogs; hash-matching but undecodable files are only caught at decode time.
- Next: thumbnails pipeline, EW media-import observation.

### schedule-files — `.selaschedule` documents (2026-10-15)
- State: **implemented-unqualified**; M1-09 stays **active**. Fulfils VAL-SHELL-004 and VAL-SCHFILE-001..007 (owner decision 2026-10-13).
- Decisions: (1) New framework-free `src/schedule_file.rs`: versioned JSON v1, bounded read (8 MiB/32 items/1024-byte title), atomic temp+rename writer with injectable write/replace hooks, `Worker` thread (queue 2). serde_json =1.0.151 added (already transitive). (2) Save = storage `Command::Songs` then file worker write; Open = file worker read then `Command::ImportSongs` (hidden tombstoned revision unless an identical revision exists; id = SHA-256 prefix of content). (3) Settings version stays 1; `recent=` lines are additive (tested). (4) Dirty guard is Save / Don't Save / Cancel (`Then::{Open,Recent,New,Quit}`), Save resumes the command after the write. (5) Errors show `Dialog::FileError` (OK) plus the footer note; the open schedule and Live are untouched. (6) DB schedule tables and commands remain but are not written by the operator. (7) EW title bar, default folder and Open Recent labels were not observed: UNOBSERVED EW8-OBS-067..069 in `docs/reference-observations.md`.
- Code: `src/schedule_file.rs`, `src/operator/schedule_files.rs`, `src/schedule.rs` (path/baseline model), `src/storage.rs` (`insert_revision`, `songs`, `import_hidden`), `src/settings.rs`, `src/operator/{menus,dialogs,controls,polling,schedule,options}.rs`, `src/i18n.rs` (EN+ID, 421 messages), `scripts/native_win.py` (`drive_file_dialog`), `examples/seed_library.rs` (`--vanished-schedule` writes `Sunday.selaschedule`).
- Upstream GPUI paths studied (zed rev a846890): `crates/gpui/src/app.rs` `prompt_for_paths`/`prompt_for_new_path`, `app/test_context.rs` `simulate_new_path_selection`/`simulate_path_prompt_response`/`did_prompt_for_paths`, `platform/test/platform.rs` prompt queues.
- Tests: `src/schedule_file/tests.rs` (round trip, limits, truncation, newer version, atomic-write faults, worker non-blocking, backpressure), storage `schedule_file_songs_reuse_identical_revisions_or_import_hidden`, settings MRU tests, `tests::schedule_files_ui` (Save As, Open Recent with missing file, invalid files, fresh library, guard Save), updated `save_and_reopen_pin_revisions_behind_an_unsaved_guard`, `quit_and_new_schedule_are_guarded`, menu bar and preflight tests.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets -- --test-threads=12`, `python -m ruff check scripts` (see final commit). Native, one at a time: `schedule-windows.py`, `menu-bar-windows.py`, `operator-menu-windows.py`, `preflight-windows.py`, `live-output-windows.py` PASS. `schedule-windows.py` failed once with "native file dialog did not close" (lost Enter); the helper now retries Enter once.
- Unexecuted: EW observation (title bar, default folder), Indonesian visual pass of the new dialog, Linux/macOS, `a11y-windows.py`.
- Known gaps: Export Standalone Schedule; theme/asset snapshots (version bump); opening while the settings file has not loaded yet skips recording the recent entry; hidden revisions are never garbage-collected.
- Next: EW observation of the title bar and default folder; item kinds beyond songs.

### schedule-remainder — duplicate, multi-select, drag reorder, Update items (2026-10-15)
- State: **implemented-unqualified**; M1-09 stays **active**. Fulfils VAL-SCHED-001..007, VAL-FOCUS-003/005. Also fixes the schedule-files MRU bug (a save/open finishing before the settings `Loaded` reply dropped the Open Recent entry).
- Decisions: (1) Duplicate Item is a Sela addition in the schedule item context menu: EW8-OBS-060 observed no copy/duplicate in EW. The copy sits directly after the original with a fresh entry id (independent occurrence), all-or-nothing against the 32-item cap. (2) Multi-select and drag are best guesses (EW8-OBS-060 UNOBSERVED): Shift+click range, Ctrl+click toggle, Shift+Up/Down extend, right-click keeps an existing selection, Go Live / Enter / double-click always send only the focused item (exactly one cue). (3) Move Up/Down and drag move the selected group as a block and keep relative order; the drop target is shown by an insertion line, dragging near the list edge autoscrolls (24 px zone, 10 px per 30 ms tick), Esc cancels with no change. (4) Update items in Schedule (Songs context menu, enabled when the schedule is non-empty) scans entries whose pinned revision is not the library head, waits for a pending catalog refresh, shows old vs new slide rows in a dialog, and only Confirm re-pins entries still on the scanned revision; Cancel changes nothing. It never touches Live: the audience keeps the old scene until an explicit Go Live. Removing the live item keeps the audience scene. (5) `Repository::songs` / `Command::Songs` limit raised to 2 x MAX_ITEMS because the review loads old and new revisions. (6) MRU: `update_recent` takes a `RecentChange` and queues it while settings are not loaded; on `Loaded` the loaded list is adopted, queued changes apply in order and the file is saved.
- Code: `src/schedule.rs`, `src/operator/{schedule,update_items,options,schedule_files,controls,menus,layout,polling,mod}.rs`, `src/operator/dialogs/mod.rs`, `src/main.rs` (Extend actions, shift-up/down), `src/i18n.rs` (EN+ID, 431 messages), `src/storage.rs`.
- Upstream GPUI paths studied (zed rev a846890): `crates/gpui/src/window.rs` (mouse-up clears `active_drag`), `elements/div.rs` (`on_drag_move`), `app.rs` (`has_active_drag`, `stop_active_drag`), `interactive.rs` (`ClickEvent::modifiers`).
- Tests: `src/schedule.rs` unit tests (group move, drop-before, block step, duplicate independence/limit); `operator::update_items` (2 unit); `tests::schedule_remainder_ui` (8 GPUI tests: modifier selection, one cue for multi-select, group move order, Esc-cancel drag, autoscroll, duplicate independence, update confirm/cancel, live unaffected); `a_save_before_settings_load_still_reaches_open_recent` (gated loader regression).
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets -- --test-threads=12` exit 0. Native, one at a time: `schedule-windows.py`, `live-output-windows.py`, `menu-bar-windows.py`, `operator-menu-windows.py` PASS (operator-menu failed once with "native file dialog did not close", a known Enter flake, then passed).
- Unexecuted: native mouse drag, Shift/Ctrl click, autoscroll and Update items dialog (`native_win.py` has no drag/modifier-click primitives; covered by GPUI tests only), Indonesian visual pass, Linux/macOS. VAL-FOCUS-004 (Songs list multi-select and Add to Schedule adding all) is not implemented here.
- Known gaps: no undo for Update items; the Songs list stays single-select.
- Next: add mouse/drag primitives to `native_win.py` and a native `schedule-remainder-windows.py`; Songs multi-select (VAL-FOCUS-004); EW observation of multi-select/drag.
### schedule-remainder — native run (2026-10-15)
- State: unchanged (**implemented-unqualified**); the native qualification gap from the previous record is closed for VAL-SCHED-001..007, VAL-FOCUS-003 and VAL-FOCUS-005.
- Added: `scripts/native_win.py` primitives `mouse_move`, `mouse_down`, `mouse_up`, `right_click`, `drag_to` (SendInput with intermediate moves) and `click_with` (Shift/Ctrl held during a click); `scripts/schedule-remainder-windows.py`; `examples/edit_song.rs` (saves a new head revision); `seed_library --extra-songs N`; UIA role/name/id on the Update items rows (`update-item-N`) so the dialog lists are inspectable.
- What the script does (real mouse, one native run at a time): Duplicate through the context menu (copy directly after, selected; removing the copy keeps the original; a saved file keeps both pinned entries); click/Shift+click/Ctrl+click/Shift+Down give 2-5, 2/4/5, 2/4/5/6 and 1-3; Ctrl+click 2 and 4 then a real drag to the end gives 1,3,5,6,2,4 with 5 and 6 selected; Go Live with a two-item selection equals the focused item alone and differs from the other item; a 26-item schedule where a drag into the bottom edge scrolls the list (493 px) and the drop lands at the last row by the pointer; Esc during a drag leaves the order; the library song is edited, Update items lists one affected item, Cancel keeps the saved snapshot, Confirm saves the new lyrics and revision; the live item's audience digest is unchanged until Go Live; Ctrl+Delete on the live item keeps the audience digest.
- Result: `python scripts/schedule-remainder-windows.py --out .amp\in\artifacts\schedule-remainder-windows` PASS (twice); captures in that folder (09-autoscroll-drag, 14-update-diff, 08-audience-multi-live).
- Gate: fmt, clippy -D warnings, `cargo test --locked --all-targets` and `ruff check scripts` exit 0.
- Limits: the schedule model has no per-item arrangement/override yet, so independence is verified through entry identity and separate file items; old/new diff text is painted, not in UIA, so it is checked by screenshot and by the saved file. VAL-FOCUS-004 stays with focus-scopes-resource-browser.
- Next: Songs multi-select (VAL-FOCUS-004); EW observation of multi-select/drag.

### preview-live-navigation — M1-10 remainder (2026-10-15)
- State: M1-10 **implemented-unqualified** for the navigation remainder (VAL-LIVE-001..009); physical-display qualification stays open.
- Decisions: PageDown/PageUp in the Live pane (focus context `SelaLivePane`, focused by Go Live and Live tile clicks) move between schedule items and Go Live at slide 1 (EW8-OBS-050/051); Right/Left/Home/End move slides inside the live item and stop at the ends (EW8-OBS-014); Home/End in a schedule row select first/last entry; Up/Down in a Preview slide move the preview selection. Label jump keys (C/E/I/M/R/T/V + number + Enter, e.g. `v 1 enter`) are UNOBSERVED and recorded as a best guess (EW8-OBS-052): they act on Live and need non-text focus. Bare `B` and `.` toggle Black (presenter remotes), so Bridge uses Shift+B: a deliberate deviation from the EW label letter. A busy preparation lane now holds the newest request (`DeferredSend.busy`) and retries when the lane drains, so rapid Next presses keep order and none is dropped. Live status shows separate Preparing, Requested, Failed and Disconnected lines distinct from "On screen". Tiles show image-background thumbnails (TILE extent 336x188, separate `tile_images` cache so full-size backgrounds are never evicted).
- Intermittent `i18n-windows.py` "Go Live showed nothing on the audience output": ROOT CAUSE = harness race, not a product bug. The audience window exists before the renderer reports Ready; `ready_for_cue()` refuses Go Live while the supervisor is `Starting` ("Live output is still starting"), and the script slept a fixed 1.0 s then pressed PageDown. The failing run's capture was black (12 KB vs 78 KB in the passing repeat) because the cue was refused by design, not dropped or delayed after acceptance. Product regression test: `go_live_before_the_renderer_is_ready_is_refused_and_reported` (src/tests/navigation.rs). Harness fix: wait for the Live status UIA node (`Live-status`, role Group) to show the connected size ("x"), press PageDown, wait for "On screen:" (not "nothing confirmed") bounded at 15 s, then keep the ink requirement with a bounded 5 s poll; `operator_state` is recorded in summary.json and the failure message.
- Upstream GPUI studied: `crates/gpui/src/window.rs` (`dispatch_key_event`), `keymap.rs` (multi-keystroke bindings, context depth precedence), `elements/img.rs` (RenderImage BGRA).
- New: `src/labels.rs`, `src/operator/navigation.rs`, `src/tests/navigation.rs` (11 tests), `scripts/navigation-windows.py`; `a11y-windows.py` gained `live_status`/`wait_live_status`.
- Results: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets -- --test-threads=12`, `python -m ruff check scripts` exit 0. Native: `python scripts/i18n-windows.py` PASS 5 consecutive runs (23 s each, operator_state "Live output 2560x1600; On screen: Signal Hymn · Verse 1") on 2026-10-15 with the secondary monitor attached; `python scripts/navigation-windows.py` PASS (go live, Right/Left/Home/End, stop at both ends, rapid Right burst, V,1,Enter, B blank and restore, killed audience shows Disconnected); `a11y-windows.py`, `schedule-windows.py`, `schedule-remainder-windows.py` PASS. Evidence: `.amp/in/artifacts/{i18n-windows,navigation-windows}`.
- Unexecuted / open: (1) `live-output-windows.py` FAILS at `next_1` because Go Live now focuses the Live pane, so its "Tab x3 to Next" path changed; the secondary display was disconnected afterwards (only DISPLAY1 present) so the corrected Tab count could not be re-verified. Next action: update that script's tab path (Live pane list focus then Previous/Next) and rerun with two monitors. (2) Preparing and Failed indicators are verified by GPUI tests only (no native fault-injection hook). (3) Label keys are unobserved in EasyWorship.
- Next: re-run `live-output-windows.py` after fixing its tab path; observe label jump keys in EW.

### preview-live-navigation — follow-up: stale Tab path and monitor roles (2026-10-18)
- Decision: leaving focus on the Live pane after Go Live is intended (EW8-OBS-014: with the main window focused, Right/Left/Home/End move Live; W07 records EW8-OBS-050/051 for PageUp/PageDown). The product is unchanged.
- `scripts/live-output-windows.py`: Previous/Next are pressed by UIA AutomationId (`live-previous`, `live-next`) through `Run.keys("click:<id>")` instead of counted Tabs, in the main, formatted and backdrop flows.
- Monitors by role: `native_win.spare_monitor(screens)` returns the non-primary screen that is not DISPLAY1. `options-windows.py --target` now defaults to it (was a hard-coded DISPLAY6, which became the primary after the 2026-10-18 renumbering). Audit of the other scripts found no hard-coded DISPLAY5/DISPLAY6 (`prep-windows.py` names DISPLAY1 on purpose).
- Results (3 monitors: DISPLAY6 primary operator, DISPLAY1 audience, DISPLAY5 spare): `live-output-windows.py` PASS, `navigation-windows.py` PASS, `i18n-windows.py` PASS, `schedule-windows.py` PASS, `a11y-windows.py` PASS, `options-windows.py` PASS (target DISPLAY5). Artifacts under `.amp/in/artifacts/*-final2` and `live-output-nav2`. Gate: fmt, clippy, `cargo test --locked --all-targets -- --test-threads=12`, `ruff check scripts` exit 0.
- Next: qualification on physical displays and EW observation of label jump keys.

### arrangement-editor-pagination — M1-06d (2026-10-19)
- State: M1-06 **implemented-unqualified** for editor arrangement controls, per-item arrangement and pagination (VAL-ARR-001..005, VAL-SCHED-002); EW observation of splitting and physical-display qualification stay open.
- Decisions: pagination uses the 1080 reference canvas (16 lines at the 48px minimum) so the editor and every output size break identically; the schedule item's arrangement is chosen with a Preview-pane button that cycles the song's arrangements (no context-menu submenu yet); the first arrangement is the active one (Make active moves it to index 0); `.selaschedule` is version 2 with an optional per-item `arrangement`; v1 files and unknown arrangement IDs fall back to the song's first arrangement. No autosave/snapshot store exists in the code yet (the menu item is disabled), so nothing persists per-item arrangements there; `Pin` (version + arrangement) is the unit it must persist.
- New: `src/arrangement_edit.rs`, `slides::{paginate, project, Page, Projection}`, `schedule::Pin`/`Schedule::set_arrangement`, `Control::Arrangement`, 21 EN/ID messages, `seed_library --arranged-song`, `scripts/arrangement-windows.py`. `scripts/schedule-windows.py` now expects file version 2.
- Results: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets -- --test-threads=12`, `python -m ruff check scripts` exit 0. Native (3 monitors, audience on the secondary): `arrangement-windows.py` PASS; `live-output-windows.py`, `schedule-windows.py`, `schedule-remainder-windows.py`, `operator-menu-windows.py`, `a11y-windows.py` PASS. Captures under `.amp/in/artifacts/arrangement-windows` (03 paginated Preview, 06b audience page 1 of 14 lines, 08 reopened).
- Unexecuted / open: the editor Arrangement tab is verified by GPUI tests only (the song-editor native driver is X11-only); tall-slide Preview tile thumbnails show only the top lines of a page; EW's own splitting rule, manual breaks and a reference-repair UI are open.
- Next: autosave/snapshot persistence of `Pin`s when that ticket lands; EW observation of overflow behavior.

### focus-scopes-resource-browser — M1-03 / M1-04 remainder (2026-10-18)
- State: **implemented-unqualified** (VAL-FOCUS-001/002/004, VAL-RES-001..004). Physical-display, Narrator and older-hardware runs stay open.
- Decisions: (1) No schema change. "Date modified" is the order of `search_rows.row`; `index_song` moves a song's row to MAX+1 on every save, so it is monotonic, but a rebuilt index restarts in id order (documented). (2) The list loads through `Command::Library(SongOrder)` (all songs, no payload decode) instead of the old 128-song `Catalog(None)`. (3) Sort order is stored in `settings.txt` (`song_sort`, `song_sort_descending`; defaults not written). (4) Sort by choices open inline under the menu item (a flyout cannot be anchored at a window edge); Right/Left open/close. (5) Only the cursor row is a tab stop (was every row), so Tab does not walk 20k rows; `scripts/schedule-windows.py` uses Down between songs. (6) Delete wording and Yes/No with default No follow EW8-OBS-065; Esc is No (EW ignored Esc). Selection moves to the neighbour row after a delete. (7) All dialogs and Options go through `Operator::modal`: full-window backdrop that occludes the pointer, centred panel, `SelaDialog` key context (Esc to `DismissDialog`, Tab/Shift+Tab to `trap_focus`), and the root drops `SelaShow` while a dialog is open so Ctrl+B/Page Down do not reach Live. (8) Search shows "More songs match than are shown..." under the list when the 200 cap is hit, and a new query resets the scroll. (9) EW also matches section labels (EW8-OBS-045); Sela does not index them (documented deviation, backlog item `m1-07-search-label-matching`); the two-character minimum is recorded as an owner-approved deviation in docs/reference-observations.md. (10) Song editor close prompt is an inline banner, not a modal: gap for VAL-FOCUS-001's song-editor clause.
- Upstream GPUI studied (zed rev a846890): `crates/gpui/src/elements/uniform_list.rs` (scroll handle, `ScrollStrategy::Nearest`, `processor` closure), `elements/div.rs` (`ScrollHandle` offset/bounds/set_offset, `on_drag`/`on_drag_move`), `window.rs` (`focus_next/prev`, tab stops by cloned `FocusHandle`), `app/context.rs` (`processor`).
- New: `src/song_order.rs`, `src/operator/resources/song_select.rs`, `src/operator/frame_log.rs` (opt-in `SELA_FRAME_LOG`), `src/tests/resource_browser.rs` (8 GPUI tests), `scripts/resource-browser-windows.py`. `a11y-windows.py` now seeds focus by clicking the Songs tab and confirming UIA focus, verifies seeding clicks by UIA `selected`, expects the selected song row as the Songs tab stop, and clicks a button before Alt+E (the walk ends in the search box).
- Results: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets -- --test-threads=12` (240 lib, 202 bin, others) and `python -m ruff check scripts` exit 0. Native, debug build, 3 monitors: `a11y-windows.py` PASS 5 consecutive runs (plus 2 earlier passes; a seeding click dropped once and one run failed before seeding was verified, both fixed in the script); `resource-browser-windows.py` PASS; `search`, `options`, `i18n`, `operator-menu`, `preflight`, `menu-bar`, `schedule`, `schedule-remainder`, `live-output`, `navigation`, `arrangement`, `chrome` PASS (`song-editor-windows.py` exit 0). 20k run (release binary, `search_bench --songs 20000 --keep`): `scripts/resource-browser-windows.py --binary target\release\sela.exe --library .amp/in/lib20k/library.sqlite` PASS: 6 of 20000 rows exposed, Ctrl+End selects and focuses row 19999, Ctrl+Home returns, 60 Page Downs end at row 240, thumb drag to the bottom shows the end and back to the middle shows middle rows, "ma" search shows the hint. Frame log: Page Down window 27 frames, render interval p95 3.0 ms, max 3.4 ms; scrollbar drag 31 frames, p50 10.7 ms, p95 11.6 ms, max 55.7 ms; row build p95 0.03 ms. Limits: the intervals follow input cadence (the log is render to render, not a GPU present time); only 27 frames for 60 key presses because input coalesces; one machine. Evidence: `.amp/in/artifacts/resource-browser-windows`, `resource-browser-20k`.
- Unexecuted / open: song-editor and Update-items modals are covered by the shared wrapper and GPUI tests, not by their own native Esc/focus walk (preflight and Options are native); Author/Copyright/Administrator/Song Number sort; Media and other list virtualization; older-hardware frame timing.
- Next: slide-label search, true modal for the song editor close prompt.

### song-metadata-fields session record

Done: audience footer raster, Song Editor Administrator/Song Number with validation, modal close prompt (GPUI test), seed_library --metadata-songs, scripts/song-metadata-windows.py (PASS; strip ink 1977 on first/last slide, 0 on plain; audience change counts 0 for editor prompt, Options, preflight). Evidence: .amp/in/artifacts/song-metadata-windows. Receipts are a frame-change-count proxy. Next: M1-05 qualification on physical output.
### layout-persistence — M1-02 remainder (2026-10-19)
- State: M1-02 stays **implemented-unqualified**; VAL-SHELL-008/009/010 implemented.
- Decisions: layout lives in `settings.txt` as `pane_ratios=a,b,c` (ten-thousandths, so a restored splitter is within 0.1 px), `resources_collapsed=0|1` and `window=x,y,w,h[,max]` (logical px). Any malformed or out-of-range layout key drops the whole layout to defaults without marking the file corrupt (other settings survive; no backup rename). The layout is read synchronously once in `main` before the window opens (`settings::peek_layout`, tiny file, no rename side effects) so the first frame already uses it; the worker's later Loaded reply only seeds the baseline. The layout is written by `settings_allow_close` (Ctrl+Q, Exit, window close) through the settings worker and the close waits for it like any pending save; it is skipped until the initial load replied. `pane_layout::fit_placement` keeps a window that has at least 200x100 px on a monitor work area with its title bar reachable, else centres it on the primary work area; size is clamped to the minimum (720x440) and the monitor. Minimum pane sizes are still enforced by `Operator::dimensions` (150/140 px) at every viewport.
- Tile fix (orchestrator note after 25f8fb6): Preview/Live tiles now size text with `tile_text_size` (all lines, height by line count at 1.3 line height and width by longest line, capped at the old 9 px), no line cap and no wrapping, so a 14-line page shows entirely in the tile like the audience.
- Upstream GPUI studied (zed rev a846890): `crates/gpui/src/window.rs` (`window_bounds`), `platform.rs` (`PlatformDisplay::visible_bounds`), `app.rs` (`displays`).
- New: `src/pane_layout.rs` (9 unit tests), 2 settings tests, 3 tile tests in `src/operator/preview_live.rs`, 2 GPUI tests (save on close and restore; corrupt layout) in `src/tests.rs`, `scripts/layout-windows.py`. Two existing close tests were adjusted because a close now also writes the layout.
- Results: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets -- --test-threads=12`, `python -m ruff check scripts` exit 0. Native (3 monitors): `python scripts/layout-windows.py --out .amp\in\artifacts\layout-persistence` PASS: both splitter columns/rows identical after restart (448/870 and the same rows), window rect 120,90,1540,990 vs 120,90,1541,990 after restart, window `40000,30000` moved inside the primary monitor at 471,167,2089,1214, malformed layout started with defaults and the next quit wrote `pane_ratios=2400,6200,6200`. `live-output-windows.py` PASS.
- Unexecuted / open: mixed-DPI window restore (windows saved on a monitor with a different scale than the primary); restoring a maximized window natively (unit/GPUI only); a native pixel check of tall tile text (GPUI bounds test only).
- Next: Handle small laptop viewports and mixed DPI; other layout modes.

### fix-sunday-preflight-golive — scrutiny round 1 (SC-PREFLIGHT-01..04, SC-LIVE-01) (2026-10-19)
- State: fixes only; M1-11b, M1-10 and M1-09 keep their **implemented-unqualified** status.
- Decisions: (1) A successful Locate also drops failed (`None`) `tile_images` entries and prefetches again, so Preview/Live thumbnails fill without re-selecting. (2) A Go Live whose image check finds the preflight queue full (`Busy`) is held unsubmitted (`HeldGoLive.generation = None`); `poll_preflight` retries the submit, so only a gone worker waives the check. (3) `go_live`, `cancel_cues` and `toggle_output` drop the hold (`Assets::drop_held`), so a late check never replaces a newer command. (4) `Operator::import_extent` fits an import to the connected output, else the configured audience monitor (saved name, else the first non-primary), else nominal. (5) Both Go Live paths (action and toolbar button) call `go_live_requested`, and `go_live_anyway`, a clean delayed check and the worker-gone path set `focus_live_pending`; `render` hands focus to the Live pane once something is live, never while a dialog is open or a text input has focus, and clears the flag when the hold is refused or dropped. (6) `preflight::Worker::start_gated` (doc-hidden) lets tests stall the worker to fill its queue without sleeping.
- Tests (each fails without its fix, checked by reverting the fix): 8 in `src/tests.rs` `preflight_ui` (locate thumbnail, full-queue hold then prompt, newer Go Live drops hold, toggle/cancel drop hold, go-live-anyway focus, clean delayed check focus, import extent) and `tests::navigation::the_go_live_button_hands_the_page_keys_to_live`.
- Results: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets -- --test-threads=12` (260 lib, 217 bin) and `python -m ruff check scripts` exit 0. One run of `settings_ui::schedule_edit_during_the_settings_wait_prompts_instead_of_closing` failed under load and passed alone, on a stash of this change, and on the full rerun (not touched here). Native, debug build: `preflight-windows.py --monitor name:\\.\DISPLAY1`, `navigation-windows.py`, `live-output-windows.py` PASS. Evidence: `.amp/in/artifacts/fix-preflight-golive-{preflight,navigation,live-output}`.
- Unexecuted / open: native check of the full-queue and import-monitor cases (GPUI/unit only: no native hook to stall the worker or to set an unplugged monitor); the other round 1 fix features.
- Next: remaining sunday-core fix features.

### fix-sunday-schedule-documents — scrutiny round 1 (SC-SCHFILE-01..03, SC-SCHED-01, SC-HARNESS-01) (2026-10-22)
- State: fixes only; M1-09 keeps its **implemented-unqualified** status. Native re-runs are not done yet (see below).
- Decisions: (1) `Then::Recent(PathBuf)`: the menu resolves the path before `guard`, because saving first reorders the recent list. (2) `resume_guarded` calls `guard` instead of `proceed`, so an edit made during a guarded save prompts again. (3) `guard` refuses New/Close/Open/Recent while `schedule_busy()` (`report_busy`). (4) `Schedule::document()` is a generation bumped by `open` and `clear`; `SaveJob.document` records it and `file_written` skips `mark_saved` and the resume when it differs (the file is still written and remembered). (5) Update items keeps line breaks (spaces normalized per line) and the Old/New cells wrap instead of truncating. (6) Don't Save on a quit that waits for a settings write records the schedule (generation plus pins); when the wait ends the window closes only if the schedule is unchanged, else the Unsaved prompt opens again. (7) `scripts/schedule-windows.py` and `scripts/arrangement-windows.py` expect file version 3 (the writer's `VERSION`).
- Test-harness fix: `settings_ui::start` now waits for the settings `Loaded` reply as well as the monitor scan. `apply_options` ignores OK before `Loaded`, so under a busy full run OK was sometimes dropped and 1-4 `settings_ui` tests failed per run (also on HEAD c7d95de, checked in a detached worktree). `drain_writer` polls until the operator has resolved the close request instead of one fixed clock tick.
- Tests: `schedule_files_ui::{recent_after_a_guarded_save_opens_the_chosen_file, an_edit_during_a_guarded_save_prompts_again, new_close_and_open_are_refused_while_a_save_runs, a_save_that_finishes_late_leaves_the_new_document_alone}`, `settings_ui::{dont_save_during_the_settings_wait_closes_without_asking_again, an_edit_after_dont_save_during_the_settings_wait_prompts_again}` (fails with the old unconditional flag, checked by reverting), `schedule::tests::the_document_generation_changes_on_open_and_clear_only`, `update_items::tests::a_change_on_a_late_line_shows_every_line_of_both_slides`.
- Results: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets`, `cargo build --locked`, `cargo build --locked --example seed_library`, `python -m ruff check scripts` exit 0 (see the commit). `cargo test --locked --bin sela` passed 3 consecutive full runs (224 passed, 3 ignored) after the harness fix.
- Unexecuted / open: native `scripts/schedule-windows.py`, `scripts/arrangement-windows.py` and `scripts/schedule-remainder-windows.py` (they take over the keyboard and mouse and need the owner idle with the secondary monitor attached); the Update items full-text check is GPUI/unit only until then.
- Next: run those three native scripts; then the remaining scrutiny round 1 features: fix-sunday-arrangement-rendering, fix-sunday-focus-modality, fix-sunday-layout-startup.

### parallel-wave-2 scaffold (2026-10-22)
- State: scaffold only, nothing user-visible. It lets eight worktrees run at once without editing the same lines: the three remaining sunday-core fix features, and EasyWorship-independent parser/locator cores pulled forward from later features (OpenLyrics, OpenSong/ChordPro and EasyWorship RTF parsers, the scripture reference parser, the FFmpeg locator). Their UI, framework and EW-observed parts stay with their features.
- Decisions: (1) Importers share one intermediate shape, `import::ParsedSong` (sections of slides, an order of section indices, metadata, warnings) or `Rejected`, so the M1-14 framework maps every format onto `storage::Song` in one place (docs/import.md). (2) `roxmltree =0.21.1` (already in the lock through a dependency) and dev-only `proptest =1.11.0` (VAL-IMPORT-017). (3) Empty `src/tests/{arrangement_rendering,focus_modality,layout_startup}.rs` modules give each fix stream its own GPUI test file.
- Results: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib import` (2 tests) exit 0.
- Next: the eight streams, each with its own work-log record.

### fix-sunday-layout-startup — scrutiny round 1 (SC-LAYOUT-01, SC-LAYOUT-03) (2026-10-22)
- State: fixes only; M1-02 keeps its **implemented-unqualified** status and no backlog checkbox changes. SC-LAYOUT-02 stays withdrawn and untouched. The native spare-monitor re-run is not done (see below).
- Decisions: (1) `parse_window` checks each origin with `(-MAX_ORIGIN..=MAX_ORIGIN).contains` instead of `abs()`. `i32::MIN.abs()` overflowed, so with dev/test overflow checks `peek_layout` panicked before the window opened (release builds wrapped and accepted the value). (2) `pane_layout::fit_placement` returns `(Placement, Option<usize>)`: the fitted rectangle and the index of its home area (the reachable area with the largest overlap, else the primary), `None` only when no monitor is known. (3) `main.rs` builds the operator window through `operator_window_options(saved, work_areas, primary, default_bounds)`, which passes the home monitor's `DisplayId` as `display_id`. Without it GPUI opened on the primary monitor and replaced bounds whose centre is not on that monitor with its default bounds. With no saved placement the window keeps `display_id: None` and the centred 1280x800 default. (4) Work areas enter as `(DisplayId, Bounds<Pixels>)` pairs rather than `PlatformDisplay` objects: GPUI's test platform has one display and ignores `display_id`, and a test `PlatformDisplay` needs the `uuid` and `anyhow` crates, which are not dependencies.
- Upstream GPUI studied (zed rev a846890, cargo checkout `zed-a70e2ad075855582/a846890`): `crates/gpui/src/window.rs:1557-1570` (`display_id` forwarded unchanged to `WindowParams`), `crates/gpui_windows/src/window.rs:517-523` (display from `display_id`, else the primary monitor) and `:1570-1595` (`retrieve_window_placement`: the bounds are kept only when `check_given_bounds` holds, else the display's default bounds, then converted with that display's scale factor), `crates/gpui_windows/src/display.rs:37-75,102-117` (logical work area at each monitor's own scale; the bounds centre must lie on the display), `crates/gpui/src/platform.rs:566-594,684-691` (`PlatformDisplay`, `DisplayId::new`), `crates/gpui/src/app.rs:1487-1509` (`displays`, `primary_display`), `crates/gpui/src/geometry.rs:740-754` (`Bounds::centered`), `crates/gpui/src/platform/test/{platform.rs:460-466,494-509,display.rs}` (one 1920x1080 test display, `display_id` ignored).
- New: `pane_layout::tests::{extreme_window_values_drop_the_layout_without_panicking, a_placement_wholly_on_the_spare_monitor_belongs_to_that_monitor}`, `settings::tests::extreme_window_coordinates_fall_back_to_the_default_layout` (the startup `peek_layout` read and `load`, with `i32::MIN`/`i32::MAX` origins and `u32::MAX` sizes), `tests::layout_startup::{a_window_saved_on_the_spare_monitor_opens_there_at_its_saved_size, a_window_saved_on_a_disconnected_monitor_opens_centred_on_the_primary}`. The existing `fit_placement` tests now also assert the area index. The layout paragraph in `docs/operator-shell.md` describes the persisted layout and the monitor rule.
- Fails without the fix (checked by temporarily reverting): with `abs()` restored, both extreme-value tests panic with "attempt to negate with overflow"; with `fit_placement` returning the fallback index, the spare-monitor index test gets `Some(0)` instead of `Some(1)`; with `display_id` dropped from the options, `a_window_saved_on_the_spare_monitor_opens_there_at_its_saved_size` gets `DisplayId(1)` instead of `DisplayId(2)`. The disconnected-monitor test passes either way (GPUI already falls back to the primary); it guards the centring on a primary that is not listed first.
- Results (worktree `fix/sunday-layout-startup`, `CARGO_TARGET_DIR=C:\sela-target\w2-layout-startup`, 4 build jobs): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -- -D warnings` exit 0; `cargo test --locked --all-targets -- --test-threads=8` exit 0 (lib 266 passed + 1 ignored, bin 226 passed + 3 ignored, `output_process` 9 + 1 ignored, `transport_process` 4 + 1 ignored, examples pass). `ruff` was not run: no script changed.
- Unexecuted / open: native `scripts/layout-windows.py` has no run that saves the window wholly on a non-primary monitor. That script is outside this stream, so the run still has to be added and executed with the owner idle and the spare monitor attached (VAL-SHELL-008). Mixed-DPI restore stays open. A reachable window whose centre is on no monitor (for example dragged partly past the left edge) still gets GPUI's default bounds on its display through `check_given_bounds`; no contract assertion covers it and EW behaviour is unobserved.
- Next: add the spare-monitor run to `scripts/layout-windows.py`, run it after the merge, then scrutiny round 2.

### fix-sunday-arrangement-rendering — scrutiny round 1 (SC-ARR-01, SC-ARR-02, SC-SONG-02) (2026-10-22)
- State: fixes only; M1-06 and M1-05 keep their **implemented-unqualified** status. No backlog checkbox changes. Native re-runs are not done yet (see below).
- Decisions: (1) Face resolutions are keyed by `preview_live::Projected` (song revision plus the item's stored arrangement choice) in `unresolved`, `resolve_fonts` and `fonts_resolved`. An item takes a landing only when the key matches and it has one face per slide. `font_job` stays a busy flag that holds the version. `fonts_resolved` takes `impl Into<Projected>`, and `From<Version>` (the default arrangement) keeps the direct calls in `src/tests.rs` compiling unchanged. (2) `slides::section_pages(&Section, master) -> Vec<Slide>` (`section_slide`, then `paginate`) serves `project`, the editor's `preview_slide` and the split badge (`Library::audience_pages`). `slides::page_count` is deleted. It returns slides rather than strings, so `project` and the editor keep the section's format and background without rebuilding them. (3) The lyrics box is the text area minus the strip's box (`footer_height`). `text_extent` now gives up the strip height once (it gave it up at both edges for lyrics centred on the whole area). Auto and Fixed sizes fit against it, and `audience::text::layers` rasterizes the lyrics into the same box. Visible effect, computed and not captured: Middle-aligned lyrics of a song with copyright centre in the area above the strip, about half a strip higher than before (31 px at 720 lines, 46 px at 1080 for the test strip), and height-limited Auto text can grow by one strip height. (4) A Fixed size that fits only without the strip drops the strip on that slide instead of failing the cue, like a strip that cannot be drawn at all; a Fixed size that does not fit the canvas is still refused. Refusing the cue (the literal reading of the triage) was rejected because the editor preview does not model the strip, so a slide the editor shows would fail on Sunday. Sela rule, unobserved in EW.
- New: `slides::section_pages`, `preview_live::Projected`, crate-visible `Library::{preview_slide, audience_pages}`. Tests: `tests::arrangement_rendering::{previewing_another_arrangement_keeps_live_faces_and_every_live_slide, the_editor_pages_a_section_exactly_like_the_audience}`, `audience::text::tests::lyrics_and_the_copyright_strip_never_share_a_row` (Bottom, Middle, tall Fixed), `slides::tests::{a_fixed_size_keeps_the_strip_only_when_it_fits_above_it, section_pages_paginate_the_normalized_text}`.
- Fails without the fix (each fix reverted temporarily, then restored): matching items by version only fails the Live test ("Live keeps its faces": Live got Short's 2 faces). Dropping only the length check fails it on a 1-face landing. Raw-text editor pagination fails the editor test ("Sixteen": 2 pages of 8 lines against 1 audience slide of 16). Without the raster reserve, Bottom, Middle and tall Fixed overlap (lyrics end on rows 677, 646 and 640; the strip starts on row 633). The pre-fix model (both-edge fit, Fixed fitted against the whole canvas, no reserve) overlaps for Bottom and tall Fixed (Middle was clear there) and fails `a_fixed_size_keeps_the_strip_only_when_it_fits_above_it`.
- Results (worktree `fix/sunday-arrangement-rendering`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -- -D warnings` exit 0; `cargo test --locked --all-targets -- --test-threads=8` exit 0 (lib 265 passed, 1 ignored; bin 227 passed, 3 ignored; `output_process` 9, `transport_process` 4, examples pass).
- Unexecuted / open: native `scripts/arrangement-windows.py`, `scripts/song-editor-windows.py` and `scripts/song-metadata-windows.py` (workers run no native scripts; the parent runs them after merging). The editor preview still draws no strip and fits without it, so for a song with copyright its size and the Format pane's fitted start size differ from the audience. EW's strip layout with Fixed sizes is unobserved. Inferred from reading, not reproduced: a font job in flight when `fonts_scanned` clears named items lands its pre-catalog resolution, which then sticks; `slides::slide_version` is keyed by revision and index only, so two arrangements of one revision share content versions.
- Next: merge, run the three native scripts, and check the audience captures of a copyrighted Middle-aligned song for the new vertical position.

### fix-sunday-focus-modality — scrutiny round 1 (SC-RES-01, SC-SONG-01) (2026-10-22)
- State: fixes only; M1-03, M1-04 and M1-05 keep their **implemented-unqualified** status. Native re-runs not done (see below). SC-SONG-03 stays deferred to user testing and was not touched.
- Decisions: (1) Measured correction to the triage: with focus left on a Songs row that is no longer rendered, GPUI dispatches from dispatch node 0, the root view element, which has no key context (the operator's `Sela SelaShow` div is a child node). So every operator key was dead: Page Down, Ctrl+End and Up did nothing and nothing went live (the new tests on the 6859b3f sources). The Go Live hazard is real for a careless fix: a focused list element without `SelaSongRow` makes Page Down resolve to Go Live through the operator root (checked by removing that context). (2) `Control::SongList` is a persistent focus handle in the control registry: not a tab stop, tab index 100, Role::List, counted in `in_resources` so Collapse moves focus off it. It sits on the `song-list` element, which now wraps the rows and the empty states (rendered whenever a library is open) and carries `SelaControl SelaSongRow` and the list actions. A click below the rows focuses the list (GPUI focus on mouse down). (3) The hand-off uses GPUI's focus-lost hook (`Context::on_focus_lost` with `Window::focus_lost_restore_target`, as Zed's `crates/workspace/src/workspace.rs:1892`), restricted to the Songs list: when the focused element leaves the tree and its nearest rendered focusable ancestor is the list, the list takes focus. No per-frame scan; detecting in the `uniform_list` processor would misfire because it also runs twice per frame with a 0..1 measuring range. (4) After a delete, `poll_library` selects the neighbour and calls `reveal_song`, so the neighbour row takes the keyboard focus the confirmation returned to the deleted row. (5) `song_library::EditorPromptOpen(usize)` counts editor windows with the close prompt open; `Library::set_confirm_close` is the only writer of `confirm_close` (prompt opens, Cancel/Esc, Yes, No, load or new song, save). No release hook: a dirty editor closes only through the prompt's No, which clears the count first. (6) While the count is nonzero the operator draws an occluding full-window backdrop and an App keystroke interceptor scoped to the operator window stops every keystroke before binding dispatch. Deviation from the triage (key context `Sela` plus an `activate` early return): dropping `SelaShow` leaves the Live pane's Page Down/Up (`SelaLivePane`), Enter on a focused button, Alt+L menu items and Ctrl+Q reachable; the interceptor covers every context, and with no input reaching the window an `activate` guard would be dead code. The subscriptions are registered once on first render and detached (`Operator` gets no new field; `operator/mod.rs` is outside this stream).
- Upstream GPUI studied (zed rev a846890): `crates/gpui/src/window.rs` (`focus_node_id_in_rendered_frame`, `dispatch_key_event`, keystroke interceptors, focus-lost dispatch at the end of `draw`, `focus_lost_restore_target`, `HitboxBehavior::BlockMouse` hit test), `crates/gpui/src/element.rs` (one dispatch node per element), `crates/gpui/src/key_dispatch.rs` (`root_node_id`), `crates/gpui/src/app.rs` (`intercept_keystrokes`, `default_global`, test-mode draw in `flush_effects`), `crates/gpui/src/app/context.rs` (`on_focus_lost`, `observe_global`), `crates/gpui/src/tab_stop.rs`, `crates/gpui/src/elements/uniform_list.rs` (measure calls), `crates/gpui/src/elements/div.rs` (focus on mouse down), `crates/workspace/src/workspace.rs` (`on_focus_lost` use).
- New: `src/tests/focus_modality.rs`, 6 GPUI tests: `page_down_after_a_scrollbar_scroll_pages_the_songs_and_never_goes_live`, `ctrl_end_after_a_wheel_scroll_reaches_the_last_song`, `deleting_the_last_song_hands_selection_and_focus_to_its_neighbour`, `deleting_the_only_song_leaves_keys_working_and_page_down_out_of_live`, `the_editor_close_prompt_blocks_operator_clicks_and_keys_until_cancelled` (b, Ctrl+B, Right, Page Down, Alt+L Enter with the Live pane focused; clicks on Black, Next, Go Live, Live output; then Esc in the editor and Page Down shows the next item), `dont_save_in_the_editor_close_prompt_unblocks_the_operator`. All 6 fail on the 6859b3f sources (restored with only the new test file). Per-fix reverts, one at a time: without the focus-lost hand-off, or without the list's `SelaSongRow` context, the scroll, Ctrl+End and only-song tests fail (without the context, Page Down goes live); without `reveal_song` the delete-last test fails; without the interceptor the prompt test fails; without the backdrop both prompt tests fail; without clearing the count on No the Don't Save test fails.
- Results: In this worktree, `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -- -D warnings` exit 0; `cargo test --locked --all-targets -- --test-threads=8` exit 0 (lib 263 passed, 1 ignored; bin 230 passed, 3 ignored; output_process 9 + 1 ignored; transport_process 4 + 1 ignored; examples composition_spike 12, input_check 9, native_cues 18 + 1 ignored, output_spike 1, video_spike 13), first run, no reruns needed. `cargo test --locked --bin sela focus_modality -- --test-threads=4`: 6 passed. `python -m ruff check scripts` not run (no script changed).
- Unexecuted / open: native `scripts/resource-browser-windows.py` and `scripts/song-metadata-windows.py` (they take over the keyboard and mouse; the parent runs them after merging). `song-metadata-windows.py` still clicks only inside the editor; it should also click the operator's Black and Go Live and press Ctrl+B and Page Down while the prompt is open, with receipt counts unchanged (scripts are outside this stream). UIA focus on the list element after a scroll is not checked natively. Unobserved in EW: focus after a scrollbar scroll, Tab from a list whose focused item is off screen (Sela goes to the visible row band), and whether a click on the main window brings a modal prompt forward (Sela does not).
- Next: merge, then the parent runs the two native scripts, with the `song-metadata-windows.py` extension above.

### import-openlyrics-parser — M1-14 parser core (2026-10-22)
- State: parser core only; M1-14 stays **planned**, not done (framework, `sela-import` worker, staged preview, duplicate policy, one-transaction commit, cancel and native checks remain). Backlog: the version/fields and entity/size/nesting items are checked; the mapping item is unchecked again in the review round (dropped attributes are not reported yet), and it and the test item note what the parser already does and what remains.
- Decisions: (1) Root must be `<song>` in the OpenLyrics namespace, else `WrongFormat` (an OpenSong `<song>` has none). (2) The first non-empty `<title>` wins; extra titles are reported as `UnknownElement("title")`. (3) `<publisher>` maps to Administrator. (4) Authors join with `"; "` (`import::song::AUTHOR_SEPARATOR`) in the one-line Authors field. (5) Only names in the code convention (letter, digits, one optional part letter) go through `label_for_code`; others stay verbatim (`bridge`, `custom`). (6) An order name selects every verse with that name in file order (lang variants); references to empty or limit-dropped verses are skipped silently. (7) XML whitespace collapses inside `<lines>`, Unicode spaces (U+3000) stay; `<chord>`/`<tag>` keep their text; `<comment>` is dropped. (8) A linear pre-scan caps nesting at 32 because roxmltree 0.21.1's `parse_element`/`parse_content` recurse per level; DTDs are disallowed (an empty DOCTYPE is rejected too, roxmltree `tokenizer::parse`), `nodes_limit` 50,000 gives `TooLarge`. (9) The parser bounds expanded order slides to `MAX_ORDER` and text to a 240 KiB budget so every parsed song passes `Song::validate`; `to_song` returns `Error::Invalid` otherwise. (10) Review round: a title of only spaces, Unicode spaces included, counts as missing, because `Song::validate` trims what `collapse` keeps; `MissingTitle` goes to the front of the warning list and survives the 64-warning cap, because the caller substitutes a title on it. Order resolution looks names up in a `HashMap`/`HashSet` instead of scanning `Vec`s, so it is linear in the order length. Verse names collapse XML whitespace; a missing or blank name gives an empty label and `InvalidField("verse name")`. A second `<copyright>`, `<ccliNo>`, `<publisher>` or `<verseOrder>` is reported as `UnknownElement(name)`. OpenLyrics 0.7 `<line>` ends a line instead of being reported and glued. Dropped attributes stay unreported: no `Warning` variant fits, and adding one is the framework's call because other streams may match on `Warning` (docs/import.md). (11) L check: no license is stated by the spec, its repository or GitHub's license API; nothing of it is copied; fixtures are original (docs/import.md).
- New: `src/import/openlyrics.rs` (`parse`, 15 tests including two 256-case proptests), `src/import/song.rs` (`to_song`, 4 tests), `pub mod song;` in `src/import/mod.rs`, fixtures `tests/fixtures/import/openlyrics/{fields,losses,unicode,malformed-unclosed,xxe-file,xxe-url,billion-laughs}.xml`, docs/import.md "OpenLyrics". VAL-IMPORT-003 is `song::tests::the_verse_order_becomes_the_arrangement_and_the_slide_order` (`Song::arrangement` and `slides::project` follow "v1 c v2 c b c", the two-slide bridge expanded); VAL-IMPORT-004 is `hostile_files_are_rejected_within_a_bounded_time`; VAL-IMPORT-016 is `indonesian_accented_and_cjk_text_is_kept_exactly`; VAL-IMPORT-017 (OpenLyrics part) is `arbitrary_bytes_never_panic` and `mutated_documents_never_panic`, which also assert that every parsed song maps to a valid `storage::Song`.
- Revert checks (each made temporarily, then restored with `git checkout`): without the depth pre-scan the cap case imports and the 1 MiB open-tag case aborts with STATUS_STACK_OVERFLOW (0xc00000fd); `split_whitespace` instead of XML whitespace fails the CJK test; no code-name gate fails the verbatim and losses tests; no warning dedupe fails three tests; no skipped-name set fails the limits test; keeping comments fails two tests; expanding only a section's first slide fails three `song` tests. Review round, failing first: the five new tests `titles_of_only_unicode_spaces_count_as_missing`, `a_long_order_of_unknown_names_resolves_within_a_bounded_time` (9.69 s against its 5 s budget in the debug test build: 20,000 empty verses and about 190,000 unknown order entries in 1 MiB), `verse_names_are_collapsed_and_a_missing_name_is_reported`, `repeated_single_properties_keep_the_first_and_are_reported` and `openlyrics_0_7_line_elements_become_line_breaks` were run against the unfixed parser with `cargo test --locked --lib import` (25 passed, 5 failed), then pass after the fix; reverting only the front insertion of `MissingTitle` to a capped `warn` fails the title test at "kept when the warning list is full" (`cargo test --locked --lib import::openlyrics::tests::titles`, restored with `git checkout`). The proptest `check` now substitutes a title only when `MissingTitle` is present, so a blank title without the warning would fail it.
- Results (worktree `D:\Projects\sela-wt\w2-import-openlyrics`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -- -D warnings` exit 0; `cargo test --locked --lib import` 25 passed (14 new), 253 filtered out, exit 0. Review round: `cargo fmt --all -- --check` exit 0 (after `cargo fmt --all` reflowed two lines of the new code); `cargo clippy --locked --all-targets -- -D warnings` exit 0; `cargo test --locked --lib import` 30 passed (19 new), 0 failed, 253 filtered out, 0.21 s, exit 0. No proptest failure, so no `proptest-regressions/` file exists yet; a failure would write `proptest-regressions/import/openlyrics.txt`, which must be committed.
- Unexecuted / open: the full `cargo test --locked --all-targets` gate (seven other streams were building on the machine; only `--lib import` was required); search by an accented word and Preview rendering of imported text (VAL-IMPORT-016, need the framework's `save_song` path); per-file errors in a staged batch (VAL-IMPORT-004 batch part, VAL-IMPORT-013); alternate titles and the attributes listed in docs/import.md are not imported; ignored attributes are not reported (needs a framework decision on a `Warning` variant); `MissingTitle` substitution and the empty verse-name label are not yet shown in a staged preview.
- Next: M1-14 framework on top of `openlyrics::parse` and `song::to_song`: `sela-import` worker, staged preview with warnings, duplicate policy, one transaction through `save_song`, cancel.

### import-opensong-chordpro-parsers (2026-10-22)
- State: M4-07 OpenSong and ChordPro parser cores **implemented-unqualified**. They produce `ParsedSong` only; the M1-14 framework hookup, staged preview, `ParsedSong` to `storage::Song` mapping (src/import/song.rs, another stream) and the native VAL-IMPORT-005/006 runs remain, so the backlog child lines stay open.
- Decisions: (1) Chords are removed in both formats (`ChordsRemoved`, once per file); the song model has no hidden chord data. (2) OpenSong: roxmltree with `allow_dtd: false` (any DOCTYPE is `Entities`), `nodes_limit` 100,000 (`TooLarge`), and a text scan that rejects nesting deeper than 32 as `TooDeep` before the parse, because roxmltree recurses per nested element and a 100,000-deep file overflowed the test thread's stack (STATUS_STACK_OVERFLOW) without the scan. (3) OpenSong `<author>` splits at `,` `;` `&` `/` `|` and the word "and"; "dan" is not a separator. (4) OpenSong `||` ends a slide, `|` breaks a line, and empty pieces are dropped; numbered lines `N text` under `[V]` become sections V1, V2 in first-appearance order. (5) Only one-letter-plus-digits tags go through `label_for_code`, because it reads only the first letter (`Coda` would become "Chorus"); other tags stay verbatim. (6) Unmapped OpenSong elements with text are `UnknownElement(name)`; known but unmapped ChordPro directives (`key`, `capo`, `new_page`) are `UnknownDirective(name)`. No `Warning` variant was added, so other streams' exhaustive matches still compile. (7) ChordPro `{subtitle}` fills the authors only when there is no `artist`/`composer`/`lyricist`, else it is reported as `UnknownDirective("subtitle")`. (8) ChordPro inline brackets nest: each matched pair and its content is removed (`x[A[B]]y` is `xy`), so no new chord forms from leftovers; a chord-only line is dropped and never separates verses. (9) `{chorus}` makes the order explicit (every section in file order plus repeats of the last chorus); without it the order is empty. (10) Every warning is reported once per file. (11) Proptest failures persist to `tests/fixtures/import/{opensong,chordpro}/proptest-regressions.txt` (`FileFailurePersistence::Direct`) instead of a root `proptest-regressions/` folder. Review fix round: (12) OpenSong presentation names resolve to a `Target` (`Sections(indices)` or `Reported`). The empty text before the first tag no longer registers "V", so an order "V" with no V section is `MissingOrderReference("V")` instead of a silent skip; a bare tag whose lines are all numbered plays its numbered verses (`V` on `[V]` with `1`/`2` lines gives V1 V2). Only names whose sections were reported as `EmptySection` or cut by `LimitReached` are skipped quietly. (13) OpenSong text after a tag's `]` is the tag's first lyric line instead of being dropped. (14) Unnumbered lines under a numbered tag keep their section ahead of the numbered verses and own the bare name; documented and asserted, not changed. (15) ChordPro brackets whose content has whitespace are lyrics (chord names have no spaces), except a `[*annotation]`; `Open [G line [C] more] end` now keeps "Open [G line more] end" instead of "Open end". (16) ChordPro `{}` is ignored, and `{: x}` is `UnknownDirective` with the whole line instead of an empty name.
- New: `src/import/opensong.rs` (20 tests), `src/import/chordpro.rs` (18 tests), fixtures `tests/fixtures/import/opensong/{morning-river,numbered-verses}.xml`, `tests/fixtures/import/chordpro/{harbor-light,unicode-lyrics}.cho` (original lyrics), `tests/fixtures/import/chordpro/proptest-regressions.txt` (3 stored seeds), docs/import.md "OpenSong and ChordPro".
- Tests: VAL-IMPORT-005/006 parser-level fixtures (`a_tagged_song_keeps_sections_order_and_metadata`, `directives_chorus_and_inline_chords_import`); VAL-IMPORT-007 edge cases (chords at line end, nested and unclosed brackets, empty sections, CRLF/LF, BOM, non-ASCII, multiple choruses, missing title, oversized, XXE, billion laughs, deep nesting, node flood, limits); VAL-IMPORT-016 ChordPro part (`indonesian_accented_and_cjk_text_is_kept_exactly`); VAL-IMPORT-017 OpenSong and ChordPro part (256 cases per property on arbitrary bytes and token soups). Revert checks (each fix disabled temporarily, then restored): without the depth scan `oversized_deep_and_node_heavy_files_are_rejected` crashed the test process with STATUS_STACK_OVERFLOW; calling `label_for_code` for every tag failed `tags_map_to_sela_labels_and_custom_tags_stay_verbatim`; the earlier single-pass bracket rule (a chord is `[` to the next `]` with no bracket between) alone failed `chords_at_line_end_nested_and_unclosed_brackets` and the ChordPro property, minimized to `[[G]]` leaving `[]`; with chord-only lines also kept, `empty_environments_are_reported` and `files_without_lyrics_or_too_large_are_rejected` failed too and the property minimized to a `[]` line between blank lines. Both seeds are stored and `minimized_property_cases` asserts them. The first stored seed (`}[` then `]`) was a test bug: the no-chord invariant must hold per line, not across a slide. Review fix round, tests written first and run against the unfixed code: `an_order_name_with_no_section_is_reported_even_if_a_hidden_block_had_it` (warnings `[]`), `a_bare_tag_in_the_order_plays_its_numbered_verses` (order `[2, 1]`), `text_after_a_tag_is_a_lyric_line`, `brackets_around_spaced_words_are_lyrics_unless_an_annotation` ("Open end") and `an_empty_directive_is_ignored_and_a_nameless_one_is_reported_whole` (`UnknownDirective("")`) failed; `unnumbered_lines_under_a_numbered_tag_come_first_and_own_the_bare_name` passed (documents existing order). New OpenSong properties `markup_soup_never_reaches_the_parser_too_deep` (XML markup soup checked against roxmltree's own depth) and `nesting_is_cut_exactly_at_the_cap` (well-formed nesting with comments, CDATA, PIs and quoted `>` or `/>`; `TooDeep` exactly above 32). Revert checks on the depth scan: skipping of comments/CDATA/PIs disabled failed `nesting_is_cut_exactly_at_the_cap` and `oversized_deep_and_node_heavy_files_are_rejected`; quote handling disabled failed only `nesting_is_cut_exactly_at_the_cap` (after adding the `<a z='/>'>` open); both restored and the seed file that run wrote was deleted.
- Results (worktree `D:\Projects\sela-wt\w2-import-opensong-chordpro`, `CARGO_TARGET_DIR=C:\sela-target\w2-import-opensong-chordpro`), fix round: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -- -D warnings` exit 0; `cargo test --locked --lib import` 49 passed, 0 failed, 253 filtered out (38 in `import::opensong` and `import::chordpro`).
- Unexecuted / open: the full `cargo test --locked --all-targets` (only the `import` filter was required for this stream); native VAL-IMPORT-005/006 (need the framework and staged preview); a per-file timeout and a memory measurement (bounded here by the 1 MiB input cap, linear scans and the 10 s check on a 1 MiB bracket line, not measured); non-UTF-8 OpenSong/ChordPro files are rejected as `InvalidUtf8`; one slide may hold up to the whole 1 MiB file, so the song mapper must enforce `storage::MAX_SONG_BYTES`; `label_for_code("Coda")` returns "Chorus" in mod.rs (guarded here, not fixed there); whether "dan" should separate OpenSong authors is an owner call; a ChordPro chord written with a space (such as `[G sus]`) is now kept as lyric text, which the staged preview shows, and real files have not been checked for that form.
- Next: wire both parsers into the M1-14 import framework by file extension (`.xml` without an OpenLyrics root, `.cho`/`.chopro`/`.crd`/`.pro`), then the native VAL-IMPORT-005/006 runs.

### import-ew-rtf-words (2026-10-22)
- State: **implemented-unqualified**, parser core only, for the M4-07 EasyWorship child (mission feature `easyworship-library-import`). `import::ew_rtf::parse(&[u8]) -> Parsed` fills `sections` and `warnings`. Opening `Songs.db`/`SongWords.db`, metadata mapping, the missing `word` row (a framework case in VAL-IMPORT-009), `song_uid` duplicates and the UI stay with that feature.
- Evidence: the local EW 8.0.49 `SongWords.db` was copied to a new `%TEMP%` folder and opened read-only (`python -I`, `sqlite3`, `?mode=ro` URI). Only control-word structure was printed, with text masked; docs/import.md records the shape. SHA-256 of the original before the copy, after the copy and at the end: `E5253E21EE1E61D4693F7BAE92629695DC5415E02A6FB78450C3DE0A162AABB3`. Its mtime stayed `2026-10-06T06:35:26.9798497Z`, no `-wal`/`-shm` appeared, and the copy was deleted. Parsing that copy with a temporary, uncommitted test printed only counts: 2 sections, slides of 3/2/1 lines and 1 line, both labels outside `labels::parse`, 0 warnings. That matches the 4 `slide_uids` and the research decode.
- Decisions: (1) One `ParagraphKind` (Lyric/Label/SlideMarker) per paragraph, set by `\pard`, `\sdparawysiwghidden` and `\sdslidemarker`, instead of two flags. (2) EW8 rules when `\sdeasyworship`, a label or a marker appears; otherwise legacy blank-line rules with `labels::parse` label lines. Unlabelled legacy blocks join the previous labelled section, or else become "Verse N". (3) Raw bytes >= 0x80 in text are `Malformed`, since the source is an SQLite TEXT column and the encoding would be a guess. (4) `\'hh` follows the font's `\fcharset` (0, 204) or `\ansicpg` (1252, 1251). Other pages decode as 1252 with `UnknownDirective`. (5) Dropped `\b`/`\i`/`\ul`/`\strike` give `UnknownElement` once each. (6) No new `Warning`/`Rejected` variants, and `mod.rs` is unchanged.
- Fixes found in review, each with a test that fails without it (checked by reverting each fix alone): the font table was a `Vec` scanned per `\'hh`, and a run of labels rescanned the blank lines before it. Each was quadratic on 1 MiB inputs; the combined worst case took 83.0 s before the fixes. Now `fonts` is a `BTreeMap`, and blank lines are dropped at a label. Tests: `largest_font_tables_decode_in_linear_time` and `largest_label_runs_parse_in_linear_time` (10 s bound; the pair runs in well under a second). A scratch mutation script also reverted 15 other rules one at a time (markers, labels, `\uc` skip, `\bin`, legacy joining, depth cap, raw-byte rejection, charset 0, `\plain`, surrogate pairs, empty-slide warnings, skipped destinations, `\*`, slide limit, label after lyrics), and the suite caught every one. `declared_cp1251_and_font_charsets_decode` was given an `\ansicpg1251` document so that charset 0 differs from the document page.
- New: `src/import/ew_rtf.rs` (parser and 24 tests: 20 unit tests and 4 bounded proptests at 256 cases each, with persistence to `tests/fixtures/import/ew_rtf/proptest-regressions.txt`). That file holds 2 seeds that the round-trip proptests minimized against the marker and legacy-join mutants: a two-slide EW8 section and a two-slide legacy section. Every run replays them. Fixtures `tests/fixtures/import/ew_rtf/{ew8_labels,legacy_blank_lines}.rtf` use original text. docs/import.md "EasyWorship song words". A backlog child line under M4-07, not done.
- Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -- -D warnings` exit 0; `cargo test --locked --lib import` exit 0 (35 passed, 253 filtered out).
- Unexecuted / open: the full `cargo test --locked --all-targets`, builds and ruff were not run, because the task's gate is the three commands above and seven other streams were building. How EW8 writes non-ASCII lyrics, where it saves per-slide formatting, and whether EW8 shows a legacy "Verse 1" first line as a label are all unobserved (docs/import.md). For mapping: the framework turns each slide of a `ParsedSection` into its own `storage::Section` with the same label. Under EW8-OBS-022, follow-on slides should carry an empty label instead, so that they join one editor group.
- Next: the `easyworship-library-import` feature: read-only copies of `Songs.db` + `SongWords.db` with a `UTF8_U_CI` collation, then call `ew_rtf::parse` per `word` row.

### scripture-reference-parser — M4-04 parser core (2026-10-22)
- State: M4-04 stays open. The parser line is **implemented-unqualified**: no Scriptures tab uses it yet, and EasyWorship 8.0.49 completion is unobserved (VAL-BIBLE-010).
- Decisions: (1) `Book` is the canonical number 1-66 over one table (`src/scripture/books.rs`): OSIS id, English name, LAI Terjemahan Baru name, English and Indonesian aliases (the first of each is the abbreviation for "Abbreviate Book Names"), KJV chapter counts. The English name of book 22 is "Song of Solomon"; "Song of Songs" is an alias. (2) `Reference` is a book plus `Span::Chapters { first, last }` or `Span::Verses { start, end }`, equal ends for a single chapter or verse; `Reference::new` is the one validation point, so a held `Reference` always has existing chapters and an ordered range. (3) `:` and `.` both separate chapter and verse; `-`, en dash and em dash separate range ends; after `C:V-` a bare number is a verse, so `John 3:18-16` is `InvalidRange`. (4) Book resolution: an exact name/alias key (letters and digits, lowercased, either language) wins, else the first book in canonical order with a key starting with the typed text; `complete` lists the exact book then the prefix books in canonical order, so its first entry is what `parse` picks. Cross-language effect: `Ma` gives Mazmur (Psalms). Recorded with examples in docs/scripture.md. (5) Errors: `Empty`, `UnknownBook(typed)`, `MissingChapter(book)`, `ChapterNotFound { book, chapter }` (also chapter 0 and overlong numbers, which saturate), `InvalidRange`, `Malformed` (no book, stray characters, verse 0, `John 3-4:2`, comma lists). (6) Verse counts are not known here: `John 3:99` parses and the Scriptures tab validates verses against the loaded translation. (7) Not supported, reported as errors: Roman numerals, verse-only shorthand for one-chapter books (`Jude 5` is chapter 5).
- New: `src/scripture/books.rs`, `src/scripture/reference.rs`, `src/scripture/mod.rs` re-exports (`Book`, `complete`, `parse`, `ParseError`, `Reference`, `Span`, `Verse`), `docs/scripture.md`. 20 unit tests: all 66 books by English name, English abbreviation, Indonesian name and Indonesian abbreviation (as written, lower, upper, with a trailing dot, and without the space for numbered books) from an independent table in the test; each book's last chapter and the next one; numbered-book spacing; every form; case/whitespace; each error; the ambiguity rule; the table invariants (every alias resolves to its own book, 1,189 chapters with 929 in the Old Testament).
- Fix found while writing: `John -3` parsed as John 3 because the book part accepts hyphens (Hakim-hakim). A book part ending in a hyphen is now `Malformed`; with that guard removed `text_that_is_not_a_reference_is_malformed` fails on "John -3" (checked by reverting it). Removing the exact-alias step of `complete` fails 10 tests.
- Review fix: the tokenizer dropped all whitespace before reading digits, so `John 1 2` parsed as John 12, `Ps 11 9` as Psalm 119 and `Mzm 2 3` as Psalm 23, with no error. Whitespace now separates tokens and ends a number, so two adjacent numbers are `Malformed`; a space is not adopted as a chapter/verse separator because EW's handling of `John 3 16` is unobserved. `John 3 : 16 - 4 : 2` still parses. Test-first: the six new `Malformed` cases (`John 1 2`, `Ps 11 9`, `Mzm 2 3`, `John 3 16-18`, `John 3:1 6`, `John 3:16-4 2`) failed on the old tokenizer (19 passed, 1 failed, `John 1 2` => `Ok(Chapters { first: 12, last: 12 })`) and pass with the fix. docs/scripture.md now states the whitespace rule and that punctuation inside a book name is ignored when matching (`Jo-hn 3` is John 3), which was the reviewer's minor note; the code there is unchanged.
- Results (worktree `D:\Projects\sela-wt\w2-scripture-reference`, `CARGO_TARGET_DIR=C:\sela-target\w2-scripture-reference`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -- -D warnings` exit 0 (the first run rejected the `&[...]` slice patterns in `parse`, fixed by matching on `tokens(locator)?[..]`); `cargo test --locked --lib scripture` 20 passed, 0 failed, 264 filtered out. After the review fix: fmt exit 0, clippy exit 0, `cargo test --locked --lib scripture` 20 passed, 0 failed, 264 filtered out.
- Unexecuted / open: full `cargo test --locked --all-targets` was not run in this stream (the brief's gate is the scripture filter; the parent runs the full gate after merging); no native check, since there is no UI. EW completion order and accept key (VAL-BIBLE-010), and EW's handling of `Jude 5` and of `John 3 16`, are unobserved. Parser messages (EN/ID `Msg` entries) belong to the Scriptures tab.
- Next: scriptures-tab-search wires `parse` and `complete` into the reference box, phrases `ParseError` in EN/ID, validates verses against the translation, and keeps the previous state on an error (VAL-BIBLE-009).

### ffmpeg-locator — M2-01/M2-02 FFmpeg base (2026-10-22)
- State: M2-01 and M2-02 stay **planned** (not done); this slice is the locator, build-license check and input argument core of mission feature ffmpeg-locate-fetch-supervise, unit-tested and not wired into the app. Decisions and the LGPL obligations and patent caveat are in docs/media.md.
- Decisions: (1) Lookup order `SELA_FFMPEG_DIR` (empty counts as unset), `<exe dir>\ffmpeg\bin`, then `.amp\in\ffmpeg\n8.1.3-lgpl\bin` (the feature text's pinned id) from `CARGO_MANIFEST_DIR` in debug builds only; PATH is never read. (2) A relative `SELA_FFMPEG_DIR` is refused as `Refusal::RelativeDir` without looking inside it, so the probed executable never depends on the start directory (review finding; before, it was resolved against the current directory). (3) `check_buildconf` matches whole flags, needs a `configuration:` section (else `Unrecognized`), and refuses `--enable-nonfree` before `--enable-gpl`; each message names the flag. (4) `ffprobe` is checked for presence only; `Located` keeps earlier refusals and `NotFound` lists every candidate with its reason. (5) The probe runs `-hide_banner -buildconf` and `-version` with stdin/stderr null, `CREATE_NO_WINDOW`, 64 KiB stdout and 5 s per command, killing and reaping the child on overrun. (6) `input_args` returns `-nostdin -hide_banner -protocol_whitelist file -i <path>` for absolute local paths only; `\\?\C:\` accepted, `\\.\`, `\\?\Volume{}`, GLOBALROOT refused as Device, UNC and `\\?\UNC` as Network, any two-plus-character `scheme:` as Url. Mapped and subst drives are not detected (needs `GetDriveTypeW`).
- New: `src/media/ffmpeg.rs` (28 tests plus 4 ignored child-process helpers), docs/media.md, M2-01/M2-02 progress notes in docs/backlog.md.
- Tests: `media::ffmpeg::tests::a_relative_env_dir_is_refused_without_probing` fails without the relative-dir check (checked by disabling it: refusal was `Missing(Ffmpeg)`). The dev-copy test now expects `None` in release builds (it asserted `Some` unconditionally). Mutation checks, all applied together and reverted: dropping the nonfree check failed `a_nonfree_build_is_refused_naming_the_flag` and `a_nonfree_install_build_is_refused_and_the_dev_copy_is_used`; dropping `-protocol_whitelist file` failed `absolute_local_paths_get_the_local_only_flags_before_the_input`; appending PATH directories to `locate()` failed `locate_reads_the_env_dir_and_never_path` ("PATH entry ... was searched"); skipping the kill on error failed `a_probe_child_that_hangs_is_killed_at_the_timeout` after 60 s.
- Results (worktree feat/ffmpeg-locator): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -- -D warnings` exit 0; `cargo test --locked --lib media` exit 0 (28 passed, 0 failed, 4 ignored, 263 filtered out, 0.62 s).
- Unexecuted / open: VAL-MEDIA-038 crafted-playlist runtime test (a local playlist referencing `http://` must fail with a protocol error and no network attempt) needs a real FFmpeg, which this stream must not download; `cargo test --release --lib media` (release-only dev-copy rule; a release build of the crate was too slow to run beside seven other streams); VAL-MEDIA-001 "discovery runs off the UI thread" and VAL-MEDIA-002 "version string is logged" (`locate()` has no caller; it documents the worker-thread rule and returns `Located::version`); VAL-MEDIA-003, 004, 030, 031 (warning UI, fetch script, Job Object supervisor) belong to the rest of the feature; mapped-network-drive detection.
- Next: the fetch script (VAL-MEDIA-004), then call `locate()` on a worker at startup, log the version and show the media-unavailable warning (VAL-MEDIA-003); run the VAL-MEDIA-038 playlist test against the dev copy.

### parallel-wave-2 integration (2026-10-22)
- State: all eight wave-2 branches merged into local `main` with `--no-ff` (layout-startup, arrangement-rendering, focus-modality, then import-openlyrics, import-opensong-chordpro, import-ew-rtf, scripture-reference, ffmpeg-locator). Each was built by one agent in its own worktree and reviewed by a second agent against the diff. The four branches whose review found a blocker or major issue got a fix round: OpenLyrics blank titles and quadratic order resolution, an OpenSong order name that vanished silently, scripture digits joined across spaces ("Ps 11 9" read as Psalm 119), and the missing FFmpeg docs and tracking. Only end-of-file work-log appends conflicted; both sides were kept.
- Integration fixes: (1) `to_song` labels only a section's first slide; the others get an empty label and join its group, as Sela's editor groups and EasyWorship shows them (EW8-OBS-022). (2) `label_for_code` no longer reads a word such as "Coda" as Chorus. (3) `EditorPromptOpen` lists the editor windows whose close prompt is open instead of counting prompts, and one `cx.on_window_closed` hook removes a window that closes by any path. With a bare count, an editor window removed with its prompt open left the operator blocked for good, Black included. A first attempt released the count from `cx.on_release` on the editor entity; it failed 2 of 2 full gate runs, because the entity is sometimes not released at all after its window closes, so entity lifetime cannot guard a safety control. New test `focus_modality::an_editor_removed_with_its_prompt_open_unblocks_the_operator` asserts the prompt list is empty once the window is gone, then waits (bounded `settle`) for Black, which lights only on the renderer's acknowledgement; an immediate read of Black after the click failed one full gate run under load. It fails without the window hook (checked by disabling it) and passes with it; `cargo test --locked --bin sela -- --test-threads=12` passed 4 consecutive runs (236 passed each). Open: why an editor entity can outlive its window (a possible leak, not investigated).
- Sequencing: the gate ran after the layout merge and after the two other fix merges together, and runs once more on the final tree. The five pure-module branches touch only new modules, so they share that final gate.
- Results: layout merge `69b5bb2` and fixes merge `fb53738` both passed the full gate (fmt, clippy, `cargo test --locked --all-targets -- --test-threads=12` with lib 268 and bin 235 on `fb53738`, both builds, ruff). Final tree (`0b97dd1` plus this commit): fmt, clippy, `cargo test --locked --all-targets -- --test-threads=12` (lib 396 passed + 5 ignored, bin 236 + 3 ignored, output_process 9, transport_process 4, examples pass), `cargo build --locked`, `cargo build --locked --example seed_library`, `python -m ruff check scripts` all exit 0.
- Unexecuted / open: every native script for the three fixes (`arrangement-windows.py`, `song-editor-windows.py`, `song-metadata-windows.py`, `resource-browser-windows.py`, `layout-windows.py` with the window on the spare monitor), plus the three Phase A re-runs from fix-sunday-schedule-documents. They take over the keyboard and mouse and need the owner idle. Owner question: a Fixed size that fits only without the copyright strip now drops the strip on that slide instead of refusing the slide (Sela's rule; EW unobserved).
- Next: native re-runs, then scrutiny-validator-sunday-core round 2.

### parallel-wave-2 native runs (2026-10-22)
- State: native re-runs for the five sunday-core scrutiny round 1 fix features, with the owner away and three monitors attached (DISPLAY5 2560x1440 primary operator at 125%, DISPLAY1 2560x1600 audience, DISPLAY6 1920x1080 spare at 100%). Debug build of the merged tree. The fixes stay **implemented-unqualified** (physical-projector, Narrator and older-hardware runs remain), but every native check they listed now passes.
- Results, each run once, one at a time, artifacts under `.amp/in/artifacts/wave2-<script>`: `schedule-windows.py` PASS (29 s), `arrangement-windows.py` PASS (54 s), `schedule-remainder-windows.py` PASS (181 s), `song-editor-windows.py` ok (54 s), `song-metadata-windows.py` PASS with audience receipts 0 for the editor prompt, Options and preflight (103 s), `resource-browser-windows.py` PASS (53 s), `layout-windows.py` PASS twice after the fix below, `live-output-windows.py` PASS, `navigation-windows.py` PASS.
- SC-SCHED-01 natively: `schedule-remainder-windows.py` now edits the song to four lines with the change on line 4 (`REVISED`) and checks the saved snapshot equals all four. `14-update-diff.png` shows the Update items New cell with all four lines and their breaks, while Live still shows the old slide.
- SC-LAYOUT-03 natively: `layout-windows.py` gained run 5 (window wholly on the spare monitor, quit, restart). Its first run FAILED: the window reopened on DISPLAY6 at the right origin but 20% smaller (960x608 to 768x487 outer). The saved value was right (`window=2649,438,942,561`). Cause, read in the pinned GPUI (zed a846890): `gpui_windows/src/window.rs` `retrieve_window_placement` creates the window on the primary monitor and then places it on `display_id` in that display's device pixels; the move fires WM_DPICHANGED, and `events.rs` `handle_dpi_changed_msg` accepts Windows' suggested rectangle, scaled by 96/120. Fix in `src/main.rs`: after the operator window opens, if its size differs from the requested windowed size by more than 2 px, `window.resize` restores it. Rerun: before and after both `[2640, 415, 3600, 1023]`, twice. This is an upstream GPUI bug (not filed); the workaround covers the operator window only. No headless test can reproduce a DPI change.
- Harness fix: `navigation-windows.py` wrote its summary in the locale code page and crashed on "⚠" after its checks passed; it now writes UTF-8.
- Unexecuted / open: mixed-DPI restore of a maximized window; song editor windows still open centred on the primary monitor (not saved); SC-SONG-03 stays with user testing (receipt-backed VAL-FOCUS-001 counts).
- Next: scrutiny-validator-sunday-core round 2, then user-testing-validator-sunday-core.

### e2e-harness — shared Linux E2E harness and parity checklist (2026-10-22)
- Ticket/state: M0-03 slice, **implemented-unqualified**. Shared infrastructure for the next wave of parallel streams; no product code changed. Worked directly on `ccr-5e192d6c-emgkai` (Linux cloud container, 4 CPUs, no Windows machine).
- Baseline fix (commit f05e27a, recorded here): the Linux test build failed with E0282 because `FONT.as_ref().into()` in two `src/audience/text.rs` test fixtures became ambiguous once another dependency added an `AsRef` impl for `[u8]`; the fixtures now build `Arc<[u8]>` directly. Test-only. After it fmt, clippy `-D warnings` and `cargo test --locked --all-targets` (706 tests) pass on Linux, and `cargo check --all-targets --target x86_64-pc-windows-gnu` passes.
- Decisions: (1) One harness module (`scripts/native_x11.py`, standard library plus xdotool/xwininfo/xprop/ImageMagick/dbus-daemon/gdbus) modelled on `native_win.py`: PID-scoped windows, focus verified before every chord (thefts logged, optional strict), root grab cropped to a window, bounded `wait_until`, read-only SQLite helpers, `nx.main` scaffolding with exit codes 0/1/2/130 and `summary.json`. (2) Private display per run by default (`-displayfd`, own Openbox), `--display :N` to attach, `serve` to keep one. (3) Every spawned process carries a private `SELA_E2E_RUN` tag; cleanup kills by tag and the harness is a child subreaper, because terminating the operator alone leaves the audience running (`retain_on_disconnect`). (4) GPUI at zed a846890 renders through wgpu Vulkan on Linux: `VK_DRIVER_FILES=/dev/null` makes the operator fail ("Failed to create surface for any enabled backend"), so the harness pins the lavapipe ICD; the audience also renders on lavapipe (its `gl` backend is not compiled in). (5) Audience under Xvfb: winit 0.30.12 counts RandR CRTCs (`x11/monitor.rs`) and Xvfb has one, so `secondary` has no monitor; `xrandr --setmonitor` on a 3840x1080 Xvfb did not change `sela --list-monitors` (still one `screen`). The `xvfb` kind runs `SELA_AUDIENCE_MONITOR=window` with Openbox rules (operator at 0,0; audience 624x351 top-right, not focused) so neither covers the other. The `dual` kind (Xorg dummy driver, DUMMY0 primary + DUMMY1, 1920x1080 each) gives the real `secondary` placement. (6) Controls are located by AccessKit author id over AT-SPI (at-spi2-core on the private bus, `gdbus` calls; 42 ids in ~0.4 s), the same ids the Windows scripts read through UIA. (7) File dialogs: GPUI uses xdg-desktop-portal on Linux, so the private bus activates xdg-desktop-portal-gtk (pinned by a private `portals.conf`); the path is typed after Ctrl+L in one burst because GTK's inline completion rewrote a slowly typed `/tmp/...` path. (8) `xdotool mousemove --sync` hangs when the pointer is already in place; the harness polls `getmouselocation` instead. (9) Installed in the container: x11-xserver-utils, xserver-xorg-video-dummy (+xserver-xorg-core), xdg-desktop-portal, xdg-desktop-portal-gtk, at-spi2-core; ruff 0.17.1 with `pip install --user`.
- New: `scripts/native_x11.py`, `scripts/live-output-x11.py` (adds `--tolerate-audience-focus`), `scripts/schedule-x11.py`, `scripts/song-editor-x11.py`, `docs/e2e-linux.md` (linked from `docs/native-testing.md`), `docs/parity-pending.md` (52 seeded items by area, twelve empty `## Stream:` sections, the Windows script run order); backlog M0-03 note and a new M1-10 item for the X11 audience focus.
- Differences from the Windows scripts (also in each docstring): the xvfb audience is a 624x351 development window, not a monitor-covering window; WM_CLOSE is Openbox Alt+F4 (WM_DELETE_WINDOW); the Save/Open dialogs are the GTK portal chooser; Arial is absent, so the third formatted slide uses the fallback face; Xvfb scale is 1.0; the editor double click is one `xdotool click --repeat 2`. Thresholds are unchanged (formatted gold 1212 samples against the Windows 400).
- Linux finding: on the dual display Openbox focuses the audience when it maps (winit 0.30 X11 ignores `with_active(false)`); `live-output-x11.py --kind dual` FAILS on it as it should, and PASSES with `--tolerate-audience-focus` (four thefts recorded). Ticketed under M1-10; Windows unaffected.
- Results: `python3 scripts/native_x11.py serve --display :100` (Xvfb :100 1920x1080x24 -nolisten tcp + Openbox), then `python3 scripts/live-output-x11.py --display :100` PASS (58 s), `python3 scripts/schedule-x11.py --display :100` PASS (34 s), `python3 scripts/song-editor-x11.py --display :100` PASS (64 s), artifacts `.amp/in/artifacts/<script>-x11/`, captures inspected (audience slides, masks, logo, gold/italic/fallback slides, Zoom/Maintain/missing/master backdrops; schedule states, guards and both GTK dialogs; editor Words, Slides, Format pane, fills and master). Private displays: `live-output-x11.py --kind dual --tolerate-audience-focus` PASS (audience 1920x1080 on DUMMY1, `.amp/in/artifacts/live-output-x11-dual`), `--kind dual` without the flag FAIL as expected, `schedule-x11.py` and `song-editor-x11.py` in parallel on private `:0`/`:1` PASS. Failure paths: a missing binary and an unusable display exit 2; a deliberate failure with the audience live exits 1 with operator and audience captures and leaves no tagged process or temporary folder. Gate: `cargo fmt --all -- --check` exit 0, `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0, `cargo test --locked --all-targets -j 3` exit 0 (706 passed, 10 ignored); `python3 -m ruff check scripts/*.py`: the new files are clean; one pre-existing finding remains (EXE001, `scripts/audience-smoke-windows.py` has a shebang but no executable bit).
- Unexecuted / open: no Windows run (no Windows machine; Rust unchanged); the older X11 scripts (`native-smoke.sh`, `operator-shell.py`, `song-library.py`) were not run or ported, and the documented `native-smoke.sh` command still uses `VK_DRIVER_FILES=/dev/null`; fixed clocks and failure injection (M0-03 first checkbox) are not part of the harness; Xvfb/lavapipe runs are not hardware, DPI, timing or screen-reader evidence.
- Next: feature streams name their checks `scripts/<feature>-x11.py`, follow docs/e2e-linux.md and add their Windows/EW leftovers under their `## Stream:` section of docs/parity-pending.md; fix the X11 audience focus (M1-10).

### import-framework — M1-14 song import framework with M4-07 OpenSong/ChordPro hookup (2026-10-22)
- State: M1-14 **implemented-unqualified** (all four checklist items checked from Linux evidence; the Windows script and every EasyWorship observation remain open, so not done). M4-07 OpenSong and ChordPro children: framework hookup, staged preview and Linux native run done, child lines stay open for Windows and the EW comparison. M1-07 "Maintain indexes ... imports" checked (import index maintenance through the save path). No SQLite schema change (schema_version_used: none).
- Scope delivered: `src/import/batch.rs` (file discovery: chosen files plus folders walked three levels without symlinks, hidden entries skipped, 20,000 entries per folder; format by extension/root; `read` bounded at `MAX_FILE_BYTES`; `stage` = parse + title from the file name + `song::to_song` + duplicate key; limits 500 files, 32 MiB staged text); `src/import/worker.rs` (the `sela-import` thread, one-job queue, `EVENT_DEPTH` 16 bounded events with back-pressure, cancel before every file, job-tagged events, test `Pace` hook); `Warning::IgnoredAttribute` in `src/import/mod.rs` and OpenLyrics reporting of title/author/verse(except name)/lines attributes; `storage::{ImportWrite, LibraryMatch, MAX_IMPORT_SONGS}`, `Repository::{commit_import, find_duplicates}`, `Command::{CommitImport, FindDuplicates}`, `Reply::{Committed, Duplicates}` and the shared `write_song` (now also behind `save_song`); `src/operator/song_import.rs` (File › Import ▸ Songs… / Song Folder…, asynchronous chooser, staged preview dialog, choice groups, bulk duplicates choice, commit, cancel, selection/focus of imported songs), small hooks in `operator/{mod,polling,menus,controls,layout,dialogs}`; 53 EN+ID strings in `src/i18n.rs`; fixtures `tests/fixtures/import/batch/` (original text: OpenLyrics with `lang`/`repeat` attributes and Spanish accents, extensionless OpenSong with a repeated `[C]`, ChordPro, malformed XML, ChordPro without words, a text note); docs/import.md "Import framework", reference-observations "Song import — UNOBSERVED best guesses" (EW8-OBS-IMP-1..6), parity-pending "Stream: import-framework" (PP-IMPF-01..08, run-order row 23), e2e-linux.md notes.
- Decisions: (1) Entry point File › Import ▸ Songs… (files, multiple) and Song Folder… (directories) flattened after Close Schedule like New ▸ — UNOBSERVED: EW8-OBS-053 shows no Import row in EW's File menu, EW8-OBS-061 none in the Songs menu, EW8-OBS-021's Import ▾ is per open song (EW8-OBS-IMP-1). The existing `menu_contents_match_the_captures` test now lists the three new rows. (2) `.xml`/extensionless XML: OpenLyrics root first, else OpenSong; a chosen file of another type is listed as unsupported, inside folders others are only counted. (3) Duplicate policy (best guess, EW8-OBS-IMP-4): title compared with case/spacing/punctuation ignored (accents count), words by SHA-256 of normalized lyrics; same words → Skip, same title → Import as copy, Replace = new revision of the library song (same id, schedules keep their pinned revision; backgrounds/formats of that song are not kept), in-batch copies → Skip; two Replaces of one song are refused before saving. Detection runs on the storage thread from the search index (titles, then words only for title matches). (4) One immediate transaction for the whole batch through `write_song`, the `save_song` path, so the FTS index, section ids, metadata and Date modified follow; any failure rolls everything back and keeps the preview for a retry. (5) Cancel in every phase; during the commit through the storage `Cancel` flag checked per song and before COMMIT; a late finish is reported honestly. (6) Storage replies carry the import job (`Request::SongImport(job)`), so a duplicate check of a cancelled import never lands on a newer one. (7) Staged songs are `Arc<Song>` so Import never copies their text on the UI thread. (8) Choice groups are one tab stop each (RadioGroup of RadioButtons, Left/Right/Up/Down), a keyboard-focused row scrolls into view, a clicked one does not move (it lost the click between mouse down and up in the first version). (9) Counted strings avoid plural forms ("Ready: 3 · Not importable: 2 · Duplicates: 0").
- Results (worktree `/home/user/sela/.claude/worktrees/wf_4eb25669-832-3`, branch `wave/import-framework`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0, 729 passed, 0 failed, 10 ignored (baseline 706; 23 new); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/song-import-x11.py --display :112` (Xvfb :112 + Openbox via `python3 scripts/native_x11.py serve --display :112`) PASS in 27 s, binary sha256 f9bed955…, imported slide live observed after 91 ms (capture polling, not cue latency); captures inspected (preview dialog, duplicates dialog, imported selection, accented search, audience text "Luz del alba, luz de paz / guíanos un poco más"); `python3 -m ruff check` and `ruff format --check` on both new scripts: clean. GPUI tests (`src/tests/song_import.rs`): `folder_import_previews_commits_and_finds_accented_words`, `file_import_by_keyboard_reports_unsupported_files_and_cancels`, `cancel_while_files_are_read_commits_nothing`, `cancel_before_the_commit_reaches_storage_saves_nothing`, `a_failing_save_rolls_back_the_batch_and_keeps_the_preview`, `duplicate_choices_skip_copy_and_replace`, `a_duplicate_reply_for_an_older_import_is_ignored`. Revert checks (each temporarily, then restored with `git checkout`): committing each song in its own transaction failed `storage::tests::a_failing_save_rolls_the_whole_import_back` and the GPUI rollback test (left `["Quiet Canticle", "Signal Hymn", "Sungai Kehidupan"]`); disabling attribute reporting failed `every_supported_field_is_mapped`, `content_that_is_not_imported_is_reported_once` and the GPUI folder test (`IgnoredAttribute("verse/@lang")`); committing without the index failed `an_import_commits_every_song_in_one_transaction_and_indexes_it` and the GPUI folder test (accented search never found the songs); removing the job guard failed `a_duplicate_reply_for_an_older_import_is_ignored`.
- Unexecuted / open: `scripts/song-import-windows.py` NOT RUN (no Windows host; PP-IMPF-07); every EasyWorship 8.0.49 observation (PP-IMPF-01..06); Indonesian visual and Narrator pass of the dialog (PP-IMPF-08); a native cancel during a long commit (covered by the storage flag test and the queued-commit GPUI test only); per-file read timeouts (a hung share would hold the worker, never the UI); performance of very large batches was not measured (bounded by 500 files / 32 MiB); `.txt`, EasyWorship and SongSelect files are not imported; the EasyWorship library importer is another stream. Linux X11 harness findings recorded in docs/e2e-linux.md (GTK chooser on Recent ignores Enter; tooltip windows; GPUI ignores xdotool's on-the-fly keymap).
- Next: run `song-import-windows.py` on Windows and the PP-IMPF observations in EW 8.0.49; adjust the entry point and duplicate policy to what EW does; then the `ew-library-import` stream can reuse `batch::Staged`, the preview and `commit_import` for EasyWorship songs.

### import-framework — review round 1 fixes (2026-10-22)

- State: M1-14 stays **implemented-unqualified** (Windows script and EW observations open); M4-07 unchanged (active).
- Fixed (each behavior fix with a test that fails without it; 8 is a test-only change):
  1. Major: Ctrl+Q (bound in the plain "Sela" context, so it fires behind a dialog) or the window close request with a dirty schedule replaced the import preview with the Unsaved prompt and left `song_import.phase` in Review/Reading/Checking, disabling File › Import until restart. `Operator::may_close` now refuses while `SongImport::holds_dialog()` (Reading, Checking, Review, Committing) and shows "Finish or cancel the import before quitting." in the dialog (EN+ID); `open_dialog` cancels the import whenever another dialog replaces `Dialog::ImportSongs`. UNOBSERVED best guess EW8-OBS-IMP-7, check PP-IMPF-09. GPUI `quit_waits_for_an_open_import_and_a_replaced_preview_cancels_it` (fails at the Ctrl+Q assertion with the old `may_close`, and at `phase == Idle` with the old `open_dialog`; both reverts tried).
  2. The folder walk's 20,000-entry budget now reports each cut chosen folder (`Found::truncated`, `Event::Found::truncated`, preview note "Folder has more than 20000 entries; files after that were not looked at: …"). Unit `a_folder_over_the_entry_budget_is_reported_not_silently_cut` (via `find_bounded` with a small budget, exact-budget case not reported); GPUI `a_folder_cut_at_the_entry_budget_is_noted_in_the_preview` (notes now carry `import-note-N` debug selectors).
  3. Only regular files are candidates in a folder walk (pipes/sockets/devices counted as not song files), and `batch::read` refuses a non-regular path from `metadata` before opening it, so a FIFO can no longer hang `sela-import`. Unit `a_named_pipe_is_never_opened` (unix; mkfifo; bounded 5 s wait). Residual: a path swapped to a FIFO between the metadata check and `open` (TOCTOU) could still block; per-file read timeouts stay open.
  4. The 32 MiB staged-text limit has its own `FileError::BatchTooLarge` ("Not read: one import holds at most 32 MiB of song text"), separate from `BatchFull` (500 files). Unit `the_byte_limit_and_the_file_limit_are_told_apart` (worker `over_budget`) plus the text check in `every_warning_and_error_has_text`.
  5. The Replace note (EN+ID) now says the new revision drops the song's slide formatting, backgrounds and master background and that the file's order replaces its arrangements; docs/import.md says the same. Inspected in the x11 duplicates capture (wraps inside the dialog). Asserted for EN and ID in `every_warning_and_error_has_text`.
  6. A chooser answered while another dialog is open now reports "Close the open dialog, then import again." instead of "Another import is still finishing". GPUI `a_chooser_answered_behind_another_dialog_says_so`.
  7. Cancelling-path coverage: storage `a_cancel_between_songs_or_before_commit_rolls_back` (cancel at check 2, 3 and 4 = between songs and right before COMMIT, via the new private `commit_import_until`; nothing saved, index consistent); GPUI `a_cancel_after_the_commit_reached_storage_rolls_back_or_reports_late` (commit handed over, Cancel → Cancelling with the dialog closed; `Err(Canceled)` → "Import cancelled"; a late `Committed` → "finished before it could be cancelled", selection and catalog follow).
  8. `menu_contents_match_the_captures` renamed `menu_contents_match_the_captures_plus_tracked_deviations`: the File list is EasyWorship's captured rows (EW8-OBS-053) with the Import rows spliced in from an explicit DEVIATIONS table naming PP-IMPF-01; same assertions, no longer presented as parity.
- Decisions: refusing the close (not cancelling the import) keeps an accidental Ctrl+Q from discarding a reviewed preview; `Choosing` and `Cancelling` do not block closing (no dialog holds state; a chooser that never returns must not trap the operator, and a cancelled commit is atomic either way).
- Results (worktree `/home/user/wt/import-framework`, branch `wave/import-framework`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0, 737 passed, 0 failed, 10 ignored (729 before; 8 new); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/song-import-x11.py --display :112` (on `python3 scripts/native_x11.py serve --display :112`, stopped afterwards) PASS, binary sha256 eb5eef95…, imported slide live observed after 89 ms (capture polling); duplicates capture inspected.
- Unexecuted / open: `scripts/song-import-windows.py` NOT RUN (no Windows host; PP-IMPF-07); EW observations PP-IMPF-01..06 and 09; PP-IMPF-08; per-file read timeouts and the FIFO TOCTOU window; a native (not in-process) cancel during a long commit; large-batch performance.
- Next: as before — Windows run and the PP-IMPF observations in EW 8.0.49.

### import-framework — review round 2 fixes (2026-10-22)

- State: M1-14 stays **implemented-unqualified** (Windows script and EW observations open); M4-07 unchanged (active).
- Fixed (each behavior fix with a test that fails without it; every revert below was tried and then restored):
  1. The settings close waiter closed the window after Don't Save without the import guard (`discard_still_holds() || may_close`). It now skips the Don't Save shortcut while `SongImport::holds_dialog()`, so `may_close` refuses and shows the quit note. The same hole existed for Save-then-Quit: `resume_guarded` → `guard(Quit)` → `proceed(Quit)` closed through `settings_allow_close` alone; `proceed` now routes `Then::Quit` through `may_close` while the preview holds state. GPUI `settings_ui::an_import_opened_during_the_settings_wait_after_dont_save_keeps_the_window` (old waiter: window count 0) and `schedule_files_ui::an_import_opened_during_a_save_then_quit_keeps_the_window` (old `proceed`: the bounded wait for the quit note times out).
  2. The quit message has its own slot (`SongImport::quit_note`, `import-quit-note`, Role::Label, below `import-error`) instead of overwriting `song_import.error`, so a failed duplicate check ("every song is offered as new") or a commit failure stays visible. `quit_waits_for_an_open_import_and_a_replaced_preview_cancels_it` now drives the real failed-check reply first and asserts the warning survives Ctrl+Q (fails when `may_close` writes `error`).
  3. `a_named_pipe_is_never_opened` names the FIFO `pipe.cho`, so the folder-walk half only passes with the `kind.is_file()` guard (fails without it: the pipe is staged).
  4. `a_folder_cut_at_the_entry_budget_is_noted_in_the_preview` now runs the real path: 20,000 empty `.txt` files in a subfolder push the walk past `MAX_FOLDER_ENTRIES` on the `sela-import` thread; the test checks `song_import.truncated`, the exact EN note text with the folder path (`SongImport::notes()`, now shared with rendering) and the rendered `import-note-N`. Fails when the `Event::Found` arm binds `truncated: _`.
  5. Native quit checks: `scripts/song-import-x11.py` presses Ctrl+Q and Alt+F4 (WM_DELETE_WINDOW) with the duplicate preview open and requires the operator, the preview and the AT-SPI `import-quit-note` "Finish or cancel the import before quitting." (capture `operator-import-quit-refused`); `scripts/song-import-windows.py` does the same with Ctrl+Q and WM_CLOSE (NOT RUN). PP-IMPF-09 lists them.
  6. Docs: backlog M1-14 note says PP-IMPF-01..09; docs/import.md, reference-observations EW8-OBS-IMP-7 and PP-IMPF-09 describe the separate note and the resumed-quit cases.
  7. The FIFO swap between the metadata check and the open is closed on Unix: `batch::open_regular` opens with `O_NONBLOCK` (`libc` 0.2.190 as a direct `cfg(unix)` dependency, already in the lock through gpui) and checks the opened file's type. Unit `opening_a_named_pipe_returns_at_once` (calls `open_regular` on a FIFO directly; without the flag the bounded 5 s wait fails). Per-file timeouts for hung network shares stay open.
- Also fixed while there: AccessKit names a `Role::Label` node from its value on AT-SPI and UIA (`label_comes_from_value`), so `import-error` and the import notes had no accessible name natively; they and the quit note now set `aria_value` as well as `aria_label` (found when the x11 quit-note check read an empty name). The same applies to `schedule-file-error` and `schedule-unsaved` (other streams' code, not changed here; recorded for their owners).
- Decisions: a quit refused because of an open import is dropped, not queued; the operator quits again after Import or Cancel (same as round 1).
- Results (worktree `/home/user/wt/import-framework`, branch `wave/import-framework`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0, 740 passed, 0 failed, 10 ignored (737 before; 3 new); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/song-import-x11.py --display :112` (on `python3 scripts/native_x11.py serve --display :112`, stopped afterwards) PASS in 29 s, binary sha256 be45d82a…, imported slide live observed after 81 ms (capture polling), quit note read over AT-SPI; quit-refused capture inspected (preview kept, note under the rows); `python3 -m ruff check` / `ruff format --check` on both scripts clean, `py_compile` OK.
- Unexecuted / open: `scripts/song-import-windows.py` NOT RUN (no Windows host; PP-IMPF-07); EW observations PP-IMPF-01..06 and 09; PP-IMPF-08; per-file read timeouts (hung shares); a native cancel during a long commit; large-batch performance.
- Next: as before — Windows run and the PP-IMPF observations in EW 8.0.49.
- Integration (2026-10-22): merged into `ccr-5e192d6c-emgkai` as merge commit `4431e12` ("Merge branch 'wave/import-framework'", `--no-ff`, no conflicts; the integration branch was still at the stream's base `8ba013e`; no schema or format version involved). Gate on the merged tree in `/home/user/sela`: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0, 740 passed, 0 failed, 10 ignored; `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; serially on `python3 scripts/native_x11.py serve --display :199` (stopped afterwards): `live-output-x11.py`, `schedule-x11.py`, `song-editor-x11.py`, `song-import-x11.py` each PASS (exit 0). Windows scripts still NOT RUN (no Windows host).

### scrutiny-r2 — sunday-core scrutiny round 2, scenarios S01/S02, setup (2026-10-22)
- State: fixes and E2E automation; M1-03, M1-05, M1-09, M1-10 and M1-16 keep **implemented-unqualified**/active (Windows, Narrator, EasyWorship and hardware checks remain); M1-15 is now **active** (S01/S02 automated on Linux only). M1-02, M1-04, M1-06, M1-07, M0-08 and M1-11 were audited where they meet the operator (key bindings and contexts, Songs rows, schedule documents and dirty state, cue preparation and acknowledgments, masks, the preflight Go Live hold) with no new confirmed defect; their state is unchanged. Not audited in depth: the Song Editor internals (`song_library.rs`, `format_pane.rs`), `storage.rs`, `images.rs`, `settings.rs`, `text_input.rs`. Worked on `wave/scrutiny-r2` in a Linux container (4 CPUs, no Windows machine). Not re-reported: the SC-* round 1 items (fixed, withdrawn or deferred as recorded).
- Confirmed defects (each fixed; each regression test failed before its fix, checked by reverting only that fix):
  - **SC2-FOCUS-01** Songs row context menu not modal for show keys: with the menu open (it takes the focus; context `SelaSongMenu` under `SelaShow`), B turned Black on and Page Down went live (measured on the unfixed sources: mask on, Live set, 2 renderer submissions). Fix: `SHOW`, `EXTEND` and `NAVIGATE` in `src/main.rs` add `!SelaSongMenu`, like `!SelaMenuBar`. Test `tests::scrutiny_r2::show_keys_stay_inert_while_the_songs_menu_is_open` (B, period, Ctrl+B, Ctrl+C, Page Down, V Enter, Right; after Esc, Ctrl+B works); native `scenario-s02-x11.py` step 2.
  - **SC2-LIVE-01** stale `live_slide` across a Live item change: `go_live_item` replaced `live` but kept the previous item's presented index, so after a failed Go Live of another item the footer read "Signal Hymn · Slide 1 of 3" (nothing of it shown), Next sent slide 2 of the new item and a surface refresh (`refresh_live_request`) sent its slide 1 unasked. Fix: a different item clears `live_slide`; `step_live` with nothing to step from says "{title} is not on screen yet; use Go Live first" (new `Msg::LiveItemNotShown`, EN+ID). Test `a_failed_go_live_of_another_item_never_borrows_the_old_slide_index` (fails with only the `live_slide = None` line reverted). New `Operator::live_footer` (the footer text, also used by `live_pane`).
  - **SC2-LIVE-02** Go Live on another entry of the same song revision (a duplicate) kept the earlier entry's cues in preparation (`go_live_item` compared versions only), so a slide the operator had moved away from reached the audience after the newer Go Live. Fix: item identity is (revision, schedule entry, chosen arrangement). Test `go_live_on_a_duplicate_entry_drops_the_previous_entrys_pending_cue` (gated preparer: 2 pending cues and 2 submissions before the fix, 1 after; fails with only the version comparison restored).
  - **SC2-LIVE-03** `focus_song` (a library pick) cleared `live_on_load` but not `live_entry_on_load`, so a Live-pane Page Down waiting for its preview survived a library pick and the next Page Down started from an entry that never went live, skipping one. Fix: `focus_song` clears it too. Test `picking_a_song_drops_a_waiting_page_down_target`.
  - **SC2-I18N-01** operator window title hard-coded in English ("Sela — Technical preview", "Untitled schedule* — Sela"). Fix: `Msg::WindowTitleIdle` and `Msg::WindowTitleUntitledDirty` (ID "Sela — Pratinjau teknis", "Jadwal tanpa judul* — Sela"), also for the initial title in `main.rs`. Test `the_window_title_follows_the_ui_language` (with the old `schedule_files.rs`: "Sela — Technical preview" in Indonesian).
  - **SC2-A11Y-01** no dialog was exposed to assistive technology: the operator dialogs (unsaved, remove, delete song, file error, preflight, Go Live missing, Update items), Options and the Song Editor close prompt had no role or name, so screen readers announced only the focused button. Fix: AccessKit `Role::Dialog` named with the question (`schedule-dialog`, `options-dialog`, `close-prompt`; a description for Delete and file errors, "Unsaved changes. Save, discard, or keep editing." on the editor prompt). GPUI exposes no accessibility tree to tests, so the regression test is native: `scenario-s01-x11.py` step 3 asserts AT-SPI role "dialog" and the name; against a binary built with only `.role(Role::Dialog)` reverted it FAILS ("control 'schedule-dialog' missing from the AT-SPI tree").
  - **SC2-A11Y-02** a window title change reached the accessibility tree only with the next unrelated frame: GPUI (zed a846890 `crates/gpui/src/window/a11y.rs` `begin_frame`, `crates/gpui/src/window.rs` `set_window_title`) names the tree's window node when a frame begins, and `sync_window_title` runs inside `render`. After Add to Schedule the AT-SPI window kept "Sela — Technical preview" until something else redrew. Fix: `window.request_animation_frame()` when the title changes. Native regression: `scenario-s02-x11.py` FAILED before the fix ("timed out after 10.0s waiting for the AT-SPI tree of 'Untitled schedule* — Sela'") and passes after it.
  - **SC2-HARNESS-01** (test harness) `native_x11.App.controls` returned role "" for every control: accesskit_unix 0.22 answers GetRoleName with an empty string. Fix: map GetRole's number to the AT-SPI role name (`_ATSPI_ROLES`, checked against atspi-common 0.13 `Role`).
- Unconfirmed suspicions (not fixed, not reproduced as a failure):
  - (Confirmed and fixed in review round 1 as SC2-FOCUS-02, below.) The Schedule item, image and New menus never took the focus, so show keys still acted while they were open.
  - A Live-pane Page Down waiting for its preview plus an explicit Go Live in between: the explicit Go Live sends the old Preview item and the older Page Down intent then fires as well (same family as the recorded `live_on_load` UNOBSERVED item under M1-10).
  - `Operator::song_rows` keeps one FocusHandle per Songs row index ever built (bounded by the library size, never pruned); its cost on a 20k library is unmeasured.
  - A `Request::Song` that fails for a pinned revision sets `library_error` and is resubmitted on every poll because Preview never matches the wanted revision (inferred from `poll_library`; storage keeps revisions, so not reproduced).
  - While the native Save/Open dialog is open the operator's show keys stay live (the Linux portal dialog is a separate process); EW unobserved.
  - Deselecting the last selected schedule entry by Ctrl+click leaves Preview on that entry's song (`toggle_entry` keeps `selected_song`); cosmetic.
- Decisions: (1) "Same Live item" means the same (revision, schedule entry, chosen arrangement); a repeated Go Live of the same item keeps its queued cues in order, as before. (2) After a failed Go Live of another item, Next/Previous send nothing rather than stepping from the failed request's slide (UNOBSERVED best guess, reference-observations "Scrutiny round 2"). (3) Dialogs use `Role::Dialog` (UIA Window, AT-SPI dialog), not AlertDialog, so the unsaved prompt has one role on both platforms. (4) S01 uses a slide Color Fill as its layout/asset stand-in because themes (M1-08) do not exist yet; the "same layout" check compares the AT-SPI rectangles of Preview and Live and the Schedule list's x and width across the restart (the list's height changes with its status line). (5) `.agents/setup` reads the channel from `rust-toolchain.toml`, passes `--no-self-update`/`--no-update` only when `rustup toolchain install --help` lists them, adds rustfmt, clippy and `x86_64-pc-windows-gnu`, runs apt without sudo as root, adds the E2E harness packages and `mingw-w64`, and installs ruff 0.17.1 with `pip --user` (falling back to `--break-system-packages` on PEP 668 systems).
- New: `src/tests/scrutiny_r2.rs` (5 GPUI tests, registered in `src/tests.rs`), `scripts/scenario-s01-x11.py`, `scripts/scenario-s02-x11.py`, `scripts/scenario-s01-windows.py`, `scripts/scenario-s02-windows.py`; i18n messages `LiveItemNotShown`, `WindowTitleIdle`, `WindowTitleUntitledDirty` (EN+ID; `Msg::ALL` 471). Docs: parity-pending "Stream: scrutiny-r2" (PP-SC2-01..07) and run-order rows 24-25 (renumbered from 23-24 at integration; row 23 is song-import-windows.py), reference-observations "Scrutiny round 2 — UNOBSERVED best guesses", backlog progress notes (M1-03, M1-10, M1-15, M1-16), e2e-linux.md (scenarios, role mapping, results), README setup paragraph. No schema change.
- Results (worktree `/home/user/sela/.claude/worktrees/wf_4eb25669-832-2`, Linux, debug build):
  - `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 396 passed + 4 ignored, bin 241 passed + 3 ignored, output_process 9 + 1 ignored, transport_process 4 + 1 ignored, examples 13, 9, 19 + 1, 1, 19; 711 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. `cargo test --locked -j 3 --bin sela scrutiny_r2`: 5 passed; on the sources before the fixes the first four failed (assertions quoted above) and the title test failed with the old `schedule_files.rs`.
  - Native, display from `python3 scripts/native_x11.py serve --display :111` (Xvfb 1920x1080x24 + Openbox): `python3 scripts/scenario-s01-x11.py --display :111` PASS (43 s; audience 624x351 window, orange verse 23340 samples, "On screen: S01 Hymn · Verse 1" then "· Chorus", saved item `[song id, 1]`, identical pane rectangles after the restart); `python3 scripts/scenario-s02-x11.py --display :111` PASS (39 s; B revision 1 -> 2 with A unchanged, the exact Verse 1 frame back after Previous, Black and Clear; "Under mask: Quiet Canticle · Verse 1" then "Under mask: Signal Hymn · Verse 1" while black). Same binary: `python3 scripts/live-output-x11.py --display :111` PASS (59 s; one run during a parallel `cargo test` timed out waiting 15 s for the audience and passed alone right after), `schedule-x11.py` PASS (36-37 s), `song-editor-x11.py` PASS (64 s). The first batch of S01/S02 failed in the scripts themselves (an editor window vanishing between two xwininfo calls; Alt+F4 sent while the editor still waited for its save reply); both were fixed in the scripts before the passing runs. Captures inspected: `01-fresh`, `06-unsaved-dialog`, `10-audience-verse`, `11-operator-live` (S01) and `02-songs-menu`, `08-b-under-mask` (S02) under `.amp/in/artifacts/scenario-s0{1,2}-x11/`.
  - `python3 -m ruff check scripts/scenario-s01-x11.py scripts/scenario-s02-x11.py scripts/scenario-s01-windows.py scripts/scenario-s02-windows.py scripts/native_x11.py` exit 0 (whole `scripts/`: only the pre-existing EXE001 in `audience-smoke-windows.py`).
  - `.agents/setup` run twice (exit 0, 4 s and 3 s; rustup 1.28.2 without `--no-update`, components and the Windows target "up to date", ruff 0.17.1); `bash -n .agents/setup` and `shellcheck .agents/setup` exit 0; `git diff --check` clean.
- Unexecuted / open: `scripts/scenario-s01-windows.py` and `scripts/scenario-s02-windows.py` NOT RUN (no Windows host; ruff-clean only); Narrator/UIA reading of the dialogs and the title (PP-SC2-05); every EW observation in PP-SC2-01..06; `--kind dual` runs of the scenarios (the xvfb audience is the development window); physical display, timing and older-hardware checks; the Indonesian visual pass of the new strings. Xvfb/lavapipe runs are not hardware, DPI or parity evidence.
- Next: the owner's Windows runs of the two scenario scripts and the PP-SC2 observations; S03/S04 after M1-12/M1-13.
- Review round 1 (2026-10-22, independent review of 1b60e99; fixes on `wave/scrutiny-r2`):
  - **SC2-I18N-02** (major) the Song Editor close prompt asked "Close without saving?" while Yes saves and closes and No discards (EW8-OBS-012 observed "Would you like to save changes before closing?"); SC2-A11Y-01 had made it the prompt's accessible name too. Fix: `Msg::CloseWithoutSaving` renamed `Msg::SaveBeforeClosing`, EN "Save changes before closing?", ID "Simpan perubahan sebelum menutup?", used through `song_library::CLOSE_PROMPT_QUESTION` for the heading and the name. Tests: `song_library::tests::the_close_prompt_asks_whether_to_save` (fails with the old strings); native `scenario-s01-x11.py` step 2 now closes the edited editor, asserts AT-SPI role `dialog` and that name, then Esc keeps the editor.
  - **SC2-FOCUS-02** (major) the Schedule item menu (and the image and New ▾ menus) opened without taking the focus or a key context, so with item 1 live a right-click on item 2 left Page Down, B, Ctrl+B, Up/Down, Home/End and Ctrl+Delete acting on the show and Esc did nothing. Fix (`src/operator/menus.rs` popup section, `layout.rs`, `schedule.rs`, `resources/media.rs`, `controls.rs`, bindings in `mod.rs`): while a popup is open the operator root's key context is `Sela SelaPopupOpen` instead of `Sela SelaShow`, so every `SHOW`/`EXTEND`/`NAVIGATE` binding is inert; the right-click menus take the focus on their first item (`window.prevent_default()` stops the row's own mouse-down focus), Up/Down (context `SelaPopupMenu`) and Tab move inside them; Esc (`MenuDismiss` in `SelaPopupOpen`) closes any of them and returns the focus to the row or New ▾; a press outside closes them (New ▾ gained `on_mouse_down_out`; a press on New ▾ itself closes it without the click reopening it, via the recorded press position); dialogs and the menu bar opened from a popup item return focus to the menu's origin (`focus_to_return`). New ▾ keeps its focus on the button so the existing Tab order test still holds. Tests: `tests::scrutiny_r2::show_keys_stay_inert_while_the_schedule_item_menu_is_open` and `show_keys_stay_inert_while_the_new_and_image_menus_are_open` (both fail on the reverted sources: "control ItemMenuDuplicate" focus, "pagedown acted under the New menu"); native `scenario-s02-x11.py` step 2b (B, Page Down, Ctrl+Delete under the item menu change neither the audience nor the schedule; Esc closes it; capture `02b-item-menu` inspected: menu open, Duplicate Item focused, Live unchanged). EW behavior unobserved (PP-SC2-01, reworded).
  - **SC2-LIVE-04** (minor) Normalize re-sent only `live_slide`, which SC2-LIVE-01 clears on an item change, so a Normalize click between another item's Go Live and its acknowledgment re-sent nothing and the slide landed with the old sizing (and with a step in flight it would have re-sent the older slide). Fix: re-send `requested_slide()`. Test `normalize_applies_to_a_go_live_still_in_preparation` (gated preparer: 2 cues pending after the toggle, the landed `size_cap` is Normalized; with `live_slide` restored it fails "the canticle's cue is prepared again with the new sizing"). PP-SC2-08.
  - Docs: backlog M1-16 note no longer claims S02 or the other dialogs are checked over AT-SPI (S01 asserts the unsaved prompt, Options and the close prompt; Delete song, file error, preflight, Go Live missing and Update items share the `schedule-dialog` node but are not asserted natively); PP-SC2-05 now quotes the real description ("Unsaved changes. Save, discard, or keep editing.") and the Delete note; new PP-SC2-08 (Normalize in flight) and PP-SC2-09 (close prompt wording); reference-observations rows SC2-FOCUS-02, SC2-LIVE-04, SC2-I18N-02.
  - Scripts: `scenario-s01-windows.py` now asserts `controlType` "Window" (AccessKit 0.34 maps `Role::Dialog` to `UIA_WindowControlTypeId`) with the name for the unsaved prompt, Options and the close prompt; `scenario-s02-windows.py` gained the item-menu step. Harness: `native_x11.viewable(wid)` (one xwininfo query; BadDrawable counts as gone) replaces `window_exists(w) and geometry(w).viewable` in `App.wait_exit` and the file-dialog close wait, and the scenario scripts' `closed()` use it (the reviewer's first `schedule-x11.py` run failed on that race in `wait_exit`).
  - Results (worktree `/home/user/wt/scrutiny-r2`, debug build): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 396 + 4 ignored, bin 245 + 3 ignored, output_process 9 + 1, transport_process 4 + 1, examples 13, 9, 19 + 1, 1, 19; 715 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `cargo test --locked -j 3 --bin sela -- scrutiny_r2 the_close_prompt_asks` 9 passed, and on the sources with the FOCUS-02 and wording fixes reverted 3 failed (close prompt, both popup tests), with only the Normalize fix reverted its test failed. Native on `python3 scripts/native_x11.py serve --display :111`: `scenario-s01-x11.py` PASS (49 s; options_dialog ["dialog", "Options"], close_prompt ["dialog", "Save changes before closing?"], capture `04b-editor-close-prompt` inspected), `scenario-s02-x11.py` PASS (44 s), `schedule-x11.py` PASS three times (34 s each), `song-editor-x11.py` PASS (64 s), `live-output-x11.py` PASS (58 s). A first S01 run failed in the script: after Esc the editor exposes no author ids, which `App.controls` reports as a timeout; the probe now treats that as "no prompt". `python3 -m ruff check scripts/` only the pre-existing EXE001 in `audience-smoke-windows.py`.
  - Unexecuted / open: the Windows scenario scripts (NOT RUN: no Windows host); EW observation of popup-menu keys (PP-SC2-01), the Normalize timing (PP-SC2-08) and the close prompt (PP-SC2-09); the image and New ▾ menus have GPUI coverage only (no native step); Narrator.
- Next: the owner's Windows runs of the two scenario scripts and the PP-SC2 observations; S03/S04 after M1-12/M1-13.
- Review round 2 (2026-10-22, independent review of ed4ba13; two minor findings, both confirmed and fixed on `wave/scrutiny-r2`):
  - **SC2-FOCUS-03** (minor) the New ▾ swallow-click flag outlived its press: `new_button_pressed` set it on any left press matching the recorded outside-close position, but only a completed click consumed it, so a press on New ▾ that closed the menu and was released elsewhere left it set and the next keyboard Enter on New ▾ did nothing; and `on_mouse_down_out` recorded the position for any button, so a right press that closed the menu made a later left click at the same pixel a no-op. Fix (`src/operator/menus.rs`, `controls.rs`, `layout.rs`): only the mouse click path consults the flag (`swallow_new_menu_click` in the button's `on_click`, for `Control::NewMenu`); `toggle_new_menu` (keyboard and click alike) clears it and the recorded position; `new_menu_pressed_outside` records the position for a left press only. Test `tests::scrutiny_r2::new_menu_swallow_never_outlives_its_click` (press, drag 300 px, release, then Enter must open; right press then left click must open; a completed click on New ▾ with the menu open still closes it); it fails on the unfixed sources ("Enter on New ▾ opens the menu") and with only the left-button filter reverted ("a left click after a right press opens the menu"). Native: `scenario-s02-x11.py` step 2c (B and Page Down inert under New ▾; `nx.drag` from New ▾ 300 px down closes it; the first Enter must open it; Esc closes it; audience and Live status unchanged) PASS, and FAILS on the binary built from the unfixed sources ("the first Enter on New ▾ after a press-and-drag was swallowed"); capture `02c-new-menu-enter` inspected (New ▾ focused, New Song / New Schedule menu open). `scenario-s02-windows.py` gained the same step (NOT RUN). UNOBSERVED in EW (PP-SC2-01 extended; reference-observations row SC2-FOCUS-03).
  - **SC2-LIVE-05** (minor) Normalize re-sent the newest request through `send()`: a new generation that cancels the double-click unmask intent, and no cancellation of the cue already in preparation with the old sizing, so B's Verse 1 reached the audience with per-slide sizing for one cue before the normalized one, and a slide double-clicked under Black landed with Black kept on. Fix (`src/operator/controls.rs`): Normalize calls `refresh_live_request()` (cancel and rebuild the newest request under its own generation, keeping the unmask intent), as surface and font refreshes do. Tests: `normalize_applies_to_a_go_live_still_in_preparation` now expects 1 pending cue with Normalized sizing after the toggle and exactly one renderer submission (the round-1 version codified 2); new `normalize_keeps_a_pending_double_click_unmask` (under Black, double-click Chorus with a gated preparer, Normalize, open the gate: the mask lifts over the Chorus and the landed `size_cap` is Normalized). Both fail with `send()` restored ("the old-sizing cue is cancelled"; 2 pending cues). PP-SC2-08 extended; reference-observations row SC2-LIVE-05 (UNOBSERVED).
  - Results (worktree `/home/user/wt/scrutiny-r2`, debug build): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 396 + 4 ignored, bin 247 + 3 ignored, output_process 9 + 1, transport_process 4 + 1, examples 13, 0, 9, 19 + 1, 1, 0, 0, 19; 717 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `cargo test --locked -j 3 --bin sela -- scrutiny_r2` 10 passed, 3 failed with `src/operator/` reverted to ed4ba13. Native on `python3 scripts/native_x11.py serve --display :111`: `scenario-s01-x11.py` PASS (48 s), `scenario-s02-x11.py` PASS (52 s; also 52 s before the final fmt-only change), FAIL on the ed4ba13 operator sources as quoted above. `python3 -m ruff check scripts/scenario-s01-x11.py scripts/scenario-s02-x11.py scripts/scenario-s01-windows.py scripts/scenario-s02-windows.py scripts/native_x11.py` all passed (whole `scripts/`: only the pre-existing EXE001 in `audience-smoke-windows.py`).
  - Unexecuted / open: the Windows scenario scripts (NOT RUN: no Windows host); EW observation of New ▾ press/drag/Enter (PP-SC2-01) and of Normalize during an in-flight cue and a pending double-click unmask (PP-SC2-08); Narrator; physical display.
- Next: the owner's Windows runs of the two scenario scripts and the PP-SC2 observations; S03/S04 after M1-12/M1-13.
- Integration (2026-10-22): merged into `ccr-5e192d6c-emgkai` as merge commit `2f95c14` ("Merge branch 'wave/scrutiny-r2'", `--no-ff`, on top of the import-framework merge `4431e12`/`2868239`). Conflicts, all resolved keeping both sides: `docs/e2e-linux.md` (script list), `docs/parity-pending.md` (Windows run order: `song-import-windows.py` keeps row 23, `scenario-s01-windows.py`/`scenario-s02-windows.py` renumbered to rows 24/25), `docs/reference-observations.md` (both UNOBSERVED tables), `docs/work-log.md` (both Current-state bullets; status rows M1-07/M1-14/M4-07 from import-framework, M1-09/M1-10/M1-15/M1-16 from scrutiny-r2; both records), `src/i18n.rs` (all 57 import and 3 scrutiny-r2 `Msg` variants in the enum, `ALL` and both tables; count guard 528), `src/tests.rs` (`scrutiny_r2` and `song_import` modules). No schema, `.selaschedule` or settings version involved; no Cargo change. Integration fix `b1ae7cd`: `scripts/song-import-x11.py` waited for the portal chooser to close with `window_exists()` then `geometry()`, which raced (BadDrawable, failed 1 of 3 runs); it now uses scrutiny-r2's single-query `nx.viewable()`. Gate on the merged tree in `/home/user/sela`: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 414 passed + 4 ignored, bin 263 passed + 3 ignored, output_process 9 + 1 ignored, transport_process 4 + 1 ignored, examples all pass); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Native, serially on `:199` (`python3 scripts/native_x11.py serve --display :199`, Xvfb 1920x1080x24 + Openbox): `live-output-x11.py` PASS (59 s), `scenario-s01-x11.py` PASS (48 s), `scenario-s02-x11.py` PASS (52 s), `schedule-x11.py` PASS (35 s), `song-editor-x11.py` PASS (65 s), `song-import-x11.py` FAIL once on the race above, then PASS 3 times (once before, twice after `b1ae7cd`; ruff check/format clean). Windows scripts NOT RUN (no Windows host).

### autosave-recovery — M1-12 autosave, revision recovery and local profiles, S03 (2026-10-22)
- State: M1-12 **implemented-unqualified** (Windows run, EasyWorship observation, real disk/ACL faults, power loss and the older-hardware hitch measurement remain). Worked in worktree branch `wave/autosave-recovery` from `ccr-5e192d6c-emgkai` (Linux cloud container, 4 CPUs, no Windows host). No SQLite schema change. Design note: docs/autosave.md.
- Decisions: (1) Revisions are files in `<profile>/Recovery/`, not SQLite rows: one JSON line plus a `sha256` trailer, published by exclusive temp file, fsync, rename and (Unix) folder fsync; the storage worker's single request slot is never used by autosave, because revisions pin committed library song revisions (never garbage-collected) instead of embedding songs. (2) `Recovery/session.json` holds `running`/`clean`, `acknowledged` and the uncommitted marker written at the first change after a commit, so a crash discloses lost newer work; a marker for exactly the committed schedule is dropped (fingerprint check). (3) Cadence (`autosave::Policy`, framework-free, injected `Instant`): commit after 2 s quiet or 10 s after the first uncommitted change, one in flight, retry after 30 s, nothing written while the recovery prompt is open; the operator drives it with the GPUI executor clock (`BackgroundExecutor::now`), wall time for "minutes ago" comes from an injected `WallClock`. (4) Retention: newest 20 revisions within 4 MiB, newest never removed, oversize revision refused (`Budget`), kept across sessions and after Don't Restore. (5) Recovery prompt only when the unacknowledged newest valid revision is unsaved, lost work is recorded, or newer revisions are damaged; Restore (default focus, best guess) restores entries plus original file and baseline, never touches Live, and commits the restored schedule before acknowledging the old run; Don't Restore/Esc acknowledges; an unanswered prompt survives another crash and a close. (6) Every close path passes the unsaved guard and settings wait, then the bounded (5 s) autosave flush that commits the final schedule and marks the run clean; Don't Save is therefore never offered later. (7) Single instance: `std::fs::File::try_lock` on `<profile>/sela.lock` in `main` before the window; a second operator shows "Sela is already running" (EN/ID, AccessKit ids `already-running-message`/`-ok`) and exits without touching the profile; a lock that cannot be created (read-only) does not block start, autosave reports why it is off. The `--library` editor mode takes no lock. (8) Role::Label nodes are named by their value in AccessKit (`label_comes_from_value`), so the prompt body and notice set `aria_value` too (the existing `schedule-file-error` label has an empty UIA/AT-SPI name for the same reason; not changed here). (9) Fingerprint hashes title lengths, not titles: measured 149 µs → 8.8 µs per 16 ms poll for 32 entries with 1 KiB titles in the debug build (ad-hoc loop); snapshot 4.8 µs per commit. (10) EW behaviour unobserved: EW8-OBS-071..075 UNOBSERVED best guesses (may need renumbering at merge), PP-AUTO-01..08 in parity-pending.md.
- Code: `src/autosave.rs` + `src/autosave/tests.rs`, `src/profile.rs`, `src/operator/autosave.rs`, `src/instance.rs`, `src/main.rs`, `src/schedule.rs` (`restore`, `baseline`), small hooks in `src/operator/{mod,polling,dialogs/mod,controls,schedule,options,assets}.rs`, `src/i18n.rs` (30 EN+ID messages, 498 total), `src/tests/autosave_recovery.rs`, `scripts/autosave-x11.py`, `scripts/autosave-windows.py`.
- Upstream GPUI studied (zed a846890): `crates/gpui/src/executor.rs` (`now`, `timer`, `advance_clock`), `crates/gpui/src/platform/test/platform.rs` (`quit` is inert in tests, hence the injectable dismiss), `crates/gpui/src/elements/div.rs` (`aria_label`, `aria_value`), `crates/gpui/src/global.rs`; `accesskit_consumer` 0.38 `node.rs` `label_comes_from_value`, `accesskit_atspi_common` 0.19.1 `node.rs` `name`.
- Tests: lib `autosave::tests` (32 incl. `s03::{killed_before_the_commit_recovers_the_last_revision_and_discloses_the_edit, killed_during_the_commit_never_recovers_a_partial_schedule, killed_after_the_commit_recovers_the_newer_revision_without_loss}` which SIGKILL a child test process) and `profile::tests` (4, incl. a killed child releasing the lock). GPUI: `tests::autosave_recovery::{an_edit_is_marked_at_once_and_committed_only_after_the_quiet_period, after_a_crash_restore_brings_back_the_last_commit_and_discloses_lost_work, a_saved_schedule_restores_with_its_file_and_only_the_unsaved_part_dirty, esc_declines_keeps_the_copies_and_the_next_start_asks_nothing, closing_with_the_prompt_unanswered_offers_it_again, a_corrupt_newest_revision_restores_the_previous_one_and_says_so, lost_work_without_any_copy_is_disclosed_with_ok_only, quit_with_dont_save_flushes_the_last_state_and_marks_the_run_clean, a_full_disk_is_reported_and_retried_until_space_returns, a_read_only_profile_turns_autosave_off_with_the_reason, autosave_never_holds_up_live_controls, without_a_setup_or_profile_there_is_no_autosave}`, `instance::tests::the_notice_names_the_profile_and_quits_on_enter_esc_or_ok`.
- Fails without the change (checked by temporarily reverting, then restoring): writing the revision straight to its final name fails `s03::killed_during_the_commit_never_recovers_a_partial_schedule`; making `mark_uncommitted` a no-op fails 5 lib tests (`newer_uncommitted_work_is_disclosed_as_lost`, `s03` before/during, `an_interrupted_commit_is_removed_and_disclosed`, `a_read_only_or_locked_profile_reports_why`); skipping the close flush fails `quit_with_dont_save_flushes_the_last_state_and_marks_the_run_clean`; dropping the unanswered-prompt rule fails `closing_with_the_prompt_unanswered_offers_it_again`.
- Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 432 passed + 4 ignored, bin 249 + 3 ignored, other targets 74; 755 passed in total against the 706 baseline); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `cargo build --locked -j 3 --bin sela --example seed_library` exit 0 (all with `CARGO_INCREMENTAL=0` because of the shared disk); `python3 -m ruff check` and `python3 -m ruff format --check` on both new scripts clean (`ruff check scripts` still reports only the pre-existing EXE001 on `audience-smoke-windows.py`).
- Native X11: `python3 scripts/native_x11.py serve --display :113` (Xvfb :113 1920x1080x24 -nolisten tcp + Openbox), then `python3 scripts/autosave-x11.py --display :113`: PASS (19 s, final binary sha256 1daceb2d935f…; an earlier run on the previous wip binary also PASSED and the first-ever run FAILED only because a Role::Label had an empty AT-SPI name, fixed by decision 8). Captures inspected (`recovery-prompt.png`: modal with the five disclosure lines, Restore focused; `restored.png`: one item, "Unsaved", "Restored the autosaved schedule (1 item(s))", "Unsaved changes are autosaved"; `second-instance.png`: the notice). Artifacts `.amp/in/artifacts/autosave-x11/`. Recorded: first revision 1 item; kill 16 ms after the edit with marker `{after 1, items 2}`; audience survived the operator SIGKILL; recovery text as expected; exits 0; revisions kept 2.
- Unexecuted / open: `scripts/autosave-windows.py` NOT RUN (no Windows host; lint `python3 -m ruff check` and `ruff format --check` clean); EW observation PP-AUTO-01..05; real disk-full volume, ACL and sharing-lock checks (PP-AUTO-07); power loss and the 90-minute older-hardware hitch measurement (PP-AUTO-08, P/H); Indonesian visual pass of the prompt and notice; Narrator/Orca names. Disk space: the shared container disk filled once during the build (os error 28); freed by deleting this worktree's `target/debug/incremental/sela-*`, later builds used `CARGO_INCREMENTAL=0`.
- Next: run `autosave-windows.py` on Windows; observe EW 8.0.49 after a forced termination and on a second launch (PP-AUTO-02/03) and adjust the prompt/notice; an in-app browser of Recovery copies with M1-13.
- Review round 1 (2026-10-22, fixer): an independent review found 3 major and 4 minor issues; all fixed. (1) Closing while the worker was still Starting queued `Job::Close`, which ran after `Opened(offer)` and acknowledged the previous run's offer unseen. `Store` now tracks its unresolved start-up offer (`offer_open`); `close` with it open commits and acknowledges nothing and records `state: "offered"` (new session state), the UI no longer special-cases an open offer (it queues the same Close, which is safe now, and treats the run as flushed once `Closed` arrives with the offer present), and `resolve` clears it so a later close is a normal clean one. (2) An unwritable `session.json` at start (disk full, read-only, locked) failed `Store::open` as a whole and dropped a readable offer: `Store::open_keeping_offer` returns `OpenFailure { error, offer }`, the worker sends it in `Reply::Opened(Err(..))`, the prompt still appears with a new line "Autosave is off (…), so this answer cannot be recorded …", Restore works read-only and nothing is acknowledged. (3) Local profile management was implied done: M1-12 gains an unchecked item and a note, new ticket **M1-12b** (planned) owns Profiles > Default / Profiles Manager… create/switch/rename, EW8-OBS-077 / PP-AUTO-10 record the observation. (4) An edit made while a commit ran had no marker until the commit's reply: `Policy::step` now emits `Mark` at once (queued behind the commit, so `after` names it; `flight_marked` avoids a second marker). (5) False loss disclosure: `Policy::step` takes `unsaved` (`Schedule::is_dirty`); only unsaved work is marked, and a new `Step::Unmark`/`Job::Unmark`/`Store::clear_uncommitted` withdraws the marker when the schedule is back at the commit or equals its file (never while a commit runs). (6) Restore now queues `storage::Command::Songs` as `Request::RecoveryCheck(document)` and runs `start_schedule_check`, so a restored schedule gets the M1-11b missing-file report. (7) A close with the prompt unanswered no longer leads to "did not close normally" next time: `Offer::unanswered` selects "Sela was closed last time before this recovery question was answered." (EN+ID; EW8-OBS-076 / PP-AUTO-09 UNOBSERVED). i18n: 2 new messages (500 total). Scripts: `autosave-x11.py` and `autosave-windows.py` gain a step that closes the window with the prompt unanswered (checks `offered`, exit 0, nothing committed) and then expect the new first line.
  - New/changed tests: lib `closing_before_the_offer_is_answered_keeps_it_for_the_next_start`, `an_unwritable_session_after_a_crash_still_returns_the_offer`, `a_withdrawn_marker_discloses_no_loss`, `an_edit_made_while_a_commit_runs_is_marked_without_waiting_for_it`, `only_unsaved_work_is_marked_and_a_marker_is_withdrawn_when_untrue` (37 autosave tests); `the_policy_marks_at_once_and_commits_after_a_quiet_period` and `reverting_to_the_committed_schedule_needs_no_commit` now expect the in-flight `Mark` and the `Unmark` (intended behaviour change). GPUI: new `closing_while_the_start_up_scan_runs_keeps_the_previous_offer` (scan held at a gate, `may_close` during Starting), `a_prompt_closed_unanswered_says_so_the_next_time`, `an_unwritable_profile_after_a_crash_still_offers_a_read_only_restore`, `a_restored_schedule_gets_the_open_time_missing_file_check`; `closing_with_the_prompt_unanswered_offers_it_again` now drives the real close and expects `offered`; `after_a_crash_restore_…` waits for the acknowledging session write as well as the revision (it read `session.json` between the two writes once). Fails without the change (temporarily reverted, then restored): disabling the `offer_open` close branch, dropping the offer from `OpenFailure` and skipping the restore check fail 5 GPUI tests (`closing_while_the_start_up_scan_runs_keeps_the_previous_offer`, `closing_with_the_prompt_unanswered_offers_it_again`, `a_prompt_closed_unanswered_says_so_the_next_time`, `an_unwritable_profile_after_a_crash_still_offers_a_read_only_restore`, `a_restored_schedule_gets_the_open_time_missing_file_check`); the policy/store tests do not compile against the old API and assert the new steps.
  - Results (HEAD before commit = 693e31d + this change): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 437 passed + 4 ignored, bin 253 + 3 ignored, output_process 9 + 1, transport_process 4 + 1, composition_spike 13, input_check 9, native_cues 19 + 1, output_spike 1, video_spike 19: 764 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `cargo build --locked -j 3 --bin sela --example seed_library` exit 0; `ruff check` / `ruff format --check` on `scripts/autosave-x11.py` and `scripts/autosave-windows.py` clean; `autosave-windows.py` parses (ast).
  - Native X11: `python3 scripts/native_x11.py serve --display :113`, then `python3 scripts/autosave-x11.py --display :113 --out <scratch>/autosave-x11`: PASS (20.5 s, binary sha256 171de1b15aca…). Recorded: kill 17 ms after the edit; first prompt "Sela did not close normally last time. …"; Alt+F4 with it unanswered → exit 0, `session.json` `{state "offered", acknowledged 0, uncommitted {after 1, items 2}}`, one revision; next prompt "Sela was closed last time before this recovery question was answered. …" with all disclosure lines; Restore, clean quit and clean start exit 0; 2 revisions kept. Captures inspected: `recovery-prompt-first.png`, `recovery-prompt.png` (new first line, Restore focused), `restored.png` (1 item, "Unsaved", "Unsaved changes are autosaved").
  - Unexecuted / open: unchanged from above (`autosave-windows.py` NOT RUN, no Windows host; PP-AUTO-01..05, -07..-10 EW/Windows observations; power loss and hitch). New: PP-AUTO-07 (d) read-only restore on a real locked session file; M1-12b planned.
  - Next: run `autosave-windows.py` on Windows; observe EW (PP-AUTO-02/03/09/10); start M1-12b after the Profiles Manager observation.
- Review round 2 (2026-10-23, fixer): two minor findings, both fixed. (1) Saving withdraws the uncommitted marker at once, but the saved schedule is committed only after the 2 s quiet period; a crash in that gap offered the older unsaved revision saying "Those changes were never saved to the schedule file", and restoring then saving would overwrite the newer file. Fix: `Store::open` reads the offered revision's own file modification time (new `Fs::modified`, one metadata call on the worker) and sets `Offer::file_saved` when it is newer than the revision's `saved_at`; the prompt then shows "Its schedule file was saved again {ago}, after this autosave, so the file may hold newer work: restoring brings back this older copy, and saving it replaces the file." (new `Msg::RecoveryFileNewer`, EN+ID, 501 messages) instead of the "never saved" line. Chosen over keeping the marker because it also covers a Save with no edit after the commit and is false for a different file opened later; EW behaviour UNOBSERVED (EW8-OBS-078, PP-AUTO-11). (2) A close requested while the start-up scan ran, followed by `Reply::Opened(Err)` (worker dropped), was never answered and waited the full 5 s `CLOSE_WAIT` with "Finishing the autosave…", and the read-only recovery prompt popped up meanwhile. Fix: the close loop stops waiting once `worker` is `None`, and `poll_autosave` holds the prompt back while a close is in progress (a cancelled close shows it on the next poll).
  - New tests: lib `autosave::tests::a_file_saved_after_the_offered_revision_is_disclosed` (no file / older mtime / newer mtime; lost stays `None`); GPUI `tests::autosave_recovery::a_file_saved_after_the_last_autosave_is_disclosed_instead_of_never_saved` (exact prompt lines) and `a_close_waiting_for_a_start_up_open_that_fails_closes_at_once` (session write fails, scan gated, `may_close` during Starting; asserts no Recovery dialog while closing, closed within 1 s of GPUI clock, session file untouched). Fails without the change (temporarily reverted, then restored): reverting `src/operator/autosave.rs` fails both GPUI tests (the prompt line, and the dialog appearing in the closing window); reverting only the loop condition fails the close test with "closed without waiting out the close timeout (5.1s)"; the lib test does not compile without `Offer::file_saved`. `i18n::tests::all_lists_every_variant_exactly_once` count guard 500 → 501 for the new message.
  - Results (HEAD before commit = ec85421 + this change): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 438 passed + 4 ignored, bin 255 + 3 ignored, output_process 9 + 1, transport_process 4 + 1, composition_spike 13, input_check 9, native_cues 19 + 1, output_spike 1, video_spike 19: 767 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0.
  - Native X11: `python3 scripts/native_x11.py serve --display :113`, then `python3 scripts/autosave-x11.py --display :113 --build --out <scratch>/r2/x11`: PASS (00:09:54 to 00:10:16, 22 s; binary sha256 03c1883f83b7…, working tree on ec85421). Kill 16 ms after the edit, marker `{after 1, items 2}`; unanswered close exit 0 with `{state offered, acknowledged 0}`; second prompt starts "Sela was closed last time before this recovery question was answered." with all disclosure lines; recovered and clean exits 0; 2 revisions kept. `recovery-prompt.png` inspected (modal, Restore highlighted). The script has an untitled schedule, so it does not exercise the new file-newer line (covered by the GPUI test); a native Save-then-kill step is not added (the file dialog would need driving).
  - Unexecuted / open: `autosave-windows.py` NOT RUN (no Windows host; unchanged by this round); PP-AUTO-11 (EW crash between a Save and the next autosave) added; earlier open items unchanged. The mtime comparison assumes the revision clock and the file system clock agree (same machine; a network share with a skewed server clock could hide or falsely show the line).
  - Next: run `autosave-windows.py` on Windows; observe EW (PP-AUTO-02/03/09/10/11); start M1-12b after the Profiles Manager observation.
- Integration (2026-10-23): merged into `ccr-5e192d6c-emgkai` as merge commit `7e679dd` ("Merge branch 'wave/autosave-recovery'", `--no-ff`, on top of the scrutiny-r2 integration `8552a38`). Conflicts, all resolved keeping both sides: `docs/e2e-linux.md` (script list; run table keeps the song-import row and notes, autosave row added), `docs/parity-pending.md` (Windows run order: `song-import-windows.py`/`scenario-s0{1,2}-windows.py` keep rows 23-25, `autosave-windows.py` renumbered to row 26), `docs/reference-observations.md` (import and scrutiny-r2 tables kept, autosave EW8-OBS-071..078 appended; no ID collision, the base ends at 070), `docs/work-log.md` (all Current-state bullets; status rows M1-12/M1-12b added before M1-14/M1-15/M4-07, M1-16 kept from the base as the superset), `src/i18n.rs` (57 import + 3 scrutiny-r2 + 33 autosave `Msg` variants in the enum, `ALL` and both tables; count guard 561), `src/operator/{controls,dialogs/mod,mod,polling,options}.rs` (both `Dialog::ImportSongs` and `Dialog::Recovery`, both `SongImport`/`RecoveryCheck` job kinds, `poll_song_import` and `poll_autosave`; the settings close waiter keeps the import guard and then hands Don't Save to `autosave_allow_close`). No SQLite schema, `.selaschedule` or settings version involved (autosave revisions are files with their own format); no Cargo change. Integration fix `153a710`: the autosave close waiter (`src/operator/autosave.rs`) took the Don't Save shortcut without the PP-IMPF-09 import guard, closing the window over an import preview opened during the flush; GPUI `an_import_preview_opened_while_the_close_flush_waits_keeps_the_window` fails without the guard and passes with it. Gate on the final tree in `/home/user/sela`: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 456 passed + 4 ignored, bin 283 + 3 ignored, output_process 9 + 1, transport_process 4 + 1, examples 13, 9, 19 + 1, 1, 19); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Native, serially on `:199` (`python3 scripts/native_x11.py serve --display :199`), each run on the merge and again after the fix: `autosave-x11.py` PASS (21 s / 22 s), `live-output-x11.py` PASS (59 s / 59 s), `scenario-s01-x11.py` PASS (50 s / 49 s), `scenario-s02-x11.py` PASS (56 s / 51 s), `schedule-x11.py` PASS (35 s / 35 s), `song-editor-x11.py` PASS (64 s / 63 s), `song-import-x11.py` PASS (33 s / 32 s). Windows scripts NOT RUN (no Windows host).

### songs-browser — Songs Sort by keys, slide-label search, copyright options (2026-10-22)

- State: M1-04 Songs remainder, M1-07 label search (backlog anchor `m1-07-search-label-matching`) and the M1-05 copyright display follow-up are **implemented-unqualified**: Linux headless and Xvfb evidence only; the Windows script was not run (no Windows host) and the EasyWorship rules below are best guesses until observed (parity-pending "Stream: songs-browser", PP-SB-01..10). Schema version 7.
- Decisions: (1) Sort by Author/Copyright/Administrator/Song Number (EW8-OBS-064) read derived keys stored in `search_rows` (schema 7), so the Songs list still loads without decoding payloads; keys are whitespace-collapsed, ASCII case-insensitive, Song Number numeric, empty keys last in both directions, ties by Title ascending then id (UNOBSERVED best guesses). The menu lists EW's five keys in EW order, then Sela's Date modified; codes `author|copyright|administrator|song_number` persist in `settings.txt` like the existing sort. Search results keep relevance order regardless of Sort by. Rows still show titles only. (2) Labels (EW8-OBS-045/046): a fourth FTS5 column `labels` holds each distinct section label once; bm25 weights title 10, lyrics 1, metadata 2, labels 0.5. (Corrected in review round 1, 2026-10-23: the weights alone did not keep a label-only hit below a lyric hit in a much longer song, and the extra column changes every row's length, so "ranks exactly as before" was not true either; see that round.) FTS5 cannot add a column, so schema 5/6 files are migrated by rebuilding the derived index inside the migration transaction behind the existing verified `.schema{N}-backup` gate (abort hook `SELA_ABORT_MIGRATION_SCHEMA7`); `songs.modified` is untouched; `check_search` also compares labels and sort keys. (3) Copyright Info (EW8-OBS-055/056) lives in a new Options › Main Output › Song tab (`src/operator/options/song.rs`; General and Song tabs are now clickable/focusable `Role::Tab`s): Display Copyright Information (on), Show on first displayed slide only (off), License Info (one line, 256 bytes), stored as `copyright_display=0`, `copyright_first_slide=1`, `license_info=` only when not default. Best guesses: "first displayed slide" = slide 1 of the item in presentation order; License Info adds a third strip line "CCLI Song # N · License Info"; settings apply when each Live cue is built (`CueJob.footer_shown`), so OK never redraws the live slide; the normalized size cap always reserves the strip so all slides of an item keep one size; the cap cache now keys on the strip text. Copyright Font, Song Font and Show Verse/Chorus Label are not built (new M1-05t). (4) The backlog test item "slow earlier query / delete selected / keyboard through virtual rows" is covered with GPUI tests on the real worker plus a test-only `search_results` entry and a stale-reply counter in `SongSearch` (review round 1: the item is unticked again, no query was actually slow). (5) Test probe `Operator::presented_footers` (`#[cfg(test)]`) records the strip each presented cue carries. (6) No new crate; no GPUI pattern beyond existing ones (tabs reuse the language-row focus pattern from options.rs; `uniform_list` unchanged), so no new upstream study was needed.
- New/changed: `src/storage.rs` (schema 7, `sort_keys`, `library` orders, 6 new tests), `src/song_order.rs`, `src/slides.rs` (`CopyrightOptions`, `CopyrightStrip`, 2–3 line strips), `src/preparation/cue.rs`, `src/settings.rs`, `src/operator/options.rs` + `options/song.rs`, `menus.rs`, `preview_live.rs`, `polling.rs`, `resources/song_search.rs`, `src/i18n.rs` (11 EN/ID strings), `src/tests/songs_browser.rs`, `examples/seed_library.rs` (`--sort-songs`), `examples/search_bench.rs` (label queries, per-order library timings), `scripts/songs-browser-x11.py`, `scripts/songs-browser-windows.py`, docs (search, storage, reference-observations best-guess section and deviation updates, parity-pending, backlog, e2e-linux).
- GPUI tests (`src/tests/songs_browser.rs`): `sort_by_each_observed_key_reorders_the_list_and_persists`, `a_slide_label_query_finds_the_song_below_title_matches`, `a_late_reply_for_an_earlier_query_never_replaces_newer_results`, `deleting_the_selected_search_hit_selects_its_neighbour_hit`, `arrow_keys_walk_past_the_viewport_in_a_sorted_virtual_list`, `copyright_options_pick_the_slides_and_the_licensing_line` (fake audience child; asserts the strip text on each presented cue), `options_song_tab_is_reachable_by_keyboard_and_escape_discards`. Unit/integration: storage `library_sorts_by_author_copyright_administrator_and_song_number`, `section_labels_are_searchable_and_rank_below_title_and_lyric_matches`, `stale_labels_or_sort_keys_fail_the_check_and_are_repaired`, `schema6_migration_indexes_labels_and_sort_keys_and_keeps_the_stamps`, `schema6_migration_abort_keeps_schema6_and_the_verified_backup`; slides `copyright_options_pick_the_slides_and_add_a_licensing_line`, `a_three_line_strip_is_drawn_taller_than_two_lines`; cue `a_hidden_strip_still_shapes_the_normalized_size_and_the_cap_follows_the_strip`; settings `copyright_options_round_trip_and_defaults_are_not_written`; song_order menu-order test.
- Fails without the change (checked once): with the label column filled with `''`, Author sorting by title and `footer_shown` = Display only, `cargo test --lib -- storage::tests::section_labels storage::tests::library_sorts storage::tests::stale_labels` failed 3/3 and `cargo test --bin sela songs_browser` failed the sort, label and copyright tests (3/7); the files were restored and both pass.
- Results: `cargo fmt --all -- --check` exit 0 (after formatting the example); `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (723 passed, 10 ignored; baseline 706); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 -m ruff check scripts/songs-browser-x11.py scripts/songs-browser-windows.py` clean. Native: `python3 scripts/native_x11.py serve --display :114` then `python3 scripts/songs-browser-x11.py --display :114` PASS twice (final run 28 s, debug binary of this tree): Title order; mouse Sort by Author/Copyright/Administrator; keyboard Shift+F10 › Sort by › Song Number and Descending; `settings.txt` holds `song_sort=song_number` + `song_sort_descending=1`; "etsa" lists Etsaer Praise then Echo Hymn (label-only hit); Options › Song first-slide-only writes `copyright_first_slide=1`; restart keeps the order. Captures inspected: `options-song-tab.png` (Song tab, both checkboxes, License Info field, unbuilt tabs greyed), `operator-label-search.png` (two hits, Preview of the label hit shows "2 · Etsaer"). Artifacts `.amp/in/artifacts/songs-browser-x11/`.
- Benchmark (this VM only, not a qualification; details and table in docs/search.md "Schema 7 re-run"): shared 4-vCPU Xeon 2.10 GHz, dev profile with opt-level 3 for libsqlite3-sys/rusqlite/sela (no release build: disk budget); base 8ba013e vs schema 7, same corpus. Two-letter prefix p95 45.0 → 40.4 ms, most common word p95 39.6 → 49.3 ms, pooled p95 72.3 → 71.3 ms (title/lyric queries within noise); label queries now match all 20k songs (p95 42.8 / 46.9 ms); DB 62.0 → 64.0 MiB; rebuild 1.27 → 1.44 s; open 487 → 552 ms; whole-list sorts 20k: Title p50 73 ms, key sorts ~100 ms p50 (storage worker).
- Unexecuted / open: `scripts/songs-browser-windows.py` NOT RUN (no Windows host; ruff clean); no EasyWorship observation (PP-SB-01..08); the audience strip itself is not checked natively on Linux (the xvfb audience window is 351 px, below the strip's 360 px minimum; GPUI tests check the cue's strip text); no release-build or reference-laptop timing (PP-SB-10); a 20k native scroll run was not repeated; Narrator.
- Next: run `songs-browser-windows.py` and PP-SB-01..10 on Windows/EW 8.0.49; M1-05t (Song tab fonts and Show Verse/Chorus Label); M1-04 Media and other resource tabs, grid sizing and collections.
- Review round 1 (2026-10-23, fixer; reviewer findings on 62fb992):
  - Fixed (major): label-only hits could outrank lyric hits. FTS5's bm25 normalizes by the whole row's length, so with labels at 0.5 a short song's label hit beat a 400-word song's lyric hit (reviewer: -2.70 vs -0.90). `SEARCH_SQL` now sorts first by `bm25(song_search,10,1,2,0)=0` (true only when every hit is in the labels; idf is clamped positive, so any other hit scores below 0), then by the old bm25 and tie keys. Regression test `a_label_only_hit_ranks_below_a_lyric_hit_in_a_much_longer_song` (the reviewer's fixture; fails with the old order). Docs made true: storage.rs comment, docs/search.md Ranking (also drops "ranks exactly as before schema 7", false because the labels add to every row's length), reference-observations best guess, PP-SB-04 (now also asks EW for a long lyric song vs a label-only song), backlog M1-07 note.
  - Fixed (minor): a schema 5/6 library whose only damage is the FTS index was refused as `Corrupt` once CURRENT became 7. `open_internal` now drops and rechecks the index for every version >= 5; an upgrade continues (backup of the file as it was, schema 7 step rebuilds the index), and the backup verification open (no upgrade) rolls the drop back inside a savepoint and skips the `song_search` shape check. Schema 1-4 and damage outside the index stay `Corrupt` with no backup. Regression test `schema5_and_6_libraries_with_only_a_damaged_index_still_migrate` (fails at the base: `Err(Corrupt)`); docs/storage.md documents the path.
  - Fixed (minor): the vacuous "typing never sends anything Live" checks. `scripts/songs-browser-x11.py` now turns Live output on, sends the first listed song Live, types "etsa" and "bravo." (plain Black keys B and ".") in the search box and "CCLI b." in License Info with real xdotool keys, and requires the audience frame digest to stay unchanged through the typing and through Options OK with first-slide-only and License Info changed (`license_info=CCLI b.` saved). Mutation check: with the "b" binding moved to the `Sela` context the script FAILS (`search 'bravo.': rows []`). New GPUI test `typing_in_license_info_never_toggles_masks_or_cues_live` sends real keystrokes (`b c l . shift-b pagedown right`) to the License Info field with a slide Live: text "bcl.B", no mask, no new renderer cue, then OK saves it; it fails with the same mutation.
  - Fixed (minor): `scripts/songs-browser-windows.py` now makes a real Options change while slide 2 is Live (untick first-slide-only, type "CCLI b." into License Info, OK; checks settings.txt and zero frame changes), and its docstring says Down x4 for Song Number (Down x7 Descending). Still NOT RUN (no Windows host); ruff clean.
  - Fixed (minor, docs): the backlog item "slow earlier query ... large libraries ..." is unticked; its note now says the late reply comes from poll timing on a 4-song library plus a synthetic generation-1 `Results`, that no real slow query on a large library was raced, and that large-library evidence is earlier streams' (only the benchmark was repeated here).
  - Benchmark of the ranking change (this VM only; same opt-level-3 dev build and corpus as above; A/B old SQL vs new SQL x2, table in docs/search.md): label query `verse` p50/p95 35.2/43.3 -> 37.9/45.0 and 39.4/49.7 ms; pooled p95 70.3 -> 73.6 and 74.5 ms; all under the 100 ms ceiling.
  - Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0, no warnings; `cargo test --locked --all-targets -j 3` exit 0 (726 passed, 10 ignored: 408+244+9+4+13+9+19+1+19; 3 new tests); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 -m ruff check` and `ruff format --check` on both songs-browser scripts clean; `python3 scripts/native_x11.py serve --display :114` then `python3 scripts/songs-browser-x11.py --display :114 --build` PASS (42 s including the build; captures inspected: `audience-live.png` shows the live Delta Hymn slide, `options-song-tab.png` shows both checkboxes ticked and "CCLI b." in License Info; artifacts `.amp/in/artifacts/songs-browser-x11/`).
  - Unexecuted / open: Windows script still NOT RUN; EW observation of the ranking (PP-SB-04) and of live-redraw on Options change (PP-SB-07); a real slow-query race on a large library; reference-laptop timings (PP-SB-10).
  - Next: unchanged (Windows/EW items PP-SB-01..10, M1-05t, M1-04 Media tabs).
- Review round 2 (2026-10-23, fixer; reviewer findings on 3a88120, two minor, no false claims):
  - Fixed (minor, Windows script): `scripts/songs-browser-windows.py` typed "CCLI b." into License Info with `nw.type_text` (`KEYEVENTF_UNICODE`). GPUI's Windows backend at the pinned zed a846890 maps that VK_PACKET input to no keystroke (`crates/gpui_windows/src/events.rs:1525 VK_PACKET => None`; the text arrives only through WM_CHAR), so the B/"." Black bindings could never fire and the check could not fail. A new `press_keys` helper sends one virtual-key press per character (Shift+C, Shift+C, Shift+L, Shift+I, Space, B, VK_OEM_PERIOD 0xBE registered as `period` in the script, because `native_win.VK` has no "." entry; zed `crates/gpui_windows/src/keyboard.rs get_key_from_vkey` maps VK_OEM_PERIOD to "." through MapVirtualKeyW). The settings.txt `license_info=CCLI b.` check still proves the keys reached the field. Docstring corrected (it no longer says Unicode typing is "real keys"). Checked on Linux by extracting `press_keys` with `ast` and a fake session: keys `['shift+c', 'shift+c', 'shift+l', 'shift+i', 'space', 'b', 'period']`. Still NOT RUN on Windows (no host).
  - Fixed (minor, docs): the X11 "Options OK ... leaves the live frame unchanged" step cannot detect a re-cue on OK, because the 624x351 xvfb audience draws no copyright strip and a re-sent Delta Hymn slide is pixel-identical. The script docstring and PASS message, `docs/e2e-linux.md` (songs-browser row) and parity-pending PP-SB-07/PP-SB-09 now say the Linux script only catches leaked Black/Clear/Logo keys there, and that "OK never re-cues" is evidenced by the GPUI test `typing_in_license_info_never_toggles_masks_or_cues_live` (renderer submission count) and, once run, by the Windows script (slide 2 would gain the strip). The harness audience size (`XVFB_AUDIENCE_SIZE`, shared) was not changed to make the strip drawable. No product code changed, so no new Rust regression test.
  - Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (726 passed, 10 ignored: 408+244+9+4+13+0+9+19+1+0+0+19); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 -m ruff check` and `ruff format --check` on both songs-browser scripts clean; `python3 scripts/native_x11.py serve --display :114` then `python3 scripts/songs-browser-x11.py --display :114 --build --out <scratchpad>` PASS (42 s; scratch output deleted afterwards).
  - Unexecuted / open: `songs-browser-windows.py` NOT RUN (no Windows host), so the virtual-key typing is unverified against a real GPUI Windows window; a US keyboard layout is assumed for VK_OEM_PERIOD; the rest as in round 1.
  - Next: unchanged (Windows/EW items PP-SB-01..10, M1-05t, M1-04 Media tabs).
- Integration (2026-10-23): merged into `ccr-5e192d6c-emgkai` as merge commit `464d82f` ("Merge branch 'wave/songs-browser'", `--no-ff`, on top of the autosave-recovery integration `483fd3a`). Conflicts, all resolved keeping both sides: `docs/e2e-linux.md` (run table keeps the song-import and autosave rows, songs-browser row added after them; scrutiny-r2 and portal notes kept), `docs/reference-observations.md` (import, scrutiny-r2 and autosave tables kept, songs-browser best guesses appended), `docs/work-log.md` (all four Current-state bullets; status rows: base M1-03/06/09/10, songs-browser M1-04 added after M1-03, M1-05 and M1-07 carry both the base's scrutiny-r2/import notes and this stream's notes), `src/i18n.rs` (count guard 468 + 93 + 11 = 572; enum, `ALL` and both tables merged cleanly), `src/operator/polling.rs` (`poll_settings(cx)` with `poll_song_import` and `poll_autosave`), `src/storage.rs` (base import tests and this stream's sort/label/migration tests both kept), `src/tests.rs` (`scrutiny_r2`, `song_import`, `songs_browser`). No version collision: the base is still at SQLite schema 6, so this stream keeps schema 7 (`library.schema6-backup`); no `.selaschedule`, settings-format or Cargo change. No integration fix needed. Gate on the merged tree in `/home/user/sela`: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 468 passed + 4 ignored, bin 291 + 3 ignored, output_process 9 + 1, transport_process 4 + 1, examples 13, 0, 9, 19 + 1, 1, 0, 0, 19); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Native, serially on `:199` (`python3 scripts/native_x11.py serve --display :199`, each script with `--display :199 --build`): `songs-browser-x11.py` PASS (43 s), `autosave-x11.py` PASS (21 s), `live-output-x11.py` PASS (60 s), `scenario-s01-x11.py` PASS (49 s), `scenario-s02-x11.py` PASS (53 s), `schedule-x11.py` PASS (35 s), `song-editor-x11.py` PASS (64 s), `song-import-x11.py` PASS (31 s). Windows scripts NOT RUN (no Windows host).

### themes — M1-08 basic themes and background resolution (2026-10-22)
- State: **implemented-unqualified** (branch `wave/themes`, Linux cloud container, 4 CPUs, no Windows host). M1-08 checklist ticked from evidence with notes in backlog.md; Windows, EasyWorship and hardware qualification remain open.
- Decisions: (1) `sela::theme::Theme` = id, name, full `SlideFormat`, optional background, `show_copyright`; the built-in theme is the pre-themes look (so untouched libraries render byte-identically and its key is 0). (2) Inheritance is explicit and field by field (docs/themes.md): slide field > theme field > built-in; background slide > song master > theme > black (keeps M1-05h's slide → master → black, EW8-OBS-037..041); copyright strip only when the theme shows the element (EW8-OBS-032, -063). (3) Which theme: schedule item snapshot > song's library theme > library default (EW8-OBS-019 "Set As Default Song Theme") > built-in; a song naming a deleted theme falls back to the default with a Preview note. (4) Song→theme is stored by song id (`song_themes`), not per revision, so applying a theme creates no revision and never disturbs pinned schedule revisions or search; schedule items instead snapshot their theme (captured when first shown/saved, explicit when applied; only explicit ones dirty the schedule) and `.selaschedule` v4 stores every item's snapshot. (5) Margins are a new `SlideFormat` field (codec bit 10, 1080-reference points, ≤400) carried to the audience as `PreparedCue` insets with one shared `slides::lyrics_box` for fit and raster; the wire appends an optional block, so cues without margins are byte-identical. (6) Copyright *position* stays the observed bottom strip, shown/hidden (TH-BG-08). (7) The theme editor reuses the Song Editor (theme mode over a one-slide sample, EW8-OBS-043) instead of a second editor; margins sit under the Text tab because EW's Text › Layout is still inert (M1-05k). (8) Warnings are Sela additions (TH-BG-07): uninstalled font, missing/changed image, WCAG contrast < 3:1 without a separating outline, image aspect off by > 2% (Stretch %, Maintain bars, Zoom crop %). (9) New shared-file edits kept append-style: i18n (+55 messages EN/ID), `tests.rs` (module line + two `theme: None` fixture fields + the newer-version test now uses `VERSION + 1`), operator `mod.rs`/`polling.rs`/`layout.rs`/`preview_live.rs` hooks, Themes tab in its existing `resources/themes.rs` placeholder; editor buttons gained AccessKit ids `editor-button-N`. No new Control enum entries (own focus handles). Schema version used: 7 (CURRENT 7; the integrator renumbers if another stream also took 7).
- Results (all on Linux, `CARGO_INCREMENTAL=0`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. `cargo test --locked --all-targets -j 3` could not be linked in one invocation because the shared disk had 30-130 MB free (other streams' targets); every target it covers was run with the same flags one at a time, deleting each test binary afterwards: `cargo test --locked --lib -j 3` 415 passed, 4 ignored; `cargo test --locked --bin sela -j 3` 243 passed, 3 ignored (incl. 6 `tests::themes`); `--test output_process` 9 passed, 1 ignored; `--test transport_process` 4 passed, 1 ignored; `--example` composition_spike 14, edit_song 0, input_check 9, native_cues 20 (1 ignored), output_spike 1, search_bench 0, video_spike 20, seed_library 0, all ok. Revert proof: with `theme::apply` short-circuited to return the song unchanged, `theme::tests::slide_then_master_then_theme_then_default`, `tests::themes::applying_a_theme_to_a_song_restyles_preview_but_live_waits_for_go_live` and `tests::themes::missing_fonts_and_themes_are_reported_in_preview_not_hidden` failed; restored, they pass. Native: `python3 scripts/native_x11.py serve --display :117` then `python3 scripts/themes-x11.py --display :117` PASS (40 s, binary sha256 f37bf47ba9dd1ca2…, artifacts `.amp/in/artifacts/themes-x11/`); captures inspected: navy/gold left-margined audience slide, pale Low Contrast item slide, theme editor in theme mode, Themes tab with thumbnails and warnings. `python3 -m ruff check scripts/themes-x11.py scripts/themes-windows.py` all checks passed (ruff format applied).
- Findings: (1) the Themes footer first wrapped its status line under the buttons, moving them; the X11 run caught clicks landing on the old positions, so the footer is now one fixed line with a truncated status. (2) X11 harness: after an operator-process editor window closes, or once the operator's X title names the schedule, AT-SPI lookups of the operator frame by title time out; `themes-x11.py` caches control centers and selects the schedule item with Down (docs/e2e-linux.md). (3) The built-in row is badged both Default and In Preview while an unthemed song is previewed, as designed; the script expects it.
- Unexecuted / open: `scripts/themes-windows.py` NOT RUN (no Windows host; PP-TH-07); EasyWorship 8.0.49 Themes tab, apply, live edit, editor, default/delete and warning behavior unobserved (PP-TH-01..06, TH-BG-01..08); `--kind dual` X11 run not done; no golden-image (R) or physical display run of margins; Narrator/Orca; Indonesian strings not reviewed by a native speaker; one-invocation `cargo test --all-targets` blocked by disk only.
- Next: run `themes-windows.py` and `song-editor-windows.py` on Windows; observe PP-TH-01..06 in EW 8.0.49; move margins into Text › Layout with M1-05k; consider "Update items in Schedule" offering theme snapshot refresh.

### themes — review round 1 fixes (M1-08) (2026-10-23)
- State: **implemented-unqualified** (branch `wave/themes`, Linux cloud container, no Windows host). Fixes all five major and four minor reviewer findings; the backlog's render checkbox is unticked (no R evidence) and two follow-up tickets (M1-08a, M1-08b) are added as planned.
- Decisions: (1) A substitute theme is never captured: `Operator::capturable_theme` returns the library theme only when the theme set is loaded and not failed (`ThemeState::trusted`, new `failed` flag set by a failed Load reply) and the song's theme resolves; otherwise `themed_item` shows the default look with a Preview note ("This song's theme is missing" or the new "Themes are not loaded …") and `write_schedule` saves the item without a snapshot (it resolves again on reopen). No automatic reload loop after a failed load (a corrupt row would fail again); the next LibraryChanged reloads (TH-BG-10). (2) Preview rebuilds when the wanted theme's whole-value stamp or its notes differ (`Item::theme_view` vs `Operator::wanted_theme`), not only the look key, so a same-look theme relabels Preview and the In Preview badge. (3) `Repository::set_song_themes` applies a theme to all songs in one Immediate transaction (theme checked first; any deleted song is `Missing` and nothing changes); the operator says "A selected song no longer exists · no song was restyled" and refreshes the catalog. The worker bound is `MAX_SONG_THEME_BATCH` = 10 000 (was MAX_THEMES = 256), and the operator refuses a larger selection with its own message. (4) The theme editor saves a library theme with the new `Command::UpdateTheme` (`Repository::update_theme`, `Missing` if deleted), new themes with `SaveTheme` until their first save, so OK after a delete in the Themes tab does not re-insert it. (5) Thumbnails: `prune_theme_thumbs` keeps only listed themes at the current output size and releases dropped images with `cx.drop_image` (the `retain` in `themes_replied` is gone). (6) Margin fields get accessible ids/names (`theme-margin-<side>-field`, "Left margin" …), "Use default" is editor button 32 (`MARGINS_DEFAULT`, focusable, Role::Button, Enter/Space via ActivateControl, tab index after the margins). (7) `themes-x11.py`/`themes-windows.py` step 6 now re-cues the Evening Navy library song so the edited theme itself is live before Reset Look + OK, then requires the navy frame unchanged until the re-cue. (8) The EW8-OBS-057 copy-on-write rule is documented as replaced (song master stays a song-local layer above the theme, TH-BG-09); the Layouts "Theme - name" selector and image "Copy to Theme ▸ Song Theme" are tracked as M1-08a/M1-08b and PP-TH-08/09. docs/themes.md no longer claims a `.selaschedule` keeps "the same look on any library" (image backgrounds are name + SHA-256 references). i18n +8 messages EN/ID (ALL count guard 523 → 531).
- Results (Linux): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 in one invocation (lib 417 passed / 4 ignored; bin sela 250 passed / 3 ignored incl. 13 `tests::themes`; output_process 9/1 ignored; transport_process 4/1 ignored; examples composition_spike 14, input_check 9, native_cues 20/1 ignored, output_spike 1, video_spike 20, edit_song/search_bench/seed_library 0); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Revert proof: with capture ungated, stamp compare replaced by the key, pruning removed, the Missing arm disabled, `UpdateTheme` bypassed, the `MARGINS_DEFAULT` arm removed and `set_song_themes` committing before a missing song, the 7 new GPUI tests and `storage::tests::song_themes_apply_to_every_song_or_to_none` failed; restored, all pass. Native: `python3 scripts/native_x11.py serve --display :117` then `python3 scripts/themes-x11.py --display :117` PASS (42 s, binary sha256 0e1855b2e1137d1a…, new capture `audience-navy-before-edit.png` inspected: navy/gold Evening Navy slide live before the edit). `python3 -m ruff check` / `ruff format --check` on both theme scripts clean; `py_compile` ok.
- Unexecuted / open: `themes-windows.py` NOT RUN (no Windows host; PP-TH-07); EW 8.0.49 observations PP-TH-01..06, -08, -09; R-level golden images; M1-08a, M1-08b planned; Orca/Narrator check of the new margin names; Indonesian strings unreviewed by a native speaker.
- Next: Windows run of `themes-windows.py`; EW observation of PP-TH-05/-08/-09 to decide M1-08a's Master scope.

### themes — review round 2 fixes (M1-08) (2026-10-23)
- State: **implemented-unqualified** (branch `wave/themes`, Linux cloud container, no Windows host). Fixes the major and all four minor reviewer findings and corrects round 1's false claim. Resumed from the `wip(themes)` checkpoint 5ecfbdc left by a container restart (it did not compile: `THEME_RETRY` was missing and `Reply::ThemeGone` had no operator arm).
- Decisions: (1) Correction of round 1 decision (5): `cx.drop_image(image, None)` inside `Operator::render` released nothing, because GPUI `a846890` `App::update_window_erased` takes the rendering window out of `App.windows` and `App::drop_image` (crates/gpui/src/app.rs:2983) only visits windows still there. `theme_list` → `ensure_theme_thumbs` → `prune_theme_thumbs` now take the operator `Window` and call `cx.drop_image(image, Some(window))`; the thumbnail-task callback (outside any window update) keeps `None`. (2) `themes_replied(AppliedSong)` updates `song_theme` of any previewed item of a restyled song, not only library previews; `wanted_theme` ignores it for entries with a snapshot, so only snapshot-less entries (e.g. a song that named a deleted theme) rebuild and capture the new theme. The reachable path: Apply to Song queued from the library, then the schedule item selected before the write runs (the item's `ThemedSong` load is submitted before the theme job). (3) `Command::SetSongTheme` answers `Reply::ThemeGone(set)` when `set_song_themes` is `Missing` because the theme is gone (not built-in, absent from the re-read set); the operator says "“name” no longer exists · no song was restyled" and shows the new set (theme dropped, selection back to built-in). A deleted song keeps "A selected song no longer exists" and a catalog refresh. (4) A theme Load failing with `Locked` (SQLite busy/locked after the 100 ms busy_timeout) or `Busy` (worker queue full, also at submit) sets `retry_at = now + THEME_RETRY` (2 s); `take_theme_job` reloads when due; status "Themes could not be loaded (Locked) · retrying". Corrupt/IO failures still wait for LibraryChanged (TH-BG-11). (5) Theme editor: `UpdateTheme` → `Missing` now uses `reject_status` (failure styling) with "… it was not saved · OK saves it as a new theme", and gives the draft a fresh `ThemeId::allocate()` with `stored = false`, so the next OK/Apply inserts a new theme; the deleted id is never re-inserted (songs that still name it stay on the default) (TH-BG-12). i18n +2 messages EN/ID (ALL count guard 531 → 533). Docs: themes.md, reference-observations TH-BG-11/-12, parity-pending PP-TH-10, backlog progress note, e2e-linux run row.
- Results (Linux, `CARGO_INCREMENTAL=0`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 in one invocation (lib 418 passed / 4 ignored; bin sela 253 passed / 3 ignored incl. 16 `tests::themes`; output_process 9/1 ignored; transport_process 4/1 ignored; examples composition_spike 14, input_check 9, native_cues 20/1 ignored, output_spike 1, video_spike 20, edit_song/search_bench/seed_library 0); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Revert proof (all five reverts applied together, then restored): `drop_image(.., None)` → `theme_thumbnails_keep_one_per_row_at_the_current_output_size` fails at the atlas assertion (`Window::has_image_atlas_entry`, GPUI test-support); the `entry.is_none()` filter → `apply_to_song_reaches_a_previewed_schedule_item_without_a_snapshot` fails; the old `SetSongTheme` arm → `apply_to_song_reports_a_theme_deleted_meanwhile_and_drops_it` and `storage::tests::apply_to_song_tells_a_deleted_theme_from_a_deleted_song` fail; no retry → `a_theme_load_blocked_by_another_connection_is_retried` fails (a second rusqlite connection holds `BEGIN EXCLUSIVE`); the old editor arm → `a_theme_editor_save_never_resurrects_a_deleted_theme` fails; restored, all pass. Native: `python3 scripts/native_x11.py serve --display :117` then `python3 scripts/themes-x11.py --display :117 --out <scratchpad>/themes-x11` PASS (exit 0, 42 s, binary sha256 9b60e612488b600a…); `operator-themes-tab.png` inspected (built-in + 4 themes with thumbnails, warnings, Default/In Preview badges). Scratch captures deleted afterwards. `df -h /` 16 GB free.
- Unexecuted / open: `themes-windows.py` NOT RUN (no Windows host; PP-TH-07); EW 8.0.49 observations PP-TH-01..06, -08..-10; atlas release is verified on GPUI's headless atlas only (no GPU atlas measurement on a real driver); R-level golden images; M1-08a, M1-08b planned; Indonesian strings unreviewed by a native speaker.
- Next: Windows run of `themes-windows.py`; EW observation of PP-TH-05/-08/-09/-10.
- Integration (2026-10-23): merged into `ccr-5e192d6c-emgkai` as merge commit `8765b3e` ("Merge branch 'wave/themes'", `--no-ff`, on top of the songs-browser integration `5a588d9`). Conflicts, all resolved keeping both sides: `docs/e2e-linux.md` (script list and run table: themes rows after songs-browser), `docs/parity-pending.md` (Windows run order: `themes-windows.py` renumbered from row 23 to row 27), `docs/reference-observations.md` (import, scrutiny-r2, autosave and songs-browser tables kept, themes TH-BG table appended), `docs/storage.md`, `docs/work-log.md` (all Current-state bullets; status rows from the base plus the M1-08 row), `src/i18n.rs` (count guard 572 + 65 = 637), `src/operator/mod.rs` (`SongImport`/`RecoveryCheck` and `Themes` requests), `src/operator/polling.rs` (`self.worker` borrow with the import cancel handle and the theme error arms), `src/storage.rs`, `src/tests.rs`, `examples/seed_library.rs` (`--sort-songs` and `--themes`). **Version collision:** the base already used SQLite schema 7 for the songs-browser search index reshape, so this stream's themes tables are renumbered to **schema 8** (`SCHEMA8`, backup `.schema7-backup`, abort hook `SELA_ABORT_MIGRATION_SCHEMA8`, tests `schema7_migration_adds_empty_themes_behind_a_verified_backup` / `schema7_migration_abort_keeps_schema7_and_the_verified_backup`; "newer schema" tests now use 9; docs/storage.md, docs/themes.md, backlog and status row updated). Since no step after schema 7 rebuilds the search index, a schema 7 file being upgraded rebuilds a damaged or inconsistent index itself (`SEARCH_SHAPE`). `.selaschedule` v4 and settings did not collide. Integration with M1-12 (in the merge commit): `schedule::Pin`/`Entry` gained theme fields, so recovery revisions are now format **version 2** (item `theme`/`theme_explicit`, pin `theme_stamp`; version 1 still reads; `session.json` stays 1), `Snapshot::of` uses `Schedule::pins`, and the fingerprint counts applied themes but not captured ones (a capture is not an edit; without this `autosave-x11.py` saw an extra commit); new test `a_revision_keeps_item_themes_and_version_1_still_reads`; docs/autosave.md updated (fingerprint timing not re-measured). The theme copyright strip uses the base's `CopyrightStrip` (`theme::copyright_of`). Integration fix `92de481`: `schedule-{x11,windows}.py` expected `.selaschedule` v3 and `autosave-{x11,windows}.py` recovery v1. Gate on the final tree in `/home/user/sela`: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 491 passed + 4 ignored, bin 308 + 3, output_process 9 + 1, transport_process 4 + 1, examples 14, 0, 9, 20 + 1, 1, 0, 0, 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Native, serially on `:199` (`python3 scripts/native_x11.py serve --display :199`, debug binary sha256 `875675e319335fc1…`): `autosave-x11.py` PASS (20 s), `live-output-x11.py` PASS (56 s), `scenario-s01-x11.py` PASS (45 s), `scenario-s02-x11.py` PASS (50 s), `schedule-x11.py` PASS (34 s), `song-editor-x11.py` PASS (63 s), `song-import-x11.py` PASS (30 s), `songs-browser-x11.py` PASS (41 s), `themes-x11.py` PASS (43 s). (A first pass on a hand-started Xvfb + plain openbox failed the focus checks of several scripts because the harness's openbox rules were missing; rerun on `serve` as above.) Windows scripts NOT RUN (no Windows host).
### media-core — FFmpeg base and decode pipeline (2026-10-22)
- State: M2-01 and M2-02 **implemented-unqualified** for this slice (branch `wave/media-core`, Linux cloud container, no Windows host). Covers on Linux: fetch, locate at startup with log and warning, ffprobe metadata, supervised decoder whose held frames are bounded by count and `frame_budget` bytes (enforced since review round 1; the first slice only reported the lowered count). Open even on Linux: Unix children are not killed if Sela itself dies (no parent-death signal), see review round 1. Nothing is shown to the operator or audience as video yet (video-playback stream). Windows runs, hardware decode and EasyWorship observations stay open (parity-pending PP-MC-01..04).
- Decisions: (1) Pins: the existing win64 asset plus `ffmpeg-n8.1.3-9-g29e619e767-linux64-lgpl-shared-8.1.tar.xz` (65,590,572 bytes, SHA-256 `d4d7b693…535a`), both matching the release's own `checksums.sha256`; the Linux build runs in place through `RPATH $ORIGIN/../lib` and reports version `n8.1.3-9-g29e619e767-20260930`. (2) `scripts/fetch-ffmpeg` / `fetch-ffmpeg.ps1`: download to a temp folder beside the destination, refuse on size then SHA-256 mismatch before unpacking, write `SELA-PIN.txt`, idempotent; `SELA_FFMPEG_BASE_URL` for mirrors/tests. Binaries stay under the gitignored `.amp/in/`. (3) Startup: `SettingsConfig::media_locator` (real `ffmpeg::locate` in `main`, fakes in tests, `None` = no check) runs on a `sela-media-locate` std thread; the 16 ms poll takes the result, prints `sela: media: …` on stderr and sets `Availability`; the status row shows an amber dot and "Video and audio unavailable: …" (Role Status, id `media-unavailable`, EN+ID) with the most actionable reason (nonfree > GPL > broken > missing). Sela-only behavior, no EW8-OBS. (4) `SELA_FFMPEG_NO_DEV_COPY=1` leaves the debug dev copy out so a debug build can show the missing/refused paths (native checks); release builds have no dev copy. (5) Decoder: ffprobe JSON → `MediaInfo`; ffmpeg `-map 0:<v> -an -sn -dn -vf scale,setsar[,pad],format=rgba,showinfo=checksum=0 -fps_mode passthrough -f rawvideo -pix_fmt rgba pipe:1`; `showinfo` lines (stderr) carry each frame's number/time/size before its bytes reach stdout; frames are `Arc<[u8]>` read exactly W×H×4; timestamps relative to the first frame, missing/backwards ones repaired (+duration, 1/rate or 1/30) and flagged. Held frames = `queue_frames` (1..=16) lowered to fit `frame_budget` (default four 4K frames), enforced by a slot count since review round 1 (see below); full queue = backpressure, not a stall; watchdog kills on `stall_timeout` of silence while waiting on FFmpeg; drop cancels without waiting. `Fit::Source/Within/Exact` (Exact letterboxes to the cue extent, so `VideoFrame::background()` is a cue `PreparedBackground` without copying). (6) `media::process::Supervised`: Unix `process_group(0)` + `kill(-pgid, SIGKILL)` before reaping; Windows Job Object `KILL_ON_JOB_CLOSE` + `CREATE_NO_WINDOW` (raw kernel32 declarations like `timer.rs`; no new crate). No Job memory limit (untested limits could break decodes). (7) VAL-MEDIA-038 finding: FFmpeg 8.1's `file` protocol already restricts nested opens to `file,crypto,data`, so the crafted HLS/ffconcat playlists are refused even without Sela's `-protocol_whitelist file`; the flag stays as defense in depth and is guarded by the existing unit test. (8) GPUI: no new upstream pattern; the locate result uses the same std-thread + poll pattern as the existing workers (zed `a846890`, no new upstream code studied).
- New: `src/media/{process,probe,decode,error,backend}.rs`, `src/operator/media_backend.rs`, `src/tests/media_backend.rs`, `tests/media_decode.rs`, `scripts/fetch-ffmpeg`, `scripts/fetch-ffmpeg.ps1`, `scripts/media-backend-x11.py`, `scripts/media-backend-windows.py`; `ffmpeg::probe_args`, `ffmpeg::probe_build` (was private `probe`), `ENV_NO_DEV_COPY`; four `Msg::MediaUnavailable*` EN/ID strings (ALL count 468 → 472); docs/media.md rewritten for the slice; backlog M2-01/M2-02 progress notes (no checkbox ticked: each still needs presentation, Windows or hardware evidence); reference-observations "Media decoding best guesses"; parity-pending PP-MC-01..04 and run-order row 23; e2e-linux.md file list. No schema change.
- Tests: unit — `media::process` (group kill takes a grandchild, drop kills and reaps, kill after exit is a no-op), `media::probe` (7), `media::error` (3), `media::backend` (3), `media::decode` (10: showinfo parsing, clock repair, fits, filter chain, args, background sharing), `media::ffmpeg::locate_leaves_out_the_dev_copy_on_request`. Integration `tests/media_decode.rs` (19, real FFmpeg, clips generated at test time with testsrc/color/sine): every frame in order with timestamps; VFR gap kept (frame 10 at 0.8333 s); 317x239, 33x17 and 1x1 with exact color; Exact letterbox into 640x360 as a background sharing pixels; Within never enlarges; 1-frame, 0.1 s and frame-limited clips; unknown codec tag `SELA` → `UnsupportedCodec` with no Opened/frame; garbage → `Corrupt`, half file → frames then terminal, header only → failure; audio-only → `NoVideo`; VAL-MEDIA-038 (HLS and ffconcat to `http://127.0.0.1:<port>` → `ProtocolRefused`, listener saw no connection, plus the decoder's own ffmpeg args print "not on whitelist"); refused/missing inputs; backpressure (ffmpeg still running after 1.2 s unread with a 300 ms stall timeout, then all 60 frames, no stall); `try_next` < 20 ms; drop kills ffmpeg (< 100 ms drop); outside SIGKILL → `Crashed(signal)`; fake silent ffmpeg → `Stalled` and the child is gone; fake hanging ffprobe → `start` returns at once, `ProbeTimedOut`; frame budget → `TooLarge` / queue lowered to 2; the GPL `/usr/bin` build refused as `Refusal::Gpl` and the pinned build accepted. GPUI `src/tests/media_backend.rs` (5): `a_gpl_ffmpeg_shows_the_warning_while_live_output_keeps_working` (Go Live and Next acknowledged by the fake audience while locating, then the warning renders, no dialog, Black acknowledged), `locate_runs_on_its_own_thread_and_never_delays_input` (locate blocked, tab click and Ctrl+B handled, locate thread ≠ UI thread), `an_accepted_ffmpeg_shows_no_warning_and_offers_its_tools`, `the_warning_follows_the_ui_language`, `the_real_locator_accepts_the_pinned_development_build`.
- Fail-without-change checks (each applied, run, then reverted with `git checkout`): disabling `poll_media_backend` in `poll` failed all 5 GPUI tests; counting the Blocked phase as a stall failed `backpressure_holds_the_decoder_without_a_stall_or_lost_frames`; making the clock ignore source timestamps failed `a_generated_clip_decodes_every_frame_in_order_with_timestamps` and `variable_frame_rate_keeps_each_frames_own_timestamp`; removing the watchdog kill failed `a_silent_decoder_is_killed_at_the_stall_timeout` (30 s); removing `-protocol_whitelist file` from both builders did **not** fail the VAL-MEDIA-038 runtime test (finding (7)).
- Results (worktree `/home/user/sela/.claude/worktrees/wf_4eb25669-832-6`, `CARGO_INCREMENTAL=0` for disk space): `scripts/fetch-ffmpeg` fresh 6 s, re-run "already holds" exit 0, 1,000-byte mirror file exit 1 "size mismatch … nothing unpacked", right-sized zero file exit 1 "SHA-256 mismatch … nothing unpacked", `--platform mac` exit 2. Gate: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `SELA_REQUIRE_FFMPEG=1 cargo test --locked --all-targets -j 3` exit 0 (lib 423 passed + 5 ignored, bin 241 + 3 ignored, media_decode 19, output_process 9 + 1 ignored, transport_process 4 + 1 ignored, examples 13/9/19/1/19); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0 (Job Object code compiles). Native: `python3 scripts/native_x11.py serve --display :115` then `python3 scripts/media-backend-x11.py --display :115` PASS twice (final on the gate binary): with = "FFmpeg n8.1.3-9-g29e619e767-20260930 accepted from the development copy", no warning; gpl = log "unavailable (Gpl): SELA_FFMPEG_DIR /usr/bin refused: the build is configured with --enable-gpl", status text exact, 85 amber samples, warning 1.0 s after launch, Go Live 84 ms to audience change, Ctrl+B black; missing = "unavailable (Missing)" and "Video and audio unavailable: FFmpeg was not found". Captures `.amp/in/artifacts/media-backend-x11/` inspected (`gpl-operator-warning.png`, `gpl-operator-live.png`, `gpl-audience-go-live.png`, `missing-operator-warning.png`): restrained amber dot and text in the Resources status row, layout unchanged. `python3 -m ruff check scripts/media-backend-x11.py scripts/media-backend-windows.py` clean (ruff 0.17.1).
- Unexecuted / open: `scripts/fetch-ffmpeg.ps1` and `scripts/media-backend-windows.py` **NOT RUN: no Windows host** (PP-MC-03: Job Object kill, `CREATE_NO_WINDOW`, `tests/media_decode.rs` on Windows); EW observations PP-MC-01/02; mapped/subst drives PP-MC-04; hardware decode, 1080p60/4K performance and memory per resolution (not measured); macOS process-group path compiles only by cfg(unix) and is unrun; installer notices/About screen; `cargo test --release` not run (disk). Disk was the binding constraint (shared volume at 99-100%): the target was hard-linked from the seed with mutable files unlinked, incremental disabled.
- Next: video-playback stream — consume `Operator::media_tools()` and `Decoder` (Fit::Exact to the output extent) for the Media Fill video background (EW8-OBS-032/039), present frames by timestamp off the UI thread, gate Go Live on the first frame, keep the live scene on failure; then run PP-MC-03 on Windows.

#### media-core — review round 1 fixes (2026-10-23)
- State: M2-01/M2-02 stay **implemented-unqualified**. Every blocker/major finding and every cheap minor one fixed on `wave/media-core` (worktree `/home/user/wt/media-core`); two minors recorded as open (below).
- Decisions: (1) Frame budget (blocker): `Shared` keeps a held-frame count (`Mutex<usize>` + `Condvar`); the supervisor reserves a slot before allocating a frame buffer (Phase Blocked, so not a stall; touch after) and the slot frees when the consumer takes the frame in `try_next`/`next_timeout` (or when the frame is discarded). At most `Opened::queue_frames` frames (queued plus the one being filled) are held, so held bytes ≤ `frame_budget`; `Decoder::buffered_frames()` reports the count. The old "at most `queue_frames + 1`" wording is gone. (2) Stderr-join hang: the supervisor drops its frame-line receiver as soon as the frame loop stops, and the stderr thread keeps draining stderr (discarding frame lines) instead of returning, so FFmpeg is never held on stderr and the join returns after the kill. (3) `DecodeOptions::content_id` is now `Option<u128>`; `None` (default) takes a process-unique id (`"sela-vid"` tag in the high 64 bits + a counter) per `Decoder::start`; `Opened::content_id` reports it. (4) `first_frame` reports its own deadline as the new `MediaError::TimedOut(timeout)` ("preparing the picture took longer than … s"); `Stalled` stays the watchdog's. (5) `scripts/fetch-ffmpeg`: a value-taking flag without a value (or with an empty one) is a usage error, exit 2 with "needs a value". (6) Warning layout: the `media-unavailable` status and the output line shrink and truncate (`min_w_0` + `truncate`; dot `flex_shrink_0`) so the Resources header buttons keep their size; the accessible name keeps the full text. Sela-only, no EW8-OBS. (7) `media::process` docs, docs/media.md and the backlog no longer claim that Sela's own exit kills the tree on Unix.
- Tests (each fails without its fix, checked by applying the revert and running): `the_frame_budget_bounds_the_frames_held_while_the_consumer_waits` (queue 8, budget 2 frames, consumer idle 1.2 s: most held 2, then all 60 frames, no stall; with the slot wait disabled it held 9); `stopping_early_with_many_pending_frame_lines_still_ends` (1x1 frames, `max_frames` 2, 1.5 s idle: `End{2, limited}`; without `drop(meta_rx)` it hung, "no terminal event within 30s"); `default_decoders_get_distinct_content_ids`; `first_frame_reports_its_own_deadline_as_a_timeout` (unix fake ffprobe; was `Stalled`); `fetch_ffmpeg_reports_a_missing_flag_value_as_usage` (unix; the old script exits 1 silently); GPUI `the_longest_warning_fits_the_minimum_window_without_squeezing_controls` (Indonesian nonfree text at 720x440; before the fix the warning ran to x=1338 in a 720 px window, after: 38 px truncated, buttons unchanged).
- Results (`CARGO_INCREMENTAL=0`, `-j 3`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `SELA_REQUIRE_FFMPEG=1 cargo test --locked --all-targets -j 3` exit 0 (lib 423 + 5 ignored, bin 242 + 3 ignored, media_decode 24, output_process 9 + 1 ignored, transport_process 4 + 1 ignored, examples 13/0/9/19+1 ignored/1/0/0/19); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/native_x11.py serve --display :115` + `python3 scripts/media-backend-x11.py --display :115` PASS (with/gpl/missing as before, GPL warning 0.908 s, 85 amber samples, Go Live 83 ms; new run 4 "narrow-id": 720x440, Indonesian GPL warning at x=629 w=79, header buttons 163/122/159 px wide and narrow, 18 amber samples; captures `narrow-id-operator-warning.png` and `gpl-operator-warning.png` inspected: "Video dan…" and the output line "O…" truncated, controls intact; full English text at 1280x800). A first attempt on a bare `Xvfb :115` + `openbox` (no `serve`) failed at "gpl: Ctrl+B did not turn the audience black"; the harness's own `serve` display is the documented setup and passed. `scripts/fetch-ffmpeg --platform` / `--dest` / `--platform ''` exit 2 with the message; `--platform mac` exit 2; re-run "already holds" exit 0. `ruff check`/`ruff format --check` clean on `scripts/media-backend-x11.py`.
- Unexecuted / open: (a) Unix children outliving a crashed or `SIGKILL`ed Sela: no parent-death signal (no `libc` dependency; `PR_SET_PDEATHSIG` is per-thread on Linux and needs care with the supervisor threads), documented in `media::process` and docs/media.md; a fix needs its own ticket slice. (b) The Windows run (PP-MC-03) does not cover the `cfg(unix)` fakes (stall, hanging probe, `first_frame` timeout, fetch usage) or the GPL locator half; PP-MC-03 now lists manual steps for them. `fetch-ffmpeg.ps1` and `media-backend-windows.py` **NOT RUN: no Windows host**; the narrow Indonesian check was not added to `media-backend-windows.py` (unrunnable here). (c) VAL-MEDIA-038 still passes without Sela's flag on FFmpeg 8.1 (as recorded before; the unit test guards the flag).
- Next: video-playback stream consumes `Decoder` (take frames with `try_next`/`next_timeout` so slots free; use `Opened::content_id`); a later slice decides the Unix orphan policy.

#### media-core — review round 2 fixes (2026-10-23)
- State: M2-01/M2-02 stay **implemented-unqualified** (branch `wave/media-core`, worktree `/home/user/wt/media-core`). All four round-2 findings (all minor) fixed; no blocker/major was reported.
- Decisions: (1) Hover hints: the Live output line (now a stateful `output-status` element) and the `media-unavailable` status get a GPUI `.tooltip` built from a new `StatusHint` view (`src/operator/mod.rs`; surface fill, border, `shadow_md`, 12 px text, max 360 px wide) showing the full text, so a sighted operator can read "O…" and "Video dan…" at 720x440. Hints use GPUI's default 500 ms show delay; GPUI positions them inside the window. Original code on `StatefulInteractiveElement::tooltip` (zed `a846890`, `crates/gpui/src/elements/div.rs`; Zed's GPL `ui::Tooltip` not used). Sela-only (reference-observations "Media decoding best guesses"). (2) `scripts/fetch-ffmpeg.ps1` parses its own arguments (no `param()` block): a value flag without a value, `-Dest -Force`, an unknown flag or platform each print a message plus usage and exit 2, as the help says (a `param()` binding error exits 1 before the script runs, confirmed under pwsh 7.4.6). A `file://` `SELA_FFMPEG_BASE_URL` is now copied with `Copy-Item`, because PowerShell 7's `Invoke-WebRequest` refuses that scheme (the header comment claimed file:// worked). (3) Linux parent-death: `Supervised::spawn` adds a `pre_exec` hook with `prctl(PR_SET_PDEATHSIG, SIGKILL)` and a `getppid` race check (raw `extern "C"` declarations, no `libc` crate). The signal follows the spawning thread; the decoder's `sela-decoder` thread spawns ffmpeg and kills/reaps it before returning, and `probe` waits on the calling thread, so the per-thread semantics fit (the reviewer's point; the round-1 deferral reason was weak). macOS stays open (no such signal), now a backlog M2-02 checkbox. (4) Docs: M2-02 status row test count, M2-01 row, Current state bullet, backlog M2-01 progress + new M2-02 checkbox, docs/media.md (process isolation, fetch scripts, native checks), PP-MC-03.
- Tests (each fails without its fix, checked by reverting the fix and running): GPUI `hovering_truncated_status_items_shows_their_full_text` (Indonesian nonfree warning at 720x440; hover each item, advance 800 ms: a `status-hint` wider than the item and inside the window; without the tooltips "hovering media-unavailable shows a hint" fails); `tests/media_decode.rs::fetch_ffmpeg_ps1_reports_usage_mistakes_as_exit_2` (runs under `SELA_PWSH`, else `pwsh`, else `powershell` on Windows, else prints SKIPPED; against the old script `-Platform` exits 1 with "Missing an argument for parameter 'Platform'"); `media::process::tests::a_child_dies_when_its_parent_is_sigkilled` (Linux; the test binary re-runs itself as a parent that spawns `sleep 30` through `Supervised`, the test SIGKILLs it, and the child must be gone or a zombie within 5 s; with the hook disabled "child … outlived its SIGKILLed parent" after 5 s).
- Results (`-j 3`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `SELA_REQUIRE_FFMPEG=1 SELA_PWSH=/tmp/claude-0/pwsh/pwsh cargo test --locked --all-targets -j 3` exit 0 (lib 424 + 5 ignored, bin 243 + 3 ignored, media_decode 25, output_process 9 + 1 ignored, transport_process 4 + 1 ignored, examples 13/0/9/19+1 ignored/1/0/0/19); without `SELA_PWSH` the PowerShell test prints `SKIPPED … no PowerShell` and passes (this container has no pwsh on PATH; the pwsh used is a scratch copy of the official PowerShell 7.4.6 linux-x64 tarball, deleted with the scratchpad); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/native_x11.py serve --display :115` + `python3 scripts/media-backend-x11.py --display :115` PASS (debug binary sha256 `39def1ea…`, final source; with/gpl/missing as before, GPL warning 0.779 s, Go Live 68 ms; narrow-id warning [629,276,79,18], header buttons 163/122/159 unchanged; new hover checks: warning hint changed box x=296 y=280 380x68 with 60 dark-text samples, output hint x=444 y=280 170x48 with 13 samples; captures `narrow-id-hint-warning.png` ("Video dan audio tidak tersedia: FFmpeg yang ditemukan berlisensi GPL" in two lines) and `narrow-id-hint-output.png` ("Output langsung mati") inspected). The same script against a build without the tooltips fails "narrow-id: hovering the warning shows no hint". `pwsh -File scripts/fetch-ffmpeg.ps1` on Linux: `-Platform linux64 -Dest <scratch>` downloaded, verified ("size and SHA-256 verified (d4d7b693…)"), unpacked (ffmpeg reports n8.1.3-9-g29e619e767-20260930, `SELA-PIN.txt` identical to the bash script's) and a re-run said "already holds", exit 0 both; a 5-byte `file://` archive → "size mismatch … nothing unpacked" exit 1; a missing `file://` source → "download failed" exit 1; scratch output deleted. `ruff check` / `ruff format --check` clean on `scripts/media-backend-x11.py`.
- Unexecuted / open: `fetch-ffmpeg.ps1` under Windows PowerShell 5.1 and on Windows, `media-backend-windows.py`, the Job Object kill **NOT RUN: no Windows host** (PP-MC-03; the hover-hint check was not added to `media-backend-windows.py`, unrunnable here). macOS children outliving a crashed Sela (backlog M2-02 checkbox). The hint is mouse-only; keyboard users get the full text only through the accessible name (the status items are not focusable, as before).
- Next: video-playback stream consumes `Decoder` (take frames with `try_next`/`next_timeout` so slots free; use `Opened::content_id`); then PP-MC-03 on Windows.
- Integration (2026-10-23): merged into `ccr-5e192d6c-emgkai` as merge commit `3b61aad` ("Merge branch 'wave/media-core'", `--no-ff`, on top of the themes integration `0a6f07c`). Conflicts, all resolved keeping both sides: `docs/e2e-linux.md` (script list: `media-backend-x11.py` added after the themes entry), `docs/parity-pending.md` (Windows run order: `media-backend-windows.py` renumbered from row 23 to row 28), `docs/reference-observations.md` (import, scrutiny-r2, autosave, songs-browser and themes tables kept, media best guesses appended), `docs/work-log.md` (all Current-state bullets; base status rows plus M2-01/M2-02 before M4-07, the stream's unchanged M1-16 row dropped for the base's newer one), `src/i18n.rs` (import/recovery/themes variants plus the four `MediaUnavailable*`; count guard 637 + 4 = 641), `src/operator/polling.rs` (`poll_settings(cx)`, `poll_song_import()` and `poll_media_backend()` all kept, `poll_autosave(cx)` after). No SQLite schema, `.selaschedule` or settings version change in this stream, so no renumbering. No integration fix needed. The FFmpeg development copy was moved (not copied) from the stream worktree to `/home/user/sela/.amp/in/ffmpeg/n8.1.3-lgpl` (gitignored; `scripts/fetch-ffmpeg` reports "already holds"). Gate on the merged tree in `/home/user/sela`: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `SELA_REQUIRE_FFMPEG=1 cargo test --locked --all-targets -j 3` exit 0 (lib 519 passed + 5 ignored, bin 315 + 3, media_decode 25, output_process 9 + 1, transport_process 4 + 1, examples 14, 0, 9, 20 + 1, 1, 0, 0, 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Native, serially on `:199` (Xvfb 1920x1080x24 + Openbox via `scripts/native_x11.py serve --display :199`, debug binary sha256 `8b9a1a2dda76ea77…`): `autosave-x11.py` PASS (19 s), `live-output-x11.py` PASS (57 s), `media-backend-x11.py` PASS (22 s), `scenario-s01-x11.py` PASS (48 s), `scenario-s02-x11.py` PASS (51 s), `schedule-x11.py` PASS (34 s), `song-editor-x11.py` PASS (63 s), `song-import-x11.py` PASS (29 s), `songs-browser-x11.py` PASS (42 s), `themes-x11.py` PASS (42 s). Windows scripts NOT RUN (no Windows host).

### pr1-review — cubic review findings on PR #1 (2026-10-23)

- Scope: all 48 cubic review threads on PR ThomasBudiarjo/sela#1 (page 1: 30, page 2: 18), each verified against the code on `wave/pr1-review` (base `5a588d9`) by tracing a real caller or input. Nothing was posted on GitHub and nothing was pushed.
- Triage: 45 fixed, 3 not fixed. P1 (2): both fixed. P2 (20): 18 fixed; not fixed: popup Tab double move (GPUI a846890 `crates/gpui/src/window.rs:6423` stops an action at its first bubble-phase listener; `scrutiny_r2::show_keys_stay_inert_while_the_schedule_item_menu_is_open` already asserts one item per Tab with wrap) and the title-length fingerprint (titles change only with their pin except when a hand-edited schedule file is reopened, a stale display title for the same pinned song; whole titles cost 149 µs per 16 ms poll, docs/autosave.md; the comment now says so). P3 (26): 25 fixed; not fixed: "first displayed slide" (already an UNOBSERVED best guess in reference-observations and PP-SB-05, whose steps start Live at slide 3).
- Fixed with regression tests (each failed once with the fix reverted): (1) failed autosave marker writes are retried: `Reply::Marked`/`Reply::Unmarked`, `Policy::marker_failed` retakes the transition after the quiet period (2 s), `Store::clear_uncommitted` keeps the marker in memory until the write lands (`a_marker_that_could_not_be_written_is_written_once_the_disk_allows`, `a_failed_marker_clear_is_written_again_on_retry`, `a_failed_marker_write_is_taken_again_after_the_quiet_period`); (2) without the profile lock autosave stays off ("… cannot be locked for one Sela at a time", or read-only), new `AutosaveRefused` global and `autosave::Error::Unlocked` (`without_the_profile_lock_autosave_stays_off_and_writes_nothing`); a bare `--operator-library library.sqlite` names `.` as its profile (`profile::qualified_library`); (3) `mark_uncommitted` keeps the previous run's marker while its offer is open (`an_unanswered_offer_survives_a_second_crash`); (4) `refresh_live_request` clears `cue_failed` (`normalize_after_a_failed_cue_clears_the_failed_indicator`); (5) Options › Song takes no input until settings load and shows "Loading settings…" (`the_song_tab_takes_no_input_until_settings_load`); (6) equal sort keys (case-only, leading zeros) break by title (`library_sorts_by_author_copyright_administrator_and_song_number`); (7) the copyright strip size comes from its first two lines and a wider licensing line is cut with "…" (`a_long_license_line_is_cut_and_never_shrinks_the_copyright`; before: a 256-byte License Info gave 12 px at 1080p and no strip at 854x480), UNOBSERVED best guess PP-SB-11; (8) a panicked `sela-import` thread ends its job with `Event::Failed`, the preview closes with "The import stopped unexpectedly …" and the next import starts a new thread (`a_panicked_worker_reports_the_job_failed_not_canceled`, `a_worker_that_stops_unexpectedly_is_reported_and_replaced`); (9) the profile lock tests no longer depend on root or `/proc` (`sela.lock` as a directory) and reap the child holder on a failed assertion. Without a test seam: a RecoveryCheck submit failure reports in the schedule panel; the Tab handles come from `OptionsTab::ALL`; `AutosaveFailed` takes `{seconds}` from the cadence.
- Docs/wording: native-testing.md (harness claims qualified, Reproduce marked M0-03 record), e2e-linux.md Results header, backlog M1-07 stale sentence, parity-pending PP-AUTO/PP-SC2 in ID order and PP-SB-11, reference-observations sort tie rule and strip best guess, autosave.md (marker retry, lock-off behaviour, failure table), import.md (`Failed`, extensionless files are read), `CapCache` comment, `import::batch` module doc, ID "secara normal" and "lirik dari berkas".
- Scripts: native_x11 (build/seed timeouts are ENVIRONMENT; `controls()` keeps polling until the app registers with AT-SPI), scenario-s01-x11 (chorus on black with text), live-output-x11 (underline check; docstring 624x351; italic slant only unit-checked), schedule-x11 (Tab count from live-output-x11, bounded 1 s watch after Down), song-import-x11 (restores keycode 93's own mapping), autosave-x11/-windows (truncated revision is FAIL); Windows only, NOT RUN: native_win `f10`, songs-browser-windows selects Echo by title after Esc, scenario-s01-windows (fresh title, Cancel keeps the window), scenario-s02-windows (B's stored lyrics), autosave-windows (sela.lock metadata watched, temporary profile removed after the processes end), song-import-windows (setup inside the cleanup scope, exactly four tiles, slide 2 selected).
- Commits: `e102f09`, `68f0265`, `f216c03`, `2388272`, `a319f17`, `8a6fabf`, `fbf03c9`, `339834d`, `eeb42ec`, `c5e9a7a`, `a9a2ffe`, plus this record. Disclosure: `68f0265` and `339834d` each add a message without raising the i18n count guard, so `i18n::tests::all_lists_every_variant_exactly_once` fails at those two commits and passes from `eeb42ec` on.
- Results (final tree): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (843 passed, 10 ignored: 473+296+9+4+13+0+9+19+1+0+0+19); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 -m ruff check` on the 12 changed scripts clean (ruff format applied where the base file was formatted). Native, serially on `:131` (`python3 scripts/native_x11.py serve --display :131`: Xvfb :131 1920x1080x24 -nolisten tcp + Openbox with the harness rules; debug binary of this tree): `schedule-x11.py` PASS 35 s, `song-import-x11.py` PASS 31 s, `autosave-x11.py` PASS 24 s, `songs-browser-x11.py` PASS 47 s, `live-output-x11.py` PASS 60 s (underline run 514 px; the non-underlined captures give at most 31 px, so the check discriminates), `scenario-s01-x11.py` PASS 47 s (23684 black samples on the chorus), `scenario-s02-x11.py` PASS 52 s, `song-editor-x11.py` PASS 66 s; display stopped afterwards.
- Unexecuted / open: every Windows script above NOT RUN (no Windows host); no EasyWorship observation (PP-SB-05, PP-SB-11); the marker retry and lock-off paths have no native run (headless tests only; a lock-less filesystem was not available).
- Next: merge into the integration branch, post the replies; Windows runs of the changed scripts with the PP items.
- Follow-up (2026-10-23): Windows CI (`windows-build`, run 39888900692 on `91a61be`) failed `tests::autosave_recovery::a_close_waiting_for_a_start_up_open_that_fails_closes_at_once` ("no prompt pops up in a window that is closing": `Some(Recovery)`); it passes on Linux. Cause, from reading `src/operator/autosave.rs`: the close-wait task handles the failed start-up open (which sets `prompt_pending`), then takes `closing` and calls `window.remove_window()`, but the window goes only after the task returns, so a regular `poll` scheduled in between sees `prompt_pending` with `closing == None` and opens the Recovery prompt. Windows' dispatcher runs that poll first; Linux's does not, so the race is not reproducible here. Fix: `window_closing` is set just before `remove_window()` and keeps the prompt back from then on (the next start asks again, since nothing was recorded). Checks after the fix and the merge with `ccr-5e192d6c-emgkai`: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (524 + 320 + 25 + 9 + 4 + 14 + 0 + 9 + 20 + 1 + 0 + 0 + 20 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; every `scripts/*-x11.py` passed on private Xvfb displays (autosave 19 s, live-output 58 s, media-backend after linking the fetched LGPL development copy into this worktree, scenario-s01 45 s, scenario-s02 52 s, schedule 35 s, song-editor 63 s, song-import 33 s, songs-browser 45 s, themes 44 s). The Windows test run on the pushed head is the regression evidence for the race.

### scripture — M4-03 providers and M4-04 Scriptures tab (2026-10-22)
- State: M4-03 **implemented-unqualified** (all five checklist lines met on Linux; no Windows run). M4-04 **implemented-unqualified** for what is built (tab, preview/live, pagination, schedule snapshots, all listed tests); its EW observation line, the scripture theme and the Windows/1080p output inspection stay open. Branch `wave/scripture` (worktree `.claude/worktrees/wf_4eb25669-832-7`), Linux cloud container, no Windows machine.
- Decisions: (1) Bundled texts: public-domain KJV (OSIS) and WEB (USFX) from GitHub `seven1m/open-bibles` at commit `f257a3559025c3f873b48a75019f53a9354ed7de` (ebible.org is blocked); URLs, licence statements, source and bundle SHA-256s are in docs/scripture.md. 66-book canon only; WEB footnotes/headings dropped (faithful verse text). KJV 31,102 verses, WEB 31,098 (real versification differences: Luke 17:36, Acts 8:37, 15:34, 24:7 absent; Romans 14:24-26). Bundles are deflated text (`miniz_oxide` 0.8.9, already in the lock through png) built by `examples/scripture_bundle.rs` with the same importer users get, SHA-256 checked before use. (2) Storage: one SQLite file per translation `<profile>/Scripture/<ID>.selabible` (application id "SBIB", format 1, verses keyed `BBCCCVVV`, FTS5 external-content index); written to `.partial` and renamed; `open` checks id/version, quick_check, metadata, count and a content SHA-256, rebuilds a damaged FTS index, and a damaged bundled file is rebuilt from the bundle. No library schema migration (schema_version_used: none). (3) Provider worker thread with a 4-command queue, started the first time the tab is shown (installing both bundles took 1.1 s in a debug build); generation-tagged replies; typing advances the generation so a late passage never replaces newer state. (4) Versification: `Inventory` from the verses present; `passage::locate` (typed reference, verse-exact) and `passage::chapters` (translation switch, keeps the selection, names missing verses, refuses a missing book/chapter). (5) Importers OSIS/USFX/Zefania with the song importers' hardening (48 MiB, no DTD/entities, depth 64, 3M nodes, 40k verses, 4 KiB/verse, 64 warnings); imported text is marked restricted. (6) UI (UNOBSERVED best guesses, docs/reference-observations.md "Scriptures"): translations in the left column, Reference/Keyword switch, completion buttons (Tab takes the first), red/muted note, whole chapters listed with the reference selected, Shift range / Ctrl toggle / double-click Go Live, footer Import Bible… / Add to Schedule. Own focus handles and tab band 2500-2561 (no change to `Control`); new key contexts `SelaScriptureReference`/`SelaScriptureList`; 51 EN/ID messages. (7) Slides: 44-character word-wrapped lines, 6 lines a slide, breaks between verses (the renderer fits lines but does not wrap: an unwrapped first attempt drew one tiny line, seen in the X11 capture); per-slide footer = slide reference over "<name> (<ID>)" through the existing copyright-strip path (new `Item.footers`). (8) Schedule: `Entry.scripture: Option<Arc<Snapshot>>`, version = content hash; `.selaschedule` version 4 stores scripture items in place; songs-only files stay version 3 for older readers. Restricted text is stored with `redistributable:false`; stripping it from portable exports is M1-13 work (open).
- Changed existing code (minimal, append-style): `Operator.scripture`, `Item.footers` (dispatch uses it), `poll_scripture` in `poll`, tab 1 dispatch, left column/footer for tab 1 in layout.rs, `focus_entry` previews a scripture entry, save/open in schedule_files.rs, `ScheduleFile.scripture` (+ `Default`), three test/example `ScheduleFile` literals. Two existing tests adjusted without weakening: `newer_version_is_rejected_with_its_number` and `invalid_files_are_rejected_and_change_nothing` now use version 5 as "newer" (4 is readable now).
- Tests: lib `scripture::*` (47: import formats/limits/hostile files, store integrity/corruption/FTS rebuild/atomic rewrite, bundles incl. the shipped KJV/WEB, worker install/rebuild/damaged/import/queue bound, locate/chapters, labels, pagination, wrap), `schedule::tests::scripture_items_pin_their_snapshot_and_reopen_with_it`, `schedule_file::tests::{songs_only_schedules_stay_version_3_for_older_readers, scripture_items_round_trip_in_place_as_version_4, bad_scripture_items_are_refused_on_write_and_read}`. GPUI `tests::scripture` (10): typing_a_reference_previews_it_and_go_live_shows_it_with_its_reference, typing_in_the_box_never_triggers_live_shortcuts, malformed_or_missing_references_keep_the_previous_state, cross_chapter_ranges_and_nonconsecutive_selection, translation_switch_keeps_the_verses_and_reports_mismatched_canon, keyword_search_starts_at_two_characters_and_handles_empty_results, long_passages_split_into_readable_slides_in_order, corrupt_translation_files_are_rebuilt_or_listed_unusable, importing_a_user_file_adds_a_restricted_translation, scripture_schedule_items_are_pinned_snapshots_that_survive_save_and_reopen.
- Fail-without-change evidence: disabling the content-hash check failed `corrupt_files_are_refused_and_a_damaged_index_is_rebuilt`; disabling scripture decoding failed `scripture_items_round_trip_in_place_as_version_4`; disabling the scripture branch of `focus_entry` failed `scripture_schedule_items_are_pinned_snapshots_that_survive_save_and_reopen`; removing the generation advance and pending reset on typing failed `malformed_or_missing_references_keep_the_previous_state` (a late passage replaced the note); that test also failed before the fix was written, as did `translation_switch_keeps_the_verses_and_reports_mismatched_canon` before `passage::chapters` (a switch with a verse the new translation lacks was refused). Each mutation was reverted.
- Results (`CARGO_INCREMENTAL=0`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked -j 3 --lib --bins` lib 428 passed 4 ignored, bin 246 passed 3 ignored; `cargo test --locked -j 3 --test output_process --test transport_process` 9 passed 1 ignored, 4 passed 1 ignored; `cargo test --locked -j 3 --example <each>` for seed_library, scripture_bundle, search_bench, edit_song (0 tests), composition_spike 13, input_check 9, native_cues 19 (1 ignored), output_spike 1, video_spike 19, all exit 0; `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 -m ruff check scripts/scripture-x11.py scripts/scripture-windows.py` all passed. The all-targets test run was split by target because the shared disk was full (one link died with "ld terminated with signal 7 [Bus error]" at 53 MB free); every target of `--all-targets` ran. Native: `python3 scripts/native_x11.py serve --display :116` (Xvfb :116 1920x1080x24 -nolisten tcp + Openbox), then `python3 scripts/scripture-x11.py --display :116` PASS on the final tree too (bundled install 1.1-1.2 s, John 3:16-18 KJV live 83-101 ms after the click as observed by capture polling, typing kept the output, "John 3 has 36 verses in KJV", WEB live, "only begotten" first hit John 1:14 live by double-click, Add to Schedule made the schedule unsaved and Ctrl+Q asked first), artifacts `.amp/in/artifacts/scripture-x11/`; captures inspected (operator tab with translations, selection, note, keyword hits, schedule item; audience KJV/WEB slides readable on 5-6 lines).
- Unexecuted / open: `scripts/scripture-windows.py` NOT RUN (no Windows host; ruff-clean); EasyWorship 8.0.49 observations PP-BIB-01..10 (docs/parity-pending.md, Stream: scripture: layout, keyword rule, selection, slide flow and reference placement, errors, translation switch, schedule items, import); the per-slide reference strip is not drawn on the 624x351 xvfb audience window (`FOOTER_MIN_HEIGHT` 360); it is checked natively with `scripture-x11.py --kind dual` (1920x1080 audience; corrected in review round 1, the earlier "GPUI tests only" understated it); a scripture theme (EW Scripture Theme, fonts/background) is not implemented; Options › Scripture settings (verse numbers, abbreviations, flow, minimum size) are fixed defaults; restricted text in portable exports (M1-13); translations with more chapters than the KJV are unreachable by reference; drag of verses to the Schedule; no keyword-search latency budget measured beyond the X11 run.
- Next: run `scripture-windows.py` on Windows and observe PP-BIB-02..08 in EW 8.0.49; then a scripture theme and Options › Scripture (M4-04), and restricted-text handling in bundles (M1-13).
- Review round 1 (2026-10-23, fixer): State unchanged (M4-03/M4-04 **implemented-unqualified**). Fixed: (1) blocker: every catalog reply re-ran the typed reference, so the refresh after any Bible import (Ok or Err) replaced Preview, cleared the schedule selection and reset a Ctrl+click selection; now only the first catalog applies typed text, a later refresh changes the translation list only (and drops the shown rows if their translation became unusable, without touching Preview). (2) major: a translation switch with an empty selection, or after a keyword hit outside the shown reference, was refused with a wrong "Not in X:" note; now an empty selection selects the referenced verses in the new translation, and a Reference/Keyword switch forgets the shown reference. (3) major: the Import Bible… button and file prompt are tested in GPUI (one file, no folders, cancel imports nothing, disabled while importing) and natively (portal Open dialog in `scripture-x11.py`). (4) minor: a switch with no loaded reference or in Keyword mode now advances the generation, drops pending/queued replies and clears the old rows first, and a typed reference still loading is retyped in the new translation (no revert, no SYB text pinned as SYA). (5) minor: the comment claiming imported text stays out of schedule files is corrected (the restricted flag is recorded, not enforced; M1-13). (6) minor: worker problems are structured (`worker::Damage`: store error, bundle error, file name) and phrased in EN/ID (12 new `ScriptureWhy*` messages, `ScriptureNotInstalled` now used, no literal "stopped"); only parenthesized technical detail stays English. (7) minor: `scripture-x11.py` asserts the reference strip on a `dual` display (ink in the bottom 8.5 % band, found at y 988-1042 of 1080), the docstring/docs no longer say "GPUI only", the GPUI comment promising a Page Down step was corrected. The selection count got an AccessKit id/role (`scripture-selection-count`) so the native check can read it. docs/scripture.md's opening claim now lists what each level checks and what is unchecked (damaged-row red styling, native `.selaschedule` v4 save/reopen).
  - New tests (each failed with its fix reverted, re-run 2026-10-23): `tests::scripture::a_catalog_refresh_after_an_import_leaves_preview_and_selection_alone` (fails with the old always-rerun: Preview became John 3:16-18 (SYA)), `translation_switch_with_no_selection_or_after_keyword_hits` and `a_switch_while_a_reply_is_in_flight_is_not_reverted_or_mislabelled` (both time out with the old `choose_translation`/mode switch); `importing_a_user_file_adds_a_restricted_translation` now drives the button and asserts the prompt options; `corrupt_translation_files_are_rebuilt_or_listed_unusable` asserts the EN and ID damage wording; worker tests assert `Damage` values.
  - Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` lib 428 passed 4 ignored, bin 249 passed 3 ignored, output_process 9/1 ignored, transport_process 4/1 ignored, composition_spike 13, input_check 9, native_cues 19/1 ignored, output_spike 1, video_spike 19 (a first run had `tests::preflight_ui::oversize_photo_imports_as_a_downscaled_copy_with_a_note` hit its 10 s bound under load; it passed alone in 4.8 s and in the full re-run; untouched by this change); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/native_x11.py serve --display :116 &` then `python3 scripts/scripture-x11.py --display :116` PASS (go live 87 ms, WEB 73 ms, keyword 73 ms, import "Imported SZEF: 2 verses, 0 notes", row "SZEF · Synthetic Zefania (restricted)", selection after import "Verses selected: 1", no focus thieves); `python3 scripts/scripture-x11.py --kind dual` PASS (audience 1920x1080, strip box [748, 988, 1170, 1042], one logged audience focus theft, the known M1-10 Linux finding); `python3 -m ruff check scripts/scripture-x11.py scripts/scripture-windows.py` clean. Captures inspected: dual KJV slide with "John 3:16-17 / King James Version (KJV)" under the text; operator after import (SZEF row, hits and selection kept, footer message).
  - Unexecuted / open: `scripture-windows.py` still NOT RUN (no Windows host) and does not drive Import Bible… (the Windows common dialog step is to add with the first Windows run); damaged-row red styling has no test (visual only); native `.selaschedule` v4 save/reopen is in-process only; restricted text is still written into `.selaschedule` (M1-13); EW observations PP-BIB-01..10 (PP-BIB-07 and PP-BIB-10 gained the empty-selection, after-keyword and import-refresh steps).
  - Next: unchanged (Windows run and EW observations), plus the import step in `scripture-windows.py`.
- Review round 2 (2026-10-23, fixer, worktree `/home/user/wt/scripture`, Linux container, display :116): State unchanged (M4-03/M4-04 **implemented-unqualified**; Windows, EW 8.0.49 and physical output still open). Fixed: (1) minor: the `if !typed_pending` guard in `choose_translation` had no test; new `a_switch_while_a_typed_reference_loads_over_a_shown_one_keeps_the_typed_one` shows John 3:16 in SYA, types 3 John 1:14 and clicks SYB before the passage is polled (asserted: Preview still John 3:16 (SYA) at the click) and expects 3 John 1:14 (SYB). (2) minor: with no usable translation (current None), text typed in the box was dropped when an import later made one usable; now that refresh (before None, current Some) looks the box's text up and fills the list and selection, through a new `PendingPassage.preview = false` so Preview still changes only on an operator action (`scripture_text_applied`; `reference_typed`/`load_reference` take `preview`). Test `text_typed_with_no_usable_translation_applies_once_an_import_adds_one` (no bundles, damaged USER file, a song in Preview, "John 3:16" typed, SLAT imported: list 2 rows, John 3:16 selected, Preview still Signal Hymn). (3) minor: translation rows say "(damaged)" / "(restricted)" in words (new `Msg::ScriptureDamagedTag` "damaged"/"rusak"; `translation_label` gives the visible label and the aria-label the same text), damaged also stays red; asserted in `corrupt_translation_files_are_rebuilt_or_listed_unusable` (EN "USER (damaged)", ID "USER (rusak)") and `importing_a_user_file_adds_a_restricted_translation` ("SZEF · Synthetic Zefania (restricted)"), and natively (`scripture-x11.py` requires the SZEF row to end with "(restricted)"; capture `operator-imported.png` inspected: the visible row reads "SZEF · Synthetic Zefania (restricted)"). (4) minor: a second Import Bible… click while the file dialog is open asked again; `ScriptureTab.import_prompt` now disables the button and is cleared on pick, cancel or error; `importing_a_user_file_adds_a_restricted_translation` clicks twice before answering and asserts no second prompt is queued. (5) minor: backlog M4-04 "Inspect" line no longer says the 1080p strip is open (checked by `--kind dual`). (6) minor: `scripture-x11.py` now saves the schedule with Ctrl+S through the portal Save dialog (asserts version 4, kinds ["scripture"], translation KJV, the pinned verse text), quits unasked, deletes `KJV.selabible`, relaunches, reopens with Ctrl+O through the portal Open dialog, and sends the item live from its snapshot; `scripture-windows.py` gained Import Bible… (native Open dialog, "(restricted)" row, message, selection and audience unchanged) and Ctrl+S (version-4 file with the KJV item) steps (NOT RUN: no Windows host; ruff clean). Harness: `native_x11.App.drive_file_dialog` raised when GTK destroyed the dialog between `window_exists` and `geometry` (3 of 5 xvfb runs failed at the pre-existing Import step with "xwininfo -id 16777219 ... Bad Drawable"); a window that vanishes between the two queries now counts as closed (shared harness, 1 function); after it 6 of 6 xvfb runs passed and `schedule-x11.py --display :116` PASS. The reopened AT-SPI frame is found by title, which can trail the X title just after Ctrl+O (one dual run timed out); the script now waits for the title and retries the lookup.
  - Mutation evidence (each reverted): removing `!typed_pending` fails (1) (bounded wait for "typed reference in SYB"); removing the before-None lookup fails (2) (bounded wait); applying it with preview=true fails (2) at "only an operator action changes Preview"; removing the `import_prompt` guard fails `importing_a_user_file_adds_a_restricted_translation` at "one Import Bible… dialog at a time". `i18n::tests::all_lists_every_variant_exactly_once` count guard raised 531 -> 532 for the new message.
  - Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 428 passed 4 ignored; bin 251 passed 3 ignored, `tests::scripture` 15; output_process 9/1 ignored; transport_process 4/1 ignored; composition_spike 13, edit_song 0, input_check 9, native_cues 19/1 ignored, output_spike 1, scripture_bundle 0, search_bench 0, seed_library 0, video_spike 19); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `ruff check`/`ruff format --check` clean for `scripts/scripture-x11.py`, `scripts/scripture-windows.py`, `scripts/native_x11.py`. Native: `python3 scripts/native_x11.py serve --display :116 &`, then `python3 scripts/scripture-x11.py --display :116` PASS (final runs: install 0.91-0.98 s, go live 75-83 ms, "Imported SZEF: 2 verses, 0 notes", "Verses selected: 1" kept, saved {version 4, kinds [scripture], "John 1:14 (KJV)", KJV}, reopened title "Scripture service.selaschedule — Sela", item "John 1:14 (KJV)" live 73-83 ms, no focus thieves); `python3 scripts/scripture-x11.py --kind dual` PASS twice (strip [748, 988, 1170, 1042], reopened item live 319-348 ms; two logged audience focus thefts on Live output on, the known M1-10 Linux finding). Captures inspected: operator after import (SZEF row labelled restricted, hits and selection kept), reopened operator (schedule "Scripture service.selaschedule", item John 1:14 (KJV) live from its snapshot with KJV.selabible deleted).
  - Unexecuted / open: `scripture-windows.py` NOT RUN (no Windows host; its new Import/Save steps are unexercised); the red colour of a damaged row is still untested (the words are tested); restricted text is still written into `.selaschedule` (M1-13); EW observations PP-BIB-01..10 (PP-BIB-08 and PP-BIB-10 gained the save/reopen, typed-before-import, double-click and row-marking steps; docs/reference-observations.md "Translation rows" is an UNOBSERVED best guess).
  - Next: run `scripture-windows.py` on Windows and observe PP-BIB-01..10 in EW 8.0.49; then a scripture theme and Options › Scripture (M4-04), and restricted-text handling (M1-13).
- Integration (2026-10-23): merged into `ccr-5e192d6c-emgkai` as merge commit `922e628` ("Merge branch 'wave/scripture'", `--no-ff`, on top of the pr1-review/M1-12 tip `3e8650d`), then `87411fd` (fix(integration), harness). Conflicts, all resolved keeping both sides: `Cargo.lock` (regenerated with `cargo update -w`: sela gains `miniz_oxide` beside `libc`), `docs/e2e-linux.md` (script list: `scripture-x11.py` after `media-backend-x11.py`), `docs/parity-pending.md` (Windows run order: `scripture-windows.py` renumbered from row 23 to row 29), `docs/reference-observations.md` (scripture table appended after the media best guesses), `docs/work-log.md` (Current-state bullets and status rows of both sides; M4-03/M4-04 rows before M4-07, the branch's stale M1-16 row dropped for the newer one), `scripts/native_x11.py` (the integration branch's `viewable()` portal-dialog close kept; it covers the branch's vanished-window fix), `src/i18n.rs` (every variant of both sides in the enum, `ALL` and both tables; count guard 707), `src/tests.rs` (module list; the newer-file test keeps `VERSION + 1`), `src/operator/{layout,mod,polling,preview_live,schedule_files}.rs`, `src/schedule.rs`, `src/schedule_file.rs`. Version collision: the branch wrote scripture items as `.selaschedule` version 4, which the integration branch already uses for the themes item snapshot, so scripture items are now **version 5** (`SCRIPTURE_VERSION`) and a schedule without them is still written as version 4 (`SONGS_ONLY_VERSION`); renumbered in code, `src/schedule_file/tests.rs` (`songs_only_schedules_stay_version_4_for_older_readers`, `scripture_items_round_trip_in_place_as_version_5`, versions 3 and 4 with scripture are corrupt, 6 is newer), `tests::scripture`, `scripture-{x11,windows}.py`, docs/scripture.md, storage.md, themes.md, backlog.md, parity-pending.md. Semantic integration: (1) `Item` gained both the theme fields and `footers`; a scripture item shows the built-in look (`theme_key` 0) and its per-slide reference footer is always shown, while a song's strip follows the copyright options (`footer_shown`); (2) `Schedule::set_theme`/`capture_theme` skip scripture entries (Apply to Item counts songs only); a save sends only song entries' themes, aligned with the song versions; (3) autosave revisions are now recovery format **version 3**: an entry's `scripture` object embeds its pinned verses (checked against the pin; versions 1-2 still read), `Schedule::restore` takes them, and Restore asks the library only for song items; the 512 KiB revision limit now counts scripture verses (a larger schedule is refused with `Error::Budget` and reported; docs/autosave.md); `autosave-{x11,windows}.py` expect version 3. New tests: `autosave::tests::a_revision_keeps_scripture_items`, and theme-skip assertions in `schedule::tests::scripture_items_pin_their_snapshot_and_reopen_with_it`. Harness fix `87411fd`: `App.controls` re-reads the window title on every attempt; before it, `media-backend-x11.py` failed 3 of 4 runs at the Indonesian step (the title switched to "Sela — Pratinjau teknis" after the harness read the English one; the bundled Bible install at start-up widened that pre-existing race), after it 4 of 4 passed.
  - Gate on the merged tree (`87411fd`'s Rust is `922e628`'s): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (994 passed, 11 ignored: lib 557/5, bin 335/3, media_decode 25, output_process 9/1, transport_process 4/1, examples composition_spike 14, input_check 9, native_cues 20/1, output_spike 1, video_spike 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Every `scripts/*-x11.py` serially on Xvfb `:199` (`serve`, Openbox) with the final harness: autosave PASS (19 s), live-output PASS (57 s), media-backend PASS (19 s), scenario-s01 PASS (45 s), scenario-s02 PASS (49 s), schedule PASS (35 s), scripture PASS (47 s), song-editor PASS (63 s), song-import PASS (29 s), songs-browser PASS (41 s), themes PASS (43 s).
  - Unexecuted / open: no Windows script run (no Windows host), including `scripture-windows.py`, `autosave-windows.py` (now expects recovery v3) and `schedule-windows.py`; no native crash-restore run with a scripture item (headless tests only); scripture themes remain open (M4-04).

### stage-display — Stage (Foldback) output, M3-01/M3-02 + stage clock (2026-10-22)
- State: M3-01 and M3-02 **implemented-unqualified**; M3-05 **implemented-unqualified (stage clock only)**, the rest of M3-05 out of scope by the owner's Mission 2026-10 decision. Branch `wave/stage-display` from `ccr-5e192d6c-emgkai`, Linux container (4 CPUs, no Windows host). Design note: `docs/stage-display.md`. No SQLite schema change; settings stay version 1 (new optional `stage_*` keys).
- Decisions: (1) Stage output is a second renderer process `sela --stage` (same winit/wgpu renderer with `Config.stage`) behind its own `output::Supervisor`, epoch and receipts; nothing shared with the audience session. (2) Routing by cue kind: `PreparedCue::for_stage` carries a `stage::StagePanel`; wire kind 7 STAGE (v2) carries it; `audience::admits` makes the audience refuse stage cues and the stage refuse slides/logos (RenderFailed, scene kept); masks/logo go to the audience supervisor only. (3) The panel follows the operator's live request (the slide handed to the audience supervisor), computed from the live item's own snapshot: next slide, else "Next item · <title>" (live entry still scheduled), else "End of schedule". (4) The stage process draws the clock (chrono `Local`, "HH:MM") on a `stage-clock` worker that re-uploads only when the minute changes, so it does not depend on the operator UI; chrono 0.4.45 added as a direct dependency with a subset of the features gpui already enables (Cargo.lock gains one line, no new crate). (5) Three stage colours ride the compositor's existing fill/outline/shadow coverage channels: no shader change. (6) Options › Foldback (EW8-OBS-054/056) has the enable check box (default off), monitor rows (Automatic = neither primary nor the audience's monitor; no free monitor = stage not started, never mapped over the audience) and Sela's clock/next switches; Live menu gains Show Stage Output and Stage Message… (UNOBSERVED additions). (7) Best guesses recorded as EW8-OBS-STG-1..7 (stream-scoped IDs to avoid collisions) and PP-STG-01..09. (8) No new GPUI pattern: existing Sela menu/modal/TextInput/AccessKit code at zed `a846890`; CheckBox/RadioButton roles from accesskit 0.24.1 via GPUI `Role`.
- New/changed files: `src/stage.rs`, `src/audience/stage.rs`, `src/operator/stage.rs`, `src/tests/stage_display.rs`, `scripts/stage-display-x11.py`, `scripts/stage-display-windows.py`, `docs/stage-display.md`; small append-style edits in `src/{scene,transport,settings,audience,main,i18n,lib,tests}.rs`, `src/operator/{mod,options,menus,controls,layout,polling,preview_live}.rs`, `src/operator/dialogs/mod.rs`, `examples/native_cues.rs`, `scripts/native_x11.py` (stage title/rule, `App.stage`, `triple` kind), `Cargo.toml`/`Cargo.lock`, docs (backlog M3-01/02/05 progress and the M3-02 test box, reference-observations, parity-pending, delivery, e2e-linux). 33 new EN+ID strings; the existing Live-menu list test gained the two new row ids.
- Environment: `/` had 15–60 MB free (other worktrees fill it), so the seed copy failed. The build ran in a RAM target: `target` is a symlink to `/dev/shm/sela-stage-target`, a symlink farm of `/home/user/sela-seed-target` with every sela-owned output removed (so nothing writes through to the seed); ~1.9 GB of tmpfs. ImageMagick and the harness temp folders used `MAGICK_TEMPORARY_PATH`/`TMPDIR=/dev/shm/sela-stage-magick`.
- Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 twice in a row (lib 407 passed/4 ignored, bin 251 passed/4 ignored, output_process 9, transport_process 4, examples 13+9+25+1+19; 738 passed) — an earlier full run failed once in `the_stage_message_is_private_until_ok_and_never_reaches_the_audience` on the shared 10 s `settle` bound under machine load (each rig starts two child processes); the module now shadows `settle` with a 30 s bound, then 2 full runs and 3 bin-only runs passed; `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 -m ruff check` on the three changed/new scripts clean (the pre-existing EXE001 on `audience-smoke-windows.py` remains). Native: `python3 scripts/stage-display-x11.py --display :118` PASS on Xvfb `:118` 1920x1080 + harness Openbox rules (both outputs `window` 624x351); `python3 scripts/stage-display-x11.py --display :118 --tolerate-output-focus --out .amp/in/artifacts/stage-display-x11-triple` PASS on Xorg dummy `:118` with DUMMY0 primary/DUMMY1/DUMMY2 (audience covers DUMMY1, Automatic stage covers DUMMY2; the stage window took the X focus on map and was re-activated — the M1-10 winit X11 finding). A first triple run failed "the stage changed before Show on Stage" because the clock minute ticked; the script now compares the stage below the header. Xvfb with two `-screen`s gave winit one monitor per screen (documented in e2e-linux.md). Captures inspected: `stage-verse/-message/-bridge-restarted.png`, `audience-verse.png`, `operator-stage-lost.png`, `options-foldback.png` in `.amp/in/artifacts/stage-display-x11{,-triple}/` (white current text, sky-blue next, amber label/message, clock, no stage colour on the audience).
- Proof the key tests guard the change: with `poll_stage` stubbed to return false, `admits` always true and `stage_monitor_arg` falling back to the first non-primary monitor, `settings::tests::stage_monitor_never_defaults_onto_the_audience_monitor`, `audience::tests::each_renderer_admits_only_its_own_cue_kind` and all eight GPUI `stage_display` tests then present FAILED; restored, all pass.
- GPUI tests (`src/tests/stage_display.rs`): `stage_follows_go_live_and_next_with_its_own_receipts`, `a_lost_stage_never_touches_the_audience_and_restarts_alone`, `a_lost_audience_leaves_the_stage_running`, `the_stage_message_is_private_until_ok_and_never_reaches_the_audience`, `the_last_slide_points_at_the_next_schedule_item`, `arrangement_changes_and_repeated_choruses_keep_next_deterministic`, `a_rejected_panel_keeps_the_previous_one_and_reports`, `options_foldback_enables_routes_and_persists_the_stage`, `no_free_monitor_keeps_the_stage_off_instead_of_covering_the_audience` (fixture `fake_output_child` mirrors both renderers' routing rules and logs every scene).
- Unexecuted / open: `scripts/stage-display-windows.py` NOT RUN (no Windows host; PP-STG-08); EasyWorship Foldback observations (PP-STG-01..07); physical stage monitor, viewing distance, mixed DPI/refresh, hotplug (PP-STG-09); alternate/stream/house outputs and per-output themes (M3-03); safe-area/font settings and chords/notes for the stage; fake-clock/suspension tests of the clock; when the audience is lost Go Live is still refused, so the stage keeps its last panel until Live output restarts (documented).
- Next: run `stage-display-windows.py` on the three-monitor Windows setup, then the PP-STG EasyWorship observations; M3-03 builds on the same per-output supervisor pattern.

### stage-display review round 1 fixes (2026-10-23)
- State: M3-01/M3-02 stay **implemented-unqualified**, M3-05 **implemented-unqualified (stage clock only)**. Fixes for the independent review of `439c9e9` (1 blocker, 3 major, 7 minor) on `wave/stage-display`.
- Decisions / fixes: (1) Blocker: the stage panel is built from a new `Operator::on_air` (the item whose cue was last handed to the audience supervisor, set with `live_slide` in `poll_cues`, cleared with it on a new session), never from `live`, which already names a newly chosen item while its cue prepares; a new item whose cue fails or is refused changes neither output. (2) Live › Show Stage Output on a **lost** stage restarts it in one activation (stop + start), as the lost status says; on a running stage it stops it. (3) The stage panel is drawn in the live slide's resolved face (the `Arc` the audience cue carries; bundled DejaVu while nothing is live); the renderer replaces characters the face lacks with U+FFFD/`?` (`stage::covered`, invisible joiners/variation selectors dropped) and leaves a label, the message, the next slide or the clock blank if it still cannot be drawn, so only the current slide (already drawn by the audience in that face) can reject a stage cue. (4) Clock worker split into `clock_step` (clock source and upload injected) with fake-clock unit tests; `redraw_applies` holds the stale-redraw rule; `drawn` keeps a `Weak` so a freed frame's address is never mistaken for a new frame's. (5) A clock upload failure is sent to the event loop (`ClockResult::Failed`), which ends the stage process, so the operator shows the stage lost instead of a frozen clock. (6) Live output turned on before the stored settings or the monitor list landed: `follow_live_output` records a `recheck` and `stage_inputs_landed` (settings `Loaded`, `monitors_scanned`) takes the decision again; the note says "Detecting monitors…" meanwhile; a stage the operator stopped is not restarted by a later scan. (7) `start_stage` refuses while Live output is off ("Turn on Live output first") and the menu row is enabled only with Live output on (or to stop a running stage). (8) StageMonitorMissing EN/ID now says the stage stays off when no monitor is free. (9) Docs: the "string copies only" / per-panel font claims were corrected rather than the transport changed; the face bytes still travel per stage cue like audience cues (see Unexecuted). (10) Windows script step 4 now types and presses "b", checks that neither output changed before Show on Stage (stage compared below the clock header), and waits for the amber message band; steps 6 (Windows) and 7 (Linux) use one Show Stage Output activation and check the lost status names it. (11) New GPUI test for the Options monitor paths; the arrangement test now switches seven times in a row (six before the rebuild settled), which the M3-02 box text now states.
- Tests added: GPUI `a_new_item_that_fails_to_prepare_never_changes_the_stage` (failing then gated preparer; also asserts the stage face is the live slide's), `the_stage_never_starts_without_live_output`, `a_late_monitor_list_still_starts_the_stage`, `settings_that_load_after_live_output_still_start_the_stage` (gated settings loader through new `rig_with`), `options_monitor_changes_restart_the_right_outputs`; `a_lost_stage_never_touches_the_audience_and_restarts_alone` now restarts with one activation and asserts a new epoch; U `stage::tests::covered_substitutes_only_what_the_face_lacks`, `audience::stage::tests::a_glyph_the_face_lacks_never_costs_the_current_slide` (replaces `a_missing_glyph_rejects_the_stage_cue`, whose asserted behaviour was the major finding), `a_next_area_that_cannot_be_drawn_is_left_blank`, `audience::tests::the_clock_redraws_once_per_minute_and_retries_a_full_slot`, `a_clock_upload_failure_is_reported_never_a_silent_freeze`, `a_redraw_of_an_older_frame_never_replaces_the_applied_one`.
- Proof the tests guard the fixes: with `stage_panel` reading `live` again, `a_new_item_that_fails_to_prepare_never_changes_the_stage` FAILED ("assertion failed: stage_shows(o, \"Signal Hymn · Bridge\")"); with the Lost restart arm, the Live-output guard in `start_stage` and `stage_inputs_landed` disabled, `a_lost_stage_never_touches_the_audience_and_restarts_alone`, `the_stage_never_starts_without_live_output`, `a_late_monitor_list_still_starts_the_stage` and `settings_that_load_after_live_output_still_start_the_stage` FAILED (10 others passed); restored, all pass. The old renderer returned an error from `base()` for an uncovered next slide, which the new glyph test asserts against; the clock tests target the new `clock_step`/`redraw_applies` seams.
- Results (worktree `/home/user/wt/stage-display`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 408 passed/4 ignored, bin 260 passed/4 ignored, output_process 9/1 ignored, transport_process 4/1 ignored, examples 13+9+29(1 ignored)+1+19; 752 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 -m ruff check scripts/stage-display-x11.py scripts/stage-display-windows.py scripts/native_x11.py` and `ruff format --check` on the two stage scripts clean; `python3 scripts/native_x11.py serve --display :118` then `python3 scripts/stage-display-x11.py --display :118` PASS (lost status "Stage output lost: its process ended · Live › Show Stage Output restarts it", one activation started a fresh stage on Bridge; `stage-bridge-restarted.png` inspected: amber label and message, white bridge text, clock, "End of schedule").
- Unexecuted / open: `scripts/stage-display-windows.py` NOT RUN (no Windows host; PP-STG-08); the three-monitor Xorg dummy run (`--kind triple`) was not repeated this round (placement code unchanged; the reviewer's run on `439c9e9` passed); the face bytes still ride every stage cue and are encoded on the operator UI thread by `Frame::command` like audience cues (bounded by one in-flight plus one wanted cue and `MAX_SOURCE_BYTES`; a per-session face cache on the wire is the follow-up for both outputs); the next slide is drawn in the current slide's face (one face per stage cue); EW's foldback font is unobserved (PP-STG-02); midnight/wall-clock-correction/suspension clock tests; physical stage monitor (PP-STG-09).
- Next: run `stage-display-windows.py` on the three-monitor Windows setup, then the PP-STG EasyWorship observations.

### stage-display review round 2 fixes (2026-10-23)
- State: M3-01/M3-02 stay **implemented-unqualified**, M3-05 **implemented-unqualified (stage clock only)**. Fixes for the independent review of `67ea5f5` (4 minor, no blocker/major, no false claims) on `wave/stage-display`.
- Decisions / fixes: (1) Stage chrome legibility: the header label, clock, "Next ·" label and stage message are drawn in the bundled DejaVu Sans (`fonts::BUNDLED`, compiled into the stage process, so nothing extra travels), and in the live slide's face only when that face draws every character of the field and DejaVu Sans does not (a CJK title): new `stage::chrome_face`/`ChromeFace` in the framework-independent `stage` module, used by `audience::stage::Canvas::draw` through a new `Ink::{Slide, Chrome}` argument. The current and next slides stay in the live face. UNOBSERVED best guess recorded in EW8-OBS-STG-1 and PP-STG-02 (which now asks to check EW's clock/label font with a CJK and a symbol/script song). (2) Transport cost: measured and budgeted instead of guessed. New `examples/stage_frame_cost.rs` times `Frame::command` for one stage cue (the UI-thread part) with the bundled face and a face padded to `MAX_SOURCE_BYTES`; budget p95 <= 8 ms at 8 MiB, the example exits non-zero over budget. The per-session face cache on the wire stays the follow-up (not needed under the budget). (3) Doc comments that the round-1 insertions had displaced are back on `fit` (not `STAND_INS`) and `settings_loaded_for_options` (not `monitors_scanned`). (4) Clock failure path: the event loop's reading of the clock worker is now `next_clock` -> `ClockPoll::{Idle, Swap, End}` (exhaustive match in `poll_clock`, which calls `fail()` on `End`); a gone worker (`Disconnected`, e.g. a panic) now also ends the stage process instead of freezing the clock silently. The fake renderer children log the face (`face_field`: font version id, revision, face index) of every stage panel and audience scene, and `a_new_item_that_fails_to_prepare_never_changes_the_stage` compares the face the stage child received with the face the audience child received for the same slide (and with the live slide's resolved face), instead of the operator's own `stage_face()`.
- Tests added/changed: U `stage::tests::chrome_prefers_the_bundled_face_unless_only_the_live_face_draws_it`, `audience::stage::tests::chrome_stays_in_the_bundled_face_whatever_the_song_face` (live face DejaVu Sans Bold: label, message, next label and clock pixels identical to the regular-face cue, slide pixels differ), `audience::tests::the_event_loop_swaps_in_a_redraw_the_worker_sent`, `a_clock_upload_failure_is_reported_never_a_silent_freeze` extended through `next_clock` (Failed -> End, then Idle, dropped sender -> End); GPUI `a_new_item_that_fails_to_prepare_never_changes_the_stage` now asserts on the received faces.
- Proof the tests guard the fixes: with `Canvas::draw` using the live face for chrome again, `chrome_stays_in_the_bundled_face_whatever_the_song_face` FAILED (src/audience/stage.rs:380); with `next_clock` mapping `ClockResult::Failed` to `Idle` (the reviewer's "dropped arm"), `a_clock_upload_failure_is_reported_never_a_silent_freeze` FAILED (src/audience.rs:1675); restored, both pass. `fail()` itself needs a winit `ActiveEventLoop` and is not unit tested; an exiting stage process shown lost is covered by GPUI `a_lost_stage_never_touches_the_audience_and_restarts_alone`.
- Results (worktree `/home/user/wt/stage-display`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 409 passed/4 ignored, bin 262 passed/4 ignored, output_process 9/1 ignored, transport_process 4/1 ignored, examples composition_spike 13, input_check 9, native_cues 31/1 ignored, output_spike 1, video_spike 19; 757 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `cargo run --locked -j 3 --example stage_frame_cost` (dev profile, 4 vCPU container, 50 encodes each): bundled 0.72 MiB median 0.02 / p95 0.03 / max 0.04 ms, 8 MiB median 0.58 / p95 0.61 / max 0.67 ms, within the 8 ms budget; `python3 -m ruff check` and `ruff format --check` on `scripts/stage-display-{x11,windows}.py` clean; `python3 scripts/native_x11.py serve --display :118` then `python3 scripts/stage-display-x11.py --display :118 --out <scratch>` PASS (binary from this round's tree, sha256 15c30354…; `stage-message.png` inspected: amber "Signal Hymn · Chorus" label and "Wrap up by eleven" message, white "03:05" clock and refrain, sky-blue "Next · Bridge" and bridge text); captures deleted afterwards, :118 stopped.
- Unexecuted / open: `scripts/stage-display-windows.py` NOT RUN (no Windows host; PP-STG-08); three-monitor Xorg dummy run not repeated (placement unchanged); the encode budget is measured only in the Linux dev profile (not on Windows, not release; the pipe write and the renderer's parse run off the UI thread and are not timed); the face bytes still ride every stage cue (per-session face cache is the follow-up for both outputs); the chrome rule has no glyph fixture beyond DejaVu Sans/Bold (the live-face branch for a CJK title is unit-tested on the selection logic only); EW's foldback font for slides and chrome unobserved (PP-STG-02); midnight/wall-clock-correction/suspension clock tests; physical stage monitor (PP-STG-09).
- Next: run `stage-display-windows.py` on the three-monitor Windows setup, then the PP-STG EasyWorship observations.
- Integration (2026-10-23): merged into `ccr-5e192d6c-emgkai` as merge commit `ec8af3e` ("Merge branch 'wave/stage-display'", `--no-ff`, on top of the scripture integration tip `eac70a5`), then `775a57e` and `a1d52eb` (fix(integration)). Conflicts, all resolved keeping both sides: `docs/parity-pending.md` (Windows run order: `stage-display-windows.py` renumbered from row 23 to row 30), `docs/reference-observations.md` (stage best guesses appended after the scripture table), `docs/work-log.md` (Current-state bullets and status rows of both sides; M3-01/M3-02/M3-05 rows before M4-03, the branch's stale M1-16 row dropped for the newer one), `src/i18n.rs` (every variant of both sides in the enum, `ALL` and both tables; count guard 740), `src/tests.rs` (module list), `src/transport.rs` (STAGE kind 7 branch kept; the slide branch keeps the M1-08 trailing insets), `src/scene.rs` (`PreparedCue` carries both `insets` and `stage`), `src/settings.rs` (copyright and stage keys), `src/operator/{controls,layout,menus,mod,options}.rs`, `src/operator/dialogs/mod.rs` (Import Songs/Recovery and Stage Message dialogs; Options shows the Song tab only under Main Output and the static Foldback tabs under Foldback). `Cargo.lock` auto-merged (sela gains `chrono`), confirmed with `cargo update --workspace --offline` (0 packages changed). No version collision: no schema, `.selaschedule`, recovery or settings version was added (stage keys stay within settings version 1). Fixes: `775a57e` boxes the settings save job (the merged `Settings` tripped `clippy::result_large_err`); `a1d52eb` gives `on_air` its own `on_air_slide`, because scrutiny-r2 (SC2-LIVE-01) clears `live_slide` on a Go Live of another item, which blanked the stage panel while the audience still showed the old slide (`a_new_item_that_fails_to_prepare_never_changes_the_stage` FAILED on the merge, passes after). `docs/e2e-linux.md` lists `stage-display-x11.py` with the feature scripts.
  - Gate on the merged tree (`a1d52eb`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (1045 passed, 12 ignored; lib 570/5, bin 361/4); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Every `scripts/*-x11.py` serially on Xvfb `:199` (`native_x11.py serve`, restarted with the merged harness so Openbox has the stage-window rule; on the old serve `stage-display-x11.py` failed "Live output on: the operator lost the input focus"): stage-display PASS (32 s), autosave PASS (20 s), live-output PASS (59 s), media-backend PASS (20 s), scenario-s01 PASS (46 s), scenario-s02 PASS (52 s), schedule PASS (35 s), scripture PASS (50 s), song-editor PASS (64 s), song-import PASS (30 s), songs-browser PASS (43 s), themes PASS (43 s).
  - Unexecuted / open: no Windows script run (no Windows host), including `stage-display-windows.py` (PP-STG-08); three-monitor Xorg dummy run not repeated at integration; EasyWorship Foldback observations PP-STG-01..07 open.

### ci-windows-scripture-sync — Windows CI fix for the scripture store (2026-10-23)
- State: fix only; M4-03/M4-04 keep **implemented-unqualified**.
- Cause: Windows CI (`windows-build`, run 39895520671 on `eac70a5`) failed seven `scripture::store`/`scripture::worker` tests with `Io("Access is denied. (os error 5)")`, also for a first write to a fresh path. `store::write` fsynced the finished `.partial` file through `fs::File::open` (read-only); `File::sync_all` is `FlushFileBuffers` on Windows, which needs a handle with write access. Linux allows fsync on a read-only descriptor, so the Linux gate could not see it. Fix: open the partial file with `OpenOptions::new().write(true)` before `sync_all`. No other `File::open(..).sync_all()` exists outside `#[cfg(unix)]` (`autosave::sync_dir` is Unix-only).
- Also carried (cubic P3 on PR #1): `scripts/native_x11.py` `controls()` names, in its timeout message, the window title its last attempt compared against instead of re-reading it at failure time.
- Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --lib -j 3 scripture` 52 passed; `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 -m ruff check scripts/native_x11.py` clean; `python3 scripts/scripture-x11.py` PASS. The Windows test run on the pushed head is the evidence for the fix (not reproducible on Linux).
- Follow-up (2026-10-23): Windows CI on `d06e1d2` (run 39897907245) passed five of the seven and failed the remaining two with os error 32 ("being used by another process"). Both were tests deleting or overwriting a translation file they still held open, which Linux allows: `store::tests::corrupt_files_are_refused_and_a_damaged_index_is_rebuilt` kept the index-rebuilt `Translation` open across the next damage step, and `worker::tests::a_damaged_bundled_file_is_rebuilt_and_a_damaged_import_is_listed_unusable` removed `SYB` while the second `Worker`'s thread still had it open (`Drop` does not wait for the thread). Fix: drop the translation first, and a test-only `Worker::shut_down` that closes the command channel and joins the thread. Product paths were checked and never replace an open file (imports always choose a free id; a bundle is reinstalled only after its `open` failed and closed). Results: `cargo fmt --all` applied, then `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --lib -j 3 scripture` 52 passed; windows-gnu check exit 0. `Worker::shut_down` now fails the test if the worker thread panicked (cubic P3). Open: the Windows CI run on the pushed head is the verification for this fix (not reproducible on Linux).

### ci-windows-stage-scripture — Windows CI fixes for stage display and scripture GPUI tests (2026-10-23)
- State: test-only fixes; M3-01/M3-02 and M4-03/M4-04 keep **implemented-unqualified**. Source: Windows CI `windows-build` run 39899535945 (job 120237649178) on `483335c`, `cargo test --bin sela`: 355 passed, 6 failed.
- Scripture (`tests::scripture::scripture_schedule_items_are_pinned_snapshots_that_survive_save_and_reopen`, os error 32 at the `remove_file` of `SYA.selabible`): the operator's scripture worker thread keeps every catalogued translation open (`Provider::open`), and Windows refuses to delete a file another handle has open without `FILE_SHARE_DELETE` (SQLite's Windows VFS opens without it). Fix: the test first calls a new `#[cfg(test)] ScriptureTab::close_translation_files`, which takes the worker and joins its thread through `Worker::shut_down` (now `#[doc(hidden)] pub` so the binary's tests can reach it; it was `cfg(test)` in the library only). No sleeps. Product check: Sela itself never deletes or replaces a translation or schedule file it holds open. The scripture worker starts once per run and is never restarted; `refresh` drops every open translation (`open.clear()`, and a failed bundle check drops its `Translation` inside `filter`) before `install` renames a rebuilt bundle into place; imports always pick a free id; reopening a schedule reads only the `.selaschedule` (snapshot verses, closed after `read`); schedule saves and autosave write a temporary file, close it and rename. Recorded limitation, not changed: on Windows a user cannot delete or replace a translation file in Explorer while Sela runs with the Scriptures tab started.
- Stage (five `tests::stage_display` tests timing out on "bounded wait for <slide> on stage"): cause in the test fixture, not the product. Both fake children appended to one shared `outputs.log`, and `writeln!` writes the text and the newline as two separate appends. The audience and stage cues of one Go Live / Next are sent in the same poll, so the two children log at nearly the same moment; with audience-text, stage-text, newline, newline the stage line ends up glued to the end of an audience line, `lines(log, "stage")` no longer sees it, and `wait_for_panel` times out although the stage applied and acknowledged the panel. The pattern fits exactly: every failing wait follows an action that cues both outputs (Go Live, Next), every passing `wait_for_panel` on Windows follows a stage-only send (idle panel, restart, message), and settles on the operator's stage receipt (`stage_shows`) after the same Go Live passed on Windows (`a_rejected_panel_keeps_the_previous_one_and_reports`, `options_monitor_changes_restart_the_right_outputs`). Reproduced on Linux by widening the window (a 3 ms sleep between text and newline in `fake_output_child`): `stage_follows_go_live_and_next_with_its_own_receipts` failed with the same "bounded wait for verse on stage" and others with mangled lines. Windows' slower file open/write is the presumed reason the window is hit there; confidence in the mechanism is high, in the Windows timing explanation medium (no Windows host).
- Stage fix: one log file per role (`log_of`, `outputs/audience.log` and `outputs/stage.log`), and each line written with a single `write_all` of text plus newline (a restarted stage child may overlap the old one being killed). With the same 3 ms split-write simulation on top of the fix, all 14 stage tests passed three runs in a row. Kept permanently: a stage `settle` timeout now reports both supervisors (status, live receipt, in flight, wanted, rejected, counters), what is on screen, the stage panel the operator wants and the Live pane's stage lines; `wait_for_panel` adds the last 10 lines of each role's log.
- Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (1045 passed: lib 570, bin 361, integration and examples 114); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0.
- Unverified until the next Windows CI run: both fixes (not reproducible on Linux as such). Expected: `cargo test --bin sela` with 0 failures. If a stage test still times out, its message now shows whether the stage child logged the panel (stage log tail), whether the operator got the receipt (`stage: ... live Confirmed`, `applied` count, stage lines "showing ..."), or whether the cue never left the operator (`wanted`/`in flight`, rejected, a `Lost` status).
- Next: read the Windows CI result for this head; then the open stage/scripture Windows qualification items stay as recorded above.

### fuzz-audit — M5-02 data safety, fuzzing and dependency audit slice; M5-07 notices (2026-10-22)
- State: M5-02 **active** (this slice implemented-unqualified: Windows run, state-machine fuzzing and the durable-write interruption audit remain); M5-07 stays **planned** with its notices slice done. Linux cloud container (4 CPUs), Rust 1.98.1, branch `wave/fuzz-audit` from `ccr-5e192d6c-emgkai`. No UI change, so no GPUI test, x11 or Windows script (the brief's E2E for this stream is the fuzz/property suites plus `scripts/audit.sh`). No schema change.
- Environment blocker met and worked around: the shared disk was full (`/` 16-50 MB free) when the stream started, so the seed target could not be copied. Builds ran with `CARGO_TARGET_DIR=/dev/shm/sela-fuzz-target` (a tmpfs copy of the seed without the `sela` artifacts) and `CARGO_INCREMENTAL=0`; cargo-deny and its advisory database also live in `/dev/shm` (`SELA_TOOLS_DIR`, `SELA_ADVISORY_DB`).
- Decisions: (1) Fuzzing is `src/fuzz/` (a `#[cfg(test)]` module of the library, one file per target: songs, scripture, schedule, codecs, settings, images, media, transport), built on proptest with a fixed seed per suite (`PROPTEST_RNG_SEED`/`PROPTEST_CASES` override), failure persistence in `tests/fixtures/fuzz/<suite>.proptest-regressions`, a retained corpus in `tests/fixtures/fuzz/<target>/` replayed every run and used as mutation seeds, structure-aware token soups, byte mutations (flip, insert token, remove, repeat, truncate; PNG CRCs repaired), and worst-case inputs at each size limit with a time bound. cargo-fuzz needs nightly and is documented as a later option (docs/security.md). (2) The invariant that found most defects: every accepted song file must convert into a song `Song::validate` accepts, so one shared `ParsedSong::fit_to_model` (text budget 240 KiB, field limits cut at a grapheme boundary, expanded order capped at 512 slides, `TooLarge` for one oversized slide) now ends OpenSong, ChordPro and EasyWorship-words parsing; OpenLyrics already applied the same limits and now shares the constants and `cut`. (3) `settings.txt`: a bare CR ends a line; values with control characters are ignored on load and left out on save instead of being rewritten; reads are capped at 1 MiB. (4) SQLite: `SQLITE_DBCONFIG_DEFENSIVE` on and `SQLITE_DBCONFIG_TRUSTED_SCHEMA` off on every connection; backup/restore refuse a copy with any trigger or view (`Unsupported`). (5) `#![deny(unsafe_code)]` on the library and binary; the one unsafe block (`src/timer.rs`, `timeBeginPeriod`) carries `#[allow(unsafe_code)]` and is listed in docs/security.md. (6) `deny.toml`: AGPL-compatible license list (GPL-2.0-only excluded, `bzip2-1.0.6` only for `libbz2-rs-sys`), all advisories including unmaintained/unsound denied except six documented unmaintained notices, bans on TLS/HTTP stacks, sources limited to crates.io, the zed-industries org and proptest's pinned git; cargo-deny's `wildcards` is allowed because it counts git dependencies as wildcards, and `scripts/audit.sh` enforces exact `=` pins and git `rev`s itself. (7) Notices: `scripts/third_party_notices.py` over `cargo metadata --filter-platform x86_64-pc-windows-msvc`, normal dependencies only, license files copied verbatim and deduplicated, SPDX standard texts (license list 3.27.0, `scripts/notices/spdx/`) for the 21 crates without a license file, plus the GPUI/Zed Apache notice, the embedded DejaVu fonts license and the FFmpeg LGPL entry with a source pointer; deterministic output, `--check` mode.
- Findings fixed (docs/security.md SEC-01..07), each with a regression test that fails without the fix, checked by reverting the fix and restoring it: SEC-01/02/03 OpenSong, ChordPro and EW words accepted songs that could never be stored (no text budget; unbounded title/authors/copyright/labels; order expanding past 512 slides) — with the three `fit_to_model` calls removed, `import::tests::{large_files_are_cut_to_the_text_budget_and_stay_storable, long_metadata_is_cut_at_a_grapheme_boundary_and_reported, the_order_is_cut_at_the_arrangement_slide_limit}`, `fuzz::songs::{worst_case_inputs_finish_within_their_time_bound, retained_corpus_replays_cleanly, chordpro_mutations_keep_the_import_invariants}` and `import::chordpro::tests::limits_hold_on_pathological_input` fail (8 failed). SEC-04 MissingTitle dropped behind 64 other warnings — `import::tests::missing_title_survives_a_full_warning_list` fails without `warn_missing_title`. SEC-05/06 settings — with the old `lines()` split, stripping render and unbounded reads, `settings::tests::{a_bare_cr_ends_a_line_and_never_joins_a_path, values_with_control_characters_are_left_out_not_rewritten, an_oversized_file_is_unreadable_and_kept_aside}` and `fuzz::settings::{control_characters_never_change_a_saved_value, mutated_files_load_or_fall_back, retained_corpus_replays_cleanly}` fail (6 failed). SEC-07 — without `check_schema_objects`, `storage::tests::restore_refuses_a_library_with_triggers_or_views` fails. Two existing tests changed expectation because a 1 MiB single slide is now `TooLarge`: `chordpro::limits_hold_on_pathological_input` keeps its linear-time bracket check at the budget size and now also asserts the whole-file case is `TooLarge`; `ew_rtf::largest_font_tables_decode_in_linear_time` keeps the 1 MiB-class font table and stops the escape run at the budget, asserting the decoded count. No panic, hang, stack overflow or memory-safety issue was found.
- New: `src/fuzz/{mod,songs,scripture,schedule,codecs,settings,images,media,transport}.rs` (43 tests), corpus `tests/fixtures/fuzz/` (chordpro and opensong warning-cap files, schedule, settings and scripture seeds, two proptest regression files), `deny.toml`, `scripts/audit.sh` (`--tools-only`, `--fuzz [CASES]`), `scripts/third_party_notices.py`, `scripts/notices/spdx/{MIT,Apache-2.0,CC0-1.0,Zlib}.txt`, `THIRD-PARTY-NOTICES` (429 crates, 240 distinct license texts), `docs/security.md`; changes in `src/import/{mod,openlyrics,opensong,chordpro,ew_rtf}.rs`, `src/settings.rs`, `src/storage.rs` (hardening, `encode`/`decode` of the song payload made `pub(crate)` for the codec suite), `src/images.rs` (`inspect` made `pub(crate)`), `src/images/downscale.rs`, `src/lib.rs`, `src/main.rs`, `src/timer.rs`; docs/import.md shared rules, docs/storage.md hardening, backlog M5-02/M5-07 notes, parity-pending PP-FUZZ-01/02.
- Results: `cargo fmt --all -- --check` exit 0. `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; the same with `--target x86_64-pc-windows-gnu` exit 0 (removing the timer allow fails it with "usage of an `unsafe` block", checked). `cargo test --locked --all-targets -j 3` exit 0: lib 447 passed + 4 ignored, bin 236 + 3 ignored, output_process 9 + 1, transport_process 4 + 1, examples 13 + 9 + 19 (+1 ignored) + 1 + 19 (757 passed; baseline 706). `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Fuzz campaign: default suites 43 passed; `PROPTEST_CASES=8192 cargo test --locked -j 3 --lib -- --test-threads 4 fuzz::` 43 passed in 15.1 s; `PROPTEST_RNG_SEED={11,22,33} PROPTEST_CASES=20000` 43 passed each (30.1 s, 31.8 s, 32.0 s). `TMPDIR=/dev/shm SELA_TOOLS_DIR=/dev/shm/sela-fuzz-tools SELA_ADVISORY_DB=/dev/shm/sela-fuzz-advisory-dbs bash scripts/audit.sh`: cargo-deny 0.20.2 (release binary, SHA-256 checked) `advisories ok, bans ok, licenses ok, sources ok`, pin check ok, `THIRD-PARTY-NOTICES is up to date (796690 bytes)`, `audit: PASS`; 41 duplicate-version warnings (GPUI/wgpu/image). Negative run with the notices tampered, `serde_json` unpinned and `Unicode-3.0` removed from the allow list: `licenses FAILED` (19 rejected), `unpinned dependency serde_json ^1.0.151`, `THIRD-PARTY-NOTICES check FAILED`, exit 1; files restored. First audit findings resolved: 5 "wildcard" git dependencies (policy change above), six unmaintained notices (accepted with reasons in deny.toml), two unused license allowances removed. `python3 scripts/third_party_notices.py --check` up to date twice in a row (reproducible). `python3 -m ruff check scripts/third_party_notices.py` clean; `shellcheck scripts/audit.sh` clean.
- Unexecuted / open: no Windows run (no Windows host): the fuzz suites and `scripts/audit.sh` under Git Bash (PP-FUZZ-01) — the Windows-only UNC/device branches of `media::ffmpeg::check_input` run only there. State-machine sequence fuzzing (delivery lanes, schedule editing) not started. Interruption/disk-full/corruption audit across every durable write boundary not done beyond existing storage process-abort tests. Schedule-file memory amplification (up to about 270 MB of serde values for an 8 MiB hostile file, on the worker) accepted and documented, not measured. Fuzz targets for the EW library importer, scripture modules and bundles wait for that code. FFmpeg source offer (L) and shipping the notices in the installer belong to M5-07 (PP-FUZZ-02). cargo-fuzz not used (nightly).
- Next: run PP-FUZZ-01 on Windows; fuzz the delivery and schedule state machines with proptest command sequences; audit interruption and disk-full at each durable write (imports, migrations, restore, cache publication); add `src/fuzz/` targets as the EW library, scripture module and bundle importers land; regenerate `THIRD-PARTY-NOTICES` with every dependency change.

### fuzz-audit — installed-font target and image corpus (2026-10-22)
- State: M5-02 stays **active**; follow-up slice to the record above, same branch and container.
- New: `src/fuzz/fonts.rs` (4 tests) builds minimal TrueType fonts (`head`, `hhea`, `maxp`, `name`) with arbitrary family name records and scans them with `fonts::Catalog::scan`, plus mutated DejaVu, small font and collection files; invariant: every listed family can be chosen (its `SlideFormat` is valid) and resolves through `Fonts::resolve`. Image corpus `tests/fixtures/fuzz/images/` (valid 2x2, edge 16385, budget 4096x4097, 16-bit 3000x3000 with no data, APNG, truncated PNG, JPEG frame header declaring 65535x65535), all original bytes written by a script.
- Finding SEC-08: a font whose family name holds control characters (for example `"\u{7f}DJGhzcA"`, minimized by proptest) was listed in the font list, but the slide format naming it is invalid, so choosing it could never be saved. `fonts::family_of` now skips such name records; another usable record still lists the face. Test `fuzz::fonts::a_family_name_with_control_characters_is_not_listed`; with the check removed it and `listed_font_families_can_always_be_chosen` fail (2 failed), restored they pass. Seed kept in `tests/fixtures/fuzz/fonts.proptest-regressions`.
- Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 451 passed + 4 ignored, bin 236 + 3, output_process 9 + 1, transport_process 4 + 1, examples 13 + 9 + 19 (+1) + 1 + 19: 761 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `PROPTEST_CASES=2000 cargo test --locked -j 3 --lib -- fuzz::fonts` 4 passed (1.8 s); `PROPTEST_CASES=20000 ... fuzz::images` 4 passed (1.1 s); `scripts/audit.sh` PASS (no dependency change).
- Unexecuted / open: as in the record above (Windows run PP-FUZZ-01, state-machine fuzzing, durable-write interruption audit).
- Next: as in the record above.

### fuzz-audit — review round 1 fixes (2026-10-23)
- State: M5-02 **active** and M5-07 **active** (the reviewer's correction: its notices slice is delivered, implemented-unqualified); both slices stay implemented-unqualified until the Windows runs (PP-FUZZ-01/02). Same container, branch `wave/fuzz-audit`, worktree `/home/user/wt/fuzz-audit`.
- Blocker 1 (branch behind the integration base): merged `ccr-5e192d6c-emgkai` three times as the base moved (775a57e, 2523643, then 483335c, a test-only scripture worker change merged after the gate below: `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0 and `cargo test --locked -j 3 --lib scripture` 60 passed on the result). Conflicts: `src/settings.rs` (base keys kept; `stage_monitor` now follows SEC-05: control characters ignored on load, left out on save, instead of stripped) and this file. The base's four unsafe sites in `src/media/process.rs` got `#[allow(unsafe_code)]` on the narrowest item and are listed as U-02..U-04 in docs/security.md ("Unsafe code", now with IDs); `src/timer.rs` no longer claims to be the only unsafe code. Semantic conflict: the base's `a_song_over_the_library_limit_is_not_staged` expected `TooLargeForLibrary`, but SEC-01 cuts every parsed song to the library limits, so it became `a_song_over_the_library_limit_is_cut_to_fit_and_reported` (staged, `LimitReached`, encoded size within `MAX_SONG_BYTES`); docs/import.md says the error is now a fallback.
- Blocker 2 (SEC-09): a title of one grapheme over 1,024 bytes was cut to "" with no `MissingTitle`. `ParsedSong::keep_title_usable` (fit_to_model and OpenLyrics) clears a title cut to nothing or blank and reports `MissingTitle` first. Tests `import::tests::a_title_cut_to_nothing_is_reported_missing` (ChordPro/OpenSong/OpenLyrics) and fuzz `grapheme_heavy_titles_keep_the_import_invariants`; corpus `tests/fixtures/fuzz/{chordpro,opensong,openlyrics}/title-one-long-grapheme.*`. With the check disabled 5 tests failed (the new two, the corpus replay and both mutation suites, whose minimized cases are kept in `songs.proptest-regressions`).
- Major (scope against the base): new suites `src/fuzz/bibles.rs` (OSIS/USFX/Zefania import, bundle decode, translation files; corpus `tests/fixtures/fuzz/bibles/`) and `src/fuzz/recovery.rs` (autosave revisions with recomputed checksums, session file); extended `codecs` (themes, margins), `schedule` (v4 themes, v5 scripture items), `media` (ffprobe reports, showinfo frames), `settings` (all sorts, copyright and stage keys), `songs` (File › Import staging), `transport` (stage frame seed, panel bounds). Findings fixed: SEC-10 `Duration::from_secs_f64` panics on huge report durations and frame gaps (`a_duration_past_the_duration_range_is_unknown_not_a_panic`, `huge_frame_times_saturate_instead_of_panicking`; the generated media suites fail without it), SEC-11 a verse cut at 4 KiB kept a trailing space (`a_verse_cut_at_the_limit_ends_without_a_space`), SEC-12 scripture SQLite not hardened and triggers/views accepted (`foreign_schema_objects_are_refused_and_connections_hardened`), SEC-13 the already-running notice read `settings.txt` unbounded (`instance::tests::the_notice_reads_the_language_within_the_settings_bound`, new `settings::peek`). Each was checked failing with its fix reverted, then restored. docs/security.md entry points, fuzz table, findings and residual risks (Bible module memory) updated.
- Minor fixes: `copy_new`'s private copy and the pre-migration backup source are hardened (`open_private_copy`, `open_untrusted_read_only`; `storage::tests::connections_to_untrusted_files_are_hardened` fails without); the transport size test now feeds a stream long enough for the body and requires `InvalidData` (fails with the bound removed); `largest_font_tables_decode_in_linear_time` again decodes a whole 1 MiB escape run (refused as `TooLarge` within the module bound) besides the in-budget run; `third_party_notices.py` separates `WITH` exceptions and fails when an exception text is missing (`scripts/test_third_party_notices.py`, run by `scripts/audit.sh`); docs/storage.md lists every hardened connection.
- Results (worktree, load average 6-7 from parallel streams): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3 --no-fail-fast` exit 0 (lib 654 + 5 ignored, bin 362 + 4, media_decode 25, output_process 9 + 1, transport_process 4 + 1, examples 14 + 0 + 9 + 32 (+1) + 1 + 0 + 0 + 0 + 0 + 20: 1,130 passed); an earlier full run under heavier load failed two base timing tests (`tests::autosave_recovery::a_close_waiting_for_a_start_up_open_that_fails_closes_at_once`, `output_process::startup_and_acknowledgment_deadlines_make_output_unknown`) that pass in isolation (12/12, 3/3) and in the run above; `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; fuzz campaign `PROPTEST_CASES=8192 cargo test --locked -j 3 --lib -- --test-threads 4 fuzz::` 70 passed (30.7 s) and `PROPTEST_CASES=20000 PROPTEST_RNG_SEED={11,22,33}` 70 passed each (62.7 s, 63.5 s, 63.6 s); `SELA_TOOLS_DIR=<scratch> SELA_ADVISORY_DB=<scratch> bash scripts/audit.sh` exit 0 (cargo-deny 0.20.2 fetched and checked; advisories, bans, licenses, sources ok; 41 duplicate warnings; notices up to date, 796,690 bytes; `audit: PASS`); `python3 -I scripts/test_third_party_notices.py` 4 OK; ruff and shellcheck clean. Native X11 on private Xvfb `:119` with the harness Openbox config (binary sha256 `43847bad8d435910…`): `autosave-x11.py` PASS (26 s; covers the already-running notice), `scripture-x11.py` PASS (50 s; bundle install into hardened translation files, Import Bible…), `song-import-x11.py` PASS (32 s). A first `scripture-x11.py` run under a plain `openbox` (no harness rules) failed on the audience window taking focus; that was the setup, not the code.
- Unexecuted / open: Windows runs (no Windows host): PP-FUZZ-01 now also asks to confirm the Job Object path (U-04) leaves no `ffmpeg.exe` after Sela is killed; PP-FUZZ-02 unchanged. No EasyWorship behavior changed, so nothing new to observe in EW 8.0.49. State-machine fuzzing, the durable-write interruption audit and an EW-library fuzz target remain (M5-02). A 48 MiB Bible module is not timed in the debug suite (worst cases are 8 MiB).
- Next: PP-FUZZ-01/02 on Windows; delivery and schedule state-machine fuzzing; interruption/disk-full audit per durable write; regenerate notices with every dependency change.

### fuzz-audit — review round 2 fixes (2026-10-23)
- State: M5-02 **active** and M5-07 **active**, both slices implemented-unqualified until the Windows runs (PP-FUZZ-01/02). Same container, branch `wave/fuzz-audit`, worktree `/home/user/wt/fuzz-audit`, private display `:119`. Six minor findings, all fixed; no blocker or major. No UI or EasyWorship behavior change, so nothing new for docs/reference-observations.md or docs/parity-pending.md.
- Storage call sites: `copy_new` records (test builds only, a thread-local) whether its source and its private copy are hardened; `storage::tests::every_copy_reads_and_writes_through_hardened_connections` runs a backup, a restore and a schema-1 migration and requires both hardened each time. Checked failing with the private copy back on `Connection::open` (`backup: left [(true, true), (false, false)]`) and with the migration source back on a plain `open_with_flags` (`pre-migration backup: left [(false, false), (true, true)]`), then restored. docs/storage.md names both tests.
- SEC-11 worst case: the fuzz `bibles` "long verse" unit is now the 4-byte "and ", so the 4,096-byte cut ends right after a space. With the `trim_end` in `scripture::import::cut` removed, `worst_case_bibles_finish_within_their_time_bound` fails (`import: untrimmed VerseId { book: Book(1), chapter: 1, verse: 1 }`); restored. docs/security.md says why.
- Session files: `fuzz::recovery::check_session` (used by the property and a new table test `incomplete_or_unknown_session_files_are_not_read`) requires an accepted session to hold exactly the file's format, version, known state, times and a complete `uncommitted` block or null (independent `serde_json` oracle), and to write back identically through `encode_session` (now `pub(crate)`); a third seed has an incomplete `uncommitted` block. With `items` defaulted to 0 in `decode_session` both tests fail; the property's minimized case (a key spelled `{items`) is kept in `tests/fixtures/fuzz/recovery-session.proptest-regressions`. docs/security.md fuzz table gives the invariant.
- Unsafe lint: `Cargo.toml` `[lints.rust] unsafe_code = "deny"` covers examples and integration tests too (the crate-root attributes stay). With an `unsafe {}` added, `cargo check --example video_spike` and `--test transport_process` fail (`error: usage of an unsafe block`); without the table the example check finished. docs/security.md "Unsafe code" corrected.
- Notices: `third_party_notices.py` `satisfied()` evaluates the SPDX expression (`AND` needs every term's text, `OR` one alternative, `AND` binds tighter, `/` is `OR`, unparsable is not covered); `standard_texts` fails when the texts present do not cover it. New unit checks `test_every_and_term_needs_its_text`, `test_one_or_alternative_is_enough` (2 of 6 fail on the previous script). `scripts/audit.sh` keeps the unit check output and prints it to stderr on failure (checked with a failing stand-in command).
- Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3 --no-fail-fast` exit 0 (lib 656 + 5 ignored, bin 362 + 4, media_decode 25, output_process 9 + 1, transport_process 4 + 1, examples 14 + 0 + 9 + 32 (+1) + 1 + 0 + 0 + 0 + 0 + 20: 1,132 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 -I scripts/test_third_party_notices.py` 6 OK; ruff and shellcheck clean; `bash scripts/audit.sh` exit 0 (advisories, bans, licenses, sources ok; notices up to date, 796,690 bytes; `audit: PASS`); `PROPTEST_CASES=4000 PROPTEST_RNG_SEED=4471 cargo test --locked -j 3 --lib fuzz::` 71 passed (36.0 s). Native X11 on `:119` (`native_x11.py serve`): `autosave-x11.py` PASS, `song-import-x11.py` PASS.
- Unexecuted / open: unchanged from round 1 (Windows runs PP-FUZZ-01/02, no Windows host; state-machine fuzzing; durable-write interruption audit; EW-library fuzz target; 48 MiB Bible module timing).
- Next: PP-FUZZ-01/02 on Windows; delivery and schedule state-machine fuzzing; interruption/disk-full audit per durable write.
- Integration (2026-10-23): merged into `ccr-5e192d6c-emgkai` as `7a8002a` (`Merge branch 'wave/fuzz-audit'`, base `c0df21e`). One conflict, `docs/work-log.md` (the base's `ci-windows-stage-scripture` record and this stream's four records at the same place): both kept. No SQLite schema, `.selaschedule` or settings format version added, so nothing renumbered; `Cargo.lock` unchanged (proptest was already locked). Gate on the merged tree: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (1132 passed, 12 ignored: lib 656/5, bin 362/4, media_decode 25, output_process 9/1, transport_process 4/1, examples composition_spike 14, input_check 9, native_cues 32/1, output_spike 1, video_spike 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Every `scripts/*-x11.py` serially on Xvfb `:199` (`native_x11.py serve`, Openbox rc identical to the merged harness's): autosave PASS (20 s), live-output PASS (56 s), media-backend PASS (19 s), scenario-s01 PASS (45 s), scenario-s02 PASS (50 s), schedule PASS (34 s), scripture PASS (48 s), song-editor PASS (63 s), song-import PASS (29 s), songs-browser PASS (41 s), stage-display PASS (31 s), themes PASS (44 s). Windows qualification (PP-FUZZ-01/02) stays open.
  - Windows evidence (added 2026-10-23): the merge base `c0df21e` passed `windows-build` on one run (39902216496) and failed on another of the same commit (39902221215: `autosave_recovery::a_close_waiting_for_a_start_up_open_that_fails_closes_at_once` and `quit_with_dont_save_flushes_the_last_state_and_marks_the_run_clean`, timing races); the merged head `c9685d3` failed `windows-build` twice on the first of those (`--bin sela` 361 passed, 1 failed). The fuzz-audit changes themselves have no Windows-only failure; the races are fixed in the ci-windows-autosave-races record and the merged tree's Windows pass is still pending.

### ew-library-import — M4-07 EasyWorship 6-8 library importer (2026-10-23)
- State: M4-07's EasyWorship library child **implemented-unqualified** (parent M4-07 stays **active**). Fix round for the independent review that found the branch empty (blocker: no commits beyond `d06e1d2`; major: empty parity section, no work-log record). The stream's first session left no commit and no record; the cause is unknown and nothing was recoverable (clean worktree, no stash, reflog only the branch creation), so this round implements the whole brief.
- Scope / files: `src/import/ew_library.rs` (new: locate, private copies, `UTF8_U_CI`, schema checks, row read, mapping, staging), `src/import/ew_fixture.rs` (new: synthetic `Songs.db`/`SongWords.db` builder shared by lib and GPUI tests through `#[path]`), `src/import/{batch,worker,mod}.rs` (`Format::EasyWorship`, `FileError::{MissingWords, Library}`, `Staged::source_id`, `worker::Source`, `Importer::submit_source`, `Event::File::name`, shared `within_budget`), `src/operator/{song_import,menus}.rs` (File › Import ▸ EasyWorship Library…, `song_uid` in-import duplicates, error texts, library over-limit note), `src/i18n.rs` (8 EN+ID messages; count guard 740 → 748), `src/tests/ew_library_import.rs` (new GPUI tests), `src/tests.rs` (menu deviation row `bar-file-import-easyworship`, PP-EWL-01), `scripts/ew-import-x11.py`, `scripts/ew-import-windows.py`, `scripts/ew_library_fixture.py` (new), `Cargo.toml` (rusqlite feature `collation`; `tempfile =3.27.0` moved from dev to normal dependencies; `Cargo.lock` unchanged), docs (import.md "EasyWorship library", reference-observations EW8-OBS-EWL-1..7, parity-pending PP-EWL-01..08 + run order row 31 + PP-IMP-01 re-run, backlog, e2e-linux).
- Decisions: (1) The original library is never opened by SQLite: both databases and any `-wal`/`-journal` are byte-copied (regular-file open, `O_NONBLOCK` on Unix, 256 MiB cap, cancel between 1 MiB chunks) into a `tempfile` folder deleted with the `Library`; size and mtime are compared before and after each copy, and a changed source or a journal that appears fails with "close EasyWorship and import again". (2) Copies are opened read-write only so SQLite can replay a copied WAL or hot journal in the copy; `trusted_schema` off, `SQLITE_LIMIT_LENGTH` 2 MiB, collation registered, `query_only` on before the songs are read. (3) `UTF8_U_CI` = Unicode lower case then code point order (guess); the preview lists songs `ORDER BY title COLLATE UTF8_U_CI, rowid`. (4) Columns are introspected: only `song.title`, `word.song_id`, `word.words` are required; `vendor_id` → CCLI number (digits ≤ 12, else `InvalidField`); description and tags have no Sela field and are reported as `IgnoredAttribute`, never dropped silently; the newest `word` row wins with an "older rows" warning. (5) A song without words is a failed row (`FileError::MissingWords`), not an empty song. (6) A repeated `song_uid` is an in-import duplicate (`batch_duplicates` checks `source_id` as well as the title/words key); re-import relies on the framework's library duplicate check (same words → Skip). The uid is not stored (no schema change). (7) A library that cannot be read at all is one failed row named after the chosen folder, so the existing preview, cancel and quit guards apply unchanged. (8) At most `batch::MAX_FILES` (500) songs per import, the rest counted ("Songs not read, over the limit…"); paging the remainder is a new open backlog item. (9) Schema knowledge is public-source only (OpenLP's EasyWorship importer description and docs/import.md); every detail is UNOBSERVED (EW8-OBS-EWL-1..7, PP-EWL-01..08).
- Tests added: U `import::ew_library::tests` (13: mapping; sources hashed before/after and copies deleted; WAL-only rows; collation order and SQLite refusing the query without it; missing word row and NULL words; older word rows; truncation/invalid CCLI/missing title/invalid UTF-8; parser and value limits; folder search and case-insensitive names; not-a-library refusals; source changed or journal appearing during the copy; cancel; oversized file); I `import::worker::tests::an_easyworship_library_is_staged_song_by_song_in_title_order`, `a_library_over_the_file_limit_counts_the_rest_and_cancel_stops_it`; `operator::song_import::tests::a_repeated_easyworship_song_uid_is_a_duplicate_inside_the_import` and new errors in `every_warning_and_error_has_text`; GPUI `tests::ew_library_import::an_easyworship_library_is_previewed_committed_and_reimported_as_duplicates`, `a_folder_without_a_library_is_one_failed_row`.
- Proof the tests guard the behavior (temporarily mutated, then restored): without the collation registration and with no journal copies, `metadata_and_words_map_onto_the_song_model`, `the_sources_are_never_written_and_the_copies_are_deleted`, `a_source_that_changes_during_the_copy_is_refused`, `rows_committed_only_to_the_wal_are_copied_with_it` and `titles_sort_with_the_registered_case_insensitive_collation` FAILED; with the `song_uid` lookup disabled in `batch_duplicates`, `a_repeated_easyworship_song_uid_is_a_duplicate_inside_the_import` and the GPUI `an_easyworship_library_is_previewed_committed_and_reimported_as_duplicates` FAILED.
- Results (worktree `/home/user/wt/ew-library-import`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 585 passed/5 ignored, bin 364 passed/4 ignored, then 25, 9/1, 4/1, 14, 9, 32/1, 1, 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/ew-import-x11.py --display :121` PASS (18 s, own Xvfb `:121` + openbox; summary "Ready: 4 · Not importable: 1 · Duplicates: 1", re-import "… Duplicates: 4", `sources_unchanged: true`, exit 0); `ruff check` and `ruff format --check` clean on the three new scripts. One earlier full `cargo test` run in this round failed `tests::autosave_recovery::a_close_waiting_for_a_start_up_open_that_fails_closes_at_once` ("no prompt pops up in a window that is closing"); it passed alone 5/5, in two `--bin sela` runs and in the final full run. It is the autosave stream's known close/prompt race (see its 2026-10-23 follow-up) showing under load on Linux, not this change; not weakened or skipped.
- Unexecuted / open: `scripts/ew-import-windows.py` NOT RUN (no Windows host; PP-EWL-07); every schema detail unobserved until checked on a copy of the owner's EW 8.0.49 library (PP-EWL-01..06, 08); EW8 non-ASCII words (PP-IMP-01); libraries over 500 songs (only the first 500 by title are staged; fixed in review round 2 below); description/tags not imported (no Sela field); a library on a slow network share is copied whole before the preview shows (bounded at 256 MiB per file, cancellable, never on the UI thread; time not measured); the 20k-song preview performance was not measured (500-row cap; measured in review round 2).
- Next: on Windows, run `ew-import-windows.py`, then PP-EWL-01..08 on a hashed copy of the owner's library; update `src/import/ew_fixture.rs`, `scripts/ew_library_fixture.py` and the mapping to the observed schema.
- Review round 2 (2026-10-23, fixer; findings: 1 major, 6 minor; HEAD before `b28e822`):
  - Merge: `ccr-5e192d6c-emgkai` (`483335c`) merged into the branch (`6f70639`); the only conflict was this file (both sides appended at the end), resolved by keeping the scripture CI follow-up bullet with its record and this record after it.
  - Major, fixed (libraries over 500 songs could never be imported past the first 500 by title): `Library::song_ids(limit, known, cancel)` now reads the songs in title order and takes first those whose `batch::title_key` is not among the titles of Sela's library, then fills the room left with known songs, re-sorted into title order; the operator passes its Songs list titles (`Operator::catalog`) as `Source::EasyWorship { folder, known_titles }` (cloned on the UI thread, keyed on the `sela-import` thread). A 1,800-song library therefore stages 1-500, then 501-1,000 on the next import, and so on; under the limit nothing changes. The library note now says so (EN + ID `ImportSongsOverLimit`). Decision: titles, not words, choose the rows, because only the catalog is in memory and the worker must not touch the library; the duplicate policy is unchanged. A song the operator skips keeps coming first (documented). UNOBSERVED best guess EW8-OBS-EWL-8, PP-EWL-08 now asks for repeated imports of the owner's large library.
  - Minor, fixed: the "changed during the copy" check runs once both databases are copied (`copy_db` returns stamps, `unchanged` checks every source and new journals), so a save to `Songs.db` while `SongWords.db` is copied is refused.
  - Minor, fixed: word rows of the chosen songs are found in one pass (`Library::scan_words`: `SELECT rowid, song_id FROM word`, cancel checked every 4,096 rows, map bounded to the chosen ≤ 500 songs), then each newest row by rowid, so no `word.song_id` index is assumed. Measured (Linux dev profile, this container, temporary test removed afterwards): 20,000 songs, 164 MB `SongWords.db`, 20,000 word rows, no index: copy 0.32 s, selection plus word scan 0.23 s, 500 songs staged in 0.88 s total. PP-EWL-04 now asks for the index list.
  - Minor, fixed: `ImportEwNoSongs` (EN + ID) names every searched folder (`Data`, `Databases\Data`, `v6.1\Databases\Data`, `Default\v6.1\Databases\Data`) and says which folders to choose.
  - Minor, fixed: a temp folder or private copy that cannot be created or written is `LibraryError::CopyFailed` ("The library could not be copied to Sela's temporary folder (check free disk space): …", EN + ID `ImportEwCopyFailed`), not `Unreadable`.
  - Minor, fixed: docs/import.md "Duplicates and re-import" cites PP-EWL-06.
  - Tests added: `import::ew_library::tests::{a_save_to_songs_db_while_songwords_db_is_copied_is_refused, a_copy_that_cannot_be_written_is_not_called_unreadable, songs_not_yet_in_the_library_are_chosen_first_over_the_limit, word_rows_are_read_in_one_pass_without_a_song_id_index}` (the last asserts at most one full-scan step per word row via `StatementStatus::FullscanStep`, test-only counter), `import::worker::tests::importing_a_large_library_again_continues_with_the_songs_not_imported` (503 songs: the first import stages songs 1-500; with those titles known in other case the second stages the 3 new ones plus 497 known). Mutation check (temporarily: known titles ignored; the Songs.db check run right after its own copy; copy write errors mapped to `Unreadable`; a scan per song): all five FAILED, then restored and passing. `i18n` ALL count 748 → 749.
  - Results (worktree `/home/user/wt/ew-library-import`): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 590 passed/5 ignored, bin 364 passed/4 ignored, media_decode 25, output_process 9/1, transport_process 4/1, composition_spike 14, input_check 9, native_cues 32/1, output_spike 1, video_spike 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/ew-import-x11.py --display :121` PASS (19 s, own Xvfb `:121` + openbox, binary sha256 `b560f23fe0c0eafb…`, summary "Ready: 4 · Not importable: 1 · Duplicates: 1", preview screenshot inspected).
  - Unexecuted / open: `scripts/ew-import-windows.py` NOT RUN (no Windows host; PP-EWL-07); no run on the owner's real library over 500 songs (PP-EWL-08); a single preview of more than 500 rows is not offered; the over-limit note is not exercised by a native script (the synthetic folder has 5 songs; the worker test covers the selection).
  - Second merge: the integration branch moved to `c0df21e` (ci-windows-stage-scripture) during the round; merged again after `f82d27b`, conflict again only at the end of this file (both records kept, theirs first). Gate on the merged tree: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 590/5 ignored, bin 364/4 ignored, the rest as above); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/ew-import-x11.py --display :121` PASS (18.4 s, binary sha256 `bccea19e0295d5f6…`, same summary).
  - Next: unchanged (Windows script, then PP-EWL-01..08 on a hashed copy of the owner's library).
- Integration (2026-10-23): merged into `ccr-5e192d6c-emgkai` as `70c3585` (`Merge branch 'wave/ew-library-import'`, base `c9685d3` after fuzz-audit). One conflict, `docs/work-log.md`: the status table (this stream's extended M4-07 row kept, the base's M5-02 and M5-07 rows kept after it) and the end of the file (the base's fuzz-audit records first, then this record). The branch adds no SQLite schema, `.selaschedule` or settings format version, so nothing was renumbered. `Cargo.lock` is unchanged (`tempfile` moved from a dev-dependency to a normal dependency at the same pin; `rusqlite` gains the `collation` feature). Gate on the merged tree: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (1155 passed, 12 ignored: lib 676/5, bin 365/4, media_decode 25, output_process 9/1, transport_process 4/1, examples composition_spike 14, input_check 9, native_cues 32/1, output_spike 1, video_spike 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0. Every `scripts/*-x11.py` run one at a time on Xvfb `:199` with Openbox: autosave PASS (20 s), ew-import PASS (18 s), live-output PASS (57 s), media-backend PASS (20 s), scenario-s01 PASS (46 s), scenario-s02 PASS (53 s), schedule PASS (35 s), scripture PASS (49 s), song-editor PASS (63 s), song-import PASS (29 s), songs-browser PASS (42 s), stage-display PASS (32 s), themes PASS (46 s). `ew-import-windows.py` and PP-EWL-01..08 stay open (no Windows host). The fuzz-audit follow-up "add an EW library fuzz target" is still to do.


### ci-windows-autosave-races — flaky autosave close tests on Windows CI (2026-10-23)
- State: product fix (close guard rule) plus test fixes; M1-12 keeps **implemented-unqualified**. Source: Windows CI `windows-latest` on commit `c0df21e`: run 39902216496 passed, run 39902221215 (job 120246129492) failed `cargo test --bin sela` with 359 passed, 2 failed, 290 s (a loaded runner). Branch `wave/winfix4` from `ccr-5e192d6c-emgkai`, Linux container (4 CPUs).
- Failure 1, `tests::autosave_recovery::a_close_waiting_for_a_start_up_open_that_fails_closes_at_once` ("no prompt pops up in a window that is closing", dialog `Some(Recovery)`): product bug. The close stores the layout first (`settings_allow_close` → `save_layout`), so `allow_close` returned false at the settings guard and never reached `autosave_allow_close`; `autosave.closing` was still `None`. When the held start-up scan then answered with an offer before the settings write landed, the regular poll saw `prompt_pending && closing.is_none() && !window_closing` and opened the prompt in a window about to close. fd01aed's `window_closing` covered only the autosave close wait. The same gap existed for Save in a quit's Unsaved prompt (the Save As picker and the file write) and for windows removed by the settings close wait, the Quit action and `proceed(Then::Quit)`, which did not set `window_closing`.
- Fix 1 (product rule): the Recovery prompt never opens while any close is in progress. `Operator::close_in_progress` (src/operator/autosave.rs) = window removal granted (`window_closing`, now set by `remove_for_close`/`window_will_close` at every close site: Quit action, `proceed(Then::Quit)`, the settings and autosave close waits, and the platform `on_window_should_close` in `main.rs`) or the autosave close wait or the settings close wait (`close_pending`, no longer test-only) or a quit's Save still running (`quitting_after_save` in src/operator/schedule_files.rs: the Save As picker, flagged by the new `save_as_quits`, or the save job queued, running or resuming with `Then::Quit`). A close the operator cancels (Cancel in the Unsaved prompt, a cancelled Save As, a failed write, an import preview or running save that refuses the close) is no longer in progress, so the held prompt shows on the next poll; an open dialog already holds it back. EasyWorship has no equivalent prompt; this is Sela's own M1-12 rule, nothing to observe.
- Failure 2, `tests::autosave_recovery::quit_with_dont_save_flushes_the_last_state_and_marks_the_run_clean` (session `"running"` instead of `"clean"`, stdout "autosave close: no reply within 5s"): test race. `CLOSE_WAIT` runs on GPUI's fake test clock, which the test loop advanced by 100 ms per ~5 ms of real time, while the worker's close flush (revision write, fsyncs, session file) runs on a real thread; on a slow disk the fake 5 s passed in well under a real second and the close was forced. Product behavior (force after `CLOSE_WAIT` on a stuck disk) is unchanged.
- Fix 2 (tests): `close_step` (src/tests.rs) advances the fake clock only while no real-thread work a close waits for is outstanding (`Operator::close_work_outstanding`, test-only: a settings write pending, or the autosave worker's start-up open or close flush unanswered); meanwhile it polls replies in real time. Used by every autosave close loop (`close_window`/`wait_closed`, the start-up-scan close, the failed-open close whose `waited` now counts fake time only, Don't Save, the import-preview close) and by the settings close loops with the same pattern (`drain_writer`, `dont_save_during_the_settings_wait_closes_without_asking_again`, `quit_after_ok_waits_for_the_write_and_keeps_painting`). The last one had been passing through the forced-close path: its second 2 s layout write never finished before the fake 5 s ran out (2.24 s run time with the old loop, 4.1 s now); it now also asserts that the layout written by the close is in the file. The stuck-writer tests (`quit_after_a_stuck_write_force_closes_after_the_bound`, `stuck_writer_with_a_dirty_schedule_prompts_after_the_bound`) still advance the clock directly, as they test the bound.
- New regression tests (src/tests/autosave_recovery.rs, settings and schedule writers held at gates by `start_held`): `the_recovery_prompt_stays_back_while_a_settings_write_holds_the_close` (the CI race made deterministic; the run is recorded `offered` and asked again), `cancelling_the_close_shows_the_recovery_prompt_it_held_back` (edit during the wait, Cancel in the Unsaved prompt, then the prompt shows), `the_recovery_prompt_stays_back_during_a_save_then_quit` (picker and held write; the file is written and the window closes without the prompt), `a_cancelled_save_as_of_a_quit_shows_the_recovery_prompt`. Checked failing without the fix: with the old prompt condition restored, these four and `a_close_waiting_for_a_start_up_open_that_fails_closes_at_once` fail (5 of 25); with `quitting_after_save` disabled the two Save tests fail; with only `save_as_quits` kept, the save-then-quit test fails once the write is in flight.
- Reproduction before the fix (Linux): (a) temporary delays (removed before commit, `grep SELA_REPRO src` empty): 300 ms in the settings worker's save made failure 1 fail 3/3, 200 ms in `TestFs::sync_dir` made failure 2 fail 3/3 with the same "no reply within 5s" and `"running"`; (b) load: `taskset -c 0` with six `yes` loaders on CPU 0, `sela-<hash> autosave_recovery --test-threads=8`, failure 1 failed 1 of 10 runs, unmodified code. After the fix: with both delays (200/300 ms) `autosave_recovery settings_ui` passed 20/20, and once with 600/800 ms; under the same CPU-0 load with the delays removed, `autosave_recovery settings_ui --test-threads=8` passed 20/20.
- Results: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 676 + 5 ignored, bin 369 + 4 ignored, media_decode 25, output_process 9 + 1, transport_process 4 + 1, examples 14 + 9 + 32 (+1) + 1 + 20: 1,159 passed); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/autosave-x11.py --kind xvfb --build` PASS (21 s, own Xvfb `:0`).
- Unverified until Windows CI: both fixes on `windows-latest` (no Windows host here; the races were reproduced on Linux by load and injected delays, not on Windows). Expected: `cargo test --bin sela` with 0 failures across repeated runs. The native Windows autosave check (`autosave-windows.py`) was not run.
- Next: read the next Windows CI runs for this head; if an autosave close test still fails, its loop now holds the fake clock while worker replies are owed, so a failure would point at product logic rather than timing.

### bundle-backup-restore — M1-13 portable bundles, backup and restore; S04 (2026-10-23)
- State: M1-13 **implemented-unqualified** (Linux checks pass; Windows run, EW observation and a real second machine open). Container: Linux, 4 CPUs, toolchain 1.98.1; branch `wave/bundle-backup-restore`, worktree `/home/user/wt/bundle-backup-restore`, private display `:122` (`python3 scripts/native_x11.py serve --display :122`).
- Scope: versioned manifest, schedule snapshots, asset hashes, relative references, optional embedded images, fonts listed only; standalone schedule export (File › Export Standalone Schedule…, the EW8-OBS-053 row) and open (File › Open Schedule); whole-profile backup and restore (Profiles › Back Up Profile… / Restore Profile…, UNOBSERVED placement); hostile-archive rejection; S04; docs/bundle.md.
- Decisions: (1) Container is a strict ZIP subset written and read by Sela's own code (`src/bundle/archive.rs`, deflate through the existing `miniz_oxide`, CRC-32 through `crc32fast`, already in the graph through png; no new crate in Cargo.lock), so every refusal is ours and any unzip tool can still inspect a bundle. (2) No compression-ratio cap: entry/total caps from the central directory plus stopping at the declared size (legit SQLite pages compress past 100:1). (3) Fonts are never embedded (redistribution rights are not knowable from a font file; DejaVu ships with Sela). (4) Restricted scripture is left out of standalone schedules and listed; backups keep imported translations (private copy). (5) Opening a package writes a new `Schedules/<title>.selaschedule` and opens it through the normal path, so preflight (M1-11b) explains what is missing; images are adopted byte for byte, never replacing another image, names compared ignoring case on every platform. (6) Restore only creates a new folder (private `.sela-restore-*` staging, one rename); opening it from the menu waits for M1-12b, the result names `--operator-library`. (7) Quit waits for a running job; Cancel leaves nothing half-written; a job that finished before seeing the cancel reports its result. (8) Extensions `.selapackage`/`.selabackup` are Sela's own (EW's unobserved, EW8-OBS-BUN-1/5).
- Files: `src/bundle.rs`, `src/bundle/{archive,manifest,tests}.rs`, `src/images.rs` (`check_embedded`, `adopt`), `src/storage.rs` (`newest_schema`, `library_schema`), `src/operator/bundles.rs` (new), `src/operator/{assets,controls,menus,mod,polling,schedule,schedule_files}.rs`, `src/operator/dialogs/mod.rs` (`Dialog::Bundle`), `src/i18n.rs` (59 EN/ID messages, count 799), `src/fuzz/portable.rs`, `src/tests/bundle_transfer.rs`, `src/tests.rs` (module; Profiles menu test lists the two Sela rows as a tracked deviation, PP-BUN-02), `tests/bundle_s04.rs`, `examples/seed_library.rs` (`--portable-song`), `scripts/bundle-x11.py`, `scripts/bundle-windows.py`, docs (bundle.md new; backlog, work-log, reference-observations EW8-OBS-BUN-1..7, parity-pending PP-BUN-01..08 and run-order row 31, security entry point and fuzz suite, e2e-linux, operator-shell), `Cargo.toml`/`Cargo.lock` (`crc32fast` direct).
- Finding fixed while writing the X11 script: the bundle working dialog closed from `poll` (no window) left keyboard focus on its vanished Cancel button, so after the preflight report closed Ctrl+O did nothing. `focus_pending_dialog` now hands focus back when a dialog button is focused with no dialog open and never records those buttons as the place to return to. GPUI regression (end of `opening_a_standalone_schedule_unpacks_it_and_preflight_explains_what_is_missing`) fails without the fix ("Ctrl+O still reaches the operator") and passes with it. Also: result lines are list items (AccessKit Labels expose no name over AT-SPI), and Unix permission bits without a file type (Python's zipfile) are accepted while link/folder types are refused.
- Results (HEAD before this commit `23cd238` plus the working tree):
  - `cargo fmt --all -- --check`: exit 0.
  - `cargo clippy --locked --all-targets -j 3 -- -D warnings`: exit 0.
  - `cargo test --locked --all-targets -j 3`: exit 0 (lib 689 passed + 5 ignored incl. 28 `bundle::tests` and 5 `fuzz::portable`; bin 367 + 4 incl. 5 `tests::bundle_transfer`; `bundle_s04` 1; media_decode 25; output_process 9 + 1; transport_process 4 + 1; examples 14, 0, 9, 32 + 1, 1, 0, 0, 0, 0, 20).
  - `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu`: exit 0, no warnings (an `unused_mut` on Windows fixed with `cfg_attr`).
  - `PROPTEST_CASES=3000 cargo test --lib fuzz::portable`: 5 passed (1.5 s).
  - `python3 scripts/bundle-x11.py --display :122`: PASS in 44 s (binary sha256 `9980e2e0ab7878ab…`), artifacts `.amp/in/artifacts/bundle-x11/` (`a-export-result.png`, `a-backup-result.png`, `b-preflight.png` with the package notes and the Missing row, `b-repaired.png`, `b-hostile-refused.png`, `b-restore-result.png`, `restored-operator.png`, `summary.json`), inspected.
  - `ruff check` and `ruff format --check` on both bundle scripts: clean.
  - Failure-first evidence: the focus regression above; the archive/manifest refusal tests assert typed errors that do not exist without the code; S04 asserts the missing font and image in preflight.
- Unexecuted / open: `scripts/bundle-windows.py` NOT RUN (no Windows host; PP-BUN-08, also the NTFS case-insensitive collision and the existing-folder restore through the Save dialog, PP-BUN-07); EasyWorship observations EW8-OBS-BUN-1..7 (PP-BUN-01..06); a real second machine; real disk-full, read-only and FAT/exFAT (no hard links) volumes during import/restore; NFC/NFD name normalization; theme-snapshot images are carried and adopted but not yet checked by preflight (M1-11 follow-up); menu-driven switch to a restored profile (M1-12b).
- Next: run `bundle-windows.py` and PP-BUN-01..06 on Windows with EasyWorship 8.0.49; then M1-12b so Restore can offer to switch to the restored profile.

### bundle-backup-restore — review round 1 fixes (2026-10-23)
- State: M1-13 **implemented-unqualified** (unchanged: Windows run, EW observation and a real second machine open; a new open checklist item "Operator export options"). Same container, branch `wave/bundle-backup-restore`, worktree `/home/user/wt/bundle-backup-restore`, private display `:122`. One major and seven minor findings; all fixed.
- Major (a FIFO named `.selapackage`/`.selabackup` hung the worker in `open(2)`, so Cancel never reached it and Quit was held for good): `open_bundle` opens the picked file through `open_picked` (`O_NONBLOCK` on Unix; links the operator picked are still followed) and refuses anything but a regular file as `NotBundle`. `bundle::tests::a_named_pipe_given_a_bundle_name_is_refused_without_waiting` (import, restore and `inspect` on a `mkfifo` pipe of each extension, 10 s bound) fails on the previous `File::open` (`opening the pipe x.selapackage waited for a writer`) and passes now.
- Worker panics: every job runs inside `catch_unwind`; a panic replies `Error::Internal` with the job's own `Done` variant (temporary file/staging folder removed by `Drop` while unwinding) and the thread takes the next job. New test hook `Worker::start_hooked` (`Stage::Before`/`After`, `start_gated` is built on it). Tests: `bundle::tests::a_panicking_job_replies_failed_and_the_worker_takes_the_next` and GPUI `a_job_that_panics_reports_failure_and_frees_the_worker` (result "… stopped because of an internal error in Sela", menu enabled, Quit not held, the next backup works). "Nothing was changed." is not added after an internal error (an import may have stored images).
- Busy race: `Worker::busy` is now a plain field set by `submit` and cleared by the `poll` that returns the reply, both on the caller's thread; the worker thread no longer writes it, so a polled reply can never be followed by a stale busy. `a_polled_reply_is_never_followed_by_a_stale_busy` (2000 submit/poll rounds) states the invariant; it also passed on the previous code in my run (the window between `send` and `store` is a few nanoseconds and was not hit), so it is not failure-first evidence; the fix is structural.
- Import finished after Cancel: `poll_bundles` takes `cancelling`; an `Imported(Ok)` after Cancel never calls `open_recent` (the operator may have edited the schedule after Cancel closed the dialog). The complete file stays in Schedules and the result says "Cancel came too late: “…” was already saved as “…” in Schedules. It was not opened; open it with File › Open Schedule." GPUI `an_import_that_finishes_after_cancel_never_replaces_the_schedule` holds the reply at `Stage::After`, presses Esc, adds a song, releases: the edit and dirty state stay, no read is queued. With the new arm disabled it fails (preflight opens instead of the note).
- Permissions: exports are created `0666 & !umask` (`Access::Shared`), backups stay `0600` (`Access::Private`); docs/bundle.md "Licensing and privacy" says so. `a_standalone_schedule_is_readable_as_the_umask_allows_and_a_backup_is_private` (Linux, umask from /proc/self/status) fails on the previous `0600` export.
- Text: `BundleErrLibrary` gets a localized reason (`BundleLibBusy`, `-Damaged`, `-Unsupported`, `-Unreadable`; disk full uses `BundleErrDiskFull`) instead of `{error:?}`; `operator::bundles::tests::library_failures_are_explained_in_the_operator_language` checks every `storage::Error` in EN and ID never shows its Rust name. English "items" became "item(s)" (project convention) in `BundleExported`/`BundleOpened`; the GPUI export test and `bundle-x11.py` expect "1 item(s)". i18n count 799 → 805 (6 new EN/ID messages).
- Embedded-images UI: the library option has no operator UI; backlog M1-13 gets the open item "Operator export options", PP-BUN-01 records that EW's export options decide its shape, docs/bundle.md names the item.
- UI-thread stat: `bundle_dir` no longer calls `Path::is_dir`; it falls back to the profile's `Schedules` path like `schedule_dir` (the native dialog copes with a missing folder). No test (a filesystem stat is not observable in the GPUI test); checked by reading.
- Docs: bundle.md (pipe refusal, too-late cancel, panics, library wording, permissions, export options, test table), backlog M1-13, parity-pending PP-BUN-01/05, reference-observations EW8-OBS-BUN-6 (Sela's too-late-cancel behavior, still UNOBSERVED in EW).
- Results (HEAD before this commit `d449aff` plus the working tree):
  - `cargo fmt --all -- --check`: exit 0.
  - `cargo clippy --locked --all-targets -j 3 -- -D warnings`: exit 0.
  - `cargo test --locked --all-targets -j 3`: exit 0 (lib 693 passed + 5 ignored, incl. 32 `bundle::tests`; bin 370 + 4, incl. 7 `tests::bundle_transfer` and the operator wording test; `bundle_s04` 1; media_decode 25; output_process 9 + 1; transport_process 4 + 1; examples composition_spike 14, input_check 9, native_cues 32 + 1, output_spike 1, video_spike 20).
  - `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu`: exit 0, no warnings.
  - `PROPTEST_CASES=3000 cargo test --locked -j 3 --lib fuzz::portable`: 5 passed (2.5 s).
  - `python3 scripts/native_x11.py serve --display :122`, then `python3 scripts/bundle-x11.py --display :122 --out <scratch>`: PASS in 45 s (binary sha256 `283a564d83bae75c…`); `a-export-result.png` inspected ("Saved “Sunday.selapackage”: 1 item(s), 3 KB", lantern.png not included, font lines); captures deleted, display stopped.
  - `ruff check` / `ruff format --check` on `scripts/bundle-{x11,windows}.py`: clean.
  - Failure-first: the pipe, permission, too-late-cancel tests fail with their fix reverted (above); the panic tests need the new hook API and the old worker thread died on a panic; the busy test did not fail on the old code (see above).
- Unexecuted / open: unchanged from the stream record (`bundle-windows.py` NOT RUN: no Windows host, PP-BUN-07/08; EW observations PP-BUN-01..06; a real second machine; real disk-full/read-only/FAT volumes; NFC/NFD; theme-snapshot images in preflight; M1-12b switch) plus the operator export options item. `io::ErrorKind` text in `BundleErrIo` is still the Rust `Display` (English) wording; not flagged by the review, left for an i18n pass.
- Next: as before (Windows run and PP-BUN-01..06 with EasyWorship 8.0.49, then M1-12b); shape the export options after PP-BUN-01.

### bundle-backup-restore — review round 2 fixes (2026-10-23)
- State: M1-13 **implemented-unqualified** (unchanged: Windows run, EW observation and a real second machine open; a new open backlog item "Localize image errors"). Same container, branch `wave/bundle-backup-restore`, worktree `/home/user/wt/bundle-backup-restore`, private display `:122`. Three minor findings; all fixed.
- Mid-write panic cleanup (the round-1 panic tests panicked at `Stage::Before`, before any temporary file or staging folder existed): new hook stage `Stage::Check`, run at every cancellation check of a job (`bundle::stop`, which all job checks and the archive chunk callback now go through; the hook is held in a thread-local set by `guarded` for one job and cleared by a drop guard, also on panic; no hook in production, one thread-local read per check). New `bundle::tests::a_job_panicking_mid_write_leaves_no_temp_file_or_staging_folder` panics at the first check that finds a `.sela-*` entry beside the destination, for export, backup and restore; asserts the panic fired, `Error::Internal`, no `.sela-*` left, no destination, then the same job succeeds on the same worker. Failure-first: with `TempFile::drop` skipping removal while panicking it fails at "export" (a `.sela-export-…partial` left); with `Staging::drop` doing so it fails at "backup" (`.sela-backup-…` left). The GPUI `a_job_that_panics_reports_failure_and_frees_the_worker` now panics mid-backup the same way and asserts nothing `.sela-*` is left in the destination folder.
- `BundleErrIo` no longer shows the English `io::ErrorKind` text: `io_line` maps kinds to 7 new EN/ID reasons (`BundleIoNotFound`, `-Exists`, `-InUse`, `-Connection` incl. stale network handle/timeouts/resets, `-Name`, `-TooBig`, `-Other`); quota exceeded uses the disk-full line, a read-only file system the "not allowed" line. `operator::bundles::tests::file_system_failures_never_show_the_english_error_kind` (20 kinds, EN and ID, exact EN/ID lines) fails on the previous `detail = kind` ("… (entity not found) shows \"entity not found\"").
- `library_line` gives each `storage::Error` its own reason: `Missing` "it is not in the profile", `Budget` "it is larger than 256 MB, or copying it took too long" (the storage budget is size 256 MB or 5 s), `Exists`, `Conflict` ("it changed while it was copied"), `Closed`, `Canceled` uses the Cancelled line; `Io` keeps "could not be read or written". The wording test now asserts 11 distinct lines for the 13 errors (Locked/Busy and Corrupt/Invalid share) and the exact EN/ID `Budget` lines; it fails on the previous catch-all (5 distinct lines). i18n count 805 → 817 (12 new EN/ID messages).
- Not changed: `images::Error`'s `Display` is English only, so `BundleErrImage` (and the Media tab's import errors, `src/operator/polling.rs`) show English detail in Indonesian; tracked as backlog M1-13 "Localize image errors" and named in docs/bundle.md.
- Docs: bundle.md (mid-write panic test, per-failure library reasons, file system reasons, test table), backlog M1-13 (new open item), parity-pending PP-BUN-05 (re-run the mid-write test) and PP-BUN-06 (full USB stick / unplugged share / 256 MB library steps, UNOBSERVED best guess).
- Results (HEAD before this commit `1c6ea27` plus the working tree):
  - `cargo fmt --all -- --check`: exit 0.
  - `cargo clippy --locked --all-targets -j 3 -- -D warnings`: exit 0.
  - `cargo test --locked --all-targets -j 3`: exit 0 (lib 694 passed + 5 ignored, incl. 33 `bundle::tests`; bin 371 + 4, incl. 7 `tests::bundle_transfer` and the 2 operator wording tests; `bundle_s04` 1; media_decode 25; output_process 9 + 1; transport_process 4 + 1; examples composition_spike 14, input_check 9, native_cues 32 + 1, output_spike 1, video_spike 20).
  - `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu`: exit 0, 0 warnings.
  - `PROPTEST_CASES=3000 cargo test --locked -j 3 --lib fuzz::portable`: 5 passed (7.8 s).
  - `python3 scripts/native_x11.py serve --display :122`, then `python3 scripts/bundle-x11.py --display :122 --build --out <scratch>`: PASS (binary sha256 `6d1631e7777e4fb4…`); `b-hostile-refused.png` inspected (the "../evil.png" refusal and "Nothing was changed."); captures deleted, display stopped.
  - `ruff check` / `ruff format --check` on `scripts/bundle-{x11,windows}.py`: clean.
- Unexecuted / open: unchanged from round 1 (`bundle-windows.py` NOT RUN: no Windows host, PP-BUN-07/08; EW observations PP-BUN-01..06; a real second machine; real disk-full/read-only/FAT volumes and an unplugged share; NFC/NFD; theme-snapshot images in preflight; M1-12b switch; operator export options) plus "Localize image errors". An import's own schedule temp file (`write_new_schedule`) is removed explicitly, not by a drop guard; it has no check point, so no test panics inside it.
- Next: as before (Windows run and PP-BUN-01..06 with EasyWorship 8.0.49, then M1-12b); shape the export options after PP-BUN-01; localize image errors.
- Integration (2026-10-23): merged into `ccr-5e192d6c-emgkai` as `51d4e08` (`Merge branch 'wave/bundle-backup-restore'`, base `df73dba` after winfix4). Conflicts, all resolved keeping both sides: `docs/work-log.md` (both Current state bullets and both stream records), `docs/e2e-linux.md` (the ew-import and bundle script notes), `docs/parity-pending.md` (Windows script table: `ew-import-windows.py` stays row 31, `bundle-windows.py` becomes row 32), `src/i18n.rs` (variant count guard 740 base + 9 ew-library-import + 77 bundle = 826), `src/operator/menus.rs` (`ImportEasyWorship` and the bundle items' enabled rules side by side), `src/tests.rs` (both `bundle_transfer` and `ew_library_import` modules). No schema, `.selaschedule` or settings version change on this branch, so nothing renumbered; `Cargo.lock` auto-merged and passes `--locked`. Merged-tree gate: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 714 + 5 ignored, bin 378 + 4 ignored, bundle_s04 1, media_decode 25, output_process 9 + 1, transport_process 4 + 1, examples all pass); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; all 14 `scripts/*-x11.py` serially on Xvfb `:199` with Openbox: PASS (autosave, bundle, ew-import, live-output, media-backend, scenario-s01, scenario-s02, schedule, scripture, song-editor, song-import, songs-browser, stage-display, themes). Windows scripts NOT RUN (no Windows host).

### video-playback — M2-02 video backgrounds, M2-03 transport, M2-05 basic thumbnails (2026-10-23)
- State: M2-02 (video backgrounds) and M2-03 **implemented-unqualified**; M2-05 **active** (basic slice implemented-unqualified); M2-04 **implemented-unqualified (feasible part)**: audio of background videos (added in the second commit of this record, `feat(M2-04)`). Worktree `/home/user/wt/video-playback`, branch `wave/video-playback` from `db0c9f0`; private display `:123` (`native_x11.py serve --display :123`).
- Scope / files: `src/background.rs` (`VideoRef`, `Fill::Video { video, repeat }`, codec tag 4, `Plan::Video`), `src/videos.rs` + `src/videos/tests.rs` (new: managed Videos folder, list with metadata, fingerprint pins, verify, probe-checked import with cancel and dedup, `poster`, `Lru`, latest-wins `Lookups` = `Thumbnailer`/`Checker`, `Worker`, process tools cache), `src/playback.rs` (new: `MediaAction`, `PlayState`, `MediaStatus`, `Timeline` media clock), `src/media/decode.rs` (`Fit::Cover`/`Fit::Stretch`/`Fit::for_aspect`, `DecodeOptions::looping` → `-stream_loop -1`, `first_frame_fit`), `src/scene.rs` (`PreparedBackground::Video`, `VideoSource`), `src/delivery.rs` (`Payload::Media`, `submit_media`, public `Command::deadline`), `src/transport.rs` (background tag 2, kinds 8 MEDIA and 9 STATUS, `PipeWorkers::offer_status`), `src/output.rs` (media slots and `media_status`), `src/audience.rs` + `src/audience/video.rs` (new: `prime`, `Playback`, `Registry`, `ToolsCell`, player thread; applied video, dirty redraw, status reports) + `src/audience/compositor.rs` (`video_texture`, `upload_frame`, `prepare_native_video`, `read_texture`), `src/operator/video.rs` (new: Videos list, import, posters, pin checks, `video_background`, transport row, Media sidebar), `src/operator/{controls,layout,mod,polling,media_backend,preview_live}.rs`, `src/operator/resources/mod.rs`, `src/song_library.rs` + `format_pane.rs` + `theme_mode.rs` (Select Media… Videos/Images with Left/Right, Repeating, posters in previews and theme thumbnails), `src/schedule_file.rs` (`VIDEO_VERSION` 6), `src/fuzz/{codecs,transport}.rs`, `src/i18n.rs` (51 EN+ID messages; count guard 749 → 800), `src/main.rs` (actions `ToggleVideoPlayback`, `RestartVideo`), `examples/seed_library.rs` (`--video-song CLIP`), `examples/composition_spike.rs`, tests (`src/tests/video_playback.rs` new, `src/tests.rs` fake renderer video/media/status, `src/tests/stage_display.rs`, `tests/media_decode.rs`, `tests/output_process.rs`), scripts (`video-x11.py`, `video-windows.py` new; `song-editor-{x11,windows}.py` press Right to reach Images), docs (media.md "Video backgrounds and playback", delivery.md wire additions, reference-observations EW8-OBS-VP-1..12, parity-pending PP-VP-01..12 + run order row 32, backlog M2-02/M2-03/M2-05, e2e-linux).
- Decisions: (1) The renderer decodes the video itself in its own supervised FFmpeg child (it locates FFmpeg at start with the operator's rules and environment); the cue carries the absolute path (first path on the pipe, checked by `input_args` rules on both ends) instead of streaming frames over the pipe. (2) First-frame gate: the preparation worker waits for the first frame until the cue deadline less 150 ms and uploads it; any failure is RenderFailed, so the live scene never changes; a primed video stands still until applied. (3) A playback thread presents frames at their timestamps (late frames skipped to the newest due) by writing one texture (`write_texture` + empty submit, no staging pile-up) and waking the event loop; the redraw only encodes. (4) Consecutive cues with the same video key share one playback (Next keeps the video running). (5) Looping uses `-stream_loop -1` (timestamps keep rising; reports wrap to the clip). (6) Transport is Play/Pause/Restart on the Safety lane after masks; reports travel in a one-slot latest-wins status slot, never in a receipt slot; the Live row shows only the renderer's report and only for an acknowledged, unsubstituted video slide. (7) Masks never pause the video (best guess). (8) Videos are copied into the profile and pinned by length plus first/last-MiB SHA-256 (a mid-file change is not noticed; documented). (9) Missing/changed/unavailable videos substitute black with a warning (image rule); no Go Live dialog for videos yet. (10) `.selaschedule` version 6 only when a video background is present; no SQLite schema change (schema version used: none). (11) Select Media… opens on Videos (EW8-OBS-039) unless the media is an image; the editor scripts press Right to reach Images. (12) Audio of video backgrounds is deferred to the M2-04 slice.
- Tests added: U `background::tests::{video_fills_reject_bad_names_repeat_bytes_and_truncation, …}`, `playback::tests` (6), `videos::tests` (11 incl. real FFmpeg import, refusals, cancel, posters, checks, `a_key_being_worked_is_never_queued_again_by_a_newer_list`), `media::decode::tests::{cover_fit_scales_past_the_frame_and_crops_evenly, stretch_fit_scales_each_axis_to_the_frame}` + looping args, `transport::tests` (4 new), `schedule_file::tests::video_backgrounds_round_trip_as_version_6_and_only_they_raise_it`, fuzz strategies/seeds; I `tests/media_decode.rs::{cover_and_stretch_fill_the_cue_extent_without_bars, looping_continues_past_the_end_with_increasing_timestamps}`, `tests/output_process.rs::media_commands_need_a_scene_and_the_status_follows_the_live_scene`; R (real adapter, lavapipe here) `audience::video::tests::{a_primed_video_holds_its_first_frame_until_applied_then_plays_pauses_and_restarts, the_same_video_is_shared_and_stops_with_its_last_handle, a_video_that_plays_once_ends_on_its_last_frame, failures_before_the_first_frame_never_give_a_playback}`; GPUI `tests::video_playback::{a_video_slide_goes_live_after_its_pin_check_and_the_transport_follows_the_renderer, a_missing_video_substitutes_black_with_a_warning_and_no_transport, without_ffmpeg_a_video_slide_goes_live_black_with_the_unavailable_warning, a_video_cue_waits_for_the_ffmpeg_lookup_and_never_replaces_the_scene_meanwhile, a_video_the_renderer_cannot_open_keeps_the_previous_scene, the_media_tab_lists_videos_by_name_and_imports_off_the_ui_thread, a_real_import_lists_the_video_and_its_thumbnail_lands}`, `song_library::tests::select_media_videos_sets_a_repeating_video_fill_that_saves`; `song_library::tests::slide_tab_sets_color_image_and_aspect_as_undo_steps` now opens Images explicitly (the picker opens on Videos).
- Proof the tests guard the behavior: three real defects were caught by the new tests before this commit and fixed: the playback thread lost the decoder's End event while skipping late frames, so a play-once video never ended (`a_video_that_plays_once_ends_on_its_last_frame` FAILED, fixed with a held-event slot); the Live transport followed the requested slide, so a video cue the renderer refused still showed the transport (`a_video_the_renderer_cannot_open_keeps_the_previous_scene` FAILED, fixed by requiring the acknowledged slide); a poster dropped from one wanted list stayed "asked" forever (the full-suite run of `a_real_import_lists_the_video_and_its_thumbnail_lands` FAILED, fixed by deduplicating in the worker, guarded by `a_key_being_worked_is_never_queued_again_by_a_newer_list`, which decodes key 1 twice without the `working` filter).
- Results (worktree `/home/user/wt/video-playback`, debug build): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 701 passed/5 ignored, bin 377/4, media_decode 27, output_process 10/1, transport_process 4/1, composition_spike 14, input_check 9, native_cues 36/1, output_spike 1, video_spike 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `python3 scripts/video-x11.py --display :123` PASS (41-42 s; summary: thumbnail navy/green samples 850/25, Go Live to first audience change 159-418 ms across runs, position "Playing · 0:01 / 0:08" after the loop, box at 48-50 px after Restart, box 54 → 270 px across Black, one FFmpeg child while playing and none after the missing-video slide; captures inspected: lyrics over the moving box, Clear without text, Bridge black with text); `media-backend-x11.py`, `live-output-x11.py`, `song-editor-x11.py` PASS on `:123` (song-editor after the Right step); `ruff check` and `ruff format` on the new scripts clean. This VM only (lavapipe, debug): while playing the audience used 48% of one CPU and its FFmpeg child 7%, audience RSS 131 MiB; the pinned FFmpeg decodes 1080p30 VP9 to RGBA at about 140 frames/s here.
- Unexecuted / open: `scripts/video-windows.py` NOT RUN (no Windows host, PP-VP-12); EW observations PP-VP-01..11 (no EW here); audio (M2-04 next slice); seek, start position, stop, video logo, video schedule items, Videos context menu, videos in the preflight report and the Go Live missing-file dialog; next-cue preload and GPU budgets (M2-05); hardware decode and 1080p60/4K pacing (M2-08); a renderer resize restarts the background video.
- Audio slice (M2-04 feasible part, `feat(M2-04)`):
  - Scope / files: `src/media/audio.rs` (new: `AudioFormat`, lock-free SPSC `SampleRing` of f32 bits in atomics, `sync_step`, `audio_args`, `AudioDecoder` — supervised FFmpeg to f32le, waits for ring room, Failed/Ended states, `stop` bounded), `src/audience/audio.rs` (new: `Output` with the real-time `render` callback, null output thread, Windows `cpal` device on its own thread), `src/audience/video.rs` (`prime` takes the output opener; the playback thread opens audio only for a clip with an audio stream, after the first-frame gate, follows the timeline each loop, Restart stops the old decode, flushes and decodes from the start), `src/audience.rs` (`VideoContext::audio`), `src/playback.rs` (`AudioState`, `MediaStatus::audio`), `src/transport.rs` (STATUS 50 bytes, audio byte; docs/delivery.md), `src/operator/video.rs` (`transport_line`, sound note), `src/i18n.rs` (3 EN+ID messages, 803), `src/tests.rs` (fake renderer reports NoDevice), `Cargo.toml`/`Cargo.lock` (`cpal = "=0.18.2"`, Windows only, default features off; adds cpal and dasp_sample to the Windows build), `THIRD-PARTY-NOTICES` (regenerated: 431 crates), scripts `video-x11.py`/`video-windows.py` (clip with a 440 Hz Opus tone; sound note; two decoder children; the x11 loop check now compares the clip position with elapsed time), docs (media.md "Audio of video backgrounds", backlog M2-04 progress, EW8-OBS-VP-13, PP-VP-13).
  - Decisions: (13) The video is the master clock; audio is corrected toward it (skip when behind, silence when ahead, 40 ms tolerance) instead of the video following the audio device clock (the video path stays as qualified so far; audio-master is an M2-04 qualification question). (14) One output per playing video (a shared mixer waits for volume/routing work). (15) `cpal` only on Windows (release platform; reuses GPUI's `windows` crate; no ALSA dependency in Linux builds); the null output keeps the decode, ring and clock path identical where no device is built. (16) A device or decode failure never stops the picture; it is reported as "Sound failed". (17) Masks never mute (best guess, EW8-OBS-VP-13). Licenses: cpal Apache-2.0, dasp_sample MIT OR Apache-2.0 (deny.toml allow list; `cargo deny` not installed here, NOT RUN).
  - Tests added: U `media::audio::tests` (5: ring order/wrap and overfill, two-thread SPSC order, `sync_step`, `audio_args`, real tone decode at 440 Hz and missing-input failure), `audience::audio::tests` (4: paused/flushed silence, skip/wait, whole frames and volume, null output real-time rate), `transport` STATUS audio byte; R `audience::video::tests::a_videos_audio_follows_its_clock_and_a_silent_one_has_none` (null output: primed reads nothing, consumes within 200 ms of the video position, stops on Pause, starts over on Restart; a silent clip opens no output); GPUI `tests::video_playback::the_transport_line_names_state_position_and_sound` and the sound-note assertion in `a_video_slide_goes_live_after_its_pin_check_and_the_transport_follows_the_renderer`.
  - Results (debug build, this VM): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 706 passed/5 ignored, bin 383/4, media_decode 27, output_process 10/1, transport_process 4/1, composition_spike 14, input_check 9, native_cues 41/1, output_spike 1, video_spike 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0 (cpal compiled); `SELA_REQUIRE_FFMPEG=1 cargo test --locked -j 3 --bin sela audience::video` 5 passed, three repeats; `python3 scripts/third_party_notices.py --check` up to date; `python3 -m unittest scripts/test_third_party_notices.py` 6 OK; `ruff check`/`ruff format --check` on both video scripts clean; `python3 scripts/video-x11.py --display :123` PASS (Go Live first change 233-464 ms; first "Playing" probe already carries "No sound output"; two FFmpeg children; audience 43-47% CPU, picture decoder 6.5-7.5%, sound decoder 0.5-1.0%, RSS 133-134 MiB; this VM only); regressions `live-output-x11.py` and `stage-display-x11.py` PASS on `:123`.
  - Unexecuted / open: any audio on real speakers or capture-measured A/V offset (no device here; M2-04 qualification); `scripts/video-windows.py` NOT RUN (no Windows host; PP-VP-12/13, it now also checks the sound note and asks the operator to listen); `cargo deny check` NOT RUN (not installed); volume, mute, fades, routing, device unplug and sample-rate change, long-run drift, audio-only items and background audio; Linux/macOS device output (not built).
- Review round 1 (`fix(M2-02)`, 2026-10-23; independent review of `caee925`):
  - Fixed: (a, major) cached pin checks: `video.checked` answered every later Go Live/Next with an Ok from the first check, so a deleted file reached the renderer (RenderFailed, old slide kept, no warning) and replaced bytes played without the "replaced" warning, contradicting PP-VP-08 "Sela now" and decision (9). Now `videos::Checked { result, started }` stamps when the read started, `VideoState::cue_request` is set by `send` and `refresh_live_request`, and `video_background` uses only a check whose read started after the request (`Checked::fresh_for`), otherwise drops it and asks the checker again (cost per video cue: one ≤2 MiB read on the checker thread and one 16 ms poll). Slide posters (`ThumbKey::Pinned`) follow the checks: a failed check drops a made poster, a passing one drops a failed poster; a rescan retries failed slide posters (`Lru::retain_entries`). (b, major) Song Editor Select Media… › Import… ran `videos::import(.., &mut || false)` on the background executor with no cancel, progress or bound: now `format_pane::MediaImport` (one at a time; a second is refused with ImportBusy; "Importing NAME…" status; the row becomes "Cancel import" for a video and sets the flag the copy polls per 1 MiB chunk; the guard cancels on drop, so a closed editor stops its copy) and `import_media_file` maps `Cancelled`; opening Media › Videos rescans, so editor imports appear without a restart. (c, minor) Videos-list posters followed only the first 64 rows: `list_thumb_rows` now wants the rows in view (list `ScrollHandle`), then 8 past each edge, at most 64. (d, minor) backlog: the M2-02 boxes are split so ticked text matches what is built (open boxes: videos in the Go Live dialog and preflight, Volume/Media Usage, stale frames after seek); M2-03 gains open boxes for the mask-behind-transport wait and keyboard routes. (e, minor) unbound `ToggleVideoPlayback`/`RestartVideo` actions and their handlers removed (no key or menu used them; PP-VP-07 records EW shortcuts as unobserved). (g, minor) docs/media.md records the upstream GPUI paths at the pinned `a846890`: `elements/img.rs` (`img`), `app.rs:2983` (`App::drop_image`, called outside window updates), `elements/div.rs` (`ScrollHandle::top_item`/`bottom_item`/`bounds_for_item`, `role`).
  - Not fixed: (f, minor) a transport command in flight holds the next mask for one Safety-lane round trip. Masks and MEDIA share the one-command Safety lane (`delivery::Lane`, the lane byte on the wire, the renderer's per-lane acceptance in `audience.rs`); a lane of its own is a protocol change, not a cheap fix. A mask still goes before a queued transport command. Recorded as an open M2-03 box and in media.md "Open". The eager Videos list (up to 4,096 focusable rows) stays: `uniform_list` would drop off-screen rows from the Tab order; open in M2-05 and media.md.
  - Decisions: (18) Every video cue checks the pin again (not only Go Live as for images), because the renderer opens the file when the cue arrives; a file removed between that check and the renderer's open is still refused by the first-frame gate. (19) The editor's import is bounded to one at a time per editor window and cancellable for videos only (an image import is short). (20) i18n +1 EN/ID message `MediaImportCancel` (count guard 803 → 804); new parity item PP-VP-14; EW8-OBS-VP-9 notes the per-cue check and the editor import best guess.
  - Tests added: U `videos::tests::{a_pin_check_is_fresh_only_for_cues_asked_for_before_its_read, the_lru_keeps_entries_by_key_and_value}`; GPUI `tests::video_playback::{a_video_replaced_or_deleted_after_it_played_is_caught_on_the_next_go_live, videos_list_posters_follow_the_rows_scrolled_into_view}`, the rescan step in `the_media_tab_lists_videos_by_name_and_imports_off_the_ui_thread`, `song_library::tests::{select_media_video_import_is_one_at_a_time_shown_and_cancellable, a_cancelled_select_media_video_import_leaves_no_file}` (real FFmpeg); `a_video_the_renderer_cannot_open_keeps_the_previous_scene` now removes the file after the Go Live check lands (the only way left to reach the renderer refusal) and still asserts the previous scene stays.
  - Proof the tests guard the fixes (each revert applied, run, restored): accepting a stale check (`if true || started >= cue_request`) → `a_video_replaced_or_deleted_after_it_played_is_caught_on_the_next_go_live` FAILED (left `None` vs the "replaced" warning) and `a_video_the_renderer_cannot_open_keeps_the_previous_scene` FAILED (no deferred cue); keeping slide posters on a failed check → the replaced-video test FAILED at the poster assertion; no rescan on Videos → `the_media_tab_lists_videos_by_name_and_imports_off_the_ui_thread` FAILED ("bounded wait for rescanned"); first-64 posters only → `videos_list_posters_follow_the_rows_scrolled_into_view` FAILED ("bounded wait for last row's poster"). The editor-import tests do not compile without `start_media_import`/`MediaImport`/`import_media_file`.
  - Results (worktree `/home/user/wt/video-playback`, debug build, this VM): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 708 passed/5 ignored, bin 387/4, media_decode 27, output_process 10/1, transport_process 4/1, composition_spike 14, input_check 9, native_cues 41/1, output_spike 1, video_spike 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `SELA_REQUIRE_FFMPEG=1 cargo test --locked -j 3 --bin sela -- a_cancelled_select_media a_real_import` 2 passed; `SELA_REQUIRE_FFMPEG=1 cargo test --locked -j 3 --lib videos::` 13 passed; `python3 scripts/native_x11.py serve --display :123` then `python3 scripts/video-x11.py --display :123` PASS (binary sha256 8ba2a0d2…; Go Live first change 322 ms with the per-cue check; "Playing · 0:04 / 0:08 · No sound output"; 2 FFmpeg children while playing, audience 49.5% CPU, picture decoder 7.5%, sound decoder 1.0%, RSS 131.2 MiB; loop wrapped at 13 s; box 48 px after Restart; box 54 → 280 px across Black; 0 FFmpeg after Live off; captures inspected: Chorus lyrics over the moving box, Bridge black with text) and `python3 scripts/song-editor-x11.py --display :123` PASS. Scratch captures deleted afterwards.
  - Unexecuted / open: `scripts/video-windows.py` NOT RUN (no Windows host; PP-VP-12/13/14); EW observations PP-VP-01..14; the items under "Not fixed".
- Review round 2 (`fix(M2-02)`, 2026-10-23; independent review of `41b1bcf`, four minors, no blocker or major):
  - Fixed: (1) a deferred video cue could stall: `Lookups::want` leaves out the key being worked, and the worker cleared `working` only after its reply was already in the channel, so when the UI took a stale check's reply first and `video_background` asked for the same video again, the re-want was dropped and nothing asked again (`missing == wanted_checks`), leaving "Preparing background…" until the next press (the live scene stayed). `Lookups::poll` now clears the in-flight mark of the key whose reply it hands out (only if the worker still has that key marked); the worker's own clear stays. (3) `video_background` now decides freshness with `Checked::fresh_for` instead of repeating `started >= cue_request`, so the round 1 record's claim and the unit test cover the production guard. (2, docs) PP-VP-08's re-run step claimed `video-windows.py` step 7 covers the deleted/replaced-after-play cases; it covers only the Bridge whose video never existed. PP-VP-08 now says no native script deletes or replaces the clip after it played (hand check plus the GPUI test). (4, test gap) PP-VP-14 "closing the editor cancels the copy" was shown only by dropping a standalone guard: a GPUI test now closes the editor window during an import, asserts nothing keeps the `Library` entity and that the copy's flag is set; PP-VP-14 also names `videos::tests::import_refuses_non_video_and_cancelled_imports_leave_no_file`, whose late cancel (third poll) stops `videos::import` inside the chunked copy (`copy_temporary`, the only poll after the probe), the same closure path `import_media_file` passes the editor's flag to.
  - Tests added: U `videos::tests::a_key_asked_again_right_after_its_reply_is_queued_even_before_the_worker_clears_it` (a `Lookups` with no worker thread, the test plays the worker stopped between its send and its clear: deterministic); GPUI `song_library::tests::closing_the_editor_during_a_select_media_import_cancels_it`.
  - Proof the tests guard the fixes (revert applied, run, restored): `poll` back to a bare `try_recv` → the new videos test FAILED (`left: []`, `right: [7]`); `MediaImport::drop` no longer calling `cancel` → the editor-close test FAILED ("the closed editor's import is cancelled"). Finding 3 is a refactor of the guard (no behaviour change); round 1's proof (accepting a stale check fails the GPUI replaced/deleted test) still applies.
  - Results (worktree `/home/user/wt/video-playback`, debug build, this VM): `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 709 passed/5 ignored, bin 388/4, media_decode 27, output_process 10/1, transport_process 4/1, composition_spike 14, input_check 9, native_cues 41/1, output_spike 1, video_spike 20); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0; `SELA_REQUIRE_FFMPEG=1 cargo test --locked -j 3 --lib videos::` 14 passed; `SELA_REQUIRE_FFMPEG=1 cargo test --locked -j 3 --bin sela -- a_cancelled_select_media a_real_import closing_the_editor_during` 3 passed; `python3 scripts/native_x11.py serve --display :123` then `python3 scripts/video-x11.py --display :123` PASS.
  - Unexecuted / open: `scripts/song-editor-x11.py` not re-run this round (no editor production code changed; only a test was added); a native script step that deletes or replaces a clip after it played (PP-VP-08 hand check); `scripts/video-windows.py` NOT RUN (no Windows host); EW observations PP-VP-01..14; round 1 "Not fixed" items.
- Next: PP-VP-12/13/14 on Windows, then the PP-VP observations in EasyWorship; a Safety-lane split for transport (M2-03) if masks must never wait.
- Integration (2026-10-23): merged into `ccr-5e192d6c-emgkai` as `a22d3af` (`Merge branch 'wave/video-playback'`, base `a6917b6` after bundle-backup-restore). Conflicts, all resolved keeping both sides: `src/i18n.rs` (bundle and video `Msg` variants in the enum and `ALL`; count guard 749 + 77 + 55 = 881), `src/operator/mod.rs` (both `Bundles::start` and `profile_for_video`), `examples/seed_library.rs` (`--portable-song` and `--video-song CLIP` both kept: doc comment, USAGE, flags, both seed blocks), `Cargo.lock` (regenerated by cargo from this branch's lock: `cpal` and `crc32fast` both listed), `docs/work-log.md` (records of both streams; Current state bullets and status rows auto-merged), `docs/parity-pending.md` (Windows run order: `bundle-windows.py` keeps row 32, `video-windows.py` renumbered to row 33, PP-VP-12 updated), `docs/reference-observations.md` (both UNOBSERVED tables), `docs/e2e-linux.md` (both script notes). No version collision: `.selaschedule` `VIDEO_VERSION` 6 is new on this side only, no SQLite schema or settings format change; wire kinds 8/9 unused on the base. Cross-stream gap found and filed (not fixed here): bundles and backups do not carry or list videos (backlog M1-13 "Videos in bundles and backups"). Gate on the merged tree: `cargo fmt --all -- --check` exit 0; `cargo clippy --locked --all-targets -j 3 -- -D warnings` exit 0; `cargo test --locked --all-targets -j 3` exit 0 (lib 747 passed/5 ignored, bin 401/4, bundle_s04 1, media_decode 27, output_process 10/1, transport_process 4/1, examples all pass); `cargo check --locked --all-targets -j 3 --target x86_64-pc-windows-gnu` exit 0, no warnings; every `scripts/*-x11.py` serially on Xvfb `:199` with Openbox: PASS (autosave 21 s, bundle 46 s, ew-import 19 s, live-output 61 s, media-backend 22 s, scenario-s01 49 s, scenario-s02 55 s, schedule 36 s, scripture 52 s, song-editor 65 s, song-import 32 s, songs-browser 45 s, stage-display 34 s, themes 46 s, video 46 s). Windows scripts NOT RUN (no Windows host); PP-VP-12/13/14 stay open.
