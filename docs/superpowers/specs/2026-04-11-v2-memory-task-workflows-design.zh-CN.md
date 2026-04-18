# V2 Memory 与 Task Workflows 设计

## 目的

本文定义 RoboCode 的 V2-C 设计：memory 与 task workflows。目标是把项目连续性从“偶然存在”提升为“显式建模”，通过引入项目级任务、双层记忆，以及 workflow 导向的 resume 能力，同时不破坏现有 session、permission 与 transcript 模型。

本设计遵循已确认方向：

- `tasks` 是项目级持久状态
- `memory` 分为 `project memory` 与 `session memory`
- `project memory` 采用建议驱动，必须显式确认后才能正式写入
- 首版命令面直接覆盖较完整的 task 与 memory 流程
- `/task resume-context` 是 workflow 驱动的，但不会静默修改 task 的业务状态

## 产品目标

RoboCode 应该帮助开发者回到一个项目时，快速回答：

- 当前有哪些事情正在推进
- 哪些任务被阻塞
- 有哪些长期决策、约束或偏好需要记住
- 本次 session 最近发生了什么
- 当前最合理的下一步是什么

V2-C 必须在 CLI 内部直接给出这些答案，并继续复用现有的 shared runtime path。

## 范围

范围内：

- 项目级 task 生命周期管理
- project memory 与 session memory
- workflow 导向的 resume 摘要
- memory suggestion 与 confirm 流程
- 所有 workflow 命令都可见于 transcript
- durable workflow storage 与派生索引

范围外：

- LSP 集成
- 超出占位 assignee 字段之外的真实 multi-agent ownership
- 定时 automation
- MCP 驱动的 workflow 同步
- 超出现有文本 CLI 的 richer TUI

## 架构

### 新增 Crate

新增：

- `robocode-workflows`

这个 crate 对 `robocode-core` 暴露统一入口，但内部从第一天就拆成这些模块：

- `tasks`
- `memory`
- `resume_context`
- `stores`

这是一种折中架构：既不把首版实现拆得过重，也不给未来的边界演化埋坑。

### 职责划分

- `robocode-core`
  负责解析 slash commands、路由 workflow 动作、执行权限检查、写 transcript command entries，并渲染 CLI 输出
- `robocode-session`
  继续作为 session transcript history 与 session index 的事实源
- `robocode-workflows`
  负责项目级 task 状态、project/session memory 状态、resume-context 派生逻辑，以及 workflow 专属持久化

关键不变量：

- transcript 记录 session 里发生了什么
- workflow storage 记录项目长期 task/memory 状态变成了什么

两者不能互相吞并。

## 数据模型

### Task

`Task` 是项目级 workflow 的主对象。

必需字段：

- `task_id`
- `title`
- `description`
- `status`
- `priority`
- `labels`
- `assignee_hint`
- `parent_task_id`
- `dependency_ids`
- `blocked_by`
- `notes`
- `created_at`
- `updated_at`
- `last_session_id`
- `last_seen_at`
- `archived_at`

V2-C 的 task status：

- `todo`
- `in_progress`
- `blocked`
- `done`
- `archived`

V2-C 的 priority：

- `low`
- `medium`
- `high`
- `critical`

子任务不单独做一套类型，而是直接用 `parent_task_id` 表达层级。

`blocked_by` 可表示为：

- 另一个 task id
- 一段自由文本的阻塞原因

### Memory Entry

project memory 与 session memory 共享一套 entry 结构。

必需字段：

- `memory_id`
- `scope`
- `session_id`
- `kind`
- `content`
- `source`
- `status`
- `created_at`
- `updated_at`
- `related_task_ids`
- `confidence_hint`

memory scope：

- `project`
- `session`

memory kind：

- `fact`
- `preference`
- `constraint`
- `decision`
- `convention`

memory source：

- `user`
- `assistant_suggestion`
- `command`
- `imported`

memory status：

- `suggested`
- `active`
- `superseded`
- `pruned`
- `rejected`

规则：

- project memory 在模型建议流程中，先以 `suggested` 存在
- session memory 可通过显式命令直接写入
- project memory 不允许在未确认时直接进入 `active`

### Resume Context Snapshot

`ResumeContextSnapshot` 是一个派生对象，不是长期主存储实体。

应包含：

- active tasks
- blocked tasks
- recently completed tasks
- relevant project memory
- recent session memory
- suggested next steps
- suggested session memory additions

它主要服务 `/task resume-context`，并且必须能够通过 workflow 状态与最近 session 元数据重建。

## 持久化模型

### Canonical Storage

workflow 状态不应直接写进 session transcript 作为主事实源。

在 session home 下、与 project key 对齐的目录中新增 workflow 数据区，与 session transcript 目录并列。

canonical 文件：

- `tasks.jsonl`
- `memory.jsonl`

派生索引：

- `workflow.sqlite3`

### 事件模型

`tasks.jsonl` 采用 append-only event log，事件类型包括：

- `task_created`
- `task_updated`
- `task_status_changed`
- `task_linked`
- `task_blocked`
- `task_unblocked`
- `task_archived`
- `task_restored`

`memory.jsonl` 采用 append-only event log，事件类型包括：

- `memory_suggested`
- `memory_confirmed`
- `memory_rejected`
- `memory_added`
- `memory_pruned`
- `memory_superseded`

### 与 Transcript 的关系

transcript 继续记录：

- slash command 的调用与输出
- suggestion 的生成
- confirm / reject 动作

workflow event log 记录：

- task 与 memory 的真实状态变更

两边都可以带轻量引用：

