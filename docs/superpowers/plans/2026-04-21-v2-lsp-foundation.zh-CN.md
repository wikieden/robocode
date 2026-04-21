# V2 LSP 基础能力实现计划

英文版： [2026-04-21-v2-lsp-foundation.md](2026-04-21-v2-lsp-foundation.md)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 通过 Language Server 引入只读语义代码智能，同时不绕过 RoboCode 既有的 session、permission、transcript 和 tool runtime 不变量。

**Architecture:** 新增 `robocode-lsp` crate，负责 LSP 配置、JSON-RPC framing、进程生命周期和语义查询归一化。`robocode-core` 暴露 LSP slash commands，并把可选 semantic provider 接入工具执行上下文。`robocode-tools` 通过现有 `ToolRegistry` 和 `ToolExecutionContext` 增加只读 `lsp_diagnostics`、`lsp_symbols`、`lsp_references` 工具。

**Tech Stack:** Rust 2024 workspace、`serde`、`serde_json`、标准库 process/pipe、现有 RoboCode crates

---

## 当前基线

V1 和 V2-A 已实现。V2-C memory/task workflows 正在 `codex/v2-memory-task-workflows` 上推进，开始本计划前应先发布或合并该分支。当前 workspace 已包含 `robocode-cli`、`robocode-config`、`robocode-core`、`robocode-model`、`robocode-permissions`、`robocode-session`、`robocode-tools`、`robocode-types`、`robocode-workflows`。

## 范围

包含：

- 新建 `robocode-lsp` crate
- 只读 LSP server 配置和生命周期
- JSON-RPC/LSP message framing 测试
- `robocode-types` 中的语义结果类型
- `/lsp`、`/lsp status`、`/lsp diagnostics`、`/lsp symbols`、`/lsp references`
- 只读模型工具：`lsp_diagnostics`、`lsp_symbols`、`lsp_references`
- transcript 可追踪的命令和工具结果

不包含：

- code actions 和自动修复
- rename/refactor 操作
- 可写 LSP requests
- remote language servers
- explicit query 之外的 project-wide background indexing
- diagnostics 或 symbols 的完整 TUI 视图

## 目标行为

- 用户可以执行 `/lsp status` 查看已配置语言服务器和运行状态。
- 用户可以执行 `/lsp diagnostics <path>` 获取已打开文件的 diagnostics。
- 用户可以执行 `/lsp symbols <path>` 获取 document symbols。
- 用户可以执行 `/lsp references <path> <line> <character>` 获取 reference locations。
- 模型可以调用只读 `lsp_*` 工具，并通过现有 tool loop 获得可序列化结果。
- V2-B 不允许任何 LSP 行为修改文件或 shell 状态。
- language server binary 缺失时，命令给出可执行错误信息且不 panic。

## 文件地图

新建：

- `robocode-lsp/Cargo.toml`
- `robocode-lsp/README.md`
- `robocode-lsp/README.zh-CN.md`
- `robocode-lsp/src/lib.rs`
- `robocode-lsp/src/config.rs`
- `robocode-lsp/src/framing.rs`
- `robocode-lsp/src/protocol.rs`
- `robocode-lsp/src/runtime.rs`

修改：

- `Cargo.toml`
- `Cargo.lock`
- `robocode-types/src/lib.rs`
- `robocode-tools/src/lib.rs`
- `robocode-core/Cargo.toml`
- `robocode-core/src/lib.rs`
- `docs/modules.md`
- `docs/modules.zh-CN.md`
- `PLAN.md`

## Task 0：完成 V2-C 分支检查点

文件：

- 除非验证暴露缺陷，否则不改源码。

- [ ] Step 1：确认当前分支和未推送提交。

运行：

```bash
git status --short --branch
git log --oneline --decorate -5
```

期望：

```text
## codex/v2-memory-task-workflows...origin/codex/v2-memory-task-workflows [ahead 3]
ee6307c (HEAD -> codex/v2-memory-task-workflows) Align PRD and gap matrix with V2 workflows
```

