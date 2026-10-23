# Portable bundles, backup and restore (M1-13)

State: **implemented-unqualified** (stream `bundle-backup-restore`,
2026-10-23). Linux U/I, GPUI, fuzz and `scripts/bundle-x11.py` checks pass;
`scripts/bundle-windows.py` is NOT RUN (no Windows host) and no EasyWorship
behavior was observed for this ticket (EW8-OBS-BUN-1..7 are UNOBSERVED best
guesses, PP-BUN-01..08 in [parity-pending.md](parity-pending.md)).

Code: `src/bundle.rs` (export, import, backup, restore, worker),
`src/bundle/archive.rs` (strict ZIP subset), `src/bundle/manifest.rs`
(versioned manifest), `src/images.rs` (`adopt`, `check_embedded`),
`src/operator/bundles.rs` (menus, dialogs, results). No GPUI types in the
domain code; every file operation runs on the `sela-bundle` worker thread.

## What exists

| Bundle | Extension | Made by | Opened by |
| --- | --- | --- | --- |
| Standalone schedule | `.selapackage` | File › Export Standalone Schedule… (row observed in EW8-OBS-053; was inert) | File › Open Schedule (Ctrl+O) |
| Profile backup | `.selabackup` | Profiles › Back Up Profile… (Sela row, UNOBSERVED placement) | Profiles › Restore Profile… (Sela row) |

EasyWorship's own file extension and dialogs for standalone schedules and
backups were not observed (EW8-OBS-BUN-1, -5); Sela's extensions are its
own. Sela does not read EasyWorship packages.

### Standalone schedule

Carries the schedule exactly as File › Save would write it (the
`.selaschedule` document with each item's pinned song revision and theme
snapshot, version 4 or 5) plus, by default, every profile image the songs
and theme snapshots use, byte for byte, so their SHA-256 pins hold on the
other machine. It is not a Save As: the open schedule keeps its file name
and unsaved state.

