# RoboCode 模块索引

## Workspace 依赖图

- `robocode-cli` 依赖 config、core、model、tools、types，用来创建终端运行时。
- `robocode-core` 依赖 LSP、model、permissions、session、tools、types、workflows，用来编排 turns 和 commands。
- `robocode-lsp` 依赖 types 和 JSON serialization，提供只读语义代码智能。
- `robocode-model`、`robocode-tools`、`robocode-permissions`、`robocode-session`、`robocode-workflows` 使用 `robocode-types` 作为共享契约。
- `robocode-workflows` 也使用 `robocode-session` 的 project identity helper。

## 数据归属图

- Transcript/session facts：`robocode-session`。
- 项目 workflow state：`robocode-workflows`。
- 共享契约：`robocode-types`。
- Permission policy：`robocode-permissions`。
- 工具实现：`robocode-tools`。
- Provider host/runtime、协议适配与动态 registry：`robocode-model`。
- 语义代码智能：`robocode-lsp`。
- CLI 展示：`robocode-cli`。

## 当前实现状态

Mainline landed：

- V1 本地 CLI 基线已实现：REPL、config、providers、permissions、transcripts、resume、file/search/shell/web/Git tools。
- V2-A session 和 command enhancement 已实现：`/status`、`/config`、`/doctor`、更丰富的 `/sessions`、分组 `/help`。
- V2-C workflow continuity 已实现：`robocode-workflows`、`/tasks`、`/task ...`、`/memory ...`、workflow JSONL logs、resume context。
- V2-B LSP foundation 已实现：`robocode-lsp`、`lsp_*` tools、`/lsp ...` commands、真实 semantic queries、session reuse、document sync。
- V2-D structured terminal view 切片已实现：分组 diagnostics、分组 symbols、紧凑 references、结构化 sessions/tasks/memory、结构化 permission denials、结构化 `/git diff` 和 `/diff`，以及共享 `robocode-core` presentation helpers。
- provider-plugin runtime 与 DeepSeek V4 已在 main 落地。mainline 使用官方 DeepSeek 模型名：默认 `deepseek-v4-flash`，可显式选择 `deepseek-v4-pro`。
- provider descriptor 矩阵已加入更多 OpenAI-compatible gateway providers：`openrouter`、`groq`、`mistral`、`together`、`kimi`、`qwen`、`zhipu`、`volcengine`。

Next planned slice：

- 以 `docs/release-0.1.9-plan.zh-CN.md` 作为当前 validation-hardening 切片。
- 强化 release gate、TUI regression snapshots、截图确认产物、`AgentTask`/lane fixtures、
  permission regressions、CI gates 和 provider compatibility coverage。
- 每个用户可见功能点都要以真实使用截图或确定性视觉产物收尾，供产品侧确认。

## 与 `.ref/claude-code-main` 的差距

已覆盖：session engine 形态、command families、permission modes、local tool registry、transcript/resume model、Git 和 web workflows。

部分覆盖：task workflow 深度、LSP runtime 深度、更丰富的 interactive TUI 行为、provider streaming/cancellation 成熟度、dynamic provider loading、更广 plugin hardening、DeepSeek Anthropic-compatible 执行路径加固、长 session summarization。

未实现：MCP、超出 provider plugins 之外的通用 skills/plugins、multi-agent/team coordinator、bridge/remote/server mode、automation/cron、voice、managed settings、analytics、feature flags。

## 模块文档

- `robocode-cli/README.zh-CN.md`
- `robocode-config/README.zh-CN.md`
- `robocode-core/README.zh-CN.md`
- `robocode-lsp/README.zh-CN.md`
- `robocode-model/README.zh-CN.md`
- `robocode-tools/README.zh-CN.md`
- `robocode-permissions/README.zh-CN.md`
- `robocode-session/README.zh-CN.md`
- `robocode-types/README.zh-CN.md`
- `robocode-workflows/README.zh-CN.md`
- `docs/provider-live-matrix.zh-CN.md`

完整路线图见 `PLAN.md`、`docs/product-requirements.zh-CN.md`、`docs/staged-roadmap.zh-CN.md`、`docs/ref-gap-matrix.zh-CN.md`。
