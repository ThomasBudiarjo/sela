# Implementation backlog

This is the executable backlog for [the engineering baseline](plan.md), from an
empty repository to a qualified release. Updated 2026-10-04. A native bootstrap
exists; the work log distinguishes implementation from qualification.

The goal is comprehensive EasyWorship feature, layout, and operator-UX coverage
with original code/assets, Rust, GPUI, offline operation, and a separately paced
live compositor. Minimum viable Sunday is an intermediate gate, not the end of
the product. This backlog is not proof that every EasyWorship capability has
already been discovered: M0-02 owns the exhaustive reference audit and must add
tickets for gaps. Never silently shrink the target to the easiest subset.

## How to pick up work

1. Read `AGENTS.md`, `plan.md`, [the work log](work-log.md), and this document.
2. Check Git status; preserve unrelated work. Find the active ticket in the log.
3. Resume its first unfinished checklist item. Otherwise pick the earliest
   dependency-ready ticket. Start with **M0-01**, then M0-03/M0-04; M0-02 can run
   alongside these when the reference installation is available.
4. Read the ticket's dependencies, reference observations, and required test
   environments. Record scope and intended checks before implementing.
5. Make a coherent slice, run applicable checks, inspect UI changes, and update
   checkboxes only from evidence. Commit each completed ticket or partial slice.
6. Update the work log in that same commit: state, files/decisions, exact commands
   and results, unexecuted checks, blockers, and a concrete next action.

IDs are permanent. Add child IDs (for example `M1-05a`) if a ticket is too large;
retain the parent and its acceptance criteria. Do not renumber existing work.
Dependencies below are minimum prerequisites, not a demand for serial execution.
Cross-cutting reference evidence is also required for every parity-sensitive
workflow even when M0-02 is not repeated in its dependency line.

Sequencing exception approved by the owner on 2026-10-04: provisional operator
UI may proceed while physical renderer qualification is unavailable. M1-02a
starts the reversible shell ahead of M0-10; reference unknowns and hardware/
release gates remain open. This permission is not a feature-parity finding.

### Milestone navigation

