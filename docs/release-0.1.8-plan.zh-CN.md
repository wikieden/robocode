# Viden 0.1.8 计划

英文版： [release-0.1.8-plan.md](release-0.1.8-plan.md)

最后更新：2026-05-27

## 版本定位

`0.1.7` 已完成 GitHub release、Homebrew tap、Codex job runtime、初版 operation
center、extension/MCP diagnostics 和副屏基础能力。`0.1.8` 是在这些能力之上继续打磨
真实编程体验的版本。

版本主题：

```text
0.1.8 = AgentTask + Live Multi-Agent Cockpit
```

核心目标：让 Viden 从“能启动和观察 agent”推进到“能清楚编排多个 coding
agent”。用户提交任务后，主屏中央必须持续显示当前到底在做什么；副屏不再只是补充信息，
而是展示子 agent、测试、诊断、MCP、extension 和证据；Codex、Claude Code、
DeepSeek、shell、tmux/PTY 和未来 ACP agent 都进入同一套 `AgentTask` 模型。

## P0：必须交付

### 1. 统一 AgentTask 运行模型

目标：所有正在工作的对象都先归一化成一个可观察任务模型，再进入主屏、副屏和命令接口。

交付：

- 定义 `AgentTask` runtime view，覆盖 Viden 主回复、provider turn、tool call、
  approval、test run、shell job、Codex job、Claude/DeepSeek lane、tmux/PTY bridge
  和未来 ACP session。
- 最小字段：`id`、`parent_id`、`agent`、`kind`、`transport`、`status`、
  `activity`、`summary`、`progress`、`started_at`、`updated_at`、`workspace`、
  `evidence`、`permissions`、`decision`、`result`、`resume_handle`。
- 统一状态集合：`queued`、`thinking`、`streaming`、`editing`、`running_tool`、
  `testing`、`waiting_approval`、`needs_input`、`blocked`、`done`、`failed`、
  `cancelled`、`archived`。
- `AgentTask` 不替代 transcript、lane artifacts、Codex job records 或 test
  evidence；它只从这些 source of truth 归一化状态，不能为 UI 编造任务。

验收：

- 同一个 Codex job 在 `/agent status`、右栏、主屏 operation center 和副屏中显示同一
  id、状态和 evidence source。
- 主回复、tool call、approval、test run 与外部 lane 可以在同一列表里排序，并能看出
  谁阻塞了当前回合。

### 2. 主屏中央“现在到底在干嘛”

目标：主屏永远回答当前工作状态，而不是让用户猜远程模型、工具或子 agent 是否还在跑。

交付：

- operation center 从 `AgentTask` view 推导主状态。
- 用户提交后 200ms 内显示 `thinking` 或 `streaming`。
- 工具调用时显示 `running_tool <tool>`，文件修改时显示 `editing <file>`，测试时显示
  `testing <command>`，审批时显示 `waiting approval`。
- 外部 agent lane 运行时显示 `supervising <n> agents`，并突出最重要的 blocker 或 next
  action。
- 状态必须带 evidence source，例如 provider request、tool call id、approval id、lane
  id、test artifact 或 Codex thread/turn id。

验收：

- provider 流式返回、tool 执行、approval 阻塞、lane 后台运行时，主屏都有不同状态。
- 没有任务时显示 `Idle` 或最近完成摘要，不显示假进度。

### 3. 编程闭环体验

目标：让“写代码 -> 审批 -> 看 diff -> 跑测试 -> 修错 -> 确认结果”在 TUI 内更顺。

交付：

- `/test` 结果进入主屏和 side-2 evidence，包含 command、status、duration、
  failure summary、output tail 和相关文件。
- edit/tool summary 聚合文件、增删行、审批状态、写入结果和 diff/review 入口。
- approval overlay 保持默认聚焦 approve，但确认/拒绝后必须立即清理，不再挡住后续内容。
- composer 保持可见：输入区更高、光标闪烁、中文输入法位置正确、窗口 resize 后重绘正常。
- 修复右栏/边框/多语言内容导致的视觉漂移，长期会话不能把侧栏撑歪。

验收：

- 完成一次“生成文件 -> 审批 -> 跑测试 -> 查看结果”的 demo，不离开 TUI。
- 每次 UI 迭代保留主屏、approval、side-1、side-2 的截图或文本快照。

