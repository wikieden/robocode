# viden-workflows

## 目的

`viden-workflows` 负责持久项目 workflow state：tasks、project/session memory、resume context、workflow event storage。

## 不负责

- Session transcript facts；使用 `viden-session`。
- Slash-command parsing；使用 `viden-runtime`。
- 权限决策；core 会把 workflow writes 交给 `viden-permissions`。

## 公共接口

- `tasks`：task reducer 和查询。
- `memory`：project/session memory reducer 和查询。
- `lanes`：typed lane 生命周期 reducer 与旧 lane 迁移。
- `resume_context`：`/task resume-context` builder。
- `stores`：workflow JSONL logs 和 derived SQLite bootstrap。

## 内部模块

### `tasks`

负责 `TaskEvent`、`TaskUpdate`、`TaskBlocker`、`TaskState`、`reduce_task_events`。支持 create、update、status、link、block、unblock、archive、restore、父子层级、依赖、派生 `Seen` 事件。

### `memory`

负责 `MemoryEvent`、`MemoryState`、`reduce_memory_events`。支持 session memory add、project memory suggest/confirm/reject、prune、supersede、active project/session memory、pending suggestions。

### `resume_context`

负责 `ResumeContextInput`、`ResumeContextBuild`、`build_resume_context`。产出 `ResumeContextSnapshot`、next steps 建议、session memory 建议和派生 task `Seen` 事件。不能改变 task 业务状态，也不能自动 confirm project memory。

### `lanes`

负责 `LaneEvent`、`LaneState` 和 `reduce_lane_events`。Lane 生命周期事实写入 `lanes.jsonl`。旧项目的 `.viden/lanes.tsv` 只作为幂等的 session 启动或 resume activation 迁移输入；此后 runtime projection 与状态视图都读取 typed event log。

### `stores`

负责 `WorkflowStore`、`WorkflowPaths`、`WorkflowTaskEvent`、`WorkflowMemoryEvent`。把 canonical workflow logs 存到 `tasks.jsonl`、`memory.jsonl` 和 `lanes.jsonl`，创建 `workflow.sqlite3`，并在 checked append 前校验事件有效性。Lane 的 load/reduce/append 事务使用项目级 advisory lock，避免并发 session 重复导入旧格式或丢失生命周期事件。

## 不变量

- Workflow JSONL 是 canonical。
- SQLite 是 derived 且可重建。
- 无效 task、memory 或 lane events 不允许 append。
- Lane 日志损坏必须作为可恢复 runtime error 暴露，客户端不能静默渲染为空 lane 集合。
- Workflow state 和 transcript state 分离，但共享 project identity。

## `.ref` 对齐

借鉴 `.ref/src/tasks/*` 与 session workflow 思路，但保持更小的 Rust event-log 模型。

## 测试

```bash
cargo test -p viden-workflows
```
