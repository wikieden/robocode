# robocode-session

## Purpose

`robocode-session` owns durable session transcripts, session listing, resume loading, and rebuildable session indexing.

## Does Not Own

- Project task or memory state; use `robocode-workflows`.
- Tool execution.
- Permission decisions.

## Public Surface

- `SessionStore`
- `SessionPaths`
- `project_key_for_path`

## Invariants

- Transcript JSONL is canonical.
- SQLite index is derived and rebuildable.
- Resume reconstructs history from transcript order.
- Workflow state must not use this crate as source of truth.

## Reference Alignment

Matches `.ref` session history behavior: append-only events and project-scoped resume.

## Test

```bash
cargo test -p robocode-session
```
