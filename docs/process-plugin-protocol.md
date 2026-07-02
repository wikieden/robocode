# Process Plugin Protocol Draft

Chinese version: [process-plugin-protocol.zh-CN.md](process-plugin-protocol.zh-CN.md)

## Purpose

Viden plugins extend the runtime without bypassing core permission, evidence,
cost, transcript, or task/lane state. The first plugin boundary is a local
process protocol over newline-delimited JSON. Native dynamic loading and WASM
can be added later, but they must preserve the same capability and event model.

This is a Phase 0-2 contract draft. It defines the boundary that core, TUI, and
GUI branches can share before any UI-specific plugin panels are implemented.

## Non-Goals

- No plugin can directly call provider, tool, permission, transcript, workflow,
  TUI, or GUI internals.
- No plugin can mutate files, run commands, update memory, or change tasks
  without returning a `RuntimeCommand` or tool request that goes through core.
- No plugin can own durable session history. JSONL transcripts remain the
  canonical audit log.

## Transport

- Transport: local child process with stdin/stdout JSONL.
- Encoding: UTF-8 JSON object per line.
- Framing: one request or event per line; no multiline JSON.
- Ordering: messages from one plugin process are ordered by plugin-local
  sequence number.
- Backpressure: core may stop reading after timeout and terminate the process.
- Cancellation: core sends `plugin.cancel`; plugins must stop work and emit a
  final `plugin.finished` or `plugin.error`.

## Handshake

Core starts the process and sends:

```json
{
  "type": "host.hello",
  "protocol_version": "0.1",
  "session_id": "ses-123",
  "workspace": "/repo",
  "runtime_contract": {
    "events": "RuntimeEventKind",
    "commands": "RuntimeCommand"
  }
}
```

The plugin replies:

```json
{
  "type": "plugin.hello",
  "protocol_version": "0.1",
  "plugin_id": "example.lint",
  "display_name": "Example Lint",
  "capabilities": ["tool_provider", "evidence_producer"]
}
```

If protocol versions are incompatible, core emits a recoverable runtime error
and does not expose the plugin.

## Manifest

Each plugin declares a manifest before it can run:

```json
{
  "id": "example.lint",
  "version": "0.1.0",
  "entrypoint": "bin/example-lint",
  "capabilities": [
    {
      "id": "lint.workspace",
      "kind": "tool_provider",
      "mutating": false,
      "requires_approval": false,
      "evidence": "structured"
    }
  ],
  "ui_contributions": [
    {
      "id": "lint.summary",
      "kind": "panel",
      "source": "runtime_event",
      "runtime_event_types": ["evidence_recorded"]
    }
  ]
}
```

UI contributions are declarative metadata only. A TUI or GUI may choose how to
render them, but the plugin cannot mutate UI state directly.

## Runtime Messages

Plugin requests that may affect the project must be represented as core
runtime commands or tool requests:

```json
{
  "type": "plugin.command_request",
  "request_id": "req-1",
  "command": {
    "type": "queue_follow_up",
    "content": "Run lint after current turn"
  }
}
```

Core replies with accepted/rejected runtime events:

```json
{
  "type": "host.runtime_event",
  "event": {
    "sequence": 42,
    "timestamp": 1782963200,
    "kind": {
      "type": "command_accepted",
      "payload": {
        "command_id": "req-1",
        "command": {
          "type": "queue_follow_up",
          "content": "Run lint after current turn"
        }
      }
    }
  }
}
```

Plugins may emit evidence, progress, and diagnostics. Core converts accepted
facts into `RuntimeEventKind::EvidenceRecorded`, `TaskUpdated`, `LaneUpdated`,
or `Error` events.

## Permission and Evidence Rules

- Mutating work is never executed inside the plugin boundary without core
  permission checks.
- Plugin output used for decisions must become runtime evidence with source,
  timestamp, summary, and optional path.
- Secrets must be referenced by environment variable names, not raw values.
- Provider API keys and endpoint changes must use `RuntimeCommand::ConfigureProvider`.
- Plugin failures are recoverable runtime errors unless core cannot preserve
  transcript or permission invariants.

## Test Contract

The protocol is not frozen until tests cover:

- manifest parsing and capability rejection;
- host/plugin handshake;
- command request acceptance and rejection;
- cancellation;
- evidence conversion into runtime events;
- permission denial for mutating plugin requests in plan/review modes;
- parity fixture replay for TUI and GUI clients.

The Phase 2 runtime fixture currently lives at
`robocode-types/tests/fixtures/runtime-contract-phase2.json`.
