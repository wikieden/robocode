# RoboCode TUI Cockpit 设计

本文记录当前 TUI 目标，避免开发偏离前期生成的主视觉和终端 agent 工作流。

## 视觉基线

- 主视觉状态：**无弹窗态**。所有弹窗必须继承同一套 aurora-cyan cockpit
  主题，不再出现另一套配色。
- 主参考图：
  `docs/previews/tui-concept-holodeck-v1.png`。
- 布局目标：高密度终端 cockpit，不是介绍页。首屏应立即服务于编码、审查、
  审批和子 agent lane 监控。
- 配色方向：深蓝黑底、青色边框、绿色成功、黄色注意/权限、红色拒绝/错误。
  避免大块异常黑带或终端默认背景泄漏。

## 主屏幕

- 顶栏：产品、provider、model、session、context window、Git 分支、权限模式、
  active-lane 数量和 telemetry 可用性。
- Transcript：左侧主面板，时间线式消息，最近内容固定留在底部可见。
- Live activity：transcript 区内部固定保留一条当前运行状态，直接回答
  RoboCode 正在干什么，例如 `Thinking...`、`Editing src/render.rs`、等待审批，
  或正在监督 active lanes。
- 右侧栏：workspace、active tasks、diagnostics、provider health、recent files。
- Composer：始终在底部可见，输入区是更高的三行输入槽，输入光标位于输入行内
  并使用原生 blinking bar cursor，带 action hints 和 approval-mode chips。
- 底部状态栏：连接状态、session、event 数量、active lanes、context window、
  theme/help 提示。token、cost、rate 指标只有接入真实 provider telemetry 后才能显示。

## 数据真实性契约

- live TUI 面板不能用 demo 值冒充真实运行状态。
- 运行时数据未接入时，显示 `unavailable`、`0` 或明确 setup 提示，不能编造
  health、latency、cost、task、diagnostic 等值。
- demo 值只能出现在明确的 `--tui-preview*` fixture 路径。
- 右栏数据源：
  - workspace：`WorkspaceSnapshot::load_current`。
  - active tasks：pending approval 加统一 `AgentTask` 视图中的 running/queued
    terminal lanes 和 delegated Codex jobs。
  - diagnostics：`WorkspaceSnapshot.diagnostics`；后台 LSP 检查、真实
    `/lsp diagnostics <path>` 或 post-edit LSP 输出后会写入
    `.robocode/diagnostics.txt` cache；为空表示 unavailable/0。
  - provider health：`ProviderStatus` 来源于 `SessionEngine` 的
    `ProviderTelemetry`；request 数量、成功/失败数量、last/average latency、
    last event count 和 last error 都是真实值。rate、token、cost 在有真实运行时
    来源前保持隐藏。
  - recent files：文件系统 metadata 修改时间。
- 新增 cockpit 指标必须在代码或文档里标明运行时数据来源。

## 命令提示列表

当输入是顶层 slash 前缀 token，比如 `/` 或 `/p`，或受支持的二级命令 query，
比如 `/lane `、`/git st`、`/task status task_`、`/lsp diagnostics src/`，
命令提示列表显示在 composer 上方。

键盘契约：

- `Up` / `Down`：移动选中命令。
- `Tab`：把选中命令补全到 composer。
- `Enter`：补全部分命令；精确命令则提交。
- 鼠标左键：按下时选中可见提示行，松开时补全该命令。
- `Esc`：关闭当前 query 的提示列表。继续编辑 query 后重新打开。
- `/exit`、`/quit`、`exit`、`quit`：退出 TUI。

渲染契约：

- 交互决策类命令必须 selector-first。`/setup`、`/settings`、`/provider`、`/models`、
  `/lane`、`/permissions`、`/theme` 都要渲染居中的 selector panel，支持搜索、键盘移动、
  鼠标选择和 `Enter` 应用。未来新增的配置、模式切换、lane/agent 操作和多选项
  工作流也要沿用同一模式。除非是 `/config`、`/status` 或 `/provider doctor`
  这类明确诊断/详情命令，否则不要退化成只展示信息的页面。

- `/setup` 是 first-run wizard，不是被动 help 页。每一行都必须是真实动作，包括
  provider 配置、model 选择、permissions、theme、当前 provider doctor、fallback
  smoke 和保存默认值。

