# Process Plugin Protocol 草案

英文版：[process-plugin-protocol.md](process-plugin-protocol.md)

## 目的

Viden 插件扩展 runtime，但不能绕过 core 的 permission、evidence、cost、
transcript 或 task/lane 状态。第一版插件边界采用本地进程协议，通过换行分隔 JSON
通信。后续可以增加 native dynamic loading 或 WASM，但必须保持同一套 capability
和 event model。

这是 Phase 0-2 的 contract 草案。它定义 core、TUI、GUI 分支在 UI 插件面板实现前
可以共享的边界。

## 非目标

- 插件不能直接调用 provider、tool、permission、transcript、workflow、TUI 或 GUI
  内部 API。
- 插件不能在不经过 core 的情况下修改文件、运行命令、更新 memory、改变 task。
  这些动作必须返回 `RuntimeCommand` 或 tool request，由 core 统一处理。
- 插件不能拥有 durable session history。JSONL transcript 仍然是唯一 canonical
  audit log。

## 传输

- 传输：本地 child process，stdin/stdout JSONL。
- 编码：UTF-8，每行一个 JSON object。
- framing：一行一个 request 或 event，不支持 multiline JSON。
- 顺序：同一插件进程内按照插件自己的 sequence number 保序。
- backpressure：core 超时后可以停止读取并终止进程。
- cancel：core 发送 `plugin.cancel`；插件必须停止工作并发送最终
  `plugin.finished` 或 `plugin.error`。

## 握手

core 启动进程后发送：

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

插件回复：

```json
{
  "type": "plugin.hello",
  "protocol_version": "0.1",
  "plugin_id": "example.lint",
  "display_name": "Example Lint",
  "capabilities": ["tool_provider", "evidence_producer"]
}
```

如果协议版本不兼容，core 发出 recoverable runtime error，并且不暴露该插件。

## Manifest

插件运行前必须声明 manifest：

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

UI contribution 只能是声明式 metadata。TUI 或 GUI 可以选择如何渲染，但插件不能直接
修改 UI state。

## Runtime Messages

可能影响项目的插件请求必须表示为 core runtime command 或 tool request：

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

core 以 accepted/rejected runtime event 回复：

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

插件可以发送 evidence、progress 和 diagnostics。core 会把被接受的 facts 转换为
`RuntimeEventKind::EvidenceRecorded`、`TaskUpdated`、`LaneUpdated` 或 `Error`。

## Permission 与 Evidence 规则

- mutating work 不能在插件边界内绕过 core permission check 执行。
- 用于决策的插件输出必须变成 runtime evidence，包含 source、timestamp、summary
  和可选 path。
- secret 必须通过环境变量名引用，不能传原始值。
- provider API key 和 endpoint 变更必须使用 `RuntimeCommand::ConfigureProvider`。
- 插件失败默认是 recoverable runtime error；只有无法保留 transcript 或 permission
  invariant 时才升级为不可恢复错误。

## 测试契约

协议冻结前必须覆盖：

- manifest 解析和 capability rejection；
- host/plugin handshake；
- command request accepted/rejected；
- cancellation；
- evidence 转换为 runtime events；
- plan/review mode 下 mutating plugin request 被 permission denial；
- TUI 和 GUI clients 使用同一 parity fixture replay。

当前 Phase 2 runtime fixture 位于
`robocode-types/tests/fixtures/runtime-contract-phase2.json`。
