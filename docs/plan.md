# Sela engineering baseline

This is a condensed implementation baseline from the product discussion, not a
verbatim copy of the original master plan. Future scope remains a product goal;
the work log records implemented slices and outstanding qualification.

The [implementation backlog](backlog.md) expands this baseline into executable
tickets, compatibility coverage, test boundaries, and release gates. The
[work log](work-log.md) records partial progress and where to resume. Keep this
baseline's architectural constraints when refining tickets; newly discovered
EasyWorship workflows must be tracked rather than silently omitted.

## Fixed product decisions

- Rust application, GPUI frontend, dedicated GPU live compositor.
- EasyWorship 8 layout and behavior parity is the target, not just inspiration.
- Modern light-mode styling with original assets; preserve familiar control
  placement and interaction rather than redesigning the workflow.
- Visual direction: clean contemporary Codex/T3code/Notion/Zed restraint, not
  EasyWorship's dated skin. Keep the 1:1 layout/behavior target. Implement and
  review actual GPUI screens; the owner explicitly rejected further Painter use.
- Windows first, followed by macOS (Apple Silicon first) and Linux (Wayland/X11).
- Offline-first, no mandatory cloud account, AGPL-3.0-or-later.
- Audience, stage/confidence, and stream outputs are long-term requirements.

## First gate: prove live output independence

Before investing in the full workspace UI, prototype a GPUI operator window and
separately paced audience output. Prove continuous output under deliberate UI
stalls, text composition over video, mixed-DPI behavior, display hotplug, and
surface recovery on representative Windows hardware.

Renderer backend, native window ownership, decoder interoperability, and process
isolation remain engineering decisions. GPUI's own output windows must not be
assumed to provide independent presentation timing. A render thread does not
survive process termination; a separate process still shares some system/GPU
failure modes. Test rendering continuity and control responsiveness separately.

**User-approved sequencing exception — 2026-10-04:** the owner authorized
operator UI implementation while physical renderer testing is unavailable.
Provisional M1 UI work may proceed before M0-10 closes. This does not qualify
output, approve unobserved reference behavior, or waive release/hardware checks.
Document reversible UI assumptions and leave dependent acceptance checks open.

## Compatibility evidence

Use EasyWorship 8 build 8.0.49 as the initial reference (official download listing
checked October 4, 2026). For each implemented workflow, record reference build,
observed action sequence, expected state, and a corresponding acceptance test.
Do not infer exact behavior from screenshots alone.

Cover pane geometry, selection versus live state, Go Live, next/previous slides,
keyboard focus, drag/drop, media controls, and Black/Clear/Logo interactions.
Explicitly resolve mask precedence, restoration, media/audio continuation,
output targeting, and Go Live while a mask is active. Unverified behavior stays
marked unverified. Proprietary integrations and licensed content require separate
technical and licensing feasibility checks; do not promise unrestricted parity.

## Runtime and storage contracts

- UI edits and selection do not mutate the current live scene.
- Immutable prepared cues cross a bounded renderer boundary. Define command
  ordering, stale-cue rejection, readiness timeouts, and renderer acknowledgments.
- Reserve capacity for safety controls; media preparation must not block them.
- The Live pane reflects acknowledged presentation state, not requested state.
- No file, database, network, or blocking decode work in the frame path.
- SQLite holds local library and schedule state, with migrations, transactions,
  recoverable backups, and full-text search. Media stays outside the database.
- Schedules retain versioned song/theme snapshots; updating from the library is
  explicit. Define asset and font resolution so reopening is predictable.
- Portable bundles use documented, versioned manifests and optional assets;
  validate archive paths, sizes, content, and redistribution permissions.
- Audience correctness takes priority over previews and thumbnails under load.

## Delivery milestones

1. **M0 — Output feasibility:** independent output, text/image composition,
   cut/fade, tested safety-control semantics, frame diagnostics, recovery strategy.
2. **M1 — Minimum viable Sunday:** songs, arrangements, search, themes,
   Schedule/Preview/Live/Resources, SQLite, autosave, OpenLyrics import,
   portable backup/export, missing-asset detection, keyboard/accessibility basics,
   and a repeatable Windows installation workflow.
3. **M2 — Media:** video backgrounds, audio, preloading, thumbnails, bounded
   caches, native hardware decode where verified, 1080p60 qualification.
4. **M3 — Production:** stage output, messages, timers, stream composition,
   multi-output soak tests; remote control and alpha/NDI follow feasibility checks.
5. **M4 — Interoperability:** additional importers, scripture providers,
   PDF/PowerPoint strategy, external controls, expanded bundle workflows.
6. **M5 — Release qualification:** signing/packaging, documentation, crash
   recovery, accessibility and localization qualification, hardware matrix.

M1 must support a saved/reopened service, independent projector lyrics over
image/color backgrounds, predictable safety controls, and recovery of committed
schedule revisions after forced termination. Run a 90-minute rehearsal on an
older Windows integrated-GPU laptop without UI workload stalling live rendering.

Later scope includes native announcement presentations, local/licensed scripture,
chords for stage output, usage reporting, house displays, remote pairing and roles,
external control integrations, and sandboxed web content. Plugins follow a stable
core and must not begin as arbitrary in-process dynamic libraries.

## Qualification and performance

Treat the original numeric budgets as provisional SLOs, not verified claims:
cold startup target 700 ms (ceiling 1.5 s), warm startup 350 ms (800 ms), idle RAM
150 MB (250 MB), indexed 20k-song search 30 ms (100 ms p95), non-video Go Live
33 ms (100 ms p95), preloaded video start 100 ms (300 ms), and 1080p60/4K60 output
within the 16.67 ms frame interval on declared supported hardware.

Before enforcing budgets, specify machines, load, percentiles, and measurement
boundaries. Separate command acceptance, frame submission, and visible output;
separate CPU memory and GPU allocations. Preserve the original goals of prompt
safety controls, no visible autosave hitch, and no sustained dropped frames.

Test pagination/shaping, arrangements, command state machines, migrations,
import/export, rendering, focus/actions, display hotplug, mixed DPI, device loss,
corrupt/missing media, decoder stalls, disk full, locked databases, failed
migrations, sleep/resume, and network failures. Qualify native hardware on each
supported OS and run 2–4 hour production soak tests as features mature.

Diagnostics should expose adapter, outputs, decoder, frame timings, dropped
frames, and cache memory. Support bundles omit lyrics and private content by
default. No mandatory telemetry. Remote access is disabled by default and must
use explicit pairing and role-scoped authorization when introduced.
