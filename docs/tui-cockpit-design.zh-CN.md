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
- 右侧栏：workspace、active tasks、diagnostics、provider health、recent files。
- Composer：始终在底部可见，输入光标位于输入行内，带 action hints 和
  approval-mode chips。
- 底部状态栏：连接状态、session、event 数量、active lanes、context window、
  theme/help 提示。token、cost、rate 指标只有接入真实 provider telemetry 后才能显示。

## 数据真实性契约

- live TUI 面板不能用 demo 值冒充真实运行状态。
- 运行时数据未接入时，显示 `unavailable`、`0` 或明确 setup 提示，不能编造
  health、latency、cost、task、diagnostic 等值。
- demo 值只能出现在明确的 `--tui-preview*` fixture 路径。
- 右栏数据源：
  - workspace：`WorkspaceSnapshot::load_current`。
  - active tasks：pending approval 加 running/queued terminal lanes。
  - diagnostics：`WorkspaceSnapshot.diagnostics`；真实 `/lsp diagnostics <path>`
    或 post-edit LSP 输出后会写入 `.robocode/diagnostics.txt` cache；为空表示
    unavailable/0。
  - provider health：`ProviderStatus` 来源于 `SessionEngine` 的
    `ProviderTelemetry`；request 数量、成功/失败数量、last/average latency、
    last event count 和 last error 都是真实值。rate、token、cost 在有真实运行时
    来源前保持隐藏。
  - recent files：文件系统 metadata 修改时间。
- 新增 cockpit 指标必须在代码或文档里标明运行时数据来源。

## 命令提示列表

当输入是单个 slash 前缀 token，比如 `/` 或 `/p`，命令提示列表显示在
composer 上方。

键盘契约：

- `Up` / `Down`：移动选中命令。
- `Tab`：把选中命令补全到 composer。
- `Enter`：补全部分命令；精确命令则提交。
- `Esc`：关闭当前 query 的提示列表。继续编辑 query 后重新打开。
- `/exit`、`/quit`、`exit`、`quit`：退出 TUI。

渲染契约：

- 提示列表使用与主 TUI 一致的 cockpit 边框、标题和行样式。
- 浮在 composer 正上方，不能遮挡输入光标。
- 展示命令、说明和选中行标记。

## 审批弹窗

审批弹窗是可交互 overlay，不是被动 transcript 卡片。

- `Tab` / `Shift-Tab` 和方向键在 apply-all、deny、diff、approve 间移动焦点。
- 默认焦点在 `Approve`，所以常见场景直接按 `Enter` 即可通过。
- `Enter` 执行当前焦点控件。
- checkbox 获得焦点时，`Space` 切换 apply-all。
- `y` 批准，`n` / `Esc` / `Ctrl-C` 拒绝。
- 鼠标点击可聚焦控件；在 deny 或 approve 上释放鼠标会完成审批。
- 批准或拒绝后，pending 弹窗必须立即消失，transcript 和右栏不能留下样式残影。

## 多屏方向

TUI 支持一个主屏幕，最多两个副屏幕：

- 主屏幕：transcript、审批、命令输入、高层状态。
- 副屏 1：子 agent / terminal lane 监控。
- 副屏 2：诊断、构建状态、文件和 ops 上下文。

核心需求不是“好看”，而是监督多个终端编程工具，例如 Codex、Claude Code、
shell job、DeepSeek lane。副屏需要暴露任务状态、最新输出、产物、进度和路由
提示，让主 agent 能判断后续动作。

## 当前实现备注

- 主屏和副屏都会响应 resize 事件并重绘。
- 行级 diff 渲染避免输入时整屏闪烁。
- composer 已按显示宽度处理中文等 CJK 输入。
- slash 提示列表是本地 UI 状态，不触发模型调用。
- 主屏 idle 时会轮询 lane artifacts，所以后台 `/lane run` 的完成、失败和
  log-tail 状态不需要按键也会刷新。
- 右侧栏 `ACTIVE TASKS` 面板会读取 `/task` 和 `/tasks` 背后的真实 workflow
  task store，并把这些 task record 与 pending approval、active lane 合并展示。
- live 副屏只读取持久化 lane 状态；如果没有 lane store，会显示空状态，而不
  回退到 preview/demo lane。
