# V2 Memory 与 Task Workflows 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**目标：** 在不破坏现有 session、permission 与 transcript 不变量的前提下，为 RoboCode 增加项目级 tasks、双层 memory，以及 workflow 导向的 resume context。

**架构：** 本阶段新增 `robocode-workflows` crate，内部拆成 `tasks`、`memory`、`resume_context`、`stores` 四个模块。`robocode-session` 继续作为 transcript 的事实源；workflow 状态保存在 append-only workflow event logs 和派生 SQLite index 中；`robocode-core` 负责把 workflow 命令接入现有 command、permission 与 transcript 主链路。

**技术栈：** Rust workspace crates、JSONL append-only logs、SQLite derived indexes、现有 RoboCode REPL / command runtime

---

## 范围

范围内：

- 新增 `robocode-workflows` crate
- 项目级 task 生命周期状态
- project memory 与 session memory
- `/tasks`、`/task ...`、`/memory ...` 命令族
- `/task resume-context`
- workflow event logs 与派生 SQLite index
- 所有 workflow 命令继续写 transcript command entries

范围外：

- LSP
- MCP
- multi-agent ownership semantics
- automation / cron
- rich TUI

## 目标行为

- RoboCode 能创建、更新、阻塞、关联、归档、恢复项目级 task。
- RoboCode 能直接写 session memory，并通过 suggest/confirm 流程维护 project memory。
- `/task resume-context` 能展示当前活跃工作、阻塞项、相关 memory 与下一步建议。
- workflow 状态可以从 append-only task/memory event logs 重建。
- workflow 命令继续走 shared command path，并写入 transcript command entries。
- workflow 写操作继续经过现有 permission 模型。

## 文件映射

**新增：**

- `robocode-workflows/Cargo.toml`
- `robocode-workflows/src/lib.rs`
- `robocode-workflows/src/tasks.rs`
- `robocode-workflows/src/memory.rs`
- `robocode-workflows/src/resume_context.rs`
- `robocode-workflows/src/stores.rs`
- `docs/superpowers/plans/2026-04-11-v2-memory-task-workflows.md`

**修改：**

- `Cargo.toml`
- `robocode-core/src/lib.rs`
- `robocode-session/src/lib.rs`
- `robocode-types/src/lib.rs`
- `robocode-permissions/src/lib.rs`
- `README.md`
- `README.zh-CN.md`

## Task 1：建立 workflow crate 骨架与共享类型

**文件：**

- 新增：`robocode-workflows/Cargo.toml`
- 新增：`robocode-workflows/src/lib.rs`
- 修改：`Cargo.toml`
- 修改：`robocode-types/src/lib.rs`

- [ ] 在 `Cargo.toml` 中把 `robocode-workflows` 加入 workspace members。
- [ ] 创建 `robocode-workflows/Cargo.toml`，依赖 `robocode-types`、`serde` 以及仓库里已使用的最小持久化辅助库。
- [ ] 在 `robocode-workflows/src/lib.rs` 中先导出空模块：
  - `tasks`
  - `memory`
  - `resume_context`
  - `stores`
- [ ] 在 `robocode-types/src/lib.rs` 中增加 workflow 共享类型：
  - `TaskId`
  - `MemoryId`
  - `TaskStatus`
  - `TaskPriority`
  - `MemoryScope`
  - `MemoryKind`
  - `MemorySource`
  - `MemoryStatus`
- [ ] 在 `robocode-types/src/lib.rs` 中增加数据结构：
  - `TaskRecord`
  - `MemoryEntry`
  - `ResumeContextSnapshot`
- [ ] 保持这些新类型与现有 session/transcript 模式兼容，至少包含：
  - `Debug`
  - `Clone`
  - `Serialize`
  - `Deserialize`
  - 在适合时增加 `PartialEq`
  - 在适合时增加 `Eq`
- [ ] 在 `robocode-types/src/lib.rs` 中为新 enum 增加 CLI / serde roundtrip 单测。
- [ ] 运行：`cargo test -p robocode-types`
- [ ] 在 crate 骨架与共享类型稳定后做一次小提交。

## Task 2：实现 workflow storage paths 与 event-log persistence

**文件：**

- 新增：`robocode-workflows/src/stores.rs`
- 修改：`robocode-session/src/lib.rs`
- 修改：`robocode-types/src/lib.rs`

- [ ] 在 `robocode-workflows/src/stores.rs` 中增加 storage-path helpers，从当前 cwd 与 session home 推导 per-project workflow home。
- [ ] 复用 `robocode-session` 已有的 project-key 约定，不引入第二套项目 identity。
- [ ] 定义 task events 与 memory events 的 append-only event payload structs。
- [ ] 增加 canonical 文件位置：
  - `tasks.jsonl`
  - `memory.jsonl`
  - `workflow.sqlite3`
