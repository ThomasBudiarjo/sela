# M1-01a durable storage contract

Implemented-unqualified prerequisite under the approved UI-first exception;
not completion of M1-01, M1-05, M1-09 or the physical-output gate.

## Integration

`sela::storage` is framework-independent. Create `Worker::open(PathBuf)` once
per operator profile. It starts opening on a dedicated thread; poll for `Opened`
before submission. No connection, filesystem inspection or migration runs on
the caller. `Repository` is public for non-UI integrations/tests but **worker-only**:
every synchronous method may perform blocking I/O. No live contracts changed.

`submit(Command)` is nonblocking and returns a cancellation handle, or `Busy`,
`Invalid`, `Closed`. One request may be outstanding, including its unconsumed
completion; rejected submissions never replace it. `poll()` returns at most one
completion without waiting. Keep polling from the UI's normal scheduling loop;
do not spin. Because there is only one outstanding request, its result identifies
the submitted command without a separate generation. UI owners must still track
their editing intent and must not apply a late reply to a different editor.

Commands: `SaveSong`, `SaveSchedule`, `Song`, `Schedule`, `DeleteSong`, `Heads`,
`Catalog`. The worker creates missing profile parent directories before opening;
an unusable parent reports `Io` without replacing existing files.
Saves with `None` create independent stable 128-bit SQLite random IDs (collision
fails, never overwrites). `Some(Version)` is an exact expected-head update;
stale/deleted identities yield `Conflict`, not last-write-wins. Success returns
the committed ID/revision. Display titles need not be unique. `Heads` returns
up to 128 versions, sorted by ID, with exclusive last-ID cursor. Fetch individual
content through `Song`/`Schedule`; `Catalog` returns current nondeleted versions
and titles with the same page bound/cursor. It decodes one bounded payload at a
time on the worker; no search/sort-by-title feature promised.
New songs can be duplicated by saving a fetched Song with `None`.

For the original read/write commands, cancellation is sampled once immediately
before command execution (copy-command exceptions are documented below). If it wins,
`Canceled` means no command I/O began. Once execution starts, cancellation does
not interrupt transactions and the actual commit/failure is reported. Handle
cancellation is advisory, not acknowledgment. Worker Drop marks the pending
command canceled and detaches, never joins; already-running commits can finish.
Drop discards their result, so **poll saves before closing when their outcome
matters**; reconcile durable heads after unexpected close. No false canceled
success is emitted. Opening itself is not interruptible. Error enums contain
neither paths nor SQLite messages nor lyric payloads; don't log Commands/Replies
(their Debug includes content).

## Schema and durable meaning

Current schema 3 uses `application_id=0x53454c41`, `user_version=3`, foreign keys on,
default rollback journal and synchronous FULL. `songs` tracks head/tombstone;
`song_revisions` stores immutable original UTF-8 payloads. Encoding is repeated
little-endian u32 byte-length + UTF-8: title, authors, copyright, license, then
label/lyrics pairs until EOF. No normalization, line-break conversion, splitting,
wrapping or provider assumptions. Empty sections and zero sections are preserved;
blank title is invalid. The original schema-1 text codec remains unchanged in
schemas 2 and 3; explicit identities, arrangements (schema 2) and per-slide
formats (schema 3, `section_formats`) live in revision-keyed tables.

`schedules` tracks head; `schedule_revisions` stores title; `items` records ordered
positions and exact immutable song revisions. Repeated occurrences are retained.
Saving the head, revision and every item is one IMMEDIATE transaction. A failure
on any later item rolls everything back, including the head. Song edit/delete
does not modify old revisions; delete is a tombstone and snapshots still resolve.
Historical deleted revisions can intentionally be reused in another snapshot.
No automatic garbage collection, library-refresh or cascade deletion exists.
Snapshot occurrence identity currently is `(schedule ID, revision, position)`;
stable cross-edit/live selection identity belongs to M1-09, not this storage slice.

Fresh schema 0 initializes schema 3. Existing schema 1 or 2 upgrades only after
the verified backup gate documented below (`<profile>.schema1-backup` or
`<profile>.schema2-backup`; schema 1 goes straight to 3 with one backup). A schema-0 database
with user objects is foreign and rejected. Migration and validation share one
transaction; failed migration drops/rolls back it. Nonmatching application IDs,
newer/unsupported versions, SQLite quick-check/foreign-key failures and malformed
payload reads are rejected. No reset, overwrite, rename or recovery mutation.
Required column checks run at open; payload semantic validation runs on bounded
reads. This is not an adversarial SQLite sandbox or exhaustive schema attestation.
Backup verification never invokes upgrading open. Versions above 3 remain
`Unsupported`; no migration resets, overwrites or replaces those profiles.

