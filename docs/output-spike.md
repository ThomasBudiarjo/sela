# M0-04 Linux audience process spike

State: **implemented-unqualified**, not a production backend decision. Original
diagnostic code only; no masks, composition, reference shortcuts or service UI.

## Boundary and provenance

One opt-in Cargo example has two modes. GPUI owns the operator UI/main thread;
`--audience` owns a separate winit event loop, native window and wgpu surface,
device and queue. A Python supervisor starts both independently, observes operator
exit, then explicitly closes audience and reaps both. No audience child is owned
by the operator, no remapping occurs, and audience has its own 25-second monotonic
deadline even if the supervisor disappears. This is not a deployment lifecycle.

GPUI provenance: `docs/gpui-bootstrap.md`, Zed
`a84689073d296dfd39987bc7dd478e43ef76d83a`, Linux shared event loop and Windows
shared UI draw coordinator. Additional GPUI window cannot provide independence;
render thread would isolate UI work but dies with operator process. Separate
process is chosen for this measurable experiment, not generic crash isolation.
System GPU/driver/compositor/display loss and machine failure remain shared.

Inspected authoritative winit **v0.30.12** `examples/window.rs`
([source](https://github.com/rust-windowing/winit/blob/v0.30.12/examples/window.rs)):
ApplicationHandler, resumed/create_window, resize, redraw, pre_present_notify.
Inspected exact downloaded wgpu **29.0.4** `src/api/{instance,adapter,surface,
surface_texture,render_pass}.rs` and wgpu-types `src/{instance,device}.rs`:
owned display handle required for GLES, request APIs, default surface config,
CurrentSurfaceTexture variants and present semantics. Rustdoc is bundled with
these pinned source files; lockfile fixes versions. winit is Apache-2.0, wgpu
MIT/Apache-2.0; framework GPUI Apache-2.0. No upstream application code/assets
copied; distribution/transitive notice audit still open.

Audience animates a yellow triangle's viewport from Instant elapsed seconds,
not accumulated frame count. WaitUntil targets ~16.667ms without catch-up bursts.
One command buffer/frame, default surface queue depth; no decoding/cache.
Telemetry uses a 2048-entry bounded try-send channel to a log writer thread;
under pressure logs drop rather than blocking renderer. Fixed diagnostic strings
are capped to 1KiB; approximately 2MiB worst-case queued payload
budget, no network telemetry. Normal exit flushes outside the frame/UI path.
Present-call log includes process ID, Unix microseconds for cross-process
correlation, and monotonic elapsed microseconds for interval distributions.
Cross-process correlation assumes no wall-clock jump in this short test.

Resize configures nonzero surfaces, zero-size skips drawing; timeout/occluded
skip, outdated reconfigures, suboptimal reconfigures, lost/validation exit with
explicit fatal diagnostic. Device/OOM/setup failures may terminate this spike;
no invented automatic recovery/remapping. Zero-size/error/device-loss branches
are not qualified by the successful resize check.

## Reproduce on the existing Linux graphical orb

For fresh-orb display provisioning, see [native setup](native-testing.md#reproduce).
Do not start/stop shared display services. Requires an existing X11 display,
Openbox or another EWMH window manager, xdotool and ImageMagick. The driver uses
private temporary runtime data, PID-scoped targeting and verifies focus before
every key. Run input tests serially. It respects DISPLAY/backend environment;
no fixed checkout path or implicit Vulkan override is embedded in the driver.

```sh
cargo build --locked --example output_spike -j 4
cargo test --locked --example output_spike -j 4
cargo clippy --locked --example output_spike -j 4 -- -D warnings
rustfmt --edition 2024 --check examples/output_spike.rs
DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/output-spike.py
git diff --check
```

Driver's default artifacts are `.amp/in/artifacts/output-spike/`: raw audience
and operator logs, summary.json, cropped window screenshots during stall, after
clean exit and after SIGKILL. Root captures are temporary; only owned window
interiors are retained. Keep other windows off these regions during capture.
Test process handles are tracked/reaped in finally; subprocess
commands have timeouts, startup retries bounded. Audience stops via native
Alt+F4, or its deadline; cleanup escalates terminate/kill only owned children.
Standalone controls Ctrl+1/2/3 and clickable labels deliberately stall actual
GPUI callback for 100/500/2000ms, Ctrl+Q/click exits operator; audience titlebar
close independently stops audience. These are diagnostic controls, not reference
compatibility semantics. No command protocol, acknowledgments or safe scene
ownership is implemented. Ctrl+P injects 2s of latency on a GPUI background worker,
with one retained task; repeated requests reject instead of queuing. The native
driver asserts a second request rejects, a Ctrl+1 handler completes before the
worker finishes, the UI observes completion and audience presentation continues.
This is controlled latency injection, not actual file/font/media preparation;
cancellation, decoding and stale-scene contracts remain M0-05/M0-06 work.

Backend explicitly selected by SELA_SPIKE_BACKEND=gl (default), vulkan or dx12;
no silent fallback. The command above disables Vulkan only for this GL run;
this Linux driver is not the Windows harness. Initial GL attempt
without owned display handle failed adapter/surface compatibility before opening;
corrected using wgpu InstanceDescriptor's documented display ownership.

## Executed evidence — 2026-10-04 UTC, debug build

Debian 12 x86_64 orb, Xvfb :99 1600x1000, Openbox/xcompmgr, Mesa 22.3.6
llvmpipe LLVM 15 CPU renderer, backend **Gl**, software virtual display. No named
physical GPU/monitor or scanout measurement. Bounded measurement run passed:

| Requested UI stall | Begin Unix µs | End Unix µs | Audience calls inside | Largest interior gap ms |
| --- | --- | --- | --- | --- |
| 100ms | 1791101981803150 | 1791101981903452 | 6 | 17.291 |
| 500ms | 1791101982234573 | 1791101982734739 | 30 | 17.343 |
| 2000ms | 1791101983066674 | 1791101985066859 | 118 | 39.527 |
| queued 100ms | 1791101985067400 | 1791101985167594 | 6 | 17.648 |

Queued Ctrl+1 during 2s stall did not run until 541µs after stall ended: output
continued, operator commands were **not responsive during stall**. This is not
safety-control latency qualification. Driver observed clean operator exit at
1791101987140664µs and second operator SIGKILL completion at
1791101989633907µs; audience logged 218 and 72 subsequent calls respectively.
621 total calls; monotonic gap p50 16.870ms, p95 19.793ms, max 47.308ms (nearest
index over 620 intervals). Not a 60Hz physical frame-pacing pass or performance
budget claim. Resize 640x360→600x320 and explicit audience close passed.

Root capture pairs were visually inspected: actual yellow geometry changes
position while GPUI callback is stalled and after operator exits. Crop root,
never capture application window directly. Initial inspected stall pair also
shows operator controls readable. Integration captures of the final 700x480
operator show complete controls/help without clipping. PNG motion plus
present timestamps supports virtual compositor continuity, **not physical
scanout**, GPU completion, end-to-end cue latency or every frame being visible.

Unit test: 10,000 motion samples bounded to [0,1] and time changes position.
Locked example build/test/clippy, rustfmt and diff check passed. No production
binary or shared native service changed. SIGKILL drops operator logger tail;
termination timestamps are supervisor observations, not speculative last frames.

Final source rerun after capping telemetry and deferring suboptimal configure
until acquired texture release also passed, in `output-spike-final/` artifacts:
619 calls, p50/p95/max 16.854/18.152/49.197ms; stalls had 6/30/117/6 calls,
exit/kill had 214/70 subsequent calls. Final 2s interval
1791102163032338–1791102165032505µs, max interior gap 44.260ms; queued action
began at 1791102165033012µs. Exit observed 1791102167158497µs, kill observed
1791102169589036µs. Both runs are virtual-display evidence only.

The integrated driver additionally rejects incomplete/out-of-order stall logs,
requires a clean operator exit, includes interval boundaries in the maximum-gap
calculation and rejects any gap ≥500ms inside the 2s stall. This discriminates a
long freeze from sustained progress; 500ms is a diagnostic threshold, **not** a
presentation SLO. Its snapshot/measurement overhead is part of the workload.

Integrated preparation run: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3
scripts/output-spike.py .amp/in/artifacts/output-preparation` passed. Preparation
began at 1791102759203072µs, rejected a duplicate at 1791102759234862µs, finished
at 1791102761203289µs and was observed by UI at 1791102761203706µs. The 100ms
operator callback ran at 1791102759900850–1791102760001012µs, entirely inside
preparation. Audience maximum gap during preparation was 41.877ms; 118 calls
occurred inside the 2s UI stall (40.161ms maximum boundary-inclusive gap).
898 total calls; p50/p95/max 16.828/17.757/46.039ms. Post-exit/kill counts 237/76.
Both preparing/completed controls and motion capture pairs were inspected.
Missing DISPLAY, missing binary and invalid X server returned nonzero without
success summary or temporary-runtime leaks. Whole-project build/test/clippy/fmt
and native bootstrap smoke passed after both worktree merges. CI now includes
example tests via `--all-targets`; remote CI itself has not been run.

## Windows DX12 physical-display evidence — 2026-10-06 (UTC+7), debug build

Host: Windows 11 Home, i7-14650HX, Intel UHD Graphics (32.0.101.6790) and RTX
4060 Laptop (32.0.15.9159). winit reported three physical monitors:

| Index | Physical size and origin | Scale | Refresh | Name |
| --- | --- | --- | --- | --- |
| 0 (primary) | 2560x1440 at 0,0 | 1.25 | 180 Hz | `\\.\DISPLAY5` |
| 1 (laptop panel) | 2560x1600 at -2560,-146 | 1.75 | 165 Hz | `\\.\DISPLAY1` |
| 2 | 1920x1080 at 2560,355 | 1.00 | 180 Hz | `\\.\DISPLAY6` |

**Caveat:** the owner was playing a game during the run. GPU contention and
any keyboard input are uncontrolled, so these are feasibility observations,
not timing qualification.

Opt-in audience environment (unset preserves the Linux behavior above):
`SELA_SPIKE_MONITOR=<index>` places the window on a specific monitor, with no
fallback; `SELA_SPIKE_FULLSCREEN=1` makes it borderless fullscreen there;
`SELA_SPIKE_SECONDS` sets the self-deadline, 25s by default, clamped to
1–600s; and wgpu's `WGPU_POWER_PREF=low|high` selects the adapter. The
audience logs the monitor list, its current monitor whenever that changes,
`scale_factor` events and the surface format, present mode and frame latency.

```powershell
cargo build --locked --example output_spike
python scripts/output-spike-windows.py .amp\in\artifacts\output-spike-windows
```

The supervisor (`scripts/output-spike-windows.py`, Win32 helpers in
`scripts/native_win.py`) first probes monitors. It places the DX12 audience on
the first non-primary monitor (here monitor 1 at 175%) and the GPUI operator on
the primary monitor, then repeats the Linux sequence with the same assertions.
It also adds the phases below. Result: **PASS** for all three phases.

- Main phase (RTX 4060, default preference): stalls of 100/100/500/2000/100ms
  had 5/6/30/118/6 audience present calls inside them, with a maximum
  boundary-inclusive gap of 17.97ms during the 2s stall. During the 2s worker
  preparation, the maximum gap was 18.05ms; duplicate preparation was rejected,
  and the operator's Ctrl+1 ran inside it. The queued action ran only after the
  2s stall ended, so operator controls are still not responsive during a stall.
  After a clean exit the audience made 224 more present calls, and 60 after
  `TerminateProcess` of a second operator. 949 calls; interval p50/p95/p99/max
  17.03/17.61/18.36/48.25ms.
- Mixed-DPI move: the driver moved the audience to the primary monitor (125%)
  and back (175%). winit emitted `scale_factor` 1.75→1.25→1.75, `monitor_current`
  tracked DISPLAY1→DISPLAY5→DISPLAY1, and the client went 1120x630→800x450→1120x630
  (640x360 logical at each scale). Maximum gap in the 1.5s after each move was
  17.73 and 18.10ms. A Win32 resize to 600x320 physical settled correctly.
- Borderless fullscreen on monitor 1: client exactly 2560x1600; during a 2s
  operator stall the maximum gap was 17.89ms (117 calls); p50/p99/max
  17.03/18.25/20.01ms over 365 calls; Alt+F4 exited 0.
- Adapter preference by monitor (6s windowed runs, first second excluded):
  `low` selected Intel UHD and `high` the RTX 4060 on all three monitors. Every
  cell had p50 ≈17.03ms and max ≤18.11ms. This run predates the switch from a
  Sela-specific variable to `WGPU_POWER_PREF`; re-run it with the current driver.

Inspected captures: `stall-a`/`stall-b` show the yellow triangle in different
positions during the 2s UI stall; `fullscreen-stall` fills the 2560x1600 panel;
`after-kill` still shows the triangle animating. Captures are GDI screen BitBlts
of the DWM-composed desktop, so they are not scanout evidence.

Findings that constrain the production renderer:

- wgpu's default surface config on DX12 picked **Mailbox** with frame latency 2.
  Presentation is paced by the 16.667ms software timer, not display refresh,
  so on 165/180Hz panels the cadence cannot match scanout and frames are
  repeated unevenly. Production needs an explicit present mode (Fifo or a
  measured alternative) and pacing derived from the target output's refresh.
- Measured intervals are present-call timestamps. DXGI frame statistics, actual
  scanout, presented-frame drops and photon latency were not measured.
- winit creates a visible, unowned 0x0 helper window per process. Win32
  automation must skip it; the first driver run targeted it by mistake.
- Cross-adapter presentation (Intel rendering to NVIDIA-attached displays or
  the reverse) worked without errors, but its copy cost is unmeasured.

Not run: physical display hotplug/unplug, sleep/resume, surface or device loss
(TDR), exclusive fullscreen, HDR, multi-hour soak, and the M0-02 reference
comparison. Sela never changes Windows display topology automatically; a
hotplug run needs the owner to physically unplug and replug a monitor while the
audience runs with a long `SELA_SPIKE_SECONDS`.

## Gate / next action

GO for the separate-process winit/wgpu audience boundary: on real Windows DX12
hardware, output continued through UI stalls, worker preparation, operator exit
and kill, mixed-DPI moves and borderless fullscreen, on both GPUs.
**NO-GO for production backend/full workspace qualification** until display
hotplug, device/surface loss recovery, explicit present-mode and refresh pacing,
scanout-level timing, and a soak on an idle machine are executed. Next: run the
hotplug case with the owner, add a DXGI-loss/TDR injection plan, and choose the
production present mode with measured evidence. M0-10 remains open.
