# Viden 0.1.11 状态

英文版： [release-0.1.11-status.md](release-0.1.11-status.md)

最后更新：2026-05-27

## 摘要

`0.1.11` 是 TUI Cockpit Reliability + Orchestration Foundation 版本。
版本目标见 [release-0.1.11-plan.zh-CN.md](release-0.1.11-plan.zh-CN.md)。

workspace package version 已 bump 到 `0.1.11`。本地 release-candidate、GitHub
release assets 和 Homebrew tap 发布后验证均已通过。

## 主要变化

- 主 transcript 固定区域从旧 `OPERATION CENTER` 更新为 `NOW WORKING`，继续读取统一
  `AgentTask` 投影。
- TUI preview / regression 新增 resize 后重绘和中文输入场景。
- `AgentLane` 投影落地，用于把 provider turn、Codex job、terminal lane、test/diff
  evidence 映射到 main、side-1、side-2 等屏幕。
- side-1 / side-2 状态区开始读取 `AgentLane` 投影，减少面板之间的状态不一致。
- `ContextBundle` 与 token efficiency 设计落盘：
  [context-bundle-token-efficiency.zh-CN.md](context-bundle-token-efficiency.zh-CN.md)。
- README、用户指南、模块索引、路线图和 TUI 设计文档已切到 `0.1.11` 线。

## 截图证据

确定性视觉证据：

- [主 cockpit](previews/generated/screenshots/0.1.11-tui-main.svg)
- [idle cockpit](previews/generated/screenshots/0.1.11-tui-main-idle.svg)
- [live provider turn](previews/generated/screenshots/0.1.11-tui-live-turn.svg)
- [resize 后重绘](previews/generated/screenshots/0.1.11-tui-main-resize.svg)
- [中文输入](previews/generated/screenshots/0.1.11-tui-cjk-input.svg)
- [命令面板](previews/generated/screenshots/0.1.11-tui-command-palette.svg)
- [lane detail](previews/generated/screenshots/0.1.11-tui-lane-detail.svg)
- [side-1 lane screen](previews/generated/screenshots/0.1.11-tui-side-1.svg)
- [side-2 ops screen](previews/generated/screenshots/0.1.11-tui-side-2.svg)

结构化截图证据：

```text
docs/previews/generated/tui-regression-evidence.json
```

## 本地 Release Candidate 证据

```bash
scripts/release-smoke.sh --version 0.1.11 --deepseek --out-dir /tmp/viden-0111-release-smoke-full
```

结果：

- passed: 11
- failed: 0
- skipped: 3
- evidence: `/tmp/viden-0111-release-smoke-full/release-evidence.json`

通过的检查：

- `cargo-fmt`
- `cargo-clippy`
- `viden-cli-tests`
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
dist/viden-v0.1.11-aarch64-apple-darwin.tar.gz
```

## 发布状态

`v0.1.11` 已发布：

- GitHub release: https://github.com/wikieden/viden/releases/tag/v0.1.11
- Release workflow: https://github.com/wikieden/viden/actions/runs/26514419762
- Release workflow conclusion: `success`
- Release published at: `2026-05-27T13:32:56Z`
- Release assets uploaded at: `2026-05-27T13:35:51Z` - `2026-05-27T13:35:52Z`
- Homebrew tap commit: `8046c32`

上传的 release assets：

```text
viden-v0.1.11-aarch64-apple-darwin.tar.gz
viden-v0.1.11-aarch64-apple-darwin.tar.gz.sha256
viden-v0.1.11-x86_64-apple-darwin.tar.gz
viden-v0.1.11-x86_64-apple-darwin.tar.gz.sha256
viden-v0.1.11-x86_64-pc-windows-msvc.tar.gz
viden-v0.1.11-x86_64-pc-windows-msvc.tar.gz.sha256
viden-v0.1.11-x86_64-unknown-linux-gnu.tar.gz
viden-v0.1.11-x86_64-unknown-linux-gnu.tar.gz.sha256
```

发布后验证：

```bash
scripts/release-smoke.sh --version 0.1.11 --quick --github-release-assets --homebrew --out-dir /tmp/viden-0111-postpublish-check
```

结果：

- passed: 10
- failed: 0
- skipped: 3
- evidence: `/tmp/viden-0111-postpublish-check/release-evidence.json`

发布后通过的检查：

- `cargo-fmt`
- `cargo-clippy`
- `viden-cli-terminal-tests`
- `tui-regression`
- `fallback-cli-smoke`
- `codex-app-server-protocol-fixture`
- `codex-app-server-write-guard`
- `lane-operator-loop-smoke`
- `github-release-assets-validation`
- `homebrew-validation`

Homebrew 验证确认：

```text
Formula viden (0.1.11)
wikieden/tap/viden 0.1.11
```

## 剩余风险

- 真实终端鼠标、输入法候选窗和光标闪烁仍需要人工在 macOS Terminal / iTerm2 中验收。
- ACP、MCP mutation、可安装 plugin/skill 生命周期仍是后续集成工作。
