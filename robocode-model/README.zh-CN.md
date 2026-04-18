# robocode-model

## 目的

`robocode-model` 负责模型 provider 抽象和 provider 协议适配。

## 不负责

- Session 编排。
- 工具执行。
- 权限提示。
- Transcript 持久化。

## 公共接口

- `ModelProvider`
- `ProviderKind`
- `ProviderConfig`
- `create_provider`

## 不变量

- Core 依赖 `ModelProvider`，不依赖具体 provider。
- 原生 tool calls 归一化为 `ModelEvent::ToolCall`。
- HTTP/provider 失败返回错误，不 panic。

## `.ref` 对齐

对齐 `.ref` 的 model/tool loop 行为，同时把厂商协议隔离在 core 外。

## 测试

```bash
cargo test -p robocode-model
```