- `/lane` 是编排动作 selector。它会列出 lane 启动命令；已有 lane 时，还会列出带
  lane id 的 inspect、timeline、diff 和 artifacts 动作，避免用户记 lane id。

- provider 和 model selector 的语义必须分开：`/provider` 是供应商配置界面，
  一级列表只展示供应商 id，例如 `deepseek`、`openrouter`，不要在供应商行里混入
  key、endpoint、model 解释；选中供应商后进入二级 `PROVIDER CONFIG` 页面，再展示
  API key 环境变量状态、endpoint 来源、已知/默认模型候选、诊断、保存默认值和当前会话切换动作。
  默认动作应该是检查或配置供应商，而不是把 provider 选择伪装成 model 选择。
  `/models` 是跨供应商模型选择器，按 provider 分组，选中一行会补全/执行同时
  切换 provider 和 model 的命令。`/model <model>` 只表示当前 provider 内的快速
  模型切换，不能隐藏“跨供应商选模型需要切 provider”这件事。

- 提示列表使用与主 TUI 一致的 cockpit 边框、标题和行样式。
- 浮在 composer 正上方，不能遮挡输入光标。
- 展示命令、说明和选中行标记。
- 长提示列表会渲染一个可见窗口和 range hint；键盘移动时会调整窗口，保证选中项
  始终可见。鼠标 hit testing 会先从可见行映射回完整 suggestion index，再执行补全。
- 受支持的二级命令族会展示本地子命令和已知运行时对象。`/lane` 会提示 lane
  ID，`/screen close` 会提示已跟踪副屏，`/task` 会提示 task ID 和 task
  status，`/memory` 会提示可操作的 memory ID，`/provider use` 会提示已注册
  provider 和 descriptor 默认模型，`/model` 会提示当前 provider 的 descriptor
  默认模型，`/git diff`、`/git add`、`/git restore` 和 `/git stash push` 会提示
  workspace 文件路径，`/agent`、`/extensions`、`/mcp` 和 `/skills` 会提示只读
  可见性和诊断命令，`/git switch` 会提示本地分支，`/git push` 会提示本地分支、remote 和已知 remote branch target，
  `/git stash pop/drop` 会提示 stash ref，
  `/git worktree remove` 会提示 worktree 路径，`/lsp` 会提示 workspace 文件路径。

## 审批弹窗

审批弹窗是可交互 overlay，不是被动 transcript 卡片。

- `Tab` / `Shift-Tab` 和方向键在 apply-all、deny、diff、approve 间移动焦点。
- 默认焦点在 `Approve`，所以常见场景直接按 `Enter` 即可通过。
- `Enter` 执行当前焦点控件。
- checkbox 获得焦点时，`Space` 切换 apply-all。
- `y` 批准，`n` / `Esc` / `Ctrl-C` 拒绝。
- 鼠标点击可聚焦控件；在 deny 或 approve 上释放鼠标会完成审批。
- `d` 会聚焦 diff/evidence 区域。该区域优先展示当前 approval prompt 携带的真实
  preview 或 evidence 行；只有 prompt 没有 preview 内容时才使用小 fallback。
- 批准或拒绝后，pending 弹窗必须立即消失，transcript 和右栏不能留下样式残影。

## 多屏方向

TUI 支持一个主屏幕，最多两个副屏幕：

- 主屏幕：transcript、审批、命令输入、高层状态。
- 副屏 1：子 agent / terminal lane 监控。
- 副屏 2：诊断、构建状态、文件和 ops 上下文。

核心需求不是“好看”，而是监督多个终端编程工具，例如 Codex、Claude Code、
shell job、DeepSeek lane。副屏需要暴露任务状态、最新输出、产物、进度和路由
提示，让主 agent 能判断后续动作。

## Agent Bridge 产品契约

cockpit 下一阶段的核心体验是 host-delegate agent bridge，参考 Claude Code
Codex 插件的模式：

- RoboCode 是 host。它接收用户主目标，并保持 operation center、composer、
  approval 和 side screens 的一致性。
- Codex 是第一个 delegate。它作为 tracked job/lane 运行，具备 readiness doctor、
  launch path、job record、cancel/result commands 和 evidence output。
