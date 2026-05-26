# RoboCode 0.1.6 Release Status

Chinese version: [release-0.1.6-status.zh-CN.md](release-0.1.6-status.zh-CN.md)

Last updated: 2026-05-26

## Goal

Version `0.1.6` is the live-cockpit and extension-foundation release. Its
purpose is to make RoboCode easier to operate during real programming work:
the main screen shows live activity, side screens expose lane and ops evidence,
agent and extension diagnostics are discoverable, and the ACP direction has a
minimal protocol proof instead of being only a roadmap note.

## Phase Mapping

1. Live cockpit visibility: local implementation landed.
2. Agent and extension visibility: local implementation landed.
3. Side-1 and side-2 evidence screens: local implementation landed.
4. ACP readiness and protocol probe: local implementation landed.
5. Release packaging: local smoke passed.
6. External publication: pending GitHub release and Homebrew tap validation.

## Candidate Evidence

- Workspace package version moved from `0.1.5` to `0.1.6`.
- `Cargo.lock` package entries now resolve to `0.1.6`.
- GitHub release workflow default tag moved to `v0.1.6`.
- README install examples now point at `v0.1.6`.
- The README system screenshot keeps its curated layout with the visible
  version updated to `0.1.6`.
- Side-2 preview validation now expects real ops panels:
  `TESTS / LSP`, `MCP / CONTEXT`, `EXTENSIONS`, and `RECENT EVIDENCE`.
- `/agent list` and `/agent doctor acp` expose the experimental ACP adapter and
  `ROBOCODE_AGENT_ACP_COMMAND` setup state.
- `/agent doctor acp` performs a minimal JSON-RPC `initialize` handshake probe,
  records JSONL evidence under `.robocode/agents/`, and reports protocol,
  agent name/version, timeout, or failure details.
- Full local release smoke with DeepSeek live provider validation passed:
  `scripts/release-smoke.sh --version 0.1.6 --deepseek --out-dir /tmp/robocode-016-release-smoke-deepseek-local`.
- Evidence directory:
  `/tmp/robocode-016-release-smoke-deepseek-local`.
- The smoke matrix passed `cargo-fmt`, `robocode-cli-tests`,
  `workspace-tests`, `tui-previews`, `fallback-cli-smoke`,
  `shell-lane-smoke`, `tmux-lane-smoke`, `package-smoke`, and
  `deepseek-cli-smoke`.
- DeepSeek V4 Flash live smoke passed; the transcript contains
  `robocode-deepseek-smoke-ok`.
- Host package smoke passed for `aarch64-apple-darwin`; the extracted binary
  prints `robocode-cli 0.1.6`.
- macOS arm64 archive SHA-256:
  `22413a9d94fc0fc950ba47e232f9025ac218eb35cd788c13b2b3d44231cadab1`.

## Validation Gates

Before publishing `v0.1.6`, run after pushing the version bump to `main`:

```bash
scripts/release-smoke.sh --version 0.1.6 --skip-package --deepseek --github-actions
```

The final status update should record:

- the GitHub Actions validation run URL;
- the published release URL and artifact list;
- Homebrew tap commit and fetch/install smoke result.

## Open Findings

### P0

- Run GitHub Actions artifact validation and publish the final release after
  the `0.1.6` version bump is on `main`.

### P1

- Full `/lane acp <agent> <task>` execution remains a follow-up. `0.1.6`
  proves the process boundary and handshake/evidence path, not the full edit
  loop.
- Extension invocation remains conservative: diagnostics and visibility are
  ready before broad plugin execution is enabled.

### P2

- Automatic task splitting remains deferred.
- Full cursor-addressed terminal replay remains deferred.
- More external coding-agent templates remain demand-driven follow-up work.

## Release Outcome

`v0.1.6` is not marked published until the release workflow uploads all
configured artifacts and the Homebrew tap has been updated.
