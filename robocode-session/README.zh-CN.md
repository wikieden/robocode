# robocode-session

## 目的

`robocode-session` 负责持久 session transcripts、session 列表、resume loading、可重建 session index。

## 不负责

- 项目 task 或 memory state；使用 `robocode-workflows`。
- 工具执行。
- 权限决策。

## 公共接口

- `SessionStore`
- `SessionPaths`
- `project_key_for_path`

## 不变量

- Transcript JSONL 是 canonical。
- SQLite index 是 derived 且可重建。
- Resume 从 transcript 顺序恢复历史。
- Workflow state 不能以此 crate 为事实源。

## `.ref` 对齐

对齐 `.ref` 的 session history 行为：append-only events 和 project-scoped resume。

## 测试

```bash
cargo test -p robocode-session
```
