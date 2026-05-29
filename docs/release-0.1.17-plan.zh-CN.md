# RoboCode 0.1.17 计划 - 日常编码闭环基线

英文版： [release-0.1.17-plan.md](release-0.1.17-plan.md)

## Summary

`0.1.17` 要把下个版本从“继续增加 surface”拉回到“真实日常可用”。目标不是让
RoboCode 看起来更完整，而是让一个普通编程闭环足够可靠，可以在真实项目里使用：

> 安装 -> 配置 provider -> 提一个小范围改动 -> 审批 edits -> 跑测试 ->
> 看失败 -> 修复或委派 -> review diff -> 保存证据并支持之后恢复。

原计划把轻量 spec/steering 放在 `0.1.16` 后面。这个方向仍然重要，但它必须服务于
日常编码闭环，而不是变成另一个孤立功能。`0.1.17` 中，spec/steering 先收敛成
最小 task brief 和 project convention layer，让真实编码任务更安全、更可重复。

## 产品目标

到 `0.1.17` 结束时，开发者应该能用 RoboCode 在单仓库里完成一个小型编码任务，
而不需要频繁切到另一个终端才能搞清楚发生了什么。

## 0.20 真正可用 North Star

`0.1.20` 前，如果下面这些都成立，RoboCode 才算开始“真正可用”：

- 首次使用可理解：provider/model/API-key 状态可见，用户能在 TUI 里修 setup。
- DeepSeek 是默认在线路径。fallback/test-local 仍保留为显式离线/测试路径，而不是
  真实使用时的静默默认值。
- 模型失败能在产品内恢复：unsupported model、unavailable model、auth 和 provider
  compatibility error 都要提示具体的 provider/model 切换动作。
- 主编码闭环可跑通：需求、文件编辑、审批、shell/test、diff review、最终总结。
- 失败恢复清楚：测试失败、apply conflict、provider error、取消 turn 都会显示下一步安全动作。
- Context 可控：用户能看到包含了什么 context、遗漏了什么，以及为什么遗漏。
- Delegated lane 至少有一条真实可用路径：Codex/Claude/shell lane 能运行、产出证据，
  并且可以 accept/discard/apply，不靠猜。
- TUI 在长 turn、resize、中文输入、审批和副屏监控时保持响应。
- 文档解释用户真实要做的 workflow，而不只是架构说明。

## P0 范围

### 1. 日常编码闭环 Smoke

增加一个确定性 smoke 场景，证明 RoboCode 能完成正常编码闭环：

1. CI 中使用离线 fallback/test provider 启动，并在人工验收中用 DeepSeek credentials
   跑同一条路径。
2. 在 fixture workspace 中请求一个小代码改动。
3. 通过 permission path 触发文件编辑。
4. 审批 edit。
5. 执行 test command。
6. 在 transcript、右栏和 side-2 显示失败或成功证据。
7. 执行 `/diff` 或 `/git diff`。
8. 输出最终总结，包含 changed files、tests 和 next action。

验收：

- 新增 `scripts/daily-loop-smoke.sh`，或扩展 `scripts/release-smoke.sh` 的
  daily-loop step。
- 证据必须包含 transcript、changed file、test output、diff summary，以及确定性
  TUI screenshot 或 ANSI capture。
- smoke 不依赖 live provider key，但真实使用验收还必须证明 DeepSeek-first launch path。

### 2. Task Brief 和 Steering Files

实现最小可用 spec/steering layer：

- `/brief` 或 `/spec` 从当前 request 创建 task brief。
- `/brief show` 展示：
  - goal
  - constraints
  - 可能涉及的 files
  - acceptance checks
  - risk notes
- Project steering files 放在 `.robocode/steering/`：
  - `conventions.md`
  - `architecture.md`
  - `workflows.md`
- ContextBundle 可以引用 active brief 和 steering summaries。

验收：

- task brief 可以创建、展示，并附加到 lane envelope。
- steering files 不会在没有显式用户命令时变成 active project facts。
- 有 active brief 时，TUI 在 `NOW WORKING` 或 side-2 中显示 brief id/title。

### 3. DeepSeek 优先的 Setup 和模型恢复

工具必须帮助新用户尽快进入可用状态：

- 干净安装且没有显式 provider 配置时，默认在线 provider target 是 DeepSeek。
- `/setup` 要打开 TUI 内交互式配置流程，而不是只打印说明：
  - 选择 provider
  - 从 provider 默认/候选模型中选择 model
  - 展示 API key env var 以及是否存在
  - 展示 config path 和保存目标
  - 有 credentials 时测试 provider reachability
