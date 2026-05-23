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
- 项目级 workflow 状态：tasks、session memory、project memory suggestions、resume context
- 支持多家 API、原生 tool-calling，以及 provider-plugin runtime 的 provider 抽象；DeepSeek 已作为首个独立 provider family 落地

## 工作区结构

- `robocode-cli`：命令行入口和 REPL
- `robocode-config`：配置加载和优先级解析
- `robocode-core`：会话引擎和编排逻辑
- `robocode-model`：provider host/runtime、协议适配器与模型实现
- `robocode-tools`：内置工具与执行适配器
- `robocode-permissions`：权限模式与决策逻辑
- `robocode-session`：transcript 存储与 resume 支持
- `robocode-types`：共享领域类型
- `robocode-workflows`：项目级 tasks、memory、resume context 与 workflow event storage

## 开发

运行测试：

```bash
cargo test --workspace
```

启动 CLI：

```bash
cargo run -p robocode-cli -- --provider fallback --model test-local
```

启动轻量 terminal UI：

```bash
cargo run -p robocode-cli -- --tui --provider fallback --model test-local
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

provider-scoped config 可以覆盖通用 API 字段：

```toml
provider = "deepseek"
model = "deepseek-v4-flash"
api_base = "https://generic.example"

[providers.deepseek]
api_base = "https://api.deepseek.com"
api_key_env = "DEEPSEEK_API_KEY"
```

provider-scoped tables 不限于一方内建 provider。任意已注册 provider id 都可以拥有自己的配置表：

```toml
provider = "openrouter"

[providers.openrouter]
api_base = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
default_model = "openai/gpt-5.2"
```

DeepSeek V4 模型名以官方 API 文档为准：

- `deepseek-v4-flash` 是 RoboCode 的默认 DeepSeek 模型
- `deepseek-v4-pro` 可显式配置，用于更强的 V4 模型
- 旧的 `deepseek-chat` 与 `deepseek-reasoner` 只作为兼容模型名保留，并已进入 DeepSeek 侧弃用计划

DeepSeek V4 兼容性是 provider-specific 行为，不按通用
OpenAI-compatible provider 处理：

- `deepseek` 会在 tool-call 轮次中保留并回放 `reasoning_content`，且不会把它泄漏进 tool arguments
- assistant tool-call message 会使用非空 `content`，匹配 DeepSeek V4 thinking mode 要求
- built-in `deepseek` descriptor 不声明 `tool_choice` 支持
- reasoning effort 映射到 DeepSeek 支持的 `high` 与 `max`
- 当客户端或工作流更适合 Anthropic-compatible 协议时，仍可使用 `deepseek-anthropic`

DeepSeek 的 API 字段优先级：

- CLI `--api-key` / `--api-base`
- `[providers.deepseek]` 配置
- `DEEPSEEK_API_KEY` / `DEEPSEEK_API_BASE`
- 通用 `api_key` / `api_base`

当前支持的 provider 家族：

- `anthropic`
- `openai`
- `openai-compatible`
- `deepseek`，作为独立 provider family，复用 OpenAI-style 协议
- `deepseek-anthropic`，用于 DeepSeek 的 Anthropic-compatible endpoint：`https://api.deepseek.com/anthropic`
- OpenAI-compatible gateway descriptors：`openrouter`、`groq`、`mistral`、`together`、`kimi`、`qwen`、`zhipu`、`volcengine`
- `ollama`
- `fallback`

当前协议族与 tool-calling 映射：

- Anthropic `tool_use`
- OpenAI-style `tool_calls`，用于 OpenAI-compatible providers，包括 DeepSeek
- DeepSeek Anthropic-compatible `tool_use`，通过 `deepseek-anthropic`
- `fallback` 与 `ollama` 的文本优先本地流程

当前 provider runtime 状态与方向：

- 继续支持 built-in providers
- provider descriptors 已经通过 provider host/runtime registry 统一流转
- `/provider list` 会展示每个已注册 provider 的协议族、默认模型、streaming 支持、tool-call 支持和紧凑 compatibility flags
- `/provider doctor [id]` 会展示 registry 诊断，也可以按 provider id 聚焦单个 provider，并包含 provider-specific compatibility 要求
- provider 绑定是 session/agent scoped，而不是 process-global
- permission prompts 会把 tool input 渲染为稳定字段，便于审批前检查
- DeepSeek V4 兼容标记已经进入 provider descriptors，built-ins 与 plugins 都可以显式声明协议差异
- 剩余 hardening 聚焦扩展 provider 矩阵的真实 API 兼容性覆盖
- 执行模型以 native dynamic loading 优先，后续再考虑 WASM 迁移

常用命令：

```text
/help
/provider
/provider doctor
/provider doctor openrouter
/help
/status
/config
/doctor
/permissions
/sessions
/resume latest
/git status
/git worktree list
/git stash list
/web search rust language --limit 3
/web fetch https://www.rust-lang.org --max-bytes 500
/task add Build workflow commands
/tasks
/task resume-context
/memory suggest Keep project memory explicit
/memory confirm mem_<id>
/memory export
```

真实 provider smoke tests 默认 ignored。要验证某个 provider 的真实 API 路径：

```bash
ROBOCODE_LIVE_PROVIDER=openrouter \
ROBOCODE_LIVE_MODEL=openai/gpt-5.2 \
ROBOCODE_LIVE_API_KEY="$OPENROUTER_API_KEY" \
cargo test -p robocode-cli selected_live_provider_generates_python_hello_world_from_natural_language -- --ignored
```

默认 CLI smoke suite 保持离线可重复，并覆盖 fallback provider 下写入、读取、运行生成 Python 文件的 file-tool workflow。

`/resume` 同时支持 `/resume #<index>` 和 `/resume <session-id-prefix>`。

当前内置工具族：

- 文件与搜索工具：`read_file`、`write_file`、`edit_file`、`glob`、`grep`
- Web 工具：`web_search`、`web_fetch`
- Git 工具：status、diff、branch、add、switch、commit、push、restore、stash、worktree
- Workflow 命令：项目级 tasks、task lifecycle、session memory、project memory suggestions、resume context
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

这是一个正在持续演进的本地优先 CLI 平台。mainline 已经包含 V1、核心 V2 session/workflow/LSP 切片、provider-plugin runtime，以及 DeepSeek v4 作为首个独立 provider 目标。下一步 provider 平台工作是加固 dynamic loading、registry refresh、streaming、cancellation 与更广的 plugin 兼容性。
