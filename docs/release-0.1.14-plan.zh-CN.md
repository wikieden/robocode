# RoboCode 0.1.14 计划

English version: [release-0.1.14-plan.md](release-0.1.14-plan.md)

最后更新：2026-05-28

## 定位

`0.1.14` 是 **Delegated Agent Trust Loop** 版本。

`0.1.13` 已把 cockpit 变成默认入口，加入第一次使用的 provider/model 设置，加固
operator loop，把 ContextBundle 注入主 provider turn，并完成 GitHub/Homebrew 发布闭环。
下一版要把多 agent 核心承诺继续做实：

> RoboCode 可以把一个有边界的编程子任务委派给 Codex、Claude 或 shell/template
> lane，显示它正在做什么，捕获它看到了什么、改了什么，保留证据，并把操作者带回
> 明确的 review/apply/discard/retry/stop 决策。

这仍然是 `0.1.x` 版本。目标是可靠的 TUI 主导编排闭环，不提前宣称完整 `0.2.0`
runtime。

这个版本要回答的产品问题不是“RoboCode 能启动多少 Agent”，而是：

> 操作者为什么应该相信这个 delegated result？

## 0.1.13 总结

已落地：

- `robocode` 默认打开 TUI；`--no-tui` 保留脚本化 REPL 行为。
- `/settings` 和 `/setup` 支持 provider/model 设置和默认值保存。
- approval、command palette、中文输入、resize、modal 可靠性有测试和确定性截图覆盖。
- `/lane diff <id>` 和 `/lane artifacts <id>` 让 lane review evidence 更容易查看。
- 主 provider turn 接收临时 ContextBundle，同时保留 raw transcript audit 数据。
- `v0.1.13` 的 GitHub Release assets、Homebrew tap、post-publish smoke 和
  DeepSeek live smoke 已完成。

仍然存在的缺口：

- Codex 和 Claude 已映射到 lane 概念，但可重复的真实 happy path 还需要发布级加固。
- 外部 agent 的 status、tail、result、touched files、artifacts 和 next action 需要
  统一进 `AgentTask` / lane evidence 模型。
- review/apply 冲突恢复需要更清晰的 preflight、失败原因和 retry lineage。
- ContextBundle 需要明确 token-budget policy、omitted-source records 和 compaction
  reason codes。
- Plugin、skill、MCP、ACP 需要先定义 descriptor/probe 边界，再扩展 mutating runtime。

## 原则

- 一个 operator loop：provider、shell、Codex、Claude、DeepSeek、tmux 和未来 ACP
  agents 共享 task、evidence、permission、context 和 decision 模型。
- 先真实再扩面：一个可靠 Codex path 和一个可靠 Claude path，比很多装饰性
  adapters 更重要。
- 证据优先：每个 delegated lane 都展示 command、status、tail、artifacts、changed
  files、diff、test result、decision 和 next action。
- token efficiency 是产品行为：budget pressure、source selection 和 compaction
  decisions 必须可见、可测试。
- 截图是发布证据：每个用户可见 TUI state 都要有证据。

## 发布切线

`0.1.14` 完成标准：

1. 已配置机器上，Codex read-only review 可以作为 delegated lane 运行；不可用时
   给出可执行 doctor 输出。
2. Claude Code 可以通过文档化 template/tmux lane 路径运行；不可用时给出可执行
   doctor 输出。
3. Shell/template、Codex、Claude 结果映射进同一套 lane review 和 `AgentTask`
   evidence 模型。
4. 操作者可以带着可见证据 inspect、accept、apply、discard、retry 或 stop
   delegated work，不发生静默 mutation。
5. ContextBundle v1 记录 source priority、budget pressure、omitted sources 和
   compaction decisions。
6. Plugin/skill/MCP/ACP 保持只读 descriptor/probe surface，除非明确进入 shared
   permission 和 evidence 路径。

## P0 切线

P0 要刻意收窄。这个版本只有当下面三条真实流程可演示时才算完成：

1. **Shell/template lane trust loop**：启动一个有边界的命令，观察 live
   tail/status，inspect timeline evidence，并且可以 stop 或 retry，同时不丢失旧证据。
2. **Codex read-only review trust loop**：把 review 任务委派给 Codex，收集 review
   evidence 和 next action，全程不修改 workspace。
3. **Claude template/tmux trust loop**：通过文档化 template/tmux 路径启动 Claude，
   观察 tail/status，inspect final output，并通过同一套 lane model stop 或 retry。

每条 P0 flow 都必须具备：

- 共享 `AgentTask` / lane evidence
- 可见的 `NOW WORKING`、side-1、side-2 状态
- 能解释结果的 timeline 或 inspect 输出
- 明确 next action
- deterministic screenshot evidence 或真实终端 notes

## 明确不做

`0.1.14` 不投入这些范围：

- 完整 ACP runtime
- plugin marketplace 或 install UX
- 默认 write-capable Codex 或 Claude lanes
- 自动多 Agent 任务拆分
- cloud、web、team、desktop surfaces
- 大范围 mutating MCP/plugin/skill execution
- 不能改善三条 P0 trust loop 的其他 agent integrations

