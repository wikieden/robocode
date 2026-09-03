# Viden 模块索引

## Workspace 依赖图

- `apps/cli` 依赖 config、runtime、provider、tools、types，用来启动产品运行时。
- `apps/tui` 负责 terminal rendering、input orchestration、previews 和 app-specific TUI state。
- `crates/core` 是稳定 facade，向 TUI、GUI、CLI 和未来 API surface 重导出 runtime 与 contract 类型。
- `crates/runtime` 依赖 LSP、provider、permissions、session、tools、types、workflows，用来编排 turns 和 commands。
- `crates/lanes` 负责 runtime 之下的 lane 生命周期编排；它只依赖 permissions、tools、types、workflows，并通过注入的接缝接收 runtime 策略。
- `crates/agents` 负责 runtime 之下的外部 agent adapters（通用 ACP 客户端、Codex app-server、共享的进程启动基础设施）；它只依赖 permissions、plugin-api、plugin-host、tools、types，并通过注入的接缝接收 permission context、approver 和 event sink。
- `crates/plugin-api` 定义共享 plugin manifest、capability、permission 和 provider descriptor 契约。
- `crates/plugin-host` 承载共享 plugin discovery / registry 边界。
- `plugins/providers/deepseek` 是第一批使用 plugin API 的 first-party provider plugin。
- `viden-lsp` 依赖 types 和 JSON serialization，提供只读语义代码智能。
- `viden-provider`、`viden-tools`、`viden-permissions`、`viden-session`、`viden-workflows` 使用 `viden-types` 作为共享契约。
- `viden-workflows` 也使用 `viden-session` 的 project identity helper。

## 数据归属图

- Transcript/session facts：`viden-session`。
- 项目 workflow state：`viden-workflows`。
- 共享契约：`viden-types`。
- Permission policy：`viden-permissions`。
- 工具实现：`viden-tools`。
- Provider host/runtime、协议适配与动态 registry：`viden-provider`。
- Plugin manifest 与 capability 契约：`viden-plugin-api`。
- Plugin registry / lifecycle 边界：`viden-plugin-host`。
- 语义代码智能：`viden-lsp`。
- App surfaces：`apps/cli`、`apps/tui`，以及未来的 `apps/gui`。

## 当前实现状态

Mainline landed：

- V1 本地 CLI 基线已实现：REPL、config、providers、permissions、transcripts、resume、file/search/shell/web/Git tools。
- V2-A session 和 command enhancement 已实现：`/status`、`/config`、`/doctor`、更丰富的 `/sessions`、分组 `/help`。
- V2-C workflow continuity 已实现：`viden-workflows`、`/tasks`、`/task ...`、`/memory ...`、workflow JSONL logs、resume context。
- V2-B LSP foundation 已实现：`viden-lsp`、`lsp_*` tools、`/lsp ...` commands、真实 semantic queries、session reuse、document sync。
- V2-D structured terminal view 切片已实现：分组 diagnostics、分组 symbols、紧凑 references、结构化 sessions/tasks/memory、结构化 permission denials、结构化 `/git diff` 和 `/diff`，以及共享 `viden-runtime` presentation helpers。
- provider-plugin runtime 与 DeepSeek V4 已在 main 落地。mainline 使用官方 DeepSeek 模型名：默认 `deepseek-v4-flash`，可显式选择 `deepseek-v4-pro`。
- provider descriptor 矩阵已加入更多 OpenAI-compatible gateway providers：`openrouter`、`groq`、`mistral`、`together`、`kimi`、`qwen`、`dashscope-coding-plan`、`dashscope-coding-plan-anthropic`、`dashscope-tokenplan`、`dashscope-tokenplan-anthropic`、`zhipu`、`volcengine`。

当前已发布版本：

- `docs/release-0.1.29-status.zh-CN.md` 记录 RC TUI 稳定性版本：RC TUI stability
  smoke、刷新后的确定性截图、live DeepSeek development smoke、GitHub Release assets、
  Homebrew tap 和 post-publish validation。
- release validation 现在把 RC stability smoke、daily-loop、lane operator-loop、本地
  package、确定性 TUI 截图、GitHub release assets 和 Homebrew 检查作为常规门禁。

下一个计划切片：

- 当前 `0.1.30` 切片是最终 0.1.x zero-bug gate：清空已知 P0/P1 TUI bugs，补齐
  真实终端截图证据，保持所有稳定性 smokes 通过，然后再进入 0.2.x surface。
- 每个用户可见功能点都要以真实使用截图或确定性视觉产物收尾，供产品侧确认。

## 与 `.ref/claude-code-main` 的差距

已覆盖：session engine 形态、command families、permission modes、local tool registry、transcript/resume model、Git 和 web workflows。

部分覆盖：task workflow 深度、LSP runtime 深度、更丰富的 interactive TUI 行为、provider streaming/cancellation 成熟度、dynamic provider loading、更广 plugin hardening、DeepSeek Anthropic-compatible 执行路径加固、长 session summarization。

未实现：MCP、超出 provider plugins 之外的通用 skills/plugins、multi-agent/team coordinator、bridge/remote/server mode、automation/cron、voice、managed settings、analytics、feature flags。

## 模块文档

- `apps/cli/README.zh-CN.md`
- `crates/config/README.zh-CN.md`
- `crates/runtime/README.zh-CN.md`
- `crates/lanes/README.zh-CN.md`
- `crates/agents/README.zh-CN.md`
- `crates/lsp/README.zh-CN.md`
- `crates/provider/README.zh-CN.md`
- `crates/tools/README.zh-CN.md`
- `crates/permissions/README.zh-CN.md`
- `crates/session/README.zh-CN.md`
- `crates/types/README.zh-CN.md`
- `crates/workflows/README.zh-CN.md`
- `docs/provider-live-matrix.zh-CN.md`
- `docs/provider-adapter-design.zh-CN.md`
- `docs/product-design-operator-loop.zh-CN.md`
- `docs/production-coding-loop-architecture.zh-CN.md`
- `docs/spec-review-0.1.24.zh-CN.md`

完整路线图见 `PLAN.md`、`docs/product-requirements.zh-CN.md`、`docs/staged-roadmap.zh-CN.md`、`docs/ref-gap-matrix.zh-CN.md`。
