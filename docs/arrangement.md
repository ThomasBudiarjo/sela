# M1-06a — bounded arrangement domain

State: **implemented-unqualified** framework-independent prerequisite, based on
LOCAL main `2b62c77`, branch `ticket/m1-06-arrangement`. No UI/reference behavior
claim, persisted schema change, added crate, or live-rendering integration.

## Contract

`SourceSnapshot::new(Version, sections)` owns exact text for one positive immutable
song revision. `Arrangement::new(source, variants)` owns that snapshot through
an Arc. Pure edits share its immutable content; resolution borrows section text,
so repeated choruses never copy lyrics. There is deliberately no library-head
lookup, provider trait, automatic refresh, or public mutation of validated data.
The caller must supply truthful revision/content pairing: these constructors
validate structure, not provenance. M1-06b's repository/worker adapter below
supplies exact stored revision pairing.

Newtyped 128-bit IDs reuse storage's `Id` vocabulary, not its persisted section
representation. Section IDs are unique within a source song; variant IDs within
an arrangement; occurrence IDs within each variant. Selection identity must use
song/version + variant ID + occurrence ID, never a label or vector position.
IDs are caller-assigned opaque values (including all-zero); pure constructors
never regenerate them. The editor's explicit CPU-only section allocator is
documented in storage.md. Repeated sections require distinct occurrence IDs.
Copied variants can retain occurrence IDs because variant identity separates them.

`with_variant` explicitly adds/replaces by ID; `without_variant` removes only that
ID. `edit` renames, inserts, removes, retargets or moves one occurrence. Insert
accepts positions 0..=length; move's destination is the **final** position
0..length after removal. IDs survive reordering. Every error returns no new draft
and leaves the original unchanged. `resolve` returns all ordered occurrences or
one error, never a prefix or silently dropped missing reference.

Empty source, variant list, variant occurrence list, section label and lyrics are
valid drafts. Variant names must have non-whitespace content; names are unique by
exact UTF-8 equality, without trimming/case folding/normalization. Original CRLF,
combining text and Unicode remain unchanged. Empty drafts are not implicitly
publishable live cues; downstream must define that policy.

