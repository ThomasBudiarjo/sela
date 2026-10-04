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
