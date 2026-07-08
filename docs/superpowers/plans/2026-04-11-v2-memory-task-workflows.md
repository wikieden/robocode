# V2 Memory and Task Workflows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add project-level tasks, two-tier memory, and workflow-oriented resume context to Viden without breaking the existing session, permission, and transcript invariants.

**Architecture:** This slice adds a new `viden-workflows` crate with internal `tasks`, `memory`, `resume_context`, and `stores` modules. `viden-session` remains the transcript source of truth, while workflow state lives in append-only workflow event logs plus a derived SQLite index. `viden-core` integrates workflow commands and routes all mutations through the existing command, permission, and transcript path.

**Tech Stack:** Rust workspace crates, JSONL append-only logs, SQLite derived indexes, existing Viden REPL/command runtime

---

## Scope

In scope:

- new `viden-workflows` crate
- project-level task lifecycle state
- project memory and session memory
- `/tasks`, `/task ...`, and `/memory ...` command families
- `/task resume-context`
- workflow event logs and derived SQLite index
- transcript-visible command behavior for all workflow commands

Out of scope:

- LSP
- MCP
- multi-agent ownership semantics
- automation/cron
- rich TUI work

## Target Behaviors

- Viden can create, update, block, link, archive, and restore project tasks.
- Viden can store session memory directly and project memory through suggest/confirm flow.
- `/task resume-context` shows active work, blockers, relevant memory, and suggested next steps.
- Workflow state can be rebuilt from append-only task and memory event logs.
- Workflow commands use the shared command path and continue writing transcript command entries.
- Workflow mutations flow through the existing permission model.

## File Map

**Create:**

- `viden-workflows/Cargo.toml`
- `viden-workflows/src/lib.rs`
- `viden-workflows/src/tasks.rs`
- `viden-workflows/src/memory.rs`
- `viden-workflows/src/resume_context.rs`
- `viden-workflows/src/stores.rs`
- `docs/superpowers/plans/2026-04-11-v2-memory-task-workflows.md`

**Modify:**

- `Cargo.toml`
- `viden-core/src/lib.rs`
- `viden-session/src/lib.rs`
- `viden-types/src/lib.rs`
- `viden-permissions/src/lib.rs`
- `README.md`
- `README.zh-CN.md`

## Task 1: Add workflow crate skeleton and shared types

**Files:**

