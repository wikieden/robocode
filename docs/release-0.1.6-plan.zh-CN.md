# RoboCode 0.1.6 计划

最后更新：2026-05-26

## 目标

`0.1.6` 要把 RoboCode 从 terminal-first coding assistant 继续推进成
multi-agent orchestration cockpit。这个版本优先改善真实编程体验：主屏能看清
后台到底在干什么，并为 ACP、plugin、skill、MCP 建好系统边界，避免后续变成一堆
互不相干的功能。

这份计划来自 0.1.5 试用后的三个发现：

1. 用户提交输入后，主 TUI 必须立刻显示远程/model/lane 是否正在工作。
2. Plugin、skill、MCP 需要统一系统设计，而不是各做各的。
3. 现在通过 tmux 调用外部工具已经有价值，但 RoboCode 的目标应该继续向 Zed 的
   ACP 方向扩展，支持更多 coding agent。

## 开发目标

下一个版本的目标是：**让 RoboCode 在真实编程过程中变得“活着”，并具备
operator-grade 的多 agent 调度感**。用户提交任务后，应该能在 cockpit 里直接看清
主 agent 在做什么、副 agent 在做什么，以及下一步该怎么接管或推进，而不是离开
TUI 到各个终端里猜状态。

版本主题：

```text
0.1.6 = Live Coding Cockpit + Agent Extension Foundation
```

### P0：必须交付

- 主屏 live activity：
  - prompt 提交后立刻显示 `Thinking...`；
  - 文件/工具动作压缩展示，例如 `Editing render.rs`；
  - approval waiting 状态清晰可见，同时不遮住整个 session；
  - 主屏直接展示 active lane 数量和关键 lane 进度。
- 状态必须有证据来源：
  - 所有可见 runtime status 必须来自 transcript events、provider telemetry、
    pending approvals、lane artifacts 或 workspace snapshots；
  - 正常运行界面不能出现 placeholder metrics。
- Agent lane 基线：
  - 保持 template、tmux、PTY lane 可用；
  - 用同一套 lane 形态展示 transport 和 status；
  - 让 `/lane inspect` 成为外部 agent 工作的可靠 debug 入口。
- 产品/设计文档：
  - 记录 adapter model；
  - 记录 plugin、skill、MCP、ACP 的系统边界；
  - 中英文文档保持同步。

### P1：应该交付

- Agent registry：
  - `/agent list` 展示 built-in 和 configured agents。**状态：初版只读 built-in
    registry 已落地。**
  - `/agent doctor [id]` 检查 binary、environment、template、tmux 和 PTY
    readiness。**状态：初版本地 binary/template diagnostics 已落地。**
- Extension surface：
  - `/extensions list` 和 `/extensions doctor` 先做只读可见性。**状态：初版
    extension visibility 已落地。**
  - `/mcp list` 和 `/mcp doctor` 先做只读可见性。**状态：初版 config-file
    visibility 已落地。**
  - `/skills list` 展示本地 workflow/task recipes。**状态：初版本地 skill listing
    已落地，默认限制输出并支持 `--all`。**
- 副屏改善：
  - side-1 优先展示 agent lanes、transport、state、latest output 和 next action。
    **状态：side-1 lane transport/state 行已落地。**
  - side-2 优先展示 tests、LSP、MCP/context、plugin health 和 evidence。

### P2：Spike，不作为发布硬门槛

- ACP proof of concept：
  - 启动一个本地 ACP-compatible process；
  - 完成最小 handshake；
  - 把 ACP events 记录为 JSONL debug log；
  - 验证 ACP 的 edit/tool/permission events 如何映射到 RoboCode lanes。
- `/lane acp <agent> <task>` 可以先保持 experimental，等 event model 清楚后再
  进入稳定命令面。

### 用户可感知成功标准

- 按 Enter 后，用户不需要猜 RoboCode 是在 thinking、editing、waiting approval，
  还是 supervising another agent。
- 主屏和副屏使用同一套 agent state 语言：`thinking`、`editing`、`testing`、
  `waiting approval`、`needs input`、`blocked`、`done`。
- Codex、Claude Code、DeepSeek lanes、shell jobs 和未来 ACP agents 在 cockpit
  里看起来是同级协作者，而不是一堆一次性集成。
