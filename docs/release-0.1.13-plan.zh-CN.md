# RoboCode 0.1.13 计划

英文版： [release-0.1.13-plan.md](release-0.1.13-plan.md)

最后更新：2026-05-27

## 定位

`0.1.13` 是 **Operator Loop Hardening** 版本。

`0.1.12` 已经把第一个可监督 operator loop 做实：共享 runtime
`AgentTask`、确定性 shell/template lane、lane ContextBundle v0、release
smoke、截图证据、GitHub release 和 Homebrew 发布。下一版不应急着宣称完整
`0.2.0` runtime，而应先把这个闭环打磨到日常编程可依赖，并让 Codex/Claude
从“已映射的 surface”推进到可复现 delegated workflow。

版本切线：

> 开发者可以给 RoboCode 一个小任务，主 cockpit 清楚说明当前在做什么，用户可
> delegate 或 inspect 一个 lane，查看 evidence，并完成 accept、apply、retry、
> stop 或 discard，而不丢上下文。

## 决策

先发 `0.1.13`，再考虑 `0.2.0`。

原因：

- `0.1.12` 证明了形态，但真实 terminal 体验、review/apply recovery、外部
  agent happy path 仍是主要风险。
- 长期路线图要求 TUI 与 shared runtime 稳定后，再扩 API、IDE、web、desktop
  和广义 ACP/plugin。
- `0.2.0` 应代表 Agent Orchestration Runtime v1，而不是又一轮 foundation
  修补。

## 原则

- 不做假编排：面板必须读取真实 runtime facts、artifacts、logs、diffs、tests
  和 decisions。
- 先给 operator 信心：主屏必须持续回答“什么在跑、改了什么、证据在哪、下一步能做什么”。
- 一个 lane 模型：shell/template、Codex、Claude、tmux/PTY、DeepSeek 和未来
  ACP adapters 继续共享 `AgentTask`、evidence、permission 和 context-budget 边界。
- token efficiency 必须成为产品行为，而不只是 telemetry。
- 所有可见交互都需要真实 terminal 或确定性截图证据。

## 范围

### P0：默认进入 TUI 与首次设置

目标：

- 让 cockpit 成为默认产品入口。
- 常见路径下，用户可在设置里选择 provider 和 model，不必手改 config。
- 第一次使用时，引导用户完成跑通真实 turn 所需的最小配置。

工作：

- 当没有显式非交互命令时，`robocode` 默认进入 TUI。
- 保留显式逃生口：`--no-tui`、现有 preview flags、`--version`、`--help` 和未来
  scripting commands 必须继续非交互。
- 在 TUI 中增加 settings surface，用于选择 provider 和 model，并复用现有 layered
  config model。
- 设置流程展示 installed/built-in providers、API key 配置状态、默认 model 和
  provider health。
- 当没有可用 provider/model 时，显示 first-run setup guide：选择 provider、选择
  model、说明 API key 来源、运行 doctor/probe、保存选择。
- config 写入必须显式、可审计、可回滚；secret value 不进入 transcript 或截图。

验收：

- 无参数运行 `robocode` 会打开主 TUI。
- `robocode --help`、`robocode --version` 和 preview/smoke commands 仍保持非交互。
- 新用户可以选择 provider/model、看到 key 是否存在、运行 probe/doctor，并保存默认值。
- 现有 config 继续有效，CLI flags 仍在本次调用中覆盖保存的默认值。

实现状态：

- 2026-05-27 本地已完成：默认启动进入主 TUI，`--no-tui` 保留旧版行式
  REPL，`/settings` 和 `/setup` 展示设置状态，`/settings provider <id>
  [model]`、`/settings model <model>` 和 `/settings save` 可持久化
  provider/model 默认值，slash suggestions 已覆盖 settings 流程。

### P0：日常 Operator Loop 可靠性

目标：

- provider、tool、test 或 lane 运行时，主 TUI 必须稳定可信。
- 清掉 exit、command entry、approval、focus、modal 生命周期中的交互陷阱。
- 保持 `0.1.12` 视觉风格，同时修正对齐和颜色回归。

工作：

- 加固 `/quit`、`/exit`、Esc、Ctrl-C 在 main、side-1、side-2、命令面板和
  modal 状态下的行为。
- 增加 slash-command 输入回归测试，避免 `/quit` 和 `/exit` 被命令面板状态吞掉。
- approve/deny 后 approval modal 立即消失，并保持默认/选中动作足够明显。
- 为 approval 按钮、side-screen command target、命令面板选择补鼠标和键盘
  focus 测试。
- 修正 border/color 一致性，避免同一条边或同一个单词渲染成混色。
- 保留输入区高度、光标可见性、中文输入预览和 resize redraw 截图。

验收：

