# Viden 0.1.20 计划 - Usability Beta Gate

英文版： [release-0.1.20-plan.md](release-0.1.20-plan.md)

## Summary

`0.1.20` 是 usability beta gate。这个版本要让 Viden 对非维护者也真正可用：

> 安装 Viden -> 打开 TUI -> 配置 provider/model -> 跑一个日常开发任务 ->
> 看懂系统现在在干嘛 -> 审批/应用变更 -> 跑测试 -> 委托一条 review lane ->
> 检查 evidence -> 有信心地结束任务。

这是一个交互和产品可用性版本。它应该优先做好少数清楚、可靠的工作流，而不是继续增加
需要猜命令 id 或读实现文档才能用的新入口。

## 产品目标

到 `0.1.20` 结束，首次使用或回访的开发者应该能够：

- 从干净安装启动 Viden，并知道下一步该做什么；
- 在 TUI 内配置 provider/model，不需要背命令；
- 通过可见动作修复缺 key、endpoint 错误、model 不兼容或 provider 请求失败；
- 完成一个小型真实 coding loop：审批、文件变更、测试、diff evidence 和最终总结；
- 启动一条 delegated lane，观察运行、检查 evidence，并 accept/apply 或 discard；
- 相信界面上的 footer action、button、selector 和 side panel 都是真功能，不是装饰。

## 0.1.19 基线

main 上已经具备：

- 默认 TUI 入口和 fallback provider 路径；
- selector-first 的 `/settings`、`/provider`、`/models`、`/permissions`、`/theme`；
- `/provider` 和 `/models` 语义分离；
- `PROVIDER CONFIG` 二级 provider 配置页；
- `NOW WORKING` runtime state projection；
- deterministic delegated shell/template lane smoke；
- TUI screenshot generation 和 regression evidence；
- GitHub release 与 Homebrew 发布流程。

当前差距是产品信心：布局仍有粗糙处，mouse coverage 不完整，first-run setup 还不够像真实
向导，settings 路径有时更像命令补全而不是配置流程。

## 实现检查点

- Setup wizard 外壳：`/setup` 现在会打开独立的 `SETUP WIZARD` selector，包含
  provider 配置、model 选择、permissions、theme、当前 provider doctor、fallback
  smoke 和保存默认值这些真实动作。确定性的 `main-setup-wizard` preview 和
  regression artifacts 已纳入视觉证据集。
- 缺 key 启动：当当前在线 provider 没检测到 API key 时，主 TUI 会预填 `/setup`，
  让干净安装直接落到可操作设置界面，而不是只看到被动 transcript 提示。
- Provider failure recovery：provider/model 错误现在会给出 recovery class，包括
  missing key、auth、rate limit、timeout、context overflow、compatibility 或 model
  unavailable，并带上具体下一步，以及切 model、provider doctor 和 fallback 命令。
- Lane 根选择器：`/lane` 现在会打开居中的动作 selector；已有 lane 时，会列出带 id
  的 inspect/timeline/diff/artifacts 动作。

## P0 Scope

### 1. First-Run Setup Wizard

把当前 command-guided setup 改成真正的分阶段 TUI wizard。

验收：

- 干净安装且没有保存 provider/model 时，TUI 显示明确 setup 状态，而不是被动 transcript。
- `/setup` 在任意 session 都能打开同一套 wizard。
- 阶段明确：provider -> API key/env -> endpoint -> model -> probe -> save defaults。
- DeepSeek 仍是默认在线路径，fallback 是离线逃生路径。
- 默认不把 API key 明文写入 config。缺 key 时显示准确 env var 和 shell export 提示。
- probe 结果有可执行下一步：继续、切 model、改 endpoint、打开 doctor 或用 fallback。
- first-run wizard 必须有确定性 preview 和截图。

### 2. Settings Modal Unification

所有用户决策类设置必须像同一个产品系统。

验收：

- `/settings` 是真实 settings hub，包含 provider、model、permissions、theme、defaults、
  diagnostics。
- `/provider` 打开 provider 列表，再进入 `PROVIDER CONFIG`。
- `/models` 打开按 provider 分组的模型选择器。
- `/permissions`、`/theme` 和后续 mode switch 使用同一 centered selector/modal 行为。
- 每个 selector 都支持键盘方向键、Enter、Esc 和鼠标点击。
- 长行必须换成 detail 区或拆分展示，不能依赖横向截断才能读懂。
- footer 文案只能写真实可用的动作。

### 3. Composer、Cursor、IME 和布局可靠性

让输入区域在日常使用里可靠。

验收：

- composer 有足够高度容纳一行有效输入和 mode/actions，不再显得挤。
- 在不可靠渲染 native blinking cursor 的终端里，仍有高对比 caret fallback。
- macOS Terminal/iTerm2 下，中文输入和 IME candidate window 尽量贴近输入文本。
- 快速 resize 不留下旧线、右栏漂移、panel 错位或弹窗残影。
- 增加 resize stress regression，覆盖 composer、selector、approval 和 lane-detail 状态。

### 4. Mouse And Focus Router

从零散 mouse handling 变成显式 focus target。

验收：