## Bounds and remaining parent work

- Song: 1024 title bytes, 4096 bytes per metadata field, 128 sections, 256 label
  bytes, 256KiB total encoded payload. Limits are UTF-8 bytes, not graphemes.
  Validation sums lengths before encoding, including oversized rejection.
- Schedule: 1024 title bytes, 32 ordered items; completion at most 8MiB encoded
  song data plus owned UTF-8/vector overhead, including repeated items.
- One capacity-one request and completion channel, one worker/connection per
  Worker; 4096-byte path; SQLite length limit 260KiB and 2MiB page-cache target.
  Caller-owned content, SQLite/OS transient allocations and thread creation are
  not hard RSS bounds. Do not create a worker per keystroke.
- Busy timeout explicitly 100ms. Kernel I/O, integrity scanning, scheduler and
  SQLite non-lock work have no hard wall-clock deadline; all stay off UI/frame.
- Total historical disk growth, retention/backups, crash/power-loss injection,
  autosave/close protocols, search and large-library performance remain open.
- Themes, fonts and asset schemas are deliberately **not** invented;
  parent M1-01/M1-06/M1-08/M1-09/M1-11 own their concrete snapshot contracts.
- GPUI editor integration, native/Windows, installed reference and physical
  qualification were not executed for this framework-independent slice.

## Dependency evidence and checks

