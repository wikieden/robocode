# Task 9 Runtime Migration and Projection Report

Date: 2026-07-20
Branch: `codex/v3-core-runtime`
Worktree: `/Users/wiki/Documents/GitHub/viden/.worktrees/v3-core-runtime`
Starting HEAD: `e81718f14485b912f9f7a938be999f467dc5df4c`
Storage checkpoint: `0af55fcc feat(core): store typed lane lifecycle events`

## Scope

Completed the remaining runtime integration for typed Lane persistence without
reworking the Task 9 storage checkpoint:

- session resume now runs the same project-scoped, one-time legacy TSV import
  before switching the active session store;
- a TSV that appears after `SessionEngine` construction is therefore imported
  at the next resume activation boundary;
- runtime Lane projection continues to read only the reduced typed
  `lanes.jsonl` state;
- English and Chinese workflow, architecture, and contract-freeze documents
  now describe `lanes.jsonl` as canonical and TSV as migration-only input.

## TDD evidence

### RED

Added
`legacy_lane_migration_runs_once_at_resume_and_runtime_replays_typed_state`
before changing production code, then ran:

```text
cargo test -p viden-runtime tests::runtime_contract_tests::legacy_lane_migration_runs_once_at_resume_and_runtime_replays_typed_state -- --exact --nocapture
```

The test failed at the migration boundary with `left: 0`, `right: 4`: the
session resumed, but a legacy TSV created after engine construction was not
imported.

### GREEN

`SessionEngine::handle_resume` now checks the legacy path and invokes
`WorkflowStore::import_legacy_lanes_tsv_once` before activating the resumed
store. The regression then proves all three runtime requirements:

1. the Task 2 legacy fixture produces four typed lanes and one migration audit;
2. a typed `StatusChanged` event is replayed into `RuntimeViewState` with its
   updated status and summary;
3. corrupting the legacy TSV after import does not affect runtime projection,
   and a repeated resume leaves one migration audit and two total typed events.

The focused RED command then passed.

## Contract and lifecycle notes

- `lanes.jsonl` remains the canonical append-only Lane lifecycle log.
- Migration source/schema/audit structure remains owned by the existing
  `viden-workflows` Task 9 storage contract.
- Import happens before `self.store` is replaced during resume. A malformed
  first-time migration therefore fails the resume without half-switching the
  active in-memory session boundary.
- Once the migration audit exists, repeated startup/resume activation is
  idempotent and does not read or parse the legacy TSV again.
- `runtime_state_events` and `/status` consume `WorkflowStore::load_lane_state`;
  no TUI lane reducer or TSV projection path was added.

## Changed ownership scope

- `crates/runtime/src/session_lifecycle.rs`
- `crates/runtime/src/tests/runtime_contract_tests.rs`
- `crates/workflows/README.md`
- `crates/workflows/README.zh-CN.md`
- `docs/architecture.md`
- `docs/architecture.zh-CN.md`
- `docs/runtime-contract-freeze-status.md`
- `docs/runtime-contract-freeze-status.zh-CN.md`
- `.superpowers/sdd/task-9-report.md`

No TUI, GUI, integration-worktree, provider, release, or live-network files
were changed.

## Verification

Passed:

- `cargo test -p viden-workflows lane_`: 7 passed.
- `cargo test -p viden-runtime legacy_lane_`: 2 passed.
- `cargo test -p viden-runtime runtime_view_state_emits_lane_facts_from_core_store`:
  1 passed.
- `cargo test -p viden-runtime runtime_view_state_reports_corrupt_lane_store`:
  1 passed.
- `cargo test -p viden-workflows --quiet`: 24 passed.
- `cargo test -p viden-runtime --quiet`: 349 passed, 1 ignored.
- `cargo test -p viden-core --quiet`: all Core facade tests passed; one manual
  fixture-refresh test remained ignored.
- `cargo clippy -p viden-workflows --all-targets -- -D warnings`: passed.
- `cargo clippy -p viden-runtime --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `scripts/check-dependency-boundaries.sh`: passed.
- `scripts/check-doc-pairs.sh` for all three changed English/Chinese pairs:
  passed.
- `scripts/check-doc-links.sh` for all three changed English/Chinese pairs:
  passed.
- `git diff --check`: passed before staging; the staged report is checked again
  before commit.

## Remaining issues

- `cargo test --workspace --quiet` was attempted. It remains blocked by the
  separately owned TUI migration to the current Core facade: the TUI still
  imports removed `viden_core::SessionEngine`/provider symbols and constructs
  legacy string-valued `AgentTaskRecord.agent`, `transport`, `kind`, and
  `status` fields. Task 9 does not modify that worktree.
- Historical planning documents that intentionally describe the legacy TUI
  implementation remain historical; the current architecture and contract
  status documents now state the typed-store behavior.
