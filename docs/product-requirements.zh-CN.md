# RoboCode 产品需求规格

英文版： [product-requirements.md](product-requirements.md)

## 目的

这份文档定义 RoboCode 的完整产品目标：一个基于 Rust、本地优先的 agentic developer CLI，其行为模型来源于 `.ref/claude-code-main`。

RoboCode 不是逐文件移植。它追求的是用户可感知运行模型、命令面和子系统边界上的高相似度，同时允许内部用 Rust 原生方式重构。

## 产品定义

### 定位

RoboCode 是一个本地优先、可扩展的开发者 Agent，运行在终端中，理解当前工作目录，通过权限门控执行工具，持久化会话，并逐步扩展到集成、远程操作和多 Agent 协作。

### 主要用户

- 在本地仓库里使用 AI 辅助开发的个人开发者
- 需要工具执行可审计、工作可恢复的仓库维护者
- 希望从本地 CLI 逐步成长到更丰富集成能力的团队

### 核心用户任务

- 在仓库里读代码、搜代码、改代码、生成代码
- 在审批约束下运行 shell 与 Git 工作流
- 把 Web 上下文检索并注入到会话中
- 在不丢失工具与审批上下文的前提下恢复历史会话
- 在高风险场景下使用只分析或高审批模式
- 后续扩展到 MCP、LSP、remote、多 Agent，而不需要切换产品

### 产品目标

- 在核心运行时行为和子系统形态上对齐参考工程
- 保持工具、审批和会话历史的强审计能力
- 从首个稳定版本起支持跨平台本地开发
- 保持内核足够可扩展，以承载后续集成和高级工作流

### 非目标

- 复刻 Bun、React、Ink 等具体技术实现
- 逐字逐条复制参考工程的所有命令
- 在第一版里交付整个平台
- 在核心工作流成熟前优先构建 analytics 和 growth tooling

## 核心运行模型

### 启动与配置

RoboCode 必须具备确定性的配置优先级模型：

1. CLI flags
2. 环境变量
3. 项目级配置
4. 全局配置
5. 内置默认值

至少要覆盖：

- provider family 和 model
- API base 与 credentials
- permission mode
- session 存储位置
- request timeout 与 retry
- additional working directories
- 未来集成所需的开关项

### 会话模型

session 是交互的持久化单位，负责持有：

- message history
- tool-call history
- permission events
- command events
- session metadata 与 summary 字段
- working directory 与 scope metadata

transcript 是持久化事实源；任何派生索引都必须可以从 transcript 文件重建。

### 消息与工具循环

RoboCode 必须保留参考工程最核心的行为：

- 用户输入进入共享 engine
- slash commands 通过同一运行时域解析，而不是 UI 旁路
- provider 返回 assistant 文本、tool calls 和 turn 完成事件
- tool call 在执行前完成标准化
- 所有工具调用都走统一运行时路径
- tool result 重新注入会话和 transcript
- 循环持续到 provider 完成本轮

必须保持的约束：

- 工具执行不能是 side channel
- 权限必须先判定再执行
- assistant 的 tool-call 意图必须进入 session state
- transcript 顺序必须足以重建会话

### 权限模型

权限是领域概念，而不是单纯的交互 UI 状态。

RoboCode 必须支持与参考工程语义等价的命名模式：

- `default`
- `acceptEdits`
- `bypassPermissions`
- `dontAsk`
- `plan`

权限子系统必须支持：

- allow / deny / ask
- per-session rules
- persisted rules
- tool-scoped rules
- path-scoped rules
- additional working directories
- 对 worktree、remote resource 等跨仓库边界流程的特殊处理

### 会话持久化与恢复

session 层必须提供：

- append-only transcript 存储
- 可重建的二级索引
- 按项目发现会话
- 快速 resume
- 后续更好的 summaries 和浏览能力

### Slash Commands

slash commands 是一等接口层。RoboCode 不需要逐字复制所有参考命令，但必须覆盖相同的行为家族。

必须覆盖的命令族：

