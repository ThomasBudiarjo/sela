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
