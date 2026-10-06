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
