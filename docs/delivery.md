# M0-06 bounded delivery model

State: **implemented-unqualified**, CPU contract slice. `delivery` is a
single-owner Rust state machine, not an IPC implementation. No live GPU renderer
or operator indicator is wired to it yet. Tests exercise both sides with an
injected submission callback; that is not audience-output E2E.

## Sequence, acknowledgment and uncertainty

The supervisor supplies a fresh, never-reused 128-bit epoch per renderer session.
One monotonically increasing sequence covers both lanes. Content revisions are
not sequences: deliberately presenting an older saved revision remains possible.
Only successful queue admission consumes a sequence. Sequence exhaustion requires
a new session instead of wrapping. Constructors do not generate/persist epochs;
that belongs to the eventual process supervisor.

- Submission records requested content, not live content.
- `Accepted` means the renderer queued the command. It does not mean upload,
  composition or presentation succeeded, and does not free admission capacity.
- `Applied` means the compositor callback successfully submitted the scene. It
  does not establish physical scanout. The controller advances its confirmed
  version only on an applied receipt, never backwards on a reordered receipt.
- `Rejected` means the command did not replace applied content, except an ambiguous
  stale receipt: that can describe a duplicate of previously applied work. Without
  newer confirmation the controller invalidates the session rather than guessing.
- Pending and currently applied duplicates return their original status without
  another presentation. Older sequences and foreign epochs cannot enter the
  renderer queue. A wrong-epoch receipt for a current command invalidates the
  controller session; receipts carrying an old epoch are simply ignored.

The renderer checks deadlines at admission and immediately before invoking the
submission callback. The callback must use prepared resources, avoid blocking
I/O, and leave actual live resources intact on failure. `now` measures the start
of that commit attempt; it is not the completion/scanout timestamp. Missing the
controller acknowledgment deadline means output is **unknown**, because the
renderer may have applied a frame whose receipt was lost. Timeout/disconnect
clears all queued requests, retains last-confirmed history for diagnostics only,
and disables submission. Fresh sessions start unknown and empty; no automatic
replay. A supervisor must also retire the old transport/renderer before reconnect.

## Capacity and ordering

There is one outstanding normal slot and one reserved safety slot, including
sent-but-unacknowledged work. Saturation returns `Busy` without eviction or an
unbounded retry queue. Both ends enforce lane limits. FIFO preserves order across
lanes: a safety command has at most one normal command ahead of it. The renderer
can therefore reach it on the next commit attempt after that command; this is a
count bound, not a measured wall-clock safety-latency guarantee.

The lane currently carries a prepared replacement snapshot. It establishes
capacity/ordering, **not Black/Clear/Logo semantics**. M0-02/M0-08 must determine
mask precedence, restoration, media continuation and Go Live under mask. Safety
resources must be pre-resolved outside the media worker. The future operator
coordinator must cancel/reject superseded preparation before submission; blindly
submitting a delayed result after a newer operator intent would give it a new
sequence and incorrectly make it current. No automatic Go Live or selection
policy is introduced by this model.

Moving a command transfers its `Arc`; pending controller metadata keeps no extra
payload copy. Two outstanding cues plus a prior applied snapshot can retain up
to 192 MiB of decoded payload at the M0-05 per-cue limit, excluding preparation,
GPU memory and external caller references. No cache, socket buffers or unbounded
ack queue are added. Transport sizing/GPU budgets remain integration work.

## Verification and remaining integration

`cargo test --locked --all-targets -j 8` exercises all 24 permutations of two
accepted/applied receipt pairs, both cross-lane orders, 1000 overload attempts per
lane order, duplicate/reordered commands, render failure, exact expiry, unknown
state, old epochs, disconnect and real-worker missing-resource recovery. Expected
revisions differ (for example 3 versus 23), exposing stale-state regressions.
Counters are saturating and diagnostics contain no cue text, paths or pixels.

Next integration: M0-07 layout/upload/composition, a bounded versioned IPC adapter
in the existing two-process spike, fresh-epoch supervision, preparation-intent
coordination and native receipt-to-frame checks. `Arc` and `Instant` are not wire
types; IPC must explicitly translate resource handles and deadline clocks. Run
real Windows/reference checks before closing the parent or M0-10 gate.
