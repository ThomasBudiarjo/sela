# M1-05b native song authoring

Implemented-unqualified under the owner's UI-first sequencing exception. This
is a usable offline song editor, not a service scheduler or audience controller.
The layout and shortcuts are provisional until observed in EasyWorship 8.0.49.
No Painter assets or Zed application UI components are used.

## Use

Build with `cargo build --locked --bin sela`. Launch `target/debug/sela`, then
choose **Songs → Open song library**, or use
`target/debug/sela --library /absolute/path/to/library.sqlite` for a separate
profile. The storage worker creates missing parent directories.

The default is `%LOCALAPPDATA%\sela\library.sqlite` on Windows or
`$XDG_DATA_HOME/sela/library.sqlite` on Linux (fallback
`$HOME/.local/share/sela/library.sqlite`). Windows execution is not qualified.
Back up the database while the editor is closed; automated backup/recovery and
destructive migrations are not implemented. Never use valuable original files
as test fixtures.

- Enter title, authors, copyright, license identifier and labeled lyric sections.
  New songs start with one empty Verse 1; empty or zero sections are retained.
- **Save / Ctrl+S** commits a new immutable revision. **Duplicate** saves the
  current draft under a new identity, even if the title is unchanged.
- **Previous / Next / Add section / Remove** change the authored sections.
  Field undo/redo is bounded; section removal/reordering has no document-level
  undo yet. **Discard edits** restores the complete last-loaded/saved song.
- **Delete**, then **Confirm delete**, tombstones a clean saved song. Existing
  schedule snapshots retain their original immutable content.
- Selecting another song or New refuses to replace unsaved edits. Ctrl+Q and
  window-manager close both guard pending operations and dirty drafts. Choose
  Keep editing, Discard and close, or Save and then close again.
- Concurrent edits are not silently overwritten: a stale save retains the draft
  and offers recovery through Duplicate or discard/reopen. Pending operations
  temporarily hide editing controls so late replies cannot discard newer typing.

The list is bounded to 128 entries per ID-ordered page, not title search/ranking.
After the final page, Refresh / first page returns to the start. Fields support
native clipboard, selection, Unicode and multiline input as documented in
[text-input.md](text-input.md). Lyrics preserve LF/CRLF and blank/trailing lines;
they scroll instead of soft-wrapping. Songs exceeding 4096 logical lyric rows per
section or containing multiline metadata cannot be loaded into this provisional
editor; the original database content remains unchanged. Total encoded song
limit is 256 KiB. Fields have the storage metadata limits.

## Ownership and reference patterns

`src/song_library.rs` owns retained field entities, dirty/close state and one
pending worker operation. All SQLite/filesystem work is on `storage::Worker`;
the UI polls at 40 ms using the background executor timer. No renderer state is
changed. Operator Ctrl+Q closes that window only, so it cannot bypass another
song window's dirty guard.

Inspected pinned/current Zed `a84689073d296dfd39987bc7dd478e43ef76d83a`:
`crates/gpui/src/window.rs` (`on_window_should_close`, focus traversal),
`crates/gpui/src/app/context.rs` (spawn/observe), and Apache GPUI examples/input.rs
and examples/tab_stop.rs (native field and focus contracts). See
[text-input.md](text-input.md) for component provenance and notices. This editor
is original code; it does not copy GPL application components.

## Reproduce available checks

```sh
cargo test --locked --all-targets -j 8
cargo clippy --locked --all-targets -j 8 -- -D warnings
cargo build --locked --bin sela -j 8
DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/song-library.py
DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/operator-shell.py
DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh "$PWD/target/debug/sela"
```

The last three use an existing Xvfb/Openbox/compositor session; `:99` and the
software-GL override are orb test settings, not deployment requirements. The
song driver accepts `SELA_BINARY=/absolute/path/to/sela` for installed-binary
checks. It types original fixture lyrics through native events, independently
decodes the committed SQLite bytes, checks blank-title rejection, dirty WM close,
keep-editing/discard-close, restart and compact resize/scroll. Screenshots are
under `.amp/in/artifacts/song-library/`; inspect them, since capture alone is not
verification. Profiles and runtime logs are temporary and removed on exit.

Rust integration tests cover asymmetric sections/Unicode, pending-close refusal,
duplicate/delete/history, stale-save draft retention, incompatible-load atomicity,
catalog pagination/current revisions/tombstones and parent-directory failure.
Storage tests separately cover locked/full database rollback and snapshot safety.

Still open: full editor undo, arrangement/pagination/themes, indexed search,
schedule UI, autosave/recovery, connected live output, installed-reference parity,
actual platform IME/accessibility, Windows/macOS/Wayland/high-DPI/physical GPU and
display qualification, performance measurements, and production installation.
