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
fallback minima. Resources collapse/restore and Reset layout are clickable only;
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