## 范围

## 实施顺序

`0.1.14` 按这个顺序推进：

1. **Trust-loop foundation**：先定义或扩展共享 lane timeline、isolation
   declaration、capability 和 evidence records，再改 UI 行为。
2. **Shell/template baseline**：先用确定性的本地 lane 跑通 timeline、inspect、
   stop、retry 和 evidence model，避免一开始被外部 Agent 不确定性干扰。
3. **Adapter doctor**：先展示 shell/template、Codex、Claude、tmux、PTY 和未来 ACP
   的 capability readiness，再启动更复杂的 delegated work。
4. **Codex read-only review**：实现第一条不修改 workspace 的 external-agent trust loop。
5. **Claude template/tmux lane**：实现第一条 terminal-template external-agent trust loop。
6. **Review/apply/retry safety**：等 lane evidence model 稳定后，再加固 conflict
   preflight 和 retry lineage。
7. **TUI evidence screens**：把 `NOW WORKING`、side-1、side-2、command palette 和
   screenshots 都接到同一个 shared snapshot。
8. **Docs and release evidence**：P0 flows 真实可用后，再更新 user docs、screenshots、
   release status 和 smoke scripts。

三条 P0 trust loop 可演示之前，不启动 P1 实现。

### P0: Adapter Doctor 和 Capability Registry

- 为 `shell/template`、`codex`、`claude`、`deepseek`、`tmux`、`pty` 和未来 `acp`
  增加共享 capability records。
- 如果现有 command surface 不够清晰，增加 `/agent list`、`/agent doctor`、
  `/agent doctor <id>` 或等价命令。
- 报告 binary presence、version、auth/setup hint、input mode、mutation mode、
  evidence mode 和 known limits。
- 在 side-1、side-2 和 command palette suggestions 中展示 readiness。
- doctor/probe 保持只读。

验收：

- 缺少 Codex 或 Claude 时输出可执行 setup 说明。
- 已配置工具展示 ready status 和支持的 lane modes。
- 测试覆盖 descriptor parsing、missing binary、configured template 和 command
  rendering。

### P0: Codex Read-Only Review Lane

- 实现 Codex review lane，输入 task、cwd/worktree、ContextBundle 和 allowed scope。
- 可用时优先 app-server/protocol evidence；保留 terminal fallback。
- 捕获 status、tail、final output、touched files、command executions、artifacts 和
  suggested next action 到 lane evidence。
- P0 不扩大 write-capable Codex work，除非隔离且显式 gated。
- 增加 deterministic fixture coverage 和 live/manual instructions。

验收：

- 已配置 Codex lane 可以运行 read-only review 并返回 evidence。
- 主屏、side-1、side-2、`/lane inspect` 和 `/status` 一致。
- result 可以 accept、discard、retry 或 archive，不修改 workspace。

### P0: Claude Template/Tmux Lane

- 加固 `ROBOCODE_LANE_CLAUDE_TEMPLATE` 和 tmux launch docs。
- 检查 `claude`、template variables、cwd/worktree 和 log capture readiness。
- 把 Claude status、tail、final output、touched files、artifacts 和 next action
  标准化进入 lane review。
- attach/send/stop/retry 时保留 evidence。

验收：

- 已配置 Claude template lane 可以 launch、observe、inspect、stop、retry。
- 缺少 template 或 binary 时输出准确 setup steps。
- TUI evidence 与 `/lane inspect`、`/lane artifacts`、`/lane diff` 一致。

### P0: Review / Apply / Retry Safety

- 增加 dirty workspace、touched-file overlap、patch applicability、deleted/moved
  files、untracked output preflight。
- apply failure 输出结构化 reason 和 next action。
- 增加 retry lineage：original objective、previous failure、omitted context、
  changed files 和 operator decision。
- accept/apply/discard/retry/stop events 写入 transcript 和 lane evidence。

验收：

- clean apply 成功并记录 diff/test evidence。
- conflict 阻止 mutation 并解释 blocking files。
- retry 创建 linked task/lane，不覆盖已有 evidence。

### P0: TUI Evidence Screens

- 为 Codex-ready、Codex-reviewing、Claude-template-ready、lane-conflict、
  lane-retry、adapter-doctor states 增加确定性截图。
- modal 和 non-modal 状态保持 no-modal 视觉风格。
- 继续保护输入区高度、光标可见、中文输入、resize redraw、边框对齐。
- 尽可能增加 macOS Terminal 和 iTerm2 真实终端 notes 或 screenshots。

验收：

- 每个新 visible state 都有 deterministic SVG 或 real-terminal evidence。
- Side panels 和 `NOW WORKING` 读取同一 shared task snapshot。
- regression output 不出现混色边框或 right-rail 漂移。

### P0: Lane Event Timeline 和 Isolation Preflight

目标：

- delegated work 运行时可解释，完成后可 review。
- 避免 parallel lanes 破坏共享 test data、cache、service 或 workspace。

工作：

- 增加 per-lane event timeline，记录 prompt/envelope creation、adapter launch、
  tool/command events、file changes、approvals、tests、failures、retries、final
  output 和 operator decisions。