- 调试外部 agent run 有清晰路径：status row -> lane detail ->
  log/artifact/event replay。
- 版本仍保持 local-first：核心体验不依赖 cloud orchestration、账号系统或远程
  registry。

## 参考信号

- Claude Code 终端版用紧凑运行行展示类似 `Moseying...`、耗时、token movement
  和提示语，用户能看出它还在工作。
- Codex 桌面版会把当前操作说清楚，例如 `Editing render.rs +129 -4` 和
  `Thinking`。
- Zed 的 ACP 方向重要之处在于：Agent Client Protocol 把 editor/client 和
  coding agent 之间的通信标准化，让一个客户端可以通过同一协议支持很多 agent。
- Zed 的 external agents 是独立进程，通过 ACP 和 Zed 通信。Zed 只转发少量
  editor-owned 设置，例如 model、mode、env、MCP context servers 和项目根目录；
  外部 agent 仍读取自己的原生配置。
- Zed 的 agent-server packaging 也值得学习：按平台声明 target、command/args、
  environment、archive 和 SHA-256。

参考文档：[Zed ACP](https://zed.dev/acp)、
[Zed external agents](https://zed.dev/docs/ai/external-agents.html) 和
[Zed agent-server extensions](https://zed.dev/docs/extensions/agent-servers)。

## 产品原则

- 主屏永远要回答：“RoboCode 现在正在干什么？”
- 外部 agent 是 adapter 后面的协作者，不是可信权威。
- ACP、tmux、PTY、CLI template lanes、plugins、skills、MCP 都应进入同一套
  lane/status/approval/evidence 模型。
- 用户可见 panel 必须有证据来源。未知 runtime state 要显示 idle、unavailable
  或 setup required。
- RoboCode 首先是本地 multi-agent cockpit，不是 cloud task runner，也不是完整
  editor 替代品。

## 工作流

### 1. Live Activity Strip

目标：解决提交 prompt 后“不知道它是不是还在工作”的问题。

交付：

- 主 transcript 区固定保留 `LIVE ACTIVITY`。
- 请求状态：
  - 最新 prompt 处理中显示 `Thinking...`；
  - 显示 provider/model；
  - 完成后显示最近 assistant/tool result 摘要。
- Tool 状态：
  - 文件变更类 tool call 压缩成 `Editing src/render.rs` 这样的行；
  - 有 pending approval 时显示 approval required。
- Lane 状态：
  - 显示 active lane 数量，以及关键 lane 的 status/progress/summary；
  - 不造假数据，只从当前 TUI state、provider telemetry、pending approvals、
    transcript events 或 lane store artifacts 推导。

验收：

- 按 Enter 后主屏立刻出现 `Thinking...`。
- active tmux/PTY/background lanes 不打开 side screen 也能在主屏看到。
- wide 和 compact layout 都保留该 strip。
- TUI preview generation 覆盖该 strip。

### 2. Agent Adapter Model

目标：把外部 coding agents 变成一等 lane backend。

Adapter families：

- `template`：当前 `ROBOCODE_LANE_<TOOL>_TEMPLATE`。
- `tmux`：当前 operator-controlled terminal session。
- `pty`：当前 embedded PTY bridge。
- `acp`：后续用于支持 Agent Client Protocol 的 JSON-RPC/ACP bridge。

统一 adapter contract：

```text
AgentAdapter
  id
  display_name
  transport: template | tmux | pty | acp
  launch
  send_task
  send_followup
  poll_status
  read_events
  stop
  capability_descriptor
```

Lane model 不应该关心 agent 是 tmux 里的 Codex、template 里的 Claude、ACP 的
Gemini，还是 ACP 的 Kiro。统一记录：

- objective
- workspace/worktree
- tool/agent id
- launch transport
- model/mode（如果可得）
- log/event paths
- permission requests
- edits/diffs
- tests/evidence
- next action

### 3. ACP Bridge Planning

目标：在不破坏现有 lane workflow 的前提下引入 ACP。

阶段：

1. `robocode-acp` spike：
   - 增加 ACP message types 和 process transport 的 crate/module 边界；
   - 如果官方 Rust ACP library 合适就直接用，否则先保持最小 JSON-RPC transport
     wrapper。
2. `/agent list` 和 `/agent doctor`：
   - 展示已配置的 template/tmux/pty/acp agents；
   - 校验 binary path、launch args、env 和 protocol handshake。
3. `/lane acp <agent> <task>`：
   - 启动 ACP server process；
   - 创建 lane；
   - 以 ACP session/request 发送任务；
   - 把 streamed text、tool/edit events、permission prompts 记录成 lane artifacts。
4. Debug visibility：
   - 写 `.robocode/agents/<lane-id>.acp.jsonl`；
   - 增加 `/agent logs <id>` 或让 `/lane inspect <id>` 回放 ACP events。

不要在 local custom-agent flow 跑通前先做 registry installation。

### 4. Plugin、Skill、MCP 系统形态

目标：一个 extension model，多种 extension kind。

建议层次：

```text
robocode extensions
  providers: model provider plugins（已开始）
  agents: template/tmux/pty/acp agent adapters
  tools: local tool plugins 和 MCP-backed tools
  skills: prompt/workflow/task templates
  context: MCP servers、repo context providers、docs/index providers
```

规则：

- Provider plugins 暂时继续归 `robocode-model`，直到 provider registry 足够稳定。
- Agent adapters 先靠近 lane orchestration，稳定后再移动到 `robocode-agents`
  或 `robocode-workflows`。
- MCP tools 必须进入现有 permission/tool/transcript path，不能开一个平行 mutation
  runtime。
- Skills 不是 tools。Skills 是可复用 task envelope、prompt 和 workflow recipe，
  可以创建 lane、配置 context 或指导主 agent。
- 每种 extension kind 都需要：
  - manifest；
  - discovery；
  - doctor output；
  - capability descriptor；
  - permission boundary；
  - debug logs。

### 5. Multi-Agent Cockpit UX

目标：让 RoboCode 更像多个 agent 的 operator console。

Main screen：

- current live activity；
- pending approval；
- top active lane；
- recent edit/test evidence。

Side-1：

- agent lanes；
- agent transport（`tmux`、`pty`、`acp`、`template`）；
- state（`thinking`、`editing`、`waiting approval`、`needs input`、`testing`）；
- next action。

Side-2：

- tests/build/LSP；
- MCP/context status；
- plugin/agent health；
- recent evidence。

命令面：

- `/agent list`
- `/agent doctor [id]`
- `/agent logs <id>`
- `/lane acp <agent> <task>`
- `/lane followup <id> <message>`
- `/extensions list`
- `/extensions doctor`
- `/mcp list`
- `/mcp doctor`
- `/skills list`

## 实施顺序

1. 在主 TUI 落地 `LIVE ACTIVITY`。
2. 用中英文文档记录 adapter/extension architecture。
3. 增加 agent registry 数据结构和 `/agent list`，先列出 built-in template/tmux
   agents。**初版只读实现已落地。**
4. 增加 `/agent doctor`，覆盖 Codex、Claude、custom templates、tmux、PTY。
   **初版只读实现已落地。**
5. 用一个本地 ACP-compatible agent 做 `robocode-acp` spike。
6. 增加实验命令 `/lane acp <agent> <task>`。
7. 把 ACP events 接入 lane artifacts 和 `LIVE ACTIVITY`。
8. 扩展 side screens，展示 transport 和 agent state。

## 非目标

- 不替换现有 tmux/PTY/template lane support。
- 不让 MCP tools 绕过现有 permission system。
- 不做完整 editor UI。
- 本地编排没稳定前不做 cloud delegation。
- local manifest discovery 和 doctor checks 没稳定前不做 plugin registry installation。

## Ready Criteria

- Main TUI 能明确显示 thinking/editing/lane work。
- 至少一个现有 external-tool lane path 用和未来 ACP lanes 相同的 transport/status 形态展示。
- Agent/plugin/skill/MCP 架构已文档化，并有命令 stub 或计划入口。
- ACP spike 能证明 RoboCode 可以启动并和一个 ACP-compatible agent process 交换消息。
- 现有 0.1.5 release smoke 仍通过。
