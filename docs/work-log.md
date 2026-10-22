# Work log and resume point

Use [backlog.md](backlog.md) for ticket checklists and test boundaries. This file
records execution state, not a second copy of the backlog. Dates use ISO format;
include timezone for timed hardware/rehearsal evidence.

## Current state

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
| M0-03 | implemented-unqualified | Three real Operator action/focus tests and repeated native X11 smoke; domain fixture extension and Windows/accessibility checks open. |
| M0-04 | implemented-unqualified | Separate audience process measured under UI stalls, synthetic preparation and operator exit/kill on Linux and Windows DX12 physical displays, incl. mixed-DPI moves, fullscreen, both GPUs, and an idle-machine rerun; hotplug, device loss, present pacing open. |
| M0-05 | implemented-unqualified | CPU snapshots, bounded workers and native resource shaping/upload checked; production intent coordination and hardware qualification open. |
| M0-06 | implemented-unqualified | Ordered bounded native owned-resource IPC/receipts and failure/expiry retention, passed on Linux GL and Windows DX12; production safety coordination/supervision and reference mask semantics open. |
| M0-07 | implemented-unqualified | Explicit-font/image native worker preparation/submission plus static/FFV1 GPU readback; resized output, transitions, performance and physical qualification open. |
| M0-08 | implemented-unqualified | M0-08a mask layer and logo slot: `masks` (None/Black/Logo plus independent Clear), Safety-lane masks and Cue-lane logo over the delivery wire (kinds 5 MASK, 6 LOGO), per-lane ordering, audience Black/Clear/Logo rendering. Operator side is M1-10c. Native hardware capture, reference mask semantics for combined masks (EW8-OBS-020) and video logo (M2-03) open. |
| M1-01 | implemented-unqualified | Schema 5 (search index, M1-07a) after schema 4 (backgrounds, M1-05h1) and schema 3 (formats, M1-05g1); schema-2 section/arrangement persistence, verified backup-gated migration/fresh restore and process-abort tests; remaining schemas, destructive migrations/recovery UI and power-loss qualification open. |
| M1-02 | implemented-unqualified | Separate-pane contemporary shell, documented-reference toolbar correction and M1-02d menu bar (EW order, Alt+mnemonic and mouse access, action parity, native script passed); layout persistence (splitters, collapsed, window placement; native PASS) done; modes/installed-reference/mixed-DPI checks open. |
| M1-03 | implemented-unqualified | Contextual keyboard access to shell and menu commands mapped to GPUI actions/key contexts with native checks; text-entry/modal/selection/live command ownership open. |
| M1-05 | implemented-unqualified | Persistent metadata/section authoring, full document undo/redo, receipt-gated OK; M1-05e EW-observed Words layout with off-thread rendered preview; M1-05f rendered Slides thumbnails (Windows native check); M1-05g implemented-unqualified (g1 per-slide format storage, g2 styled rendering with the operator font gate, Windows DX12 native check passed; g3a Format pane and g3b Ctrl+A whole-song selection with replace-typing, GPUI and Windows native checks). M1-05h implemented-unqualified (h0 RUN-W06I observed, h1 model and schema 4, h2 rendering with the operator background gate and Windows native check, h3 Slide pane editing with the master's Layouts tab and Windows native check), M1-05i implemented-unqualified (operator song menu, GPUI tests and Windows native script), M1-05j–o (planned format controls and tabs), M1-05p–s (planned: gradient, opacity/rotate/flip, Theme Elements/layouts, Auto aspect and image tiles), drag selection across cells, IME/accessibility qualification open. M1-05 song metadata (song-metadata-fields, 2026-10-18): implemented-unqualified (schema 6, Administrator/Song Number inputs and Inspector rows, audience copyright strip, metadata search, Date modified stamp, editor close modal; native `song-metadata-windows.py` PASS). |
| M1-06 | implemented-unqualified | Stable IDs, immutable domain, backed-up migration, editor data/undo persistence; M1-06d adds the editor Arrangement tab, per-schedule-item arrangement (`.selaschedule` v2), deterministic overflow pagination with feedback; reference repair UI and EW observation open. |
| M1-07 | active | M1-07a implemented-unqualified: schema 5 FTS5 index maintained in the song transaction, escaped prefix queries, bm25 ranking with deterministic ties, generation-tagged search, repair/rebuild, 20k database benchmark (pooled p95 75.9 ms; one-letter prefix p95 109 ms). M1-07b implemented-unqualified: Songs search box (2-character minimum, generation-guarded, typing never fires shortcuts, clear/Esc, no-results, selection and save refresh) and the accessibility Edit; 20k keystroke-to-paint p95 81.5-83.8 ms (3 runs). Slide-label search, older-hardware and Narrator runs open. |
| M1-09 | active | Schedule files implemented-unqualified (`.selaschedule` Save/Save As/Open/Open Recent/Close, atomic worker I/O, hidden revisions; see work-log record "schedule-files"). M1-09a implemented-unqualified: framework-free `schedule` (session-local entry IDs pinning song revisions, 32-item cap, move/remove/neighbor, dirty tracking), Schedule pane with drag/drop, save/open dialogs and unsaved guard, Live identity by entry, renderer-acknowledged Live marker; Windows native check passed. schedule-remainder (implemented-unqualified): duplicate, Shift/Ctrl multi-select, group move, drag reorder with autoscroll/Esc cancel, reviewable Update items. Open: EW observation of multi-select/drag, schedule item types beyond songs, persisted item IDs, native drag run, qualification. |
| M1-10 | active | M1-10a: production `sela --audience` renderer mode (moved compositor/text, centered text, settled surface-extent frame, non-activating monitor-covering window, scene retained after controller loss), Windows DX12 smoke on two monitors. M1-10b: operator output supervisor, Songs list, section slides in Preview, Go Live/double-click, Previous/Next, Live shows renderer-acknowledged slide, Windows keyboard-driven native check. M1-10c (with M0-08a): operator Logo/Black/Clear toggles with acknowledged indicators, picture logo from the profile image folder, show-control key context, Windows native checks passed. M1-09a schedule items now exist. Navigation remainder (page/arrow/label keys, indicators, tile thumbnails) implemented-unqualified, see preview-live-navigation record. Open: preparation off the UI thread, reference-observed behavior (EW8-OBS-020), hotplug, latency, physical qualification. |
| M1-11 | active | M1-11a implemented-unqualified: hardened PNG/JPEG import (8 MiB, 16384 px, 64 MiB decoded, header bomb guard, Exif orientation applied in the shared decoder, ICC ignored, APNG refused, identical re-import reused, same name different bytes numbered, cancellable temporary-and-link copy, side-specific errors), `preflight::check`/`Worker` (missing/changed images, missing fonts, logo) and hash-checked `images::relink`. M1-11b implemented-unqualified: preflight report on schedule open, Locate…/relink, Go Live missing-file warning, import Cancel and notes, photos over the decode budget downscaled at import (source untouched). Open: thumbnails pipeline, EW media observation, native Locate… chooser, Linux/macOS. |
| M1-16 | implemented-unqualified | Developer-local Linux install prerequisite; settings slice (versioned store + Options dialog with Output monitor) implemented-unqualified; language slice (i18n tables, all operator/editor strings migrated, Options language radio, runtime switch + persistence, native check) implemented-unqualified; a11y slice (AccessKit ids/roles/localized names, toggle/selected/expanded/disabled states, documented Tab order, Songs-row arrows, UIA dump + VAL-A11Y native scripts pass) implemented-unqualified; Windows installer, Narrator qualification and dependency gates remain open. |

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
