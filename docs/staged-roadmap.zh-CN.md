# RoboCode 分阶段路线图

英文版： [staged-roadmap.md](staged-roadmap.md)

## 目的

这份路线图把完整的 RoboCode 产品需求翻译成可交付的阶段，而不是按当前仓库历史来倒推。

更长期的产品战略见 [RoboCode 长期路线图](long-term-roadmap.zh-CN.md)。这份阶段路线图是
交付地图；长期路线图是产品和市场地图。

## 长期定位

RoboCode 的长期定位不是单一 TUI，也不是又一个 coding agent CLI，而是：

> 开箱即用的多 Agent 编排运行时 + 极致 token 效能优化层。

TUI 是第一阶段的主产品形态，因为它最适合承载高密度状态、审批、子 agent lane、
测试、诊断和多屏监督。只有当 TUI cockpit 和核心 runtime 足够稳定后，才逐步扩展到
CLI automation、API server、desktop、Web、IDE/ACP adapter 等其他入口。

长期产品支柱：

- 多 Agent 编排：内置 planner、coder、reviewer、tester、researcher、doc writer
  等角色，并支持 Codex、Claude Code、DeepSeek、shell、MCP tools 和未来 ACP agents。
- Token 效能引擎：按任务动态构造 context bundle，自动压缩 transcript、裁剪长日志、
  去重 tool results、控制每个 agent 的 token budget 和成本上限。
- 共享事实层：所有 agent 读写统一的 facts、events、artifacts、diff、diagnostics、
  test results 和 user constraints，而不是互相转发整段聊天记录。
- 多前端形态：TUI 优先；CLI、API、IDE、Web 和 desktop 复用同一套 orchestration
  runtime，而不是各自实现一套 agent 逻辑。

## 阶段定义

### V1：本地核心 CLI

目标：
交付一个可靠的、本地优先的开发者 Agent CLI，具备 durable session、权限系统和高价值本地工具。

必须具备：

- 交互式 REPL
- 启动配置模型
- provider 抽象
- 文件、搜索、shell、web、Git 工具族
- permission modes 与 approvals
- append-only transcript 与 resume
- 基础 slash commands

退出标准：

- 用户可以端到端完成本地读代码和改代码流程
- 工具调用、审批和 transcript 历史都可审计
- 切换 provider 不需要改 core engine
- 会话可以按项目稳定恢复

### V2：开发者增强层

目标：
把本地 CLI 核心提升为真正可日常使用的 TUI cockpit 和开发助手。

必须具备：

- 更广的命令面
- 更好的 session 浏览和 summary
- 更强的 Git 与 diff 流程
- 支持 dynamic provider loading 的 plugin-extensible provider runtime
- LSP 集成
- memory 与 task 管理
- 更丰富的 TUI 和交互
- 主屏实时工作状态与统一 `AgentTask` 视图

退出标准：

- 用户可以在不频繁回退到 ad hoc shell 的情况下完成更多开发流程
- provider 的增长不再需要反复修改 core-engine
- 具备超越 grep / file editing 的语义级代码辅助
- session 和 task 的连续性从“能用”提升到“有意设计”
- 用户始终能从 TUI 中判断当前 agent 正在做什么、证据来自哪里、下一步可以如何操作

### V3：Agent 编排与 Token 效能层

目标：
把 RoboCode 从单 agent 开发助手升级为多 Agent 编排系统，并把 token 使用效率作为一等产品能力。

必须具备：

- 统一 `AgentTask`、`AgentLane`、`Artifact`、`Evidence` 和 `ContextBundle` 模型
- planner -> worker -> reviewer -> tester 的默认工作流模板
- 外部 terminal coding tools 的受监督 lane runtime，例如 Codex、Claude Code、DeepSeek-TUI 和 shell job
- context bundle builder、semantic file selection、diff-aware context、tool output compaction
- token budget、model routing、cost dashboard 和 context pressure 可视化
- TUI 副屏用于真实 agent lanes、tests、diagnostics 和 next actions，而不是装饰性面板

退出标准：

- 用户可以开箱即用地运行多 Agent 编排流程
- 多个 agent 共享结构化事实和 artifacts，而不是复制完整对话
- 每个 agent 的 token 消耗、上下文来源、输出证据和下一步动作都可见
- TUI 已能稳定承载编排过程，其他入口仍可暂缓

### V4：生态与平台扩展层

目标：
把稳定的多 Agent runtime 扩展为可插拔开发平台，同时保持 TUI 作为主操作面。

必须具备：

- MCP 集成
- skills 与 plugins
- 多 Agent 协调
- ACP / external agent adapter
- bridge 与 remote session 支持
- automation 和 cron 风格工作流

退出标准：

