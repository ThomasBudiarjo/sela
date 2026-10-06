# M0-07 integrated static composition diagnostic

State: **implemented-unqualified**. Run this opt-in example to prepare immutable
image/font resources, rasterize the supplied font on a worker, upload the mask
and background to wgpu, compose, and validate/read back pixels. This is not the
live output loop or a service-authoring app. GPUI remains the operator frontend.

```sh
cargo test --locked --all-targets -j 8
cargo build --locked --example composition_spike -j 8
runtime=$(mktemp -d /tmp/sela-composition.XXXXXX)
XDG_RUNTIME_DIR="$runtime" target/debug/examples/composition_spike \
  .amp/in/artifacts/composition vulkan
status=$?
rmdir "$runtime"
test "$status" -eq 0
```

The explicit backend argument also accepts `dx12` and `metal`. Metal is
untested. There is no silent GL fallback. Adapter choice follows wgpu's
standard `WGPU_POWER_PREF=low|high` (unset uses wgpu's default).

Windows DX12, 2026-10-06, debug build, run from PowerShell:
`$env:WGPU_POWER_PREF='low'; target\debug\examples\composition_spike.exe <out> dx12`
passed every pixel, contain/cover and overflow check on **Intel UHD Graphics**,
and again with `high` on the **RTX 4060 Laptop** (driver 32.0.15.9159).
Single-run compose+readback ms (color/contain/cover): Intel 3.0/3.4/3.9, NVIDIA
1.3/11.7/8.0, with preparation+raster around 40ms in both runs. These are noisy
single samples taken while the owner was running a game, not performance
evidence. The inspected `image-contain.png` showed readable accented Latin and
connected Arabic and the correct sidebars. Set a private XDG directory on Linux to
avoid driver runtime warnings; do not force `VK_DRIVER_FILES=/dev/null` for this
Vulkan example. That workaround belongs only to the separate GPUI GL smoke.
Missing CLI arguments or an unknown backend return nonzero without creating the
output directory. A working GPU backend is required for this opt-in command;
ordinary unit tests do not request one.

## Declared fixture and pixel policy

- Original text, supplied DejaVu Sans with retained notices, 40-pixel font,
  52-pixel line height, 32-pixel margins; advanced Latin/combining/Arabic shaping.
  Explicit three-line content, no wrap, no hidden font fallback.
- Fixed 641×360 output; diagnostic logical pixels equal physical texture pixels
  at scale 1.0. This does not test monitor DPI. Width deliberately exercises GPU
  row padding. Actual font coverage is checked for opaque and antialiased ink.
- Color `[18,53,109,255]`; generated asymmetric 3×2 RGBA PNG for image fixtures,
  including one half-alpha cell. Input files are removed before composition,
  proving that preparation owns bytes rather than reopening paths.
- `color.png`: full-output background. `image-contain.png`: centered aspect-fit
  image, black sidebars. `image-cover.png`: centered crop filling output. Text
  uses the whole output rectangle independently of image fit.
- Nearest image sampling, sRGB input/output, linear-light alpha composition over
  opaque black. Per-byte color tolerance is ±1 where documented in GPU checks;
  white full-coverage and opaque output endpoints are exact. No universal font
  golden or cross-platform pixel-identity claim.
- Overflow is rejected before upload; the overflow case has no clipped "success"
  frame. Unit tests also cover missing glyph, invalid font and all overflow axes.

Detailed API, upstream/license provenance and limits:
[CPU text](composition-text.md), [GPU](composition-gpu.md),
[preparation](preparation.md), [delivery](delivery.md).

## Executed evidence — 2026-10-04 UTC

Debian 12 x64 orb, 16 virtual CPUs (Intel Xeon 2.60 GHz), Rust 1.98.1 debug build,
Vulkan llvmpipe LLVM15.0.6, Mesa22.3.6 CPU adapter. Headless offscreen rendering
succeeded despite the separate GPUI/Vulkan black-window finding.

- All-target tests: **27 passed** (18 library, 3 GPUI, 5 composition, 1 animation).
  Strict all-target clippy, formatting and all-target build passed.
- Integrated command passed repeatedly: `PASS: GPU pixel checks; actual-font
  color/contain/cover; overflow rejected`. Retained GPU checks cover asymmetric
  orientation, padding, crop/fit both axes, alpha, color conversion and recovery
  after invalid input. Actual font-mask opaque and uncovered pixels are checked.
- Inspected all three generated frames: readable title/Latin accents/connected
  Arabic, no glyph boxes, clipping or row corruption; contain has 50/51-pixel
  sidebars for odd output width, cover has none. These are GPU readback images,
  not screenshots of physical scanout or native operator interaction.
- Native regression: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  scripts/native-smoke.sh "$PWD/target/debug/sela"` passed PID-scoped focus,
  unbound key, resize, Ctrl+Q, WM close and exit/window removal.

One measured repeat used `/usr/bin/time -v` around the already-built binary with
the command above. Workload includes setup, tiny GPU assertion frames, three
641×360 readbacks and PNG writing. Preparation/raster: **42.226 ms**; color,
contain, cover allocation/upload/draw/readback calls: **5.271 / 77.773 / 12.595 ms**.
Total wall time **0.51 s**, maximum RSS **115,540 KiB**, no swaps. Prior run:
48.848 ms preparation and 67.254 / 51.730 / 3.587 ms composition/readback. These
variable software/debug samples are **not frame pacing, CPU-only render cost,
GPU-memory measurements, percentiles or production SLO qualification**.
Do not optimize a live compositor based on readback timings alone.

Review evidence is local ignored `.amp/in/artifacts/composition/`: three PNGs,
`run.log`, `resources.txt`. The executable recreates the fixture without private
assets. File writing, GPU waits and joined workers are allowed only because this
is an offscreen diagnostic with no UI/live frame loop.

## Remaining work

Integrate GPU-ready resources with the native audience process and M0-06 receipt
boundary; keep uploads, decoding, shaping and file work outside the live frame.
Settle video decoder/GPU copies and synchronization. Cut/fade/interruption and
Black/Clear/Logo require installed-reference observations; do not infer them from
these static frames. Golden-transition frames, device loss/recovery, actual
allocation budgets, Windows/DX12/mixed-DPI/physical display tests and M0-10 remain
open. A software PNG does not waive the plan's gate before the full M1 workspace.

## M0-07d — Native resource continuation (2026-10-04)

- [x] Share composition shader/pixel policy without invoking blocking readback
  from the audience loop: `from_device`, worker `prepare_native`, submit-only
  `submit_native`, renderer-owned immutable `ReadyComposition` bindings.
- [x] Receive actual owned UTF-8/font/asymmetric image, raster/upload outside the
  native callback, validate missing glyph/overflow/budgets before replacement.
- [x] Correlate native receipts with client-only captures and exact retained RGB
  digests after failed/stale/expired preparation; restart unknown without replay.
- [ ] Production intent coordinator, pre-resolved safety resources, resized-output
  preparation/retention, video/transition/device-loss qualification and measured
  workload/RSS/GPU/frame-pacing budgets. Windows and physical scanout remain open.

Policy is the existing original 32px inset, explicit supplied font, advanced
shaping, no-wrap, white coverage, 1.3× line-height, nearest image contain/black
sidebars/straight-alpha-over-black and linear sRGB composition. A color uses cover
on a 1×1 opaque image to fill the output. Native requires a supported sRGB surface.
The text diagnostic retains its stricter bounds: 2MiB font, 4096 UTF-8 bytes,
32 lines, 1–96px size (1–288px since M1-06c), raster edge <=4096 and <=16MiB alpha. CPU transport's
broader valid input can therefore be rejected at native layout; no silent shrink,
wrap, fallback or cropping. Images are already normalized RGBA, not decoded here.
No EasyWorship layout/mask/transition behavior is invented by this diagnostic.

Provenance: reused original repository `examples/composition/{text,gpu}.rs` policy
and shader. Inspected pinned wgpu **29.0.4** `src/api/queue.rs::{write_texture,
write_buffer,submit}`: writes stage until submit; worker flushes with `submit([])`
and `Device::poll(PollType::Wait)` capped at 2s before readiness. No mapped readback
or frame-side waits. wgpu MIT/Apache-2.0, winit 0.30.12 Apache-2.0, existing
cosmic-text/DejaVu notices retained; no upstream application code or GPUI patterns
copied. Dependency pins/manifest/lock are unchanged. Async upload failure retires
the worker/session instead of accumulating staging/retrying on an uncertain GPU.

Executed software-native diagnostic on owned supervised Xvfb **:102**, 1024×768,
no WM/compositor, GL Mesa22.3.6 llvmpipe LLVM15 CPU adapter; shared :99 untouched:

```sh
export CARGO_TARGET_DIR=/home/user/workspace/repo/target
# Cargo batches serialized with flock /tmp/sela-cargo-continuation.lock;
# touch src/*.rs and changed examples/tests inside lock before each batch.
cargo test --locked --all-targets -j4
cargo clippy --locked --all-targets -j4 -- -D warnings
cargo fmt --all -- --check
cargo build --locked --example native_cues -j4
uvx ruff check scripts/native-cues.py
DISPLAY=:102 VK_DRIVER_FILES=/dev/null python3 scripts/native-cues.py \
  --backend gl --out .amp/in/artifacts/native-resources-final
git diff --check
```

Native evidence includes readable “Signal café” / “Beacon” over six distinct
tiles. All six asymmetric sample points and >100 white coverage pixels are
independently checked. Missing glyph, overflow, corrupt font, oversized text,
old sequence and 1ms resource deadline each reject without changing the full
captured RGB digest. Retired resource epoch cannot replace new-session startup;
fresh explicit cue applies. The driver uses bounded nonblocking multi-write
resource packets (>PIPE_BUF), not an unbounded blocking supervisor write.
Captures/logs/receipt timestamps and RGB hashes: ignored
`.amp/in/artifacts/native-resources-final/` (15 PNGs plus summary/logs).
Inspected actual text/image and overflow-retention captures; no glyph boxes,
clipping or row corruption. They are virtual native screenshots, **not offscreen
readbacks, physical GPU/display evidence, scanout timestamps or parity proof**.