- [ ] 实现 task/memory event 的 append helpers。
- [ ] 实现 task/memory event streams 的 load/replay helpers。
- [ ] 增加 workflow 派生 SQLite index 的初始化与重建路径，保持和现有 “canonical JSONL + rebuildable SQLite” 思路一致。
- [ ] 如果缺少必要的 project-key 或 path helper，可在 `robocode-session/src/lib.rs` 中暴露，但不要把 workflow 状态挪进该 crate。
- [ ] 在 `robocode-workflows/src/stores.rs` 中补测试：
  - path derivation
  - JSONL append/load roundtrip
  - SQLite rebuild from event logs
- [ ] 运行：`cargo test -p robocode-workflows stores`
- [ ] 在 storage 与 rebuild 行为稳定后提交。

## Task 3：实现项目级 task domain 与 reducer

**文件：**

- 新增：`robocode-workflows/src/tasks.rs`
- 修改：`robocode-workflows/src/lib.rs`
- 修改：`robocode-types/src/lib.rs`

- [ ] 在 `robocode-workflows/src/tasks.rs` 中定义 task-domain commands/events：
  - create
  - update
  - status change
  - link dependency
  - block
  - unblock
  - archive
  - restore
- [ ] 用 `parent_task_id` 表示 hierarchy，不引入独立 subtask 类型。
- [ ] `blocked_by` 支持两种形式：
  - another task id
  - free-form text reason
- [ ] 实现 reducer，通过 task events 重建 `TaskRecord` 状态。
- [ ] 实现 task query helpers：
  - active tasks
  - blocked tasks
  - archived tasks
  - task lookup by id
  - child tasks
- [ ] 验证关键不变量：
  - archived tasks 不能重复 archive
  - dependency links 不能指向不存在 task
  - restore 必须从 archived 状态恢复
- [ ] 增加测试：
  - create/update roundtrip
  - link/block/unblock 行为
  - archive/restore 行为
  - hierarchy reconstruction
- [ ] 运行：`cargo test -p robocode-workflows tasks`
- [ ] 在 task domain 稳定后单独提交，再开始 memory。

## Task 4：实现 project/session memory domain 与 suggestion flow

**文件：**

- 新增：`robocode-workflows/src/memory.rs`
- 修改：`robocode-workflows/src/lib.rs`
- 修改：`robocode-types/src/lib.rs`

- [ ] 在 `robocode-workflows/src/memory.rs` 中定义 memory-domain commands/events：
  - add
  - suggest
  - confirm
  - reject
  - prune
  - supersede
- [ ] 强化 scope 规则：
  - session memory 可以直接写入
  - project memory suggestion 初始状态为 `suggested`
  - project memory 必须 confirm 后才能变成 `active`
- [ ] 实现 reducers/query helpers：
  - active project memory
  - active session memory
  - pending suggestions
  - pruned/superseded history
- [ ] 通过 `related_task_ids` 支持 memory 与 task 关联。
- [ ] 增加测试：
  - direct session-memory add
  - project-memory suggest/confirm flow
  - reject flow
  - prune/supersede flow
  - scope isolation by session id
- [ ] 运行：`cargo test -p robocode-workflows memory`
- [ ] 在 memory 行为与测试稳定后提交。

## Task 5：实现 resume-context 派生逻辑

**文件：**

- 新增：`robocode-workflows/src/resume_context.rs`
- 修改：`robocode-workflows/src/lib.rs`
- 修改：`robocode-session/src/lib.rs`
- 修改：`robocode-types/src/lib.rs`

- [ ] 在 `robocode-workflows/src/resume_context.rs` 中增加 resume-context builder，输入包括：
  - current task state
  - memory state
  - recent session summaries 与必要的 recent transcript metadata
- [ ] 产出 `ResumeContextSnapshot`，包含：
  - active tasks
  - blocked tasks
  - recently completed tasks
  - relevant project memory
  - recent session memory
  - suggested next steps
  - suggested session-memory additions
- [ ] 只允许以下派生副作用：
  - 更新 `last_seen_at`
  - 更新 `last_session_id`
- [ ] 不允许 `resume-context` 自动修改 task status 或自动确认 project memory。
- [ ] 增加测试：
  - active/blocked/recent task selection
  - relevant memory selection
  - suggested next-step output
  - 仅更新派生字段、不修改 task-status
- [ ] 运行：`cargo test -p robocode-workflows resume_context`
- [ ] 在 resume-context 行为确定后提交。

