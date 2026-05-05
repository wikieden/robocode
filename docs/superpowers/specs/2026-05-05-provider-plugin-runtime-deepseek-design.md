# Provider Plugin Runtime and DeepSeek v4 Design

## Purpose

This document defines a new provider-platform slice for RoboCode. The immediate
goal is to support DeepSeek v4 as a first-class provider, but the actual design
target is broader: the model layer must evolve from a small built-in provider
factory into a plugin-extensible provider runtime.

The confirmed direction for this slice is:

- RoboCode must support both Anthropic/Claude-style and OpenAI-style protocol
  families
- DeepSeek must be supported as an independent provider family, not only as an
  OpenAI-compatible endpoint alias
- provider registration must support dynamic loading
- the first delivery may use native dynamic libraries
- the plugin API must be designed so it can later migrate to a WASM execution
  model without rewriting the host/provider contract

## Product Goal

RoboCode should make it cheap to add new model providers over time without
changing `SessionEngine`, tool/runtime flow, or other core product surfaces.

After this slice, RoboCode should be able to answer:

- which provider families are available right now
- which ones are built in versus dynamically loaded
- which protocol family each provider uses
- how provider-specific configuration is resolved
- how a new provider can be added without editing core orchestration logic

DeepSeek v4 is the first real acceptance target, but the product requirement is
the provider plugin runtime itself.

## Scope

In scope:

- provider plugin host/runtime in `robocode-model`
- dynamic provider registry
- built-in providers as one registry source
- native dynamic-library provider plugins as the first dynamic loading mode
- a provider ABI/contract designed for later WASM migration
- provider-scoped configuration with generic fallback
- DeepSeek v4 as the first plugin-backed provider example
- continued support for both Anthropic-style and OpenAI-style protocol families

Out of scope for the first implementation slice:

- full WASM runtime execution for provider plugins
- plugin marketplace/distribution
- plugin signing and trust enforcement
- network sandboxing for plugins
- remote plugin installation UX
- unrelated core-engine, session, or tool-loop redesign

## Architecture

### Layered Model

The provider system should be split into five layers:

1. `ProviderHost`
2. `ProviderRegistry`
3. `ProviderDescriptor`
4. `ProtocolAdapter`
5. `ProviderPlugin`

This separates provider discovery, metadata, protocol behavior, and runtime
construction so new providers do not force changes into the core engine.

### ProviderHost

`ProviderHost` lives in `robocode-model` and owns:

- loading built-in providers
- scanning plugin directories
- loading dynamic provider plugins
- constructing the in-memory registry
- resolving the selected provider at startup/runtime

It is the host-side runtime surface, not the protocol implementation layer.

### ProviderRegistry

`ProviderRegistry` is the canonical lookup surface for provider availability.

Responsibilities:

- store built-in provider descriptors
- store dynamically loaded provider descriptors
- resolve provider id collisions
- expose provider lookup by `provider_id`
- expose provider listing for CLI/status surfaces

The registry is a product-facing domain object, not just an internal map.

### ProviderDescriptor

`ProviderDescriptor` is a serializable declaration of provider identity and
capabilities. It should be transport-safe across a future WASM boundary.

Required fields:

- `provider_id`
- `display_name`
- `version`
- `protocol_family`
- `default_api_base`
- `default_model`
- `env_mappings`
- `capabilities`
- `config_schema_version`

Optional fields may include:

- `docs_url`
- `supports_streaming`
- `supports_native_tool_calling`
- `supports_reasoning_controls`
- `provider_metadata`

### ProtocolAdapter

`ProtocolAdapter` implements protocol-family behavior.

The first two protocol families are:

- `anthropic`
- `openai`

Responsibilities:

- encode model requests
- decode streamed or batched model responses
- normalize tool-calling behavior into RoboCode model events
- normalize usage reporting
- normalize provider errors

Multiple providers may share one adapter family. This is how DeepSeek can be a
distinct provider while still using the OpenAI-style protocol family.

### ProviderPlugin

`ProviderPlugin` binds provider identity to runtime behavior.

Responsibilities:

- expose a `ProviderDescriptor`
- resolve provider-scoped configuration
- declare which protocol adapter it uses
- perform provider-specific validation
- construct a concrete `ModelProvider`

The plugin is not allowed to reach directly into `SessionEngine` or transcript
logic. It only participates through the model-provider boundary.

## Dynamic Loading Model

### Registry Sources

The registry must support multiple sources:

1. built-in providers
2. local dynamic plugins discovered from plugin directories
3. future remote/package-managed plugin sources

The first implementation only needs sources 1 and 2 to be live, but the host
API should be designed so source 3 can be added later without redesigning the
registry contract.

### Native Dynamic Libraries

The first dynamic execution mode uses native dynamic libraries:

- macOS: `.dylib`
- Linux: `.so`
- Windows: `.dll`