- `origin_session_id`
- 产生的 `task_id`
- 产生的 `memory_id`

系统应能把 session 行为和 workflow 状态联系起来，但不能要求通过重放 transcript 才能恢复 workflow 状态。

## 命令面

### Task 命令

V2-C 首版必须覆盖：

- `/tasks`
- `/task add <title>`
- `/task view <task-id>`
- `/task update <task-id>`
- `/task status <task-id> <status>`
- `/task link <task-id> <depends-on-id>`
- `/task block <task-id> <reason|task-id>`
- `/task unblock <task-id>`
- `/task archive <task-id>`
- `/task restore <task-id>`
- `/task resume-context`

预期行为：

- `/tasks` 默认显示当前项目的活跃任务
- `/task view` 展示单个 task 的完整状态
- `/task link` 建立任务依赖
- `/task block` 可用 task id 或文本原因标记阻塞
- `/task resume-context` 输出 workflow 摘要与下一步建议

### Memory 命令

V2-C 首版必须覆盖：

- `/memory`
- `/memory project`
- `/memory session`
- `/memory add <content>`
- `/memory suggest`
- `/memory confirm <memory-id>`
- `/memory reject <memory-id>`
- `/memory prune <memory-id>`
- `/memory export`

预期行为：

- `/memory` 默认显示 active project memory 摘要
- `/memory project` 仅显示 project memory
- `/memory session` 仅显示当前 session memory
- `/memory add` 支持显式手动写入
- `/memory suggest` 展示待确认的建议项
- `/memory confirm` 把 project memory suggestion 提升为 active
- `/memory reject` 保留完整审计痕迹
- `/memory prune` 让 memory 退役，但不删历史
- `/memory export` 输出可读、可持久化的 memory 快照

## 权限与确认模型

workflow 命令不能绕开现有权限系统。

命令分三类：

- 只读命令
- 受控写入命令
- 建议确认命令

只读命令：

- `/tasks`
- `/task view`
- `/task resume-context`
- `/memory`
- `/memory project`
- `/memory session`
- `/memory suggest`

受控写入命令：

- `/task add`
- `/task update`
- `/task status`
- `/task link`
- `/task block`
- `/task unblock`
- `/task archive`
- `/task restore`
- `/memory add`
- `/memory prune`

建议确认命令：

- `/memory confirm`
- `/memory reject`

规则：

- 只读命令默认直接执行，除非未来策略另有约束
- 写入命令继续走现有 permission engine
- project memory 可由 assistant 生成 suggestion，但在 confirm 前不算 active
- transcript 与 workflow logs 都必须记录确认结果

## `/task resume-context` 行为

`/task resume-context` 是 V2-C 的核心入口。

它的输出应分为四部分：

1. project workflow summary
2. memory summary
3. suggested next steps
4. suggested session-memory updates

允许的副作用：

- 更新进入上下文的 task 的 `last_seen_at`
- 为当前 session 明确引用的 task 更新 `last_session_id`
- 生成下一步 task focus 建议
- 生成 session memory suggestion 草案

不允许的副作用：

- 静默改变 task status
- 自动确认 project memory
- 自动 archive / restore / relink task

它的“workflow 驱动”含义是帮助用户决定下一步做什么，而不是在后台偷偷推进 workflow 状态。

## CLI 渲染要求

V2-C 继续保持 RoboCode 当前的轻量 CLI 风格。

渲染建议：

- `/tasks` 用紧凑列表，显示 status、priority 与 blocker 信号
- `/task view` 用详情卡片式文本
- `/memory suggest` 以可确认条目的形式输出
- `/task resume-context` 先输出摘要，再输出明确建议动作

这一版不要求 rich TUI。

## 测试策略

必需测试类别：

- task event roundtrip tests
- memory event roundtrip tests
- workflow 派生索引重建测试
- `robocode-core` 命令路由测试
- workflow 写入命令的权限集成测试
- project memory suggestion-confirmation 流程测试
- `/task resume-context` 派生逻辑测试
- workflow 命令的 transcript logging 测试

关键场景：

- 同一项目跨多个 session 创建和更新任务
- 建立 task link / block，再在后续 session 恢复上下文
- project memory suggestion 中部分 confirm、部分 reject
- 显式写入 session memory，并验证 scope 隔离
- 在混合 task/memory 历史下生成 resume context
- 通过 append-only event logs 重建 workflow index

## 本阶段非目标

V2-C 暂不尝试：

- 真实 agent assignment 或 ownership enforcement
- 每轮对话都自动任务规划
- 跨项目 memory federation
- remote workflow sync
- scheduled workflow execution
- semantic code intelligence

## 文件方向

预期新增文件与模块：

- `robocode-workflows/Cargo.toml`
- `robocode-workflows/src/lib.rs`
- `robocode-workflows/src/tasks.rs`
- `robocode-workflows/src/memory.rs`
- `robocode-workflows/src/resume_context.rs`
- `robocode-workflows/src/stores.rs`

预期集成点：

- `Cargo.toml`
- `robocode-core/src/lib.rs`
- `robocode-session/src/lib.rs`
- `robocode-types/src/lib.rs`
- `README.md`
- `README.zh-CN.md`

## 退出标准

当 RoboCode 能做到以下几点时，本设计算落地：

- 跟踪具有生命周期与依赖状态的项目级 task
- 分离维护 project memory 与 session memory
- 对 assistant 建议的 project memory 强制显式确认
- 生成实用的 workflow-oriented resume context
- 在整个过程中继续保持 transcript 与 permission 的核心不变量