- 外部工具生态可以通过稳定接口接入 RoboCode
- remote 与集成客户端能复用与本地 session 相同的执行和权限模型
- 多 Agent 工作流不会绕开 transcript 和权限保证
- plugin、skill、MCP 和 ACP 都通过统一权限、事实层、token budget 和 evidence 约束

### 远期平台能力

目标：
在核心工作流稳定后，加入更偏产品规模化的高级能力。

目标能力：

- voice interaction
- multi-device handoff
- analytics 与 managed settings
- feature-flag infrastructure
- 仍然有价值时再引入参考工程中特定运营能力

退出标准：

- 更重的产品化能力不能破坏核心本地开发工作流

## 优先级规则

- V1 行为是后续所有阶段的基线契约
- V2 应优先把 TUI cockpit 的真实状态、输入体验和编程闭环打稳，而不是过早平台扩张
- V3 应优先交付开箱即用的多 Agent 编排和 token 效能，而不是只增加更多面板
- V4 必须复用 V1 / V2 / V3 的执行不变量，而不是引入新的 side-channel runtime
- TUI 是长期主界面；其他形态必须复用同一 runtime，并在 TUI 主线稳定后再扩展
- 远期平台能力必须服从核心工作流成熟度

## 当前仓库映射

Mainline landed：

- V1 本地 CLI 核心：REPL、config resolution、provider abstraction、permissions、transcripts/resume、Git tools、web tools
- V2-A session commands：`/status`、`/config`、`/doctor`、更丰富的 `/sessions`、分组 `/help`
- V2-C workflow continuity：project tasks、project/session memory、workflow JSONL logs、resume context
- V2-B LSP foundation：real semantic queries、session reuse、document synchronization、`lsp_*` tools、`/lsp ...` commands
- V2-D structured terminal views：分组 diagnostics、分组 symbols、紧凑 references、sessions、tasks、memory、diff、permission denials，以及共享 presentation helpers
- Provider 平台切片：provider host/runtime registry、provider-scoped config，以及 DeepSeek v4 作为首个独立 provider 目标
- Provider hardening 检查点：descriptor validation、registry refresh coverage、blank-key handling、provider-scoped diagnostics，以及 offline/live smoke harnesses
- DeepSeek V4 兼容标记：reasoning-content replay、非空 assistant tool-call content、显式 `tool_choice` capability，以及 `high`/`max` reasoning-effort metadata

当前已发布版本：

- `docs/release-0.1.22-status.zh-CN.md` 记录 provider detail 可用性补丁：API key
  脱敏显示、provider detail 动作行简化、确定性截图证据、GitHub Release、
  Homebrew tap 和 post-publish smoke。
- `0.1.22` 保持 0.1.21 的交互系统不变，把 provider detail 进一步收敛成
  settings-form 风格的配置界面。
- `docs/release-0.1.21-status.zh-CN.md` 记录已打 tag、已发布、已更新 Homebrew、已完成
  post-publish verification 的 Usability Beta Gate release。
- `0.1.21` 增加可操作 setup wizard、缺 key 首次启动入口、provider failure recovery
  分类、居中的 lane action selector，以及刷新后的 0.1.21 截图证据。
- `docs/release-0.1.23-status.zh-CN.md` 记录 provider/model 设置补丁：opencode 风格的供应商
  连接、Favorites 优先的 model 选择、provider auth-mode 元数据、确定性截图、GitHub
  Release、Homebrew tap 和 post-publish smoke。
- `0.1.23` 把供应商/model 选择进一步靠近 opencode 模式：`/connect` 是供应商
  连接 picker，`/provider` 保留为命令式 provider 操作入口，`/models` 显示 Favorites、
  Recent 和按供应商分组的 active model 行。Favorites 是 provider/model 组合，不会
  在后续 provider 分组重复出现，也可以在 selector 里用 `Ctrl-F` 置顶。`/connect
  <provider>` 现在会进入真实的 provider scoped 配置动作：key env、endpoint、默认模型，
  以及 `/models` 使用的 active/favorite model 列表。
- `0.1.18` 仍是 Interaction Hardening 检查点：settings 决策是 selector-first，
  后续交互决策面必须是可执行 picker，而不是被动信息页。

下一个计划版本：

- 继续推进 provider 配置流程，下一步是更完整的聚焦编辑表单，包括连接测试、保存/取消语义
  和更清晰的字段焦点。当前 provider scoped 的 key env、endpoint、默认模型、active
  model 列表、favorite model 列表和 auth-mode 元数据已经具备真实运行时/配置接线。
- 每个用户可见功能点完成前，都必须提供一张真实使用截图或确定性视觉产物

这并不改变路线图顺序。它说明 RoboCode 已不再只是早期 V1 状态，但后续阶段仍应按顺序推进，而不是因为分支存在就提前拉动。
