# robocode-workflows

## Purpose

`robocode-workflows` owns durable project workflow state: tasks, project/session memory, resume context, and workflow event storage.

## Does Not Own

- Session transcript facts; use `robocode-session`.
- Slash-command parsing; use `robocode-core`.
- Permission decisions; core routes workflow writes through `robocode-permissions`.

## Public Surface

- `tasks`: task reducer and queries.
- `memory`: project/session memory reducer and queries.
- `resume_context`: builder for `/task resume-context`.
- `stores`: workflow JSONL logs and derived SQLite bootstrap.

## Internal Modules

### `tasks`

Owns `TaskEvent`, `TaskUpdate`, `TaskBlocker`, `TaskState`, and `reduce_task_events`. Supports create, update, status, link, block, unblock, archive, restore, parent/child hierarchy, dependencies, and derived `Seen` events.

### `memory`

Owns `MemoryEvent`, `MemoryState`, and `reduce_memory_events`. Supports session memory add, project memory suggest/confirm/reject, prune, supersede, active project/session memory, and pending suggestions.

### `resume_context`

Owns `ResumeContextInput`, `ResumeContextBuild`, and `build_resume_context`. Produces `ResumeContextSnapshot`, suggested next steps, suggested session memory, and derived task `Seen` events. It must not change task business status or auto-confirm project memory.

### `stores`

Owns `WorkflowStore`, `WorkflowPaths`, `WorkflowTaskEvent`, and `WorkflowMemoryEvent`. Stores canonical workflow logs in `tasks.jsonl` and `memory.jsonl`, creates `workflow.sqlite3`, and validates checked appends before writing.

## Invariants

- Workflow JSONL is canonical.
- SQLite is derived and rebuildable.
- Invalid task/memory events must not be appended.
- Workflow state and transcript state are separate but share project identity.

## Reference Alignment

Uses `.ref/src/tasks/*` and session workflow ideas, but keeps a smaller Rust event-log model.

## Test

```bash
cargo test -p robocode-workflows
```
