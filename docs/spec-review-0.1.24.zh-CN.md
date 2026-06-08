# RoboCode 0.1.24 Spec Review

英文版： [spec-review-0.1.24.md](spec-review-0.1.24.md)

最后更新：2026-06-07

## 目的

本文件按 spec-first 原则审查当前代码、产品文档和发布计划之间的差异。Spec 是目标行为；
代码必须实现它，或者文档必须明确标为计划中能力，不能把未来能力写成当前已完成能力。

本轮重点覆盖：

- TUI provider turn、Plan 模式输入、approval、streaming 和 scrollback；
- `/connect`、`/provider`、`/models`、`/setup` 的交互语义；
- ContextBundle、provider error recovery、真实 provider smoke；
- AgentTask、side panels、lane/delegate evidence 和核心运行时分层；
- release gate、截图证据和测试策略。

## 已经稳住的能力

- Provider、tool、permission、transcript 已经走共享 runtime path；Plan 模式在 core
  层可以拦截 mutating tool call 和 shell-backed `/test`。
- `ContextBundle` 已经接入 provider request，并有 request compaction 测试覆盖大历史
  裁剪。
- TUI 已经有 `/connect` 和 `/models` 的本地 interaction panel，不再只依赖 command
  palette 补全。
- provider turn 现在通过 `TuiRuntime` worker dispatch，并立即把控制权还给 TUI 主循环。
  streaming delta、approval request、cancel、resize、scroll、finish/error event 都由同一个
  主事件循环消费，不再走旧的 `run_provider_turn_interactive` 内循环。
- `/quit`、`/exit`、resize 重绘、中文显示宽度、provider telemetry、recent files、
  LSP snapshot、lane artifacts 和 Codex job evidence 都已经有实现基础。
- release gate、daily-loop smoke、plan-mode smoke 和 DeepSeek development scenario
  smoke 已经形成脚本入口。

## P0 差异

| 优先级 | 差异 | 代码位置 | 影响 | Spec 目标 |
| --- | --- | --- | --- | --- |
| P0 | active-turn queue 仍需要 durable core/runtime ownership | `robocode-cli/src/tui/state.rs` `PendingTurn`、`robocode-cli/src/tui/app.rs` `queue_active_turn_input` | TUI 主循环现在能在 provider turn 运行时继续接收输入，queued prompts 在成功/失败后都能保留，queued count 也已经通过共享 `AgentTask` projection 可见；durable core/no-TUI queue ownership 仍是后续工作 | queue 继续在 `AgentTask` 中可见，支持 cancel、retry、restore all drafts 和 side-panel evidence；如果后续 non-TUI surface 需要，再把 durable queue ownership 下沉到 core |
| P0 | 非阻塞 provider-turn smoke 还需要更强终端覆盖 | `robocode-cli/src/tui/app.rs`、`scripts/plan-mode-smoke.sh` | unit test 已证明 fake slow provider 启动不会阻塞 UI thread，但仍需要 Terminal/iTerm2 人工证据和更完整的输入/resize/models smoke | fake slow provider 运行时，用户可以输入、编辑、滚动、resize、打开 `/models`、cancel，并看到 queued count |
| P0 | streaming delta 不再强制回到底部，查看历史时 transcript badge 会标记 new output | `robocode-cli/src/tui/app.rs`、`robocode-cli/src/tui/render.rs` | 用户查看历史时不再被新 token 拉到底部，并能看到 `history N · new output`；更强的 jump-to-latest affordance 留到 zero-bug pass 继续打磨 | 只有用户在 bottom/follow 模式时 auto-follow；离开底部后显示 new output marker |
| P0 | 413 / argument-too-long recovery 已部分自动化，但还需要完整 launch-path 审计 | `robocode-core/src/runtime_loop.rs`、`robocode-tools/src/shell.rs`、`robocode-core/src/agent_commands.rs`、`robocode-cli/src/tui/screen.rs` | provider 413 现在会用压缩上下文 retry 一次，已知 shell/ACP/screen 长 payload 路径不再塞巨大 argv；未来新增 launch surface 还要在 zero-bug gate 前继续验证 | provider error classifier + request shrink/retry；所有 shell、lane、ACP、screen launch path 都使用 tempfile/stdin 并保留 audit evidence |

## P1 差异

