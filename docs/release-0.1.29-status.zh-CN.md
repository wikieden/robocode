# Viden 0.1.29 状态 - RC TUI 稳定性

English version: [release-0.1.29-status.md](release-0.1.29-status.md)

`0.1.29` 已完成。本版本冻结新的 UI surface，把剩余 P0/P1 TUI guardrails
收敛成 release-visible 的 RC smoke gate。

## 状态

- Workspace version：`0.1.29`
- Git tag：`v0.1.29`
- GitHub Release：已发布在
  <https://github.com/wikieden/viden/releases/tag/v0.1.29>
- Homebrew tap：已在 `wikieden/homebrew-tap` commit `0681269` 同步

## 当前已实现

- 新增 `scripts/rc-tui-stability-smoke.sh`，作为聚焦 TUI 非阻塞、scrollback、
  repaint、provider/model setup、synthetic planning、LIVE WORK 和 deterministic
  preview guardrails 的 RC gate。
- 新增 `scripts/rc-tui-stability-contract-smoke.sh`，如果 RC stability gate、文档
  或 release-smoke 集成被移除，release smoke 会失败。
- 已把 RC stability gate 集成到 `scripts/release-smoke.sh`。

## P0/P1 TUI Backlog

- Known open P0：自动化 RC smoke 结果为 `0`。
- Known open P1：自动化 RC smoke 结果为 `0`。
- Known P2 / 人工风险：本次自动化 RC run 未提供 macOS Terminal 和 iTerm2
  真实终端截图证据。最终 0.1.x zero-bug gate 必须补齐这类证据，或者明确记录
  剩余 terminal-specific 风险。

## 验证

- PASS `scripts/rc-tui-stability-contract-smoke.sh`
- PASS `bash scripts/rc-tui-stability-smoke.sh /tmp/viden-0129-rc-tui-stability`
- RC evidence summary：`/tmp/viden-0129-rc-tui-stability/summary.md`
- PASS `VIDEN_TUI_SCREENSHOT_VERSION=0.1.29 scripts/tui-regression.sh docs/previews/generated`
- PASS `scripts/release-gate.sh --version 0.1.29 --phase prepublish --out-dir /tmp/viden-0129-release-gate`
- Prepublish evidence：`/tmp/viden-0129-release-gate/prepublish/summary.md`
- Structured prepublish evidence：
  `/tmp/viden-0129-release-gate/prepublish/release-evidence.json`
- Deterministic screenshots：`docs/previews/generated/screenshots/`

## 剩余 Gate

`0.1.29` 已无剩余 release gate。

## Release Gate

`0.1.29` 已完成：

- prepublish gate 通过，证据在 `/tmp/viden-0129-release-gate/prepublish`；
- GitHub Release workflow run
  [`27318839422`](https://github.com/wikieden/viden/actions/runs/27318839422)
  通过，并上传 `8` 个 assets；
- GitHub Release `v0.1.29` 已发布，包含 assets 和 checksums；
- Homebrew tap 已同步到 `0.1.29`，commit `0681269`；
- postpublish gate 通过，证据在 `/tmp/viden-0129-release-gate/postpublish`。

## DeepSeek Smoke 证据

- Provider/model：`deepseek / deepseek-v4-flash`
- Scenario：`python_add_module_with_test`
- Requests：`3` ok，`0` errors
- Tokens：input `10938`，output `318`，total `11256`
- Elapsed seconds：`4`
- Estimated cost：`¥0.011574 CNY`
- Failure classification：无；smoke passed。
- Evidence：
  `/tmp/viden-0129-release-gate/prepublish/deepseek-dev-scenario/summary.md`
