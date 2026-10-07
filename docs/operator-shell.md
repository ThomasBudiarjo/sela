# M1-02a provisional operator shell

Implemented-unqualified on local main `115f545`, with the owner's approved
early-UI sequencing exception. This is an original empty GPUI operator workspace,
not song editing, storage, an audience renderer or EasyWorship parity.

Schedule / Preview / Live occupy the upper row at provisional 24/38/38 percent;
Resources spans the bottom. Five resource tabs change the truthful empty-library
label, retaining the selected tab through collapse and reset. Collections have
no stored entries. Go Live, Black, Clear and Logo are disabled text with **no
handlers**; Live says Output not connected. Quit is clickable and uses the existing
semantic `Quit` action / Sela key context / Ctrl+Q. No other shortcuts were invented.

Two six-pixel column splitters and a six-pixel row splitter use typed GPUI drag
events and an Empty ghost, capturing the original mouse-down position rather than
the post-threshold constructor offset. Release anywhere in the window clears the
owned marker; a retained activation subscription cancels it on deactivation.
Drags do not focus another control. Geometry is recalculated from viewport bounds
on render; resizing retains proportions, with 150px upper pane minima and 140px
upper/resources area minima. Subminimum viewports use nonnegative equal-share
fallback minima. Resources collapse/restore and Reset layout support click and keyboard;
reset restores provisional ratios and expands Resources. Layout persistence,
alternative layout modes, installed-reference focus/traversal and mixed-DPI
qualification remain parent M1-02 acceptance, not silently dropped functionality.

## Provenance and unknowns

Historical official v7 Quick Start (2023):
<https://support.easyworship.com/support/solutions/articles/24000020385-quick-start-guide>
supports the pane families, bottom Resources, five tabs and alternative views.
Exact 8.0.49 geometry, control positioning and focus behavior are unknown.
The owner rejected further Painter use. Its early images are not the UI
specification or parity evidence. No proprietary assets or invented import/help/
menu controls were copied.

Reviewed pinned Zed `a84689073d296dfd39987bc7dd478e43ef76d83a`:
`crates/gpui/src/elements/div.rs` (on_drag, on_drag_move, mouse capture),
`crates/gpui/src/app/context.rs` (observe_window_activation/bounds),
`crates/gpui/src/window.rs` (viewport bounds),
`crates/gpui/src/app/test_context.rs` (mouse, resize, debug selectors),
`crates/gpui/examples/tab_stop.rs` (focus/key-context patterns).
GPUI has no resizable wrapper. Only Apache-2.0 framework APIs are used;
no GPL Zed application UI components, assets or code were copied.

## Reproduction and executed checks

From this worktree, using the shared target directory:

```sh
export CARGO_TARGET_DIR=/home/user/workspace/repo/target
cargo test --locked --all-targets -j 4
cargo build --locked --bin sela -j 4
cargo clippy --locked --all-targets -j 4 -- -D warnings
cargo fmt --all -- --check
uvx ruff check scripts/operator-shell.py
DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/operator-shell.py
DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh "$CARGO_TARGET_DIR/debug/sela"
git diff --check
```

29 tests pass: 18 library, **five actual operator tests** (three existing Quit/
focus/context tests and two substantive shell interaction tests), five composition,
one output diagnostic. Shell tests cover all tabs, collapse/restore, reset, both
sizes, bounds, all three drags beyond viewport, release, cross-axis isolation,
retained focus and subminimum geometry. Simulated input is not native evidence.

Native driver uses existing :99 Xvfb/Openbox/xcompmgr, Debian12 x64 software GL
llvmpipe, debug build. Requires xdotool, xwininfo (`x11-utils` installed in this
orb), ImageMagick import/convert. PID-scoped discovery and verified focus precede
input; bounded subprocess waits, owned process reaping and temporary runtime/root
captures avoid touching other windows or stopping shared services. xwininfo client
coordinates correct xdotool's decoration offset before root crop/input. Script
exit verifies Ctrl+Q, while native-smoke also checks 720x440, unbound input and WM
close. Script sends controls; visual inspection plus GPUI tests verify their state,
not an accessibility-tree or physical-display assertion.

