# GUI 0.1.0-rc.3 D1 Cockpit Integration Status

Chinese version:
[release-gui-0.1.0-rc.3-status.zh-CN.md](release-gui-0.1.0-rc.3-status.zh-CN.md)

Date: 2026-07-24

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
| `cargo test -p viden-types` | PASS, 77 passed |
| `cargo test -p viden-runtime` | PASS, 461 passed and 1 ignored |
| `cargo test -p viden-core` | PASS, 3 manual fixture refresh tests ignored |
| `cargo test -p viden-tui` | PASS, 269 library and 1 API test |
| `cargo test -p viden-gui` | PASS |
| `npm --prefix apps/gui test -- --run` | PASS, 17 files and 243 tests |
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
- executable size: `27,826,752` bytes
- signature: ad-hoc linker-signed, no TeamIdentifier

This is not a distribution-signed candidate. Project signing and notarization
were not run.

## Native App Smoke

Status: **PENDING MAC UNLOCK**.

A safe temporary Git repository with a committed minimal README was prepared
at `/tmp/viden-native-smoke.Hmd3ak`. The first Computer Use call targeted the
exact built `.app`, but the macOS desktop was locked and automatic unlock could
not unlock it. No Welcome, Open Project, zero-Lane, New Lane, composer,
approval, ACP readiness, locale, skin, or one-Lane/one-Agent native result is
claimed yet. Deterministic fixture parity is not a substitute for this native
smoke.

The smoke can resume without rebuilding after the Mac is unlocked. It must use
the fallback `test-local` model only, must not enter credentials, and must save
key screenshots under
`docs/release-evidence/gui-d1-cockpit/native-smoke/`.

## Decision

The deterministic integration and standalone-app build gates pass. The
candidate is not native-closed-loop certified and is not main-ready until the
built App completes the documented desktop smoke. No live provider, ACP login,
credential creation, push, merge, tag, signing, notarization, Homebrew
mutation, release, or publication was performed.
