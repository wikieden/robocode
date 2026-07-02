# RoboCode Agent Guide

## Mission

RoboCode is a Rust-first, local-first agentic developer CLI inspired by
`.ref/claude-code-main`. Treat the reference project as a behavioral guide, not
as a file-by-file port. Preserve user-facing runtime patterns where valuable,
but keep the implementation Rust-native and simpler than the reference when the
extra platform machinery is not yet needed.

## Current Architecture

- `robocode-cli`: binary entrypoint, cockpit TUI, and line REPL.
- `robocode-core`: session engine, slash commands, provider/tool loop, workflow command routing.
- `robocode-model`: provider abstraction for Anthropic, OpenAI, OpenAI-compatible, Ollama, and fallback flows.
- `robocode-provider-sdk` / `robocode-provider-deepseek`: dynamic provider plugin ABI and the DeepSeek plugin.
- `robocode-tools`: local shell, file, search, web, and Git tool implementations.
- `robocode-permissions`: permission modes, path scope checks, and allow/ask/deny decisions.
- `robocode-session`: JSONL transcript storage and rebuildable SQLite session index.
- `robocode-lsp`: read-only semantic code intelligence (diagnostics, symbols, references).
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
- Treat GitHub Release and Homebrew tap sync as one release unit:
  - every GitHub Release must update `wikieden/homebrew-tap` to the same version;
  - release completion requires post-publish smoke with both GitHub assets and
    Homebrew validation;
  - do not report a release as complete while the Homebrew tap is stale or
    unverified.
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

At time of writing, `main` sits at the completed `0.1.30` final zero-bug TUI
gate, and Viden is the adopted product direction (see
`docs/viden-design-adoption.md`). Active development is the `0.2.0`
architecture cut on `codex/viden-core-runtime`: the `viden-core` facade,
runtime supervisor, runtime contract freeze fixtures, and the command/event
bridge, following `docs/parallel-development-plan.md`.

If this branch has already merged, treat `PLAN.md`,
`docs/parallel-development-plan.md`, and `docs/staged-roadmap.md` as the
current roadmap source.
