# M0-06 bounded delivery model

State: **implemented-unqualified**, CPU contract slice. `delivery` is a
single-owner Rust state machine, not an IPC implementation. No live GPU renderer
or operator indicator is wired to it yet. Tests exercise both sides with an
injected submission callback; that is not audience-output E2E.

## Sequence, acknowledgment and uncertainty

The supervisor supplies a fresh, never-reused 128-bit epoch per renderer session.
One monotonically increasing sequence covers both lanes. Content revisions are
not sequences: deliberately presenting an older saved revision remains possible.
Only successful queue admission consumes a sequence. Sequence exhaustion requires
a new session instead of wrapping. Constructors do not generate/persist epochs;
that belongs to the eventual process supervisor.

- Submission records requested content, not live content.
- `Accepted` means the renderer queued the command. It does not mean upload,
  composition or presentation succeeded, and does not free admission capacity.
- `Applied` means the compositor callback successfully submitted the scene. It
  does not establish physical scanout. The controller advances its confirmed
  version only on an applied receipt, never backwards on a reordered receipt.
- `Rejected` means the command did not replace applied content, except an ambiguous
  stale receipt: that can describe a duplicate of previously applied work. Without
  newer confirmation the controller invalidates the session rather than guessing.
- Pending and currently applied duplicates return their original status without
  another presentation. Older sequences and foreign epochs cannot enter the
  renderer queue. A wrong-epoch receipt for a current command invalidates the
  controller session; receipts carrying an old epoch are simply ignored.

The renderer checks deadlines at admission and immediately before invoking the
submission callback. The callback must use prepared resources, avoid blocking
I/O, and leave actual live resources intact on failure. `now` measures the start
of that commit attempt; it is not the completion/scanout timestamp. Missing the
controller acknowledgment deadline means output is **unknown**, because the
renderer may have applied a frame whose receipt was lost. Timeout/disconnect
clears all queued requests, retains last-confirmed history for diagnostics only,
and disables submission. Fresh sessions start unknown and empty; no automatic
replay. A supervisor must also retire the old transport/renderer before reconnect.

## Capacity and ordering

There is one outstanding normal slot and one reserved safety slot, including
sent-but-unacknowledged work. Saturation returns `Busy` without eviction or an
unbounded retry queue. Both ends enforce lane limits. FIFO preserves order across
lanes: a safety command has at most one normal command ahead of it. The renderer
can therefore reach it on the next commit attempt after that command; this is a
count bound, not a measured wall-clock safety-latency guarantee.

The lane currently carries a prepared replacement snapshot. It establishes
capacity/ordering, **not Black/Clear/Logo semantics**. M0-02/M0-08 must determine
mask precedence, restoration, media continuation and Go Live under mask. Safety
resources must be pre-resolved outside the media worker. The future operator
coordinator must cancel/reject superseded preparation before submission; blindly
submitting a delayed result after a newer operator intent would give it a new
sequence and incorrectly make it current. No automatic Go Live or selection
policy is introduced by this model.

Moving a command transfers its `Arc`; pending controller metadata keeps no extra
payload copy. Two outstanding cues plus a prior applied snapshot can retain up
to 192 MiB of decoded payload at the M0-05 per-cue limit, excluding preparation,
GPU memory and external caller references. No cache, socket buffers or unbounded
ack queue are added. Transport sizing/GPU budgets remain integration work.

## Verification and remaining integration

`cargo test --locked --all-targets -j 8` exercises all 24 permutations of two
accepted/applied receipt pairs, both cross-lane orders, 1000 overload attempts per
lane order, duplicate/reordered commands, render failure, exact expiry, unknown
state, old epochs, disconnect and real-worker missing-resource recovery. Expected
revisions differ (for example 3 versus 23), exposing stale-state regressions.
Counters are saturating and diagnostics contain no cue text, paths or pixels.

Next integration: M0-07 layout/upload/composition, a bounded versioned IPC adapter
in the existing two-process spike, fresh-epoch supervision, preparation-intent
coordination and native receipt-to-frame checks. `Arc` and `Instant` are not wire
types; IPC must explicitly translate resource handles and deadline clocks. Run
real Windows/reference checks before closing the parent or M0-10 gate.

## M0-06b — Native static-color pipe integration

