# Viden Agent 指南

英文版：[AGENTS.md](AGENTS.md)

## 如何使用本指南

这是面向编码 agent 和自动化的仓库级规范文件。诸如根目录 `CLAUDE.md` 这类
工具专属入口文件必须指向这里，而不是维护一份互相竞争的项目规范副本。

指令优先级为：

1. 用户当前的请求；
2. 仓库根目录的这份 `AGENTS.md`；
3. 被修改文件最近一层的嵌套 `AGENTS.md`；
4. 下方链接的长期架构、设计和开发文档。

动手编辑前先读完所有适用的指令文件。当任务跨越 Core、TUI、GUI 或设计包时，
先拆分归属或显式协调各层嵌套规则，再开始写。绝不要假设历史分支描述或聊天
摘要仍然有效；请核对 Git 和被引用的真源文件。

关键来源：

- roadmap 与发布排期：`PLAN.md` 和 `docs/staged-roadmap.md`；
- V3 分支拓扑：`docs/parallel-development-plan.md`；
- 架构与模块边界：`docs/architecture.md` 和
  `docs/modules.md`；
- 编码、文档与注释标准：
  `docs/development-standards.md`；
- 前端 contract：`docs/frontend-integration-contract.md`；
- 视觉接入规则：`docs/viden-design-adoption.md` 以及
  `docs/viden-design/Viden/` 下的嵌套指令。

## 使命

Viden 是一个 Rust 优先、本地优先的 agentic 开发者工作台，灵感来自
`.ref/claude-code-main`。把参考项目当作行为指南，而不是逐文件移植。在有价值的
地方保留面向用户的 runtime 模式，但实现要保持 Rust 原生，并在额外平台机制尚不
需要时比参考项目更简单。

## 当前架构

Workspace 代码按产品界面和可复用 core 划分：

- `apps/cli`：二进制入口、flags 和 bootstrap。
- `apps/tui`：终端渲染、输入编排、previews 和 app 专属 TUI 状态。
- `apps/gui`：Tauri 桌面客户端，由其嵌套 `AGENTS.md` 管辖。
  `0.1.0-alpha.1` 框架门禁选择了 Tauri；详见
  `docs/gui-framework-decision.md`。
- `crates/core`：稳定 runtime facade 和共享 contract 再导出。
- `crates/context`：原生 context 选择、不可变内容引用、
  retrieval、compaction、quality 和 cost 核算。
- `crates/runtime`：会话引擎、slash commands、provider/tool loop、workflow 命令路由。
- `crates/lanes`：lane 生命周期编排与 lane 本地副作用，
  位于 runtime 之下；permission gate 和事件脱敏这类 runtime 策略只注入给它，
  它不会反向导入。
- `crates/agents`：位于 runtime 之下的外部 agent adapter 层——通用 ACP
  客户端、Codex app-server 客户端，以及两者共享的进程启动基础设施；
  permission context、approver 和 event sink 只注入给它，
  它不会反向导入。
- `crates/provider`：provider 抽象、registry 和协议适配。
- `crates/plugin-api`：共享 plugin manifest、capability、permission 和 provider descriptor 契约。
- `crates/plugin-host`：provider/tool/agent/workflow plugins 的静态 plugin registry 边界。
- `crates/tools`：本地 shell、文件、搜索、web 和 Git 工具实现。
- `crates/permissions`：permission modes、路径 scope 检查，以及 allow/ask/deny 决策。
- `crates/session`：JSONL transcript 存储和可重建的 SQLite session 索引。
- `crates/types`：message、tool、permission、session、runtime snapshot、task 和 memory 的共享领域类型。
- `crates/config`：分层 config 解析。
- `crates/workflows`：项目 task、project/session memory、resume context 和 workflow event 存储。
- `crates/lsp`：只读语义 diagnostics、symbols、references 和
  文档同步。
- `plugins/providers/deepseek`：DeepSeek provider plugin。

## 不可协商的 Invariants

