# Still images and missing-resource preflight (M1-11)

This note records the limits, policies and decisions for still images in the
profile and for the schedule preflight. Code: `src/images.rs` (import, relink,
decode entry points), `src/scene.rs` (`decode_image`, the one bounded decoder)
and `src/preflight.rs` (report and worker). EasyWorship's own media import
(formats, copy versus reference, duplicate handling, missing-media prompts) has
not been observed in 8.0.49; everything here is Sela policy until it is.

## M1-11a — import hardening and preflight API

State: **implemented-unqualified**. No operator UI uses the new APIs yet
(M1-11b). The existing Media tab import (`Job::Import`) and the Song Editor's
Import… (`images::import`) get the hardened behavior without code changes.

### Storage and identity

- Images are copied (managed, not referenced) into
  `<profile>/Resources/Images/`, outside SQLite. Songs pin an image by bare
  file name and SHA-256 (`background::ImageRef`); the bytes are the identity,
  the name is the operator's label.
- The logo is chosen by name in `<profile>/logo.txt` and hashed when loaded;
  it is not pinned, so a replaced logo file is shown, not reported.

### Limits

| Limit | Value | Where |
| --- | --- | --- |
| File size | 8 MiB (`images::MAX_FILE_BYTES`, `scene::MAX_SOURCE_BYTES`) | every read |
| Edge | 16384 px (`images::MAX_EDGE`) | header check and decode |
| Decoded size | 64 MiB straight RGBA (`images::MAX_DECODED_BYTES`): 16 777 216 px, e.g. 4096 × 4096 or 5120 × 3200 | header check and decode |
| Decoder buffer | the same 64 MiB, so 16-bit PNGs stop at 8 388 608 px | header check and decode |
| Images listed | 4096 | `images::list` |
| Name suffixes | " (2)" to " (1000)" | import |

- A typical 24-megapixel camera photo (6000 × 4000) exceeds the decoded
  budget and is refused at import with "Image is too large …"; the operator
  has to save a smaller copy. Raising the budget needs a memory decision for
  the operator's background cache and the renderer, so it is not changed here.
- The logo goes through the renderer's preparation, which also checks the
  GPU's maximum texture size; a logo wider than that (for example 8192 px on
  some integrated GPUs) fails there even though it imported. Backgrounds are
  fitted on the CPU first and do not have this limit.
