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

- [ ] **PP-SC2-01 Keys while a context menu is open** (SC2-FOCUS-01;
  EW8-OBS-010, -018). Steps: with A1 live, right-click a Songs row so its
  context menu opens, then press B, period, Ctrl+B and Page Down; repeat with
  the Schedule item menu open; record whether the audience or the toolbar
  Black state changes and whether the menu stays open; also open the
  toolbar New ▾ dropdown and the Media image row menu and press the same
  keys, then Esc. Sela now: the Songs row menu, the Schedule item menu
  (Duplicate/Remove) and the image menu take the focus, and while any of
  them or New ▾ is open every show shortcut (B, period, Ctrl+B/C/L, Page
  Down, arrows, Home/End, Ctrl+Delete) is inert; Esc closes the menu and
  returns the focus to the row or button that opened it (SC2-FOCUS-01, -02;
  UNOBSERVED best guess). Also on New ▾: with its menu open, press the left
  button on New ▾, drag away and release, then press Enter (and separately
  right-press New ▾, then left-click it); record whether the menu opens.
  Sela now: the press closes it, and the next Enter or left click opens it
  (SC2-FOCUS-03). Re-run: `scenario-s02-windows.py` (Songs, Schedule item and
  New ▾ menus).
- [ ] **PP-SC2-02 Next/Previous after a Go Live that did not show**
  (SC2-LIVE-01; EW8-OBS-014, -050). Steps: hard to force in EW; the nearest
  case is Go Live on an item whose background media is missing. Record
  which slide Next/Previous then send and what the Live pane footer says.
  Sela now: until a slide of the new live item has been shown, Next/Previous
  send nothing and say "<title> is not on screen yet; use Go Live first"; the
  footer says "<title> · not sent" (UNOBSERVED best guess). Re-run:
  `navigation-windows.py`, `scenario-s02-windows.py`.
- [ ] **PP-SC2-03 Go Live on a duplicate schedule item while the original is
  stepping** (SC2-LIVE-02; EW8-OBS-013). Steps: schedule A1 twice, take the
  first live, press Right and immediately select the second A1 and Go Live;
  record whether the first item's next slide flashes on the audience. Sela
  now: a Go Live on another entry, even of the same song revision, drops the
  earlier entry's cues still being prepared, so only the duplicate's slide is
  shown. Re-run: `schedule-remainder-windows.py`.
- [ ] **PP-SC2-04 Live Page Down then a Songs click** (SC2-LIVE-03;
  EW8-OBS-050). Steps: with schedule item 1 live and the Live pane focused,
  press Page Down and at once click a song in Songs; then press Page Down in
  Live again; record which schedule item goes live. Sela now: picking a song
  cancels the waiting Page Down, and the next Page Down goes to the item
  after the one on screen. Re-run: `navigation-windows.py`.
