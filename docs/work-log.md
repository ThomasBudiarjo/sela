# Work log and resume point

Use [backlog.md](backlog.md) for ticket checklists and test boundaries. This file
records execution state, not a second copy of the backlog. Dates use ISO format;
include timezone for timed hardware/rehearsal evidence.

## Current state

- **Slide backgrounds render (M1-05h2, 2026-10-07):** color and image
  backgrounds reach the audience, the editor preview and thumbnails. Images
  are decoded, hash-checked and fitted (Zoom/Stretch/Maintain) off the UI
  thread and cached in the operator (96 MiB, at least 3); a cue waits with
  "Preparing background…" until its image is ready; a missing or changed
  image shows black with a warning in Live and Preview. GPUI tests, the
  DX12/Vulkan readback check and the Windows live-output script pass.
  M1-05h stays **active**; next: h3, the Slide pane editing UI.
- **Slide background model (M1-05h1, 2026-10-07):** `background::Background`
  (None/Color/Media fill, Maintain/Stretch/Zoom), `Section.background`,
  `Song.master` and schema 4 behind a verified `.schema3-backup`; slides
  carry the resolved background. Nothing renders or edits it yet. M1-05h
  stays **active**; next: h2 rendering and preparation.
- **Slide backgrounds observed (RUN-W06I, 2026-10-07):** EW8-OBS-037–043:
  backgrounds are per slide and fall back to the theme's Master layout
  (Edit Slide Layouts opens a Layouts tab); fills None/Color/Gradient/Media;
  Aspect Maintain/Stretch/Zoom; Ctrl+A applies a fill to all slides.
  M1-05h is **active** with the approved spec (slide → song master → black,
  Zoom default, black bars, black plus a warning for a missing image).
  Next: h1, the background model and schema 4.
- **Whole-song selection (M1-05g3b, 2026-10-07):** Ctrl+A in Words or Slides
  selects every slide; every Format pane change then applies to all slides
  as one undo step; typing, Paste or Backspace over it replaces the song with
  one slide (one undo step); Copy copies the whole text. GPUI tests and the
  Windows native script pass. M1-05g is now **implemented-unqualified**
  (IME, Linux/macOS and EW confirmation of the replace result open).
  Deferred formatting work is ticketed as M1-05j–o. Next: M1-05h per-song
  background, in the owner-approved order.
- **Song Editor Format pane (M1-05g3a, 2026-10-07):** the toolbar Format
  toggle docks the Text › Style pane (font list with samples, size, colors,
  B/I/U, alignment, outline and shadow with sliders and an angle dial).
  Changes apply to the caret slide as one undo step. GPUI tests and the
  Windows native script pass. M1-05g stays **active**; next is g3b, Ctrl+A
  whole-song selection with replace-typing.
- **Ctrl+A formatting observed (RUN-W06H, 2026-10-07):** EW8-OBS-034–036:
  Ctrl+A selects every slide and a format change (Italic, font) applies to
  all of them. M1-05g stays **active**; next is g3a (Format pane), then g3b
  (Ctrl+A whole-song selection) per the approved spec.
- **Styled slides render (M1-05g2, 2026-10-06):** M1-05g stays **active**;
  g1 and g2 are done, g3 is next. Every per-slide `SlideFormat` field stored
  in g1 now renders on one shared path (audience, editor preview, Slides
  thumbnails): installed fonts through the shared off-thread catalog
  (`fonts::shared()`) with bundled DejaVu fallback and an editor warning,
  B/I/U (real or synthesized), color, H/V alignment, auto or fixed size,
  Outer outline and shadow. The operator gates Go Live/Next/Previous on
  background font resolution without replacing the live scene. Windows DX12
  native checks pass. Next: g3, the Song Editor Format pane (Text › Style
  subset), per-slide apply, Ctrl+A whole-song selection with replace-typing,
  undo, GPUI tests and native script.
- **Per-slide formats stored (M1-05g1, 2026-10-06):** slice 1 adds
  `format::SlideFormat` on every `storage::Section` and schema 3
  (`section_formats`) behind a verified `.schema2-backup`.
- **EasyWorship Format pane observed (RUN-W06G, 2026-10-06):** the owner's
  restarted 8.0.49 accepted toolbar clicks, so the Format inspector was recorded
  (EW8-OBS-027–033, superseding unverified EW8-OBS-026): Text › Style (font,
  Auto/fixed size, color, B/I/U, super/subscript, indent, alignment, outline,
  shadow, bullets), Text › Layout (margins, auto sizing with normalize, wrap,
  capitalization), Style, Arrange and the Slide pane (background, for M1-05h).
  Formatting applies **per slide** in EW. Owner chose to stay on EW8 rather
  than switch the reference to EW7. Next: M1-05g after the owner picks the
  formatting scope.
- **Song Editor Slides tab (M1-05f, 2026-10-06):** Slides shows rendered
  thumbnails (the preview raster box-filtered to 320×180 off the UI thread,
  bounded text-keyed cache) with label-kind caption bars or grey italic
  "Slide N", a narrowed pane and a blue selection frame (EW8-OBS-024). Windows
  native check passes. Implemented-unqualified. Next: M1-05g per-song text
  formatting (schema change with verified backup), then M1-05h background.
- **EasyWorship-style Song Editor (M1-05e, 2026-10-06):** RUN-W06E observed the
  8.0.49 Song Editor (EW8-OBS-021–026). The editor now has the Title in the
  toolbar, EW's tool order (unbuilt tools disabled), Words as one list of
  slides with inline label/lyrics cells and label-kind group colors, Ctrl+Enter
  splitting into an unlabeled slide, Backspace joining, `+`, Apply/OK/Cancel,
  and a real audience-raster slide preview rendered off the UI thread. Windows
  native check passes. Implemented-unqualified. Next, in the owner-approved
  order: M1-05f Slides tab thumbnails, M1-05g per-song text formatting, M1-05h
  song background.
- **Basic schedule (M1-09a, 2026-10-06):** Schedule items pin song revisions;
  add by button or drag, remove (confirmed, or Ctrl+Delete), reorder by drag or
  Up/Down, Down/Up select, Ctrl+S/Ctrl+O save and open, unsaved guard on Open,
  New Schedule, Ctrl+Q and window close. The live item keeps its identity
  across reorder/removal and no cue is sent. Library double-click goes to Live.
  Windows native keyboard check passes. Implemented-unqualified. The approved
  2026-10-06 spec is complete; next: choose the next dependency-ready ticket
  (M1-09 remaining items, W04 observation, or M1-04/M1-08).
- **Masks and picture logo (M0-08a + M1-10c, 2026-10-06):** Logo/Black/Clear
  toolbar buttons and Ctrl+L/B/C (Page Down = Go Live) drive the real audience;
  indicators follow renderer acknowledgments; Media → Images imports a picture
  and sets it as the logo. Windows DX12 native checks pass. Both
  implemented-unqualified.
- **Better slide output (M1-06c, 2026-10-06):** first-arrangement order, text
  up to 288px, "Normalize text size across slides" toggle, Ctrl+Enter section
  split with undo. Implemented-unqualified.
- **EasyWorship masks observed (M0-02, 2026-10-06):** RUN-W03 recorded the
  single-song W02/W03 static cases (EW8-OBS-014–020): Black and Logo replace
  each other, Clear stacks, masks survive Go Live/navigation/Live off, Live
  double-click unmasks. Implemented in M0-08a and M1-10c.
- **Operator live output (M1-10b, 2026-10-06):** Live on starts `sela --audience`
  on the secondary monitor; Songs → Preview slide → Go Live shows white text on
  the audience, and Live marks only renderer-acknowledged slides. Windows native
  keyboard check passes. Next: M1-10 safety buttons with M0-08, schedule items
  (M1-09), slide shortcuts after W02/W04 observation, hotplug with the owner.
- **Windows host available (2026-10-06):** Windows 11 Home, i7-14650HX, Intel UHD
  + RTX 4060 Laptop, three displays at mixed scale. M0-01 is **done**: locked
  Windows build/clippy/fmt/tests pass and the native operator window passes the
  Win32 smoke. Windows exposed and fixed a backup durability bug (M1-01).
  M0-04 DX12 audience spike passed on physical displays (stalls, preparation,
  exit/kill, mixed-DPI moves, fullscreen, both GPUs) while the owner was gaming;
  hotplug/device loss/present pacing remain open. M0-07 composition/video
  readback passed on DX12 for Intel and NVIDIA. The idle-machine rerun of the
  audience spike (with the `WGPU_POWER_PREF` adapter matrix) passed. Next:
  hotplug with the owner present. M0-02 is active: the first installed
  observation of unlicensed EasyWorship 8.0.49 recorded the main layout.
- All three Oracle findings fixed: M1-05 pending native-input ownership, M0-07d
  preparation backpressure ordering, and M0-06 disconnected receipt invalidation.
  Regression/native checks passed; separate local fix commits, no push authorized.
- Latest continuation merged and checked: **M1-06b** backed-up arrangement/
  section-ID persistence and lossless editor round trips; **M0-06c/M0-07d** owned
  resource transport and native worker-prepared text/image composition. Individual
  local commits retained. Owner permits Windows/physical qualification at the end;
  available automated/native checks continue before commits. No gate is waived.
- Application: native GPUI technical preview builds and renders on Linux software
  OpenGL, with persistent offline song authoring; no service schedule UI or live
  lyric presentation yet. The Linux preview is reinstalled and native-tested.
  Opt-in winit/wgpu diagnostics cover independent animation, owned text/image
  native cue delivery/receipts and separate explicit-font/image GPU readback.
- Latest parallel wave is merged into local `main`, retaining each subticket:
  **M1-05c** document undo, **M1-01b** verified backup/restore and process-abort
  tests, **M1-06a** immutable arrangement domain, **M0-06b** bounded static-color
  native pipes. **M1-02c/M1-05d** correct toolbar/editor placement from documented
  official references; no Painter. Parent regression fixes and combined checks
  are committed. These slices are implemented-unqualified, not completed parents.
- Next concrete slices: M1-06 arrangement controls and explicit missing-reference
  repair, followed by reference-directed pagination/fitting. Storage schema 2 and
  editor undo already retain explicit IDs and variants; do not regenerate them.
  Renderer can proceed separately with resized-output preparation/retention and
  production preparation-intent/pre-resolved-safety coordination before wiring
  live controls. Native preparation is currently FIFO diagnostic, not a proven
  safety-priority path. No mask-policy guesses or service-ready claims.
- The owner permits UI implementation before physical renderer qualification;
  this changes sequencing, not reference, hardware or service-ready claims.
- M0-07a/b/c/d text/GPU/video/native-resource slices are locally merged. Native
  GPU-ready bindings and receipts are checked on software GL; resize, transitions,
  production performance and physical qualification remain open.
- M0-02 reference ledger, M0-03 native harness and M0-04 audience spike were
  developed in separate worktrees and merged into local `main`, with ticket
  commits and an M0-04 integration/failure-check checkpoint.
- Resume M0-04 on Windows DX12 with two physical displays, DPI/topology changes,
  hotplug and physical capture; run the M0-02 installed-reference runbook alongside.
  No Windows runner is connected. These required checks block gate closure.
- Remaining M1 UI slices may proceed under the approved exception; no M0-10
  physical Windows qualification or service-ready release gate has closed.
- Setup and application changes are local only; no push/publication authorized.
- Parallel opportunity: M0-02 reference observation when a lawful EasyWorship
  8.0.49 installation on Windows is available.
- Known qualification needs: real Windows GPU/displays; EasyWorship reference;
  later physical audio/capture/controllers/mobile devices and permitted provider
  accounts. None is assumed available merely because this repo is in an orb.
- Native E2E: bounded X11 PID-scoped focus/resize/key/exit checks passed. Windows
  accessibility/physical display qualification remains open; see native runbook.

## Ticket status index

Only tickets that have started or changed state need entries. Unlisted tickets
remain `planned`. Update the current state above when switching work.

| Ticket | State | Completed slice / remaining work |
| --- | --- | --- |
| PLAN-001 | done | 63 implementation tickets, 326 ticket checklist items, test/compatibility matrices and commit/resume rules. Documentation verified; delivered in this local planning commit. No application implementation. |
| M0-01 | done | Pinned native GPUI window, lockfile, CI and instructions; Linux and Windows 11 build/test/native launch/resize/quit verified (2026-10-06). |
| M0-02 | active | Public-source ledger (SRC-01–11) and runbook; installed runs on unlicensed 8.0.49: W01 partial (layout, unlicensed output, song editor) and W02/W03 single-song static cases (selection, Go Live, masks, shortcuts, logo); W06 partial (Song Editor layout, Format pane, Ctrl+A formatting, slide backgrounds and the Master layout, RUN-W06E–I). Open: multi-item W02, combined-mask double-click, motion/audio, W04–W06. |
| M0-03 | implemented-unqualified | Three real Operator action/focus tests and repeated native X11 smoke; domain fixture extension and Windows/accessibility checks open. |
| M0-04 | implemented-unqualified | Separate audience process measured under UI stalls, synthetic preparation and operator exit/kill on Linux and Windows DX12 physical displays, incl. mixed-DPI moves, fullscreen, both GPUs, and an idle-machine rerun; hotplug, device loss, present pacing open. |
| M0-05 | implemented-unqualified | CPU snapshots, bounded workers and native resource shaping/upload checked; production intent coordination and hardware qualification open. |
| M0-06 | implemented-unqualified | Ordered bounded native owned-resource IPC/receipts and failure/expiry retention, passed on Linux GL and Windows DX12; production safety coordination/supervision and reference mask semantics open. |
| M0-07 | implemented-unqualified | Explicit-font/image native worker preparation/submission plus static/FFV1 GPU readback; resized output, transitions, performance and physical qualification open. |
| M1-01 | implemented-unqualified | Schema 4 (backgrounds, M1-05h1) after schema 3 (formats, M1-05g1); schema-2 section/arrangement persistence, verified backup-gated migration/fresh restore and process-abort tests; remaining schemas, destructive migrations/recovery UI and power-loss qualification open. |
| M1-02 | implemented-unqualified | Separate-pane contemporary shell and documented-reference toolbar correction; persistence/modes/installed-reference/DPI checks open. |
| M1-03 | implemented-unqualified | Contextual keyboard access to shell with native checks; text-entry/modal/selection/live command ownership open. |
| M1-05 | implemented-unqualified | Persistent metadata/section authoring, full document undo/redo, receipt-gated OK; M1-05e EW-observed Words layout with off-thread rendered preview; M1-05f rendered Slides thumbnails (Windows native check); M1-05g implemented-unqualified (g1 per-slide format storage, g2 styled rendering with the operator font gate, Windows DX12 native check passed; g3a Format pane and g3b Ctrl+A whole-song selection with replace-typing, GPUI and Windows native checks). M1-05h active (h0 RUN-W06I observed, h1 model and schema 4, h2 rendering with the operator background gate and Windows native check; h3 editing open), M1-05i (operator song menu), M1-05j–o (deferred format controls and tabs), drag selection across cells, IME/accessibility qualification open. |
| M1-06 | implemented-unqualified | Stable section/variant/occurrence IDs, immutable domain, backed-up migration and editor data/undo persistence; arrangement controls/reference repair and pagination open. |
| M1-10 | active | M1-10a: production `sela --audience` renderer mode (moved compositor/text, centered text, settled surface-extent frame, non-activating monitor-covering window, scene retained after controller loss), Windows DX12 smoke on two monitors. M1-10b: operator output supervisor, Songs list, section slides in Preview, Go Live/double-click, Previous/Next, Live shows renderer-acknowledged slide, Windows keyboard-driven native check. No checklist box closed: masks, schedule items, preparation off the UI thread, reference-observed behavior and latency remain open. |
| M1-16 | implemented-unqualified | Developer-local Linux install prerequisite only; Windows installer/settings/accessibility and dependency gates remain open. |

## Session records

### M0-01 — Native bootstrap — 2026-10-04 (Asia/Jakarta)

- Scope: one Rust package, pinned GPUI/platform and stderr diagnostics, minimal
  original light window, contextual Quit action, close lifecycle, lockfile,
  Linux checks/Windows build CI, setup dependencies and build instructions.
- Upstream pin, inspected API paths and Apache/GPL boundary are recorded in
  `docs/gpui-bootstrap.md`; no Zed application components/assets copied.
- Verification: `cargo build --locked -j 8` passed; `target/debug/sela --version`
  printed `Sela 0.1.0 (technical preview)`; `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -j 8 -- -D warnings`, `git diff --check`
  passed. `cargo test --locked -j 8` passed with **zero tests**, not feature coverage;
  M0-03 adds meaningful action/native checks.
- Initial compile exposed the two-argument close callback and a focus-handle
  borrow; both corrected against pinned GPUI before final checks.
- Native N: Xvfb 1600x1000, Openbox, xcompmgr, Debian 12 x64, Mesa llvmpipe
  software GL. `DISPLAY=:99 xdotool ... key --clearmodifiers ctrl+q` and
  `... key --clearmodifiers alt+F4` closed the window; service recorded clean exit.
  Root `import` captures cropped with ImageMagick were inspected at 960x600 and
  720x440: readable title, limitation text and Quit control, no clipping.
  Evidence: `.amp/in/artifacts/m0-01-bootstrap.png`, `m0-01-small.png`.
- Failure evidence: software Vulkan configured but produced black captures;
  native GL test used `VK_DRIVER_FILES=/dev/null`. Direct-window captures also
  failed/hung; root captures succeeded. No claim that Vulkan/native GPUs pass.
- Setup: `bash -n .agents/setup`, `shellcheck .agents/setup` passed; installation
  11.903s; locked fetch warm run 3s, subsequent run 0.932s. No secrets required.
- Unrun: Windows CI (not pushed/dispatched), Windows build/native launch,
  hardware, accessibility and EasyWorship installed observation. `list_runners`
  returned no connected runners. No milestone gate waived.
- Delivery: local M0-01 implementation checkpoint, not an installer/release.
- Next: integrate M0-02 research and run M0-03/M0-04 isolated worktrees; obtain
  Windows dual-display/reference access for required qualification.

### M0-01 — Partial: orb prerequisites — 2026-10-04 (Asia/Jakarta)

- Scope: `.agents/setup`, `rust-toolchain.toml`, `.gitignore`, README setup notes;
  no application manifest, service, credentials or renderer selection.
