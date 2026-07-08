# Viden 0.1.9 计划

英文版： [release-0.1.9-plan.md](release-0.1.9-plan.md)

最后更新：2026-05-27

## 版本定位

`0.1.8` 已发布 live multi-agent cockpit 基础：统一任务投影、operation center
状态、增强 `/test` 证据、副屏预览、发布打包和 Homebrew tap。

`0.1.9` 的目标是把这套基础打磨到足够可靠，方便更大范围的真实试用。

版本主题：

```text
0.1.9 = Verification Hardening + Screenshot-Gated UX
```

核心目标：每个重要编程工作流都要有确定性测试、release smoke 证据，以及一张真实使用截图
或终端画面，产品侧确认后才能认为该功能完成。

## P0：必须交付

### 1. 发布门禁强化

目标：一个 release 命令能证明 build、package、smoke test 和安装路径都健康。

交付：

- 把 `scripts/release-smoke.sh` 升级为标准发布门禁。
- 纳入 `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  相关 crate focused tests、`cargo test --workspace`、package smoke、fallback CLI
  smoke、Codex app-server protocol fixture、app-server write guard、lane operator loop
  smoke 和 TUI preview 生成。
- DeepSeek live smoke 与 GitHub release asset validation 保持显式 opt-in。
- release candidate 打 tag 或发布后，增加 Homebrew tap 验证。
- 在 smoke 输出目录写入结构化摘要，例如 `release-evidence.json`。
- 实现 opt-in 的 `--github-release-assets` 和 `--homebrew` release-smoke 检查，
  让发布后的验证也复用同一个命令入口。

验收：

- 维护者跑一个本地 release gate 就能知道哪个子检查失败。
- evidence 目录包含日志、package 元数据、生成预览和简洁摘要。
- release 文档记录被接受构建的命令和 evidence 目录。

### 2. TUI 回归测试框架

目标：防止 layout、边框、配色、resize、输入区、命令面板和审批弹窗回归。

交付：

- 新增专用 TUI regression 脚本，为 main idle、main active、approval overlay、
  command palette、side-1、side-2 和主题变体生成稳定 text/ANSI/SVG 快照。
- 覆盖典型终端尺寸：紧凑、默认、宽屏桌面、竖屏副屏。
- 增加 panel bounds、右栏宽度、composer 高度、命令面板位置和副屏可渲染断言。
- 增加 color-integrity 检查，避免标题和边框出现意外混色。
- 截图产物放在 `docs/previews/generated/` 或清晰命名的 evidence 目录。
- 第一版 regression 入口是 `scripts/tui-regression.sh`，它会包装 preview 生成并导出
  0.1.9 命名的截图产物。

验收：

- 调整窗口大小后不再留下残影、断裂面板或错误右栏边框。
- composer 明显更高，光标状态可见，中文输入仍在输入区内。
- 每次改 TUI 都必须包含重新生成的截图或文本快照。

### 3. 截图确认门禁

目标：每个用户可见功能点最后都要有可人工确认的真实使用证据。

交付：

- 每个 feature task 至少捕获一张真实使用截图、终端录制帧或确定性 TUI SVG，展示功能在上下文中的真实状态。
- canonical preview 放到 `docs/previews/generated/`；临时验证产物放到 release evidence 目录。
- 每个功能的最终报告必须链接截图路径，并说明该截图证明了什么场景。
- approval overlay、副屏、命令面板、lane view、`/test` evidence 和安装流程都需要独立截图或捕获。

验收：

- 没有截图或等价视觉产物的用户可见功能不能标记完成。
- 截图展示的是运行功能后的真实屏幕状态，不只是设计稿。
- 产品侧可以根据视觉证据批准或驳回每个功能点。

### 4. AgentTask 与 Lane 验证

目标：多 agent 编排能力不依赖真实 Codex、Claude 或 DeepSeek 可用性也能测试。

交付：

- 增加 fixture，把 Viden 主回合、tool call、approval、`/test`、shell job、
  Codex job、Claude/DeepSeek lane、tmux、PTY 和未来 ACP 风格事件归一化为同一套
  `AgentTask` view。
- 增加生命周期测试：queued、thinking、streaming、editing、running tool、testing、
  waiting approval、blocked、done、failed、cancelled、archived。
- 扩展 lane smoke，覆盖 `/lane inspect`、`/lane send`、`/lane accept`、`/lane revise`、
  `/lane discard`、`/lane apply`、冲突处理、cleanup、archive、tmux 和 PTY evidence 路径。
- 验证主屏 operation center 永远解释当前 active task 在做什么，并给出 evidence source。

验收：

- mock Codex、Claude、DeepSeek、shell、tmux 和 PTY 事件都能生成稳定 `AgentTask` 行。
- 同一个 active task 在 operation center、右栏、side-1、side-2 和命令输出中保持一致。
- 一个 lane 可以启动、观察、追问、接受或丢弃，并能从 artifacts 审计。

### 5. 权限与安全回归套件

目标：测试必须证明新的 agent、plugin、MCP、lane 和 app-server 路径不会绕过 approval、
transcript 或 workspace 安全边界。

交付：

- 增加 shell、file write、Git mutation、app-server write、lane mutation、
  plugin/MCP invocation 和 Plan 模式 mutation blocking 回归测试。
- Codex app-server write-capable turn 在 live protocol 行为安全前继续保留显式实验开关。
- 增加 workspace 外写入、path traversal、隐藏文件和生成 artifacts 的 path-scope 测试。
- 确保 approval decision 和 denial 都写入 transcript 或 evidence logs。

验收：

- 缺少权限或被拒绝时，mutation path 默认失败关闭。
- Plan 模式阻止 file、shell、Git、task、memory、lane、plugin、MCP 和 app-server mutation。
- 安全检查进入默认 release gate。

## P1：应该交付

### 6. CI 矩阵升级

- 拆成 PR fast、main full 和 release full 三档。
- PR fast 跑 fmt、clippy、focused tests 和 quick smoke。
- Main full 跑 workspace tests、TUI regression、lane smoke 和 release smoke quick。
- Release full 构建所有支持平台 package、上传 artifacts、校验 sha256 并验证 Homebrew tap 发布。

### 7. Provider 兼容矩阵

- fallback provider 继续作为确定性 baseline。
- DeepSeek live smoke 保持 opt-in，并记录所需环境变量。
- 增加 OpenAI-compatible 与 Anthropic-style response shape 的 provider fixture tests，覆盖
  tool-call replay 和非空 assistant tool content。

### 8. 文档与操作手册

- 增加 testing and validation guide，说明 local、CI、live-provider、release 和截图门禁验证。
- 英文和中文文档保持同步。
- 增加 release checklist，包含打 tag 前的截图 review 和产品侧确认。

## 非目标

- 不做新的 marketplace 或云端 registry。
- 不做完整 ACP 实现，只做 fixture 和 mapping tests。
- app-server write path 在安全行为被证明前不默认开启。
- 除非现有 Rust / shell 栈无法关闭测试缺口，否则不引入新依赖。

## 截图证据合同

每个功能完成报告必须包含：

- 功能名；
- 用来验证的命令或工作流；
- 截图或视觉产物路径；
- 该产物证明了什么；
- 如有，剩余视觉风险。

推荐产物命名：

```text
docs/previews/generated/0.1.9-<feature>-main.svg
docs/previews/generated/0.1.9-<feature>-approval.svg
docs/previews/generated/0.1.9-<feature>-side-1.svg
docs/previews/generated/0.1.9-<feature>-side-2.svg
```

临时 release evidence 也可以放在：

```text
/tmp/viden-019-release-smoke-*/screenshots/
```

## 开发顺序建议

1. 增加 testing and validation guide 与 release checklist。
2. 升级 `scripts/release-smoke.sh`，输出结构化 evidence。
3. 增加 TUI regression 脚本和截图证据命名约定。
4. 扩展 `AgentTask` 与 lane fixtures。
5. 增加权限与安全回归覆盖。
6. 接入 PR、main、release 三档 CI。
7. 跑一次完整 0.1.9 release-candidate 验证，并附带截图。

## 验证门槛

0.1.9 发布前必须通过：

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 相关 crate focused tests
- `cargo test --workspace --quiet`
- `scripts/release-smoke.sh --quick`
- live provider credentials 可用时，运行
  `scripts/release-smoke.sh --version 0.1.9 --deepseek`
- 所有必需视觉状态的 TUI regression snapshots
- lane operator smoke 和 artifact inspection
- permission 与 app-server write-guard smoke
- host 平台 package smoke
- 发布后的 GitHub release asset 与 sha256 validation
- Homebrew tap 更新后的 fetch/install 验证
- 每个用户可见功能点的截图确认
