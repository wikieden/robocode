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
neutral descriptor plus optional process configuration. Core crates do not
depend on Headroom, Python, Pyo3, or any adapter-specific runtime.

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

- the same schema version, request id, canonical hash, permission snapshot
  reference, scope binding, and content kind;
- reduced UTF-8 content within strict byte/item/depth limits;
- omission records;
- the negotiated reducer id and version;
- deterministic quality facts, including score and evidence recall;
- latency and health metadata.

The adapter may optimize provider input, but it cannot mutate canonical context,
bypass permissions, weaken evidence recall, or change Merge Gate truth.

## Host Validation And Fallback

`crates/plugin-host` negotiates schema and content-kind support before calling an
adapter. Production adapters use `ContextReducerProcessConfig`: a safe
executable path plus literal args, optional cwd, explicit environment allowlist,
and bounded stderr capture. The host never invokes a shell, clears the
environment before applying the allowlist, sends request/response JSON over
bounded pipes, and enforces a host wall-clock deadline. On timeout the host
kills and waits for the direct child before returning native fallback. The
adapter contract disallows shell wrapping and child spawning when cross-platform
process-group cancellation is unavailable, so the direct-child kill guarantee is
the production cancellation boundary.

In-process closure executors exist only for trusted tests and cooperative local
harnesses. They run on a named worker thread with an owned request value,
`catch_unwind`, and a host wall-clock `recv_timeout`, but they cannot cancel
arbitrary user code after timeout and are not the production adapter boundary.
The runtime uses the process transport when configured, otherwise it uses no
external adapter. The host does not trust response-reported latency for timeout
decisions.

External output is accepted only when request id, canonical hash, permission
snapshot reference, scope, content kind, negotiated reducer id/version, schema
version, size, encoding, and quality/evidence thresholds all pass.

Timeouts, crashes, absent adapters, malformed responses, wrong schema versions,
wrong hashes, wrong scopes, oversize responses, and quality failures produce
bounded health evidence and deterministic native fallback. Startup and provider
requests must continue when the native reducer is healthy.

The host includes a circuit breaker with bounded `open_until` backoff. After the
failure threshold, calls are skipped until the monotonic deadline. The next call
after the deadline is a half-open probe: success resets the breaker, while
failure reopens it. Telemetry is limited to health, measured latency, and failure
category; it must not include secrets or raw local paths.

Runtime records successful and fallback adapter attempts through
`ContextReductionRecorded` / `ContextReductionRecord`. The record carries the
adapter id/version, bounded status/reason, host-measured latency, fallback flag,
item/view binding, and timestamp. It intentionally omits request content,
canonical storage paths, credentials, raw stderr, and raw adapter output.
Default-disabled adapters do not emit failure noise.