Inspected corrected client crops, all in `.amp/in/artifacts/operator-shell/`:
`normal.png` (1280x800), `compact.png` (720x440), `scriptures.png`
(selected tab and label), `collapsed.png` (restore control, no library),
`drag-minimum.png` (Schedule minimum, no pane overlap). All have readable chrome,
dark canvases, explicit development/output limitations and no clipped controls.
Initial captures exposed decoration-offset targeting and shared typed-listener
cross-axis movement; corrected both and added an isolation regression assertion.

Not executed/claimed: Windows/native GPU/physical output, screen-reader/traversal,
mixed DPI, installed EasyWorship, persistence, other modes or measured performance.
Software Vulkan black captures remain a known environment limitation; explicit
GL is diagnostic only. Next: integrator merges this local ticket, reruns combined
checks and retains parent qualification/persistence/mode items open.

## M1-02b: contemporary light chrome

The owner's target is EasyWorship layout with a clean Codex/T3code/Notion/Zed
visual finish, not the original dated desktop skin. This styling slice preserves
all pane extents, control regions, minimum sizes and interaction handlers. Sela's
own neutral palette, 13px UI text, restrained weight hierarchy, 1px visible
splitters (unchanged 6px hit region), subtle hover/pressed buttons, underlined
active resource tab and centered empty states are implemented directly in GPUI.
Preview/Live retain charcoal canvases; unavailable live controls remain inert.
Existing DejaVu font dependency is retained, not a copied Zed font asset.

Inspected current Zed HEAD (same pinned `a84689073d296dfd39987bc7dd478e43ef76d83a`):
`assets/themes/one/one.json`, `assets/settings/default.json`,
`crates/ui/src/styles/typography.rs`, `crates/ui/src/components/tab.rs`,
`crates/ui/src/components/button/button_like.rs` for design observations.
Zed defaults to One Light, compact sans typography, flat adjacent surfaces and
subtle neutral button feedback. GPL application components/theme files are not
copied/imported; original styles use Apache GPUI `Styled`/`InteractiveElement`
from `crates/gpui/src/styled.rs` and `crates/gpui/src/elements/div.rs`.
Codex/T3code/Notion are user-provided aesthetic references, not code dependencies.

Merged-tree checks: 40 all-target tests, strict Clippy, fmt, native driver and
native-smoke pass. Parent inspected normal, compact, selected Scriptures,
collapsed, minimum drag and new `hover-reset.png` captures. At 720×440 Preview
text is centered at y≈168 within its y72–264 canvas; Live centers higher because
its bottom controls consume 32px. No extra geometry correction is required.
Before-style capture is retained for comparison; this is a first styling slice,
not a finished product UI or owner approval of the design.

## M1-03a: bounded shell keyboard access

Implemented-unqualified on `ticket/m1-03-focus`, based on local main `c937670`.
Tab / Shift+Tab traverse Reset layout, Quit, Collapse/Restore resources, then
Songs / Scriptures / Media / Presentations / Themes, wrapping in either direction.
Traversal does **not** select a tab. Enter / Space activate only the focused
control. Click focuses and activates once. Focus has a pale blue background;
selected resource tabs retain their separate underline. Root Ctrl+Q still routes
from child contexts. No Go Live, safety, slide navigation or search bindings exist.
Disabled Live labels have no focus handle or action handler.

These keys/order/wrapping are reversible **Sela accessibility policy**, not
observed EasyWorship 8.0.49 behavior. Layout 1:1 remains the target; no Painter,
new control placement, domain behavior or output connection is introduced.
Retained handles and GPUI's rendered tab-stop tree exclude hidden resource tabs.
Collapse clicks focus the surviving collapse control; reset focuses its own
surviving control and expands Resources without changing selected tab. Splitter
mouse down suppresses GPUI's default ancestor refocus, retaining the current
root or child focus while preserving geometry and drag/release handling.