- Upstream: Zed commit
  [`a846890`](https://github.com/zed-industries/zed/commit/a84689073d296dfd39987bc7dd478e43ef76d83a),
  `rust-toolchain.toml`, `script/linux`, and GPUI/Linux/WGPU manifests. This
  inspected revision is provenance, not yet Sela's GPUI dependency pin.
- Decisions: pin Rust 1.98.1 with rustfmt/clippy; install only missing Debian
  prerequisites; persist cargo discovery for login shells; no resume script
  because nothing needs authentication or service repair. Native package list
  must be revalidated against the eventual application feature set.
- Verification on Debian 12 x86_64 orb:
  - `bash -n .agents/setup` and `shellcheck .agents/setup`: passed.
  - `time .agents/setup`: first install 15.885s. Replaced deprecated rustup
    auto-install with explicit `rustup toolchain install --no-self-update --no-update`.
    Final script warm runs: 0.409s and 0.531s, no package downloads.
  - `RUSTUP_HOME="$fresh" time .agents/setup` with a disposable empty rustup
    directory: final-script fresh toolchain installation passed in 12.55s;
    temporary toolchain removed afterward.
  - `env -i HOME="$HOME" USER="$(id -un)" PATH=/usr/local/bin:/usr/bin:/bin
    /bin/bash -lc 'command -v cargo rustc; rustc --version'`: proxies found in
    `/home/user/.cargo/bin`, Rust 1.98.1. Rust 2024 stdin compile/run in the same
    clean login-shell environment printed `Rust compile/run smoke passed`.
    Initial smoke-command shell quoting failed; corrected command passed.
  - `pkg-config --modversion fontconfig wayland-client xkbcommon-x11`: 2.14.1,
    1.21.0, 1.5.0. `vulkaninfo --summary`: Mesa llvmpipe CPU device; not physical
    GPU/display qualification.
  - `grep -Fc '# Sela orb Rust environment' "$HOME/.bash_profile"`: one hook
    after repeated runs. `git check-ignore .amp/portals/test.json target/test`:
    both ignored. Documentation verifier and `git diff --check`: passed.
- Not run: GPUI build/render, app tests, Windows build/native launch, hardware
  checks; no application exists. Software Vulkan detection is not app execution.
- Delivery: local partial commit `chore(M0-01): prepare Rust and GPUI orb prerequisites`;
  no push or project-setting change. Future-orb activation awaits authorization.
- Resume: finish M0-01 upstream application-pattern review, dependency pin,
  manifest/window, CI and Windows verification. No full parent checkbox is
  checked because this setup slice does not satisfy an entire combined item.

### PLAN-001 — Executable roadmap and checkpoint discipline — 2026-10-04

- Request: detailed tickets from zero through release, resumable checklists,
  explicit test boundaries, comprehensive EasyWorship feature/layout/UX coverage,
  optimization and feasible E2E, with commits per ticket or partial ticket.
- Scope: repository documentation only; no GitHub issues, pushes or app changes.
- Decisions: track build-specific reference evidence separately from public
  feature descriptions; leave provider/legal and physical hardware gates explicit.
  Keep native E2E conditional, without weakening manual/hardware acceptance.
- Sources: existing `docs/plan.md`, root `AGENTS.md`, official EasyWorship feature
  page and search results linked in `docs/backlog.md`; no installed reference
  observation performed and no parity claimed.
- Changes: `docs/backlog.md`, `docs/work-log.md`, root guidance and README/plan links.
- Verification: `git diff --check` passed. Inline Python structure check passed:
  63 unique tickets, 326 ticket checklist items, dependency graph acyclic, all
  ticket references and local Markdown links valid. Reproducible final check below.
- Not run: app/unit/UI/E2E/hardware tests; this is documentation-only and there is
  no runnable application. Reference installation and provider rights remain unverified.
- Delivery: local commit `docs(PLAN-001): add executable backlog and checkpoint rules`;
  no push, GitHub issues, release or deployment performed.
- Resume: start M0-01 by inspecting/pinning upstream GPUI patterns and toolchain;
  no application ticket has been started. Do not jump to the full UI or database.

Documentation verification (run from repository root; Python 3 standard library):

```sh
git diff --check
python3 - <<'PY'
from pathlib import Path
import re
text = Path('docs/backlog.md').read_text()
parts = re.split(r'^### (M\d-\d{2}) — [^\n]+\n', text, flags=re.M)
tickets = dict(zip(parts[1::2], parts[2::2]))
assert len(tickets) == len(parts[1::2]), 'duplicate IDs'
graph = {}
for key, body in tickets.items():
    assert 'Acceptance:' in body, key
    assert len(re.findall(r'^- \[[ x]\]', body, re.M)) >= 5, key
    header = re.search(r'Depends on: (.*?)Tests:', body, re.S)
    assert header, key
    graph[key] = re.findall(r'M\d-\d{2}', header[1])
    assert all(dep in tickets for dep in graph[key]), key
visited, active = set(), set()
def visit(key):
    assert key not in active, ('cycle', key)
    if key in visited:
        return
    active.add(key)
    for dep in graph[key]:
        visit(dep)
    active.remove(key)
    visited.add(key)
for key in tickets:
    visit(key)
assert all(ref in tickets for ref in re.findall(r'\bM\d-\d{2}\b', text))
for path in [Path('README.md'), Path('AGENTS.md'), *Path('docs').glob('*.md')]:
    source = path.read_text()
    assert all(line == line.rstrip() for line in source.splitlines()), path
    for link in re.findall(r'\]\(([^)]+)\)', source):
        if '://' in link:
            continue
        filename, _, fragment = link.partition('#')
        target = path.parent / filename if filename else path
        assert target.is_file(), (path, link)
        if fragment:
            headings = re.findall(r'^#+ (.+)$', target.read_text(), re.M)
            anchors = [re.sub(r'[^\w\- ]', '', h.lower()).replace(' ', '-') for h in headings]
            assert fragment in anchors, (path, link)
count = sum(len(re.findall(r'^- \[[ x]\]', body, re.M)) for body in tickets.values())
print(f'PASS: {len(tickets)} unique tickets; {count} ticket checklist items')
print('PASS: dependencies acyclic; references, local links/anchors and whitespace valid')
PY
```

## Copy this record when beginning a ticket or partial slice

```text
### <ticket ID> — <slice title> — <date/timezone>

- State: active | blocked | implemented-unqualified | done | deferred
- Scope / checklist items addressed:
- Dependency and reference observation IDs:
- Decisions / upstream GPUI commit and paths (when applicable):
- Files / durable evidence paths:
- Verification: exact commands + decisive results + environment/build/fixture.
- UI evidence: inspected screenshots/states or interaction/accessibility results.
- Not run: checks and concrete reason; do not report these as passing.
- Remaining checklist / blockers / smallest unblock action:
- Next action: exact command, file/function or observation to resume with.
- Delivery: local partial/completed commit; pushed/published only if actually done.
```

Include the log/checklist update in the same commit as its implementation slice.
Use `git log --grep='<ticket ID>'` to find commits; no need to amend a commit merely
to put its own hash in this file. A blocked partial slice may be committed with
failing checks disclosed, but cannot be marked done or qualified.

### M0-02 — Partial: public-source ledger and Windows runbook — 2026-10-04

- State: **blocked** for installed observation; documentation checkpoint complete,
  not parent-ticket completion. Independently assigned alongside active M0-01;
  the current-state/index above describes the bootstrap checkpoint, not a claim
  that this parallel research has not started.
- Scope: `docs/reference-observations.md` and this appended record only. No
  README/backlog/plan gate changes, UI implementation, or completed M0-02 boxes.
- Evidence: ledger EW8-OBS-001–009, source register SRC-01–04, observation runs
  W01–W06 and separate Sela acceptance handoff G01 in
  [reference-observations.md](reference-observations.md).
- Sources: full official update, Quick Start, EasyWorship 7 shortcuts and Screen
  Setup pages fetched via `read_web_page(forceRefetch=true)` on 2026-10-04.
  Official listing explicitly says “Build 8.0.49 · Released Jun 30, 2026”.
  Historical guide says Preview navigation does not advance live output and
  Live double-click can exit Logo/Black/Clear; v7 shortcut guide assigns Ctrl+C
  both editor-copy and show text-toggle. These are documented-only, not observed
  8.0.49 behavior. Screen Setup describes extended desktop routing, not pacing.
- Decisions: preserve unknown mask precedence/restore/Go Live/transport/output
  scope and focus dispatch. Runbook separates reference compatibility from
  Sela's acknowledged-state, private-selection and output-independence contracts.
  No upstream GPUI pattern review needed: no code or GPUI implementation added.
- Verification: `git diff --check` passed; Python standard-library link/whitespace
  check below passed for both owned files (local links/anchors, unique ledger IDs,
  nine records). All four external source URLs returned full page content in the
  live fetches above. No proprietary installer/assets downloaded or copied.
- Not run: installed reference N/H/L entitlement confirmation, Windows/native
  app tests, physical display/GPU/audio, timing, E2E or failure injection. No
  lawful installed reference or Windows runner is available; public source access
  does not substitute for them. Zero installed observations/artifacts/passes.
- Remaining/next action: obtain authorized Windows operator with lawful 8.0.49,
  physical operator/audience displays and capture; execute W01 then W02/W03 static
  cases and append results/artifacts. Continue focus/routing/full inventory;
  G01 requires the Sela spike and M0-03/M0-08/M0-09 prerequisites. No gates waived.
- Delivery: local `docs(M0-02): checkpoint reference ledger and Windows runbook`
  commit on `ticket/m0-02-reference`; no push, publication or additional agents.

Reproduce this slice's documentation check from repository root:

```sh
git diff --check
python3 - <<'PY'
from pathlib import Path
import re
for path in map(Path, ['docs/reference-observations.md', 'docs/work-log.md']):
    source = path.read_text()
    assert all(line == line.rstrip() for line in source.splitlines()), path
    for link in re.findall(r'\]\(([^)]+)\)', source):
        if '://' in link:
            continue
        filename, _, fragment = link.partition('#')
        target = path.parent / filename if filename else path
        assert target.is_file(), (path, link)
        if fragment:
            headings = re.findall(r'^#+ (.+)$', target.read_text(), re.M)
            anchors = [re.sub(r'[^\w\- ]', '', h.lower()).replace(' ', '-') for h in headings]
            assert fragment in anchors, (path, link)
ledger = Path('docs/reference-observations.md').read_text()
ids = re.findall(r'^\| (EW8-OBS-\d{3}) \|', ledger, re.M)
assert len(ids) == len(set(ids)) == 9
print('PASS: two owned files; local links/anchors and whitespace; nine unique ledger IDs')
PY
```

### M0-03 — Bounded operator harness — 2026-10-04 (Asia/Jakarta)

- State: **implemented-unqualified**, bounded local checkpoint, not full parent
  completion. Dependency M0-01 Linux bootstrap executed; its Windows gate remains
  open. Scope: actual Operator action/focus tests, X11 smoke and native runbook.
  No Cargo manifest/lock changes, audience spike or speculative operator features.
- Files: `src/main.rs` (shared binding setup only), `src/tests.rs`,
  `scripts/native-smoke.sh`, `docs/native-testing.md`, M0-03 backlog boxes and
  this appended record. Upstream pin/path/license and accessibility findings are
  recorded in the native runbook; inspected the existing pinned cargo checkout.
- U/I: `CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo test --locked -j 4`
  passed **3 tests**, 0 failed/ignored. Actual Operator tree, production binding,
  retained focus, input dispatch, blur/refocus, unbound key, wrong context and
  explicit direct-action contrast. Capture observer does not replace the Quit
  handler; upstream test platform quit is no-op, so only native proves exit.
- Checks: `cargo fmt --all -- --check`,
  `CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo clippy --locked --all-targets -j 4 -- -D warnings`,
  `CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo build --locked -j 4`,
  `bash -n scripts/native-smoke.sh`, `shellcheck scripts/native-smoke.sh`,
  `git diff --check`: passed. Cargo serialized shared build locks normally.
- Native: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh /home/user/workspace/repo/target/debug/sela .amp/in/artifacts`
  passed; immediately repeated without artifact argument, passed. Each run
  launched twice, verified PID-window focus, Ctrl+J survival, 720x440 resize,
  Ctrl+Q and Alt+F4 exit status 0 and window removal. Only test PID/windows used;
  existing supervised Xvfb/Openbox/xcompmgr services were not changed/stopped.
- Environment: Debian 12 x64 orb, Xvfb :99 1600x1000, Mesa llvmpipe software GL;
  Vulkan black-capture limitation remains. Binary SHA-256 at verification:
  `ac83cb51fbbe72b477b359d74c59d3facff6566be9de92c834a38912792ce88c`.
- UI evidence: root captured then cropped, inspected
  `.amp/in/artifacts/m0-03-native-small.png` (720x440): readable title,
  limitation text and Quit control, no text clipping. Native stderr retained in
  `.amp/in/artifacts/m0-03-native.log`; temporary root/profile/runtime cleaned.
  Artifacts are local ignored evidence, not committed assets.
- Failure checks: `DISPLAY=:99 scripts/native-smoke.sh /tmp/sela-no-such-binary`
  returned nonzero with missing-binary diagnostic and no leftover smoke directory.
  Explicit window XSendEvent input trial failed (BadWindow/quit timeout), cleaned
  its process, and was replaced with focus-verified XTEST; two final passes above.
  Concurrent focus-changing automation remains unsafe; coordinate native input.
- Not run: Windows build/native UIA/Narrator, physical GPU/displays, Wayland,
  performance, installed EasyWorship compatibility. No suitable Windows/physical
  runner/reference in this orb. Manual equivalent and non-skipping policy are in
  `docs/native-testing.md`; upstream AccessKit plumbing is not Sela accessibility
  qualification. Wrong-context negative is headless only, not a fabricated UI.
- Remaining: first M0-03 combined fixture/clock/failure checklist stays open:
  conventions and temporary XDG dirs exist, but clocked domain and I/O fixtures
  do not yet exist. Extend them when actual domain adapters arrive; no test-only
  production flags. Integrator should rerun tests/native smoke after merging,
  then execute Windows/manual accessibility fallback on a real runner.
- Delivery: local `feat(M0-03): checkpoint bounded GPUI and X11 harness` commit;
  no push/publication. Full ticket qualification remains open.

### M0-04 — Linux audience process spike — 2026-10-04 UTC

- State: **implemented-unqualified** parallel slice; Windows parent gate open.
- Scope: opt-in `examples/output_spike.rs`, example dev dependencies/lockfile,
  `scripts/output-spike.py`, `docs/output-spike.md`, M0-04 checklist only. No
  `src/main.rs` or native harness changes; no production renderer/scene protocol.
- Upstream: bootstrap GPUI pin and exact winit 0.30.12 example/wgpu 29.0.4
  API rustdoc source paths and license boundaries recorded in output-spike note.
- Chosen boundary: independently supervised audience process owns winit/wgpu GL
  surface/event loop; monotonic triangle motion, ~16.667ms pacing, bounded 2048
  telemetry queue off frame path, explicit native close, 25s self deadline.
- Verification (CARGO_TARGET_DIR=/home/user/workspace/repo/target):
  `cargo build --locked --example output_spike -j 4`,
  `cargo test --locked --example output_spike -j 4` (one test passed),
  `cargo clippy --locked --example output_spike -j 4 -- -D warnings`,
  `rustfmt --edition 2024 --check examples/output_spike.rs`, `git diff --check`;
  `python3 scripts/output-spike.py` passed on shared :99 Xvfb/Openbox/xcompmgr,
  Debian12 llvmpipe Mesa22.3.6 GL. Initial missing display handle adapter error
  fixed against wgpu documented owned-display API; no shared service changed.
- N/I/P: actual 100/500/2000ms GPUI callbacks stalled; 6/30/118 audience calls
  inside exact intervals, queued 100ms action only started after stall. Clean
  operator exit and second operator SIGKILL had 218/72 subsequent calls.
  621 calls, interval p50/p95/max 16.870/19.793/47.308ms. Native resize and close
  passed; all owned processes reaped. Not physical scanout or 60Hz qualification.
- Durable timestamp table, failure boundaries, commands and decision in
  `docs/output-spike.md`; review artifacts `.amp/in/artifacts/output-spike/`
  raw logs/summary/root capture pairs. Inspected cropped root stall geometry
  visibly moved; no direct application capture. Minimal diagnostic controls,
  not EasyWorship parity. Bottom help at small window is a known layout limit.
- Not run: slow preparation, forced surface/device loss, Windows DX12/native
  mixed-DPI dual physical displays/fullscreen/hotplug, physical scanout/cue
  latency, text-over-video, production lifecycle or memory/CPU/GPU soak. Orb
  software display cannot replace these gates; parent not done.
- Next: integrate this local slice; obtain Windows hardware and owning two-process
  supervisor, run backend dx12 with physical capture; extend failure/preparation
  tests before M0-10 decision. Local coherent M0-04 commit only; no push.
- Final rerun after telemetry 1KiB cap/suboptimal surface-release ordering:
  same build/test/clippy/fmt/diff commands passed; `python3 scripts/output-spike.py
  .amp/in/artifacts/output-spike-final` passed, 619 calls, stall calls 6/30/117/6,
  post-exit/kill 214/70; p50/p95/max 16.854/18.152/49.197ms. Prior measurement
  artifacts retained; final exact 2s/exit/kill timestamps appended in spike note.

### M0-04 — Integration and delayed preparation — 2026-10-04 (Asia/Jakarta)

- Scope: integrated M0-02/M0-03/M0-04 worktrees in local main, retained both
  work-log records when resolving the append conflict; no implementation conflict.
  Added single in-flight delayed background task and busy/completed feedback.
  Uses pinned GPUI background executor/entity task/update patterns from
  `crates/gpui/examples/testing.rs`; no actual decoder or production scene protocol.
- Corrected driver title-only targeting and hard-coded orb checkout/environment:
  now PID-scoped, verifies focus on every key, uses private temporary runtime,
  inherits explicit DISPLAY/backend choice, closes/reaps only owned processes.
  Captures root to temporary storage and retains cropped owned windows only.
  Strengthened interval assertions (complete ordered stalls, boundaries included,
  ≥500ms stall-gap rejection as diagnostic only), actual clean-exit status and
  one worker/busy rejection/control action entirely within preparation interval.
- `cargo fmt --all -- --check`, `cargo build --locked --all-targets -j 8`,
  `cargo test --locked --all-targets -j 8` (**4 passed**),
  `cargo clippy --locked --all-targets -j 8 -- -D warnings`,
  `uvx ruff check scripts/output-spike.py`, Bash syntax/ShellCheck, and
  `git diff --check`: passed. CI commands now include example tests; remote
  workflows have not been pushed or run. Formatter changes were reread.
- Native integrated baseline: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  scripts/native-smoke.sh /home/user/workspace/repo/target/debug/sela .amp/in/artifacts`
  passed after merge and again after preparation changes. Rendered resized
  bootstrap inspected: no clipped labels. No fake web UI or browser evidence.
- `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/output-spike.py
  .amp/in/artifacts/output-preparation` passed on Debian12 Xvfb/Openbox/xcompmgr,
  Mesa22.3.6 llvmpipe GL, debug build. 898 present calls; p50/p95/max intervals
  16.828/17.757/46.039ms; 118 inside 2s UI stall; 237/76 after clean exit/SIGKILL.
  During 2s background delay the 100ms UI handler completed before worker finish,
  repeated preparation was rejected, and audience maximum gap was 41.877ms.
  Exact correlation times are in `docs/output-spike.md` and artifact summary.json.
- Inspected preparation pending/completed screenshots and two-frame comparisons
  during UI stall/after exit: readable controls/help and changed triangle position.
  Artifact directory `.amp/in/artifacts/output-preparation/`; captures/logs are
  local evidence, not physical scanout or hardware performance qualification.
- Failure checks: subprocess runs with missing DISPLAY, nonexistent target binary
  and invalid X server all returned nonzero within 15s, produced no success
  summary, and left no `sela-output-*` runtime directories. Initial Ruff findings
  (imports/mode/file scope/style) corrected with formatter and scoped file lifetime.
- Remaining blockers: Windows build/native launch/DX12 physical dual-monitor
  mixed-DPI/fullscreen/hotplug/recovery; lawful installed EasyWorship 8.0.49
  observations; software Vulkan black-capture finding. Production command,
  scene, text/video and data/UI tickets have not been implemented or qualified.
  M0-10 entry-to-M1 gate is not waived; usable/installed Sunday app is not delivered.
- Next concrete action: connect an authorized Windows runner with physical output
  and reference access; execute `docs/reference-observations.md` W01–W05 and the
  Windows spike sequence in `docs/output-spike.md`, recording actual evidence.
- Delivery: local ticket commits plus worktree merges and this M0-04 checkpoint;
  no push, installer, signed package, publication or deployment.
- Final identical-source repeat passed: 883 calls; 117 during 2s stall;
  post-exit/kill 241/75; preparation max gap 54.505ms. Summary retained as
  `.amp/in/artifacts/output-preparation/repeat-summary.json`; transient duplicate
  captures removed. Native setup commands are now explicit in the runbook.

### M0-05 — CPU preparation checkpoint — 2026-10-04 UTC

- State: **implemented-unqualified** partial slice. Framework-independent owned
  scene/version/resource types and one bounded worker, no new UI or production
  renderer/backend selection. Detailed ownership/budgets/limitations are in
  [preparation.md](preparation.md). Direct pinned image/hash/font dependencies
  were already transitive; lock update adds only root dependency entries.
- Worker reads and hashes capped source files, decodes static PNG/JPEG, parses
  supplied font face 0, returns read-only snapshots. Acceptance supersedes older
  work; saturation/invalid input does not. Exact deadlines reject even queued
  results; cancellation and teardown do not wait on blocking OS calls.
- U/I: `cargo test --locked --all-targets -j 8` passed **14 tests** (10 new
  preparation tests, 3 GPUI tests, 1 spike test). Asymmetric RGBA/alpha and JPEG
  normalization, immutable font/text/image snapshots, missing/changed/corrupt
  resources, invalid dimensions, budgets/overflow, worker saturation/failure,
  queued stale results, exact deadline, recovery and nonblocking cancellation/drop.
- `cargo clippy --locked --all-targets -j 8 -- -D warnings`,
  `cargo fmt --all -- --check`, `git diff --check` and existing inline documentation
  checks passed. Initial clippy array-chunk warning corrected with clippy's fix;
  reran tests and strict clippy. Formatter/fix output reread. Font SHA-256 matched
  fixture README; full redistribution notices retained (trailing spaces removed).
- Not run/claimed: rendering/shaping/glyph coverage, GPU upload/readiness,
  APNG fixture regression, measured RSS, Windows/hardware or physical display
  latency. These modules are not yet wired to the native spike. U/I resource
  readiness is not presentation acknowledgment. Parent combined acceptance boxes
  remain open where downstream renderer work is needed.
- Delivery: local `feat(M0-05): checkpoint bounded CPU scene preparation` commit;
  no push or installer. Next: M0-06 bounded ordered submission and acknowledgment
  state; keep safety capacity tests separate from unobserved mask semantics.

### M0-06 — Ordered delivery model checkpoint — 2026-10-04 UTC

- State: **implemented-unqualified** partial slice. `src/delivery.rs` separates
  requested/accepted/applied/unknown state, one global sequence per fresh epoch,
  FIFO normal/safety admission with one outstanding slot each, renderer-side
  rejection and idempotent pending/current duplicates. No IPC or native rendering
  callback is implemented. Full contract and integration obligations:
  [delivery.md](delivery.md).
- Deadline is checked before the callback, not after changing output. Receipt
  timeout/disconnect invalidates the session, clears requests and never replays
  them. Last-confirmed history is not labelled live after disconnect. Ambiguous
  stale receipts require newer confirmation or mark output unknown.
- Verification: `cargo test --locked --all-targets -j 8`: **22 passed** (18 library,
  3 GPUI, 1 spike). Eight new delivery tests cover all 24 receipt permutations,
  both lane orders, 1000 rejected admissions per lane order, duplicate/reordered
  commands, render failure retaining prior Arc, exact deadline/no callback,
  restart/foreign/unsent receipts, renderer overload and real missing-file worker
  recovery. `for run in $(seq 1 20); do cargo test --locked --lib -j 8 --quiet;
  done` passed all 20 runs (18 tests/run); no randomized schedule claim.
- `cargo build --locked --all-targets -j 8`, strict all-target clippy,
  `cargo fmt --all -- --check`, `git diff --check` and inline documentation
  checks passed. Formatted code and entire test module reread/reviewed.
- Not run/claimed: actual safety-control semantics or timing, renderer IPC,
  preparation-intent coordination, receipt-to-GPU-frame integration, physical
  scanout, Windows/reference/hardware. Reserved lane currently holds a prepared
  replacement snapshot, not an invented Black/Clear/Logo state machine. Combined
  parent boxes remain open for those integration requirements.
- Delivery: local `feat(M0-06): checkpoint ordered delivery and acknowledgment model`
  commit, no push/installer. Next: M0-07 explicit-font composition/readback;
  integrate resource readiness before applying this model to the native spike.

### M0-07a — Explicit-font CPU text raster — 2026-10-04 UTC

- State: **implemented-unqualified** parallel CPU diagnostic slice on
  `ticket/m0-07-text`, based on local main 71dc754, not origin/main. Owns only
  `examples/composition/text.rs`, Cargo dev dependency/root lock entry,
  `docs/composition-text.md` and this appended record. Parent boxes are left to
  integration; no main example, src, other modules or backlog edits.
- API: supplied single-face font bytes and original UTF-8 text to bounded
  row-major white alpha; Advanced shaping, explicit lines, no wrap/margin,
  all-line logical and raster ink overflow rejection, source-over mark coverage.
  Collection headers rejected before fontdb allocation; empty supplied-only DB.
  Per-job caches dropped; not a hard memory/time bound or adversarial sandbox.
- Upstream commits/paths, dependency/fixture notices, bounds and exact harness
  commands are in [composition-text.md](composition-text.md). No GPUI code added.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --example text_check -j 4` **3 passed**;
  `cargo clippy --locked --example text_check -j 4 -- -D warnings`,
  `cargo run --locked --example text_check -j 4`,
  `rustfmt --edition 2024 --check examples/composition/text.rs examples/text_check.rs`
  passed. Initial iterator move/borrow error fixed and checks rerun. Temporary
  harness and `/tmp` PGM removed before commit; module reread after formatting.
- Inspected ignored `.amp/in/artifacts/m0-07a-text.png`: readable Café, acute a,
  Arabic سلام and explicit Signal beacon second line; no clipped ink. Tests also
  reject missing glyph, both logical overflow axes, left ink bearing, malformed/
  collection fonts and oversized inputs. `git diff --check` passed.
- Not run: GPU/native upload/readback or golden/transition/video, Windows,
  physical GPU/displays, reference compatibility, performance/RSS. CPU checks
  do not qualify parent M0-07 or waive M0-10. Local subticket commit only, no push.
- Next: integrator includes this module in composition example and reruns example
  tests/clippy plus GPU readback; then execute Windows/reference/hardware gates.

### M0-07b — Offscreen static GPU composition/readback — 2026-10-04 UTC

- State: **implemented-unqualified** subticket on `ticket/m0-07-compose`, based
  on local main `71dc754` M0-05/M0-06 contracts. Assigned GPU module/docs only;
  parent owns permanent common example/checklist and parallel explicit-font mask.
- Files: `examples/composition/gpu.rs`, `docs/composition-gpu.md`, this one
  appended record. No manifest/src/text/backlog/main-example changes. Original
  embedded WGSL full-screen triangle loads nearest sRGB image and linear R8
  coverage, composites straight-alpha image over opaque black and white glyphs
  in linear light, writes sRGB RGBA8, strips padded GPU readback rows.
- Decisions/upstream: exact pinned wgpu29.0.4 MIT/Apache-2.0 API paths,
  coordinate/color policy, per-resource 64MiB/8192/device caps, 10s poll + 1s
  callback wait and remaining default-error/driver-memory boundaries documented
  in [composition-gpu.md](composition-gpu.md). No GPUI patterns or new deps.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --example compose_gpu_check -j 4` (2 GPU-free tests),
  `cargo test --locked --all-targets -j 4` (**24 passed**),
  `cargo clippy --locked --example compose_gpu_check -j 4 -- -D warnings`,
  `rustfmt --edition 2024 --check examples/composition/gpu.rs examples/compose_gpu_check.rs`,
  `git diff --check`: passed. Initial v29 API compile errors and Clippy findings
  corrected, final checks rerun; automatic chunk fix rolled back before explicit
  reference-comparison correction. Formatted code reread.
- R/GPU: `runtime=$(mktemp -d /tmp/sela-compose-runtime.XXXXXX)`;
  `XDG_RUNTIME_DIR="$runtime" cargo run --locked --example compose_gpu_check -j 4`;
  `rmdir "$runtime"`: passed repeatedly using actual Vulkan llvmpipe LLVM15.0.6
  CPU adapter, Mesa22.3.6, Debian12 x64 debug orb, no surface/native service
  changes. Initial successful run without XDG directory had runtime warnings;
  private-runtime runs did not. No GL fallback or global environment mutation.
- Real readback checked asymmetric 3x2 RGBA orientation/nonaligned rows, both
  contain/cover aspect axes and crop boundaries, transparent background, zero/
  half/full coverage (~188 half-white, not 128), midtone linear-light color,
  input rejection and recovery. Retained `Compositor::check_readback()` allows
  parent to repeat all pixel checks; normal unit tests never request GPU.
- Evidence: ignored `.amp/in/artifacts/m0-07b-gpu.png` (321x180) inspected via
  media tool: centered four-color image, black sidebars, white synthetic H and
  half-coverage bar visible. Synthetic mask only, not font/shaping evidence.
  Temporary harness removed before commit; full reproduction is in module note.
- Not run: explicit-font integration, Windows/DX12, physical GPU/display or
  scanout, native E2E, forced loss/OOM/timeout, cut/fade/video, performance/RSS or
  installed-reference checks. Software offscreen pixels do not qualify them;
  parent M0-07 remains open. Budget is per resource, not total resident memory.
- Next: integrator includes GPU module in common example, calls diagnostic
  `check_readback()` only opt-in and feeds CPU worker's exact output-size alpha
  mask into `render`; rerun all-target checks and inspect actual font frames.
- Delivery: local `feat(M0-07b): add offscreen GPU composition and readback`
  commit; no push/publication/backend selection or waived qualification.

### M0-07 — Parallel integration and actual-font GPU frames — 2026-10-04 UTC

- Merged text/GPU worktrees via separate merge commits; only conflict was two
  appended work-log records, both retained. Reviewed both implementations and
  dependency diff. Added permanent `composition_spike` example linking the actual
  M0-05 preparer, explicit-font worker and GPU composition; no fake web UI or
  optimistic M0-06 rendering acknowledgment. README and module handoffs now link
  [composition-spike.md](composition-spike.md) instead of requiring scratch code.
- `cargo test --locked --all-targets -j 8`: **27 passed**, 0 failed/ignored.
  `cargo clippy --locked --all-targets -j 8 -- -D warnings`,
  `cargo build --locked --all-targets -j 8`, `cargo fmt --all -- --check`, diff and
  inline documentation checks passed. Initial format-macro compile typo fixed;
  Clippy's const-expression chunk autofix produced invalid syntax and rolled
  itself back; corrected the const generic braces manually and reran strict checks.
  Final code/formatter output reread.
- R: `XDG_RUNTIME_DIR="$runtime" target/debug/examples/composition_spike
  .amp/in/artifacts/composition vulkan` passed twice, with private mktemp runtime
  removed afterward. Full per-pixel GPU checks plus actual white-font mask and
  uncovered-background checks. Both source image/font files removed before
  rendering. Overflow fixture rejected, never silently clipped/uploaded.
- Inspected all 641×360 color/contain/cover images: readable Latin accents,
  combining mark and connected Arabic; correct contain sidebars/cover filling,
  no glyph boxes/clipping/row corruption. Latest retained representative frame
  inspected again after measured repeat. Evidence in `.amp/in/artifacts/composition/`.
- P diagnostic only: Debian12 x64, 16-vCPU Xeon 2.60GHz, debug, Vulkan CPU adapter
  llvmpipe LLVM15.0.6 Mesa22.3.6. `/usr/bin/time -v` measured one full binary run:
  wall 0.51s, max RSS 115540KiB, no swaps; preparation/raster 42.226ms,
  color/contain/cover allocation/upload/readback 5.271/77.773/12.595ms.
  No frame-time/SLO/physical-GPU claim; note records variability and exact boundary.
- N regression: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh
  "$PWD/target/debug/sela"` passed focus/unbound key/resize/Ctrl+Q/WM-close/exit.
  CLI failure checks via `subprocess.run(..., timeout=10)` passed for missing
  arguments and unsupported backend, nonzero with diagnostic and no output dir.
- Remaining: native audience GPU-ready scene/IPC/receipt integration; video
  surface/copy/sync spike; observed cut/fade/interruption/masks; goldens, resource
  recovery, production memory/performance, Windows/hardware. Refreshed Amp
  `list_runners`: **none connected**. Lawful installed 8.0.49 and physical Windows
  dual-display checks are still required; M0-10/full-M1 gate is not waived.
- Delivery: local integration checkpoint, worktree commits retained; no push,
  installer or service-ready app. Resume partial M0-07 integration above. Connect
  an authorized Windows/reference environment to unblock observed transitions,
  M0-08 and the hardware gate; do not label static PNGs a usable worship app.

### Sequencing approval and next slices — 2026-10-04 UTC

- Owner cannot currently provide hardware and explicitly authorized continuing
  operator UI first while renderer testing remains unavailable. Recorded the
  exception in plan/backlog; no release or parity gate marked done/deferred.
- M0-07c scope: bounded FFmpeg child decode of an original lossless FFV1 fixture,
  complete RGBA frames to existing wgpu compositor, ordered frame/copy checks,
  overload/cancel/decoder failure, exact limits and licensing/interop findings.
- M1-02a scope: original native GPUI Schedule/Preview/Live over Resources shell,
  empty/disconnected/disabled states, resource tabs, bounded splitters and compact
  window. Historical Quick Start Guide supports pane families/resource tabs only;
  exact 8.0.49 dimensions/focus/modes remain unverified. Initial geometry is
  reversible development policy, not a new claim of compatibility.
- Upstream review completed for pinned GPUI div drag/focus, canvas, window bounds
  observation and test mouse APIs; current upstream comparison matched the pin.
  Keep framework Apache APIs, not GPL Zed application UI components.
- Independent worktrees will own video example/modules versus operator source/
  tests/native driver. Merge with local ticket commits and combined checks;
  serialize native focus-changing tests. No push/publication authorization.

### M1-02a — Empty operator shell — 2026-10-04 UTC

- State: **implemented-unqualified**, early-UI exception; local main `115f545`
  base on `ticket/m1-02-shell`, not origin/main. No physical/reference gate waived.
- Scope: `src/main.rs`, original `src/operator.rs`, `src/tests.rs`,
  `scripts/operator-shell.py`, `docs/operator-shell.md`, this appended record.
  No library/contracts, manifests, examples, plan/backlog/README changes.
- Separate Schedule/Preview/Live over full-width Resources; provisional 24/38/38,
  five truthful empty tabs, collections, collapse/restore/reset, three bounded
  proportional splitters. Live disconnected; safety/Go Live have no handlers.
  Retained root focus and semantic clickable/Ctrl+Q Quit; no storage/import claims.
- Pinned upstream Apache framework paths/licenses and historical-reference limits
  are recorded in the shell note. Persistence/other modes remain parent work.
- Verification: shared CARGO_TARGET_DIR; `cargo test --locked --all-targets -j 4`
  **29 passed**; bin build, strict all-target clippy, fmt check, Ruff and diff check
  passed. Two new actual-tree tests cover tabs, collapse/restore/reset, compact
  bounds, every drag/release, focus, cross-axis isolation and subminimum fallback.
- Native: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/operator-shell.py`
  and existing native-smoke passed on shared Xvfb/Openbox/xcompmgr software GL;
  owned app reaped, shared services unchanged. Installed x11-utils for xwininfo.
  Corrected decoration-offset crops/input and drag cross-axis bug after visual
  inspection; regression assertion added. Initial restore test reused the old
  button position after collapse; corrected to query its new bounds.
- Inspected `.amp/in/artifacts/operator-shell/{normal,compact,scriptures,collapsed,
  drag-minimum}.png`: readable 1280x800/720x440, true tab/restore states, bounded
  minimum drag. Details/exact repeat commands in `docs/operator-shell.md`.
- Unrun: Windows, hardware, mixed DPI, installed 8.0.49, accessibility and
  performance; no parity or audience qualification claim. Next: parent merges,
  reruns combined tests/native checks, maintains persistence/modes/qualification.
- Delivery: local `feat(M1-02a)` commit only; no push/publication.

### M0-07c — software decoder/GPU interop — 2026-10-04 UTC

- Implemented-unqualified diagnostic in `examples/video_spike.rs` and
  `examples/video/decoder.rs`; details, limits, source/legal review, exact
  reproduction and remaining work in [video-spike.md](video-spike.md).
- Original private 12-frame 320×180/12fps FFV1 BGRA Matroska decoded off caller
  into capacity-one complete RGBA queue. Supervisor kills/reaps on cancellation,
  Drop/deadline; polling does not join. Explicit local format/codec/protocol,
  output-size forcing, 1920×1080/120-frame/32MiB/30s limits; no new dependencies.
- Checks with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --example video_spike -j 8`: **9 passed**;
  `cargo clippy --locked --example video_spike -j 8 -- -D warnings`,
  `cargo build --locked --example video_spike -j 8`, `cargo fmt --all -- --check`
  passed. Initial Clippy constant-chunk warning fixed; final files reread.
- R: private mktemp XDG runtime, `target/debug/examples/video_spike
  .amp/in/artifacts/video` (shared target absolute path) passed twice on Vulkan
  llvmpipe/Mesa22.3.6, FFmpeg5.1.9-0+deb12u1 GPL build. All original frame bytes,
  order, actual-font white glyph/uncovered GPU pixels, missing/corrupt errors,
  deterministic queue-saturation deadline/cancel, child reaping checked.
  Ordinary tests need neither FFmpeg nor GPU; static Unix child tests additionally
  cover partial EOF, bounds, watchdog and asynchronous Drop.
- First/last PNGs inspected under `.amp/in/artifacts/video`: readable actual text,
  asymmetric green stripe, moving pink rectangle. No native X11 used. No native
  hardware/Windows, original PTS/audio/zero-copy, frame-SLO or hostile-input
  sandbox claim. Encoder fixture setup remains synchronous without a watchdog;
  production lifecycle/resource qualification remains open.
- Next: parent merge and combined validation/backlog update; carry unresolved
  native surface/fence/PTS/audio/device/memory work forward. Local coherent
  `feat(M0-07c)` commit only; no push or waived release gates.

### M0-07c — Parent integration review and failure fixes — 2026-10-04 UTC

- Merged the shell and decoder worktrees locally, preserving both ticket commits
  and both appended records. Reviewed actual implementations before integration.
- Fixed terminal decoder failures exposing buffered frames, completion racing the
  final poll, and the fixture encoder's unbounded exit wait. Added real-child
  success/failure final-buffer and encoder timeout/error regressions.
- `cargo test --locked --all-targets -j 8`: **40 passed** (11 video tests).
  `cargo clippy --locked --all-targets -j 8 -- -D warnings`,
  `cargo build --locked --all-targets -j 8`, `cargo fmt --all -- --check` passed.
  Formatted implementation and diff reread. Runtime private mktemp directory:
  `XDG_RUNTIME_DIR="$runtime" target/debug/examples/video_spike
  .amp/in/artifacts/video` passed all 12 exact frame/readback and failure checks.
- Parent inspected both first/last PNGs: readable actual text, left green stripe,
  rectangle moves from left to right across the captured endpoints, no corruption.
  Actual ordering/bytes verified by the executable, not inferred from screenshots.
- Remains diagnostic-only: no native playback, audio, original PTS, physical
  Windows/GPU qualification or hostile-process sandbox. Kernel metadata/spawn
  latency and inherited stdout descendants are outside the watchdog contract.
- Delivery: local ticket fix checkpoint; no push or release. Continue operator
  work under the approved sequencing exception; native renderer integration and
  observed transitions remain open.

### M1-02a — Shell integration and native evidence — 2026-10-04 UTC

- Parent reviewed and merged original worktree commit. Fixed collapsed layout's
  unused six-pixel gutter; added all three lower drag-boundary assertions, alongside
  existing upper-bound tests. Native driver uses repo-relative target by default
  and distinguishes asserted input/exit checks from visually inspected state.
- `cargo test --locked --all-targets -j 8`: **40 passed**, including five operator
  tests. All-target strict Clippy/build, fmt, Ruff and diff checks passed.
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/operator-shell.py` and
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh
  "$PWD/target/debug/sela"` passed on Debian12 Xvfb/Openbox software GL.
- Parent inspected normal, compact, Scriptures, collapsed and drag-minimum PNGs
  under `.amp/in/artifacts/operator-shell/`: correct selected label, full collapsed
  height, minimum 150px Schedule, no clipped controls or pane overlap. Automated
  GPUI tests assert state/bounds; captured images alone do not prove transitions.
- Persisted installed xwininfo and FFmpeg diagnostic prerequisites in `.agents/setup`.
  `bash -n .agents/setup`, `shellcheck .agents/setup`, two setup runs passed (2s/1s);
  no service restart. These setup changes are only local, not active for future
  project orbs until shipped. README/backlog now reflect the merged diagnostics.
- New owner direction: retain EasyWorship layout 1:1 as the compatibility target,
  modern Zed visual styling, not the dated Painter concept. Begin M1-02b styling
  only; preserve placement/behavior and original assets. Zed HEAD matches our pin;
  inspect One Light/settings and button/tab source as design observations only.
- Windows/reference/mixed-DPI and persistent geometry/modes remain open. No
  service-ready/installable release claim, no push. Next: implement styling,
  capture all affected states again, then continue remaining operator tickets.

### M1-02b — Direct GPUI styling checkpoint — 2026-10-04 UTC

- Implemented original neutral chrome, typography hierarchy, thin visual dividers
  with unchanged drag hit areas, subtle hover/pressed buttons and selected tabs.
  No pane/control-region reorder, invented menu or live behavior. Provenance and
  license boundary are in `docs/operator-shell.md`; no Zed components/assets copied.
- Owner clarified design references as Codex/T3code/Notion/Zed and prohibited
  further Painter use. Recorded in AGENTS/plan. Early generated studies are not
  specifications or accepted designs; future iteration is actual GPUI only.
- `cargo test --locked --all-targets -j 8`: **40 passed**; strict all-target
  Clippy, fmt, bin build, Ruff and diff checks passed. Native shell driver and
  native-smoke passed with `DISPLAY=:99 VK_DRIVER_FILES=/dev/null`.
- Inspected actual normal/compact/Scriptures/collapsed/minimum-drag/hover captures
  under `.amp/in/artifacts/operator-shell/`. Layout and labels fit both sizes;
  hover gives subtle feedback, all empty/disconnected claims remain explicit.
  Direct compact-image review rejected an automated visual-analysis false alarm:
  Preview label really is centered in its taller canvas; Live has bottom controls.
- This is not owner design approval, full feature parity or a usable worship
  release. Persistent layout, modes, accessible controls and content authoring
  remain. Local styling commit only; no push/publication.
- Next independent slices: M1-01a durable song/schedule repository as prerequisite
  to real operator content, and M1-03a keyboard access to existing shell controls.
  Both proceed under the UI-first sequencing exception; no output gate waived.

### M1-01a — Durable song and schedule repository — 2026-10-04 UTC

- State: **implemented-unqualified** prerequisite on `ticket/m1-01-store`, based
  on this thread's local main `c937670`, not origin/main. Scope limited to new
  `src/storage.rs`, `src/lib.rs` export, Cargo dependency/lock, `docs/storage.md`
  and this appended record. No operator/main/tests/backlog/plan/setup edits.
- Read project guidance, plan/backlog/work-log before implementation; proceed
  under owner's explicit UI/prerequisite sequencing exception. No parent done
  claim, physical/reference gate waiver or fabricated theme/asset schema.
- API and schema evidence: [storage.md](storage.md). Stable random 128-bit IDs,
  duplicate titles, bounded lossless Unicode song payload, immutable revision
  history, tombstone deletes and ordered FK-backed schedule snapshots. Expected
  head conflicts and whole schedule transaction rollback are explicit.
- Initial empty schema 0→1 transaction only; unsupported/newer/foreign/corrupt
  files rejected without reset/replacement. Dependency pin rusqlite 0.40.2,
  bundled SQLite 3.53.2, limits. Authoritative rustdoc and downloaded dependency
  license/source notices inspected; no GPUI patterns or live contracts changed.
- One background connection/thread per Worker; async open, one outstanding
  bounded command/result including unconsumed completion, nonblocking submit/
  poll/drop. Pre-start cancel does no command I/O; started transactions return
  actual commit/failure, never a fabricated canceled success. Errors omit paths,
  SQLite messages and content. Synchronous Repository is documented worker-only.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target` on Debian
  12 x64 orb: `cargo test --locked --all-targets -j4` **48 passed**, zero failed/
  ignored; `cargo clippy --locked --all-targets -j4 -- -D warnings`,
  `cargo fmt --all -- --check`, `git diff --check` passed. Final code/format and
  lock diff reviewed. No existing dependency version upgraded; eight new
  dependency packages plus unified hashbrown feature added by Cargo.
- Eight new U/I tests use disposable databases for reopen/Unicode/CRLF/LF/empty
  sections/duplicate titles; snapshot survival after edit/delete; stale revision
  and FK failure byte-unchanged rollback; injected second-item trigger failure;
  initial migration rollback; byte-preserved newer/foreign/corrupt files; real
  competing lock (~100ms timeout), actual max_page_count disk-full rollback/
  reopen; pre-start cancel, race-legal actual result, queue busy and nonjoining
  Drop. No skipped DB checks; test fixtures removed by TempDir.
- Not run: GPUI editor/native E2E, Windows, installed reference, physical GPU/
  displays, crash/power-loss recovery, host-volume exhaustion or performance/
  RSS qualification. No UI integration exists yet; no suitable physical/reference
  environment. SQLite page-limit FULL is simulated capacity, not physical disk.
- Remaining parent: arrangements/themes/assets/font references, destructive
  upgrade backups, history retention/search, autosave/close outcome recovery,
  stable cross-revision occurrence selection. Kernel/SQLite non-lock I/O has no
  hard deadline and memory limits are payload/cache policies, not hard RSS.
- Next: integrator cherry-picks this local ticket commit, reruns combined tests,
  then wires Worker open/poll/SaveSong/Heads to the actual GPUI editor with explicit
  busy/conflict/error/dirty-close states. Poll writes before intentional close;
  reconcile heads after unexpected Drop. Local per-ticket commit only; no push.

### M1-03a — Existing shell keyboard access — 2026-10-04 UTC

- State: **implemented-unqualified** bounded subticket on `ticket/m1-03-focus`,
  based on LOCAL main `c937670`, not origin/main. Assigned UI-first exception;
  no parent completion or qualification gate changed. Owns only `src/main.rs`,
  `src/operator.rs`, `src/tests.rs`, `scripts/operator-shell.py`,
  `docs/operator-shell.md` and this appended session record. No storage changes.
- Retained eight control focus handles, semantic contextual Tab/Shift+Tab and
  Enter/Space actions, click focus, wrapping traversal, distinct focus background
  versus selected-tab underline. Hidden tabs leave the rendered traversal tree;
  collapse/reset focus surviving controls. Root Ctrl+Q preserved from children.
  No disconnected live handlers or invented safety/navigation bindings.
- Reversible Sela accessibility policy, not installed EasyWorship observation.
  EasyWorship 1:1 target/neutral styling/geometry retained; no Painter. Reviewed
  current Zed HEAD (`git ls-remote https://github.com/zed-industries/zed HEAD`)
  matching pin `a84689073d296dfd39987bc7dd478e43ef76d83a`; Apache GPUI example
  `crates/gpui/examples/tab_stop.rs` and window/div/interactive focus APIs listed
  in shell note. No GPL application UI copied. Explicitly rejects synthesized
  keyboard clicks so semantic keys cannot double-activate or bypass bindings.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --all-targets -j 4`: **42 passed**, seven actual operator
  tree/input tests. `cargo clippy --locked --all-targets -j 4 -- -D warnings`,
  `cargo fmt --all -- --check`, `uvx ruff check scripts/operator-shell.py`,
  `git diff --check`: passed. `cargo fmt --all` / `uvx ruff format
  scripts/operator-shell.py` output reread. Initial new exact-child drag-focus
  assertion failed: GPUI defaults refocused the tracked ancestor on mouse down.
  Suppressed only splitter default focus, retaining both prior root and child;
  reran all tests with exact focus plus containment assertions, no weakening.
- Native: own `cargo build --locked --bin sela -j 4` immediately before
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null CARGO_TARGET_DIR=/home/user/workspace/repo/target
  python3 scripts/operator-shell.py`; passed, including native focus/selection
  pixel assertions, size, focus, survival and Ctrl+Q clean exit. Then
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh
  /home/user/workspace/repo/target/debug/sela`: passed input/resize/Ctrl+Q/WM-close.
  Debian12 x64 debug/Xvfb/Openbox/xcompmgr software GL; services left running.
- Inspected actual `.amp/in/artifacts/operator-shell/` captures:
  `focus-scriptures-unselected.png` separates focus from Songs selection;
  `focus-media-selected.png` proves Space-selected Media; `keyboard-collapsed.png`
  has focused Restore and no tabs (Enter did not toggle twice);
  `keyboard-reset.png` restores Resources while retaining Media selection.
  Artifacts are ignored local evidence, not physical/accessibility qualification.
- Not run: installed 8.0.49, Windows/UIA/Narrator/screen-reader, mixed DPI,
  physical GPU/displays or performance; orb software display cannot qualify
  them. Text-input/modal/multi-selection/live-navigation ownership remains future
  work, not invented here. Backlog/plan intentionally untouched by ownership.
- Delivery: local `feat(M1-03a)` subticket commit only, no push/publication.
  Next: integrator merge and rerun combined checks; obtain authorized Windows
  reference/accessibility environment before claiming focus compatibility.

### M1-01a / M1-03a — Parent review and merged verification — 2026-10-04 UTC

- Merged both worktrees with separate local merge commits, retaining subticket
  commits. Only appended work-log conflict; both records preserved. Reviewed
  storage API/schema/migration/worker/test implementation and actual focus diff.
- Strengthened snapshot regression with distinct lyrics under duplicate titles;
  ordered B/A/A content and revisions must survive edit/delete/reopen. Identical
  payload fixtures could not detect accidental reordered song resolution.
- Combined `cargo test --locked --all-targets -j 8`: **50 passed**; strict
  all-target Clippy, fmt and bin build passed. Native shell driver and native-smoke
  rerun from main with `DISPLAY=:99 VK_DRIVER_FILES=/dev/null`: passed focus/
  selection pixel assertions, input, resize, keyboard and WM exits.
- Parent inspected four actual keyboard captures: unselected Scriptures focus,
  Media activation, collapsed Restore focus, reset with retained Media selection.
  Existing artifact directory under main contains reproducible native evidence.
- No direct DB/file work added to UI; storage is not yet connected. Domain
  payload bounds and transaction outcomes reviewed; initial schema only, no
  destructive upgrade or host-disk/power-loss qualification claimed.
- Next M1-05a scope: persistent native field entities using Apache GPUI input
  callbacks, Unicode/grapheme selection, bounded edits, clipboard/IME, multiline
  lyric layout/scroll/caret. Then wire editor save/load through the existing
  bounded storage Worker. No synthetic key-to-string web substitute or Painter.
- Upstream input example inspected through Librarian: single-line only; native
  UTF16 ranges and marked replacement require careful conversion. Existing
  example preedit selection math must not be copied unchanged. GPUI API paths,
  ownership and licensing must be documented with the implementation.
- Local checkpoint only; no push/install/release claim. Windows/reference gates
  remain open; they do not stop approved provisional operator implementation.

### M1-05a — Native text-entry component — 2026-10-04 UTC

- State: **implemented-unqualified**, assigned large bounded component slice on
  `ticket/m1-05-input` in `/home/user/workspace/sela-input`, based on LOCAL main
  `3523641`, not origin/main. Read guidance/plan/backlog/work log; owner's UI-first
  prerequisite approval applies. No Painter, parent M1-05 completion or waived
  physical/reference gates. No main/operator/storage/lib/backlog/plan/README edits.
- Owns `src/text_input.rs`, permanent native `examples/input_check.rs`,
  `docs/text-input.md`, `LICENSE-GPUI-APACHE`, direct pinned unicode-segmentation
  manifest/root lock entry and this appended record. No dependency upgrades;
  1.13.3 was already locked. Original buffer/history/logical-row layout; Apache
  GPUI native handler/custom Element scaffolding attribution/modifications retained.
- Personally read pinned/current Zed a846890 GPUI examples/input.rs/tab_stop.rs,
  src/input.rs/text_system.rs/window.rs; current HEAD verified with
  `git ls-remote https://github.com/zed-industries/zed HEAD`. No GPL editor/ui code
  or assets copied. API, policies and license boundary in text-input note.
- Retained Focusable fields; validated atomic load; explicit errors/counter;
  UTF8/UTF16 clamp/reject policy, fixed composition-relative endpoints,
  marked underline/unmark notification, anchor/head direction, grapheme movement/
  collapsed deletion, copy/cut/paste, contextual Enter without root activation,
  bounded snapshots, real multiline logical rows including trailing empty rows,
  horizontal/vertical clipped scrolling, caret reveal and scroll-aware IME/hit
  geometry. No storage/network. Single-line rejects rather than flattens newlines;
  original Unicode/LF/CRLF preserved, new Enter inserts LF.
- Verification (`CARGO_TARGET_DIR=/home/user/workspace/repo/target`):
  `cargo test --locked --all-targets -j4`: **58 passed**, 8 new module tests,
  zero failed/ignored; `cargo clippy --locked --all-targets -j4 -- -D warnings`,
  `cargo build --locked --example input_check -j4`,
  `cargo fmt --all -- --check`, `git diff --check`: passed, repeated after final
  cut/resize fixes. Formatted code reread. `cmp LICENSE-GPUI-APACHE` against pinned
  upstream license passed. Initial imported GPUI test macro recursion and paint
  borrow errors corrected; no suppressions or skipped checks.
- Tests: nonzero-prefix surrogate+combining IME relative selection and explicit
  precedence; malformed range/selection atomicity and surrogate clamp; byte budget
  on both sides with prefix/suffix; line/history count+byte limits; grapheme deletion
  including scalar-interior caret; CRLF/blank/trailing rows; native callbacks for
  scrolled hit/range geometry and trailing empty caret; reversed selection/vertical
  movement; load rollback; clipboard newline policy and real parent action leak
  negative. Headless callbacks are not an actual IME service session.
- Native: `DISPLAY=:99 VK_DRIVER_FILES=/dev/null` bounded inline Python subprocess/
  xdotool driver against shared target's `debug/examples/input_check`; full run
  passed PID-focus, typing/Tab/Enter, clipboard/errors, undo/redo, horizontal/
  vertical scroll, scrolled edit, mouse drag, WM close exit 0. Final binary repeat
  passed native cut/paste Unicode roundtrip, undo/redo, 420×440 resize/selection,
  close exit 0. Private temp runtime/root captures removed, owned apps reaped;
  existing Xvfb/Openbox services untouched. First driver failed only because it
  wrongly required Alt+F4 survival; corrected exit assertion then reran.
- Inspected actual local ignored `.amp/in/artifacts/input-check/` initial, newline/
  byte-error, redo, horizontal-scroll, scrolled-edit, mouse-selection, cut-empty,
  cut-paste-roundtrip and compact-selection PNGs: readable, correct content/error,
  clipboard preserves 47-byte Unicode/CRLF fixture, empty/trailing caret, clipped
  long text, revealed edited row, selection aligned to visible rows. Summary
  JSON separates asserted focus/exit from visual content evidence. Exact manual
  replay and binary commands are in text-input note; no scratch driver retained.
- Not run: actual platform IME/dead keys/CJK/dictation, Windows/macOS/Wayland,
  screen reader/accessibility/mixed DPI/bidi geometry, physical GPU/reference,
  measured performance/RSS or editor DB/save/close integration. No qualification,
  performance or parity claim. Snapshot composition is not coalesced; no sticky
  vertical column, blinking caret, word/double-click selection or soft wrap claim.
- Next: parent imports module and retains fields, registers contextual bindings,
  observes counter/text/error, validates combined storage envelope and owns save/
  close Worker flow. Rerun combined checks/native fixture; execute actual platform
  IME/accessibility and installed reference checks on authorized native runners.
- Delivery: coherent local `feat(M1-05a)` subticket commit only; no push/publication.

### M1-05b — Persistent native song authoring — 2026-10-04 UTC

- Implemented-unqualified. Original separate authoring window launched from
  Songs or explicit `--library` profile, retained native fields, labeled sections,
  create/edit/duplicate/tombstone delete, validation and bounded ID-paged catalog.
  SQLite and parent-directory creation stay on the bounded storage worker.
  Inline dirty/pending-close guards cover Ctrl+Q and WM close; operator Quit now
  closes only itself, never bypassing another editor's unsaved guard.
- Immutable expected-head saves preserve concurrent-writer conflicts; late
  replies cannot replace newer edits because busy controls are temporarily hidden.
  Incompatible stored metadata/line counts reject before changing any field.
  Documented original provisional UX and missing full-document undo; no live
  connection, installed-reference parity, release, or performance claim.
- Upstream GPUI close/focus/spawn contracts and licensing references are in
  `docs/song-library.md`; no Painter or Zed application components used.
- `cargo test --locked --all-targets -j 8`: **70 passed**, zero failed/ignored
  (includes shared input tests in both app/example). Strict all-target Clippy,
  fmt and app/all-target build passed. Added tests check different sections,
  Unicode, pending/dirty close, duplicate/delete history, stale write retention,
  atomic incompatible load, 128-entry catalog boundary/current revision/tombstone,
  and worker parent creation/failure without replacing existing bytes.
- Native commands, serial on existing Xvfb/Openbox/software GL:
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/song-library.py`,
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/operator-shell.py`,
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh "$PWD/target/debug/sela"`:
  all passed. Song driver independently decodes SQLite bytes after real native
  title/metadata/two-section multiline entry; verifies blank-title rejection,
  dirty WM close, keep-editing, discard-close, same-profile restart and clean exit.
  Extended shell check opens the editor by keyboard, types a draft, closes only
  the operator, asserts the editor survives, and exercises its dirty-close guard
  before final exit. Its profile is disposable; no real default data is touched.
- Inspected actual `.amp/in/artifacts/song-library/` saved/reopened-chorus,
  validation, unsaved-close, keep-editing, compact and compact-scrolled captures.
  Metadata and asymmetric lyrics match; controls fit at 720×440; scrolling exposes
  section/lyrics without overlap. Content crossing scroll viewport edges is
  intentionally clipped. Status validation is inline, not a native popup.
- Initial integration-test macro import/borrow and strict lint errors corrected.
  First extended native driver assumed focus stayed on a removed Keep-editing
  button; screenshot showed Title focus restored. Corrected test to focus Title
  and traverse both close choices; full repeat passed. No failure suppressed.
  The multi-window driver's first fixed launcher coordinate missed after a
  splitter drag; using the real tab order exercised the launcher reliably.
- Still unrun: Windows install/IME/accessibility/high-DPI, physical GPU/output,
  installed EasyWorship editor comparison and measured performance. Catalog is
  not search, editor is not a service scheduler; backups/autosave remain open.
  Next: developer-local install and launch/save/reopen of the installed binary,
  then arrangement/pagination or resource-search prerequisites. Commit is local;
  no push/publication authorized.

### M1-16a — Locally installed Linux developer preview — 2026-10-04 UTC

- Partial prerequisite under the owner's request for installable/useful progress;
  does not close M1-16's Windows scope, M1-12 dependency, M0-10 or Sunday gate.
  Source app remains the M1-05b technical preview. Added reproducible README
  instructions, ignored generated Cargo install root, and a foreground orb
  Desktop launcher which preserves supplied display/Wayland/audio variables.
- `cargo install --path . --locked --offline --debug --bin sela --root "$PWD/.amp/install" -j 8`:
  passed without sudo using cached dependencies. Installed executable reports
  `Sela 0.1.0 (technical preview)`; `ldd` output asserted to contain no `not found`.
  SHA256: `6638e4f6dbde5ef9de37beeddbe855d32e5030e3ba28a554fd6038b79069c2dd`.
  This is a local unoptimized ELF needing setup libraries/fonts, not a portable,
  signed, published or Windows package. Initial uncached setup requires network.
- `SELA_BINARY="$PWD/.amp/install/bin/sela" DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/song-library.py`:
  passed complete native input/independent SQLite/save/reopen/dirty-close/resize
  flow on the installed executable. Inspected its actual reopened-chorus capture:
  all original metadata, Chorus section identity and both distinct lines present,
  readable with no overlap. Same retained native artifact path as M1-05b.
- `DISPLAY=:99 VK_DRIVER_FILES=/dev/null scripts/native-smoke.sh "$PWD/.amp/install/bin/sela"`
  and `DISPLAY=:99 scripts/native-smoke.sh "$PWD/scripts/run-orb-preview.sh"`:
  both passed input/focus/resize, Ctrl+Q and WM close, clean exit/window removal.
  `bash -n scripts/run-orb-preview.sh`, `shellcheck scripts/run-orb-preview.sh`,
  and launcher `--version`: passed. Launcher deliberately uses orb software GL;
  it is not a recommendation for untested physical hardware.
- Bounded inline Python executed two additional disposable-root cycles of the
  same offline Cargo install command, installed `--version`, then
  `cargo uninstall --root <temporary-root>/installed sela`. Binary removed each
  time and a sibling original SQLite retention fixture's SHA256 stayed identical.
  Temporary roots removed; main `.amp/install/bin/sela` remains installed. This
  verifies filesystem retention only, not an actual Sela schema upgrade or S08.
- Unrun: Windows/macOS installer, clean-machine dependency packaging, production
  upgrade/rollback/signing, real platform IME/accessibility and physical output.
  No performance or complete parity claim. Song authoring is usable; schedule,
  arrangement/theme authoring, connected live output and recovery remain open.
- Next concrete implementation: M1-05c bounded whole-document undo for section
  changes, preserving field editing and saved revision/dirty invariants; inspect
  reference semantics when access is available. Then M1-06 arrangement/pagination.
  Local per-subticket commits only; no push, publication or deployment authorized.

### M1-05c — Bounded whole-document song undo/redo — 2026-10-04 UTC

- State: **implemented-unqualified**, assigned worktree `/home/user/workspace/sela-undo`
  on `ticket/m1-05-undo`, based on LOCAL main `2b62c77`, not origin/main. Read
  AGENTS/plan/backlog/work log first; owner authorized this non-layout functionality.
  Owns song_library/text_input, their notes, native song driver and this appended
  record only. No Painter, storage/model/renderer/main/Cargo/backlog or header/index
  changes; no new visible controls or reference-parity claim.
- One chronological whole-Song/section snapshot owner: 64 retained snapshots and
  8 MiB accounted bytes combined across undo/redo. Metadata/lyrics/native preedit
  and Add/Remove share the timeline; navigation/selection/no-ops leave it alone.
  New edits truncate redo. Original Unicode/emoji/combining/LF/CRLF preserved.
  Post-Add validation makes structural payload failures atomic; cap/no-op actions
  no longer reload fields. Current/baseline/allocator/transient memory is outside
  retention accounting; no measured RSS/latency qualification.
- Focused synchronous content callback seam avoids deferred/coalesced observer
  history and stale section association, with no field read during its borrow.
  Only owned fields disable local snapshot recording and bubble semantic Undo/
  Redo; standalone component behavior remains covered. WeakEntity lifetime and
  contextual actions follow inspected Apache GPUI APIs at current/pinned Zed
  `a84689073d296dfd39987bc7dd478e43ef76d83a`; exact paths/provenance in owner note.
- Save/Duplicate retain both stacks and selected section, advancing baseline and
  immutable Version only on successful receipt. Undo after Save is dirty; redo to
  baseline is clean. Save with a redo branch retains that branch. Busy undo/redo
  ignored. New/successful Load/Discard/successful Delete reset history. Failed
  validation/input/structural edits/submission/conflict/load retain history and
  durable state. Dirty/pending close guards unchanged. Carets/composition reset
  on document restoration; no selection restoration or edit coalescing claim.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --all-targets -j2`: **76 passed**, zero failed/ignored;
  `cargo clippy --locked --all-targets -j2 -- -D warnings`,
  `cargo fmt --all -- --check`, `uvx ruff check scripts/song-library.py`,
  Python AST parse and `git diff --check`: passed. Formatted/linter output reread.
  New tests cover cross-navigation asymmetric edits/add/remove/zero sections,
  Unicode, save/duplicate/redo boundaries, New/Load/Discard/Delete reset,
  invalid-load redo retention, dirty close, pending-ignore, conflict/unavailable/
  validation save failure, rejected native edits, 128-section/encoded-size caps,
  count/combined-byte eviction and synchronous content-only callbacks.
- Initial new tests had owned-index move/borrow compile errors (fixed with cloned
  expected values/focus handles). Zero-section direct helper kept focus on the
  removed lyric entity, unlike real Remove-button invocation; corrected the test
  to exercise retained control focus and reran. Clippy collapsed nested if and
  malformed-range fixture construction corrected; Ruff import/executable mode/
  explicit expected search failure check fixed. Final checks passed; no failures
  suppressed or native checks represented as executed.
- Native focus-changing checks intentionally **not run** on shared `:99` while
  parallel work proceeds. Extended `scripts/song-library.py` is lint/parse-checked
  but native replay pending: after parent merge, build merged bin with shared
  target and `-j2`, run serially with `SELA_BINARY="$CARGO_TARGET_DIR/debug/sela"
  DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/song-library.py`.
  Driver asserts save-undo dirty WM close and independent SQLite one-section
  removal/two-section undo restoration. Inspect saved-document-undo, removed-
  section, restored-section and existing compact/reopen PNGs. Owner note contains
  exact replay and manual navigation/branch/final-section checks.
- Remaining: native replay above, installed 8.0.49 semantics, Windows/platform IME/
  accessibility/hardware and measured performance. Parent consolidated checklists
  remain untouched. Next: integrate local commit and execute combined serial native
  replay; then continue arrangement/pagination without claiming parity or waiving
  qualification. Local coherent subticket commit only; no push/publication.

### M1-01b — Verified database backup / restore prerequisite — 2026-10-04 UTC

- **Implemented-unqualified** on `ticket/m1-01-backup`, independent worktree
  `/home/user/workspace/sela-backup`, based on LOCAL main `2b62c77`, not origin.
  Read guidance, plan/backlog/work log. Owned only storage implementation/tests,
  storage contract, existing rusqlite feature and this appended record; no UI,
  domain module/backlog/index/header edits, new crates or dependency upgrades.
- Added `BackupNew` / `RestoreNew` and `Copied` on capacity-one Worker;
  read-only restore input, fresh destination, no current-profile switching,
  online SQLite consistent committed snapshot including WAL. Private same-parent
  staging, supported-schema/quick-check/FK/history verification, explicit close
  and file sync precede atomic no-overwrite hard-link publication. Source and
  existing destination/aliases/SQLite sidecars are never overwritten.
- Budgets: 64 pages/step, 256MiB logical DB, cooperative five seconds,
  100ms lock timeout, 2MiB per connection cache target, 4096-byte paths,
  historical decode one bounded song/schedule at a time. No hard RSS/disk/OS-I/O
  deadline. `Locked` fails rather than indefinite retries. Copy cancellation
  differs from writes: sampled between steps/rows and before publication;
  cancellation that loses publication race reports actual outcome. Drop detaches.
- Exact provenance and scoped checklist in `docs/storage.md`: SQLite backup.html,
  c3ref/backup_finish.html, WAL sections 2–4 and pinned rusqlite 0.40.2 local
  src/backup.rs. Enabled backup feature only; Cargo.lock unchanged.
- Executed with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --lib storage::tests -j2`: **14 passed**;
  `cargo test --locked --all-targets -j2`: **74 passed**, zero failed/ignored;
  `cargo clippy --locked --all-targets -j2 -- -D warnings`,
  `cargo fmt --all -- --check`, `git diff --check`: passed. Initial Clippy
  single-element test-loop lint corrected without suppression; checks repeated.
- Four new executed tests cover Unicode original/edit/tombstone revisions and
  asymmetric B,A,B schedule backup/reopen/worker restore on live WAL; competing
  uncommitted writer/exclusive lock; content-corrupt verification failure;
  corrupt/newer restore input byte retention; source/destination/hardlink/sidecar
  collision and cancellation; bounded worker saturation. Actual child process
  abort after partial online-backup step leaves no output and unchanged source.
  Separate actual child abort after committed song/schedule plus cache-spilled
  uncommitted head/revision/item changes reopens with committed song and ordered
  schedule intact and uncommitted revision absent. Disposable data only.
- No power-loss guarantee: directory metadata is not synced; hard-link outcome
  is authoritative, cleanup best effort. Abort can leave private abandoned staging;
  post-publication/pre-ack process death means caller outcome unknown, inspect
  destination rather than overwrite. Trusted local directory/hard-link-capable
  filesystem required; hostile directory/sidecar races excluded. Windows ACLs,
  filesystem qualification, sync/volume failure injection, continuous-writer
  restart stress and measured large-library budgets remain unexecuted.
- Schema 1 has no actual destructive upgrade: its verified-backup-before-mutation
  migration gate remains open, not simulated or silently marked done. Parent
  M1-01/M1-12/M1-13 integration, rotation, recovery UI and portable assets open.
  Next: parent merge/consolidate checklist; future concrete migration must consume
  verified backup acknowledgment before mutating and execute its own rollback
  checks. Local per-subticket commit only; no push/publication/deployment.

### M1-06a — Bounded arrangement domain — 2026-10-04 UTC

- State: **implemented-unqualified** assigned parallel non-UI prerequisite on
  `ticket/m1-06-arrangement`, `/home/user/workspace/sela-arrangement`, from LOCAL
  main `2b62c77`, not origin/main. Read guidance/plan/backlog/work-log. Only
  `src/arrangement.rs`, library export, `docs/arrangement.md` and this append;
  no persisted Song/Section/schema, editor, Cargo, UI or renderer changes.
- Contract: explicit typed section/variant/occurrence IDs reuse opaque 128-bit
  storage Id vocabulary. Occurrence scope is variant-local; selection uses the
  full version/variant/occurrence tuple. Owned immutable revision snapshot shared
  by Arc; repeated occurrences borrow identical section content, never head data.
  Pure named-variant/edit/reorder/remove/retarget operations are all-or-error;
  final-position move and empty draft policy explicit. Limits/reference/name/ID
  validation and future non-destructive ID migration obligations in arrangement
  note. No legacy label/index ID conversion or provenance verification claim.
- Verification with `CARGO_TARGET_DIR=/home/user/workspace/repo/target`:
  `cargo test --locked --lib -j2`: **33 passed**, including five new arrangement
  tests; `cargo clippy --locked --lib --tests -j2 -- -D warnings`,
  `cargo fmt --all -- --check`, `git diff --check`: passed. Formatter output
  reread. Initial shared-target reuse returned the baseline's 28 tests (zero
  arrangement tests); detected via filtered listing, touched this worktree's
  source timestamps and reran, observing actual compilation and all five tests.
  Only the recompiled 33-test result is arrangement evidence.
- U cases: scrambled source order, duplicate labels/distinct IDs, asymmetric
  V1/C/V2/C/C, pointer-shared choruses/distinct occurrences, both move directions,
  retarget/remove/insert and independent variants; missing/deleted references and
  internal final-reference corruption give no partial result; rejected edits
  retain original; empty policy, lossless Unicode/CRLF, exact source/arrangement
  byte boundaries, name bytes, section/variant/occurrence count boundaries.
- Not run/claimed: native arrangement E2E, Windows/reference/physical rendering,
  layout/pagination/font fitting, performance/RSS or persistence migration.
  There is no UI or persisted adapter to exercise; no GPUI pattern added.
  Parent M1-06 remains open. Source revision/content provenance is the future
  repository adapter's obligation; domain payload budgets are not hard RSS caps.
- Next: parent merge export/appended record, rerun combined checks with observed
  test names (shared target can reuse another worktree artifact); settle storage
  ID/arrangement migration and editor selection/undo before wiring controls.
  Local coherent M1-06a subticket commit only; no push/publication/deployment.

### M0-06b — Bounded native static-color cue pipe diagnostic — 2026-10-04 UTC

- Owner-authorized parallel worktree `/home/user/workspace/sela-native-cues`,
  branch `ticket/m0-06-native-cues`, based on **local main `2b62c77`**, not remote
  main. Read plan/backlog/work log and preparation/delivery/output/composition
  notes. Scope is backend diagnostic only; no Painter, main/operator layout,
  storage, manifest/lockfile or reference safety-policy changes. Scoped checklist,
  schema, API/license provenance and remaining work appended in `docs/delivery.md`.
- Implemented-unqualified static-color fallback, not full text/image delivery:
  fixed version-1 73-byte-max owned frames, no Rust memory layouts or file refs,
  worker-only stdin/stdout I/O, inbound 2/outbound 4 inline frame capacities;
  reconstruct immutable validated opaque-color cues, renderer-owned Ready after
  native surface/device setup, epoch/sequence/lane enforcement and preparation
  rejection without replacing prior scene. Native commit encodes a GPU pass,
  calls queue.submit and native present **before Applied**; no GPU wait/readback
  or pipe/file/decode/raster/logging in the frame callback. No Applied is emitted
  by non-GPU process test fixtures. No main UI controls are wired.
- Renderer-local receipt-read timestamp preserves inbound queue expiration;
  budgets are 1–5000ms, not synchronized process clocks or hard pipe-transit
  expiry. Controller timeout/disconnect remains Unknown and disables delivery;
  old child must be retired/reaped before a fresh epoch, with no automatic replay.
  Example normally closes at 24s; independent process-exit watchdog caps even
  stuck startup at 25s. Driver bounds startup/receipts/capture/retirement, escalates
  only owned children, records receipts and checks native cropped pixels.
- Checks with shared `CARGO_TARGET_DIR=/home/user/workspace/repo/target`, `-j2`:
  `cargo test --locked --all-targets -j2` **79 passed, 1 ignored child fixture**;
  three integration tests invoke that fixture as actual Rust child processes.
  Covers exchange/reject, independent renderer lane saturation/FIFO, sequence and
  epoch rejection, real killed/stalled child disconnect, exact injected deadline,
  fresh unknown/no replay. Six transport unit tests include all truncated frame
  lengths, malformed/oversize header/body fields, full owned image/font rejection
  instead of silent stripping, inbound queue expiry, prior-snapshot retention,
  receipt round trips and 1000 nonblocking writer-overload attempts. Existing
  Delivery/Preparer failure tests also passed unchanged.
- `cargo clippy --locked --all-targets -j2 -- -D warnings`,
  `cargo fmt --all -- --check`,
  `cargo build --locked --example native_cues -j2`, Python AST parse and
  `git diff --check`: passed. Initial alpha-malformation assertion accidentally
  rewrote 255 with 255; corrected the test to alpha 0 and reran. An intermediate
  shared-target build reused another worktree's Sela rlib despite correct local
  sources; touching local `src/lib.rs` forced a root-crate rebuild (no
  content or dependency change). Final all-target compile/tests/clippy passed
  together; parent should serialize merged Cargo/native qualification runs.
- Provisioned only own supervised `sela-cue-x11` Xvfb **:101**, 1024×768 (no WM
  or compositor), never touched reserved :99/focus. Executed twice:
  `DISPLAY=:101 VK_DRIVER_FILES=/dev/null CARGO_TARGET_DIR=/home/user/workspace/repo/target python3 scripts/native-cues.py`
  with `--out .amp/in/artifacts/native-cues` then
  `--out .amp/in/artifacts/native-cues-final`: passed. GL adapter startup logs
  identify Mesa22.3.6 llvmpipe LLVM15.0.6 CPU renderer. Final summary ties eight
  client-only PNGs to receipt sequences/timestamps and independently checks three
  interior RGB samples each. Red/green apply, invalid-alpha/extent failure retain
  red, stale sequence retains green, currently applied duplicate doesn't replace,
  retired epoch rejects and restarted output remains unconfirmed until a fresh
  explicit cue. Inspected actual red/retention/green/unconfirmed captures. This
  is software virtual native evidence, **not physical scanout/GPU completion**.
- Negative driver checks (missing DISPLAY/binary, invalid backend) and missing
  audience CLI args returned nonzero. Separate inline subprocess check left stdin
  open without commands: Ready arrived, standalone audience exited cleanly after
  **24.060s** without supervisor EOF. No stuck-driver watchdog fault injection,
  physical/Windows/macOS/reference checks, performance/RSS/latency budgets or
  operator focus regression executed; parent owns serialized post-merge native
  checks. Own display service stopped after checks; no push/publication.
- Next: versioned owned text/font/image schema with total/per-field budgets,
  renderer preparation/upload worker and GPU-ready ownership separated from
  offscreen readback, then receipt-correlated asymmetric resource failure tests.
  Preparation-intent coordinator, durable epochs, production supervision and
  device-loss handling remain open. Static-color slice is implemented-unqualified;
  M0-06/M0-07/M0-10 are not done and no Black/Clear/Logo semantics were invented.

### M1-02c — Reference-directed toolbar correction — 2026-10-04 UTC

- Implemented-unqualified on `ticket/m1-ui-reference`, LOCAL main `231fffb`
  (all four non-UI merges), not origin/main. Read instructions/plan/backlog/log.
  Owner explicitly requested actual UI correction; no Painter, dependency,
  storage, arrangement or renderer changes. Scoped checklist/provenance in
  `docs/operator-shell.md`, documented-only evidence D-UI-01 in reference ledger.
- Personally fetched/inspected official toolbar image: exact left/right action
  order, right group above Live, no bottom live controls or invented output
  handlers. Its build is unknown, not verified 8.0.49. Original clean GPUI chrome,
  preserved pane/splitter geometry; Reset/Quit moved to Resources chrome.
- Real New menu and bottom Songs + open blank authoring; companion M1-05d follows.
  New/menu join semantic traversal, menu overlays without reflow. Fixed removed
  menu-item focus after native replay caught failed operator close. No assertion
  dropped. Disabled output controls remain inert, no color-diagnostic wiring.
- Combined slice checks: `CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo
  test --locked --all-targets -j4`: **96 passed, 1 ignored child fixture** (three
  process tests invoke it); new test names actually observed. Root sources touched
  before initial build to defeat shared-worktree stale cache. Strict all-target
  Clippy (`-D warnings`), fmt, Ruff both drivers, diff check and bin build passed.
- Serial `DISPLAY=:99 VK_DRIVER_FILES=/dev/null CARGO_TARGET_DIR=/home/user/workspace/repo/target
  python3 scripts/operator-shell.py`, song-library driver and native-smoke against
  shared target binary passed. X11 services untouched. Actual normal/New menu/
  compact/focus/collapse/reset/drag/editor-guard PNGs inspected under
  `.amp/in/artifacts/operator-shell/`; screenshots are ignored local evidence.
- Windows/installed8.0.49/accessibility/DPI/physical GPU/display/performance unrun:
  no suitable reference/hardware environment; no parity or release qualification.
  Local subticket commit only, no push. Next: retain companion editor correction,
  parent reviews/reruns integration and consolidates backlog; exact reference
  interaction and audience qualification gates remain open.

### M1-05d — Reference-directed song editor correction — 2026-10-04 UTC

- Implemented-unqualified companion on `ticket/m1-ui-reference`, LOCAL main
  `231fffb`, retaining M1-05c chronological document history and all four non-UI
  merges. Owns song_library/source tests, native song driver, owner note, ledger
  and appended record only. No storage/arrangement/transport/scene/Cargo/native
  diagnostic/backlog changes; root-source touches only forced correct cache build.
- Personally read official Support7 articles (2024-09-03 and 2023-01-05) and
  inspected working historical editor screenshot. D-UI-02 explicitly documents
  those sources, not an installed8.0.49 finding. Apache GPUI upstream commit,
  inspected paths and license recorded; original UI/fixture text, no Painter.
- Title/Words/Slides/section selector left, draft preview or Inspector metadata
  right, Add/Remove bottom-left and OK/Cancel bottom-right. Explicit Library
  selector retains bounded worker catalog/loading/paging outside Words; existing
  New/Save/Duplicate/Delete/Discard/Previous/Next reachable. Compact Inspector
  clipping caught by visual review: omit redundant header status below900px,
  keep truthful status below toolbar; fixed footer and scrolling preserve access.
- Slides are four-line original text thumbnails; preview is logical draft text,
  not audience WYSIWYG/pagination/themes/arrangement or matching output. No fake
  unsupported formatting/media/output features. Native field entities retained.
- OK waits for committed Saved receipt, not submission; validation/conflict/error
  keep draft open; Cancel/WM/Ctrl+Q retain existing pending/dirty guards. Ordinary
  Save advances only durable baseline/Version and retains selection/history.
  New test exercises mode/navigation history invariants, invalid/conflicting OK
  and pending Cancel. Old history/failure tests remain, revealing Inspector
  before metadata-field focus instead of typing into a removed focus subtree.
- Final `CARGO_TARGET_DIR=/home/user/workspace/repo/target cargo test --locked
  --all-targets -j4`: **96 passed, 1 ignored child fixture**, new actual test names
  and all five arrangement tests observed. Strict all-target Clippy `-D warnings`,
  fmt, Ruff both drivers, diff check, locked bin build passed. Serial native
  song-library/operator-shell drivers and native-smoke passed with DISPLAY=:99,
  VK_DRIVER_FILES=/dev/null and the same shared target. Shared services untouched.
- Native independent SQLite decoding verifies metadata/two asymmetric sections,
  save-undo dirty close, retained committed bytes on discard, structural undo
  across saves, reopen and actual OK-save/close; invalid OK also asserts survival
  and zero commits. This executes the previously pending M1-05c native replay.
  Coordinates were updated, not payload/undo/failure assertions removed.
- Inspected actual ignored `.amp/in/artifacts/song-library/` initial/draft/Slides,
  Inspector-filled, validation/validation-ok, unsaved-close, restored-section,
  reopened-chorus, compact/compact-inspector/compact-library-selector/scrolled
  PNGs, plus operator's independent editor-close guard. Original native crops,
  not reference/composited images. Commands and scoped checklist in owner note.
- Remaining: lawful installed8.0.49 behavior, Windows/real IME/accessibility/DPI,
  hardware/audience output and performance unavailable/unrun, no parity/release
  claim. Next: parent reviews these two local commits, reruns merged integration,
  consolidates backlog and schedules required native/reference qualifications;
  unsupported authoring/arrangement/pagination remains open. No push/publication.

### M1-05d — Integration regression: whitespace-title navigation — 2026-10-04 UTC

- Parent review found section-row selection rejected a whitespace-only unsaved
  title, unlike Add/Previous/Next. Treat whitespace as blank only in the temporary
  size-validation copy; retain exact draft bytes and require a real title on save.
- Added an asymmetric two-section GPUI regression proving selection actually
  changes, correct field content loads, document/history remain unchanged, and
  OK still refuses persistence/close. After correcting a test-only borrow lifetime,
  `cargo test --locked --bin sela blank_title_section_navigation -j4` failed at
  section 1 versus expected 0 before the fix. The same test passes after the fix.
- `cargo test --locked --all-targets -j4`: **97 passed, 1 ignored subprocess
  fixture**, invoked by three process tests. Strict all-target Clippy `-D warnings`
  and `cargo fmt --all -- --check` passed. Updated song guide entry paths and OK,
  Cancel and backend-only backup limits. No visual geometry change in this fix.
- `cargo install --path . --locked --offline --debug --bin sela --root
  "$PWD/.amp/install" -j4` succeeded. Installed `--version` printed
  `Sela 0.1.0 (technical preview)`; `DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  scripts/native-smoke.sh "$PWD/.amp/install/bin/sela"` passed both Ctrl+Q and
  WM-close scenarios. Full merged native/visual evidence is recorded below.
- Local subticket fix, not a push or Windows qualification. Next: complete
  parent integration checkpoint and retain the remaining parent-ticket gates.

### M0-06b — Native driver executable metadata — 2026-10-04 UTC

- Parent's combined Ruff check found EXE001: the shebang-bearing native cue
  driver lacked executable permission. Set its tracked mode to 0755; no Python
  content changed. `scripts/native-cues.py --help` and `uvx ruff check
  scripts/operator-shell.py scripts/song-library.py scripts/native-cues.py` now
  pass; `git diff --check` passes. The full native cue replay had already passed
  via `python3`; the mode change only makes direct invocation consistent.
- Local M0-06b follow-up; parent integration evidence follows. No push.

### Parallel wave — Parent integration and installed preview — 2026-10-04 UTC

- Four independently owned non-UI worktrees plus the reference-directed UI
  worktree are locally merged with per-subticket commits intact. Parent inspected
  storage publication/validation, arrangement identity/bounds, transport/native
  submit/receipt ordering, document callbacks/history and toolbar/editor changes.
  Resolved append-only work-log conflicts without dropping worker records;
  retained both library module exports. Main checkout owns combined verification.
- Shared target reuse had been detected in worker runs. Touched merged root
  sources before rebuild and confirmed real compilation plus new test names;
  did not treat stale zero-test/filter output as verification. Final
  `cargo test --locked --all-targets -j4`: **97 passed, 1 ignored child fixture**
  (three process tests explicitly invoke it). Strict all-target Clippy, fmt,
  all-target build and `git diff --check` passed. Final Ruff across all three
  native Python drivers passed after the recorded executable-mode correction.
- Serial Linux X11/software GL commands with `DISPLAY=:99 VK_DRIVER_FILES=/dev/null`:
  `python3 scripts/operator-shell.py`, `python3 scripts/song-library.py`,
  `scripts/native-smoke.sh "$PWD/target/debug/sela"`, and
  `python3 scripts/native-cues.py --backend gl --out .amp/in/artifacts/native-cues`
  all passed. Existing shared Xvfb/Openbox/xcompmgr services were not restarted.
  Native cue summary binds receipt sequences to sampled cropped pixel colors;
  failed/stale/retired commands retain state and fresh startup does not replay.
- Reinstalled via the exact offline debug Cargo install command above. Installed
  smoke passed; `DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  SELA_BINARY="$PWD/.amp/install/bin/sela" python3 scripts/song-library.py` passed:
  actual validation, independent SQLite payload checks, two-section editing,
  save/document/structural undo, dirty-close, reopen, resize and OK commit-close.
- Parent inspected actual `.amp/in/artifacts/operator-shell/{normal,compact}.png`,
  `song-library/{draft,slides,compact-inspector,compact-scrolled}.png` and native
  cue `render-failure-retains-red.png`. Toolbar grouping/placement, Words/Slides,
  draft preview, Inspector and fixed footer are legible without overlap. Compact
  form crops at its scroll boundary; scrolling reveals lyrics while footer stays
  reachable. These are native software-rendered captures, not physical evidence.
- Updated backlog evidence and README entry paths. The arrangement domain is
  not persisted; backup is a worker API, not a recovery UI; native transport is
  opaque-color-only, not text/image output or Black/Clear/Logo implementation.
  Official marketing/Support screenshots are documented evidence, not installed
  8.0.49 verification. No indexed search, service scheduler or live lyric UI yet.
- Unrun: Windows/package installer, installed reference, physical GPU/projector,
  mixed DPI, IME/accessibility qualification, destructive migration, power-loss
  recovery, measured large-workload performance and long hardware soak. No access
  or implementation prerequisite for these checks was invented. No parent ticket
  or milestone gate was closed by this wave.
- Delivery: all work is committed locally; nothing pushed, published or deployed.
  Preview launcher remains `scripts/run-orb-preview.sh`. Resume the explicit-ID
  persistence/migration prerequisite and renderer resource transport separately;
  obtain Windows/reference access when the owner is available.

### Continuation scope — 2026-10-04 UTC

- Owner asks to continue feasible implementation and postpone final Windows/
  hardware proof. Resume two independent prerequisites from local main: storage
  explicit-ID/arrangement migration with verified backup, and owned font/text/
  image transport with native readiness separated from frame submission.
- Storage owns repository/domain/editor data round trips and their tests;
  renderer owns scene/transport/native examples and their tests. No Painter or
  live-control semantics change. Reference unknowns remain explicit.
- Each worktree records/commits its own bounded slices. Parent merges locally,
  reviews combined contracts and runs integration/native checks. Shared Cargo
  target must be serialized and root-source compilation observed to avoid the
  previously detected stale-worktree artifact reuse. No push authorized.

### M1-06b — Backed-up stable-ID/arrangement persistence — 2026-10-04 UTC

- **Implemented-unqualified** in `/home/user/workspace/sela-persistence`, branch
  `ticket/m1-06-persistence`, based on LOCAL main **f161dc3**, not origin/main.
  Read AGENTS/plan/backlog/work-log/arrangement/storage before implementation.
  Owned storage/song-library, their three notes, native song driver and this
  append only; central backlog/current-state consolidation remains parent-owned.
  No Painter, visual geometry, renderer/scene/transport/delivery/main/library
  export/example/Cargo/dependency or real-profile writes. No push/publication.
- Schema 2 adds revision-keyed section-ID mappings and ordered named variants/
  occurrences with FK/uniqueness checks. Original schema-1 payload codec remains
  exact; migration does not rewrite payloads, old schedules, heads or tombstones.
  Legacy IDs assigned once with SQLite randomblob per immutable revision, never
  inferred from labels/indices/text or claimed continuous across old revisions.
  New source edits preserve section IDs and variants. Historical arrangements
  and schedule snapshots remain exact after edits/tombstones. Late missing
  references reject all, never silently drop a chorus or return a partial variant.
- Opening legacy profile holds IMMEDIATE writer reservation across a separate
  read-only online backup and migration. `<profile>.schema1-backup` must be
  verified/synced/published fresh before any upgrade DDL/mapping. Verification
  uses non-upgrading open; backup/RestoreNew retain input schema 1 or 2. Migration
  DDL/mappings/user_version are one transaction. Backup conflict/corruption/lock
  blocks mutation. Failed/aborted migration retains verified backup; deterministic
  backup name intentionally blocks blind retry with Exists. Preserve both files
  and restore to a fresh profile for recovery; no overwrite/rotation/recovery UI.
- Existing Song is the editor/repository source of truth, now Section.id plus
  Song.variants. SaveSong/Song/Schedule worker round-trip all data; added
  Repository/worker Arrangement(v) returns truthful immutable source pairing.
  Pure Song adapter still requires caller provenance. New/Add CPU-only SHA-256
  clock/PID/counter allocator does not read files/random devices; no cryptographic
  uniqueness claim. Undo/load/save never regenerate IDs. Duplicate retains
  song-local document IDs under a fresh song ID. Initial baseline shares one
  blank allocation; history bytes include variants. Referenced Remove fails
  atomically with status, no new controls/focus/action mappings.
- Preserve maximum 256KiB unarranged legacy codec payload; arrangement creation
  still obeys existing stricter text-plus-ID domain budget and may reject such a
  document without losing text. Existing capacity-one worker and bounded reads
  remain; added schedule arrangement completion overhead documented. No hard
  RSS/migration deadline, measured performance, or physical/fsync qualification.
- Executed final checks with actual root compilation/new test names observed:
  `flock /tmp/sela-cargo-continuation.lock bash -c 'set -e; touch src/*.rs; export CARGO_TARGET_DIR=/home/user/workspace/repo/target; cargo test --locked --all-targets -j4; cargo clippy --locked --all-targets -j4 -- -D warnings; cargo fmt --all -- --check'`:
  **104 passed, zero failed, one ignored subprocess fixture explicitly
  invoked by three process tests**. Six new storage and one GPUI test: legacy WAL/
  per-revision ID reload/old 2,1,2 schedules; backup gate conflict/corrupt/real
  competing writer/DDL rollback; actual child SIGABRT after inserted mapping
  rolls back schema/rows and retains backup; exact legacy payload max; schema2
  V1/C/V2/C/C/reorder/source provenance/late reference and insert failures/
  tombstone/backup/restore/worker round trips; deliberate late corrupt reference
  and mapping position never truncate/publish; duplicate-label editor load/save/
  duplicate/undo/redo/new Add identity/referenced and unreferenced Remove.
  Existing storage crash/disk-full/worker/history and arrangement bounds pass.
  `uvx ruff check scripts/song-library.py`, Python AST parse and `git diff --check`
  passed. Formatter output and edited notes reread. Shared lock released outside
  Cargo; no other worktree's root artifact accepted as evidence.
- Initial compile used unsupported rusqlite usize FromSql (changed to checked
  contiguous i64 positions). Initial test assumed byte-identical whole SQLite
  backup header (corrected to schema/payload equivalence and immutable backup
  bytes after verification), DDL error classification corrected to Corrupt.
  Random New IDs exposed independent blank baseline allocation/old blank equality:
  share initial allocation and assert fresh blank content/ID instead. Maximum
  legacy test needed two sections/minimal metadata to exercise stricter domain
  bound. Deliberately corrupt external fixture now explicitly disables FK before
  injecting a broken late reference. No failed expectation suppressed.
- Native song driver retains original independent payload decoder and now checks
  schema2 distinct section IDs and exact retention across removal/undo/save.
  Lint/parse checked only: parent owns :99 focus replay, not run or disturbed here.
  After merge, parent must rebuild merged root under shared lock/touch and execute
  `DISPLAY=:99 VK_DRIVER_FILES=/dev/null SELA_BINARY=/home/user/workspace/repo/target/debug/sela CARGO_TARGET_DIR=/home/user/workspace/repo/target python3 scripts/song-library.py`
  plus combined native operator checks. Status error text is the only visible
  wording change; no visual controls/arrangement UX introduced.
- Windows/hardlink/ACL/installed8.0.49/IME/accessibility/physical display/GPU,
  real volume/power-loss faults, large-library measurements and recovery chooser
  remain unrun/open. Next concrete action: parent local merge/review/native replay
  and central checklist consolidation, then arrangement controls/explicit repair
  and pagination. Local M1-06b commit carries requested Amp-Thread-ID trailer;
  no parent ticket marked done or qualification waived.

### M0-06c — Owned resource wire checkpoint — 2026-10-04 UTC

- Resumed renderer continuation on `ticket/m0-06-resources` from parent LOCAL
  main f161dc3, not origin/main. Read engineering/plan/backlog/work-log,
  delivery/composition/preparation notes. No main/UI/storage/dependency edits.
- Added version-2 bounded font/UTF-8/normalized-RGBA transport and immutable
  worker-only owned reconstruction; v1 color/receipt/Ready remains compatible.
  Exact schema, per-field/aggregate and channel memory ceiling in delivery.md.
  Timestamp precedes v2 body transfer and survives queued reconstruction.
- Serialized shared-target Cargo under `/tmp/sela-cargo-continuation.lock`,
  touching root sources: `cargo test --locked --lib transport -j4` **6 passed**,
  including actual image/font byte round trips, truncated/oversized/trailing
  resource frames and existing 1000-attempt overload/deadline tests. Compilation
  of this worktree was observed. Cargo fmt executed; native helper slice remains
  unstaged for the next subticket. All-target attempt exposed native helper-only
  compile/dead-code issues, being corrected before M0-07d verification.
- State: implemented-unqualified transport checkpoint, not native resource proof.
  Next: bounded native worker raster/upload, submit-only bindings, asymmetric
  text/image retained-state replay; Windows/physical/reference proof stays open.

### M0-07d — Worker-prepared native text/image submission — 2026-10-04 UTC

- Added bounded native worker (one executing/one waiting/one completion), owned
  reconstruction/font shaping/raster/upload outside event callbacks and shared
  `ReadyComposition` bindings. Existing offscreen `render` still performs diagnostic
  readback; native `submit_native` never invokes it. Upload staging flush and <=2s
  GPU completion wait occur only on worker; upload failure retires, no retry loop.
- GPU-ready admission retains RendererSession FIFO/epoch/duplicate/deadline
  enforcement; Applied only follows Queue::submit/native notify/present. Added
  ordered Busy rejection for saturated preparation. Resource deadline includes
  body transfer, queue, preparation/upload and result wait. Valid resource-size
  framing allowance corrected to +160 bytes (image+text metadata requires 133).
- Exact final verification, under shared-target flock with touched sources and
  actual compilation: `cargo test --locked --all-targets -j4` **105 passed,
  1 ignored child fixture** (four process tests execute it); strict
  `cargo clippy --locked --all-targets -j4 -- -D warnings`,
  `cargo fmt --all -- --check`, `cargo build --locked --example native_cues -j4`,
  `uvx ruff check scripts/native-cues.py`, `git diff --check` all passed.
  Eight transport tests include source/text/image/aggregate byte boundaries,
  malformed/truncated/trailing framing, queued v2 expiry, exact owned font/image
  bytes and 1000 overload rejections retaining revision23. New real-child test
  checks actual UTF-8/font/asymmetric RGBA across pipes; no fake Applied there.
- `DISPLAY=:102 VK_DRIVER_FILES=/dev/null CARGO_TARGET_DIR=/home/user/workspace/repo/target
  python3 scripts/native-cues.py --backend gl --out
  .amp/in/artifacts/native-resources-final` passed twice during final development;
  last pass includes fatal-upload retirement code. Dedicated supervised
  `sela-resources-xvfb`, 1024×768, Mesa22.3.6/llvmpipe LLVM15 CPU GL, was stopped
  afterward; no shared :99 service/input/focus touched. Fifteen native client
  PNGs, receipt timestamps, pixel samples and full RGB hashes plus adapter logs
  persist in that ignored artifact directory. Actual text/image and expired
  retention captures inspected: readable Signal café/Beacon, six asymmetric tiles,
  no clipped glyph/row corruption. Driver checks six colors, real white coverage,
  identical retained RGB after missing glyph, overflow, invalid font, oversized
  text, stale sequence and 1ms expiry; retired resource epoch/startup no replay.
- State: implemented-unqualified diagnostic resource slice, not main operator
  wiring, installed-reference parity or physical output. Explicit channel/CPU/GPU
  ceilings and stricter native text limits/provenance in owner notes. Resize keeps
  owned resources but requires new-extent preparation; visible resized retention
  unqualified. Upload/device-loss/watchdog fault injection, measured memory/frame
  pacing, Windows/DX12/Metal, physical GPU/display, mixed DPI and reference remain
  unrun. No dependency/GPUI/Painter/storage/editor changes or push.
- Parent next: merge local resource subtickets, preserve append-only log records,
  consolidate central backlog/current state, touch merged root sources under
  flock and rerun integration plus serial native/operator regressions. Continue
  production intent/pre-resolved safety coordinator and resize preparation before
  live-control wiring; do not close parent M0 gates from software-native captures.

### M0-07d — Parent native capture correction — 2026-10-04 UTC

- Merged native replay on shared :99 with Openbox exposed a driver geometry bug:
  xdotool's reported origin shifted the crop downward by about 20px. The top-left
  expected red sample at (100,160) read the lower ochre tile; screenshot inspection
  also showed a bottom decoration strip. Worker :102 had no window manager, so
  its passing captures did not expose this. No renderer assertion was weakened.
- Use xwininfo absolute client origin/width/height, matching existing GPUI drivers.
  Ruff caught an initial capture-label shadow before execution; renamed the local
  field. `uvx ruff check scripts/native-cues.py` and `DISPLAY=:99
  VK_DRIVER_FILES=/dev/null python3 scripts/native-cues.py --backend gl --out
  .amp/in/artifacts/native-resources-integrated` now pass all 15 captures, pixel
  samples and retained full-image RGB hashes. `git diff --check` passes.
- Inspected corrected text-asymmetric-image and expired-retains-text-image PNGs:
  complete six tiles, equal-height rows, readable Signal café/Beacon, no decoration
  strip or shifted crop. Receipt sequence/pixel checks are executed evidence;
  images alone do not prove timing or physical scanout. No application behavior
  change. Local M0-07d follow-up, no push.

### M1-06b / M0-06c / M0-07d — Merged verification — 2026-10-04 UTC

- Merged both independent worktrees into LOCAL main, preserving all three
  implementation commits. Only conflict was append-only work-log records; both
  retained. Parent reviewed migration backup/transaction boundaries, historical
  payload/ID/arrangement resolution, editor history, bounded wire parsing,
  preparation queue ownership, GPU-ready bindings and native receipt ordering.
- Held `/tmp/sela-cargo-continuation.lock`, touched merged src/examples/test roots
  and observed actual compilation. `cargo test --locked --all-targets -j4`:
  **112 passed, zero failed, 1 ignored subprocess fixture**, explicitly invoked by
  four process tests. `cargo clippy --locked --all-targets -j4 -- -D warnings`,
  `cargo fmt --all -- --check`, `cargo build --locked --all-targets -j4`, Ruff on
  all three native Python drivers and `git diff --check` passed.
- Serial `DISPLAY=:99 VK_DRIVER_FILES=/dev/null` runs of
  `python3 scripts/song-library.py`, `python3 scripts/operator-shell.py` and
  `scripts/native-smoke.sh "$PWD/target/debug/sela"` passed. Song replay verifies
  schema2 distinct IDs and retention across structural removal/undo/save against
  independent SQLite reads, plus existing payload/close/reopen assertions.
- Native resources driver initially failed under Openbox; fixed and verified
  actual client coordinates as recorded immediately above. Final run: PASS,
  15 captures, actual text coverage/asymmetric pixels, identical retained RGB
  hashes after invalid font, missing glyph, overflow, oversized text, stale and
  expired cues, fresh restart without replay. Evidence in
  `.amp/in/artifacts/native-resources-integrated/summary.json` and its PNGs;
  text-asymmetric-image and expired-retains-text-image personally inspected.
- Shared GPU helper regression: private temporary XDG runtime, explicit Vulkan
  llvmpipe, `target/debug/examples/composition_spike <temporary-output> vulkan`
  passed pixel/color/contain/cover/overflow checks; `target/debug/examples/video_spike
  <temporary-output>` passed 12 exact ordered RGBA frames, actual-text GPU pixels,
  missing/corrupt, saturated deadline/cancel and retained-last-frame checks.
  Disposable outputs removed. These runs are checks, not performance qualification.
- Reinstalled with `cargo install --path . --locked --offline --debug --bin sela
  --root "$PWD/.amp/install" -j4`; installed `--version` prints
  `Sela 0.1.0 (technical preview)`. Installed native-smoke passed Ctrl+Q and WM
  close. No original profile opened/migrated during testing; fixtures are disposable.
- README/backlog/current-state and arrangement replay checklist updated. UI layout
  unchanged; no arrangement controls, pagination or Go Live wiring claimed.
  Windows/installed reference, real GPU/projector, mixed DPI, accessibility/IME,
  power-loss/volume faults, production budgets and physical soaks remain open,
  with final hardware verification postponed at the owner's request.
- Delivery: local commits and merges only, no push/publication/deployment.
  Resume arrangement UI/repair or renderer resize/safety coordination as above.

### M0-06 — Oracle disconnected receipt fix — 2026-10-05 UTC

- A matched Rejected(Disconnected) now invalidates the session, clears both
  pending lanes and exposes Unknown; prior confirmation remains diagnostic history
  only. Late acknowledgments cannot revive it and new sends require a fresh session.
- Regression first failed (Confirmed revision 3 instead of Unknown), then passed
  for both Cue/Safety orderings, two dispatched pending commands, late Applied
  and new sends. `cargo test --locked --lib delivery::tests -j4`: 9 passed;
  `cargo clippy --locked --lib -j4 -- -D warnings` and fmt passed.
- No UI/protocol shape change. Remaining Oracle fixes are pending native editor
  input and renderer backpressure; combined checks follow those changes.

### M1-05 — Oracle pending-input ownership fix — 2026-10-05 UTC

- Synchronously lock TextInput mutations when storage work is accepted (including
  initial open), before the next redraw can detach the native handler. Native
  replace/preedit and local undo/redo cannot change buffer/history while locked.
  Programmatic acknowledged loads remain possible. Unlock on completion/error,
  unless a committed OK is awaiting window removal; owner actions/history are
  also blocked during that final interval. OK independently compares the current
  document against the Saved snapshot before closing.
- Inspected pinned Zed a84689073d296dfd39987bc7dd478e43ef76d83a:
  crates/gpui/src/window.rs handle_input (5245–5266) and
  crates/gpui_linux/src/linux/x11/window.rs handle_ime_commit/preedit (1224–1247).
  Input-handler lifetime extends until redraw, so hidden controls are not a lock.
  Existing Apache GPUI APIs only, no new dependency or copied application code.
- GPUI regression first failed with late commit/preedit bytes in the field after
  OK submission. Now exercises OK, Load, Delete and conflicting Save, injecting
  both callbacks in the same App update before any redraw/poll, checking unchanged
  field/edit-count/history, actual durable reply, and unlock/close behavior.
  Separate test bypasses native lock with a programmatic load and proves OK cannot
  close over a mismatched snapshot. No layout/appearance changes.
- `cargo test --locked --all-targets -j4`: 117 passed, 1 ignored subprocess fixture
  (includes in-progress renderer regressions). Strict all-target Clippy,
  fmt and all-target build passed. Serial DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  song-library.py, operator-shell.py and native-smoke.sh target/debug/sela passed,
  including independent SQLite/undo/reopen/OK/dirty-close/focus checks. Native IME
  interleaving is simulated via its actual input-handler API; Windows/real IME
  device qualification remains open. Local commit only; renderer fix follows.

### M0-07d — Oracle preparation ordering fix and final checks — 2026-10-05 UTC

- Saturated preparation retains one bounded Frame and stops pipe intake until
  enqueued, rather than rejecting a later stamp before earlier preparation has
  completed. Preserves receiver timestamp/deadline; worker disconnect is fatal.
  Documented extra CPU ownership ceiling of 64MiB+168 bytes. Still FIFO diagnostic
  preparation, not production safety-priority or measured memory qualification.
- Deterministic queue-full regression withholds the worker receive, tests both
  Cue/Safety orders with asymmetric revisions 7/29, repeated blocked polls (no
  further pipe intake), and ordered Accepted/Applied. Separate test checks expired
  retained deadlines and disconnected preparation worker.
- `cargo test --locked --all-targets -j4`: 117 passed, zero failed, one ignored
  subprocess fixture explicitly invoked by four process tests.
  `cargo clippy --locked --all-targets -j4 -- -D warnings`,
  `cargo fmt --all -- --check`, `cargo build --locked --all-targets -j4` and
  `uvx ruff check scripts/native-cues.py scripts/song-library.py
  scripts/operator-shell.py` passed. Combined native editor/operator/smoke checks
  recorded above also passed with all three fixes present.
- `DISPLAY=:99 VK_DRIVER_FILES=/dev/null python3 scripts/native-cues.py --backend
  gl --out .amp/in/artifacts/oracle-fixes-native`: PASS, 16 captures. New real-process
  case sends Cue 2 then Safety 3 without waiting for acknowledgments; both receive
  Accepted then Applied, terminal order is 2 then 3, final blue RGB is (31,53,179).
  Existing failure/expiry retention and fresh-session/no-replay assertions pass.
  Evidence: `.amp/in/artifacts/oracle-fixes-native/summary.json`; inspected
  `consecutive-lanes-blue.png`, uniform blue without old content/decorations.
- Reinstalled using `cargo install --path . --locked --offline --debug --bin sela
  --root "$PWD/.amp/install" -j4`; `.amp/install/bin/sela --version` prints
  `Sela 0.1.0 (technical preview)`. `DISPLAY=:99 VK_DRIVER_FILES=/dev/null
  scripts/native-smoke.sh "$PWD/.amp/install/bin/sela"`: PASS for focus, resize,
  Ctrl+Q and WM close with clean exit/window removal. Temporary profile only.
- All three requested Oracle fixes are implemented and locally verified. Backlog
  evidence updated without closing parent qualification gates. Windows/real IME,
  installed reference, GPU/projector, production safety/performance and installer
  checks remain open. Next implementation slices remain arrangement controls/
  missing-reference repair or renderer resize/safety coordination. Local commits
  only; no push, publication or deployment.

### M0-01 — Windows build and native launch — 2026-10-06 (Asia/Jakarta, UTC+7)

- State: **done**. Closes the last M0-01 checklist item. Base `b553d9a` local main.
- Environment: Windows 11 Home Single Language, i7-14650HX, Intel UHD Graphics
  (driver 32.0.101.6790) + NVIDIA RTX 4060 Laptop (32.0.15.9159); displays
  DISPLAY5 primary 2048x1152 logical, DISPLAY1 1463x914, DISPLAY6 1920x1080.
  Installed via winget: rustup 1.29.1 (toolchain 1.98.1 from rust-toolchain.toml),
  VS 2022 Build Tools 17.14 VCTools + Windows 11 SDK 10.0.26100 + CMake, Python
  3.12.10 (user scope) and Ruff (pip --user). Exact commands in gpui-bootstrap.md.
- Windows-only defect found and fixed (M1-01 storage): `copy_new` flushed the
  staged backup through `File::open(..).sync_all()`. Windows `FlushFileBuffers`
  needs a write handle, so every verified backup and schema-1 migration returned
  `Error::Io` (six storage tests failed). Now opens with write access before
  `sync_all`. Also silenced the Windows-only `unused_mut` warning on the
  backup-directory builder. Windows staging directories inherit the parent ACL
  (no 0o700 equivalent); recorded as an open Windows hardening item, not fixed.
- Verification in plain PowerShell 7 from this checkout (fresh Windows target dir):
  `cargo build --locked --all-targets -j 16` passed (4m57s cold);
  `cargo fmt --all -- --check` and
  `cargo clippy --locked --all-targets -j 16 -- -D warnings` passed;
  `cargo test --locked --all-targets -j 16` **112 passed, 1 ignored subprocess
  fixture**, 0 failed (lib 52, bin 28, transport_process 4, composition 5,
  input_check 9, native_cues 7, output_spike 1, video_spike 6). Linux runs 117:
  five video decoder child-process tests are `cfg(unix)`. Before the fix: 46/52
  lib tests passed, six storage backup/migration tests failed with `Io`.
  `target\debug\sela.exe --version` printed `Sela 0.1.0 (technical preview)`;
  SHA-256 `221a734621f3c6c7ddf2cecd4f67bbb592062bb3e60021ddfea75e03b6b22c45`.
- Native N: new `scripts/native-smoke-windows.py` (+ `scripts/native_win.py`)
  `python scripts/native-smoke-windows.py target\debug\sela.exe .amp\in\artifacts\windows`
  PASS: PID-scoped window, verified foreground before every SendInput, survived
  Ctrl+J, DPI 120 (125%) client settled at 900x550 = 720x440 logical, smaller
  request refused (minimum held), 180 sampled colors, Ctrl+Q exit 0 after 2.1s,
  relaunch Alt+F4 exit 0, both windows removed. Ruff check passed after removing
  one unused noqa. Window was on the primary 2048x1152 display only.
- UI evidence: inspected `.amp/in/artifacts/windows/m0-01-windows-small.png`
  (900x550): readable Segoe UI toolbar, Schedule/Preview/Live, Resources tabs and
  "+ New Song", no clipping at the minimum size. Log shows GPUI DirectWrite with
  Segoe UI, RTX 4060 Direct3D 11.1 device; `0x887A002D` DXGI debug interface
  missing (optional Graphics Tools, debug-only) and a benign shutdown
  `window not found`. `.amp/in/` is now gitignored (it was only locally excluded).
- Not run: Windows CI (not pushed), Narrator/UIA semantics, IME, cross-monitor
  DPI moves, Intel-GPU operator rendering, song-library/operator-shell native
  drivers (X11-only, not yet ported), EasyWorship reference (not installed).
- Next: M0-04 Windows run of `examples/output_spike.rs` on a second physical
  display with DX12; then M0-07 composition/video/native cue checks on DX12.
- Delivery: local `fix(M1-01)` and `feat(M0-01)` commits; no push.

### M0-04 / M0-07 — Windows DX12 physical-display and GPU readback — 2026-10-06 (UTC+7)

- State: both **implemented-unqualified**; no checklist box closed.
  Evidence detail and tables: output-spike.md (Windows section),
  composition-spike.md and video-spike.md. Same host as the M0-01 record.
- Owner was playing a game throughout. Timing is under uncontrolled GPU load
  and possible concurrent input; treat it as feasibility, not qualification.
- Changes: `examples/output_spike.rs` opt-in `SELA_SPIKE_MONITOR`,
  `SELA_SPIKE_FULLSCREEN`, `SELA_SPIKE_SECONDS` (default 25s, clamp 1..600) and
  wgpu `WGPU_POWER_PREF`, plus monitor/scale/surface telemetry; defaults keep
  the Linux path unchanged. `composition/gpu.rs` and `native_cues.rs` also honor
  `WGPU_POWER_PREF`; `video_spike` gained the optional backend argument.
  New `scripts/output-spike-windows.py`; `native_win.py` skips winit's visible
  0x0 helper window and falls back to AttachThreadInput for foreground.
  `scripts/native-cues.py` is now cross-platform: reader/writer threads replace
  select/nonblocking pipes, Win32 BitBlt capture on Windows, and a single
  raw-RGB sampler for both platforms. Its Linux path was not re-executed.
- Driver failures before the pass: (1) targeted winit's 0x0 helper window, so
  moves/captures/Alt+F4 hit the wrong HWND (65-byte PNGs, no scale events);
  fixed by skipping zero-area windows and matching the audience title within
  the PID. (2) Twice the operator never became foreground; added the
  AttachThreadInput fallback and a 1s settle before fullscreen-phase input.
- `python scripts/output-spike-windows.py` PASS (main, fullscreen, adapter
  matrix). Main: 949 presents, p50/p95/p99/max 17.03/17.61/18.36/48.25ms; 2s stall
  118 presents with a 17.97ms maximum gap; preparation gap 18.05ms; 224/60
  presents after exit/kill. Moves 175%→125%→175% emitted scale 1.75/1.25/1.75,
  client 1120x630/800x450/1120x630, maximum gaps 17.73/18.10ms. Fullscreen
  2560x1600 on the laptop panel, 2s stall gap 17.89ms. Matrix: low=Intel UHD,
  high=RTX 4060 on all three monitors, p50≈17.03ms, max ≤18.11ms (run with the
  earlier `SELA_SPIKE_POWER` name; rerun pending with `WGPU_POWER_PREF`).
- Inspected stall-a/stall-b (triangle moved during the stall), fullscreen-stall,
  after-kill under `.amp/in/artifacts/output-spike-windows/`.
- M0-07: `composition_spike <out> dx12` PASS with WGPU_POWER_PREF low (Intel)
  and high (NVIDIA); `video_spike <out> dx12` PASS on both, plus `vulkan` on NVIDIA,
  with FFmpeg 7.1.1 gyan.dev full build. Inspected `image-contain.png`.
- Findings: wgpu default DX12 present mode was Mailbox (latency 2) with
  timer pacing. That cannot match 165/180Hz scanout, so production must choose
  the present mode and pacing explicitly. Present-call timing is not scanout.
- Checks after changes: ruff on all seven Python scripts, `cargo fmt --check`,
  strict all-target clippy, `cargo test --locked --all-targets` 112 passed /
  1 ignored. All passed.
- Not run: `native-cues.py --backend dx12` and the matrix rerun (both open
  windows that can take focus from the owner's game), display hotplug (needs the
  owner to unplug a monitor), device loss/TDR, sleep/resume, DXGI frame
  statistics, idle-machine soak, Linux re-execution of the refactored cue driver.
- Next: when the machine is idle run
  `python scripts/native-cues.py --backend dx12 --out .amp\in\artifacts\native-cues-windows`
  and `python scripts/output-spike-windows.py`; then a hotplug run with
  `SELA_SPIKE_SECONDS=120` while the owner unplugs/replugs monitor 2.
- Delivery: local `feat(M0-04)` and `feat(M0-07)` commits; no push.

### M0-04 — Idle-machine Windows rerun — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**; no checklist box closed.
- Owner stopped gaming. Same host and debug build as above.
- `python scripts/output-spike-windows.py .amp\in\artifacts\output-spike-windows-idle`
  PASS (main, fullscreen, `WGPU_POWER_PREF` matrix). Main 944 calls, p50/p95/
  p99/max 17.04/17.73/18.24/23.39ms; 2s stall max gap 18.20ms; preparation
  18.64ms; 218 calls after exit, 60 after kill; scale 1.75→1.25→1.75.
  Fullscreen 2560x1600 p50/p99/max 17.02/18.33/18.84ms. Matrix low=Intel UHD,
  high=RTX 4060 on all three monitors. Present mode again Mailbox latency 2.
  Detail in output-spike.md.
- Not run: hotplug (owner must unplug a monitor), device loss/TDR, sleep/resume,
  DXGI frame statistics, multi-hour soak.
- Next: hotplug with `SELA_SPIKE_SECONDS=120` while the owner unplugs and
  replugs monitor 2.

### M0-06 / M0-07d — Windows DX12 native cue driver — 2026-10-06 (UTC+7)

- State: both **implemented-unqualified**; no checklist box closed.
- `cargo build --locked --example native_cues` then
  `python scripts/native-cues.py --backend dx12 --out .amp\in\artifacts\native-cues-windows`
  PASS on the RTX 4060 (DX12, driver 32.0.15.9159). First Windows execution of
  the cross-platform driver. All 16 receipt-correlated captures matched their
  expected sample colors, including failure retention of the text/image scene,
  old-epoch rejection and the unconfirmed restarted session. Detail in
  delivery.md.
- Not run: Intel adapter for this driver, Linux re-execution of the refactored
  driver, physical scanout timing.

### M0-02 — First installed observation, partial W01 — 2026-10-06 (UTC+7)

- State: **active** (was blocked). Owner installed the free, unlicensed
  EasyWorship 8 (registry 8.0.49, executable FileVersion 8.0.49.0).
- New `scripts/reference_win.py`: lists one image's windows and captures a
  single HWND with PrintWindow, plus focused click/type helpers. Captures never
  include other applications.
- Recorded RUN-W01-2026-10-06 and EW8-OBS-010–013 in reference-observations.md:
  main-window layout (menu row, toolbar groups, Schedule/Preview/Live with
  Preview Output/Live Output strips and "Slide N of M", Songs library), the
  unlicensed output windows on the laptop panel with a "NOT LICENSED FOR LIVE
  PROJECTION" logo, and the song editor structure and dirty-cancel prompt.
- Failed: synthetic clicks inside the song editor did not move focus, so no
  fixture songs exist and W02 (Go Live, Preview/Live, next/previous) did not run.
  Draft discarded with No; nothing saved in the EasyWorship profile.
- Checks: `python -m ruff check scripts/reference_win.py` passed.
- Not run: About dialog capture, W02–W06, masks, keyboard/focus, routing.
- Next: owner-assisted fixture creation (or an import file), then W02.

### M1-10a — Application audience renderer mode — 2026-10-06 (UTC+7)

- State: **active** parent; no checklist box closed. Owner request to integrate
  the UI with the engine is treated as approval to start M1-10 before M1-03,
  M1-09 and M0-08 finish (2026-10-04 sequencing exception).
- Moved `examples/composition/{gpu,text}.rs` to `src/audience/{compositor,text}.rs`
  and the native cue renderer into `src/audience.rs`; `examples/native_cues.rs`
  is now a thin wrapper, and the composition/video examples include the moved
  files. cosmic-text, winit, wgpu and pollster became normal dependencies.
- `text::raster_aligned` adds centered lines (horizontal and vertical).
- Transport frame kind 4 SURFACE (epoch, width, height): the audience reports
  its settled drawable extent after Ready and after each resize. Reports wait
  for 250ms without resize, because Windows emits transient sizes on monitor
  arrival and a cue built for one would fail.
- `sela --audience EPOCH_HEX <INDEX|secondary|window> [backend]` opens a
  borderless window covering the chosen monitor. It is created without
  activation and is not winit fullscreen: winit's `set_fullscreen` (and
  fullscreen-at-creation) force the window active on Windows and took keyboard
  focus from the operator. The window is sized again after arrival because a
  monitor with another scale factor rescaled it (3584x2240 on a 2560x1600 panel).
  With `retain_on_disconnect` the applied scene stays on screen after the
  controller pipes close, until the window is closed.
- New `scripts/audience-smoke-windows.py`: Ready then one settled surface
  report, client equals surface, window rect equals monitor rect, foreground
  unchanged, centered text cue Accepted then Applied, no foreign pixels over the
  output (composed-screen capture of the audience client only), retained after
  stdin close, WM_CLOSE exit 0.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` and `cargo build --locked --all-targets` pass.
  `python -m ruff check scripts/audience-smoke-windows.py` and `ruff format` pass.
  `python scripts/native-cues.py --backend dx12 --out .amp\in\artifacts\native-cues-windows-m1-10a`
  PASS. `python scripts/audience-smoke-windows.py --monitor secondary` (laptop
  panel 2560x1600, 168 DPI) and `--monitor 0` (primary 2560x1440, 120 DPI) PASS,
  0 foreign pixels; evidence in `.amp\in\artifacts\audience-smoke-windows-*`.
- Not run: Linux re-execution of the moved renderer, Intel adapter, hotplug of
  the audience monitor, physical scanout timing, taskbar behavior with other
  taskbar settings.
- Next: M1-10b operator supervisor that spawns `sela --audience` with a fresh
  epoch, Go Live from preview slides, Live pane showing confirmed/unknown/
  disconnected state, next/previous.

### M1-10b — Operator live output and acknowledged Live — 2026-10-06 (UTC+7)

- State: **active** parent, implemented-unqualified slice; no checklist box
  closed (safety buttons, preparing state off the UI thread, observed 8.0.49
  behavior, masked Go Live and latency remain open).
- New `src/output.rs` `Supervisor`: spawns one audience child per session off the UI
  thread with a fresh SHA-256 epoch; Starting → Connected{extent, caps} or
  Lost (spawn failure, 15s startup timeout, exit, protocol or delivery error).
  One cue in flight (3s acknowledgment) plus one wanted cue that coalesces to the
  latest; rejected cues keep the prior confirmed state; children are killed and
  reaped off-thread on drop. `Delivery::is_connected` added.
- New `src/slides.rs`: one slide per section, versions `revision << 16 | index`,
  white centered DejaVu Sans on black. Font size fitted from glyph advances
  (5% margin) within the audience text limits (96px, 32 lines, 4096 bytes,
  4096px area); missing glyphs fail before delivery.
- `src/operator.rs`: storage-worker catalog in Songs (refreshed on activation),
  Preview slide tiles (click, Enter/Space, double-click = Go Live per SRC-02,
  documented-only), Live ○/● toggle, Go Live, Previous/Next stopping at the ends,
  Live pane with output line, "On screen", "Sending…" and rejection lines, red
  confirmed / amber sending borders; resend on settled surface change. No replay
  into a new session. `sela --operator-library PATH`; `SELA_AUDIENCE_MONITOR`,
  `SELA_AUDIENCE_BACKEND`.
- Tests: `tests/output_process.rs` (fake audience child: apply, reject, resize,
  wrong epoch, exit, hang, silent), supervisor and slide unit tests, operator
  tests `go_live_requires_preview_and_output`,
  `go_live_shows_renderer_acknowledged_slide`,
  `rejected_or_lost_output_is_never_shown_as_live`, updated keyboard traversal,
  and `audience::tests::operator_slide_cues_fit_the_audience_text_preparer`.
- Native: new `scripts/live-output-windows.py`, `examples/seed_library.rs`,
  `native_win.press`. The first run failed correctly: the real renderer rejected
  a 256px cue (limit 96px); Live showed "Last cue not shown" and "nothing
  confirmed", audience stayed black. Fixed the fit, added the cross-check test.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (144 passed, 3 ignored fixtures),
  `cargo build --locked` and `cargo build --locked --example seed_library` pass.
  `python -m ruff check scripts` passes; `ruff format` applied to the new script
  only (five older scripts are not ruff-formatted; unchanged).
  `python scripts/live-output-windows.py` PASS three times (`--out` default,
  `-r1`, `-r2`) on the laptop panel 2560x1600 @168 DPI, DX12, RTX 4060 Laptop:
  operator kept the foreground, Go Live/Next/Previous observed 171–188 ms after
  the key at capture granularity, Next stopped at the end, Live off and Ctrl+Q
  ended the child. `slides::cue` on the UI thread: p50 0.10 ms, p95 0.11 ms, max
  0.79 ms (200 runs, 2560x1600, dev profile, throwaway example not committed).
- Not run: primary-monitor output with the operator on the same monitor, Linux/
  macOS native, Intel adapter, audience hotplug, installed 8.0.49 W02/W03,
  physical scanout latency, screen reader.
- Next: M0-08 mask semantics then Black/Clear/Logo buttons; M1-09 schedule items
  as Live sources; W02 observation (needs owner-assisted fixtures) for slide
  shortcuts, selection and double-click; move cue preparation to a worker if
  themes or images make it measurable.

### M0-02 — W02/W03 single-song observation — 2026-10-06 (UTC+7)

- State: **active**. Scope: the static W02/W03 cases reachable with one song,
  as planned in the approved 2026-10-06 spec (Phase 1).
- Sources: added SRC-05–SRC-11 (help.easyworship.com Toolbar, Live Area,
  Preview Area, Helpful Shortcut Keys, Editing Images, Editing Videos; the
  Adding And Editing Songs article). SRC-04 was already Screen Setup, so the
  spec's "SRC-04 to SRC-10" became SRC-05–SRC-11.
- New `scripts/reference-w03-windows.py` (probe/press/seq/run). Input only to
  the EasyWorship main window after a foreground check; audience evidence is
  the Live Output window's screen rectangle captured until stable; toggle
  indicators read from a PrintWindow toolbar crop; JSONL row and contact
  sheet per case.
- Fixtures: owner's song "waeqwe" (4 slides, placeholder text). Generated
  original logo and green-stripe background images; imported via Media →
  Images → +, logo set with "Use As Logo Background", background applied with
  Copy to Theme ▸ Song Theme and "Set As Default Song Theme".
- First run RUN-W03-20261006-132625 was invalid for masks: the Media tab stayed
  active, so the image item was previewed and live. Kept as evidence that Clear
  does nothing on a live image item. Rerun RUN-W03-20261006-133553 (165 rows)
  on the song is the source of EW8-OBS-014–019; EW8-OBS-020 lists what did
  not run.
- Findings: Black and Logo replace each other, Clear stacks with either; Go
  Live, Preview double-click and Live ‹ › keep masks; Live double-click clears
  a single mask and applies that slide; Live off keeps mask state; Ctrl+B/L/C
  and Page Down match the buttons with Preview focus; Live slide single-click
  applies immediately; › stops at the last slide.
- Checks: `python -m ruff check scripts/reference-w03-windows.py` and
  `python -m ruff format --check` pass. No Rust changes.
- Not run: combined-mask Live double-click, masked Live single-click, profile
  reopen, motion/audio, alerts, multiple outputs, multi-item schedule cases,
  W04 focus contexts, transition timing. The EasyWorship profile now holds the
  two images, a "sela-w03-background" default song theme and the logo setting.
- Next: Phase 2, M0-08a `src/masks.rs` (cover None/Black/Logo plus Clear flag)
  and M1-10c operator toggles with a picture logo.

### M0-08a — Mask layer and logo slot — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Scope: Phase 2a–2c of the approved
  2026-10-06 spec; operator UI, logo source and native checks are M1-10c.
- Added `src/masks.rs` (cover None/Black/Logo plus independent Clear, from
  EW8-OBS-015–017; combined-mask Live double-click is provisional per
  EW8-OBS-020). `Payload{Scene,Logo,Mask}` in `delivery`; masks on the Safety
  lane, logo on the Cue lane. Wire kinds 5 MASK (v1, 30 bytes) and 6 LOGO
  (v2 owned resource). Supervisor mask/logo slots with coalescing and pump
  order mask, logo, cue. Audience: masks bypass the preparation worker,
  background-only bind group for Clear, Black clear pass, retained logo
  bindings, wrong-extent slides keep the last frame.
- Decision: `RendererSession` orders sequences per lane. Otherwise a mask sent
  while a slide is still in preparation would make that slide Stale. Updated
  `reordered_commands_and_duplicates_never_reapply_old_content` to assert
  same-lane reordering and added `lanes_are_ordered_independently`.
- Checks: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (159 passed, 3 ignored fixtures) and
  `cargo build --locked` pass on Windows 11.
- Not run: native capture of Black/Clear/Logo frames, operator indicator
  agreement, Linux/macOS, video logo (deferred to M2-03).
- Next: M1-10c operator toggles (acknowledged indicators, Live pane mask lines,
  Live double-click unmask, Ctrl+B/L/C and PageDown in a show-control key
  context), Media Images list and "Use as Logo Background", native checks.
### M1-10c — Operator masks and picture logo — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Scope: Phase 2d–2f of the approved
  2026-10-06 spec (operator toggles, picture logo, native checks).
- Added `src/images.rs`: profile `Resources/Images/` beside the library,
  PNG/JPEG copy-import with a free " (N)" name, 8 MiB / 4096-file caps,
  SHA-256 `ResourceRef`, `logo.txt` for the chosen logo, and a bounded worker
  (Scan/Import/UseAsLogo/LoadLogo) so no file work runs on the UI thread.
- Operator: Logo/Black/Clear toolbar buttons with Off/Pending/On indicators from
  supervisor acknowledgments; `ToggleBlack/ToggleLogo/ToggleClear/GoLive`
  actions bound to Ctrl+B/L/C and Page Down in `SelaShow && !SelaTextInput`;
  Live pane mask lines; Live single-click applies, double-click unmasks after
  its cue leaves the slot; mask intent survives Live off/on (EW8-OBS-017).
  The logo is prepared off-thread per surface extent and resent on new session
  and resize; Logo intent goes out as Black until the logo is with the renderer.
  Media tab lists images with Import image… and Use As Logo Background (button
  and right-click menu).
- Decisions: text-field exclusion for show keys is Sela policy (W04 open);
  combined-mask Live double-click and single-click under a mask are provisional
  (EW8-OBS-020); logo uses Contain, diverging from EW8-OBS-019 "fills the
  output" until a non-matching aspect ratio is observed; video logo is M2-03.
- Checks (Windows 11, RTX 4060 Laptop): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 73, bin 47 + 1 ignored fixture,
  integration suites all pass), `cargo build --locked`,
  `python -m ruff check scripts` pass. `ruff format --check` passes for
  `live-output-windows.py`; `native_win.py` and `native-cues.py` were already
  unformatted at HEAD and were left unformatted.
