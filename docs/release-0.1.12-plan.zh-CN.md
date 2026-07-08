# Viden 0.1.12 计划

英文版： [release-0.1.12-plan.md](release-0.1.12-plan.md)

最后更新：2026-05-27

## 版本定位

`0.1.12` 是 Agent Orchestration Operator Loop 版本。

`0.1.11` 已经把 TUI 可靠性、`NOW WORKING`、`AgentTask` / `AgentLane`
投影、截图回归和 token/context 设计打成基础。`0.1.12` 的目标不是继续堆面板，
而是把这些基础变成真实编程工作流：

- 用户把一个开发任务交给 Viden。
- Viden 能拆出可监督的 agent/lane 工作。
- 主屏能持续说明当前谁在工作、在做什么、证据在哪里、下一步需要谁决策。
- side screen 能承担真正的观察、控制、review、apply 工作。
- token/context 的输入输出开始被预算和压缩约束，而不是只做展示。

这个版本仍然属于 `0.1.x`：目标是可用的 operator loop，不宣称完整 `0.2.0`
多 Agent runtime。

## 前几个版本复盘

### 0.1.8：AgentTask 地基很关键，但范围偏宽

`0.1.8` 把 `AgentTask` runtime view、operation center、side-2 evidence、
Codex app-server fixture、lane operator loop smoke 都打进来了。这证明方向是对的：
Viden 必须有统一事实层，不能每个面板各自拼状态。

反思：

- 当时覆盖面很广，provider、tool、lane、Codex、diff、test、approval 都进来了，但用户
  真正能稳定感知的“一条完整编程闭环”还不够清晰。
- `AgentTask` 的字段够多，但运行时写入、优先级选择、下一步操作还需要更像 product
  workflow，而不是只是 projection。

### 0.1.9：验证体系补得及时

`0.1.9` 把 release smoke、clippy gate、TUI regression screenshot 和发布后检查做成标准门禁。
这是后续版本能快速发布的基础。

反思：

- 确定性截图很适合防布局回归，但不能替代真实终端交互验证。
- 后续每个 TUI 功能需要同时有 snapshot evidence 和 manual terminal checklist。

### 0.1.10：用户最需要的是“远端是否在工作”

`0.1.10` 把 provider turn 开始前的 pending `AgentTask` 接入主屏，让用户能看到 provider
正在 thinking。

反思：

- 这是非常高价值的体验点，说明 `NOW WORKING` 必须优先服务“当前状态不确定”的焦虑。
- 但 provider turn 只是第一类远端工作；shell/test、lane、approval、review/apply 也要进入同一机制。

### 0.1.11：TUI 可靠性是前置条件，但不是最终价值

`0.1.11` 加强了 resize、中文输入、`NOW WORKING` 命名、`AgentLane` 投影和 token/context
设计，并完成 GitHub release / Homebrew 发布闭环。

反思：

- TUI 稳定性是继续做多 agent 的门槛，但用户核心需求仍是“帮我编排多个编程工具完成任务”。
- side screen 不能只是状态橱窗；它们必须变成 operator control surface。
- token/context 不能停在文档层，需要至少对一次 provider/lane prompt 产生实际影响。

## 版本切线

`0.1.12` 的切线是：**一个真实可用的 operator-loop vertical slice**。

优先级从高到低：

1. **P0：统一运行时事实层。** provider、tool、shell/test、lane、approval 的关键动作必须写入
   `AgentTask`，并能被 `NOW WORKING`、side-1、side-2 一致读取。
2. **P0：跑通一个稳定 lane 闭环。** 先以 deterministic `shell/template` lane 作为可测试
   baseline，完成 dispatch -> observe -> review -> apply/discard；Codex/Claude 复用同一模型，
   但不把所有外部 agent 全部做满作为 P0。
3. **P0：ContextBundle v0 真正参与一次 prompt 构造。** 至少在 provider turn 或 delegated
   lane 中记录 context sources、估算 token、压缩长 tool output。
4. **P1：Codex/Claude adapter 深化。** 在 P0 闭环稳定后，把 Codex/Claude 的 status、tail、
   result、review/apply 接入同一 operator loop。
5. **P1：真实终端体验验收。** Terminal / iTerm2 的中文输入、resize、approval、鼠标交互继续
   要保留人工截图或录像证据。
