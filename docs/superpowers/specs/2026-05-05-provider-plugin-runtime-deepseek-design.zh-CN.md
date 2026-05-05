# Provider Plugin Runtime 与 DeepSeek v4 设计

## 目标

本文档定义 RoboCode 在 provider 平台层的一次新设计切片。直接目标是支持 DeepSeek v4，但真正的设计目标更大：模型层需要从“小型内建 provider factory”升级为“支持插件扩展的 provider runtime”。

这次确认的方向如下：

- RoboCode 必须同时支持 Anthropic/Claude-style 与 OpenAI-style 两类协议族
- DeepSeek 必须作为独立 provider family 被支持，而不是仅作为 OpenAI-compatible endpoint alias
- provider registry 必须支持动态加载
- 首版交付可以先使用原生动态库
- 插件 API 从第一天起就要按未来可迁移到 WASM 的方式设计，避免之后重写 host/provider contract

## 产品目标

RoboCode 应当让“新增模型 provider”这件事长期保持低成本，而不是每次都去改 `SessionEngine`、tool/runtime flow 或其他 core product surface。

完成这一切片后，RoboCode 应该能清晰回答：

- 当前有哪些 provider family 可用
- 哪些是内建的，哪些是动态加载的
- 每个 provider 属于哪种协议族
- provider-specific config 如何解析
- 如何在不修改 core orchestration logic 的情况下增加一个新 provider

DeepSeek v4 是首个实际验收目标，但正式需求是 provider plugin runtime 本身。

## 范围

范围内：

- `robocode-model` 内的 provider plugin host/runtime
- 动态 provider registry
- 内建 providers 作为 registry 的一种来源
- 原生动态库 provider plugin 作为首个动态加载模式
- 一个从第一天就为后续 WASM 迁移预留空间的 provider ABI/contract
- provider-scoped configuration 与 generic fallback
- DeepSeek v4 作为第一个 plugin-backed provider 示例
- 持续支持 Anthropic-style 与 OpenAI-style 两种协议族

首版实现范围外：

- 完整的 WASM runtime 执行 provider plugin
- plugin marketplace / distribution
- plugin signing 与 trust enforcement
- plugin 的 network sandboxing
- 远程安装 UX
- 与此需求无关的 core-engine、session、tool-loop 重设计

## 架构

### 分层模型

provider 系统应拆成五层：

1. `ProviderHost`
2. `ProviderRegistry`
3. `ProviderDescriptor`
4. `ProtocolAdapter`
5. `ProviderPlugin`

这样可以把 provider discovery、metadata、protocol behavior 与 runtime construction 分开，避免新增 provider 时不断侵入 core engine。

### ProviderHost

`ProviderHost` 位于 `robocode-model` 中，负责：

- 加载 built-in providers
- 扫描 plugin 目录
- 加载动态 provider plugins
- 构建内存中的 registry
- 在启动或运行时解析用户选中的 provider

它是宿主端 runtime surface，不负责协议本身的实现。

### ProviderRegistry

`ProviderRegistry` 是 provider 可用性的规范查询面。

职责：

- 保存 built-in provider descriptors
- 保存动态加载的 provider descriptors
- 处理 provider id 冲突
- 按 `provider_id` 提供查询
- 为 CLI/status surface 提供 provider 列表

registry 是正式的产品域对象，而不只是内部 map。

### ProviderDescriptor

`ProviderDescriptor` 是 provider 身份与能力的可序列化声明对象，必须能安全跨越未来的 WASM 边界。

必需字段：

- `provider_id`
- `display_name`
- `version`
- `protocol_family`
- `default_api_base`
- `default_model`
- `env_mappings`
- `capabilities`
- `config_schema_version`

可选字段可包括：

- `docs_url`
- `supports_streaming`
- `supports_native_tool_calling`
- `supports_reasoning_controls`
- `provider_metadata`

### ProtocolAdapter

`ProtocolAdapter` 负责协议族实现。

首批两类协议族：

- `anthropic`
- `openai`

职责：

