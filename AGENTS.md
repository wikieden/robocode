# Viden Agent Guide

## How To Use This Guide

This is the canonical repository-wide instruction file for coding agents and
automation. Tool-specific entry files such as root `CLAUDE.md` must point here
instead of maintaining a competing copy of project policy.

Instruction precedence is:

1. the user's current request;
2. this repository-root `AGENTS.md`;
3. the nearest nested `AGENTS.md` for files being changed;
4. durable architecture, design, and development documents linked below.

Read every applicable instruction file before editing. When a task crosses
Core, TUI, GUI, or the design package, split ownership or explicitly reconcile
the nested rules before writing. Never assume a historical branch description
or chat summary is current; verify Git and the referenced source-of-truth files.

Key sources:

- roadmap and release sequencing: `PLAN.md` and `docs/staged-roadmap.md`;
- V3 branch topology: `docs/parallel-development-plan.md`;
- architecture and module boundaries: `docs/architecture.md` and
  `docs/modules.md`;
- coding, documentation, and comment standard:
  `docs/development-standards.md`;
- frontend contract: `docs/frontend-integration-contract.md`;
- visual adoption rules: `docs/viden-design-adoption.md` and the nested
  instructions under `docs/viden-design/Viden/`.

## Mission

Viden is a Rust-first, local-first agentic developer workspace inspired by
`.ref/claude-code-main`. Treat the reference project as a behavioral guide, not
as a file-by-file port. Preserve user-facing runtime patterns where valuable,
but keep the implementation Rust-native and simpler than the reference when the
extra platform machinery is not yet needed.

## Current Architecture

Workspace code is split by product surface and reusable core:

- `apps/cli`: binary entrypoint, flags, and bootstrap.
- `apps/tui`: terminal rendering, input orchestration, previews, and app-specific TUI state.
- `apps/gui`: Tauri desktop client, governed by its nested `AGENTS.md`. The
  `0.1.0-alpha.1` framework gate selected Tauri; see
  `docs/gui-framework-decision.md`.
- `crates/core`: stable runtime facade and shared contract re-exports.
- `crates/context`: native context selection, immutable content references,
  retrieval, compaction, quality, and cost accounting.
- `crates/runtime`: session engine, slash commands, provider/tool loop, workflow command routing.
- `crates/lanes`: lane lifecycle orchestration and lane-local side effects,
  below the runtime; runtime policy such as the permission gate and event
  redaction is injected into it, never imported by it.
- `crates/provider`: provider abstraction, registry, and protocol adapters.
- `crates/plugin-api`: shared plugin manifest, capability, permission, and provider descriptor contracts.
- `crates/plugin-host`: static plugin registry boundary for provider/tool/agent/workflow plugins.
- `crates/tools`: local shell, file, search, web, and Git tool implementations.
- `crates/permissions`: permission modes, path scope checks, and allow/ask/deny decisions.
- `crates/session`: JSONL transcript storage and rebuildable SQLite session index.
- `crates/types`: shared domain types for messages, tools, permissions, sessions, runtime snapshots, tasks, and memory.
- `crates/config`: layered config resolution.
- `crates/workflows`: project tasks, project/session memory, resume context, and workflow event storage.
- `crates/lsp`: read-only semantic diagnostics, symbols, references, and
  document synchronization.
- `plugins/providers/deepseek`: DeepSeek provider plugin.

## Non-Negotiable Invariants

- All model tool calls and local command effects must flow through the shared runtime path.
- Permission checks happen before mutation, not after.
- Transcript history remains auditable and append-only for session facts.
- JSONL stays canonical for durable logs; SQLite is a derived, rebuildable index.
- Session state and workflow state are related but separate:
  - `viden-session` records what happened in a session.
  - `viden-workflows` records durable project task and memory state.
- Project memory suggested by an assistant must not become active without explicit confirmation.
- Plan mode must block mutating workflow, file, shell, Git, and memory/task changes.
- Core is the only authority for runtime facts and side effects. Frontends may
  own presentation state but must not create parallel business reducers.
- Frontends must recover missing or out-of-order state through the versioned
  snapshot/replay contract, never by guessing from display text.

## Standard Change Workflow

Before editing:

1. Read the applicable root and nested `AGENTS.md` files.
2. Inspect `git status`, active worktrees, and the actual branch base. Fetch the
   remote when branch freshness affects the task.
3. Identify the owning product track and write scope. Do not start if the same
   files are owned by another active task without a serialization decision.
4. Locate the current contract, design, test, and documentation sources before
   adding a new abstraction or surface.

While editing:

1. Keep the change focused and reversible.
2. For behavior changes, use TDD and verify the initial failure is relevant.
3. Preserve runtime, permission, persistence, and frontend dependency
   boundaries.
4. Update affected English/Chinese docs and concise invariant comments in the
   same change set.