- Claude Code、DeepSeek TUI、shell、tmux/PTY、plugin、skill、MCP 和 ACP agents
  后续都应该接入同一套 lifecycle，而不是各自拥有一套一次性面板。
- UI 必须在主屏回答“delegate 现在在干什么”，不能只把状态放到副屏。副屏负责深度，
  主屏仍是 operator 的事实表面。
- delegate 和 terminal work 必须在 TUI 中进入同一套可见 `AgentTask`
  模型。Codex jobs、Claude/DeepSeek/shell lanes、tmux sessions、PTY
  bridges 和未来 ACP agents 在拥有更丰富的 agent 专属控制前，应共享 id、
  agent、transport、status、activity、progress、evidence、pid 和 result
  概念。
- delegate 结果必须转成 evidence：changed files、commands、test output、
  final summaries、errors，以及 resume/thread handles。
- write-capable delegate work 必须显式并经过权限控制；review 和 diagnostics 可以
  保持 read-only。

## AgentTask 运行模型

`AgentTask` 是 TUI 的统一观察模型，用来回答“现在谁在工作、做到了哪一步、证据在哪里”。
它不是新的 source of truth，而是从 transcript events、pending approvals、
provider telemetry、test evidence、lane artifacts、Codex job records、tmux/PTY
logs 和未来 ACP events 归一化出来的运行时视图。

最小字段：

- `id` / `parent_id`：支持主回复和子 agent / tool work 关联。
- `agent` / `kind` / `transport`：区分 `robocode`、`codex`、`claude`、
  `deepseek`、`shell`、`mcp`、`skill`、`acp`，以及 `provider`、`tool`、
  `lane`、`job`、`test`、`approval`。
- `status`：统一使用 `queued`、`thinking`、`streaming`、`editing`、
  `running_tool`、`testing`、`waiting_approval`、`needs_input`、`blocked`、
  `done`、`failed`、`cancelled`、`archived`。
- `activity` / `summary` / `progress`：给主屏中央状态和 side-screen row 使用。
- `workspace` / `evidence` / `permissions` / `decision` / `result` /
  `resume_handle`：把可审计证据、权限边界和后续动作连起来。Evidence 行应优先展示
  command、failure、conflict、path、changed files、patch artifact 和
  review/apply result，再展示泛化 transcript 标签。

主回复状态也必须进入 `AgentTask`：用户提交后显示 `thinking/streaming`，
工具调用时显示 `running_tool`，审批阻塞时显示 `waiting_approval`，完成后显示
`done` 和最后摘要。主屏 operation center、右栏 `ACTIVE TASKS`、side-1 lane
列表、side-2 evidence、`/agent status` 和 `/lane inspect` 必须读取同一份
normalized view，不能各自拼接一套状态。

## 当前实现备注

- 主屏和副屏都会响应 resize 事件并重绘。
- 行级 diff 渲染避免输入时整屏闪烁。
- provider turn 通过 worker/channel 边界执行，主 TUI event loop 会继续轮询输入、
  lane artifacts、approval prompt 和 repaint tick。审批 prompt 会桥接回 UI，并沿用
  现有 permission path 处理，完成后 worker 才继续。
- composer 已按显示宽度处理中文等 CJK 输入；输入行保持原生 blinking bar
  cursor，并预留更高输入槽，让长会话里也容易找到输入位置。
- 主 transcript 顶部会保留固定的 `NOW WORKING` 区域。它从 pending
  approvals、统一 `AgentTask` view、最近 user turn、最近 tool call 或最近
  transcript entry 推导状态，所以主屏可以展示 `reply thinking`、
  `DeepSeek is thinking`、`Approval needed: ...`、`Supervising 2 agents: ...`、
  紧凑 edit 摘要、delegate-agent progress 和 evidence
  source，同时不编造运行时数据。
- Approval overlay 和 `waiting_approval` task 只有在仍然 live 时才算阻塞；如果后续
  approval resolution、tool result、assistant reply 或 `/test` command result
  已经闭环，就不能继续占用 operation center 或 modal layer。
