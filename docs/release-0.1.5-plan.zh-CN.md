# RoboCode 0.1.5 计划

最后更新：2026-05-25

## 主题

`0.1.5` 是编程体验版本。`0.1.4` 已经证明 TUI cockpit、DeepSeek V4 Flash
provider 路径、shell lane、tmux lane 和 release artifacts 可以端到端跑通。
下一版要把这些能力变成日常写代码时顺手的体验。

北极星目标：

> 在真实 Rust、JavaScript 或 Python 项目里，用户可以留在 RoboCode TUI 内完成：
> 理解需求、修改代码、审查 diff、运行测试、修复失败、总结结果。

聚焦对标：

- `docs/code-agent-experience-benchmark-2026-05-25.zh-CN.md` 对比 Codex、
  Claude Code、DeepSeek-TUI / CodeWhale 和 Zed，是本版本计划的对标来源。

## 产品原则

- 优先优化写代码闭环，而不是扩大功能表面积。
- 每次 mutation 在 approval 前都要解释清楚。
- 原始证据保持可审计，界面渲染保持简洁可操作。
- 一个可靠工作流优先于多个半连接的快捷方式。
- 在产品决策改变前，`/lane` 继续保持 TUI-first。
- 截图、TUI snapshot 和 smoke transcript 都是 release evidence。

## 非目标

- 除非直接阻塞编程闭环，否则不启动 MCP、remote bridge、automation 等 V3 平台扩展。
- `0.1.5` 不做完整 terminal emulator；先增强持久化 log replay 和 lane 控制。
- 除非现有 crate 或本地 helper 无法安全解决问题，否则不为了 UI polish 增加新依赖。
- 副屏不能只是装饰；必须显示对编程有用的证据。

## P0：TUI 交互稳定性

这些是 release blockers，因为它们影响每一次编程会话。

- Composer：
  - 输入区更高、更容易看见；
  - 光标保持可见并闪烁；
  - 中文输入法候选窗靠近当前输入行；
  - multiline input 不挤压 bottom bar。
- Resize 和 redraw：
  - 终端 resize 后不留下旧边框或错位右侧 panel；
  - transcript、right rail、composer、bottom status 在常见宽度下对齐；
  - compact、normal、wide、short terminal 都有 snapshot/smoke 覆盖。
- Approval modal：
  - 默认焦点停在 approve；
  - modal 打开时快捷键仍可用；
  - 支持鼠标点击 approve、deny、diff、apply-all；
  - 决策后 modal 立即消失。
- Command palette：
  - `/` 打开可用的提示列表；
  - 方向键选择，Enter 补全，Tab 补全，Esc 关闭；
  - 描述文本不溢出 palette，也不把 composer 推乱。
- Rendering safety：
  - ANSI、OSC、emoji、中文宽字符、shell prompt 噪声都不能破坏 panel 对齐；
  - right rail 数据必须在 panel 内截断或换行。

验收证据：

- `cargo test -p robocode-cli`。
- TUI preview generation 通过，并覆盖 composer、command palette、modal、lane
  detail、side-1、side-2 和 multiscreen snapshots。
- 手动或 tmux-driven fallback TUI smoke 覆盖输入、slash suggestions、approval、
  resize 和 `/exit`。
- 最终视觉 checkpoint 保存截图或 rendered preview。

## P1：编程闭环

这是本版本的核心价值。

- Diff review：
  - 按 added/modified/deleted 显示 changed files；
  - approval 前和 tool call 后显示简洁 diff summary；
  - 下一步动作明确：approve、deny、inspect、run tests 或 continue。
- Test workflow：
  - 增加面向操作者的 `/test` 或 `/run test` 流程；
  - 汇总 exit code、失败命令、失败文件和有用 tail lines；
  - test evidence 绑定到当前 task/session。
  - 当前 checkpoint：`/test <command>` 已经通过 shell permission path 实现，
    `/status` 会显示最近一次测试的 command、status、duration 和 output tail；
    exit-code / failing-file 提取仍是后续细化项。
- Structured tool results：
  - compile errors、test failures、lint failures、shell failures 以结构化证据显示，
    不直接堆 raw log；
  - 成功写文件时显示 path、size 和简短 effect summary。
