# Engineering rules

- Keep GPUI as the operator frontend and Rust as the implementation language.
- Preserve the EasyWorship layout and behavior compatibility goal. Observe the
  pinned reference before choosing shortcuts, focus rules, drag/drop behavior,
  selection rules, or Black/Clear/Logo semantics. Record unknowns instead of guessing.
- Before implementing non-trivial GPUI patterns, inspect current
  `zed-industries/zed`, especially `crates/gpui` and relevant feature code.
  Record upstream paths and commits or issues in the implementation PR or design
  note. Pin known-good dependency revisions and update intentionally.
- Study patterns without blindly copying Zed internals. Check licenses and retain
  notices: GPUI and Zed's application UI components have different licenses.
- Keep domain types framework-independent. Use GPUI actions and key contexts for
  semantic operator commands; avoid ad-hoc global key matching.
- Never block the UI or live frame path with file, database, network, or decoding
  work. Bound queues and caches, with explicit backpressure and memory budgets.
- Live cues must be prepared before application, ordered, and acknowledged by the
  renderer. Failed or stale preparation must not replace the current scene.
- Test failure behavior as well as happy paths. Physical display, GPU, and native
  platform qualification cannot be replaced by headless tests.
- Do not claim implemented feature parity, performance, or crash isolation without
  executed checks. Add crates only when concrete ownership boundaries need them.

## Ticket execution and checkpoints

- Before implementation, read `docs/plan.md`, `docs/backlog.md`, and
  `docs/work-log.md`. Resume the active partial ticket before starting unrelated
  work; otherwise choose a dependency-ready ticket and record its ID and scope.
- Pursue comprehensive EasyWorship feature, layout, and UX coverage, not only
  the minimum Sunday subset. Use the backlog's compatibility inventory and
  build-specific observations. Add tickets for newly discovered gaps; never
  silently drop a workflow or claim parity for unobserved behavior. Keep original
  code/assets and explicitly track licensing/provider feasibility blockers.
- Keep checklist progress and the work log current in the same commit as the
  implementation. Record decisions, exact verification commands/results,
  unexecuted checks and their reasons, durable evidence, blockers, and the next
  concrete action so another session can resume without conversation history.
- Always make a local commit for each completed ticket. For larger tickets,
  commit coherent partial slices, including a checkpoint before pausing or ending
  a work session when changes exist. Use messages such as
  `feat(M1-05): add song metadata validation` or
  `wip(M0-04): checkpoint output timing spike`. Do not mix unrelated tickets or
  stage another contributor's changes. Do not create empty checkpoint commits.
- Run applicable checks before each commit. A necessary partial checkpoint may
  contain incomplete work, but disclose failing/unrun checks and the resume step;
  do not mark it done. If committing is blocked, record/report the exact blocker
  and preserve changes. Never push, publish, or deploy without authorization.
- Distinguish `planned`, `active`, `blocked`, `implemented-unqualified`, `done`,
  and explicitly approved `deferred` states. Missing Windows/GPU/reference/device
  checks keep a required qualification open even if headless tests pass.
- Follow the backlog's U/I/R/N/E/H/P/L test boundaries. Try native GPUI E2E where
  feasible; if unavailable or unreliable, record why and use reproducible manual
  native checks plus lower-level tests. Do not build a fake web operator UI just
  to automate it, and do not treat browser emulation as physical device evidence.
- Measure performance-sensitive changes on declared workloads/hardware; track
  startup, search, cue latency, frame pacing and CPU/GPU/cache budgets. Optimize
  measured bottlenecks without sacrificing correctness or safety-control priority.
