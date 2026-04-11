# RoboCode 架构

英文版： [architecture.md](architecture.md)

## Workspace 布局

- `robocode-cli`：面向用户的 REPL 和 slash commands
- `robocode-config`：配置加载、优先级合并和启动默认值
- `robocode-core`：会话引擎和 turn 编排
- `robocode-model`：provider 抽象、HTTP 适配和 tool-calling 协议转换
- `robocode-tools`：内置本地工具和执行适配器
- `robocode-permissions`：权限模式、规则和审批决策
- `robocode-session`：JSONL transcript 和 SQLite 索引
- `robocode-types`：共享领域类型

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

## Provider 抽象

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
- `ollama`
- `fallback`

HTTP provider 使用系统 `curl`，因此 workspace 能保持依赖轻量且可离线编译。provider 配置中也包含 timeout 和 retry，HTTP 路径会对瞬时失败做重试，并返回结构化错误。

当前协议支持：

- Anthropic 原生 `tool_use`
- OpenAI 原生 `tool_calls`
- OpenAI-compatible 的相同工具调用消息形状
- Ollama 的纯文本聊天流
- 本地 `fallback` 行为，用于离线与 smoke test

即使没有配置凭证，RoboCode 仍然可以通过 deterministic fallback 启动，而不是直接失败。

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
- `/git ...`
- `/web ...`

## 平台说明

RoboCode 在不同平台上共用同一套 engine，只在必要处切换执行适配器：

- macOS / Linux 使用 POSIX shell adapter
- Windows 使用 PowerShell adapter

目标是保证工具契约层面的行为一致，而不是强行让所有系统拥有完全相同的 shell 语法。