- [ ] Step 2：运行 workspace 验证。

运行：

```bash
cargo test --workspace --quiet
```

期望：退出码为 `0`。

- [ ] Step 3：开始 LSP 实现前先发布或合并当前分支。

推荐发布路径：

```bash
git push -u origin codex/v2-memory-task-workflows
```

期望：远端分支更新到 `ee6307c` 或更新提交。

## Task 1：新增 LSP crate skeleton 和共享类型

文件：

- 修改：`Cargo.toml`
- 新建：`robocode-lsp/Cargo.toml`
- 新建：`robocode-lsp/src/lib.rs`
- 修改：`robocode-types/src/lib.rs`

- [ ] Step 1：把新 crate 加入 workspace。

在 `Cargo.toml` members 中加入：

```toml
    "robocode-lsp",
```

- [ ] Step 2：创建 `robocode-lsp/Cargo.toml`。

使用：

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

- [ ] Step 3：创建 `robocode-lsp/src/lib.rs`。

使用：

```rust
pub mod config;
pub mod framing;
pub mod protocol;
pub mod runtime;

pub use config::{LspServerConfig, LspServerRegistry};
pub use runtime::{LspRuntime, LspRuntimeStatus, SemanticProvider};
```

- [ ] Step 4：在 `robocode-types/src/lib.rs` 中增加语义共享类型。

定义：

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

- [ ] Step 5：增加 serde roundtrip 测试。

在 `robocode-types` 现有 tests 中添加 `lsp_diagnostic_roundtrips_json`：

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

- [ ] Step 6：验证 crate skeleton。

运行：

```bash
cargo test -p robocode-types
cargo test -p robocode-lsp
```

期望：两个命令都通过。

## Task 2：实现 LSP server 配置

文件：

- 新建：`robocode-lsp/src/config.rs`
- 修改：`robocode-lsp/src/lib.rs`

- [ ] Step 1：先写 extension-to-server 解析测试。

在 `config.rs` 中添加：

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

- [ ] Step 2：实现 `LspServerConfig` 和 `LspServerRegistry`。

目标结构：

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

默认 registry：

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

- [ ] Step 3：增加 lookup helpers。

实现：

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

- [ ] Step 4：运行配置测试。

运行：

```bash
cargo test -p robocode-lsp config
```

期望：测试通过。

## Task 3：实现 JSON-RPC framing 和协议归一化

文件：

- 新建：`robocode-lsp/src/framing.rs`
- 新建：`robocode-lsp/src/protocol.rs`

- [ ] Step 1：增加 framing 测试。

添加：

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

- [ ] Step 2：实现 `encode_message`。

使用：

```rust
pub fn encode_message(value: &serde_json::Value) -> Result<Vec<u8>, String> {
    let body = serde_json::to_vec(value).map_err(|err| err.to_string())?;
    let mut output = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    output.extend(body);
    Ok(output)
}
```

- [ ] Step 3：实现保守的 `decode_message`。

行为：

- buffer 不包含完整 header/body 时返回 `Ok(None)`
- `Content-Length` 非法时返回 `Err`
- JSON body 解析为 `serde_json::Value`

- [ ] Step 4：增加 protocol constructors。

在 `protocol.rs` 定义：

```rust
pub fn initialize_request(id: u64, root_uri: &str) -> serde_json::Value;
pub fn did_open_text_document(path_uri: &str, language_id: &str, text: &str) -> serde_json::Value;
pub fn document_symbol_request(id: u64, path_uri: &str) -> serde_json::Value;
pub fn references_request(id: u64, path_uri: &str, line: u32, character: u32) -> serde_json::Value;
```

- [ ] Step 5：运行协议测试。

运行：

```bash
cargo test -p robocode-lsp framing
cargo test -p robocode-lsp protocol
```

期望：测试通过。

## Task 4：实现可 mock 的 runtime facade

文件：

- 新建：`robocode-lsp/src/runtime.rs`
- 修改：`robocode-lsp/src/lib.rs`

- [ ] Step 1：定义只读 provider trait。

