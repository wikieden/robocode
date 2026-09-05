# Viden GUI 分支线 Agent 指南

英文版：[AGENTS.md](AGENTS.md)

## 作用范围

本文件适用于 `apps/gui/**`。在实现之前，先读仓库根目录的 `AGENTS.md`、
`docs/parallel-development-plan.md`、`docs/gui-version-functional-design.md`、
GPUI 可行性调研，以及 `docs/viden-design/Viden/GUI/` 下当前的设计来源。

## 启动门禁

对 V3 正式实现，任务必须点名不可变的 `frontend-contract-v1` Core checkpoint
SHA 和目标 GUI 版本，从用于本地 cockpit 垂直切片的 `gui-v0.1.0` 开始。

- 从那个确切的 checkpoint 在 `.worktrees/v3-gui-client` 中创建
  `codex/v3-gui-client`。
- 如果拿不到 checkpoint SHA，就停留在交互/参考评审、框架 spike 规划和
  Core contract 缺口分析阶段。
- 在构建正式客户端之前，先与用户确认 GUI 交互和参考界面。
- 按此顺序读设计：`docs/viden-design/Viden/index.html`，然后是 GUI
  设计稿索引，然后是 `Viden - 桌面驾驶舱 (GUI).html`，最后是 GUI
  组件库。D11、D4、D2 和 D6 页面在理解驾驶舱之后用于细化流程。

## 框架门禁（已完成）

`0.1.0-alpha.1` 证据门禁在同一个 Core fixture 和 D1 垂直切片上运行了 Tauri
和 GPUI，并选定 **Tauri** 作为唯一正式框架；GPUI 保留为对比 spike。完整记录、
证据和复现路径见 `docs/gui-framework-decision.md` 和
`apps/gui/evidence/framework-gate/`。产品 contract 保持框架中立：Core
commands/events/snapshot/replay 仍是唯一的 runtime 边界；该门禁中未验证的
Tauri 结果（p95 timing、原生 IME/accessibility、Linux/Windows、soak、
signing/updater/credential、crash recovery）仍是 release blockers，
由后续 release gates 跟踪。没有新的、被记录的门禁决策，
不要启动第二个正式框架。

## 归属与客户端边界

- 拥有 `apps/gui/**`、GUI adapters/components/screens、平台测试、
  截图和 GUI 用户文档。
- 只依赖 `viden-core` 和前端中立的 contracts。
- 每次 mutation 都作为 `RuntimeCommand` 发送，并等待 `RuntimeEvent`。
- 出现 sequence gap 时，通过 snapshot/replay 恢复；绝不靠猜测状态继续。
- 不要直接访问 provider、tools、permissions、sessions、workflows、JSONL
  或 SQLite。
- 缺失的字段或命令是 Core contract requests。不要新增 GUI 私有的 reducer、
  gate 模型、成本估算或执行路径。
- 未与归属分支线协调时，不要编辑 `crates/**`、`apps/tui/**`
  或共享 design tokens。

## 产品顺序与验收

按以下顺序实现第一条本地操作者闭环：

1. D11 项目接入与首启设置。
2. D4 Lane 创建。
3. D1 Cockpit。
4. D2 permission/decision 切片。
5. D6 空态、断线、provider error、context overflow 和 reconnect
   recovery 状态。

把最新的 GUI 设计目录当作唯一视觉参考。复用共享 tokens、已注册组件和品牌
资产；原型 Babel runtime、mock data 和窗口脚手架不是生产 runtime。

语言、locale、skin、mode、density、font scale 和 accessibility flags 来自
Core presentation preferences。GUI 把语言和外观作为配置项暴露，并 import
或派生共享 tokens，而不是交付第二套 skin 调色板。

必须验证与 TUI 的 fixture parity、CJK IME、keyboard-only 操作、可见 focus、
accessibility semantics、event-to-visible 延迟、有界的 transcript 虚拟化、
crash/reconnect 完整性、截图，以及所有受支持平台的 build/launch 路径。
除了本分支上记录的框架专属命令，还要运行 `cargo test --workspace --quiet`。

交接必须包含基线 Core SHA、分支、worktree、HEAD、框架门禁结果、
fixture parity、性能/accessibility 证据、截图、contract requests 和 blockers。
没有用户明确指示，不要合并或推送 `main`。