- Native: `cargo build --locked --example seed_library` then
  `python scripts/live-output-windows.py --out .amp\in\artifacts\live-output-windows-masks{,-r1,-r2}`
  PASS three times (2560x1600 secondary, DX12): Black/Clear/Logo frames and
  lit buttons agree, each second press restores the identical slide frame,
  Logo returns after Live off/on, 171–188 ms per change (capture polling).
  `cargo build --locked --example native_cues` then
  `python scripts/native-cues.py --backend dx12 --out .amp\in\artifacts\native-cues-windows-masks`
  PASS: Clear keeps the image background without text, Black, Logo without a
  logo is RenderFailed and stays black, duplicate mask Applied without redraw,
  unmask byte-identical to the retained text/image frame. Captures inspected.
- Not run: native file picker (logo seeded into the profile; GPUI test covers
  the prompt), Linux/macOS, UIA names, installed observation of EW8-OBS-020,
  physical scanout timing.
- Next: Phase 3 M1-06c — raise the text size limit, resize-to-fit/normalize,
  Ctrl+Enter split with undo, arrangement order on output.

### M1-06c — Better slide output — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Scope: Phase 3 of the approved
  2026-10-06 spec.
- `slides::slides` follows the first arrangement (falls back to section order
  when there is none, it is empty, or a section does not resolve).
  Text limit 96 → 288px in `audience::text` and `slides.rs`.
  `slides::Sizing{PerSlide,Normalized}` and `size_cap`; `cue` takes a cap.
