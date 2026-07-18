# Viden 长期路线图

English version: [long-term-roadmap.md](long-term-roadmap.md)

最后更新：2026-06-26

## 战略判断

Viden 不应该靠“又一个单 Agent 聊天 CLI”来竞争。真正长期机会是成为 AI 编程工作的
本地优先操作层：

> 一个多 Agent 编程 cockpit，让 Agent 工作可见、可控、可审查、可复用，并且极致优化
> token 效能。

TUI 是第一个产品形态，不是最终产品边界。它适合作为起点，因为 approvals、logs、
tests、diagnostics、副屏 lane 和多 Agent 状态都需要高密度监督。等 runtime 稳定后，
同一套编排模型可以继续支撑 CLI 自动化、IDE/ACP adapter、桌面端、Web 和团队工作流。

GUI / Desktop 工作必须跟随 runtime contract，而不是提前开始。Viden 需要先完成核心结构、
共享 event/command model、context/cost facts、可监督 Agent 闭环和 release gate。
runtime/UI contract freeze 后，TUI 与 GUI 可以按
[Viden 并发开发计划](parallel-development-plan.zh-CN.md) 并行开发。GUI 产品契约记录在
[GUI 版本功能设计](gui-version-functional-design.zh-CN.md)。

接受的 TUI / GUI 视觉源现在是 `docs/viden-design/Viden/`，由
[Viden 设计接入决策](viden-design-adoption.zh-CN.md) 约束。Viden 只作为 legacy
implementation 和 compatibility 名称保留，直到 rename migration 被明确规划。

## 市场判断

当前 AI coding 工具正在往几个方向收敛：

- Claude Code 强在 terminal-native agent loop、hooks、MCP、subagents 和工作流约定。
- Codex 强在本地 coding agent surface，应该被视为重要 delegated lane，而不只是竞品。
- Zed 在推进 editor-native parallel agents 和 ACP external-agent 边界。
- Kiro 在推进 spec-driven development、steering files、hooks、MCP 和项目知识。
- Aider 提醒我们：repo map、git-native、小步改动仍然能击败更重的 agent 系统。
- OpenHands、Goose、Kilo 等项目指向更大的 platform、SDK、cloud 和多入口未来。
- Hacker News 的用户反馈反复说明：Agent 成功不只取决于自治程度，更取决于 context
  控制、任务边界、证据、隔离、成本可见性和人工 review。

对 Viden 的含义：不要同时追所有入口。先赢下 operator loop。

## 产品身份

Viden 应该成为：

- 本地优先的 AI coding operator cockpit。
- 其他 coding agents 的 supervisor，而不只是 provider client。
- 面向 transcript、logs、diffs、tests 的结构化事实和证据层。
- 一个 token-efficiency engine，决定每个 Agent 需要什么 context，并解释省略了什么。
- 一个安全 extension runtime，承载 providers、MCP servers、skills、hooks、ACP agents、
  shell jobs 和未来集成。
- 一个高密度操作产品，让 TUI 和 GUI 使用同一套 runtime facts、lane/session 语义、
  decision gate 和 evidence/context 表达。

Viden 不应该成为：

- Claude Code、Codex、Zed、Kiro 或 Aider 的复制品。
- 只有漂亮 side panels、但不能控制真实工作的 TUI。
- 权限、凭证、证据模型成熟前的 marketplace。
- 本地 runtime 可信前的 cloud/team 产品。
- editor 集成边界清楚前的第二个编辑器。

## 长期支柱

### 1. Operator Loop

核心循环是：

1. 澄清意图
2. 构造 task/spec/context
3. 路由到一个或多个 lane
4. 观察 live status
5. 收集 evidence
6. review/apply/discard/retry
7. 把决策保留下来作为未来 context

所有功能都应该强化这个循环。如果一个功能增加了自动化，但没有增强可见性、可审查性或
恢复能力，就应该延后。

### 2. Shared Agent Runtime

所有 Agent surface 都应该使用同一套模型：

- `AgentTask`
- `AgentLane`
- `ContextBundle`
- `Evidence`
- `Artifact`
- `Decision`
- `Permission`
- `Budget`
- `CredentialHandle`

Providers、shell lanes、Codex、Claude、DeepSeek、tmux、PTY、ACP、MCP、skills 和
hooks 都不应该绕出 side-channel runtime。

### 3. Context 与 Token 效能

Token 效能是产品能力，不是内部实现细节。

长期能力：

- source ranking 和 pin/omit 控制
- repo maps 和 semantic summaries
- diff-aware context selection
- task-specific memory retrieval
- 长日志 summary + tail 保留
- per-lane token 和 cost budgets
- 昂贵 turn 前的 context pressure warning
- 带 reason code 的 omitted-source records
- 相关 lanes 之间复用 context bundles

### 4. Evidence 与 Trust

用户应该始终能回答：

- Agent 现在在做什么？
- 它看到了什么？
- 它改了什么？
- 它运行了什么？
- 哪里失败了？
- Viden 为什么认为现在可以继续？
- 下一个安全动作是什么？