- 所有模型 tool call 和本地命令副作用都必须走共享 runtime 路径。
- Permission checks 发生在 mutation 之前，而不是之后。
- Transcript 历史对 session facts 保持可审计和 append-only。
- 持久日志以 JSONL 为准；SQLite 是派生的、可重建的索引。
- Session 状态和 workflow 状态相关但彼此独立：
  - `viden-session` 记录一个 session 里发生了什么。
  - `viden-workflows` 记录长期的项目 task 和 memory 状态。
- Assistant 建议的 project memory 必须经过显式确认才能成为 active。
- Plan mode 必须阻断 workflow、文件、shell、Git 以及 memory/task 的变更。
- Core 是 runtime 事实和副作用的唯一权威。前端可以拥有展示状态，
  但不得创建平行的业务 reducers。
- 前端必须通过版本化的 snapshot/replay contract 恢复缺失或乱序的状态，
  绝不能靠显示文本猜测。

## 标准变更流程

编辑前：

1. 读适用的根级和嵌套 `AGENTS.md` 文件。
2. 检查 `git status`、活动 worktrees 和真实分支基线。当分支新鲜度影响任务时，
   先 fetch 远端。
3. 确认归属的产品线和写入 scope。若同一批文件已由另一个活动任务拥有，
   在没有串行化决策前不要开始。
4. 在引入新抽象或新界面前，先定位当前的 contract、设计、测试和文档来源。

编辑时：

1. 保持变更聚焦且可回退。
2. 行为变更使用 TDD，并确认最初的失败是相关的。
3. 保持 runtime、permission、持久化和前端依赖边界。
4. 在同一个变更集中更新受影响的中英文文档和简洁的 invariant 注释。
5. 每完成一个有意义的增量就运行最小的有用检查。

交付前：

1. 复查完整 diff，包括未跟踪文件和生成产物。
2. 运行 `git diff --check`、相关聚焦测试，以及下方验证矩阵要求的更大范围门禁。
3. 确认文档、注释、fixtures、migrations、截图和发布证据是否必要并已处理。
4. 报告确切证据以及任何未运行的内容。代码能编译或单条 happy-path 测试通过，
   都不代表分支已完成。

## 工作规则

- 特性工作使用独立的 git worktree。推荐位置：`.worktrees/<branch-name>`。
- 保留用户的 dirty 变更。不要回退或覆盖不是你创建的工作。
- 在确认来源之前，把已有变更和未跟踪变更都视为用户所有。
  绝不用破坏性清理让 worktree 看起来干净。
- 使用聚焦的 commits。每个 commit 应描述一个连贯的 checkpoint。
- 行为变更使用 TDD：
  - 先写一个失败测试，
  - 确认它按预期原因失败，
  - 实现最小的通过改动，
  - 重跑聚焦测试。
- 编辑面向用户的文档时保持双语：
  - 同步更新英文和 `*.zh-CN.md` 对应版本。
- 把文档和代码注释视为实现的一部分，并作为必须遵守的编码标准：
  - 当行为、命令、架构、配置或用户可见 UI 变化时，同步更新相关文档；
  - 对不明显的控制流、invariants、协议边界或安全规则，补充简洁注释；
  - 避免只复述显而易见代码的噪音注释。
- 在结束任何代码变更前，明确判断这个 diff 是否需要文档更新或解释性注释，
  并在相关时把该判断写进验证说明。
- 项目编码标准遵循 `docs/development-standards.md`，
  尤其是其中的文档与代码注释要求。
- 根目录文档保持精简。完整产品细节放在 `docs/` 下。
- 不要编辑 `.ref/`；它只是参考材料。
- 不要把 `.omx/`、`.viden/`、`.worktrees/`、`.ref/` 和构建产物纳入被跟踪源码。

## 文档与设计规则

- 面向用户的文档是双语的。同步更新英文和
  `*.zh-CN.md` 对应版本。