- `/lane inspect <id>` 会读取持久化 lane artifacts：`.log` 尾部、`.done`
  exit code、log path、done path、envelope path 和 envelope preview。
- template-launched Codex 和 Claude lane 会在 Git `HEAD` 可用时运行于
  `.robocode/worktrees/` 下的 per-lane 隔离 worktree。task envelope 会记录
  lane workspace 和 mutation scope。
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
- `/lane cleanup <id>` 会通过移除隔离 worktree 来归档 lane，但只有 worktree
  干净时才会执行。有未提交变更时必须显式使用
  `/lane cleanup <id> --force`，并且每次 cleanup 都会先写入
  `.robocode/lanes/<lane-id>.cleanup.md`。
- `/lane attach <id>` 会为 lane workspace 打开交互式终端，并记录
  `.robocode/lanes/<lane-id>.attach.md`。`/lane detach <id>` 只清除 attached UI
  状态，不会杀掉外部 terminal 进程。
- Provider health 已接入共享 runtime loop 测量到的模型请求 telemetry：真实
  request 数、成功/失败数、last/average latency、last event count 和最后一次
  provider error。
- TUI 会解析来自 core 真实事件的 LSP diagnostics，并持久化到
  `.robocode/diagnostics.txt`，所以主屏和副屏可以展示同一份有证据来源的
  diagnostics snapshot。
- `/screen side-1` 和 `/screen side-2` 现在会用当前 provider、model、theme
  和 workspace 启动真实副屏 TUI 进程。主屏最多跟踪两个副屏，`/screen list`
  显示状态，`/screen close <side-1|side-2>` 会停止跟踪，并在已知 pid 时发送
  终止请求。
- screen registry 会持久化到 `.robocode/screens.tsv`，所以主屏和副屏进程可以
  观察同一份 companion-screen 状态。
- `ROBOCODE_SCREEN_LAUNCH_TEMPLATE` 可以覆盖默认的当前二进制启动方式，用于
  桌面相关流程，例如打开新 terminal 窗口，或把副屏路由到另一块显示器。支持
  `{screen}`、`{provider}`、`{model}`、`{theme}` 以及 shell-quoted 的
  `{name:q}` 占位符。
- `ROBOCODE_LANE_CODEX_TEMPLATE` 和 `ROBOCODE_LANE_CLAUDE_TEMPLATE` 支持
  `{task}`、`{envelope}`、`{cwd}`、`{worktree}` 以及 shell-quoted 的
  `{name:q}` 形式。`{cwd}` 和 `{worktree}` 都会解析为真实 lane workspace。
- `ROBOCODE_LANE_ATTACH_TEMPLATE` 可以覆盖默认 lane attach launcher。它支持
  `{lane}`、`{task}`、`{tool}`、`{cwd}`、`{worktree}`、`{log}` 以及
  shell-quoted 的 `{name:q}` 形式。macOS 有默认 Terminal.app launcher；其他
  平台应提供该 template，例如 tmux 或桌面 terminal 命令。
- 修改 cockpit 行为、命令、架构、配置或 UI 时，必须同步更新相关文档。注释
  用来说明不明显的不变量和安全边界，不重复解释显而易见的代码。

## 近期缺口

- embedded PTY 仍是后续工作；当前 lane 支持非交互 shell 命令、
  template-launched Codex/Claude adapter、外部 terminal attach，并持久化
  envelope / log / exit-code artifacts；Unix 平台支持 process-group stop。
- apply 当前通过 `/lane apply <id>` 走保守 patch 路径。更完整的 merge /
  conflict review 仍是后续工作。discard lane 只记录决策，不会默认删除日志、
  worktree 或变更；清理必须通过单独的 `/lane cleanup` 命令执行。
- Provider token、cost、rate telemetry 尚未接入，所以 live UI 会继续隐藏这些
  指标。
- Diagnostics 仍需要显式 LSP 来源，例如 `/lsp diagnostics <path>` 或 LSP runtime
  的 post-edit diagnostics。live TUI 目前不会自动后台跑 checker。
- 副屏启动已经是真实进程管理，但跨物理显示器的 OS 窗口自动摆放仍由 launcher
  template 或桌面手动操作负责。
- 命令提示列表目前是固定命令注册表；命令参数和二级提示待后续扩展。
- 视觉还需要持续用截图和 holodeck 主参考图对比。
