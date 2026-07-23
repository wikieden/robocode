# GUI D1 Cockpit Integration Checkpoints

Chinese version: [checkpoints.zh-CN.md](checkpoints.zh-CN.md)

Date: 2026-07-24

This evidence describes a local candidate only. It is not a published,
signed, notarized, pushed, merged, tagged, or live-provider-certified release.

## Candidate Line

| Item | SHA / path |
| --- | --- |
| Base `origin/main` | `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb` |
| Core `0.3.5` source / merge | `f7fe1b31dfb237e4062209767a7051c2b2c68b93` / `76f7f8e3a84ff38846023dda7dead0c50bfb2b68` |
| TUI `0.3.3` source / merge | `6260f183d19da27e61fdf068d67a9c481c68d829` / `026736dc4c16b1d039b80e77b9fe8ff99788d51b` |
| GUI `0.1.0-rc.3` source / merge | `1c44094dd29674e1cc585ff6c83302581440aeb0` / `864966d0677e9d958396fac150f4701b2d14b0a1` |
| Integration fix | `cb9baaf7ff212655d3b1ea8dd3cb4684ae40f7d0` |
| Fixture parity | `d4fe33fb0510bf05fb4586ddf2ec4cd7718f185d` |
| Task 14 base | `d4fe33fb0510bf05fb4586ddf2ec4cd7718f185d` |
| Branch | `codex/d1-cockpit-closed-loop` |
| Worktree | `.worktrees/d1-cockpit-closed-loop` |

The older `codex/d1-cockpit-integration` Core+GUI-only line is retained only as
historical blocked evidence. The table above is the current candidate.

## Deterministic Evidence

| Command | Result |
| --- | --- |
| `bash scripts/native-acp-fixture-parity.sh` | PASS: exact Core replay/hash, TUI render, and GUI projection proofs |
| `cargo test -p viden-types` | PASS, 77 |
| `cargo test -p viden-runtime` | PASS, 461 + 1 ignored |
| `cargo test -p viden-core` | PASS |
| `cargo test -p viden-tui` | PASS, 269 + 1 API |
| `cargo test -p viden-gui` | PASS |
| `npm --prefix apps/gui test -- --run` | PASS, 17 files / 243 tests |
| `npm --prefix apps/gui run build` | PASS |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `cargo test --workspace --quiet` | PASS |
| `scripts/tui-turn-controller-smoke.sh` | PASS |
| `scripts/tui-regression.sh` | PASS after Core 0.3.5 extension count repair |
| `scripts/rc-tui-stability-smoke.sh` | PASS after the same repair |
| `cargo fmt --all -- --check` | PASS |

The canonical parity fixture contains 22 events and ends at
`fixture:interaction-closed-loop@22` with Core view SHA-256
`46db05abaaae36cf37cb7ffa0493a4ef8c158a2d5b4ffeef08d01dbf8e284ed0`.
See
[fixture-parity.md](../native-acp-interaction/fixture-parity.md).

## App Build Evidence

`npm --prefix apps/gui run tauri -- build --bundles app` completed successfully:

- App: `target/release/bundle/macos/Viden.app`
- executable:
  `target/release/bundle/macos/Viden.app/Contents/MacOS/viden-gui`
- `CFBundleIdentifier`: `dev.viden.gui`
- `CFBundleExecutable`: `viden-gui`
- `CFBundleShortVersionString`: `0.1.0-rc.3`
- executable size: `27,826,752` bytes
- signature: ad-hoc linker-signed, no TeamIdentifier

No project distribution-signing or notarization gate was run.

## Native Smoke Boundary

Status: **PENDING MAC UNLOCK**.

The smoke fixture is `/tmp/viden-native-smoke.Hmd3ak`, a clean Git repository
with one committed README. Computer Use targeted the exact App path above, but
reported that the Mac was locked and automatic unlock failed. No native
interaction or screenshot evidence exists yet, so this checkpoint does not
claim Welcome, project selection, zero-Lane, Lane creation, one-Lane/one-Agent,
composer input, runtime output, approval, ACP readiness, locale, or skin
success.

After unlock, the smoke must resume against this bundle with fallback
`test-local`, save key screenshots under `native-smoke/`, and record either
typed completion or an exact typed recovery/rejection. It must not enter
credentials or run a live provider/ACP turn.
