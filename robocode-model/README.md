# robocode-model

## Purpose

`robocode-model` owns model-provider abstraction and provider protocol adaptation.

## Does Not Own

- Session orchestration.
- Tool execution.
- Permission prompts.
- Transcript persistence.

## Public Surface

- `ModelProvider`
- `ProviderKind`
- `ProviderConfig`
- `create_provider`

## Invariants

- Core depends on `ModelProvider`, not concrete providers.
- Native tool calls normalize into `ModelEvent::ToolCall`.
- HTTP/provider failures return errors, not panics.

## Reference Alignment

Matches `.ref` model/tool loop behavior while isolating vendor protocols from core.

## Test

```bash
cargo test -p robocode-model
```
