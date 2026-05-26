# RoboCode 0.1.7 计划

英文版： [release-0.1.7-plan.md](release-0.1.7-plan.md)

最后更新：2026-05-26

## 目标

`0.1.7` 的目标是继续优化真实编程体验，把 0.1.6 建立起来的 TUI cockpit、
lane、extension 可见性，推进成可日常使用的多 agent 编排工作台。

版本主题：

```text
0.1.7 = Programming Experience + Agent Orchestration Backbone
```

核心判断：RoboCode 不是单纯做一个好看的 TUI，也不是只把 Codex、Claude Code、
DeepSeek 等工具拉起来。它要成为一个本地 multi-agent cockpit：用户在主屏输入目标，
RoboCode 能拆分、派发、观察、审批、收敛结果，并让不同 coding agent 通过统一机制
协作。

## 版本问题陈述

当前体验里最影响继续深入试用的问题有三类：

- 运行状态不够强：用户输入后，主窗口中间需要持续说明现在到底在做什么，例如
  thinking、editing、testing、waiting approval、supervising lanes。
- 扩展系统仍偏只读：plugin、skill、MCP 已有可见性，但还没有形成一个能真正提升
  开发体验的加载、诊断、调用和权限模型。
- 多 agent 还在终端集成阶段：tmux/PTY/template 已能接入外部工具，但后续要向
  Zed ACP 方向扩展，把不同 coding agent 变成统一 adapter 下的一等 lane backend。

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

### 3. Agent Lane 生命周期

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

### 4. Extension System 第一版可用化

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

### 5. ACP Adapter Spike

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

### 6. Side-2 Ops 屏幕真实化

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
- plugin、skill、MCP 的问题能被诊断，而不是只表现为“命令没反应”。
- 一次真实小功能开发能在 TUI 内完成：输入需求、审批修改、运行测试、查看结果、
  接受或修订 lane 输出。

## 开发顺序建议

1. 主屏任务状态中心：先把用户最痛的“不知道它在干什么”解决掉。
2. side-2 evidence 面板：让测试、LSP、MCP、extension 状态有统一观察面。
3. lane lifecycle 打磨：把 tmux/PTY/template 外部工具形成可操作闭环。
4. extension descriptor 和 doctor：先做诊断和边界，再做复杂执行。
5. ACP spike：并行验证协议方向，但不阻塞日常编程体验交付。

## 验证门槛

- `cargo fmt --check`
- 相关 crate focused tests
- `cargo test --workspace --quiet`
- TUI preview：主屏、side-1、side-2 都要留截图或文本快照
- 至少一次 fallback/provider smoke，覆盖：
  - prompt submit -> live activity
  - file edit approval
  - `/test`
  - side-1 lane evidence
  - side-2 ops evidence
