# M1-05b/c native song authoring

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
  **Ctrl+Z / Ctrl+Shift+Z** (Cmd on macOS) undo/redo the complete document in
  chronological order, including metadata, lyrics and Add/Remove, even after
  switching sections. **Discard edits** restores the complete last-loaded/saved song.
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

Still open: native qualification of document undo, arrangement/pagination/themes, indexed search,
schedule UI, autosave/recovery, connected live output, installed-reference parity,
actual platform IME/accessibility, Windows/macOS/Wayland/high-DPI/physical GPU and
display qualification, performance measurements, and production installation.

## M1-05c document history contract and scoped checklist

State: **implemented-unqualified** on `ticket/m1-05-undo`, based on LOCAL main
`2b62c77`. No layout/toolbar redesign, new visible controls, Painter or reference
parity claim. These shortcuts and history boundaries are provisional Sela policy;
installed EasyWorship 8.0.49 grouping, selection restoration and save boundaries
remain unknown.

- One history owner in `Library`; its snapshots contain the whole original-byte
  `Song` and selected section index, not `Version`, saved baseline or UI fields.
  Field user edits report synchronously through `use_document_history`; the
  callback updates the owner without reading the currently borrowed field.
  Ordinary GPUI observation remains for status/error notification only, not
  chronological recording. This avoids notification coalescing losing edits.
- Maximum **64 snapshots combined across undo/redo**, **8 MiB accounted combined
  snapshot bytes** (UTF-8 string lengths plus document/section struct sizes).
  Oldest undo entries are evicted first, then furthest redo entries if needed.
  A snapshot larger than the budget is evicted; no truncation of document text.
  Current draft/baseline, field buffers, allocator overhead and temporary clones
  are outside this retention budget; this is not a hard RSS/performance claim.
- Each content-changing native replacement/paste/cut/preedit callback or
  structural operation is one step. No typing/composition coalescing yet.
  No-op replacements, caret/selection movement, focus and Previous/Next are not
  edits and do not erase redo. A new content edit truncates redo. Undo/redo loads
  retained fields, clears composition/errors and resets their carets to zero;
  it restores the affected section, not historical text selection/scroll.
- **Save and Duplicate retain both history stacks**, advancing the saved baseline
  and immutable expected-head Version only on successful reply. Undo afterward
  is dirty relative to that new baseline and does not roll back durable revisions
  or identity. Redo to the baseline becomes clean again. Saving while a redo
  branch exists retains it. Successful save/duplicate does not reload fields or
  reset the selected section/carets. Busy undo/redo is ignored.
- **New, successful Load, Discard edits and successful Delete reset history**.
  Discard restores the saved baseline/Version; loading a different song cannot
  undo into the previous song. Invalid load, rejected input, failed structural
  edits, failed submission and failed/conflicting saves preserve document/history
  and baseline/Version. Add validates the post-operation envelope before applying;
  128-section/empty-remove no-ops leave fields untouched. Dirty/pending close and
  replacement guards remain authoritative; history alone does not imply dirty.
- Standalone `TextInput` and `input_check` retain their original field history.
  Only owned song fields disable local snapshots and propagate semantic Undo/
  Redo to the contextual parent handler. No global key matching.

Upstream review on 2026-10-04: `git ls-remote https://github.com/zed-industries/zed
HEAD` returned the existing pin `a84689073d296dfd39987bc7dd478e43ef76d83a`.
Inspected Apache framework `crates/gpui/src/app/context.rs` (weak entity,
observe/subscribe, queued `emit` effects), `crates/gpui/src/app.rs` (`propagate`),
`crates/gpui/src/app/entity_map.rs` (`WeakEntity::update`), and
`crates/gpui/examples/input.rs`. Chose an original synchronous ownership seam
instead of delayed content events because effects can run after navigation within
one app update. No GPL application components/assets or dependency changes.

- [x] Chronological asymmetric multi-section edit/Add/Remove/undo/redo; cross-section
  navigation and selection preserve history; controls and fields resolve actions.
- [x] Saved/duplicated baseline and immutable Version survive undo/redo; dirty
  close, pending ignore, validation/conflict/unavailable-save retention tested.
- [x] Redo branching, zero sections, 128 sections, post-Add payload failure,
  Unicode/combining/emoji/CRLF, count and combined byte eviction covered.
- [x] Synchronous content-only field seam and unchanged standalone history tested.
- [ ] Parent serial native driver replay and visual content inspection (not run
  in this worktree to avoid focus interference on shared `:99`).
- [ ] Installed-reference, Windows/real IME/accessibility and measured performance.

