# Runtime Contract Freeze Status

Chinese version: [runtime-contract-freeze-status.zh-CN.md](runtime-contract-freeze-status.zh-CN.md)

This status records the core-only Phase 0-2 checkpoint for the Viden
Runtime-first refactor. It intentionally does not implement new TUI or GUI
surfaces.

## Scope

In scope:

- frontend-neutral runtime schema;
- core runtime bridge and command bus;
- non-UI supervisor boundary for submit/cancel/approval;
- process-plugin protocol draft;
- cross-frontend fixture replay;
- documentation of remaining UI-branch gates.

Out of scope for this branch:

- TUI rendering rewrites;
- GUI implementation;
- visual parity screenshots.

## Evidence

| Requirement | Current evidence | Status |
| --- | --- | --- |
| `viden-core` facade | `viden-core/src/lib.rs` re-exports `RuntimeSupervisor`, `SessionEngine`, and runtime contract types | Done |
| Runtime schema | `robocode-types/src/runtime.rs` defines `RuntimeCommand`, `RuntimeEventKind`, `RuntimeViewState`, approvals, evidence, provider health, cost, tool calls, tasks, lanes | Done |
| Runtime replay reducer | `RuntimeViewState::apply_event` and `robocode-types` tests replay snapshot, approvals, tasks, queued input, lane, evidence, provider, and cost facts | Done |
| Core bridge | `SessionEngine::runtime_snapshot`, `runtime_view_state`, `runtime_events_for_engine_events`, `handle_runtime_command`, and `process_runtime_input_with_approval` | Done |
| Command bus | Tests cover user input, queued follow-up, mode switching, permission-level switching, provider config, model selection, active model activation/deactivation | Done |
| Plan-mode mutation safety | Existing permission and workflow tests cover mutating tool denial and workflow task mutation denial while plan mode is active | Done |
| Supervisor boundary | `RuntimeSupervisor` tests cover active provider cancellation and approval response delivery without TUI coupling | Done |
| Permission/mode contract | `runtime_command_bus_covers_plan_build_review_permission_contract` covers plan/review/explore read-only behavior and build restoration to ask | Done |
| Lane facts emitted by core | `runtime_view_state_emits_lane_facts_from_core_store` proves `.robocode/lanes.tsv` is projected into `LaneUpdated` runtime facts without TUI code | Done |
| Provider/model, approval, lane, task, cost, evidence fixture | `robocode-types/tests/fixtures/runtime-contract-phase2.json` plus fixture replay test | Done |
| Process-plugin protocol draft | `docs/process-plugin-protocol.md` and Chinese counterpart | Done |
| Thin TUI client proof | Deferred to TUI client branch by phase constraint; current branch proves the shared fixture and API boundary only | Deferred |
| GUI API proof | Documented through runtime schema, fixture, GUI functional design, and process-plugin draft; executable GUI client tests wait for GUI branch | Deferred |

## Verification Snapshot

Latest local checks for this branch:

```bash
cargo test -p viden-core
cargo test -p robocode-types runtime_contract_fixture_replays_phase2_cross_frontend_facts -- --nocapture
cargo test -p robocode-core runtime_command_bus_covers_plan_build_review_permission_contract -- --nocapture
cargo test -p robocode-core runtime_view_state_emits_lane_facts_from_core_store -- --nocapture
cargo fmt --check
git diff --check
RUST_TEST_THREADS=1 cargo test --workspace --quiet
```

`cargo test --workspace --quiet` with default parallelism currently exposes an
unrelated TUI lane timing flake:
`tui::lane::tests::lane_run_refreshes_failed_exit_code_and_inspect_tail`.
The focused test passes, and the serial workspace gate passes. Because this
branch must not do UI-layer development, the flake is recorded here instead of
being fixed in this checkpoint.

## Next Handoff

The next branch can start from this contract boundary:

1. Core continues with context/token/cost and plugin runtime implementation.
2. TUI client branch consumes `viden-core` and the runtime fixture instead of
   directly calling core internals.
3. GUI branch starts only after the shared fixture set is sufficient for parity
   tests.