- Operator: "Normalize text size across slides: Off/On" in the Songs footer
  (control 21, after Clear in tab order); cap cached per song revision, extent
  and setting; toggling resends the live slide; not persisted.
  `scripts/live-output-windows.py` tab counts updated (19 tabs to the first song,
  9 back to Live output).
- Song editor: Ctrl+Enter (`SplitSection`, `SongLibrary` context) splits the
  section at the lyrics cursor; new section ID, copied label; arrangements get
  the new section after each occurrence of the split one; one undo step.
  `TextInput::selection()` added.
- Decisions: always resize to fit (the fixed-size mode needs a theme font
  size); normalize off by default; copied label and mid-line split are
  provisional; SRC-12 added for EasyWorship's auto-size settings.
- Checks (Windows 11): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 75, bin 49 + 2 ignored: fake
  audience fixture and timing; all other suites pass), `cargo build --locked`,
  `python -m ruff check scripts`, `ruff format --check scripts/live-output-windows.py`.
- Measured: `cargo test --locked --release --bin sela large_text_preparation_time -- --ignored --nocapture`:
  2560x1600 p50 0.6–2.7 ms (max 3.3), 3840x2160 at 288px p50 0.7–3.5 ms
  (max 4.2); dev profile p50 3.7–29.9 ms (max 32.7).