| 优先级 | 差异 | 代码位置 | 影响 | Spec 目标 |
| --- | --- | --- | --- | --- |
| P1 | provider/model 交互仍分裂为 TUI panel 和 core command text fallback | `robocode-cli/src/tui/app.rs`、`robocode-core/src/provider_commands.rs` | 用户在某些路径仍会看到命令说明而不是可操作表单 | TUI 使用直接表单和选择器；core command 输出只作为 no-TUI fallback |
| P1 | 全局 `/models` 已收窄为 active/favorite models，但 recent-model 持久化仍较薄 | `robocode-cli/src/tui/app.rs` model picker 构造 | 全局 picker 不再拉入 descriptor known models；provider-scoped setup 仍能展示 known candidates。recent persistence 和 favorite 管理还需要继续补强 | global `/models` 只读 active/favorite/recent；provider-scoped setup 才展示 known candidates |
| P1 | provider doctor/probe 仍可同步走 `run_settings_command` | `robocode-cli/src/tui/app.rs` settings command path | 当 doctor 变成真实网络探测时，面板可能冻结 | doctor/probe 是 background job，返回 tail、status、evidence 和 cancel |
| P1 | Product view model 边界尚未正式化 | `robocode-cli/src/tui/state.rs` `agent_tasks` projection | TUI 继续混合 runtime snapshot、transcript-derived tasks、workspace scans 和本地 pending state | core 暴露 `RuntimeViewSnapshot`，TUI 只渲染产品 view model |
| P1 | provider capability 差异还没有完整适配层 | `robocode-model/src/providers.rs`、`robocode-model/src/adapters.rs` | DeepSeek、DashScope、OpenRouter、Anthropic/OpenAI-compatible 的工具调用、reasoning、streaming、错误恢复差异会泄漏到 UI | 每个 provider descriptor 明确 auth、endpoint、models、tool semantics、stream fields、error mapping 和 retry policy |

## P2 差异

- `docs/tui-cockpit-design*.md` 现在应描述真实边界：provider turn 在 `TuiRuntime`
  worker 中执行并把事件送回主循环；runtime-visible queue state 和完整 smoke evidence
  仍未完成。
- 主 TUI 设计文档已改成使用 `RoboCode` 或内部角色，例如 `RoboCode is planning`、
  `Operator is reviewing context`、`Tool runner is waiting for approval`。旧 release
  status、历史截图或审计文档中仍可能出现 `DeepSeek is thinking`，保留为历史记录即可，
  不应继续作为新 UI 文案。
- 文档整体已有中英文结构，但旧 `docs/code-agent-benchmark.md` 仍是英文单文件。后续若继续
  作为用户可见材料，需要补 `zh-CN` 对照或移入内部 research。

## Spec 修正原则

1. 文档中的 “已实现”、“当前实现”、“可用” 必须能在代码、测试、截图或 smoke evidence
   中找到对应证据。
2. 目标行为写入 roadmap 或 release plan 时，必须配套 acceptance gate。
3. TUI 不能因为 provider、approval、doctor、context build、lane 或 tool execution
   进入嵌套 input loop。
4. provider/model 设置必须是直接交互：列表选择、表单编辑、Enter 生效、Esc 取消；
   命令文本只能作为 no-TUI fallback。
5. 用户可见功能完成时必须给真实终端截图或 deterministic preview。

## 优先级开发计划

### P0-A TurnController 与唯一主事件循环

- 新增 `TurnController` 或等价结构，负责 active turn、queued turns、cancel、
  streaming delta、approval request、tool/lane jobs 和 final result。
- TUI 主循环只做一件事：接收 terminal input、worker events、timers 和 resize，然后更新
  state/render。
- 删除 active-turn 专属键盘循环，`handle_submitted_input` 不再等待 provider turn 完成后才返回。
- 当前进展：`TuiRuntime` 现在把 `SessionEngine` 放在 worker thread 中。`handle_submitted_input`
  启动 provider turn 后立即返回；主循环消费 stream、approval、cancel、finish/error event。
  旧 `run_provider_turn_interactive` / `poll_active_turn_input` 路径已经删除。fake slow
  provider unit test 已证明 turn dispatch 不会等待 provider 完成。

验收：

- fake slow provider 运行 30 秒时，用户可以输入、编辑、滚动、resize、打开 `/models`，
  且 queued count 可见。
- `/plan on` 下提交长规划任务后，下一条输入不会被锁死或丢失。

### P0-B 非阻塞 approval

- 把 approval prompt 转成 `InteractionPanel::Approval` 或独立 modal state。
- 移除 approval path 中直接 `event::read()` 的阻塞循环。
- 键盘、鼠标、resize、scroll 都由主循环统一分发。
- 当前进展：阻塞式 approval reader 和 active-turn event pump 都已经移除。approval 现在使用
  `ActiveApproval` callback object，由 TUI 主循环处理。后续可以清理成一等
  `InteractionPanel::Approval`，但它已经不再需要嵌套 input loop。

