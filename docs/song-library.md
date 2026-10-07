# M1-05b/c/d native song authoring

Implemented-unqualified under the owner's UI-first sequencing exception. This
is a usable offline song editor, not a service scheduler or audience controller.
The layout and shortcuts are provisional until observed in EasyWorship 8.0.49.
No Painter assets or Zed application UI components are used.

## Use

Build with `cargo build --locked --bin sela`. Launch `target/debug/sela`, then
choose **New → New Song** or **Songs → + New Song**. In the editor, **Library**
opens the saved-song catalog; **Inspector** reveals metadata. Use
`target/debug/sela --library /absolute/path/to/library.sqlite` for a separate
profile. The storage worker creates missing parent directories.

The default is `%LOCALAPPDATA%\sela\library.sqlite` on Windows or
`$XDG_DATA_HOME/sela/library.sqlite` on Linux (fallback
`$HOME/.local/share/sela/library.sqlite`). Windows execution is not qualified.
For manual file backups, close the editor first and preserve any SQLite sidecars;
do not copy a live database file alone. The [storage worker](storage.md) now has
verified online backup and restore-to-new-profile APIs, but no UI backup chooser,
automatic rotation or recovery workflow. Schema-1 open now has a worker-owned
verified-backup migration gate (see storage.md). Never use valuable
original files as test fixtures.

- Enter the title in the toolbar and slides in **Words** (see M1-05e below);
  authors, copyright and license identifier are in **Inspector**. New songs
  start with one empty, unlabeled slide; empty or zero slides are retained.
- **Apply / Save / Ctrl+S** commits a new immutable revision. **Duplicate** saves the
  current draft under a new identity, even if the title is unchanged.
- **OK** saves and closes only after a successful commit receipt. Validation,
  conflicts and storage failures keep the draft open. **Cancel** uses the same
  pending-operation and unsaved-change guards as window close.
- **+ / −** below Words append a slide or remove the current one.
  **Ctrl+Z / Ctrl+Shift+Z** (Cmd on macOS) or toolbar **Undo / Redo** undo/redo
  the complete document in chronological order, including metadata, lyrics,
  splits, joins and +/−, even after moving between slides. **Discard edits**
  (Library pane) restores the complete last-loaded/saved song.
- **Delete**, then **Confirm delete**, tombstones a clean saved song. Existing
  schedule snapshots retain their original immutable content.
- Selecting another song or New refuses to replace unsaved edits. Ctrl+Q and
  window-manager close both guard pending operations and dirty drafts. Choose
  Keep editing, Discard and close, or Save and then close again.
- Concurrent edits are not silently overwritten: a stale save retains the draft
  and offers recovery through Duplicate or discard/reopen. Pending operations
  synchronously lock field mutations, including retained native IME handlers
  before redraw; controls also hide while busy. Editing resumes after completion
  or failure, except while a committed OK is closing. OK additionally compares
  the current document to the acknowledged saved snapshot before closing.

The list is bounded to 128 entries per ID-ordered page, not title search/ranking.
After the final page, Refresh / first page returns to the start. Fields support
native clipboard, selection, Unicode and multiline input as documented in
[text-input.md](text-input.md). Lyrics preserve LF/CRLF and blank/trailing lines;
they scroll instead of soft-wrapping. Songs exceeding 4096 logical lyric rows per
section or containing multiline metadata cannot be loaded into this provisional
editor; the original database content remains unchanged. Total encoded song
limit is 256 KiB. Fields have the storage metadata limits.

## M1-05e — EasyWorship Words layout

Implemented-unqualified. Layout and behavior follow the installed EasyWorship
8.0.49 observations EW8-OBS-021..026 (RUN-W06E in
[reference-observations.md](reference-observations.md)), in Sela's light
finish. Original GPUI code; no EasyWorship code or assets.

- Window "Song Editor - <title>" ("Untitled" while empty). Toolbar: Title field
  top-left with New/Undo/Redo below it, then Text, Scripture, Shape, Media and,
  at the right, Format, Animate, Presentation. Format opens the Format pane
  (M1-05g3a, below); the other six are shown disabled until implemented.
  Sela's Library and Inspector follow.
- Words is one list of slides. Each row has the slide number, a bold label cell
  (placeholder "label") and a lyrics cell (placeholder "song"). A labeled slide
  starts a bordered group; unlabeled slides join the group above. Group colors
  by label kind: Verse and unknown labels blue, Chorus/Pre-Chorus rose,
  Bridge/Tag purple, Ending dark red, Intro green (EW hues as accents with light
  tints). The selected slide's number cell is highlighted.
