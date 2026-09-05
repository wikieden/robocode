# Viden TUI 分支线 Agent 指南

英文版：[AGENTS.md](AGENTS.md)

## 作用范围

本文件适用于 `apps/tui/**`。在改动 TUI 行为或视觉之前，先读仓库根目录的
`AGENTS.md`、`docs/parallel-development-plan.md`，以及
`docs/viden-design/Viden/TUI/` 下当前的设计来源。

## 启动门禁

对 V3 正式实现，任务必须点名不可变的 `frontend-contract-v1` Core checkpoint
SHA 和目标 TUI 版本，从用于薄客户端 parity 的 `tui-v0.3.0` 开始。

- 从那个确切的 checkpoint 在 `.worktrees/v3-tui-client` 中创建
  `codex/v3-tui-client`。
- 如果拿不到 checkpoint SHA，就停留在交互/参考评审和 Core contract 缺口分析
  阶段。不要开始正式迁移。
- 在改动正式 UI 之前，先与用户确认 TUI 交互决策和参考界面。
- 按此顺序读设计：`docs/viden-design/Viden/index.html`，然后是 TUI
  设计稿索引，然后是 `Viden - 统一原型 (TUI).html`，最后是 TUI
  组件库。单个 TUI 页面是补充证据，不是起点。

## 归属与客户端边界

- 拥有 `apps/tui/**`、TUI 专属测试、TUI previews/截图，以及 TUI
  用户文档。
- 只从 `RuntimeViewState` 加 TUI 本地布局/输入状态渲染。
- 通过 Core client 发送 `RuntimeCommand`，并等待有序的
  `RuntimeEvent` 确认。
- 不拥有权威的 lane 生命周期、worktrees、terminal/tmux/PTY 启动、
  accept/apply、冲突恢复、持久化、provider/tool 执行或 permission reduction。
- 当 Core 为某个事实提供了 event 时，不要从 transcript 文本、命令输出或进程
  退出文案推断成功。
- 缺失的字段或命令是 Core contract requests。不要为绕过它们而建立
  TUI 私有业务模型。
- 未与归属分支线协调时，不要编辑 `crates/**`、`apps/gui/**`
  或共享 design tokens。

## 交互验收

在对齐已接受的 TUI 设计的同时，保持 0.1.30 zero-bug 基线：

- Normal、Insert 和 Overlay 输入模式各有明确归属。
- `Esc` 按 overlay -> selection -> insert 逐层回退；`Ctrl-C` 只针对当前工作。
- streaming、tools 和 approvals 期间 composer 仍然可用。
- Queue/cancel、bracketed paste、多行输入、CJK 宽度/光标行为、
  scrollback、resize、focus 和 selector-first 导航不得回归。
- Approval 动作保持固定且可操作；ambient ticker 内容不携带动作。
- 共享 Core fixtures 重放出的业务事实与 GUI 一致。
- 语言、locale、skin、mode、density 和 terminal color capability 来自
  Core presentation preferences。把共享 design tokens 映射到 terminal truecolor、
  ANSI 256 和 ANSI 16 降级；不要建立私有 TUI 主题注册表。
- 用户可见的 selector 标签、approval 文案、status rows 和窄屏回退，
  在英文和简体中文下都必须成立。

## 开发与验证

- 行为变更使用 TDD，并保持双语文档同步。
- 复用 canonical TUI kit 和已注册字形；不要引入 emoji 或私有 design tokens。
- 运行：

```bash
cargo test -p viden-tui
scripts/tui-turn-controller-smoke.sh
scripts/rc-tui-stability-smoke.sh
scripts/tui-regression.sh
cargo test --workspace --quiet
```

交接必须包含基线 Core SHA、分支、worktree、HEAD、fixture 重放结果、
smoke/regression 证据、截图、contract requests 和 blockers。没有用户明确指示，
不要合并或推送 `main`。