Authoritative rusqlite rustdoc inspected on 2026-10-04:
[crate 0.40.2](https://docs.rs/rusqlite/0.40.2/rusqlite/) and
[Connection API](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html).
Reviewed open flags (omit URI interpretation), IMMEDIATE transaction rollback,
busy timeout and limits. Pin `=0.40.2`, bundled SQLite, limits; rusqlite is MIT,
SQLite public domain (dependency source notices retained, no copied application
code). Earlier API inspection of 0.38.0 was superseded by Cargo's available
maintained 0.40.2; no existing dependency versions were upgraded.

Disposable fixture tests live alongside the implementation. Reproduce:

```sh
export CARGO_TARGET_DIR=/home/user/workspace/repo/target
cargo test --locked --all-targets -j4
cargo clippy --locked --all-targets -j4 -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Tests cover reopen, original Unicode and intentional CRLF/LF/trailing breaks,
duplicate titles, snapshots after edit/delete, stale heads, FK and injected
second-item trigger rollback, migration transaction rollback, byte-preserved
newer/foreign/corrupt files, real competing connection lock timeout, actual SQLite
`max_page_count` disk-full rollback/reopen, pre-start cancel, race-legal cancel
outcome, queue saturation and nonjoining Drop. SQLite disk-full simulation is
not host-volume exhaustion or hardware power-loss evidence.

## M1-01b — backup / restore-to-new-profile prerequisite

State: **implemented-unqualified**, LOCAL main `2b62c77` prerequisite, not full
M1-01 or M1-13 portable bundles. No destructive schema migration was invented.

### API and safety contract

Submit `Command::BackupNew(destination)` or
`Command::RestoreNew { source, destination }` on the existing Worker and poll
`Reply::Copied`. Restore does not switch the worker's current profile: open a
new Worker at the restored path after acknowledgment. Synchronous
`Repository::backup_new` / `restore_new` are background-only. All filesystem and
SQLite work stays off the submit/poll caller. Both paths are capped at 4096 bytes;
one outstanding command/completion remains the bound, including backup/restore.

Parent must already exist and be a trusted, local profile directory, not a
hostile writable directory or network share. Source is never replaced. Existing
destination (including symlinks, source aliases/hardlinks) or destination SQLite
`-wal`, `-shm`, `-journal` yields `Exists`. Final no-replace hard-link publication
also protects against a racing destination file creation. Sidecar/directory
races by hostile actors are outside this contract. Filesystems lacking hard
links fail with `Io`, no copy/rename fallback that could overwrite a file.
Unix staging is 0700 and published database 0600; Windows inherits directory ACLs
and remains unqualified. No paths/SQLite messages/content added to Error or logs.

Online Backup copies SQLite's committed view, including WAL content, into a
random exclusively-created sibling staging directory. It copies 64 pages/step,
fails `Locked` on BUSY/LOCKED instead of retrying indefinitely, and checks a
256MiB logical database budget before copying and after each step. Concurrent
external commits may restart the copy; completion is a consistent snapshot,
**not** necessarily the head at submission time. Worker commands cannot mutate
its source connection during copying. Restore opens the input read-only without
CREATE/URI/migration. Missing/corrupt/foreign/newer inputs fail without reset;
read-only SQLite may use/create WAL shared-memory auxiliary files, so this is
not a promise of zero filesystem auxiliary activity. Unrecoverable hot-journal
inputs fail; restore is not a repair engine.

Private output is converted to rollback-journal mode, explicitly closed, reopened
through existing supported-schema/quick-check/FK validation, then every historical
song payload and schedule snapshot is bounded-decoded. Head resolution,
tombstones and contiguous occurrence positions are checked. Verification is not
an adversarial SQLite sandbox or exhaustive schema attestation. The output is
closed and file `sync_all` succeeds **before** publication. Success requires all
these checks; malformed history never becomes a published backup.

The five-second cooperative budget is sampled between copy steps and historical
rows; `Budget` also means oversize. Busy timeout is 100ms; queries, integrity
scans, OS I/O/sync, allocation, close and scheduling are not hard deadlines.
Two connections use 2MiB cache targets; semantic validation holds one <=256KiB
song or <=32-item/8MiB schedule at a time. These are not hard RSS or physical disk
quotas; staging/journal/transient overhead and a final size-check step can exceed
logical budget, and abandoned staging can accumulate. No measured large-library
performance, rotation or retention policy is claimed.

Unlike writes, copy cancellation is sampled during steps/validation and once
immediately before publication. `Canceled` means no destination published by
this operation (private I/O may have occurred). Cancellation after the final
sample loses: report actual `Copied`/failure, never false canceled success.
Worker Drop detaches; publication can win a concurrent Drop. Always consume the
completion when outcome matters. Ordinary failures best-effort remove staging;
process abort can leave `.sela-backup-*` private directories. They are not backups
and are never auto-restored; remove only known abandoned staging with no active
worker. A process dying after publication but before acknowledgment leaves an
unknown-to-caller outcome: inspect/reopen the chosen destination, do not overwrite.

Hard-link creation is the publication boundary; no subsequent fallible cleanup
changes its outcome. Parent-directory metadata is not synced: **no power-loss,
reboot durability or hardware/fsync guarantee**. This deliberately avoids
returning an ambiguous generic failure after publishing. SQLite FULL/file sync
is not qualification for faulty hardware, volume exhaustion or Windows behavior.

### Scoped checklist and executed evidence

- [x] Worker-owned online backup, bounded commands, verified fresh publication.
- [x] Validated read-only-input restore to a fresh profile; no in-place recovery.
- [x] Disposable Unicode/revision/tombstone and asymmetric `B,A,B` snapshot
  backup/reopen/restore including a live WAL source.
- [x] Real exclusive lock rejection and uncommitted competing-writer exclusion;
  corrupt/newer input byte retention, destination/source collision and pre-copy
  cancellation; corrupt historical payload fails after private copy, no publish.
- [x] Child process abort after first partial backup step leaves no destination
  and unchanged source. Child process abort with spilled uncommitted transaction
  restores committed head/content and removes uncommitted revision on reopen.
  These are actual process termination tests, not transaction-Drop simulations.
- [ ] Real destructive migration backup/upgrade/rollback gate: schema 1 has only
  fresh initialization. Future migrations must gate mutation on acknowledged
  verified backup and test the actual migration, not hypothetical destructive SQL.
- [ ] Windows/local-filesystem qualification, power-loss/volume failure injection,
  measured large-library budgets, continuous writer restart/starvation stress,
  UI recovery chooser/rotation and portable asset bundle integration.

Authoritative provenance inspected 2026-10-04: SQLite
[Online Backup](https://www.sqlite.org/backup.html),
[C backup API](https://www.sqlite.org/c3ref/backup_finish.html) (step locks,
automatic restart, DONE, finish rollback), and
[WAL](https://www.sqlite.org/wal.html) sections 2–4 (WAL is persistent state,
FULL sync and checkpoint semantics). Pinned rusqlite **0.40.2** local registry
`src/backup.rs` inspected (`Backup::new`, `step`, `progress`, `Drop` finish;
Drop ignores finish return, so explicit DONE + close/reopen validation required),
corresponding [versioned module](https://docs.rs/rusqlite/0.40.2/rusqlite/backup/index.html).
Only its existing `backup` feature enabled; Cargo.lock unchanged, no new crate or
upgrade, original implementation, existing MIT/public-domain notices retained.

Executed on Linux x64 orb with shared target and `-j2`:
`cargo test --locked --lib storage::tests -j2` (14 pass, zero ignored),
`cargo test --locked --all-targets -j2` (74 pass, zero failed/ignored),
`cargo clippy --locked --all-targets -j2 -- -D warnings`,
`cargo fmt --all -- --check`, `git diff --check`.

## M1-06b — stable IDs / persisted arrangements / backed-up schema 2

State: **implemented-unqualified**, from LOCAL main `f161dc3`, branch
`ticket/m1-06-persistence`. This section supersedes M1-01b's historical
"future migration" checklist above. No dependency, operator geometry, live
scene, renderer, or output-control change; all exercised profiles are disposable.

`Section` now carries `arrangement::SectionId`; `Song` also carries bounded
`Vec<Variant>`. Existing SaveSong/Song/Schedule replies round-trip the complete
document. `Repository::arrangement(Version)` and worker `Command::Arrangement`
return `Reply::Arrangement` for exactly that stored immutable revision, never
the head. The pure `Song::arrangement(version)` adapter validates caller-provided
pairing; use the repository/worker adapter when provenance matters. SaveSong
persists text, section-position/ID mappings, ordered named variants and ordered
occurrences in the same IMMEDIATE transaction as head advancement. Foreign keys
and uniqueness supplement domain validation. Missing references reject the
whole save/read; no variant truncation, implicit retargeting or lyric refresh.

`section_ids`, `variants`, `occurrences` are keyed by immutable song ID/revision.
Positions only store order; identity is the persisted opaque ID. Old payloads,
schedule titles/items/order and tombstones are untouched. Each legacy immutable
revision receives fresh SQLite random section IDs once, even when its labels,
indices or text equal another revision. No ambiguous old cross-revision
continuity is invented. Legacy has no variants, so migration invents none.
New edits/whole-document undo retain IDs and variants. Duplicate creates a new
song ID, retaining document-local IDs; identity remains scoped to the song.

Opening schema 1 on the worker reserves the writer with BEGIN IMMEDIATE, then
copies through a separate read-only connection to `<profile>.schema1-backup`.
The verified, synced, no-overwrite publication must succeed before migration
DDL or mappings begin. The reservation excludes competing commits between the
backup snapshot and migration, including WAL writers. Copy verification opens
with migration **disabled**, checks schema 1 history as legacy text, and leaves
the published backup schema 1. RestoreNew similarly preserves schema 1 or 2;
only a later explicit Repository/Worker open upgrades a restored schema-1 file.
DDL/mappings/user_version commit atomically. Error or process death before
commit rolls them back; a published backup remains for recovery. Fresh empty
initialization does not need a backup because no legacy history exists.

An existing backup/alias/sidecar, permission/full/budget/lock/integrity failure
blocks opening/mutation, not just backup notification. Backup is never reused,
upgraded, overwritten, rotated or removed automatically. After failed migration,
the retained deterministic backup can make retry return `Exists`: preserve both
files, inspect/recover via RestoreNew to a fresh profile (whose migration backup
path is fresh), rather than delete/overwrite the backup blindly. Recovery UI and
profile switching remain parent work. Online backup preserves content/schema,
not identical SQLite file header/page bytes; historical payload bytes are exact.

Original text codec limits still apply, including a full 256KiB legacy payload.
Unarranged songs are validated for text and unique section IDs without tightening
their legacy byte budget. Creating a SourceSnapshot/arranged song additionally
requires the existing 256KiB text-plus-24-bytes-per-section domain budget;
oversized legacy text remains readable/editable but arrangement creation can
fail atomically until explicitly shortened. Variants retain the existing 16 /
512 occurrences each / 64KiB aggregate domain budgets. Bounded SQL reads include
one overflow row and check contiguous positions, full mapping count and IDs.
Schedule completion can additionally own up to 32 arrangement payloads (2MiB
structural accounting plus vectors). Transient validation clones and SQL row
strings are not hard RSS guarantees; no measured latency claim. Migration loops
one bounded revision at a time; the backup's 256MiB/five-second cooperative budget
does not impose a hard migration deadline or post-upgrade database disk quota.

Editor allocation is CPU-only SHA-256 truncated to 128 bits over nanosecond
wall-clock/PID/process atomic counter, not labels/indices/content and not a
cryptographic random/identity guarantee. Counter separates concurrent allocations;
duplicate IDs within a song fail validation/constraints rather than overwrite.
Undo restores old IDs rather than calling the allocator. Imported/caller-provided
IDs still require truthful provenance. Clock/PID reuse or adversarial collision
qualification is not claimed; SQLite random IDs remain migration's allocator.

Scoped evidence: six new storage tests plus one GPUI test cover legacy WAL and
revision-local IDs, old schedule 2/1/2 bytes/order, schema-1 backup/restore and
schema-2 arrangement backup/restore, conflict/corrupt backup gate/real writer lock,
DDL rollback, actual SIGABRT after mapping insertion with rollback and retained
backup, maximum legacy codec boundary, duplicate IDs/labels, V1/C/V2/C/C exact
revision, reordering, late missing reference/corrupt order no partial read/copy,
late occurrence insert rollback, worker round trips and editor save/duplicate/
undo/redo/rejected Remove. Existing historical schedule, capacity-one worker,
disk-full and process-abort backup/write contracts also run unchanged in intent.

Verification: serialized shared target, source timestamps refreshed before each
Cargo batch; actual root compilation and new test names observed. Commands:
`flock /tmp/sela-cargo-continuation.lock bash -c 'touch src/*.rs; export CARGO_TARGET_DIR=/home/user/workspace/repo/target; cargo test --locked --all-targets -j4'`
(104 passed, 1 ignored subprocess fixture invoked by three process tests);
same lock/touch/export with `cargo clippy --locked --all-targets -j4 -- -D warnings`
and `cargo fmt --all -- --check`; `uvx ruff check scripts/song-library.py`,
Python AST parse and `git diff --check` passed. Native replay is parent-owned
and was not run on shared :99. Windows/hard-link/ACL/physical GPU, installed
reference, real volume/power-loss failure and performance remain unqualified.

## M1-05g1 — per-slide formats / backed-up schema 3

State: **implemented-unqualified** (slice 1 of M1-05g; rendering and the
editor Format pane follow). Source: EW8-OBS-028–030 and EW8-OBS-033
(formatting is per slide in EasyWorship 8.0.49).

`storage::Section` carries a `format::SlideFormat`: optional overrides for font
family, bold, italic, underline, size (Auto or Fixed 1–288), color, horizontal
and vertical alignment, outline (enabled, color, size 1–50, opacity 0–100) and
shadow (enabled, color, angle 0–359, offset 0–100, blur 0–50, opacity 0–100).
`None` keeps the default look; an explicit `enabled: false` stays distinct from
`None`. These bounds are Sela's, not observed EW ranges.

`section_formats(song, revision, position, format)` holds a row only for slides
with a non-default format, keyed to `section_ids` by a foreign key. The blob is
a versioned canonical codec (codec byte, presence mask, present fields; at most
256 bytes). Reads reject unknown codecs/fields, truncation, trailing bytes,
out-of-range values, a stored default and rows past the revision's slides as
`Corrupt`; nothing is silently dropped. Saving writes the rows in the same
IMMEDIATE transaction as the revision; old revisions keep their formats.

Opening schema 2 publishes the verified `<profile>.schema2-backup` (schema 2,
checked with migration disabled) before the DDL, then creates the table and
sets `user_version=3` in one transaction. Schema 1 migrates to 3 in one
transaction behind its existing single backup. Backup conflict, writer lock,
DDL conflict and process abort after the DDL (`SELA_ABORT_MIGRATION_SCHEMA3`
test hook) leave the profile at schema 2 with identical bytes. RestoreNew
preserves schema 1, 2 or 3.

Tests: `format::tests::*` (round trip per field, strict decode, bounds) and
storage `schema2_migrates_with_a_verified_backup_and_keeps_ids`,
`schema2_migration_gates_leave_the_library_unchanged`,
`schema2_migration_abort_keeps_schema2_and_the_verified_backup`,
`slide_formats_round_trip_per_revision_and_default_stores_nothing`,
`invalid_or_corrupt_formats_are_rejected_not_dropped`; the schema 1 migration
test now also checks the live profile reaches schema 3 with one backup.