- 失败测试和 lane conflict 要显示成 operator action，而不是仅作为 log：先展示失败
  command 或 conflict summary，再展示 next action（`open failure, patch, rerun
  tests` 或 `inspect conflict and revise/apply`）。
- `/diff` 和 `/git diff` 输出也属于 `AgentTask`：非空 diff 使用 `kind=diff`、
  `status=needs_input`，带 files/additions/deletions/path evidence，并提示先 review
  diff 再测试或提交。
- transcript-derived tasks 会分别保留最近的 diff、test、tool 和 provider
  representative entry，让 side-2 能把当前 review surface 和最近 verification /
  edit evidence 放在一起比较。
- slash 提示列表是本地 UI 状态，不触发模型调用。它现在支持 `/lane`、
  `/agent`、`/extensions`、`/mcp`、`/skills`、`/screen`、`/provider`、`/lsp`、
  `/task`、`/memory` 和 `/git` 的二级提示；
  当前 TUI state 能提供对象时，会显示动态 ID 或最近文件。provider 和 model 提示
  会读取当前 runtime provider registry descriptor，所以 `/provider use` 能提示
  已注册 provider ID 和已知 descriptor 默认模型，`/model` 能提示当前 provider 的
  默认模型。memory 操作会读取 workflow memory snapshot，所以 `/memory confirm`、
  `/memory reject` 和 `/memory prune` 会提示相关 memory ID，不再要求操作者手动
  复制。Agent 和 extension 命令现在提供 `/agent list`、`/agent doctor`、
  `/agent review codex`、`/agent challenge codex`、`/agent run codex`、
  `/agent run codex --write`、`/agent status`、`/agent result`、`/agent cancel`、`/extensions list`、
  `/extensions doctor`、`/mcp list`、`/mcp doctor` 和 `/skills list` 提示，在 runtime extension execution 超出共享 permission path 前先把可见性补上。Git 和 LSP 路径提示会复用右栏收集到的 workspace 文件快照，不会在操作者
  每次输入时重新扫描文件系统。`/git switch` 和 `/git push` 会读取当前 workspace
  的本地分支快照；`/git push` 还会读取 `git remote` 和 `git branch -r` 快照，用于
  提示 remote 和 remote branch target。`/git stash pop/drop` 会读取当前
  `git stash list` 快照并提示 stash ref；`/git worktree remove` 会读取当前
  `git worktree list --porcelain` 快照并提示 worktree 路径。长列表现在按窗口展示，
  选中行保持可见，footer hint 会显示当前可见范围。
- `/agent list` 和 `/agent doctor` 会展示 template、tmux、PTY、
  custom-template、Codex 和实验 ACP adapters。Codex readiness 会检查本地
  `codex` binary、app-server support、auth、config sources 和 job store path。
  `/agent review codex`、`/agent challenge codex` 和 `/agent run codex [--write]` 会在
  `.robocode/agents/` 下创建 tracked jobs；`/agent status`、
  `/agent result <id>` 和 `/agent cancel <id>` 用于查看和控制这些 jobs。
  `--write` 必须显式传入，并且会先经过 RoboCode mutating permission prompt，
  approval 后才让 Codex 以 `workspace-write` sandbox 启动。Codex jobs 会保存启动时 Git status baseline，并从 result/log output 中提取
  resume/session hints 和 touched-file evidence，所以 status/result 视图能在可用时显示
  `codex resume ...` 和相关文件。TUI 也会读取 app-server result/log artifacts，
  提取 thread、turn、status、approval、resume、command-output、file-change、
  patch、diff、filesystem、error 和 final-message evidence。App-server result
  summary 会把最终 `agentMessage` text 持久化为 `message:`，所以 `/agent
  result`、side-2 和 `AgentTask` 对 delegate answer 的展示保持一致。主窗口 `NOW WORKING` 区域和右栏
  `ACTIVE TASKS` panel 会读取同一份 job records，所以 operator 继续输入时也能看到
  Codex 是否仍在工作。ACP readiness 通过 `ROBOCODE_AGENT_ACP_COMMAND` 配置；`/agent doctor acp`
  可以运行最小 JSON-RPC `initialize` handshake，并写入
  `.robocode/agents/acp-doctor-*.jsonl` evidence。完整 `/lane acp` 执行仍是后续工作。