Reviewed current upstream HEAD via `git ls-remote` (same pinned
`a84689073d296dfd39987bc7dd478e43ef76d83a`): Apache-2.0
`crates/gpui/examples/tab_stop.rs`, `crates/gpui/src/window.rs`
(`FocusHandle::tab_index/tab_stop`, containment, rendered-tree focus_next/prev),
`crates/gpui/src/elements/div.rs` (track_focus, focus style, default mouse focus
and synthesized keyboard clicks), and `crates/gpui/src/interactive.rs`
(`ClickEvent`). Original implementation uses semantic actions in `Sela` and
`SelaControl` contexts. Keyboard click synthesis is explicitly rejected by the
click callback so an unbound key cannot bypass action policy or double-activate.
No GPL application UI was copied.

Executed reproduction commands above: **42 all-target tests pass**, including
seven actual operator-tree tests. New tests assert forward/backward wrapping,
focused Enter/Space including Quit, hidden-tab removal, safe collapse/reset,
unfocused/unbound rejection, unchanged selection/ratios on irrelevant keys,
root Quit from children, inert Live labels and exact focus retained after drags.
Native driver additionally asserts pixel colors at focus/selection locations
in fixed 1280×800 captures, PID focus, size, survival and exit; these are bounded
rendered-state assertions, not an accessibility-tree or physical-device test.

Inspected `focus-scriptures-unselected.png` (Songs underline/empty label while
Scriptures has focus), `focus-media-selected.png` (Media focus and underline),
`keyboard-collapsed.png` (focused Restore, no tabs), and `keyboard-reset.png`
(focused Reset, Resources restored, Media selection retained), under
`.amp/in/artifacts/operator-shell/`. Native Enter collapse stayed collapsed,
demonstrating no synthesized second activation on key release. Native-smoke
also passes Ctrl+Q and WM-close clean termination. Shared X11 services unchanged.

Open qualifications: installed reference observation, Windows/UIA/Narrator,
physical GPU/display, mixed DPI, screen readers and performance. Text inputs,
modals, multi-selection and actual navigation/live commands are not implemented
in this shell; qualify their ownership with their future controls, not here.

## M1-02c — Reference-directed toolbar correction

Implemented-unqualified, local base `231fffb`, branch `ticket/m1-ui-reference`.
No Painter. Personally fetched and inspected the official marketing interface
image linked in the reference ledger. One toolbar: **New, Open, Save, Web,
Remote** left; **Go Live, Alerts, Logo, Black, Clear, Live** right, above Live.
There is no separate menu row in that image. Its build is not proven 8.0.49.
Only New is supported in this operator toolbar; other labels are disabled,
not focusable and have no handlers. Offline/output-unavailable status is explicit.
The diagnostic color renderer is not connected. Bottom Live buttons are removed.

New opens a small original menu with a real New Song item. Songs has a real
bottom-left + New Song launcher. Both open a blank draft; the companion M1-05d
slice supplies the corrected editor. Reset/Quit remain accessible in Resources
chrome, with existing semantic shortcuts and bounded splitters preserved.
New joins the existing keyboard traversal after the prior nine controls; its
menu item follows it only when rendered. Activation returns operator focus to
the surviving New button before removing the menu, so later Ctrl+Q routes.
The menu overlays rather than reflows Schedule/Preview/Live/Resources.
These menu/focus/compact policies are provisional, not installed observations.

Upstream checked before implementation: current HEAD and pinned dependency both
`a84689073d296dfd39987bc7dd478e43ef76d83a`, confirmed with `git ls-remote`.
Personally inspected Apache-2.0 `crates/gpui/examples/tab_stop.rs`,
`crates/gpui/src/elements/div.rs`, `crates/gpui/src/window.rs`,
`crates/gpui/src/tab_stop.rs` and `crates/gpui/LICENSE-APACHE`. Original GPUI
Div/control/focus implementation; no GPL application UI, assets or code copied.