State: **implemented-unqualified**, bounded diagnostic subticket, based on local
main `2b62c77`. The CPU model above remains framework-independent. New
`transport` workers and `examples/native_cues.rs` connect it to a real separate
winit/wgpu audience process. No operator controls, GPUI layout, Painter,
Black/Clear/Logo policy, production transport or automatic replay is introduced.
This is deliberately the useful static-color fallback, **not text/image IPC**.

### Scoped checklist

- [x] Versioned, owned, allocation-bounded local pipe color protocol; capability
  readiness originates in the renderer after native surface/device setup.
- [x] Reconstruct validated immutable CPU snapshots; enforce renderer epochs,
  sequence ordering, independent lane capacity and FIFO across lanes.
- [x] Submit a real native GPU clear pass before Applied; keep prior scene on
  invalid resources, mismatched extent, stale commands or failed submission.
- [x] Worker-only pipe I/O, bounded inbound/outbound queues, nonblocking native
  polling/receipt enqueue, bounded startup and child lifetime, explicit retirement.
- [x] Real child exchange, saturation, rejection, disconnect and injected-clock
  timeout tests; native receipt-correlated capture/restart driver on own display.
- [x] Transfer font/text/image resources with per-field/total size validation,
  renderer-worker shaping/upload, GPU-ready ownership and failure tests. The
  existing offscreen composition/readback helper is **not** a live-frame API.
- [ ] Preparation-intent coordinator, production controller/supervisor, durable
  never-reused epoch registry, hard end-to-end expiration protocol, measured
  latency/memory budgets, asynchronous device-loss/recovery qualification.
- [ ] Windows/DX12, macOS/Metal, physical GPU/display/scanout and reference mask
  semantics. Parent M0-06/M0-07/M0-10 gates remain open.

### Wire schema and bounds

All fields are explicitly little-endian, with no Rust `Arc`, `Instant`, enum
layout, pointer, path or mutable file handle serialized. Eight-byte header:
`SCUE` (4 bytes), version `1` (u8), kind (u8), exact body length (u16).
Unknown version/kind/length, short header/body and oversized declarations fail
before allocating payload memory. Reader uses a fixed 73-byte array; it never
allocates according to an untrusted length. Version 1 supports only these kinds:

| Kind | Body bytes | Fields, in order |
| --- | --- | --- |
| 1 command | 65 | epoch u128, sequence u64, lane u8, remaining budget milliseconds u32, content ID u128, revision u64, width u32, height u32, opaque sRGB RGBA8 (4 bytes) |
| 2 receipt | 25 | epoch u128, sequence u64, outcome u8 |
| 3 Ready | 20 | renderer epoch u128, max texture dimension u32 |

Lane 0 is Cue; 1 is reserved Safety (replacement only, no mask semantics).
Receipt codes: 0 Accepted, 1 Applied, 2 Busy, 3 Disconnected, 4 TimedOut,
5 WrongEpoch, 6 Stale, 7 RenderFailed, 8 SequenceExhausted. Other codes fail.
Sequence zero, invalid lane/budget, zero/over-capability/over-pixel-budget extent
and non-opaque alpha cannot become a native cue. Structurally valid commands
whose resources fail validation consume their ordering identity and return a
rejection without changing prior live resources; invalid framing/direction
retires the pipe. A sender attempting text/images gets `Unsupported`, never a
silently simplified scene. If it already took a Delivery command, it must either
record a local RenderFailed receipt before sending anything or disconnect; it
must not leave that slot outstanding or retry automatically.

Budgets are 1–5000ms. A receiver-local timestamp captured by the pipe reader
includes inbound channel delay; it is metadata, not serialized. Sender retains
its original acknowledgment deadline. **OS pipe transit/writer queue time is not
a shared-clock expiration guarantee**: controller expiry means Unknown and the
owner must retire/reap the child, not infer that it cannot subsequently submit.
This conservative uncertainty is intentional; no wall-clock synchronization or
unsafe cross-process `Instant` conversion is claimed.

Two workers per endpoint: inbound capacity 2, outbound 4, plus one executing
frame each. All frame values are inline. Kernel pipe capacity, worker stacks,
Rust/channel overhead and GPU surface buffers are additional, not measured RSS.
Delivery still admits one normal and one safety command; workers are not an
alternative unbounded command-admission API. Native polls at most two incoming
frames/tick, performs at most one commit attempt/redraw and uses ~16.667ms pacing
without catch-up bursts. Full receipt queue or writer/reader failure exits the
native session rather than blocking, dropping Applied and claiming health.
No decode, raster, readback, GPU wait, pipe/file/log work occurs in the frame
callback. GPU initialization and adapter provenance logging happen only before
Ready. Surface configure/driver calls can still stall; no hard frame-time or
GPU completion claim is made.

