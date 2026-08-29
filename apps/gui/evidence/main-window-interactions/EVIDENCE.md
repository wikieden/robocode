# Main-window interactions — visual evidence

Chinese version: [EVIDENCE.zh-CN.md](EVIDENCE.zh-CN.md)

Covers the main-window interaction work on `claude/gui-main-window-interactions`:
the D6 recovery actions (restart / close Lane, the presentation-only inspect
expansion, and the rejection alert), the composer work-mode / permission /
model selectors together with the cockpit statusbar, the Settings overlay
behind the activity rail's gear, D11 project intake, and the D12 merge-gate
accept / bounce action bar with its mandatory reason input.

## What the harness is

[`qa.html`](qa.html) plus [`qa.ts`](qa.ts) render exactly one screenshot state
per `?state=` value. The page calls **only exported production render
functions** — `renderD1Cockpit`, `renderD6Recovery` (through the cockpit's own
work surface), `renderD11Intake`, `renderD12IntegrationGate`, and the
components they mount. Nothing here reimplements a control, a label, or a
layout, so a regression in `src/**` shows up in the capture instead of being
masked by a harness copy of the UI.

Rendering lives in `qa.ts` rather than an inline `<script>` so `tsc --noEmit`
typechecks the harness against the production render signatures.

### Determinism

- `Date.now` is frozen to `2026-01-01T00:00:00.000Z` before the first render.
  The work-status strip prints `now() - startedAt`, so an unfrozen clock would
  make two captures of the same state differ.
- `Math.random` is frozen to `0`.
- The cockpit is mounted with `poll: false`, so no timer ever refreshes a
  capture while the operator is framing it.
- Every host callback is a promise that never resolves, so no state can change
  after the harness settles. The single exception is `d6-error`, whose
  `sendD6Intent` rejects with one fixed sentence on purpose — that rejection
  *is* the state being captured.
- Each state finishes by setting `document.documentElement.dataset.captureReady`
  to its own name. Capture only after that attribute is present.

## Where the projections come from

| State group | Source | Kind |
| --- | --- | --- |
| `d1*`, `settings*`, `d6-*` | [`../../tests/support/d1_projection.ts`](../../tests/support/d1_projection.ts) | the shared D1 fixture the vitest suites mount |
| `d12-*` | [`../gui-screen-restore/projections/d12.json`](../gui-screen-restore/projections/d12.json) | **generated** by `tests/capture_projections.rs` |
| `d11` | mirrors the fixtures in `tests/d11_intake.spec.ts` | hand-written; D11 has no generated capture projection yet |

The D12 projection is never hand-written. `tests/capture_projections.rs` runs
the real Rust projection over the canonical `frontend-contract-v1`
`merge-gate.json` fixture and serializes the result, so the captured pixels are
what Core facts actually produce. Regenerate after any projection change:

```bash
cargo test -p viden-gui --test capture_projections -- --ignored
```

W4 gave the D12 gate actions typed availability codes; the committed
`d12.json` now carries `missing_evidence` on accept and `no_actor` on reject,
which is what `d12-blocked` captures.

### Deltas written on top of a source

Every value the harness adds is a delta on one of the sources above, and each
one carries an inline comment in `qa.ts` naming the fixture it mirrors.

| State | Delta | Mirrors |
| --- | --- | --- |
| all `d1*` | `preferences.locale/skin/mode` follow the URL parameters | the cockpit reads its locale from the projection, not the document element |
| all `d1*` | a `ContextUsageProjection` on the context dock and the three statusbar fields the shared fixture leaves empty (`context`, `diagnosticsCount`, `pendingGateCount`) | the populated statusbar fixture in `tests/statusbar.spec.ts` |
| all `d1*` | `agentAdapters[0].models` | the adapter fixture in `tests/composer_controls.spec.ts` |
| `d6-actions`, `d6-error` | a stopped session whose `restart` carries a session id and `close_lane` a lane id | the `STOPPED` fixture in `tests/d6_recovery.spec.ts` |
| `d12-actions` | required evidence recorded, validator satisfied, both actions available with a `null` code | the `DECIDABLE` fixture in `tests/d12_integration_gate.spec.ts` |
| `d11` | a probed `/workspace/demo` rust project with a credential-locked provider | the probed-project fixture in `tests/d11_intake.spec.ts` |
| `palette` | one cross-Lane merge gate and one ask handed to `loadPaletteCrossLane` | the gate fixture in `tests/d12_integration_gate.spec.ts` and the single `liveWork.approvals` entry the D1 fixture already carries |

## How to run it

Start the dev server from the worktree that owns `apps/gui/**`:

```bash
npm --prefix apps/gui run dev -- --port 4173 --strictPort
```

Then open each URL in the authorized Browser runtime at a 1440x900 viewport,
wait for `data-capture-ready`, and capture. This mirrors
`tools/capture-d1-visual.sh`: the procedure standardizes URLs and dimensions
and does not invoke browser automation outside that runtime.

All URLs share the prefix
`http://localhost:4173/evidence/main-window-interactions/qa.html`.

