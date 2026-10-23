# Stage (Foldback) output — M3-01/M3-02 slice

State: **implemented-unqualified** (stage-display stream, 2026-10-22). Linux
GPUI tests and a Linux X11 run pass; no Windows host, no physical stage
monitor and no EasyWorship 8.0.49 session were available. EasyWorship calls
this output **Foldback** (EW8-OBS-054/056); Sela's UI uses that name in
Options and "stage" elsewhere. Owner scope (plan.md, Mission 2026-10) keeps
only the stage clock from clocks/timers (M3-05).

## What exists

- **Second renderer process.** `sela --stage EPOCH_HEX MONITOR [BACKEND]`
  runs the same winit/wgpu renderer as `--audience` with `Config.stage`:
  borderless, non-activating, monitor-covering (or `window` for
  development), retained on controller loss. The operator owns a second
  `output::Supervisor` for it (`src/operator/stage.rs`), with its own fresh
  epoch, bounded slots (one cue in flight, one wanted), receipts and
  failure state. Nothing is shared between the two supervisors.
- **Routing.** Stage content travels only as a stage cue
  (`PreparedCue::for_stage`, wire kind 7 `STAGE`, below). Each renderer
  refuses the other kind before preparation (`audience::admits`): an
  audience process rejects any stage cue as RenderFailed and keeps its
  scene, and a stage process rejects slides and logos. Masks and the logo
  are sent only to the audience supervisor. The audience never receives the
  next slide, the stage message or stage labels.
- **Content** (`stage::StagePanel`, `stage::upcoming`): the current slide
  text, "title · slide name", the next slide of the same live item, else
  "Next item · <title>" of the following schedule entry (only while the live
  entry is still scheduled), else "End of schedule"; the optional stage
  message line; clock and next-area switches. Next is computed from the live
  item's own slide snapshot, so a repeated chorus is next exactly where the
  arrangement repeats it and an arrangement or song edit after Go Live
  changes nothing until the item goes live again.
