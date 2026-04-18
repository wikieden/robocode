# robocode-workflows

## 目的

`robocode-workflows` 负责持久项目 workflow state：tasks、project/session memory、resume context、workflow event storage。

## 不负责

- Session transcript facts；使用 `robocode-session`。
- Slash-command parsing；使用 `robocode-core`。
- 权限决策；core 会把 workflow writes 交给 `robocode-permissions`。

## 公共接口

- `tasks`：task reducer 和查询。
- `memory`：project/session memory reducer 和查询。
- `resume_context`：`/task resume-context` builder。
- `stores`：workflow JSONL logs 和 derived SQLite bootstrap。

## 内部模块

### `tasks`

负责 `TaskEvent`、`TaskUpdate`、`TaskBlocker`、`TaskState`、`reduce_task_events`。支持 create、update、status、link、block、unblock、archive、restore、父子层级、依赖、派生 `Seen` 事件。

### `memory`

负责 `MemoryEvent`、`MemoryState`、`reduce_memory_events`。支持 session memory add、project memory suggest/confirm/reject、prune、supersede、active project/session memory、pending suggestions。

### `resume_context`

负责 `ResumeContextInput`、`ResumeContextBuild`、`build_resume_context`。产出 `ResumeContextSnapshot`、next steps 建议、session memory 建议和派生 task `Seen` 事件。不能改变 task 业务状态，也不能自动 confirm project memory。

### `stores`

负责 `WorkflowStore`、`WorkflowPaths`、`WorkflowTaskEvent`、`WorkflowMemoryEvent`。把 canonical workflow logs 存到 `tasks.jsonl` 和 `memory.jsonl`，创建 `workflow.sqlite3`，并在 checked append 前校验事件有效性。

## 不变量

- Workflow JSONL 是 canonical。
- SQLite 是 derived 且可重建。
- 无效 task/memory events 不允许 append。
- Workflow state 和 transcript state 分离，但共享 project identity。

## `.ref` 对齐

借鉴 `.ref/src/tasks/*` 与 session workflow 思路，但保持更小的 Rust event-log 模型。

## 测试

```bash
cargo test -p robocode-workflows
```