- Native: `python scripts/live-output-windows.py --out .amp\in\artifacts\live-output-windows-m1-06c`
  PASS (2560x1600 DX12); captures inspected: the verse fills the width at the
  new size; the operator footer shows the Normalize toggle.
- Not run: native Ctrl+Enter and Normalize (no Windows song-editor driver),
  EasyWorship 8.0.49 observation of auto-size and split, Linux/macOS.
- Next: Phase 4 M1-09a — Schedule pane items pinned to song revisions, add/
  remove/reorder, Save/Open with Ctrl+S/Ctrl+O, live item identity.

### M1-09a — Basic schedule — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Scope: Phase 4 of the approved
  2026-10-06 spec (early start with the same approval as M1-10; M1-04 and
  M1-08 have not started).
- Added `src/schedule.rs` (framework-free): entries with session-local
  `EntryId`s pinning song `Version`s, insert/push (full at 32 = storage
  `MAX_ITEMS`), remove, move_to/move_by, neighbor (stops at the ends), snapshot,
  `mark_saved` (edits during a save stay dirty), open (checks title count),
  clear; dirty compares entry versions with the last saved/opened baseline.
  `Repository::schedule_catalog` and `Command/Reply::ScheduleCatalog` list
  current schedule titles.
- Operator: Schedule pane (header, Unsaved, rows, "● Live" only after renderer
  acknowledgment, drag/drop from Songs and between rows, right-click Remove
  From Schedule, footer Up/Down/Remove, message line); Add to Schedule in the
  Songs footer; Open/Save toolbar buttons; New ▾ → New Schedule. Dialogs for
  save-as title, open list, remove confirmation and unsaved guard. Storage
  requests go through one queued worker job. Live identity is the entry, so
  reorder/removal never sends a cue; Live notes "Live item is no longer in the
  schedule". Library double-click goes to Live from the first slide. Window
  close (`on_window_should_close`) and Ctrl+Q use `may_close`. Actions
  SaveSchedule/OpenSchedule/Next/PreviousScheduleItem/RemoveScheduleItem bound
  to Ctrl+S/Ctrl+O/Down/Up/Ctrl+Delete in `SelaShow && !SelaTextInput`.
  Overlays (dialogs, item, New and Media image menus) now `occlude()` so clicks
  no longer fall through to rows behind them (found by a GPUI test).