- **When it changes.** The panel follows what was handed to the audience,
  not the newest request: it is built from `on_air` (the item whose slide
  cue was last given to the audience supervisor) and `live_slide`, both set
  only at that hand-over, never from `live` (which already names a newly
  chosen item while its cue prepares). It is rebuilt on each poll (slide
  strings copied, the face's `Arc` cloned) and sent when it or its face
  differs from the one handed to the stage session: when a slide cue is
  handed to the audience supervisor, when the stage (re)connects or changes
  extent, and when the message, clock/next options or the schedule
  neighbour change. A slide, or a newly chosen item, that fails to prepare
  or is refused by the audience changes neither output (GPUI test
  `a_new_item_that_fails_to_prepare_never_changes_the_stage`, which also
  covers the time while the new item's cue is still preparing).
  The Live pane shows each output's own acknowledged state ("On screen: …"
  for the audience, "Stage W×H · showing …" for the stage).
- **Composition** (`src/audience/stage.rs`, stage preparation worker only):
  `stage::layout` places a header (amber current label left, white clock
  right), an optional amber message band, the current slide large in white,
  a dim divider, the sky-blue next label and the next slide (at most 0.6 of
  the current size). `stage::fit` estimates sizes from the face's advances;
  the raster steps down at most six times if shaping still overflows. The
  three colours ride the compositor's existing fill/outline/shadow coverage
  channels, so no new shader or GPU path exists. **Face:** the live slide's
  resolved face (the same bytes the audience cue carries), bundled DejaVu
  Sans while nothing is live, so the stage covers the scripts the song's
  font covers. The current and next slides are drawn in that face. The
  chrome (header label, clock, "Next ·" label, stage message) is drawn in
  the bundled DejaVu Sans (`fonts::BUNDLED`, compiled into the stage
  process, never sent), and in the live face only when that face draws
  every character of the field and DejaVu Sans does not (a CJK title):
  `stage::chrome_face` (review round 2: a decorative, script, symbol or
  digit-less song face used to make the clock and labels unreadable).
  In the chosen face a character it lacks becomes U+FFFD (or `?`; invisible
  joiners and variation selectors are dropped) via `stage::covered`, and an
  optional field that still cannot be drawn is left blank. Only the current
  slide itself can reject a stage cue, and it is the text the audience
  already drew in this face, so a glyph in the next slide never holds the
  stage a slide behind. The next slide is drawn in the current slide's face
  even when its own slide uses another one (one face per stage cue).
- **Clock.** The stage process draws local time ("HH:MM", 24-hour, chrono
  `Local`) itself: a `stage-clock` worker checks every 250 ms and, when the
  minute changes, re-uploads the applied panel's coverage with the new time
  (one frame held, one result slot, retried when full). The event loop only
  swaps in a redraw of the panel that is still applied at the same extent
  (`redraw_applies`). A clock upload failure is reported to the event loop,
  which ends the stage process (never a silently frozen clock); the
  operator then shows the stage lost. `clock_step` takes the clock source
  and the upload as parameters, so `audience::tests` drive it with a fake
  clock (minute change, full slot retry, new frame, failure). The clock keeps
  running when the operator stalls or exits; it is not acknowledged to the
  operator (it is renderer-owned content, not an operator command).
- **Lifecycle.** Options › Foldback › "Enable support for Foldback Output"
  (default off, EW8-OBS-056) makes the stage start and stop with Live
  output. If Live output comes on before the stored settings or the monitor
  list have loaded, the start decision is taken again when they land. Live ›
  Show Stage Output (Sela addition) stops a running stage, starts a stopped
  one while Live output is on (it is inert while Live output is off), and
  restarts a lost one in a single activation, as the lost status says; it
  touches only the stage. Live › Stage Message… opens a one-line dialog whose
  text is private until Show on Stage (Clear Message removes it, Cancel/Esc
  keeps the old one). Disabling Foldback stops only the stage; a new stage
  monitor restarts only the stage; a new audience monitor restarts both
  outputs, and an Automatic stage then moves off the new audience monitor
  (GPUI test `options_monitor_changes_restart_the_right_outputs`).
- **Monitor choice** (`settings::stage_monitor_arg`): the saved stage
  monitor while connected, else the first monitor that is neither primary nor
  the audience's monitor. With no such monitor the stage is **not started**
  ("No free monitor for the stage output…"), so a stage feed is never mapped
  over the audience by default; choosing any monitor explicitly is honoured.
  `SELA_STAGE_MONITOR` (index, `secondary`, `window`, `name:DEVICE`)
  overrides the placement for development and the X11 harness.
- **Settings** (profile `settings.txt`, version 1 unchanged): `stage_enabled`,
  `stage_monitor`, `stage_clock=0`, `stage_next=0`, written only when not
  the default; older builds ignore them. No SQLite schema change.

## Wire: kind 7 STAGE (version 2)

Header and the first 61 body bytes are the v2 owned-resource command
(stamp, lane, budget, content version, extent), then the background (tag 0
plus opaque RGBA; an image background is refused for stage cues), then:

| Field | Bytes |
| --- | --- |
| flags (bit 0 clock, bit 1 next area; other bits invalid) | 1 |
| font version (id u128, revision u64) | 24 |
| face index | u32 |
| font bytes | u32 length + ≤ 8 MiB |
| current label, current, next label, next, message | each u32 length + ≤ 4096 bytes UTF-8 |

Trailing bytes, unknown flags, oversize fields, invalid UTF-8 and a panel
outside `StagePanel::is_valid` (more than 32 lines, a label or message with
a line break, a message over 160 characters) fail before a cue exists.
Content id `stage::STAGE_ID`, revision = panels sent in this operator run,
so a stage version never collides with a slide version.

## Bounds and failure isolation

Per stage session: the supervisor's one in-flight plus one wanted cue (the
panel is at most 5 × 4 KiB of text plus the live slide's face, at most
`MAX_SOURCE_BYTES` = 8 MiB, shared by `Arc` with the audience cue; 0.7 MiB
for the bundled face),
the renderer's one queued and one buffered preparation, the applied frame's
base coverage (W×H×4, ~8 MiB at 1080p), one clock redraw in flight and one
waiting. A stage process that exits, is killed, times out or sends garbage
becomes `Lost` in its own supervisor only; the audience session, its epoch,
its pending cues and its masks are untouched (GPUI test
`a_lost_stage_never_touches_the_audience_and_restarts_alone`, X11 step 6).
A lost audience leaves the stage running (`a_lost_audience_leaves_the_stage_running`),
but Go Live then refuses as before ("Output state unknown"): the stage keeps
its last panel until Live output is restarted. Stage GPU upload failure
(panel or clock redraw) ends the stage process only.