### 4. Agent Lane Operator Loop

目标：外部 coding agent 不只是终端进程，而是可观察、可追问、可接受或丢弃的协作者。

交付：

- Codex、Claude Code、DeepSeek、shell、tmux、PTY lane 都映射成 `AgentTask`。
- `/lane inspect` 展示 objective、transport、workspace、latest output、changed
  files、test evidence、next action 和 decision history。
- `/lane send`、`/lane accept`、`/lane revise`、`/lane discard`、`/lane apply` 串成明确
  operator loop。
- side-1 优先展示真实 lane evidence、最新输出和下一步动作。

验收：

- 一个 tmux/PTY coding-agent lane 能启动、观察、追问、接受或丢弃，并留下可审计证据。

## P1：应该交付

### 5. Codex Adapter 深化

- 继续以 Claude Code Codex 插件为参考，完善 Codex setup/doctor、review、
  adversarial review、task/rescue、status、result、cancel、resume/follow-up。
- 在 live smoke 证明安全前，app-server task path 保持 opt-in；成熟后再考虑默认走
  protocol path。
- Codex app-server thread/turn/event evidence 继续映射到 `AgentTask`、lane evidence
  和 side-screen rows。

### 6. Plugin / Skill / MCP / Tool / Agent 扩展基础

- 定义统一 extension descriptor：`id`、`kind`、`source`、`capabilities`、
  `permissions`、`health`、`entrypoints`。
- `/extensions doctor`、`/mcp doctor`、`/skills list` 输出 actionable diagnostics。
- MCP 和 extension invocation 必须进入共享 permission/runtime/transcript path，不能绕开
  审批和审计。

### 7. ACP Adapter Spike

- 以 Zed ACP 方向为长期参考，保留 process transport、handshake、JSONL event log 和
  event-to-AgentTask mapping。
- 0.1.8 不要求完整 ACP 编辑闭环，但要明确 text/edit/tool/permission/completion event
  如何映射到 lane artifacts。

## 非目标

- 不做云端 agent registry。
- 不做 marketplace。
- 不把 Viden 变成完整 IDE。
- 不让 plugin、skill、MCP、ACP 绕过权限、transcript 和 approval。
- 不把自动任务拆分作为默认行为；planner 可以探索，但需要用户确认。

## 用户可感知成功标准

- 用户提交任务后，不再需要猜模型或远程 agent 是否在工作。
- 主屏能看到当前动作；side-1 能看到子 agent；side-2 能看到测试、LSP、MCP、extension
  和 evidence。
- Codex、Claude Code、DeepSeek、shell job 和未来 ACP agent 使用同一套
  `AgentTask` / status / approval / evidence 语言。
- 一次真实小功能开发能在 TUI 内完成：输入需求、审批修改、运行测试、查看结果、接受或修订
  lane 输出。

## 开发顺序建议

1. `AgentTask` runtime view 和 reducer。
2. 主屏 operation center 改为读取 `AgentTask`。
3. 编程闭环 evidence：edit/test/diff/approval。
4. side-1 lane operator loop 和 side-2 ops evidence。
5. Codex adapter protocol path hardening。
6. extension descriptor / doctor。
7. ACP event mapping spike。

## 验证门槛

- `cargo fmt --check`
- 相关 crate focused tests
- `cargo test --workspace --quiet`
- TUI preview 或截图：idle、thinking、tool call、approval、test result、side-1、side-2
- 本机 Codex auth 和 rate limit 可用时，运行
  `scripts/smoke-codex-app-server.sh`
- 运行 `scripts/smoke-codex-app-server-protocol-fixture.sh`，用于确定性覆盖
  command/file/approval/error protocol event ingestion
- 运行 `scripts/smoke-codex-app-server-write-guard.sh`，用于验证 experimental
  write-capable app-server turn 的默认安全 guard
- 运行 `scripts/smoke-lane-operator-loop.sh`，用于 focused 覆盖 lane inspect/send/
  accept/apply/conflict/cleanup/archive
- 至少一次 provider smoke，覆盖：
  - prompt submit -> `AgentTask` -> operation center
  - file edit approval
  - `/test`
  - Codex status/result 或 mock app-server evidence
  - side-1 lane evidence
  - side-2 ops evidence
