# V2 LSP Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add read-only semantic code intelligence through language-server integration without bypassing RoboCode's existing session, permission, transcript, and tool-runtime invariants.

**Architecture:** Add a new `robocode-lsp` crate that owns LSP configuration, JSON-RPC framing, process lifecycle, and semantic query normalization. `robocode-core` exposes LSP slash commands and wires an optional semantic provider into tool execution. `robocode-tools` adds read-only `lsp_diagnostics`, `lsp_symbols`, and `lsp_references` tools through the same `ToolRegistry` and `ToolExecutionContext` path as all other tools.

**Tech Stack:** Rust 2024 workspace, `serde`, `serde_json`, standard-library process and pipe handling, existing RoboCode crates

---

## Current Baseline

V1 and V2-A are implemented. V2-C memory/task workflows are active on `codex/v2-memory-task-workflows` and should be published or merged before this plan starts. The root workspace already contains `robocode-cli`, `robocode-config`, `robocode-core`, `robocode-model`, `robocode-permissions`, `robocode-session`, `robocode-tools`, `robocode-types`, and `robocode-workflows`.

## Scope

In scope:

- new `robocode-lsp` crate
- read-only LSP server configuration and lifecycle
- JSON-RPC/LSP message framing tests
- normalized semantic result types in `robocode-types`
- `/lsp`, `/lsp status`, `/lsp diagnostics`, `/lsp symbols`, and `/lsp references`
- read-only model tools: `lsp_diagnostics`, `lsp_symbols`, `lsp_references`
- transcript-visible command and tool results

Out of scope:

- code actions and automatic fixes
- rename/refactor operations
- write-capable LSP requests
- remote language servers
- project-wide background indexing beyond server startup and explicit queries
- full TUI views for diagnostics or symbol trees

## Target Behaviors

- A user can run `/lsp status` and see configured language servers and runtime state.
- A user can run `/lsp diagnostics <path>` and receive diagnostics for an opened file.
- A user can run `/lsp symbols <path>` and receive document symbols from the language server.
- A user can run `/lsp references <path> <line> <character>` and receive reference locations.
- The model can call read-only `lsp_*` tools and receive serializable results through the same tool loop as grep/glob/read tools.
- No LSP action mutates files or shell state in this V2-B slice.
- If a language server binary is missing, commands fail with actionable messages and do not panic.

## File Map

Create:

- `robocode-lsp/Cargo.toml`
- `robocode-lsp/README.md`
- `robocode-lsp/README.zh-CN.md`
- `robocode-lsp/src/lib.rs`
- `robocode-lsp/src/config.rs`
- `robocode-lsp/src/framing.rs`
- `robocode-lsp/src/protocol.rs`
- `robocode-lsp/src/runtime.rs`

Modify:

- `Cargo.toml`
- `Cargo.lock`
- `robocode-types/src/lib.rs`
- `robocode-tools/src/lib.rs`
- `robocode-core/Cargo.toml`
- `robocode-core/src/lib.rs`
- `docs/modules.md`
- `docs/modules.zh-CN.md`
- `PLAN.md`

## Task 0: Finish V2-C Branch Checkpoint

Files:

- No source edits unless verification exposes a defect.

- [ ] Step 1: Confirm the active branch and unpublished commits.

Run:

```bash
git status --short --branch
git log --oneline --decorate -5
```

Expected:

```text
## codex/v2-memory-task-workflows...origin/codex/v2-memory-task-workflows [ahead 3]
ee6307c (HEAD -> codex/v2-memory-task-workflows) Align PRD and gap matrix with V2 workflows
```

- [ ] Step 2: Run workspace verification.

Run:

```bash
cargo test --workspace --quiet
```

Expected: exit code `0`.

- [ ] Step 3: Publish or merge the branch before starting LSP implementation.

Preferred publish path:

```bash
git push -u origin codex/v2-memory-task-workflows
```

