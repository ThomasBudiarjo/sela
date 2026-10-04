# M1-05a native text-entry foundation

Original GPUI component, **implemented-unqualified** provisional development
policy under the owner's UI-first approval. No Painter, storage, network, save,
close, live action or operator integration. Installed EasyWorship 8.0.49 input
behavior has not been observed; these editing policies are reversible, not parity.

## Parent API

Add `mod text_input;` to the binary and call `text_input::bind_keys(cx)` once
at application startup. Retain entities in the editor model, never create fields
inside `render`:

```rust,ignore
let title = cx.new(|cx| TextInput::new("", false, 4096, 0, cx).unwrap());
let lyrics = cx.new(|cx| TextInput::new("", true, 256 * 1024, 1, cx).unwrap());
```

`new(text: &str, multiline: bool, max_bytes: usize, tab_index: isize,
cx: &mut Context<TextInput>) -> Result<TextInput, InputError>`;
`text() -> &str`; `error() -> Option<&InputError>`;
`edit_count() -> u64`; `set_text(&str, cx) -> Result<(), InputError>`.
Implements `Render`, `Focusable`, `EntityInputHandler`; the custom Element installs
`ElementInputHandler` during painting. Observe entity notifications to refresh
parent state. Notifications also occur for selection/error/focus-related changes;
use the counter or original-byte comparison to distinguish content edits.

Successful load resets undo/redo, caret to zero and marked text; increments the
counter (including construction). Record a saved baseline after loading. The
counter is monotonic, including undo/redo, not a proof of dirty content: compare
original text or a saved snapshot for a true dirty-close decision. Failed load
returns the error and keeps all text/history/selection/counter intact. Parent
owns validation across fields and the storage envelope: two individually legal
fields can together exceed the song's storage budget.

### Optional document-owned history (M1-05c)

`use_document_history(on_edit: impl FnMut(&str, &mut App) + 'static)` opts a
retained field into parent-owned history. The callback receives the new original
UTF-8 bytes **synchronously for content-changing user edits only**. Loads via
`set_text`, rejected/no-op edits and selection/focus changes never invoke it.
This preserves ordering even for multiple callbacks in one GPUI app update;
ordinary observe/notify can coalesce and is unsuitable for document chronology.

The callback must not read/update the currently borrowed field, or initiate
another edit; callers must not invoke native editing methods while the owning
document entity is already borrowed. Song Library captures a WeakEntity and
updates only its independent draft/history, with no field read or I/O. Parent
loads remain safe because `set_text` does not call back. The optional method has
a scoped dead-code allowance because the standalone example compiles this same
module without a document owner; its actual behavior is tested in both targets.

Opt-in disables local buffer snapshot recording and propagates semantic Undo/
Redo actions to a contextual parent handler. Without opt-in, all existing field
history/selection behavior remains unchanged. The parent must register/handle
the actions and implement its own limits, baseline and close policies; see
[song-library.md](song-library.md). Undo/redo document restoration uses validated
`set_text` loads, resets field caret/composition and does not recursively record.

InputError is `ByteLimit | LineLimit | Newline | InvalidRange`; rejected edits
are not truncated, and a readable inline red error persists until another edit,
load or undo/redo. `Enter` is consumed even in single-line fields (does nothing);
multiline Enter inserts LF. Parent owns Tab traversal, close/save and commands.
The fixture's contextual Next/Previous actions call GPUI focus_next/focus_prev;
field focus handles are tab stops with supplied indices.

## Text, selection and layout policy

- Original UTF-8/Unicode and existing LF/CRLF bytes are preserved on load, accessor,
  clipboard and save handoff. No NFC normalization or newline replacement.
  New Enter uses LF, so mixed line endings can result intentionally. CRLF is a
  grapheme and deletion traverses it atomically. LF ends a logical row; CR directly
  before LF is hidden in layout but remains in the buffer. Standalone CR is
  preserved as a literal scalar, not interpreted as a new logical row.
- Single-line fields reject any insertion/load/paste/preedit containing CR or LF,
  atomically. They never flatten lyrics. Clipboard cut/copy uses selected bytes.
- Internal offsets are UTF-8 bytes. Platform offsets are UTF-16 code units;
  out-of-bounds endpoints clamp to document end; an endpoint inside a surrogate
  pair clamps down to its scalar start. Reversed platform ranges are rejected,
  not swapped. Native IME selections may land between combining scalars.
  Movement/hit testing snaps down to grapheme boundaries; collapsed deletion
  inside a grapheme removes the whole containing grapheme. Explicit nonempty
  platform selection replacement is scalar-based and honors that selection.
- Replacement precedence: explicit platform range, then marked range, then
  current selection. Preedit selection is composition-relative: convert against
  **new text**, then add replacement start to **both** endpoints. Marked text is
  underlined and `unmark_text` notifies. Direction uses retained anchor/head;
  Shift movement/mouse extends without losing direction when crossing anchor.
- One ShapedLine per actual logical row, including empty/trailing rows; 13px
  text, 22px line height. **No soft wrapping**: horizontal and vertical scrolling
  use clipped custom painting. Wheel/trackpad pans, movement/typing reveals caret;
  drag outside the viewport extends selection and reveals it. Up/Down carries
  the current shaped x into the adjacent logical row (no sticky-column memory).
  Home/End are logical row boundaries. No word, double-click or document-boundary
  shortcuts are claimed. Caret is steady, not blinking.