Known cost (measured, budgeted, not removed): like an audience cue, every
stage cue carries its face bytes on the wire. `Frame::command` encodes them
on the operator UI thread inside the supervisor, they are piped again (on
the supervisor's writer thread), and the stage renderer parses them per
cue, so each slide change or message change moves the face size (0.7 MiB
bundled, up to 8 MiB installed) through the operator thread and the stage
pipe. It is bounded by the supervisor's one in-flight plus one wanted cue.
Budget: the UI-thread encode of one stage cue stays at p95 <= 8 ms with an
8 MiB face (half a 60 Hz frame). Measured 2026-10-23 in this Linux
container (4 vCPU, dev profile, `sela` at opt-level 0), 50 encodes each:
`cargo run --locked -j 3 --example stage_frame_cost` reported bundled face
(0.72 MiB) median 0.02 ms / p95 0.03 ms / max 0.04 ms, 8 MiB face median
0.58 ms / p95 0.61 ms / max 0.67 ms; the example exits non-zero over
budget. Not measured: Windows hosts, a release build, and the pipe write
and renderer parse (off the UI thread). A per-session face cache on the
wire (send by version once) is the follow-up and applies to audience cues
as well.

## Decisions and best guesses (UNOBSERVED in EW 8.0.49)

Listed in [reference-observations.md](reference-observations.md#stage-output-foldback--unobserved-best-guesses-2026-10-22-stage-display)
and as PP-STG items in [parity-pending.md](parity-pending.md#stream-stage-display):
foldback content/layout/colours, 24-hour clock without seconds, masks do not
affect the stage, the stage follows Live output on/off, the Live menu
additions, the stage message, the no-free-monitor refusal, and only the
Foldback › General tab.

## Verification (2026-10-22, Linux container)

- U: `stage::tests` (next/last/repeated/empty-next, message bounds, clock
  text, layout bounds and non-overlap at five extents, fit),
  `settings::tests::stage_*`, `transport::tests::stage_panels_round_trip_as_their_own_kind`,
  `transport::tests::malformed_stage_frames_fail_before_becoming_cues`,
  `audience::tests::each_renderer_admits_only_its_own_cue_kind`,
  `audience::stage::tests` (colour channels per region, clock never touches
  the slide, blank next/message, uncovered glyphs drawn as stand-ins, an
  undrawable next area left blank, long text, chrome identical whatever the
  live face `chrome_stays_in_the_bundled_face_whatever_the_song_face`),
  `stage::tests::covered_*`,
  `stage::tests::chrome_prefers_the_bundled_face_unless_only_the_live_face_draws_it`,
  `audience::tests` clock worker tests (fake clock and upload; the event
  loop's `next_clock` turns a failed upload or a gone worker into ending the
  stage process, `a_clock_upload_failure_is_reported_never_a_silent_freeze`).
  The `fail()`/winit exit itself needs an `ActiveEventLoop` and is not unit
  tested; a stage process that exits is shown lost by the GPUI test
  `a_lost_stage_never_touches_the_audience_and_restarts_alone`.
- Performance: `examples/stage_frame_cost.rs` (UI-thread encode budget,
  see Bounds).
- I/E (GPUI with fake renderer children that mirror the routing rules and log
  every scene): `src/tests/stage_display.rs`, fourteen tests.
- N (X11): `scripts/stage-display-x11.py --display :118` PASS; captures in
  `.amp/in/artifacts/stage-display-x11/`.
- Not run: `scripts/stage-display-windows.py` (no Windows host), physical
  stage monitor at viewing distance, mixed DPI/refresh, hotplug.

No new GPUI pattern was introduced (menus, modal dialog, TextInput and
AccessKit roles follow existing Sela code at zed `a846890`; the CheckBox and
RadioButton roles come from accesskit 0.24.1 through GPUI's `Role`).
