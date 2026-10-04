# Work log and resume point

Use [backlog.md](backlog.md) for ticket checklists and test boundaries. This file
records execution state, not a second copy of the backlog. Dates use ISO format;
include timezone for timed hardware/rehearsal evidence.

## Current state

- Application: native GPUI technical preview builds and renders on Linux software
  OpenGL, with persistent offline song authoring; no service schedule UI or live
  lyric presentation yet. An opt-in winit/wgpu
  audience-process diagnostic has measured independent animation; a separate
  static composition example now verifies explicit-font GPU readback.
- M0-01 bootstrap is **implemented-unqualified** pending Windows build/launch.
- Latest slices: **M1-05b / M1-16a** persistent native song editor and installed
  Linux developer preview checked. Next: M1-05c whole-document undo/section
  edit semantics, then M1-06 arrangements/pagination prerequisites.
  M1-05a native fields, M1-01a durable storage and M1-03a keyboard shell are merged,
  as are the M0-07c video and M1-02a/b shell/style slices. The owner
  explicitly permits UI implementation before physical renderer qualification;
  this changes sequencing, not reference, hardware or service-ready claims.
- M0-07a text and M0-07b GPU worktrees are merged into local `main`, retaining
  their individual subticket commits. Static composition is checked; resume
  native GPU-readiness/receipt integration next, with observed
  transition behavior pending reference access.
- M0-02 reference ledger, M0-03 native harness and M0-04 audience spike were
  developed in separate worktrees and merged into local `main`, with ticket
  commits and an M0-04 integration/failure-check checkpoint.
- Resume M0-04 on Windows DX12 with two physical displays, DPI/topology changes,
  hotplug and physical capture; run the M0-02 installed-reference runbook alongside.
  No Windows runner is connected. These required checks block gate closure.
- M1-02a may proceed under the approved early-UI exception; other milestones
  remain planned and M0-10 still requires physical Windows evidence for closure.
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
| M0-01 | implemented-unqualified | Pinned native GPUI window, lockfile, CI and instructions; Linux build/render/quit verified. Windows build and native launch open. |
| M0-02 | blocked | Public-source ledger and executable observation runbook; lawful installed 8.0.49/Windows access missing, zero installed observations. |
| M0-03 | implemented-unqualified | Three real Operator action/focus tests and repeated native X11 smoke; domain fixture extension and Windows/accessibility checks open. |
| M0-04 | implemented-unqualified | Separate audience process measured under UI stalls, bounded synthetic slow preparation and clean/forced operator exit; Windows physical qualification open. |
| M0-05 | implemented-unqualified | CPU snapshots and bounded worker tested; shaping/upload, renderer integration and applied-state checks remain. |
| M0-06 | implemented-unqualified | Bounded ordered model with reserved capacity and receipt tests; IPC, native integration and reference mask semantics open. |
| M0-07 | implemented-unqualified | Explicit-font color/image and decoded FFV1 GPU readback; native integration, transitions and physical qualification open. |
| M1-01 | implemented-unqualified | Durable song/schedule snapshot repository and bounded worker; UI integration, remaining domain schemas/backups/recovery open. |
| M1-02 | implemented-unqualified | Separate-pane shell and initial contemporary chrome; persistence/modes/reference/DPI checks open. |
| M1-03 | implemented-unqualified | Contextual keyboard access to shell with native checks; text-entry/modal/selection/live command ownership open. |
| M1-05 | implemented-unqualified | Native metadata/section authoring, save/load/duplicate/delete and dirty-close guard; full undo/reference/Windows qualification open. |
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
