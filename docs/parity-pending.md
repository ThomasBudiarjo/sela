# Parity pending: EasyWorship observations and Windows runs

This is the single checklist of what the owner still has to do on Windows:
observe EasyWorship **8.0.49** (the pinned reference) and run Sela's Windows
native scripts. It was seeded on 2026-10-22 from every open item in
[reference-observations.md](reference-observations.md) (unobserved best
guesses, runbook W01–W06/G01 leftovers, the "Open:" list of the M0-02 status
row) and from the "not run"/Windows items in the [work log](work-log.md)
ticket status index and recent records. Linux runs
([e2e-linux.md](e2e-linux.md)) never close an item here.

How to use it:

- Observation rules stay those of the reference runbook: lawful install,
  disposable profile, original fixtures (songs A1–A3/B1–B3, an original logo,
  an original timecoded clip with audible ticks), never save into EasyWorship
  unless an item says so, record build/OS/DPI/displays and artifacts. Append a
  dated `RUN-…` section and new `EW8-OBS-…` rows to
  reference-observations.md; never overwrite an earlier result.
- When an item is done, tick it and add `run/record, date, evidence path`.
  When an observation contradicts Sela, open a ticket and link it here.
- "Sela now" is the current behavior or rule (best guesses are marked); "Re-run"
  names the Sela script to run after the observation or after a fix.
- Feature streams add their own items under their `## Stream:` section below
  (same item format), including every Windows script they could not run.