- [ ] **PP-SC2-05 Screen reader announcement of dialogs and the window
  title** (SC2-A11Y-01, -02; VAL-A11Y). Steps: with Narrator on, open the
  unsaved-schedule prompt (Ctrl+Q with an unsaved schedule), the Delete song
  confirmation, Options and the Song Editor close prompt; then add an item to
  an untitled schedule. Record what Narrator reads. Sela now: each dialog is a
  UIA Window element (AccessKit Dialog) named with its question (the editor
  prompt is named "Save changes before closing?" and carries "Unsaved
  changes. Save, discard, or keep editing." as its description; Delete song
  carries its note "Schedules that use this song keep their copy. Live
  output does not change." and a file error its message), and the title change reaches
  UIA with the next frame Sela requests. Re-run: `a11y-windows.py`,
  `scenario-s01-windows.py` (unsaved prompt, Options, editor close prompt).
- [ ] **PP-SC2-06 Indonesian window title** (SC2-I18N-01). Steps: switch to
  Bahasa Indonesia, add an item to an untitled schedule, read the title bar.
  Sela now: "Sela — Pratinjau teknis" and "Jadwal tanpa judul* — Sela"
  (EW8-OBS-067 itself is still unobserved). Re-run: `i18n-windows.py`.
- [ ] **PP-SC2-08 Normalize text size while a Go Live is in flight**
  (SC2-LIVE-04; EW8-OBS-014). Steps: with item A live, select item B, press
  Go Live and immediately click "Normalize text size across slides"; record
  the size B's first slide lands with and whether it changes on the next
  slide; also under Black double-click a Live slide and click Normalize at
  once, and record whether the mask lifts. Sela now: the toggle rebuilds the
  newest requested slide (B's Verse 1 here) in place, so B lands once, with
  the new sizing and no intermediate old-sizing frame, and a double-click
  unmask still lifts the mask over that slide (SC2-LIVE-04, -05; UNOBSERVED
  best guess; the timing window is short on fast hardware). Re-run:
  `live-output-windows.py`.
- [ ] **PP-SC2-09 Song Editor close prompt wording** (SC2-I18N-02;
  EW8-OBS-012). Steps: edit a song, press Cancel (and close the editor
  window), read the prompt and press each of Yes, No and Cancel in turn on a
  fresh edit. EW 8.0.49 observed: "You have unsaved changes. Would you like
  to save changes before closing?", Yes/No/Cancel, No discarded. Sela now:
  heading "Save changes before closing?" (ID "Simpan perubahan sebelum
  menutup?"), Yes saves and closes, No discards, Cancel or Esc keeps editing;
  it said "Close without saving?", which inverted Yes and No. Re-run:
  `scenario-s01-windows.py`, `song-metadata-windows.py`.
- [ ] **PP-SC2-07 Run the S01 and S02 Windows scenarios** (not run: no
  Windows host on 2026-10-22). Steps: `python scripts\scenario-s01-windows.py`
  then `python scripts\scenario-s02-windows.py` with a secondary monitor and
  the keyboard idle; attach `.amp/in/artifacts/scenario-s0{1,2}-windows/`.
  Sela now: both PASS on Linux/X11 (`scenario-s0{1,2}-x11.py`, xvfb `:111`,
  2026-10-22). Re-run: these two scripts.

## Stream: import-framework

Added 2026-10-22 (M1-14, M4-07 OpenSong/ChordPro; docs/import.md "Import
framework", reference-observations "Song import — UNOBSERVED best guesses").

- [ ] **PP-IMPF-01 Where EW8 imports songs into the library**
  (EW8-OBS-IMP-1). Steps: in a disposable profile, open every top menu at a
  size where nothing is cut off (File, Edit, Live, Profiles, View, Help,
  including File › New ▸ and Profiles › Utilities ▸), the Songs library
  context menu, the Songs footer and the Song Editor's Import ▾; record each
  import entry, its submenu and dialog title. Sela now: File › Import ▸
  Songs… / Song Folder… after Close Schedule (best guess; EW8-OBS-053 shows
  no Import row). Re-run: `song-import-windows.py`, `menu-bar-windows.py`;
  GPUI `menu_contents_match_the_captures_plus_tracked_deviations`, whose
  DEVIATIONS table must lose the Import rows (or this item close) once EW
  is observed.
- [ ] **PP-IMPF-02 Formats and the chooser** (EW8-OBS-IMP-2). Steps: open the
  import dialog found in PP-IMPF-01; record its file-type filter list,
  whether several files or a folder can be chosen, and import one original
  OpenLyrics, OpenSong and ChordPro file each (fixtures from
  `tests/fixtures/import/batch/`). Record what EW keeps and drops (labels,
  order, chords, authors, CCLI). Sela now: those three formats, several
  files or folders, chords removed with a warning. Re-run:
  `song-import-windows.py`.
- [ ] **PP-IMPF-03 Preview or report before saving** (EW8-OBS-IMP-3). Steps:
  import the batch folder (including `broken.xml` and `empty.cho`); record
  whether EW shows a list, warnings or errors before or after saving, and
  what happens to the bad files. Sela now: staged preview with every file,
  its warnings and errors; nothing saved before Import. Re-run:
  `song-import-windows.py`.
- [ ] **PP-IMPF-04 Duplicates** (EW8-OBS-IMP-4). Steps: import the same
  folder twice, then a file with an existing title but other words; record
  EW's prompt wording, choices and default, and what the library holds
  afterwards (two songs, replaced, skipped). Sela now: same words → Skip,
  same title → Import as copy, Replace = new revision, bulk choice. Re-run:
  `song-import-windows.py`; GPUI `duplicate_choices_skip_copy_and_replace`.
- [ ] **PP-IMPF-05 After the import** (EW8-OBS-IMP-5). Steps: record which
  song is selected or previewed, where focus lands, whether the Songs list
  scrolls to the new songs and whether any message shows. Sela now: imported
  songs selected, first previewed and focused, counts in the Songs footer.
  Re-run: `song-import-windows.py`.
- [ ] **PP-IMPF-06 Cancel and partial failure** (EW8-OBS-IMP-6). Steps: start
  a large import (copy the batch folder 50 times) and cancel midway; record
  whether part of it was saved. Sela now: one transaction, nothing saved on
  cancel or failure. Re-run: `song-import-windows.py`; GPUI
  `cancel_while_files_are_read_commits_nothing`.
- [ ] **PP-IMPF-07 Windows run of `song-import-windows.py`** (NOT RUN: no
  Windows host on 2026-10-22). Steps: run it (see the run order table) and
  inspect `01-import-preview.png` and `05-audience-imported-slide.png`;
  check the common folder dialog accepts the typed path (the script presses
  Enter until it closes). Sela now: the Linux counterpart
  `song-import-x11.py` passes. Re-run: the script itself.
- [ ] **PP-IMPF-08 Indonesian and Narrator pass of the import dialog.**
  Steps: switch to Bahasa Indonesia, open the preview with duplicates and
  failures, check for clipping; with Narrator, Tab through the choice groups
  and confirm each is announced as a radio group with its title and the
  selected choice. Sela now: EN/ID strings and AccessKit roles exist, not
  heard with a screen reader. Re-run: `i18n-windows.py`, `a11y-windows.py`.
- [ ] **PP-IMPF-09 Quit while an import is open** (EW8-OBS-IMP-7). Steps:
  make an unsaved schedule change, open EW's import dialog found in
  PP-IMPF-01 and, with its list or progress showing, press Ctrl+Q, then try
  the window's close button; record whether EW closes, prompts about the
  schedule, or refuses, and whether the import survives. Sela now: the
  close is refused with "Finish or cancel the import before quitting." in
  the import dialog (its own line under any import error, which stays);
  after Cancel the usual Save / Don't Save / Cancel prompt appears. The same
  refusal holds when the preview was opened while an earlier quit waited
  for a settings write after Don't Save, or for a Save before quitting.
  Also observe in EW: if a quit is pending (prompt answered, still saving)
  can the import dialog be opened at all. Re-run: GPUI
  `quit_waits_for_an_open_import_and_a_replaced_preview_cancels_it`,
  `an_import_opened_during_the_settings_wait_after_dont_save_keeps_the_window`,
  `an_import_opened_during_a_save_then_quit_keeps_the_window`; native
  `python3 scripts/song-import-x11.py` (Ctrl+Q and Alt+F4 on the duplicate
  preview) and `scripts/song-import-windows.py` (Ctrl+Q and WM_CLOSE).

## Stream: ew-library-import

## Stream: autosave-recovery

Sela side: [autosave.md](autosave.md); best guesses EW8-OBS-071..078 in
reference-observations.md. Observe only on a disposable EW profile and
schedule; never on a service in use.

- [ ] **PP-AUTO-01 Does EW autosave schedules?** (EW8-OBS-071). Steps: on a
  disposable profile create a new schedule, add two songs, do not save; note
  the time; wait 1, 5 and 10 minutes, each time listing the profile folder
  (`%ProgramData%\Softouch\EasyWorship\<profile>\` and
  `%LOCALAPPDATA%`/`%APPDATA%` EasyWorship folders) with timestamps; record
  any new or changed file and its name. Sela now (best guess): revisions in
  `<profile>\Recovery\` 2 s after the last change, at most 10 s after the
  first. Re-run: `autosave-windows.py`.
- [ ] **PP-AUTO-02 EW after a forced termination** (EW8-OBS-072, -073).
  Steps: with an unsaved two-song schedule (and once with a saved schedule
  plus one unsaved change), end `EasyWorship.exe` in Task Manager (End task)
  or `taskkill /F`; relaunch; capture any prompt (wording, buttons, default
  button, Esc), which schedule appears, its title bar and unsaved marker,
  and whether Live/audience state comes back. Sela now (best guess): modal
  "Recover the unsaved schedule?" with Restore (default) / Don't Restore,
  lost-work disclosure, original file and baseline kept, Live untouched.
  Re-run: `autosave-windows.py`, then `cargo test --bin sela autosave_recovery`.
- [ ] **PP-AUTO-03 EW second instance** (EW8-OBS-074). Steps: with EW
  running, start `EasyWorship.exe` again (Start menu and a double-clicked
  `.ews` file); record whether the running window comes to the front, a
  message appears (capture its text), or a second window opens. Sela now:
  "Sela is already running" naming the profile, exits 0, no profile change.
  Re-run: `autosave-windows.py`.
- [ ] **PP-AUTO-04 EW profile/schedule backups and retention** (EW8-OBS-075).
  Steps: open the Profiles menu and Options; record every backup/restore
  item, its default folder and any count or age limit; list the profile
  folder for backup files after several saves. Sela now: newest 20 autosave
  revisions within 4 MiB, kept across sessions and after Don't Restore.
  Re-run: none (record only; M1-13 owns explicit backup/restore).
- [ ] **PP-AUTO-05 EW Exit with an unsaved schedule after a recovery**.
  Steps: after PP-AUTO-02, change the restored schedule and choose File ›
  Exit; record the prompt (EW8-OBS-070 wording) and whether a later launch
  asks about recovery again. Sela now: the normal unsaved guard; Don't Save
  marks the run clean and the next start asks nothing. Re-run:
  `autosave-windows.py`.
- [ ] **PP-AUTO-09 EW closed with a recovery prompt unanswered**
  (EW8-OBS-076). Steps: after a forced termination as in PP-AUTO-02, if EW
  shows a recovery prompt, close EW (title-bar X, and once Alt+F4) without
  answering it; relaunch and record whether the prompt returns, its wording
  (does it still say EW closed abnormally?) and whether the schedule is
  still offered. Also close EW as fast as possible after launch (before any
  prompt appears) and record the same. Sela now (best guess): nothing is
  committed or acknowledged, `session.json` says `offered`, and the next
  prompt starts "Sela was closed last time before this recovery question
  was answered." Re-run: `autosave-x11.py` / `autosave-windows.py` step 4,
  `cargo test --bin sela autosave_recovery`.
- [ ] **PP-AUTO-10 EW Profiles menu and Profiles Manager…** (EW8-OBS-077,
  M1-12b). Steps: on a disposable install open Profiles; record every item
  and its state; open Profiles Manager… and capture the dialog; create a
  profile "Test B", switch to it, rename it, switch back and delete it,
  recording each prompt (wording, buttons, default), whether a running
  schedule/Live output is closed or kept on switch, and which data (songs,
  media, schedules, settings, backups) is per profile (list
  `%ProgramData%\Softouch\EasyWorship\` before and after). Sela now: one
  implicit profile per library folder; Profiles items rendered inert; no
  create/switch/rename (M1-12b planned). Re-run: none yet (M1-12b adds its
  own scripts).
- [ ] **PP-AUTO-11 EW crash between a Save and the next autosave**
  (EW8-OBS-078). Steps: open a saved schedule, add an item and wait until
  EW has autosaved it (if PP-AUTO-01 shows EW autosaves at all), add one
  more item, press Ctrl+S and end EW with Task Manager within a second;
  relaunch and record whether a recovery copy is offered, its wording,
  whether EW compares it with the newer file, and what Restore then puts in
  the schedule (and whether saving overwrites the newer file). Sela now
  (best guess): the older revision is offered with "Its schedule file was
  saved again {ago}, after this autosave, so the file may hold newer work:
  restoring brings back this older copy, and saving it replaces the file."
  instead of "never saved". Re-run: `cargo test --bin sela
  a_file_saved_after_the_last_autosave`, `cargo test --lib
  a_file_saved_after_the_offered_revision`.
- [ ] **PP-AUTO-06 Run `autosave-windows.py`** (M1-12, S03; NOT RUN: no
  Windows host on 2026-10-22). Checks LockFileEx as the profile lock and
  TerminateProcess as the kill, which the Linux run (`autosave-x11.py`,
  PASS) cannot. Prerequisites: seed (`--second-song`), secondary monitor.
- [ ] **PP-AUTO-07 Real Windows failure conditions** (M1-12 checklist 5).
  Steps: on a disposable profile, (a) put the profile on a small nearly full
  VHD and edit until autosave fails, then free space; (b) deny write on
  `Recovery\` with an ACL; (c) hold `Recovery\session.json` open with a
  sharing lock (e.g. PowerShell `[IO.File]::Open(..., 'Open', 'Read', 'None')`);
  record the footer notes and that editing and Live continue; (d) end Sela
  with TerminateProcess after an unsaved edit has been autosaved, then
  relaunch while (c)'s lock is held: the recovery prompt must still appear
  with the "Autosave is off (…), so this answer cannot be recorded" line and
  Restore must work read-only. Sela now: fault-adapter tests only
  (`autosave::tests`, `tests::autosave_recovery`).
  Re-run: the same steps after any fix.
- [ ] **PP-AUTO-08 Power loss and the autosave hitch** (M1-12 acceptance,
  P/H). Steps: (a) pull power (or hard-reset a VM) during repeated
  autosaves on a disposable profile, then reopen and record the prompt and
  restored schedule; (b) during the 90-minute rehearsal on the older
  integrated-GPU laptop, edit the schedule every few seconds while Live
  output runs a slide sequence and record frame pacing around commits
  (`frame_log`). Sela now: process-kill tests (SIGKILL, Linux) and a GPUI
  check that Live receipts arrive while a commit is held; no physical
  power-loss or hardware timing evidence. Re-run: `autosave-windows.py`,
  `prep-windows.py`.

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
  Sela now: only the next cue changes; OK sends no renderer cue (GPUI test
  `typing_in_license_info_never_toggles_masks_or_cues_live`, submission
  count). The Linux script only shows the frame stays the same, which cannot
  reveal a re-cue there (no strip on the 351 px xvfb output, so a re-sent
  slide is identical); the Windows script would see slide 2 gain the strip.
  Re-run: `songs-browser-windows.py`.
- [ ] **PP-SB-08 Copyright Font, Song Font and Show Verse/Chorus Label**
  (EW8-OBS-056). Steps: open each menu in Options › Main Output › Song and
  record the choices and their effect on output. Sela now: not built (the
  strip uses the slide face). Re-run: none yet.
- [ ] **PP-SB-09 Windows run of `songs-browser-windows.py`** (NOT RUN: no
  Windows host on 2026-10-22). Steps: seed with
  `seed_library.exe --sort-songs`, run the script with the audience on the
  secondary monitor; it covers mouse and keyboard Sort by with UIA ticks,
  label search, the Options Song tab, the strip ink on slides 1 and 2 with
  first-slide-only, and (since 2026-10-23) a real Options change typed into
  License Info while slide 2 is Live (zero frame changes expected; a re-cue
  on OK would add the strip to slide 2). Since review round 2 the License
  Info text is typed as one virtual-key press per character (B and "." as
  VK_B and VK_OEM_PERIOD), because `KEYEVENTF_UNICODE` input reaches GPUI
  (zed a846890 `crates/gpui_windows/src/events.rs`, `VK_PACKET => None`)
  only as WM_CHAR text and never runs the keymap. Sela now: the Linux script
  PASSES; since 2026-10-23 it also keeps a song Live and checks that typing
  in the search box and License Info and OK leave the audience frame
  unchanged (no strip ink check there, the xvfb output is 351 px tall, so it
  cannot see a re-cue on OK; the GPUI test counts cues). Re-run: itself.
- [ ] **PP-SB-10 20k search and sort timings on the reference laptop**
  (M1-07, M1-04). Steps: `cargo run --locked --release --example search_bench`
  and `search-windows.py --latency-library` on the Windows machine of the
  2026-10-13 runs and on the older integrated-GPU laptop. Sela now: only a
  Linux VM run with opt-level overrides (work log, songs-browser record).
  Re-run: `search-windows.py`, `resource-browser-windows.py`.
- [ ] **PP-SB-11 A License Info longer than the output width**
  (EW8-OBS-055/063; pr1-review 2026-10-23). Steps: type a License Info of
  about 250 characters, OK, send a song with copyright text and a Song
  Number Live at 1920x1080 and at a small output (e.g. 854x480); capture the
  strip; record whether EW wraps, shrinks, cuts or hides the licensing line
  and whether the "Title - Author" and copyright lines change size. Sela now
  (best guess): the title and copyright lines keep the size they have
  without License Info; a licensing line wider than the strip is cut at a
  character and ends in "…" (dropped when not even that fits). Re-run:
  `cargo test --lib a_long_license_line_is_cut`, `songs-browser-windows.py`.

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
| 23 | `song-import-windows.py` | seed; secondary monitor; native folder dialog; the batch fixtures in `tests\fixtures\import\batch` |
| 24 | `scenario-s01-windows.py` | none seeded (fresh profile); secondary monitor; native Save dialog |
| 25 | `scenario-s02-windows.py` | seed (`--second-song`); secondary monitor |
| 26 | `autosave-windows.py` | seed (`--second-song`); secondary monitor; no other Sela instance (it starts a second one on purpose) |

Linux counterparts, where they exist, are named `scripts/<feature>-x11.py`
(see [e2e-linux.md](e2e-linux.md)); a Linux pass does not replace these runs.
