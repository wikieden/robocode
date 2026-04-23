# robocode-lsp

## 目的

`robocode-lsp` 负责 RoboCode 的 Language Server Protocol 基础能力：server 配置、JSON-RPC framing、协议 request 构造，以及供 core commands 和 tools 使用的只读语义 runtime facade。

## 不负责

- CLI 展示或审批提示。
- Transcript/session 持久化。
- 文件修改、code actions、rename 或 refactor 执行。
- Provider/model 行为。
- 通用 grep、glob 或文件读取工具。

## 主要依赖

- `robocode-types`：可序列化的语义结果契约。
- `serde` 和 `serde_json`：LSP/JSON-RPC payload。

## Public Surface

- `LspServerConfig` 和 `LspServerRegistry` 描述已配置 language servers。
- `encode_message` 和 `decode_message` 实现 LSP `Content-Length` framing。
- `initialize_request`、`did_open_text_document`、`document_symbol_request`、`references_request` 构造协议 payload。
- `LspRuntime` 和 `SemanticProvider` 暴露 diagnostics、symbols、references、status。

## Runtime Invariants

- V2-B LSP 行为保持只读。
- Core 和 tools 必须通过共享 runtime path 调用本 crate；LSP 不能建立平行 command 或 transcript path。
- 缺失或不支持的 language server 返回清晰错误，不能 panic。
- 语义结果类型归 `robocode-types` 所有，便于未来 MCP、plugin、agent flows 复用。

## 测试命令

```bash
cargo test -p robocode-lsp
```

## `.ref` 对齐

参考工程把语义代码智能视为开发辅助能力，而不是 file/search tools 的替代品。RoboCode 采用同样行为：LSP 增强 grep/read/edit 工作流，同时保留 permission、transcript 和 tool-loop 边界。