- [x] Correct action groups/order; remove bottom Live controls; honest disabled state.
- [x] Real New menu and Songs +; preserve splitters/collapse/reset/Quit.
- [x] Actual-tree keyboard/menu/no-reflow tests and inert output assertions.
- [x] Updated native coordinates without dropping focus/selection pixel assertions;
  both launch routes and independent editor dirty-close guarantee replayed.
- [x] Inspect actual normal/New menu/compact/keyboard/drag captures.
- [ ] Installed 8.0.49, Windows/accessibility/DPI, physical output and performance.

Combined checks with M1-05d: `CARGO_TARGET_DIR=/home/user/workspace/repo/target
cargo test --locked --all-targets -j4`: **96 passed, 1 ignored child fixture**
(fixture is invoked by three process tests); both new test names observed.
Strict all-target Clippy, fmt check, Ruff for both native drivers and diff check
passed. Build immediately before serial `DISPLAY=:99 VK_DRIVER_FILES=/dev/null
CARGO_TARGET_DIR=/home/user/workspace/repo/target python3 scripts/operator-shell.py`
passed, as did song-library driver and native-smoke against the same binary.
Existing shared X11 services untouched. Retained actual ignored evidence:
`.amp/in/artifacts/operator-shell/{normal,new-menu,compact,keyboard-collapsed,
keyboard-reset,drag-minimum,editor-close-guard}.png`. Initial driver detected focus
on the removed menu item; repaired the implementation, not the exit assertion.

## M1-10b — Operator live output, Go Live and acknowledged Live

Implemented-unqualified on local `main`. Supersedes the inert Go Live / Live
statements in the M1-02 sections above. The operator now drives the real
`sela --audience` renderer (M1-10a). Owner's request to integrate the UI with the
engine is the sequencing approval; M1-03, M1-09 and M0-08 remain open.

- **Live ○ Off / ● On** (toolbar, rightmost) starts or ends one audience child
  through `output::Supervisor` with a fresh epoch. `SELA_AUDIENCE_MONITOR`
  chooses `secondary` (default), a monitor index or `window`;
  `SELA_AUDIENCE_BACKEND` overrides the graphics backend. Off ends the child. A
  new session never replays earlier intent: Go Live must be repeated.
- **Songs** lists the library catalog, loaded by the storage worker and refreshed
  when the window is activated. Clicking or Enter/Space on a song previews it.
  `sela --operator-library PATH` opens the operator on a chosen library.
- **Preview** shows one slide per song section (trimmed lyrics, label below);
  since M1-06c, per occurrence of the song's first arrangement if it has one.
  Selecting a slide never touches the audience. Double-click sends it to output,
  per the historical v7 Quick Start (SRC-02, documented-only for 8.0.49).
- **Go Live** sends the selected Preview slide. **‹ Previous / Next ›** step the
  Live item and stop at its first/last slide (Sela policy, unobserved).
- **Live** shows only what the renderer acknowledged as Applied: a red border on
  the confirmed slide and "On screen: Song · Section". A cue still in flight is an
  amber border plus "Sending…". At most one cue is in flight and the latest wanted
  cue replaces any older unsent one, so rapid input cannot apply an older slide
  last. A rejected cue shows "Last cue not shown: …" and keeps the prior Live
  state; a lost renderer shows "output state unknown". A settled surface change
  resends the current slide at the new extent.
- Slide cues are white centered DejaVu Sans on black (provisional until a theme
  ticket). Size is fitted from the font's glyph advances, 5% margin, within the
  renderer's text limits (96px, raised to 288px in M1-06c; 32 lines, 4096 bytes,
  4096px text area). Missing
  glyphs or oversize text fail before delivery and leave Live unchanged.
- Tab order: the ten existing controls, then Live output, Go Live, Previous,
  Next, song rows and preview slides. Sela accessibility policy, not observed.

Not implemented: schedule items, Black/Clear/Logo and Page Down (added in
M1-10c below), Alerts, themes, arrangements on output, slide-level arrow keys, auto-follow,
combined/contiguous modes and multi-output targeting.

