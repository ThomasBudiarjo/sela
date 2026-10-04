# Native testing: bounded M0-03 slice

Status: **implemented-unqualified**. Linux X11 automation is feasible for the
bootstrap; Windows, physical GPU/display and accessibility qualification remain
open. No reference parity or production output is claimed.

## Layers and fixtures

- U/I: `src/tests.rs` constructs the actual `Operator`, its retained focus handle
  and production key bindings. A parent capture-phase observer records Quit
  dispatch without replacing its handler. Three GPUI tests cover focused Ctrl+Q,
  unbound Ctrl+J, blur/refocus and a deliberately mismatched binding context.
  Direct focus-handle dispatch is explicitly contrasted with input dispatch.
  Upstream TestPlatform's `quit()` is a no-op: these tests do **not** prove exit.
- N/E feasibility: `scripts/native-smoke.sh` launches the built application twice,
  discovers only its PID's visible window, activates/verifies focus, sends native
  XTEST input, verifies survival after Ctrl+J, resizes to 720x440, then requires
  clean process exit and window disappearance for Ctrl+Q and WM Alt+F4.
- H/P: Xvfb/software GL is offscreen virtual presentation, not real displays,
  timing, driver recovery or hardware qualification. No benchmark is claimed.

Fixture convention: original synthetic content only; bootstrap uses its existing
static technical-preview text, no proprietary assets or user library. Native
runs use `mktemp`, private mode-700 runtime/config/data/cache directories and an
EXIT/INT/TERM trap that cleans only the launched PID and its temporary directory.
No global settings, WM or display services are changed. There is no application
profile/database or clock-driven behavior yet. Future timed tests must use GPUI's
deterministic executor or an injected domain clock rather than wall-clock sleeps;
future I/O failures need explicit bounded fake adapters, not production test flags.
Current failure injection is missing focus/wrong context/unbound input; preparation,
storage and renderer failures are not implemented and therefore not tested here.

## Reproduce

```sh
export CARGO_TARGET_DIR=/home/user/workspace/repo/target
cargo test --locked -j 4
cargo build --locked -j 4
DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh \
  "$CARGO_TARGET_DIR/debug/sela" .amp/in/artifacts
bash -n scripts/native-smoke.sh
shellcheck scripts/native-smoke.sh
```

Prerequisites: existing X11 display with EWMH WM (tested Openbox), xdotool,
ImageMagick `import`/`convert`, coreutils timeout, fonts. Coordinate exclusive
keyboard/focus use during this short test: XTEST follows the verified focused
window, so concurrent focus changes invalidate the run. Never broadly search
titles or send input to another worker's window. Explicit `--window` XSendEvent
was evaluated and rejected: it produced BadWindow during quit and a subsequent
quit timeout. Focused XTEST passed repeatedly instead.

Polling is bounded to 100 startup/exit and 50 resize attempts with 100ms sleeps;
each xdotool command is capped at 5s, each image command at 10s. A slow command
can extend polling beyond nominal 10s, but cannot wait indefinitely. Cleanup
allows 2s after TERM then KILL. No background app survives the script.
Failure is a nonzero exit, not a skipped pass. To test startup failure handling,
use a nonexistent binary path: the launched process must fail, the script must
return nonzero and leave no temporary directory.

Capture the root then crop to PID-window geometry; direct-window import was
black/hung on this orb. Inspect the resulting PNG manually: automated resize and
exit assertions do not establish readable pixels. Logs and cropped PNG are
copied only if an artifact directory is supplied; transient root captures are
deleted. Do not retain/share the root screenshot (other apps may be visible).

The tested Debian 12 x64 orb uses Xvfb :99, Openbox, xcompmgr, Mesa llvmpipe GL.
Vulkan previously rendered black. `VK_DRIVER_FILES=/dev/null` is a per-command
software-GL workaround, not a global preference or Vulkan qualification. The
script creates its own runtime directory rather than changing `/tmp/sela-runtime`.

## Upstream and accessibility

Inspected pinned checkout `a84689073d296dfd39987bc7dd478e43ef76d83a`:
`crates/gpui/examples/testing.rs` (visual context, focus dispatch),
`crates/gpui/src/app/test_context.rs` (simulate keystrokes),
`crates/gpui/src/elements/div.rs` (capture action),
`crates/gpui/src/platform/test/platform.rs` (no-op quit),
`crates/gpui_windows/src/window.rs` (`a11y_init`, AccessKit adapter), and
`crates/gpui_windows/src/events.rs` (`WM_GETOBJECT`, UIA provider response).
Framework APIs/patterns only, original tests; no Zed application code/assets.
See [bootstrap provenance](gpui-bootstrap.md) for pin/license boundaries.

AccessKit/UIA plumbing exists upstream; this does not prove the bootstrap Quit
div exposes a usable button role/name/action or tab order. GPUI element ID `quit`
is a local test/layout identifier, not a demonstrated native automation selector.
Window PID and semantic key bindings suffice for this bounded X11 smoke, not
stable targeting of a future full operator UI. No Windows session is available.

Manual Windows/native fallback: build per bootstrap note; record OS/GPU/DPI,
open Sela, activate it, Ctrl+J (must stay open), resize 960x600 → 720x440 and
inspect text/controls, Ctrl+Q (must close/process exit), reopen and titlebar-close
(must exit). Check keyboard-only tab order and Narrator/Accessibility Insights
roles/names/actions separately; report missing semantics rather than guessing.
Use a lawful pinned EasyWorship installation for compatibility tests; Quit is
Sela's development control, not an observed reference shortcut.

Headless CI runs GPUI tests only. Native runs are opt-in and fail without DISPLAY;
absence of a graphical/Windows/hardware runner must be recorded as **not run**,
never hardware-qualified or silently skipped green. Do not replace GPUI with a
web UI to manufacture coverage.

## Evidence record

For every run retain: ticket/state, source revision and binary identity, exact
command/result, OS/backend/display/GPU/DPI, fixture, observed states, cropped
artifact paths, failures and unexecuted qualifications. The session record in
[work-log.md](work-log.md) contains this slice's commands/results. Repeat the
native checks after integration because shared target binaries may be rebuilt
by another worktree; a screenshot alone does not identify the tested build.