| State | URL | What the capture must show |
| --- | --- | --- |
| `d1` | `…/qa.html?state=d1` | the full cockpit; the titlebar project selector with its branch and dirty marker beside the `↑/↓` and worktree chips; all nine statusbar segments carrying a fact plus the pending-gate chip; the three composer selector pills |
| `d1-mode-menu` | `…/qa.html?state=d1-mode-menu` | the work-mode popover open over the composer, with the current mode marked selected |
| `d1-model-menu` | `…/qa.html?state=d1-model-menu` | the model popover open, showing both the provider group and the adapter group Core published |
| `palette` | `…/qa.html?state=palette` | the ⌘K command palette open over the cockpit from the titlebar toggle, with all four sections visible — Actions, Jump to (the cross-Lane gate and ask plus the Lane), Settings, and the permanently disabled Files row naming `GUI-CORE-022` |
| `settings` | `…/qa.html?state=settings` | the Settings overlay open over the cockpit with an unsaved draft; Cancel and Save enabled |
| `settings-unavailable` | `…/qa.html?state=settings-unavailable` | the same overlay read-only, naming the absent `ui.preference_persistence` capability; Save disabled |
| `d6-actions` | `…/qa.html?state=d6-actions` | the recovery surface with Restart agent and Close Lane enabled, and the inspect facts expanded |
| `d6-error` | `…/qa.html?state=d6-error` | the same surface after a refused restart, with Core's rejection rendered as an alert |
| `d12-actions` | `…/qa.html?state=d12-actions` | the merge gate with Accept available and the bounce reason input filled and enabled |
| `d12-blocked` | `…/qa.html?state=d12-blocked` | the same gate with Accept unavailable, naming `missing_evidence`, and the reason input disabled |
| `d11` | `…/qa.html?state=d11` | the project intake screen with the probed project and the provider warning |

`mode=dark|light` and `locale=en|zh-CN` are accepted on every state and resolve
through the shared `resolveTheme` path, so the harness never ships a second
palette. `mode=light` also selects the `ice` skin, matching how the design
pairs them. Suggested locale and skin proof:
`…/qa.html?state=d1&mode=light&locale=zh-CN`.

## Captured screenshots

Captured 2026-08-21 with headless Chrome
(`--headless --window-size=1440,900 --virtual-time-budget=6000`) against the
vite dev server on port 4173, then visually reviewed (6 of 11 sampled in
review; all 11 DOM-verified at build time).

| File | State | Viewport | Mode | Locale |
| --- | --- | --- | --- | --- |
| [d1-1440x900-dark-en.png](d1-1440x900-dark-en.png) | d1 | 1440x900 | dark | en |
| [d1-mode-menu-1440x900-dark-en.png](d1-mode-menu-1440x900-dark-en.png) | d1-mode-menu | 1440x900 | dark | en |
| [d1-model-menu-1440x900-dark-en.png](d1-model-menu-1440x900-dark-en.png) | d1-model-menu | 1440x900 | dark | en |
| [settings-1440x900-dark-en.png](settings-1440x900-dark-en.png) | settings | 1440x900 | dark | en |
| [settings-unavailable-1440x900-dark-en.png](settings-unavailable-1440x900-dark-en.png) | settings-unavailable | 1440x900 | dark | en |
| [d6-actions-1440x900-dark-en.png](d6-actions-1440x900-dark-en.png) | d6-actions | 1440x900 | dark | en |
| [d6-error-1440x900-dark-en.png](d6-error-1440x900-dark-en.png) | d6-error | 1440x900 | dark | en |
| [d12-actions-1440x900-dark-en.png](d12-actions-1440x900-dark-en.png) | d12-actions | 1440x900 | dark | en |
| [d12-blocked-1440x900-dark-en.png](d12-blocked-1440x900-dark-en.png) | d12-blocked | 1440x900 | dark | en |
| [d11-1440x900-dark-en.png](d11-1440x900-dark-en.png) | d11 | 1440x900 | dark | en |
| [d1-1440x900-light-zh-CN.png](d1-1440x900-light-zh-CN.png) | d1 | 1440x900 | light | zh-CN |

The eight topbar-bearing captures (`d1*`, `settings*`, `d6-*`) were recaptured
on 2026-08-21 after the titlebar git block landed and show the project
selector with its branch, the dirty dot, and both `.gitops` chips
(`↑1 ↓0`, `⎇ 1 worktree`); the standalone `d11`/`d12*` screens carry no
cockpit titlebar, so their earlier captures remain valid.

The command palette added a `.tbtbtn` toggle to `.tbtools`; the eight
topbar-bearing images plus the new `palette` state were recaptured on
2026-08-21 at 1440x900 and visually reviewed (the palette capture shows the
prefix legend, all four sections, kbd hints, and the disabled Files row).

| File | State | Viewport | Mode | Locale |
| --- | --- | --- | --- | --- |
| [palette-1440x900-dark-en.png](palette-1440x900-dark-en.png) | palette | 1440x900 | dark | en |

## Known limitation

The authorized Browser runtime renders and verifies these pages but cannot
write PNG files, so this directory holds the reproducible harness rather than
committed images until an operator captures them. `tools/capture-d1-visual.sh`
deliberately stops at the same boundary.
