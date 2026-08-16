# Viden GUI Framework Decision

Chinese version: [gui-framework-decision.zh-CN.md](gui-framework-decision.zh-CN.md)

Decision date: 2026-07-20

Component: `viden-gui 0.1.0-alpha.1`

Status: selected for the production baseline; the production Tauri client has
since been bootstrapped (see [`apps/gui/README.md`](../apps/gui/README.md))

## Record Recovery Note (2026-08-16)

This record was written on the `gui-v0.1` alpha line on 2026-07-20 but did not
reach mainline with the rest of that work; it was recovered from the original
commit `d9d5b26b` on 2026-08-16. The alpha spike harness (`apps/gui/spikes/**`)
that the Reproduce commands below exercise has since been superseded by the
production Tauri client under `apps/gui/`, so rerunning the full gate requires
checking out the historical alpha tree at that commit. The machine-readable
evidence records, the comparator tooling under `apps/gui/tools/`, and the
`apps/gui/framework-gate.toml` selection record are preserved as the immutable
gate evidence.

## Decision

Viden selects **Tauri** as the single production GUI framework. GPUI remains an
alpha comparison spike and will not be used to start a second production
client.

The selection follows the published hard rule: GPUI can become the production
framework only when every required measurement and hard gate passes. A missing,
partial, or failed result is not a pass. The reproducible comparator found 16
GPUI blockers, so it selected Tauri.

This decision does not declare the Tauri client release-ready. Tauri retains the
same unverified performance, accessibility, cross-platform, soak, packaging,
credential, and recovery gates listed below. Task 5 may bootstrap only the
Tauri production client; later release gates must close those limitations with
real evidence.

## Evidence Summary

| Gate | Tauri | GPUI | What was actually verified |
| --- | --- | --- | --- |
| Equal D1 slice | Pass | Pass | Candidate tests exercise the shared roles, action log, projection hash, queue/cancel, approval, history, theme, and focus behavior. |
| Ordered events | Pass | Pass | The shared adapter preserved identity and order for exactly 10,000 events. |
| Transcript scale | Partial | Partial | Shared paging covered exactly 50,000 rows; neither framework renderer was instrumented for bounded virtualization. |
| macOS build and launch | Pass | Pass | Both debug binaries built and stayed alive for a five-second smoke on Darwin arm64. |
| Composer input p95 `< 50 ms` | Unavailable | Unavailable | No input timing collector exists. |
| Event-to-visible p95 `< 100 ms` | Unavailable | Unavailable | No native event-to-paint instrumentation exists. |
| Frame work p95 `< 16 ms` | Unavailable | Unavailable | No frame timing collector exists. |
| Native CJK IME | Partial | Partial | Framework composition tests pass, but no operating-system IME injection was captured. |
| Keyboard-only | Partial | Partial | Focus traversal tests pass, but no complete native-window run was captured. |
| Screen reader | Unavailable | Unavailable | No assistive-technology run was captured. |
| Linux and Windows build/launch | Unavailable | Unavailable | Only the local macOS host was exercised. |
| Bounded soak and near-zero idle CPU | Unavailable | Unavailable | No soak or CPU sampler exists. |
| No long-lived framework fork | Pass | Pass | Both spikes consume released framework packages and carry no repository patch or fork. |
| Visual parity | Unavailable | Unavailable | No repeatable live D1 screenshot comparison was captured. |
| Signing, updater, credential storage, crash recovery | Unavailable | Unavailable | These production delivery paths do not exist in the alpha spikes. |

The machine-readable candidate records are [Tauri
evidence](../apps/gui/evidence/framework-gate/tauri.json) and [GPUI
evidence](../apps/gui/evidence/framework-gate/gpui.json). The generated blocker
list is [framework gate decision evidence](../apps/gui/evidence/framework-gate/decision.md),
and the active selection is recorded in
[`apps/gui/framework-gate.toml`](../apps/gui/framework-gate.toml).

## Reproduce

Run from the repository root:

```bash
apps/gui/tools/run-framework-gate.sh tauri crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json
apps/gui/tools/run-framework-gate.sh gpui crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json
python3 apps/gui/tools/compare-framework-gate.py \
  apps/gui/evidence/framework-gate/tauri.json \
  apps/gui/evidence/framework-gate/gpui.json
```

Each candidate record includes the fixture digest, host/tool versions, exact
commands, exit codes, and output tails. Every D1 test command first verifies
that the supplied file is byte-identical to the committed fixture and matches
its fixture ID and projection digest. A native launch passes only when the
process remains alive for the complete five-second interval. The comparator
rejects any pass record backed by a failed command, and the runner records
unavailable evidence as unavailable rather than substituting estimates.

## Consequences

- Task 5 creates only the Tauri production shell.
- GPUI remains a comparison spike and does not receive parallel production
  features.
- Tauri may directly reuse the accepted `tokens.css` and GUI component assets.
- The Core command/event/snapshot/replay boundary remains framework-neutral;
  framework selection does not move business state into the frontend.
- Missing Tauri gates remain release blockers and must be measured on the
  required platforms before beta or stable claims.

See the [GUI functional design](gui-version-functional-design.md) and [parallel
development plan](parallel-development-plan.md) for the resulting roadmap.
