# GUI D1 Cockpit Integration Checkpoints

Chinese version: [checkpoints.zh-CN.md](checkpoints.zh-CN.md)

Date: 2026-07-27

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
| `npm --prefix apps/gui test -- --run` | PASS, 17 files / 248 tests |
| `npm --prefix apps/gui run build` | PASS |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `cargo test --workspace --quiet` | PASS |
| `scripts/tui-turn-controller-smoke.sh` | PASS |
| `scripts/tui-regression.sh` | PASS after Core 0.3.5 extension count repair |
| `scripts/rc-tui-stability-smoke.sh` | PASS after the same repair |
| `cargo fmt --all -- --check` | PASS |
| `npm --prefix apps/gui test -- tests/agent_menu.spec.ts tests/d1_cockpit.spec.ts --run` | PASS, 69 tests after native-smoke repairs |
| `git diff --check` | PASS |

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
- executable size: `27,830,256` bytes
- signature: ad-hoc linker-signed, no TeamIdentifier

No project distribution-signing or notarization gate was run.

## Native Smoke Boundary

Status: **PASS**.

The smoke fixture is `/tmp/viden-native-smoke.Hmd3ak`, which began as a clean
Git repository with one committed README. Creating the in-repository fixture
worktrees made those fixture paths visible as untracked state afterward; the
standalone App was exercised at `1229x768` with fallback `test-local`. The
observed native path covered:

- persistent Welcome and Open Project;
- a project cockpit with zero Lanes;
- the compact New Lane menu with the Native Viden Agent selected;
- Core-owned preview, application-level approval, worktree creation, and the
  exact created Lane;
- one Lane with one Native execution owner;
- editable composer input, retained initial task submission, and a confirmed
  follow-up submission.

The fallback provider projected typed `Unavailable` transcript content, so this
checkpoint claims command confirmation and operable input, not a meaningful
assistant answer or live-provider inference. ACP discovery was visible only as
unavailable in this offline run. The observed UI was English with
`aurora/dark`; no locale or skin configuration surface was exposed, so
configurability remains unverified.

Native evidence is stored under [native-smoke](native-smoke/):

| File | Observation |
| --- | --- |
| `01-welcome.jpeg` | persistent Welcome shell |
| `02-zero-lane.jpeg` | opened project with zero Lanes |
| `03-new-lane.jpeg` | compact New Lane menu |
| `04-approval-obscured-before-fix.jpeg` | defect-before-fix evidence: Lane rail covered approval actions |
| `04-approval-after-fix.jpeg` | post-fix approval is full-width; Once, Repo paths, and Deny are unobscured |
| `05-lane-created-after-fix.jpeg` | mouse-clicking Once created the exact Lane/worktree and Native owner |
| `06-follow-up-confirmed-unavailable.jpeg` | editable follow-up was confirmed; fallback response remained unavailable |

The native run exposed and the candidate fixes three closed-loop defects:
the New Lane overlay was clipped by the Lane rail, Create waited indefinitely
instead of yielding to an interactive approval, and the pre-registration
approval was not projected through the exact pending Create owner. The final
mouse-only retest of the rebuilt App passed: the Lane rail collapsed when the
approval appeared, all approval actions were unobscured, and a real-coordinate
click on `Y · Once` continued to the exact created Lane, branch, worktree,
Native owner, and retained initial task submission.