## Task 6：把 workflow runtime 接入 RoboCode core

**文件：**

- 修改：`robocode-core/src/lib.rs`
- 修改：`robocode-permissions/src/lib.rs`
- 修改：`robocode-types/src/lib.rs`
- 修改：`Cargo.toml`

- [ ] 在需要的位置增加 `robocode-workflows` 依赖。
- [ ] 扩展 `SessionEngine` 初始化，让它可以基于当前 cwd 与 session home 构造 workflow runtime/store。
- [ ] 增加只读命令处理：
  - `/tasks`
  - `/task view`
  - `/task resume-context`
  - `/memory`
  - `/memory project`
  - `/memory session`
  - `/memory suggest`
- [ ] 增加写命令处理：
  - `/task add`
  - `/task update`
  - `/task status`
  - `/task link`
  - `/task block`
  - `/task unblock`
  - `/task archive`
  - `/task restore`
  - `/memory add`
  - `/memory confirm`
  - `/memory reject`
  - `/memory prune`
  - `/memory export`
- [ ] 保持所有 workflow 命令继续走现有 slash-command pipeline，并写入 `TranscriptEntry::Command`。
- [ ] 在 `robocode-permissions/src/lib.rs` 中增加 workflow 写操作的权限集成：
  - reads default-allow
  - workflow writes ask by default，除非 mode/rules 覆盖
- [ ] 在 core 中增加测试：
  - command parsing
  - transcript command logging
  - mutating workflow commands 的 permission gating
  - memory confirm/reject command paths
  - `/task resume-context` rendering
- [ ] 运行：`cargo test -p robocode-core`
- [ ] 在 CLI 命令面稳定后提交。

## Task 7：补 workflow summaries、exports 与文档

**文件：**

- 修改：`robocode-core/src/lib.rs`
- 修改：`README.md`
- 修改：`README.zh-CN.md`

- [ ] 优化 CLI 渲染：
  - `/tasks` 显示紧凑列表，包含 status、priority、blocker hints
  - `/task view` 显示完整详情
  - `/memory suggest` 清晰显示 pending items
  - `/task resume-context` 先输出 summary，再输出 suggested actions
- [ ] 实现 `/memory export` 的稳定、可读输出格式。
- [ ] 在 README 中增加命令示例：
  - `/tasks`
  - `/task add`
  - `/task resume-context`
  - `/memory suggest`
  - `/memory confirm`
- [ ] 中英文 README 一起更新。
- [ ] 运行聚焦 smoke checks：
  - `cargo run -p robocode-cli -- --provider fallback --model test-local`
  - `/task add`
  - `/tasks`
  - `/task resume-context`
  - `/memory add`
  - `/memory suggest`
  - `/memory confirm`
- [ ] 在文档与渲染稳定后最后提交。

## Task 8：最终验证与收尾

**文件：**

- 修改：`robocode-workflows/src/*.rs`
- 修改：`robocode-core/src/lib.rs`
- 修改：`README.md`
- 修改：`README.zh-CN.md`

- [ ] 在实现过程中持续运行聚焦测试：
  - `cargo test -p robocode-types`
  - `cargo test -p robocode-workflows`
  - `cargo test -p robocode-core`
- [ ] 最终运行全量验证：
  - `cargo test --workspace --quiet`
- [ ] 做最终 CLI smoke pass，覆盖：
  - `/tasks`
  - `/task add`
  - `/task block`
  - `/task resume-context`
  - `/memory add`
  - `/memory suggest`
  - `/memory confirm`
  - `/memory export`
- [ ] 验证 workflow 数据确实写在独立 workflow store 中，而 transcript 仍然只记录 workflow 命令行为。
- [ ] 验证完成后，使用标准 branch-finishing 流程决定 merge、PR 或 keep-as-is。

## 验收标准

- RoboCode 拥有新的 `robocode-workflows` crate，内部包含 task、memory、resume-context、store 模块。
- Task 状态是项目级持久状态，并且可通过 append-only task events 重建。
- Project memory 与 session memory 清晰分离，并符合已确认的 scope 规则。
- Project memory suggestion 必须经显式确认后才成为 active。
- `/task resume-context` 能产出有价值的 workflow context，且不会静默修改 task 的业务状态。
- Workflow 命令继续出现在 transcript command history 中，并服从 permissions。
- Workspace 全量测试通过。

## 后续工作

本计划完成后，最自然的后续计划是：

- V2-B LSP foundation
- V2-D rich TUI and structured workflow views
- 在 workflow 状态成熟后，再考虑 V3 automation
