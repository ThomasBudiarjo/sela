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

## Gate / next action

GO for continuing independent-process feasibility work; **NO-GO for production
backend/full workspace qualification**. Windows physical dual-monitor mixed-DPI,
fullscreen, display hotplug, device/surface recovery, actual scanout timing,
text-over-video, real resource preparation and production command/lifecycle protocol are
unrun. Run this example on authorized Windows DX12 hardware (set backend dx12,
launch two modes under an owning supervisor), instrument stalls and exit with
physical capture, then extend M0-04 evidence. Do not mark parent done or M0-10
passed from Linux virtual GL evidence.
