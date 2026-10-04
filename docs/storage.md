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

Cancellation is sampled once immediately before command execution. If it wins,
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

Version 1 uses `application_id=0x53454c41`, `user_version=1`, foreign keys on,
default rollback journal and synchronous FULL. `songs` tracks head/tombstone;
`song_revisions` stores immutable original UTF-8 payloads. Encoding is repeated
little-endian u32 byte-length + UTF-8: title, authors, copyright, license, then
label/lyrics pairs until EOF. No normalization, line-break conversion, splitting,
wrapping or provider assumptions. Empty sections and zero sections are preserved;
blank title is invalid. Binary codec is internal to schema version 1, not a bundle.

`schedules` tracks head; `schedule_revisions` stores title; `items` records ordered
positions and exact immutable song revisions. Repeated occurrences are retained.
Saving the head, revision and every item is one IMMEDIATE transaction. A failure
on any later item rolls everything back, including the head. Song edit/delete
does not modify old revisions; delete is a tombstone and snapshots still resolve.
Historical deleted revisions can intentionally be reused in another snapshot.
No automatic garbage collection, library-refresh or cascade deletion exists.
Snapshot occurrence identity currently is `(schedule ID, revision, position)`;
stable cross-edit/live selection identity belongs to M1-09, not this storage slice.

Only the non-destructive empty schema 0 → 1 migration exists. A schema-0 database
with user objects is foreign and rejected. Migration and validation share one
transaction; failed migration drops/rolls back it. Nonmatching application IDs,
newer/unsupported versions, SQLite quick-check/foreign-key failures and malformed
payload reads are rejected. No reset, overwrite, rename or recovery mutation.
Required column checks run at open; payload semantic validation runs on bounded
reads. This is not an adversarial SQLite sandbox or exhaustive schema attestation.
There is no destructive upgrade yet; a future upgrade must implement a verified
backup before mutation, not reuse this empty-database migration indiscriminately.

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
- Arrangements, themes, fonts and asset schemas are deliberately **not** invented;
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
