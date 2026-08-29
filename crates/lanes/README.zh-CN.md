# viden-lanes

## 目的

`viden-lanes` 负责 lane 编排：agent lane 的持久生命周期，以及 lane 命令允许产生
的全部本地副作用。它包含 lane 状态机（`LaneSupervisor`）、把命令串行化到单个
lane 的 per-lane worker 线程，以及执行 worktree、process、terminal 和 patch 副作用
的 effect executor。

它位于 `viden-runtime` 之下，不知道 session、provider、ACP 或任何前端。

## 不负责

- Session、provider 或 agent adapter 状态。
- Permission 决策序列本身；由 runtime 注入。
- 事件脱敏策略；由 runtime 注入。
- Lane 持久化内部实现；由 runtime 提供 `LanePersistence`。
- 直接访问操作系统。所有副作用都经过 `viden-tools` backends。

## 公共接口

- `LaneSupervisor`：lane 状态机和命令入口。
- `LanePersistence` 和 `WorkflowLanePersistence`：lane 事实存储接缝。
- `LaneEffectExecutor`、`LaneEffectRequest`、`LaneEffectResult` 和
  `LocalLaneEffectExecutor`：lane 副作用接缝及其本地实现。
- `LaneEventSink`：lane 事件的发布出口。
- `LaneApprovalResolver`：排队 approval 如何经由 runtime 的共享 permission gate
  重新校验。
- `LaneCommandRedactor`：已公告命令允许暴露的内容。
- `workspace_eligibility` 和 `resolve_lane_output_log`。

## 不变量

- Permission checks 先于 effects。lane worker 在调用 effect executor 之前，先用
  lane 作用域的 `PermissionEngine` 解析每次 mutation；排队 approval 会通过注入的
  resolver 重新校验，因此 plan-mode 复检仍可否决操作者的 "allow"。
- 只有在 scope、过期时间和 permission epoch 同时成立时，排队 approval 才被接受。
- Runtime 拥有的策略只能注入，不能导入。这保持依赖边单向，并由
  `scripts/check-dependency-boundaries.sh` 强制。
- 一个 worker 线程拥有一个 lane，因此同一 lane 上的命令是串行的。

## 测试

```bash
cargo test -p viden-lanes
```

Lane 行为另有 runtime 套件的端到端覆盖，它通过 `RuntimeSupervisor` 驱动 lanes：

```bash
cargo test -p viden-runtime
```
