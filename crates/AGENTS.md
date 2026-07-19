# Viden Core Track Agent Guide

## Scope

This file applies to `crates/**`. For V3 parallel development, this is the Core
ownership area. Read the repository-root `AGENTS.md`, `PLAN.md`, and
`docs/parallel-development-plan.md` before changing code here.

## Branch And Worktree

- Use branch `codex/v3-core-runtime` in `.worktrees/v3-core-runtime`.
- Create it from synchronized `origin/main`.
- Preserve unrelated work and do not edit another owner's worktree.
- Root workspace manifests, frontend contracts, and shared fixtures are shared
  surfaces; call out their impact in the handoff.

## C0 Contract Freeze

Before production TUI or GUI implementation starts, deliver an immutable
`frontend-contract-v1` checkpoint containing:

- the versioned `RuntimeCommand -> RuntimeEvent -> RuntimeViewState` protocol;
- `schema_version`, capability discovery, event cursors, snapshot/replay, and
  sequence-gap recovery;
- typed lane, task, gate, route, gate-strength, and mutation-policy records;
- a transport-neutral Core client contract;
- migration coverage for legacy persisted records and fixtures;
- parity fixtures shared by Core, TUI, and GUI.

Report the exact checkpoint SHA and do not rewrite that checkpoint after TUI or
GUI branches have started from it.

## Ownership And Boundaries

- Core exclusively owns authoritative runtime facts, persistence, permissions,
  provider/tool execution, lane side effects, and workflow reduction.
- Keep Core independent of `apps/tui` and `apps/gui`.
- Do not implement frontend layout or frontend-local interaction state here.
- After contract freeze, make backward-compatible extensions whenever
  possible. Breaking changes require a schema version, migration, fixtures,
  and an explicit cross-track notice.
- All mutation still passes through permission checks before effects occur.
- Preserve append-only JSONL facts and rebuildable SQLite indexes.
- Plan mode must reject mutation before file, shell, Git, workflow, memory, or
  task state changes.

## Development And Verification

- Use TDD for behavior changes.
- Add concise comments for protocol boundaries and non-obvious invariants.
- Update English and Chinese user-facing documentation together.
- Run focused crate tests while developing, then run:

```bash
cargo test -p viden-types
cargo test -p viden-session
cargo test -p viden-workflows
cargo test -p viden-runtime
cargo test -p viden-core
scripts/check-dependency-boundaries.sh
cargo test --workspace --quiet
```

The handoff must include the branch, worktree, HEAD, contract/schema changes,
migrations, fixture coverage, test evidence, frontend-consumable interfaces,
open contract requests, and blockers. Do not merge or push `main` without
explicit user direction.
