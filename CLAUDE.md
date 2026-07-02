# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Viden is a Rust-first, local-first AI coding agent orchestration workspace with CLI, TUI, and future GUI app surfaces. It is inspired by `.ref/claude-code-main` — treat that directory as read-only behavioral reference, never edit it and never port it file-by-file. `AGENTS.md` is the canonical agent guide; this file summarizes and extends it.

## Commands

```bash
cargo build                                          # build workspace
cargo test -p <crate>                                # focused tests while developing (e.g. -p viden-runtime)
cargo test -p viden-runtime <test_name>              # single test
cargo test --workspace --quiet                       # required before calling a branch complete
cargo clippy                                         # lint
cargo fmt                                            # format edited Rust code before handoff
```

Run from source:

```bash
cargo run -p viden-cli                            # cockpit TUI (DeepSeek default; needs DEEPSEEK_API_KEY for live turns)
cargo run -p viden-cli -- --provider fallback --model test-local    # offline smoke session
cargo run -p viden-cli -- --no-tui --provider fallback --model test-local   # legacy line REPL
```

Smoke / release scripts (under `scripts/`): `tui-regression.sh`, `tui-previews.sh` (deterministic TUI screenshots), `release-gate.sh`, `release-smoke.sh`, `provider-live-smoke.sh`, plus focused contract smokes (`plan-mode-smoke.sh`, `tdd-testing-contract-smoke.sh`, etc.).

## Architecture

Cargo workspace; app surfaces live under `apps/`, reusable core crates live under `crates/`, and first-party plugin implementations live under `plugins/`:

- `apps/cli` — binary entrypoint, flags, bootstrap, current CLI/TUI launcher.
- `apps/tui` — terminal app boundary; future home for the full TUI render/input loop.
- `crates/core` — stable runtime facade and contract re-exports for all frontends.
- `crates/runtime` — session engine, turn orchestration, slash-command routing, provider/tool loop, shared presentation helpers.
- `crates/provider` — provider host/registry and protocol adapters (Anthropic `tool_use` vs OpenAI `tool_calls` translation), built-in providers (Anthropic, OpenAI, OpenAI-compatible, Ollama, fallback).
- `crates/plugin-api` — plugin manifest, capability, permission, provider descriptor, and ABI contracts.
- `crates/plugin-host` — shared static plugin registry boundary for providers, tools, agents, context sources, and workflows.
- `plugins/providers/deepseek` — DeepSeek provider plugin.
- `crates/tools` — local file/search/shell/web/Git/LSP/test tool implementations.
- `crates/permissions` — permission modes, path scope checks, allow/ask/deny decisions.
- `crates/session` — JSONL transcripts (canonical, append-only) + SQLite index (derived, rebuildable), resume.
- `crates/workflows` — durable project tasks, project/session memory, resume context, workflow event logs.
- `crates/lsp` — read-only semantic code intelligence (diagnostics, symbols, references).
- `crates/types` — shared domain contracts used by everything.
- `crates/config` — layered config resolution.

Data ownership split that matters: `viden-session` records *what happened in a session*; `viden-workflows` records *durable project task/memory state*. They are related but separate.

Deeper docs: `docs/architecture.md`, `docs/modules.md`, `docs/development-standards.md`, roadmap in `PLAN.md` / `docs/staged-roadmap.md`.

## Non-Negotiable Invariants

- All model tool calls and local command effects flow through the shared runtime path.
- Permission checks happen **before** mutation, not after.
- Transcripts are auditable and append-only; JSONL is canonical, SQLite is derived/rebuildable.
- Plan mode must block mutating workflow, file, shell, Git, and memory/task changes.
- Assistant-suggested project memory must not become active without explicit user confirmation.

## Working Rules

- Use isolated git worktrees for feature work, preferred at `.worktrees/<branch-name>`. Never commit `.omx/`, `.robocode/`, `.worktrees/`, `.ref/`, or build artifacts.
- TDD for behavior changes: failing test → verify it fails for the right reason → smallest passing change → rerun focused tests.
- Docs and comments are part of the delivery, not cleanup: update affected docs in the same change set, and keep user-facing docs bilingual (update the matching `*.zh-CN.md` together, or call out the gap).
- Document only implemented behavior — no placeholders or future plans presented as features.
- Comments explain invariants, safety/permission boundaries, protocol/persistence contracts, and non-obvious rendering/concurrency rules — not what the next line does.
- Interactive TUI features default to the selector-first model (actionable picker, search, keyboard + mouse, current value visible); information-only pages are reserved for diagnostics like `/status`, `/config`, `/provider doctor`.
- Visual TUI changes require deterministic screenshot/preview regeneration (`scripts/tui-previews.sh`) for review before release.
- Releases are one unit: GitHub Release + `wikieden/homebrew-tap` bump to the same version + post-publish smoke. A release is not complete while the tap is stale.

## Verification Order

Smallest meaningful check first, then broaden: `cargo fmt` on edited code → focused crate tests → `cargo test --workspace --quiet` for shared/release-facing changes → TUI previews for visual changes → docs review. State honestly what was not tested.