- A new song puts the caret in slide 1's label. Down (and, provisionally,
  Enter) in a label moves to its lyrics; Up on the first lyric line moves to the label; Up/Down at a cell
  edge cross to the neighbouring slide. Enter in lyrics is a line break and a
  blank line does not split. **Ctrl+Enter** in lyrics splits at the caret: the
  rest becomes a new **unlabeled** slide right after it (the newline before the
  caret is dropped) and the caret moves to its start. Arrangements gain the new
  slide after every occurrence of the split one.
- Backspace at the start of an unlabeled slide joins it to the previous slide
  (inverse of Ctrl+Enter; provisional, unobserved in EW). Its occurrences leave
  arrangements; Undo restores them. On a labeled slide Backspace at the start
  of the lyrics moves to the label and never joins.
- **+** appends an empty unlabeled slide with the caret in its label. EW shows
  it as its own group; Sela derives groups from labels, so it joins the group
  above until labeled (recorded deviation). **−** (Sela only) removes the
  current slide unless an arrangement uses it.
- The right pane previews the caret's slide as the audience would show it: the
  same bundled font, fitted size, 32 px inset and centered raster as the
  audience preparer (`audience::text_coverage`), at 1280×720, white on black.
  Rasterizing runs on the background executor, latest-wins with one job in
  flight; replaced images are dropped from the GPU atlas. An empty slide shows
  "Double click to edit song"; double-click focuses its lyrics. Text the
  audience would reject (overflow, missing glyph) shows a message instead of an
  image. No copyright strip, theme or background yet (M1-05g/h).
- Footer: disabled "Apply changes to items in schedule" (schedule items pin
  revisions; update-in-schedule is not implemented), **Apply** (enabled while
  dirty; saves and stays open), **OK**, **Cancel**.
- Library (Sela) keeps the saved-song list with Duplicate, Delete, Discard
  edits and paging.

Not yet matched: drag selection across cells (Ctrl+A selects the whole
document since M1-05g3b, below), typing-group undo (Sela undoes
per native edit), canvas text-box editing
and its context menu (EW8-OBS-025), and the zoom slider.

Checks: GPUI tests `words_cells_navigate_and_new_song_focuses_first_label`,
`ctrl_enter_splits_without_label_and_backspace_joins`,
`preview_follows_the_caret_slide_off_thread`,
`preview_matches_audience_raster_and_rejects_unshowable_text`,
`label_kinds_and_groups_follow_observed_palette` plus the updated history,
pending-lock, save/duplicate/delete and limit tests. Windows native replay:

```powershell
cargo build --locked --bin sela
python scripts/song-editor-windows.py   # captures in .amp/in/artifacts/song-editor-windows/
```

It types through SendInput (Unicode), asserts the foreground, checks the window
title, preview ink, split/join, and decodes the saved SQLite payload. The X11
`scripts/song-library.py` still uses the M1-05d form coordinates and is not
ported to this layout.

## M1-05f — Slides tab thumbnails

Implemented-unqualified, following EW8-OBS-024 (installed EasyWorship 8.0.49
capture `25-slides-tab`).

- **Slides** narrows the left pane to a thumbnail column (264 px). Each slide
  shows its number at the left, a rendered thumbnail and a caption bar below
  it: the label on its label-kind tint (same palette as Words), or grey italic
  "Slide N" on a neutral bar when unlabeled (as EW shows an unlabeled slide).
  The selected slide has a blue rounded frame; clicking or Space/Enter on a
  focused thumbnail selects it and the right preview follows.
- Thumbnails are the 1280×720 preview raster box-filtered 4× down to 320×180,
  so they keep the audience layout (fitting directly at thumbnail size would
  change proportions because the text inset is fixed in pixels). An empty slide
  is plain black; text the audience would reject shows "Cannot be shown".
- Rendering runs on the background executor, one slide at a time in slide
  order, and only while Slides is shown. The cache is keyed by slide text and
  pruned to the current slides on every render (also after edits in Words), so
  it holds at most one 225 KiB image per slide (≤128) plus one landing render,
  under 30 MiB; pruned images are dropped from the GPU atlas.
- In the Slides tab, actions that would put the caret in a slide (+, undo/redo)
  focus that slide's thumbnail instead, so shortcuts keep working. Double-click
  on the preview switches to Words with the caret in that slide's lyrics
  (Sela substitute: EW enters canvas text editing, EW8-OBS-025).

Not yet matched: EW's full-theme thumbnails (background, copyright strip;
M1-05g/h), unobserved Slides keyboard navigation (Sela uses Tab order), drag
reordering and auto-scrolling to the selected thumbnail.

