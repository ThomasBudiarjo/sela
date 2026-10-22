# M1-12 — Autosave, revision recovery and local profiles

State: **implemented-unqualified** (autosave-recovery stream, 2026-10-22).
Linux unit, integration, GPUI and native X11 checks pass; the Windows run,
EasyWorship observations, physical power loss and the older-hardware hitch
measurement are open (see [parity-pending.md](parity-pending.md#stream-autosave-recovery)).
EasyWorship 8.0.49 behaviour after a crash, and whether it autosaves
schedules at all, is **unobserved**; everything below that faces the
operator is a best guess recorded as EW8-OBS-071..077 in
[reference-observations.md](reference-observations.md#autosave-and-recovery--unobserved-best-guesses-2026-10-22-autosave-recovery).

Code: `src/autosave.rs` (framework-free store, policy, worker),
`src/profile.rs` (profile lock), `src/operator/autosave.rs` (poll loop,
prompt, footer, close flush), `src/instance.rs` (second-instance notice),
`src/main.rs` (lock before the window, `AutosaveSetup`), `Schedule::restore`
and `Schedule::baseline` in `src/schedule.rs`.

## Profile and single instance

- The profile is the folder of the library database: `%LOCALAPPDATA%\sela`
  on Windows, `$XDG_DATA_HOME/sela` (else `~/.local/share/sela`) elsewhere,
  or the folder of `--operator-library PATH`. It holds `library.sqlite`,
  `settings.txt`, `Resources/`, `Schedules/`, `Recovery/` and `sela.lock`.
- Before the operator window opens, `main` takes an OS lock on
  `<profile>/sela.lock` (`std::fs::File::try_lock`: `flock` on Unix,
  `LockFileEx` on Windows). The OS releases it when the process ends in any
  way (crash, SIGKILL, TerminateProcess), so no stale lock blocks a restart.
- A second operator on a locked profile opens no library, writes no profile
  file and shows "Sela is already running" naming the profile (EN/ID, in the
  profile's saved language), quitting on OK, Enter, Esc or close. Bringing
  the first window to the front would need inter-process signalling; Sela
  has no listeners, so it does not.
- A profile whose lock file cannot be created (read-only folder, no locks)
  still opens; autosave then reports why it is off.
- The standalone editor mode (`--library PATH`) takes no lock; SQLite's own
  locking covers its database writes.

**Local profile management is not delivered by M1-12.** There is exactly one
implicit profile per library folder. EasyWorship's Profiles menu (Default ✓,
Profiles Manager… to create, switch, rename and delete profiles; captured in
[operator-shell.md](operator-shell.md)) stays rendered but inert. Creating,
switching and renaming profiles, and what EW keeps per profile, belong to
ticket **M1-12b** (planned) in the backlog; the EW observation is
EW8-OBS-077 / PP-AUTO-10. Everything above (lock, `Recovery/`, settings) is
already per profile folder, so M1-12b adds the chooser and the switch, not a
new storage layout.

## Committed, dirty, uncommitted

- **Saved / dirty**: `Schedule::is_dirty` compares the entries with the
  baseline of the `.selaschedule` file (unchanged from M1-09).
- **Committed**: a revision is durably in `<profile>/Recovery/`.
- **Uncommitted**: the schedule differs from the newest committed revision.
  The marker `uncommitted: {after, since_ms, items}` in
  `Recovery/session.json` records that a crash now would lose work, so the
  prompt can say so. It is written at once (one small atomic write per
  transition, not per edit) when the schedule is uncommitted **and** has
  unsaved changes (`Schedule::is_dirty`): opening a saved `.selaschedule`
  (or saving) is committed later but never marked, because its content is
  in the file. An edit made while a commit is running is marked at once
  too; the worker handles jobs in order, so that marker lands after the
  commit and names it in `after`. The marker is withdrawn (`Unmark`) when it
  would no longer be true: the schedule went back to the newest commit
  (undo) or now equals its file (save). It is never withdrawn while a commit
  runs, since that commit may still fail. A marker that arrives for exactly
  the schedule a commit already holds is dropped.

A revision records the entries (pinned song revision, arrangement, title),
the schedule file path (UTF-8 only; a non-UTF-8 path is recorded as
untitled) and the file's baseline, so a restored schedule keeps its file
and only the unsaved part shows as unsaved. Revisions pin library song
revisions instead of embedding songs: committed song revisions are never
deleted or garbage-collected (docs/storage.md) and a schedule only pins
committed ones, so an autosave needs neither the storage worker nor any
database access.

## Files and commit protocol

- `Recovery/autosave-NNNNNNNNNNNN.selarecovery`: one JSON line
  (`format "sela-recovery"`, `version 1`, `sequence`, `saved_at_ms`, `path`,
  `items[{song_id, revision, arrangement, title}]`, `baseline[...]`) and a
  second line `sha256 <hex of the first line>`. A file whose checksum,
  structure or sequence (name versus body) does not match is **damaged** and
  never restored. At most 512 KiB, 32 items, 1 KiB titles, 4 KiB path.
- `Recovery/session.json`: `state` (`running` while an operator owns the
  profile, `clean` after a clean close, `offered` after a close with the
  previous run's recovery offer still unanswered), `started_at_ms`,
  `acknowledged` (the newest sequence the operator has seen) and the
  `uncommitted` marker.
- Every write is: create `<name>.tmp` (exclusive), write, `fsync` the file,
  rename over the final name, `fsync` the folder (Unix; Windows has no
  directory handle sync in `std`). A failure removes the temporary file and
  leaves the previous file untouched. Leftover `.tmp` files are removed at
  the next start; a leftover revision temporary file is reported as an
  interrupted autosave.

## Cadence and the UI thread

`autosave::Policy` runs on the poll loop's 16 ms tick with the GPUI
executor clock (`BackgroundExecutor::now`, the test scheduler's fake clock
in GPUI tests). A commit starts once the schedule has been unchanged for
2 s (`quiet`) or 10 s after the first uncommitted change (`max_delay`),
whichever comes first, so continuous editing still commits every 10 s. One
commit runs at a time; a failed commit retries after 30 s (`retry`); a job
the bounded queue (4) refuses is tried again on a later tick. Nothing is
written while the recovery prompt is open.

UI-thread cost per tick: one fingerprint of the schedule (ids, revisions,
arrangements, title lengths, path, baseline). Measured on this container in
the debug build (sela crate unoptimized) for a full 32-entry schedule with
1 KiB titles: 8.8 µs per fingerprint and 4.8 µs per commit snapshot (an
ad-hoc 10 000-iteration loop, not a committed benchmark; hashing whole
titles had cost 149 µs and was replaced by their lengths). All file work
runs on the `sela-autosave` thread; the live frame path is untouched (GPUI
test `autosave_never_holds_up_live_controls`: Go Live, Next and Black are
acknowledged while an autosave write is held at the disk).

## Retention

After every commit the oldest revisions are removed until at most 20 remain
and they total at most 4 MiB (`Budget`); the newest is never removed. A
single revision larger than the budget is refused (`Budget`: "the schedule
is larger than the autosave budget") and older revisions stay. Damaged
revisions count toward the budget and rotate out like the others. Revisions
are kept across sessions and after a clean close (they are the backups);
declining recovery keeps them too. A failed removal keeps the commit and
shows "Old autosave copies could not be removed".

## Recovery

At start the autosave worker scans `Recovery/` (off the UI thread). When
`session.json` says `running` (the lock guarantees no other operator is
running, so the previous run ended uncleanly), says `offered` (the previous
run was closed with this offer unanswered) or is damaged, it looks at
revisions newer than `acknowledged`, newest first, skipping damaged ones
(at most 64 read). It offers recovery when that revision has unsaved
changes, when the marker shows lost work (`after` is at least the newest
sequence on disk), or when newer revisions were damaged. A revision equal
to its saved file and no lost work needs no prompt.

The prompt (`Dialog::Recovery`, modal, focus on **Restore**) lists, one line
each: Sela did not close normally (or, after `offered`, that Sela was closed
before this question was answered); the last autosave's age, schedule name
and item count; that its changes were never saved to the file; that newer
changes (with their age and item count) were not autosaved and cannot be
recovered; how many newer copies were damaged; that an autosave was cut
off; that declined copies stay in the Recovery folder; and, when this run
cannot write its session (autosave off), that the answer cannot be recorded
and a restored schedule is not autosaved. With nothing to restore it shows
the loss with OK only.

- **Restore** replaces the (empty) schedule with the revision's entries,
  file and baseline, never touches Live, and commits the restored schedule
  again at once as this run's revision before acknowledging the previous
  run, so a crash right after the answer still finds it. Its songs are then
  loaded by the storage worker and get the same missing image/font check
  (M1-11b preflight report) as an opened schedule file.
- **Don't Restore** (Esc) acknowledges the previous run; its files stay.
- An unanswered prompt survives another crash: the new session keeps the
  old `acknowledged` value and marker until the answer.
- Closing Sela with the offer unresolved (prompt open, or the start-up scan
  not even finished: the close is queued behind it, so the scan's reply and
  any offer come first) commits nothing and acknowledges nothing; the store
  records the run as `offered`, and the next start asks again, opening with
  "Sela was closed last time before this recovery question was answered."
  (EW8-OBS-076, unobserved). Answering the prompt makes a later close a
  normal clean one.
- If this run cannot write `session.json` at start (disk full, read-only or
  locked) after an unclean run, the readable offer is still shown: Restore
  works read-only, nothing is committed or acknowledged, the footer says
  "Autosave is off: …", and the next start offers the same copy again.

## Close

Every close path (Quit, Ctrl+Q, the window's close button) passes the
existing unsaved-schedule guard and the settings write wait, then the
autosave flush: a `Close` job commits the schedule if it differs from the
newest commit and marks the run `clean`. The window closes when the reply
arrives, or after 5 s ("Finishing the autosave…" shows meanwhile) so a
stuck disk never traps the operator; the next start then treats the run as
unclean and offers what was committed. After Don't Save the final schedule
is still committed as a backup but never offered (the run is clean). An
edit made after a flush (a cancelled close) reopens the run on its next
marker or commit.

## Failure handling

| Failure | What the operator sees | Data |
| --- | --- | --- |
| Disk or quota full | "Autosave failed: the disk is full. Unsaved changes are not protected; Sela retries every 30 seconds." | no partial file; older revisions intact; retried |
| Read-only profile / folder | "Autosave is off: the profile folder is read-only." (at start) or the failed note | editing continues |
| Session unwritable at start after a crash | the recovery prompt still appears, with "Autosave is off (…), so this answer cannot be recorded …"; footer "Autosave is off: …" | read-only restore; offered again next start |
| Recovery file locked by another program | "Autosave failed: another program is holding a recovery file. …" | retried |
| Corrupt newest revision | recovery restores the newest valid one and says how many were damaged | damaged files kept until rotation |
| Revision over budget | "Autosave failed: the schedule is larger than the autosave budget. …" | older revisions intact |
| Old copies not removable | "Old autosave copies could not be removed: …" | the commit stands |
| Autosave while Live is on | nothing: work stays on the worker thread | Live receipts unaffected |
| Second instance | "Sela is already running" notice | profile untouched |

## Known gaps

- Older revisions (rotation backups, declined copies) can only come back
  through the prompt after an unclean exit; there is no in-app browser of
  the Recovery folder yet (M1-13 owns explicit backup/restore).
- Restore replaces the schedule shown at that moment without the unsaved
  guard. The prompt opens a few milliseconds after start, before anything
  can be edited, so in practice it replaces the empty start schedule.
- Local profile management (create, switch, rename): M1-12b, not built.
- An edit that is undone while a commit is running keeps its marker until
  the next step after that commit (conservative, never a false "no loss").
- The folder `fsync` after a rename is Unix-only; on Windows the rename's
  durability across power loss is unqualified (PP-AUTO-08).
- The already-running notice does not bring the first window forward.

## Evidence

- U/I (`cargo test --lib autosave profile`): revision codec with every
  truncation and bit flip refused, fingerprint/snapshot, first run, clean
  close, unclean offer, lost marker, late marker, saved-only revision,
  corrupt latest with fallback, mis-named revision, interrupted commit,
  damaged session, unanswered offer, a close with the offer unresolved
  (`offered`), an unwritable session after a crash still returning the
  offer, a withdrawn marker, restore commit, decline, work after a
  flush, rotation by count and bytes, budget refusal, disk full, read-only,
  locked, rotation failure, error mapping, cadence (quiet, max delay,
  retry, revert with unmark, unsaved-only marking, marking during a running
  commit, unqueued, restore/flush), worker non-blocking and bounded
  queue, profile lock (same process, child process killed, read-only), and
  **S03** with real child processes killed by SIGKILL before, during and
  after a commit (`autosave::tests::s03`).
- GPUI (`src/tests/autosave_recovery.rs`, `src/instance.rs`): see the work
  log record for the list.
- Native X11: `scripts/autosave-x11.py` (first run with Live on, second
  instance, SIGKILL after a newer edit, the prompt closed unanswered and
  recorded `offered`, the prompt again with the "closed before answered"
  wording and Restore, clean quit, clean start).
- Windows: `scripts/autosave-windows.py`, **not run** (no Windows host).

## Upstream GPUI studied (zed `a846890`)

- `crates/gpui/src/executor.rs`: `BackgroundExecutor::now` reads the
  scheduler clock, `timer` waits on it, `advance_clock` (test support)
  moves it; used as the injected cadence clock.
- `crates/gpui/src/platform/test/platform.rs`: `quit` is inert under test,
  hence the injectable dismiss closure of the already-running notice.
- `crates/gpui/src/elements/div.rs`: `aria_label` / `aria_value`;
  `accesskit_consumer` 0.38 `Node::label_comes_from_value` (Role::Label is
  named by its value on UIA and AT-SPI), so the prompt body and notice set
  both.
- `crates/gpui/src/global.rs` (`Global`): `AutosaveSetup` and the held
  profile lock live as app globals.
