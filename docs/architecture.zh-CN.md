# RoboCode 架构

英文版： [architecture.md](architecture.md)

## 目标架构

RoboCode 是 local-first developer agent runtime。CLI 是入口；
`robocode-core` 负责 agent loop；持久化状态、工具、权限、workflow、
LSP 和模型 provider 都通过清晰的子系统边界接入。

```mermaid
flowchart TB
    User["User / Developer"] --> CLI["robocode-cli<br/>REPL / Slash Commands / Terminal Views"]

    CLI --> Core["robocode-core<br/>SessionEngine / Command Router / Agent Loop"]

    Core --> Config["robocode-config<br/>Layered Config / Provider-Scoped Config"]
    Core --> Perm["robocode-permissions<br/>Permission Modes / Approval Gate"]
    Core --> Session["robocode-session<br/>JSONL Transcript / SQLite Index / Resume"]
    Core --> Workflows["robocode-workflows<br/>Tasks / Memory / Resume Context"]
    Core --> Tools["robocode-tools<br/>File / Search / Shell / Web / Git / LSP Tools"]
    Core --> Model["robocode-model<br/>ProviderHost / Registry / Protocol Adapters"]

    Tools --> LSP["robocode-lsp<br/>Diagnostics / Symbols / References"]
    Tools --> LocalOS["Local OS<br/>Filesystem / Shell / Git / Network"]

    Model --> ProviderSDK["robocode-provider-sdk<br/>Plugin ABI / Descriptor Contract"]
    Model --> Builtins["Built-in Providers<br/>Anthropic / OpenAI / Ollama / Fallback"]
    Model --> DeepSeek["robocode-provider-deepseek<br/>DeepSeek Plugin"]

    ProviderSDK --> DynamicPlugins["Dynamic Provider Plugins<br/>Native dylib / so / dll now<br/>WASM later"]

    Builtins --> APIs["Model APIs"]
    DeepSeek --> APIs
    DynamicPlugins --> APIs

    APIs --> Anthropic["Anthropic-style<br/>tool_use"]
    APIs --> OpenAI["OpenAI-style<br/>tool_calls"]
    APIs --> DeepSeekAPI["DeepSeek<br/>deepseek-v4-flash / deepseek-v4-pro<br/>OpenAI + Anthropic endpoints"]
```

## Workspace 布局

- `robocode-cli`：面向用户的 REPL 和 slash commands
- `robocode-config`：配置加载、优先级合并和启动默认值
- `robocode-core`：会话引擎和 turn 编排
- `robocode-model`：provider host/runtime、HTTP 适配、动态 provider registry，以及 tool-calling 协议转换
- `robocode-tools`：内置本地工具和执行适配器
- `robocode-permissions`：权限模式、规则和审批决策
- `robocode-session`：JSONL transcript 和 SQLite 索引
- `robocode-types`：共享领域类型
- `robocode-workflows`：项目级 task、memory、resume-context 与 workflow log 状态
- `robocode-lsp`：language server 配置、协议 framing、语义查询执行和结果归一化

整个 workspace 中，`robocode-session` 的 JSONL transcript 是持久化事实源；SQLite 只是可重建的索引，用来更快地列会话和恢复会话。

## 配置模型

启动配置按照固定优先级解析：

1. CLI flags
2. 环境变量
3. 项目级 `.robocode/config.toml`
4. 全局配置文件
5. 内置默认值

当前已覆盖的配置项：

- provider family
- model name
- API base URL
- API key
- provider-scoped config 与 generic fallback
- permission mode
- session home
- request timeout
- retry count

这样在启动完成后，engine 和 provider 层就不需要再各自做零散的环境变量读取。

## 主执行流程

1. CLI 接收一行用户输入
2. `robocode-core` 判断它是 slash command、直接工具请求，还是普通模型 prompt
3. 普通 prompt 会写入 transcript 并交给 model provider
4. provider 返回 assistant 文本和/或 tool calls
5. assistant 的 tool call 会先写入内存中的会话状态
6. 工具调用交给 permission engine 判定
7. 如果需要审批，CLI 提示用户并把决策回传给 engine
8. 工具通过统一 registry 执行
9. 工具结果写入 transcript，并重新注入到会话历史
10. 引擎循环执行，直到 provider 完成本轮

这个流程保证所有工具调用都走同一条主路径：校验、权限决策、执行、transcript 记录、模型回注。