Ready means device/surface configured, **not an applied startup scene**. Exact
output extent is checked again at submission. Applied follows command encoding,
`Queue::submit`, `pre_present_notify` and native `SurfaceTexture::present` calls;
it does not certify asynchronous GPU completion, compositor visibility or physical
scanout. The driver independently checks visible color afterward. Invalid cue
releases an acquired texture without presenting it; previous output remains.
Ordinary redraws and resize retention produce no new receipts. Surface fatal
errors/panics terminate the child; missing receipts mean Unknown. Device-loss
recovery is not implemented or qualified.

Child normally expires at 24s, with a 25s independent process-exit watchdog that
also bounds stuck startup calls. Driver startup is capped at 10s, each receipt
at 3s, each external capture command at 5s, retirement at 3s then owned-child
terminate/kill escalation. It reaps the old child and closes pipes before a new
epoch, starts unknown with no retained command, and sends a new explicit cue.
Random 128-bit initial epochs plus a distinct second epoch are diagnostic run
IDs, not a persistent production guarantee. Dropping PipeWorkers never joins
possibly stuck I/O; ownership requires retiring those pipes/processes, not
repeatedly constructing workers to bypass backpressure.

### Provenance, replay and executed checks

Consulted authoritative pinned source/rustdoc: winit **0.30.12**
`src/window.rs::pre_present_notify` and existing `examples/window.rs` lifecycle
provenance in [output-spike.md](output-spike.md); wgpu **29.0.4**
`src/api/{queue,surface_texture,render_pass}.rs` (`Queue::submit`, discard on
unpresented texture drop and surface status handling). Lockfile is unchanged.
winit Apache-2.0; wgpu MIT/Apache-2.0. No upstream application code/assets copied,
no new GPUI patterns, and existing supplied-font license notices remain intact.

```sh
export CARGO_TARGET_DIR=/home/user/workspace/repo/target
cargo test --locked --all-targets -j2
cargo clippy --locked --all-targets -j2 -- -D warnings
cargo fmt --all -- --check
cargo build --locked --example native_cues -j2
# Parent's reserved :99 can be used only once merges/native runs are serialized:
DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/native-cues.py \
  --backend gl --out .amp/in/artifacts/native-cues
git diff --check
```

Driver uses existing authorized X11 only, never starts a display or sends focus
events. Requires xdotool and ImageMagick `import`/`convert`; keep other windows
away from the child's capture region. Backend is explicit, with no fallback;
do not disable Vulkan when requesting Vulkan. `--binary` overrides target path.
The same audience can run natively on another platform, but this capture driver
is X11-specific, not Windows qualification.

Executed in this worktree on 2026-10-04: all-target tests **79 passed, one ignored
subprocess fixture** (that fixture is executed by three real-child tests), strict
all-target clippy, example build, rustfmt/Cargo fmt and diff checks. Six transport
unit tests exercise truncation/oversize/schema/resource rejection, round trips,
inbound queue expiry and 1000 nonblocking overload attempts. The child fixtures
never emit fake Applied: their compositor callback rejects; model success tests
remain explicitly model tests. Native driver passed twice on dedicated supervised
Xvfb **:101**, 1024×768, no WM/compositor, Mesa22.3.6 GL llvmpipe LLVM15.0.6 CPU
adapter. Shared :99 was not used. Final ignored artifacts:
`.amp/in/artifacts/native-cues-final/` (8 client-only PNGs, receipt/capture
timestamps and checked RGB values in summary.json, startup adapter logs).
Asymmetric red/green, invalid-alpha and extent-failure retention, old sequence,
current duplicate, retired epoch, unconfirmed restart and explicit fresh cue all
passed. These are virtual native capture checks, **not physical scanout or a
performance qualification**. Missing DISPLAY/binary, invalid backend and missing
audience arguments returned nonzero; idle standalone child cleanly expired near
24s. Hard watchdog's stuck-driver path was not fault-injected. No operator/native
focus regression rerun here; parent owns that serialized post-merge check.

