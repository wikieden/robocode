# RoboCode 与 `.ref` 差距矩阵

英文版： [ref-gap-matrix.md](ref-gap-matrix.md)

这份矩阵把 `.ref/claude-code-main`、当前 RoboCode 仓库，以及目标状态放在一起对比。

| 子系统 | `.ref` 能力摘要 | RoboCode 当前状态 | 目标状态 | 差距 | 阶段 | 备注 |
|---|---|---|---|---|---|---|
| 核心会话引擎 | 共享 query loop，支持 tool-call continuation 和 transcript 驱动运行时 | 已实现共享 engine 和统一工具循环 | 在所有主要运行时路径上对齐参考行为 | 低 | V1 | 高相似度对标 |
| 配置系统 | 启动编排和复杂环境/bootstrap 逻辑 | 已实现确定性配置合并；managed settings 缺失 | 稳定的本地/全局配置，并为 managed settings 预留空间 | 中 | V1 / 远期 | bootstrap 内部允许 Rust 化简化 |
| Provider 系统 | 以 Anthropic 为中心，产品集成很深 | 已有多 provider 抽象和原生 tool-calling | 更成熟的多 provider 层，带 streaming 和更强兼容性 | 中 | V1 / V2 | 保持 vendor-agnostic core |
| 工具运行时 | 大型工具 registry，统一权限化执行 | 已有本地工具的统一 registry | 保持统一运行时并继续扩展工具族 | 中 | V1 / V2 / V3 | 高相似度对标 |
| 权限系统 | 一等模式、规则、提示和边界处理 | 已有核心模式和规则；策略深度还较轻 | 覆盖本地、remote、集成流程的成熟规则系统 | 中 | V1 / V2 / V3 | 高相似度对标 |
| Session 存储与恢复 | JSONL 事实源，带 resume 和 metadata | 已实现 JSONL + SQLite；浏览深度还基础 | 更强的 summaries、selectors 和管理能力 | 中 | V1 / V2 | 高相似度对标 |
| Slash commands | 覆盖 runtime、config、auth、tasks、integrations、UI 的大命令面 | 已有 runtime、sessions、git、web 核心命令族 | 扩展到 config、diagnostics、integrations、workflows 的更完整命令面 | 高 | V1 / V2 / V3 | 不必逐字复制每个命令 |
| 文件与搜索工具 | read、write、edit、glob、grep | 已实现 | 保持并继续加固 | 低 | V1 | 在工具家族层面已达标 |
| Git 工作流 | commit 导向命令和更广 workflow helpers | 已有 status、diff、switch、add、commit、push、restore、stash、worktree | 更深的 review 与 workflow 支持 | 中 | V1 / V2 | 核心流程高相似度 |
| Web 工具 | search 和 fetch 原生进入工具系统 | 已实现 | 继续增强质量和来源处理 | 低 | V1 / V2 | 在工具家族层面已达标 |
| MCP | server 管理和 MCP-backed tool invocation | 未开始 | 完整 MCP lifecycle、discovery、invocation 和管理命令面 | 高 | V3 | 高相似度对标 |
| LSP | 语言服务器集成与推荐逻辑 | V2-B active / partial implementation：已有 real semantic queries、session reuse、document sync、normalized output | 与本地工作流整合的语义级代码智能 | 中 | V2 | 已在 `codex/v2-lsp-foundation` 实现；仍比参考平台更轻 |
| Skills | 可复用 workflow system | 未开始 | 本地 skill discovery 与执行模型 | 高 | V3 | 行为相似，Rust 原生实现 |
| Plugins | 内置和第三方插件加载 | 未开始 | 带清晰信任边界的 plugin loading 和管理 | 高 | V3 | 行为相似，Rust 原生实现 |
| 多 Agent / Teams | Agent tool、coordinator、team workflows、inter-agent messaging | 未开始 | 在共享运行时保证下的协调式委派工作流 | 高 | V3 | 高相似度对标 |
| Bridge / Remote | IDE bridge、remote session manager、server-oriented flows | 未开始 | 可复用的 remote / bridge 层，具备 permission callbacks | 高 | V3 | 高相似度对标 |
| Memory | persistent memory 支持 | V2-C active / partial implementation：已有 project/session memory、suggestion confirmation、event logs | 与长期工作流绑定的显式 memory 模型 | 中 | V2 | 已在 `codex/v2-memory-task-workflows` 实现；仍比参考平台更轻 |
| Tasks | task 创建与管理 | V2-C active / partial implementation：已有 lifecycle reducer、blockers、archive/restore、resume context | 融入 session、后续扩展到 agents 的 task lifecycle | 中 | V2 | 已在 `codex/v2-memory-task-workflows` 实现；agent integration 仍是未来工作 |
| Automation / Cron | 定时与 durable automation 流程 | 未开始 | session 和 durable automation 支持 | 高 | V3 | 应放在核心工作流成熟之后 |
| Voice | 语音输入与状态管理 | 未开始 | voice-assisted workflow layer | 高 | 远期 | 参考工程有，但优先级较低 |
| TUI / Screens | 丰富 Ink UI、screens、structured diff、专项视图 | 当前只有极简 REPL 和文本帮助 | 更丰富的 diff、sessions、permissions、integrations TUI | 高 | V2 | UX 意图对齐，不要求同框架 |
| Analytics / feature flags / managed settings | 产品运营、策略、遥测、受管配置 | 当前按设计未实现 | 只在核心产品成熟后按需引入 | 高 | 远期 | 早期不应优先 |

## 总结

RoboCode 已经覆盖了参考工程最重要的架构主梁：

- shared session engine
- shared tool runtime
- permissions
- transcripts 与 resume
- provider abstraction
- 高价值本地开发工具

当前最大的缺口主要不是“本地 CLI 核心”，而是平台级子系统和成熟度缺口：

- MCP
- 更成熟的 LSP 平台层
- skills 与 plugins
- 多 Agent 协调
- bridge 与 remote
- memory、tasks、automation
- 更丰富的终端 UI

明确延后的是参考工程里的产品化运营系统：

- analytics
- feature flags
- managed settings
- 以及其他不能在早期直接提升开发者核心工作流的基础设施
