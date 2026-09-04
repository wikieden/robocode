# Viden Claude Code 入口

英文版：[CLAUDE.md](CLAUDE.md)

这个文件是 Viden 仓库的 Claude Code 入口。仓库根目录的 `AGENTS.md` 是跨工具的
规范真源。动手前先读完它；本文件只补充 Claude 专属的路由和命令快捷方式，
不重新定义项目规范。

不要把本文件与
`docs/viden-design/Viden/CLAUDE.md` 混淆。后者只管辖已采纳的
设计包，当工作触及该子目录时必须阅读。

## 强制阅读顺序

1. 读根目录 `AGENTS.md`。
2. 对预期写入 scope 中的每个文件，读最近一层的嵌套 `AGENTS.md`。
3. 读该任务对应的计划、contract、设计或发布状态文档。
4. 在依赖分支名或先前交接说明之前，先检查真实的 Git 分支、worktree、
   dirty 状态和远端新鲜度。

重要的嵌套 scope：

| Scope | 指令文件 |
| --- | --- |
| Core crates | `crates/AGENTS.md` |
| TUI 客户端 | `apps/tui/AGENTS.md` |
| GUI 客户端 | `apps/gui/AGENTS.md` |
| 设计包 | `docs/viden-design/Viden/AGENTS.md` 及其本地 `CLAUDE.md` |

当指令冲突时，优先遵循用户当前请求，然后是根目录
`AGENTS.md`，再然后是最近一层的嵌套指令。遇到真正的 scope 冲突要停下来
提出，而不是悄悄选择更大范围的 mutation。

## 仓库概览

Viden 是一个 Rust 优先、本地优先的 AI 编码工作台，拥有共享 runtime
和多个客户端：

- `apps/cli`：二进制入口和 bootstrap；
- `apps/tui`：终端客户端；
- `apps/gui`：规划中的桌面客户端边界；
- `crates/core`：稳定的多前端 facade；
- `crates/runtime`：session 与 agent 执行编排；
- `crates/lanes`：lane 生命周期编排与 lane 本地副作用，
  位于 runtime 之下；
- `crates/agents`：位于 runtime 之下的外部 agent adapters（ACP、Codex
  app-server），只通过 tool capabilities 访问操作系统；
- `crates/context`：context、evidence-reference、retrieval 和 cost 引擎；
- `crates/types`：共享领域类型和协议类型；
- `crates/session`：canonical JSONL session facts 和可重建的 SQLite 索引；
- `crates/workflows`：长期项目 tasks 和 memory；
- `crates/provider`、`crates/tools` 和 `crates/permissions`：执行
  适配器和 mutation 策略边界；
- `crates/plugin-*` 和 `plugins/**`：扩展契约和第一方
  plugins。

Core 拥有权威业务状态和副作用。TUI 和 GUI 是同一套
command/event/snapshot/replay contract 的客户端。前端不得直接访问
provider、tool、permission、session、workflow、JSONL 或 SQLite 内部实现。

## 启动检查清单

实现前：

```bash
pwd
git status --short --branch
git worktree list --porcelain
git log -1 --oneline --decorate
```

如果任务依赖最新主线，先 fetch 并比较，然后再建分支。保留用户的 dirty 和
未跟踪工作。特性工作使用位于
`.worktrees/<branch-name>` 的独立 worktree。

动手前先给请求分类：

- 评审、诊断或状态：检查并报告；未被要求就不要实现；
- 设计或规划：按要求更新长期计划/规格，但不要暗示代码已实现；
- 实现：使用 TDD，随行为更新文档/注释，验证，并创建一个聚焦的 checkpoint；
- 发布或推送：只有在明确授权时才执行外部 mutation。

## 常用命令

构建与格式化：

```bash
cargo build --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
```

聚焦测试与完整测试：

```bash
cargo test -p viden-types
cargo test -p viden-session
cargo test -p viden-workflows
cargo test -p viden-lanes
cargo test -p viden-runtime
cargo test -p viden-core
cargo test -p viden-tui
cargo test --workspace --quiet
```

离线 runtime smoke：

```bash
cargo run -p viden-cli -- --provider fallback --model test-local
cargo run -p viden-cli -- --no-tui --provider fallback --model test-local
```

依赖、TUI 和文档门禁：

