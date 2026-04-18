# RoboCode Agent Guide

## Mission

RoboCode is a Rust-first, local-first agentic developer CLI inspired by
`.ref/claude-code-main`. Treat the reference project as a behavioral guide, not
as a file-by-file port. Preserve user-facing runtime patterns where valuable,
but keep the implementation Rust-native and simpler than the reference when the
extra platform machinery is not yet needed.

## Current Architecture

- `robocode-cli`: binary entrypoint and lightweight REPL.
- `robocode-core`: session engine, slash commands, provider/tool loop, workflow command routing.
- `robocode-model`: provider abstraction for Anthropic, OpenAI, OpenAI-compatible, Ollama, and fallback flows.
- `robocode-tools`: local shell, file, search, web, and Git tool implementations.
- `robocode-permissions`: permission modes, path scope checks, and allow/ask/deny decisions.
- `robocode-session`: JSONL transcript storage and rebuildable SQLite session index.
- `robocode-types`: shared domain types for messages, tools, permissions, sessions, runtime snapshots, tasks, and memory.
- `robocode-config`: layered config resolution.
- `robocode-workflows`: project tasks, project/session memory, resume context, and workflow event storage.

## Non-Negotiable Invariants

- All model tool calls and local command effects must flow through the shared runtime path.
- Permission checks happen before mutation, not after.
- Transcript history remains auditable and append-only for session facts.
- JSONL stays canonical for durable logs; SQLite is a derived, rebuildable index.
- Session state and workflow state are related but separate:
  - `robocode-session` records what happened in a session.
  - `robocode-workflows` records durable project task and memory state.
- Project memory suggested by an assistant must not become active without explicit confirmation.
- Plan mode must block mutating workflow, file, shell, Git, and memory/task changes.

## Working Rules

- Use an isolated git worktree for feature work. Preferred location: `.worktrees/<branch-name>`.
- Preserve dirty user changes. Do not revert or overwrite work you did not create.
- Use focused commits. Each commit should describe one coherent checkpoint.
- Use TDD for behavior changes:
  - write a failing test,
  - verify it fails for the expected reason,
  - implement the smallest passing change,
  - rerun focused tests.
- Keep docs bilingual when editing user-facing documentation:
  - update English and `*.zh-CN.md` counterparts together.
- Keep root docs compact. Put full product detail under `docs/`.
- Do not edit `.ref/`; it is reference material only.
- Keep `.omx/`, `.robocode/`, `.worktrees/`, `.ref/`, and build artifacts out of tracked source.

## Testing

Use focused checks while developing:

```bash
cargo test -p robocode-types
cargo test -p robocode-session
cargo test -p robocode-workflows
cargo test -p robocode-core
```

Before calling a branch complete, run:

```bash
cargo test --workspace --quiet
```

For CLI-facing behavior, add a fallback-provider smoke test when practical:

```bash
cargo run -p robocode-cli -- --provider fallback --model test-local
```

## Reference Project Guidance

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

## Current Branch Context

At time of writing, active development is `V2-C Memory and Task Workflows` on
`codex/v2-memory-task-workflows`. This branch adds `robocode-workflows`, task
and memory shared types, project workflow event logs, task/memory reducers,
resume context derivation, and initial `/task` / `/memory` command integration.

If this branch has already merged, treat `PLAN.md` and `docs/staged-roadmap.md`
as the current roadmap source.