- Decisions (provisional, unobserved in 8.0.49): Up/Down move buttons,
  Ctrl+Delete removes without confirmation, Down/Up stop at the ends, a song
  dropped on a row inserts before it, library double-click starts at slide 1,
  item IDs are not persisted. Library edits never update scheduled revisions.
- Tests: `schedule::tests`, `storage` `schedule_catalog_lists_current_titles`,
  GPUI `schedule_add_reorder_select_navigate_and_remove`,
  `drag_songs_into_the_schedule_reorder_and_cancel`,
  `live_item_identity_survives_reorder_duplicates_and_removal`,
  `save_and_reopen_pin_revisions_behind_an_unsaved_guard`,
  `quit_and_new_schedule_are_guarded`, `library_double_click_goes_straight_to_live`;
  two traversal tests updated for the new controls.
- Checks (Windows 11, RTX 4060 Laptop): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 82, bin 55 + 2 ignored, integration
  and example suites all pass), `cargo build --locked`,
  `python -m ruff check scripts`, `ruff format --check` on
  `schedule-windows.py` and `live-output-windows.py` pass.
- Native: `cargo build --locked --example seed_library` (now with
  `--second-song`), then
  `python scripts/live-output-windows.py --out .amp\in\artifacts\live-output-windows-m1-09a`
  PASS (tab counts 25 to the first song, 15 back to Live output) and
  `python scripts/schedule-windows.py --out .amp\in\artifacts\schedule-windows-m1-09a-r2`
  PASS (secondary 2560x1600, DX12): keyboard add/duplicate/select/remove/
  reorder, Ctrl+S save read back from SQLite, two items sent live (62/63 ms,
  capture polling), live item removal keeps the frame, WM_CLOSE and Ctrl+Q
  guarded, reopen with Ctrl+O. The first run failed on a missing `down` key in
  `native_win.py` (added Up/Down/Delete). Captures inspected; a doubled border
  around the title field was removed.
