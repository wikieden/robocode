# RoboCode 0.1.4 Release Status

Last updated: 2026-05-25

## Goal

Version `0.1.4` is the stability-focused TUI cockpit preview release. Its
purpose is to make the current five-phase plan usable enough for external
testing: stable TUI basics, real provider/tool flows, lane operator visibility,
and installable release artifacts.

## Phase Mapping

1. Baseline operator run: complete.
2. P0 TUI interaction fixes: complete for the 0.1.4 release gate.
3. Lane operator workflow hardening: complete for shell and tmux lane smoke.
4. Provider compatibility pass: complete for DeepSeek V4 Flash live smoke.
5. Release candidate packaging: complete for the non-upload artifact build gate.

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
- Fallback TUI smoke passed in tmux with `/lane run printf robocode-lane-smoke`,
  `/lane inspect L1`, and `/exit`; lane evidence was written under an isolated
  `/tmp/robocode-014-tui-smoke.*` workspace.
- Tmux lane smoke passed in tmux with `/lane tmux L1`; the lane wrote
  `L1.tmux.md` and a live `L1.log` under an isolated
  `/tmp/robocode-014-tmux-smoke.*` workspace.
- DeepSeek V4 Flash TUI live smoke passed with `DEEPSEEK_API_KEY` present:
  prompt `reply exactly ROBOSMOKE` produced assistant response `ROBOSMOKE` in
  both the TUI pane capture and JSONL transcript.
- GitHub Actions release artifact validation passed with
  `upload_to_release=false` for all configured targets:
  `aarch64-apple-darwin`, `x86_64-apple-darwin`,
  `x86_64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc`.
  Run: https://github.com/wikieden/robocode/actions/runs/26401318871.
- Final release workflow passed with `upload_to_release=true` and uploaded all
  configured artifacts to the GitHub release.
  Run: https://github.com/wikieden/robocode/actions/runs/26401753477.

## Changes Landed Toward 0.1.4

- Workspace package version moved from `0.1.3` to `0.1.4`.
- `Cargo.lock` package entries now resolve to `0.1.4`.
- CLI gained `--version` / `-V` for release smoke checks and issue reports.
- GitHub release workflow default tag moved to `v0.1.4`.
- README install examples now point at `v0.1.4`.
- The README system screenshot keeps its curated layout with the visible version
  updated to `0.1.4`.
- Lane log summaries now strip terminal control sequences and prompt-only noise
  before persisting or rendering, so tmux/PTY logs cannot push escape sequences
  into the cockpit layout.

## Open Findings

### P0

- No unresolved automated or live-smoke P0 blocker is confirmed.

### P1

- `/lane` is still a TUI/runtime command surface; the ordinary REPL returns
  `Unknown command /lane`. This matches the current architecture direction but
  should stay explicit in release notes so users do not expect lane management
  outside the cockpit yet.
- Full manual operator coverage for approval modal edge cases, IME positioning,
  mouse interaction, side-screen lifecycle, and lane apply/conflict recovery is
  still recommended after `0.1.4`; shell and tmux lane smoke are covered for the
  release gate.

### P2

- Full cursor-addressed terminal replay remains deferred.
- Inline conflict editing remains deferred.
- More external coding-tool templates remain demand-driven follow-up work.

## Release Outcome

`v0.1.4` is published at:

- https://github.com/wikieden/robocode/releases/tag/v0.1.4

The release contains:

- `robocode-v0.1.4-aarch64-apple-darwin.tar.gz`
- `robocode-v0.1.4-x86_64-apple-darwin.tar.gz`
- `robocode-v0.1.4-x86_64-unknown-linux-gnu.tar.gz`
- `robocode-v0.1.4-x86_64-pc-windows-msvc.tar.gz`
- matching `.sha256` files for each archive.

## Next Follow-Up

- Keep `/lane` documented as TUI-only until the product decision changes.
- Continue manual UX hardening for approval modal edge cases, IME positioning,
  mouse interaction, side-screen lifecycle, and lane apply/conflict recovery.
- Consider replacing the Windows `.tar.gz` archive with a `.zip` package in a
  future release if early testers prefer the native Windows convention.
