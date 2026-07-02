# viden-config

## Purpose

`viden-config` owns deterministic runtime configuration resolution.

## Does Not Own

- Provider execution.
- Permission policy evaluation beyond carrying the selected mode.
- Session or workflow persistence.

## Public Surface

- `CliOverrides`
- `ResolvedConfig`
- `load_config`

## Provider Plugin Directories

Provider plugin directories are now part of the same deterministic config
pipeline as provider/model/API settings:

- `provider_plugin_dirs = ["./plugins"]` in config files.
- `ROBOCODE_PROVIDER_PLUGIN_DIRS` as a platform path-list environment variable.
- `--provider-plugin-dir <dir>` from the CLI, repeatable.

The resolved value is carried in `ResolvedConfig::provider_plugin_dirs` so the
CLI can build the active `ProviderHost` from explicit, auditable inputs.

## Invariants

- Precedence is `CLI > environment > project config > global config > defaults`.
- Config loading reads files/env only; it must not execute side effects.
- Summaries must not expose raw API keys.
- Provider plugin directory resolution is data-only; plugin loading belongs to
  `viden-provider`.

## Reference Alignment

Matches `.ref` layered settings behavior, without managed settings or analytics machinery.

## Test

```bash
cargo test -p viden-config
```
