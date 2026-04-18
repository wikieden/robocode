# robocode-cli

## 目的

`robocode-cli` 负责二进制入口和轻量 REPL。它把 CLI 参数、配置解析、终端输入、审批提示交给 `robocode-core`。

## 不负责

- Session 编排、provider/tool loop、workflow state。
- 权限决策；这里只渲染审批提示。
- Transcript、JSONL、SQLite 持久化。

## 公共接口

- CLI 启动参数和环境传递。
- Runtime snapshot 构造。
- REPL 渲染 `EngineEvent` 输出。

## 不变量

- 命令和 mutation 不能绕过 `robocode-core`。
- 保持 `robocode-config` 的配置优先级。
- 终端输出必须在无 rich TUI 时仍可用。

## `.ref` 对齐

行为上参考 `.ref/claude-code-main/src/main.tsx` 的启动和 REPL wiring，不复制 Bun/React/Ink 内部实现。

## 测试

```bash
cargo test -p robocode-cli
```
