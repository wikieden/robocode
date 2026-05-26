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
6. External publication: GitHub release and Homebrew tap validation passed.

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
- GitHub Actions release artifact validation passed with
  `upload_to_release=false` for all configured targets:
  `aarch64-apple-darwin`, `x86_64-apple-darwin`,
  `x86_64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc`.
  Run: https://github.com/wikieden/robocode/actions/runs/26440197730.
- The final GitHub release workflow passed with `upload_to_release=true` and
  uploaded all configured artifacts.
  Run: https://github.com/wikieden/robocode/actions/runs/26440351407.
- Homebrew tap `wikieden/homebrew-tap` now points RoboCode formula URLs and
  SHA-256 values at `v0.1.6`.
  Commit: https://github.com/wikieden/homebrew-tap/commit/b8c94da.
- Homebrew fetch smoke passed:
  `brew fetch --force wikieden/tap/robocode` reported
  `Formula robocode (0.1.6)`.

## Validation Gates

All planned `0.1.6` validation gates have passed:

- local release smoke with package and DeepSeek live provider validation;
- GitHub Actions artifact validation with `upload_to_release=false`;
- final GitHub release artifact upload with `upload_to_release=true`;
- Homebrew tap update and fetch smoke.

## Open Findings

### P0

- None for the `0.1.6` release.

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

`v0.1.6` is published at:

- https://github.com/wikieden/robocode/releases/tag/v0.1.6

The release contains:

- `robocode-v0.1.6-aarch64-apple-darwin.tar.gz`
- `robocode-v0.1.6-x86_64-apple-darwin.tar.gz`
- `robocode-v0.1.6-x86_64-unknown-linux-gnu.tar.gz`
- `robocode-v0.1.6-x86_64-pc-windows-msvc.tar.gz`
- matching `.sha256` files for each archive.

GitHub reported asset digests:

- `aarch64-apple-darwin`:
  `sha256:5c2783b86574edf95a66af7b176ea6e3c24680782f53817d00661661397faac3`
- `x86_64-apple-darwin`:
  `sha256:7229d9a2dcdd796735ccbde6dcfccac8d35a66454b76e42cca561242e3789c6a`
- `x86_64-unknown-linux-gnu`:
  `sha256:b47d2648de98a72e2d9e0b8afef1a92090bb4325374d6f69086b7b790f9da77e`
- `x86_64-pc-windows-msvc`:
  `sha256:4b94e1f645b8b383a1b131f24e5eebfacff6f3d1ac684c05fbfe4a372b4ce386`

Homebrew install path:

```bash
brew tap wikieden/tap
brew install robocode
```
