# GUI 0.1.0-rc.3 D1 Cockpit Integration Status

Chinese version:
[release-gui-0.1.0-rc.3-status.zh-CN.md](release-gui-0.1.0-rc.3-status.zh-CN.md)

Date: 2026-07-28

This is a local, unpublished integration candidate. It is not a tag, push,
main merge, signed or notarized build, Homebrew update, release, or live
provider certification.

## Current Candidate

| Field | Value |
| --- | --- |
| Branch | `codex/d1-cockpit-closed-loop` |
| Worktree | `.worktrees/d1-cockpit-closed-loop` |
| Base | `origin/main` at `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb` |
| Core | `0.3.5`, source `f7fe1b31`, merge `76f7f8e3` |
| TUI | `0.3.3`, source `6260f183`, merge `026736dc` |
| GUI | `0.1.0-rc.3`, source `1c44094d`, merge `864966d0` |
| Fixture parity | `interaction-closed-loop`, 22 ordered events, PASS |

The candidate was rebuilt from current `origin/main` in the required Core ->
TUI -> GUI order. The former `codex/d1-cockpit-integration` Core+GUI-only
checkpoint is historical blocked evidence, not the current candidate. Its TUI
compile drift, missing parity script, and native Lane-creation result must not
be attributed to this rebuilt line.

## Deterministic Gates

| Gate | Result |
| --- | --- |
| `bash scripts/native-acp-fixture-parity.sh` | PASS, one exact Core, TUI, and GUI proof |
| `cargo test -p viden-types` | PASS, 78 passed |
| `cargo test -p viden-runtime` | PASS, 461 passed and 1 ignored |
| `cargo test -p viden-core` | PASS, 3 manual fixture refresh tests ignored |
| `cargo test -p viden-tui` | PASS, 269 library and 1 API test |
| `cargo test -p viden-gui` | PASS |
| `npm --prefix apps/gui test -- --run` | PASS, 17 files and 249 tests |
| `npm --prefix apps/gui run build` | PASS |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `cargo test --workspace --quiet` | PASS |
| `scripts/tui-turn-controller-smoke.sh` | PASS |
| `scripts/tui-regression.sh` | PASS after aligning the stale extension-capability assertion from 15 to Core 0.3.5's 16 |
| `scripts/rc-tui-stability-smoke.sh` | PASS after the same scoped gate repair |
| `cargo fmt --all -- --check` | PASS |

The parity details are recorded in
[fixture-parity.md](release-evidence/native-acp-interaction/fixture-parity.md).

## Standalone macOS Build

`npm --prefix apps/gui run tauri -- build --bundles app` passed.

- bundle:
  `target/release/bundle/macos/Viden.app`
- executable:
  `target/release/bundle/macos/Viden.app/Contents/MacOS/viden-gui`
- bundle identifier: `dev.viden.gui`
- version: `0.1.0-rc.3`
- executable size: `27,832,064` bytes
- signature: ad-hoc linker-signed, no TeamIdentifier

This is not a distribution-signed candidate. Project signing and notarization
were not run.

## Native App Smoke

Status: **PASS FOR THE SCOPED LOCAL CLOSED LOOP**.

The standalone App was exercised through Computer Use at `1229x768` against
`/tmp/viden-native-smoke.Hmd3ak`, which began as a clean temporary Git
repository with one committed README. The mouse-driven path covered Welcome,
Open Project, zero-Lane, New Lane with the built-in Viden Agent, the exact
Core-owned preview and application approval, worktree/Lane creation, one
Native execution owner, retained initial-task submission, and editable
follow-up submission. Approval actions were directly visible after the Lane
rail collapsed, and `Y · Once` was clicked with real screen coordinates.

The fallback `test-local` provider confirmed the Native submissions, but typed
user and assistant transcript rows remained explicitly `Unavailable`; no
meaningful Native assistant answer is claimed. A second clean fixture,
`/tmp/viden-codex-acp-final.wYycYE`, exercised the locally logged-in Codex ACP
adapter. The App discovered Codex, created and approved one Lane, promoted the
provisional Lane owner to the exact ACP session owner, displayed `Route ACP`,
and completed session `agent-session_1785242592994456000` with `end_turn`,
no tool calls, and the exact answer `GUI ACP OWNER OK`. This certifies the
current local ChatGPT-authenticated Codex ACP path, not portable credentials,
Claude/Kiro authentication, or an OpenAI provider release.

English and `aurora/dark` were observable, but no locale or skin configuration
entry was exposed, so configurability remains an explicit future gate.
Screenshots and the exact boundary are recorded in
[GUI D1 cockpit checkpoints](release-evidence/gui-d1-cockpit/checkpoints.md).

## Decision

The deterministic integration, standalone-app build, and scoped native
closed-loop gates pass. This remains a local candidate rather than a
distribution or live-provider certification: fallback transcript visibility,
locale/skin configuration, DeepSeek/OpenAI provider behavior, and Claude/Kiro
ACP authentication are not certified. Codex ACP is certified only for the
current local ChatGPT login. No credential creation, push, merge, tag,
signing, notarization, Homebrew mutation, release, or publication was
performed.