6. **P2：Extension/ACP/MCP 扩展。** 先做 descriptor、doctor、probe、capability 和 event
   mapping；不抢 P0 闭环资源。

## 核心目标

### 1. 统一 AgentTask 成为运行时事实层

`0.1.11` 主要完成 TUI 读取同一投影。`0.1.12` 要进一步让关键运行时动作进入同一
`AgentTask` 生命周期。

必须覆盖：

- provider turn：thinking、streaming、tool-call、completed、failed。
- tool call：approval required、running、result、failed。
- shell/test：command、pid/exit、duration、tail、artifact。
- external lane：Codex、Claude、DeepSeek、shell/template、tmux/PTY 的 start、tail、review、apply、stop。
- approval：pending、approved、denied、default action、decision evidence。

每个 `AgentTask` 至少要有：

- id、agent/provider/lane、transport、status、started_at、updated_at。
- objective / current_action。
- evidence rows：transcript event、tool call、diff、test result、artifact、log tail。
- next_action：wait、approve、inspect、attach、send、review、apply、stop、retry。

### 2. 主屏 `NOW WORKING` 变成真实操作中枢

用户输入后，主屏必须立刻回答“现在到底在干嘛”。

必须实现：

- provider 等待远端响应时显示 thinking/streaming 状态、elapsed time、provider/model。
- tool call 等待审批时显示审批类型、默认操作、快捷键提示。
- shell/test 运行时显示命令、运行时间、最后输出摘要。
- external agent/lane 运行时显示 lane、transport、当前阶段和最新 evidence。
- 多个任务同时存在时显示优先级最高的 active task，并提示还有多少 background tasks。

这个区域不能只是视觉装饰；它必须从真实 `AgentTask` 状态读取。

### 3. Side Screen 变成真实 Agent 控制台

`side-1` 和 `side-2` 要从“更多面板”升级为 operator console。

`side-1` 聚焦 Agent/Lane：

- 列出 active / completed / failed lanes。
- 支持 inspect、attach、send、stop、retry 的命令入口。
- 能看到每个 lane 的状态、elapsed、last output、artifact、next action。
- 与主屏 `NOW WORKING` 使用同一 `AgentTask` 数据。

`side-2` 聚焦 Evidence/Ops：

- 汇总 test、diff、diagnostics、git、tool output、artifact。
- 显示最近失败和阻塞原因。
- 为 review/apply 决策提供证据，而不是只显示静态占位。

### 4. 编程闭环：dispatch -> observe -> review -> apply

`0.1.12` 要先把一个最小可用的多 agent 编程闭环跑通。

目标流程：

1. 用户提交开发任务。
2. Viden 生成或接受一个小 plan。
3. 用户把子任务派发到一个 lane，例如 Codex、Claude、shell/template 或 DeepSeek。
4. lane 运行过程被 `AgentTask` 观测并显示。
5. 结果进入 review 状态，展示 touched files、diff、test/evidence。
6. 用户可以 accept/apply/discard/retry。
7. 最终结果回写 transcript、workflow event 和 recent evidence。

优先支持一个稳定 happy path，再扩展更多 agent 类型。

### 5. Extension/Adapter 地基

这个版本要把 plugin、skill、MCP、ACP 的边界设计成可继续实现的结构。

必须完成：

- 统一 descriptor 文档：provider plugin、agent adapter、skill、MCP server、tool surface。
- `/extensions doctor`、`/mcp doctor`、`/skills list` 的输出继续保持真实诊断。
- ACP 继续作为实验性 adapter：优先做 probe、capability、job envelope、event mapping，不做完整编辑器级 host。
- 所有 extension 调用必须走共享 permission、transcript、evidence、token budget 边界。

### 6. 真实终端体验继续收敛

`0.1.11` 已把 resize 和中文输入加入确定性 preview。`0.1.12` 需要继续把真实终端中的体验问题纳入版本目标。

必须覆盖：

- macOS Terminal 和 iTerm2 中的 resize redraw。
- 中文输入法候选窗位置和输入光标可见性。
- approval modal 的键盘默认通过、快捷键、鼠标点击。
- side screen 打开/关闭、focus 切换、lane attach/send 的可操作性。

这部分不能只靠 SVG snapshot；需要保留实际终端截图或手工验证记录。

### 7. ContextBundle / Token 效能 v0

`0.1.12` 要从设计进入最小实现。

