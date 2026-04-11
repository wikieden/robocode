# RoboCode 计划索引

英文版： [2026-04-11-robocode-plan-index.md](2026-04-11-robocode-plan-index.md)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把完整的 RoboCode 产品需求拆成一组可按顺序执行的实现计划，避免每次实现前重新讨论产品边界。

**Architecture:** RoboCode 已经有一个早期 V1 基线，因此下一层计划应该建立在现有核心之上按子系统扩展。这份索引是详细执行计划的依赖图，而不是把完整产品目标写成一个超大计划。

**Tech Stack:** Rust workspace、现有 RoboCode crates、Markdown 计划文档

---

## 排序规则

- 先执行 V2 本地开发者增强，再做 V3 平台化工作。
- 每份计划都必须保持 shared engine、permission、transcript 这些不变量。
- 避免为 MCP、agents、remote 建立 side-channel runtime。
- 只有在确实改善边界时才新增 crate 或拆分模块。

## 计划队列

### Plan 1：V2-A Session 与命令面增强

目的：

- 扩展本地命令面
- 让 sessions 更容易查看和恢复
- 在 CLI 内暴露配置和健康状态

主要文件：

- `robocode-cli/src/main.rs`
- `robocode-core/src/lib.rs`
- `robocode-session/src/lib.rs`
- `robocode-types/src/lib.rs`
- `robocode-config/src/lib.rs`

输出：

- 详细计划文件：`docs/superpowers/plans/2026-04-11-v2-session-command-enhancement.md`

### Plan 2：V2-B LSP 基础能力

目的：

- 在不破坏现有工具循环的前提下引入语义级代码智能

预期文件：

- 新建 `robocode-lsp` crate
- `Cargo.toml`
- `robocode-core/src/lib.rs`
- `robocode-tools/src/lib.rs`
- `robocode-types/src/lib.rs`

### Plan 3：V2-C Memory 与 Task 工作流

目的：

- 增加与 sessions 绑定的长期 memory 和 task 状态

预期文件：

- 新建 `robocode-memory` 或 `robocode-workflows` crate
- `robocode-core/src/lib.rs`
- `robocode-session/src/lib.rs`
- `robocode-types/src/lib.rs`

### Plan 4：V2-D 丰富 TUI 与结构化视图

目的：

- 提升 session 浏览、diff 展示和审批体验

预期文件：

- `robocode-cli/src/main.rs`
- 新建面向 TUI 的展示模块或 crate
- `robocode-core/src/lib.rs`

### Plan 5：V3-A MCP 与 Plugin Runtime

目的：

- 引入外部工具生态与扩展加载

预期文件：

- 新建 `robocode-mcp` crate
- 新建 `robocode-plugins` crate
- `robocode-core/src/lib.rs`
- `robocode-tools/src/lib.rs`

### Plan 6：V3-B 多 Agent 与 Coordinator

目的：

- 增加委派执行、teams 与 transcript-safe coordination

预期文件：

- 新建 coordinator / agent crates
- `robocode-core/src/lib.rs`
- `robocode-types/src/lib.rs`
- `robocode-session/src/lib.rs`

### Plan 7：V3-C Bridge、Remote 与 Server Mode

目的：

- 支持 IDE 连接和远程 RoboCode 会话

预期文件：

- 新建 bridge / remote / server crates
- `robocode-core/src/lib.rs`
- `robocode-permissions/src/lib.rs`
- `robocode-session/src/lib.rs`

## 执行顺序

- [ ] 先执行 Plan 1。
- [ ] Plan 1 完成后，再在语义能力与 workflow continuity 之间判断优先级，并选择 Plan 2 或 Plan 3。
- [ ] 在 V2 命令面和 session 面稳定之前，延后 Plan 5 到 Plan 7。

## 退出条件

当以下文档中的每个主要子系统：

- `docs/product-requirements.md`
- `docs/staged-roadmap.md`
- `docs/ref-gap-matrix.md`

都拥有对应的详细执行计划，并且具备明确执行顺序时，这份索引就算完成。
