# viden-workflows

## Purpose

`viden-workflows` owns durable project workflow state: tasks, project/session memory, resume context, and workflow event storage.

## Does Not Own

- Session transcript facts; use `viden-session`.
- Slash-command parsing; use `viden-runtime`.
- Permission decisions; core routes workflow writes through `viden-permissions`.

## Public Surface

- `tasks`: task reducer and queries.
- `memory`: project/session memory reducer and queries.
- `lanes`: typed lane lifecycle reducer and legacy lane migration.
- `resume_context`: builder for `/task resume-context`.
- `stores`: workflow JSONL logs and derived SQLite bootstrap.

## Internal Modules

### `tasks`

Owns `TaskEvent`, `TaskUpdate`, `TaskBlocker`, `TaskState`, and `reduce_task_events`. Supports create, update, status, link, block, unblock, archive, restore, parent/child hierarchy, dependencies, and derived `Seen` events.

### `memory`

Owns `MemoryEvent`, `MemoryState`, and `reduce_memory_events`. Supports session memory add, project memory suggest/confirm/reject, prune, supersede, active project/session memory, and pending suggestions.

### `resume_context`

Owns `ResumeContextInput`, `ResumeContextBuild`, and `build_resume_context`. Produces `ResumeContextSnapshot`, suggested next steps, suggested session memory, and derived task `Seen` events. It must not change task business status or auto-confirm project memory.

### `lanes`

Owns `LaneEvent`, `LaneState`, and `reduce_lane_events`. Lane lifecycle facts are stored in `lanes.jsonl`. A legacy project `.viden/lanes.tsv` is accepted only as idempotent session-start or resume-activation migration input; runtime projection and status views read the typed event log afterwards.

### `stores`

Owns `WorkflowStore`, `WorkflowPaths`, `WorkflowTaskEvent`, and `WorkflowMemoryEvent`. Stores canonical workflow logs in `tasks.jsonl`, `memory.jsonl`, and `lanes.jsonl`, creates `workflow.sqlite3`, and validates checked appends before writing. Lane load/reduce/append transactions use a project-scoped advisory lock so concurrent sessions cannot duplicate a legacy import or lose a lifecycle event.

## Invariants

- Workflow JSONL is canonical.
- SQLite is derived and rebuildable.
- Invalid task, memory, or lane events must not be appended.
- A corrupt lane log must remain visible as a recoverable runtime error; clients must not silently render an empty lane set.
- Workflow state and transcript state are separate but share project identity.

## Reference Alignment

Uses `.ref/src/tasks/*` and session workflow ideas, but keeps a smaller Rust event-log model.

## Test

```bash
cargo test -p viden-workflows
```