Common Windows prerequisites are listed once in
[Windows script run order](#windows-script-run-order).

## Reference access and fixtures (W01)

- [ ] **PP-REF-01 About build and license limits** (EW8-OBS-001, -010).
  Steps: open Help › About in the installed EasyWorship, capture the build
  string (menus do not render through PrintWindow: use a screen grab); record
  what the unlicensed edition blocks. Sela now: the pin is FileVersion
  8.0.49.0 read from the executable only. Re-run: none (record only).
- [ ] **PP-REF-02 Original A/B fixtures and capture setup** (W01 steps 2–4).
  Steps: create songs A and B with three slides each by hand (synthetic input
  did not reach the editor's fields in RUN-W01), an original logo and a
  timecoded clip with audible ticks; set up a camera or capture of the
  physical audience display with a visible time marker. Needed by most items
  below. Sela now: n/a. Re-run: none.

## Selection, Preview and Live (W02)

- [ ] **PP-W02-01 Multi-item selection and Go Live** (EW8-OBS-002, -013; M0-02
  "multi-item W02"). Steps: with A1 live, single-click B in the Schedule,
  select B2 in Preview, step Preview next/previous, select a resource, type a
  search; record whether A1 stays on the audience after each; then Go Live by
  button, by Preview double-click and by Page Down, and record which slide
  applies and where focus lands. Sela now: Preview selection never changes
  the audience; Go Live applies the selected Preview slide and focuses the
  Live pane (EW8-OBS-014, single item). Re-run: `live-output-windows.py`,
  `navigation-windows.py`.
- [ ] **PP-W02-02 Editing the live item.** Steps: with A live, edit and save A
  (and separately edit-and-cancel a copy of B); record whether the audience
  changes before a new Go Live. Sela now: the live scene changes only on a
  new cue; schedule items pin song revisions until Update items. Re-run:
  `schedule-remainder-windows.py`.
- [ ] **PP-W02-03 Duplicate occurrences, reorder and removal of the live
  item.** Steps: schedule A twice, go live on the second occurrence, reorder
  and remove preceding items, then delete the live item; record the Live
  marker and the audience. Sela now: the live item keeps its identity across
  reorder/removal and no cue is sent (M1-09a). Re-run: `schedule-windows.py`,
  `schedule-remainder-windows.py`.
- [ ] **PP-W02-04 Pending library Go Live against a newer Live selection**
  (EW8-OBS-066b). Steps: start a Songs-tab Go Live on a large song and, while
  it loads, click another Live slide; record which wins. Sela now: the older
  pending Songs Go Live still replaces the newer Live selection when it loads
  (behavior unchanged, UNOBSERVED). Re-run: `live-output-windows.py`.

## Masks and logo (W03)

- [ ] **PP-MASK-01 Live double-click with combined masks** (EW8-OBS-020,
  M0-02 "combined-mask double-click"). Steps: from A1 live set Black+Clear,
  double-click Live slide 2; repeat with Logo+Clear; record indicators and
  audience. Sela now: one double-click clears both cover and Clear
  (provisional). Re-run: `live-output-windows.py`.
- [ ] **PP-MASK-02 Single-click on a Live slide under a mask** (EW8-OBS-020).
  Steps: under each single mask single-click Live slide 3. Sela now: applies
  the slide and keeps the mask (provisional). Re-run: `live-output-windows.py`.
- [ ] **PP-MASK-03 Mask across profile close/reopen** (W03 lifecycle).
  Steps: with each single mask on, close and reopen EasyWorship; record the
  startup mask state. Sela now: masks persist across Live output off/on in one
  session (EW8-OBS-017); no mask state is saved, so a restart starts with
  none. Re-run:
  `live-output-windows.py`.
- [ ] **PP-MASK-04 Masks with motion and audio** (EW8-OBS-006, -020; M0-02
  "motion/audio"). Steps: W03 single, pair and masked-apply cases with the
  timecoded clip live, including clip end and loop boundary; record picture
  position and audible continuation. Sela now: no video/audio playback yet
  (M2-03/M2-04). Re-run: none yet (stream `video-playback`).
- [ ] **PP-MASK-05 Masks with several outputs and alerts** (EW8-OBS-020, W03
  outputs). Steps: repeat single-mask and masked-apply cases with alternate or
  foldback output enabled and with an alert showing. Sela now: one audience
  output, no alerts. Re-run: none yet (stream `stage-display`).
- [ ] **PP-MASK-06 Transition durations** (EW8-OBS-020). Steps: measure the
  slide and mask transitions with a camera or frame capture (Options ›
  Transitions shows 00:500 blends). Sela now: cuts only. Re-run: none yet.
- [ ] **PP-MASK-07 Logo failure cases** (W03 failure records). Steps: set a
  logo, then delete or corrupt the image file and press Logo; also alternate
  Go Live and masks rapidly. Sela now: a failed logo keeps the previous one;
  without a logo, Logo refuses with a message. Re-run:
  `live-output-windows.py`, `prep-windows.py`.
- [ ] **PP-MASK-08 Video logo** (SRC-10 "Set as Logo Background"; M0-08).
  Steps: right-click a video › Set as Logo Background, press Logo; record
  playback. Sela now: picture logos only. Re-run: none yet (stream
  `video-playback`).

## Keyboard and focus (W04)

- [ ] **PP-KEY-01 Show keys while a text field has focus** (EW8-OBS-004,
  -018, -047). Steps: with focus in the Songs search box type plain b, c, l;
  then press Ctrl+B, Ctrl+L, Ctrl+C; repeat in the song editor and in a modal
  dialog; record clipboard and audience separately. Sela now (best guess):
  plain letters are text, Ctrl+B/L do nothing and Ctrl+C copies while a text
  field has focus (`SelaShow && !SelaTextInput`). Re-run: `search-windows.py`,
  `a11y-windows.py`.
- [ ] **PP-KEY-02 Down from the search box** (EW8-OBS-047). Steps: type a
  query, press Down; record whether focus moves into the list. Sela now: no
  rule is recorded or tested for Down in the box. Re-run: `search-windows.py`.
- [ ] **PP-KEY-03 Preview Up/Down and PageUp targets** (EW8-OBS-049, -050).
  Steps: Preview-focused Up/Down on a multi-line slide; Live-focused PageUp on
  the second schedule item; Schedule-focused PageUp. Sela now (best guess):
  Up/Down move the Preview selection; PageUp loads the previous schedule item
  at slide 1. Re-run: `navigation-windows.py`.
- [ ] **PP-KEY-04 Focus after Page Down in the Schedule** (EW8-OBS-048).
  Steps: Schedule focused, Page Down; record which pane has focus. Sela now:
  Go Live focuses the Live pane. Re-run: `navigation-windows.py`.
- [ ] **PP-KEY-05 Label jump keys** (EW8-OBS-052; M0-02 "label jump keys").
  Steps: with a saved song whose labels are Verse 1/Chorus, Live focused,
  press V 1 Enter, C Enter, B 1 Enter (and Shift+B), with Preview focused too.
  Sela now (best guess): C/E/I/M/R/T/V + number + Enter on the Live pane jump
  Live; Bridge is Shift+B because bare B is Black. Re-run:
  `navigation-windows.py`.
- [ ] **PP-KEY-06 Focus after Alt+Tab and modal dialogs** (W04). Steps: from
  each pane, Alt+Tab away and back, open/close a dialog; record focus and
  Tab/Shift+Tab traversal. Sela now: Esc returns focus to the opener; the
  documented 31-stop Tab order. Re-run: `a11y-windows.py`,
  `resource-browser-windows.py`.
- [ ] **PP-KEY-07 Song editor close prompt and the main window** (unobserved,
  operator-shell.md). Steps: open the editor's unsaved-changes prompt, click
  the main window; record whether the prompt comes forward and whether the
  main window accepts input. Sela now: the operator takes no input while the
  prompt is open and does not bring it forward. Re-run:
  `song-metadata-windows.py`.

## Displays, routing and hardware (W05, M0-04, G01)

- [ ] **PP-DSP-01 Output routing and focus** (EW8-OBS-007; W05). Steps: move,
  resize, minimize and occlude the main window; search and edit while
  watching the audience; record whether focus is ever stolen or output lands
  on the operator display. Sela now: audience window is non-activating on
  Windows (asserted by `live-output-windows.py`). Re-run:
  `live-output-windows.py`, `audience-smoke-windows.py`.
- [ ] **PP-DSP-02 Hotplug, sleep/resume, topology reorder** (W05; M0-04,
  M1-10). Steps (EasyWorship, then Sela): unplug and replug the audience
  display during Live, sleep/resume, change the primary monitor; record
  warnings and where output goes. Sela now: untested; a missing saved monitor
  falls back to the first non-primary, never the primary. Re-run:
  `output-spike-windows.py`, `options-windows.py`, `live-output-windows.py`.
- [ ] **PP-DSP-03 Device loss and present pacing** (M0-04, M0-07). Steps:
  driver reset (or GPU switch) during Live; measure frame intervals with a
  physical capture under UI stalls. Sela now: not measured. Re-run:
  `output-spike-windows.py`.
- [ ] **PP-DSP-04 Mixed-DPI window restore** (layout-persistence, M1-02). Steps:
  maximize the operator on a monitor whose scale differs from the primary,
  quit and restart; also restore a non-maximized window there. Sela now: the
  non-maximized case passes (`layout-windows.py` run 5); the maximized case is
  unit/GPUI only; song editor windows always open centered on the primary
  (not saved). Re-run: `layout-windows.py`.
- [ ] **PP-DSP-05 G01 physical acceptance** (M0-10). Steps: the G01 handoff in
  reference-observations.md on the named dual-display setup: text+image
  composition, masks, mixed DPI, hotplug, UI stalls, overload, renderer exit
  and device loss with timestamps. Sela now: implemented-unqualified slices
  only. Re-run: `output-spike-windows.py`, `prep-windows.py`,
  `live-output-windows.py`.
- [ ] **PP-DSP-06 Physical projector and older hardware** (wave-2 records;
  M1-07, M1-03/M1-04). Steps: run the live and search scripts on a real
  projector and on an older integrated-GPU laptop; record frame timing and
  search keystroke-to-paint. Sela now: measured only on the i7-14650HX laptop.
  Re-run: `live-output-windows.py`, `search-windows.py` (release, 20k
  library), `resource-browser-windows.py` (release).

## Song editor and formatting (W06)

- [ ] **PP-ED-01 Master edit reach** (EW8-OBS-043, -057; M0-02 "Master edit
  reach"). Steps: in Edit Slide Layouts change the Master background; check a
  slide without its own background, a slide with one, and another song on the
  same theme; record how the Layouts tab closes. Sela now (best guess):
  slides without a background follow the song master; slides with one keep
  it; the master is song-local. Re-run: `song-editor-windows.py`.
- [ ] **PP-ED-02 Ctrl+A tabs and whole-song undo/replace** (EW8-OBS-035,
  -036, -058; M1-05g). Steps: Ctrl+A in Words and in Slides, record which
  Format tabs show; undo a whole-song format change; type over the whole-song
  selection. Sela now: Ctrl+A shows the Slide tab with Text reachable; one
  undo step per whole-song change; typing replaces the song with one slide.
  Re-run: `song-editor-windows.py`.
- [ ] **PP-ED-03 Format pane control behavior** (EW8-OBS-026–033). Steps: with
  the real mouse, click B/I/U, outline/shadow sliders and dials, the Size
  step buttons; record step amounts and what is saved. Sela now: implemented
  from captures (M1-05g3a). Re-run: `song-editor-windows.py`.
- [ ] **PP-ED-04 Aspect Auto and gradient fill** (EW8-OBS-038, -040). Steps:
  choose an image, record what Auto does for 4:3, 16:9 and 21:9 images;
  inspect Gradient Fill. Sela now: Zoom is the default; Gradient Fill is
  unavailable (M1-05p, M1-05s). Re-run: `song-editor-windows.py`,
  `live-output-windows.py`.
- [ ] **PP-ED-05 Splitting and pagination** (EW8-OBS-012, M1-06). Steps: type a
  long verse that overflows a slide; record whether EasyWorship splits,
  shrinks or clips, and any manual break. Sela now: overflowing sections
  paginate into balanced audience pages. Re-run: `arrangement-windows.py`.
- [ ] **PP-ED-06 Copyright strip details** (EW8-OBS-055, -063). Steps: fill
  Administrator and Song Number, check the strip; toggle "Show on first
  displayed slide only"; fill License Info. Sela now (best guess): strip on
  every slide, Administrator and Song Number not drawn, Song Number holds the
  CCLI number; the first-slide option is not built. Re-run:
  `song-metadata-windows.py`.
- [ ] **PP-ED-07 Arrangement editor** (M1-06d). Steps: observe EasyWorship's
  arrangement/order controls, if any, and the editor's behavior for repeated
  sections. Sela now: Arrangement tab (GPUI tests only; no native editor
  run). Re-run: `arrangement-windows.py`; a native editor case is still to be
  written.

## Songs library, search and sort

- [ ] **PP-LIB-01 Search box title echo** (EW8-OBS-044). Steps: find the
  trigger for the box showing the previewed title. Sela now: a plain empty
  search field. Re-run: `search-windows.py`.
- [ ] **PP-LIB-02 Search fields** (EW8-OBS-046). Steps: with author and
  copyright filled, search for a word that is only there. Sela now: matches
  author, copyright, license, administrator and song number; slide labels
  are indexed since schema 7 (songs-browser). Re-run: `search-windows.py`,
  `song-metadata-windows.py`.
- [ ] **PP-LIB-03 Sort direction default** (EW8-OBS-064). Steps: read the
  checked direction under Sort by on a fresh profile. Sela now: Ascending;
  every observed key plus Date modified. Re-run: `resource-browser-windows.py`,
  `songs-browser-windows.py`.
- [ ] **PP-LIB-04 Update items in Schedule wording** (EW8-OBS-061). Steps: edit
  and save a scheduled song, choose Update items in Schedule; record the
  dialog (this needs one save in a disposable profile). Sela now (best guess):
  a reviewable old/new list with Yes/No. Re-run: `schedule-remainder-windows.py`.

## Schedule and schedule files

- [ ] **PP-SCH-01 Multi-select and drag reorder** (EW8-OBS-060). Steps:
  Ctrl/Shift-click schedule items, Move Up/Down and Remove on the selection;
  drag an item inside the Schedule (insertion line, autoscroll, Esc). Sela now
  (best guess): Shift range, Ctrl toggle, group move, insertion line,
  autoscroll, Esc cancels. Re-run: `schedule-remainder-windows.py`.
- [ ] **PP-SCH-02 Title bar with a schedule file** (EW8-OBS-067). Sela now:
  "name.selaschedule — Sela", `*` when unsaved, "Untitled schedule* — Sela".
  Re-run: `schedule-windows.py`.
- [ ] **PP-SCH-03 Save/Open default folder** (EW8-OBS-068). Sela now: beside
  the current file, else beside the newest recent file, else
  `<profile>\Schedules`. Re-run: `schedule-windows.py`.
- [ ] **PP-SCH-04 Open Recent labels and count** (EW8-OBS-069). Sela now: file
  name only, up to 10, newest first. Re-run: `menu-bar-windows.py`.
- [ ] **PP-SCH-05 Schedule item types beyond songs** (M1-09). Steps: add
  scripture, media and presentation items; record their rows and menus. Sela
  now: songs only. Re-run: none yet (streams `scripture`, `media-core`).

## Options and settings

- [ ] **PP-OPT-01 Unread Options pages** (EW8-OBS-056). Steps: scroll Main
  Output › Scripture to the end, open Transitions › Show More, read every
  Alternate Output and Foldback tab, scroll Slide Labels below <EMPTY>.
  Sela now: Options mirrors the read parts only. Re-run: `options-windows.py`.

## Media and images

- [ ] **PP-MED-01 Media tab and image items** (M1-11; EW8-OBS-019). Steps:
  observe Media › Images/Videos lists, thumbnails, import and relink of a
  missing file, and an image item live with Clear. Sela now: Media › Images
  imports pictures for the logo, with the preflight report and Locate…;
  image items cannot go live yet. Re-run:
  `preflight-windows.py`, `live-output-windows.py`.
- [ ] **PP-MED-02 Native Locate… and large imports** (M1-11b record). Steps:
  run Locate… through the native file chooser, import a 24 MP photo and
  cancel an import. Sela now: covered by GPUI/unit tests only. Re-run:
  `preflight-windows.py` (extend for Locate…).

## Import and scripture cores

- [ ] **PP-IMP-01 EasyWorship song storage details** (import-ew-rtf-words).
  Steps: in a disposable profile, save songs with non-ASCII lyrics and
  per-slide formatting and with a legacy "Verse 1" first line; inspect how
  EasyWorship stores them. Sela now: the RTF words parser follows the
  documented format only; these cases are unobserved (docs/import.md).
  Re-run: none yet (stream `ew-library-import`).
- [ ] **PP-BIB-01 Scripture reference entry** (VAL-BIBLE-010). Steps: type
  references in the Scriptures tab: completion order and accept key, "Jude 5",
  "John 3 16", "John 3:16-4:2". Sela now: parser only; `John 3 16` is
  malformed, no UI. Re-run: none yet (stream `scripture`).

## Windows-only Sela runs

- [ ] **PP-WIN-01 Narrator pass** (M1-16, M1-07, M5-03). Steps: walk the
  operator, Options, song editor and search box with Narrator; record roles,
  names and toggle states heard. Sela now: UIA tree verified by
  `a11y-windows.py` and `uia-names-windows.ps1`, not with a screen reader.
  Re-run: `a11y-windows.py`.
- [ ] **PP-WIN-02 Indonesian visual pass** (schedule-files, preflight,
  schedule-remainder records). Steps: switch to Bahasa Indonesia and inspect
  the schedule file dialogs, the preflight report and Update items for
  clipping. Re-run: `i18n-windows.py`.
- [ ] **PP-WIN-03 Song editor modal close user test** (SC-SONG-03). Steps: owner
  test of the editor close prompt with receipt counts (VAL-FOCUS-001). Re-run:
  `song-metadata-windows.py`.
- [ ] **PP-WIN-04 Power-loss and installer** (M1-01, M1-16). Steps: pull power
  during a save and a migration on a disposable profile; install with the
  Windows installer when it exists. Sela now: process-abort tests only; no
  installer. Re-run: none (manual).

## Stream: scrutiny-r2

## Stream: import-framework

## Stream: ew-library-import

## Stream: autosave-recovery

## Stream: bundle-backup-restore

## Stream: songs-browser

Sela state after the stream (2026-10-22): Sort by Title/Author/Copyright/
Administrator/Song Number (+ Date modified), schema 7 slide-label search and
Options › Main Output › Song › Copyright Info. Linux evidence:
`scripts/songs-browser-x11.py` PASS on Xvfb; nothing below is closed by it.

- [ ] **PP-SB-01 Sort by order rules** (EW8-OBS-064). Steps: in a disposable
  profile create (or import) four songs: one without an author, two with
  authors differing only in case ("alpha writer", "Bravo Writer"), Song
  Numbers 7, 10 and 0042, one Copyright starting "(c)" and one "©". Choose
  Sort by > Author, Copyright, Administrator, Song Number, each Ascending and
  Descending; record the order and where empty keys go, and whether Song
  Number sorts as text or number. Sela now (best guess): empty keys last in
  both directions, ASCII case ignored, Song Number numeric, ties by Title.
  Re-run: `songs-browser-windows.py`, `resource-browser-windows.py`.
- [ ] **PP-SB-02 Sort by with a search active** (EW8-OBS-045/064). Steps: type
  a query matching several songs, then change Sort by; record whether the
  results reorder. Sela now: search results stay in relevance order; Sort by
  applies to the unfiltered list. Re-run: `songs-browser-windows.py`.
- [ ] **PP-SB-03 Sort key shown in the list** (EW8-OBS-064). Steps: after Sort
  by Author, record whether the Songs list shows an author column or text.
  Sela now: titles only. Re-run: `songs-browser-windows.py`.
- [ ] **PP-SB-04 Label search ranking** (EW8-OBS-046). Steps: with one song
  titled "Glory Road" and another whose slide label is "Glory", type "glo";
  record the result order; then type a label number ("verse 2"). Also add a
  long song (many slides) with "glory" once in its lyrics and record whether
  it lists above or below the label-only song. Sela now: title match first,
  then songs with a lyric or metadata hit, and label-only matches after all
  of them whatever the song lengths; "verse 2" needs both words.
  Re-run: `songs-browser-windows.py`, `search-windows.py`.
- [ ] **PP-SB-05 "Show on first displayed slide only"** (EW8-OBS-055/056).
  Steps: tick it in Options › Main Output › Song; send a song with copyright
  text Live starting at slide 3 (click slide 3, Go Live), then step to slides
  4 and 1; record which slides show the strip. Sela now (best guess): only
  slide 1 of the item shows it, whichever slide went live first. Re-run:
  `songs-browser-windows.py`.
- [ ] **PP-SB-06 License Info text on the output** (EW8-OBS-055/063). Steps:
  type "CCLI License # 1234567" in License Info, OK, send a song with a Song
  Number and copyright text Live; capture the strip. Sela now (best guess): a
  third line "CCLI Song # N · CCLI License # 1234567". Re-run:
  `songs-browser-windows.py`, `song-metadata-windows.py`.
- [ ] **PP-SB-07 Options change while a song is Live** (EW8-OBS-055). Steps:
  with a copyright song Live, untick Display Copyright Information and OK;
  record whether the live slide redraws at once or only on the next slide.
  Sela now: only the next cue changes; the live slide stays (GPUI test, and
  the Linux script with first-slide-only and License Info changed). Re-run:
  `songs-browser-windows.py`.
- [ ] **PP-SB-08 Copyright Font, Song Font and Show Verse/Chorus Label**
  (EW8-OBS-056). Steps: open each menu in Options › Main Output › Song and
  record the choices and their effect on output. Sela now: not built (the
  strip uses the slide face). Re-run: none yet.
- [ ] **PP-SB-09 Windows run of `songs-browser-windows.py`** (NOT RUN: no
  Windows host on 2026-10-22). Steps: seed with
  `seed_library.exe --sort-songs`, run the script with the audience on the
  secondary monitor; it covers mouse and keyboard Sort by with UIA ticks,
  label search, the Options Song tab, the strip ink on slides 1 and 2 with
  first-slide-only, and (since 2026-10-23) a real Options change typed with
  B and "." into License Info while slide 2 is Live (zero frame changes
  expected). Sela now: the Linux script PASSES; since 2026-10-23 it also
  keeps a song Live and checks that typing in the search box and License
  Info and OK leave the audience frame unchanged (no strip ink check there,
  the xvfb output is 351 px tall). Re-run: itself.
- [ ] **PP-SB-10 20k search and sort timings on the reference laptop**
  (M1-07, M1-04). Steps: `cargo run --locked --release --example search_bench`
  and `search-windows.py --latency-library` on the Windows machine of the
  2026-10-13 runs and on the older integrated-GPU laptop. Sela now: only a
  Linux VM run with opt-level overrides (work log, songs-browser record).
  Re-run: `search-windows.py`, `resource-browser-windows.py`.

## Stream: media-core

## Stream: video-playback

## Stream: scripture

## Stream: themes

## Stream: stage-display

## Stream: fuzz-audit

## Windows script run order

Prerequisites for every run: Windows 11 with Python 3 on PATH, the repository
root as the working directory, `cargo build --locked` and `cargo build --locked
--examples` (debug `target\debug\sela.exe` plus `seed_library.exe`,
`edit_song.exe`, `output_spike.exe`), the owner's keyboard and mouse idle for
the whole run (the scripts verify the foreground before each input and fail on
a theft), no other Sela instance, and the three-monitor setup of the latest
records (DISPLAY5 primary operator, DISPLAY1 laptop panel as the audience,
DISPLAY6 spare) unless noted. Each script seeds its own library into a
temporary folder and uses a private profile. Run them one at a time, in this
order (smoke first, then shell, then live/schedule flows, then editor, then
timing):

| # | Script | Extra prerequisites |
| --- | --- | --- |
| 1 | `native-smoke-windows.py target\debug\sela.exe` | none (one monitor is enough) |
| 2 | `audience-smoke-windows.py` | a non-primary monitor |
| 3 | `output-spike-windows.py` | `output_spike.exe`; two monitors |
| 4 | `a11y-windows.py` | seed (`--second-song`); audience on the secondary |
| 5 | `chrome-windows.py` | seed; a monitor that fits a 1600x1000 client |
| 6 | `menu-bar-windows.py` | seed; secondary monitor |
| 7 | `options-windows.py` | seed; spare monitor (`--target`, default the non-primary that is not DISPLAY1) |
| 8 | `i18n-windows.py` | seed; audience as a window (`SELA_AUDIENCE_MONITOR=window` set by the script) |
| 9 | `layout-windows.py` | seed; spare monitor for run 5 |
| 10 | `navigation-windows.py` | seed; secondary monitor |
| 11 | `live-output-windows.py` | seed (`--formatted-song`, `--background-song`); unobstructed secondary monitor |
| 12 | `schedule-windows.py` | seed (`--second-song`); secondary monitor; native Save/Open dialogs |
| 13 | `schedule-remainder-windows.py` | seed (`--second-song`, `--extra-songs`); `edit_song.exe`; secondary monitor |
| 14 | `arrangement-windows.py` | seed (`--arranged-song`); secondary monitor |
| 15 | `operator-menu-windows.py` | seed (`--second-song`); native Open dialog |
| 16 | `preflight-windows.py` | seed (`--vanished-schedule`); secondary monitor |
| 17 | `song-editor-windows.py` | none beyond `sela.exe` |
| 18 | `song-metadata-windows.py` | seed (`--metadata-songs`, `--vanished-schedule`); secondary monitor |
| 19 | `search-windows.py` | seed (`--second-song`); for latency also `cargo build --release` and a 20k library from `cargo run --locked --release --example search_bench -- --keep PATH` (`--binary target\release\sela.exe --latency-library PATH`) |
| 20 | `resource-browser-windows.py` | seed (`--second-song`, `--extra-songs`); release binary and a 20k library for the scrolling timings |
| 20a | `songs-browser-windows.py` | seed (`--sort-songs`); audience on the secondary monitor |
| 21 | `prep-windows.py` | release binary (`--binary target\release\sela.exe`); seed (`--heavy-song`); audience on the laptop panel (`--monitor name:\\.\DISPLAY1`, the default) |
| 22 | `reference-w03-windows.py` | not a Sela check: EasyWorship 8.0.49 open and maximized with one song previewed from the Songs tab; observation only |

Linux counterparts, where they exist, are named `scripts/<feature>-x11.py`
(see [e2e-linux.md](e2e-linux.md)); a Linux pass does not replace these runs.