这需要 event timelines、audit replay、changed-file evidence、test output、
diagnostics、permission history，以及 UI release 的截图/smoke evidence。

### 5. Multi-Agent Orchestration

Multi-agent 不是“生成更多 Agent”。它是有边界的并行工作、清晰角色和可恢复状态。

目标内置角色：

- planner
- implementer
- reviewer
- tester
- researcher
- doc writer
- release/verifier

目标编排模式：

- plan -> implement -> review -> test
- parallel investigation lanes
- adversarial review lane
- failing tests rescue lane
- documentation/update lane
- release validation lane

### 6. Isolation 与 Safety

并行 coding agents 需要的不只是 git worktrees。

长期 lane isolation 应该建模：

- worktree 或 branch
- writable path scope
- environment variables
- caches
- test database/schema
- service ports
- background processes
- setup and teardown commands
- cleanup proof

### 7. 可扩展，但不碎片化

Viden 应该支持生态扩展，但必须保持同一套 permission 和 evidence model。

Extension layers 的成熟顺序：

1. descriptors and doctors
2. read-only probes
3. supervised invocation
4. permission-gated mutation
5. marketplace/install UX

ACP 应该被视为潜在的 Agent 互操作边界，类似 LSP 对 editor/language-server 集成的意义。

### 8. Runtime 稳定后再扩产品形态

TUI 仍然是 runtime 被证明之前的主要入口。之后再扩：

- scripts/CI 使用的 CLI automation
- editor-native context 的 ACP/IDE adapter
- local integrations 用的 API/server mode
- 更强视觉监督的 desktop app
- local workflows 可重复后再做 web/team dashboard
- credential/audit 边界成熟后再考虑 cloud/remote

## 阶段路线

### Horizon 1: 0.1.x - Cockpit And Delegated Lanes

目标：
把 TUI-led operator loop 做实、做可信。

关键结果：

- default TUI 稳定
- first-run setup 可用
- provider/model 设置清晰
- approval UX 可靠
- input、focus、mouse、modal、caret 和 resize 行为稳定到可以日常使用
- provider/tool 活跃时持续重绘 working state，而不是让 cockpit 看起来卡住
- side screens 显示真实 lane state
- shell/template lanes 有实际用途
- Codex 和 Claude 可以作为 supervised delegated lanes
- lane event timeline 存在
- ContextBundle v1 可见
- lane isolation preflight 存在
- docs、screenshots、release assets 和 Homebrew 形成常规流程
- 0.1.x final 前通过 [TUI Stability Zero-Bug Gate](tui-stability-zero-bug-gate.zh-CN.md)：
  已知 P0/P1 TUI 显示、输入、弹窗、滚动、resize 和状态错误清零

暂时不做：

- 默认开启大范围 write-capable external agents
- cloud/team dashboards
- plugin marketplace
- lane evidence 稳定前的完整 ACP runtime

### Horizon 2: 0.2.x - Spec, Context, And Evidence Runtime

目标：
让 Viden 从 cockpit 变成可重复的 coding workflow engine。

关键结果：

- `/spec` 或等价命令生成 requirements/design/tasks
- steering files 记录项目约定
- task envelopes 直接喂给 lanes
- ContextBundle 支持 pin/omit/source priority/reason codes
- cost/rate/time budget ledger 存在
- event timelines 和 audit replay 成为一等能力
- reviewer/tester lanes 变成标准模板
- lane apply/discard/retry lineage 可持久追踪
- local release/test workflows 编码为可复用 flows

带来的结果：

- 用户可以在 apply 之前看到 scope、context、budget 和 evidence，因此更敢让 Agent
  做长任务。
- 即便 Codex 或 Claude 是后端，Viden 仍然有自己的差异化价值。

### Horizon 3: 0.3.x - External Agent And ACP Interoperability

目标：
让 Viden 成为异构 coding agents 的 supervisor。

关键结果：

- ACP probe 和一个真实 ACP compatibility target
- external agent capability registry
- Codex/Claude/DeepSeek/Gemini/custom templates 共享同一 lane lifecycle
- 发现 agent-native config，而不是重复复制配置
- MCP/plugin/skill descriptors 在 doctor 和 side-2 中可见
- credential handles 防止 secret 泄露到 transcript
- hooks 有类型、可 blocking、可记录、可 inspect

带来的结果：

- Viden 可以为每个任务编排最合适的 Agent，而不是变成某一家 provider 的 wrapper。

### Horizon 4: 0.4.x - Built-In Multi-Agent Workflows

目标：
交付开箱即用、无需大量手工 wiring 的编排能力。

关键结果：

- 内置 workflow templates：plan、implement、review、test、release
- background reviewer/tester lanes
- 自动任务拆分，但边界对人可见
- failing tests rescue loops
- lanes 之间复用 context
- budget-aware model routing
- project-level memory 和 decision records 真正可用
- TUI、CLI、provider、lane、extension 都有 deterministic validation packs

