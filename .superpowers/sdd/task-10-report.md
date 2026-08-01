# Task 10 Owner-Scoped Lane Runtime Evidence Report

Date: 2026-07-20
Branch: `codex/v3-core-runtime`
Worktree: `/Users/wiki/Documents/GitHub/viden/.worktrees/v3-core-runtime`
Audited HEAD: `7873dc4eebb92114717be51c483657fcd02f2e3e`

## Outcome

Task 10 requires no additional production or test code. Its acceptance surface
was already implemented and regression-tested by the Task 8 historical Lane
work, principally:

- `ba326a59 feat(core): add lane supervisor contract boundary`
- `cbf53ba1 feat(core): isolate owner-scoped lane runtimes`
- `3a16d329 fix(core): secure lane mutation boundaries`
- the subsequent Task 8 recovery, persistence, permission, and completion-order
  fixes through `e81718f1`

The gap audit found exact acceptance coverage rather than a missing behavior.
Adding another initially-green copy of these tests would violate the requested
strict TDD workflow, so no artificial RED or duplicate implementation was
created.

## Acceptance mapping

### Lane A approval does not block Lane B

`lane_supervisor_keeps_waiting_lane_isolated_and_routes_owner_events` creates
Lane A and leaves its start approval pending, then starts Lane B, sends input to
Lane B, and accepts Lane B output. The test waits for Lane B `Done` while Lane A
reaches `Cancelled` (`lane_supervisor_tests.rs:2698-2842`).

The same test verifies the effect trace contains create/start/input for Lane B,
does not contain start/stop for the still-unapproved Lane A, and observes Lane
B input queue/dequeue ordering (`lane_supervisor_tests.rs:2844-2929`).

### Cancel is owner-scoped

The test sends `CancelActiveTurn` with owner A only and proves Lane A becomes
`Cancelled` while Lane B becomes `Done`. The production cancel path resolves
the Lane worker selected by `owner.lane_id` and rejects owner mismatch before
sending `LaneWorkerMessage::Cancel` (`lane_supervisor.rs:478-496`).

### Receipt, error, and approval ownership

The integrated test proves:

- a wrong-owner approval response is rejected under owner B;
- Lane B input errors are emitted under owner B;
- every approval request/resolution is owned by Lane A;
- every Lane update and receipt (`LaneOutputAppended`) carries the owner whose
  lane ID matches the payload lane ID.

The exhaustive routing assertions are in
`lane_supervisor_tests.rs:2852-2948`.

### Plan rejects before effects

`lane_supervisor_plan_mode_rejects_effectful_commands_before_effects` sends
Create, Start, Apply, Cleanup, Archive, Accept, and Attach while the engine is in
Plan mode. It observes seven Plan-mode rejections owned by the target lane and
asserts the shared fake effect counter remains zero
(`lane_supervisor_tests.rs:170-270`). This strictly includes the four Task 10
required commands: create, start, apply, and cleanup.

### No global active slot

- ordinary runtime controls use
  `BTreeMap<RuntimeOwnerKey, ActiveRuntimeControl>`
  (`runtime_supervisor.rs:188-211`), not one global option;
- Lane workers use a `BTreeMap<lane_id, LaneWorkerHandle>`
  (`lane_supervisor.rs:65-78`);
- each `LaneWorkerHandle::spawn` owns a separate channel, permission state, and
  thread (`lane_worker.rs:73-145`).

### Detached remains active

The Task 2 follow-up is confirmed by `LaneStatus::is_active`: `Detached` is
explicitly included in the active set (`crates/types/src/agent.rs:97-110`). A
detached Lane may therefore remain counted as active while its background
runtime/session is still alive.

## Verification

Passed on audited HEAD:

- exact independent-Lane owner-routing test: 1 passed;
- exact Plan-before-effect test: 1 passed;
- `cargo test -p viden-runtime lane_supervisor_ --quiet`: 34 passed;
- `cargo test -p viden-runtime runtime_supervisor_ --quiet`: 57 passed;
- `cargo test -p viden-permissions plan_mode_ --quiet`: 2 passed;
- `cargo test -p viden-session --quiet`: 19 passed;
- `cargo test -p viden-runtime --quiet`: 349 passed, 1 ignored;
- clippy with `-D warnings` for runtime, permissions, and session: passed;
- `cargo fmt --all -- --check`: passed;
- `scripts/check-dependency-boundaries.sh`: passed;
- `git diff --check`: passed before adding this report; the staged report is
  checked before any checkpoint commit.

## Scope and remaining issues

No Core source, test, protocol, schema, workflow, TUI, GUI, integration,
provider, release, or live-network behavior was changed. No Task 11+ surface
was entered.

The repository-wide workspace gate remains owned by the separate TUI migration
to the current Core facade; Task 10's requested Core/runtime gates are green.