5. Run the smallest useful check after each meaningful increment.

Before handoff:

1. Review the complete diff, including untracked files and generated assets.
2. Run `git diff --check`, relevant focused tests, and the broader gate required
   by the verification matrix below.
3. Confirm whether docs, comments, fixtures, migrations, screenshots, and
   release evidence were required and handled.
4. Report exact evidence and anything not run. A branch is not complete because
   code compiles or a single happy-path test passes.

## Working Rules

- Use an isolated git worktree for feature work. Preferred location: `.worktrees/<branch-name>`.
- Preserve dirty user changes. Do not revert or overwrite work you did not create.
- Treat existing and untracked changes as user-owned unless provenance is
  established. Never use destructive cleanup to make a worktree look clean.
- Use focused commits. Each commit should describe one coherent checkpoint.
- Use TDD for behavior changes:
  - write a failing test,
  - verify it fails for the expected reason,
  - implement the smallest passing change,
  - rerun focused tests.
- Keep docs bilingual when editing user-facing documentation:
  - update English and `*.zh-CN.md` counterparts together.
- Treat documentation and code comments as part of the implementation and as a
  required coding standard:
  - update relevant docs whenever behavior, commands, architecture, configuration, or user-visible UI changes;
  - add concise comments for non-obvious control flow, invariants, protocol boundaries, or safety rules;
  - avoid noisy comments that merely restate obvious code.
- Before finishing any code change, explicitly check whether the diff needs
  documentation updates or explanatory comments, and include that decision in
  verification notes when relevant.
- Follow `docs/development-standards.md` for the project coding standard,
  especially the documentation and code-comment requirements.
- Keep root docs compact. Put full product detail under `docs/`.
- Do not edit `.ref/`; it is reference material only.
- Keep `.omx/`, `.viden/`, `.worktrees/`, `.ref/`, and build artifacts out of tracked source.

## Documentation And Design Rules

- User-visible documentation is bilingual. Update the English and
  `*.zh-CN.md` counterpart together.
- Root documents stay concise; detailed designs, plans, investigations, and
  release evidence belong under `docs/`.
- Documentation describes verified current behavior. Clearly label proposals,
  prototypes, partial implementations, and future gates.
- The accepted visual source is `docs/viden-design/Viden/`. Its nested
  `AGENTS.md`, `CLAUDE.md`, `tokens.css`, `docs/DESIGN-REF.md`,
  `docs/SPEC.md`, and `docs/screens-status.js` define local governance.
- Do not treat archived pages, deleted imports, generated previews, mock data,
  Babel prototype scaffolding, or `.ref/` content as production truth.
- Shared tokens and registered components are reused, not copied into frontend
  forks. A visual behavior change must update the appropriate design status,
  changelog, guard baseline, and review evidence when required by the design
  package rules.

## Testing

Choose verification by change scope. Use focused checks while developing:

```bash
cargo test -p viden-types
cargo test -p viden-session
cargo test -p viden-workflows
cargo test -p viden-lanes
cargo test -p viden-runtime
```

Additional required gates:

- Core/shared contract changes: focused affected crates,
  `scripts/check-dependency-boundaries.sh`, then the workspace suite.
- TUI behavior: `cargo test -p viden-tui`,
  `scripts/tui-turn-controller-smoke.sh`, `scripts/rc-tui-stability-smoke.sh`,
  and `scripts/tui-regression.sh` as applicable.
- TUI visuals: regenerate deterministic evidence with
  `scripts/tui-previews.sh` and review the output.
- Context/evidence/cost changes: run the relevant context benchmark contract
  smoke and preserve canonical evidence parity.
- Docs-only changes: run the document pair/link checks with explicit changed
  paths plus `git diff --check`.
- Release-facing changes: use the release gate/smoke scripts and live-provider
  evidence required by the release plan.

Before calling a shared or implementation branch complete, run:

```bash
cargo test --workspace --quiet
```

For CLI-facing behavior, add a fallback-provider smoke test when practical:

```bash
cargo run -p viden-cli -- --provider fallback --model test-local
```

Do not run live provider, publish, release, or Homebrew mutation steps unless
the task explicitly authorizes them. State skipped gates in the handoff.

## Commit And Handoff Standard

- Commit one coherent checkpoint at a time with an imperative message that
  explains the delivered behavior or contract.
- Do not stage unrelated user changes or generated artifacts that are outside
  the task's evidence requirements.
- A handoff must include:
  - branch, worktree, and HEAD;
  - changed files/modules and ownership scope;
  - behavior and contract impact;
  - migrations, fixtures, docs/comments, and visual evidence when relevant;
  - exact verification commands and outcomes;
  - skipped checks with the reason;
  - blockers, contract requests, and the next safe step.
- Distinguish committed, merely present in a worktree, pushed, merged, and
  released states. Never describe one as another.

