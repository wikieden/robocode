# robocode-model

## 目的

`robocode-model` 负责模型 provider 抽象和 provider 协议适配。
它把 provider identity、protocol family、配置优先级和运行时 provider
构造隔离在 `robocode-core` 之外。

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
- `ProviderHost`
- `ProviderRegistry`
- `ProviderDescriptor`

## Provider Plugin Runtime

Provider runtime 分为两层：

- 内建 providers 通过稳定 Rust 代码注册，目前包括 Anthropic、OpenAI、
  OpenAI-compatible、Ollama、fallback、DeepSeek，以及 DeepSeek
  Anthropic-compatible。
- 动态 provider plugins 从解析后的 plugin 目录发现。CLI/config 层支持
  `provider_plugin_dirs`、`ROBOCODE_PROVIDER_PLUGIN_DIRS`，以及可重复的
  `--provider-plugin-dir <dir>`。

动态加载是 descriptor-driven。原生 plugin 暴露
`robocode_provider_descriptor_json` symbol，并返回序列化 provider
descriptor。host 会校验 descriptor，把它与内建 providers 合并，拒绝
provider-id 冲突，并通过已注册的 protocol adapter 创建运行时 provider
实例。这样 plugin boundary 保持为序列化、由 host 中介的边界，而不是暴露不稳定
的 Rust trait-object ABI。

Registry refresh 对调用方保持原子语义：

- `ProviderHost::refresh` 从默认 plugin 目录重建 registry。
- `ProviderHost::refresh_from_dirs` 从显式目录重建 registry。
- refresh 失败时返回错误，并保留之前可用的 registry。
- 已创建的 provider 实例在 refresh 后仍保持独立；新 provider 实例使用刷新后的
  registry。

当前边界：动态 plugins 注册 descriptors，并复用 host 侧 OpenAI 或 Anthropic
protocol adapters。完整的 plugin-backed request execution、streaming、
cancellation、signing、sandboxing、marketplace/distribution 属于后续
hardening 工作。

## 不变量

- Core 依赖 `ModelProvider`，不依赖具体 provider。
- 原生 tool calls 归一化为 `ModelEvent::ToolCall`。
- HTTP/provider 失败返回错误，不 panic。
- Provider identity 与 protocol family 分离。
- Plugin descriptors 必须先校验，再进入 registry。
- Registry refresh 失败时不能静默丢掉之前正常工作的 registry。

## `.ref` 对齐

对齐 `.ref` 的 model/tool loop 行为，同时把厂商协议隔离在 core 外。Plugin
runtime 借鉴参考工程的可插拔能力面，不照搬其 JavaScript/Bun 实现细节。

## 测试

```bash
cargo test -p robocode-model
```