- 根目录文档保持简洁；详细设计、计划、调查和发布证据放在 `docs/` 下。
- 文档描述已验证的当前行为。清楚标注提案、原型、部分实现和未来门禁。
- 采纳的视觉真源是 `docs/viden-design/Viden/`。其嵌套的
  `AGENTS.md`、`CLAUDE.md`、`tokens.css`、`docs/DESIGN-REF.md`、
  `docs/SPEC.md` 和 `docs/screens-status.js` 定义了该目录的局部治理。
- 不要把归档页、已删除的导入、生成的 previews、mock data、
  Babel 原型脚手架或 `.ref/` 内容当作生产真源。
- 共享 tokens 和已注册组件应复用，而不是复制进前端分叉。
  当设计包规则要求时，视觉行为变更必须更新相应的设计状态、
  changelog、guard baseline 和评审证据。

## 测试

按变更范围选择验证方式。开发过程中使用聚焦检查：

```bash
cargo test -p viden-types
cargo test -p viden-session
cargo test -p viden-workflows
cargo test -p viden-lanes
cargo test -p viden-runtime
```

额外必需门禁：

- Core/共享 contract 变更：受影响 crates 的聚焦测试、
  `scripts/check-dependency-boundaries.sh`，然后是 workspace 套件。
- TUI 行为：`cargo test -p viden-tui`、
  `scripts/tui-turn-controller-smoke.sh`、`scripts/rc-tui-stability-smoke.sh`，
  以及适用时的 `scripts/tui-regression.sh`。
- TUI 视觉：用 `scripts/tui-previews.sh` 重新生成确定性证据并复查输出。
- Context/evidence/cost 变更：运行相关 context benchmark contract
  smoke，并保持 canonical evidence parity。
- 纯文档变更：用显式 changed paths 运行文档 pair/link 检查，
  外加 `git diff --check`。
- 面向发布的变更：使用发布计划要求的 release gate/smoke 脚本和
  live-provider 证据。

在宣布共享分支或实现分支完成前，运行：

```bash
cargo test --workspace --quiet
```

对 CLI 相关行为，可行时补一条 fallback-provider smoke 测试：

```bash
cargo run -p viden-cli -- --provider fallback --model test-local
```

除非任务明确授权，否则不要运行 live provider、publish、release 或 Homebrew
变更步骤。在交接说明中列出被跳过的门禁。

## Commit 与交接标准

- 每次提交一个连贯的 checkpoint，commit message 用祈使语气说明交付的行为或
  contract。
- 不要把无关的用户变更或超出本任务证据要求的生成产物纳入暂存。
- 一份交接必须包含：
  - 分支、worktree 和 HEAD；
  - 变更的文件/模块和归属 scope；
  - 行为和 contract 影响；
  - 相关时的 migrations、fixtures、文档/注释和视觉证据；
  - 确切的验证命令和结果；
  - 被跳过的检查及原因；
  - blockers、contract requests 和下一个安全步骤。
- 区分已提交、仅存在于 worktree、已推送、已合并和已发布这几种状态。
  绝不把其中一种说成另一种。

## 发布纪律

- 把 GitHub Release 和 `wikieden/homebrew-tap` 视为同一版本的一个发布单元。
- 发布完成需要 GitHub assets、Homebrew 更新和发布后 smoke 证据。
- 当 tap 过期、assets 缺失，或必需的 live/打包证据未验证时，
  不要报告发布已完成。
- Publishing、tagging、pushing 和 Homebrew 变更需要用户显式授权。

## 参考项目指引

长期参考架构（用户决策，2026-08-17）：在提出或决定任何需求或架构变更前，
先查阅 `openai/codex` 和 `deepseek-ai/deepseek-harness`。

- `openai/codex`（Rust agent CLI）与 Viden 在架构上同构：core 拥有状态、
  薄前端、JSONL facts 加派生索引。对照它检查 contract fixtures、协议演进
  纪律、sandbox/permission 的决策与执行分离、app-server daemon surface、
  headless contract 客户端，以及可替换的 compaction 策略。本地深读笔记若存在，
  放在 `proposals/` 下。