使用：

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

- [ ] Step 2：实现 `LspRuntimeStatus`。

字段：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspRuntimeStatus {
    pub configured_servers: Vec<String>,
    pub running_servers: Vec<String>,
    pub last_error: Option<String>,
}
```

- [ ] Step 3：实现 `LspRuntime`。

最小 public API：

```rust
impl LspRuntime {
    pub fn new(registry: LspServerRegistry) -> Self;
    pub fn status(&self) -> LspRuntimeStatus;
}
```

随后为 `LspRuntime` 实现 `SemanticProvider`。第一版可对缺失或不可用进程返回清晰错误：

```text
No configured language server for <path>
```

或：

```text
Language server command not found: rust-analyzer
```

- [ ] Step 4：增加 runtime 测试。

覆盖：没有配置 server 的 path 返回非 panic 错误，`status()` 报告 `rust-analyzer` 已配置。

- [ ] Step 5：运行 runtime 测试。

运行：

```bash
cargo test -p robocode-lsp runtime
```

期望：测试通过。

## Task 5：把只读 LSP 工具接入共享 tool runtime

文件：

- 修改：`robocode-tools/src/lib.rs`
- 修改：`robocode-core/Cargo.toml`
- 修改：`robocode-core/src/lib.rs`

- [ ] Step 1：扩展 `ToolExecutionContext`，加入可选 semantic provider。

目标形状：

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

- [ ] Step 2：更新现有 context 构造，统一传 `semantic: None`。

搜索：

```bash
rg -n "ToolExecutionContext" robocode-*
```

所有现有 context literal 都必须在新增 `semantic: None` 后编译。

- [ ] Step 3：在 `ToolRegistry::builtin` 注册新工具。

加入：

```rust
registry.register(LspDiagnosticsTool);
registry.register(LspSymbolsTool);
registry.register(LspReferencesTool);
```

每个 `ToolSpec` 必须设置 `is_mutating: false`。

- [ ] Step 4：实现 provider pass-through。

输入 schema：

- `lsp_diagnostics`: `path=file`
- `lsp_symbols`: `path=file`
- `lsp_references`: `path=file line=0 character=0`

当 `semantic` 为 `None` 时返回：

```text
LSP semantic provider is not available
```

- [ ] Step 5：增加工具测试。

覆盖：

- 三个工具 spec 均为非 mutating
- 缺失 semantic provider 时返回清晰失败
- 非法 line/character 返回 input validation error
- mock semantic provider output 写入 `ToolResult.output`

- [ ] Step 6：运行工具测试。

运行：

```bash
cargo test -p robocode-tools lsp
```

期望：测试通过。

## Task 6：在 core 中增加 `/lsp` 命令族

文件：

- 修改：`robocode-core/Cargo.toml`
- 修改：`robocode-core/src/lib.rs`

- [ ] Step 1：为 `robocode-core` 增加 `robocode-lsp` 依赖。

使用：

```toml
robocode-lsp = { path = "../robocode-lsp" }
```

- [ ] Step 2：让 `SessionEngine` 持有 LSP runtime。

增加字段：

```rust
lsp_runtime: Option<Arc<LspRuntime>>,
```

默认使用 `LspServerRegistry::default()` 初始化，测试可显式传 `None`。

- [ ] Step 3：增加命令路由。

支持：

```text
/lsp
/lsp status
/lsp diagnostics <path>
/lsp symbols <path>
/lsp references <path> <line> <character>
```

- [ ] Step 4：保持命令 transcript-visible。

所有 `/lsp ...` 命令必须使用 `/task`、`/memory`、`/sessions`、`/status`、`/doctor` 同一套 command event append path。

- [ ] Step 5：增加 core 命令测试。

覆盖：

- `/help` 列出 LSP 命令族
- `/lsp status` 不需要启动 language server 也能工作
- `/lsp diagnostics README.md` 在没有 Markdown server 时清晰失败
- `/lsp references src/lib.rs abc 1` 返回 validation error
- `/lsp` command entries 写入 transcript

- [ ] Step 6：运行 core 测试。

运行：

```bash
cargo test -p robocode-core lsp
```

期望：测试通过。

## Task 7：补文档和模块索引

文件：

- 新建：`robocode-lsp/README.md`
- 新建：`robocode-lsp/README.zh-CN.md`
- 修改：`docs/modules.md`
- 修改：`docs/modules.zh-CN.md`
- 修改：`PLAN.md`

- [ ] Step 1：新增 crate README。

英文 README 必须包含：

- purpose
- does not own
- main dependencies
- public surface
- runtime invariants
- test command
- `.ref` alignment

中文 README 必须镜像同样章节。

- [ ] Step 2：更新 module index。

把 `robocode-lsp` 加入：

- workspace dependency map
- data ownership map
- current implementation status
- `.ref` gap summary

- [ ] Step 3：更新 `PLAN.md`。

实现开始后，把 V2-B 从 next 调整为 active，并保留 V2-D 作为下一项 UI-focused slice。

- [ ] Step 4：运行 docs 检查。

运行：

```bash
rg -n "robocode-lsp|LSP|semantic|diagnostics|symbols|references" robocode-lsp docs/modules.md docs/modules.zh-CN.md PLAN.md
rg -n "TB[D]|TO[D]O|fi[l]l in|place[h]older" robocode-lsp docs/modules.md docs/modules.zh-CN.md PLAN.md
```

期望：

- 第一个命令能找到预期 LSP 引用
- 第二个命令没有匹配

## Task 8：最终验证与 smoke test

文件：

- 除非验证暴露缺陷，否则不改源码。

- [ ] Step 1：运行 focused crate tests。

运行：

```bash
cargo test -p robocode-types
cargo test -p robocode-lsp
cargo test -p robocode-tools
cargo test -p robocode-core
```

期望：全部通过。

- [ ] Step 2：运行 full workspace verification。

运行：

```bash
cargo test --workspace --quiet
```

期望：退出码为 `0`。

- [ ] Step 3：运行手动 CLI smoke。

运行：

```bash
cargo run -p robocode-cli -- --provider fallback --model test-local
```

随后输入：

```text
/lsp status
/lsp diagnostics README.md
/lsp symbols robocode-core/src/lib.rs
/lsp references robocode-core/src/lib.rs 0 0
```

期望：

- status 展示已配置 LSP servers
- Markdown diagnostics 因未配置 Markdown server 清晰失败
- Rust semantic commands 返回 language-server 结果或清晰的缺失 `rust-analyzer` 错误
- 不修改任何文件

## 验收标准

- `robocode-lsp` 作为 workspace crate 存在，并具备已测试的 config、protocol、framing、runtime facade modules。
- `robocode-types` 拥有可序列化 semantic result types。
- `robocode-tools` 通过现有 registry 暴露只读 `lsp_diagnostics`、`lsp_symbols`、`lsp_references`。
- `robocode-core` 暴露 transcript-visible `/lsp` 命令族。
- LSP 失败路径清晰、可执行、且不 panic。
- V2-B 不绕过 permission、transcript 或 shared tool execution paths。

## 风险与缓解

- 风险：language-server lifecycle 变成大型 async subsystem。缓解：V2-B 保持同步和 query-driven，延后 background indexing。
- 风险：LSP tools 形成平行 tool runtime。缓解：强制走 `ToolRegistry` 和 `ToolExecutionContext`。
- 风险：缺失 server binary 导致测试不稳定。缓解：unit tests 使用 mocks 验证 protocol/config/runtime，真实 server 只作为 smoke path。
- 风险：line/character indexing 与编辑器习惯不同。缓解：在公开命令边界明确并实现 zero-based LSP positions。

## 后续工作

本计划完成后：

- V2-D 可以用结构化终端视图展示 diagnostics、symbols、references。
- 后续 V2 可以在明确权限检查后加入 code actions、rename 和 workspace symbols。
- V3 MCP/plugins 必须通过相同 tool 和 transcript 边界复用 semantic results。
