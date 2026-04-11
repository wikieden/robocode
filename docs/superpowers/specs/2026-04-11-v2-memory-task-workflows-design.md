# V2 Memory and Task Workflows Design

## Purpose

This document defines the V2-C design for RoboCode's memory and task
workflows. The goal is to make project continuity explicit instead of
incidental by introducing project-level tasks, two-tier memory, and a
workflow-oriented resume surface without breaking the existing session,
permission, and transcript model.

The design follows the confirmed direction for this slice:

- tasks are project-level durable state
- memory is split into project memory and session memory
- project memory writes are suggestion-driven and require explicit confirmation
- the first command surface includes advanced task and memory flows
- `/task resume-context` is workflow-driving, but it does not silently mutate
  task business state

## Product Goal

RoboCode should help a developer return to a project and answer:

- what is currently in flight
- what is blocked
- what long-lived decisions or constraints should be remembered
- what happened recently in this session
- what the most sensible next step is

V2-C must make those answers available inside the CLI through the same shared
runtime path used by existing commands and tools.

## Scope

In scope:

- project-level task lifecycle management
- project memory and session memory
- workflow-aware resume summaries
- memory suggestion and confirmation flow
- transcript-visible command behavior for all workflow commands
- durable workflow storage and derived indexes

Out of scope:

- LSP integration
- multi-agent ownership semantics beyond placeholder assignee fields
- scheduled automation
- MCP-backed workflow sync
- rich TUI views beyond current CLI text rendering

## Architecture

### New Crate

Add a new crate:

- `robocode-workflows`

The crate should expose a unified public API to `robocode-core`, but its
internal modules should be separated from day one:

- `tasks`
- `memory`
- `resume_context`
- `stores`

This is intentionally a hybrid between a monolithic first pass and a fully
split crate topology. It keeps the implementation manageable while preserving
clear subsystem boundaries for later V2 and V3 work.

### Responsibility Split

- `robocode-core`
  parses slash commands, routes workflow actions, applies permissions, records
  transcript command entries, and renders CLI output
- `robocode-session`
  remains the source of truth for session transcript history and session index
- `robocode-workflows`
  owns project-level task state, project/session memory state, resume-context
  derivation, and workflow-specific persistence

The key invariant is:

- transcript records what happened during a session
- workflow storage records the current durable task and memory state

Neither subsystem should absorb the other.

## Data Model

### Task

Tasks are the primary project-level workflow object.

Required fields:

- `task_id`
- `title`
- `description`
- `status`
- `priority`
- `labels`
- `assignee_hint`
- `parent_task_id`
- `dependency_ids`
- `blocked_by`
- `notes`
- `created_at`
- `updated_at`
- `last_session_id`
- `last_seen_at`
- `archived_at`

Task status values for V2-C:

- `todo`
- `in_progress`
- `blocked`
- `done`
- `archived`

Task priority values for V2-C:

- `low`
- `medium`
- `high`
- `critical`

Subtasks should use `parent_task_id` rather than a separate subtask type.

`blocked_by` may reference:

- another task id
- a free-form textual blocker reason

### Memory Entry

Project memory and session memory should share one entry shape.

Required fields:

- `memory_id`
- `scope`
- `session_id`
- `kind`
- `content`
- `source`
- `status`
- `created_at`
- `updated_at`
- `related_task_ids`
- `confidence_hint`

Memory scopes:

- `project`
- `session`

Memory kinds:

- `fact`
- `preference`
- `constraint`
- `decision`
- `convention`

Memory sources:

- `user`
- `assistant_suggestion`
- `command`
- `imported`

Memory status values:

- `suggested`
- `active`
- `superseded`
- `pruned`
- `rejected`

Rules:

- project memory may exist only as `suggested` or `active` after a model
  suggestion flow
- session memory may be written directly through explicit commands
- project memory never becomes `active` without confirmation

### Resume Context Snapshot

`ResumeContextSnapshot` is a derived, query-time object rather than a canonical
stored entity.

Required content:

- active tasks
- blocked tasks
- recently completed tasks
- relevant project memory
- recent session memory
- suggested next steps
- suggested session memory additions

This object exists to support `/task resume-context` and related future
workflow surfaces. It should be rebuildable from workflow state plus recent
session metadata.

## Persistence Model

### Canonical Storage

Workflow state should not be stored inside the session transcript as its
primary source of truth.

Create a workflow data area under the session home and project key, parallel to
session transcript storage.

Canonical files:

- `tasks.jsonl`
- `memory.jsonl`

Derived index:

- `workflow.sqlite3`

### Event Model

`tasks.jsonl` should be append-only and record events such as:

- `task_created`
- `task_updated`
- `task_status_changed`
- `task_linked`
- `task_blocked`
- `task_unblocked`
- `task_archived`
- `task_restored`

`memory.jsonl` should be append-only and record events such as:

- `memory_suggested`
- `memory_confirmed`
- `memory_rejected`
- `memory_added`
- `memory_pruned`
- `memory_superseded`

### Relationship to Transcript

Transcript entries continue to record:

- the slash command invocation and output
- suggestion generation
- confirmation or rejection actions

Workflow event logs record:

- the durable task and memory state transitions themselves

Both event streams may include optional cross references:

