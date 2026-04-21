# robocode-config

## Purpose

`robocode-config` owns deterministic runtime configuration resolution.

## Does Not Own

- Provider execution.
- Permission policy evaluation beyond carrying the selected mode.
- Session or workflow persistence.

## Public Surface

- `CliOverrides`
- `ResolvedConfig`
- `load_config`

## Invariants

- Precedence is `CLI > environment > project config > global config > defaults`.
- Config loading reads files/env only; it must not execute side effects.
- Summaries must not expose raw API keys.

## Reference Alignment

Matches `.ref` layered settings behavior, without managed settings or analytics machinery.

## Test

```bash
cargo test -p robocode-config
```
