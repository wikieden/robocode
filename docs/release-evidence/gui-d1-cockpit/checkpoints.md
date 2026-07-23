# GUI D1 Cockpit Integration Checkpoints

Chinese version: [checkpoints.zh-CN.md](checkpoints.zh-CN.md)

Date: 2026-07-24

This file records the local Core+GUI integration checkpoint for the D1 cockpit.
It is evidence only; it is not a published release.

## Checkpoints

| Item | SHA / path |
| --- | --- |
| Base `origin/main` | `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb` |
| Core source | `f7fe1b31dfb237e4062209767a7051c2b2c68b93` |
| Core merge | `6d0094a11cc64c097a2f48ee6122ec8bc95a2d23` |
| GUI source | `4cb2a498c5b091f62f554ff407acdf162f96cc1e` |
| GUI merge | `d27dc09fd230e3c2fb3ae79fbf3d10a45f400226` |
| Branch | `codex/d1-cockpit-integration` |
| Worktree | `/Users/wiki/Documents/GitHub/viden/.worktrees/d1-cockpit-integration` |
| App bundle | `target/release/bundle/macos/Viden.app` |
| DMG bundle | `target/release/bundle/dmg/Viden_0.1.0-rc.3_aarch64.dmg` |

## Versions

| Component | Version | Evidence |
| --- | --- | --- |
| Core | `0.3.5` | `crates/core/release-manifest.toml` |
| GUI | `0.1.0-rc.3` | `apps/gui/manifests/0.1.0-rc.3.toml`, app `Info.plist` |
| TUI | `0.3.3` | Not merged in this task; workspace still contains inherited TUI/Core API drift. |

## Deterministic Evidence

| Command | Result |
| --- | --- |
| `cargo test -p viden-types` | PASS, 77 passed |
| `cargo test -p viden-runtime` | PASS on final rerun, 461 passed and 1 ignored; first full run had one transient cancellation-test failure |
| `cargo test -p viden-core` | PASS |
| `cargo test -p viden-gui` | PASS |
| `npm --prefix apps/gui ci` | PASS, 0 vulnerabilities |
| `npm --prefix apps/gui test -- tests/d1_cockpit.spec.ts --run` | PASS, 54 focused D1 tests after native-menu repair |
| `npm --prefix apps/gui test -- --run` | PASS, 17 files and 239 tests after native-menu repair |
| `npm --prefix apps/gui run build` | PASS after native-menu repair |
| `bash scripts/native-acp-fixture-parity.sh` | BLOCKED, script absent |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `cargo test --workspace --quiet` | BLOCKED, `viden-tui` compile failure against current Core facade |
| `npm --prefix apps/gui run tauri build` | PASS after native-menu repair |

## Hash Evidence

| Artifact | SHA-256 |
| --- | --- |
| `crates/types/tests/fixtures/frontend-contract-v1/d1-main-cockpit.json` | `f96ba30cc6e80aa52cb15a2fd1f03c082487a3cd4779c25f61e42ee1548e1e3b` |
| `apps/gui/evidence/0.1.0-rc.3/d1-design-reference-canonical.png` | `f9209057b5538278da861e04bb43b891438802d9a41dcb5f1476b341b93dc11c` |
| `apps/gui/evidence/0.1.0-rc.3/d1-context-dock-bottom-1280x800.png` | `0179f20ac53a484dfb0194392d206d7e182eae1d33d0fd0e94f43c1e2fcc6c30` |
| `apps/gui/evidence/0.1.0-rc.3/d1-design-reference-vs-actual.png` | `d27302d81afaeadfc156513eed30d251ff09194b1b3392010baeac5602ced5e8` |
| `apps/gui/evidence/0.1.0-rc.3/accepted-target-dark-cockpit.png` | `d4c97aa4ebe603eddd290785a0e632fd41b72a94de5e7ccb6206352bb0f37e36` |

## Native Smoke Boundary

The app bundle was built and launched, and its metadata was verified:

- `CFBundleExecutable`: `viden-gui`
- `CFBundleIdentifier`: `dev.viden.gui`
- `CFBundleName`: `Viden`
- `CFBundleShortVersionString`: `0.1.0-rc.3`
- `CFBundleVersion`: `0.1.0-rc.3`
- app size: `27M`
- DMG size: `9.5M`
- signature: ad-hoc, no TeamIdentifier

The exact bundle launch produced PID `49949`, but the process-level desktop
path reported zero Accessibility windows for that PID while an older same-name
Viden process owned one window.

Independent native-app desktop control verified the integration bundle through
Welcome, native Open Project, selecting a safe temp project, D1 shell retention,
and opening the compact `+ New Lane` popup.

This checkpoint includes a scoped GUI repair that keeps native `Viden Agent`
Lane creation enabled while ACP probes run. ACP choices remain correctly gated
on ACP readiness. Post-repair native smoke shows the menu resolves: `Viden
Agent` is enabled and selectable, `Codex` is Ready, `Kiro` is Ready, and
`Claude` is disabled with an initialize-probe failure. Selecting `Viden Agent`
closes the menu, but after 5 seconds no new Lane appears. The active native
smoke blocker is native Lane creation/Core owner binding. The existing selected
Lane also still has no sole Core execution owner, so send remains disabled. The
repair is covered by `npm --prefix apps/gui test --
tests/d1_cockpit.spec.ts --run` and the full GUI npm suite.
