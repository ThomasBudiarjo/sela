# M0-07b offscreen static GPU composition

State: **implemented-unqualified** subticket; no production backend decision,
transition/video implementation, physical display or EasyWorship parity claim.
Depends on local M0-05/M0-06 contracts, not an assumption that origin has them.
Only `examples/composition/gpu.rs` and documentation belong to this slice.
The permanent [composition example](composition-spike.md) now integrates the
explicit-font mask; the temporary example used below was removed before commit.

## API and pixel policy

`Compositor::new(wgpu::Backends) -> Result<Compositor, Box<dyn Error>>` requests
an offscreen adapter/device with no surface/display handle and no silent backend
fallback outside the caller's supplied flags. `adapter_info()` exposes provenance.
`render(size: sela::scene::Extent, background: Image<'_>, text_alpha: &[u8],
fit: Fit) -> Result<Vec<u8>, Box<dyn Error>>` returns tightly packed RGBA8 sRGB,
top-to-bottom rows, no padding, always alpha 255. `Image` has public `width`,
`height`, `rgba`; `Fit` is `Contain` or `Cover`. A 1x1 image is a solid color.
Use Cover for full-output color regardless of output aspect ratio.

Physical pixels only: top-left (0,0), fragment centers (x+0.5,y+0.5); no DPI or
logical-coordinate conversion here. The parent rasterizer must provide exactly
width*height alpha coverage bytes in the same coordinate space, with opaque white
glyph color. No font lookup, shaping, text positioning or image decode here.
Centered contain uses the smaller output/image axis scale; cover uses the larger.
Half-open rectangle inclusion decides bars at pixel centers; cover clips at
output edges. `textureLoad` with floored scaled coordinates implements nearest
sampling (no interpolation/sampler), deliberately suitable for asymmetric fixtures.
Text covers the whole output, including letterboxes, independent of image fit.

Background is straight-alpha sRGB RGBA8, uploaded as `Rgba8UnormSrgb`, so RGB
loads decode to linear light while alpha remains linear coverage. Transparent
image pixels composite over opaque black: linear `rgb * alpha`. Mask is
`R8Unorm`; white glyph coverage then computes `mix(background_linear, 1, mask)`.
Target is `Rgba8UnormSrgb`, encoding once at write. There is no fixed-function
blend (the shader does both over operations); no CPU image compositor and no
shader file read. Half white coverage over black is ~188 sRGB, not 128.
Hidden RGB under zero background alpha cannot leak. Clear/letterboxes are opaque
black; this is a composition policy, not reference Black/Clear/Logo semantics.

## Bounds and failure boundary

Both background and output require nonzero edges <=8192 and the actual requested
device's max 2D texture dimension. Each RGBA payload <=`MAX_SCENE_BYTES` (64MiB),
with u64 size arithmetic before conversion and exact input lengths; alpha must
match output pixels. Rejection happens before any upload/allocation. Padded
readback also checks max buffer size. Readback stride is ceil(width*4/256)*256;
rows strip padding even for 3- and 321-pixel widths. Queue texture uploads need
no 256-byte stride alignment per pinned wgpu docs.

Caller must serialize this diagnostic on a worker, not a GPUI callback or live
frame path. One call allocates background/mask/target, 16-byte rectangle uniform,
one padded readback buffer, and one tight result; no cache or persistent work
queue. These are **per-resource** 64MiB limits, not a total 64MiB memory cap:
background <=64MiB, mask <=16MiB, target <=64MiB, readback <=66MiB (conservative
padding bound), result <=64MiB, plus caller inputs and driver upload/staging.
No measured RSS/allocation budget claim. No concurrent admission framework added.

Exactly one full-screen triangle/draw, copy, submission per call. Native wgpu
`Device::poll(Wait { submission_index, timeout: Some(10s) })` and callback receive
timeout 1s bound the explicit completion/readback waits. Setup/request-device,
driver allocation and submission are not guaranteed bounded by that poll timeout.
Mapping/adapter/device/poll errors return errors; input rejection can be followed
by a valid render. wgpu validation/OOM/internal errors can still invoke its default
uncaptured-error behavior; forced device loss/timeout/OOM recovery is untested.
This does not retain or acknowledge a live scene; M0-06 application integration
must prepare GPU readiness separately, not treat a CPU snapshot or PNG as scanout.

## Authoritative source and licensing

