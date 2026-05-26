# RoboCode 0.1.7 Release Status

Chinese version: [release-0.1.7-status.zh-CN.md](release-0.1.7-status.zh-CN.md)

Last updated: 2026-05-26

## Goal

Version `0.1.7` is the Codex Adapter and Agent Orchestration Backbone
release. Its purpose is to make RoboCode feel less like a terminal launcher and
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
6. Release packaging and external publication: pending final smoke and release
   workflow.

## Candidate Evidence

- Workspace package version moved from `0.1.6` to `0.1.7`.
- `Cargo.lock` package entries now resolve to `0.1.7`.
- README install examples and release workflow defaults now point to `v0.1.7`.
- The 0.1.7 plan is the active next-iteration core:
  Host-Delegate Agent Bridge, Codex Adapter, live operation center, extension
  diagnostics, and ACP adapter spike.
- `/agent doctor codex` checks command availability, version, app-server
  support, auth status, config sources, and job-store path.
- `/agent review codex`, `/agent challenge codex`, and
  `/agent run codex [--write] <task>` create tracked Codex job records and
  artifacts under `.robocode/agents/`.
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
  `scripts/release-smoke.sh --version 0.1.7 --deepseek --out-dir /tmp/robocode-017-release-smoke-deepseek-local-2`.
- Evidence directory:
  `/tmp/robocode-017-release-smoke-deepseek-local-2`.
- The smoke matrix passed `cargo-fmt`, `robocode-cli-tests`,
  `workspace-tests`, `tui-previews`, `fallback-cli-smoke`,
  `shell-lane-smoke`, `tmux-lane-smoke`, `package-smoke`, and
  `deepseek-cli-smoke`.
- DeepSeek V4 Flash live smoke passed; the transcript contains
  `robocode-deepseek-smoke-ok`.
- Host package smoke passed for `aarch64-apple-darwin`; the extracted binary
  prints `robocode-cli 0.1.7`.
- macOS arm64 archive SHA-256:
  `c9a17d5d4d3d36824616505a3abde659a6db173fffa21c22b3f60b83d988d1a2`.

## Validation Gates

Planned gates before publication:

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
- External release, GitHub artifacts, and Homebrew tap validation are pending.

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

Pending publication.
