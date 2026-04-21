# robocode-permissions

## 目的

`robocode-permissions` 负责 permission modes、路径作用域检查、allow/ask/deny 决策。

## 不负责

- Prompt 渲染。
- 工具执行。
- Transcript 或 workflow 存储。

## 公共接口

- `PermissionEngine`
- `PermissionContext`
- `PermissionDecision`

## 不变量

- `plan` 模式拒绝 mutation。
- 作用域内安全读取可自动允许。
- 越界路径默认拒绝，除非明确特殊处理。
- Workflow writes 在 core 中按 mutating actions 处理。

## `.ref` 对齐

基于 `.ref/src/types/permissions.ts`：modes、scoped rules、approval outcomes。

## 测试

```bash
cargo test -p robocode-permissions
```