- Hit testing and IME first-row range rectangles use the same actual element
  origin minus scroll. A multirow IME rectangle describes its first row, not a
  union. Out-of-viewport point queries clamp to the nearest logical row/character.
  The component has a fixed 22px single-row or 220px multiline viewport, plus
  seven-pixel padding and border; parent can place it in its actual editor layout.

## Bounds and provenance

Max bytes is `min(caller limit, 256KiB)`; at most 4096 logical rows. Validate the
resulting byte/row count and preedit range before copying snapshots or allocating
the replacement. Undo and redo each retain at most 32 snapshots and 2MiB of text
(4MiB combined), plus one transient snapshot/current text and GPUI shaping caches.
Every accepted replacement is a snapshot, including preedit updates; composition
is not coalesced into a single undo group. These are text/cache policies, not
hard RSS or real-time guarantees. Shaping is bounded by the field cap and runs on
the UI thread; no file/database/network work occurs there. Performance on large
production songs and complex scripts needs measured qualification.

Inspected current HEAD with `git ls-remote https://github.com/zed-industries/zed
HEAD`: `a84689073d296dfd39987bc7dd478e43ef76d83a`, same as our pinned dependency.
Read its `crates/gpui/examples/input.rs`, `examples/tab_stop.rs`,
`src/input.rs`, `src/text_system.rs` shape_line contracts and `src/window.rs`
handle_input/content mask APIs from the local cargo checkout. GPUI's Apache
example informed input/Element scaffolding; Sela's buffer, policies, logical-row
layout, bounds, history, tests and styling are original modifications. Retained
upstream copyright and full Apache-2.0 terms in `LICENSE-GPUI-APACHE`. This note
is the adaptation/modification notice. No GPL `crates/editor` or `crates/ui`
implementation/assets copied. Sela remains AGPL-3.0-or-later. The only dependency
change is direct `unicode-segmentation = "=1.13.3"`, already locked; no upgrades.

## Verification and native fixture

```sh
export CARGO_TARGET_DIR=/home/user/workspace/repo/target
cargo test --locked --example input_check -j4
cargo clippy --locked --all-targets -j4 -- -D warnings
cargo fmt --all -- --check
cargo build --locked --example input_check -j4
DISPLAY=:99 VK_DRIVER_FILES=/dev/null "$CARGO_TARGET_DIR/debug/examples/input_check"
```

The binary is in the exported shared target, not necessarily local `target`.
The real native fixture retains two entities, displays bytes/edit counters/errors,
has native Tab traversal and clear rejection styling. No fake web input.
Focused tests in the module run through this permanent example target.

Native review artifacts: `.amp/in/artifacts/input-check/` in this worktree.
Tests cover nonzero-prefix surrogate/combining IME, explicit range precedence,
invalid/clamped ranges, grapheme boundaries/deletion, line/byte/history budgets,
CRLF/trailing empty rows, callback scroll-aware hit/range geometry, atomic load,
contextual Enter and clipboard rejection versus a real root action.
Headless callback tests are not an executed platform IME session.

Executed on Debian12 x64, Xvfb :99/Openbox/software GL, debug binary: PID-scoped
XTEST focus before each key/type, private temporary XDG runtime, owned process
deadline and cleanup, root capture cropped to the owned client. No shared service
restart. Automated checks assert focus, input survival and WM close exit 0;
content is visually reviewed, not inferred from key delivery alone. Useful images
include `newline-error.png`, `byte-error.png`, `horizontal-scroll.png`,
`scrolled-edit.png`, `mouse-selection.png`, `cut-paste-roundtrip.png` and
`compact-selection.png`; `native-summary.json` records the run boundary.

Manual native replay using the command above (reproducible fixture, no scratch
driver retained):

1. Ctrl+A, type `Native title`; Tab, Ctrl+A, type `Native lyrics`, Enter,
   type `Second line`. Ctrl+A/Ctrl+C, Shift+Tab/Ctrl+V: title must stay unchanged
   with `Newline` error. Tab/Ctrl+Z/Ctrl+Shift+Z: undo/redo lyric edit.
2. Ctrl+A, type a 191-byte single-line lyric; Ctrl+A/Ctrl+C,
   Shift+Tab/Ctrl+A/Ctrl+V: original title stays unchanged with `ByteLimit` error.
   Tab/Right/End: line end and caret visible, leading content scrolls out.
3. Enter and type `Row 00` through `Row 23`, each followed by Enter. Up/Home,
   type `EDIT `: visible edited last row, earlier rows scrolled out. Drag a
   selection across visible rows: highlights follow actual text coordinates.
4. Restart fixture, Tab/Ctrl+A/Ctrl+X: zero-byte lyrics with empty caret.
   Ctrl+V: all original Unicode/blank/trailing rows return, 47 bytes. Undo/redo;
   resize client to 420×440, move Up/Up/Home/Right/Shift+Right: readable selection
   and wrapped fixture labels. Alt+F4: clean process exit.

An initial inline driver wrongly asserted the app must survive Alt+F4; corrected
the driver to wait for exit 0, not a component fix. Full input run then passed;
final binary repeat additionally checked cut/paste and resize. Performance/RSS
has not been measured; the max-budget history test is correctness only.

Not qualified: Windows/macOS/Wayland IME services, CJK dead keys/dictation,
screen reader/accessibility, mixed DPI, bidi selection geometry, physical GPU,
installed EasyWorship behavior, RSS/latency/SLO or integration with song save/close.
Do not mark parent M1-05 done. Next: parent imports retained fields and bounded
storage worker; run actual platform IME/reference/accessibility checks.