- `deepseek-ai/deepseek-harness`（Node.js，“一切皆 plugin”，基于 Cordis）是
  plugin 优先组合、把 subagent 工作委派给外部 CLI，以及 web operator surface
  的参考。它是开发者预览版，会有破坏性变更；引用时必须写明查阅的确切 commit，
  并且绝不 vendor 它的代码。

如果某个需求决策同时偏离这两个参考，必须在作为依据的计划或设计文档中记录
原因。

`.ref/claude-code-main` 中有用的模式：

- `main.tsx`：启动与 runtime 编排。
- `commands.ts`：宽泛的 slash-command surface 和命令族结构。
- `Tool.ts`：工具契约和共享执行语义。
- `types/permissions.ts`：permission modes 和策略形态。
- `tasks/*`：task/session workflow 思路。
- `bridge/*`、`plugins/*`、`context/*`、`keybindings/*`：未来平台扩展参考。

不要照搬：

- Bun、React 或 Ink 的实现细节。
- 在核心工作流成熟前引入产品分析和 managed settings。
- 在本地 CLI 模型稳定前引入 remote/bridge/MCP/multi-agent 复杂度。

## V3 并行开发协调

Core、TUI 和 GUI 并行工作以 `docs/parallel-development-plan.md`
及其中文版为准。把以下内容当作该计划的执行规则：

| 分支线 | 嵌套指令 | 独占实现 scope |
| --- | --- | --- |
| Core | `crates/AGENTS.md` | `crates/**` 和共享 runtime contracts |
| TUI | `apps/tui/AGENTS.md` | `apps/tui/**` 和 TUI 专属证据 |
| GUI | `apps/gui/AGENTS.md` | `apps/gui/**` 和 GUI 专属证据 |

- 最多三个并发实现负责人：Core、TUI 和 GUI。只读协调任务不占用实现 scope。
- 各自独立跟踪版本：Core 用 `core-v0.3.x`、TUI 用
  `tui-v0.3.x`、GUI 用 `gui-v0.1.x`，直到计划被修订。讨论集成时，
  报告必须同时写明 workspace candidate 以及 Core、TUI、GUI 版本。
- 从同步后的 `origin/main` 开出 `codex/v3-core-runtime`。在正式 TUI 或 GUI
  实现开始前，Core 必须发布不可变的 `frontend-contract-v1` checkpoint。
- `codex/v3-tui-client` 和 `codex/v3-gui-client` 必须从那个确切的 Core
  checkpoint 开出，而不是从更旧的 UI 分支或未经验证的本地检出。
- 每条实现分支放在各自的 `.worktrees/<branch-name>` worktree 中。
  重叠的写入 scope 必须串行化。
- Core 拥有权威状态和副作用。TUI 和 GUI 只能维护本地展示状态，
  并且必须使用共享的 command、event、snapshot 和 replay contracts。
- 缺失的前端能力是一个 Core contract request。不要用前端私有 reducer、
  直接访问 runtime 或推断成功来绕过它。
- 语言、locale、skin、mode、density、font scale 和 accessibility 设置属于
  Core 拥有、前端消费的共享展示偏好。前端可以拥有本地渲染，
  但不能拥有独立的偏好持久化或私有 skin 调色板。
- 先从 `docs/viden-design/Viden/index.html` 评审设计。TUI 继续看
  TUI 设计稿索引、统一原型和组件库。GUI 继续看 GUI 设计稿索引、
  桌面驾驶舱和组件库。
- 按固定顺序 Core -> TUI -> GUI 集成。每一步之后运行 parity fixtures 和
  相应的分支门禁。
- 每次交接都必须报告分支、worktree、HEAD、变更的归属 scope、测试、
  contract requests、blockers 和下一步安全的并行工作。
- 除非用户显式要求，否则不要合并或推送 `main`。

## 当前分支上下文

当前规划线是 V3 多前端开发。把 `PLAN.md`、
`docs/parallel-development-plan.md` 和
`docs/parallel-development-plan.zh-CN.md` 当作 roadmap 和分支拓扑来源。
旧文档中的历史分支描述不是当前建分支的依据。
