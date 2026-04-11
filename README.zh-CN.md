# RoboCode

RoboCode 是一个用 Rust 实现的、本地优先的开发者 Agent CLI，目标是复刻 Claude Code 参考工程的核心本地运行模型。

英文版： [README.md](README.md)

当前仓库已经包含：

- 一个多 crate 的 Rust workspace
- 一个轻量级 REPL CLI
- 分层启动配置，支持项目级和全局配置
- 基于 JSONL transcript 和 SQLite 索引的会话持久化
- 带权限控制的统一工具运行时
- 内置本地工具：shell、文件、搜索、Web、Git，以及 worktree / stash / restore 流程
- 支持多家 API 与原生 tool-calling 的 provider 抽象

## 工作区结构

- `robocode-cli`：命令行入口和 REPL
- `robocode-config`：配置加载和优先级解析
- `robocode-core`：会话引擎和编排逻辑
- `robocode-model`：模型 provider 抽象与实现
- `robocode-tools`：内置工具与执行适配器
- `robocode-permissions`：权限模式与决策逻辑
- `robocode-session`：transcript 存储与 resume 支持
- `robocode-types`：共享领域类型

## 开发

运行测试：

```bash
cargo test --workspace
```

启动 CLI：

```bash
cargo run -p robocode-cli -- --provider fallback --model test-local
```

使用显式配置文件启动：

```bash
cargo run -p robocode-cli -- --config .robocode/config.toml
```

配置来源包括：

- 全局配置文件
- 项目级 `.robocode/config.toml`
- 环境变量
- CLI 参数

优先级为 `CLI > environment > project config > global config > defaults`。

配置示例：

```toml
provider = "openai"
model = "gpt-5.2"
permission_mode = "acceptEdits"
request_timeout_secs = 120
max_retries = 2
```

当前支持的 provider 家族：

- `anthropic`
- `openai`
- `openai-compatible`
- `ollama`
- `fallback`

当前原生 tool-calling 映射：

- Anthropic `tool_use`
- OpenAI / OpenAI-compatible `tool_calls`
- `fallback` 与 `ollama` 的文本优先本地流程

常用命令：

```text
/help
/provider
/permissions
/sessions
/resume latest
/git status
/git worktree list
/git stash list
/web search rust language --limit 3
/web fetch https://www.rust-lang.org --max-bytes 500
```

`/resume` 同时支持 `/resume #<index>` 和 `/resume <session-id-prefix>`。

当前内置工具族：

- 文件与搜索工具：`read_file`、`write_file`、`edit_file`、`glob`、`grep`
- Web 工具：`web_search`、`web_fetch`
- Git 工具：status、diff、branch、add、switch、commit、push、restore、stash、worktree
- shell 执行，带 POSIX 与 PowerShell 平台适配

## 项目文档

- `docs/architecture.md`
- `docs/architecture.zh-CN.md`
- `docs/reference-analysis.md`
- `docs/reference-analysis.zh-CN.md`
- `docs/product-requirements.md`
- `docs/product-requirements.zh-CN.md`
- `docs/staged-roadmap.md`
- `docs/staged-roadmap.zh-CN.md`
- `docs/ref-gap-matrix.md`
- `docs/ref-gap-matrix.zh-CN.md`
- `docs/superpowers/plans/2026-04-11-robocode-plan-index.md`
- `docs/superpowers/plans/2026-04-11-robocode-plan-index.zh-CN.md`
- `docs/superpowers/plans/2026-04-11-v2-session-command-enhancement.md`
- `docs/superpowers/plans/2026-04-11-v2-session-command-enhancement.zh-CN.md`

## 当前状态

这是一个正在持续演进的 V1 实现，当前重点仍然是把本地 CLI 核心打磨稳定，再逐步扩展更大的平台能力。