- 编码 model requests
- 解码流式或批式 responses
- 将 tool-calling 行为归一化为 RoboCode 的 model events
- 归一化 usage reporting
- 归一化 provider errors

多个 provider 可以复用同一个 adapter family。这正是 DeepSeek 能作为独立 provider，同时仍复用 OpenAI-style protocol 的原因。

### ProviderPlugin

`ProviderPlugin` 把 provider 身份与运行时行为绑定起来。

职责：

- 暴露 `ProviderDescriptor`
- 解析 provider-scoped configuration
- 声明自己绑定哪种 protocol adapter
- 执行 provider-specific validation
- 构造具体的 `ModelProvider`

plugin 不允许直接触达 `SessionEngine` 或 transcript logic。它只能通过 model-provider boundary 参与系统运行。

## 动态加载模型

### Registry 来源

registry 必须支持多种来源：

1. built-in providers
2. 从 plugin 目录动态发现的本地 plugins
3. 未来的 remote/package-managed plugin sources

首版实现只要求 1 和 2 可用，但 host API 要按未来支持 3 的方式设计，避免再改一轮 registry contract。

### 运行中 Registry Reload

provider host 必须支持运行中刷新 registry。

Phase 1 的要求：

- 当前进程可以重新扫描 plugin sources 并重建 provider registry
- 新加载出来的 provider 无需重启整个应用，就能被新创建的 provider instance 使用

Phase 1 不要求：

- 自动热替换已经绑定到活跃 session 中的 provider instance
- 隐式把已有 session 迁移到新加载的 provider

这样可以先让运行中动态加载变得可用，而不必一开始就承担已运行对象原地切换的一致性风险。

### 原生动态库

首版动态执行模式使用原生动态库：

- macOS：`.dylib`
- Linux：`.so`
- Windows：`.dll`

loader 应能发现候选文件、尝试加载它们、抽取 descriptor 与 entrypoint surface，并在加载失败时返回结构化错误，而不是让 host 崩溃。

### ABI 边界

ABI 边界不能直接暴露内部 Rust traits，也不能依赖不稳定的 Rust 对象布局。

边界应采用：

- 稳定导出入口函数
- C-compatible 或 byte-serialized payload 边界
- host 侧 Rust wrapper，把 plugin ABI 调用转换成内部 model abstractions

这同时是 native dynamic loading 安全性与未来 WASM 可迁移性的要求。

## WASM 迁移约束

首版虽然可以先以 native library 执行 plugin，但 plugin contract 必须按未来会运行在 WASM 中的方式设计。

这意味着：

- provider descriptors 必须可序列化
- request/response/tool-call payloads 必须可序列化
- plugin entrypoints 不能假设能直接拿到 host 侧 Rust structs
- plugin/host 交互应以 capability 为中心，而不是以 pointer 或 trait-object 为中心

预期的后续演进是：

- 保持相同的 provider host/registry 结构
- 用 WASM runtime 替换或补充 native loader
- 基本复用同一套 descriptor 和消息协议，而不必重新设计

## Provider 绑定模型

provider 选择必须是 instance-scoped，而不是 process-global。

规则：

- 每个 `SessionEngine` 或 agent runtime 都持有自己的 `ModelProvider` instance
- 同一进程中的多个并发 agents 可以同时使用不同 provider
- registry 是共享的 lookup state，但活动中的 provider binding 属于各自 session/agent instance
- 系统不能依赖一个可变的全局 “current provider”

这同时是 multi-agent correctness 与 runtime plugin loading 的要求。  
也就是说，registry reload 可以改变“未来新 session 能选什么”，但不能强制已有 session 立刻切换 provider。

## 协议族要求

RoboCode 必须继续同时支持两类主要协议风格：

- Anthropic/Claude-style
- OpenAI-style

这是硬性架构要求。plugin system 不能把所有厂商都偷偷降格成“OpenAI-compatible”这一种隐式抽象。

规则：