- 用户可从 idle、命令面板、modal 和安全 running 状态退出。
- approval modal 默认 approve，快捷键可用，决策后 modal 消失。
- idle、running、approval、command palette、CJK input、resize、side-1、
  side-2、lane review 均有截图证据。
- macOS Terminal 有人工截图或 notes；测试机器安装 iTerm2 时覆盖 iTerm2。

实现状态：

- 2026-05-28 已开始本地实现：命令面板回归测试已锁定 exact `/quit`
  和 `/exit`，确保 Enter 会提交退出而不是补全；partial `/q` 和 `/ex`
  仍会补全。Approval 键盘测试已覆盖默认 approve focus、移动到 diff/deny、
  Enter 激活，以及 y/n/Ctrl-C 直接快捷键。

### P0：Evidence-Backed Review / Apply / Retry

目标：

- 让确定性 shell/template lane review/apply 从 smoke-only 变成真实编码闭环。
- 每个 lane result 都带足够 evidence，支持安全决策。

工作：

- 把 changed files、diff summary、test result、exit code、artifacts、log tail
  汇总为一条 lane review record。
- 增加 conflict-aware apply preflight：dirty workspace、touched-file overlap、
  patch/apply failure reason 和 recovery next action。
- 如果当前 inspect 太密，新增 `/lane diff <id>` 和 `/lane artifacts <id>`。
- `/lane retry <id>` 保留上一轮 objective、context sources、changed-file
  evidence 和 failure reason。
- lane decision events 写入 transcript/workflow evidence，resume 后仍能解释发生了什么。

验收：

- 确定性 lane 能 run，产生 artifact/diff/test，进入 review，clean apply，并记录最终 decision。
- 失败或冲突 lane 能 retry 或 discard，原因可见，且没有静默 workspace mutation。
- side-1、side-2 和 `NOW WORKING` 对同一 lane 展示一致。

实现状态：

- 2026-05-28 已开始本地实现：`/lane diff <id>` 会写入并展示聚焦的
  `L*.diff.patch` artifact，`/lane artifacts <id>` 会列出持久化 lane 文件，
  两个命令都已接入 slash suggestions。Focused tests 已覆盖命令面板路由和
  artifact/diff 输出。

### P0：Main Provider Turn 的 ContextBundle v0.5

目标：

- 把 ContextBundle 从 lane-only metadata 推进到主 provider 路径，同时不破坏 provider 兼容。
- 让 token pressure 变得可行动。

工作：

- 从 user task、selected files、latest diff、diagnostics、recent test/lane
  summaries、memory/task summaries、recent transcript summary 构造 main-turn
  ContextBundle。
- 长 tool/lane/test output 使用 summary + tail compaction，同时保留原始 audit 数据。
- 增加 provider-side context pressure 行：sources、estimated tokens、largest
  contributors、compaction notes。
- 增加 soft/hard budget 行为：soft budget 警告，hard budget 裁剪低优先级 sources，
  并记录省略内容。
- 通过保守 helper 组装 provider prompt，让 OpenAI、Anthropic-style、DeepSeek、
  fallback 和 descriptor-backed providers 复用同一 bundle。

验收：

- 至少一个真实 provider turn 使用 ContextBundle 生成的 prompt input。
- 测试证明 provider prompt 被压缩时，原始 transcript/tool output 仍保留。
- main status 和 side-2 一致展示 context pressure。

实现状态：

- 2026-05-28 已开始本地实现：主 provider turn 会构造保守的 ContextBundle，
  并作为临时 system context message 追加到 `ModelRequest`，但不会把这段生成
  context 写入 raw transcript。Runtime task evidence 和 `/status` 已展示
  context pressure、source count、largest sources 和 compaction notes。
  Focused tests 已覆盖 provider request 注入和 runtime evidence。

### P1：Codex / Claude 可复现 Happy Path

目标：

- 让 Codex 和 Claude adapter 成为可复现、可产证据的 lanes。
- 有 protocol/app-server events 时优先使用；template/tmux fallback 仍保持实用。

工作：

- 增强 `/agent doctor codex`、`/agent doctor claude` 和 template readiness diagnostics。
- Codex 增加可复现 read-only review smoke；Claude 在本机安装时增加 template/tmux smoke。
- status、tail、result、touched files、final output、suggested next action 映射到共享
  `AgentTask` 和 lane review records。
- write-capable external-agent work 继续走显式 permission、isolated worktree 和
  apply/review 边界。
- docs 与 doctor output 清楚说明 unsupported 或 credential-gated 情况。

验收：

- 配好 Codex 的机器上，RoboCode 能启动 read-only Codex review，并在 TUI 中展示 result/evidence。
- 配好 Claude Code 的机器上，RoboCode 能运行 template/tmux lane，并在 TUI 展示 tail/result/evidence。
- 缺工具时给出可行动 doctor output，而不是空面板。

### P1：真实 Terminal 验收 Harness

目标：