带来的结果：

- Viden 成为真正的多 Agent 编程工作台，而不是手工启动任务的 cockpit。

### Horizon 5: 0.5.x - Platform Surfaces

目标：
把稳定 runtime 暴露到 TUI 之外。

关键结果：

- scriptable CLI flows
- local API/server mode
- IDE/ACP adapter
- desktop/visual operator view 探索
- CI/release assistant mode
- team/report export surfaces
- 稳定 extension SDK boundaries

带来的结果：

- 团队和高级用户可以把 Viden 嵌入开发流程，而不绕过它的 permission、evidence
  和 budget model。

### Horizon 6: 1.0 - Reliable Local AI Coding Operating Layer

目标：
让 Viden 可靠到可以推荐给真实项目日常使用。

1.0 标准：

- multi-provider setup 稳定
- 核心 TUI workflows 在常见 terminal sizes 和输入模式下可靠
- delegated lanes 可观察、可恢复
- context 和 budget 行为可见、可控
- permission 和 credential 边界有文档、有测试
- 多平台 release packaging 常规化
- 常见 workflow 有一等 docs 和 screenshots
- 外部集成不绕过 audit、permissions 或 evidence

## 版本规划原则

- 每个 release 都应该改进 observability、context efficiency、isolation、
  reviewability 或 repeatability 之一。
- 0.1.x final 是 TUI 稳定性出口：在 P0/P1 TUI bug 清零前，不进入 0.2.x。
- 每个用户可见功能都需要真实截图或 deterministic visual artifact。
- 新 adapters 先 read-only 或 supervised，再进入 mutating。
- 新 extension surfaces 先 descriptor/doctor/probe，再 invocation。
- 当前自动化不能解释自己之前，不增加更多自动化。
- 一个优秀的 end-to-end lane 优先于很多半可用 integrations。
- TUI polish 重要，但只在提升 operator confidence 时才优先。

## 推荐后续顺序

`0.1.24` 之后，建议顺序是：

1. `0.1.25`: TUI Display Cleanup。集中清理边框、竖线、颜色、IME、光标、modal
   位置、right rail drift 和提示框位置。
2. `0.1.26`: TUI Regression Pack。把历史显示 bug 做成 deterministic preview、
   terminal smoke 或人工截图 checklist。
3. `0.1.27`: Daily Coding Loop Hardening。用真实开发任务验证输入、审批、测试、diff、
   error recovery、scrollback 和 provider setup。
4. `0.1.28`: Delegated Lane Visibility Cleanup。确保 side screens、lane evidence、
   Codex/Claude/shell job 状态一致且不假显示。
5. `0.1.29`: 0.1.x RC Stabilization。停止扩大新 UI surface，只修 P0/P1 TUI bug。
6. `0.1.30`: 0.1.x Final Zero-Bug Gate。P0/P1 TUI backlog 清零、截图证据齐全、
   quick/full release gates 通过、GitHub Release 与 Homebrew 同步后，才进入 0.2.x。
7. `0.2.0`: Runtime 分层和事件闭环。把 core / TUI / provider / tool / lane /
   evidence 分清楚，通过统一 `RuntimeSnapshot` / event stream 传递 plan、build、
   approval、tool、provider 和 lane 状态，TUI 只订阅状态，不直接绑定业务逻辑。
8. `0.2.1`: Context 与 token/cost 引擎。实现 `ContextBundle`、语义文件选择、日志压缩、
   tool result 去重、token budget 和费用面板，解决长任务上下文膨胀、DeepSeek 413 和成本不可见；
   增加 canonical raw context、derived views、scoped handles/retrieval、确定性 reducers
   和 cost ledger，详见
   [Context、Evidence 与 Cost Engine 设计](superpowers/specs/2026-07-18-context-evidence-cost-engine-design.zh-CN.md)。
9. `0.2.2`: Agent 执行闭环。把 planner、coder、reviewer、tester、doc-writer 做成可监督角色，
   每个角色都有任务、输入、输出、证据、失败分类和下一步动作。
10. `0.2.3`: Evidence 与 Merge Gate。接受变更前必须具备 canonical task、context、
    permission、test、review、doc 和 release evidence。
11. `0.2.4`: Plugin Runtime Boundary。与 process-plugin/external-agent contracts 一起
    增加带 native fallback 的可选 context reducer adapters。
12. `0.2.5`: 真实开发场景 gate。每次发布必须运行 DeepSeek live development smoke
    和 Context Engine A/B token/cost/success evidence。
13. `0.3.0`: 多前端 Contract Freeze。冻结 UI/runtime contract 和 Viden migration plan，
    然后再进入并行 frontend 实现。
14. `0.3.1`: TUI 与 GUI 并行实现。Core/runtime、TUI、GUI 分支可并行推进，最多三个
    active owner，所有 frontend 都必须消费同一套 runtime。

这条线保持 Viden 的核心 wedge：不是最大自治，而是最大 operator trust。