```bash
scripts/check-dependency-boundaries.sh
scripts/tui-turn-controller-smoke.sh
scripts/rc-tui-stability-smoke.sh
scripts/tui-regression.sh
scripts/tui-previews.sh
scripts/check-doc-pairs.sh <changed-markdown-path> [...]
scripts/check-doc-links.sh <changed-markdown-path> [...]
git diff --check
```

先运行最小的相关检查，再按根目录 `AGENTS.md` 扩大范围。未经明确授权，
不要运行 live-provider、publish、release 或 Homebrew mutations。

## V3 Core、TUI 与 GUI 路由

以 `PLAN.md` 和
`docs/parallel-development-plan.md` 为准。

```text
origin/main
  -> codex/v3-core-runtime
       -> immutable frontend-contract-v1 checkpoint
            -> codex/v3-tui-client
            -> codex/v3-gui-client
```

规则：

- 最多三个并发实现负责人：Core、TUI 和 GUI。
- Core、TUI 和 GUI 有各自独立的版本线：`core-v0.3.x`、
  `tui-v0.3.x` 和 `gui-v0.1.x`。相关时，集成报告还要写明聚合的
  workspace candidate。
- Core 先启动，并发布确切的 checkpoint SHA。
- TUI 和 GUI 从那个 SHA 开始。没有它，它们只能停留在设计、
  交互/参考确认、框架 spike 规划或 contract-gap
  分析阶段。
- 重叠的写入 scope 要串行化。不要让多个任务各自独立修复同一份
  共享 manifest、contract、fixture、token 或设计决策。
- 缺失的客户端数据或动作要转成 Core contract requests。
- 语言、locale、skin、mode、density、font scale、terminal color capability
  和 accessibility 设置都通过 Core 拥有的共享展示偏好 contract 流转。
  前端不得持久化第二套偏好模型，也不得内置私有调色板。
- 按 Core -> TUI -> GUI 集成，并在每一步之后重跑 parity 证据。
- 除非用户显式要求，否则不要合并或推送 `main`。

## 设计与视觉工作

采纳的视觉真源是 `docs/viden-design/Viden/`。编辑它之前，
按该包的要求读它本地的 `AGENTS.md`、本地 `CLAUDE.md`、`docs/SPEC.md` 和
`docs/DESIGN-REF.md`。

- `tokens.css` 是 token 真源。
- 已注册的组件和图标应复用，而不是复制。
- 视觉评审从 `docs/viden-design/Viden/index.html` 开始。TUI 打开
  TUI 设计稿索引、统一原型和组件库。GUI 在使用更底层页面之前，
  先打开 GUI 设计稿索引、桌面驾驶舱和组件库。
- TUI 使用 canonical TUI kit 和已注册字形；不要添加 emoji。
- 在 Tauri/GPUI 垂直切片门禁决定之前，GUI 保持框架中立。
- 归档页、mock data、生成的 previews、原型 runtime 脚手架
  和 `.ref/` 都不是生产真源。
- 视觉变更需要嵌套规则指定的设计包 guards、状态/changelog 更新
  和截图证据。

## 编码与文档纪律

- 所有模型 tool call 和本地副作用都走共享 runtime。
- Permission checks 发生在 mutation 之前。
- Plan mode 在副作用发生前拒绝所有 mutation 路径。
- JSONL facts 保持 append-only 和 canonical；SQLite 保持派生。
- Assistant 建议的 project memory 需要用户显式确认。
- 行为变更使用 TDD，并确认失败测试是按预期原因失败的。
- 当命名和类型不足以说明问题时，用简洁注释解释协议、持久化、permission、
  渲染和并发方面的 invariants。
- 同步更新受影响的英文和 `*.zh-CN.md` 文档。
- 只把已验证的行为描述为已实现；诚实标注提案和部分完成的工作。
- 绝不编辑 `.ref/`，也不要提交 `.omx/`、`.viden/`、`.worktrees/`、`.ref/`
  或构建产物。

## 交接格式

实现工作结束时给出：

1. 结果，以及用户可见或 contract 层面的影响；
2. 分支、worktree 和确切 HEAD；
3. 变更的模块和归属 scope；
4. 测试、smoke 检查、截图、fixtures、migrations 和文档/注释；
5. 未运行的检查及原因；
6. blockers 和 Core contract requests；
7. 下一个安全动作。

分别说明工作是仅存在、已提交、已推送、已合并还是已发布。
在必需证据缺失时，不要称某个分支已完成。