## 终端展示

`robocode-core` 负责 plain-text terminal presentation helpers，让 slash-command
views 保持一致，而不要求立即引入 full-screen TUI。当前 structured views 覆盖：

- LSP diagnostics、symbols、references
- session 列表
- project tasks 和 memory
- 阻断执行的 permission denials / approval outcomes
- 带 files/additions/deletions 摘要的 `/git diff` 与 `/diff`

renderer 统一管理 section titles、summaries、entry headings、field rows、empty
states 和 diff summaries。后续 TUI 工作应复用这些输出契约，而不是新增第二套 command-result model。

## Transcript Schema

canonical transcript 采用 JSONL。每一行都是一个带类型标签的 `TranscriptEntry`：

- `message`
- `tool_call`
- `tool_result`
- `permission`
- `command`
- `session_meta`

transcript 是 append-only。SQLite 存储派生摘要，始终可以从 JSONL 重建。

当前 session 元数据支持：

- 按项目列出会话
- `/sessions` 输出当前仓库的会话
- `/resume latest`
- `/resume #<index>`
- `/resume <session-id-prefix>`

## 权限模型

当前支持的模式：

- `default`
- `acceptEdits`
- `bypassPermissions`
- `dontAsk`
- `plan`

规则分为 allow、deny、ask 三类。additional working directories 可以扩展路径作用域。文件读取和搜索在作用域内可自动允许；变更型操作除非模式或规则允许，否则都要审批。

权限系统里还包含少量特例。例如 Git worktree 可能会操作仓库根目录之外的路径，因此这些路径会进入审批，而不是直接被视为 out-of-scope deny。

## Provider Runtime

模型层暴露一个 provider trait，接收：

- session id
- 当前 model 名称
- 会话消息
- tool specs
- 当前 permission mode

provider 返回流式或批式事件：

- assistant text
- tool calls
- end-of-turn

当前 V1 已有的 provider family：

- `anthropic`
- `openai`
- `openai-compatible`
- `deepseek`，作为独立 provider family，使用官方 OpenAI-style API surface
- `deepseek-anthropic`，用于 DeepSeek 官方 Anthropic-compatible API surface
- OpenAI-compatible gateway descriptors：`openrouter`、`groq`、`mistral`、`together`、`kimi`、`qwen`、`zhipu`、`volcengine`
- `ollama`
- `fallback`

main 上的 provider runtime 已经从小型 built-in factory 演进为 provider host/runtime，包含：

- built-in provider descriptors
- dynamic provider registry
- built-in descriptors 的兼容矩阵，包括协议族、默认模型、streaming capability 和 tool-call capability
- 将 protocol adapters 与 provider identity 分离
- 先支持 native dynamic loading、后续可迁移到 WASM 的 plugin contract
- instance-scoped provider binding，使同一进程中的不同 sessions/agents 能并发使用不同 provider

HTTP provider 使用系统 `curl`，因此 workspace 能保持依赖轻量且可离线编译。provider 配置中也包含 timeout 和 retry，HTTP 路径会对瞬时失败做重试，并返回结构化错误。

当前协议支持：

- Anthropic 原生 `tool_use`
- OpenAI 原生 `tool_calls`
- OpenAI-compatible 的相同工具调用消息形状
- 通过 descriptor-backed HTTP provider 复用 OpenAI-compatible gateway providers
- DeepSeek 作为独立 provider identity，提供：
  - `deepseek`：绑定 OpenAI-style adapter family，endpoint 为 `https://api.deepseek.com`
  - `deepseek-anthropic`：绑定 Anthropic-style adapter family，endpoint 为 `https://api.deepseek.com/anthropic`
- DeepSeek V4 默认使用 `deepseek-v4-flash`；可显式选择 `deepseek-v4-pro`
- Ollama 的纯文本聊天流
- 本地 `fallback` 行为，用于离线与 smoke test

即使没有配置凭证，RoboCode 仍然可以通过 deterministic fallback 启动，而不是直接失败。

运行时 provider 加载目标：

- 进程运行中可刷新 registry
- 新加载的 provider 可被新建的 provider instances 使用
- 活跃 session 保持自己已经绑定的 provider instance，而不是原地热替换
- built-in 与动态发现的 descriptors 已经进入同一个 registry，但完整 plugin-backed execution、streaming、cancellation 与更广 provider 兼容性仍在继续加固

