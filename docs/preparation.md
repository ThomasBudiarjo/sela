# M0-05 CPU preparation contract

This is an implemented CPU-resource slice, not a production renderer or a GPU
readiness guarantee. `scene` and `preparation` contain no GPUI types. M0-07 must
finish layout/shaping and renderer-owned upload before presentation; M0-06 owns
requested versus applied state. Neither the bootstrap nor triangle spike uses
these contracts yet. Windows/reference and M0-10 gates remain open.

## Ownership and versioning

`SceneSpec` is an editable owned value. Submitting a clone freezes its content
revision. `ContentVersion` carries a caller-assigned stable 128-bit identity and
monotonic revision; library/storage will own identity generation. Revisions are
not command sequence numbers. Selecting another item need not create a revision.

Resource references carry identity, revision, path and expected SHA-256. Only the
worker opens paths; changed bytes fail rather than silently substituting another
resource under the same version. `PreparedCue` owns normalized, straight-alpha
RGBA8 image bytes, font face-0 bytes and original UTF-8 text/line breaks. Read-only
accessors and shared `Arc` ownership keep edits or deleted source files from
changing that cue. Caller-held snapshots remain alive until their last owner
drops them. No system-font fallback or glyph-coverage promise is made here.

Static PNG/JPEG are accepted; animated PNG and other formats are rejected. No
EXIF orientation, ICC conversion or typography fidelity is claimed by this
decoder slice. M0-07 must settle those composition rules before user-facing use.
The font fixture is redistributable with its adjacent notices.

## Backpressure and limits

- One worker, one waiting job and one buffered completion. Submit and poll never
  block on worker completion. A full queue returns `Busy`, leaving the last
  accepted request intact. Accepted input cancels the older request.
- Source files: 8 MiB each; UTF-8 text: 64 KiB; paths: 4096 platform string units;
  font size: 1–512 logical units. Zero extents are invalid.
- Dimensions must fit renderer capabilities and at most 16,777,216 pixels.
  Owned image/text/font payload per cue: at most 64 MiB. Decode allocation limit:
  64 MiB. Intermediate decode/conversion buffers and allocator overhead are
  additional; this is not a measured process-RSS bound. Queued completion and
  executing worker may each hold a prepared payload. Consumers must bound their
  retained snapshots; there is no cache in this slice.
- Deadlines start on acceptance, including queue wait. `poll` rejects at exactly
  the deadline even with a queued completion. Request identity filters stale
  results; failure never supplies a replacement cue.
- Cancellation is cooperative before/after blocking stages. OS filesystem calls
  and decoders cannot be forcibly interrupted by a Rust thread. Teardown cancels
  without joining on the UI thread. A stuck worker consumes this one slot; do
  not repeatedly recreate workers to bypass backpressure. Hard termination and
  production worker supervision require process isolation, not this API.

## Renderer-boundary follow-through

CPU readiness is neither renderer acceptance nor presentation. M0-06 must use
fresh renderer epochs and ordered command identities, retain applied state until
acknowledgment, and reject old-session commands after reconnect. A future IPC
format needs an explicit schema version, size/capability validation and bounded
resource transfer. Rust memory layout, `PathBuf`, `Arc` and `Instant` are not a
wire format. No plugin ABI or general transport framework is introduced now.

## Checks

`cargo test --locked --all-targets -j 8` runs real temporary-file/hash/PNG/JPEG/font
checks plus gated-worker failure tests. A supplied monotonic clock makes deadline
boundary tests exact; gates make overload, cancellation and stale completion
reproducible. Font bytes and asymmetric pixels are checked, not just successful
return values. Separate native/render/Windows checks are still required when
these contracts are integrated into the compositor.
