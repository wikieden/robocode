# RoboCode 0.1.29 Status - RC TUI Stability

Chinese version: [release-0.1.29-status.zh-CN.md](release-0.1.29-status.zh-CN.md)

`0.1.29` is complete. This release freezes new UI surface area and turns the
remaining P0/P1 TUI guardrails into a release-visible RC smoke gate.

## Status

- Workspace version: `0.1.29`
- Git tag: `v0.1.29`
- GitHub Release: published at
  <https://github.com/wikieden/robocode/releases/tag/v0.1.29>
- Homebrew tap: synced in `wikieden/homebrew-tap` commit `0681269`

## Implemented So Far

- Added `scripts/rc-tui-stability-smoke.sh`, a focused RC gate for TUI
  non-blocking, scrollback, repaint, provider/model setup, synthetic planning,
  LIVE WORK, and deterministic preview guardrails.
- Added `scripts/rc-tui-stability-contract-smoke.sh` so release smoke fails if
  the RC stability gate, docs, or release-smoke integration are removed.
- Integrated the RC stability gate into `scripts/release-smoke.sh`.

## P0/P1 TUI Backlog

- Known open P0: `0` from the automated RC smoke.
- Known open P1: `0` from the automated RC smoke.
- Known P2 / manual risk: Terminal and iTerm2 real-terminal screenshot
  evidence was not supplied to the automated RC run. The final 0.1.x
  zero-bug gate must either attach that evidence or explicitly record the
  remaining terminal-specific risk.

## Verification

- PASS `scripts/rc-tui-stability-contract-smoke.sh`
- PASS `bash scripts/rc-tui-stability-smoke.sh /tmp/robocode-0129-rc-tui-stability`
- RC evidence summary: `/tmp/robocode-0129-rc-tui-stability/summary.md`
- PASS `ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.29 scripts/tui-regression.sh docs/previews/generated`
- PASS `scripts/release-gate.sh --version 0.1.29 --phase prepublish --out-dir /tmp/robocode-0129-release-gate`
- Prepublish evidence: `/tmp/robocode-0129-release-gate/prepublish/summary.md`
- Structured prepublish evidence:
  `/tmp/robocode-0129-release-gate/prepublish/release-evidence.json`
- Deterministic screenshots: `docs/previews/generated/screenshots/`

## Remaining Gate

No remaining release gate for `0.1.29`.

## Release Gate

`0.1.29` is complete:

- prepublish gate passed, evidence at
  `/tmp/robocode-0129-release-gate/prepublish`;
- GitHub Release workflow run
  [`27318839422`](https://github.com/wikieden/robocode/actions/runs/27318839422)
  passed and uploaded `8` assets;
- GitHub Release `v0.1.29` is published with assets and checksums;
- Homebrew tap is synced to `0.1.29`, commit `0681269`;
- postpublish gate passed, evidence at
  `/tmp/robocode-0129-release-gate/postpublish`.

## DeepSeek Smoke Evidence

- Provider/model: `deepseek / deepseek-v4-flash`
- Scenario: `python_add_module_with_test`
- Requests: `3` ok, `0` errors
- Tokens: input `10938`, output `318`, total `11256`
- Elapsed seconds: `4`
- Estimated cost: `¥0.011574 CNY`
- Failure classification: none; smoke passed.
- Evidence:
  `/tmp/robocode-0129-release-gate/prepublish/deepseek-dev-scenario/summary.md`