Inspected pinned downloaded wgpu **29.0.4**, in Cargo registry
`wgpu-29.0.4/src/api/{instance,adapter,device,queue,buffer,command_encoder}.rs`:
request APIs, poll, write_texture, map_async/get_mapped_range/unmap and copy APIs;
`wgpu-types-29.0.4/src/{instance,texture,lib}.rs`: explicit
`InstanceDescriptor::new_without_display_handle`, `TexelCopyBufferLayout` alignment,
`PollType::Wait` bounded timeout. `BufferViewMut` is write-only in v29: use
`slice(...).copy_from_slice`, not a mutable byte slice. Pipeline/render-pass usage
follows the already inspected pinned APIs in [output-spike.md](output-spike.md).
Versioned upstream rustdocs:
[wgpu 29.0.4](https://docs.rs/wgpu/29.0.4/wgpu/),
[wgpu-types 29.0.4](https://docs.rs/wgpu-types/29.0.4/wgpu_types/).
wgpu is MIT/Apache-2.0; original Sela shader/module, no copied application code or
assets, no new dependencies. Cargo.lock already pins wgpu and pollster 0.4.
No GPUI pattern added; existing GPUI provenance remains in the bootstrap/spike
notes. Transitive/distribution notice audit is still a separate gate.

## Executed checks — 2026-10-04 UTC

Debian12 x86_64 orb, debug build. Offscreen **Vulkan** actually succeeded on
`AdapterInfo`: `llvmpipe (LLVM 15.0.6, 256 bits)`, device type Cpu, vendor 65541,
driver llvmpipe, driver_info `Mesa 22.3.6 (LLVM 15.0.6)`, backend Vulkan. No X11
window/surface or shared native service was used/changed. GPUI black Vulkan
capture is not evidence that this independent wgpu readback fails.

`check_readback()` is a repeatable opt-in method retained for the parent main:
asymmetric 3x2 orientation/RGBA/row padding; 128/64/32 sRGB midtone roundtrip and
half white overlay (~205/192/189); zero/half/full glyph coverage; half-alpha white
background (~188); transparent magenta rejection; exact contain/cover pixel
boundaries and center crop for wide AND tall images; invalid mask/zero background/
over-edge output rejection; successful render following rejected calls. Endpoint
and geometry checks are exact; color conversion tolerates +/-1 byte (half white
187..189), not universal backend pixel identity. Normal unit tests request no GPU:
two tests cover both fit axes, lengths, zero/8193/u32max edges, device cap,
overbudget sizes and exact 64MiB acceptance arithmetic.

Exact executed commands, from this worktree, with temporary `compose_gpu_check`
including the module and calling `new(VULKAN)`, `adapter_info`, `check_readback`:

```sh
export CARGO_TARGET_DIR=/home/user/workspace/repo/target
cargo test --locked --example compose_gpu_check -j 4
cargo test --locked --all-targets -j 4
cargo clippy --locked --example compose_gpu_check -j 4 -- -D warnings
rustfmt --edition 2024 --check examples/composition/gpu.rs examples/compose_gpu_check.rs
runtime=$(mktemp -d /tmp/sela-compose-runtime.XXXXXX)
XDG_RUNTIME_DIR="$runtime" cargo run --locked --example compose_gpu_check -j 4
rmdir "$runtime"
git diff --check
```

All passed after correcting initial v29 constructor/write-only API compile errors
and Clippy array-chunk suggestions (automatic fix rolled back because reference
comparison needed dereferencing; corrected explicitly). All-target tests: **24
passed**, including two new GPU-free module tests. Initial successful GPU run
without private XDG directory printed two runtime warnings; private-runtime reruns
passed without them. No global environment mutation inside the module.

Reproduce the retained pixel assertions using the permanent command in
[composition-spike.md](composition-spike.md). It calls `check_readback()` before
rendering actual text. There is no need to recreate a disposable harness.

Reviewed `.amp/in/artifacts/m0-07b-gpu.png` via media inspection: centered 2x2
image with black sidebars, red/green/blue/muted-yellow quadrants, opaque white
synthetic H (left stem crosses into bar deliberately) and lighter half-coverage
right bar. Pixel checks, not visual inspection, establish alpha/color correctness.
Ignored local review evidence, not a committed golden. Mask is synthetic: no font
shaping claim until parent integrates the parallel explicit-font CPU raster.

## Integration and open qualification

The common Cargo example includes this module, so its unit tests now run under
ordinary all-target checks. `check_readback` runs only in explicit GPU diagnostic
mode; `render` receives the rasterizer's exact-size mask. The parent integration
note records actual-font checks; reviewed cross-platform goldens remain open.
Source and destination transition lifetime, observed cuts/fades, video surfaces,
Windows DX12, physical GPU/display/scanout, hotplug/device failure, native E2E,
timing/CPU/GPU/memory measurement and backend selection remain unrun/open.
Software offscreen results cannot qualify those gates; M0-07 is not done.
