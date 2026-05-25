# RoboCode 0.1.4 Release Status

Last updated: 2026-05-25

## Goal

Version `0.1.4` is the stability-focused TUI cockpit preview release. Its
purpose is to make the current five-phase plan usable enough for external
testing: stable TUI basics, real provider/tool flows, lane operator visibility,
and installable release artifacts.

## Phase Mapping

1. Baseline operator run: in progress.
2. P0 TUI interaction fixes: in progress.
3. Lane operator workflow hardening: pending full manual operator run.
4. Provider compatibility pass: pending DeepSeek live smoke confirmation.
5. Release candidate packaging: in progress.

## Baseline Evidence

- Workspace tests passed with `cargo test --workspace --quiet`.
- TUI previews regenerated for baseline verification with
  `scripts/tui-previews.sh /tmp/robocode-014-baseline-preview`.
- `robocode-cli --help` prints startup flags and provider list.
- `robocode-cli --version` now prints `robocode-cli 0.1.4`.
- Fallback REPL smoke passed with `/status` and `/exit`.
- Release packaging smoke passed for `aarch64-apple-darwin` with:
  `scripts/package-release.sh 0.1.4 aarch64-apple-darwin`.
- Packaged binary smoke passed after extracting
  `dist/robocode-v0.1.4-aarch64-apple-darwin.tar.gz`.
- macOS arm64 archive SHA-256:
  `747afc5cd066939f97d12180a1deaf6c608b088ccbadaf4f1e604f3d83c13fb3`.

## Changes Landed Toward 0.1.4

- Workspace package version moved from `0.1.3` to `0.1.4`.
- `Cargo.lock` package entries now resolve to `0.1.4`.
- CLI gained `--version` / `-V` for release smoke checks and issue reports.
- GitHub release workflow default tag moved to `v0.1.4`.
- README install examples now point at `v0.1.4`.
- The README system screenshot keeps its curated layout with the visible version
  updated to `0.1.4`.

## Open Findings

### P0

- No unresolved automated P0 blocker is confirmed in this baseline pass.
- DeepSeek live TUI smoke still needs credentialed manual confirmation before
  release.

### P1

- `/lane` is still a TUI/runtime command surface; the ordinary REPL returns
  `Unknown command /lane`. This matches the current architecture direction but
  should stay explicit in release notes so users do not expect lane management
  outside the cockpit yet.
- Full manual operator run is still required for resize behavior, approval modal
  clearing, IME positioning, mouse interaction, side-screen lifecycle, tmux/PTY
  log capture, and lane apply/conflict recovery.

### P2

- Full cursor-addressed terminal replay remains deferred.
- Inline conflict editing remains deferred.
- More external coding-tool templates remain demand-driven follow-up work.

## Next Gate

Before tagging `v0.1.4`, complete:

- Fallback TUI manual smoke.
- DeepSeek V4 Flash live smoke.
- One shell lane operator run.
- One tmux or PTY lane operator run where supported.
- `cargo test --workspace --quiet`.
- Release artifact build for all configured GitHub Actions targets.