- `/test <command>` 是真实 runtime command，不是视觉占位符。它会走 shell
  approval，记录最近一次测试的 status、exit code、duration、command、failure
  summary、可能失败文件和 output tail，并通过 `/status` 展示紧凑状态。失败测试输出
  也会归一化成 `AgentTask.evidence` 行（`failure`、`failing-file`、`tail` 和
  `rerun <command>`），让 side-2 和主屏 operation center 可以引导 patch/rerun
  恢复闭环。
- `/extensions doctor` 和 `/mcp doctor` 是 readiness reports，不是占位符：会展示
  provider plugin dirs、MCP config files 和 server names、project/user/legacy
  skill root counts，以及 extension mutation 必须进入共享 tool permission path
  的边界。
- 主屏 idle 时会轮询 lane artifacts，所以后台 `/lane run` 的完成、失败和
  log-tail 状态不需要按键也会刷新。
- 副屏 2 是 ops/evidence cockpit。它渲染 `TESTS / LSP`、`MCP / CONTEXT`、
  `EXTENSIONS` 和 `RECENT EVIDENCE` 面板。测试行来自真实 `/test` transcript
  evidence，LSP 行读取 `WorkspaceSnapshot.diagnostics`，MCP 行检查 workspace/user
  config 文件路径，extension 行汇总 provider/catalog/lane/MCP/skill readiness，
  `RECENT EVIDENCE` 行读取统一 `AgentTask` runtime view，展示 approval、tool、
  lane、Codex job 的 `id / agent / status / progress / activity`，并把
  `evidence`、`decision`、`result` 和下一步 operator action 作为二级行。
  对 failed/blocked task，二级行优先展示 command、failure、failing-file、tail、
  rerun、path、lines 和 changed files，确保 operator 先看到可以继续行动的证据。
  completed app-server text turn 会优先展示最终 `message ...` evidence，再展示
  低信号的 protocol id。
- 右侧栏 `ACTIVE TASKS` 面板会读取 `/task` 和 `/tasks` 背后的真实 workflow
  task store，并把这些 task record 与 pending approval、active lane 合并展示。
- live 副屏只读取持久化 lane 状态；如果没有 lane store，会显示空状态，而不
  回退到 preview/demo lane。
- `/lane inspect <id>` 会读取持久化 lane artifacts：`.log` 尾部、`.done`
  exit code、log path、done path、envelope path、terminal attach/tmux/PTY
  artifact path、timeline rows 和 envelope preview。`/lane timeline <id>` 会聚焦同一份
  持久化 event chronology，方便 operator review。
- template-launched Codex 和 Claude lane 会在 Git `HEAD` 可用时运行于
  `.robocode/worktrees/` 下的 per-lane 隔离 worktree。task envelope 会记录
  lane workspace、mutation scope、isolation warnings 以及 cleanup/verification hints。
- `/lane inspect <id>` 还会展示相关 changed-file snapshot：隔离的外部 lane
  使用 lane worktree，非隔离 shell lane 使用当前 workspace。它也会展示来自
  exit/log artifact 的 verification evidence，以及显式 lane decision artifact。
- `/lane accept <id>`、`/lane revise <id>` 和 `/lane discard <id>` 会把操作者的
  明确决策记录到 `.robocode/lanes/<lane-id>.decision.md`。
- `/lane apply <id>` 会把已 accepted 的隔离 lane worktree 通过可审计 Git
  patch 应用回当前 workspace。它会写入
  `.robocode/lanes/<lane-id>.apply.patch` 和
  `.robocode/lanes/<lane-id>.apply.md`；除非显式传入 `--force`，否则会拒绝
  未 accepted 的 lane；它不会自动 commit，也不会删除 lane worktree。
  如果 patch 无法干净应用，RoboCode 会保持主 workspace 不变，把 lane 标为
  `apply_conflict`，并写入 `.robocode/lanes/<lane-id>.apply-conflict.md`，
  记录直接 apply check、three-way apply check 和 changed-file 上下文。
- `/lane resolve <id>` 会在操作者已经调整主 workspace 或 lane worktree 后，
  重试一个 `apply_conflict` lane。它复用 `/lane apply` 的可审计 patch 路径：
  Git patch 必须先通过 `git apply --check`，RoboCode 才会修改主 workspace。
  干净重试会写入正常的 `.apply.md`；仍有冲突时会刷新 `.apply-conflict.md`。