- Decompression bombs: declared dimensions are checked from the header
  (`PngDecoder::with_limits` with `image::Limits` max width, height and
  allocation; the JPEG header's dimensions) before any pixel buffer exists,
  and `decode_image` repeats the checks and bounds the decoder's own buffer
  (`ImageDecoder::total_bytes`). Rotation for Exif orientations 5–8 makes one
  transient copy, so a decode peaks at about twice the budget.

### Import steps

`images::import_checked(profile, source, cancel)` (and the unchanged
`images::import`, which returns only the name) run on a background thread:

1. Name: the source's stem with control characters replaced, trimmed and
   without leading dots, and its extension lowercased (`png`, `jpg`, `jpeg`).
   The format is taken from the content, not the extension.
2. Read at most 8 MiB.
3. Header inspection: PNG or JPEG only, APNG refused, dimensions bounded,
   orientation and ICC presence recorded.
4. Full bounded decode with `scene::decode_image`, so an imported image never
   turns out corrupt later (damaged data is refused as "damaged or
   incomplete").
5. Name choice: `name.png`, then `name (2).png`, …. If one of those already
   holds byte-identical content, that name is returned with `reused: true`
   and nothing is copied (bounded: at most 64 MiB of same-stem files are
   compared per import; beyond that a new numbered copy is made). Identical
   bytes under a different stem are copied again; deduplicating across the
   whole folder would mean hashing it on every import.
6. Copy: written in 256 KiB chunks to a dot-prefixed temporary
   (`.sela-import-<pid>-<n>.tmp`, never listed), synced, then given its final
   name with a hard link that fails if the name exists, so an existing image
   is never replaced. On volumes without hard links (FAT, exFAT, some network
   shares) the name is reserved with an empty file and the temporary renamed
   over it. The temporary is removed on every exit path.

The same name with different bytes therefore gets the next free name, and
songs pinned to the original keep it, byte for byte.

`Imported` reports the stored name, SHA-256, displayed extent (after
orientation), `reused`, `oriented` and `icc_ignored`, so M1-11b can show
what Sela did with the file.

### Orientation, color, animation

- Orientation: the Exif orientation (JPEG APP1, PNG `eXIf`) is applied in
  `scene::decode_image`, so backgrounds, thumbnails, the editor preview and
  the logo all show the displayed orientation, and the extent is the rotated
  one. Images imported before this change that carry an orientation now
  display rotated; their SHA-256 and pins are unchanged.
- Color: embedded ICC profiles are ignored and samples are treated as sRGB;
  PNG `gAMA`, `cHRM`, `sRGB` and `cICP` chunks are ignored too. 16-bit PNGs
  are reduced to 8 bits, grayscale is expanded, and alpha is kept straight.
  A wide-gamut (Display P3, Adobe RGB) or CMYK image may therefore look
  different from a color-managed viewer. CMYK JPEGs are converted by the
  decoder without color management; this path is untested.
- Animation: animated PNGs are refused at import ("Animated images are not
  supported; export a still …") and already rendered black before, because
  the renderer only takes still images. Taking the first frame was rejected:
  it would show something other than what the operator chose, and how
  EasyWorship treats animated images is unobserved. GIF, BMP, TIFF, WebP and
  other formats are not supported at all (the `image` crate is built with
  PNG and JPEG only).

### Cancellation and failure

- `import_checked` and `relink` take a cancel flag, checked between steps and
  before every 256 KiB chunk; a cancelled operation leaves no file (the
  temporary is removed) and returns `Cancelled`. Once the final name exists
  the import is complete. The decode itself is not interruptible; it is
  bounded by the limits above.
- `images::Worker::cancel_imports()` cancels every import submitted so far,
  running or queued; later imports are unaffected. The import replies
  `Imported(Err(Cancelled))`.
- A crash between writing and linking can leave a dot-prefixed temporary in
  the images folder. It is never listed or referenced; removing stale
  temporaries is left to a later cleanup slice.
- Errors name the side that failed and the recovery: `ReadDenied` (the chosen
  file), `ProfileDenied` (the profile's `Resources/Images` folder, including
  read-only file systems), `DiskFull`, `SourceMissing` (the chosen file was
  moved), `TooLarge`, `Dimensions`, `Animated`, `Corrupt`, `Unsupported`,
  and `Io(kind)` with the kind in the message.

### Preflight

`preflight::check(profile, songs, fonts, cancelled)` takes a schedule's
resolved songs in item order (`storage::Reply::Schedule`) and reports, per
item with problems:

- images that would render black, grouped by `ImageRef` with every use
  (`Use::Master`, `Use::Slide(section index)`): `Missing`, `Changed`
  (SHA-256 mismatch), `TooLarge` or `Invalid`, the same `images::Substitute`
  values the renderer's warning uses;
- named font families that resolve to the bundled DejaVu Sans fallback
  (not installed, or catalogued but the file no longer loads), with the
  slides that use them;
- the profile's logo, when `logo.txt` is unreadable or names an image that
  cannot be read.

Images are hashed, not decoded: a matching hash means the bytes chosen in
the editor. Fonts are resolved through the given store
(`fonts::shared()` in the app), exactly as rendering resolves them; until a
catalog has been scanned `fonts_checked` is false and no font is reported.
Work is bounded: 256 distinct image names (each read at most 8 MiB) and 64
distinct families per check, with skipped uses counted in
`unchecked_images` and `unchecked_fonts`; `is_clean()` is false while
anything is unchecked.

`preflight::Worker` runs checks and relinks on its own thread with two
queued requests and two buffered replies; `check` returns a generation, and
a newer check cancels an older one still running (superseded checks do not
reply). The UI never calls `preflight::check` directly.

### Repair (relink)

`images::relink(profile, image, source, cancel)` (also `Request::Relink` on
the preflight worker) copies a located file into the profile under the pinned
name only if its SHA-256 equals the pin:

- a different file is refused with `Mismatch` and nothing changes;
- if the pinned name already holds the matching bytes, `AlreadyPresent`;
- if the pinned name holds a different image (the "changed" case), the
  relink is refused with `NameTaken`: that file may be pinned by other songs
  under its new hash, so it is never replaced. The operator renames or
  removes it first. A guided "keep both" action is M1-11b's to design.
- The copy uses the same temporary-and-link steps as import.

### Tests

`cargo test --lib images:: preflight::` covers: same name with different
bytes and identical re-import; oversized file; declared-dimension bombs
(100000², 16385 × 1, 1 × 16385, 4097 × 4096, 16000², a 16-bit 3000²) and a
truncated file at exactly the budget; APNG; Exif orientations 1, 3, 6 and 8
in PNG `eXIf` and 6 in a JPEG APP1 segment (decode, thumbnail and import
extent); cancellation after part of the copy is on disk and before it;
worker cancellation of a queued import; relink acceptance, mismatch, missing
source, cancellation and an occupied name; error mapping; preflight of
missing (moved), changed and present images, missing and present fonts, a
vanished font file, the logo, an unscanned catalog, the bounds and
cancellation; and the preflight worker. Fixtures are generated in the tests
(PNG chunks with their CRCs, the Exif IFD and the JPEG APP1 segment are
assembled by hand). Permission denial runs for real: `#[cfg(unix)]` with
`chmod`, and on Windows an ignored test that adds and removes deny entries
with `icacls` (`cargo test --lib images::tests::permission -- --ignored`).

### Open

- M1-11b operator UI: run preflight when a schedule opens or before Live,
  show the report, Locate… with a file prompt that submits
  `Request::Relink`, Cancel import, and show `Imported` notes (reused,
  rotated, color profile ignored).
- EasyWorship observation of media import, duplicates and missing-media
  handling; supported formats beyond PNG/JPEG (M2 for video).
- Larger photos (decoded budget), stale temporary cleanup, thumbnails at
  scale and metadata (M1-11 checklist), Linux/macOS runs of the permission
  test, and native checks.