- 发布前更难漏掉 terminal UX 缺陷。
- 保留确定性截图，同时为截图无法证明的场景增加真实 terminal evidence。

工作：

- 增加 macOS Terminal 和 iTerm2 manual acceptance checklist。
- 增加 helper scripts，以固定尺寸启动 main、side-1、side-2 并收集截图。
- 标记每项交互是 deterministic、manual verified，还是当前机器不可测。
- 把真实使用截图或 notes 放到 `docs/previews/manual/0.1.13/`。

验收：

- release status 区分 deterministic SVG evidence 和 real-terminal evidence。
- 如果 iTerm2 未安装，release status 明确记录此缺口，同时 Terminal evidence 仍可执行。

### P2：Extension / ACP / MCP 边界说明

目标：

- 让未来平台能力继续对齐，但不吞掉整个版本。

工作：

- 更新 adapter/extension 文档，说明 plugin、skill、MCP、ACP 都必须经过同一套
  permission/evidence/context 边界。
- 成本低时补 descriptor/doctor/probe 测试。
- `0.1.13` 不实现 mutating generalized runtime。

验收：

- 文档清楚区分真实能力、实验能力和 deferred 能力。
- 没有 extension path 绕过 permissions、transcript、evidence 或 token-budget 假设。

## 非目标

- 不宣称完整 Agent Orchestration Runtime v1。
- 不做广义 marketplace/plugin install flows。
- 不扩 desktop/web/IDE/API surfaces。
- 不做完整 Zed 级 ACP host。
- Codex/Claude write-capable happy path 不是 blocker，除非 permission 与 apply 边界完全可审计。

## 实施顺序

1. 默认 TUI 与首次设置：默认启动行为、`--no-tui`、provider/model settings、
   first-run guide、config persistence、doctor/probe。
2. 交互加固：`/quit`、`/exit`、命令面板、approval modal 生命周期、focus、border/color 一致性。
3. Review/apply 加固：lane decision record、dirty/conflict preflight、
   retry/discard evidence、side-screen 一致性。
4. Main-turn ContextBundle：builder、compaction、prompt integration、pressure UI、测试。
5. Codex/Claude happy path：doctor/probe、read-only review、template/tmux result mapping、截图。
6. 真实 terminal harness：scripts/checklist、Terminal/iTerm2 evidence、0.1.13 截图刷新。
7. 文档与发布：README、user guide、modules、roadmap、release status、GitHub release、
   Homebrew tap、post-publish smoke。

## 测试计划

Focused tests：

- 默认启动进入 TUI，同时 `--help`、`--version`、previews 和 `--no-tui` 保持非交互；
- provider/model settings 能列出选项、遵守 CLI override 优先级，并持久化显式保存的选择；
- first-run setup 能处理缺 key、probe 失败和成功选择 provider/model，且不泄露 secrets；
- slash-command 输入与退出行为；
- 命令面板 filtering/selection，同时 typed commands 不回归；
- approval modal 默认/快捷键/鼠标 decision 与清理；
- theme border/color rendering snapshots；
- lane review/apply/retry/discard 状态流；
- dirty workspace 和 apply-conflict preflight；
- ContextBundle provider prompt compaction 与 raw audit preservation；
- Codex/Claude adapter event-to-`AgentTask` fixture mapping。

Regression 和 smoke：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/release-smoke.sh --version 0.1.13 --deepseek --out-dir /tmp/robocode-0113-release-smoke-full
```

人工验收：

- macOS Terminal：idle、resize、CJK input、command palette、approval modal、
  provider thinking、shell/test running、lane review/apply。
- iTerm2：安装时跑同一 checklist。
- 真实 Codex/Claude 只在本机认证与工具可用时验证；否则用 doctor output 和 fixture tests
  证明失败路径。

发布验证：

```bash
scripts/release-smoke.sh --version 0.1.13 --quick --github-release-assets --homebrew --out-dir /tmp/robocode-0113-postpublish-check
```

## 发布标准

- workspace version 为 `0.1.13`。
- 所有 P0 都通过测试，并具备截图或人工 evidence。
- `robocode` 默认进入 TUI，first-run provider/model setup 已文档化并验证。
- README 和 user guide 说明当前真实能力与实验 adapter 边界。
- release status 记录验证、assets、Homebrew tap 和剩余风险。
- GitHub release 与 Homebrew tap 发布完成，post-publish smoke 通过。

## 后续

`0.1.13` 之后：

- 如果 operator loop 在真实日常使用中稳定，进入 `0.2.0`：Agent Orchestration
  Runtime v1、默认 planner -> worker -> reviewer -> tester workflow、更完整 token
  efficiency engine、更强 Codex/Claude/ACP adapter contracts。
- 如果交互或 apply/retry 可靠性仍弱，先发 `0.1.14` 做第二轮 hardening，再宣称 runtime v1。
