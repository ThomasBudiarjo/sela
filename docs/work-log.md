# Work log and resume point

Use [backlog.md](backlog.md) for ticket checklists and test boundaries. This file
records execution state, not a second copy of the backlog. Dates use ISO format;
include timezone for timed hardware/rehearsal evidence.

## Current state

- Application: native GPUI technical preview builds and renders on Linux software
  OpenGL; no service authoring or lyric presentation yet. An opt-in winit/wgpu
  audience-process diagnostic has measured independent animation; a separate
  static composition example now verifies explicit-font GPU readback.
- M0-01 bootstrap is **implemented-unqualified** pending Windows build/launch.
- Active continuation: **M1-02b** Zed-inspired light styling of the native shell,
  preserving EasyWorship pane layout. M0-07c video and M1-02a shell are locally
  merged and checked. The owner
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
| M1-02 | active | M1-02a separate-pane shell merged/tested; M1-02b modern Zed-inspired light styling requested, no speculative live semantics. |

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