验收：

- approval 出现时，鼠标点击、`y/n/Enter/Esc`、resize、scroll 都可用。
- approval 完成后 modal 立即消失，pending task 同步更新为 accepted/denied。

### P0-C Core-visible Queue、错误恢复和 Scrollback

- queued follow-up 进入 runtime snapshot，而不是仅存在 `PendingTurn.queued_inputs`。
- 失败后恢复所有 queued drafts，并在 transcript 写明哪些已保留、哪些等待重试。
- streaming 时加入 follow-mode 状态；用户滚动离底后不再 auto-follow。
- 当前进展：streaming delta 不再把 `transcript_scroll` 重置到底部；active turn 失败时
  会恢复第一条 queued draft，并列出剩余已保留 drafts；成功后如果自动启动下一条 queued
  turn，剩余 queued prompts 会继续挂在新 pending turn 后面；queued count 已进入
  `AgentTask.summary`、evidence 和 next action。durable core/no-TUI queue ownership
  仍是后续工作。

验收：

- 连续输入 3 条 follow-up，provider 失败后 3 条都可恢复或继续明确排队。
- streaming 中向上滚动后 viewport 不跳到底部；transcript badge 显示
  `history N · new output` marker。

### P0-D Context Failure Recovery

- 增加 provider error classifier：413、429、401、404 model missing、timeout、network、
  unsupported tools、invalid tool result sequence。
- 413 现在会自动缩小 provider request view 后 retry 一次，并把 compaction note 写入
  transcript/events。
- builtin shell tool 对长命令使用 stdin；ACP shell startup 和 TUI side-screen shell template
  会把过大的 launch command 写入临时脚本，必要时保留协议 stdin 给 JSON-RPC 使用。
- 剩余未来 launch surface，包括新的 lane adapters，都必须遵守同一个 no-large-argv
  invariant。

验收：

- deterministic 413 provider fixture 能触发 shrink/retry。
- builtin shell tool、ACP startup path 和 TUI side-screen template path 的 long shell
  payload 不再触发 OS `Argument list too long`。

### P1-A Provider Setup Forms

- `/connect` 一级只展示 provider 名；Enter 后进入 provider-specific setup。
- API key provider 进入 key input；web-login provider 展示登录 URL/action；local provider 展示
  local health/action。
- 保存后进入 provider-scoped model picker；global `/models` 只显示已配置 provider 和
  active/favorite/recent models。
- 当前进展：global `/models` 现在只使用 active/favorite model rows；provider-scoped
  setup 仍展示 descriptor default 和 known model candidates，方便配置好的 provider
  激活更多模型，但不污染全局 picker。

验收：

- DeepSeek key 可更新、删除、重新设置；key 只脱敏显示开头和结尾。
- 未配置 provider 不出现在 global `/models`，但在 `/connect` provider-scoped setup
  里可选择 known model candidates。

### P1-B Async Doctor、RuntimeViewSnapshot 和 Provider Adapter Matrix

- doctor/probe 统一作为 background job。
- core 输出 `RuntimeViewSnapshot`，TUI right rail、side-1、side-2、NOW WORKING 都读同一份。
- provider descriptors 增加 capability matrix：auth、models、stream fields、tool behavior、
  context limit、error recovery。

验收：

- provider doctor 运行期间 UI 不冻结。
- side panels 和主屏状态在同一 turn 中展示同一个 task id/status/evidence。

## 测试与发布门禁

- `cargo fmt --check`
- TDD testing contract smoke: `scripts/tdd-testing-contract-smoke.sh`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `scripts/tui-turn-controller-smoke.sh`
- `scripts/tui-regression.sh docs/previews/generated`
- `scripts/plan-mode-smoke.sh /tmp/robocode-0124-plan-mode-smoke`
- `scripts/daily-loop-smoke.sh /tmp/robocode-0124-daily-loop-smoke`
- fake slow provider non-blocking TUI smoke
- deterministic approval non-blocking smoke
- deterministic 413 shrink/retry smoke
- `scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash`
- `scripts/release-gate.sh --version 0.1.24`
- `scripts/release-gate.sh --version 0.1.24 --phase postpublish`

## 文档落地动作

- `docs/release-0.1.24-plan*.md` 必须引用本 spec review 作为 release gate。
- `docs/testing-validation-plan*.md` 必须增加 spec drift gate，防止文档再次领先实现。
- `docs/tui-cockpit-design*.md` 已把当前 implementation notes 改成真实边界描述；后续
  review 需防止再次把目标写成已落地。
- `docs/modules*.md` 必须把本文件列入 roadmap/reference 文档。
