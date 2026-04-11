# V2 Session and Command Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand RoboCode's local CLI with richer session metadata, new runtime inspection commands, and a more informative command surface while preserving the current shared engine and transcript model.

**Architecture:** This slice extends existing V1 crates instead of creating new platform subsystems. The implementation should propagate startup/config state into `SessionEngine`, enrich `SessionSummary` and SQLite-backed session indexing, and add `/status`, `/config`, and `/doctor` commands plus better `/sessions` output without bypassing the existing command/transcript path.

**Tech Stack:** Rust, existing RoboCode workspace crates, SQLite fallback indexing, REPL command handling

---

## Scope

In scope:

- expose runtime status and loaded config from inside the CLI
- enrich session summaries and session-list rendering
- add lightweight environment diagnostics
- improve help text so the command surface reads like a product, not a demo

Out of scope:

- LSP
- MCP
- new crates for remote, agent, or plugin workflows
- full TUI rewrite

## Target Behaviors

- `/status` shows session id, cwd, provider, model, permission mode, transcript path, and session-home/index location.
- `/config` shows the resolved runtime config summary and the config files that contributed to it.
- `/doctor` runs a lightweight diagnostic pass for expected local dependencies and reports missing pieces clearly.
- `/sessions` shows more useful summary metadata than only title and preview.
- session indexing stores enough metadata to support richer browsing without reopening transcript files for every list render.
- all new commands write transcript command entries just like existing slash commands.

## File Map

**Modify:**

- `robocode-cli/src/main.rs`
- `robocode-core/src/lib.rs`
- `robocode-session/src/lib.rs`
- `robocode-types/src/lib.rs`
- `robocode-config/src/lib.rs`
- `README.md`

**Create:**

- `docs/superpowers/plans/2026-04-11-v2-session-command-enhancement.md`

## Task 1: Add runtime startup snapshot types

**Files:**
- Modify: `robocode-types/src/lib.rs`
- Modify: `robocode-config/src/lib.rs`
- Modify: `robocode-cli/src/main.rs`
- Modify: `robocode-core/src/lib.rs`

- [ ] Add a shared runtime snapshot type in `robocode-types` for the fields `SessionEngine` needs to render `/status` and `/config`.
- [ ] Include at minimum:
  - cwd
  - provider family
  - model label
  - permission mode
  - resolved config summary string
  - loaded config file list
  - session home override or effective home path
- [ ] Thread this snapshot from CLI startup into `SessionEngine` construction instead of rebuilding those details ad hoc inside `robocode-core`.
- [ ] Keep the existing startup banner behavior unchanged except for any new summary fields needed by tests.
- [ ] Add unit coverage in existing test modules to verify `SessionEngine` can render the stored startup snapshot even when the provider later changes model labels.

## Task 2: Enrich session summary metadata

**Files:**
- Modify: `robocode-types/src/lib.rs`
- Modify: `robocode-session/src/lib.rs`

- [ ] Extend `SessionSummary` with additional derived metadata:
  - message count
  - tool-call count
  - command count
  - last activity kind
  - last activity preview
- [ ] Update transcript summarization logic so these fields are derived from transcript entries in one pass.
- [ ] Update the SQLite schema and upsert path to store the new summary fields.
- [ ] Preserve backward compatibility by:
  - tolerating an older SQLite table layout
  - falling back to project-directory transcript scanning when the index is missing or stale
- [ ] Add tests covering:
  - JSONL-only fallback
  - SQLite index update with new fields
  - mixed sessions with messages, commands, and tool results

## Task 3: Improve `/sessions` and `/resume` ergonomics

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `robocode-session/src/lib.rs`

- [ ] Update session-list rendering to show richer summary rows without becoming noisy.
- [ ] Keep support for:
  - `/resume latest`
  - `/resume #<index>`
  - `/resume <session-id-prefix>`
- [ ] Make `/sessions` clearly mark the current session and include the last activity kind.
- [ ] Ensure ambiguous prefix matches still fail with a helpful list view.
- [ ] Add tests for:
  - current-session marker rendering
  - rich list formatting
  - ambiguous prefix error messaging

## Task 4: Add `/status` and `/config`

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `robocode-cli/src/main.rs`
- Modify: `robocode-types/src/lib.rs`

- [ ] Add `/status` as a read-only command rendered entirely from engine state.
- [ ] Add `/config` as a read-only command rendered from the startup snapshot and resolved config summary.
- [ ] Include these fields in `/status`:
  - session id
  - cwd
  - provider family
  - model
  - permission mode
  - transcript path
  - session home
- [ ] Include these fields in `/config`:
  - config summary string
  - loaded config files or `<none>`
  - startup overrides that were explicitly applied
- [ ] Add command tests that verify both commands appear in `/help` and are written to transcript command logs.

## Task 5: Add lightweight `/doctor`

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `robocode-cli/src/main.rs`

- [ ] Add a lightweight `/doctor` command that reports availability of:
  - `git`
  - `rg`
  - `sqlite3`
  - `curl`
- [ ] Report each dependency as `ok`, `missing`, or `not required for current path` only when justified.
- [ ] Avoid destructive or networked checks.
- [ ] Keep output simple and terminal-friendly.
- [ ] Add tests that inject command-availability shims or helper functions so diagnostics can be validated deterministically.

## Task 6: Refresh help and docs

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `README.md`

- [ ] Update `/help` output to include `/status`, `/config`, and `/doctor`.
- [ ] Reorganize help text so commands appear grouped by purpose rather than as a flat mixed list.
- [ ] Update `README.md` command examples to include the new runtime-inspection commands.

## Task 7: Verification and finish

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `robocode-session/src/lib.rs`
- Modify: `README.md`

- [ ] Run focused tests while implementing:
  - `cargo test -p robocode-session`
  - `cargo test -p robocode-core`
- [ ] Run final full verification:
  - `cargo test --workspace`
- [ ] Manual smoke-check in the CLI:
  - `cargo run -p robocode-cli -- --provider fallback --model test-local`
  - `/status`
  - `/config`
  - `/doctor`
  - `/sessions`
  - `/resume latest`
- [ ] Update docs only after command output is finalized.

## Acceptance Criteria

- the CLI exposes `/status`, `/config`, and `/doctor`
- the session list shows richer metadata than the current V1 output
- session indexing remains rebuildable from transcript files
- no new command bypasses transcript logging
- all current and new tests pass

## Follow-On Work

After this plan lands, the next detailed plan should be one of:

- LSP foundation
- memory and task workflows
- richer TUI and structured diff views

