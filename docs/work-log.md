# Work log and resume point

Use [backlog.md](backlog.md) for ticket checklists and test boundaries. This file
records execution state, not a second copy of the backlog. Dates use ISO format;
include timezone for timed hardware/rehearsal evidence.

## Current state

- Application: native GPUI technical preview builds and renders on Linux software
  OpenGL; no service authoring or audience output yet.
- M0-01 bootstrap is **implemented-unqualified** pending Windows build/launch.
- Next slices: M0-03 native harness and M0-04 independent audience spike in separate
  worktrees; M0-02 reference research runs alongside, installed observation blocked.
- M1–M5 remain `planned`; M0-10 requires physical Windows evidence before full UI.
- Setup and application changes are local only; no push/publication authorized.
- Parallel opportunity: M0-02 reference observation when a lawful EasyWorship
  8.0.49 installation on Windows is available.
- Known qualification needs: real Windows GPU/displays; EasyWorship reference;
  later physical audio/capture/controllers/mobile devices and permitted provider
  accounts. None is assumed available merely because this repo is in an orb.
- Native E2E: not evaluated; M0-03 decides feasibility and records fallback.

## Ticket status index

Only tickets that have started or changed state need entries. Unlisted tickets
remain `planned`. Update the current state above when switching work.

| Ticket | State | Completed slice / remaining work |
| --- | --- | --- |
| PLAN-001 | done | 63 implementation tickets, 326 ticket checklist items, test/compatibility matrices and commit/resume rules. Documentation verified; delivered in this local planning commit. No application implementation. |
| M0-01 | implemented-unqualified | Pinned native GPUI window, lockfile, CI and instructions; Linux build/render/quit verified. Windows build and native launch open. |

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
