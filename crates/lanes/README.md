# viden-lanes

## Purpose

`viden-lanes` owns lane orchestration: the durable lifecycle of an agent lane
and every local side effect a lane command is allowed to cause. It holds the
lane state machine (`LaneSupervisor`), the per-lane worker thread that
serializes commands against one lane, and the effect executor that performs
worktree, process, terminal, and patch work.

It sits below `viden-runtime` and knows nothing about sessions, providers, ACP,
or any frontend.

## Does Not Own

- Session, provider, or agent-adapter state.
- The permission decision sequence itself; the runtime injects it.
- The event redaction policy; the runtime injects it.
- Lane persistence internals; the runtime supplies a `LanePersistence`.
- Direct operating-system access. Every effect goes through `viden-tools`
  backends.

## Public Surface

- `LaneSupervisor`, the lane state machine and command entry point.
- `LanePersistence` and `WorkflowLanePersistence`, the lane fact store seam.
- `LaneEffectExecutor`, `LaneEffectRequest`, `LaneEffectResult`, and
  `LocalLaneEffectExecutor`, the lane effect seam and its local implementation.
- `LaneEventSink`, where lane events are published.
- `LaneApprovalResolver`, how a queued approval is re-validated against the
  runtime's shared permission gate.
- `LaneCommandRedactor`, what an announced command may reveal.
- `workspace_eligibility` and `resolve_lane_output_log`.

## Invariants

- Permission checks precede effects. The lane worker resolves every mutation
  against a lane-scoped `PermissionEngine` before calling the effect executor,
  and a queued approval is re-validated through the injected resolver so the
  plan-mode re-check can still deny an operator "allow".
- A queued approval is honored only while its scope, expiry, and permission
  epoch all still hold.
- Runtime-owned policy is injected, never imported. This keeps the dependency
  edge one-directional and is enforced by
  `scripts/check-dependency-boundaries.sh`.
- One worker thread owns one lane, so commands against a lane are serialized.

## Test

```bash
cargo test -p viden-lanes
```

Lane behavior is additionally covered end to end by the runtime suite, which
drives lanes through `RuntimeSupervisor`:

```bash
cargo test -p viden-runtime
```
