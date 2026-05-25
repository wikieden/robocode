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
- `ProviderTelemetry`, exposed through `SessionEngine::provider_telemetry()`
  for real model-request counts, latency, event count, provider-reported token
  usage, derived token throughput, optional provider-reported cost, and
  last-error state.
- Runtime/session/provider/Git/Web/task/memory command handling.
- Provider runtime commands:
  - `/provider` reports the current provider instance.
  - `/provider list` renders the active provider registry, including compact
    provider compatibility flags.
  - `/provider doctor [id]` renders provider diagnostics, including focused
    compatibility requirements for one provider when an id is passed.
  - `/provider reload` reloads provider plugin descriptors without replacing
    the current provider instance.
  - `/provider use <id> [model]` switches the current provider instance through
    the active registry.

## Invariants

- Tool calls and mutating workflow commands must pass permission checks before execution.
- Slash commands write `TranscriptEntry::Command`.
- Provider reload is atomic: a failed reload reports diagnostics and keeps the
  previous registry active.
- Provider switching must go through the active `ProviderHost`; it must update
  transcript metadata for provider and model.
- Provider health surfaces must use `ProviderTelemetry`; do not invent latency,
  token, cost, or rate values in callers. Cost remains absent unless a provider
  or future pricing layer supplies auditable cost data.
- `/task resume-context` may update derived fields but must not change task business status.
- Transcript auditability must remain intact.

## Reference Alignment

Maps behavior from `.ref` `main.tsx`, `commands.ts`, `Tool.ts`, permission types, and task/session flows into Rust orchestration.

## Test

```bash
cargo test -p robocode-core
```
