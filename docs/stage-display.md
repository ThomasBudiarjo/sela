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
- **When it changes.** The panel follows the operator's live request: it is
  rebuilt (string copies only) on each poll and sent when it differs from
  the one handed to the stage session, which happens when a slide cue is
  handed to the audience supervisor, when the stage (re)connects or changes
  extent, and when the message, clock/next options or the schedule
  neighbour change. A slide that fails to prepare changes neither output.
  The Live pane shows each output's own acknowledged state ("On screen: …"
  for the audience, "Stage W×H · showing …" for the stage).
- **Composition** (`src/audience/stage.rs`, stage preparation worker only):
  `stage::layout` places a header (amber current label left, white clock
  right), an optional amber message band, the current slide large in white,
  a dim divider, the sky-blue next label and the next slide (at most 0.6 of
  the current size). `stage::fit` estimates sizes from the face's advances;
  the raster steps down at most six times if shaping still overflows. The
  three colours ride the compositor's existing fill/outline/shadow coverage
  channels, so no new shader or GPU path exists. Bundled DejaVu Sans; a
  missing glyph rejects the cue (the previous panel stays), like slides.
- **Clock.** The stage process draws local time ("HH:MM", 24-hour, chrono
  `Local`) itself: a `stage-clock` worker checks every 250 ms and, when the
  minute changes, re-uploads the applied panel's coverage with the new time
  (one frame held, one result slot, retried when full). The event loop only
  swaps in a redraw of the panel that is still applied. The clock keeps
  running when the operator stalls or exits; it is not acknowledged to the
  operator (it is renderer-owned content, not an operator command).
- **Lifecycle.** Options › Foldback › "Enable support for Foldback Output"
  (default off, EW8-OBS-056) makes the stage start and stop with Live
  output. Live › Show Stage Output (Sela addition) stops or restarts only
  the stage; Live › Stage Message… opens a one-line dialog whose text is
  private until Show on Stage (Clear Message removes it, Cancel/Esc keeps the
  old one). Disabling Foldback stops only the stage; a new stage monitor
  restarts only the stage.
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
panel is at most 5 × 4 KiB of text plus the shared 0.7 MiB bundled font),
the renderer's one queued and one buffered preparation, the applied frame's
base coverage (W×H×4, ~8 MiB at 1080p), one clock redraw in flight and one
waiting. A stage process that exits, is killed, times out or sends garbage
becomes `Lost` in its own supervisor only; the audience session, its epoch,
its pending cues and its masks are untouched (GPUI test
`a_lost_stage_never_touches_the_audience_and_restarts_alone`, X11 step 6).
A lost audience leaves the stage running (`a_lost_audience_leaves_the_stage_running`),
but Go Live then refuses as before ("Output state unknown"): the stage keeps
its last panel until Live output is restarted. Stage GPU upload failure ends
the stage process only.

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
  the slide, blank next/message, missing glyph, long text).
- I/E (GPUI with fake renderer children that mirror the routing rules and log
  every scene): `src/tests/stage_display.rs`, nine tests.
- N (X11): `scripts/stage-display-x11.py --display :118` PASS; captures in
  `.amp/in/artifacts/stage-display-x11/`.
- Not run: `scripts/stage-display-windows.py` (no Windows host), physical
  stage monitor at viewing distance, mixed DPI/refresh, hotplug.

No new GPUI pattern was introduced (menus, modal dialog, TextInput and
AccessKit roles follow existing Sela code at zed `a846890`; the CheckBox and
RadioButton roles come from accesskit 0.24.1 through GPUI's `Role`).