Parent replay after merging (run serially, build the merged source immediately
before running; do not reuse a parallel worker's binary):

```sh
export CARGO_TARGET_DIR=/home/user/workspace/repo/target
cargo build --locked --bin sela -j2
SELA_BINARY="$CARGO_TARGET_DIR/debug/sela" DISPLAY=:99 VK_DRIVER_FILES=/dev/null \
  python3 scripts/song-library.py
```

The extended driver checks save-then-undo dirty WM-close, metadata-focus undo of
the latest lyric edit, control-focus structural undo/redo, a saved removal's
independently decoded one-section payload, and restoration of both asymmetric
sections after undo across another Save. Inspect `saved-document-undo.png`,
`removed-section.png`, `restored-section.png`, `saved.png` and existing compact/
reopened captures. Also manually navigate Previous/Next between lyric edits,
undo each in chronological order, redo, branch with new typing, remove the final
section at Remove-button focus and undo to restore it. No new appearance change;
native action/focus/content qualification remains pending, not visual redesign.

## M1-05d — Reference-directed native authoring correction

Implemented-unqualified on `ticket/m1-ui-reference`, LOCAL base `231fffb`.
Original GPUI editor, not Painter or a web substitute. Historical official
Support 7 articles and the working editor screenshot were personally read and
inspected; evidence/version limits are D-UI-02 in the reference ledger. Current
8.0.49 interactions, typography, dimensions and pagination remain unobserved.
Upstream Apache GPUI focus/control/Div/Window/tab-stop APIs and license inspected
at current/pinned `a84689073d296dfd39987bc7dd478e43ef76d83a`; exact paths in
the M1-02c shell note. No Zed GPL application UI, reference song/assets or new
dependencies copied.

The blank editor has Title upper-left, selectable draft sections and Words/Slides
on the left, a local draft preview on the right, Inspector top-right, bottom-left
Add/Remove and bottom-right OK/Cancel. Words edits the selected label/lyrics;
Slides shows original text-only section thumbnails (first four lines), not
rendered audience slides. Native fields retain document history across view
changes. The preview displays logical draft lines, with scrolling; **not WYSIWYG,
font fitting, arrangement/pagination, theme or rendered-output matching**.
Inspector replaces the right preview with the three existing metadata fields.
Unsupported formatting/theme/media/arrangement controls are not faked.

Explicit **Library** opens the existing bounded worker-backed ID-order saved-song
selector, separate from Words; it temporarily replaces the preview, not the
document. Load/refresh/paging, New, Save-without-close, Duplicate, confirmed
tombstone Delete, Discard edits and Previous/Next remain reachable. Dirty song
replacement still requires save/discard; every hidden field remains retained and
validated. At 720×440, the redundant header status is omitted to keep Inspector
visible; status remains below the toolbar. Left/right panes scroll, and bottom
Add/Remove/OK/Cancel stay fixed. Title/section rows scroll within the left pane.

OK submits the existing immutable expected-head save, then waits for the actual
`Reply::Saved` receipt before closing. It never closes on validation, submission,
storage or stale-head conflict failure. Cancel uses the existing dirty/pending
guard. Pending operations hide mutable controls and intentional close waits.
Save retains undo/redo/selection and advances only the committed baseline and
Version; OK uses that same path. No close-on-submit, new synchronous UI I/O,
history reset or database/schema change introduced. Validation status is refreshed
after a new edit without claiming that the new draft has already validated.

- [x] Clean blank editor from both main launch routes, explicit Library selector.
- [x] Title/Words/Slides/selected section/local preview/Inspector/bottom controls.
- [x] Preserve M1-05c chronological history, save/load/duplicate/delete/close guards.
- [x] New test covers modes/no history churn, section selection, invalid OK,
  conflicting OK retaining draft, and pending Cancel; existing failure tests pass.
- [x] Both native drivers replayed; independent SQLite bytes, metadata, two
  asymmetric sections, save-undo dirty close, structural undo across Save,
  reopen, compact states and successful OK close checked. This executes the
  previously pending M1-05c serial native replay; no history assertions removed.
- [x] Inspect actual Words, Slides, Inspector, validation/dirty-close and compact
  Words/Inspector/Library/scrolled captures, plus restored/reopened content.
- [ ] Installed8.0.49/Windows/real IME/accessibility/DPI/physical output/performance.

Executed on Debian12 x64 orb, Xvfb/Openbox software GL, shared services unchanged:

```sh
export CARGO_TARGET_DIR=/home/user/workspace/repo/target
# Root src/main.rs and src/lib.rs touched before initial compilation for cache drift.
cargo test --locked --all-targets -j4             # 96 passed, 1 child fixture ignored
cargo clippy --locked --all-targets -j4 -- -D warnings
cargo fmt --all -- --check
uvx ruff check scripts/operator-shell.py scripts/song-library.py
git diff --check
cargo build --locked --bin sela -j4
DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/operator-shell.py
DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/song-library.py
DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh "$CARGO_TARGET_DIR/debug/sela"
```

All pass; new test names and all five arrangement-domain tests observed, avoiding
stale shared-worktree cache evidence. Native captures are actual client crops
under `.amp/in/artifacts/song-library/`: `initial`, `draft` (Words), `slides`,
`inspector-filled`, `validation`, `unsaved-close`, `saved-document-undo`,
`removed-section`, `restored-section`, `reopened-chorus`, `compact`,
`compact-inspector`, `compact-library-selector`, `compact-scrolled` PNGs.
Initial native driver still used old form coordinates; retained independent
payload/undo/close assertions and corrected replay for actual Inspector/Words.
Initial hidden-metadata focus tests now reveal Inspector before native field
edits; no failure expectations weakened. Native OK independently verifies
committed bytes after process exit. Real platform qualification remains open.