- 增加 `/lane timeline <id>`，或在命令面更简单时并入 `/lane inspect <id>`。
- 扩展 lane envelope 的 isolation declarations：worktree、env vars、cache dirs、
  database/schema scope、service ports、setup command、verification command 和
  cleanup command。
- 多个 mutating lanes 启动前展示 isolation preflight warnings。

验收：

- 操作者能在不只相信 final summary 的情况下复盘 lane 为什么得到某个结果。
- 缺少 cleanup 或存在 shared test-data risk 的 lane 会在 launch 前提示。
- Timeline rows 和 isolation warnings 显示在 side-1/side-2，并存为 evidence。

### P1: ContextBundle v1 Policy

- 增加 source priority、soft budget、hard budget、omitted-source records 和
  compaction reason codes。
- 长 lane/test/tool output 在进入 provider 或 external-agent input 前做 summary +
  tail compaction。
- raw logs 和 transcript entries 保持可 audit。
- `/status`、side-2、lane inspect 中展示 budget pressure 和 largest sources。

验收：

- 测试证明 compacted model input 不删除 raw audit data。
- 高 context pressure 展示 omitted 内容和原因。
- Codex、Claude、shell lanes 接收可见 ContextBundle envelopes。

### P1: Cost / Rate / Runtime Budget Ledger

目标：

- 在 automation 扩大前，让 provider 和 lane economics 可见。
- 防止 long-running loops 静默烧配额。

工作：

- 增加 per-lane ceilings：max turns、max estimated tokens、max cost、max
  wall-clock time、max retries。
- 在 side-2、`/status` 和 lane inspect 中展示 budget burn rate 和 remaining budget。
- 把 provider rate-limit signals 和 budget stops 记录为 evidence rows。

验收：

- delegated lane 可以因 budget ceiling 被停止，并给出结构化原因。
- 操作者能看到哪个 lane 或 provider 消耗最多 budget。

### P1: Lightweight Steering、Hooks 和 Credential Boundaries

目标：

- 吸收 HN/Kiro/Claude 最强信号，但不提前做完整 plugin marketplace。
- 让 automation 可观察且安全。

工作：

- 定义 project steering files：conventions、architecture、workflows、protected
  paths。
- 增加轻量 spec envelope：requirements、design notes、tasks、tests、acceptance
  criteria。
- 设计只读 hook probes：pre-tool、post-tool、notification、stop events；hook
  outputs 进入 evidence rows。
- 为未来 MCP/plugin/agent calls 定义 secret handles，让 model context 可以请求
  capability，但看不到 raw credential values。

验收：

- delegated task envelope 可以引用 steering/spec files。
- Hook probe output 可见、可测试，且不修改外部系统。
- 文档解释 capability use 和 secret exposure 的区别。

### P1: Extension Boundary Docs And Probes

- 定义 provider plugins、agent adapters、skills、MCP servers、ACP agents 的
  descriptor fields。
- 为每一类增加只读 `doctor` / `probe` / `capabilities` 输出。
- 文档说明未来 mutating extension calls 必须经过 shared permission、transcript、
  evidence 和 token-budget boundaries。

## 验证计划

Focused tests:

- adapter descriptor 和 doctor output
- Codex fixture event 映射到 `AgentTask` 和 lane evidence
- Claude template readiness 和 lane envelope generation
- dirty workspace 和 conflict preflight
- retry lineage 和 decision persistence
- ContextBundle v1 source priority、omission、compaction 和 raw-log preservation
- lane timeline 和 isolation preflight
- budget ceiling 和 budget-stop evidence
- steering/spec envelope references
- hook probe output 和 credential-handle rendering
- agent doctor 与 delegated lane commands 的 command palette entries

Smoke/regression:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `scripts/tui-regression.sh docs/previews/generated`
- `scripts/smoke-lane-operator-loop.sh`
- Codex read-only fixture smoke
- Claude template/tmux smoke when available
- `scripts/release-smoke.sh --version 0.1.14 --out-dir /tmp/robocode-0114-release-smoke-full-local`

Manual acceptance:

- macOS Terminal 和 iTerm2 TUI launch
- side-1/side-2 delegated lane observation
- Codex configured read-only review（如果可用）
- Claude configured template/tmux lane（如果可用）
- resize、中文输入、approval modal、mouse/keyboard focus
- 每个新 user-visible state 的截图

Publish validation:

- tag `v0.1.14`
- GitHub Release workflow 上传 macOS arm64、macOS x86_64、Linux x86_64、Windows
  x86_64 archives 和 sha256
- 更新 `wikieden/homebrew-tap`
- post-publish smoke 验证 GitHub assets 和 Homebrew install path

## 不做

- 完整 `0.2.0` orchestration runtime 宣称。
- 没有 isolated review/apply 的 write-capable Codex/Claude autonomous mutation。
- marketplace-style plugin 或 skill installation。
- descriptor/probe/capability mapping 之外的 mutating MCP/ACP runtime。
- 绕过 TUI-led runtime 的 IDE/web/desktop/API surfaces。
