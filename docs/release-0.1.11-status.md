# RoboCode 0.1.11 Status

Chinese version: [release-0.1.11-status.zh-CN.md](release-0.1.11-status.zh-CN.md)

Last updated: 2026-05-27

## Summary

`0.1.11` is the TUI Cockpit Reliability + Orchestration Foundation release.
The version target is documented in [release-0.1.11-plan.md](release-0.1.11-plan.md).

Workspace package version has been bumped to `0.1.11`. Local
release-candidate, GitHub release asset, and Homebrew tap post-publish
validation have all passed.

## Main Changes

- The fixed main-transcript band is now `NOW WORKING` instead of the older
  `OPERATION CENTER`, while still reading from the shared `AgentTask`
  projection.
- TUI preview / regression coverage now includes resized redraw and CJK input
  scenarios.
- `AgentLane` projection has landed to map provider turns, Codex jobs,
  terminal lanes, test/diff evidence, and related work onto main, side-1, and
  side-2 screens.
- side-1 / side-2 status areas now read the `AgentLane` projection to reduce
  inconsistent status between panels.
- `ContextBundle` and token-efficiency design is documented in
  [context-bundle-token-efficiency.md](context-bundle-token-efficiency.md).
- README, user guide, module index, staged roadmap, and TUI design docs now
  reference the `0.1.11` line.

## Screenshot Evidence

Deterministic visual evidence:

- [main cockpit](previews/generated/screenshots/0.1.11-tui-main.svg)
- [idle cockpit](previews/generated/screenshots/0.1.11-tui-main-idle.svg)
- [live provider turn](previews/generated/screenshots/0.1.11-tui-live-turn.svg)
- [resized redraw](previews/generated/screenshots/0.1.11-tui-main-resize.svg)
- [CJK input](previews/generated/screenshots/0.1.11-tui-cjk-input.svg)
- [command palette](previews/generated/screenshots/0.1.11-tui-command-palette.svg)
- [lane detail](previews/generated/screenshots/0.1.11-tui-lane-detail.svg)
- [side-1 lane screen](previews/generated/screenshots/0.1.11-tui-side-1.svg)
- [side-2 ops screen](previews/generated/screenshots/0.1.11-tui-side-2.svg)

Structured screenshot evidence:

```text
docs/previews/generated/tui-regression-evidence.json
```

## Local Release Candidate Evidence

```bash
scripts/release-smoke.sh --version 0.1.11 --deepseek --out-dir /tmp/robocode-0111-release-smoke-full
```

Result:

- passed: 11
- failed: 0
- skipped: 3
- evidence: `/tmp/robocode-0111-release-smoke-full/release-evidence.json`

Passing checks:

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

Package smoke generated:

```text
dist/robocode-v0.1.11-aarch64-apple-darwin.tar.gz
```

## Publish State

`v0.1.11` is published:

- GitHub release: https://github.com/wikieden/robocode/releases/tag/v0.1.11
- Release workflow: https://github.com/wikieden/robocode/actions/runs/26514419762
- Release workflow conclusion: `success`
- Release published at: `2026-05-27T13:32:56Z`
- Release assets uploaded at: `2026-05-27T13:35:51Z` - `2026-05-27T13:35:52Z`
- Homebrew tap commit: `8046c32`

Uploaded release assets:

```text
robocode-v0.1.11-aarch64-apple-darwin.tar.gz
robocode-v0.1.11-aarch64-apple-darwin.tar.gz.sha256
robocode-v0.1.11-x86_64-apple-darwin.tar.gz
robocode-v0.1.11-x86_64-apple-darwin.tar.gz.sha256
robocode-v0.1.11-x86_64-pc-windows-msvc.tar.gz
robocode-v0.1.11-x86_64-pc-windows-msvc.tar.gz.sha256
robocode-v0.1.11-x86_64-unknown-linux-gnu.tar.gz
robocode-v0.1.11-x86_64-unknown-linux-gnu.tar.gz.sha256
```

Post-publish validation:

```bash
scripts/release-smoke.sh --version 0.1.11 --quick --github-release-assets --homebrew --out-dir /tmp/robocode-0111-postpublish-check
```

Result:

- passed: 10
- failed: 0
- skipped: 3
- evidence: `/tmp/robocode-0111-postpublish-check/release-evidence.json`

Passing post-publish checks:

- `cargo-fmt`
- `cargo-clippy`
- `robocode-cli-terminal-tests`
- `tui-regression`
- `fallback-cli-smoke`
- `codex-app-server-protocol-fixture`
- `codex-app-server-write-guard`
- `lane-operator-loop-smoke`
- `github-release-assets-validation`
- `homebrew-validation`

Homebrew validation confirmed:

```text
Formula robocode (0.1.11)
wikieden/tap/robocode 0.1.11
```

## Remaining Risk

- Real-terminal mouse behavior, IME candidate placement, and cursor blinking
  still need manual acceptance in macOS Terminal / iTerm2.
- ACP, MCP mutation, and installable plugin/skill lifecycles remain follow-up
  integration work.