- Create: `viden-workflows/Cargo.toml`
- Create: `viden-workflows/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `viden-types/src/lib.rs`

- [ ] Add `viden-workflows` to the workspace members in `Cargo.toml`.
- [ ] Create `viden-workflows/Cargo.toml` with dependencies on `viden-types`, `serde`, and any minimal persistence helpers already used in the workspace.
- [ ] Add empty module exports in `viden-workflows/src/lib.rs` for:
  - `tasks`
  - `memory`
  - `resume_context`
  - `stores`
- [ ] Define shared workflow-facing types in `viden-types/src/lib.rs` for:
  - `TaskId`
  - `MemoryId`
  - `TaskStatus`
  - `TaskPriority`
  - `MemoryScope`
  - `MemoryKind`
  - `MemorySource`
  - `MemoryStatus`
- [ ] Add data structs in `viden-types/src/lib.rs` for:
  - `TaskRecord`
  - `MemoryEntry`
  - `ResumeContextSnapshot`
- [ ] Keep initial type derivations compatible with transcript/session patterns already used in the repo:
  - `Debug`
  - `Clone`
  - `Serialize`
  - `Deserialize`
  - `PartialEq`
  - `Eq` where valid
- [ ] Add unit tests in `viden-types/src/lib.rs` for CLI/serde roundtrip of the new enums.
- [ ] Run: `cargo test -p viden-types`
- [ ] Commit with a focused message after the crate skeleton and shared types are green.

## Task 2: Implement workflow storage paths and event-log persistence

**Files:**

- Create: `viden-workflows/src/stores.rs`
- Modify: `viden-session/src/lib.rs`
- Modify: `viden-types/src/lib.rs`

- [ ] Add storage-path helpers in `viden-workflows/src/stores.rs` that derive a per-project workflow home alongside existing session storage.
- [ ] Reuse the existing project-key convention from `viden-session` instead of inventing a second project identity.
- [ ] Define append-only event payload structs for:
  - task events
  - memory events
- [ ] Add canonical file locations:
  - `tasks.jsonl`
  - `memory.jsonl`
  - `workflow.sqlite3`
- [ ] Implement append helpers for task and memory events.
- [ ] Implement load/replay helpers for task and memory event streams.
- [ ] Add a SQLite derived-index bootstrap path for workflow state, mirroring the current “canonical JSONL + rebuildable SQLite” approach.
- [ ] Expose any missing project-key or path helper from `viden-session/src/lib.rs` if needed, but do not move workflow state into that crate.
- [ ] Add tests in `viden-workflows/src/stores.rs` for:
  - path derivation
  - JSONL append/load roundtrip
  - SQLite rebuild from event logs
- [ ] Run: `cargo test -p viden-workflows stores`
- [ ] Commit once storage and rebuild behavior are stable.

## Task 3: Implement project task domain and reducer logic

**Files:**

- Create: `viden-workflows/src/tasks.rs`
- Modify: `viden-workflows/src/lib.rs`
- Modify: `viden-types/src/lib.rs`

- [ ] Define task-domain commands/events in `viden-workflows/src/tasks.rs` for:
  - create
  - update
  - status change
  - link dependency
  - block
  - unblock
  - archive
  - restore
- [ ] Use `parent_task_id` for hierarchy rather than a separate subtask type.
- [ ] Represent blockers as either:
  - another task id
  - free-form text reason
- [ ] Implement a reducer that rebuilds `TaskRecord` state from task events.
- [ ] Add task query helpers for:
  - active tasks
  - blocked tasks
  - archived tasks
  - task lookup by id
  - child tasks
- [ ] Validate invariants such as:
  - archived tasks cannot be re-archived
  - dependency links cannot point to missing tasks
  - restore requires archived state
- [ ] Add tests for:
  - create/update roundtrip
  - link/block/unblock behavior
  - archive/restore behavior
  - hierarchy reconstruction
- [ ] Run: `cargo test -p viden-workflows tasks`
- [ ] Commit the task domain before starting memory.

## Task 4: Implement project/session memory domain and suggestion flow

**Files:**

- Create: `viden-workflows/src/memory.rs`
- Modify: `viden-workflows/src/lib.rs`
- Modify: `viden-types/src/lib.rs`

- [ ] Define memory-domain commands/events in `viden-workflows/src/memory.rs` for:
  - add
  - suggest
  - confirm
  - reject
  - prune
  - supersede
- [ ] Enforce scope rules:
  - session memory can be added directly
  - project memory suggestions start as `suggested`
  - project memory becomes `active` only after confirmation
- [ ] Implement reducers/query helpers for:
  - active project memory
  - active session memory
  - pending suggestions
  - pruned/superseded history
- [ ] Support task linkage through `related_task_ids`.
- [ ] Add tests for:
  - direct session-memory add
  - project-memory suggest/confirm flow
  - reject flow
  - prune/supersede flow
  - scope isolation by session id
- [ ] Run: `cargo test -p viden-workflows memory`
- [ ] Commit after memory behavior and tests are green.

## Task 5: Implement resume-context derivation

**Files:**

- Create: `viden-workflows/src/resume_context.rs`
- Modify: `viden-workflows/src/lib.rs`
- Modify: `viden-session/src/lib.rs`
- Modify: `viden-types/src/lib.rs`

- [ ] Add a resume-context builder in `viden-workflows/src/resume_context.rs` that consumes:
  - current task state
  - memory state
  - recent session summaries and recent transcript metadata where needed
- [ ] Build `ResumeContextSnapshot` with:
  - active tasks
  - blocked tasks
  - recently completed tasks
  - relevant project memory
  - recent session memory
  - suggested next steps
  - suggested session-memory additions
- [ ] Allow only these derived side effects:
  - updating `last_seen_at`
  - updating `last_session_id`
- [ ] Do not allow `resume-context` to auto-change task status or auto-confirm project memory.
- [ ] Add tests for:
  - active/blocked/recent task selection
  - relevant memory selection
  - suggested next-step output
  - derived field updates without task-status mutation
- [ ] Run: `cargo test -p viden-workflows resume_context`
- [ ] Commit once resume-context behavior is deterministic.

## Task 6: Integrate workflow runtime into Viden core

**Files:**

- Modify: `viden-core/src/lib.rs`
- Modify: `viden-permissions/src/lib.rs`
- Modify: `viden-types/src/lib.rs`
- Modify: `Cargo.toml`

- [ ] Add `viden-workflows` as a dependency where needed.
- [ ] Extend `SessionEngine` setup so a workflow runtime/store can be constructed from the current cwd and session home.
- [ ] Add read-only command handling for:
  - `/tasks`
  - `/task view`
  - `/task resume-context`
  - `/memory`
  - `/memory project`
  - `/memory session`
  - `/memory suggest`
- [ ] Add mutation command handling for:
  - `/task add`
  - `/task update`
  - `/task status`
  - `/task link`
  - `/task block`
  - `/task unblock`
  - `/task archive`
  - `/task restore`
  - `/memory add`
  - `/memory confirm`
  - `/memory reject`
  - `/memory prune`
  - `/memory export`
- [ ] Keep all workflow commands inside the existing slash-command pipeline so they still write `TranscriptEntry::Command`.
- [ ] Add permission integration for workflow mutations in `viden-permissions/src/lib.rs`:
  - reads default-allow
  - workflow writes ask by default unless mode/rules override
- [ ] Add core tests for:
  - command parsing
  - transcript command logging
  - permission gating on mutating workflow commands
  - memory confirm/reject command paths
  - `/task resume-context` rendering
- [ ] Run: `cargo test -p viden-core`
- [ ] Commit once the CLI command surface is stable.

## Task 7: Add workflow summaries, exports, and docs

**Files:**

- Modify: `viden-core/src/lib.rs`
- Modify: `README.md`
- Modify: `README.zh-CN.md`

- [ ] Refine CLI rendering so:
  - `/tasks` shows a compact list with status, priority, and blocker hints
  - `/task view` shows full task detail
  - `/memory suggest` shows pending items clearly
  - `/task resume-context` shows summary first, then suggested actions
- [ ] Implement `/memory export` with a stable user-readable output format.
- [ ] Add README command examples for:
  - `/tasks`
  - `/task add`
  - `/task resume-context`
  - `/memory suggest`
  - `/memory confirm`
- [ ] Update both English and Chinese READMEs together.
- [ ] Run focused smoke checks:
  - `cargo run -p viden-cli -- --provider fallback --model test-local`
  - `/task add`
  - `/tasks`
  - `/task resume-context`
  - `/memory add`
  - `/memory suggest`
  - `/memory confirm`
- [ ] Commit documentation and rendering cleanup last.

## Task 8: Final verification and finish

**Files:**

- Modify: `viden-workflows/src/*.rs`
- Modify: `viden-core/src/lib.rs`
- Modify: `README.md`
- Modify: `README.zh-CN.md`

- [ ] Run focused crate tests during implementation:
  - `cargo test -p viden-types`
  - `cargo test -p viden-workflows`
  - `cargo test -p viden-core`
- [ ] Run final full verification:
  - `cargo test --workspace --quiet`
- [ ] Run a final CLI smoke pass covering:
  - `/tasks`
  - `/task add`
  - `/task block`
  - `/task resume-context`
  - `/memory add`
  - `/memory suggest`
  - `/memory confirm`
  - `/memory export`
- [ ] Verify that workflow data is written outside transcript canonical files and that transcript command entries still exist for workflow commands.
- [ ] After verification, use the standard branch-finishing flow to choose merge, PR, or keep-as-is.

## Acceptance Criteria

- Viden has a new `viden-workflows` crate with task, memory, resume-context, and store modules.
- Task state is durable at the project level and rebuildable from append-only task events.
- Project memory and session memory are distinct and follow the confirmed scope rules.
- Project memory suggestions require explicit confirmation before becoming active.
- `/task resume-context` produces useful workflow context without silently mutating task business state.
- Workflow commands remain visible in transcript command history and obey permissions.
- Full workspace tests pass.

## Follow-On Work

After this plan lands, the most natural next plans are:

- V2-B LSP foundation
- V2-D rich TUI and structured workflow views
- V3 automation once workflow state is stable enough to schedule safely