- `/lane cleanup <id>` 会通过移除隔离 worktree 来归档 lane，但只有 worktree
  干净时才会执行。有未提交变更时必须显式使用
  `/lane cleanup <id> --force`，并且每次 cleanup 都会先写入
  `.robocode/lanes/<lane-id>.cleanup.md`。
- `/lane archive <id>` 会记录 `.robocode/lanes/<lane-id>.archive.md` 并把
  lane 标记为 archived，但不会删除日志、决策、apply 记录或隔离 worktree。
  仍处于 queued/running/attached 的 live lane 必须先 stop、完成或 detach。
- `/lane attach <id>` 会为 lane workspace 打开交互式终端，并记录
  `.robocode/lanes/<lane-id>.attach.md`。`/lane detach <id>` 只清除 attached UI
  状态，不会杀掉外部 terminal 进程。
- `/lane tmux <id>` 会为 lane workspace 创建或复用命名 tmux session。side-1
  lane monitor 和聚焦 lane modal 会对已 attached 的 tmux lane 直接显示
  `tmux attach -t ...` 命令；对尚未 attached 的 lane，则显示 `/lane tmux <id>`
  作为下一步交互入口。使用默认 tmux template 时，pane 输出会 pipe 到标准 lane
  `.log`，所以副屏和 `/lane inspect` 可以观察实时 tmux 输出。
- `/lane pty <id>` 会为 lane workspace 启动 embedded PTY bridge。它创建
  `.robocode/lanes/<lane-id>.pty.in` 作为输入 FIFO，写入
  `.robocode/lanes/<lane-id>.pty.md` 作为审计记录，并把输出捕获到标准 lane
  `.log`。`/lane inspect <id>` 会展示这些 PTY artifact path，`/lane send <id>
  <text>` 可以从 TUI 内向该 PTY bridge 写入一行输入。
- side-1 的 `LIVE OUTPUT` 和聚焦 lane modal 会在可用时回放最新持久化 lane
  `.log` 尾部；只有尚未捕获到 terminal 输出时，才回退显示 lane summary。
  这样 tmux、PTY 和后台 lane 在完整 terminal emulator 落地前，也已经有一段
  cockpit 内的 screen-state 切片。
- side-1 lane 行现在使用统一 agent 语言，直接展示 transport（`template`、
  `tmux`、`pty` 或 `shell`）和 state（`thinking`、`editing`、`testing`、
  `needs input`、`blocked` 或 `done`），并和 attach/evidence 提示放在一起。
- Provider health 已接入共享 runtime loop 测量到的模型请求 telemetry：真实
  request 数、成功/失败数、last/average latency、last event count、
  provider 返回的 token usage、请求耗时允许时的 token throughput，以及最后一次
  provider error。
- Provider health 指标行按稳定的 label/value 结构渲染：左侧 label 使用指标色，
  `Configured`、`0 ok / 0 err` 等值保持正文色，避免紧凑右侧栏里出现单词内部或
  同一指标值被切碎染色的效果。
- TUI 会解析来自 core 真实事件的 LSP diagnostics，并持久化到
  `.robocode/diagnostics.txt`，所以主屏和副屏可以展示同一份有证据来源的
  diagnostics snapshot。
- `/screen side-1` 和 `/screen side-2` 现在会用当前 provider、model、theme
  和 workspace 启动真实副屏 TUI 进程。主屏最多跟踪两个副屏，`/screen list`
  显示状态，`/screen close <side-1|side-2>` 会停止跟踪，并在已知 pid 时发送
  终止请求。
- screen registry 会持久化到 `.robocode/screens.tsv`，所以主屏和副屏进程可以
  观察同一份 companion-screen 状态。
