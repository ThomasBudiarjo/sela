# EasyWorship reference evidence and Windows runbook

## Scope and status — 2026-10-04

Update 2026-10-06: the first installed run (RUN-W01-2026-10-06 below) recorded
EW8-OBS-010–012 against an unlicensed 8.0.49 install. RUN-W03-2026-10-06
then executed the static single-item W02/W03 cases and recorded
EW8-OBS-014–020. The rest of this section describes the 2026-10-04 research
checkpoint.

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
| SRC-05 | [Toolbar](https://help.easyworship.com/Toolbar.html), help.easyworship.com, © 2026, fetched 2026-10-06; build not stated | Logo, Black and Clear are each "a toggle button"; clicking again toggles it off. Clear "clears all text and foreground elements from the live output". Live enables/disables output; when off "the audience will see the Windows wallpaper or another program". Silent on combinations. |
| SRC-06 | [Live Area](https://help.easyworship.com/LiveArea.html), © 2026, fetched 2026-10-06 | Forward/back arrows or shortcut keys advance slides; no editing in Live; "if you are currently on a Logo, Black, or Clear screen, you can double-click a slide in the Live Area to turn that off and resume Live Output." |
| SRC-07 | [Preview Area](https://help.easyworship.com/PreviewArea.html), © 2026, fetched 2026-10-06 | Preview shows the current resource or schedule selection; double-click sends to live immediately; Preview forward/back "does not advance them in the live output". Preview Area exists only in the "Preview and Live" view. |
| SRC-08 | [Helpful Shortcut Keys](https://help.easyworship.com/HelpfulShortcutKeys.html), © 2026, fetched 2026-10-06 | Show Controls: Page Down Go Live; Ctrl+C toggles text and foreground; Ctrl+B Black; Ctrl+L Logo; Right/Left next/previous slide; Home/End first/last slide; Down/Up next/previous schedule item; Ctrl+O/Ctrl+S open/save schedule; V/C/#+Enter jumps. Editor Keys: Ctrl+Enter new or split slide; Ctrl+Del deletes the selected schedule item; Ctrl+C copy. Focus dispatch between the two groups is not described. |
| SRC-09 | [Editing Images](https://help.easyworship.com/EditingImages.html), © 2026, fetched 2026-10-06 | Image right-click menu: Media Usage (background/foreground/Automatic), Aspect Ratio, Copy to Theme ▸ Song/Scripture/Presentation Theme, "Use as Logo Background" (Logo button outputs that image; clicking again turns it off), Delete, Update items in Schedule. |
| SRC-10 | [Editing Videos](https://help.easyworship.com/EditingVideos.html), © 2026, fetched 2026-10-06 | Video right-click "Set as Logo Background"; Repeating On loops "until you stop it or until you go live to something else". Mask behavior during playback not described. |
| SRC-12 | [Global song settings in EasyWorship](https://blog.easyworship.com/2019/12/04/global-song-settings-in-easyworship/) (blog, 2019-12-04; search snippet only, page returned 404 on 2026-10-06) and [Options (EW 6)](https://testportal.easyworship.com/support/solutions/articles/8225-options-ew-6-) (2015) | Auto Sizing: "Do not auto size text" or "Resize text to fit element"; with the latter, a "Normalize text size across slides" check box. Applies to older builds; 8.0.49 placement and default are not established. |
| SRC-11 | [Adding And Editing Songs](https://support.easyworship.com/support/solutions/articles/24000020402-adding-and-editing-songs), modified Sep 3, 2024, re-read 2026-10-06 (also D-UI-02) | Ctrl+Enter at a line start splits a slide; drag songs from Resources to Schedule; select a song and Go Live, or double-click it to send it directly to Live; Remove From Schedule asks for confirmation. |

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
| EW8-OBS-005 | unverified | Black/Clear/Logo precedence, combinations, repeat toggles, initial state, restore and Go Live under any mask. Static single-item cases since observed as EW8-OBS-015–017; the rest is EW8-OBS-020. | W03; M0-06, M0-08, M1-10; S02 |
| EW8-OBS-006 | unverified | Media/audio continue, pause, reset or restart under each mask and masked cue change; per-output scope unknown. | W03; M2-03, M2-04, M3-01, M3-04; S05/S06 |
| EW8-OBS-007 | documented-only | SRC-04 describes extended desktop/control versus output routing; actual 8.0.49 topology, focus and hotplug behavior unknown. | W05; M0-04, M0-09, M0-10; S06 |
| EW8-OBS-008 | unverified | Full menu/settings/dialog/pane/layout/editor/resource/context/drag-drop inventory and feature-family coverage. | W06; M0-02, M1-02, M5-06 and backlog family owners |
| EW8-OBS-009 | unverified | Sela Windows output continuity/control response under stalls, overload and failure. This is Sela acceptance, not inferred EasyWorship internals. | G01; M0-04, M0-06–M0-10; H/P |
| EW8-OBS-010 | observed (RUN-W01-2026-10-06) | Installed executable FileVersion 8.0.49.0, unlicensed edition; About dialog not captured. Empty Default profile main-window layout: menus, toolbar groups, Schedule/Preview/Live panes, Preview Output/Live Output strips, Resources tabs and Songs library. | W01; M0-02, M1-02, M1-10 |
| EW8-OBS-011 | observed (RUN-W01-2026-10-06) | Unlicensed output at startup: output windows already cover the non-primary laptop panel and show the logo with "NOT LICENSED FOR LIVE PROJECTION"; the Live Output strip shows the logo thumbnail and "Slide 1 of 1". | W01/W05; M0-08, M0-09, M1-10 |
| EW8-OBS-012 | observed (RUN-W01-2026-10-06) | Song editor structure and dirty-cancel prompt (Yes/No/Cancel). Slide splitting from typed text is unverified. | W01/W06; M1-05, M1-06 |
| EW8-OBS-013 | unverified | W02 Preview/Live selection, Go Live and next/previous with original fixtures. Blocked: synthetic clicks did not move focus inside the song editor, so fixtures were not created; the unlicensed edition may also restrict live projection. Single-item part since observed as EW8-OBS-014; multi-item, editing and schedule cases remain open. | W02; M1-10, M1-03 |
| EW8-OBS-014 | observed (RUN-W03-2026-10-06) | One 4-slide song: single-click in Preview changes only the Preview selection; Go Live applies the selected Preview slide; double-click in Preview applies the clicked slide; single-click on a Live slide applies it immediately; Live Output ‹ › move one slide and › stops at the last slide (no wrap); with the main window focused after a Preview click, Page Down applies the selected Preview slide and Right/Left/End/Home move Live next/previous/last/first. | W02; M1-10, M1-03 |
| EW8-OBS-015 | observed (RUN-W03-2026-10-06) | Mask truth table, button clicks from all-off: each mask toggles on/off on repeat. Black and Logo replace each other (the later press wins, the other indicator goes off); Clear stacks with either. Reachable: none, Black, Clear, Logo, Black+Clear, Logo+Clear; Black+Logo unreachable. Black shows full black, Logo shows the logo image over the whole output, Clear shows the slide background without text; Black/Logo cover a stacked Clear, and turning them off reveals the Clear state. | W03; M0-08, M1-10 |
| EW8-OBS-016 | observed (RUN-W03-2026-10-06) | Under each single mask, Go Live, Preview double-click and Live ‹ › keep the mask and its indicator while the Live pane moves; turning the mask off shows the newly applied slide. Double-click on a Live slide turns the single mask off and applies that slide (confirms SRC-06 for single masks). | W03; M0-08, M1-10 |
| EW8-OBS-017 | observed (RUN-W03-2026-10-06) | Live off with a mask on: Live indicator goes off, the mask indicator stays lit and the audience panel shows the Windows desktop; Live on returns the masked output. | W03; M0-08, M0-09, M1-10 |
| EW8-OBS-018 | observed (RUN-W03-2026-10-06) | With the main window foreground and focus in the Preview list, Ctrl+B, Ctrl+L and Ctrl+C toggle Black, Logo and Clear exactly like the buttons, and Page Down is Go Live. Behavior with focus in search, editor or dialogs is not observed (W04 stays open). | W03/W04; M1-03, M1-10 |
| EW8-OBS-019 | observed (RUN-W03-2026-10-06) | Logo source: Media → Images → + opens "Import Image Resource Item"; right-click → "Use As Logo Background" makes Logo show that image filling the output. The image menu matches SRC-09 (Media Usage Automatic ✓/Background/Foreground). Copy to Theme ▸ Song Theme creates a song theme; its "Set As Default Song Theme" re-renders the song preview with that background. With an image item (Automatic usage) live, Clear and Ctrl+C change nothing and the Clear indicator stays off. | W03/W06; M1-10, M2-03, M4-01 |
| EW8-OBS-020 | unverified | Not run: Live double-click with Black+Clear or Logo+Clear; single-click on a Live slide under a mask; profile close/reopen with a mask; masks with motion/audio, multi-output, alerts, multi-item schedules; transition durations (only coarse settle times exist). | W03; M0-08, M2-03, M3-01 |
| EW8-OBS-044 | observed (RUN-W07A) | Songs search box shows previewed title then an empty "Search" state | W04/W07; M1-03, M1-04, M1-07b |
| EW8-OBS-045 | observed (RUN-W07A) | Live filtering, clear button, Esc, no-results state; plus B/C/L-in-search rule (UNOBSERVED, best guess) | W04/W07; M1-03, M1-04, M1-07b, M1-10 |
| EW8-OBS-046 | observed (RUN-W07A) | Word-prefix match over title, labels and lyric words | W04/W07; M1-04, M1-07b |
| EW8-OBS-047 | observed (RUN-W07A) | Result selection, double-click; Ctrl+B/L/C in search UNOBSERVED with `SelaShow && !SelaTextInput` best guess | W04/W07; M1-03, M1-04, M1-07b |
| EW8-OBS-048 | observed (RUN-W07A) | Schedule-focused arrows, Home/End, PageDown focus change | W04/W07; M1-03, M1-09 |
| EW8-OBS-049 | observed (RUN-W07A) | Preview-focused arrows, Home/End (partial) | W04/W07; M1-03, M1-10 |
| EW8-OBS-050 | observed (RUN-W07A) | Live-focused Left/Right/Home/End slides, PageDown next schedule item; PageUp target best guess | W04/W07; M1-03, M1-10 |
| EW8-OBS-051 | observed (RUN-W07A) | Boundary behavior without wrap | W04/W07; M1-03, M1-10 |
| EW8-OBS-052 | unverified (RUN-W07A) | Label jump keys; best guess states Live focus and Live target | W04/W07; M1-03, M1-10, slide-labels |
| EW8-OBS-053 | observed (RUN-W07A) | Menu inventory and Options entry point | W04/W07; M1-02, M5-06 |
| EW8-OBS-054 | observed (RUN-W07A) | Options sections and Main Output tabs | W04/W07; M5-06 |
| EW8-OBS-055 | observed (RUN-W07A) | Options text lists for Main Output tabs (VAL-REF-004) | W04/W07; M5-06 |
| EW8-OBS-056 | observed (RUN-W07A) | Slide Labels, Advanced and other Options sections as text (VAL-REF-004) | W04/W07; M5-06, slide-labels |
| EW8-OBS-057 | unverified (RUN-W07A) | Master edit reach: slide inheritance and cross-song/theme scope with implementation rule (VAL-REF-005) | W04/W07; M1-05h, themes-tab-editor, theme-model-storage |
| EW8-OBS-058 | observed (RUN-W07A) | Ctrl+A in Words keeps the Text tab reachable | W04/W07; M1-05 |
| EW8-OBS-059 | observed (RUN-W07A) | Schedule add by drag, context menu | W04/W07; M1-09, schedule-remainder |
| EW8-OBS-060 | observed (RUN-W07A) | Schedule copy/duplicate absent; multi-select and drag reorder best guesses | W04/W07; M1-09, schedule-remainder |
| EW8-OBS-061 | observed (RUN-W07A) | Library context menu, Update items in Schedule | W04/W07; M1-04, M1-09 |
| EW8-OBS-062 | observed (RUN-W07A) | Song Info tab fields | W04/W07; M1-05, song-metadata-fields |
| EW8-OBS-063 | observed (RUN-W07A) | Copyright strip on output; Administrator/CCLI best guesses | W04/W07; M1-05, song-metadata-fields |
| EW8-OBS-064 | observed (RUN-W07A) | Sort by submenu | W04/W07; M1-04 |
| EW8-OBS-065 | observed (RUN-W07A) | Delete confirmation default No | W04/W07; M1-04 |
| EW8-OBS-066 | observed (RUN-W07A) | Go Live under Black/Logo keeps the mask | W04/W07; M0-08, M1-10 |

## Windows observation runbook (W01 partial; W02/W03 single-item static cases executed)

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

## RUN-W03-2026-10-06 — Static W02/W03 cases on one song

- Date/observer: 2026-10-06 13:26–13:40 +07:00, Droid agent on the owner's
  machine with the owner's authorization; the owner created the song and left
  the machine to the automation. Same install and host as RUN-W01. Output
  window: the laptop panel, rect (-2560,-146)–(0,1453), 168 DPI; main window
  maximized on the 120 DPI primary.
- Fixtures: song "waeqwe", 4 slides of placeholder text typed by the owner
  (no licensed lyrics). Original images generated for this run:
  `explore/sela-w03-logo.png` (orange 1920x1080 with a blue square, SHA-256
  prefix CD895BD8FDB3EAE4) as logo, and `explore/sela-w03-background.png`
  (green stripes, AFC69C3FFFCFAE36) as the default song theme background, so
  Clear (background only) differs from Black.
- Method: `scripts/reference-w03-windows.py run`. Input goes only to the
  EasyWorship PID's main window after a foreground check. Each step captures
  the Live Output window's screen rectangle until three identical frames,
  reads the toggle indicators (blue fill) from a PrintWindow toolbar crop and
  writes one JSONL row. Rows hash the output above the taskbar strip, which
  Windows keeps over the output's bottom edge. Per-case contact sheets show
  toolbar, Live pane and audience per step. Artifacts (ignored, local) in
  `.amp/in/artifacts/reference/ew8-w03/`: `RUN-W03-20261006-133553.jsonl`
  (165 rows, C703BBA1F4F56736), 32 `RUN-W03-20261006-133553-*-sheet.png`
  files and per-step `*-output.png` images.
- First attempt `RUN-W03-20261006-132625.jsonl` (0A593B12BA15D948) is kept
  but not used for mask rows: importing the logo left the Media tab active, so
  Preview and Live held the image item rather than the song. It is the source
  of the image-item Clear finding in EW8-OBS-019. Returning to the Songs tab
  restores the song preview.
- The unlicensed "EASYWORSHIP / NOT LICENSED FOR LIVE PROJECTION" watermark is
  drawn over every state, including Black and Logo. It did not hide any
  result: text, background, black and logo remain distinguishable.

Output classes by hash: slides s1–s4 (green with text), BG (green, no
text), BLACK, LOGO and DESKTOP. Indicators: B Black, C Clear, G Logo, V Live.
Every case starts at s1 live with all masks off, read back from indicators.

| Case | Inputs from reset → observed output [indicators] |
| --- | --- |
| Selection | Preview 2 → s1 [V]; Preview 3 → s1; Live slide 3 → s3; Live › → s4; › → s4; ‹ → s3; Preview 1 double-click → s1 |
| Single | Black → BLACK [BV] → s1 [V] → BLACK; Clear → BG [CV] → s1 → BG; Logo → LOGO [GV] → s1 → LOGO |
| Pairs | Black, Clear, Clear, Black → BLACK [BV], BLACK [BCV], BLACK [BV], s1. Black, Logo, Logo, Black → BLACK, LOGO [GV], s1, BLACK. Clear, Black, Black, Clear → BG, BLACK [BCV], BG [CV], s1. Clear, Logo, Logo, Clear → BG, LOGO [GCV], BG, s1. Logo, Black, Black, Logo → LOGO, BLACK [BV], s1, LOGO. Logo, Clear, Clear, Logo → LOGO, LOGO [GCV], LOGO [GV], s1 |
| Triples | All six orders then reverse are consistent with the pair rule. Example Clear, Black, Logo, Logo, Black, Clear → BG, BLACK [BCV], LOGO [GCV], BG [CV], BLACK [BCV], BLACK [BV] |
| Masked apply | Each mask on → Preview 2 + Go Live: mask unchanged → mask off: s2. Each mask on → Preview 3 double-click: mask unchanged → mask off: s3 |
| Masked nav | Each mask on → Live › and ‹: mask unchanged → Live slide 2 double-click: s2 [V], mask indicator off |
| Lifecycle | Each mask on → Live off: DESKTOP, mask indicator still lit, V off → Live on: mask output again → mask off: s1 |
| Shortcuts | Ctrl+B, Ctrl+L, Ctrl+C twice each: same as the buttons. Preview 3, Page Down → s3; Right → s4; Left → s3; End → s4; Home → s1 |

Coarse timing: inputs that changed the output settled 0.83–1.03 s after the
input (this includes about 0.2 s of stability detection), and unchanged
outputs 0.34–0.41 s. This is consistent with a default fade but does not
measure a transition duration.

Interpretation for Sela, provisional where marked: model masks as a cover
(None, Black or Logo; a new cover replaces the old one) plus an independent
Clear flag. Draw the cover if set, else the background-only slide if Clear is
set, else the full slide. Go Live and navigation change the underlying slide
and keep masks. Live double-click clears masks; clearing both cover and Clear
in a combined state is provisional (EW8-OBS-020). Output off keeps mask state.

## RUN-W06E-2026-10-06 — Song Editor Words/Slides (partial W06)

- Date/observer: 2026-10-06 ~16:30 +07:00, Droid agent on the owner's machine
  with the owner's authorization (owner chose agent-driven observation). Same
  install and host as RUN-W01. Editor window "Song Editor - …", class
  `FMTForm_PresentationEditor`, default 1292x750 physical at 120 DPI.
- Fixtures: the owner's placeholder song "waeqwe" (edited, then Cancel → No;
  not saved) and a new untitled song with original placeholder text ("Line
  one", "Chorus words", …), also discarded with Cancel → No. Nothing was saved.
- Method: local ad-hoc driver (`.amp/in/explore/ew.py`, not committed) using
  `scripts/reference_win.py` PrintWindow captures, `mouse_event` clicks and
  Unicode `SendInput` typing to the editor after a foreground check. Popup menus
  render only after the cursor hovers them; they were captured from the screen.
  Artifacts (ignored, local): `.amp/in/artifacts/reference/ew8-w06-editor/`
  `07-editor-waeqwe.png` … `35-confirm2.png`.

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-021 | observed (RUN-W06E) | Editor layout: Title field (placeholder "Title") top-left inside the toolbar with Import ▾, Print, Spell check ▾, Undo ▾ icons under it; then Text, Scripture ▾, Shape ▾, Media ▾; right end Format, Animate, Presentation. Left pane tabs Words / Slides with + at its bottom-left; right pane the rendered slide (theme background, theme text, "copyright" strip) on a dark surround, with a text tool icon, contrast icon and zoom slider (32%) below it. Footer: "Apply changes to items in schedule" checkbox, Apply (disabled until a change), OK, Cancel. Library row right-click: New Song..., Edit Song..., Delete, Update items in Schedule, Sort by ▸, Refresh. |
| EW8-OBS-022 | observed (RUN-W06E) | Words is one continuous document of slides. Each slide row: number, slide icon, a label line (bold; grey italic "label" placeholder when empty) and lyric lines ("song" placeholder). A labeled slide starts a bordered group; following unlabeled slides join it. Group color follows the label kind: Verse (and unrecognised text) blue, Chorus and Pre-Chorus rose, Bridge and Tag purple, Ending dark red, Intro green. The selected slide's number cell is highlighted. Ctrl+A selects the whole Words document. |
| EW8-OBS-023 | observed (RUN-W06E) | Editing: a new song focuses slide 1's label. Clicking text places the caret and selects that slide; the preview shows the caret's slide and updates per keystroke. Enter inserts a line in the same slide; a blank line does not split. Ctrl+Enter splits at the caret: the text after it becomes a new unlabeled slide right after (same group), the line break before the caret is dropped, the caret moves to the new slide, slides renumber. Down from a label moves into its lyrics, Up from the first lyric line into the label. Typing a label on an unlabeled slide starts a new group there. The Words + appends an empty slide as its own group and selects it; an empty slide previews as "Double click to edit song". Ctrl+Z undoes in typing groups. Cancel with changes asks Yes/No/Cancel (EW8-OBS-012). |
| EW8-OBS-024 | observed (RUN-W06E) | Slides tab: narrower left pane, vertical list of rendered slide thumbnails with the number at the left and a caption bar below each one, filled with the label kind color and showing the label, or "Slide N" (grey italic) when unlabeled; the selected slide has a blue rounded frame. |
| EW8-OBS-025 | observed (RUN-W06E) | Canvas: single-click on the song text selects its text box (covers the whole slide; eight handles and a rotate handle); double-click enters in-place text editing with a caret. Right-click on the box: Edit Points, Bring to Front, Send to Back, Align Elements ▸, Distribute Elements ▸, Cut, Copy, Paste, Paste Without Formatting, Duplicate, Lock, Reapply Layout to Element. |
| EW8-OBS-026 | superseded (RUN-W06G) | Toolbar buttons (Title field, Media ▾, Format) ignored synthetic `mouse_event` and absolute `SendInput` clicks, so Format/Animate/Presentation panels, font, size, color, style, alignment and background controls were not observed. Help "Edit a Song" (help.easyworship.com, © 2026, build not stated) documents font selector, auto sizing, font size, color, Bold/Italic/Underline/Shadow/Outline, horizontal and vertical alignment, indent, Reset Styles, Theme, Masters tab and Inspector background fill per song or slide. Needs the owner to click, or another input path. |

## RUN-W06G-2026-10-06 — Song Editor Format pane (partial W06)

- Date/observer: 2026-10-06 ~18:50–19:40 +07:00, Droid agent on the owner's
  machine with the owner's authorization; same install and host as RUN-W06E.
  The owner restarted EasyWorship and reopened the editor during the run; the
  editor was used at 1292x750 and maximized (2578x1398 physical, 120 DPI).
- Fixture: the owner's placeholder song "waeqwe". Every editor session was
  closed with Cancel → No (Yes/No/Cancel "You have unsaved changes…"); nothing
  was saved.
- Method: the RUN-W06E local driver plus local probes (`.amp/in/explore/`,
  not committed): `mouse_event` clicks, wheel, screen grabs, pixel samples.
  Notes `.amp/in/explore/ew-format-notes.md`; captures (ignored, local)
  `.amp/in/artifacts/reference/ew8-w06-editor/g11…g45` and
  `.amp/in/artifacts/reference/ew8-w06g-format/`.
- Input caveats (tooling, not EasyWorship semantics): the editor opened by the
  driver before the restart ignored all toolbar input (`mouse_event`, held
  `SendInput`, `PostMessage`, touch injection; `WM_NCHITTEST` was `HTCLIENT`,
  no overlay window, UI Automation exposes only the `Editor_Output` child).
  After the restart toolbar clicks worked. With the Format pane open, the
  toolbar buttons' hover/click area reached about 100 px below the drawn
  toolbar (over the pane's tab row and Style|Layout toggle), and clicks on
  B/I/U reopened the font list about 100 px above them, also after
  minimize/maximize and in a fresh editor. These controls were read from
  captures; their click behavior is not recorded.

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-027 | observed (RUN-W06G) | Format is a toolbar toggle that docks an inspector pane at the right (canvas zoom 32% → 25% at 1292x750); a second click closes it. Its content follows the canvas selection: a selected element shows tabs Style \| Text \| Arrange; no canvas element selected shows the Slide pane (with a Text tab beside it after selecting a slide in Words). The pane reopens on the last tab and scroll position. Selecting the box while the pane is closed does not open it. Animate opens a Transition pane instead (Blend, Change, Preview ▸, Duration 00:500, Show More; Slide Timing: Advance ▾ On Click, Delay, Loop Slide, Always start slide from beginning [x]). Media opens a docked Videos panel. |
| EW8-OBS-028 | observed (RUN-W06G) | Text › Style, FONT section: family dropdown (alphabetical installed fonts; each row shows the name in the UI font and a sample in that font); Size dropdown showing "Auto" with A▾/A▴ step buttons; Color swatch ▾; B, I, U, superscript, subscript; decrease/increase indent; horizontal align left/center/right; vertical align top/middle/bottom. The Size menu has header "Auto Sizing", radios "Do not auto size text" / "Resize text to fit element", then "Reset Size". Choosing "Do not auto size text" turns Size into an editable number showing the size the fit had produced (78 on this slide). Color popup: tabs Swatches \| Spectrum \| Values, a grey ramp row above a hue × lightness grid, and a Hex Value field (#FFFFFF). |
| EW8-OBS-029 | observed (RUN-W06G) | Text › Style continues: OUTLINE type ▾ (None, Outer, Center, Inner), color, join ▾ (Round, Square, Bevel), Size slider + spin, Opacity slider + spin; SHADOW mode ▾ (None, Enabled), color, Angle dial + spin, Offset, Blur and Opacity slider + spin; BULLETS Indent Level, Bullet Style ▾ (None). The theme defaults on this install's song box: Tahoma, Auto (resize to fit), white, Bold on, centered both ways, Outline Outer black Round 7 / 100, Shadow Enabled black angle 315 offset 18 blur 9 opacity 90. |
| EW8-OBS-030 | observed (RUN-W06G) | Text › Layout: MARGINS Left 23, Right 23, Top 14, Bottom 14; TEXT LAYOUT Vertical Align ▾ (Center), Rotate dial + spin + flip H/V; AUTO SIZING radios "Do not auto size text" / "Resize text to fit element" with the sub-option "Normalize text size across slides" [x]; TEXT FORMATTING Word Wrapping [x], Capitalize all words [ ], Capitalize first word of each line [ ]. |
| EW8-OBS-031 | observed (RUN-W06G) | Style tab (element): FILL, BORDER, SHADOW, REFLECTION dropdowns (None) and ELEMENT OPACITY slider + spin (100). Arrange tab: Back \| Front, Backward \| Forward; align left/center/right and top/middle/bottom; distribute (disabled for one element); Dimensions Left 0, Top 0, Width 2304, Height 1440 (the song box covers the whole slide), Constrain Proportions; Rotate + flip; Lock \| Unlock. |
| EW8-OBS-032 | observed (RUN-W06G) | Slide pane: Slide Layout thumbnail with "Master" ▾; Theme Elements (Show/Hide) checklist song [x], scripture [ ], copyright [x]; Background: Media Fill ▾ with thumbnail and name, Select Media…, Media Usage ▾ (Background), Aspect Ratio Auto + ▾ (Stretch), Repeating Auto (disabled), Rotate + flip H/V, Volume (disabled for an image), Opacity 100; Edit Slide Layouts at the bottom. |
| EW8-OBS-033 | observed (RUN-W06G) | Formatting scope is per slide: Shadow → None on slide 1 removed the shadow on slide 1 only; slide 3 kept Shadow Enabled, and slide 1 kept None after switching away and back. Apply became enabled once the box was selected and the Format › Text pane shown, before any value changed. A reopened editor shows the saved/theme values again (unsaved changes are discarded with Cancel → No). Not observed: a way to apply formatting to all slides at once (multi-select, Masters, Reset Styles, Reapply Layout to Element), what is saved, schedule/live rendering of a formatted song. |

## RUN-W06H-2026-10-07 — Ctrl+A whole-song formatting (partial W06)

- Date/observer: 2026-10-07 +07:00, Droid agent on the owner's machine with
  the owner present and authorizing; same install and host as RUN-W06G. The
  owner opened and maximized the editor himself (2578x1398 physical, 120 DPI);
  it was later restored to 1816x1005 (cause not recorded).
- Fixture: the owner's placeholder song "waeqwe" (four slides: "etsaer",
  two unlabeled, "sadsdasd"). The owner closed EasyWorship himself at the end;
  the agent saved nothing (its final Cancel click found no editor window).
- Method: the RUN-W06E driver through a local wrapper
  (`.amp/in/explore/ewh.py`, not committed). Captures (ignored, local):
  `.amp/in/artifacts/reference/ew8-w06h-format/h00…h08`.
- Input caveat (tooling): a synthetic `mouse_event` click and a hover-then-click
  on Italic both focused the font-family field instead (the RUN-W06G
  B/I/U quirk). The owner clicked Italic with the real mouse.
- The owner ended the run after EW8-OBS-035, reporting that with Ctrl+A every
  formatting choice (font and the rest) applies to all slides. Undo of a
  whole-song format change, typing over the selection, slider/dial drags and
  the Size step amount were not observed.

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-034 | observed (RUN-W06H) | Ctrl+A with the caret in slide 2's lyrics selects the text of every Words row (labels and lyrics of all slides, empty labels shown as selected gaps) and moves the caret to the end of the last slide; the preview follows that slide. The selection stays (grey) while a Format pane control has focus (h02, h03). |
| EW8-OBS-035 | observed (RUN-W06H) | With everything selected, Italic (owner's real click) made every slide italic: all Words rows render italic and every Slides thumbnail shows italic text (h05, h06). In the same run the family became Agency FB on every slide after the stray font-field focus; the agent did not choose it, so the trigger is unrecorded, but the change applied to all slides. The Slides tab shows every thumbnail selected and the Format pane shows only the Slide tab (no Text tab) while all slides are selected (h06). Owner-reported (not separately observed): every Format pane choice applies to all slides after Ctrl+A. |
| EW8-OBS-036 | observed (RUN-W06H) | Switching Words → Slides → Words ends the whole-song selection: Words shows only a caret at the end of the last slide, and the Text pane shows that slide's values (Agency FB, B and I on) (h07). One Ctrl+Z then changed no visible format (h08); what it undid is unrecorded. |

## RUN-W06I-2026-10-07 — Slide pane background and the Master layout (partial W06)

- Date/observer: 2026-10-07 +07:00, Droid agent on the owner's machine with
  the owner authorizing; same install and host as RUN-W06G. The owner had
  opened a new song editor ("Song Editor - Untitled", maximized 2578x1398
  physical, 120 DPI) with the Format pane on the Slide tab. The owner made
  steps i01–i04 with the real mouse, then went away; the agent drove the
  rest with synthetic clicks.
- Fixture: the new, untitled song (one empty slide, then two). The installed
  theme is "sela-w03-background" (the RUN-W03 image as its background).
  Images in Media › Images: "sela-w03-background" and "sela-w03-logo". The
  agent closed the editor with Cancel → No (i21); nothing was saved.
- Method: the RUN-W06E driver through a local wrapper
  (`.amp/in/explore/ewi.py`, not committed). Popups are separate
  `FMTsdPopupForm` windows that `PrintWindow` captures as blank, so they
  were captured with a screen grab of the popup rectangle only. Captures
  (ignored, local): `.amp/in/artifacts/reference/ew8-w06i-background/i00…i21`.
- Input caveat (tooling): the first synthetic click on Slide Layout ▾ opened
  an empty popup. After Edit Slide Layouts, synthetic clicks on the Fill ▾
  were ignored (the RUN-W06G quirk), so master edits were not observed.
- Owner-reported before the run: Aspect Ratio offers Maintain, Stretch and
  Zoom; the master theme is reached from the Slide tab.

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-037 | observed (RUN-W06I) | Slide Layout ▾ opens "Select a Layout" with one entry, "Master" (checked, black thumbnail), and a "< Change Theme" link (i02). The Master is a layout of the song's theme, not a separate song setting. |
| EW8-OBS-038 | observed (RUN-W06I) | Background fill ▾ lists None, Color Fill, Gradient Fill, Media Fill (i03). Color Fill shows a color swatch ▾ (blue, `#0000FF`, on first choice) and Opacity 100; the media rows disappear and Apply becomes enabled (i04). Switching back to Media Fill shows "None" with a black background: the earlier image is not kept (i05). |
| EW8-OBS-039 | observed (RUN-W06I) | Select Media… opens a picker: a category ▾ (MEDIA Videos, Images, Feeds; COLLECTIONS ▸; MY COLLECTIONS ▸, Videos first and empty here), a search field, and a grid of thumbnails named without extensions (i06–i08). No import control. One click on a thumbnail chooses it and closes the picker; the pane shows the thumbnail and name (i09). |
| EW8-OBS-040 | observed (RUN-W06I) | Aspect Ratio ▾ lists Maintain, Stretch, Zoom (i10). Choosing an image resets it to Auto (highlighted) + Stretch, which stretches a 16:9 image over the 16:10 (2304x1440) slide (i09). Choosing an aspect turns Auto off (i05, i11). Maintain letterboxes the image with black bars (i11). What Auto means beyond showing Stretch is unrecorded. |
| EW8-OBS-041 | observed (RUN-W06I) | A background is per slide: with slide 1 set to the logo image, a new slide (+) shows the theme's background (sela-w03-background, Auto + Stretch), not slide 1's (i12). The new slide's pane shows only the Slide tab. The slide context menu offers New Slide, Duplicate Slide, Delete Slide, Cut, Copy, Paste, Paste Without Formatting; no revert or reset of the background (i13). |
| EW8-OBS-042 | observed (RUN-W06I) | After Ctrl+A in Words, the Slide pane shows blank values where slides differ: Media Fill with an empty thumbnail and no name, an empty aspect with Auto off, an empty Theme Elements list (i14). Choosing Color Fill then gave every slide the blue fill (slide 1 sampled `0000ff`, slide 2 in the preview; i16, i17). |
| EW8-OBS-043 | observed (RUN-W06I) | Edit Slide Layouts opens a "Layouts ⊗" tab beside Words and Slides with a "Theme - sela-w03-background ▾" selector and one layout card "Master" (black thumbnail). The canvas shows sample lyrics ("song text line one…four") and a "Song Title Example - Author / © 2014 Publishing Company Copyright" footer over the theme background; the pane shows only a Slide tab with Theme Elements and the Master's Background (Media Fill sela-w03-background, Auto + Stretch, Opacity 100) (i19). Not observed: whether a Master change reaches slides without their own background, whether it changes the theme for other songs, and how the Layouts tab closes. |

## RUN-W07A-2026-10-08 — Foundation and Sunday-core unknowns (W04 search, keys, Options, editor, schedule)

- Date/observer: 2026-10-08, Droid agent (mission worker) on the owner's
  machine. EasyWorship 8.0.49.0, unlicensed edition (the "About EasyWorship"
  registration nag was dismissed with Continue at window pixels (574,353),
  96 DPI). Main window `FMTFormEZWorshipMain` "EasyWorship - Default" 1936x1048
  physical at 96 DPI on DISPLAY6; Song Editor 2578x1398 at 120 DPI.
- Fixture: the single library song "waeqwe" (labels "etsaer" and "sadsdasd",
  gibberish lyrics such as "wqeqwewe", "qweqwe", "fedwfeq2fqef"). No song was
  added, edited-and-saved or deleted; the schedule held three drags of that
  song. The Song Editor and the Delete confirmation were cancelled; on exit
  EW asked "The current Schedule is not saved. Would you like to save it
  now?" (Yes / No / Cancel) and the agent chose No. Nothing was saved; no EW
  data directory was written. EW closed via WM_CLOSE and `Get-Process
  EasyWorship` was empty.
- Method: local drivers in `.amp/in/artifacts/ew-w07-driver/` (`w07.py` wrapping
  `.amp/in/explore/ew.py`; `crop.py`, `montage.py`, `grabrect.py`, `drag.py`),
  PrintWindow captures plus screen grabs for popups. Captures (ignored,
  local): `.amp/in/artifacts/ew-w07a-search/`, `ew-w07b-keys/`,
  `ew-w07c-labeljump/`, `ew-w07d-options/`, `ew-w07f-editor/`,
  `ew-w07g-schedule/`, `ew-w07h-update/`, `ew-w07i-songs-menu/`,
  `ew-w07j-boundary/`, `ew-w07k-golive-masks/`, `ew-w07-nag/`.
- Input caveats (tooling, not EW semantics): toolbar buttons (Go Live, Logo,
  Black, Clear) ignore synthetic mouse clicks (the known toolbar quirk), so
  masks were driven with Ctrl+B / Ctrl+L and Go Live with the Live menu
  (Alt+L, Down, Enter). Top menus open only with Alt+letter; a menu opens
  upward and is cut off when the window is maximized, so menus were read at the
  800x600 restored size. Library-pane clicks that missed a control are
  noted below where it matters.
- Only one song exists, so search ranking, multi-hit lists and Sort by effects
  on order could not be observed.

### W04 library search box (topic 1)

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-044 | observed (RUN-W07A) | Songs search box lives at the bottom of the Songs pane, left of it a magnifier/menu icon; placeholder "Search". When the pane first showed the box it displayed the previewed song title ("waeqwe") with the whole text selected and ignored typed keys (typed letters were dropped); clicking the left icon switched it to the empty "Search" field that accepts text. Likely a "current item" display state versus a search state; trigger not isolated (UNOBSERVED exact rule). Best guess for Sela: a plain empty search field, no title echo. |
| EW8-OBS-045 | observed (RUN-W07A) | Live filtering: the list updates after each keystroke, no Enter, minimum length 1 ("w" matches). A "clear" ⊗ button appears at the right of the box while it holds text; clicking it (or Ctrl+A then Backspace, or Esc) empties the box and restores the full list. Focus stays in the box while typing. No-results state: the list is empty with no message text, no "no results" row (typing "z" or "zq"). Enter in the box did nothing visible (no list change, no preview/live change). |
| EW8-OBS-046 | observed (RUN-W07A) | Match rule: case-insensitive **word-prefix** match over title, slide labels and lyric words. Matches: "w", "WAE", "waeqwe" (title word prefix); "qwe", "qweq" (lyric word "qweqwe"); "ets", "etsa" (label "etsaer"); "sad", "sads" (label "sadsdasd"). Non-matches: "aeq", "ae" (substrings inside "waeqwe" do not match), "z". Not a substring match. Whether the field is "contextual" beyond title+lyrics+labels (author, copyright) is UNOBSERVED (single song with empty metadata); best guess: same word-prefix rule over Title/Author/Copyright and lyrics. |
| EW8-OBS-047 | observed (RUN-W07A) | After a search, selection/focus: a click on the result row selects it into Preview; double-click sends it to Preview and Live. Down in the box was not shown to move into the list (UNOBSERVED). **Mask keys while the search box has focus (VAL-REF-001):** plain letters B, C and L were never typed into the box in RUN-W07A (only w, WAE, qwe, ets, sad, z, zq and similar), so whether plain B/C/L typed there toggles Black/Clear/Logo is UNOBSERVED (no new EW run was made). Best guess and Sela rule: plain typing in a text field never toggles a mask; B/C/L become characters in the box. Ctrl+B, Ctrl+L and Ctrl+C with focus in the search box are UNOBSERVED (EW8-OBS-018 observed them only with focus in the Preview list). Best guess and Sela rule: they follow the `SelaShow && !SelaTextInput` key-context policy in `docs/operator-shell.md`, so while the box has focus Ctrl+C is text copy and Ctrl+B/Ctrl+L do not toggle masks; masks stay reachable from the toolbar buttons and from the list/pane contexts. Owners: M1-03, M1-04, M1-07b, M1-10. |

Implementation implications (M1-07b): live filter from 1 character is EW
behaviour; the owner decision is FTS only for >=2 chars, so Sela shows the full
list for 1 character and records it as a deliberate deviation. Use word-prefix
matching on title, labels and lyric words, not substring. Esc and ⊗ clear;
empty result shows an empty list.

### Arrow/page/home/end semantics (topic 2)

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-048 | observed (RUN-W07A) | Schedule focused (three song items): Up/Down move the selected item one step, End selects the last item, Home the first (no wrap at the ends); Preview and Live do not change when only the selection moves. PageDown moved the selection down one item and its highlight turned grey (focus left the Schedule); a following PageUp did not move it back. The exact focus target after PageDown is UNOBSERVED. Right/Left in the Schedule had no visible effect. |
| EW8-OBS-049 | observed (RUN-W07A) | Preview focused (song with 4 slides): Home selects slide 1, End slide 4, Up moved slide 2 → 1; Down at slide 2 did not move it (once; may step through lines inside a slide, UNOBSERVED); Right/Left did not change the slide from slide 1; PageDown left the slide and turned the highlight grey. Treat Preview arrows as partially observed. |
| EW8-OBS-050 | observed (RUN-W07A) | Live focused: Right goes to the next slide (s2 → s3), Left to the previous (s3 → s2); Home → slide 1, End → slide 4; Up and Down did not change the slide. PageDown from slide 2 loaded the **next Schedule item** into Live at its slide 1 (the schedule items were the same song three times); PageUp at slide 1 did not change it. This resolves the help-page conflict as: Left/Right (and Home/End) move slides, PageDown/PageUp move schedule items; Up/Down are not slide navigation in Live. Best guess, not observed: PageUp at slide 1 did nothing here because the previous schedule item did not exist or the item was the first one; the PageUp target (previous schedule item, mirroring PageDown) was not observed. Sela rule: PageUp loads the previous schedule item at its slide 1. |
| EW8-OBS-051 | observed (RUN-W07A) | Boundaries: with slide 4 (last) live, Right stayed on slide 4 twice (no wrap, no automatic jump to the next item); with slide 1 live, Left stayed on slide 1 (no previous item). The schedule selection did not change. Advancing across items is by PageDown (EW8-OBS-050) and by Go Live with Options › Advanced "Advance Schedule after Go Live is pressed". |

Implementation implications (M1-02/M1-03): Left/Right = prev/next slide with
no wrap or item crossing; Home/End = first/last slide of the live song;
PageDown/PageUp = next/previous schedule item (loads slide 1); Up/Down in the
Schedule move the selection. Preview Up/Down and PageUp-in-Schedule remain
open (best guess: Up/Down = line step, PageUp mirrors PageDown).

### Label jump keys (topic 3)

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-052 | unverified (RUN-W07A) | Plain keys V, then 2, then Enter, and C, Enter, and 3, Enter with Live focused and slide 2 live changed nothing. The fixture's labels ("etsaer", "sadsdasd") are not recognised label kinds and a new labelled song cannot reach Live without saving, so a real "V,1,Enter" jump could not be tested. Options › Slide Labels (EW8-OBS-056) lists the key table, so the jump keys exist as a feature of the label table. Best guess: typing the label's shortcut letter, an optional number and Enter jumps to that labelled slide within a short timeout (as the help text says); Sela's label-jump feature will use that and treat unrecognised labels as Verse. **Focus and target (best guess):** the jump needs the Live pane to have keyboard focus (the same pane where Left/Right navigate slides, EW8-OBS-050) and acts on **Live**, applying the matched slide directly (Preview is not changed); with focus in Preview it would move only the Preview selection. Typing in a text field (search box, editor) never starts a jump. Owners: M1-03, M1-10, slide-labels work. |

### Options dialog inventory (topic 4)

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-053 | observed (RUN-W07A) | Menus (read at 800x600): **File** New ▸, Open Schedule Ctrl+O, Open Recent, Save Schedule Ctrl+S, Save Schedule As..., Export Standalone Schedule..., Close Schedule, Reports and Printing ▸, Exit. **Edit** Cut, Copy, Paste, Delete, Select All and Options... (last). **Live** Go Live, Go Back (disabled), separator, Show Logo on Live Output, Black on Live Output, Clear Text on Live Output, separator, ✓ Show Live Output. Also Profiles, View and Help menus (captures `ew-w07d-options/menu-*.png`). The Options dialog opens from Edit › Options (class `FMTForm_Options`, 716x570, modal with OK / Cancel). |
| EW8-OBS-054 | observed (RUN-W07A) | Options is a left list of sections: Main Output, Alternate Output, Foldback, Service Intervals, Slide Labels, Advanced. **Main Output** has a tab strip General, Song, Scripture, Presentations, Transitions, Alerts (captures `main-*.png`, `opt00.png`). |
| EW8-OBS-055 | observed (RUN-W07A) | Main Output › Song tab includes copyright handling: the option "Show on first displayed slide only" governs where the copyright strip appears (unchecked in this install = every slide). Other Song/Scripture/Presentations/Transitions/Alerts controls were read from captures `ew-w07d-options/main-song.png`…`main-alerts.png` (montages `mA.png`, `mB.png`); the full textual control list is in EW8-OBS-056. |
| EW8-OBS-056 | observed (RUN-W07A) | Slide Labels section: table of label kind → shortcut: BRIDGE "B, NUM ."; CHORUS "C"; END/ENDING "E, NUM ."; INTRO "I"; MISC "M"; PRE-CHORUS "R"; TAG "T, NUM ."; VERSE "V" (`sec-labels.png`, `mD.png`). Advanced section contains "Advance Schedule after Go Live is pressed" (checked here) among its options; Alternate Output, Foldback and Service Intervals pages are in `mC.png`/`mD.png`. No Language selector was found in any section (UNOBSERVED beyond that; Sela's English/Indonesian switch is Sela's own). The dialog was closed with Cancel. Textual control list per section (transcribed from the captures, 800x570-class dialog, left list Main Output / Alternate Output / Foldback / separator / Service Intervals / Slide Labels / separator / Advanced; bottom of every page: a preview thumbnail with a red "12, 5" badge, a "Preview Output" checkbox, OK and Cancel):
- **Main Output › General** (`opt00.png`): "Select Output Monitor" group with Output Monitor combo (here "Monitor 2 (Secondary)"), Alpha Channel combo (Disabled), Output Position group with Left, Top, Width, Height spin boxes (-2560, -146, 2560, 1600) and a Monitor Setup button; "Screen Margins" Left, Top, Right, Bottom spin boxes (0); "Options" Default Font menu button and "Disable Logo on Live Output" checkbox (off).
- **Main Output › Song** (`main-song.png`): "Song Layout" Song Font menu, "Show Verse/Chorus Label" checkbox (off) with a disabled Label Font menu; "Copyright Info" "Display Copyright Information" checkbox (on) with a Copyright Font menu, a License Info text box (empty), and "Show on first displayed slide only" checkbox (off).
- **Main Output › Scripture** (`main-scripture.png`): "Enable Scripture Support" ("Enable support for displaying scriptures", on); "Scripture Layout" Scripture Font menu, "Show verse numbers" (on) with Verse Font menu, "Show Reference" (on) with Reference Font menu, Reference Location combo ("After Each Slide"), checkboxes Show Complete Passage (off), Reference Indent (on), Abbreviate Book Names (off), Additional Line Spacing (off), Show Reference Only (Hide Scripture Text) (off), then Break on new verse (off), "Automatically Flow Scripture on Multiple Slides" (on) and "Minimum font size before breaking slide" spin box (48). The page continues below the visible area, so further controls are unknown.
- **Main Output › Presentations** (`main-presentations.png`): "Presentation Layout" with Title Font, SubTitle Font and Content Font menus only.
- **Main Output › Transitions** (`main-transitions.png`): a segmented selector Slide / Black / Clear / Logo (Slide active); for each, "Blend" with Change and Preview > buttons, a Duration slider with a time box ("00:500") and a Show More button. Contents behind Show More are unknown.
- **Main Output › Alerts** (`main-alerts.png`): "Nursery Alert Options": Enable Nursery Alert (on) with Nursery Font menu, Background Color, Background Opacity slider and box (100), Screen Location combo (Top Right), "Automatically Remove" (on) with an After time box (01:00); "Message Alert Options": Enable Message Alert (on) with Message Font menu, Background Color, Background Opacity (100), Screen Location combo (Bottom), Scroll Speed slider.
- **Alternate Output** (`sec-alt.png`): tab strip General, Song, Scripture, Presentations, Transitions, Alerts; General shows "Enable support for Alternate Output" (off) and then the same Select Output Monitor / Output Position / Monitor Setup, Screen Margins and Options (Default Font) controls, all greyed. Other tabs were not captured (unknown).
- **Foldback** (`sec-foldback.png`): tab strip General, Song, Scripture, "Alerts and Clocks"; General shows "Enable support for Foldback Output" (off) and the same greyed monitor, margins and Default Font controls. The preview thumbnail shows a clock ("02:35") and a "next line" strip. Other tabs not captured (unknown).
- **Service Intervals** (`sec-service.png`, `mD.png`): title "Service Intervals - Service start and stop times", an empty table with Day, Start Time, Stop Time columns and an "Add Service Interval" button.
- **Slide Labels** (`sec-labels.png`, `mD.png`): "Slide Labels and Shortcuts" table with columns Label, Colors (two color combos), ShortCut combo and a remove ⊗ per row, plus an "Add Label" button; rows BRIDGE, CHORUS, END, ENDING, INTERLUDE, INTRO, INTRODUCTION, MISC, PRE-CHORUS, SLIDE, TAG, VAMP, VERSE, and an <EMPTY> row (INTERLUDE, SLIDE and VAMP have no shortcut; the table scrolls, rows below <EMPTY> unknown).
- **Advanced** (`sec-advanced.png`): "Live Options": "Show live screen on startup" (on), "Advance Schedule after Go Live is pressed" (on); "DVD Options": "Prevent DVD spin down" (off); "Advanced Font Settings": "Edit Font Fallback List" and "Edit Compatibility Fonts List" menu buttons; "Schedule File Options": "Always save schedules as standalone files (slower)" (off); "Remote Control Settings": "Enable Remote Control" (on) and a Name box ("EasyWorship Remote (THOMAS)").
Implementation implications: M5-06 Options maps General/Song/Scripture/Presentations/Transitions/Alerts and the Alternate/Foldback pages from this list; unknown items (Scripture tail, Transitions Show More, non-General Alternate/Foldback tabs, rows below <EMPTY>) stay open for a later observation. |

Implementation implications (M5-06 settings): mirror the section list; Options
stays Edit › Options with Cancel discarding. Per-field details are in the
captures; do not claim a field that is not in `mA/mB/mC/mD.png`.

### Song Master edit and Ctrl+A in the Song Editor (topics 5, 6)

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-057 | unverified (RUN-W07A) | Whether a Master layout edit reaches existing slides. Edit Slide Layouts was not opened again: RUN-W06I already found synthetic clicks on the Fill control ignored, and no input path was found this run. Best guess (used by M1-05h): slides without their own background follow the Master; slides with their own keep it. **Cross-song/theme scope (UNOBSERVED, implementation rule):** the Layouts tab was reached from the Song Editor, with the "Theme - sela-w03-background" selector naming the theme it edits. Slide inheritance: slides without their own background follow the Master (best guess above). Scope rule for Sela: a Master edit in the Song Editor is a theme-level edit of the named theme, so other songs using that theme follow it only when they were saved against the same theme; to keep edits song-local, Sela copies the theme on first Master edit (copy-on-write) and tells the operator; deliberate theme-wide edits go through the Themes tab. Why unobserved: the Fill control ignored synthetic clicks and saving was forbidden. Owners: themes-tab-editor, theme-model-storage, M1-05h. |
| EW8-OBS-058 | observed (RUN-W07A) | Ctrl+A in Words (caret in slide 2) selects the whole document and the preview moves to the last slide. The Format pane then shows two tabs, **Slide and Text**, and clicking Text works: it shows Style/Layout (Tahoma, Auto, etc.) for the multi-selection, with Theme Elements blank. This differs from EW8-OBS-035 (only Slide tab while all slides selected in the Slides tab): the Text tab stays reachable after Ctrl+A in Words (`ew-w07f-editor/e09-ctrla.png`, `e10-text-click.png`). The editor was cancelled with Cancel → No. |

### Schedule (topic 7)

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-059 | observed (RUN-W07A) | Dragging the song row from the Songs list onto the Schedule pane adds an item ("Schedule - UNTITLED"); three drags gave three items titled "waeqwe". Double-click on a library row sends it to Preview and Live, not to the Schedule. Right-click on a schedule item: Add Item ▸, Edit Item..., Edit Title..., Edit Note..., Move Item Up, Move Item Down, Remove from Schedule, Duplicate Slide (disabled), Expand All Items, Collapse All Items, Check Schedule for changes..., ✓ Auto-Expand Items When Dragging, Refresh Thumbnails, View ▸. There is **no** Duplicate Item or Copy Item entry. |
| EW8-OBS-060 | observed (RUN-W07A) | Ctrl+C then Ctrl+V and Ctrl+D with a schedule item selected did not add or copy an item. Multi-select (Ctrl/Shift click) was not captured (UNOBSERVED; best guess: Ctrl/Shift multi-select as in other lists, Move/Remove apply to the selection). Reorder by drag inside the Schedule was not captured. **Best guess, drag reorder (UNOBSERVED; schedule-remainder):** dragging an item row inside the Schedule moves it, with a horizontal insertion line between rows as the drop indicator, autoscroll when the pointer is near the top or bottom edge, Esc cancelling the drag with the order unchanged, and a drop on the same position being a no-op; Move Item Up/Down (EW8-OBS-059) is the keyboard-equivalent. Reason: synthetic mouse-drag inside the Schedule was not captured in RUN-W07A. The earlier work-log wording that this best guess already existed was incorrect; it is first recorded here. |
| EW8-OBS-061 | observed (RUN-W07A) | Library context menu on a song: New Song..., Edit Song..., Delete, Update items in Schedule, Sort by ▸, Refresh. With an empty schedule "Update items in Schedule" is disabled. With three schedule items it is enabled; choosing it (song unchanged) closed the menu with no dialog (UNOBSERVED: the dialog wording/diff only shows when the song changed, which needs a save). Best guess: a confirmation listing the affected schedule items; Sela's equivalent shows a count and Yes/No. |

### Song properties and copyright (topic 8)

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-062 | observed (RUN-W07A) | Song Editor › Presentation › Info tab fields: Title, Author, Copyright (with a © button), Administrator, Song Number, and Reporting Provider ▾ (None). The Presentation pane sits at the right end of the toolbar and has tabs; Info is the metadata tab. |
| EW8-OBS-063 | observed (RUN-W07A) | Copyright on output: the slide's copyright strip (theme element "copyright") renders "Title - Author" on line 1 and the Copyright text on line 2 (typed Author "Synthetic Author", Copyright "Synthetic Copyright 2026" → strip "waeqwe - Synthetic Author…" / "Synthetic Copyright 2026"); updates as typed. Song Number was not shown in the strip. Administrator display and the CCLI number mapping are UNOBSERVED (a click aimed at the Administrator field typed into another field). First- vs last-slide: Options › Main Output › Song "Show on first displayed slide only" decides it (unchecked: every slide); the editor preview showed the strip on the shown slide. **Best guesses (UNOBSERVED; song-metadata-fields):** Administrator is stored and sortable (EW8-OBS-064) but not drawn in the default copyright strip, which shows only "Title - Author" and Copyright; Song Number is likewise not drawn and is an own field. CCLI number mapping: the Song Number field holds the CCLI song number, and the Administrator field the administering publisher; if a theme or the License Info box (Options › Main Output › Song, EW8-OBS-055) is non-empty, Sela appends it as a licensing line (best guess "CCLI Song # N" plus License Info). Reason: a click aimed at the Administrator field typed into another field, and saving was forbidden. The earlier work-log wording claiming these best guesses already existed was incorrect. |

### Songs list Sort by and Delete (topic 9)

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-064 | observed (RUN-W07A) | Library context menu Sort by ▸ submenu: Title (✓), Author, Copyright, Administrator, Song Number; separator; Ascending (✓?) / Descending. The checked direction mark was read as Descending checked in the capture; treat direction default as unconfirmed. `ew-w07i-songs-menu/ctx2-sortby.png`. |
| EW8-OBS-065 | observed (RUN-W07A) | Delete opens a "Confirm" dialog (class `FMTForm_CollectionItemRemoveDlg`) "Are you sure you want to permanently delete this song?" with Yes and No. The default is No (it looks focused); Esc did **not** close it; clicking No closed it and kept the song. Enter was not pressed (it could have deleted). |

### Go Live under masks (topic 10)

| ID | Status | Observation |
| --- | --- | --- |
| EW8-OBS-066 | observed (RUN-W07A) | With Black on (Ctrl+B), selecting Preview slide 2 and Live › Go Live moves Live to slide 2 and Black stays on; Ctrl+B again turns it off. Same with Logo (Ctrl+L): Go Live to slide 3 kept Logo until Ctrl+L. This repeats and agrees with the W03 masked-apply rows. Toolbar button clicks were ignored by synthetic input and are not part of this observation. |
| EW8-OBS-066b | unverified | **Pending library Go Live versus a newer Live selection.** In Sela a Songs-tab Go Live of a song that is still loading stays pending (`live_on_load`); a later Live click or step on the current item does not clear it, so the older Songs Go Live replaces the newer Live selection once the song loads. A newer Songs Go Live, a refused Go Live and a mask toggle do end an armed double-click unmask. EW ordering for the pending-load case has not been observed; behavior is left unchanged and tracked as an unknown (M1-10). |

Implementation implications: Go Live and navigation keep masks; the Delete
confirmation defaults to No and ignores Esc (Sela should still allow Esc =
No); Sort by lists Title, Author, Copyright, Administrator, Song Number plus
direction.

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

### Schedule files — UNOBSERVED best guesses (2026-10-15, schedule-files)

EasyWorship 8.0.49 was not run for the schedule-files change. Nothing below is
an observation; each item is a reversible best guess to check in the next EW
session (observe only; never save in EW; cancelling a Save dialog is fine).

| ID | State | Record |
|---|---|---|
| EW8-OBS-067 | UNOBSERVED | Main-window title bar with a saved/unsaved schedule file. Sela shows "name.selaschedule — Sela" with `*` after the name when unsaved and "Untitled schedule* — Sela" before the first save. |
| EW8-OBS-068 | UNOBSERVED | Default folder of the native Save/Open schedule dialogs. Sela starts beside the current file, else beside the newest recent file, else `<profile>\Schedules`. |
| EW8-OBS-069 | UNOBSERVED | Open Recent labelling and count (Sela: file name only, up to 10, newest first). EW8-OBS-053 records only that the item exists. |
| EW8-OBS-070 | observed text, reused | Unsaved-changes wording "The current Schedule is not saved. Would you like to save it now?" comes from the recorded EW exit prompt (Yes/No/Cancel); Sela's buttons are Save / Don't Save / Cancel with Cancel as the default. |

- schedule-remainder (2026-10-15): Sela adds a Duplicate Item entry to the schedule item menu although EW8-OBS-060 observed none in EW (deliberate deviation, VAL-SCHED-001). Shift/Ctrl multi-select, group move, drag-reorder insertion line/autoscroll/Esc cancel, and the Update items diff dialog remain best guesses (UNOBSERVED beyond EW8-OBS-059..061).

- Note (preview-live-navigation, 2026-10-15): EW8-OBS-052 label jump keys remain UNOBSERVED; Sela implements C/E/I/M/R/T/V + number + Enter on the Live pane as a best guess, with Bridge on Shift+B because bare B/period are Black for presenter remotes.

## Owner-approved deviations from observations

- **EW8-OBS-045, search minimum length:** EW filters from one character; Sela starts at two (owner decision, bounds the one-letter p95 of 109 ms measured in M1-07a). One character shows the full list and issues no query.
- **EW8-OBS-045, label matching:** EW also matches slide/section text labels; Sela does not index them (recorded gap, docs/search.md and docs/backlog.md).
- **EW8-OBS-064, Sort by:** Sela offers Title and Date modified with Ascending/Descending; Author, Copyright, Administrator and Song Number are backlog (docs/backlog.md M1-04). The checked direction in EW was unconfirmed, Sela defaults to Ascending. The choices open in place under Sort by instead of a flyout.
- **EW8-OBS-065, Delete confirmation:** wording and Yes/No with default No follow EW; Esc closes the dialog as No in Sela (EW ignored Esc).

## Song metadata best guesses (song-metadata-fields)

- Copyright footer is drawn on every slide (EW8-OBS-063, first-slide-only option unchecked).
- Administrator is stored and shown in the Inspector but not drawn in the footer (unobserved).
- CCLI maps to the Song Number field.
- Metadata search covers authors, copyright, license, administrator and song number.
- Close prompt: Yes saves and closes, No discards, Cancel or Esc keeps editing; it is modal.

### Stage output (Foldback) — UNOBSERVED best guesses (2026-10-22, stage-display)

EasyWorship 8.0.49 was not run for the stage-display stream (no Windows host).
The only observed facts are EW8-OBS-054/056: Options has a **Foldback**
section with tabs General, Song, Scripture and "Alerts and Clocks"; General
shows "Enable support for Foldback Output" (off) and its own monitor, margins
and Default Font controls; the preview thumbnail shows a clock ("02:35") and a
"next line" strip. Everything below is a reversible Sela best guess; IDs are
stream-scoped so parallel streams do not collide (the integrator may renumber).
Steps and re-runs are the PP-STG items in [parity-pending.md](parity-pending.md#stream-stage-display).

| ID | State | Record |
|---|---|---|
| EW8-OBS-STG-1 | UNOBSERVED | Foldback content and layout. Sela: header with "title · slide name" (left) and the clock (right), optional message band, current slide large, divider, "Next · <slide name>" and the next slide smaller; dark background (16,17,20), white current text and clock, sky-blue next, amber labels and message; drawn in the live slide's resolved face (bundled DejaVu Sans while nothing is live), characters that face lacks shown as U+FFFD stand-ins, fitted per region. EW's Foldback › General "Default Font" may own this instead. |
| EW8-OBS-STG-2 | UNOBSERVED | What "next" is at the last slide. Sela: "Next item · <title>" of the following schedule entry (its text is not loaded), "End of schedule" after the last entry or for a library song outside the schedule; next is computed from the live item's own slide snapshot (repeated chorus = next where the arrangement repeats it). |
| EW8-OBS-STG-3 | UNOBSERVED (format seen) | Clock: the preview showed "02:35". Sela: local time of day, 24-hour "HH:MM", no seconds, drawn by the stage process itself and updated on the minute; on by default with an Options switch. Countdowns/timers are out of scope (owner, Mission 2026-10). |
| EW8-OBS-STG-4 | UNOBSERVED | Black/Clear/Logo and the foldback. Sela: masks target the Main Output only; the stage keeps showing the current and next slide so performers keep their place. |
| EW8-OBS-STG-5 | UNOBSERVED | Foldback lifecycle and menu. Sela: with Foldback enabled the stage starts and stops with Live › Show Live Output; Sela adds Live › Show Stage Output (stop a running stage, restart a lost one in one activation, start one while Live output is on; inert while Live output is off) and Live › Stage Message… (EW8-OBS-053 lists neither). |
| EW8-OBS-STG-6 | UNOBSERVED | Stage-only message. Sela: one line (≤ 160 characters) typed privately in a dialog and sent only on Show on Stage; never drawn on the audience. EW's Alerts (EW8-OBS-056 Main Output › Alerts) are M3-04 and may own this. |
| EW8-OBS-STG-7 | UNOBSERVED | Foldback monitor default with no free monitor. Sela: "Automatic" picks a monitor that is neither primary nor the audience's; with none, the stage is not started and the Live pane says so; an explicit choice (even the audience or primary monitor) is honoured. Only Foldback › General is built; Song, Scripture and Alerts and Clocks tabs show "not available". |
