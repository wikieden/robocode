# RoboCode 0.1.9 Status

Chinese version: [release-0.1.9-status.zh-CN.md](release-0.1.9-status.zh-CN.md)

Last updated: 2026-05-27

## Summary

`0.1.9` is the Verification Hardening + Screenshot-Gated UX release. The
version target is documented in [release-0.1.9-plan.md](release-0.1.9-plan.md).

Workspace package version has been bumped to `0.1.9`. Local release-candidate
validation passed, including clippy-as-gate, full workspace tests, TUI
regression screenshots, package smoke, and DeepSeek live smoke. The GitHub
release, multi-platform assets, and Homebrew tap are now published and verified.

## What Changed

- `scripts/release-smoke.sh` is now the canonical release gate and writes
  structured `release-evidence.json`.
- The release gate now includes `cargo clippy --workspace --all-targets --
  -D warnings`.
- Post-publication checks are exposed through opt-in
  `--github-release-assets` and `--homebrew` flags.
- GitHub release metadata validation now retries transient API fetch failures
  before failing the release gate.
- `scripts/tui-regression.sh` exports deterministic screenshot artifacts for
  product review.
- TUI preview evidence is refreshed for `0.1.9`, and the README system
  screenshot now uses the generated main cockpit SVG.
- CI PR/main checks now use the quick release gate instead of separate ad-hoc
  build/test commands.
- Existing clippy warnings across shared crates, model parsing, core runtime,
  config, session, LSP, and TUI code were cleaned up so clippy can be enforced
  as a release blocker.

## Screenshot Evidence

Persisted visual evidence:

- [main cockpit](previews/generated/screenshots/0.1.9-tui-main.svg)
- [idle cockpit](previews/generated/screenshots/0.1.9-tui-main-idle.svg)
- [command palette](previews/generated/screenshots/0.1.9-tui-command-palette.svg)
- [lane detail](previews/generated/screenshots/0.1.9-tui-lane-detail.svg)
- [side-1 lane screen](previews/generated/screenshots/0.1.9-tui-side-1.svg)
- [side-2 ops screen](previews/generated/screenshots/0.1.9-tui-side-2.svg)

Structured screenshot evidence:

```text
docs/previews/generated/tui-regression-evidence.json
```

## Local Release Candidate Evidence

Full local release smoke:

```bash
scripts/release-smoke.sh --version 0.1.9 --deepseek --out-dir /tmp/robocode-019-release-smoke-full
```

Result:

- passed: 11
- failed: 0
- skipped: 3
- evidence: `/tmp/robocode-019-release-smoke-full/release-evidence.json`

Passed checks:

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

Package smoke produced:

```text
dist/robocode-v0.1.9-aarch64-apple-darwin.tar.gz
```

The extracted binary reported:

```text
robocode-cli 0.1.9
```

## Publication Status

Published and verified.

- GitHub release:
  [v0.1.9](https://github.com/wikieden/robocode/releases/tag/v0.1.9)
- Release workflow:
  [run 26496859443](https://github.com/wikieden/robocode/actions/runs/26496859443)
- Workflow status: `completed` / `success`
- Release published at: `2026-05-27T07:19:05Z`
- Main release commit: `99b3957`
- Homebrew tap commit: `6796da2`

Uploaded release assets:

- `robocode-v0.1.9-aarch64-apple-darwin.tar.gz`
- `robocode-v0.1.9-aarch64-apple-darwin.tar.gz.sha256`
- `robocode-v0.1.9-x86_64-apple-darwin.tar.gz`
- `robocode-v0.1.9-x86_64-apple-darwin.tar.gz.sha256`
- `robocode-v0.1.9-x86_64-pc-windows-msvc.tar.gz`
- `robocode-v0.1.9-x86_64-pc-windows-msvc.tar.gz.sha256`
- `robocode-v0.1.9-x86_64-unknown-linux-gnu.tar.gz`
- `robocode-v0.1.9-x86_64-unknown-linux-gnu.tar.gz.sha256`

Post-publication validation:

```bash
scripts/release-smoke.sh --version 0.1.9 --quick --github-release-assets --homebrew --out-dir /tmp/robocode-019-postpublish-check
```

Result:

- passed: 10
- failed: 0
- skipped: 3
- evidence: `/tmp/robocode-019-postpublish-check/release-evidence.json`
- Homebrew formula: `wikieden/tap/robocode 0.1.9`

## Remaining Risks

- No release-blocking risks remain for `0.1.9`.
- The screenshot gate currently uses deterministic preview SVGs. IME placement,
  live cursor blink, and mouse behavior still require occasional real terminal
  screenshots during feature-specific UI work.
- Codex app-server write-capable turns remain behind the experimental guard.