- `ROBOCODE_SCREEN_SIDE_1_LAUNCH_TEMPLATE` 和
  `ROBOCODE_SCREEN_SIDE_2_LAUNCH_TEMPLATE` 可以为每个副屏覆盖默认的当前二进制
  启动方式，`ROBOCODE_SCREEN_LAUNCH_TEMPLATE` 作为共享 fallback。支持
  `{screen}`、`{title}`、`{role}`、`{display}`、`{display_index}`、
  `{provider}`、`{model}`、`{theme}`、`{cwd}`、`{binary}`、`{args}` 以及
  shell-quoted 的 `{name:q}` 占位符。这样操作者可以把副屏交给 Terminal.app、
  iTerm、tmux 或显示器摆放脚本启动。
- `ROBOCODE_LANE_CODEX_TEMPLATE` 和 `ROBOCODE_LANE_CLAUDE_TEMPLATE` 支持
  `{tool}`、`{task}`、`{envelope}`、`{cwd}`、`{worktree}` 以及 shell-quoted 的
  `{name:q}` 形式。`{cwd}` 和 `{worktree}` 都会解析为真实 lane workspace。
- `/lane ask <tool> <task>` 使用 `ROBOCODE_LANE_<TOOL>_TEMPLATE` 接入自定义受控
  外部工具，例如 Gemini、Junie 或本地 coding CLI。template 未配置时会保留已渲染
  task envelope，并让 lane 停在 queued 状态，而不是丢掉请求。
- `ROBOCODE_LANE_PTY_TEMPLATE` 可以覆盖 embedded PTY bridge。它支持
  `{lane}`、`{task}`、`{tool}`、`{cwd}`、`{worktree}`、`{command}`、`{input}`、
  `{log}`、`{shell}` 以及 shell-quoted 的 `{name:q}` 形式。默认 Unix template
  使用系统 `script` 命令和 lane 输入 FIFO。
- `ROBOCODE_LANE_ATTACH_TEMPLATE` 可以覆盖默认 lane attach launcher。它支持
  `{lane}`、`{task}`、`{tool}`、`{cwd}`、`{worktree}`、`{log}` 以及
  shell-quoted 的 `{name:q}` 形式。macOS 有默认 Terminal.app launcher；其他
  平台应提供该 template，例如 tmux 或桌面 terminal 命令。
- 修改 cockpit 行为、命令、架构、配置或 UI 时，必须同步更新相关文档。注释
  用来说明不明显的不变量和安全边界，不重复解释显而易见的代码。

## 近期缺口

- embedded PTY 已经有第一版受控 bridge：`/lane pty` 与 `/lane send`。更完整的
  cockpit 会在副屏和聚焦 lane modal 回放最近的持久化 lane log tail；更完整的
  inline terminal emulator、cursor-addressed screen-state replay 仍是后续工作。
- apply 当前通过 `/lane apply <id>` 走保守 patch 路径，并在 patch 无法干净应用
  时记录 conflict report。`/lane resolve <id>` 提供人工清理冲突后的操作者重试
  闭环；完整的内联 conflict editor 仍是后续工作。discard lane 只记录决策，
  不会默认删除日志、worktree 或变更；清理必须通过单独的 `/lane cleanup` 命令
  执行。
- Provider token telemetry 现在来自 OpenAI-compatible、Anthropic 和
  Ollama-style 响应中的真实 `usage` payload。token rate 只有在同时有 usage 和
  非零请求耗时时才会计算。cost 仍只在 provider 返回 cost 数据时显示；RoboCode
  不会在 TUI 里编造价格。
- Diagnostics 来源于共享 LSP runtime：post-edit diagnostics、显式
  `/lsp diagnostics <path>`，以及 live TUI 对 workspace Rust 文件的节流后台检查。
  如果项目没有配置或无法启动 language server，仍显示 `diagnostics unavailable`。
- 主屏渲染现在统一使用 display-cell 宽度辅助函数处理中文、emoji modifier、
  combining mark、transcript 换行、topbar fit 和审批预览边框，避免长时间多语言
  会话把右侧栏挤歪。
- 副屏启动已经是真实进程管理。跨物理显示器的窗口摆放现在有了明确的
  per-screen launcher-template 集成点，但 OS 级窗口移动仍由已配置的 terminal
  或显示器摆放脚本负责。
- 命令提示列表的二级提示已覆盖主要命令族，也覆盖常见 Git 和 LSP 路径类命令的
  workspace 文件路径提示。
- 视觉还需要持续用截图和 holodeck 主参考图对比。
