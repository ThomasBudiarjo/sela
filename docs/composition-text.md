# M0-07a explicit-font CPU text diagnostic

This module was introduced as `examples/composition/text.rs` and moved to
`src/audience/text.rs` in M1-10a, where the audience renderer also uses
`raster_aligned` for centered lines. It is not product typography or operator UI.
The [composition example](composition-spike.md) includes it explicitly.
Call `raster(font, text, width, height, font_size)` on a preparation worker, never
the UI thread or render frame path. It returns `Result<Vec<u8>, TextError>`:
row-major `width * height` white-text alpha coverage (zero is transparent).
Caller owns placement/margins, tint, upload, compositing and readiness admission.
The output carries coverage only, not an RGB color-space conversion.

## Contract and limits

- Single supplied SFNT face only; no filesystem, network, discovery or fallback.
  Collections (`ttcf`) and unsupported headers are rejected before fontdb loads
  bytes. TTF parser and fontdb validate the face, then FontSystem preflights it.
- Font bytes <=2 MiB; UTF-8 text <=4 KiB; <=32 lines; finite font size 1..96;
  nonzero extents <=4096 on each axis; alpha payload <=16 MiB.
- Physical pixel coordinates at scale 1.0; font size is pixels; line height is
  size times 1.3. No margin, wrapping, ellipsis or automatic size reduction.
  Explicit line endings use cosmic-text's line parser (including CRLF).
  Advanced shaping handles bidi/Arabic and combining clusters.
- Every buffer line is laid out and checked, not only viewport-filtered
  `layout_runs`. Logical width/height overflow and nonzero raster ink outside any
  edge fail rather than silently clip. Negative bearings may require caller
  choices of text/font; the module does not add a compensating margin.
- `Bounds` means input/payload limit; `InvalidFont` means unsupported, malformed,
  collection or unusable face; `MissingGlyph` means glyph zero or a non-supplied
  face; `Overflow` means logical/ink overflow. On failure no partial mask escapes.
  Overlapping raster marks accumulate source-over alpha with integer rounding.
- Font database, shaping buffer and Swash cache are per-job and dropped on return.
  These bounds are **not a hard total-memory or execution-time cap**. Swash and
  shaping internals can allocate independently and expose no cancellation/time
  callback. This is not a hostile-font sandbox, crash-isolation claim or RSS
  measurement; worker admission/cancellation must not block safety controls.
  Color glyph RGB is intentionally discarded; unsupported fonts/render paths
  and general typography coverage are not qualified by this diagnostic fixture.

## Upstream and licensing

Authoritative upstream review supplied for this slice:

- cosmic-text **0.19.0**, commit
  `c24886c2471e5606587c46090cd25dbbf209186b`, MIT OR Apache-2.0:
  `src/font/system.rs` (constructors/get_font), `src/buffer.rs` (line layout,
  viewport iterator/draw), `src/layout.rs`, `src/render.rs`, `src/swash.rs`.
- fontdb **0.23.0**, commit `62cfd96671eba4debe73eeecf541b1a3a051d223`, MIT:
  `src/lib.rs` (empty Database, load_font_data and collection allocation).

The installed 0.19.0 source was also inspected for layout baseline and draw API.
Use `Database::new` plus `FontSystem::new_with_locale_and_db`; never substitute
`FontSystem::new` or `new_with_fonts`, which discover system fonts. The only added
dependency is exact cosmic-text dev-dependency with defaults disabled, `std` and
`swash`; it was already transitive. Lockfile changes only the root dependency list.
No GPUI pattern or Zed application code is added/copied. Preserve dependency
license notices on redistribution. Existing fixture `tests/fixtures/DejaVuSans.ttf`
and its full `DejaVuSans.LICENSE` remain unchanged; provenance/hash are in that
directory's README. All diagnostic text is original, not copyrighted lyrics.

## Executed checks and integration handoff

A temporary `examples/text_check.rs` included the module by path, used the fixture
and wrote a 480x120 PGM to `/tmp`. It was removed before the ticket commit. With
`CARGO_TARGET_DIR=/home/user/workspace/repo/target`:

```sh
cargo test --locked --example text_check -j 4
cargo clippy --locked --example text_check -j 4 -- -D warnings
cargo run --locked --example text_check -j 4
rustfmt --edition 2024 --check examples/composition/text.rs examples/text_check.rs
```