Checks: `thumbnail_is_the_preview_box_filtered`,
`slides_tab_thumbnails_render_off_thread_and_prune_edits` and the native replay
above (captures `05-slides`, `06-slide-1-selected`, `07-unlabeled-slide`).

## M1-05g3a — Format pane

Implemented-unqualified. Layout and defaults follow EW8-OBS-027..029 and
EW8-OBS-033 (RUN-W06G); code in `src/song_library/format_pane.rs`.

- The toolbar **Format** toggle (underlined while on) docks a 284 px pane at
  the right of the preview, which shrinks to make room. The pane shows the
  EW element tabs Style · Text · Arrange with Text active and its
  Style/Layout switch on Style; the other tabs and Layout are shown
  disabled.
- **Font**: the bundled DejaVu Sans first, then every installed family from
  the shared catalog, each row with a sample in that family. The list is
  virtualized, so only visible rows load faces. It reads "Loading fonts…"
  and cannot open until the background scan has landed.
- **Size**: Auto, or the number field after "Do not auto size text". That
  choice starts from the size the preview laid out (in 1080-reference px,
  EW8-OBS-028). "Resize text to fit element" returns to Auto and "Reset
  Size" clears the override. A▾/A▴ step by 2 px (Sela choice; EW's step is
  unobserved).