- **Images**: an image is embedded only when the profile copy still matches
  its pin; a missing, changed or unreadable one is listed in the manifest
  as not embedded and named in the result ("Not included because they are
  missing or changed in this profile …"). `ExportOptions { embed_images:
  false }` lists every image without carrying any. It is library API only
  for now: the menu always embeds, and an operator choice (an export
  options dialog, shaped after PP-BUN-01) is the open M1-13 item "Operator
  export options".
- **Fonts are never embedded.** Sela cannot tell from a font file whether
  its license allows passing it on (OS/2 embedding bits govern document
  embedding, not redistribution), and the only fonts Sela knows to be
  redistributable, the bundled DejaVu faces, ship with every Sela. Each
  named family is listed with whether it was installed on the exporting
  machine; the result tells the operator which fonts the other computer
  needs.
- **Scripture**: items whose translation is not redistributable
  (`Snapshot::redistributable == false`, M4-03) are left out, listed by
  reference and translation, and the remaining items keep their order.
- **Opening** (Ctrl+O on a `.selapackage`): the whole bundle is validated
  first (below). Then each embedded image is stored in
  `<profile>/Resources/Images/` with `images::adopt`: identical bytes under
  the name (or a numbered variant) are reused; a different image is never
  replaced and the copy gets the next free "name (n).png", compared
  ignoring letter case so a profile moved to Windows or macOS keeps
  working; the schedule's references follow the new name. The schedule is
  written as a new file `<profile>/Schedules/<title>.selaschedule` ("(n)"
  when taken, never replacing a file) and opened like any schedule file,
  so its songs join the library as hidden revisions and the preflight
  report (M1-11b) runs. The package notes appear at the top of that report
  ("From the standalone schedule"), or on their own when nothing needs
  attention: images added, reused or renamed, images the package does not
  carry, fonts it needs, items the sender left out.
- **Repair**: a missing image is repaired with the report's Locate… (hash
  checked relink, M1-11); a missing font is reported as "DejaVu Sans is
  used instead" and never claimed exact (M1-05g2b font gate).

### Profile backup

Holds a verified copy of `library.sqlite` (SQLite backup API through
`Repository::restore_new`, which refuses foreign schemas and checks every
revision), `settings.txt`, `logo.txt`, `Resources/Images/*`,
`Schedules/**/*.selaschedule` (four folder levels) and imported translations
`Scripture/*.selabible`. It leaves out `sela.lock`, `Recovery/` (autosave
copies of the moment), the bundled KJV and WEB translations (Sela rebuilds
them), dot files and anything else; the result lists what was skipped and
why (link, too large, not a profile file, unreadable). The open schedule's
unsaved changes are not in it, and the result says so.

- Recent-schedule entries inside the profile are stored profile-relative
  and point into the restored profile; entries elsewhere are left out (they
  name places on this machine) and counted.
- Symbolic links are never followed (`O_NOFOLLOW` on Unix, link check
  everywhere), so a link cannot pull files from outside the profile into a
  backup.
- **Restore** never writes into an existing file or folder. Profiles ›
  Restore Profile… asks for the backup, then for the name of a **new**
  profile folder. Everything is extracted and verified into a private
  `.sela-restore-*` folder beside it (the library again through
  `restore_new`), and one rename makes the folder appear. The open profile
  is not changed; the result names the `--operator-library` path to start
  Sela with the restored profile. Switching profiles from the menu belongs
  to M1-12b (Profiles Manager).
- Restoring a backup from a newer Sela (library schema above
  `storage::newest_schema()`, or a newer manifest) is refused before
  anything is written.

## Container: a strict ZIP subset

Every bundle is a ZIP archive any unzip tool can list (Python's `zipfile`
reads and CRC-checks them in `bundle-x11.py`). Sela writes and reads only:

- one disk, no ZIP64 (all offsets and sizes below 4 GiB), no encryption,
  no data descriptors, methods 0 (stored: images) and 8 (deflate: JSON,
  SQLite, text), UTF-8 names (flag bit 11), fixed 1980-01-01 times, regular
  file entries only; the manifest is the last entry;
- names that pass `archive::safe_name`: relative, `/`-separated, at most 240
  bytes and 8 components; no empty, `.` or `..` component, no leading `/`,
  no `\`, `:`, `<>"|?*` or control characters, no component starting with a
  space or ending with a dot or space, no Windows device names (`CON`,
  `NUL`, `COM1`, …, with or without an extension);
- no two names equal ignoring case, and no file that is also another
  entry's folder;
- every local header must repeat its central record, entries may not
  overlap each other or the central directory (overlapping entries are the
  "better zip bomb"), and the central directory must end exactly at the end
  record;
- links, folders and special files (Unix file-type bits other than a
  regular file, the MS-DOS folder bit) are refused.

Size bounds are checked from the central directory before any data is
read, and again while inflating: output past an entry's declared size
stops extraction at once (nothing past it is written), and the CRC-32 and
the declared size must match at the end.

| Bound | Standalone schedule | Profile backup |
| --- | --- | --- |
| Entries | 514 (schedule, manifest, 512 images) | 16,384 |
| Largest entry | 8 MiB (schedule file and image caps) | 256 MiB (library; 128 MiB translation, 8 MiB image or schedule, 1 MiB settings, 4 KiB logo) |
| All entries | 1 GiB | 3 GiB |
| Manifest | 4 MiB | 4 MiB |

There is no compression-ratio limit: an empty-page SQLite library or a long
run of repeated lyrics legitimately compresses past 100:1, and the declared
size bound already stops a bomb at its declared size.

## Manifest (`manifest.json`, version 1)

```json
{
  "format": "sela-bundle",
  "version": 1,
  "kind": "schedule",
  "generator": "Sela 0.1.0",
  "title": "Sunday",
  "files": [
    {"path": "schedule.selaschedule", "size": 2871, "sha256": "…", "role": "schedule"},
    {"path": "images/harbor.png", "size": 1044, "sha256": "…", "role": "image"}
  ],
  "images": [
    {"name": "harbor.png", "sha256": "…", "embedded": true},
    {"name": "lantern.png", "sha256": "…", "embedded": false}
  ],
  "fonts": [{"family": "Sela Missing Serif", "embedded": false, "installed": false}],
  "omitted": [{"kind": "scripture", "title": "John 3:16 (NIV)", "translation": "NIV", "reason": "restricted"}],
  "library_schema": 8
}
```

- `kind` is `schedule` or `profile`; opening a bundle of the other kind says
  where it belongs ("… is a profile backup; use Profiles › Restore
  Profile…").
- `files` lists **every** other entry exactly once with its size and
  SHA-256; an unlisted entry, a missing one, a size or hash mismatch is
  "damaged". Each role has a fixed place: `schedule.selaschedule` and
  `images/<name>` in a standalone schedule; `library.sqlite`,
  `settings.txt`, `logo.txt`, `Resources/Images/<name>`,
  `Schedules/…/*.selaschedule` and `Scripture/<id>.selabible` in a backup.
  Image names must be valid profile image names (PNG/JPEG).
- `images` names every pinned image the schedule uses; an embedded one must
  be the listed `images/<name>` file with the same SHA-256, and every image
  file must be described by one.
- `fonts[].embedded` must be `false` in version 1; `installed` is `null`
  when the exporting machine had not scanned its fonts yet.
- `library_schema` (backups) is the library's `PRAGMA user_version`.
- Readers ignore unknown fields and refuse a `version` they do not know
  ("made by a newer Sela … update Sela"). A later version that adds a
  feature older readers cannot honor must bump `version`.

## Validation before mutation

Opening a standalone schedule reads and checks the whole archive, the
manifest, every entry's CRC-32 and SHA-256, the schedule document
(`schedule_file::decode`, so a newer schedule version is refused) and each
embedded image's header (`images::check_embedded`: PNG/JPEG, still, size
and dimension bounds) before the profile changes. Only then are images
adopted and the schedule file written, each atomically (temporary file,
then a no-replace link). A failure in that phase (disk full) can leave
already stored images behind, which are new files only and harmless.

The chosen package or backup is opened without waiting (`O_NONBLOCK` on
Unix) and refused as "not a Sela bundle" unless it is a regular file, so a
named pipe or device given a bundle's name cannot hold the worker inside
`open(2)` where Cancel would never reach it (and Quit would wait for good).

Restore writes only inside its private staging folder until the final
rename; an interrupted restore (crash, power cut, Cancel) leaves at most
that `.sela-restore-*` folder, which Sela never reads and the operator may
delete. Exports and backups are written to a `.sela-export-*`/
`.sela-backup-*` temporary beside the destination and renamed over it
only when complete (the Save dialog already asked before an existing file
is replaced), so a failed or cancelled job keeps the earlier file.

## Operator UI

- File › Export Standalone Schedule… is enabled when a library and
  profile are open, the schedule has items and no bundle job runs. Its Save
  dialog suggests `<schedule name>.selapackage` beside the schedule (else in
  `Schedules`); the extension is added once.
- Profiles › Back Up Profile… (suggests `<profile folder> YYYY-MM-DD
  .selabackup` beside the profile folder, so a backup is never inside the
  profile it backs up) and Restore Profile… (an Open dialog for the backup,
  then a Save dialog for the new folder's name).
- While a job runs, a modal dialog shows what is being written and a
  Cancel button (Esc); the job stops at its next check and leaves nothing
  half-written ("Cancelled; nothing was changed"). A job that finished
  before it saw the cancel shows its result instead; for a standalone
  schedule that means it is **not opened** (the Cancel closed the dialog,
  so the operator may have edited the current schedule since): the
  complete file stays in `Schedules` and the result says "Cancel came too
  late: … It was not opened; open it with File › Open Schedule." Quit waits
  for the job (the footer says why). One job runs at a time; the worker is
  free again as soon as its reply is shown.
- A job that panics (a bug in Sela or a library it uses) is caught on the
  worker: its temporary file or staging folder is removed while the panic
  unwinds, the result says "… stopped because of an internal error in
  Sela", and the worker takes the next job, so the menu and Quit are never
  held by it. Images an import stored before the panic stay, as for a disk
  full error.
- Library failures are explained in the operator's language ("it is busy;
  wait a moment, then try again", "it is damaged", "it is not a Sela
  library, or a newer Sela made it", "it could not be read or written";
  disk full has its own line), never as an internal error name.
- Results and errors appear in the same dialog (heading per job, one list
  item per line, errors in the error color, OK). Every line is an
  AccessKit list item with id `bundle-result-<n>`; the package notes inside
  the preflight report are `preflight-package-<n>`.
- Strings are in English and Indonesian (`Msg::Bundle*`, `BarBackupProfile`,
  `BarRestoreProfile`).

## Recovery steps

- **"… is damaged or incomplete"**: the file changed after it was written
  (copy error, truncated download). Export or back up again from the
  source machine; nothing on this machine was changed.
- **"… was refused because it could write outside its place …"**: the
  archive was not made by Sela or was tampered with; nothing was written.
- **"… made by a newer Sela"**: update Sela on this machine.
- **Missing images after opening**: Locate… in the report with the
  original files; only a file whose bytes match the pin is accepted.
- **Missing fonts**: install the font (with a license that allows it) and
  reopen the schedule; until then DejaVu Sans is used and the report says so.
- **Restore refused, "already exists"**: choose a new folder name; Sela
  never merges into or replaces a folder.
- **Leftover `.sela-restore-*`, `.sela-backup-*`, `.sela-export-*`**: left
  only when Sela stopped during a job; safe to delete.
- **Using a restored profile**: start Sela with `--operator-library
  "<folder>/library.sqlite"` (shown in the result) until M1-12b adds
  profile switching.

## Licensing and privacy

Portability does not imply permission to redistribute. Sela never copies
font files, leaves out scripture from restricted translations in
standalone schedules, and carries only images the operator imported into
the profile; the operator is responsible for the rights to share the
images and lyrics in a standalone schedule. A profile backup is a private
copy of the operator's own data: it includes imported (possibly
restricted) translations and every song and schedule, and should be stored
like the profile itself.

File permissions on Unix (Linux, macOS): a profile backup is created
owner-only (`0600`), like its staging files. A standalone schedule is meant
to be handed to other people, so it is created with `0666` minus the
user's umask (usually `0644`, readable by other local users and by a
Samba share's mapped account); copy it elsewhere or `chmod` it to narrow
that. Windows files take the folder's access list in both cases. Bundles
hold no absolute paths: a backup's title
is the profile folder's name, and recent schedules outside the profile are
dropped from it.

## Tests and evidence

| Label | Where | What |
| --- | --- | --- |
| U | `src/bundle/tests.rs` | safe names; round trip of stored and deflated entries; writer refusals; hostile names, links, folders, CP437 names; duplicate and case/folder conflicts; encryption, data descriptors, other methods, ZIP64 extra; gaps, truncation, local/central mismatch, two sizes; overlapping entries; declared-size bounds; a 64 MiB deflate bomb stops at its declared size; CRC, short and damaged streams; manifest round trip, unknown fields, newer version, traversal and misplaced paths, duplicates, bad hashes, unlisted images, embedded fonts; layout rules |
| I | `src/bundle/tests.rs` | export/import across machines (different depths, restricted scripture left out with order kept, fonts listed, preflight explains), name collision ignoring case never replaces and references follow, export without images then relink repair, tampered packages (flipped image, extra/missing entry, size, junk/newer manifest, no manifest, changed schedule, fake image) change nothing, hostile archives write nothing outside, cancelled export keeps the old file, backup/restore round trip with relative recent schedules, links never followed, restore never writes into an existing place, damaged/newer backups restore nothing, an aborted restore (child process, `SIGABRT` after 1 and 4 files) leaves existing data intact, worker one job at a time; review round 1: a named pipe named `.selapackage`/`.selabackup` is refused at once (`a_named_pipe_given_a_bundle_name_is_refused_without_waiting`), a panicking job replies `Internal` and the worker takes the next (`a_panicking_job_replies_failed_and_the_worker_takes_the_next`), a polled reply never leaves a stale busy (`a_polled_reply_is_never_followed_by_a_stale_busy`, 2000 rounds), package `0666 & !umask` and backup `0600` (Linux) |
| I (S04) | `tests/bundle_s04.rs` | export on profile A (DejaVu Sans installed), open on a clean profile B at another path with no fonts: preflight reports the font and the image the bundle could not carry, an impostor file is refused, Locate… repairs, the font falls back with a warning, installing it makes the report clean; the backup restores beside B without touching B |
| Fuzz | `src/fuzz/portable.rs` | mutated packages and backups: the reader never panics or writes past a declared size; imports either succeed with only images and one schedule inside the profile or leave it empty with nothing beside it; restores either produce an openable library or leave nothing; mutated manifests decode and round trip or are refused |
| GPUI | `src/tests/bundle_transfer.rs` | `export_standalone_schedule_writes_a_package_without_saving_the_schedule`, `opening_a_standalone_schedule_unpacks_it_and_preflight_explains_what_is_missing` (incl. Locate… and the keyboard after the report closes), `damaged_or_misplaced_bundles_change_nothing_and_say_why`, `backup_and_restore_from_the_profiles_menu_never_touch_the_open_profile`, `a_running_job_can_be_cancelled_and_holds_back_quit`, `an_import_that_finishes_after_cancel_never_replaces_the_schedule`, `a_job_that_panics_reports_failure_and_frees_the_worker`; `operator::bundles::tests::library_failures_are_explained_in_the_operator_language` (EN and ID) |
| N (Linux) | `scripts/bundle-x11.py` | see its docstring; PASS on Xvfb `:122` |
| N (Windows) | `scripts/bundle-windows.py` | NOT RUN: no Windows host |

Not covered: real disk-full and read-only volumes during an import or
restore (the error paths are typed and unit-tested only through
cancellation and refusals), FAT/exFAT volumes without hard links (the
reserve-and-rename fallback is not exercised on Linux tmpfs), NFC/NFD
Unicode name normalization (names are compared after lowercasing only),
case-insensitive volumes (the collision rules are platform independent and
tested, but no case-insensitive filesystem was mounted), and the EW
reference behaviors listed above.