- runtime control：help、model/provider、permissions、plan mode
- session control：sessions、resume、diff，后续扩展 share/export
- repository workflows：Git status、branch、diff、add、commit、restore、stash、worktree 等
- environment 和 diagnostics：config、doctor、context、usage/cost、status
- integration management：MCP、plugins、skills、remote、auth
- collaboration 和 workflow：tasks、agents、teams、memory

### Provider 抽象

provider 层必须保持厂商无关。

至少要支持：

- provider family 选择
- model 选择
- timeout 与 retry 策略
- 文本生成
- 原生 tool-calling
- 结构化错误
- 未来跨 provider 的 streaming 和 cancellation

目标支持：

- Anthropic
- OpenAI
- OpenAI-compatible APIs
- Ollama 或等价本地模型后端
- fallback / offline development mode

### 统一工具运行时

工具执行必须成为系统中最稳定的接口边界。

每个工具定义都应包含：

- public name 和 description
- mutating / non-mutating 分类
- input contract
- permission expectation
- execution handler
- 可序列化 result shape

完整产品目标中的最小工具家族：

- shell execution
- file read / write / edit
- codebase search / globbing
- Git workflows
- web search / fetch
- MCP-backed tools
- LSP-backed actions
- 后续的 agent、team、task、remote-trigger tools

## 子系统需求

### CLI / REPL / Slash Commands

目标：
提供默认的本地交互入口。

要求：

- 从一开始就有轻量交互式 REPL
- 后续逐步增强终端 UI
- 可发现的命令面与帮助输出
- 在 provider 和工具变化下仍保持稳定的命令解析
- 高级子系统不可用时的安全降级

阶段优先级：
- V1 核心
- V2 增强 TUI

### 配置系统

目标：
为本地和全局运行行为提供一个稳定、一致的配置入口。

要求：

- 确定性优先级
- 明确的配置 schema
- 兼容优先的默认值
- 环境变量和 CLI override
- 后续配置迁移能力

阶段优先级：
- V1 核心

### Provider 系统

目标：
支持多模型后端，同时不让 core logic 绑定单一厂商。

要求：

- 一致的内部 provider contract
- 厂商协议适配层
- 原生 tool-calling
- retry 和 timeout 策略
- 对弱协议 provider 的兼容路径

阶段优先级：
- V1 核心
- V2 持续增强

### 工具系统

目标：
把所有可行动能力都暴露在统一的权限化运行时之下。

要求：

- 单一 registry 模型
- 一致工具契约
- 可序列化结果
- transcript 可见性
- 后续支持 MCP、plugins、agent-generated tools

阶段优先级：
- V1 核心，持续扩展

### 权限系统

目标：
让工具执行安全、可审计、可策略化。

要求：

- 命名模式
- 明确决策
- 规则持久化
- 路径作用域
- additional directories
- 跨根目录流程的特殊处理
- 后续扩展到 remote 和集成策略

阶段优先级：
- V1 核心
- V2 / V3 继续增强

### Session / Transcript / Resume

目标：
让 session 持久化、可恢复、可检查。

要求：

- append-only transcript
- 可重建索引
- 按项目发现会话
- 快速 resume
- 后续更好的 summaries 和浏览

阶段优先级：
- V1 核心
- V2 增强

### Git Workflows

目标：
在 Agent 内直接支持本地仓库工作流。

要求：

- 查看仓库状态
- stage 与 commit
- restore 与 stash
- worktree 支持
- 更丰富的 diff 和 branch 流程
- 后续扩展 review / PR comment 相关能力

阶段优先级：
- V1 核心
- V2 增强

### Web 工具

目标：
让 Agent 不离开会话即可获取外部上下文。

要求：

- search 和 fetch
- transcript 可见结果
- 大小与作用域控制
- 后续更强的来源处理

阶段优先级：
- V1 核心
- V2 增强

### MCP 系统

目标：
把外部工具生态和结构化远程资源接进同一运行时模型。

要求：

- MCP server 注册与生命周期管理
- MCP tool discovery 和 invocation
- 权限化执行
- session 可见结果
- MCP 管理命令面

阶段优先级：
- V3

