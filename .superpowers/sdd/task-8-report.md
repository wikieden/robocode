# Task 8 Core Runtime Fix Report

Date: 2026-07-20
Branch: `codex/v3-core-runtime`
Worktree: `/Users/wiki/Documents/GitHub/viden/.worktrees/v3-core-runtime`

## Delivered checkpoints

- `c7218dce Stabilize asynchronous runtime event tests`
  - ACP live-sink observation now waits for both the proposed merge gate and
    assistant delta, uses a monotonic condition boundary, and fails with the
    observed event set instead of silently returning partial results.
  - Lane approval tests now wait for the supervisor completion event after the
    effect and use a supervisor-queue barrier instead of a process-global Lane
    worker hook.
- Permission-control semantics checkpoint (the commit containing this report)
  - Ordinary blocking tool approvals fail closed as soon as a permission or
    work-mode control is submitted. A later persistence failure cannot revive
    the already resolved request; the user must trigger a new request.
  - Failed controls are removed from the applied-state projection, while their
    submitted generations remain monotonic and are never reused.
  - Lane approvals retain applied-epoch transactional semantics. Failed
    controls do not advance the Lane permission engine/epoch pair.

## Root causes and TDD evidence

### ACP live sink

The test waited only for `AssistantDelta`, used a one-second wall-clock deadline,
and accepted an unmet condition by returning partial events. Production emits
the proposed merge gate before the assistant delta through the same FIFO sink,
so the missing-gate assertion was an observation-boundary failure.

RED: `channel_event_wait_fails_at_unmet_condition_boundary` did not panic under
the old helper. GREEN: the helper asserts the requested condition at a monotonic
deadline and the ACP test requests both required events.

### Lane effect completion

`ApprovalResolved(Allow)` is emitted before the approved effect. The old test
returned on that event and immediately inspected the effect recorder.

RED: an added assertion showed that the returned event set deterministically
lacked the response `CommandAccepted`. GREEN: the collector now waits for both
the approval resolution and response-command completion.

### Lane permission snapshot ordering

The old test used a process-global Lane worker hook and a single release token,
which did not isolate the synchronization from parallel Lane workers. The
reported timeout contained only the ReadOnly command acceptance/snapshot and no
approval resolution, proving the supervisor was waiting at the Lane completion
barrier rather than missing a terminal event predicate.

GREEN: the test now blocks the uniquely named ReadOnly supervisor command,
queues `ReadOnly -> approval response -> Ask`, then releases the queue. This
tests the intended ordering directly and removes the global worker hook.

### Ordinary approval failure semantics

The previous failed-control test directly inserted a channel approval without a
real active job. That state is unreachable in production: a real ordinary tool
approval blocks the supervisor worker, so the approval must resolve before the
queued control can execute and encounter a persistence failure.

RED: the multi-reservation `fail e1 -> fail e2` case reported ordinary epoch 0
instead of the required monotonic epoch 2. GREEN:

- a real `SubmitUserInput -> ApprovalRequested -> submit failing control ->
  AllowOnce` flow resolves the stale request as `Deny` with no effect;
- after the failed control is rejected, a newly triggered approval can execute;
- `fail e1 -> success e2`, `success e1 -> fail e2 -> success e3`, and
  `fail e1 -> fail e2` clear all reservations and retain the correct applied
  state;
- an actual next reservation uses the next generation;
- the live snapshot and Lane permission template match the applied state and
  applied epoch.

## Final-review HIGH findings

1. Ordinary active-tool approval ordering: addressed by submission-time
   reservations and stale-Deny handling on the direct approval-response path,
   with the real-tool regression test above.
2. Lane response TOCTOU: addressed by the existing worker completion channel in
   `LaneSupervisor::respond_to_approval`; the supervisor does not publish the
   response command acceptance until the Lane worker has completed or rejected
   the approved operation. The synchronization regression now observes that
   completion event.
3. Session allow-rule owner isolation: current Lane permissions are worker-local,
   and `lane_supervisor_session_allow_rule_does_not_cross_lane_owner` covers the
   cross-owner case. The review finding referenced the pre-fix shared merge
   boundary rather than the current worker-local installation path.

## Verification

Passed:

- ACP focused test, failed-control Lane test, and Lane permission-snapshot test:
  20 consecutive runs each.
- `cargo test -p viden-runtime agent_commands::tests:: -- --nocapture`:
  60 passed.
- `cargo test -p viden-runtime lane_supervisor -- --test-threads=32`:
  passed; the group also passed 20 consecutive parallel runs before handoff.
- Both new permission-control tests: passed.
- `cargo test -p viden-runtime --quiet`, twice under concurrent load:
  348 passed, 1 ignored in each run.
- `cargo test -p viden-core`: passed (worker verification).
- `cargo clippy -p viden-runtime --all-targets -- -D warnings`.
- `cargo fmt --all -- --check`.
- `scripts/check-dependency-boundaries.sh`.
- `scripts/check-doc-pairs.sh docs/core-0.3-compatibility.md docs/core-0.3-compatibility.zh-CN.md`.
- `scripts/check-doc-links.sh docs/core-0.3-compatibility.md docs/core-0.3-compatibility.zh-CN.md`.
- `git diff --check`.

Attempted but blocked outside this Core fix scope:

- `cargo test --workspace --quiet`: the current TUI checkout still references
  removed Core facade exports and older task fields. Examples include
  `viden_core::SessionEngine` and legacy `AgentTaskRecord.agent/transport`
  accesses. No TUI, GUI, integration, live-provider, publish, push, or release
  mutation was performed.
