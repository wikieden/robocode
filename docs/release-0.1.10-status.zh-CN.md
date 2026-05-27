# RoboCode 0.1.10 状态

英文版： [release-0.1.10-status.md](release-0.1.10-status.md)

最后更新：2026-05-27

## 摘要

`0.1.10` 是 Programming Cockpit Feedback 版本。版本目标见
[release-0.1.10-plan.zh-CN.md](release-0.1.10-plan.zh-CN.md)。

workspace package version 已 bump 到 `0.1.10`。本地 release-candidate 验证已通过。
GitHub release assets 和 Homebrew tap 仍需等发布完成后验证。

## 主要变化

- TUI provider turn 在请求开始前会创建 live pending `AgentTask`。
- 主屏 operation center 和右栏会把这条 pending 请求显示成真实 runtime evidence，
  包含 provider、model 和 workspace。
- provider 返回后，pending 请求自动清除，由 approval、tool、diff、test 或 assistant
  transcript task 接管当前状态。
- `scripts/tui-regression.sh` 支持通过 `ROBOCODE_TUI_SCREENSHOT_VERSION` 生成版本化
  截图名，默认 `0.1.10`。
- README、用户指南、阶段路线图和验证文档已切到 `0.1.10` 发布线。

## 截图证据

预期持久化的确定性视觉证据：

- [主 cockpit](previews/generated/screenshots/0.1.10-tui-main.svg)
- [idle cockpit](previews/generated/screenshots/0.1.10-tui-main-idle.svg)
- [live provider turn](previews/generated/screenshots/0.1.10-tui-live-turn.svg)
- [命令面板](previews/generated/screenshots/0.1.10-tui-command-palette.svg)
- [lane detail](previews/generated/screenshots/0.1.10-tui-lane-detail.svg)
- [side-1 lane screen](previews/generated/screenshots/0.1.10-tui-side-1.svg)
- [side-2 ops screen](previews/generated/screenshots/0.1.10-tui-side-2.svg)

结构化截图证据：

```text
docs/previews/generated/tui-regression-evidence.json
```

## 本地 Release Candidate 证据

```bash
scripts/release-smoke.sh --version 0.1.10 --deepseek --out-dir /tmp/robocode-0110-release-smoke-full
```

结果：

- passed: 11
- failed: 0
- skipped: 3
- evidence: `/tmp/robocode-0110-release-smoke-full/release-evidence.json`

通过的检查：

- `cargo-fmt`
- `cargo-clippy`
- `robocode-cli-tests`
- `workspace-tests`
- `tui-regression`
- `fallback-cli-smoke`
- `codex-app-server-protocol-fixture`
- `codex-app-server-write-guard`
- `lane-operator-loop-smoke`
- `package-smoke`
- `deepseek-cli-smoke`

Package smoke 生成：

```text
dist/robocode-v0.1.10-aarch64-apple-darwin.tar.gz
```

## 发布状态

待完成：

- GitHub release: `v0.1.10`
- Release workflow
- 多平台 release assets
- Homebrew tap formula
- 发布后验证

## 剩余风险

- ACP、MCP mutation、可安装 plugin/skill 生命周期仍是后续集成工作。
- 确定性截图可以证明布局回归，但真实 terminal 光标闪烁、输入法位置和鼠标行为仍需要人工
  终端验证。
