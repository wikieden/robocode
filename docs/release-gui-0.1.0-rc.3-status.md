# GUI 0.1.0-rc.3 D1 Cockpit Integration Status

Chinese version: [release-gui-0.1.0-rc.3-status.zh-CN.md](release-gui-0.1.0-rc.3-status.zh-CN.md)

Date: 2026-07-24

This is a local, unpublished D1 cockpit integration checkpoint. It is not a
tag, push, main merge, signed build, notarized build, Homebrew update, or live
provider certification.

## Integrated Inputs

| Field | Value |
| --- | --- |
| Branch | `codex/d1-cockpit-integration` |
| Worktree | `/Users/wiki/Documents/GitHub/viden/.worktrees/d1-cockpit-integration` |
| Base | `origin/main` at `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb` |
| Core source | `f7fe1b31dfb237e4062209767a7051c2b2c68b93` |
| Core merge commit | `6d0094a11cc64c097a2f48ee6122ec8bc95a2d23` |
| GUI source | `4cb2a498c5b091f62f554ff407acdf162f96cc1e` |
| GUI merge commit | `d27dc09fd230e3c2fb3ae79fbf3d10a45f400226` |
| Core version | `0.3.5` |
| GUI version | `0.1.0-rc.3` |
| TUI version | `0.3.3`, not merged in this task |

## Gate Results

| Gate | Result | Evidence |
| --- | --- | --- |
| `cargo test -p viden-types` | PASS | 77 passed |
| `cargo test -p viden-runtime` | PASS after rerun | Final rerun: 461 passed, 1 ignored. The first full run failed once in `acp_async_job_can_be_cancelled_by_pid`; the isolated test and full rerun passed. |
| `cargo test -p viden-core` | PASS | Unit, CoreClient, frontend contract, host services, and workspace identity tests passed; 3 manual fixture refresh tests ignored. |
| `cargo test -p viden-gui` | PASS | 82 Rust GUI tests passed across adapter, D11, D1, D4, D6, permission, reconnect, and virtualization surfaces. |
| `npm --prefix apps/gui ci` | PASS | 0 vulnerabilities. |
| `npm --prefix apps/gui test -- tests/d1_cockpit.spec.ts --run` | PASS | 54 focused D1 tests passed after the native-menu availability repair. |
| `npm --prefix apps/gui test -- --run` | PASS | 17 files, 239 tests passed after the native-menu availability repair. |
| `npm --prefix apps/gui run build` | PASS | TypeScript check and Vite production build passed after the native-menu availability repair. |
| `bash scripts/native-acp-fixture-parity.sh` | BLOCKED | Script is absent in the Core+GUI-only integration; no TUI branch was merged. |
| `bash scripts/check-dependency-boundaries.sh` | PASS | Completed with exit 0. |
| `cargo test --workspace --quiet` | BLOCKED | `viden-tui` does not compile against the merged Core facade; representative errors are removed `viden_core` root exports, removed `ApprovalResponse.approved`, and old string-shaped `AgentTaskRecord`/`AgentLaneRecord` fields. |
| Manifest/hash integrity | PASS | `d1-main-cockpit.json` and rc.3 screenshot hashes match `apps/gui/manifests/0.1.0-rc.3.toml`. |
| `npm --prefix apps/gui run tauri build` | PASS | Built `target/release/bundle/macos/Viden.app` and `target/release/bundle/dmg/Viden_0.1.0-rc.3_aarch64.dmg` after the native-menu availability repair. |

## Native App Smoke

The exact unsigned app bundle was built and launched:

- app path: `target/release/bundle/macos/Viden.app`
- executable: `Contents/MacOS/viden-gui`
- bundle id: `dev.viden.gui`
- version: `0.1.0-rc.3`
- signature: ad-hoc, no TeamIdentifier
- first launched PID: `49949`

The local process-level launch showed PID `49949` from the exact integration
bundle. That first desktop-control path could not distinguish a visible window
for the new PID because an older Viden process from
`.worktrees/native-acp-integration` was already running with the same product
process name.

Independent native-app desktop control then verified the reachable UI state on
the integration bundle: persistent Welcome shell, native Open Project folder
dialog, selecting an existing safe temp project, and D1 shell retention passed.
The Lanes rail opened and showed the compact `+ New Lane` popup.

Repair applied in this checkpoint: `apps/gui/src/components/agent_menu.ts` now
keeps the native `Viden Agent` option enabled whenever Core workspace
eligibility allows Lane creation; ACP options still remain gated by ACP
readiness. The focused and full GUI npm suites pass after the repair.

Post-repair native smoke: the New Lane menu resolves. `Viden Agent` is enabled
and selectable; `Codex` is Ready; `Kiro` is Ready; `Claude` is disabled with an
initialize-probe failure. Selecting `Viden Agent` closes the menu, but after 5
seconds no new Lane appears. The current native smoke blocker is native Lane
creation/Core owner binding. The existing selected Lane also still lacks a sole
Core execution owner, so send remains disabled there.

## Decision

This checkpoint is a reviewable Core+GUI integration candidate, but it is not a
complete D1 cockpit certification. A scoped GUI repair keeps native Lane
creation available during ACP probing, but the remaining blockers are outside
that local menu-state fix: the TUI/Core workspace compile drift, the missing
native ACP parity script, native Lane creation/Core owner binding after
selecting `Viden Agent`, and no sole Core execution owner for the pre-existing
selected Lane.

Next safe step: serialize the TUI/Core compatibility migration or authorize a
fresh native smoke after the native-menu repair, then rerun workspace, parity,
and native desktop gates.
