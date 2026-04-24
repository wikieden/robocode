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

状态：

- 已在当前 V2 分支完成

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

状态：

- `codex/v2-lsp-foundation` 上的 active implementation target
- 当前 semantic code intelligence 的 dev baseline

目的：

- 在不破坏现有工具循环的前提下引入语义级代码智能

预期文件：

- 新建 `robocode-lsp` crate
- `Cargo.toml`
- `robocode-core/src/lib.rs`
- `robocode-tools/src/lib.rs`
- `robocode-types/src/lib.rs`

输出：

- 详细计划文件：`docs/superpowers/plans/2026-04-21-v2-lsp-foundation.md`

### Plan 3：V2-C Memory 与 Task 工作流

状态：

- 已在前序分支 `codex/v2-memory-task-workflows` 实现
- 当前待并入更广的 current dev baseline

目的：

- 增加与 sessions 绑定的长期 memory 和 task 状态

预期文件：

- 新建 `robocode-memory` 或 `robocode-workflows` crate
- `robocode-core/src/lib.rs`
- `robocode-session/src/lib.rs`
- `robocode-types/src/lib.rs`

### Plan 4：V2-D 丰富 TUI 与结构化视图

状态：

- planning branch active on `codex/v2-d-structured-views`
- 应先在现有 REPL 中做 structured rendering，再考虑 full-screen TUI

目的：

- 提升 session 浏览、diff 展示和审批体验

预期文件：

- `robocode-cli/src/main.rs`
- 新建面向 TUI 的展示模块或 crate
- `robocode-core/src/lib.rs`

输出：

- 详细计划文件：`docs/superpowers/plans/2026-04-23-v2-d-structured-views.md`

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

- [x] 先执行 Plan 1。
- [x] 因 workflow continuity 优先级更高，先于 Plan 2 执行 Plan 3。
- [ ] 完成 Plan 3 的 merge 或等价落地，使其并入更广的 current dev baseline。
- [ ] 持续推进 Plan 2，直到当前 LSP 分支可合并。
- [ ] 当前 LSP 分支足够稳定后，再开始 Plan 4。
- [ ] Plan 4 的起步阶段先做现有 REPL 中的 structured renderers，不要先上 heavier TUI shell 或新 crate。
- [ ] 在 V2 命令面、session 面、workflow 面和 LSP 面稳定之前，延后 Plan 5 到 Plan 7。

## 退出条件

当以下文档中的每个主要子系统：

- `docs/product-requirements.md`
- `docs/staged-roadmap.md`
- `docs/ref-gap-matrix.md`

都拥有对应的详细执行计划，并且具备明确执行顺序时，这份索引就算完成。
