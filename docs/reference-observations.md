# EasyWorship reference evidence and Windows runbook

## Scope and status — 2026-10-04

Update 2026-10-06: the first installed run (RUN-W01-2026-10-06 below) recorded
EW8-OBS-010–012 against an unlicensed 8.0.49 install. The rest of this section
describes the 2026-10-04 research checkpoint.

M0-02 partial research checkpoint; installed observation is **blocked**. No lawful
installed EasyWorship reference or Windows runner is available for this slice.
There are **zero installed observations** and no Sela compatibility or hardware
passes. This document does not change [plan gates](plan.md) or
[backlog acceptance](backlog.md). M0-01 remains independently active.

Initial reference target: EasyWorship **8.0.49 on Windows**. A public listing is
not evidence of the installed binary, license entitlement, or workflow behavior.
All OS/DPI/display/fixture/action/result/artifact fields for installed observation
remain unverified until the run below is executed. Public-page research observer:
Amp task executor, Linux x86_64 orb, 2026-10-04; full pages fetched live with
`read_web_page(forceRefetch=true)` rather than relying on search snippets.

Evidence states are separate from ticket states:

- `observed`: executed against identified installed build with durable evidence.
- `documented-only`: authoritative page says this; not installed verification.
- `unverified`: no qualifying evidence; never fill an expected result by guessing.
- `conflicting`: preserve both results and run a discriminating test.
- `approved deviation`: record explicit approval and scope; not reference parity.

Stable IDs below must not be renumbered. Append dated runs rather than overwriting
contradictions. Documentation claims can remain documented-only even after an
installed result is added in a separately identified run.

## Authoritative source register