- `origin_session_id`
- produced `task_id`
- produced `memory_id`

The system must be able to correlate session activity with workflow state
changes without making one depend on replaying the other.

## Command Surface

### Task Commands

Required commands for V2-C:

- `/tasks`
- `/task add <title>`
- `/task view <task-id>`
- `/task update <task-id>`
- `/task status <task-id> <status>`
- `/task link <task-id> <depends-on-id>`
- `/task block <task-id> <reason|task-id>`
- `/task unblock <task-id>`
- `/task archive <task-id>`
- `/task restore <task-id>`
- `/task resume-context`

Expected behavior:

- `/tasks` shows active project tasks by default
- `/task view` renders a single task's full state
- `/task link` creates task dependencies
- `/task block` marks a task blocked by another task or explicit reason
- `/task resume-context` renders a workflow summary plus next-step suggestions

### Memory Commands

Required commands for V2-C:

- `/memory`
- `/memory project`
- `/memory session`
- `/memory add <content>`
- `/memory suggest`
- `/memory confirm <memory-id>`
- `/memory reject <memory-id>`
- `/memory prune <memory-id>`
- `/memory export`

Expected behavior:

- `/memory` defaults to active project memory overview
- `/memory project` shows project memory only
- `/memory session` shows current session memory only
- `/memory add` supports explicit manual writes
- `/memory suggest` surfaces pending suggestions
- `/memory confirm` promotes a project memory suggestion to active
- `/memory reject` leaves a full audit trail
- `/memory prune` retires memory without deleting history
- `/memory export` outputs a durable, user-readable snapshot

## Permission and Confirmation Model

Workflow commands must not create a side channel around the existing
permission system.

Command classes:

- read-only commands
- controlled mutation commands
- suggestion-confirmation commands

Read-only commands:

- `/tasks`
- `/task view`
- `/task resume-context`
- `/memory`
- `/memory project`
- `/memory session`
- `/memory suggest`

Controlled mutation commands:

- `/task add`
- `/task update`
- `/task status`
- `/task link`
- `/task block`
- `/task unblock`
- `/task archive`
- `/task restore`
- `/memory add`
- `/memory prune`

Suggestion-confirmation commands:

- `/memory confirm`
- `/memory reject`

Rules:

- read-only commands execute without approval prompts unless future policy says
  otherwise
- mutation commands flow through the existing permission engine
- project memory suggestions may be generated by the assistant, but they remain
  non-active until explicit confirmation
- transcript and workflow logs must both reflect the confirmation decision

## `/task resume-context` Behavior

`/task resume-context` is the centerpiece of V2-C.

Its output should include four parts:

1. project workflow summary
2. memory summary
3. suggested next steps
4. suggested session-memory updates

Allowed side effects:

- update `last_seen_at` on tasks surfaced into the resume context
- update `last_session_id` for tasks materially referenced by the current
  session
- create derived suggestions for next task focus
- create suggested session memory candidates

Disallowed side effects:

- silently changing task status
- auto-confirming project memory
- archiving, restoring, or relinking tasks

The command is workflow-driving in the sense that it helps the user decide what
to do next. It is not a hidden workflow executor.

## CLI Rendering Expectations

V2-C should preserve the lightweight CLI style already established in RoboCode.

Rendering guidance:

- `/tasks` should render a compact list with status, priority, and blocker cues
- `/task view` should render a detail card
- `/memory suggest` should render pending memory items as confirmable entries
- `/task resume-context` should render a readable summary followed by explicit
  suggested actions

No rich TUI is required in this slice.

## Testing Strategy

Required test categories:

- task event roundtrip tests
- memory event roundtrip tests
- derived workflow index rebuild tests
- command routing tests in `robocode-core`
- permission integration tests for mutation commands
- suggestion-confirmation flow tests for project memory
- `/task resume-context` derivation tests
- transcript logging tests for workflow commands

Key scenarios:

- create and update tasks across multiple sessions in one project
- link and block tasks, then recover context later
- suggest project memory, confirm some, reject others
- add session memory explicitly and verify session scoping
- generate resume context after mixed task and memory history
- rebuild workflow index from append-only event logs

## Non-Goals for This Slice

V2-C should not yet attempt:

- real agent assignment or ownership enforcement
- automatic task planning from every conversation
- cross-project memory federation
- remote workflow synchronization
- scheduled workflow execution
- semantic code intelligence

## File Direction

Expected new files and modules:

- `robocode-workflows/Cargo.toml`
- `robocode-workflows/src/lib.rs`
- `robocode-workflows/src/tasks.rs`
- `robocode-workflows/src/memory.rs`
- `robocode-workflows/src/resume_context.rs`
- `robocode-workflows/src/stores.rs`

Expected integration points:

- `Cargo.toml`
- `robocode-core/src/lib.rs`
- `robocode-session/src/lib.rs`
- `robocode-types/src/lib.rs`
- `README.md`
- `README.zh-CN.md`

## Exit Criteria

This design is satisfied when RoboCode can:

- track durable project tasks with lifecycle and dependency state
- maintain project memory and session memory separately
- require explicit confirmation for assistant-suggested project memory
- produce useful workflow-oriented resume context
- preserve transcript and permission invariants while doing so