### Provider Plugin Runtime

provider runtime 把 provider identity 和 protocol behavior 分离。registry
回答“有哪些 provider 可用”；host 为每个 session 或 agent 创建
instance-scoped provider。

```mermaid
flowchart TB
    Core["robocode-core<br/>SessionEngine / Agent Runtime"] --> Host["robocode-model::ProviderHost"]

    Host --> Registry["ProviderRegistry<br/>provider lookup / reload / collision checks"]
    Host --> Factory["Provider Factory<br/>per-session provider instances"]

    Registry --> Builtin["Built-in descriptors<br/>anthropic / openai / ollama / fallback / deepseek"]
    Registry --> PluginLoader["Dynamic Plugin Loader<br/>scan plugin dirs"]
    PluginLoader --> NativeLib["Native plugins<br/>dylib / so / dll"]
    NativeLib --> Descriptor["PluginDescriptor JSON<br/>stable ABI boundary"]

    Factory --> AdapterChoice["Protocol Adapter Binding"]
    AdapterChoice --> AnthropicAdapter["Anthropic-style adapter<br/>tool_use"]
    AdapterChoice --> OpenAIAdapter["OpenAI-style adapter<br/>tool_calls"]

    Builtin --> DeepSeekOpenAI["deepseek<br/>OpenAI-style<br/>https://api.deepseek.com"]
    Builtin --> DeepSeekAnthropic["deepseek-anthropic<br/>Anthropic-style<br/>https://api.deepseek.com/anthropic"]

    OpenAIAdapter --> APIs["External Model APIs"]
    AnthropicAdapter --> APIs

    APIs --> DeepSeekAPI["DeepSeek<br/>deepseek-v4-flash / deepseek-v4-pro"]
    APIs --> OpenAIAPI["OpenAI / compatible"]
    APIs --> AnthropicAPI["Anthropic / compatible"]
```

## 工具系统

当前内置工具：

- `shell`
- `read_file`
- `write_file`
- `edit_file`
- `glob`
- `grep`
- `web_search`
- `web_fetch`
- `git_status`
- `git_diff`
- `git_branch`
- `git_switch`
- `git_add`
- `git_commit`
- `git_push`
- `git_restore`
- `git_stash_list`
- `git_stash_push`
- `git_stash_pop`
- `git_stash_drop`
- `git_worktree_list`
- `git_worktree_add`
- `git_worktree_remove`
- `lsp_diagnostics`
- `lsp_symbols`
- `lsp_references`

每个工具都定义：

- metadata
- mutability
- schema hint
- execution logic

所有内置工具都返回可序列化结果，因此它们的行为可以完整进入 transcript。

CLI 当前也通过 slash commands 暴露这些工具面：

- `/help`
- `/model`
- `/provider`
- `/permissions`
- `/plan`
- `/sessions`
- `/resume`
- `/diff`
- `/test <command>`
- `/git ...`
- `/web ...`
- `/tasks`
- `/task ...`
- `/memory ...`
- `/lsp ...`

当前 workflow / LSP 说明：

- `robocode-workflows` 把 task / memory state 放在 canonical transcript 之外，但仍保持 JSONL event logs 可重建。
- `/test <command>` 复用 shell tool 的权限路径，并把最近一次 test evidence
  存在 `SessionEngine` 中，让 `/status` 能报告最新 verification command、
  exit code、可能失败文件数量和 output tail，而不引入第二条执行通道。命令输出还
  包含一个小 parser，用于提取常见 Rust/cargo 和 pytest failure-summary / file
  模式。
- 成功的 `write_file` 和 `edit_file` result 会结构化为 `path`、`size` 和 `effect`
  行，让 transcript 和 TUI surface 不必解析自由文本也能总结文件变更。
- `robocode-lsp` 当前通过 language-server stdio sessions 提供 query-driven 的 semantic code intelligence。
- 当前 LSP runtime 已覆盖 real queries、session reuse、document synchronization 和 normalized output，但仍属于 early implementation，而不是完整成熟的长期 LSP 平台层。

## 平台说明

RoboCode 在不同平台上共用同一套 engine，只在必要处切换执行适配器：

- macOS / Linux 使用 POSIX shell adapter
- Windows 使用 PowerShell adapter

目标是保证工具契约层面的行为一致，而不是强行让所有系统拥有完全相同的 shell 语法。
