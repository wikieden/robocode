# Viden Claude Code Entry

This file is the Claude Code entry point for the Viden repository. The
repository-root `AGENTS.md` is the canonical cross-tool policy. Read it in full
before taking action; this file adds Claude-specific routing and command
shortcuts without redefining project policy.

Do not confuse this file with
`docs/viden-design/Viden/CLAUDE.md`. The latter governs only the accepted
design package and must be read when work touches that subtree.

## Mandatory Read Order

1. Read root `AGENTS.md`.
2. Read the nearest nested `AGENTS.md` for every file in the intended write
   scope.
3. Read the controlling plan, contract, design, or release-status document for
   the task.
4. Inspect the actual Git branch, worktree, dirty state, and remote freshness
   before relying on a branch name or previous handoff.

Important nested scopes:

| Scope | Instructions |
| --- | --- |
| Core crates | `crates/AGENTS.md` |
| TUI client | `apps/tui/AGENTS.md` |
| GUI client | `apps/gui/AGENTS.md` |
| Design package | `docs/viden-design/Viden/AGENTS.md` and its local `CLAUDE.md` |

When instructions conflict, follow the user's current request, then root
`AGENTS.md`, then the nearest nested instructions. Stop and surface a genuine
scope conflict instead of silently choosing a wider mutation.

## Repository Summary

Viden is a Rust-first, local-first AI coding workspace with a shared runtime
and multiple clients:

- `apps/cli`: binary entry and bootstrap;
- `apps/tui`: terminal client;
- `apps/gui`: planned desktop client boundary;
- `crates/core`: stable multi-frontend facade;
- `crates/runtime`: session and agent execution orchestration;
- `crates/lanes`: lane lifecycle orchestration and lane-local side effects,
  below the runtime;
- `crates/context`: context, evidence-reference, retrieval, and cost engine;
- `crates/types`: shared domain and protocol types;
- `crates/session`: canonical JSONL session facts and rebuildable SQLite index;
- `crates/workflows`: durable project tasks and memory;
- `crates/provider`, `crates/tools`, and `crates/permissions`: execution
  adapters and mutation policy boundaries;
- `crates/plugin-*` and `plugins/**`: extension contracts and first-party
  plugins.

Core owns authoritative business state and effects. TUI and GUI are clients of
the same command/event/snapshot/replay contract. A frontend must not talk
directly to provider, tool, permission, session, workflow, JSONL, or SQLite
internals.

## Startup Checklist

Before implementation:

```bash
pwd
git status --short --branch
git worktree list --porcelain
git log -1 --oneline --decorate
```

If the task depends on the latest mainline, fetch and compare before creating a
branch. Preserve dirty and untracked user work. Use an isolated worktree at
`.worktrees/<branch-name>` for feature work.

Classify the request before acting:

- review, diagnosis, or status: inspect and report; do not implement unless
  asked;
- design or planning: update durable plans/specs when requested, but do not
  imply code is implemented;
- implementation: use TDD, update docs/comments with the behavior, verify, and
  create a focused checkpoint;
- release or push: perform external mutation only when explicitly authorized.

## Common Commands

Build and format:

```bash
cargo build --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
```

Focused and full tests:

```bash
cargo test -p viden-types
cargo test -p viden-session
cargo test -p viden-workflows
cargo test -p viden-lanes
cargo test -p viden-runtime
cargo test -p viden-core
cargo test -p viden-tui
cargo test --workspace --quiet
```

Offline runtime smoke:

```bash
cargo run -p viden-cli -- --provider fallback --model test-local
cargo run -p viden-cli -- --no-tui --provider fallback --model test-local
```

Dependency, TUI, and document gates:

```bash
scripts/check-dependency-boundaries.sh
scripts/tui-turn-controller-smoke.sh
scripts/rc-tui-stability-smoke.sh
scripts/tui-regression.sh
scripts/tui-previews.sh
scripts/check-doc-pairs.sh <changed-markdown-path> [...]
scripts/check-doc-links.sh <changed-markdown-path> [...]
git diff --check
```

Run the smallest relevant check first, then broaden according to root
`AGENTS.md`. Do not run live-provider, publish, release, or Homebrew mutations
without explicit authorization.

## V3 Core, TUI, And GUI Routing

The controlling files are `PLAN.md` and
`docs/parallel-development-plan.md`.

```text
origin/main
  -> codex/v3-core-runtime
       -> immutable frontend-contract-v1 checkpoint
            -> codex/v3-tui-client
            -> codex/v3-gui-client
```

Rules:

- At most three implementation owners run concurrently: Core, TUI, and GUI.
- Core, TUI, and GUI have independent version lines: `core-v0.3.x`,
  `tui-v0.3.x`, and `gui-v0.1.x`. Integration reports also name the aggregate
  workspace candidate when relevant.
- Core starts first and publishes the exact checkpoint SHA.
- TUI and GUI start from that SHA. Without it, they remain in design,
  interaction/reference confirmation, framework-spike planning, or contract-gap
  analysis.
- Overlapping write scopes serialize. Do not let multiple tasks repair the same
  shared manifest, contract, fixture, token, or design decision independently.
- Missing client data or actions become Core contract requests.
- Language, locale, skin, mode, density, font scale, terminal color capability,
  and accessibility settings flow through a shared Core-owned presentation
  preference contract. Frontends must not persist a second preference model or
  ship private palettes.
- Integrate Core -> TUI -> GUI and rerun parity evidence after each step.
- Do not merge or push `main` unless the user explicitly requests it.

## Design And Visual Work

The accepted visual source is `docs/viden-design/Viden/`. Before editing it,
read its local `AGENTS.md`, local `CLAUDE.md`, `docs/SPEC.md`, and
`docs/DESIGN-REF.md` as required by that package.

- `tokens.css` is the token source of truth.
- Registered components and icons are reused rather than copied.
- Start visual review from `docs/viden-design/Viden/index.html`. For TUI, open
  the TUI design index, unified prototype, and component library. For GUI, open
  the GUI design index, desktop cockpit, and component library before using
  lower-level pages.
- TUI uses the canonical TUI kit and registered glyphs; do not add emoji.
- GUI remains framework-neutral until the Tauri/GPUI vertical-slice gate is
  decided.
- Archived pages, mock data, generated previews, prototype runtime scaffolding,
  and `.ref/` are not production truth.
- Visual changes require the design-package guards, status/changelog updates,
  and screenshot evidence specified by the nested rules.

## Coding And Documentation Discipline

- All model tool calls and local effects flow through the shared runtime.
- Permission checks occur before mutation.
- Plan mode rejects all mutation paths before effects.
- JSONL facts stay append-only and canonical; SQLite remains derived.
- Assistant-suggested project memory requires explicit user confirmation.
- Use TDD for behavior changes and verify the failing test fails for the
  intended reason.
- Explain protocol, persistence, permission, rendering, and concurrency
  invariants with concise comments where names and types are insufficient.
- Update affected English and `*.zh-CN.md` documentation together.
- Describe only verified behavior as implemented; label proposals and partial
  work honestly.
- Never edit `.ref/` or commit `.omx/`, `.viden/`, `.worktrees/`, `.ref/`, or
  build artifacts.

## Handoff Format

End implementation work with:

1. outcome and user-visible or contract impact;
2. branch, worktree, and exact HEAD;
3. changed modules and ownership scope;
4. tests, smoke checks, screenshots, fixtures, migrations, and docs/comments;
5. checks not run and why;
6. blockers and Core contract requests;
7. the next safe action.

State separately whether work is only present, committed, pushed, merged, or
released. Do not call a branch complete while required evidence is missing.