## Release Discipline

- Treat GitHub Release and `wikieden/homebrew-tap` as one release unit at the
  same version.
- Release completion requires GitHub assets, Homebrew update, and post-publish
  smoke evidence.
- Do not report a release complete while the tap is stale, assets are missing,
  or required live/packaging evidence is unverified.
- Publishing, tagging, pushing, and Homebrew changes require explicit user
  authorization.

## Reference Project Guidance

Standing reference architectures (user decision, 2026-08-17): consult
`openai/codex` and `deepseek-ai/deepseek-harness` before proposing or deciding
any requirement or architecture change.

- `openai/codex` (Rust agent CLI) is architecturally isomorphic to Viden:
  core-owned state, thin frontends, JSONL facts with a derived index. Check
  contract fixtures, protocol evolution discipline, the sandbox/permission
  decision-versus-enforcement split, the app-server daemon surface, headless
  contract clients, and swappable compaction strategies against it. Local
  deep-read notes, when present, live under `proposals/`.
- `deepseek-ai/deepseek-harness` (Node.js, "everything is a plugin", built on
  Cordis) is the reference for plugin-first composition, delegating subagent
  work to external CLIs, and a web operator surface. It is a developer preview
  with breaking changes; cite the exact commit consulted and never vendor its
  code.

A requirement decision that diverges from both references must record the
reason in the controlling plan or design document.

Useful `.ref/claude-code-main` patterns:

- `main.tsx`: startup and runtime orchestration.
- `commands.ts`: broad slash-command surface and command family structure.
- `Tool.ts`: tool contracts and shared execution semantics.
- `types/permissions.ts`: permission modes and policy shape.
- `tasks/*`: task/session workflow ideas.
- `bridge/*`, `plugins/*`, `context/*`, `keybindings/*`: future platform expansion references.

Do not copy:

- Bun, React, or Ink implementation details.
- Product analytics and managed settings before core workflows mature.
- Remote/bridge/MCP/multi-agent complexity before the local CLI model is stable.

## V3 Parallel Development Coordination

The controlling plan for Core, TUI, and GUI parallel work is
`docs/parallel-development-plan.md` and its Chinese counterpart. Treat the
following as execution rules for that plan:

| Track | Nested instructions | Exclusive implementation scope |
| --- | --- | --- |
| Core | `crates/AGENTS.md` | `crates/**` and shared runtime contracts |
| TUI | `apps/tui/AGENTS.md` | `apps/tui/**` and TUI-specific evidence |
| GUI | `apps/gui/AGENTS.md` | `apps/gui/**` and GUI-specific evidence |

- Use at most three concurrent implementation owners: Core, TUI, and GUI. A
  read-only coordination task does not own an implementation scope.
- Track versions independently: Core uses `core-v0.3.x`, TUI uses
  `tui-v0.3.x`, and GUI uses `gui-v0.1.x` until the plan is revised. Reports
  must name the workspace candidate plus the Core, TUI, and GUI versions when
  integration is discussed.
- Start `codex/v3-core-runtime` from synchronized `origin/main`. Core must
  publish an immutable `frontend-contract-v1` checkpoint before production
  TUI or GUI implementation begins.
- Start `codex/v3-tui-client` and `codex/v3-gui-client` from the exact Core
  checkpoint, not from an older UI branch or an unverified local checkout.
- Keep each implementation branch in its own `.worktrees/<branch-name>`
  worktree. Overlapping write scopes must be serialized.
- Core owns authoritative state and side effects. TUI and GUI may maintain
  local presentation state only and must use the shared command, event,
  snapshot, and replay contracts.
- A missing frontend capability is a Core contract request. Do not bypass it
  with a frontend-private reducer, direct runtime access, or inferred success.
- Language, locale, skin, mode, density, font scale, and accessibility settings
  are shared presentation preferences owned by Core and consumed by frontends.
  Frontends may own local rendering, but not independent preference persistence
  or private skin palettes.
- Review design from `docs/viden-design/Viden/index.html` first. For TUI,
  continue through the TUI design index, unified prototype, and component
  library. For GUI, continue through the GUI design index, desktop cockpit,
  and component library.
- Integrate in the fixed order Core -> TUI -> GUI. Run parity fixtures and the
  relevant branch gate after each step.
- Every handoff must report the branch, worktree, HEAD, changed ownership
  scope, tests, contract requests, blockers, and next safe parallel work.
- Do not merge or push `main` unless the user explicitly asks for that action.

## Current Branch Context

The current planning line is V3 multi-frontend development. Treat `PLAN.md`,
`docs/parallel-development-plan.md`, and
`docs/parallel-development-plan.zh-CN.md` as the roadmap and branch-topology
sources. Historical branch descriptions in older documents are not authority
for current branch creation.
