# Viden GUI 0.1.0-rc.3 D1 Design QA

canonical implementation source:
`crates/types/tests/fixtures/frontend-contract-v1/d1-main-cockpit.json`

same-state design reference source:
`apps/gui/evidence/0.1.0-rc.3/d1-design-reference.html`

same-state design reference screenshot:
`apps/gui/evidence/0.1.0-rc.3/d1-design-reference-canonical.png`

production implementation screenshot:
`apps/gui/evidence/0.1.0-rc.3/d1-main-dark.png`

same-state comparison evidence:
`apps/gui/evidence/0.1.0-rc.3/d1-design-reference-vs-actual.png`

historical visual reference:
`apps/gui/evidence/0.1.0-rc.3/accepted-target-dark-cockpit.png`

## Provenance

The QA pass/fail basis is the same-state pair:

- `d1-design-reference-canonical.png`: independent design-reference HTML,
  populated only from `d1-main-cockpit.json` facts, using the D1 design
  hierarchy and tokens without importing or calling the production D1 renderer.
  Its persistent skeleton follows the accepted target: narrow activity rail,
  central work surface, right Context Dock, composer/status regions, and no
  permanently visible Lane list column.
- `d1-main-dark.png`: production D1 renderer using the same canonical fixture
  facts.

Both target-size captures were produced through Browser-controlled Chrome from
`d1-target-viewport-capture.html`, which embeds the target page in a
`5140x2650` iframe viewport and captures the full page. Chrome capped the outer
browser viewport at `2560x1267`, so the exact target surface is the Browser
rendered nested viewport, not a resized non-Browser screenshot.

The historical accepted target PNG remains as a visual lineage reference only.
It predates rc.3 and is not a same-state acceptance target because it shows a
permission/composer scenario, provider/context/cost values, and transcript
content that are absent from `d1-main-cockpit.json`.

## Canonical State

The actual rc.3 captures use the committed canonical fixture only:

- one running Lane and one ACP session;
- `fallback` provider family with `test-local` model label;
- Core-owned workspace source on `codex/d1-cockpit-core`;
- `codegraph` MCP connected and `rust-analyzer` LSP ready;
- one modified workspace-change fact for `crates/types/src/runtime.rs`;
- one failed `viden-types` check at `crates/types/src/tests.rs:2500`;
- no approval request, provider health object, typed context budget, token
  usage, cost, signing, notarization, or release state.

## Capture Dimensions

- design reference: `5140x2650`
- production actual: `5140x2650`
- same-state comparison: `10280x2650`
- responsive dark: `1280x800`
- responsive narrow drawer open: `960x640`
- Context Dock bottom-state proof: `1280x800`
- compact readable: `1280x800`
- light and zh-CN target captures: `5140x2650`

## Findings

No actionable P0/P1/P2 findings remain for the canonical fixture rendering.

## Required Fidelity Surfaces

- Fonts and typography: hierarchy remains readable across target-size,
  responsive, compact, and zh-CN captures. Text wraps inside cards and dock
  rows without escaping buttons, composer controls, or status surfaces.
- Spacing and layout rhythm: topbar, activity rail, work surface, composer,
  Context Dock, and status bar retain the D1 proportions and dense cockpit
  rhythm.
- Colors and tokens: dark captures use the Aurora dark shell; light capture
  switches through resolved Ice light preferences. The independent reference
  and production renderer both use the registered token family and avoid a
  white outer web frame.
- State semantics: provider, context, cost, permission, check, and workspace
  facts match the canonical fixture. Missing Core facts remain visibly absent
  or unavailable instead of being fabricated for visual parity.
- Copy and locale: English and zh-CN captures localize user-facing D1 UI copy.
  File paths, commands, ids, and provider names remain literal by design.

## Runtime And Layout Proofs

- Browser-controlled same-state target captures use the exact `5140x2650`
  nested viewport.
- Browser-controlled responsive captures use exact `1280x800` and `960x640`
  clipped surfaces from the same harness.
- The `960x640` capture loads `d1-canonical-qa.html?drawer=open`; Browser DOM
  proof recorded `data-drawer-open="true"` on the dock and
  `aria-expanded="true"` on the toggle.
- The supplemental bottom-state capture loads
  `d1-canonical-qa.html?contextScroll=bottom`; Browser DOM proof recorded
  `scrollTop=183`, `scrollHeight=1376`, and `clientHeight=1193`, with MCP,
  LSP, task checklist, running task, and unavailable context facts reachable in
  the Context Dock tail.
- Accessibility provenance was collected from
  `http://127.0.0.1:4173/evidence/0.1.0-rc.3/d1-canonical-qa.html`.
- The accessibility probe found zero unnamed buttons, a present transcript
  `role="log"`, nine landmarks, no document horizontal overflow, and only Vite
  debug connection messages in the console log sample.

## Follow-up Notes

- The accepted target should not be used as same-state evidence unless it is
  regenerated from `d1-main-cockpit.json`.
- Keep future D1 pass/fail visual QA based on independent design-reference
  state versus production actual state.

final result: passed