Bounds: 128 source sections; 256-byte labels/names; source text plus 24 bytes per
section <=256KiB; 16 variants; 512 occurrences per variant; aggregate arrangement
payload <=64KiB (4-byte count + each variant's 24-byte ID/count/length overhead,
name bytes and 32 bytes per occurrence's two IDs). This is an accounting policy,
not a serialization format or hard RSS/CPU guarantee. Caller-owned inputs can
already exceed limits before rejection; accepted drafts/resolution are bounded.
Small linear lookups avoid unused generic frameworks. No performance claim.

## Persistence/editor integration policy

Legacy schema-1 `storage::Song.sections` has only label/lyrics vectors. Duplicate labels
and changing vector indices cannot supply stable identity. No adapter guesses IDs
in the domain slice. The backed-up, transactional schema migration assigns and
persists explicit section IDs once per legacy snapshot, and preserves a mapping
for any migrated arrangements. Do not regenerate IDs on each read, hash labels,
or rewrite original immutable revisions/schedule snapshots. Cross-revision
identity for old ambiguous sections requires an explicit migration policy, not
inferred label/index continuity; retaining separate legacy revision mappings is
safer than claiming false continuity. New edits retain section IDs; deleted IDs
are not reused. Each new repeated occurrence gets its own persisted ID.

Persist exact version, ID mappings and arrangement data atomically. Old snapshot
resolution must remain available after library edit/delete. Refresh is explicit:
construct/validate a new arrangement against the new snapshot; missing sections
fail the whole operation, requiring user repair. No fallback to current lyrics.
Keep undo/dirty/selection contracts in the owning editor slice. The schema-2
editor integration preserves stored data without introducing arrangement controls.

## Scoped checklist and verification

- [x] Explicit typed IDs, named separate variants and no lyric duplication.
- [x] Immutable exact-version snapshot ownership and all-or-error resolution.
- [x] Pure atomic edits, final-position moves, removal and retargeting.
- [x] Reference/ID/name validation, empty policy, count and UTF-8 byte budgets.
- [x] U tests: asymmetric V1/C/V2/C/C; scrambled sources/repeated labels;
  pointer-shared chorus/distinct occurrence IDs; reorder/separation; deleted and
  missing references; failed edit preservation; empty/Unicode/exact limits.
- [x] Persist IDs/variants; backed-up migration retaining old revisions (M1-06b).
- [x] Editor data selection/undo/save/load preserves IDs and arrangements (GPUI tests).
- [x] Native replay of schema-2 editor, independent persisted-ID checks (parent integration).
- [ ] Native arrangement-editing controls and explicit reference repair.
- [ ] Reference-observed manual breaks, splitting, pagination/font fitting,
  overflow feedback, fonts/bidi/aspect changes and deterministic layout.
  (M1-06c: documented Ctrl+Enter split, fit/normalize sizing and first-
  arrangement output order; none of them observed on 8.0.49 yet.)
- [ ] Renderer preparation/acknowledgment isolation and native/Windows/hardware
  reference qualification. Domain tests do not replace any of these gates.

Commands (shared target, Debian Linux orb):
`CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo test --locked --lib -j2`;
`CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo clippy --locked --lib --tests -j2 -- -D warnings`;
`cargo fmt --all -- --check`; `git diff --check`.
Results: 33 library tests passed (five new arrangement tests); strict Clippy,
format and diff checks passed. Shared-target stale baseline reuse was detected
and invalidated with source timestamp refresh before observing actual compilation
and the new tests. Verify test names when combining worktrees with a shared target.
No GPUI pattern was implemented, so upstream GPUI review is not applicable here.

## M1-06b persistence prerequisite — 2026-10-04

Implemented-unqualified on `ticket/m1-06-persistence`, LOCAL main `f161dc3`.
`storage::Section.id` is the same `SectionId` vocabulary and `Song.variants` is
the original domain `Vec<Variant>`; no competing editor model or serialized lyric
copies. `Repository::arrangement(v)` / worker Arrangement(v) reconstructs a
validated immutable source and all named variants from exactly v, including
tombstones. SaveSong atomically persists source IDs and ordered variants/
occurrences alongside original text. No fallback to head or reference trimming.
See [storage migration contract](storage.md#m1-06b--stable-ids--persisted-arrangements--backed-up-schema-2).

Legacy per-revision mappings allocate once under a verified schema-1 backup and
transaction, never by matching equal label/text/index across revisions. Old text
codec and schedule rows are untouched. The maximum unarranged legacy payload
is retained even if an explicit arrangement request exceeds this domain's
stricter source text-plus-ID budget; that request fails instead of losing text.
CPU-only editor allocation happens on New/Add, never Reload/Undo/Reorder.
IDs in Duplicate stay song-local; occurrence IDs remain variant-local.

Six storage tests and one GPUI editor test added; all-target suite 104 passed,
1 ignored child fixture explicitly invoked by three existing subprocess tests.
Observed actual root compilation/new names with shared-target lock/touch; strict
Clippy, fmt, Ruff/AST/diff passed. Tests exercise V1/C/V2/C/C/reorder and exact
old snapshot after edits/tombstone, duplicate labels/IDs, late corrupt/missing
reference and failed occurrence insert all-or-error, worker source provenance,
backup/restore both schemas and real process-abort migration rollback.
No renderer, layout, new GPUI pattern or native focus replay in this slice.
Parent owns merged serial :99 replay; Windows/reference/hardware/pagination and
performance remain open. Next: native replay and arrangement-control design,
explicit missing-reference repair/refresh, then deterministic pagination.

## M1-06c — Better slide output

Implemented-unqualified on local `main`. Scope: Phase 3 of the approved
2026-10-06 spec. Nothing here is observed on EasyWorship 8.0.49 yet.

- **Output order.** `slides::slides` follows the song's first arrangement,
  one slide per occurrence (V1/C/V2/C/C gives five slides). With no
  arrangement, an empty one, or one with an unresolved section, it falls back
  to stored section order so no lyrics are hidden. Choosing among
  arrangements stays with the M1-06 controls. Slide identity stays
  `revision << 16 | position`, so repeated choruses are distinct cues.
- **Text size.** The audience text preparer and `slides.rs` now allow 1–288px
  (was 96). Each slide is still fitted to the 32px-inset area with a 5%
  advance margin and at most a sixth of the text height, so a short line on a
  2560x1600 output is 256px and on 3840x2160 is 288px.
- **Resize text to fit / Normalize text size across slides.** EasyWorship's
  global song settings offer "Do not auto size text" or "Resize text to fit
  element", with "Normalize text size across slides" under the latter (SRC-12,
  documented-only). Sela always resizes to fit; its fixed-size mode needs a
  theme font size and waits for the theme ticket. **Normalize text size across
  slides** (Songs footer toggle, off by default until the EasyWorship default
  is observed) uses the smallest fitted size of the item's slides for every
  slide. It is computed once per song revision, output size and setting, and
  turning it on or off resends the live slide. The setting is not persisted
  (no settings store yet).
- **Ctrl+Enter split** (SRC-08, SRC-11). With the cursor in the song editor's
  lyrics, the text from the cursor on becomes a new section right after the
  current one, with a new section ID and the same label (copied label and
  splitting mid-line are provisional; SRC-11 describes a line start). A line
  break right before the cursor is dropped. Every arrangement gets the new
  section after each occurrence of the split one, so the output keeps all
  lyrics. One undo step restores the original; redo repeats it. Outside the
  lyrics, or at the 128-section or arrangement limits, nothing changes and the
  status line says why.

Measured text preparation (`audience::tests::large_text_preparation_time`,
`cargo test --locked --release --bin sela large_text_preparation_time -- --ignored --nocapture`,
20 runs, i7-14650HX): 2560x1600 at 241–256px, 1/2/4 lines, p50 0.6/1.6/2.7 ms
(max 3.3 ms); 3840x2160 at 288px, p50 0.7/2.0/3.5 ms (max 4.2 ms). The dev
profile is 4–30 ms p50 (max 32.7 ms). This runs on the audience preparation
worker, not the frame loop or the operator UI thread.

Tests: `slides::tests::first_arrangement_orders_slides_and_falls_back_to_sections`,
`slides::tests::normalized_sizing_uses_the_smallest_fitted_size`,
`slides::tests::font_size_fits_lines_and_width` (256/288px),
`audience::tests::operator_slide_cues_fit_the_audience_text_preparer` (every
fitted size up to 4160x2160 rasterizes), `audience::text` bounds (289px is
rejected), `tests::normalize_text_size_applies_to_the_live_slide` and
`song_library::tests::ctrl_enter_splits_the_section_at_the_cursor_and_undo_restores`.

Native: `python scripts/live-output-windows.py --out .amp\in\artifacts\live-output-windows-m1-06c`
PASS on 2560x1600 DX12; the two-line verse now spans x 108–2454. Ctrl+Enter
and the Normalize toggle were not driven natively (the song-editor native
driver is X11-only); GPUI tests cover them.

Open: observe EasyWorship's auto-size default, normalize scope (item or
service) and Ctrl+Enter behavior mid-line on 8.0.49; arrangement choice;
pagination of overflowing sections; persisted text settings.