- `/settings` 保留命令式路径，服务脚本和高级用户：
  `/settings provider <id> [model]`、`/settings model <model>`、`/settings save`。
- `/doctor` 覆盖 TUI、provider、git workspace、release version 和 lane prerequisites。
- 缺少 provider credential 时，显示可执行的修复命令或 env-var 提示。
- 当前 model 不可用时，RoboCode 要显示可行动的换模提示，而不是直接露出原始
  provider error。覆盖 unknown model、unavailable model、auth/model permission error、
  context-limit mismatch 和 provider protocol incompatibility。
- 模型恢复提示要建议：
  - provider 默认模型
  - 已知兼容备选模型
  - `/settings model <candidate>`
  - `/settings provider <provider> <model>`
  - `/provider doctor <provider>`

验收：

- `robocode-cli --provider fallback --model test-local` 仍是离线逃生路径。
- 没有保存 provider 时启动，会进入 DeepSeek-first setup path。
- DeepSeek setup path 在文档、TUI、`/setup`、`/settings` 和 `/doctor` 中可见。
- 交互式 provider/model 配置流程可以保存 provider/model 默认值，但不会保存 API key。
- 故意配置无效 model 时，界面出现可见的“切换模型”动作，并至少给出一个兼容
  DeepSeek candidate。
- `doctor` output 被 daily-loop 或 release smoke 捕获为证据。

### 4. 可 Review 的 Diff 和 Test Evidence

强化“这次改动是否可以接受”的路径：

- `/diff` 总结 changed files、additions/deletions 和可能的 test commands。
- `/test <command>` 失败时展示：
  - failing command
  - exit code
  - duration
  - top failure lines
  - 可能相关 file paths
  - 建议 rerun command
- side-2 优先显示最新 diff/test evidence，而不是低信号行。

验收：

- daily-loop smoke 中的失败测试会产生可行动 next action。
- 成功测试会产生明确的 `ready for review` 状态。

### 5. 真实使用截图集

每个 P0 功能都要有一个确定性 artifact 或真实截图：

- setup/doctor
- 交互式 provider/model setup
- model failure 和 switch-model prompt
- active brief
- edit approval
- test failure 或 success
- diff review
- final ready state
- side-2 evidence

## P1 范围

- 扩展鼠标覆盖到右栏、side panels 和 lane modal controls。
- 对支持取消的 provider/runtime path 实现真正 cancellation。
- provider 暴露 streaming event 时，实现第一版 streaming token renderer。
- 增加 `robocode doctor --json`，服务自动化。
- 增加 `--daily-loop-smoke` preview fixture，用于 release screenshots。
- provider/model favorites 和 last-known-good model history。

## 非目标

- 不扩展 ACP/MCP/plugin mutation。
- 不做 marketplace 或 install UX。
- 不默认启用 Codex/Claude write-capable 模式。
- 不做完整 spec 产品；task brief 和 steering 先保持最小。

## 测试计划

Focused：

- brief/steering command tests
- ContextBundle 包含 active brief/steering summaries
- setup/doctor provider diagnostics tests
- 默认 DeepSeek provider resolution tests
- 交互式 provider/model setup reducer tests
- provider/model failure classification 和 recovery-prompt tests
- diff/test evidence reducers
- TUI active brief 和 side-2 evidence render tests

Regression：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/release-smoke.sh --version 0.1.17 --quick --out-dir /tmp/robocode-0117-release-smoke-local
```

Manual：

- 从 Homebrew 安装。
- 无保存配置启动，确认进入 DeepSeek-first setup path。
- 用 fallback provider 作为显式离线路径启动。
- 用 DeepSeek credentials 和有效 model 启动。
- 用 DeepSeek credentials 和无效/不可用 model 启动，确认出现 switch-model prompt。
- 从交互式 setup flow 保存 provider/model 并重启。
- 在一个小型真实仓库上跑日常编码闭环。
- 为这个闭环至少截一张真实终端图。

## 0.18-0.20 桥接

- `0.1.18`：失败恢复和 review gates。让 test failure、diff review、
  apply/rollback 成为产品里最强的一条路径。
- `0.1.19`：delegated lane 真正有用。让一条 Codex/Claude/shell delegated
  workflow 足够可靠，可用于真实 review task。
- `0.1.20`：usability beta。干净安装后，应能跑通 daily coding loop 和一条
  delegated review loop，并产出文档化证据。
