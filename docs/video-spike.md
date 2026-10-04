# M0-07c: software video → offscreen GPU diagnostic

Implemented-unqualified; not a player, production backend choice, audio support,
hardware zero-copy, native presentation or frame-time qualification.

## Reproduce

```
export CARGO_TARGET_DIR=/home/user/workspace/repo/target
cargo test --locked --example video_spike -j 8
cargo clippy --locked --example video_spike -j 8 -- -D warnings
cargo build --locked --example video_spike -j 8
cargo fmt --all -- --check
runtime=$(mktemp -d)
XDG_RUNTIME_DIR="$runtime" "$CARGO_TARGET_DIR/debug/examples/video_spike" .amp/in/artifacts/video
rmdir "$runtime"
```

All passed on Debian 12 x64, FFmpeg 5.1.9-0+deb12u1, Vulkan llvmpipe
LLVM15.0.6 / Mesa22.3.6. Nine ordinary tests do not invoke FFmpeg or GPU;
Unix child tests use only trusted static `sh -c` commands. Windows lifecycle
tests and hardware qualification remain open. Initial strict Clippy rejected
constant `chunks_exact`; replaced with `as_chunks`, reran all commands.

CLI privately generates 12 original 320×180 RGBA frames at known 12 fps,
encodes losslessly as FFV1 BGRA in Matroska, then decodes on workers. No downloaded
or proprietary video. Every decoded byte is independently compared to the
deterministic original at its expected index: channel swaps, missing frames,
reorder, motion/color errors and truncation fail. Actual explicit DejaVuSans
text uses existing `composition/text.rs` in preflight and existing
`Compositor::render`/`check_readback`. Every uncovered output pixel is checked
within one quantization unit; opaque glyph pixels must be white. First/last PNGs
were inspected: readable “Video · Café”, green asymmetric left stripe, pink
rectangle moves from left to right. Output: `.amp/in/artifacts/video/frame-00.png`
and `frame-11.png` (ignored artifacts, not source fixtures).

## API, ownership and limits

`Decoder::start(path, width, height, count, deadline)` returns a polling handle;
`poll()` yields complete owned `Frame { index, rgba }`, `completion()` consumes
the single final result; `cancel()` and Drop only set an atomic flag. One child
per handle; no restart API. Source metadata/open and spawn/read/wait occur off
caller on supervisor/reader workers. Capacity-one queue plus one reader frame;
full queue uses cancellation-aware retry, never a blocking send. `saturated()`
is diagnostic evidence/gate, not a production metric. Completion is after
owned child exit/reaping and reader join. Drop initiates asynchronous cleanup,
not synchronous completion; kernel spawn/kill/wait latency is not a hard OS SLO.

Nonzero dimensions ≤1920×1080, 1–120 frames, regular source ≤32 MiB,
deadline >0 and ≤30 seconds. Decoder forces output size/rgba regardless of source
dimensions. Input protocol whitelist `file`, format Matroska, decoder FFV1,
explicit first video map, input/output `-threads 1`, `-filter_threads 1`,
`-nostdin`, `-xerror`, and frame limit. Commands use argument arrays, not shell
interpolation; stderr is discarded rather than buffered. Encoder is private
fixed-size setup only and its synchronous wait is outside UI/rendering; it has
no independent watchdog. Decoder read failures/EOF are bounded by supervisor
deadline if child hangs before exit. Cancel/deadline kill child to unblock read,
discard queue visibility, join reader and reap. Failure never updates the
demonstration's last accepted frame. Polling caller must reject stale generations
when integrating with real application lifecycle; this example has no live scene.

Tests cover one-byte partial reads, clean/mid-frame EOF, zero/overflow bounds,
sleeping-child deadline/cancel, asynchronous Drop/reaping, deterministic queue-full
gate cancellation. CLI asserts installed FFmpeg success, missing `Input`, corrupt
`Decoder`, saturated queue `Deadline`/`Cancelled`, no post-cancel frame, ordered
frames and retained last composed result. No external dependency in ordinary tests.

## Copies, synchronization and unresolved work

FFmpeg decoded CPU buffer → kernel stdout pipe → reader-owned Vec → capacity-one
queue (ownership move) → wgpu upload → source texture → composited texture →
staging readback → CPU PNG. GPU command submission/readback are synchronous
diagnostics outside any live UI. At most queued + reader + caller RGBA payloads
(3×8,294,400 bytes at max); GPU textures/staging, font masks, FFmpeg internal
allocations and kernel pipes are extra. Source metadata cap has TOCTOU limitations.
This is diagnostic process isolation, **not hostile-input memory sandboxing**.
Rawvideo has no original PTS; indices are monotonic, synthetic timestamps `i/12`
would apply only to this fixed-rate fixture. No real-time pacing or performance
SLO is measured. Native texture sharing/fences, hardware decoder surfaces,
color management, original timestamps/VFR, audio sync, Windows/DX12, device loss,
GPU budgets and physical display behavior remain unresolved.

## Sources and licensing

Reviewed official CLI/legal sources: https://ffmpeg.org/ffmpeg.html and
https://ffmpeg.org/legal.html (option scoping, rawvideo limitations, license).
Installed `ffmpeg -version`: 5.1.9-0+deb12u1, GCC12, `--enable-gpl`, no
`--enable-nonfree`. FFV1 BGRA support was checked using encoder help; exact
roundtrip is additional executed evidence. FFmpeg default LGPL2.1-or-later;
optional GPL components apply GPL to the whole build. External executable is
diagnostic only, not bundled and not a global production decision. Distribution
source/notices/build/options/license compatibility audit is **not waived**.
Original fixture/code belongs to this project; explicit font licensing remains
in `tests/fixtures` notices. Existing GPU/text dependencies and licenses unchanged.
No GPUI pattern or application UI copied; no dependency changes.
