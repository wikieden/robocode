# viden-provider

## 目的

`viden-provider` 负责模型 provider 抽象和 provider 协议适配。
它把 provider identity、protocol family、配置优先级和运行时 provider
构造隔离在 `viden-runtime` 之外。

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
- `ModelRequestControl`

## Provider Plugin Runtime

Provider runtime 分为两层：

- 内建 providers 通过稳定 Rust 代码注册，目前包括 Anthropic、OpenAI、
  OpenAI-compatible、Ollama、fallback、DeepSeek，以及 DeepSeek
  Anthropic-compatible。
- 动态 provider plugins 从解析后的 plugin 目录发现。CLI/config 层支持
  `provider_plugin_dirs`、`VIDEN_PROVIDER_PLUGIN_DIRS`，以及可重复的
  `--provider-plugin-dir <dir>`。

动态加载是 descriptor-driven。原生 plugin 暴露
`viden_plugin_descriptor_json` symbol，并返回序列化 provider
descriptor。host 会校验 descriptor，把它与内建 providers 合并，拒绝
provider-id 冲突，并通过已注册的 protocol adapter 创建运行时 provider
实例。这样 plugin boundary 保持为序列化、由 host 中介的边界，而不是暴露不稳定
的 Rust trait-object ABI。

动态 provider 的 API base 按以下优先级解析：

1. 显式 runtime config 或 CLI override。
2. Descriptor `env_mappings.api_base_env`。
3. Descriptor `default_api_base`。

解析后的 API base 必须以 `http://` 或 `https://` 开头。来自环境变量的无效值会让
provider construction 失败，不会继续传给 HTTP transport。

Registry refresh 对调用方保持原子语义：

- `ProviderHost::refresh` 从默认 plugin 目录重建 registry。
- `ProviderHost::refresh_from_dirs` 从显式目录重建 registry。
- `ProviderHost::refresh_diagnostic` 和
  `ProviderHost::refresh_from_dirs_diagnostic` 会为运行时 reload diagnostics
  保留结构化 `ProviderPluginError` 细节。
- refresh 失败时返回错误，并保留之前可用的 registry。
- 已创建的 provider 实例在 refresh 后仍保持独立；新 provider 实例使用刷新后的
  registry。

Plugin loader 失败使用 `ProviderPluginError` 结构化表达，包含 kind、path 和
message。Registry/host API 为兼容性仍返回可读字符串，同时 diagnostic
host/registry constructors 和 refresh API 会保留结构化错误，供 CLI diagnostics
使用。

当前边界：动态 plugins 注册 descriptors，并复用 host 侧 OpenAI 或 Anthropic
protocol adapters。Request execution 已支持共享的 `ModelRequestControl`
取消信号，core/runtime 调用方可以停止 provider dispatch 和正在运行的 HTTP
子进程。同一个 control object 可以请求 streaming-compatible OpenAI/Anthropic
payload，parser 可以把 SSE text/tool-call deltas 归一化回 `ModelEvent`。完整
incremental UI delivery、provider-owned execution、signing、sandboxing、
marketplace/distribution 属于后续 hardening 工作。Parser 也会把 provider
返回的 usage 保留为 `ModelEvent::Usage`，供 core telemetry 使用。

## 不变量

- Core 依赖 `ModelProvider`，不依赖具体 provider。
- 原生 tool calls 归一化为 `ModelEvent::ToolCall`。
- Provider 返回的 token usage 归一化为 `ModelEvent::Usage`；当 provider 没有
  返回 usage 时，parser 不能合成假的 usage。
- HTTP/provider 失败返回错误，不 panic。
- Provider identity 与 protocol family 分离。
- Plugin descriptors 必须先校验，再进入 registry。
- Dynamic provider API base 解析必须保持 explicit-over-env-over-default 优先级。
- Registry refresh 失败时不能静默丢掉之前正常工作的 registry。
- Cancellation 必须在 dispatch 前检查，也必须在 HTTP transport 等待 provider
  子进程时检查。
- Streaming request 必须同时满足 caller preference 和 provider capability support。
- Streaming protocol parsing 必须保持和非 streaming JSON parser 一致的
  text/tool-call 语义。

## `.ref` 对齐

对齐 `.ref` 的 model/tool loop 行为，同时把厂商协议隔离在 core 外。Plugin
runtime 借鉴参考工程的可插拔能力面，不照搬其 JavaScript/Bun 实现细节。

## 测试

```bash
cargo test -p viden-provider
```
