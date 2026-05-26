# RoboCode 0.1.7 计划

英文版： [release-0.1.7-plan.md](release-0.1.7-plan.md)

最后更新：2026-05-26

相关 adapter 记录：
[codex-app-server-adapter.zh-CN.md](codex-app-server-adapter.zh-CN.md)

## 目标

`0.1.7` 的目标是继续优化真实编程体验，把 0.1.6 建立起来的 TUI cockpit、
lane、extension 可见性，推进成可日常使用的多 agent 编排工作台。

版本主题：

```text
0.1.7 = Codex Adapter + Agent Orchestration Backbone
```

核心判断：RoboCode 不是单纯做一个好看的 TUI，也不是只把 Codex、Claude Code、
DeepSeek 等工具拉起来。它要成为一个本地 multi-agent cockpit：用户在主屏输入目标，
RoboCode 能拆分、派发、观察、审批、收敛结果，并让不同 coding agent 通过统一机制
协作。

这一版的一号参考实现是 OpenAI 的 Claude Code Codex 插件：
[`openai/codex-plugin-cc`](https://github.com/openai/codex-plugin-cc)。它证明了
我们要的产品形态：一个主 coding agent 可以通过 plugin/command/subagent surface
调用 Codex，让后台任务可观察，并在不切换工具的情况下查看结果或继续 Codex 工作。
RoboCode 应该把这个模式内化成一等本地 agent adapter，而不是继续把 Codex 当成
普通 terminal command。

## 下一迭代核心：Host-Delegate Agent Bridge

下一轮实现要围绕一个产品闭环推进：

```text
RoboCode host -> delegate agent -> observable job -> evidence -> operator decision
```

在这个闭环里，RoboCode 是 host cockpit，Codex 是第一个 delegate agent。Claude
Code Codex 插件证明了这个形态，但 RoboCode 要把它抽象成可复用机制，后续 Claude
Code、DeepSeek TUI、tmux/PTY agents 和 ACP-compatible agents 都能接进来。

核心设计规则：

- delegate agent 不能只是一个 raw terminal。它需要 descriptor、readiness
  doctor、launch command、job record、event/evidence stream、cancel path，以及
  可选 resume handle。
- 每个 delegate task 都进入同一套 lane lifecycle：queued、running、waiting for
  approval、blocked、done、failed、archived。
- 主 TUI 必须在一个刷新周期内把 active delegate work 显示到 operation center，
  用户不应该猜远端 agent 是 thinking、editing、testing 还是卡住了。
- 结果必须是可操作 evidence，而不是 transcript 装饰：changed files、commands、
  tests、final output、errors 和 thread/session IDs 都应该能通过
  `/agent status`、`/agent result`、`/lane inspect` 和 side-screen evidence panels
  查询。
- write-capable delegate work 必须继续走 RoboCode permissions 和 approval。
  read-only review 可以轻量，但 mutation 不能绕过 shared tool/runtime/transcript
  path。

这个核心的实现优先级：

1. 先补完 Codex job/event adapter，让它能暴露 thread IDs、touched files、
   command/test evidence 和 resume hints。
2. 主屏 operation center 和副屏使用同一套 job/evidence model。
3. 泛化 adapter contract，让 plugin、skill、MCP、tmux/PTY 和 ACP agents 复用同一
   lifecycle，而不是每个命令各自发明 status 格式。
4. 做通一条真实 write-capable delegated task path，并强制显式 approval，再把它
   作为 Claude/DeepSeek/ACP backend 的模板。

## 版本问题陈述

当前体验里最影响继续深入试用的问题有三类：

- 运行状态不够强：用户输入后，主窗口中间需要持续说明现在到底在做什么，例如
  thinking、editing、testing、waiting approval、supervising lanes。
- 扩展系统仍偏只读：plugin、skill、MCP 已有可见性，但还没有形成一个能真正提升
  开发体验的加载、诊断、调用和权限模型。
- 多 agent 还在终端集成阶段：tmux/PTY/template 已能接入外部工具，但后续要向
  Zed ACP 方向扩展，把不同 coding agent 变成统一 adapter 下的一等 lane backend。
- 当前最强的 adapter 目标就是 Codex 本身：Claude Code 的 Codex 插件暴露
  `/codex:review`、`/codex:rescue`、`/codex:status`、`/codex:result`、
  `/codex:cancel` 和 `/codex:setup`，背后有 companion runtime 和 Codex
  app-server integration。RoboCode 应该原生支持同一套 operator loop。

## 版本定义

`0.1.7` 成功的标准不是“跑完以后能看到结果”，而是用户在真实编程过程中就能感觉它
可控、可观察、可接管。用户应该能提交任务、看到主 agent 当前动作、观察副 agent
lane 进度、审批或拒绝修改、运行测试，并且在不离开 cockpit 的情况下理解
extension/MCP 为什么不可用。

硬发布门槛：

- Codex 成为一等 agent backend，具备 setup/doctor、review、task、status、
  result、cancel 和 resume 类流程。
- 主屏有真实 operation center，所有状态都有 runtime evidence 支撑。
- composer、approval overlay、窗口 resize 已稳定到可日常交互。
- side-1 和 side-2 使用真实 lane、test、LSP、MCP、extension、evidence 状态，
  不依赖 preview-only placeholder。
- 外部 agent 使用同一套 lane lifecycle 和 operator decision language。
- ACP 有清楚的 adapter 边界和可工作的 handshake/event-log spike，即使完整 ACP
  编辑闭环仍保持 experimental。
- plugin、skill、MCP、tool、agent 这些 extension kind 有统一 descriptor 形态、
  诊断路径和权限边界。

砍线：

- 如果时间紧，完整 ACP task execution 和自动任务拆分可以继续保持 experimental。
- 不能砍 Codex adapter、live operation center、composer 可用性、lane lifecycle
  和 extension diagnostics。它们是下一阶段编程体验的地基。

## 参考模型：Codex Plugin for Claude Code

RoboCode 要明确学习 `openai/codex-plugin-cc`，但不照搬它的 Node 实现。这个参考设计
有五个值得保留的部分：

- Plugin/command surface：
  `/codex:review`、`/codex:adversarial-review`、`/codex:rescue`、
  `/codex:status`、`/codex:result`、`/codex:cancel` 和 `/codex:setup`。
- Thin local runtime：
  companion script 检查 Codex availability/auth，启动 Codex 工作，保存 job
  records，并渲染 status/result output。
- Protocol-backed integration：
  插件使用本地 `codex` binary 和 Codex app-server，而不是只抓 terminal 文本。
- Background job model：
  长时间 review 和 rescue task 可以在后台继续跑，host tool 后续展示 status 和
  final result。
- Safety posture：
  review 默认 read-only，write-capable rescue 必须显式选择；可选 review gate 要
  可见，因为它可能形成循环并消耗 usage。

RoboCode 对应翻译：

- `/agent doctor codex` 替代 `/codex:setup`。
- `/agent review codex [--base <ref>]` 替代 `/codex:review`。
- `/agent challenge codex ...` 替代 `/codex:adversarial-review`。
- `/agent run codex [--write] <task>` 或 `/lane codex <task>` 替代
  `/codex:rescue`。
- `/agent status`、`/agent result <id>` 和 `/agent cancel <id>` 管理后台 job。
- Codex app-server events 转成 RoboCode lane events、evidence records 和
  side-screen rows。

## 开发里程碑

### M1：Live Cockpit Stability

重点：先把日常交互手感修稳，再加更多编排能力。

- 主屏 operation center。
- composer 高度、闪烁光标、中文输入法位置。
- resize redraw 和边框对齐。
- approval overlay 默认焦点、关闭、确认后的清理。

退出标准：

- 用户可以输入、审批、拒绝、调整窗口、继续对话，不出现视觉漂移或隐藏状态。
- 截图/preview 覆盖 idle、thinking、tool call、approval、test result 状态。

### M2：Evidence-Driven Programming Loop

重点：让 edit/test/review 可见、可操作。

- `/test` evidence model 和 side-2 渲染。
- edit summary 展示文件、增删行、审批状态、写入结果。
- 当前轮变更的 diff/review 入口。
- tool、test、lane、approval 的 recent evidence timeline。

退出标准：

- “生成文件 -> 审批 -> 运行测试 -> 查看结果”能在 TUI 内完成。
- 不翻 raw transcript，也能看到最近失败摘要和相关变更文件。

### M3：Codex Adapter Core

重点：让 Codex 成为 RoboCode 第一个 protocol-backed external coding agent。

- Codex availability/auth doctor。
- Codex app-server process 或 broker 边界。
- Review 和 adversarial-review flows。
- Task/rescue flow，区分 read-only 和 write-capable modes。
- 带 status/result/cancel/resume 的 background job records。
- Codex thread/turn/events 到 RoboCode lane/evidence records 的映射。

退出标准：

- 用户可以从 RoboCode 启动 Codex review、看到进度、获取结果，并在需要时 resume
  Codex session。
- 用户可以把一个明确任务交给 Codex，在 TUI 中把它看作 agent lane，并看到 changed
  files、commands、tests 和 final output evidence。

### M4：Agent Lane Operator Loop

重点：把外部工具变成可监督协作者。

- 统一 lane states。
- `/lane inspect`、`/lane send`、`/lane revise`、`/lane accept`、
  `/lane discard`、`/lane apply` 操作闭环。
- side-1 使用真实 lane evidence，并突出 next action。
- tmux/PTY/template lane 观察能力加固。

退出标准：

- 一个 tmux 或 PTY coding-agent lane 可以启动、观察、追问、接受或丢弃，并留下
  可见证据。

### M5：Extension Foundation

重点：先让 plugin、skill、MCP、tool、agent 可诊断，再让它们变强。

- 统一 extension descriptor。
- `/extensions doctor` 和更有用的 `/skills list`。
- MCP context/config 状态进入 side-2。
- extension 调用进入共享 permission/runtime/transcript path。

退出标准：

- MCP 配置缺失、binary 缺失、skill disabled、extension health failed 都能输出
  actionable diagnostics。

### M6：ACP Bridge Spike

重点：证明未来多 agent 协议方向，同时不破坏本地 cockpit。

- ACP process transport 边界。
- handshake 和 JSONL event log。
- text、edit、tool、permission、completion event 的 lane mapping 设计。
- `/agent doctor acp` readiness 和 protocol evidence。

退出标准：

- mock ACP-compatible process 可以 handshake、emit events，并留下可回放证据，
  这些证据能干净映射为 lane artifacts。

## P0：必须交付

### 1. 主屏任务状态中心

目标：主窗口永远能回答“现在 RoboCode 在干什么”。

交付：

- 在 transcript 中间或固定 live activity 区域展示当前 primary operation：
  `Thinking`、`Editing <file>`、`Running tests`、`Waiting approval`、
  `Supervising <n> lanes`、`Idle`。
- 状态行必须带证据：来源可以是 provider request、tool call、pending approval、
  test event、lane artifact 或 transcript event。
- 长操作需要显示持续时间和关键上下文，例如当前文件、命令、lane ID、token/event
  变化。
- 状态不能只在副屏存在；主屏必须足够让用户判断是否继续等待、审批、打断或切换
  lane。

验收：

- 用户按 Enter 后 200ms 内主屏出现可见工作状态。
- provider 正在流式返回、tool 正在执行、approval 阻塞、lane 后台运行时，主屏都有
  不同状态表达。
- 没有运行中任务时显示 `Idle` 或最近完成摘要，不显示假进度。

### 2. 编程闭环体验

目标：把“写代码、看 diff、跑测试、修错、确认结果”的闭环做顺。

交付：

- `/test` 结果进入主屏和 side-2 evidence，而不只停留在 transcript 文本里。
- 文件编辑 tool call 需要聚合成可扫读的 edit summary：文件、增删行、是否待审批、
  是否已写入。
- 增加或完善 diff/review 入口，让用户能从 TUI 直接定位本轮变更。
- approval overlay 继续默认聚焦 approve，但需要在主屏状态里清楚显示阻塞原因。
- 输入区保持高可见性：光标闪烁、中文输入法位置正确、长输入不压扁 composer。

验收：

- 完成一次“生成文件 -> 审批 -> 运行测试 -> 查看结果”的 demo，不需要离开 TUI。
- 用户能从主屏或 side-2 找到最近测试输出、失败摘要和相关文件。

### 3. Codex Adapter 和 Job Runtime

目标：以 Claude Code Codex 插件为参考工作流，让 Codex 成为一等 external agent
backend。

交付：

- `/agent doctor codex` 检查 `codex` binary、app-server support、auth readiness、
  config source 和 workspace trust/setup 状态。
- `/agent review codex` 对 working tree 或 base branch 发起 read-only Codex
  review，支持 foreground/background。
- `/agent challenge codex` 发起可指定焦点的 adversarial review，挑战假设、权衡和
  failure modes。
- `/agent run codex [--write] <task>` 启动 tracked Codex task；write-capable run
  必须显式选择并经过权限控制。
- `/agent status`、`/agent result <id>`、`/agent cancel <id>` 和 resume/follow-up
  handling 对 Codex jobs 可用。
- Codex app-server notifications、final output、touched files、command
  executions、test evidence 和 thread IDs 都持久化为 RoboCode evidence。

当前实现状态：

- 已落地：`/agent doctor codex` 检查 Codex command、version、
  `app-server` 可用性、auth status、config sources 和 job-store path。
- 已落地：`/agent review codex`、`/agent challenge codex` 和
  `/agent run codex [--write] <task>` 会启动 tracked Codex CLI jobs，并在
  `.robocode/agents/` 下记录每个 job 的 log 和 result artifacts。
- 已落地：`/agent run codex --write <task>` 是显式 write-capable delegate path。
  它会先走 RoboCode mutating permission path，获得 approval 后才用 Codex
  `workspace-write` sandbox 启动。
- 已落地：`/agent status`、`/agent result <id>` 和 `/agent cancel <id>` 会读取并控制
  `.robocode/agents/codex-jobs.jsonl` 中的 tracked job records。
- 已落地：Codex jobs 会记录启动时 Git status baseline，并从 job output 提取
  resume/session hints 和 touched-file evidence，所以 `/agent status` 与
  `/agent result <id>` 能在可用时显示 `codex resume ...` 和相关文件。
- 已落地：TUI workspace snapshot 会读取 tracked Codex jobs，所以主窗口
  `LIVE ACTIVITY` strip 和右栏 `ACTIVE TASKS` panel 会显示正在运行的 Codex 工作，
  不再让用户提交后猜远端是否还在工作。
- 已落地：`/extensions doctor` 和 `/mcp doctor` 会按 surface 输出 readiness，
  包括 provider plugin dirs、MCP config files 和 server names、project/user/legacy
  skill roots，以及 permission boundary 提醒。
- 已落地：`/agent doctor codex` 会探测 experimental app-server JSON schema
  surface，并报告 thread lifecycle、review、turn control、event、evidence 和
  approval protocol groups 是否可用。
- 已落地：`/agent probe codex` 会通过 stdio 执行 live app-server
  `initialize` 握手，并写入可回放的 JSONL response/notification evidence。
- 已落地：`/agent probe codex --thread` 会启动 ephemeral read-only Codex
  app-server thread，并捕获结构化 `threadId` / `thread/started` evidence，
  但不会运行 model turn。
- 已落地：`/agent probe codex --turn <task>` 会启动 read-only app-server turn，
  并捕获结构化 turn/item/completion event evidence。
- 已落地：完成后的 app-server turn probe 会写入 tracked Codex job records 和
  result summaries，因此 `/agent status`、`/agent result` 和 TUI job rail 都能显示
  结构化 thread/turn evidence。
- 已落地：`/agent run codex --app-server <task>` 会启动异步 read-only
  app-server turn job，并复用 tracked job/status/result surfaces，同时默认路径仍保留
  CLI fallback。
- 剩余：通过 config flag/default 推广 app-server path，并把 server approval
  requests 接入 RoboCode permissions。
- app-server protocol 调研已记录在
  [codex-app-server-adapter.zh-CN.md](codex-app-server-adapter.zh-CN.md)。

验收：

- 可以启动 read-only Codex review，在 TUI 中观察并渲染结果。
- background Codex task 可以通过 `/agent status` 查看，通过 `/agent result`
  取回结果。
- write-capable Codex work 不能绕过 RoboCode permissions、transcript 和 approval。

### 4. Agent Lane 生命周期

目标：让外部 coding agent 不只是“启动了一个终端”，而是有生命周期、有证据、有决策。

交付：

- lane 状态统一为：`queued`、`thinking`、`editing`、`testing`、
  `needs input`、`waiting approval`、`blocked`、`done`、`failed`、
  `archived`。
- `/lane inspect <id>` 展示 objective、transport、workspace、latest output、
  changed files、test evidence、next action 和 decision history。
- `/lane send <id> <text>`、`/lane accept`、`/lane revise`、`/lane discard`、
  `/lane apply` 串成明确操作闭环。
- side-1 继续作为 agent lanes cockpit，优先显示真实 lane evidence，而不是
  预览数据。

验收：

- 用 tmux 或 PTY 启动 Codex/Claude/DeepSeek 类外部工具后，RoboCode 能持续观察
  latest output，并给出下一步操作提示。
- lane 完成后能看到结果证据，并能选择 accept/revise/discard/apply。

## P1：应该交付

### 5. Extension System 第一版可用化

目标：plugin、skill、MCP 不只展示“存在”，而是形成可诊断、可调用、可扩展的系统。

交付：

- 定义统一 extension descriptor：
  - `id`
  - `kind: plugin | skill | mcp | tool | agent`
  - `source`
  - `capabilities`
  - `permissions`
  - `health`
  - `entrypoints`
- `/extensions doctor` 输出 actionable diagnostics：缺 binary、缺配置、权限不满足、
  schema 错误、版本不兼容。
- `/skills list` 增加 skill summary 和触发方式，避免只是一堆路径。
- MCP config 进入 runtime context：side-2 能显示启用的 MCP server、配置来源和错误。
- 所有 extension 调用必须走共享 permission/runtime path，不能绕开 transcript。

验收：

- 用户能通过 `/extensions doctor` 知道为什么某个 MCP 或 skill 不能用。
- side-2 能区分 configured、ready、failed、disabled。

### 6. ACP Adapter Spike

目标：建立未来支持更多 coding agent 的协议边界。

交付：

- 新增 ACP adapter 设计文档或模块边界，说明如何映射到 RoboCode lane event。
- 完成最小 process transport spike：
  - launch agent server；
  - handshake；
  - 记录 JSONL event；
  - 映射 text/edit/tool/permission 事件到 lane artifact。
- `/agent doctor <id>` 能区分 template/tmux/pty/acp 的 readiness。

验收：

- 可以用 mock ACP server 或最小 compatible server 跑通 handshake 和 event log。
- 不要求 0.1.7 支持完整 ACP 编辑闭环，但事件模型必须清楚。

### 7. Side-2 Ops 屏幕真实化

目标：副屏 2 成为 tests、LSP、MCP、extension、evidence 的操作面板。

交付：

- `TESTS / LSP`：显示最近测试命令、状态、耗时、失败摘要，以及 LSP diagnostics。
- `MCP / CONTEXT`：显示 MCP 配置来源、context window、workspace snapshot。
- `EXTENSIONS`：显示 provider、agent、skill、MCP 的 ready/failed/disabled 数量。
- `RECENT EVIDENCE`：显示最近 tool/test/lane artifact，而不是普通聊天摘要。

验收：

- 跑完 `/test` 后 side-2 能看到测试结果。
- MCP 配置错误或缺失时 side-2 有明确提示。

## P2：探索项

- Agent task planner：把一个大目标拆成多个 lane 任务，但仍需要用户确认。
- Lane scheduling policy：限制并发、选择 agent、选择 workspace/worktree。
- Remote/desktop companion：为未来桌面版或 editor integration 保留状态协议。
- 可选 task graph 视图：展示 main agent 与 side agents 的依赖关系。

## 非目标

- 不做云端 agent registry。
- 不做账号系统或远程任务托管。
- 不把 RoboCode 变成完整 IDE。
- 不在 extension system 稳定前引入复杂 marketplace。
- 不让 plugin、skill、MCP 或 ACP 绕开权限、transcript 和 approval。

## 用户可感知成功标准

- 用户提交任务后，不再需要猜模型是否在工作。
- 用户能在主屏看到当前动作，在 side-1 看到子 agent，在 side-2 看到证据和诊断。
- Codex、Claude Code、DeepSeek、shell job 和未来 ACP agent 在 TUI 里使用同一套
  lane/status/approval/evidence 语言。
- Codex 具体要像原生能力：setup、review、rescue/task、background status、
  result replay、cancel 和 resume 都能在 RoboCode 内完成，不需要打开另一个终端。
- plugin、skill、MCP 的问题能被诊断，而不是只表现为“命令没反应”。
- 一次真实小功能开发能在 TUI 内完成：输入需求、审批修改、运行测试、查看结果、
  接受或修订 lane 输出。

## 开发顺序建议

1. Codex adapter core：先把具体 external-agent workflow 做出来，以 Claude Code
   Codex 插件作为参考产品形态。
2. 主屏任务状态中心：Codex 和 RoboCode 工作中时，主屏都要看得到。
3. side-2 evidence 面板：让测试、LSP、MCP、extension 和 Codex job evidence 有
   统一观察面。
4. lane lifecycle 打磨：把 Codex、tmux、PTY/template 外部工具形成同一套操作闭环。
5. extension descriptor 和 doctor：先做诊断和边界，再做复杂执行。
6. ACP spike：等 Codex 证明具体 adapter model 后，再验证通用协议方向。

## 验证门槛

- `cargo fmt --check`
- 相关 crate focused tests
- `cargo test --workspace --quiet`
- TUI preview：主屏、side-1、side-2 都要留截图或文本快照
- 至少一次 fallback/provider smoke，覆盖：
  - prompt submit -> live activity
  - file edit approval
  - `/test`
  - Codex setup/review/status/result，或 mock Codex app-server 等价验证
  - side-1 lane evidence
  - side-2 ops evidence
