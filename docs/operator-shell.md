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