### LSP 系统

目标：
在 shell 和 grep 之外增加语义级代码理解。

要求：

- 语言服务器管理
- symbol / reference 级操作
- 和本地工具的协作流程
- 不可用时的平稳降级

阶段优先级：
- V2

### Skills / Plugins

目标：
让可复用工作流和第三方扩展进入系统，而不让 core code 膨胀。

要求：

- skill discovery 与执行模型
- plugin loading 模型
- 本地与远程扩展的信任边界
- 列出和管理扩展的命令面

阶段优先级：
- V3

### 多 Agent / Team / Coordinator

目标：
支持超越单线程对话的委派和协调工作流。

要求：

- agent spawning
- inter-agent messaging
- team-level orchestration
- transcript-aware coordination
- 权限和作用域隔离

阶段优先级：
- V3

### Bridge / Remote / Server Mode

目标：
支持 IDE 连接、远程会话和服务化运行模式。

要求：

- bridge protocol
- remote session transport
- 跨进程 permission callbacks
- server / daemon mode
- 与本地 session 语义保持一致

阶段优先级：
- V3

### Memory / Tasks / Automation / Cron

目标：
支持超过单轮 prompt 生命周期的长期工作流。

要求：

- persistent memory model
- task lifecycle management
- scheduled execution / reminders
- durable 与 session-scoped automation

阶段优先级：
- V2：memory 和 tasks
- V3：automation 和 cron

### Voice

目标：
在确有价值的场景下支持语音交互。

要求：

- voice capture 与 transcription
- voice session state
- 向文本交互平稳回退

阶段优先级：
- 远期

### UI / TUI / Visual Assist

目标：
当更丰富交互能显著改善理解时，逐步超越纯 REPL。

要求：

- 更好的 diff 展示
- session 浏览界面
- 上下文化 permission prompt
- MCP、tasks、memory、remote 等状态的 richer views

阶段优先级：
- V2

### 运营型平台能力

目标：
在产品成熟后支持多环境、多团队、多策略的产品化运行。

要求：

- analytics 和 usage tracking
- feature flags
- managed settings
- policy limits 和 remote governance

阶段优先级：
- 远期

## 外部接口与公开能力面

### 命令面

RoboCode 必须定义稳定的命令家族，而不是临时堆出来的命令集合。完整目标至少覆盖：

- runtime control
- session control
- repository workflows
- diagnostics 和 config
- integrations
- collaboration
- platform administration

### 工具契约

公开工具定义必须暴露：

- 稳定名称
- 清晰能力描述
- declared mutability
- input contract
- permission expectation
- transcript 可存储的 result format

### Provider 配置接口

公开 provider 接口必须允许选择：

- provider family
- model
- endpoint
- credentials
- timeout
- retry settings

### Permission Modes

公开权限面至少暴露：

- `default`
- `acceptEdits`
- `bypassPermissions`
- `dontAsk`
- `plan`

### Session Selectors

公开 session 接口必须支持：

- latest
- list index
- id-prefix
- project scoping

### Working Directory 与 Scope Controls

公开工作区模型必须支持：

- primary working directory
- additional working directories
- Git worktree 流程
- 未来 remote / bridge 提供的 workspace scopes

### 未来集成接口

MCP、remote、多 Agent 这些子系统必须能够插入现有 command、permission、tool、transcript 模型，而不是建立新的平行运行时。

## 非功能需求

- 支持 macOS / Linux / Windows
- 通过 durable transcript 和 rebuildable index 保证 recoverability
- 对工具、权限和命令行为具备审计能力
- provider、tool、plugin、MCP 可扩展
- 具备交互式 CLI 使用所需的性能
- 通过显式审批和 scope-aware execution 保证安全
- 兼容策略上以行为相似度优先，而不是实现相似度

## 验收标准

完整需求集必须能回答：

- RoboCode 最终是什么
- 哪些子系统在正式范围内
- 每个子系统属于哪个阶段
- 每个核心子系统“做到什么算够用”
- 如何在不逐文件移植的前提下保持与 `.ref` 的高相似度