The loader should discover candidate files, attempt to load them, extract a
descriptor and entrypoint surface, and report structured load failures rather
than crashing the host.

### ABI Boundary

The ABI boundary must not expose internal Rust traits or rely on unstable Rust
object layouts.

The boundary should instead use:

- stable exported entrypoints
- C-compatible or byte-serialized payload boundaries
- host-side Rust wrappers that translate plugin ABI calls into internal model
  abstractions

This is required both for native dynamic loading safety and for future WASM
portability.

## WASM Migration Constraint

The first implementation may execute plugins as native libraries, but the
plugin contract must be designed as if it will later run under WASM.

That means:

- provider descriptors must be serializable
- request/response/tool-call payloads must be serializable
- plugin entrypoints must not assume direct access to host-side Rust structs
- plugin/host interaction should be capability-oriented rather than pointer- or
  trait-object-oriented

The expected later evolution is:

- keep the same provider host/registry shape
- replace or supplement the native loader with a WASM runtime
- reuse the same descriptor and message contract with minimal redesign

## Protocol Family Requirement

RoboCode must continue to support both major protocol styles:

- Anthropic/Claude-style
- OpenAI-style

This is a hard architectural requirement. The plugin system is not allowed to
collapse all vendors into one implicit OpenAI-compatible abstraction.

Rules:

- providers declare their `protocol_family`
- adapters own protocol behavior
- providers own identity, configuration, and validation
- the core engine sees normalized `ModelEvent` output regardless of provider

## DeepSeek v4 Requirement

DeepSeek must be a first-class provider family with:

- `provider_id = "deepseek"`
- `display_name = "DeepSeek"`
- `protocol_family = "openai"`
- default model target `deepseek-v4`

DeepSeek must remain usable even though it uses the OpenAI-style adapter
family. The user-facing product should treat it as a distinct provider, not as
an undocumented endpoint variation of `openai`.

### Configuration Resolution

DeepSeek configuration priority should be:

1. provider-scoped DeepSeek config values
2. `DEEPSEEK_API_KEY`
3. `DEEPSEEK_API_BASE`
4. generic config values such as `api_key` and `api_base`
5. provider defaults from the descriptor

The provider-specific path should have priority, while the generic path serves
as compatibility fallback.

### Compatibility Rule

RoboCode should also continue to support pointing a generic OpenAI-compatible
provider configuration at a DeepSeek endpoint when the user explicitly wants
that path. But this must not replace or weaken the independent DeepSeek product
surface.

## Configuration Model

The current provider configuration shape should evolve into three layers:

1. generic provider config
2. provider-scoped config
3. plugin-declared config schema

### Generic Config

Shared fields:

- `provider`
- `model`
- `api_key`
- `api_base`
- `request_timeout_secs`
- `max_retries`

### Provider-Scoped Config

Provider-scoped configuration should allow fields such as:

- `providers.deepseek.api_key`
- `providers.deepseek.api_key_env`
- `providers.deepseek.api_base`
- `providers.deepseek.default_model`

and similarly for other providers.

### Plugin-Declared Schema

Plugins should be able to declare:

- supported config keys
- supported environment-variable mappings
- required versus optional fields
- defaults and validation rules

The host uses this for config validation and user-facing error messages.

## Acceptance Criteria

The completed product behavior for this slice should satisfy:

1. RoboCode can list both built-in and dynamically loaded providers.
2. DeepSeek can be selected as `provider=deepseek`.
3. DeepSeek v4 can be constructed through the plugin system without modifying
   `SessionEngine`.
4. Anthropic-style and OpenAI-style providers both still function through the
   normalized provider contract.
5. Provider-specific config takes precedence over generic fallback config.
6. Plugin load failures are surfaced as structured errors, not host crashes.
7. Adding a new provider that reuses an existing protocol adapter should not
   require core-engine changes.
8. The plugin contract does not expose unstable Rust trait ABI across the
   dynamic boundary.

## Risks and Constraints

### Risks

- native dynamic library ABI instability
- cross-platform loading differences
- plugin trust and safety concerns
- over-coupling provider identity to protocol implementation

### Constraints

- the first slice should not require a full plugin marketplace
- the first slice should not promise strong sandboxing
- the first slice must keep current provider behavior working

### Mitigations

- use a serialized/stable plugin boundary
- keep protocol families separate from provider identities
- treat native loading as a first delivery step, not the final security model
- keep the plugin contract capability-oriented for later WASM migration

## Delivery Phasing

### Phase 1: Provider Plugin Runtime + DeepSeek v4

Target:

- provider host/runtime
- dynamic registry
- native plugin loading
- provider-scoped config
- DeepSeek plugin
- protocol-family binding

### Phase 2: Plugin Hardening

Target:

- signing/trust model
- packaging/distribution
- stronger isolation
- WASM runtime support
- plugin authoring docs/tooling

The first implementation plan should target Phase 1 only, while preserving the
architectural runway for Phase 2.