All passed; **3 tests, zero failed/ignored**. Checks distinguish Unicode coverage,
antialiasing/nonempty ink, explicit second-line placement, missing glyph,
horizontal/vertical overflow, negative-bearing ink overflow, oversized inputs,
invalid size/extents, malformed font and collection header. Initial compilation
found a fontdb iterator borrow across the move; explicitly dropping the iterator
fixed it. No font discovery was added to work around errors.

`magick /tmp/sela-text-mask.pgm .amp/in/artifacts/m0-07a-text.png` produced ignored
review evidence, visually inspected: Café, combining acute a and Arabic سلام on
line one; Signal beacon on line two, no clipping. This is a CPU mask inspection,
not GPU golden output or proof of full Arabic typography. Integrator should wire
this module into the real composition example and rerun these tests via that
example. No standalone harness is intentionally retained.

The permanent integration now runs these tests with
`cargo test --locked --example composition_spike` and adds actual-font GPU
readback/inspection. See [composition-spike.md](composition-spike.md) for evidence.

Not executed by this CPU slice: native/GPU composition, alpha golden comparisons,
physical display/Windows qualification, performance/RSS/cancellation measurement,
installed EasyWorship reference, transitions or text-over-video. Parent M0-07
remains active/qualification-open; no backlog boxes are closed by this slice.

## M1-05g2 — styled layers and colored blend (2026-10-06)

The M0-07a single-mask diagnostic grew into the product text path. One path
now carries the whole per-slide SlideFormat: slides::cue resolves the
format (face, synth flags, fitted or fixed size, alignment), then
udience::text::layers renders three RGBA coverage channels (fill incl.
underline, Outer outline by chamfer dilation, shadow by offset + box blur)
and the compositor blends them shadow under outline under fill, in linear
light, with each layer's color and opacity. Point-valued effects (outline
size, shadow offset/blur) scale by the 1080-line reference height, so the
audience and the 1280x720 editor preview/thumbnails agree. A WGSL shader does
the audience blend; compositor::blend_pixels is its exact CPU twin for the
editor's off-thread renders.

### Face resolution and the live gate

onts::shared() holds one catalog and face cache for the process. Whoever
opens first (operator or song editor) scans the system font directories off
the UI thread; resolution can read files, so it runs only on background
threads. The operator resolves a previewed song's formats as one background
job and attaches them to the item: a Go Live/Next/Previous that races the
resolver keeps the current live scene, shows Resolving fonts…, and retries
when the faces land (unit-tested by stripping the attachment and driving
the landing callback). A late catalog scan re-resolves items that named a
family and refreshed the live cue, since their earlier resolution was the
bundled fallback.

### Bounds and overflow authority

Every logical line's advance and accumulated line height are validated
before drawing, not only viewport-filtered runs. Fake-italic shear and
negative left bearings clip at the area edge like other fringe ink (EW
parity unobserved; recorded as a Sela decision); oversized logical lines
still fail with Overflow. Fixed sizes that do not fit are refused like
other unshowable text, and a failed preparation never replaces the live
scene.

### Measured preparation (ignored test large_text_preparation_time)

Windows 11, 2560x1600 and 3840x2160, bundled DejaVu Sans, 20 runs:
2560x1600: 1 line 256px p50 5.5 ms (max 6.8), 2 lines 241px p50 14.7
(15.7), 4 lines 253px p50 26.4 (28.4); 3840x2160: 1 line 288px p50 7.7
(8.4), 2 lines p50 20.5 (21.7), 4 lines p50 34.5 (36.2). All layers
(fill+outline+shadow) included; runs on the preparation worker, never the
UI thread.

### Native Windows checks (2026-10-06)

scripts/live-output-windows.py PASS on the secondary monitor
(2560x1600): the formatted song's Go Live shows the bold gold right/bottom
slide (color box 590,1378-2512,1514; margins 48 right, 86 bottom vs 590
left, 1378 top), Next shows the synth-italic underlined centered slide and
the installed-Arial slide, Next stops at the last slide, masks and sessions
behave as before, both operators exit 0 and end their audience children.
Alignment is asserted by margin comparison because auto-fit fills the area
width; the gold slide uses a fixed size so its placement is visible.
scripts/song-editor-windows.py PASS: preview and thumbnails render on the
same path with the scan running.
