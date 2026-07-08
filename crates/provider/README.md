# viden-provider

## Purpose

`viden-provider` owns model-provider abstraction and provider protocol adaptation.
It keeps provider identity, protocol family, config precedence, and runtime
provider construction outside `viden-runtime`.

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
- `ProviderHost`
- `ProviderRegistry`
- `ProviderDescriptor`
- `ModelRequestControl`

## Provider Plugin Runtime

The provider runtime has two layers:

- Built-in providers are registered through stable Rust code and currently
  include Anthropic, OpenAI, OpenAI-compatible, Ollama, fallback, DeepSeek, and
  DeepSeek Anthropic-compatible entries.
- Dynamic provider plugins are discovered from resolved plugin directories. The
  CLI/config layer supports `provider_plugin_dirs`, `VIDEN_PROVIDER_PLUGIN_DIRS`,
  and repeatable `--provider-plugin-dir <dir>` inputs.

Dynamic loading is descriptor-driven. A native plugin exposes the
`viden_plugin_descriptor_json` symbol and returns a serialized provider
descriptor. The host validates the descriptor, merges it with built-ins, rejects
provider-id collisions, and creates runtime provider instances through the
registered protocol adapter. This keeps the plugin boundary serialized and
host-mediated instead of exposing an unstable Rust trait-object ABI.

Dynamic provider API base resolution uses this precedence:

1. Explicit runtime config or CLI override.
2. Descriptor `env_mappings.api_base_env`.
3. Descriptor `default_api_base`.

The resolved API base must start with `http://` or `https://`. Invalid
environment-derived values fail provider construction instead of being passed to
HTTP transport.

Registry refresh is atomic from the caller's perspective:

- `ProviderHost::refresh` rebuilds from the default plugin directories.
- `ProviderHost::refresh_from_dirs` rebuilds from explicit directories.
- `ProviderHost::refresh_diagnostic` and
  `ProviderHost::refresh_from_dirs_diagnostic` preserve structured
  `ProviderPluginError` details for runtime reload diagnostics.
- Failed refreshes return an error and keep the previously active registry.
- Existing provider instances remain independent after refresh; new provider
  instances use the refreshed registry.

Plugin loader failures are structured as `ProviderPluginError` with a kind,
path, and message. Registry/host APIs still expose readable strings for
compatibility, while diagnostic host/registry constructors and refresh APIs preserve the
structured error for CLI diagnostics.

Current boundary: dynamic plugins register descriptors and reuse host-side
OpenAI or Anthropic protocol adapters. Request execution supports a shared
`ModelRequestControl` cancellation signal so core/runtime callers can stop
provider dispatch and in-flight HTTP subprocesses. The same control object can
request streaming-compatible OpenAI/Anthropic payloads, and the parser can
normalize SSE text/tool-call deltas back into `ModelEvent`. Parsers also
preserve provider-reported usage as `ModelEvent::Usage` for core telemetry.
Full incremental UI delivery, provider-owned execution, signing, sandboxing, and
marketplace/distribution are future hardening work.

## Invariants

- Core depends on `ModelProvider`, not concrete providers.
- Native tool calls normalize into `ModelEvent::ToolCall`.
- Provider-reported token usage normalizes into `ModelEvent::Usage`; parsers
  must not synthesize usage when the provider did not report it.
- HTTP/provider failures return errors, not panics.
- Provider identity is separate from protocol family.
- Plugin descriptors are validated before they become visible in the registry.
- Dynamic provider API base resolution must preserve explicit-over-env-over-default precedence.
- Registry refresh must not silently drop the previous working registry on load
  failure.
- Cancellation must be checked before dispatch and while HTTP transport is
  waiting on provider subprocesses.
- Streaming requests must require both caller preference and provider
  capability support.
- Streaming protocol parsing must preserve the same text/tool-call semantics as
  non-streaming JSON parsing.

## Reference Alignment

Matches `.ref` model/tool loop behavior while isolating vendor protocols from
core. The plugin runtime borrows the reference project's pluggability shape, not
its JavaScript/Bun implementation details.

## Test

```bash
cargo test -p viden-provider
```
