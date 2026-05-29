# 测试与验证计划

英文版： [testing-validation-plan.md](testing-validation-plan.md)

最后更新：2026-05-27

## 目的

RoboCode 应该按“用户会长时间真实使用的开发者工具”来验证，而不是只按库代码的单元测试来验证。
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

### 3. TUI 视觉检查

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

### 4. 安全与权限检查

触及 tools、approvals、permissions、lanes、app-server integration、plugins、MCP、skills
或 workflow state 时运行：

```bash
scripts/smoke-codex-app-server-write-guard.sh
scripts/smoke-codex-app-server-protocol-fixture.sh
scripts/smoke-lane-operator-loop.sh
```

默认预期是 fail-closed。没有有效权限决策的 mutation path 不能继续执行。

### 5. Live Provider 检查

默认 CI 使用确定性 fallback tests。只有凭据和 rate limit 可用时才跑真实 provider：

```bash
scripts/release-smoke.sh --quick --deepseek
```

真实 provider 检查用于证明兼容性，不能替代确定性 fixture。

### 6. 发布检查

打 release candidate tag 前：

```bash
scripts/release-smoke.sh --version <version>
```

凭据可用时：

```bash
scripts/release-smoke.sh --version <version> --deepseek --github-actions
```

发布后：

```bash
gh release view v<version>
scripts/release-smoke.sh --version <version> --github-release-assets --homebrew --skip-package
```

GitHub Release 和 Homebrew 是同一个发布单元。每次发布 GitHub Release，都必须把
`wikieden/homebrew-tap` 同步到相同版本后，才能认为发布完成。如果 tap 未更新，
或跳过 Homebrew 检查，release status 必须记录为未完成，而不是部分通过。

release status 文档必须记录命令、evidence 目录、release URL、workflow run、assets、
Homebrew 结果和剩余风险。

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
/tmp/robocode-<version>-*/screenshots/
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

- 支持平台 package build
- release asset upload
- sha256 validation
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
