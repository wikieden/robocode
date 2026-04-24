# RoboCode 分阶段路线图

英文版： [staged-roadmap.md](staged-roadmap.md)

## 目的

这份路线图把完整的 RoboCode 产品需求翻译成可交付的阶段，而不是按当前仓库历史来倒推。

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
把本地 CLI 核心提升为真正可日常使用的开发助手。

必须具备：

- 更广的命令面
- 更好的 session 浏览和 summary
- 更强的 Git 与 diff 流程
- LSP 集成
- memory 与 task 管理
- 更丰富的 TUI 和交互

退出标准：

- 用户可以在不频繁回退到 ad hoc shell 的情况下完成更多开发流程
- 具备超越 grep / file editing 的语义级代码辅助
- session 和 task 的连续性从“能用”提升到“有意设计”

### V3：平台扩展层

目标：
把 RoboCode 从本地 agent CLI 扩展为一个可扩展开发平台。

必须具备：

- MCP 集成
- skills 与 plugins
- 多 Agent 协调
- bridge 与 remote session 支持
- automation 和 cron 风格工作流

退出标准：

- 外部工具生态可以通过稳定接口接入 RoboCode
- remote 与集成客户端能复用与本地 session 相同的执行和权限模型
- 多 Agent 工作流不会绕开 transcript 和权限保证

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
- V2 应优先增强本地开发效率，而不是过早平台扩张
- V3 必须复用 V1 / V2 的执行不变量，而不是引入新的 side-channel runtime
- 远期平台能力必须服从核心工作流成熟度

## 当前仓库映射

Mainline landed：

- REPL 和命令循环
- config resolution
- provider abstraction
- permissions
- transcripts 与 resume
- Git 和 Web 工具

Current dev baseline：

- 前序 V2-C 分支已经补上 task、memory、resume-context 等 workflow continuity
- 当前 V2-B 分支已经补上基于 LSP 的 semantic assistance，包括 real queries、session reuse、document synchronization
- V2-D 分支已经存在，但目前仍是 structured terminal views 的 planning-only branch

这并不改变路线图顺序。它说明 RoboCode 已不再只是早期 V1 状态，但后续阶段仍应按顺序推进，而不是因为分支存在就提前拉动。
