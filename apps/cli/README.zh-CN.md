# viden-cli

## 目的

`viden-cli` 负责终端入口包和轻量 REPL。安装后的用户命令是 `viden`。它把 CLI 参数、配置解析、终端输入、审批提示交给 `viden-runtime`。

## 不负责

- Session 编排、provider/tool loop、workflow state。
- 权限决策；这里只渲染审批提示。
- Transcript、JSONL、SQLite 持久化。

## 公共接口

- CLI 启动参数和环境传递。
- `--provider-plugin-dir <dir>`，用于可重复配置动态 provider plugin 发现目录。
- Provider plugin 加载失败时的结构化启动诊断。
- 把 provider runtime state 传给 REPL，使 `/provider list` 和
  `/provider reload` 可以检查并刷新 plugin descriptors，且 `/provider use <id>
  [model]` 可以通过同一个 registry 切换 provider。
- Runtime snapshot 构造。
- REPL 渲染 `EngineEvent` 输出。

## 不变量

- 命令和 mutation 不能绕过 `viden-runtime`。
- 保持 `viden-config` 的配置优先级。
- 启动时必须根据解析后的配置构造 `ProviderHost`，包括显式 provider plugin 目录。
- 必须把启动时的 provider host 和 plugin 目录传给 `viden-runtime`；CLI 不直接实现
  provider reload 行为。
- 必须把 provider request defaults 传给 `viden-runtime`，让 provider switching
  使用与启动阶段一致的 timeout、retry、API base 和 API key 默认值。
- Provider plugin loader 错误必须渲染 kind、path、message 和 detail，不能退化成
  不透明字符串。
- 终端输出必须在无 rich TUI 时仍可用。

## `.ref` 对齐

行为上参考 `.ref/claude-code-main/src/main.tsx` 的启动和 REPL wiring，不复制 Bun/React/Ink 内部实现。

## 测试

```bash
cargo test -p viden-cli
```
