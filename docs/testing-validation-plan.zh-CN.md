# 测试与验证计划

英文版： [testing-validation-plan.md](testing-validation-plan.md)

最后更新：2026-06-07

## 目的

Viden 应该按“用户会长时间真实使用的开发者工具”来验证，而不是只按库代码的单元测试来验证。
验证体系必须同时证明行为、安全、发布状态和 TUI 可见质量。

这份文档是 `0.1.10` 及后续版本的长期验证合同。

## 验证层次

### 1. 本地快速检查

普通功能开发中运行：

```bash
cargo fmt --check
cargo test -p <touched-crate> --quiet
scripts/release-smoke.sh --quick
```

这一层用于实现过程中的快速反馈。

### 2. Workspace 检查

声称代码改动完成前运行：

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
```

如果改动涉及 TUI 渲染、lane 编排、权限、provider 协议、发布打包或安装流程，还必须运行相关
focused smoke 脚本。

### 3. Spec Drift 检查

任何用户可见行为、架构边界、命令语义或配置流程变更，都必须先检查相关 spec 文档：

- 如果文档写的是“当前实现”、“已实现”、“可用”，必须能指向代码、测试、截图或 smoke evidence。
- 如果代码还没做到，文档必须改为“目标”、“计划中”或“后续版本”，并在 release plan 里写清
  acceptance gate。
- TUI 长任务不能引入新的嵌套 input loop。provider turn、approval、doctor/probe、
  context build、tool/lane job 都必须通过主事件循环或 background job 回传事件。
- `/connect`、`/models`、`/setup`、`/permissions`、`/theme` 等交互设置不能退回成
  “显示命令说明然后让用户猜下一步”。TUI 下必须是 selector/form/modal first，core
  command text 只能作为 no-TUI fallback。

涉及 0.1.24 operator-loop 或 provider setup 时，还必须对照
`docs/spec-review-0.1.24.zh-CN.md`。P0 gap 没关闭时不能把版本标记为完成。

建议增加或运行的 focused gate：

```bash
rg -n "event::read\\(" viden-cli/src/tui
scripts/plan-mode-smoke.sh /tmp/viden-spec-plan-smoke
scripts/tui-regression.sh docs/previews/generated
scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash
```

`event::read()` grep 不是永久禁止所有 terminal event 读取；它用来提醒 review
者确认是否又在 modal、approval 或 active-turn 里新增了会接管主循环的阻塞读事件。

### 4. TDD 发布合同

所有行为变更必须按 `RED -> GREEN -> REFACTOR` 推进。这里的行为包括代码能力、TUI
交互、provider 适配、release gate、测试脚本和文档合同本身。

每个 TDD 垂直切片必须满足：

- 一个行为、一个失败测试、一个最小实现，不能先横向写一批测试再统一补实现。
- RED：先新增或调整一个可观察行为测试，运行并确认它因目标行为缺失而失败。
- GREEN：只写通过当前测试所需的最小实现，不顺手扩大范围。
- REFACTOR：所有相关测试变绿后再清理命名、重复和边界。
- 完成说明要记录红灯命令、绿灯命令、变更文件和是否需要截图证据。

发布前必须运行测试合同 smoke：

```bash
scripts/tdd-testing-contract-smoke.sh
```

该 smoke 会确认测试计划、release plan、spec review 和 `scripts/release-smoke.sh`
仍然包含 TDD gate。它不是替代业务测试，而是防止“测试环节本身”再次从发布流程里滑走。

### 5. TUI 视觉检查

所有 TUI 可见改动都必须提供视觉产物。能稳定生成时优先使用确定性 SVG 或 ANSI snapshot；
如果功能依赖真实终端行为，例如输入法位置、光标、鼠标交互或 resize，则使用真实终端截图。

运行确定性的 TUI regression 入口：

```bash
scripts/tui-regression.sh docs/previews/generated
```

TUI 功能工作必须覆盖的状态：

- main idle；
- main active / thinking / streaming；
- approval overlay；
- 涉及时的 command palette 或 slash-command suggestions；
- test result evidence；
- 涉及 lane 时的 side-1 lane view；
- 涉及 diagnostics 或 tests 时的 side-2 ops/evidence view；
- layout 改动时的紧凑和宽屏终端尺寸。

功能完成报告必须包含产物路径，并简短说明截图证明了什么。

### 6. 安全与权限检查

触及 tools、approvals、permissions、lanes、app-server integration、plugins、MCP、skills
或 workflow state 时运行：

```bash
scripts/smoke-codex-app-server-write-guard.sh
scripts/smoke-codex-app-server-protocol-fixture.sh
scripts/smoke-lane-operator-loop.sh
scripts/plan-mode-smoke.sh
```

默认预期是 fail-closed。没有有效权限决策的 mutation path 不能继续执行。
`scripts/plan-mode-smoke.sh` 会跑真实 no-TUI session，证明 Plan 模式会拦截直接
文件修改和基于 shell 的 `/test` 执行；切回 `/plan off` 后，同一写入路径才允许在审批后落盘。

### 7. Live Provider 检查

默认 CI 使用确定性 fallback tests。只有凭据和 rate limit 可用时才跑真实 provider：

```bash
scripts/release-smoke.sh --quick --deepseek
scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash
scripts/context-engine-benchmark.sh --provider deepseek --model deepseek-v4-flash --runs 3 --out-dir /tmp/viden-context-live
```

DeepSeek smoke 是真实开发场景，不是 echo test。它会生成并测试一个 Python
模块，然后在 `usage.json` 和 `summary.md` 里记录 token 使用量和 CNY 费用估算。
真实 provider 检查用于证明兼容性，不能替代确定性 fixture。

context engine release benchmark 会用同一个 disposable DeepSeek 开发场景分别跑
显式 benchmark projection mode off/on，默认每组三次，且至少三次。生产默认 runtime
行为仍然是 context-engine on；off cohort 只是 test/live benchmark override，会发送 raw
transcript history，而不是 `ContextBundle` projection。每次记录 prompt version、
provider/model、task/test result、非空 evidence hashes、input/output/cached/total
tokens、estimated/actual cost、可用时的 first-token/total latency、真实 provider
request input chars、projected context chars、raw baseline chars、context event/source
counts、retry count、compression ratio、bundle build latency 和 failure class。live
benchmark 是 billable gate，必须有 `DEEPSEEK_API_KEY`。

billable gate 前必须先跑离线 deterministic gate：

```bash
scripts/context-engine-benchmark.sh --fixtures crates/runtime/src/tests/fixtures/context-benchmark --runs 3 --out-dir /tmp/viden-context-benchmark
scripts/context-engine-benchmark-contract-smoke.sh
```

它会对缺字段、task/test success mismatch、evidence mismatch、cohort run count 或
run index 不精确、空 evidence、缺 request/projection metrics、median input-token
reduction 小于 20%、permission bypass、provider 413/context-overflow、unclassified
failure、engine-on p95 bundle build 超过 200 ms 执行 fail-closed。
通过后会生成 `summary.md`、`comparison.json`、`failure-classification.json` 和
`runs/` 下的 per-run usage JSON。

### 8. 强制发布检查

每次发布都必须通过 release gate。不能只凭临时本地命令就打 tag、发布或把版本标记为完成。

打 tag 或发布 release candidate 前，运行 prepublish gate：

```bash
scripts/release-gate.sh --version <version>
```

prepublish gate 会封装 Task 10 guard fixture checks、deterministic context-engine
fixture benchmark、完整的 `scripts/release-smoke.sh --deepseek` 和 billable DeepSeek
context-engine A/B benchmark，因此必须有 `DEEPSEEK_API_KEY`，并记录真实 DeepSeek
token/费用/耗时 summary。如果 key 不可用，发布就是 blocked；最多只能称为本地 RC，
不能算已完成发布。

需要排查底层问题时，可以直接跑：

```bash
scripts/release-smoke.sh --version <version> --deepseek
```

GitHub assets 发布并更新 Homebrew 后，运行 postpublish gate：

```bash
scripts/release-gate.sh --version <version> --phase postpublish
```

它会封装：

```bash
gh release view v<version>
scripts/release-smoke.sh --version <version> --github-release-assets --homebrew --skip-package
```

GitHub Release 和 Homebrew 是同一个发布单元。每次发布 GitHub Release，都必须把
`wikieden/homebrew-tap` 同步到相同版本后，才能认为发布完成。如果 tap 未更新，
或跳过 Homebrew 检查，release status 必须记录为未完成，而不是部分通过。

release status 文档必须记录精确 gate 命令、evidence 目录、release URL、workflow run、
assets、Homebrew 结果、真实 provider token/费用 summary 和剩余风险。

## 截图证据合同

每个用户可见功能都必须以真实使用视觉证据收尾。产品侧能检查该产物之前，功能不能算完成。

最终功能说明必须包含：

```text
Feature:
Scenario:
Command/workflow:
Artifact:
Proves:
Remaining visual risk:
```

推荐产物位置：

```text
docs/previews/generated/
/tmp/viden-<version>-*/screenshots/
```

使用稳定命名：

```text
0.1.10-<feature>-main.svg
0.1.10-<feature>-approval.svg
0.1.10-<feature>-side-1.svg
0.1.10-<feature>-side-2.svg
0.1.10-<feature>-terminal.png
```

## CI 门禁建议

### PR Fast

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 能判断 changed paths 时运行 focused crate tests
- `scripts/release-smoke.sh --quick`

### Main Full

- full workspace tests
- deterministic TUI regression snapshots
- lane operator smoke
- app-server protocol fixture
- app-server write guard
- host 平台 package smoke

### Release Full

- `scripts/release-gate.sh --version <version>`
- 支持平台 package build
- release asset upload
- sha256 validation
- 真实 DeepSeek 开发场景 smoke 与 token/费用 summary
- GitHub release inspection
- Homebrew tap update 与 fetch verification
- screenshot evidence review

## 完成规则

报告功能或版本完成前，必须确认：

- 测试通过；
- 相关安全检查通过；
- 文档已更新，或明确说明无需更新；
- 可见行为有截图证据；
- 最终报告包含 artifact path；
- 剩余风险被明确说明，而不是隐藏。