| Milestone | Tickets | Gate |
| --- | --- | --- |
| [M0: Output feasibility](#m0--establish-output-feasibility) | 10 | M0-10: independent output qualified on Windows |
| [M1: Minimum viable Sunday](#m1--minimum-viable-sunday-saved-and-recoverable) | 16 | M1-15: saved/reopened service and 90-minute rehearsal |
| [M2: Media](#m2--production-media-and-bounded-resource-usage) | 8 | M2-08: media/failure/performance qualification |
| [M3: Production outputs](#m3--production-outputs-messages-and-timing) | 8 | M3-07: multi-output soak |
| [M4: Authoring and interoperability](#m4--complete-authoring-interchange-and-control-coverage) | 12 | M5-06 audits complete workflow coverage |
| [M5: Qualification and release](#m5--optimization-platform-qualification-and-release) | 9 | M5-09: qualified candidate and authorized publication |

### Status and closure

All implementation tickets initially have status **planned**. The work log is
the status index; each ticket below is the checklist source of truth.

- `planned`: not started; dependencies or resources may still be missing.
- `active`: implementation/checks in progress; exactly identify the current slice.
- `blocked`: record dependency, device, credentials, decision, or external access
  needed and the smallest unblock action. Preserve completed checkboxes.
- `implemented-unqualified`: code exists and available checks passed, but a
  required reference/native/hardware check has not run. This is **not done**.
- `done`: all checklist items and the common definition of done are satisfied.
- `deferred`: explicit user-approved scope decision, with reason and revisit
  condition. This does not count as feature parity or passing a release gate.

A feasibility ticket may finish with a documented negative finding, but its
downstream feature remains blocked/deferred, not implemented. Do not mark a
milestone complete while its required tickets are unqualified. A narrower
release needs an explicit scope decision and published limitations.

### Common definition of done (applies to every ticket)

- [ ] Ticket-specific checklist and acceptance behavior are satisfied.
- [ ] Relevant EasyWorship observation IDs are linked, or uncertainty/deviation
  is explicit and approved before claiming parity.
- [ ] Tests cover the likely wrong implementation and at least one meaningful
  failure/boundary case; expected results are not derived from the code tested.
- [ ] Exact test commands, decisive results, OS/toolchain/device where relevant,
  and evidence paths are in the work log. Do not invent passing commands.
- [ ] Visual UI changes were rendered and inspected; interaction changes have
  native action/focus/accessibility checks where available.
- [ ] Performance-sensitive code has bounded work/memory and appropriate timing
  evidence. No new blocking work on operator/live frame paths.
- [ ] Data/version compatibility, licenses, and privacy implications are resolved
  where applicable; operator-facing errors explain recovery.
- [ ] Work log and checklists updated; coherent local commit made. Push, PR,
  deployment, and release publication require separate user authorization.

These global boxes are a reusable checklist, not a global completion counter.
Record their per-ticket outcome in the work log rather than checking them once
for the entire project.

## Test boundaries and E2E policy

| Label | What can be established | What it cannot establish |
| --- | --- | --- |
| U | Pure Rust unit/property/state-machine tests; deterministic fake clocks and faults | Real GPU/native event behavior |
| I | Temporary SQLite/files, import/export, worker/protocol/process integration | Physical scanout, external device reliability |
| R | Offscreen render/readback and approved golden images on declared backend/fonts | Projector timing, all GPU drivers, human readability |
| N | Running native GPUI app: input, focus, resize, accessibility and visual inspection | Hardware not attached to the tested machine |
| E | Real app E2E driven through native inputs/accessibility, if the M0-03 spike works | A mock backend alone is not audience-output E2E |
| H | Physical target hardware: Windows first, later macOS/Linux/mobile; GPU, displays, audio/capture/control devices, hotplug and power events | Untested hardware/OS combinations |
| P | Repeatable performance/load/soak scenario on a named machine and build | Performance on unspecified machines |
| L | License/provider/API review and permission evidence | Legal availability inferred from a working parser or SDK |

The orb can execute U/I after toolchain setup. R/N depend on installed graphical
support and must be tried, not assumed unavailable. Linux orb success does not
qualify Windows. H requires an actual target machine. E is conditional: test the
native automation path first; do not add a web frontend or browser-driven mock
just to get an E2E badge. Browser E2E is appropriate for a real remote web client.
If native automation is unreliable/unavailable, use U/I/R plus reproducible N/H
scripts, document the limitation, and continue. A skipped E check does not waive
its underlying acceptance behavior. No requirement to build custom automation
infrastructure beyond what proves useful.

Fixture policy: synthetic/original lyrics, redistributable fonts/media, temporary
profiles, fixed seeds, non-sensitive failure logs. Never commit licensed songs,
Bible databases, tokens, or private service content as test fixtures.

### Repeatable end-to-end scenarios

These are scenario IDs, not claims that automation currently exists. Use E when
feasible and N otherwise; retain H checks for actual audience presentation.

| ID | Actions | Required assertion | Owner |
| --- | --- | --- | --- |
| S01 | Fresh offline profile → create song/theme → schedule → save → close/reopen → Go Live | Same revision/assets/layout; audience applies the intended cue; operator Live follows apply acknowledgment | M1-15 |
| S02 | Present A → edit/select B → next/previous → mask toggles → Go Live under mask | Editing never changes A; reference-observed mask restoration and focus rules hold | M1-15 |
| S03 | Commit revision → make newer edit → terminate process during autosave → reopen | Last committed revision recovers, no partially saved schedule, unsaved loss disclosed | M1-12 |
| S04 | Transfer bundle to clean offline machine with missing font/asset | Preflight explains missing dependencies; repair works; no silent substitute claimed exact | M1-13 |
| S05 | Video + lyrics live → search/import/edit → bad decoder/slow storage → mask | Current scene remains valid; bounded queues; measured frame/control responsiveness | M2-08 |
| S06 | Audience + stage + stream → output-specific message → disconnect/reconnect display | No private stage data on audience; unaffected outputs continue; explicit remap/recovery | M3-07 |
| S07 | Unpaired remote request → pair limited role → disconnect/reconnect → revoke | Unauthorized commands rejected; no replay/stale cue; local safety control retained | M4-09 |
| S08 | Install → create service → upgrade → reopen → uninstall preserving data → reinstall | No committed data loss; upgrade failure has documented recovery | M5-07 |

## Reference feature coverage

Sources checked 2026-10-04:

- [Official features](https://easyworship.com/software/features): feature-family
  inventory, **not** a build-specific behavioral specification.
- [Official landing page](https://www.easyworship.com/landing): editor/shape engine,
  alternate output, nursery alerts, media and control integrations.
- [Support/training](https://support.easyworship.com/support/home): discover
  workflow instructions; verify each article's version before using it.
- [Scripture guide](https://support.easyworship.com/support/solutions/articles/24000020401-scriptures):
  historical 2020 documentation only; not evidence of exact 8.0.49 behavior.
- [Pinned reference update listing](https://www.easyworship.com/software/update):
  EasyWorship 8.0.49 per the baseline; installed reference still needs observation.

Marketing pages may describe newer features. M0-02 records whether each exists
in the pinned build. No exact shortcut, mask precedence, pane dimension, or
drag/drop behavior has been verified in this planning task.

| Feature family / audit target | Implementation owners | Evidence still needed |
| --- | --- | --- |
| Schedule, Preview, Live, Resources, resizers, thumbnail sizing, contiguous/combined layout modes | M1-02–M1-04, M1-09, M1-10 | Geometry and every selection/focus/navigation mode |
| Songs, sections, arrangements, metadata, search, editing, usage reports | M1-05–M1-07, M4-05 | Exact editor and library workflows |
| Themes, typography, backgrounds, transitions, copyright positioning | M0-07, M1-08, M4-01 | Style inheritance and fit rules |
| Go Live, Black/Clear/Logo, media controls, keyboard/presenter remote | M0-06, M0-08, M1-10, M2-03, M4-11 | State-transition and output-target truth tables |
| Images, video, audio, playlists, resource collections/thumbnails | M1-11, M2-01–M2-07 | Supported formats, looping and continuation behavior |
| Scripture translations, reference/keyword search, continuous browsing, verse selection | M4-03, M4-04 | Build-specific UX, distribution licenses |
| Presentation/theme editor, shapes/layers, slide animation, announcements | M4-01, M4-02 | Full tool/inspector/animation inventory |
| PowerPoint, PDF, existing library/service import and portable interchange | M1-13, M1-14, M4-06, M4-07 | Fidelity and legal format support boundaries |
| Audience, confidence/stage, alternate/stream/house outputs, per-output themes | M3-01–M3-03 | Routing, content and control behavior |
| Messages, nursery alerts, clocks/countdowns/timers; stage chords/notes | M3-02, M3-04, M3-05 | Recipient, priority, dismissal, timer semantics |
| Live camera/capture, NDI input/output/HX, alpha/key-fill, OBS/vMix/Wirecast | M2-07, M3-06, M3-08 | SDK rights, actual device and receiver tests |
| One-click web display, online video | M4-08 | Sandboxing and supported playback rights |
| Mobile remote, pairing, permissions | M4-09, M4-10 | Native/mobile-web feasibility and real devices |
| MIDI send/receive/sync, Stream Deck, Companion, presenter remotes | M4-11 | Protocol mapping and actual controller tests |
| SongSelect/CCLI, reporting, premium media, online designer, Planning import | M4-05, M4-12 | Provider permission/API availability; separate services not automatically part of offline core |
| Profiles/settings, backup, recovery, help, installation, accessibility/localization | M1-12, M1-13, M1-16, M5-03–M5-08 | Full settings/dialog inventory and native behavior |

M0-02 expands this into individual observations. New discoveries must get an
owner ticket, not disappear into an unsupported-features footnote. Licensed
catalogs and hosted subscription services need permission/feasibility decisions;
aim for equivalent workflows without copying proprietary content or requiring
cloud accounts for the core application.

### Reference observation record

M0-02 should create a durable observation ledger when observations begin. Use
stable IDs such as `EW8-OBS-001`, linked from tickets and tests. Each record needs:

- Build/OS/DPI/output layout and source provenance (installed observation versus
  version-qualified documentation); date and observer.
- Preconditions and original/synthetic fixture; exact input/action sequence.
- Observed selection, focus, Preview, Live, audience, media/audio and mask state.
- Expected assertion, implementation ticket and automated/manual test mapping.
- Screenshot/recording path when useful, with no licensed/private content leaked.
- Status: observed, documented-only, unverified, conflicting, or approved deviation.
  For a conflict, preserve both observations and specify the next discriminating test.

## M0 — Establish output feasibility

### M0-01 — Reproducible Rust/GPUI bootstrap

Depends on: none. Tests: U, N; Windows build required.

- [x] Inspect current Zed `crates/gpui` and relevant minimal window/action examples;
  record upstream commit/paths and license boundaries before adopting patterns.
- [x] Pin Rust toolchain and known-good GPUI revision; choose the smallest crate
  structure with clear ownership, not an empty crate for each future feature.
- [x] Add application entry point and a minimal operator window with clean exit.
- [x] Document Linux development and Windows build prerequisites and commands.
- [x] Add formatting, lint, unit-test and Windows build CI; keep GPU tests separate.
- [x] Document run/test commands from a clean checkout; smoke-open on Windows.

Acceptance: reproducible build and native window; no renderer-independence claim.
Orb setup can be a partial slice; a cross-compile alone does not satisfy Windows
native launch. No full pane design, database, or media stack in this ticket.

### M0-02 — Build-specific compatibility evidence and gap inventory

Depends on: none; reference installation/access required. Tests: N, H, L.

- [ ] Record actual EasyWorship version/build, OS, display geometry/DPI, and lawful
  observation method; keep private content out of screenshots.
- [ ] Audit all menus, settings, dialogs, panes, layout modes, editors, resource
  tabs, context menus, keyboard actions, focus transitions and drag/drop targets.
- [ ] Record observation IDs with setup, exact action sequence, resulting state,
  screenshot/recording where useful, and version-qualified source.
- [ ] Build a mask truth table: precedence, restore, audio/video continuation,
  Go Live under mask, per-output targeting, repeated toggles and startup state.
- [ ] Audit the feature families above, mark absent/newer-build features, and
  create child tickets for every uncovered workflow or approved deviation.
- [ ] Map observations to acceptance scenarios; mark all unknowns explicitly.

Acceptance: implementation has a traceable behavioral reference, not screenshots
alone. Partial observations unblock only the workflows they actually cover.

### M0-03 — Test harness and native E2E feasibility

Depends on: M0-01. Tests: U, I, N, E feasibility.

Bounded slice **implemented-unqualified**; see `docs/native-testing.md` and the
M0-03 work-log record. Windows/native accessibility and physical checks remain open.

- [ ] Add synthetic fixture conventions, isolated temporary profiles, fixed clocks
  and reproducible failure injection without production-only test shortcuts.
- [x] Inspect current GPUI test/action/focus patterns and Windows accessibility
  support; try a native open-window → focus → action → assert-state smoke test.
- [x] Evaluate stable element/action targeting; distinguish direct action tests
  from true input-driven E2E and offscreen output from physical presentation.
- [x] If viable, keep one minimal automated smoke test and bounded timeouts; if
  not, record the precise blocker and a reproducible manual equivalent.
- [x] Separate headless CI from native/GPU jobs with explicit skip reasons;
  prohibit a skipped job from appearing as hardware qualification.
- [x] Define evidence format: build, environment, fixture, command, result, artifact.

Acceptance: a useful layered test path exists; native E2E is optional, not a
reason to switch GPUI to a web implementation or postpone all other testing.

### M0-04 — Independent audience-output architecture spike

Depends on: M0-01. Tests: I, N, H, P.

- [x] Prototype a separately paced audience surface showing animated diagnostics
  while GPUI remains the operator frontend; no full operator layout yet.
- [x] Compare feasible backend/window ownership and render-thread/process choices;
  inspect upstream patterns and record the chosen boundary and alternatives.
- [x] Inject deliberate UI stalls and slow preparation; measure output continuity
  separately from control responsiveness during the stall.
- [x] Test operator process termination according to the chosen architecture;
  state exactly which failures output can and cannot survive.
- [ ] Exercise mixed-DPI dual monitors, resize, fullscreen and hotplug on Windows.
- [ ] Record frame timing, hardware/driver, failure behavior and a go/no-go decision
  before committing to a production backend or full workspace UI.

Acceptance: real Windows evidence for independence, or a documented failing spike
and follow-up architecture work. A render thread alone is not crash isolation.

Linux partial evidence: [output-spike.md](output-spike.md), implemented-unqualified.
UI stalls, bounded delayed preparation, clean exit and SIGKILL measured.
Windows DX12 (2026-10-06) passed stalls, preparation, exit/kill, mixed-DPI moves
across three physical monitors, resize, borderless fullscreen and Intel/NVIDIA
selection. Hotplug, device loss, present-mode/refresh pacing and idle-machine
timing remain open, so both remaining boxes stay unchecked. Preparation is synthetic latency injection,
not production decoding or prepared-scene ownership.

### M0-05 — Framework-independent scene and preparation contracts

Depends on: M0-04. Tests: U, I.

CPU preparation slice: [preparation.md](preparation.md). Renderer integration,
shaping/upload and applied-state follow-through remain open; no gate is waived.

- [x] Define minimal immutable scene/cue types, stable identities, revisions,
  asset/font references and renderer capabilities without GPUI types.
- [ ] Separate selected/editing content, prepared cue and applied scene state.
- [ ] Move filesystem/decode/font preparation off UI and live frame paths.
- [x] Define ownership/lifetimes, cancellation, preparation timeout and budgets.
- [ ] Test missing asset, invalid scene and late completion after cancellation;
  preserve the prior valid scene in every failed preparation case.
- [x] Document versioning needs at the renderer boundary without inventing a
  public plugin API or unnecessary transport abstraction.

Acceptance: a cue is either ready to apply or rejected without changing live.

### M0-06 — Ordered cue delivery, acknowledgments and safety capacity

Depends on: M0-05. Tests: U, I.

CPU model and M0-06b/c native owned-resource IPC: [delivery.md](delivery.md).
Bounded font/text/image frames, worker preparation/upload, native submit/present
receipts, stale/failed/expired scene retention and fresh-epoch/no-replay checks
pass in the standalone diagnostic. Oracle regressions cover matched disconnected
receipts invalidating both lanes and bounded preparation backpressure preserving
FIFO/deadlines; consecutive native Cue/Safety receipts and final pixels pass.
Production preparation-intent coordination,
pre-resolved safety resources, supervision and reference mask semantics remain open.

- [x] Define sequence/epoch rules, accepted/applied/rejected acknowledgment meanings,
  stale rejection and restart behavior; distinguish submission from visibility.
- [ ] Implement a bounded command boundary with explicit overload policy and
  reserved safety-control capacity; retain ordering guarantees across lanes.
- [x] Model requested versus acknowledged state; never optimistically label a
  requested cue as live or replay old pending cues after reconnection.
- [ ] Test reorder, duplicate, delayed preparation, out-of-order acknowledgments,
  saturation, renderer disconnect and timeout with asymmetric cue revisions.
- [ ] Test safety commands under media-load saturation, including ordering when
  a mask and Go Live cross; use the M0-02 specification for operator semantics.
- [x] Expose actionable errors and counters without logging lyrics by default.

Acceptance: only a current prepared cue applies; failed/stale work cannot replace
live; safety delivery has an explicit bound and no unbounded queue.

### M0-07 — Minimal composition, transitions and decoder interop spike

Depends on: M0-05, M0-06. Tests: U, R, H, P.

Static diagnostic: [composition-spike.md](composition-spike.md), including actual
font/GPU readback. [Video diagnostic](video-spike.md) establishes a software
FFV1/RGBA copy path. M0-07d now renders worker-prepared explicit-font text over
asymmetric images on a native surface, with receipt-correlated retention checks.
Resized-output preparation, transitions, production budgets and hardware remain open.
Windows DX12 composition and video readback passed on Intel UHD and RTX 4060
(2026-10-06); the native cue driver is ported to Win32 but not yet run there.

- [x] Render text over solid color and still images with explicit logical/physical
  coordinates, aspect fit/crop, alpha and color-space assumptions.
- [x] Establish shaping/font fallback/line-break ownership and deterministic
  reference fonts; include non-ASCII and overflow fixtures.
- [ ] Implement cut/fade and interrupted-transition behavior from observations;
  retain immutable source/destination scenes for the required lifetime.
- [x] Prototype text over a decoded video surface to settle decoder/GPU interop
  before M2; document copies, sync and licensing tradeoffs, not full video UX.
- [ ] Inspect rendered normal/overflow/transition frames; compare golden output
  within a documented backend tolerance rather than universal pixel identity.
- [ ] Measure composition cost and allocation behavior; record GPU evidence.

Acceptance: core composition is correct; video spike findings constrain M2 rather
than claiming production codecs/audio support.

### M0-08 — Black/Clear/Logo state machine

Depends on: M0-02, M0-06, M0-07. Tests: U, I, R, N, H.

Progress: M0-08a (mask layer on the Safety lane, logo slot, per-lane renderer
ordering) is **implemented-unqualified**; see [delivery](delivery.md#m0-08a--mask-layer-and-logo-slot).
Operator toggles, picture logo and native agreement checks are M1-10c
(**implemented-unqualified**, see [operator shell](operator-shell.md#m1-10c--masks-and-picture-logo)).

- [x] Translate observed mask behavior into explicit transitions and tests before UI.
- [x] Implement masks independently from editing/selection and apply via the
  ordered command boundary, including repeated toggles and Go Live under mask.
  (Boundary and coalescing in M0-08a; operator wiring in M1-10c.)
- [ ] Implement observed restoration, logo resolution, transition and output scope.
  (Restoration and picture logo done for single static items; logo fit is
  Contain vs. observed "fills the output", transitions and multi-output open.)
- [x] Test all mask combinations, stale acknowledgments, failed logo preparation,
  and masks while a scene transition is pending (headless and fake audience).
- [x] Verify visible output and acknowledged operator indicators agree.
  (Windows DX12 only: `live-output-windows.py` and `native-cues.py`, M1-10c.)
- [ ] Record media/audio cases pending M2; do not invent their semantics now.

Acceptance: observed static-scene safety semantics hold, with explicit pending
media extensions and no UI-only indicator pretending to change audience output.

### M0-09 — Surface recovery and frame diagnostics

Depends on: M0-04, M0-06, M0-07. Tests: I, N, H, P.

- [ ] Expose adapter/output IDs, frame intervals, missed frames, preparation and
  acknowledgment timings; bound buffers and separate CPU/GPU memory estimates.
- [ ] Define lifecycle for resize, minimize, occlusion, output loss, device loss
  and renderer restart; identify recoverable versus fatal errors.
- [ ] Rebuild invalid surfaces/resources off the critical path where possible;
  never apply stale cues during recovery.
- [ ] Test injected loss and actual unplug/replug, DPI moves and sleep/resume.
- [ ] Define operator warnings and explicit reassign/retry controls; do not
  silently present to an unintended monitor after topology changes.
- [ ] Verify diagnostics exclude content and do not introduce measurable stalls.

Acceptance: recovery behavior and its limits are reproducible on named hardware.

### M0-10 — Output feasibility gate

Depends on: M0-02, M0-03, M0-08, M0-09. Tests: H, P.

- [ ] Run a Windows dual-display scenario: text/image, transition, mask, deliberate
  UI stall, overload, mixed DPI, unplug/replug and renderer failure.
- [ ] Record observed behavior, frame/control metrics, unresolved defects and
  decoder-interop findings using redistributable content.
- [ ] Decide backend/window/process design and pin relevant dependency revisions.
- [ ] Create corrective tickets for failures; do not waive independence to start UI.
- [ ] Publish the verified failure boundary and approve entry to M1.

Acceptance: the highest-risk output assumptions have executed evidence. Headless
success cannot close this gate.

## M1 — Minimum viable Sunday, saved and recoverable

### M1-01 — SQLite schema, migrations and repository boundary

Depends on: M0-05, M0-10. Tests: U, I.

M1-01a/b are locally merged as prerequisites to the UI-first exception:
[storage contract](storage.md). Songs/revisions/schedule snapshots and a bounded
worker exist. Verified online backup and fresh-profile restore include live WAL
and actual process-abort tests. M1-06b adds schema-2 section IDs/arrangements and
transactional schema-1 upgrade gated by verified backup, including actual aborted
migration rollback. Themes/assets, destructive upgrades, recovery UI and power-loss
qualification remain open. No physical-output gate is waived.

- [ ] Define songs, revisions, arrangements, themes, schedules and asset references;
  document stable IDs and transaction boundaries; media bytes stay outside SQLite.
- [ ] Add versioned migrations, backups before destructive upgrade and recovery
  behavior for newer/unsupported schema versions.
- [x] Run database work on bounded background workers with cancellation rules.
- [x] Test fresh/old/corrupt database, locked database, disk full, failed migration
  and rollback using disposable fixtures.
- [x] Prove failed writes cannot leave half a committed schedule revision.

Acceptance: durable domain storage without blocking frame paths or losing the
original database on migration failure.

### M1-02 — Operator shell, pane geometry and layout modes

Depends on: M0-02, M0-03, M0-10. Tests: N, E if viable, H.

Early slice M1-02a is authorized by the sequencing exception above: separate-pane
empty shell, provisional geometry, splitters and resource tabs, now locally merged
and native-tested. M1-02b/c add contemporary chrome and official-image-directed
toolbar placement (D-UI-01), including New Song entry. Persisted layout,
other modes and installed-reference behavior remain parent follow-through.

- [x] Inspect relevant current GPUI split-pane/list/focus code and record references.
- [ ] Implement Schedule/Preview/Live/Resources positions and observed pane sizing,
  minimum sizes, splitters and persisted geometry with original light-mode assets.
- [ ] Implement observed separate/combined/contiguous modes; add child tickets if
  any mode needs a materially different data model, without dropping that mode.
- [ ] Handle resize, small laptop viewport, mixed DPI and restored/offscreen windows.
- [ ] Inspect screenshots of each layout, collapsed/minimum states and empty/error
  states; compare interaction sequences with the reference.

Acceptance: recognizable layout and behavior, not merely a similar screenshot.
No speculative redesign of control placement or default focus.

### M1-03 — Semantic actions, keyboard focus and selection rules

Depends on: M0-02, M1-02. Tests: U, N, E if viable.

M1-03a adds keyboard access to current shell controls only. Tab/Shift+Tab and
Enter/Space are explicitly provisional Sela accessibility policy, not installed
reference observations. Native input/focus/selection checks passed; text-entry,
modal scopes, multi-selection and live commands remain open.

- [ ] Map observed commands to GPUI actions/key contexts, not global key matching.
- [ ] Define focus traversal, text-input ownership, modal scope and selection
  anchors for single, range and multi-selection where observed.
- [ ] Implement next/previous, Go Live and safety bindings in the proper contexts.
- [ ] Test typing in search/editor, open modal, unfocused panes, repeat keys and
  boundary slides so navigation cannot accidentally fire while entering text.
- [ ] Verify keyboard-only access, visible focus and disabled/unavailable actions.

Acceptance: focus and selection match observed workflows, with no typing-triggered
live change and no shortcut-only access to critical controls.

### M1-04 — Resource browser, collection organization and search UI

Depends on: M1-01, M1-02, M1-03. Tests: U, I, N, P.

- [ ] Implement observed resource tabs, list/grid sizing, sorting, context actions,
  collections/folders and selection behavior; audit any missing browser controls.
- [ ] Add asynchronous, cancellable search with result generations to reject
  stale queries; implement empty/loading/error states without list selection drift.
- [ ] Virtualize large lists and bound thumbnail requests/cache ownership.
- [ ] Test a slow earlier query completing after a newer one, deletion of selected
  resource, large libraries and keyboard navigation through virtualized rows.
- [ ] Defer decoder-specific thumbnail production to M2-05, not browser behavior.

Acceptance: resource browsing remains responsive and cannot act on stale results.

### M1-05 — Song model and editor

Depends on: M1-01, M1-03. Tests: U, I, N.

M1-05a–d are **implemented-unqualified**: native fields, persistent song editing,
bounded chronological whole-document undo across section/metadata edits, and
documented-reference editor layout with receipt-gated OK. See
[use/check instructions](song-library.md). Native replay verifies save/undo,
structural undo, reopen and dirty-close retention against independent SQLite
bytes. Direct native-handler regressions verify pending operations reject late
commit/preedit before redraw and OK cannot close over a newer document snapshot.
Installed-reference workflow/field confirmation, Windows/IME/accessibility
and production qualification remain open.

M1-05e (EW Song Editor Words layout, EW8-OBS-021..026) is
**implemented-unqualified**: Title in the toolbar with EasyWorship's tool order
(unimplemented tools shown disabled), Words as one list of slides with inline
bold label and lyrics cells, label-kind group colors, Up/Down/Enter across
cells, Ctrl+Enter splitting into an unlabeled slide, Backspace joining an
unlabeled slide (provisional), `+` appending, the footer Apply/OK/Cancel, and a
real slide preview rendered off the UI thread with the audience text raster.
Native Windows replay: `scripts/song-editor-windows.py`. Open: cross-cell
selection (EW Ctrl+A selects the whole document), the Apply-to-schedule option,
IME/accessibility, and installed-reference confirmation of unverified details.
Follow-ups:

- M1-05f Slides tab: rendered thumbnails with label-colored caption bars and
  "Slide N" for unlabeled slides (EW8-OBS-024). **Implemented-unqualified**:
  preview raster box-filtered to 320×180 off the UI thread, text-keyed cache
  pruned to the current slides (≤129 × 225 KiB), narrowed pane, selection
  frame; native replay in `scripts/song-editor-windows.py`. Open: full-theme
  thumbnails (M1-05g/h), Slides keyboard navigation, drag reorder,
  installed-reference confirmation.
- M1-05g Song text formatting (font, auto/fixed size, color, B/I/U,
  shadow, outline, alignment). Observed in RUN-W06G (EW8-OBS-027–033): Format
  inspector Text › Style/Layout controls, theme defaults, and **per-slide**
  scope in 8.0.49 (an "apply to all slides" path was not observed). Owner
  decision 2026-10-06: per slide; Ctrl+A selects all slides so one change
  formats the whole song, and typing then replaces the whole text like EW
  (owner-reported, unobserved). **Active**, three slices:
  - [x] g1 per-slide `SlideFormat` model and schema 3 with a verified backup
    (implemented-unqualified, see [storage](storage.md)).
  - [x] g2 rendering core (g2a): installed-font catalog with bounds and LRU
    (`src/fonts.rs`, bundled DejaVu Sans + Bold), format-aware cue with
    1080-reference effect scaling, synth B/I margins and fixed-size
    refusal; transport text wire tag 2; audience fill/outline/shadow
    coverage layers (underline via decorations) blended in linear light on
    the GPU with an exact CPU twin; editor preview and thumbnails on the
    same path with `(text, format)` cache keys. **Implemented-unqualified**:
    native DX12 audience check passed in g2b below; negative-bearing ink
    clips at the area edge (EW parity unobserved, recorded in the work log).
  - [x] g2b operator/editor integration (M1-05g2, 2026-10-06): one shared
    font store (`fonts::shared()`), scanned off the UI thread by whichever
    window opens first; the operator resolves each previewed song's formats
    as one background job and gates Go Live/Next/Previous on it (the
    current scene stays, `Resolving fonts…`, retry on landing; a late scan
    re-resolves named-family items and refreshes the live cue); editor
    preview/thumbnails resolve the same way and the caption warns on
    bundled fallback; seed `--formatted-song`; native DX12 check extended and
    **PASS** (gold right/bottom, italic centered underlined, installed
    Arial; margins compared because auto-fit fills the area width, the
    gold slide uses a fixed size). Rendering is qualified for this
    machine; timing recorded in `docs/composition-text.md`.
  - [ ] g3 Song Editor Format pane (Text › Style subset), per-slide apply,
    Ctrl+A whole-song selection, undo, GPUI tests and native script.
  - Deferred (record as tickets when g3 lands): superscript/subscript, indent,
    bullets, margins, capitalization, word wrap, rotation, Center/Inner
    outline, Style tab (fill/border/reflection), Arrange.
- M1-05h Per-song background (color or Media image) saved with the revision.
  Planned.
- M1-05i Library right-click New Song…/Edit Song…/Delete in the operator
  (EW8-OBS-021). Planned.

- [ ] Implement title, authors, copyright/license identifiers, lyrics and labeled
  sections based on observed fields; keep identity separate from display title.
- [x] Support create/edit/duplicate/delete, validation, undo/redo and unsaved-close
  behavior without changing current live content.
- [x] Preserve intentional line breaks and Unicode; define empty-section behavior.
- [ ] Test duplicate titles, long lines, repeated sections, composed/decomposed
  Unicode, cancel versus save and deletion of a song used in a schedule.
- [x] Inspect actual editor, metadata, error and unsaved-change states.

Acceptance: editable songs persist losslessly and schedule snapshots survive
library changes. No licensed lyrics required for tests.

### M1-06 — Arrangements, pagination and text fitting

Depends on: M1-05, M0-07. Tests: U, R, N.

M1-06a/b [arrangement model and persistence](arrangement.md) are locally merged:
immutable exact-revision data, backed-up legacy migration and stable section/
variant/occurrence IDs. Editor save/load/duplicate/undo round trips preserve them;
native replay independently verifies section IDs in SQLite. Arrangement editing
controls, explicit reference repair, pagination and fitting remain open.
M1-06c (**implemented-unqualified**, [arrangement](arrangement.md#m1-06c--better-slide-output)):
output follows the first arrangement, text up to 288px with optional
normalized size, and Ctrl+Enter section split with undo.

- [x] Model ordered section occurrences and arrangement variants without duplicating
  the underlying song or losing occurrence identity.
- [ ] Implement reference-observed slide splitting, manual breaks, text fitting,
  section labels, slide ordering and overflow feedback.
  (Documented split, fit/normalize and arrangement order done in M1-06c;
  8.0.49 observation, manual breaks and overflow pagination open.)
- [ ] Keep layout deterministic for a declared font set, theme and output size.
- [ ] Test asymmetric arrangement V1/C/V2/C/C, deleted sections, very long verse,
  font fallback, right-to-left/combining text and output aspect changes.
- [ ] Verify editing/re-pagination while live cannot change acknowledged content.

Acceptance: correct content and ordering, readable output and explicit overflow;
do not silently truncate lyrics or hide missing sections.

### M1-07 — Indexed song search and large-library performance

Depends on: M1-01, M1-04, M1-05. Tests: U, I, P.

- [ ] Add indexed/FTS search for observed title/lyrics/metadata queries and filters.
- [ ] Define normalization, escaping, ranking and deterministic tie handling;
  keep display text unchanged.
- [ ] Maintain indexes transactionally on edits/deletes/imports and support rebuild.
- [ ] Test punctuation, diacritics, Indonesian text, duplicate titles, empty queries,
  corrupt index recovery and canceled requests.
- [ ] Benchmark a synthetic 20k-song corpus, separating database query latency
  from end-to-end rendered result latency; profile before optimizing.

Acceptance: correct ranked results and reproducible measurements against the
provisional 30 ms target/100 ms p95 ceiling, not an unqualified speed claim.

### M1-08 — Basic themes and background resolution

Depends on: M1-06, M1-01. Tests: U, R, N.

- [ ] Implement font, size, color, alignment, margins, shadow/outline, background
  color/image and copyright position where confirmed by the reference.
- [ ] Specify default/theme/item overrides and snapshot inheritance explicitly.
- [ ] Preview changes without applying them live; implement reset/cancel/undo.
- [ ] Validate missing fonts/assets, unsuitable aspect ratios and text contrast;
  make substitutions visible rather than claiming identical output.
- [ ] Render long/short text, landscape variants and override combinations.

Acceptance: theme edits are predictable, serializable and isolated from live;
advanced shape/layer tooling remains tracked in M4-01.

### M1-09 — Schedule editing and versioned content snapshots

Depends on: M1-04, M1-06, M1-08. Tests: U, I, N.

Progress: M1-09a (song items pinned to revisions, add/remove/reorder by buttons
and drag, Save/Open with Ctrl+S/Ctrl+O, unsaved guard, stable live item
identity) is **implemented-unqualified**; see
[operator shell](operator-shell.md#m1-09a--basic-schedule). Duplicate/copy,
multi-item move, autoscroll, theme/asset references and explicit library
refresh remain open.

- [ ] Add/remove/reorder/duplicate items and observed context/drag-drop actions,
  including correct insertion target, autoscroll and canceled drag.
- [ ] Persist song/theme versions and asset/font references in schedule snapshots.
- [ ] Make library refresh explicit with a reviewable difference and undo policy.
- [ ] Define stable selection/live identity when preceding items move or delete.
- [ ] Test duplicate occurrences, multi-item reorder, delete-live-item, canceled
  edit and library update while the original revision is live.

Acceptance: editing the service cannot silently retarget the current live cue;
reopening resolves the intended revision, not today's library contents.

### M1-10 — Preview/Live controls and acknowledged state

Depends on: M1-03, M1-09, M0-08. Tests: U, I, N, H.

Progress: M1-10b (Live output, Go Live, acknowledged Live) and M1-10c (Black/
Clear/Logo toggles, Ctrl+B/L/C, Page Down, Live single/double-click, Media
Images and picture logo) are **implemented-unqualified**; see
[operator shell](operator-shell.md#m1-10c--masks-and-picture-logo).

- [ ] Connect preview preparation, slide selection, Go Live, next/previous and
  safety buttons to domain commands rather than editing state directly.
  (Done for single songs, masks and selected schedule items (M1-09a);
  schedule-level next/previous from Live is open.)
- [ ] Show preparing/failed/disconnected/requested states separately from Live.
- [ ] Implement observed double-click, boundary navigation, auto-follow and
  contiguous/combined mode behavior using reference observations.
  (Preview/Live double-click and › stop at the end done; arrow keys,
  auto-follow and layout modes open.)
- [ ] Bound preview work and give audience rendering priority under overload.
- [ ] Test delayed/rejected apply, repeated Go Live, rapid navigation, live item
  removal and masked Go Live; inspect audience and operator state together.

Acceptance: Live reflects renderer acknowledgment; preview/selection never leaks
to audience and rapid input cannot apply an older scene last.

### M1-11 — Still-image assets and missing-resource preflight

Depends on: M1-01, M1-04, M1-08. Tests: U, I, R, N.

Note: M1-10c added only a minimal profile `Resources/Images/` folder (PNG/JPEG
copy-import on a bounded worker, 8 MiB cap, SHA-256 identity) to source the
logo. Nothing below is done by it; thumbnails, metadata and preflight remain.

- [ ] Import supported still-image formats on bounded workers; document supported
  dimensions, byte limits, orientation, color handling and animated-file policy.
- [ ] Store media outside SQLite with stable identity and managed/reference policy.
- [ ] Add original thumbnail pipeline and lazy loading without decoding in frames.
- [ ] Preflight a schedule for missing/corrupt files and fonts; provide locate/repair
  actions without silently replacing unrelated assets.
- [ ] Test moved files, same-name different bytes, decompression bombs, permission
  denied, metadata rotation and cancellation mid-import.

Acceptance: failed imports/preflight preserve current library and live scene.

### M1-12 — Autosave, revision recovery and local profiles

Depends on: M1-01, M1-09. Tests: U, I, N, P; S03.

- [ ] Define committed/dirty state, bounded autosave cadence, flush/close behavior,
  profile location and single-instance/concurrent-open policy.
- [ ] Commit schedule revisions transactionally without UI-thread filesystem work.
- [ ] Implement recovery prompt and backup rotation with documented retention.
- [ ] Inject termination before/during/after commit; recover the last committed
  revision and disclose newer unsaved work rather than claiming zero loss.
- [ ] Test disk full, locked/read-only profile, corrupt latest revision, exhausted
  backup budget and autosave while output is active.

Acceptance: S03 passes and live rendering shows no autosave-induced sustained hitch.

### M1-13 — Portable bundle, backup and restore

Depends on: M1-09, M1-11, M1-12. Tests: U, I, N; S04.

- [ ] Specify versioned manifest, snapshots, asset hashes, relative references,
  optional embedded assets and permitted font/content redistribution.
- [ ] Implement export/import/backup/restore with atomic publication and validation
  before mutation; explain missing/licensed assets and version incompatibility.
- [ ] Reject traversal, absolute paths, symlink escape, duplicate/conflicting names,
  oversized/expanding archives, checksum mismatches and malformed manifests.
- [ ] Test round-trip on a fresh offline profile and transfer between machines with
  different paths, fonts and case-sensitivity; test interrupted restore.
- [ ] Document format and recovery steps; never overwrite original data on failure.

Acceptance: S04 passes; portability does not imply permission to redistribute assets.

### M1-14 — OpenLyrics import

Depends on: M1-05, M1-06. Tests: U, I, N, L.

- [ ] Pin supported OpenLyrics versions/fields and consult authoritative format docs.
- [ ] Map sections, metadata, line breaks and arrangements with loss diagnostics.
- [ ] Disable external entity/network resolution; bound input size and nesting.
- [ ] Stage import with preview, duplicates policy and transaction/cancel behavior.
- [ ] Test malformed XML, unknown fields, repeated labels, Unicode and partial batch
  failure using redistributable fixtures; keep originals untouched.

Acceptance: supported data imports correctly and unsupported content is reported,
not silently discarded or guessed.

### M1-15 — Minimum viable Sunday rehearsal gate

Depends on: M1-07, M1-10, M1-11, M1-12, M1-13, M1-14, M1-16.
Tests: I, N, E if viable, H, P; S01–S04.

- [ ] Prepare a complete synthetic service with arrangements, image/color themes,
  saved snapshots, keyboard navigation and expected audience slides.
- [ ] Execute S01–S04, including failed preparation and safety controls under load.
- [ ] Run a 90-minute rehearsal on an older Windows integrated-GPU laptop with
  independent projector output while searching, editing and autosaving.
- [ ] Record frame/control timings, memory trend, recovery outcomes, manual visual
  observations and any automation limitations.
- [ ] Fix release-blocking failures and rerun affected cases; document remaining
  non-M1 features instead of claiming full EasyWorship parity.

Acceptance: a saved/reopened service is usable without internet, missing-resource
surprises or UI-workload stalls in the measured audience output.

### M1-16 — Early Windows installation, settings and accessibility basics

Depends on: M1-02, M1-03, M1-12. Tests: I, N, H.

M1-16a is a **partial developer-local Linux install/check slice**, not completion
of this Windows ticket or its dependency gates. Cargo local installation and the
installed native song editor are checked in the orb; Windows packaging, upgrade,
settings, accessibility and service rehearsal remain open. See README install
instructions and the work log for precise evidence and limits.

- [ ] Produce repeatable local Windows installer/package with required runtime
  dependencies and profile/data locations; no publishing/signing secret required.
- [ ] Add observed core settings and output selection, persistence, validation and
  cancel/reset behavior without exposing unsupported features as functional.
- [ ] Verify clean install, offline launch, standard-user permissions and safe
  uninstall that preserves user data unless explicitly requested otherwise.
- [ ] Label controls, expose focus/state where GPUI/platform supports it, and test
  keyboard-only operation and readable high-DPI sizing.
- [ ] Record screen-reader/native accessibility gaps for M5-03 rather than hiding them.

Acceptance: another Windows operator can install and rehearse M1 from instructions.

## M2 — Production media and bounded resource usage

### M2-01 — Decoder/backend and codec licensing decision

Depends on: M0-07, M1-15. Tests: I, H, P, L.

- [ ] Use the M0 interop findings to compare native/software decode options,
  codec/container coverage, GPU transfer paths and distribution obligations.
- [ ] Pin a supported format matrix, hardware/software fallback and failure policy.
- [ ] Test real Windows integrated-GPU decode, timestamp delivery and device loss.
- [ ] Define worker/process isolation and CPU/GPU frame/audio memory budgets.
- [ ] Record unsupported/proprietary formats and create feasibility follow-ups.

Acceptance: backend choice has hardware and licensing evidence, not just a sample
video playing once; no unsupported codec advertised.

### M2-02 — Bounded asynchronous video preparation and playback

Depends on: M2-01, M0-06. Tests: U, I, R, H.

- [ ] Implement open/probe/preload/decode/frame delivery outside UI/render paths.
- [ ] Bound packet/frame queues and decide drop/backpressure behavior by timestamps.
- [ ] Handle cancellation, end-of-stream, bad timestamps, corrupt media and decoder
  stalls without replacing a valid scene with partially prepared content.
- [ ] Recycle resources safely on cue switch; prevent stale frames after seek/restart.
- [ ] Test variable frame rate, multiple resolutions, short clips, unsupported codecs
  and failed first-frame preparation; inspect text overlay on playing video.

Acceptance: ready video is cue-addressable and failure leaves live stable.

### M2-03 — Media controls, looping and mask continuation

Depends on: M2-02, M1-10, M0-02. Tests: U, I, N, H.

- [ ] Implement observed play/pause/stop/seek, looping, start position and replay rules.
- [ ] Specify preview versus live transport ownership and behavior on Go Live.
- [ ] Complete M0-08 video cases and specify audio transport intent for
  Black/Clear/Logo continuation/restore; actual audio qualification belongs to M2-04.
- [ ] Test seek while loading, repeated toggles, loop boundary, end-of-stream under
  mask, rapid cue change and failed seek while previous scene is active.
- [ ] Inspect transport feedback against actual presented position, not only requests.

Acceptance: media controls match observations and cannot leak preview playback
into audience/audio unexpectedly.

### M2-04 — Audio clock, routing and synchronization

Depends on: M2-02, M2-03. Tests: U, I, H, P.

- [ ] Define master clock, drift correction, mute/volume/fade and output-device routing.
- [ ] Implement audio-only resources and observed playlist/background-audio behavior.
- [ ] Keep real-time callbacks free of blocking allocation/I/O and bound buffering.
- [ ] Test audio-device unplug, sample-rate changes, silence, drift, clipping policy,
  seek, pause/resume and masks with actual speakers/capture measurement.
- [ ] Document mute and failure defaults; separate operator preview audio if supported.

Acceptance: measured A/V synchronization and safe routing on declared devices;
silent/headless tests alone do not qualify audio.

### M2-05 — Thumbnails, preload scheduler and cache budgets

Depends on: M2-02, M1-04, M1-11. Tests: U, I, P.

- [ ] Share bounded preparation ownership where appropriate; prioritize live-ready
  cues over visible previews and background thumbnails.
- [ ] Define CPU/GPU/disk cache budgets, eviction, in-flight deduplication and
  generation-based invalidation when assets/themes change.
- [ ] Add cancelable next-cue preload without evicting assets used by live scenes.
- [ ] Test scrolling/import storms, oversized frames, budget exhaustion and stale
  thumbnails; verify both cache hits and invalidations, not only total memory.
- [ ] Profile allocation/copy hot spots and optimize measured bottlenecks only.

Acceptance: bounded steady-state resources under load with audience priority.

### M2-06 — Media library batch workflows and sequences

Depends on: M2-03, M2-04, M2-05. Tests: U, I, N.

- [ ] Finish observed media tabs, collections, batch import, rename/delete and
  sequence/playlist workflows with progress and cancellation.
- [ ] Add media metadata/unsupported-format diagnostics and relink actions.
- [ ] Preserve ordering and live ownership when a playing item moves or is deleted.
- [ ] Test mixed good/bad batches, duplicate names/content, interrupted import,
  next-item preparation failure and deleting referenced assets.
- [ ] Inspect loading, partial-success and empty-collection UI states.

Acceptance: partial batch failure is recoverable and cannot invalidate live resources.

### M2-07 — Live camera/capture input feasibility and implementation

Depends on: M2-01, M2-02. Tests: I, N, H, L.

- [ ] Audit reference feed setup and define supported capture APIs/devices, formats,
  latency, hotplug and permission behavior; avoid promising every capture card.
- [ ] Prototype bounded timestamped input, then implement prepare/apply integration.
- [ ] Handle disconnected/blocked device, changed format and stalled producer while
  preserving other output layers and exposing a clear recovery state.
- [ ] Validate at least one named physical device before marking its support done.
- [ ] Track Blackmagic/Magewell/AJA-specific paths separately when SDKs are needed;
  network video input remains gated by M3-08.

Acceptance: only tested capture paths are supported; simulators prove protocol
handling, not device interoperability.

### M2-08 — Media performance and failure qualification

Depends on: M2-03, M2-04, M2-05, M2-06, M2-07. Tests: H, P; S05.

- [ ] Run S05 with declared codec/resolution and named integrated-GPU hardware.
- [ ] Measure 1080p60 frame pacing, preloaded startup, safety-control latency,
  A/V drift, memory plateaus and CPU/GPU load during edit/import storms.
- [ ] Test decode stall, corrupt/missing media, resource pressure and device recovery.
- [ ] Optimize measured bottlenecks and compare before/after distributions with
  identical fixtures; retain correctness/failure tests through each optimization.
- [ ] Publish supported media matrix and unresolved driver/codec restrictions.

Acceptance: qualified media paths meet declared budgets; 4K60 remains a separately
measured configuration, not inferred from 1080p success.

## M3 — Production outputs, messages and timing

### M3-01 — Multi-output routing and per-output composition

Depends on: M2-08. Tests: U, I, R, N, H.

- [ ] Model audience, stage, alternate/stream and house outputs with stable identity,
  independent geometry/themes and explicit routing.
- [ ] Define which cue/control state is shared versus output-specific from observations.
- [ ] Bound per-output resources and specify failure isolation and synchronization.
- [ ] Test mixed aspect/DPI/refresh rates, one output failing, topology reorder and
  safety targeting; never silently map a stage feed onto audience.
- [ ] Inspect output configuration and representative rendered compositions.

Acceptance: shared content does not require identical appearance or leak private
stage information; each physical route is qualified explicitly.

### M3-02 — Stage/confidence view and performer information

Depends on: M3-01, M1-06. Tests: U, R, N, H.

- [ ] Audit observed current/next content, labels, notes, clock and stage layout controls.
- [ ] Implement stage-specific composition and safe-area/font/readability settings.
- [ ] Add tracked Sela requirements for chords/notes as explicit extensions where
  not established in the pinned reference, with stable arrangement association.
- [ ] Test last slide, repeated chorus, empty next item and rapid arrangement change.
- [ ] Verify stage-only information never enters audience/stream renders or bundles
  intended to exclude private notes; inspect at actual stage viewing distance.

Acceptance: stage content stays useful and private with deterministic next-item logic.

### M3-03 — Alternate/stream/house compositions

Depends on: M3-01, M1-08. Tests: U, R, N, H.

- [ ] Implement per-output themes, lower-thirds, safe areas and output enable/disable.
- [ ] Apply observed alternate-output layout and content overrides without changing
  the primary audience theme or playback state.
- [ ] Define fallback when an output-specific asset is missing or a sink is slow.
- [ ] Test long text at different aspect ratios, masked output independence and
  output-specific overrides while shared content advances.
- [ ] Inspect audience/full-frame, lower-third and house-display representative states.

Acceptance: all selected outputs show the intended shared cue with their own layout.

### M3-04 — Messages, nursery alerts and overlays

Depends on: M3-01, M0-08. Tests: U, I, R, N, H.

- [ ] Observe compose/show/hide, presets, recipients, priority, duration and mask
  interaction for message and nursery alerts.
- [ ] Implement bounded overlay commands with explicit output targeting and acknowledgment.
- [ ] Define interruption/dismissal/replacement rules and keep typing private until sent.
- [ ] Test simultaneous alerts, long text, rapid replacement, output loss, mask
  transitions and timeout/expiry while operator UI stalls.
- [ ] Verify private speaker alerts cannot appear on public outputs by default.

Acceptance: only intended recipients see an applied alert; stale alerts do not reappear.

### M3-05 — Clocks, countdowns and service timers

Depends on: M3-02, M3-04. Tests: U, R, N, H.

- [ ] Audit timer modes/start/pause/reset/expiry and output visibility against reference.
- [ ] Use monotonic elapsed time and explicit wall-clock/timezone conversion where needed.
- [ ] Implement countdown/count-up/time-of-day composition without frame-path polling I/O.
- [ ] Test pause/resume, midnight, wall-clock correction, sleep/wake, negative/expired
  durations and restart policy with fake clocks and actual suspension.
- [ ] Inspect audience/stage expiry and warning states without leaking stage-only timers.

Acceptance: timers do not drift merely because UI updates are delayed.

### M3-06 — Alpha/key-fill and streaming interoperability

Depends on: M3-03. Tests: R, H, P, L.

- [ ] Choose supported alpha output paths and document straight/premultiplied alpha,
  color-space, frame sync and SDK/license obligations.
- [ ] Implement only feasible sinks after a small interoperability spike.
- [ ] Test edge halos, shadows, transparent backgrounds and key/fill alignment over
  asymmetric checker/color footage, including disconnect and slow consumer.
- [ ] Verify named supported OBS/vMix/Wirecast paths and hardware key/fill separately.
- [ ] Record unsupported receivers/devices and latency/copy costs.

Acceptance: actual receiver/capture evidence, not a transparent PNG or SDK build alone.

### M3-07 — Multi-output production soak gate

Depends on: M3-02, M3-03, M3-04, M3-05, M3-06, M3-08.
Tests: H, P; S06.

- [ ] Run S06 and a 2–4 hour service loop with video, audio, messages, timers,
  preparation, library editing and each supported output path active.
- [ ] Measure per-output frame distributions, memory growth, drift and control latency.
- [ ] Exercise individual sink loss, reconnect, UI stall and device pressure.
- [ ] Fix cross-output starvation/leaks and rerun the failing scenario plus baseline.
- [ ] Publish the supported simultaneous-output/hardware matrix and limits.

Acceptance: production claims apply only to the measured output combinations.

### M3-08 — NDI input/output and HX feasibility

Depends on: M2-07, M3-01. Tests: I, H, P, L.

- [ ] Review current NDI/HX SDK terms, distribution, platform and codec requirements;
  separate input, output and HX capability decisions.
- [ ] Prototype discovery/connect/frame delivery/alpha where available and record
  end-to-end latency and memory/copy cost.
- [ ] Implement bounded networking off render paths with timeout/reconnect behavior.
- [ ] Test producer/receiver loss, slow network, changing format and permission/firewall
  failures with named real peers; retain live stability on disconnect.
- [ ] If blocked by rights or feasibility, leave the feature blocked with an explicit
  release scope decision rather than marking NDI parity achieved.

Acceptance: tested and legally distributable paths only; HX is not implied by NDI.

## M4 — Complete authoring, interchange and control coverage

M4 is a grouping, not a requirement to wait for all of M3: tickets can start when
their stated dependencies are ready. In particular scripture and presentation
authoring can proceed after M1 while media/output qualification is underway.

### M4-01 — Full presentation/theme editor

Depends on: M1-08, M1-09, M0-02. Tests: U, I, R, N.

- [ ] Inventory observed Format/Animate/Presentation tools, text boxes, images,
  shapes, layers, inspector fields, masters/defaults and theme inheritance.
- [ ] Implement object selection, move/resize, ordering, alignment and supported
  styling with undo/redo and keyboard access; preserve logical coordinates.
- [ ] Add presentation slide CRUD/reorder/duplicate and schedule/library workflows.
- [ ] Implement observed animation/transition controls using prepared runtime data;
  split large editor tools into child tickets with individual acceptance fixtures.
- [ ] Test multi-object edits, nested undo, deleted assets, cancel/save, overflow and
  editing a presentation already live; inspect each tool's non-default states.

Acceptance: each inventoried editor capability has implementation/test coverage
or an explicit outstanding gap. One text-box demo does not close this ticket.

### M4-02 — Announcement loops and timed presentation playback

Depends on: M4-01, M2-03, M3-05. Tests: U, I, N, H.

- [ ] Audit auto-advance, loop, per-slide duration, media completion and interruption.
- [ ] Implement timing through the ordered cue pipeline, independent of UI cadence.
- [ ] Define manual takeover/resume and schedule transitions using observed behavior.
- [ ] Test zero/long durations, missing slide media, loop boundary and operator
  override at the same instant as an automatic advance.
- [ ] Verify no orphaned timer can advance a later manually selected live item.

Acceptance: timed announcements remain predictable under edit/prepare load.

### M4-03 — Scripture providers and licensed local storage

Depends on: M1-01, M1-13. Tests: U, I, L.

- [ ] Define translation metadata, canon/book/verse identity, versification and
  provider capabilities without assuming all translations share verse numbering.
- [ ] Establish a lawful bundled test translation or synthetic corpus and a
  rights-aware import/provider mechanism; no scraped proprietary Bible databases.
- [ ] Support offline installed texts, integrity/version checks and index rebuild.
- [ ] Test missing books, differing versification, Unicode, corrupt provider files
  and restricted export/redistribution flags.
- [ ] Document translation availability and permissions independently from UI parity.

Acceptance: a functional offline provider path; "90+ translations" cannot be
claimed from a generic provider interface without licensed content.

### M4-04 — Scripture search, continuous browsing and presentation

Depends on: M4-03, M1-04, M1-08, M1-10. Tests: U, I, R, N.

- [ ] Observe reference/keyword search, book aliases/autocomplete, continuous verse
  scrolling, consecutive/nonconsecutive selection and direct Go Live.
- [ ] Implement reference parser with localized aliases and unambiguous range errors.
- [ ] Add translation selection, verse/reference labels, theme/pagination and
  schedule snapshots respecting content permissions.
- [ ] Test cross-chapter ranges, nonconsecutive verses, translation switches,
  mismatched canon, empty search and long passages with readable splitting.
- [ ] Inspect browser/editor/output, including malformed reference recovery.

Acceptance: selected verses appear in intended order with correct translation
and references; historical support documentation alone is not UX verification.

### M4-05 — Usage reporting and SongSelect/CCLI feasibility

Depends on: M1-05, M1-10, M1-14. Tests: U, I, N, L.

- [ ] Define local usage events from applied content rather than preview selections;
  specify deduplication, retry/restart and privacy/retention behavior.
- [ ] Implement local review/export reports using song/license metadata.
- [ ] Verify current provider API/access terms and reporting permissions before
  implementing optional SongSelect authentication, search and import.
- [ ] Keep credentials in platform secure storage; implement offline/token-expiry,
  cancellation, rate-limit and duplicate-import handling without blocking live.
- [ ] Test with mocks for failures and a permissioned real sandbox/account for
  integration; never claim automatic compliance from an unverified report format.

Acceptance: local reporting works offline; external integration is separately
permissioned and can remain blocked without disabling the core application.

### M4-06 — PDF/PowerPoint import and fidelity strategy

Depends on: M1-13, M4-01, M2-01. Tests: U, I, R, N, H, L.

- [ ] Evaluate native integration, conversion and rasterization for each format,
  including Office availability, animations, embedded media, fonts and licensing.
- [ ] Choose/document supported fidelity and platform matrix; never label a static
  slide conversion as full PowerPoint animation/media compatibility.
- [ ] Isolate untrusted document processing with bounded work, no macro execution
  or unattended external-link fetching; stage import before applying.
- [ ] Test malformed/password-protected/huge documents, missing fonts, mixed slide
  sizes, embedded media and interrupted conversion with original fixtures.
- [ ] Inspect resulting slides and expose unsupported-feature warnings before service.

Acceptance: supported paths are usable offline with honest fidelity diagnostics.

### M4-07 — Additional importers and migration from existing libraries

Depends on: M1-13, M1-14, M4-06. Tests: U, I, N, L.

- [ ] Audit needed EasyWorship/library/schedule and other interchange formats,
  public specifications, lawful access and available user-owned fixtures.
- [ ] Create one child ticket per feasible format/version, with supported field
  mapping, duplicate policy, preservation guarantees and unsupported data report.
- [ ] Implement read-only source access, staged preview and transactional import;
  do not bypass encryption/DRM or mutate the original installation's database.
- [ ] Test cross-version, malformed, partially missing assets and duplicate IDs;
  verify semantic content/arrangement/theme differences, not just item counts.
- [ ] Document blocked formats and provide permitted interchange alternatives.

Acceptance: each claimed importer has a versioned corpus and fidelity report;
the parent is not closed by one generic parser.

### M4-08 — Sandboxed webpage/online-video display

Depends on: M2-08, M3-01. Tests: I, N, H, L.

- [ ] Audit observed one-click web display; evaluate embedded browser process,
  compositor integration, media rights and supported platforms.
- [ ] Define allowed schemes, navigation/download/file/device permissions, cookies
  and profile isolation; no arbitrary page code in the main renderer process.
- [ ] Prepare/apply a web surface with explicit loading/offline/failure states and
  operator control over audio/focus/navigation.
- [ ] Test malicious navigation, popup/download, browser crash, infinite script,
  network loss and masked display without blocking local lyric presentation.
- [ ] State which sites/media are supported; DRM or service terms may block playback.

Acceptance: optional web content cannot compromise offline core availability;
failure has an explicit safe presentation behavior.

### M4-09 — Authenticated remote-control boundary

Depends on: M1-10, M0-06. Tests: U, I; S07.

- [ ] Define an opt-in local control protocol, versioning, explicit pairing and
  role-scoped commands; listen nowhere by default until enabled.
- [ ] Protect transport/session credentials, rate limits, revocation and replay;
  do not equate being on the local Wi-Fi with authorization.
- [ ] Route remote actions through the same ordering/safety model with acknowledged
  state and explicit local operator ownership/override behavior.
- [ ] Test unpaired/wrong-role/replayed/out-of-order requests, disconnect/reconnect,
  revoked clients and command floods; keep queues bounded.
- [ ] Document threat model, firewall/discovery behavior and private-state exposure.

Acceptance: S07 protocol checks pass; no second unsynchronized control path.

### M4-10 — Mobile remote operator experience

Depends on: M4-09, M3-02. Tests: I, N, E where supported, H.

- [ ] Choose mobile-web/native client scope after a pairing/control prototype;
  preserve core GPUI frontend and record reference mobile UX observations.
- [ ] Implement pair/unpair, schedule/slide navigation, current live feedback and
  role-appropriate stage/control views with clear disconnected state.
- [ ] Bound thumbnails/state updates and handle slow/reconnected clients without
  replaying stale commands or optimistic false-live indicators.
- [ ] Test touch, orientation, background/resume and accessibility on real iOS and
  Android devices; browser emulation is supplementary, not device qualification.
- [ ] Use browser E2E if the actual client is web; otherwise document native coverage.

Acceptance: permissioned remote use on named devices; pairing/revocation is usable
without exposing stage/private content to other clients.

### M4-11 — MIDI, synchronization, Stream Deck, Companion and presenter remotes

Depends on: M4-09, M1-03. Tests: U, I, N, H, L.

- [ ] Inventory observed MIDI send/receive/sync and controller command mapping;
  inspect public APIs/SDK licenses before adapter implementation.
- [ ] Create child tickets for MIDI, multi-instance sync, Stream Deck, Companion
  and keyboard-like remotes with declared supported versions/devices.
- [ ] Translate inputs into semantic actions and acknowledged feedback; add explicit
  arming/mapping settings, reconnect rules, rate limiting and loop prevention.
- [ ] Test simultaneous local/remote commands, duplicate messages, feedback loops,
  disconnected devices and sync peers at different schedule revisions.
- [ ] Qualify actual hardware and named integrations; measure sync skew rather than
  promising perfectly simultaneous output from protocol messages alone.

Acceptance: each advertised controller path has input and failure evidence;
unimplemented adapters remain open rather than closing the family ticket early.

### M4-12 — Provider ecosystem, hosted features and extension gate

Depends on: M1-13, M4-05, M4-09. Tests: I, N, L.

- [ ] Audit premium media browsing/download, online media designer and Planning
  import against the pinned build; separate hosted services from desktop behavior.
- [ ] Investigate permitted APIs/content rights and define equivalent local/import
  workflows where proprietary services cannot be integrated.
- [ ] Create implementation child tickets for approved integrations, including
  authentication, offline fallback, cancellation, checksums and rights metadata.
- [ ] Require user scope decisions for building any new hosted planning/designer
  service; do not silently expand the offline desktop project into a SaaS product.
- [ ] Assess plugin need only after core contracts stabilize; if approved, design
  sandboxed/versioned capabilities, never arbitrary in-process native libraries.

Acceptance: every discovered ecosystem feature has a permissioned implementation
path or an explicit blocked/deferred decision; this gate alone is not parity.

## M5 — Optimization, platform qualification and release

### M5-01 — Performance baselines and targeted optimization

Depends on: M1-15; repeat after M2-08 and M3-07. Tests: P, H.

- [ ] Specify baseline machines, OS/drivers, release build settings, cold/warm
  definitions, dataset, sample count and percentile methodology.
- [ ] Measure startup, idle CPU/CPU RAM/GPU memory, 20k search, prepare/accept/apply/
  visible Go Live, video start and per-output frame times independently.
- [ ] Compare provisional targets: cold 700 ms (1.5 s ceiling), warm 350 ms (800 ms),
  idle RAM 150 MB (250 MB), search 30 ms (100 ms p95), non-video Go Live 33 ms
  (100 ms p95), preloaded video 100 ms (300 ms), 60 Hz frame interval 16.67 ms.
- [ ] Clarify percentile and observation boundary for every target before gating;
  frame submission timestamps alone cannot establish visible Go Live latency.
- [ ] Profile real bottlenecks, optimize one coherent change at a time, and compare
  identical workloads; preserve rendering, ordering and failure semantics.
- [ ] Add stable regression checks with tolerances to avoid flaky microbench gates;
  publish 1080p60 and any 4K60 claims separately by hardware/output count.

Acceptance: evidence-backed budgets and improvements, no hard-coded benchmark
shortcuts, unbounded caches, or skipped work masquerading as optimization.

### M5-02 — Data safety, fuzzing and dependency/security audit

Depends on: M1-13, M4-06, M4-07, M4-08, M4-09. Tests: U, I, L.

- [ ] Threat-model archives, documents, media, network clients, browser surfaces,
  provider credentials, diagnostics and update/install paths.
- [ ] Fuzz real parsers and state-machine sequences with useful seeded corpora,
  resource limits and invariants; retain minimized regression cases.
- [ ] Audit dependency advisories/licenses, unsafe boundaries, secret storage and
  export permissions; remove unused capabilities and unresolved high-risk paths.
- [ ] Test interruption/disk-full/corruption across each durable write boundary,
  including imports, migrations, backup restore and cache publication.
- [ ] Document findings, fixes and explicitly accepted residual risks.

Acceptance: meaningful malformed/adversarial inputs exercise behavior beyond
input rejection; no high-risk unresolved issue hidden by green happy-path tests.

### M5-03 — Accessibility, localization and layout qualification

Depends on: M1-16, M4-01, M4-04. Tests: U, R, N, H.

- [ ] Audit all panes/dialogs/editors for keyboard-only operation, focus order,
  accessible names/roles/states and screen-reader behavior on Windows.
- [ ] Verify high DPI, large text, contrast, reduced motion and small-screen layouts;
  classify GPUI/platform limitations and implement feasible remedies.
- [ ] Externalize operator strings; confirm the initial language set (proposed:
  Indonesian/English) with plural/date/number handling and fallback.
- [ ] Test long translated labels, Unicode input/IME, font fallback and RTL content;
  ensure localization does not alter semantic action bindings unexpectedly.
- [ ] Inspect each affected non-default UI state and rerun reference workflow checks.

Acceptance: published accessibility/language coverage is based on actual native
checks, not only the presence of labels or translation files.

### M5-04 — macOS and Linux portability qualification

Depends on: M3-07, M5-01. Tests: U, I, R, N, H, L.

- [ ] Audit platform seams for display ownership, decode/audio, font resolution,
  filesystem paths/case, shortcuts, accessibility and installer/runtime dependencies.
- [ ] Implement and qualify macOS Apple Silicon first, then Linux Wayland/X11;
  keep Windows first-class and use child tickets per platform/backend.
- [ ] Test saved bundles across platforms, mixed DPI, fullscreen, hotplug,
  sleep/resume, permission prompts, media fallback and multi-output recovery.
- [ ] Record platform-specific unsupported integrations instead of claiming identical
  SDK availability; add build/native test coverage where runners are available.
- [ ] Publish platform hardware/driver matrices; do not infer macOS success from Linux.

Acceptance: each advertised OS has native evidence. An earlier Windows-only
release is possible only with an explicit release-scope decision.

### M5-05 — Diagnostics, crash reports and support workflow

Depends on: M0-09, M1-12, M3-07. Tests: U, I, N, H.

- [ ] Add bounded logs and diagnostics UI for adapters, outputs, decoders, frame
  timing, queues/cache memory and recovery failures.
- [ ] Create opt-in support export with preview/redaction; omit lyrics, private
  notes, paths identifying users and secrets by default; no mandatory telemetry.
- [ ] Define crash artifacts and detect/recover unclean shutdown without reapplying
  an unintended cue or leaking content into reports.
- [ ] Test log rotation, export failure, disk full and redaction using planted
  synthetic secrets/content; verify excluded bytes are actually absent.
- [ ] Document troubleshooting and how to collect hardware qualification evidence.

Acceptance: actionable support evidence without content disclosure or frame stalls.

### M5-06 — Final parity audit and volunteer usability rehearsal

Depends on: M0-02, M3-07, M4-02, M4-04, M4-05, M4-07, M4-08,
M4-10, M4-11, M4-12, M5-03. Tests: N, H, L.

- [ ] Walk every reference observation and feature-family entry; link implemented
  behavior to its tests or record exact remaining difference and owner ticket.
- [ ] Recheck pane geometry, all layout modes, context menus, focus, selection,
  drag/drop, editor tools, media, safety controls and output targeting.
- [ ] Have representative volunteer operators prepare/run/recover a service from
  written tasks; record mistakes and surprising behavior, not only completion time.
- [ ] Fix workflow blockers and rerun the affected sequences; retain original assets
  and branding instead of copying EasyWorship proprietary material.
- [ ] Obtain explicit decisions for inaccessible/licensed/provider gaps; publish the
  compatibility matrix without claiming unrestricted or complete parity.

Acceptance: comprehensive audit completed and gaps visible; approval of a known
gap changes release scope, not the factual implementation status of that feature.

### M5-07 — Install, upgrade, signing and release artifacts

Depends on: M1-16, M5-02, M5-04. Tests: I, N, H, L; S08.

- [ ] Produce reproducible versioned packages for the declared release platforms,
  checksums, dependency notices and AGPL source/build availability.
- [ ] Test clean standard-user install, offline launch, upgrade from supported
  versions, rollback/failure recovery, reinstall and safe uninstall via S08.
- [ ] Establish signing/notarization and secure credentials for applicable platforms;
  never commit credentials or depend on them for local debug builds.
- [ ] Specify update notification/download/verification/rollback if updates are
  included; updates must not interrupt a live service or require online startup.
- [ ] Stage artifacts and release checklist locally; ask before publication,
  infrastructure changes, workflow dispatch that changes shared state or deployment.

Acceptance: installation and upgrade are reproducible; locally staged or unsigned
artifacts are not described as published, signed releases.

### M5-08 — Operator documentation and recovery runbooks

Depends on: M1-15, M3-07, M5-05, M5-06. Tests: N, H.

- [ ] Document first run, display setup, library/import, schedule, themes, safety
  controls, keyboard actions, media/audio, stage/stream and backup transfer.
- [ ] Provide projector-loss, failed media, disk-full, corrupt-profile, unexpected
  shutdown and lost-remote recovery instructions with audience-safe steps.
- [ ] Explain rights/licensing and supported formats/providers/platforms accurately.
- [ ] Include readable original screenshots and synthetic tutorial service fixtures.
- [ ] Have a second operator execute install/rehearsal/recovery using only the docs;
  correct instructions that depend on unstated knowledge.

Acceptance: documented tasks are executable by an operator, not just API descriptions.

### M5-09 — Final release-candidate qualification and publication gate

Depends on: M5-01, M5-02, M5-03, M5-04, M5-05, M5-06, M5-07, M5-08.
Tests: U, I, R, N, E where viable, H, P; S01–S08.

- [ ] Freeze declared release scope/support matrix and dependency versions;
  explicitly list blocked/deferred features and any approved narrower milestone.
- [ ] Run full automated suites, available native E2E, manual reference workflows,
  clean install/upgrade and failure-injection scenarios on the release candidate.
- [ ] Run 2–4 hour soak per supported critical hardware/output combination;
  measure frame timing, memory, A/V synchronization and safety response.
- [ ] Triage every failure/flaky check, fix blockers and rerun affected/full gates
  as needed; no blanket skips to manufacture a passing release.
- [ ] Prepare release notes, known limitations, checksums, source/notices and rollback
  instructions; record candidate commit and qualified artifact identities.
- [ ] Obtain explicit authorization before publishing; after publication verify
  downloadable artifacts and clean install match the qualified candidate.

Acceptance: a traceable qualified release, with actual delivery state recorded.
"Done locally", "published" and "installed successfully" are distinct outcomes.

## Completion means evidence, not a checkbox sweep

The first useful implementation slice is M0-01. The first architectural go/no-go
is M0-10. The first usable service gate is M1-15. Comprehensive functionality,
optimization, platform support and release readiness continue through M5-09.
No ticket can turn unknown reference behavior, unavailable hardware or restricted
provider access into a passing result merely by documenting that limitation.