- provider 通过 `protocol_family` 声明协议族
- adapter 负责协议行为
- provider 负责身份、配置与校验
- core engine 最终只看到归一化后的 `ModelEvent`

## DeepSeek v4 要求

DeepSeek 必须作为一等 provider family，至少包含：

- `provider_id = "deepseek"`
- `display_name = "DeepSeek"`
- `protocol_family = "openai"`
- 默认模型目标 `deepseek-v4`

即使 DeepSeek 复用 OpenAI-style adapter，产品层也必须把它视为独立 provider，而不是 `openai` 的一个未文档化 endpoint 变体。

### 配置解析

DeepSeek 配置优先级建议为：

1. provider-scoped 的 DeepSeek config values
2. `DEEPSEEK_API_KEY`
3. `DEEPSEEK_API_BASE`
4. 通用 `api_key` / `api_base`
5. descriptor 内的 provider defaults

也就是说，provider-specific path 优先，generic path 作为兼容回退。

### 兼容规则

RoboCode 仍应允许用户显式把 generic OpenAI-compatible provider 指向 DeepSeek endpoint 并正常工作。  
但这种兼容路径不能替代或弱化 DeepSeek 作为独立 provider 的正式产品面。

## 配置模型

当前 provider 配置模型应升级为三层：

1. generic provider config
2. provider-scoped config
3. plugin-declared config schema

### Generic Config

共享字段：

- `provider`
- `model`
- `api_key`
- `api_base`
- `request_timeout_secs`
- `max_retries`

### Provider-Scoped Config

provider-scoped config 应允许如下字段：

- `providers.deepseek.api_key`
- `providers.deepseek.api_key_env`
- `providers.deepseek.api_base`
- `providers.deepseek.default_model`

其他 provider 同理。

### Plugin-Declared Schema

plugin 应能声明：

- 支持哪些 config keys
- 支持哪些环境变量映射
- 哪些字段是 required / optional
- 默认值与验证规则

host 用这些声明来做配置校验和用户可读错误提示。

## 验收标准

这一需求完成后，应至少满足：

1. RoboCode 能列出 built-in 与 dynamically loaded providers。
2. DeepSeek 可以通过 `provider=deepseek` 被选择。
3. DeepSeek v4 能通过 plugin system 正常构造 provider，而无需改 `SessionEngine`。
4. 当前进程可以在运行中刷新 provider registry，且新加载的 provider 能被新的 provider instance 使用。
5. 多个并发 agents 或 sessions 可以在同一进程中同时使用不同 provider。
6. Anthropic-style 与 OpenAI-style providers 都仍能通过统一 provider contract 正常工作。
7. provider-specific config 优先于 generic fallback config。
8. plugin load failures 以结构化错误呈现，而不是 host crash。
9. 对于复用现有 protocol adapter 的新 provider，不需要 core-engine 改动。
10. plugin contract 不直接把不稳定的 Rust trait ABI 暴露到动态边界之外。

## 风险与约束

### 风险

- native dynamic library ABI 稳定性
- 跨平台加载差异
- plugin trust 与安全边界
- 把 provider identity 与 protocol implementation 绑得过紧

### 约束

- 首版不要求完整 plugin marketplace
- 首版不承诺强沙箱隔离
- 首版必须保持现有 provider 行为继续可用

### 缓解方式

- 使用可序列化/稳定的 plugin 边界
- 将 protocol family 与 provider identity 明确分离
- 把 native loading 视为第一阶段交付，而不是最终安全形态
- 把 plugin contract 设计成 capability-oriented，为后续 WASM 迁移留好接口

## 交付分期

### Phase 1：Provider Plugin Runtime + DeepSeek v4

目标：

- provider host/runtime
- dynamic registry
- native plugin loading
- provider-scoped config
- DeepSeek plugin
- protocol-family binding

### Phase 2：Plugin Hardening

目标：

- signing/trust model
- packaging/distribution
- 更强隔离
- WASM runtime support
- plugin authoring docs/tooling

第一版 implementation plan 应只落地 Phase 1，但架构必须为 Phase 2 留出空间。