Next bounded slice: explicit owned text/font/image wire schema with total budgets,
renderer preparation worker and nonblocking GPU-ready commit. Reuse composition
policies/tests but separate its blocking readback path, then extend this driver
to asymmetric text/image captures and failed-resource retention. Do not wire main
UI or mark complete feature parity from static-color success.

## M0-06c — Owned resource transport continuation

Version 1 remains byte-compatible for colors/receipts/Ready. Version 2 command
header is `SCUE`, u8 version=2, u8 kind=1, reserved u16=0, then u32 body length
(little endian). The first 61 body bytes match v1 command metadata. Background
tag 0 carries opaque RGBA4; tag 1 carries resource ID u128/revision u64, extent
u32/u32 and a u32-length normalized straight-alpha RGBA blob. Text tag 0 means
absent; tag 1 carries font ID/revision, font size u16, length-prefixed UTF-8,
then length-prefixed face-0 font bytes. No paths, mutable handles or hashes of
external files cross this boundary. Trailing bytes and unknown tags fail.

Body declarations outside 67..64MiB+160 fail before payload allocation. Per-field
limits remain 64KiB text, 8MiB font and 64MiB decoded image; aggregate owned
resources must fit 64MiB. `PreparedCue::from_owned` validates image length,
capabilities, font parsing and aggregate bounds, **not glyph/layout/GPU readiness**.
Frame construction and reconstruction are worker-only APIs for resource cues.
V2 receiver-local budget starts before reading length/body, so body transfer,
inbound queue and subsequent preparation delay cannot restart it. No shared-clock
or pipe-before-header deadline guarantee is claimed.

Pipe capacities remain 2 inbound/4 outbound plus executing frames, now each up
to 64MiB+172 wire bytes rather than inline 73 bytes. Thus a single endpoint can retain eight
maximal frames (~512MiB), excluding caller copies, kernel buffers and allocator
overhead. This explicit diagnostic ceiling is **not** a production RSS target.
Use Delivery's one-Cue/one-Safety admission; never use the pipe as a work backlog.
Native readiness and software captures are recorded separately below.

## M0-07d — Native prepared text/image submission

State: **implemented-unqualified**, standalone diagnostic only. The audience now
uses a single preparation worker with one queued frame and one buffered result.
Worker-only reconstruction/font parsing, explicit-font shaping/raster, allocation,
GPU upload and a two-second upload-completion wait precede native admission.
No resource file is opened by the audience. The pipe reader/writer remain separate
bounded workers. Saturated preparation retains one frame and stops pipe intake
until that frame can be enqueued, without changing its receiver timestamp or
consuming a newer ordering identity ahead of earlier preparation. Lane overload
is still rejected by RendererSession in preparation-completion order. No eviction
or unbounded retry backlog. Proper Delivery admission supplies at most
one Cue and one Safety; these are replacement lanes, not implemented mask policy.
Preparation is FIFO, not a preemptible real-time safety path.

Accepted now means GPU-ready resources entered RendererSession, not presentation.
The renderer deadline still uses the original reader timestamp: queue, shaping,
upload and completion-buffer delay are included. Expiry at admission or redraw
rejects TimedOut; malformed/glyph/layout failures reject RenderFailed. GPU upload
failure/timeout or worker disconnect retires the session (output Unknown), never
loops to accumulate more staging uploads. No automatic replay on restart.

At most two ready pending bindings, one applied binding, one buffered completion
and one executing upload are retained. Each background/mask pair is at most
64MiB+16MiB (~400MiB for five), excluding upload staging, driver allocations,
surface buffers and alignment. Native CPU ownership adds one queued resource
frame plus one retained backpressure frame (at most 64MiB+168 owned bytes extra),
one executing reconstruction (temporarily both frame and copied payload),
one completion and two pending/one applied cues to the pipe ceiling above.
Raster output plus inset mask can add 32MiB, font shaping copies/cache are bounded
by the diagnostic input limits but not measured RSS. No persistent cache exists.
These deliberately conservative ceilings require measured production tuning.

Redraw only encodes a pass using existing bindings, Queue::submit, native notify
and present. Applied follows those calls, not GPU completion/physical scanout.
No raster/decode/file/pipe/readback/upload wait occurs in redraw/about_to_wait.
Exact cue/surface extent must match. On resize the old bindings/scene remain
owned but are not sampled into an incompatible extent: new-extent preparation
is required; visible retention during resize is **not qualified**. Surface loss
and driver stalls still require supervision and physical qualification.

See composition-spike.md for policy, provenance and actual native evidence.
