# Viden TUI 0.3.1 Client Boundary Certification

Chinese version: [release-tui-0.3.1-status.zh-CN.md](release-tui-0.3.1-status.zh-CN.md)

TUI `0.3.1` is certified in this branch as a CoreClient-only component
candidate. This document does not declare a GitHub/Homebrew distribution
release.

## Source State

- Branch: `codex/v3-tui-task12-certify`
- Worktree: `/Users/wiki/Documents/GitHub/viden/.worktrees/v3-tui-task12-certify`
- TUI 0.3.1 task base: `4fbe426cd0b1bff43ae94e1a87ad26f58632b8a1`
- HEAD at the certification evidence run: `4fbe426cd0b1bff43ae94e1a87ad26f58632b8a1`
- Final commit rule: this tracked file cannot self-reference its own commit.
  The handoff must report the final commit SHA after commit creation.
- Push, merge, tag, GitHub Release, Homebrew, and publication state: not done
  and not authorized by this certification task.

## Core And Fixture Evidence

- Reviewed Core checkpoint: `a927e2f31d2cb9bb6015c30bc0ed0976e958c77e`
- Frontend schema: `1`
- Frozen contract payload:
  `5bd2b80b0953f4194d082940a7b9164c7231ca2d`
- Capability inventory: `15` frozen base capabilities plus `10` negotiated
  feature-extension capabilities; the sets are disjoint.
- Extension fixture SHA-256:
  `96dd5fde9f1241eb50f9d8978cf478d0ac5d3327448dc6ccde9d0e5018ce1580`
- Nine-fixture base-corpus aggregate SHA-256:
  `e272d7bee25af5d4a0e719aa7226f1b5bf22086e90f0d02224196c41ce67fcab`
- The base fixture file list, each file digest, and corpus aggregate are pinned
  in `apps/tui/release-manifest.toml`; any fixture drift fails regression.
- Token/catalog SHA-256 values are verified against
  `apps/tui/release-manifest.toml` during the regression run.
- TUI replay tests compare canonical projected-view hashes for all nine frozen
  base fixtures and the extension fixture against their independent Core
  fixture oracles; the extension replay also asserts the exact lane owner.

Structured evidence:

- `target/tui-regression/0.3.1/tui-0.3.1-certification.json`
- `target/tui-regression/0.3.1/shared-fixture-digests.sha256`
- `target/tui-regression/0.3.1/tui-boundary-report.txt`
- `target/tui-regression/0.3.1/tui-regression-evidence.json`
- `target/tui-stability/0.3.1/summary.md`

The certification JSON SHA-256 from the recorded run is
`8cadb425a118f8fce29bafaba9af8972e7967967376827d42bde69eb1f9dbae1`.
Generated evidence stays under `target/`; accepted design HTML and the TUI
component/cockpit reference shots were read only and their digests are recorded
in the JSON.

## Certified Behavior And Presentation

The structured report records `22` exact passing tests and covers:

- composer editing during stream, tool, and approval activity;
- Normal, Insert, and Overlay ownership;
- bracketed paste with no send, grapheme/CJK cursor geometry, actionable
  approvals, and cancellation bound to the exact live lane owner;
- deterministic `80`, `112`, and `160` column render models;
- `en` and `zh-CN` catalog/projection parity;
- eight registered palettes across truecolor, ANSI 256, and ANSI 16 (`24`
  combinations), three densities, and reduced motion;
- Settings Apply and Reset waiting for matching Core receipts;
- atomic Aurora dark/regular fallback for invalid appearance;
- absence of TUI authoritative effects, runtime-internal dependencies, and
  private preference persistence.

## Verification

- PASS `cargo fmt --all --check`
- PASS `cargo test -p viden-tui --quiet` (`260` unit tests plus `1` API test)
- PASS `cargo test -p viden-cli --quiet` (`34` unit tests; `3` integration tests
  passed and `2` live tests were ignored)
- PASS `scripts/tui-turn-controller-smoke.sh`; all `28` named filters require
  exactly one passing test.
- PASS `scripts/rc-tui-stability-smoke.sh target/tui-stability/0.3.1`
- PASS `scripts/tui-regression.sh target/tui-regression/0.3.1`
- PASS `cargo test --workspace --quiet` outside the sandbox. The first sandboxed
  run reached `viden-plugin-host` and failed only two process-reaping tests
  because `ps` returned `Operation not permitted`; the required escalated rerun
  completed with exit `0` without a code workaround.
- PASS `cargo clippy --workspace --all-targets -- -D warnings` after removing
  one test-only `clone` on a `Copy` preference receipt.
- PASS `scripts/check-doc-pairs.sh` and `scripts/check-doc-links.sh` with the
  six changed bilingual user/stability/status documents.
- PASS `git diff --check`

## Manual Evidence, Contract Gaps, And Risks

- Real macOS Terminal and iTerm2 screenshots were not supplied. Deterministic
  previews do not replace that manual release evidence.
- `mouse_capture` remains `false`. The accepted design permits an explicit
  optional mouse mode, but TUI 0.3.1 has no `mouse_capture=true` preference or
  negotiated contract path yet. Every certified action has a keyboard path.
- Schema 1 has no persisted color-depth field. Color depth remains a clearly
  local, session-only terminal preview and creates no private TUI store.
- Core exposes lane-advertised session ids, not global session enumeration.
- Trusted frontend secret ingress remains unavailable; provider detail is
  handle-only and read-only when Core exposes no safe ingress.
- Live-provider, billable, publish, release, tag, push, merge, and Homebrew
  gates were not run because this task did not authorize them.