Cue construction runs on the UI thread. Measured `slides::cue` for a two-line
slide at 2560x1600, 200 runs, dev profile: p50 0.10 ms, p95 0.11 ms, max 0.79 ms.
Each cue carries the 760 KB (759,720-byte) bundled font over the pipe; visible latency below is
an upper bound at capture granularity, not scanout timing.

### Windows native evidence — 2026-10-06 (UTC+7)

`cargo build --locked && cargo build --locked --example seed_library`, then
`python scripts/live-output-windows.py` (keyboard only, operator foreground
asserted before every key without re-activating it). PASS three times on the
laptop panel (`secondary`, 2560x1600 @168 DPI, DX12, RTX 4060 Laptop): no audience
before Live on; child window covers the monitor without taking focus; empty first
frame; Go Live, Next, Next, Previous changed the captured output (observed
171–188 ms after the key); Next at the end changed nothing; Previous restored the
second slide exactly; Live off and Ctrl+Q ended the child; second session empty.
Evidence: `.amp\in\artifacts\live-output-windows{,-r1,-r2}\`. The first run caught a
real renderer rejection (fitted 256px exceeded the 96px limit): Live showed "Last
cue not shown" and "nothing confirmed" while the audience stayed black. That fix
added `audience::tests::operator_slide_cues_fit_the_audience_text_preparer`.

Open: installed 8.0.49 observation (W02/W03), audience monitor hotplug, primary-
monitor output, Linux/macOS native runs, mask states and measured Go Live latency.

## M1-10c — Masks and picture logo

Implemented-unqualified on local `main`, on top of M0-08a (mask layer and logo
slot in [delivery](delivery.md#m0-08a--mask-layer-and-logo-slot)). Behavior
follows EW8-OBS-014–019; cases listed in EW8-OBS-020 are marked provisional.

- **Logo / Black / Clear** in the toolbar toggle the operator's mask intent
  (`src/masks.rs`): Black and Logo replace each other, Clear stacks with either,
  a second press turns a mask off. The button shows what the renderer
  acknowledged, not the click: plain when off, amber outline while the change
  is in flight (or armed with Live output off), pale red fill with red text once
  the audience confirmed it. Logo is greyed out until a logo is set. Live output off keeps the intent and the lit-but-armed state;
  Live output on sends it again in the new session (EW8-OBS-017).
- **Keys**: Ctrl+B, Ctrl+L, Ctrl+C toggle Black, Logo, Clear and Page Down is
  Go Live (EW8-OBS-018). They are GPUI actions bound in the
  `SelaShow && !SelaTextInput` context, so they do nothing while a search or
  editor text field has focus. That text-field rule is Sela policy; EasyWorship
  focus contexts are still open (W04).
- **Live pane**: under a mask it shows "Mask: Black" and "Under mask: Song ·
  Section"; in flight "Sending mask: …"; with Live output off "Mask armed: … ·
  shown when Live output is on"; a renderer refusal "Mask … not shown: …".
  Masks persist across Go Live, Preview double-click and ‹ › (EW8-OBS-016).
- **Live slides**: single-click applies the clicked slide (EW8-OBS-014).
  Double-click also clears the mask once that slide's cue has left the slot
  (EW8-OBS-016 for a single mask). Clearing Black+Clear or Logo+Clear with one
  double-click, and keeping the mask on a single click, are provisional
  (EW8-OBS-020).
- **Media → Images** lists PNG/JPEG files in the profile's
  `Resources/Images/` folder (next to the library). **Import image…** opens the
  native file picker; the copy is written under a free name ("Logo (2).png").
  Selecting an image and **Use As Logo Background**, or right-click → Use As
  Logo Background, stores its name in `Resources/Images/logo.txt`. Listing,
  copying, hashing and reading run on a bounded image worker, never on the UI
  thread; images are capped at 8 MiB and 4096 files.
- **Logo on output**: the operator prepares the logo for the current surface
  extent with the existing off-thread `Preparer` (5 s budget) and hands it to
  the supervisor, again after a new session or a resize. Until the logo for this
  surface is with the renderer, a Logo intent is sent as Black so slide text
  never shows in between; Live shows "Preparing logo · covering with Black".
  Without a logo, Logo refuses with "No logo set · Media → select an image →
  Use As Logo Background". A failed logo keeps the previous one.
- Tab order adds Logo, Black, Clear after Next, and Import image… / Use As Logo
  Background after them while Media is shown; image rows follow the slides.
  M1-06c adds "Normalize text size across slides" after them while Songs is
  shown (see [arrangement](arrangement.md#m1-06c--better-slide-output)).

Known divergence: EW8-OBS-019 says the logo fills the output. Sela draws image
backgrounds with Contain (letterboxed on a different aspect ratio). Which fit
EasyWorship uses for a non-matching logo is not observed; kept provisional
rather than guessed. Video logos are deferred to M2-03. Image items as live
items (where Clear does nothing, EW8-OBS-019) are not implemented.

### Windows native evidence — 2026-10-06 (UTC+7)

`python scripts/live-output-windows.py` (extended for masks) PASS three times on
the laptop panel (`secondary`, 2560x1600, DX12, RTX 4060 Laptop). After Go Live,
Next and Previous, from the unmasked slide: Ctrl+B is black, Ctrl+C removes the
text, Ctrl+L shows the seeded logo color at the center with no text, and each
second press restores the identical slide frame; the matching toolbar button is
lit only after the audience changed. Logo left on survives Live off/on and comes
back lit in the new session without replaying a slide. Each change was observed
171–188 ms after the key (16 ms capture polling; an upper bound, not scanout).
Evidence: `.amp\in\artifacts\live-output-windows-masks{,-r1,-r2}\`.
`scripts/native-cues.py` adds renderer-level mask frames, including Clear over an
image background (see delivery).

GPUI tests: `show_keys_toggle_masks_with_acknowledged_indicators`,
`live_slide_clicks_apply_and_double_click_unmasks`,
`show_keys_stay_out_of_text_fields`, `media_logo_is_imported_persisted_and_shown`
and `images::tests`. The native file picker itself was not driven by the native
script (the logo is seeded into the profile); it is covered by the GPUI test
through the simulated path prompt.

Open: installed observation of the cases in EW8-OBS-020, W04 focus contexts,
logo fit, Linux/macOS native runs, UIA names for the mask buttons and the
image context menu, and physical scanout timing.

## M1-09a — Basic schedule

Implemented-unqualified on local `main`. Phase 4 of the approved 2026-10-06
spec; a first slice of [M1-09](backlog.md#m1-09--schedule-editing-and-versioned-content-snapshots).
The framework-free model is `src/schedule.rs`; saving and opening use the
existing schedule tables through the storage worker, never the UI thread.

- **Items are pinned song revisions.** Add to Schedule (Songs footer) or a
  drag from Songs adds the song revision the library shows now. Editing the song
  later does not change the item; reopening a saved schedule resolves the saved
  revisions, not today's library. The same song can appear more than once.
  Up to 32 items (the storage limit); a full schedule says so and adds nothing.
- **Schedule pane**: header "Schedule · title" (or "Schedule" when untitled)
  and "Unsaved" while it differs from what was last saved or opened. Rows show
  a number and the song title. Selecting a row previews its slides; Page Down
  (Go Live) sends the preview slide as before. Down/Up select the next/previous
  item and stop at the ends.
- **Reorder and remove**: drag a row onto another row to move it there, footer
  **Up / Down** buttons, or right-click → Remove From Schedule. The footer
  **Remove** and the menu ask first ("Remove “title” from the schedule?"), with
  focus on Keep. Ctrl+Delete removes the selected item without asking. A song
  dropped on a row is inserted before it; dropped on the empty list area it is
  appended. Releasing a drag outside the pane does nothing.
- **Live identity**: each item has a session-local ID, so the Live item stays
  the same item when items before it move, are removed or are duplicates of it.
  "● Live" marks a row only once the renderer confirmed one of its slides.
  Removing or moving the live item never sends a cue: the audience keeps the
  current slide and Live shows "Live item is no longer in the schedule".
- **Save / Open**: Ctrl+S or **Save** saves. An untitled schedule asks for a
  title first (Enter confirms); a titled one saves a new revision in place.
  Edits made while a save is in flight stay "Unsaved". If the schedule was saved
  elsewhere since it was opened, the save is refused ("Not saved: this schedule
  was saved elsewhere since it was opened"). Ctrl+O or **Open** lists saved
  schedules by title (case-insensitive); click a title, or focus it and press
  Enter, to open it.
- **Unsaved guard**: Open, **New ▾ → New Schedule**, Ctrl+Q and closing the
  window ask "… without saving schedule changes?" (Cancel or Discard changes)
  when the schedule is unsaved. Quit waits while a save is in progress.
- **Library double-click** goes straight to Live from the song's first slide
  (EasyWorship documents this); single-click still previews.
- The save, open, remove dialogs and the item and New menus block clicks to the
  panes behind them. The same fix applies to the Media image menu.
- Tab order adds Open, Save, Add to Schedule, Up, Down, Remove and New
  Schedule (while the New menu is open); schedule rows follow the song rows,
  and the open dialog cycles its own saved titles and buttons. Ctrl+S, Ctrl+O, Up, Down and Ctrl+Delete are GPUI actions
  in the `SelaShow && !SelaTextInput` context.

Provisional (not observed in EasyWorship 8.0.49): the Up/Down move buttons,
Ctrl+Delete without confirmation, Down/Up stopping at the ends, drop-on-row inserting before the
row, double-click starting at the first slide, and the dialog wording. Not yet
implemented: duplicate/copy item, multi-item select and move, autoscroll while
dragging, themes and media items, schedule-level next/previous from Live, an
explicit library refresh for pinned items, and persistent item IDs (reopening
allocates new ones).

### Windows native evidence — 2026-10-06 (UTC+7)

`python scripts/schedule-windows.py --out .amp\in\artifacts\schedule-windows-m1-09a-r2`
PASS (secondary 2560x1600, DX12). Keyboard only: adds a song twice and a
second song, selects with Down/Up, removes one with Ctrl+Delete, moves one up,
saves as "sunday" with Ctrl+S and reads the saved title, order and pinned
revisions back from SQLite. With Live output on, Page Down sends the first item
and Down + Page Down the second, each changing the audience capture (62 and
63 ms, capture polling); Ctrl+Delete on the live item leaves the audience
frame unchanged. WM_CLOSE and Ctrl+Q both stay open behind the unsaved guard;
Cancel keeps the window, Discard quits. A second launch reopens "sunday" with
Ctrl+O. `scripts/live-output-windows.py` still passes with the new controls.

GPUI tests: `schedule_add_reorder_select_navigate_and_remove`,
`drag_songs_into_the_schedule_reorder_and_cancel`,
`live_item_identity_survives_reorder_duplicates_and_removal`,
`save_and_reopen_pin_revisions_behind_an_unsaved_guard`,
`quit_and_new_schedule_are_guarded`, `library_double_click_goes_straight_to_live`,
plus `schedule::tests` and `storage` `schedule_catalog_lists_current_titles`.
Mouse drag and drop is covered by GPUI tests only.

Open: installed observation of schedule shortcuts, context menu, drag/drop
insertion and delete confirmation; Linux/macOS native runs; UIA names.

## M1-05i — Operator song menu

Implemented-unqualified; merged into local `main` in parallel wave 1. Ticket M1-05i of
[M1-05](backlog.md#m1-05--song-model-and-editor); item order from
EW8-OBS-021.

- **Menu**: right-clicking a Songs row selects it like a click (Preview shows
  its slides) and opens a menu at the pointer with, in order, **New Song…**, **Edit Song…**, **Delete**,
  **Update items in Schedule**, **Sort by ▸** and **Refresh**. Update items in
  Schedule and Sort by ▸ are shown disabled (their behavior is unobserved).
  With a Songs row focused, Shift+F10 or the Menu (Apps) key opens it below the
  row with New Song… highlighted. Down/Up (and Tab/Shift+Tab) move the
  highlight, skip disabled items and wrap; Enter or Space chooses; Escape or a
  click outside closes it and focus returns to the row. These are GPUI actions
  (`OpenSongMenu` in the `SelaSongRow` context; `MenuNext`, `MenuPrevious`,
  `MenuConfirm`, `MenuDismiss` in `SelaSongMenu`), bound in `Operator::new`.
  The menu blocks clicks to the panes behind it, like the other operator menus.
- **New Song…** opens a blank song editor window; **Edit Song…** opens the
  editor on that song (the editor selects the revision once its catalog
  arrives; `song_library::open_song`).
- **Delete** asks "Delete “title” from the song library?" with the note
  "Schedules that use this song keep their copy. Live output does not change."
  Focus starts on **Keep**; **Delete** confirms. The delete goes through the
  operator's storage worker (one job at a time; a second delete while one is
  queued says "Not deleted: wait for the library to finish the previous
  change"). Storage soft-deletes the song, so schedule items keep their pinned
  revisions, and nothing is sent to the audience. On success the row leaves
  Songs, the library selection clears (Preview keeps the slides it shows) and
  the Songs footer says "Deleted “title”". If the song changed or was deleted
  elsewhere first, the footer says "Not deleted: “title” was changed or
  deleted elsewhere" in red and the row stays; other storage failures say so
  with the error.
- **Refresh** and editor changes: Refresh marks Songs stale and the catalog is
  reloaded through the storage worker, not the UI thread. An editor window
  bumps the `LibraryChanged` GPUI global after each save or delete; the
  operator observes it and reloads Songs the same way. A library selection
  follows its song to the newer revision after a reload.

Provisional (not observed in EasyWorship 8.0.49): keyboard access to the menu
(Shift+F10/Menu key, highlight, wrap), the Delete wording and Keep as the
default button, Preview keeping a deleted song's slides, and the message
texts. Not implemented: Update items in Schedule, Sort by options, deleting
several selected songs at once.

GPUI references (pin `a84689073d`): `crates/gpui/src/elements/anchored.rs`,
`crates/gpui/src/elements/deferred.rs`, `crates/gpui/examples/popover.rs`
(deferred + anchored + `on_mouse_down_out`), `crates/gpui/src/elements/div.rs`
(`on_mouse_down_out`, `on_children_prepainted`, focus on mouse down),
`crates/gpui/src/app.rs` and `crates/gpui/src/app/context.rs`
(`set_global`, `observe_global`), `crates/gpui/src/keymap/context.rs`,
`crates/gpui/src/window.rs` (action dispatch), and
`crates/gpui_windows/src/events.rs` (VK_APPS maps to the `menu` key).

GPUI tests: `song_menu_items_order_disabled_items_and_dismissal`,
`edit_song_opens_that_song_new_song_a_blank_one_and_a_save_refreshes_songs`,
`deleting_a_scheduled_live_song_keeps_snapshots_and_the_live_scene`,
`a_failed_delete_shows_why_and_keeps_the_row`.

Native: `scripts/operator-menu-windows.py` (Shift+F10 and Escape, keyboard
Edit Song…, a real right-click, Menu key Delete → Keep, then Delete with the
SQLite row checked and a schedule save keeping the pinned item) **PASS** on
Windows 11 after the wave-1 merge (`python scripts/operator-menu-windows.py
--out .amp\in\artifacts\operator-menu-windows-m1-05i`: menu 64,794 changed
pixels, Escape residue 0, Keep kept the song, Delete deleted it, the saved
schedule still pins it, exit 0). The script first selects the row with
Space, because opening the menu selects its row; the first run, without
that step, failed on a Preview change, not a menu defect. The confirmation
appears at the operator's top left, over the Schedule pane and the Preview
header, like the other operator confirmations. The EasyWorship observation
of the unobserved items was skipped by owner decision for this ticket.

Open: EW observation of keyboard access,
Delete confirmation and the two disabled items; Linux/macOS native runs; UIA
names.
