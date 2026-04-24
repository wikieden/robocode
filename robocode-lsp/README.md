# robocode-lsp

## Purpose

`robocode-lsp` owns RoboCode's Language Server Protocol foundation: server configuration, JSON-RPC framing, protocol request construction, and the read-only semantic runtime facade used by core commands and tools.

## Does Not Own

- CLI rendering or approval prompts.
- Transcript/session persistence.
- File mutation, code actions, rename, or refactor execution.
- Provider/model behavior.
- General grep, glob, or file reading tools.

## Main Dependencies

- `robocode-types` for serializable semantic result contracts.
- `serde` and `serde_json` for LSP/JSON-RPC payloads.

## Public Surface

- `LspServerConfig` and `LspServerRegistry` describe configured language servers.
- `encode_message` and `decode_message` implement LSP `Content-Length` framing.
- `initialize_request`, `did_open_text_document`, `document_symbol_request`, and `references_request` build protocol payloads.
- `LspRuntime` and `SemanticProvider` expose diagnostics, symbols, references, and status.

## Runtime Invariants

- V2-B LSP behavior is read-only.
- Core and tools must call this crate through shared runtime paths; LSP must not create a parallel command or transcript path.
- Missing or unsupported language servers return clean errors rather than panics.
- Semantic result types remain owned by `robocode-types` so future MCP, plugin, and agent flows can reuse them.

## Test Command

```bash
cargo test -p robocode-lsp
```

## `.ref` Alignment

The reference project treats semantic code intelligence as an auxiliary developer capability, not a replacement for local file/search tools. RoboCode follows that behavior: LSP augments grep/read/edit workflows while preserving permission, transcript, and tool-loop boundaries.
