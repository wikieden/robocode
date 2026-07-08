# Viden 0.1.12 状态

英文版： [release-0.1.12-status.md](release-0.1.12-status.md)

最后更新：2026-05-27

## 摘要

`0.1.12` 是 Agent Orchestration Operator Loop 版本。版本目标见
[release-0.1.12-plan.zh-CN.md](release-0.1.12-plan.zh-CN.md)。

workspace package version 已 bump 到 `0.1.12`。本地 release-candidate 验证已通过：
11 项 green，包括 DeepSeek smoke 和 deterministic lane operator loop smoke。

## 主要变化

- 在 `viden-types` 新增共享 `AgentTaskRecord`、`AgentTaskStatus`、
  `AgentTaskEvidence`、`AgentNextAction`、`AgentLaneRecord` 和 ContextBundle
  record。
- 新增 `SessionEngine::agent_task_snapshot()`，并让 provider turn、tool call、
  permission approval wait、tool result、`/test` command 写入 runtime task snapshot。
- TUI `NOW WORKING`、right rail、side-1、side-2 使用同一 `AgentTaskRecord`
  形状，同时保留 transcript fallback projection。
- `/lane run` 和 `/lane ask <tool> <task>` 的 lane envelope 已写入 ContextBundle v0
  sources、token estimate、pressure、largest sources、compaction notes 和预算元数据。
- 新增 `/lane retry <id>`，失败或 blocked lane 可直接重排，不必手工重构任务。
- side-2 ops evidence 在存在 lane envelope 时展示 context pressure。
- README、用户指南、模块索引、阶段路线图和截图证据已切到 `0.1.12` 线。

## 截图证据

确定性视觉证据：

- [主 cockpit](previews/generated/screenshots/0.1.12-tui-main.svg)
- [idle cockpit](previews/generated/screenshots/0.1.12-tui-main-idle.svg)
- [live provider turn](previews/generated/screenshots/0.1.12-tui-live-turn.svg)
- [resize 后重绘](previews/generated/screenshots/0.1.12-tui-main-resize.svg)
- [中文输入](previews/generated/screenshots/0.1.12-tui-cjk-input.svg)
- [命令面板](previews/generated/screenshots/0.1.12-tui-command-palette.svg)
- [lane detail](previews/generated/screenshots/0.1.12-tui-lane-detail.svg)
- [side-1 lane screen](previews/generated/screenshots/0.1.12-tui-side-1.svg)
- [side-2 ops screen](previews/generated/screenshots/0.1.12-tui-side-2.svg)

结构化截图证据：

```text
docs/previews/generated/tui-regression-evidence.json
```

## 本地验证

聚焦实现检查已通过：

```bash
cargo test -p viden-core -p viden-cli -p viden-types --quiet
```

结果：

- `viden-cli`: 203 passed, 0 failed
- `viden-core`: 93 passed, 0 failed
- `viden-types`: 6 passed, 0 failed

截图回归已通过：

```bash
scripts/tui-regression.sh docs/previews/generated
```

## Release Candidate 证据

```bash
scripts/release-smoke.sh --version 0.1.12 --deepseek --out-dir /tmp/viden-0112-release-smoke-full
```

结果：

- passed: 11
- failed: 0
- skipped: 3
- evidence: `/tmp/viden-0112-release-smoke-full/release-evidence.json`

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
dist/viden-v0.1.12-aarch64-apple-darwin.tar.gz
```

发布后验证目标：

```bash
scripts/release-smoke.sh --version 0.1.12 --quick --github-release-assets --homebrew --out-dir /tmp/viden-0112-postpublish-check
```

结果：

- passed: 10
- failed: 0
- skipped: 3
- evidence: `/tmp/viden-0112-postpublish-check/release-evidence.json`

## 发布状态

`v0.1.12` 已发布：

- GitHub release: https://github.com/wikieden/viden/releases/tag/v0.1.12
- Release workflow: https://github.com/wikieden/viden/actions/runs/26518796829
- Release workflow conclusion: `success`
- Release published at: `2026-05-27T14:49:11Z`
- Release assets uploaded at: `2026-05-27T14:51:33Z` - `2026-05-27T14:51:35Z`
- Homebrew tap commit: `3cb201c`

发布资产：

```text
viden-v0.1.12-aarch64-apple-darwin.tar.gz
viden-v0.1.12-aarch64-apple-darwin.tar.gz.sha256
viden-v0.1.12-x86_64-apple-darwin.tar.gz
viden-v0.1.12-x86_64-apple-darwin.tar.gz.sha256
viden-v0.1.12-x86_64-pc-windows-msvc.tar.gz
viden-v0.1.12-x86_64-pc-windows-msvc.tar.gz.sha256
viden-v0.1.12-x86_64-unknown-linux-gnu.tar.gz
viden-v0.1.12-x86_64-unknown-linux-gnu.tar.gz.sha256
```

## 剩余风险

- Codex/Claude adapter 已复用共享 AgentTask/lane 形状，但完整 happy path 不作为
  `0.1.12` release blocker。
- ContextBundle v0 只接入 lane envelope；主 provider prompt 路径仍只记录 context
  pressure 可见性，尚未用 bundle 改写 prompt construction。
- MCP、skills、plugins、ACP 暂停在 descriptor/doctor/probe/capability/event mapping
  深度，尚未进入通用 mutating plugin runtime。