必须完成：

- 为 provider turn 或 delegated lane 构造 `ContextBundle` v0。
- 记录 context sources：user task、selected files、diff、diagnostics、tests、memory、lane summaries。
- 长 tool output 使用 summary + tail 进入 prompt，原始输出仍保留 transcript/audit。
- side/status 显示 context pressure、estimated tokens、largest sources。
- 每个 agent/lane 可以有预算字段：soft budget、hard limit、current estimate。

### 8. 截图与真实使用证据

每个用户可见功能点完成后，都要留下截图或确定性 TUI 视觉产物。

至少保留：

- provider thinking 的 `NOW WORKING`。
- shell/test running 的 `NOW WORKING`。
- approval pending/default approve。
- side-1 active lane control。
- side-2 evidence/ops。
- lane review/apply 决策。
- ContextBundle/token pressure。
- 多任务 background 状态。

## 建议实施顺序

### Milestone A：AgentTask runtime write path

- 梳理现有 projection-only 状态来源。
- 建立统一 `AgentTask` update path 或 reducer。
- provider、tool、shell/test、approval 先接入。
- 增加 focused tests，证明同一 task 在 `NOW WORKING`、right panel、side-2 中一致。

### Milestone B：deterministic lane operator loop

- 选择 `shell/template` lane 作为 P0 baseline。
- 完成 dispatch、observe、tail、review、apply/discard、retry/stop。
- 生成对应 TUI screenshot：active lane、review、apply result、side-2 evidence。

### Milestone C：ContextBundle v0

- 定义并实现最小 `ContextBundle` 构造路径。
- 对长 tool output 做 summary + tail。
- 在 TUI 中显示 context sources、estimated tokens、pressure。
- 加测试证明原始 transcript 不丢、prompt 输入被压缩。

### Milestone D：Codex/Claude adapter 复用

- 不重写一套 UI；只把 Codex/Claude 的 status/result/tail 映射到同一 `AgentTask` 和 lane API。
- 增加 adapter doctor/probe 和一个可复现 smoke。
- 保留不能稳定自动化的真实终端验证项。

## 非目标

- 不在 `0.1.12` 宣称完整 `0.2.0` 多 Agent runtime。
- 不做 marketplace、远程团队协作、Web/Desktop/IDE 入口。
- 不接入完整 MCP mutating tool runtime，除非走完 permission/evidence 边界。
- 不把 ACP 做成完整 Zed 级 host；只做 adapter/probe/job/event 基础。
- 不为了“看起来多 agent”增加没有真实状态支撑的假面板。
- 不同时铺开所有外部 coding agent；先保证一个 operator loop 真正闭环。

## 验收标准

- 用户能把一个小型编程任务派发到至少一个 agent/lane，并在 TUI 中观察全过程。
- `NOW WORKING`、side-1、side-2、recent evidence 对同一个任务状态不矛盾。
- provider/tool/shell/lane 的关键动作都能映射成 `AgentTask`。
- review/apply/retry/stop 至少有一个稳定 happy path。
- ContextBundle v0 能说明 prompt 里用了哪些上下文、估计多少 token、哪里压力最大。
- 每个用户可见交互都有截图证据。
- 文档明确哪些能力是真功能，哪些仍是实验性 adapter 或只读诊断。
- P0 功能必须能通过 deterministic smoke 验证；外部 agent 的不稳定项只能列为 P1/P2 或人工验证项。

## 验证

至少运行：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/release-smoke.sh --version 0.1.12 --deepseek --out-dir /tmp/viden-0112-release-smoke-full
```

人工验证：

- macOS Terminal / iTerm2 各跑一次 TUI。
- 真实 provider turn：观察 thinking -> streaming -> completed。
- 真实 shell/test：观察 running -> result。
- 至少一条 external lane：dispatch -> observe -> review -> apply/discard。
- 中文输入、resize、approval、命令提示列表继续回归。

## 后续承接

`0.1.12` 完成后，进入 `0.1.13` 或 `0.2.0` 的判断标准：

- 如果 operator loop 仍不稳定，继续 `0.1.13` 做 reliability + review/apply hardening。
- 如果 operator loop 已经稳定，进入 `0.2.0`：Agent Orchestration Runtime v1，
  默认 planner -> worker -> reviewer -> tester workflow，以及更完整的
  ContextBundle/token efficiency engine。
