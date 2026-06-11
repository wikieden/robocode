# RoboCode 0.1.29 状态 - RC TUI 稳定性

English version: [release-0.1.29-status.md](release-0.1.29-status.md)

`0.1.29` 正在进行。本版本冻结新的 UI surface，把剩余 P0/P1 TUI guardrails
收敛成 release-visible 的 RC smoke gate。

## 状态

- Workspace version：`0.1.29`
- Git tag：待发布
- GitHub Release：待发布
- Homebrew tap：待同步

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
- PASS `bash scripts/rc-tui-stability-smoke.sh /tmp/robocode-0129-rc-tui-stability`
- RC evidence summary：`/tmp/robocode-0129-rc-tui-stability/summary.md`
- PASS `ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.29 scripts/tui-regression.sh docs/previews/generated`
- PASS `scripts/release-gate.sh --version 0.1.29 --phase prepublish --out-dir /tmp/robocode-0129-release-gate`
- Prepublish evidence：`/tmp/robocode-0129-release-gate/prepublish/summary.md`
- Structured prepublish evidence：
  `/tmp/robocode-0129-release-gate/prepublish/release-evidence.json`
- Deterministic screenshots：`docs/previews/generated/screenshots/`

## 剩余 Gate

- 发布 GitHub Release `v0.1.29`。
- 同步 `wikieden/homebrew-tap` 到 `0.1.29`。
- 运行 `scripts/release-gate.sh --version 0.1.29 --phase postpublish --out-dir /tmp/robocode-0129-release-gate`。

## DeepSeek Smoke 证据

- Provider/model：`deepseek / deepseek-v4-flash`
- Scenario：`python_add_module_with_test`
- Requests：`3` ok，`0` errors
- Tokens：input `10938`，output `318`，total `11256`
- Elapsed seconds：`4`
- Estimated cost：`¥0.011574 CNY`
- Failure classification：无；smoke passed。
- Evidence：
  `/tmp/robocode-0129-release-gate/prepublish/deepseek-dev-scenario/summary.md`
