# Viden 0.1.5 Release Status

Last updated: 2026-05-26

## Goal

Version `0.1.5` is the programming-experience release. Its purpose is to make
the TUI cockpit feel usable for the everyday coding loop: understand the task,
approve mutations, inspect diff and test evidence, supervise lanes, and produce
installable release artifacts.

## Phase Mapping

1. TUI interaction stability: local smoke passed.
2. Coding loop evidence: local smoke passed.
3. Lane operator workflow: local smoke passed for shell and tmux lane gates.
4. Provider compatibility: DeepSeek V4 Flash live smoke passed.
5. Release artifact validation: GitHub Actions artifact run passed.

## Candidate Evidence

- Workspace package version moved from `0.1.4` to `0.1.5`.
- `Cargo.lock` package entries now resolve to `0.1.5`.
- GitHub release workflow default tag moved to `v0.1.5`.
- README install examples now point at `v0.1.5`.
- The README system screenshot keeps its curated layout with the visible
  version updated to `0.1.5`.
- Local release smoke is scripted through `scripts/release-smoke.sh`; the
  script captures logs, generated TUI previews, fallback CLI smoke, lane smoke,
  and host package smoke in one evidence directory.
- Full local release smoke with DeepSeek live provider validation passed:
  `scripts/release-smoke.sh --version 0.1.5 --deepseek --out-dir /tmp/viden-015-release-smoke-deepseek-local`.
- Evidence directory:
  `/tmp/viden-015-release-smoke-deepseek-local`.
- DeepSeek V4 Flash live smoke passed; the transcript contains
  `viden-deepseek-smoke-ok`.
- Host package smoke passed for `aarch64-apple-darwin`; the extracted binary
  prints `viden-cli 0.1.5`.
- macOS arm64 archive SHA-256:
  `734fe4a266178946b871e10a847ec8ac1f50642e270f708d8446fe5a81315e78`.
- GitHub Actions release artifact validation passed with
  `upload_to_release=false` for all configured targets:
  `aarch64-apple-darwin`, `x86_64-apple-darwin`,
  `x86_64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc`.
  Run: https://github.com/wikieden/viden/actions/runs/26430970204.
- Homebrew tap `wikieden/homebrew-tap` now points Viden formula URLs and
  SHA-256 values at `v0.1.5`.
  Commit: https://github.com/wikieden/homebrew-tap/commit/8faa918.
- Homebrew fetch smoke passed after refreshing the local tap:
  `brew fetch --force wikieden/tap/viden` reported `Formula viden (0.1.5)`.

## Validation Gates

Before publishing `v0.1.5`, run after pushing the version bump to `main`:

```bash
scripts/release-smoke.sh --version 0.1.5 --skip-package --deepseek --github-actions
```

The final status update should record:

- the final release workflow run URL after upload;
- the published release URL and artifact list.

## Open Findings

### P0

- Continue watching visual alignment regressions in the right rail and side
  screens; frame glyphs now use stable frame coloring independent of row
  semantic highlights.

### P1

- `/lane` remains a TUI/runtime command surface; the ordinary REPL still treats
  `/lane` as unknown. This is intentional for `0.1.5` and should remain explicit
  in release notes.

### P2

- Full cursor-addressed terminal replay remains deferred.
- Inline conflict editing remains deferred.
- More external coding-tool templates remain demand-driven follow-up work.

## Release Outcome

`v0.1.5` is published at:

- https://github.com/wikieden/viden/releases/tag/v0.1.5

The final release workflow passed with `upload_to_release=true` and uploaded
all configured artifacts to the GitHub release.
Run: https://github.com/wikieden/viden/actions/runs/26431142668.

The release contains:

- `viden-v0.1.5-aarch64-apple-darwin.tar.gz`
- `viden-v0.1.5-x86_64-apple-darwin.tar.gz`
- `viden-v0.1.5-x86_64-unknown-linux-gnu.tar.gz`
- `viden-v0.1.5-x86_64-pc-windows-msvc.tar.gz`
- matching `.sha256` files for each archive.

GitHub reported asset digests:

- `aarch64-apple-darwin`: `sha256:a45984fd9f8907fd1379e22a3fa1fe1123a58a05b26c149cd05466b1a5c18026`
- `x86_64-apple-darwin`: `sha256:cd2d5cd03ee0e3db67347d89f41cfac616620c26a897d033c34f8816d9448c3e`
- `x86_64-unknown-linux-gnu`: `sha256:197013fd0648e993b60192b0567ca5385798996f4df2bb40f83e4ca090a3b38f`
- `x86_64-pc-windows-msvc`: `sha256:eab9c08586b4e52f303560924a23bb7bfdcc0701029e0ef8dfe3fbc64ba3bb2c`

Homebrew install path:

```bash
brew tap wikieden/tap
brew install viden
```
