# Viden 0.1.10 Status

Chinese version: [release-0.1.10-status.zh-CN.md](release-0.1.10-status.zh-CN.md)

Last updated: 2026-05-27

## Summary

`0.1.10` is the Programming Cockpit Feedback release. The version target is
documented in [release-0.1.10-plan.md](release-0.1.10-plan.md).

Workspace package version has been bumped to `0.1.10`. Local release-candidate
validation, GitHub release publication, release assets, and Homebrew tap
verification have all passed.

## Changes

- TUI provider turns now create a live pending `AgentTask` before the provider
  request starts.
- The operation center and right rail surface that pending request as real
  runtime evidence, including provider, model, and workspace.
- The pending request clears when the provider returns, allowing approval, tool,
  diff, test, and assistant transcript tasks to become the source of truth.
- `scripts/tui-regression.sh` now supports versioned screenshot names through
  `VIDEN_TUI_SCREENSHOT_VERSION`, defaulting to `0.1.10`.
- README, user guide, staged roadmap, and validation docs now point at the
  `0.1.10` release line.

## Screenshot Evidence

Expected deterministic visual evidence:

- [main cockpit](previews/generated/screenshots/0.1.10-tui-main.svg)
- [idle cockpit](previews/generated/screenshots/0.1.10-tui-main-idle.svg)
- [live provider turn](previews/generated/screenshots/0.1.10-tui-live-turn.svg)
- [command palette](previews/generated/screenshots/0.1.10-tui-command-palette.svg)
- [lane detail](previews/generated/screenshots/0.1.10-tui-lane-detail.svg)
- [side-1 lane screen](previews/generated/screenshots/0.1.10-tui-side-1.svg)
- [side-2 ops screen](previews/generated/screenshots/0.1.10-tui-side-2.svg)

Structured screenshot evidence:

```text
docs/previews/generated/tui-regression-evidence.json
```

## Local Release Candidate Evidence

```bash
scripts/release-smoke.sh --version 0.1.10 --deepseek --out-dir /tmp/viden-0110-release-smoke-full
```

Result:

- passed: 11
- failed: 0
- skipped: 3
- evidence: `/tmp/viden-0110-release-smoke-full/release-evidence.json`

Passed checks:

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

Package smoke generated:

```text
dist/viden-v0.1.10-aarch64-apple-darwin.tar.gz
```

## Publish Status

Published and verified.

- GitHub release:
  [v0.1.10](https://github.com/wikieden/viden/releases/tag/v0.1.10)
- Release workflow:
  [run 26507982488](https://github.com/wikieden/viden/actions/runs/26507982488)
- Workflow status: `completed` / `success`
- Release published at: `2026-05-27T11:18:45Z`
- Main release commit: `7c7fcad`
- Homebrew tap commit: `bffa27e`

Uploaded release assets:

- `viden-v0.1.10-aarch64-apple-darwin.tar.gz`
- `viden-v0.1.10-aarch64-apple-darwin.tar.gz.sha256`
- `viden-v0.1.10-x86_64-apple-darwin.tar.gz`
- `viden-v0.1.10-x86_64-apple-darwin.tar.gz.sha256`
- `viden-v0.1.10-x86_64-pc-windows-msvc.tar.gz`
- `viden-v0.1.10-x86_64-pc-windows-msvc.tar.gz.sha256`
- `viden-v0.1.10-x86_64-unknown-linux-gnu.tar.gz`
- `viden-v0.1.10-x86_64-unknown-linux-gnu.tar.gz.sha256`

Post-publish verification:

```bash
scripts/release-smoke.sh --version 0.1.10 --quick --github-release-assets --homebrew --out-dir /tmp/viden-0110-postpublish-check
```

Result:

- passed: 10
- failed: 0
- skipped: 3
- evidence: `/tmp/viden-0110-postpublish-check/release-evidence.json`
- Homebrew formula: `wikieden/tap/viden 0.1.10`

## Remaining Risks

- No release-blocking risks remain for `0.1.10`.
- ACP, MCP mutation, and installable plugin/skill lifecycle remain deferred
  integration work.
- Deterministic screenshots prove layout regressions, but true terminal cursor
  blink, IME position, and mouse behavior still require manual terminal review.
