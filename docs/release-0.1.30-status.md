# RoboCode 0.1.30 Status - Final Zero-Bug TUI Gate

Chinese version: [release-0.1.30-status.zh-CN.md](release-0.1.30-status.zh-CN.md)

`0.1.30` is in progress. This release is the final 0.1.x TUI stability closure
before the project moves into 0.2.x spec/context/evidence runtime work.

## Status

- Workspace version: `0.1.30`
- Git tag: pending
- GitHub Release: pending
- Homebrew tap: pending

## Implemented So Far

- Added `scripts/final-zero-bug-smoke.sh`, the final zero-bug gate for
  deterministic TUI evidence, RC TUI stability, plan-mode smoke, daily-loop
  smoke, and real macOS Terminal/iTerm2 screenshot evidence.
- Added `scripts/final-zero-bug-contract-smoke.sh` to make the final gate
  integration release-visible and CI-safe.
- Integrated the contract smoke into `scripts/release-smoke.sh`.
- Integrated the final zero-bug gate into `scripts/release-gate.sh` for
  `0.1.30` prepublish runs.

## P0/P1 TUI Backlog

- Known open P0: `0` from the final zero-bug smoke.
- Known open P1: `0` from the final zero-bug smoke.
- Known P2 / manual risk: none release-blocking in this run.
- Manual screenshot acceptance:
  `docs/previews/manual/0.1.30/README.md`

## Verification

- PASS `scripts/final-zero-bug-contract-smoke.sh`
- PASS `ROBOCODE_TUI_MANUAL_EVIDENCE_DIR=docs/previews/manual/0.1.30 scripts/final-zero-bug-smoke.sh /tmp/robocode-0130-final-zero-bug`
- PASS `ROBOCODE_TUI_MANUAL_EVIDENCE_DIR=docs/previews/manual/0.1.30 scripts/release-gate.sh --version 0.1.30 --phase prepublish --out-dir /tmp/robocode-0130-release-gate`
- Prepublish evidence:
  `/tmp/robocode-0130-release-gate/prepublish/summary.md`
- Structured prepublish evidence:
  `/tmp/robocode-0130-release-gate/prepublish/release-evidence.json`
- Final zero-bug evidence:
  `/tmp/robocode-0130-release-gate/prepublish/final-zero-bug/summary.md`
- Deterministic screenshots: `docs/previews/generated/screenshots/`
- Manual Terminal/iTerm2 screenshots: `docs/previews/manual/0.1.30/`
- Pending postpublish gate after GitHub Release and Homebrew sync.

## DeepSeek Smoke Evidence

- Provider/model: `deepseek / deepseek-v4-flash`
- Scenario: `python_add_module_with_test`
- Requests: `3` ok, `0` errors
- Tokens: input `10934`, output `298`, total `11232`
- Elapsed seconds: `5`
- Estimated cost: `¥0.011530 CNY`
- Failure classification: none; smoke passed.
- Evidence:
  `/tmp/robocode-0130-release-gate/prepublish/deepseek-dev-scenario/summary.md`