- Not run: native mouse drag/drop (GPUI tests only), EasyWorship 8.0.49
  observation of schedule shortcuts/menus/drop insertion, Linux/macOS, UIA.
- Next: the approved spec is complete. Choose the next dependency-ready ticket:
  remaining M1-09 items (duplicate, multi-move, autoscroll, explicit refresh),
  schedule-level Live navigation (M1-10), or W04 observation.

### M1-05e — EasyWorship Song Editor Words layout — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Owner request: make Song New/Edit match
  EasyWorship 8. Reverse engineering the EW binary was declined (EULA,
  copyright, original-code rule); the installed 8.0.49 editor was observed as a
  black box instead (RUN-W06E, EW8-OBS-021–026 in reference-observations.md,
  captures in `.amp/in/artifacts/reference/ew8-w06-editor/`, nothing saved in
  EW). Owner-approved plan: four phases, each its own commit: M1-05e Words and
  layout, M1-05f Slides tab, M1-05g formatting (follow the official help,
  provisional), M1-05h background; EW layout/behavior in Sela's light style.
- `src/song_library.rs` rewritten around per-slide label/lyrics cells
  (`TextInput` flow mode, index-parallel to `draft.sections`), keeping the
  whole-document history, storage worker, guards and action indices (5/6 are
  now toolbar Undo/Redo, 7/8 `+`/`−`, 1 is footer Apply). Ctrl+Enter no longer
  copies the label; Backspace at the start of an unlabeled slide joins it to the
  previous one and drops its occurrences (provisional). History steps move the
  caret to the restored slide; reloading a cell keeps its caret. New songs start
  unlabeled (EW) instead of "Verse 1". Window title "Song Editor - <title>".
- `src/text_input.rs`: flow mode (borderless, height follows rows, wheel goes
  to the list, Up/Down at the edges, single-line Enter and Backspace at 0
  propagate), placeholders, `set_cursor`; the unfocused 3 px caret-selection
  quad is no longer painted for empty selections.
- Preview: `audience::text_coverage` (extracted from `prepare_frame`, same
  inset/raster) plus `slides::section_slide`; the editor rasterizes 1280x720
  BGRA on the background executor, one job in flight, latest wins, and drops
  replaced images with `App::drop_image`. Upstream checked at the pinned
  `a846890`: `crates/gpui/src/assets.rs` (`RenderImage`),
  `elements/img.rs` (`ImageSource::Render`), `app.rs`/`window.rs`
  (`drop_image`), `elements/div.rs` (`ScrollHandle::scroll_to_item`).
- Tests: new GPUI `words_cells_navigate_and_new_song_focuses_first_label`,
  `ctrl_enter_splits_without_label_and_backspace_joins`,
  `preview_follows_the_caret_slide_off_thread`; unit
  `preview_matches_audience_raster_and_rejects_unshowable_text`,
  `label_kinds_and_groups_follow_observed_palette`; the existing editor tests
  were moved from the old label/lyrics fields to cells without weakening their
  failure assertions (one expectation changed: undo restores the slide where
  the undone edit happened).
- Checks (Windows 11): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 82, bin 59 + 2 ignored, integration
  and example suites pass), `cargo build --locked`, `python -m ruff check scripts`.
- Native: `python scripts/song-editor-windows.py` PASS (new; Unicode SendInput
  and a click helper added to `native_win.py`): "Song Editor - Untitled", caret
  in slide 1's label, Enter to lyrics, line breaks, Ctrl+Enter, Up to slide 2's
  label, preview ink (black 38882 / white 910 samples), split and join, title
  click + typing, Ctrl+S payload decoded from SQLite, window title follows,
  Ctrl+Q exits 0. Captures in `.amp/in/artifacts/song-editor-windows/`
  inspected; stray caret quads in unfocused cells were found and fixed.
- Not run: X11 `scripts/song-library.py` (still targets the M1-05d form; not
  ported), IME/UIA, Linux/macOS, EW confirmation of Enter-in-label, Backspace
  join and the separate group for an appended unlabeled slide (recorded as
  Sela deviations in song-library.md).
- Next: M1-05f Slides tab with rendered thumbnails and label-colored caption
  bars (EW8-OBS-025), reusing `render_preview` at thumbnail size with a bounded
  cache.

### M1-05f — Song Editor Slides tab thumbnails — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**. Phase 2 of the owner-approved Song Editor
  plan. Source: EW8-OBS-024 and the installed 8.0.49 capture
  `.amp/in/artifacts/reference/ew8-w06-editor/25-slides-tab.png` (an appended
  unlabeled slide shows a neutral bar with grey italic "Slide 9", so Sela uses a
  neutral bar for every unlabeled slide; unlabeled slides inside a labeled group
  were not observed).
- `src/song_library.rs`: `render_preview` split into `coverage` and
  `image_from`; `render_thumbnail` box-filters the 1280x720 coverage 4x to
  320x180 so thumbnails keep the audience layout. `ensure_thumbnails` renders
  one missing slide at a time on the background executor, only in Slides, and
  prunes the text-keyed cache to the current slides on every render (dropping
  images from the atlas): at most 129 x 225 KiB. `thumbnail_row` draws number,
  thumbnail and caption bar with the selection frame; the Slides pane is 264 px.
- Bug found natively and fixed: in Slides, `+` and undo focused a Words cell
  (or a removed thumbnail) that is not rendered, so Ctrl+Q stopped reaching
  the editor. `focus_cell` now focuses the slide's thumbnail in Slides and
  history moves a focused thumbnail to the restored slide. Preview double-click
  switches to Words (Sela substitute for EW canvas editing).
- Tests: unit `thumbnail_is_the_preview_box_filtered`; GPUI
  `slides_tab_thumbnails_render_off_thread_and_prune_edits` (no rendering in
  Words, order, pruning after a Words edit, rejected text, `+` then Ctrl+Z
  keeps focus in the editor).
- Checks (Windows 11): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 82, bin 61 + 2 ignored, other
  suites pass), `cargo build --locked`, `python -m ruff check scripts`.
- Native: `python scripts/song-editor-windows.py` PASS twice, now also
  clicking Slides (thumbnail samples black 16816 / white 47), selecting slide 1
  (preview follows), `+` (grey italic "Slide 3"), Ctrl+Z, Ctrl+Q exit 0.
  Captures `05-slides`, `06-slide-1-selected`, `07-unlabeled-slide`
  inspected. The driver clicks logical coordinates for this layout at 125%.
- Not run: IME/UIA, Linux/macOS, EW confirmation of Slides keyboard navigation
  and of captions for unlabeled slides inside a labeled group.
- Next: M1-05g per-song text formatting (font, auto/fixed size, color, B/I/U,
  shadow, outline, alignment), following the EW help as provisional
  (EW8-OBS-026), with a schema change behind the verified-backup gate.

### M0-02 / RUN-W06G — EasyWorship Format pane observation — 2026-10-06 (UTC+7)

- State: observation only, no code. Owner asked to retry observing the Format
  panel before M1-05g. RUN-W06G recorded EW8-OBS-027–033 in
  reference-observations.md; EW8-OBS-026 is superseded. Nothing was saved in
  EasyWorship (every editor session closed with Cancel → No).
- Retry findings: the editor opened by the driver ignored all toolbar input
  again (`mouse_event`, held `SendInput`, `PostMessage`, touch injection;
  `HTCLIENT` hit test, no overlay, UIA shows only the preview child). After the
  owner restarted EW and reopened the editor, toolbar clicks worked. Remaining
  synthetic-input quirks (toolbar hit area over the pane's tab row; B/I/U clicks
  reopening the font list) are recorded as tooling caveats, not behavior.
- Owner suggested switching the reference to EasyWorship 7; recommendation was
  to keep the pinned 8.0.49 (all prior observations and the compatibility
  target are EW8; EW7 would mix builds). The owner continued on EW8.
- Key result for M1-05g: formatting is per slide in 8.0.49 (Shadow → None on
  slide 1 left slide 3 unchanged and persisted across slide switches). The
  Slide pane (EW8-OBS-032) covers the M1-05h background controls.
