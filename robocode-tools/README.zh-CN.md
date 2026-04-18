# robocode-tools

## 目的

`robocode-tools` 负责内置本地工具和执行适配器。

## 不负责

- 权限决策。
- 模型规划。
- Transcript 或 workflow state。

## 公共接口

- `BuiltinTool`
- `ToolRegistry`
- shell、files、glob、grep、web、Git 内置工具。

## 不变量

- Mutating tools 必须在 `ToolSpec` 中标记为 mutating。
- 输出必须变成可序列化的 `ToolResult`。
- Shell 保持平台适配：Unix 用 POSIX，Windows 用 PowerShell。

## `.ref` 对齐

用 Rust traits 和本地 adapters 对齐 `.ref` 的 `Tool.ts` 和 tool registry 行为。

## 测试

```bash
cargo test -p robocode-tools
```
