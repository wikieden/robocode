# Viden 0.1.7 Release Status

Chinese version: [release-0.1.7-status.zh-CN.md](release-0.1.7-status.zh-CN.md)

Last updated: 2026-05-26

## Goal

Version `0.1.7` is the Codex Adapter and Agent Orchestration Backbone
release. Its purpose is to make Viden feel less like a terminal launcher and
more like a local host cockpit: Codex becomes the first protocol-aware delegate
agent, the main TUI shows live work state, and side screens expose lane,
extension, MCP, and evidence state.

## Phase Mapping

1. Live operation center: local implementation landed.
2. Codex job runtime: local implementation landed for CLI jobs and
   experimental app-server jobs.
3. Agent evidence in the cockpit: local implementation landed.
4. Extension and MCP diagnostics: local implementation landed.
5. ACP direction: protocol handshake/probe remains experimental and documented.
6. Release packaging and external publication: GitHub release and Homebrew tap
   validation passed.

## Candidate Evidence

- Workspace package version moved from `0.1.6` to `0.1.7`.
- `Cargo.lock` package entries now resolve to `0.1.7`.
- README install examples and release workflow defaults now point to `v0.1.7`.
- The 0.1.7 plan was the release core for Host-Delegate Agent Bridge, Codex
  Adapter, live operation center, extension diagnostics, and the ACP adapter
  spike. The active follow-up plan is `docs/release-0.1.8-plan.md`.
- `/agent doctor codex` checks command availability, version, app-server
  support, auth status, config sources, and job-store path.
- `/agent review codex`, `/agent challenge codex`, and
  `/agent run codex [--write] <task>` create tracked Codex job records and
  artifacts under `.viden/agents/`.
- `/agent status`, `/agent result <id>`, and `/agent cancel <id>` expose the
  tracked Codex job lifecycle.
- The TUI `OPERATION CENTER` is fixed at the top of the transcript and labels
  the evidence source for provider turns, approvals, lanes, tool calls, and
  Codex jobs.
- TUI Codex job snapshots extract app-server result/log evidence such as
  thread ID, turn ID, turn status, and approval requests.
- `/extensions doctor` and `/mcp doctor` report readiness by surface, including
  provider plugin dirs, MCP config files, skill roots, and permission boundary
  reminders.
- Default `cargo test --workspace --quiet` passes after stabilizing
  subprocess-backed Codex, ACP, and lane tests.
- Full local release smoke with DeepSeek live provider validation passed:
  `scripts/release-smoke.sh --version 0.1.7 --deepseek --out-dir /tmp/viden-017-release-smoke-deepseek-local-2`.
- Evidence directory:
  `/tmp/viden-017-release-smoke-deepseek-local-2`.
- The smoke matrix passed `cargo-fmt`, `viden-cli-tests`,
  `workspace-tests`, `tui-previews`, `fallback-cli-smoke`,
  `shell-lane-smoke`, `tmux-lane-smoke`, `package-smoke`, and
  `deepseek-cli-smoke`.
- DeepSeek V4 Flash live smoke passed; the transcript contains
  `viden-deepseek-smoke-ok`.
- Host package smoke passed for `aarch64-apple-darwin`; the extracted binary
  prints `viden-cli 0.1.7`.
- macOS arm64 archive SHA-256:
  `c9a17d5d4d3d36824616505a3abde659a6db173fffa21c22b3f60b83d988d1a2`.
- GitHub Actions release artifact validation passed with
  `upload_to_release=false` for all configured targets:
  `aarch64-apple-darwin`, `x86_64-apple-darwin`,
  `x86_64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc`.
  Run: https://github.com/wikieden/viden/actions/runs/26449257109.
- The final GitHub release workflow passed with `upload_to_release=true` and
  uploaded all configured artifacts.
  Run: https://github.com/wikieden/viden/actions/runs/26449437778.
- Homebrew tap `wikieden/homebrew-tap` now points Viden formula URLs and
  SHA-256 values at `v0.1.7`.
  Commit: https://github.com/wikieden/homebrew-tap/commit/8e84a89.
- Homebrew fetch smoke passed:
  `brew fetch --force wikieden/tap/viden` reported
  `Formula viden (0.1.7)`.

## Validation Gates

All planned `0.1.7` validation gates have passed:

- `cargo fmt --check`
- `git diff --check`
- `cargo test --workspace --quiet`
- `scripts/release-smoke.sh --version 0.1.7 --deepseek`
- GitHub Actions release artifact validation with `upload_to_release=false`
- final GitHub release artifact upload with `upload_to_release=true`
- Homebrew tap update and fetch smoke

## Open Findings

### P0

- None known for local source validation or local release smoke.
- None for the `0.1.7` release.

### P1

- The app-server task path is still experimental and should remain opt-in until
  live smoke proves normal jobs can safely default to the protocol path.
- Full ACP editing remains a follow-up; 0.1.7 keeps the protocol boundary and
  evidence model visible.

### P2

- Automatic task splitting remains deferred.
- Full cursor-addressed terminal replay remains deferred.
- More external coding-agent templates remain demand-driven follow-up work.

## Release Outcome

`v0.1.7` is published at:

- https://github.com/wikieden/viden/releases/tag/v0.1.7

The release contains:

- `viden-v0.1.7-aarch64-apple-darwin.tar.gz`
- `viden-v0.1.7-x86_64-apple-darwin.tar.gz`
- `viden-v0.1.7-x86_64-unknown-linux-gnu.tar.gz`
- `viden-v0.1.7-x86_64-pc-windows-msvc.tar.gz`
- matching `.sha256` files for each archive.

GitHub reported asset digests:

- `aarch64-apple-darwin`:
  `sha256:ec000b139ede57d27035e9ba2ed95f111e3f6d0e40fe2c2c648b63d6fbf7a2a9`
- `x86_64-apple-darwin`:
  `sha256:a50ceac337ffad807bb4ae6935ff5177a25f36e098eee10a91e0c1b9ce3b86bc`
- `x86_64-unknown-linux-gnu`:
  `sha256:758a38f4ef1a217e02b77647aa2ee22e049ce7bd214c55dbd8cd4e9b606065ae`
- `x86_64-pc-windows-msvc`:
  `sha256:f2c8e9d2247dd61cc81bd21d21ea48f92120d683e3e4bbfade9bb534e365b581`

Homebrew install path:

```bash
brew tap wikieden/tap
brew install viden
```
