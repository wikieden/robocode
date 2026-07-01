# Viden Parallel Development Plan

Chinese version: [parallel-development-plan.zh-CN.md](parallel-development-plan.zh-CN.md)

## Purpose

This plan defines how Viden should enter the large Runtime-first refactor while
supporting future parallel development by up to three people or agents.

The core rule is:

> Refactor structure first. Start parallel TUI and GUI work only after the
> runtime contracts are stable enough to prevent duplicate business logic.

## Target Development Shape

Viden is moving toward a Runtime-first platform:

- `viden-core` is the public core facade for runtime, orchestration, context,
  permissions, evidence, cost, tasks, lanes, and extension contracts.
- TUI and GUI are product clients. They render state and send commands; they do
  not own provider loops, tool execution, permission decisions, or task state.
- Extensions run through a declared plugin boundary and cannot bypass the
  runtime permission/evidence path.
- RoboCode remains a legacy compatibility name during migration; the active
  product, documentation, UI, and new architecture direction are Viden.

## Phase Plan

### Phase 0: Architecture Cut

Freeze the intended workspace structure, dependency direction, public runtime
contracts, plugin protocol shape, and migration strategy before broad edits.

Deliverables:

- `viden-core` facade design
- UI model contract: `RuntimeSnapshot`, event stream, command actions, approval
  requests, evidence views, and UI contribution model
- process-plugin protocol draft
- Viden rename and RoboCode compatibility migration plan
- contract-test fixture plan for TUI and GUI

### Phase 1: Core Structure Refactor

Do structure and boundary work first. This phase should avoid large visual TUI
rewrites and should not start GUI implementation.

Deliverables:

- `viden-core` facade introduced or staged behind compatibility exports
- Runtime supervisor and event stream extracted from TUI-owned state
- command bus for user input, mode switching, approvals, provider/model setup,
  cancellation, queued follow-ups, and tool/lane actions
- permission checks centralized before mutation
- task, lane, evidence, cost, context, provider health, and transcript facts
  emitted by core runtime
- TUI converted incrementally to consume runtime facts instead of owning
  business state

### Phase 2: Contract Freeze

Freeze the first usable cross-frontend contract before concurrent UI work.

Required gate:

- core replay tests pass for runtime snapshots and events
- permission/mode contract tests cover plan/build/review behavior
- provider/model setup, approval, lane, task, cost, and evidence fixtures exist
- a thin TUI client can run from the shared contract without direct business
  calls
- GUI required APIs are documented and covered by schema or fixture tests

### Phase 3: Parallel TUI and GUI Development

After the contract freeze, split work into independent branches/worktrees.

Recommended branch ownership:

| Branch | Owner | Scope |
| --- | --- | --- |
| `codex/viden-core-runtime` | Core owner | Runtime contracts, plugin protocol, migration, bug fixes |
| `codex/viden-tui-client` | TUI owner | Terminal rendering, keyboard/input, panes, scrollback, status, errors |
| `codex/viden-gui-tauri-client` | GUI owner | Tauri + Web cockpit, settings, agent board, evidence, approval, provider/model |

Rules:

- TUI and GUI branches may not call provider, tool, permission, transcript, or
  workflow internals directly.
- Shared contract changes start in the core branch with tests, then UI branches
  rebase or merge them.
- TUI and GUI can differ in layout and interaction details, but they must show
  the same runtime facts for the same fixture.
- UI plugin contributions must be declarative. Plugins can contribute panels,
  settings, commands, and cards, but cannot mutate UI internals.

### Phase 4: Integration and Release

Merge order:

1. Core/runtime branch
2. TUI client branch
3. GUI client branch

Release gates:

- full workspace tests
- runtime replay and permission/mode tests
- plugin manifest/capability tests
- TUI/GUI parity fixture tests
- deterministic TUI previews and GUI screenshots
- real DeepSeek development smoke with token, cost, duration, and failure class
- Viden binary/config migration tests
- RoboCode compatibility shim tests
- GitHub Release and Homebrew tap validation as one release unit

## Concurrency Rules

- Use isolated worktrees under `.worktrees/<branch-name>`.
- At most three active owners should touch the architecture at once: core,
  TUI, and GUI.
- Large files should be split during Phase 1 before UI branches diverge.
- Shared contracts are changed by tests first, implementation second.
- UI branches must rebase or merge the core branch frequently; long-lived UI
  branches cannot invent private runtime state to keep moving.
- Documentation changes follow the owner of the changed behavior. User-facing
  docs need English and Chinese updates together.

## Version Mapping

- `0.2.0`: Architecture cut and core structure refactor.
- `0.2.1`: Context, token/cost, evidence, and runtime fact model.
- `0.2.2`: supervised multi-agent execution loop.
- `0.2.3`: plugin runtime, process-plugin protocol, and real development gate.
- `0.3.0`: contract freeze for multi-frontend work and Viden migration plan.
- `0.3.1`: parallel TUI and GUI implementation branches.
- `0.3.2`: integration release candidate with TUI/GUI parity.
- `0.3.3`: operable GUI beta and Viden compatibility migration hardening.
- `0.3.4`: visual fidelity and production release gate.