Expected: remote branch updated with commits through `ee6307c` or newer.

## Task 1: Add LSP Crate Skeleton and Shared Types

Files:

- Modify: `Cargo.toml`
- Create: `robocode-lsp/Cargo.toml`
- Create: `robocode-lsp/src/lib.rs`
- Modify: `robocode-types/src/lib.rs`

- [ ] Step 1: Add the new crate to the workspace.

Change `Cargo.toml` members to include:

```toml
    "robocode-lsp",
```

- [ ] Step 2: Create `robocode-lsp/Cargo.toml`.

Use this crate manifest:

```toml
[package]
name = "robocode-lsp"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
robocode-types = { path = "../robocode-types" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] Step 3: Create `robocode-lsp/src/lib.rs`.

Use module exports:

```rust
pub mod config;
pub mod framing;
pub mod protocol;
pub mod runtime;

pub use config::{LspServerConfig, LspServerRegistry};
pub use runtime::{LspRuntime, LspRuntimeStatus, SemanticProvider};
```

- [ ] Step 4: Add semantic shared types in `robocode-types/src/lib.rs`.

Define:

```rust
pub type LspServerId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspLocation {
    pub path: String,
    pub range: LspRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDiagnostic {
    pub path: String,
    pub range: LspRange,
    pub severity: Option<u8>,
    pub source: Option<String>,
    pub code: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspSymbol {
    pub name: String,
    pub kind: u32,
    pub path: String,
    pub range: LspRange,
    pub selection_range: Option<LspRange>,
    pub container_name: Option<String>,
}
```

- [ ] Step 5: Add serde roundtrip tests for the new types.

Add a test named `lsp_diagnostic_roundtrips_json` under the existing `robocode-types` tests:

```rust
#[test]
fn lsp_diagnostic_roundtrips_json() {
    let diagnostic = LspDiagnostic {
        path: "src/lib.rs".to_string(),
        range: LspRange {
            start: LspPosition { line: 1, character: 2 },
            end: LspPosition { line: 1, character: 5 },
        },
        severity: Some(2),
        source: Some("rust-analyzer".to_string()),
        code: Some("E0308".to_string()),
        message: "mismatched types".to_string(),
    };

    let encoded = serde_json::to_string(&diagnostic).unwrap();
    let decoded: LspDiagnostic = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, diagnostic);
}
```

- [ ] Step 6: Verify the crate skeleton.

Run:

```bash
cargo test -p robocode-types
cargo test -p robocode-lsp
```

Expected: both commands pass.

## Task 2: Implement LSP Server Configuration

Files:

- Create: `robocode-lsp/src/config.rs`
- Modify: `robocode-lsp/src/lib.rs`

- [ ] Step 1: Write tests for extension-to-server resolution.

Add tests in `config.rs`:

```rust
#[test]
fn registry_resolves_rust_files_to_rust_analyzer() {
    let registry = LspServerRegistry::default();
    let config = registry.for_path(Path::new("robocode-core/src/lib.rs")).unwrap();
    assert_eq!(config.id, "rust-analyzer");
    assert!(config.file_extensions.contains(&"rs".to_string()));
}

#[test]
fn registry_returns_none_for_unknown_extension() {
    let registry = LspServerRegistry::default();
    assert!(registry.for_path(Path::new("README.md")).is_none());
}
```

- [ ] Step 2: Implement `LspServerConfig` and `LspServerRegistry`.

Use this shape:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerConfig {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub file_extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerRegistry {
    servers: Vec<LspServerConfig>,
}
```

Default registry:

```rust
impl Default for LspServerRegistry {
    fn default() -> Self {
        Self {
            servers: vec![LspServerConfig {
                id: "rust-analyzer".to_string(),
                command: "rust-analyzer".to_string(),
                args: Vec::new(),
                file_extensions: vec!["rs".to_string()],
            }],
        }
    }
}
```

- [ ] Step 3: Add lookup helpers.

Implement:

```rust
impl LspServerRegistry {
    pub fn all(&self) -> &[LspServerConfig] {
        &self.servers
    }

    pub fn for_path(&self, path: &Path) -> Option<&LspServerConfig> {
        let ext = path.extension()?.to_string_lossy();
        self.servers
            .iter()
            .find(|server| server.file_extensions.iter().any(|candidate| candidate == ext.as_ref()))
    }
}
```

- [ ] Step 4: Run config tests.

Run:

```bash
cargo test -p robocode-lsp config
```

Expected: tests pass.

## Task 3: Implement JSON-RPC Framing and Protocol Normalization

Files:

- Create: `robocode-lsp/src/framing.rs`
- Create: `robocode-lsp/src/protocol.rs`

- [ ] Step 1: Add framing tests.

Add tests:

```rust
#[test]
fn encodes_content_length_header() {
    let payload = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"});
    let encoded = encode_message(&payload).unwrap();
    assert!(encoded.starts_with(b"Content-Length: "));
    assert!(encoded.windows(4).any(|window| window == b"\r\n\r\n"));
}

#[test]
fn decodes_single_message_from_buffer() {
    let raw = b"Content-Length: 37\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}";
    let decoded = decode_message(raw).unwrap().unwrap();
    assert_eq!(decoded["id"], 1);
}
```

- [ ] Step 2: Implement `encode_message`.

Use:

```rust
pub fn encode_message(value: &serde_json::Value) -> Result<Vec<u8>, String> {
    let body = serde_json::to_vec(value).map_err(|err| err.to_string())?;
    let mut output = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    output.extend(body);
    Ok(output)
}
```

- [ ] Step 3: Implement a conservative `decode_message`.

Behavior:

- return `Ok(None)` when the buffer does not contain a full header and body
- return `Err` for invalid `Content-Length`
- parse the JSON body into `serde_json::Value`

- [ ] Step 4: Add protocol constructors.

In `protocol.rs`, define helpers for:

```rust
pub fn initialize_request(id: u64, root_uri: &str) -> serde_json::Value;
pub fn did_open_text_document(path_uri: &str, language_id: &str, text: &str) -> serde_json::Value;
pub fn document_symbol_request(id: u64, path_uri: &str) -> serde_json::Value;
pub fn references_request(id: u64, path_uri: &str, line: u32, character: u32) -> serde_json::Value;
```

- [ ] Step 5: Run protocol tests.

Run:

```bash
cargo test -p robocode-lsp framing
cargo test -p robocode-lsp protocol
```

Expected: tests pass.

## Task 4: Implement Runtime Facade with Mockable Semantic Provider

Files:

- Create: `robocode-lsp/src/runtime.rs`
- Modify: `robocode-lsp/src/lib.rs`

- [ ] Step 1: Define a read-only provider trait.

Use:

```rust
use std::path::Path;

use robocode_types::{LspDiagnostic, LspLocation, LspPosition, LspSymbol};

pub trait SemanticProvider: Send + Sync {
    fn diagnostics(&self, cwd: &Path, path: &Path) -> Result<Vec<LspDiagnostic>, String>;
    fn symbols(&self, cwd: &Path, path: &Path) -> Result<Vec<LspSymbol>, String>;
    fn references(
        &self,
        cwd: &Path,
        path: &Path,
        position: LspPosition,
    ) -> Result<Vec<LspLocation>, String>;
}
```

- [ ] Step 2: Implement `LspRuntimeStatus`.

Fields:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspRuntimeStatus {
    pub configured_servers: Vec<String>,
    pub running_servers: Vec<String>,
    pub last_error: Option<String>,
}
```

- [ ] Step 3: Implement `LspRuntime`.

Minimum public API:

```rust
impl LspRuntime {
    pub fn new(registry: LspServerRegistry) -> Self;
    pub fn status(&self) -> LspRuntimeStatus;
}
```

Then implement `SemanticProvider` for `LspRuntime`. In the first green implementation, missing or unavailable server processes may return a clear error:

```text
No configured language server for <path>
```

or:

```text
Language server command not found: rust-analyzer
```

- [ ] Step 4: Add runtime tests with missing-server behavior.

Test that a path with no configured server returns a non-panicking error and that `status()` reports `rust-analyzer` as configured.

- [ ] Step 5: Run runtime tests.

Run:

```bash
cargo test -p robocode-lsp runtime
```

Expected: tests pass.

## Task 5: Add Read-Only LSP Tools to the Shared Tool Runtime

Files:

- Modify: `robocode-tools/src/lib.rs`
- Modify: `robocode-core/Cargo.toml`
- Modify: `robocode-core/src/lib.rs`

- [ ] Step 1: Extend `ToolExecutionContext` with an optional semantic provider.

Target shape:

```rust
#[derive(Clone)]
pub struct ToolExecutionContext {
    pub cwd: PathBuf,
    pub semantic: Option<Arc<dyn SemanticToolProvider>>,
}

pub trait SemanticToolProvider: Send + Sync {
    fn diagnostics(&self, cwd: &Path, path: &Path) -> Result<String, String>;
    fn symbols(&self, cwd: &Path, path: &Path) -> Result<String, String>;
    fn references(&self, cwd: &Path, path: &Path, line: u32, character: u32) -> Result<String, String>;
}
```

- [ ] Step 2: Preserve existing tests by updating context builders to pass `semantic: None`.

Search:

```bash
rg -n "ToolExecutionContext" robocode-*
```

Every existing context literal must compile after adding `semantic: None`.

- [ ] Step 3: Register new tools in `ToolRegistry::builtin`.

Add:

```rust
registry.register(LspDiagnosticsTool);
registry.register(LspSymbolsTool);
registry.register(LspReferencesTool);
```

Each `ToolSpec` must set `is_mutating: false`.

- [ ] Step 4: Implement the tools as provider pass-throughs.

Expected input schemas:

- `lsp_diagnostics`: `path=file`
- `lsp_symbols`: `path=file`
- `lsp_references`: `path=file line=0 character=0`

When `semantic` is `None`, return:

```text
LSP semantic provider is not available
```

- [ ] Step 5: Add tool tests.

Cover:

- all three tool specs are non-mutating
- missing semantic provider returns a clean failure
- invalid line/character values return an input validation error
- mock semantic provider output becomes `ToolResult.output`

- [ ] Step 6: Run tool tests.

Run:

```bash
cargo test -p robocode-tools lsp
```

Expected: tests pass.

## Task 6: Add `/lsp` Slash Commands in Core

Files:

- Modify: `robocode-core/Cargo.toml`
- Modify: `robocode-core/src/lib.rs`

- [ ] Step 1: Add `robocode-lsp` as a `robocode-core` dependency.

Use:

```toml
robocode-lsp = { path = "../robocode-lsp" }
```

- [ ] Step 2: Add LSP runtime ownership to `SessionEngine`.

Add a field:

```rust
lsp_runtime: Option<Arc<LspRuntime>>,
```

Initialize it with the default registry unless tests intentionally pass `None`.

- [ ] Step 3: Add command routing.

Support:

```text
/lsp
/lsp status
/lsp diagnostics <path>
/lsp symbols <path>
/lsp references <path> <line> <character>
```

- [ ] Step 4: Keep command behavior transcript-visible.

All `/lsp ...` commands must use the same command event append path used by `/task`, `/memory`, `/sessions`, `/status`, and `/doctor`.

- [ ] Step 5: Add core command tests.

Cover:

- `/help` lists the LSP command family
- `/lsp status` works without spawning a language server
- `/lsp diagnostics README.md` fails cleanly when no server is configured for Markdown
- `/lsp references src/lib.rs abc 1` returns a validation error
- `/lsp` command entries appear in the transcript

- [ ] Step 6: Run core tests.

Run:

```bash
cargo test -p robocode-core lsp
```

Expected: tests pass.

## Task 7: Add Documentation and Module Index Updates

Files:

- Create: `robocode-lsp/README.md`
- Create: `robocode-lsp/README.zh-CN.md`
- Modify: `docs/modules.md`
- Modify: `docs/modules.zh-CN.md`
- Modify: `PLAN.md`

- [ ] Step 1: Add crate README files.

English README must include:

- purpose
- does not own
- main dependencies
- public surface
- runtime invariants
- test command
- `.ref` alignment

Chinese README must mirror the same sections.

- [ ] Step 2: Update module index docs.

Add `robocode-lsp` to:

- workspace dependency map
- data ownership map
- current implementation status
- `.ref` gap summary

- [ ] Step 3: Update `PLAN.md`.

Move V2-B from "next" to "active" after implementation starts, and keep V2-D as the next UI-focused slice.

- [ ] Step 4: Run docs checks.

Run:

```bash
rg -n "robocode-lsp|LSP|semantic|diagnostics|symbols|references" robocode-lsp docs/modules.md docs/modules.zh-CN.md PLAN.md
rg -n "TB[D]|TO[D]O|fi[l]l in|place[h]older" robocode-lsp docs/modules.md docs/modules.zh-CN.md PLAN.md
```

Expected:

- first command finds the expected LSP references
- second command returns no matches

## Task 8: Final Verification and Smoke Test

Files:

- No source edits unless verification exposes a defect.

- [ ] Step 1: Run focused crate tests.

Run:

```bash
cargo test -p robocode-types
cargo test -p robocode-lsp
cargo test -p robocode-tools
cargo test -p robocode-core
```

Expected: all pass.

- [ ] Step 2: Run full workspace verification.

Run:

```bash
cargo test --workspace --quiet
```

Expected: exit code `0`.

- [ ] Step 3: Run manual CLI smoke.

Run:

```bash
cargo run -p robocode-cli -- --provider fallback --model test-local
```

Then enter:

```text
/lsp status
/lsp diagnostics README.md
/lsp symbols robocode-core/src/lib.rs
/lsp references robocode-core/src/lib.rs 0 0
```

Expected:

- status renders configured LSP servers
- Markdown diagnostics fail cleanly because no Markdown server is configured
- Rust semantic commands either return language-server results or a clear missing `rust-analyzer` error
- no command mutates files

## Acceptance Criteria

- `robocode-lsp` exists as a workspace crate with tested config, protocol, framing, and runtime facade modules.
- `robocode-types` owns serializable semantic result types.
- `robocode-tools` exposes read-only `lsp_diagnostics`, `lsp_symbols`, and `lsp_references` through the existing registry.
- `robocode-core` exposes transcript-visible `/lsp` commands.
- LSP failures are clean, actionable, and non-panicking.
- No V2-B behavior bypasses permission, transcript, or shared tool execution paths.

## Risks and Mitigations

- Risk: language-server lifecycle can become a large async subsystem. Mitigation: keep V2-B synchronous and query-driven; defer background indexing.
- Risk: LSP tools could become a parallel tool runtime. Mitigation: wire them through `ToolRegistry` and `ToolExecutionContext`.
- Risk: missing server binaries make tests flaky. Mitigation: unit-test protocol/config/runtime behavior with mocks and treat real server availability as a smoke path.
- Risk: line/character indexing differs by editor expectation. Mitigation: document and implement zero-based LSP positions at the public command boundary.

## Follow-On Work

After this plan lands:

- V2-D can render diagnostics, symbols, and references in structured terminal views.
- Later V2 work can add code actions, rename, and workspace symbols behind explicit permission checks.
- V3 MCP/plugins must reuse semantic results through the same tool and transcript boundaries.
