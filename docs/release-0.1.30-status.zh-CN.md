# RoboCode 0.1.30 状态 - 最终 Zero-Bug TUI Gate

English version: [release-0.1.30-status.md](release-0.1.30-status.md)

`0.1.30` 正在进行中。本版本是进入 0.2.x spec/context/evidence runtime 前，
0.1.x TUI 稳定性的最终收口。

## 状态

- Workspace version：`0.1.30`
- Git tag：pending
- GitHub Release：pending
- Homebrew tap：pending

## 当前已实现

- 新增 `scripts/final-zero-bug-smoke.sh`，作为最终 zero-bug gate，覆盖
  deterministic TUI evidence、RC TUI stability、plan-mode smoke、daily-loop
  smoke，以及真实 macOS Terminal/iTerm2 截图证据。
- 新增 `scripts/final-zero-bug-contract-smoke.sh`，让 final gate 集成变成
  release-visible 且 CI-safe。
- 已把 contract smoke 集成到 `scripts/release-smoke.sh`。
- 已把 final zero-bug gate 集成到 `scripts/release-gate.sh`，并在 `0.1.30`
  prepublish run 中自动执行。

## P0/P1 TUI Backlog

- Known open P0：final zero-bug smoke 结果为 `0`。
- Known open P1：final zero-bug smoke 结果为 `0`。
- Known P2 / 人工风险：本次 run 没有 release-blocking 项。
- 人工截图验收：`docs/previews/manual/0.1.30/README.zh-CN.md`

## 验证

- PASS `scripts/final-zero-bug-contract-smoke.sh`
- PASS `ROBOCODE_TUI_MANUAL_EVIDENCE_DIR=docs/previews/manual/0.1.30 scripts/final-zero-bug-smoke.sh /tmp/robocode-0130-final-zero-bug`
- PASS `ROBOCODE_TUI_MANUAL_EVIDENCE_DIR=docs/previews/manual/0.1.30 scripts/release-gate.sh --version 0.1.30 --phase prepublish --out-dir /tmp/robocode-0130-release-gate`
- Prepublish evidence：`/tmp/robocode-0130-release-gate/prepublish/summary.md`
- Structured prepublish evidence：
  `/tmp/robocode-0130-release-gate/prepublish/release-evidence.json`
- Final zero-bug evidence：
  `/tmp/robocode-0130-release-gate/prepublish/final-zero-bug/summary.md`
- Deterministic screenshots：`docs/previews/generated/screenshots/`
- Manual Terminal/iTerm2 screenshots：`docs/previews/manual/0.1.30/`
- Pending 发布 GitHub Release 和 Homebrew 同步后的 postpublish gate。

## DeepSeek Smoke 证据

- Provider/model：`deepseek / deepseek-v4-flash`
- Scenario：`python_add_module_with_test`
- Requests：`3` ok，`0` errors
- Tokens：input `10934`，output `298`，total `11232`
- Elapsed seconds：`5`
- Estimated cost：`¥0.011530 CNY`
- Failure classification：无；smoke passed。
- Evidence：
  `/tmp/robocode-0130-release-gate/prepublish/deepseek-dev-scenario/summary.md`
