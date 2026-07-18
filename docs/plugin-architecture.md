# Plugin Architecture

Chinese version: [plugin-architecture.zh-CN.md](plugin-architecture.zh-CN.md)

Viden plugins extend runtime behavior through versioned descriptors and host
validation. The Rust-native runtime remains authoritative for local state,
permissions, transcript facts, canonical context storage, evidence verification,
and Merge Gate decisions.

## Context Reducer Capability

`context_reducer` is an optional optimization capability for reducing canonical
context into bounded provider-ready views. It is disabled by default and only
runs when configuration explicitly opts in to an adapter descriptor.

The descriptor is represented by `ContextReducerDescriptor` in
`crates/plugin-api`:

- `reducer_id`, `display_name`, `version`;
- `supported_schema_versions`, currently schema version `1`;
- supported `content_kinds`;
- hard `limits` for input bytes, output bytes, output item count, and depth;
- `default_enabled`, which must remain `false` for built-in safety;
- `config_schema_version`.

Future adapters, including Headroom-style adapters, are represented by the same
neutral descriptor. Core crates do not depend on Headroom, Python, Pyo3, or any
adapter-specific process runtime.

## Request Envelope

The host sends `ContextReducerRequest` only after native context storage has
produced a canonical item and permission scope. The request contains:

- schema version and request id;
- content kind;
- canonical item id, canonical content SHA-256, optional evidence id, and a
  logical reference;
- reduction budget and policy;
- role/task scope;
- permission snapshot reference;
- optional native baseline quality facts.

Requests must never include local storage paths, raw credentials, API keys, or
mutable handles to the canonical store.

## Response Envelope

Adapters return `ContextReducerResponse` with:

- the same schema version, request id, canonical hash, and scope binding;
- reduced UTF-8 content within strict byte/item/depth limits;
- omission records;
- reducer id and version;
- deterministic quality facts, including score and evidence recall;
- latency and health metadata.

The adapter may optimize provider input, but it cannot mutate canonical context,
bypass permissions, weaken evidence recall, or change Merge Gate truth.

## Host Validation And Fallback

`crates/plugin-host` negotiates schema and content-kind support before calling an
adapter. External output is accepted only when request id, canonical hash, scope,
schema version, size, encoding, and quality/evidence thresholds all pass.

Timeouts, crashes, absent adapters, malformed responses, wrong schema versions,
wrong hashes, wrong scopes, oversize responses, and quality failures produce
bounded health evidence and deterministic native fallback. Startup and provider
requests must continue when the native reducer is healthy.

The host includes a circuit breaker with bounded backoff so repeated failing
adapters are skipped instead of repeatedly delaying context assembly. Telemetry
is limited to health, latency, and failure category; it must not include secrets
or raw local paths.
