# viden-session

## Purpose

`viden-session` owns durable session transcripts, session listing, resume loading, and rebuildable session indexing.

## Does Not Own

- Project task or memory state; use `viden-workflows`.
- Tool execution.
- Permission decisions.

## Public Surface

- `SessionStore`
- `SessionPaths`
- `project_key_for_path`
- `default_session_home_dir`
- `SessionStore::query_recent_work`

## Invariants

- Transcript JSONL is canonical.
- SQLite index is derived and rebuildable.
- Resume reconstructs history from transcript order.
- Workflow state must not use this crate as source of truth.
- Cross-project recent-work discovery scans only the shared
  `<session-home>/projects` inventory and validates canonical root metadata.
- Recent-work results are bounded whitelist DTOs; transcript paths, previews,
  arbitrary metadata, and bodies never cross that boundary.

## Reference Alignment

Matches `.ref` session history behavior: append-only events and project-scoped resume.

## Test

```bash
cargo test -p viden-session
```