- **Color**, Outline color and Shadow color: a swatch grid (Sela's own
  colors in EW's grey-ramp-over-hues layout) and a Hex Value field
  (`#RRGGBB`). Spectrum and Values are shown disabled.
- **B / I / U**, left/center/right and top/middle/bottom alignment buttons.
  X², X₂ and the indent buttons are shown disabled.
- **Outline**: None or Outer (Center and Inner are listed but disabled), then
  color, size and opacity. **Shadow**: None or Enabled, then color, an
  angle dial (degrees counterclockwise from the right; 315 points
  down-right) with its number field, then offset, blur and opacity.
  Turning an effect on starts from the EW theme values (outline black 7 px
  at 100 %, shadow black 315° offset 18 blur 9 at 90 %). Its other controls
  stay disabled until it is on.
- A change applies to the **caret slide** (in Slides, the selected slide)
  and is one whole-document undo step. A slider or dial drag is one step
  too: the drag follows the pointer anywhere in the window without
  history and records the pre-drag document when the button is released.
  A typed value commits on Enter, Up/Down (which also step it) or focus
  loss. Out-of-range or non-numeric values are refused with a status
  message, the slide stays unchanged and the field reloads. Nothing applies
  while a storage operation is pending.
- Buttons, menus, sliders and the dial do not take focus on click, so the
  caret stays in Words. They are keyboard reachable in layout order after
  the editor's own controls: Space/Enter activates, Up/Down/Enter/Escape
  drive an open menu, arrows step a focused slider or dial. Escape or a
  click outside closes a menu and restores the previous focus, and a click
  that dismisses a menu on its own trigger does not reopen it.

Not yet matched: the deferred Text › Style/Layout controls (M1-05j–m), the
Style and Arrange tabs (M1-05n–o), and what the pane shows when slides
disagree (unobserved). Ctrl+A whole-song formatting is in M1-05g3b below.

Checks: GPUI tests `format_pane_applies_to_the_caret_slide_as_one_undo_step`,
`menus_open_by_keyboard_and_a_dismissing_click_does_not_reopen`,
`number_fields_refuse_invalid_values_and_hex_sets_the_color`,
`slider_and_dial_drags_are_one_undo_step_and_save_round_trips`, the fitted
size in `preview_matches_audience_raster_and_rejects_unshowable_text`, and
unit tests for hex parsing, slider and dial mapping, theme defaults, the
palette and the font label. The native replay above opens the pane, sets
slide 1 red and bold by keyboard and decodes the stored format (captures
`08-before-format` to `12-format-saved`).

GPUI patterns inspected at the pinned Zed revision
`a84689073d296dfd39987bc7dd478e43ef76d83a` (Apache-2.0 GPUI only):
`crates/gpui/src/keymap.rs` and `keymap/context.rs` (context depth
precedence for `SelaMenu`/`SelaSlider` bindings over the root),
`elements/div.rs` (mouse-down focus after element listeners, so
`prevent_default` keeps the caret; capture-phase `on_mouse_down_out`;
`on_mouse_move` while hovered), `elements/anchored.rs` and
`elements/deferred.rs` (popovers above the pane, snapped to the window),
`elements/uniform_list.rs` (font list and `scroll_to_item`) and
`elements/canvas.rs` (slider and dial bounds from prepaint). Zed's GPL
`font_picker` was read for names only; no code was copied.

## M1-05g3b — Whole-song selection

Implemented-unqualified. Selection and formatting follow EW8-OBS-034–036
(RUN-W06H); replacing the text follows the owner's report of EW behavior.

- **Ctrl+A** (Cmd+A on macOS) in a Words cell or on a Slides thumbnail
  selects every slide: every label and lyrics cell shows its text selected,
  every slide number and thumbnail shows the selected state, and in Words
  the caret moves to the end of the last slide. Ctrl+A in the Title,
  Inspector and Format pane fields still selects only that field's text.
- While all slides are selected, **every Format pane change applies to
  every slide** as one undo step (a whole slider drag included). The pane
  shows the caret slide's values. Using the pane keeps the selection.
- **Typing, Paste, Backspace, Delete, Cut or Enter** over the selection
  replaces the song with one unlabeled slide holding the new text (empty for
  Backspace/Delete/Cut, a line break for Enter). It keeps slide 1's id and
  format; other slides leave the arrangements. That is one undo step, and
  the caret is at the end of the new text. These are Sela choices: EW's
  result, undo and labels after a replacement are unobserved.
- **Copy** (and Cut) put the whole song on the clipboard as text: each
  slide's label line, if any, above its lyrics, slides separated by a blank
  line.
- The selection ends on a caret key (arrows, Home/End and their Shift
  forms), a mouse press in Words, focus moving into another cell, the
  Words tab (EW8-OBS-036; Slides keeps it), a double click on the preview,
  Undo/Redo, slide structure edits (+, −, Ctrl+Enter, joining, choosing a
  slide) and opening another song. Wholly selected cells then collapse to
  their end.
- Keys that the platform still delivers to a removed cell before the next
  paint (for example the rest of a fast typed word right after the
  replacement) go to the focused cell instead of being lost.

Not yet matched: EW shows only the Slide pane while all slides are selected
(EW8-OBS-035), Sela keeps the Text pane so the change can be chosen. IME
composition over the selection empties the song and composes into slide 1;
it is unqualified, like IME in general. Mixed values across slides are not
shown.

Checks: GPUI tests
`ctrl_a_selects_every_slide_and_formats_apply_to_all_in_one_step` and
`typing_over_the_whole_song_replaces_it_as_one_undo_step`. The native
replay above selects all on slide 1's thumbnail, sets every slide green by
keyboard and decodes both stored formats, then double clicks the preview,
types over Ctrl+A in Words and checks the saved song is one slide with
slide 1's format (captures `13-all-selected` to `15-typed-over`).

GPUI patterns inspected at `a84689073d296dfd39987bc7dd478e43ef76d83a`:
`crates/gpui/src/keymap.rs` (a later binding wins at equal context depth,
so `SongWords > SelaTextInput` Ctrl+A overrides the field's select-all),
`elements/div.rs` (`capture_action` runs root-to-focus before the cell's
own handlers and continues unless propagation stops;
`capture_any_mouse_down`), `window.rs` (`defer` runs after the cell's
update, so the replacement can reload cells) and `app/test_context.rs`
(`simulate_input` sends one keystroke per character).

## M1-05h2 — Slide backgrounds render

Implemented-unqualified (slice h2 of M1-05h; editing is h3). Model and
storage are in [storage](storage.md#m1-05h1--slide-backgrounds--backed-up-schema-4);
EW behavior in EW8-OBS-037–043.

- A slide shows its own background, else the song master, else black. A
  color fill is the cue's solid background. An image (`Fill::Media`) is read
  from `<profile>/Resources/Images/`, checked against the SHA-256 pinned when
  it was chosen, decoded within the 8 MiB file / 64 MiB pixel budgets and
  fitted on the CPU to exactly the output size (`background::fit`): **Zoom**
  covers and crops the overflow evenly, **Stretch** fills and distorts,
  **Maintain** shows the whole image between opaque black bars. The renderer
  then composites it 1:1, so the audience protocol and shader are unchanged;
  `PreparedBackground::Image.rgba` is shared (`Arc`) so a cached background
  is not copied per cue.
- **Operator.** The image worker gets `Job::Background(image, aspect,
  extent)` and replies with the fitted pixels or a `Substitute` (missing,
  changed, too large, unreadable). The operator keeps at most one such job
  queued (so an import still fits the two-job queue) and caches fitted
  backgrounds (96 MiB, never fewer than 3 entries, least recently used
  evicted). Each poll prefetches, most urgent first: a deferred cue, the
  live slide, the previewed slide, then both neighbours; at the output size
  when Live output is on, else at 1920×1080 so Preview can warn early.
  Prefetching stops at what the cache holds for that size, so it never
  evicts its own work. A Go Live, Next or Previous whose image is not ready
  shows "Preparing background…", keeps the current scene, and is sent when
  the decode lands (the same deferral as the font gate). A substituted image
  goes live as **black** with the warning in the Live status, and the
  Preview pane shows the same warning for the previewed slide, for example
  "Background image ‘x.png’ is missing; showing black". Substitutes are
  forgotten when the images folder is rescanned. Preview and Live tiles show
  a color fill behind their text; image fills still show black there.
- **Song Editor.** The preview and Slides-tab thumbnails composite text over
  the same background through `compositor::blend_over`, the CPU twin of the
  shader (linear light, straight alpha over black). Fitted 1280×720 images
  are cached by image and aspect, four at most (about 14 MiB), behind a mutex
  that only the background render tasks take. The caption shows the
  substitution warning, joined with a font fallback warning when both apply.

Not yet matched: editing (h3), gradient fills, opacity/rotate/flip and video
backgrounds (new tickets), EW's Auto aspect toggle, rendered image
backgrounds in the operator's text-only tiles. The cue frame is still
encoded on the UI thread by `output::Supervisor::send`, which copies the
fitted image once (16 MiB at 2560×1600); this was not measured separately.

Checks: unit tests for the three fits (exact crop, stretch edges, bars both
ways, alpha passthrough), `plan`, decode substitution (missing, changed,
corrupt, GIF, unsafe name, oversize output), the cache budget and minimum,
worker replies, `slides::cue` image passthrough and wrong-size refusal,
`blend_over`; editor tests `preview_composites_over_the_fitted_image_or_substitutes_black`
and `editor_backdrops_keep_at_most_four_fitted_images`; operator GPUI test
`image_backgrounds_gate_the_cue_and_missing_ones_show_black_with_a_warning`.
`Compositor::check_readback` now compares `blend_over` with the GPU readback
over a fitted image (DX12 and Vulkan pass). `scripts/live-output-windows.py`
adds a third operator phase on `seed_library --background-song`.

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
Slides showed original text-only section thumbnails (first four lines), not
rendered audience slides (superseded by M1-05f rendered thumbnails). Native fields retain document history across view
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

## M1-06b — lossless identity/arrangement data prerequisite

Implemented-unqualified on `ticket/m1-06-persistence`, LOCAL main `f161dc3`.
The existing whole-Song source of truth now includes explicit section IDs and
stored variants/occurrences. Field editing, Save, Duplicate, Load, baseline and
whole-document Undo/Redo retain them. New/Add allocate a new ID using CPU-only
allocation; undo restores the exact previous ID, not a new one. Initial blank
draft and baseline share one allocation so opening remains clean. A fresh New
document has fresh section identity. History byte accounting includes variant
names and occurrence storage, retaining the 64 snapshots / combined 8MiB policy.

Removing a referenced section is rejected before draft/history mutation; it does
not delete the variant, truncate repeated occurrences or retarget to another
duplicate label. Existing status reports the reference/limit rejection. No
arrangement controls or repair UI are added: backend clients must explicitly
repair/remove references in a complete valid document before that section can
be removed. Unreferenced structural edits and all existing guards still work.
No geometry/chrome/focus/action mapping or Painter change.

New GPUI test executes durable load with duplicate labels/repeated occurrences,
field edit/save/duplicate, whole-document undo/redo, fresh Add/undo identity,
rejected referenced Remove with unchanged history, and valid unreferenced Remove.
Existing test now checks fresh blank content/baseline/ID rather than comparing to
an independently allocated `blank()`; failure/dirty/undo assertions retained.
All-target tests: 104 passed, 1 ignored subprocess fixture invoked separately;
strict Clippy, fmt, Ruff and Python AST/diff checks passed with serialized shared
target and observed new tests. No new GPUI pattern, upstream dependency or UI
component copied; existing documented history/input provenance remains applicable.

Native driver still decodes the original text codec and now independently queries
schema-2 section IDs, checking distinct IDs on first save and exact retention
across Remove/structural Undo/save. It was lint/parse checked, **not replayed** here:
parent owns shared :99 and will run after merge, using a freshly compiled merged
binary (shared target must be locked and root sources touched). Exact replay:
`DISPLAY=:99 VK_DRIVER_FILES=/dev/null SELA_BINARY=/home/user/workspace/repo/target/debug/sela CARGO_TARGET_DIR=/home/user/workspace/repo/target python3 scripts/song-library.py`.
No new display/server launched. Native error-state rendering, Windows/reference/
IME/accessibility/hardware and measured memory/latency qualification remain open.