- Task continuity：
  - active task panel 反映真实 `/task` 和 lane state；
  - resume session 后显示上次改了什么、测了什么、还剩什么。
- Status clarity：
  - `/status` 显示 provider、model、context、permissions、dirty files、active
    task、last test result 和 recent lane state。

验收证据：

- fallback provider coding smoke 创建或修改一个小文件，审查 diff，运行测试命令，
  并干净退出。
- DeepSeek V4 Flash coding smoke 完成一个小代码或脚本任务，至少包含一次 tool call
  和一次 verification command。
- session transcript 能证明 diff/test evidence 已记录。

## P1：Lane 操作体验

Lane 有用的标准是减少上下文切换，而不是多一个看 log 的地方。

- Lane creation：
  - `/lane run`、`/lane codex`、`/lane claude`、`/lane ask` 能从 command palette
    发现；
  - 缺少 template 时明确告诉用户要配置什么。
- Lane inspect：
  - 显示 command、status、pid/session、workspace、changed files、last output、
    exit code、decision artifacts 和建议下一步；
  - tmux 和 PTY attach/send 路径要明显。
- Lane apply and recovery：
  - accept/apply/resolve/cleanup 是一个有引导的序列；
  - conflict evidence 清楚，但不暗示系统已经自动 revert。
- Side screens：
  - side-1 优先展示 agent lanes 和 live output；
  - side-2 优先展示 diagnostics、tests、repo state 和 pressure indicators；
  - 即使没有 external agents，副屏也要有用。

验收证据：

- shell lane smoke 覆盖 create、inspect、complete、archive 或 cleanup。
- tmux lane smoke 覆盖 attach command generation、log capture、inspect 和 cleanup。
- template-lane dry run 证明 missing-template 和 configured-template 两条路径都容易理解。

## P2：项目上下文与引导

核心闭环稳定后再深化这些体验。

- `/context` 显示 current files、dirty changes、task、recent tests、recent
  diagnostics、active provider 和 lane state。
- LSP diagnostics 可以生成 fix task 或 lane。
- Recent files 和 workspace panels 优先展示与当前任务相关的文件。
- Provider health 显示可行动的失败消息，而不只是 latency 数字。
- Release notes 明确说明哪些是真功能、哪些是 preview、哪些有意后置。

## Release Smoke Matrix

打 `v0.1.5` tag 前运行：

- `cargo fmt --check`
- `cargo test -p robocode-cli`
- `cargo test --workspace --quiet`
- `scripts/tui-previews.sh /tmp/robocode-015-preview`
- fallback TUI coding smoke
- DeepSeek V4 Flash TUI coding smoke
- shell lane operator smoke
- tmux lane operator smoke
- host platform package smoke
- GitHub Actions release artifact validation，全部配置目标，`upload_to_release=false`

## 开发切片

1. Composer、cursor、IME 和 resize 稳定性。
2. Approval modal 和 command palette ergonomics。
3. 主编程闭环里的 diff 和 test evidence。
4. Lane inspect/apply/side-screen operator flow。
5. Context/status polish 和 final release smoke。

## 对标后的产品决策

- RoboCode 继续定位为 local terminal cockpit，不做 cloud task runner，也不替代完整 editor。
- 对齐 Codex 和 Zed 对集中 diff/review evidence 的体验预期。
- 对齐 Claude Code 对顺滑 terminal action loop、permissions 和后续 hooks 的体验预期，
  但 `0.1.5` 先聚焦 approval 和 test loop。
- 对齐 DeepSeek-TUI 的 terminal density 和 DeepSeek V4 Flash provider 可见性，
  但只做有真实 runtime state 支撑的 panel。
- Lane 视为受监督的 work threads。副屏应该展示 agent lanes、tests、diagnostics、
  repo state 和 recommended next actions。
- MCP、remote automation 和完整 terminal emulator 后置，除非它们直接阻塞编程闭环。

## Exit Criteria

`0.1.5` ready 的标准：

- 用户可以在 TUI 里完成一个小型真实编程任务，而不是每一步都切回临时 shell；
- typing、resize、approval 和 lane updates 时 TUI 视觉稳定；
- shell 和 tmux lanes 提供有用证据，而不是 raw noise；
- DeepSeek V4 Flash 可以完成小型 live coding smoke；
- release artifacts 能为所有配置目标构建成功。