- focus target 明确：`composer`、`selector`、`approval`、`right-rail`、`lane-detail`、
  `side-screen`、`transcript`。
- 鼠标点击支持 selector rows、provider config actions、approval buttons、right-rail
  tasks、lane controls 和 side-screen route selection。
- 鼠标滚轮滚动当前 focus 的可滚动 panel。
- Esc 行为一致：先关 modal/selector，再清 composer command，最后才在合适场景退出。
- focus state 足够可见，用户知道键盘动作会作用到哪里。

### 5. Provider Failure Recovery

Provider/model 失败要引导修复，而不是只显示 API error。

验收：

- 分类常见失败：缺 key、endpoint 错误、auth failure、rate limit、timeout、model
  unavailable、不支持 tool-call 格式、context overflow、provider compatibility mismatch。
- 每类失败都给具体动作：切 model、打开 `/models`、打开 provider config、run doctor、
  use fallback 或 retry。
- 当前 model 已知有风险或兼容检查失败时，TUI 显示 model recovery prompt。
- `/doctor` 和 `/provider doctor <id>` 共用同一套 provider readiness facts。
- 增加 failure classification 和 recovery prompt 聚焦测试。

### 6. Daily Coding Loop Evidence

普通单 agent coding loop 必须可证明。

验收：

- deterministic daily-loop smoke 覆盖：prompt -> provider turn -> approval -> write file ->
  shell/test -> diff/test evidence -> final summary。
- main screen 在 thinking、tool call、approval、shell/test execution、completion 时都显示
  `NOW WORKING`。
- right rail 和 recent files 从真实 runtime facts 更新。
- 增加 live thinking、approval、shell/test running、diff/test evidence、final summary 截图。

### 7. Delegated Review Loop Beta

保持一条 delegated lane workflow 可用，其他 agent 集成先不扩面。

验收：

- `/lane` 根命令打开 actionable selector，不只是文字 help。
- lane id 操作不用手敲 id：inspect、timeline、diff、artifacts、accept、apply、
  discard、retry、stop、cleanup 都能选择。
- side-1 显示 lane console/tail/transport state。
- side-2 显示 artifacts、changed files、context pressure、decision state 和 apply/conflict
  status。
- deterministic shell/template lane 仍是 CI baseline。
- Codex/Claude 除非在发布机能稳定验证 happy path，否则仍作为 optional probe。

## P1 Scope

- favorite providers/models 和 last-known-good model history。
- provider API 支持时做 remote model discovery。
- 更好的 tmux/PTY attach ergonomics 和 lane input forwarding。
- provider request 发送前显示 token/context pressure warnings。
- 窄终端下更紧凑的 right-rail layout。

## Non-Goals

- `0.1.20` 不做广义 plugin marketplace。
- 本版本不把 ACP/MCP/skills 做成 mutating runtime surface。
- 不默认让 Codex/Claude write-capable。
- 不把 API key 明文保存到 config 作为默认 setup path。
- 不用漂亮截图替代 smoke evidence。

## Test Plan

聚焦测试：

- setup wizard 状态流和 persistence 边界；
- provider config action selection 和 mouse hit testing；
- settings/provider/model/permissions/theme selector 行为；
- provider failure classification 和 recovery prompts；
- composer caret 和 CJK preview rendering；
- selector、approval、lane detail、composer 的 resize stress；
- lane selector id/action flows；
- daily coding loop runtime state projection。

回归：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-previews.sh docs/previews/generated
scripts/tui-regression.sh docs/previews/generated
scripts/daily-loop-smoke.sh
scripts/release-smoke.sh --version 0.1.20 --quick \
  --out-dir /tmp/viden-0120-release-smoke-local
```

人工验证：

- macOS Terminal 和 iTerm2 first-run setup。
- selector、approval、lane detail、provider turn active 时拖动 resize。
- CJK input 和 IME candidate window。
- provider config、model selector、approval、right rail、lane controls 的鼠标选择。
- fallback provider daily loop。
- 有凭证时跑 DeepSeek live provider daily loop。
- deterministic delegated lane review loop。

## Screenshot Evidence

必须有确定性或真实使用截图：

- first-run setup wizard；
- provider list 和 `PROVIDER CONFIG`；
- 按 provider 分组的 model selector；
- settings hub；
- visible composer/caret/CJK input；
- resize stress redraw；
- live provider thinking / `NOW WORKING`；
- approval modal default action；
- shell/test running；
- diff/test evidence；
- lane selector；
- lane running side-1；
- lane evidence side-2；
- lane accept/apply/discard；
- final daily-loop summary。

## Release Standard

`0.1.20` 完成条件：

- clean-install setup 能完全从 TUI 完成；
- fallback 和 live-provider recovery path 已文档化并测试；
- daily coding loop smoke 通过；
- delegated lane operator-loop smoke 通过；
- interaction regression 覆盖 resize、mouse、selector、approval 和 CJK input；
- 截图已生成并被文档引用；
- README 和 user guide 只描述已实现行为；
- GitHub release assets 和 Homebrew formula 已发布；
- post-publish smoke 验证 GitHub assets 和 Homebrew。
