# robocode-types

## Purpose

`robocode-types` owns shared domain contracts used by every other crate.

## Does Not Own

- Runtime behavior.
- Persistence implementation.
- Provider/tool execution.

## Public Surface

- Message, tool, provider, permission, transcript, session, runtime, task, memory, and resume context types.
- CLI-name helpers for public enums.
- Lightweight transcript encode/decode helpers.

## Invariants

- Keep types behavior-neutral.
- Serialization shape changes must be deliberate; JSONL logs and adapters depend on them.
- Public CLI names for modes/statuses should stay stable.

## Reference Alignment

Collects contracts corresponding to `.ref` command, log, permission, task, and id types.

## Test

```bash
cargo test -p robocode-types
```
