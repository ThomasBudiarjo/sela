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
- **Preview** shows one slide per song section (trimmed lyrics, label below).
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
  renderer's text limits (96px, 32 lines, 4096 bytes, 4096px text area). Missing
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