| ID | Source, page metadata | Exact decisive evidence and limits |
| --- | --- | --- |
| SRC-01 | [Official update listing](https://www.easyworship.com/software/update), fetched 2026-10-04 | “Build 8.0.49 · Released Jun 30, 2026”; Windows and MacOS download links. Establishes public listing only. No installer downloaded or executed; no checksum/signature verified. |
| SRC-02 | [Quick Start Guide](https://support.easyworship.com/support/solutions/articles/24000020385-quick-start-guide), modified Jan 5, 2023, 3:10 PM (page timezone unspecified) | Under Preview Area: preview displays current resource/schedule selection; double-click sends to output; forward/back in Preview “does not advance them in the live output.” Under Live Area: no editing; double-click a Live slide while Logo/Black/Clear is active turns that off and resumes output. Historical guide explicitly describes version 7 views, not 8.0.49 qualification. |
| SRC-03 | [Shortcut Keys For EasyWorship 7](https://support.easyworship.com/support/solutions/articles/24000020383-shortcut-keys-for-easyworship-7), modified Nov 24, 2020, 3:43 PM (timezone unspecified) | Show Control: Page Down = Go Live; Ctrl+C toggles live text; Ctrl+B Black; Ctrl+L Logo; Home/End first/last slide; Up/Down previous/next. Editor section also assigns Ctrl+C to copy. Does not establish 8.0.49 bindings or focus dispatch/priority. |
| SRC-04 | [Screen Setup](https://support.easyworship.com/support/solutions/articles/24000020382-screen-setup), modified Feb 12, 2026, 12:49 PM (timezone unspecified) | Extended desktop separates control and audience/foldback. Windows instructions: Edit → Options → Output Monitor; Foldback has its own monitor setting. Now includes Mac instructions, despite Support 7 navigation. No explicit 8.0.49 applicability or timing/crash independence proof. |

Source dates, titles and quotations above are durable textual evidence, not a
mirror of proprietary manuals/assets. No community requests or marketing parity
claims are accepted as exact behavior. These pages do not resolve mask precedence,
Go Live under masks, transport continuation or global focus rules.

## Observation ledger and acceptance mapping

Every installed run must add build/About evidence, Windows build, GPU/driver,
display geometry/refresh/DPI/topology, timezone/date/observer, lawful access method
(no secrets), fixture hashes, preconditions, exact input sequence, actual result
and artifact paths. Record selection, keyboard focus, Preview, Live, audience,
mask indicators and video/audio positions independently, before and after each
action. A thumbnail is not proof of physical audience output.

| ID | Status | Claim or unresolved question | Run / acceptance owners |
| --- | --- | --- | --- |
| EW8-OBS-001 | documented-only | SRC-01 lists 8.0.49; actual installed build/OS/layout/license method unknown. | W01; M0-02, all downstream parity tests |
| EW8-OBS-002 | documented-only | SRC-02 distinguishes Preview selection/navigation from Live; exact 8.0.49 selection, double-click and edit-live behavior unknown. | W02; M0-05, M1-03, M1-09, M1-10; S02 |
| EW8-OBS-003 | documented-only | SRC-02 describes double-click in Live clearing Logo/Black/Clear; 8.0.49 target slide, transition and restoration unknown. | W03; M0-08, M1-10; S02 |
| EW8-OBS-004 | documented-only | SRC-03 names v7 shortcuts; Ctrl+C has editor/show meanings. 8.0.49 focus capture, propagation and precedence unknown. | W04; M1-03, M1-10; S02 |
| EW8-OBS-005 | unverified | Black/Clear/Logo precedence, combinations, repeat toggles, initial state, restore and Go Live under any mask. | W03; M0-06, M0-08, M1-10; S02 |
| EW8-OBS-006 | unverified | Media/audio continue, pause, reset or restart under each mask and masked cue change; per-output scope unknown. | W03; M2-03, M2-04, M3-01, M3-04; S05/S06 |
| EW8-OBS-007 | documented-only | SRC-04 describes extended desktop/control versus output routing; actual 8.0.49 topology, focus and hotplug behavior unknown. | W05; M0-04, M0-09, M0-10; S06 |
| EW8-OBS-008 | unverified | Full menu/settings/dialog/pane/layout/editor/resource/context/drag-drop inventory and feature-family coverage. | W06; M0-02, M1-02, M5-06 and backlog family owners |
| EW8-OBS-009 | unverified | Sela Windows output continuity/control response under stalls, overload and failure. This is Sela acceptance, not inferred EasyWorship internals. | G01; M0-04, M0-06–M0-10; H/P |
| EW8-OBS-010 | observed (RUN-W01-2026-10-06) | Installed executable FileVersion 8.0.49.0, unlicensed edition; About dialog not captured. Empty Default profile main-window layout: menus, toolbar groups, Schedule/Preview/Live panes, Preview Output/Live Output strips, Resources tabs and Songs library. | W01; M0-02, M1-02, M1-10 |
| EW8-OBS-011 | observed (RUN-W01-2026-10-06) | Unlicensed output at startup: output windows already cover the non-primary laptop panel and show the logo with "NOT LICENSED FOR LIVE PROJECTION"; the Live Output strip shows the logo thumbnail and "Slide 1 of 1". | W01/W05; M0-08, M0-09, M1-10 |
| EW8-OBS-012 | observed (RUN-W01-2026-10-06) | Song editor structure and dirty-cancel prompt (Yes/No/Cancel). Slide splitting from typed text is unverified. | W01/W06; M1-05, M1-06 |
| EW8-OBS-013 | unverified | W02 Preview/Live selection, Go Live and next/previous with original fixtures. Blocked: synthetic clicks did not move focus inside the song editor, so fixtures were not created; the unlicensed edition may also restrict live projection. | W02; M1-10, M1-03 |

## Windows observation runbook (not executed)

### W01 — Access, environment and original fixtures

1. Obtain an authorized Windows operator and lawful 8.0.49 installation through
   the official supplier. Confirm About build rather than assuming the current
   download still supplies the pin. Record license/trial limitations without
   tokens, credentials or personal account screenshots. Stop if build differs;
   retain separate build records, do not relabel newer behavior as 8.0.49.
2. Use a disposable profile/service in a rehearsal space, never an active service.
   Record Windows build, CPU/RAM, GPU/driver, display connector/model/resolution,
   refresh, Windows scaling and coordinates; use extended, not mirrored, output.
   Start with operator + physical audience display. Add a distinct stage/alternate
   display only when available; record absent outputs as not run.
3. Create original A and B songs/presentations with slides A1/A2/A3 and B1/B2/B3,
   distinguishable colors/text, a separate original LOGO image, and an original
   timecoded motion clip with audible ticks. Record durations, hashes, loop and
   transition settings. Use no licensed songs, Bible text or supplier artwork.
4. Capture operator and actual audience together (camera where needed), include a
   visible action/time marker and audio recording for transport cases. Store
   reviewable redacted evidence under `.amp/in/artifacts/reference/<run-id>/`;
   this is a proposed path, no artifacts currently exist. Hash files, record
   capture method/frame rate and note dropped frames or camera limitations.

### W02 — Selection versus applied content

In every layout mode actually offered by 8.0.49 (record menu labels; do not assume
v7 mode names), reset to A1 live. Single-click B in Schedule, select B2 in Preview,
navigate Preview next/previous, select a resource, type a search, then edit/save
and cancel a copy of B. Record all state after each input and whether A1 remains
visible. Test edits to A while A is live separately. Press Go Live using its
button, then repeat from reset with Preview double-click and the installed
shortcut if confirmed. Record which selected item/slide applies and focus after
application. Navigate Live first/last/boundaries and repeat with duplicate A
schedule occurrences, preceding-item reorder/removal and live-item deletion.

Compatibility expected results remain pending installed observation. Sela's
independent invariant already requires private selection/editing not to mutate
the applied scene, and Live to follow renderer acknowledgment; do not describe
that design contract as measured EasyWorship acknowledgment behavior.

### W03 — Safety-mask truth table

Start each independent case at A1 live with all masks known off. Click controls
first; later repeat confirmed shortcuts and the documented Live double-click
exit separately. Never assume the masks are independent booleans or exclusive.
Record active indicators and visible layers; distinguish request from result.

| Matrix case | Exact sequence from reset | Required fields, all currently unverified |
| --- | --- | --- |
| Single/repeat | For each M in Black/Clear/Logo: M on → M again → M again | Visible text/background/logo; indicator; restore slide; transition |
| Ordered pairs | For all six ordered distinct pairs M,N: M → N → N → M | Precedence, replacement/stacking, restoration at every step |
| Ordered triples | All six permutations of Black/Clear/Logo, then reverse toggles | Reachable combined state and exit order; no assumed precedence |
| Masked apply | For every reachable masked state above: select B2 → Go Live → exit using recorded control sequence | Whether Go Live clears/preserves mask; selected/underlying/applied cue; exposed slide on exit |
| Masked navigation | Under each reachable state: next/previous Live; Preview double-click; Live double-click B/A slide as available | Which navigation unmasks, target slide and selection/focus changes |
| Lifecycle | With each single mask active: close/reopen disposable profile, toggle output off/on | Startup/output-enable state; last scene restoration; no assumed persistence |
| Motion/audio | Repeat single, pair and masked-apply cases with timecoded clip + ticks; include clip end and loop boundary | Presented position and audible continuation/pause/mute/restart; timeline after exit |
| Outputs | Repeat single-mask and masked-apply cases with each available audience/stage/alternate routing and alerts | Exact targeted outputs/layers and unaffected outputs; absent routes not run |

For each row write `(run, case, prestate, input, selected, focus, Preview, Live,
mask indicators, audience/stage/alternate pixels, video position, audio, poststate,
evidence timecode, status)`. If a combined state is unreachable, record the
sequence proving it, not a fabricated result. Missing/corrupt logo, failed B
preparation and rapid Go Live/mask alternation must get separate failure records.
Only observed static cases can unblock M0-08; pending media cases remain explicit
for M2. A documentation-only shortcut never fills this truth table.

### W04 — Keyboard/focus discrimination

With A1 live/B2 selected, explicitly click and record focus in Schedule, Preview,
Live, resource list, search text box and song editor; also test a modal dialog and
after Alt+Tab back from another app. Record Tab/Shift+Tab traversal and visible
focus separately from selection/live highlights. For each context test confirmed
Go Live, arrows/Home/End and mask bindings. Use Ctrl+C on selected synthetic editor
text and on show control as separate reset cases: record clipboard and audience,
including whether both change. Test typing ordinary characters in search/editor,
keyboard repeat and dialog dismissal. Do not install global bindings based on
the v7 guide. Record conflicts by context; rerun with a minimal fixture and exact
focus to discriminate. Native Sela actions/key-context tests belong to M1-03;
browser key emulation is not native qualification.

### W05 — Physical routing/focus behavior

Verify operator controls are only on the chosen operator display and the intended
cue only on the chosen audience display. Move/resize/minimize/occlude operator,
change operator focus, and move across mixed-DPI displays (record actual scaling).
Observe audience output while searching/editing. Unplug audience, record output
mapping/warnings and unaffected routes, then reconnect; repeat sleep/resume and
topology reorder. Capture whether focus is stolen or output falls onto operator.
These are observed reference behaviors, not proof of its rendering architecture
or a waiver of Sela's stricter independence/private-routing requirements.

### W06 — Full inventory continuation

Walk every backlog feature-family row, all menus/settings/dialogs/context menus,
empty/populated/error states, splitters/layout modes, resource tabs and editors.
Record each item's installed availability, exact labels, selection/focus and
drag/drop sequence (source, insertion target, autoscroll, cancel, modifiers).
Distinguish absent in 8.0.49 from inaccessible due to entitlement/hardware. Assign
new observation IDs and owner tickets; propose uncovered child tickets for the
backlog owner rather than silently dropping workflows. W02–W05 are a focused
subset, not an exhaustive M0-02 audit or permission to close its checkboxes.

## G01 — Sela M0-10 acceptance handoff (also not executed)

After native Windows Sela spike prerequisites exist, use the same named physical
dual-display setup and original fixtures. Link candidate commit, dependency pins,
backend/process design, build commands, diagnostics and evidence. Execute text +
image composition, cut/fade, reference-qualified masks/Go Live, mixed DPI,
hotplug/sleep/surface recovery, deliberate UI stalls, preparation overload and
renderer failure. Use controlled fault injection in the disposable Sela spike;
do not inject arbitrary failures into proprietary reference internals.

- During declared UI stalls (e.g. 100/500/2000 ms, repeated with recorded counts),
  measure physical motion continuity/frame intervals independently of control
  latency. Static output alone cannot prove frame pacing. Record distributions,
  sample counts, capture resolution and limits; submission times are not scanout.
- Separately timestamp input/request, command acceptance, renderer apply/ack and
  visible cue. Under overload verify bounded queues, safety capacity, rejection
  of failed/stale preparation and preservation of previous valid scene.
- Contrast operator-window occlusion, UI-thread stall, operator process exit,
  renderer exit and device/surface loss. Record actual survivable boundary; a
  thread is not process-crash isolation, and process separation is not GPU-loss
  isolation. Check controls recover, output routing stays intentional and stale
  cues do not reappear after restart.
- Failures need corrective tickets and reruns. Retain provisional plan budgets
  until workload/hardware/percentile boundaries are declared; do not invent new
  pass thresholds here. Linux/offscreen/native automation can supplement but
  cannot replace Windows H/P. M0-10 entry-to-M1 approval remains unchanged.

## Smallest unblock action

Arrange an authorized Windows operator with lawful **8.0.49**, two physical
displays and capture capability. Execute W01, then W02 and W03 static cases,
append identified results/artifacts and map only those proven workflows to tests.
Follow with W04/W05 and exhaustive W06; qualify Sela independently with G01 when
M0-03/M0-08/M0-09 are ready. No installed access, masks, focus, display timing,
provider rights or parity is inferred from this documentation checkpoint.

## RUN-W01-2026-10-06 — First installed observation (partial W01)

- Date/observer: 2026-10-06 ~11:00 +07:00, Droid agent on the owner's machine,
  owner present and authorizing. Lawful access: owner installed the free,
  unlicensed EasyWorship 8 download; no account, key or license dialog was
  touched. Registry display name "EasyWorship 8", version 8.0.49; executable
  `D:\Program Files\EasyWorship 8\EasyWorship.exe` FileVersion 8.0.49.0,
  ProductVersion 8.0. Help → About was not captured (menu popups do not render
  through the window-scoped capture), so the About build remains unconfirmed.
- Host: Windows 11 Home Single Language 25H2 build 26200.9457 (registry
  ProductName still reads "Windows 10"), i7-14650HX, 48 GB, Intel UHD + RTX 4060
  Laptop. Displays as in output-spike.md: DISPLAY5 2560x1440 at 125% primary,
  laptop DISPLAY1 2560x1600 at 175% left of it, DISPLAY6 1920x1080 at 100%.
  Extended desktop. EasyWorship windows are per-monitor DPI aware.
- Method: `scripts/reference_win.py` lists the EasyWorship PID's windows and
  captures one HWND at a time with PrintWindow, so no other application's
  content enters the evidence. Artifacts (ignored, local):
  `.amp/in/artifacts/reference/ew8-w01/`: `main-restored.png` (SHA-256 prefix
  2202935A6758114F), `live-output-initial.png` (F597AEDD58984D14),
  `song-editor-new.png` (529294CE345D831D), `editor-cancel-confirm.png`
  (980C71F8D75CAD61). Profile "Default", 0 songs, nothing saved.

Observed, EW8-OBS-010 (main window, empty profile, maximized on the primary):

- Menu row: File, Edit, Live, Profiles, View, Help. Dark theme by default.
- Toolbar left: New ▾, Open ▾, Save (disabled while empty), Web, Remote ▾
  (disconnected icon). Toolbar right: Go Live, Alerts ▾, Logo, Black, Clear,
  Live. Labels sit under icons. This matches D-UI-01 plus a separate menu row.
- Upper area, left to right: Schedule (header has a view-mode toggle ▾ and a
  gear ▾), Preview (view-mode ▾), Live (view-mode ▾). At this window size the
  widths are about 25% / 53% / 22% of the window.
- Preview and Live each end in an output strip: "Preview Output" with a black
  thumbnail and ‹ › arrows; "Live Output" with the current output thumbnail,
  "Slide 1 of 1" and ‹ › arrows.
- Resources: tabs Songs, Scriptures, Media, Presentations, Themes; right end
  has + and ▶. Songs tab: left tree SONGS → All Songs (selected), ONLINE →
  SongSelect, COLLECTIONS, MY COLLECTIONS, with +▾ and gear▾ below it; the list
  has columns Title, Author, Copyright; the footer has +, gear▾, a centered
  "0 songs" count and a view-mode ▾ at the right.

Observed, EW8-OBS-011 (unlicensed output): before any Go Live, three
"Live Output" and three "LayeredWindow" windows cover the laptop panel at
2560x1599 physical (one row short of the 1600 panel height). They show the
EasyWorship logo with "NOT LICENSED FOR LIVE PROJECTION" on black. EasyWorship
chose the non-primary laptop panel without being asked.

Observed, EW8-OBS-012 (song editor via the Songs footer +): window "Song Editor
- Untitled" with a Title field; Words/Slides tabs; slide row 1 with "label" and
"song" placeholders; toolbar Text, Scripture ▾, Shape ▾, Media ▾, then Format,
Animate, Presentation; WYSIWYG canvas reading "Double click to edit song" with a
"copyright" strip, 32% zoom slider; footer Apply (disabled until a change), OK,
Cancel. Cancel after edits asks "You have unsaved changes. Would you like to
save changes before closing?" with Yes/No/Cancel; No discarded the draft.

Not observed: synthetic mouse clicks on the editor's Title field did not move
keyboard focus; typed text landed in the slide label/body unpredictably, so no
fixture songs were created and slide splitting stays unverified (D-UI-02
documents Ctrl+Enter). W02–W06 did not run. Next: create fixtures A/B by hand
with the owner (or via an import file), then run W02 with Go Live, Preview
double-click and next/previous, noting what the unlicensed edition permits.

## D-UI-01 — Official toolbar image (documented-only, 2026-10-04)

- Source: <https://www.easyworship.com/software/features>; directly fetched and
  visually inspected <https://cdn.easyworship.com/files/software/features/ew-interface.webp>.
- Visible single toolbar left New, Open, Save, Web, Remote; right Go Live,
  Alerts, Logo, Black, Clear, Live, in that order, above the Live pane.
  No separate menu row is visible. Schedule/Preview/Live span the upper area,
  tabbed Resources the lower area. Image dimensions 1736×954; not a native
  measurement or DPI/layout specification. Existing bounded pane ratios remain
  provisional rather than pretending these pixels prove exact 8.0.49 geometry.
- **Build unidentified, not proven EasyWorship 8.0.49.** Marketing imagery is
  documented evidence only, not an installed observation. Shortcuts, focus,
  menus, masks, selection, output behavior and responsive layout remain unknown.
- Used to correct M1-02c action placement with original GPUI styling; reference
  screenshot/song/background assets were not copied into application or fixtures.

## D-UI-02 — Official song authoring guidance (documented-only, 2026-10-04)

- Personally read <https://support.easyworship.com/support/solutions/articles/24000020402-adding-and-editing-songs>,
  modified **2024-09-03**, under **Support 7**: Songs bottom + opens Song Editor;
  Title, Words label/lyrics, bottom-left Add, top-right Inspector for song
  copyright information, OK saves changes. Documents Ctrl+Enter split/new slide,
  theme/text/scripture/media/arrange, import, search and live actions too; those
  are not implemented or silently treated as completed by this UI correction.
- Personally read <https://support.easyworship.com/support/solutions/articles/24000020385-quick-start-guide>,
  modified **2023-01-05**, **Support 7**: New toolbar → New Song; Words/Slides
  left and editable WYSIWYG preview right; Masters, Theme/Text/Scripture/Media,
  Arrange and Inspector. Sela's preview is local draft text, explicitly **not**
  that WYSIWYG functionality. Provisional shortcuts remain unchanged rather than
  pretending documented Ctrl+Enter is observed 8.0.49 or adding splitting here.
- Working official attachment personally fetched and visually inspected:
  <https://s3.amazonaws.com/cdn.freshdesk.com/data/helpdesk/attachments/production/24018643889/original/U0uDOPvM27V3pQwL_KlrGEv99LcimpEbvw?1516306231>.
  Visible title upper-left, editor/format toolbar, Words/Slides/Masters and
  section list left, large preview right, + bottom-left, OK/Cancel footer right.
  Attachment is historical (query timestamp), **not verified 8.0.49**. Broken
  older help image links are not substitute evidence. No reference lyrics,
  background, screenshot or proprietary icons used as Sela assets/fixtures.
- Supports M1-05d editor region correction only. Exact selected-section focus,
  cancel/dirty/conflict semantics, undo, small-window behavior, audience layout,
  metadata field details and full build-specific interactions still need lawful
  installed observations. The missing capabilities stay in the compatibility
  inventory/future tickets; no full editor/workflow-parity finding made.