- Not observed: applying formatting to all slides at once, Reset Styles,
  Masters, what Apply/OK saves, schedule/live rendering of a formatted song,
  click behavior of B/I/U and the font list (only read from captures).
- Next: owner decides M1-05g scope (per slide as observed, or per song first
  as a recorded deviation), then implement with the schema change behind the
  verified-backup gate.

### M1-05g1 — Per-slide format model and schema 3 — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**; M1-05g stays **active** (g2, g3 open).
- Owner decisions: formatting is per slide (EW8-OBS-033); Ctrl+A selects all
  slides so one change formats the whole song; typing during Ctrl+A replaces
  the whole song text like EW (undoable). The Ctrl+A behavior is
  owner-reported, not yet observed. Approved plan: g1 model/storage, g2
  rendering with installed fonts, g3 Format pane.
- `src/format.rs` (new, no GPUI): `SlideFormat` with optional font, B/I/U,
  `Size::{Auto, Fixed}`, color, `Align`, `VAlign`, `Outline`, `Shadow`;
  `is_valid` bounds (Sela's, not EW's); canonical versioned codec with strict
  decode. `storage::Section.format`; `Song::validate` rejects invalid formats.
- `src/storage.rs`: `SCHEMA3` `section_formats` (rows only for non-default
  formats, FK to `section_ids`); fresh profiles start at 3; schema 2 migrates
  behind a verified `.schema2-backup`, schema 1 straight to 3 behind its
  single `.schema1-backup`; reads reject bad rows as `Corrupt`; copy/restore
  accept schemas 1–3; unsupported-version tests moved to 4. Editor history
  byte accounting includes font names. Other `Section` literals gained a
  default format.
- Tests: 3 `format::tests`, 5 new storage tests (migration, gates, abort hook
  `SELA_ABORT_MIGRATION_SCHEMA3`, round trip per revision, invalid/corrupt
  rows); schema 1 migration test asserts schema 3 and a single backup.
- Checks (Windows 11): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 90, bin 61 + 2 ignored, other
  suites pass), `cargo build --locked`.
- Not run: native scripts (no UI or renderer change in this slice), migration
  of a real owner profile (none exists outside tests), Linux/macOS.
- Next: g2. Extend `PreparedText`/transport with the resolved format, load
  installed fonts off the UI thread with bounds, render fill/outline/shadow
  masks and blend them with colors in the compositor, and move the editor
  preview/thumbnails to the same path.

### M1-05g2a — Styled slide rendering core — 2026-10-06 (UTC+7)

- State: **implemented-unqualified**; M1-05g stays **active** (g2b, g3 open).
- One rendering path for the whole `SlideFormat`: audience frame, editor
  preview and Slides thumbnails all go through `slides::cue` →
  `audience::text::layers` → blend (GPU shader or CPU twin). Point-valued
  effects (outline size, shadow offset/blur) scale by the 1080-line reference
  height so the audience and previews agree at any extent.
- `src/fonts.rs` (new, no GPUI): installed-font `Catalog::scan` with bounds
  (8 MiB files, 64-face collections, 8192 faces, 32 MiB loaded-face cache),
  SHA-256-tracked faces, `Resolved` (bundled DejaVu Sans regular + Bold from
  dejavu-fonts 2.37, license note in `tests/fixtures/README.md`).
  `Resolved::bundled` resolves without I/O; scanning stays off the UI thread.
- `src/scene.rs`: `OwnedText`/`TextStyle`/`OutlineStyle`/`ShadowStyle` with
  strict bounds; `PreparedCue::from_owned` validates face index and style.
- `src/slides.rs`: format-aware cue (Bold → bundled Bold face, else synth
  margins 1/24 em; Italic 0.25 em shear margin; underline; color; H/V
  alignment); auto-size fit and `Fixed` refuse-if-unshowable; `size_cap`
  resolves the worst-case size across slides for scrolling checks.
- `src/transport.rs`: text wire tag 2 carries the resolved format (face
  index, size, color, align, valign, B/I/U flags, outline, shadow); tag 1
  rejected with a compatible-version error.
- `src/audience/text.rs`: `layers` renders fill/outline/shadow coverage
  (RGBA, `Rgba8Unorm`) via cosmic-text: per-line advance/line-height overflow
  authority, alignment, synth B/I (shear, stroke double), underline through
  cosmic decorations, chamfer dilate (outline, units are 12ths of a pixel),
  axis-generic box blur (shadow), ink clipped only at the canvas edge.
- `src/audience/compositor.rs`: three-channel WGSL blend (shadow → outline →
  fill in linear light), 64-byte uniform, colored readback checks; exact CPU
  twin `blend_pixels` for the editor preview/thumbnails.
- `src/song_library.rs`: preview and thumbnails render via the same `layers` +
  CPU blend, cache key is now `(text, format)`; the thumbnail box filter
  averages all 16 preview pixels per output pixel (the old filter sampled
  one). `src/audience.rs` prepares frames from coverage + `Blend`; text work
  stays on the worker. `src/operator.rs` resolves bundled faces per send;
  installed-family resolution is deferred to g2b.
- Decisions: fake-italic shear and negative left bearings clip at the area
  edge like other effects (the advance-fit authority rejects oversized
  text; cosmic-text drops glyphs past the buffer width either way); EW
  parity for negative-bearing rejection was not observed, recorded here.
- Tests: fonts 6; scene/slides/transport suites updated for owned text and
  the wire tag; audience text 16 (styled fill/outline/shadow geometry, synth
  B/I/U, clip-vs-overflow, whole-format 1920×1080 render); compositor colored
  readback (fill blue / outline red / 50% green shadow, half-linear green
  187–189); song library styled preview/thumbnail ink.
- Checks (Windows 11): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked --all-targets` (lib 99, bin 65 + 2 ignored, output
  9 + 1 ignored, transport 4 + 1 ignored), `cargo build --locked`.
- Not run: native DX12 audience-window E2E with a styled cue (g2b, needs the
  operator format-resolution slice), installed-font scan against a real
  Windows font directory (g2b), Linux/macOS.
- Next: g2b. Operator resolves installed families via the catalog with a
  loading gate, seed one formatted song, extend the song-editor and
  live-output native scripts, run the Windows DX12 audience check, measure
  layer preparation timing, update `docs/composition-text.md`, backlog and
  work log, commit.

### M1-05g2b — Installed-font gate, native DX12 qualification — 2026-10-06 (UTC+7)

- State: M1-05g **g2 done** (g2a core + g2b integration); g3 (Format pane)
  stays open, so the ticket stays active.
- One shared font store: `fonts::shared()` (catalog + bounded face cache).
  Whichever window opens first (operator or song editor) scans the system
  font directories off the UI thread and installs the catalog; resolution
  can read files and stays on background threads.
- Operator gate: a previewed song's formats resolve as one background job
  (`Item.resolved`, live item first). A Go Live/Next/Previous that races the
  resolver keeps the current live scene, shows `Resolving fonts…` and
  retries when the faces land; a late catalog scan resets items that named a
  family (their earlier resolution was the bundled fallback), re-resolves
  and refreshes the live cue, and drops the stale size cap. `send` never
  resolves on the UI thread.
- Song editor: `pixels` resolves through the shared store (off the UI
  thread), preview and thumbnails agree with the audience, and the preview
  caption shows the fallback warning (`Font "X" unavailable · showing
  DejaVu Sans`, amber) while still rendering the bundled fallback.
- Seed: `seed_library --formatted-song` now seeds a single-song library
  with three styled slides (bold gold right/bottom + Outer outline +
  shadow at a fixed size, synth italic + underline centered, installed
  Arial bold italic) for the native check.
- Native scripts: `live-output-windows.py` gains a second operator phase
  on the formatted library — color-aware checks (gold pixels, margin-based
  right/bottom assertion, centered ink box), Next-stops-at-end, clean
  exit. `song-editor-windows.py` unchanged and re-run (regression with the
  editor scan).
- Checks (Windows 11, secondary monitor 2560x1600):
  `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --
  -D warnings`, `cargo test --locked --all-targets` (lib 99, bin 67 + 2
  ignored, other suites pass), `cargo build --locked`, `ruff check
  scripts`; `scripts/live-output-windows.py` **PASS** (both phases, gold
  box 590,1378-2512,1514, margins 48/86 vs 590/1378; italic centered;
  Arial visible; both operators exit 0 and end their children);
  `scripts/song-editor-windows.py` **PASS** (preview black=38854/white=931,
  thumbnails black=16784/white=74).
- Two native iterations before PASS, both check-bound, not render bugs:
  auto-fit fills the area width, so Right alignment is invisible with a
  fitted line — the gold slide uses `Fixed(120)` — and the "pushed right"
  assertion became a margin comparison (right/bottom margins smaller than
  left/top) instead of assuming a narrow block.
- Measured (`large_text_preparation_time`, ignored): 2560x1600 1/2/4
  lines p50 5.5/14.7/26.4 ms (max 6.8/15.7/28.4); 3840x2160 p50
  7.7/20.5/34.5 ms (max 8.4/21.7/36.2). Recorded in
  `docs/composition-text.md`.
- Not run: Linux/macOS, physical-projector/output-device checks beyond the
  secondary monitor, a real owner profile with installed-family formats.
- Next: g3. Enable the toolbar Format toggle as a right pane (Text › Style
  subset), apply to the caret/selected slide, Ctrl+A whole-song selection
  with replace-typing, whole-document history, GPUI tests, extend the
  native script.
- Docs fix (2026-10-06, follow-up commit): these two entries and the
  M1-05g2 section of `docs/composition-text.md` were first written through
  PowerShell double-quoted strings, which consumed every inline-code
  backtick (and turned some following letters into control characters).
  Restored from the source; write docs with the file tools or .NET
  `File.WriteAllText`, never through PowerShell string interpolation.

### M0-02 / RUN-W06H — Ctrl+A whole-song formatting observation — 2026-10-07 (UTC+7)

- State: observation only, no code. Phase 0 of the approved M1-05g3 spec
  (owner answers: EW-style font rows with samples, full sliders and angle
  dial now, observe first with the owner present). Recorded
  EW8-OBS-034–036 in reference-observations.md; captures in
  `.amp/in/artifacts/reference/ew8-w06h-format/`.
- Result: Ctrl+A in Words selects every slide's labels and lyrics with the
  caret at the end of the last slide; Italic then applied to every slide
  (Words and Slides thumbnails); the Slides tab shows every thumbnail
  selected. Leaving Words for Slides and back ended the selection. The
  owner ended the run and confirmed that after Ctrl+A every format choice
  (font and the rest) applies to all slides.
- Not observed (Sela choices recorded in the g3 entries): undo of a
  whole-song change, typing/paste/Backspace over the selection, Copy,
  slider/dial drags, Size step amount, Reset Size/Styles, what the pane
  shows for mixed values. A synthetic Italic click focused the font field
  instead (RUN-W06G quirk); the family became Agency FB on all slides
  without an agent choice, trigger unrecorded.
- The owner closed EasyWorship himself; the agent saved nothing.
- Next: g3a Format pane (per-slide apply, sliders, dial, undo), then g3b
  Ctrl+A whole-song selection.

### M1-05g3a — Song Editor Format pane — 2026-10-07 (UTC+7)

- State: g3a **implemented-unqualified**; g3b (Ctrl+A) open, so M1-05g
  stays active. Behavior and provenance are in
  [song-library.md](song-library.md#m1-05g3a--format-pane).
- Code: new `src/song_library/format_pane.rs` (pane state, controls,
  popovers, sliders, dial, `format_targets` = the caret slide today, so
  g3b only widens that range). `song_library.rs`: toolbar button 18 toggles
  the pane, which docks right of the preview; `Preview.fitted` carries the
  laid-out size (1080-reference px) from the same off-thread raster; root
  mouse move/up listeners end a drag anywhere; undo/redo end a drag first;
  `begin` drops one; pane fields lock with the editor. `main.rs` binds the
  `SelaMenu`/`SelaSlider` keys through `song_library::bind_keys`.
- Sela choices (EW unobserved): A▾/A▴ step 2 px; a whole drag is one undo
  step; out-of-range typed values are refused rather than clamped; the
  palette is Sela's own; a dismissing click on a trigger does not reopen
  its menu.
- Fixed during the slice: the hex field stayed read-only after the
  initial library open (`sync_input_lock` locked it, nothing unlocked
  it), so `sync_pane` now resets it each render; a click on pane chrome or
  a disabled control moved focus to the window root, so the pane root
  prevents the default focus change (fields still focus first in bubble
  order).
- Checks (Windows 11): `cargo fmt --all -- --check`, `cargo clippy
  --locked --all-targets -- -D warnings`, `cargo test --locked
  --all-targets` (lib 99, bin 77 + 2 ignored, other suites pass), `cargo
  build --locked`, `ruff check scripts`; `scripts/song-editor-windows.py`
  **PASS** (preview black=38854/white=931, thumbnails black=16784/white=74,
  red preview samples 4528, stored format `01 22 00 01 FF 00 00` for slide
  1 only). Captures `08-before-format` to `12-format-saved` inspected:
  popover under Color, red bold slide 1 in preview and thumbnail, slide 2
  unchanged, disabled outline/shadow values dimmed.
- Not run: Linux/macOS, screen reader/IME, a mouse drag of a slider in the
  native script (GPUI tests cover drags), a font choice from the installed
  list natively.
- Next: g3b. `SelectAllSlides` on Ctrl+A in Words and Slides, all-slides
  visual, exit on caret/click/tab/undo, replace-typing as one undo step
  (one unlabeled slide keeping slide 1's id and format, arrangements
  pruned), Copy of the whole text, tests, native script, deferred tickets.

### M1-05g3b — Whole-song selection and replace-typing — 2026-10-07 (UTC+7)

- State: g3b **implemented-unqualified**, so M1-05g moves to
  **implemented-unqualified** (not done). Behavior and provenance are in
  [song-library.md](song-library.md#m1-05g3b--whole-song-selection).
- Code: `text_input.rs` gains `intercept_typing` (platform text and IME
  input is offered to the owner first, with whether it replaces the whole
  text) and a public `select_all`. `song_library.rs`: `SelectAllSlides`
  bound to ctrl-a/cmd-a in `SongWords > SelaTextInput` and `SongWords`
  (Words list and Slides list); `all`/`all_focus` state;
  `select_all_slides`, `exit_all`, `song_text`, `replace_all`, `all_key`;
  `whole_song_keys` captures the cell edit keys and the caret keys on the
  Words list; render ends the mode when focus enters another cell; every
  structure, history, tab and song-open path calls `exit_all`.
  `format_pane.rs`: `format_targets` is every slide while `all`.
- Sela choices (EW unobserved): the replacement is one unlabeled slide with
  slide 1's id and format and arrangements pruned; Enter replaces with a
  line break; Copy/Cut use label-above-lyrics text with blank lines between
  slides; the Text pane stays available while all slides are selected (EW
  shows only the Slide pane, EW8-OBS-035).
- Fixed during the slice: the first native run stored only "H" of
  "Hallelujah". The replacement removes the cell being typed in (slide 2),
  and Windows keeps sending characters to that removed cell's input
  handler until the next paint. A removed cell now forwards committed text
  to the focused cell (outside the Library borrow, since that cell's own
  hook reads the Library); a GPUI test drives the removed cell directly.
- Checks (Windows 11): `cargo fmt --all -- --check`, `cargo clippy
  --locked --all-targets -- -D warnings`, `cargo test --locked
  --all-targets` (lib 99, bin 79 + 2 ignored, other suites pass), `cargo
  build --locked`, `ruff check scripts`; `scripts/song-editor-windows.py`
  **PASS** (earlier steps unchanged: preview black=38854/white=931,
  thumbnails 16784/74, red 4528; then stored formats `01 22 00 01 00 FF 00`
  and `01 20 00 00 FF 00` for both slides after Ctrl+A, and after typing
  over Ctrl+A one slide "Hallelujah" with slide 1's format). Captures
  `13-all-selected` (both thumbnails framed), `14-all-green` and
  `15-typed-over` (one unlabeled slide, green bold preview) inspected.
- Not run: Linux/macOS, IME composition over the selection, screen reader,
  a native mouse click ending the selection (GPUI test covers caret keys
  and focus change), EW confirmation of the replace result and undo.
- Next: M1-05h per-song background (color or Media image), following the
  RUN-W06G Slide pane observations (EW8-OBS-032).

### M0-02 / RUN-W06I — Slide pane background observation — 2026-10-07 (UTC+7)

- State: observation only, no code. Slice h0 of the approved M1-05h spec
  (`~/.factory/specs/2026-10-07-m1-05h-per-slide-backgrounds-with-a-song-master.md`).
  Recorded EW8-OBS-037–043 in reference-observations.md; captures in
  `.amp/in/artifacts/reference/ew8-w06i-background/`.
- Documented sources read first: help.easyworship.com "Edit a Song"
  (© 2026, build not stated) and the EasyWorship blog "Individual Song
  Settings" (2020-01-17, EW7 Inspector): a background can be set per
  slide, or for the whole song through the Masters tab or Apply to Theme.
  8.0.49 has no Masters tab (EW8-OBS-021); its Master is the theme layout
  behind Edit Slide Layouts (EW8-OBS-043).
- Owner decisions before the run (binding for M1-05h): per slide with a
  master; resolution slide → song master → black (theme slots in with
  M1-08); a new image defaults to Zoom, not EW's Auto + Stretch; Maintain
  bars are black; a missing or changed image renders black with a visible
  warning in Preview, never changing the live scene; color and image in
  this ticket, gradient (EW lists Gradient Fill) and video later.
- Result: per-slide background with a theme Master fallback; a new slide
  uses the Master, not the previous slide's choice; Ctrl+A applies a fill
  to every slide and shows blank values where slides differ; Select
  Media… is a category/search/thumbnail picker without import; choosing an
  image resets aspect to Auto + Stretch; Maintain letterboxes in black.
- Not observed: Master edits reaching slides (synthetic clicks ignored in
  the Layouts tab), whether a Master edit changes the theme for other
  songs, how the Layouts tab closes, Gradient Fill controls, None fill
  output, what Auto means, background undo, saved/live rendering.
- Privacy: the first capture used a screen grab of the editor rectangle
  while another application covered it; that file was deleted unread
  beyond the first view and later captures use `PrintWindow`, or a screen
  grab of a popup rectangle only.
- The agent closed the editor with Cancel → No; nothing was saved.
- Next: h1, `src/background.rs` model, `Song.master` and
  `Section.background`, schema 4 with a verified backup, slide resolution.

### M1-05h1 — Slide background model and schema 4 — 2026-10-07 (UTC+7)

- State: h1 **implemented-unqualified**; M1-05h stays active. Contract in
  [storage](storage.md#m1-05h1--slide-backgrounds--backed-up-schema-4).
- Code: new `src/background.rs` (`Background`, `Fill`, `Aspect`, `ImageRef`,
  codec, `resolve`). `storage.rs`: `Section.background`, `Song.master`,
  validation, `SCHEMA4` (`song_backgrounds`, `section_backgrounds`), one
  backup for any schema below 4, `SELA_ABORT_MIGRATION_SCHEMA4` test hook,
  load and save. `slides.rs`: `Slide.background`, `section_slide(section,
  master)`. `song_library.rs`: the preview slide and thumbnails resolve
  against the draft's master, the cache key includes the background, and
  the whole-song replacement keeps slide 1's background. Every other new
  section (+, Ctrl+Enter split, blank song) has none, so it follows the
  master like a new EW slide (EW8-OBS-041). `scripts/song-library.py` (Linux
  replay, stale since schema 3) now expects schema 4.
- Checks (Windows 11): `cargo fmt --all -- --check`, `cargo clippy --locked
  --all-targets -- -D warnings`, `cargo test --locked --all-targets` (lib
  108, bin 79 + 2 ignored, other suites pass), `cargo build --locked`,
  `ruff check scripts`; `scripts/song-editor-windows.py` **PASS** (unchanged
  samples: preview black=38854/white=931, thumbnails 16784/74, red 4528).
- Not run: `scripts/song-library.py` (Linux/X11 replay), Linux/macOS.
- Next: h2. `background::fit` (Maintain/Stretch/Zoom to the output extent,
  black bars), an images worker `Decode` job with a bounded cache in the
  operator, a "Preparing background…" gate, black plus a Preview warning for
  a missing or changed image, editor preview/thumbnail compositing over the
  fitted image, the native live-output check.

### M1-05h2 — Slide backgrounds render — 2026-10-07 (UTC+7)

- State: h2 **implemented-unqualified**; M1-05h stays active. Behavior in
  [song library](song-library.md#m1-05h2--slide-backgrounds-render).
- Code: `background.rs` `plan` (black for none, no fill or an empty Media
  Fill) and `fit` (Zoom crop, Stretch, Maintain on black bars; `image`
  Triangle resize, exact copy at equal size). `scene.rs` shares the bounded
  PNG/JPEG decode (`decode_image`) and `PreparedBackground::Image.rgba` is
  `Arc<[u8]>`. `images.rs`: `Substitute`, `decode_background` (hash pinned,
  16384-edge/64 MiB source budget), `fitted_background`,
  `background_version`, `BackgroundCache` (96 MiB, minimum 3, LRU,
  substitutes free), `Job::Background`/`Reply::Background`. `slides::cue`
  takes the prepared background and refuses an image of another size;
  `slides::color_background`. `operator.rs`: prefetch (deferred, live,
  preview, ±1; output size or 1920×1080 while off; capped at cache
  capacity), one background job at a time, the "Preparing background…"
  gate retried on landing, black plus warning in Live status and the
  Preview pane, substitutes forgotten on a rescan, color fills in tiles.
  `song_library.rs`: `Backdrops` (profile folder, four fitted 1280×720
  images) for the preview and thumbnails, caption warning.
  `compositor.rs`: `blend_over`, and `check_readback` compares it with the
  GPU over a fitted image. The existing "colored layer blend order" check
  expected blue for an uncovered pixel over black since g2a (never run
  since); it now expects black. `seed_library --background-song`;
  `live-output-windows.py` third phase (and `ruff format` of the file).
- Checks (Windows 11, RTX 4060 Laptop): `cargo fmt --check`, `cargo clippy
  --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets`
  (lib 116, bin 83 + 2 ignored, other suites pass), `cargo build --locked`,
  `ruff check scripts`, `ruff format --check scripts/live-output-windows.py`;
  `target/debug/examples/composition_spike <out> vulkan` and `dx12` **PASS**;
  `python scripts/live-output-windows.py --out
  .amp\in\artifacts\live-output-windows-m1-05h2` **PASS** on a 2560×1600
  audience (Zoom corners green, Maintain bars black with red/blue edges,
  missing image black with the warning visible in `operator-backdrop-missing`,
  master color; observed 829/391/469/485 ms in a debug build, comparable to
  text slides); `python scripts/song-editor-windows.py --out
  .amp\in\artifacts\song-editor-windows-m1-05h2` **PASS** (samples unchanged).
- Not run: Linux/macOS, a 4K output (cache minimum path is unit-tested only),
  a separate timing of the UI-thread frame encode of the fitted image, the
  editor with an image background natively (needs h3's UI or a seeded song
  opened in the editor; unit and GPUI tests cover it).
- Next: h3. Format pane Slide tab with the Background section (Fill, color,
  Select Media… with Import, Aspect), master editing, Ctrl+A, undo, GPUI
  tests, `song-editor-windows.py`; then the new tickets (gradient, opacity/
  rotate/flip, Theme Elements/Edit Slide Layouts, video backgrounds).
