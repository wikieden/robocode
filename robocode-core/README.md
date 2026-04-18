# robocode-core

## Purpose

`robocode-core` owns `SessionEngine`: the shared runtime path for user input, slash commands, provider events, tool calls, permission checks, transcript writes, and workflow commands.

## Does Not Own

- Concrete model protocols.
- Tool implementation details.
- JSONL/SQLite storage internals.
- Task and memory reducer rules.

## Public Surface

- `SessionEngine`
- `EngineEvent`
- Runtime/session/Git/Web/task/memory command handling.

## Invariants

- Tool calls and mutating workflow commands must pass permission checks before execution.
- Slash commands write `TranscriptEntry::Command`.
- `/task resume-context` may update derived fields but must not change task business status.
- Transcript auditability must remain intact.

## Reference Alignment

Maps behavior from `.ref` `main.tsx`, `commands.ts`, `Tool.ts`, permission types, and task/session flows into Rust orchestration.

## Test

```bash
cargo test -p robocode-core
```
