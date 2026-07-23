# Viden GUI 0.1.0-rc.3 D1 Certification Evidence

This directory records local, unpublished certification evidence for the
canonical D1 cockpit. It is a GUI-only checkpoint and is not a tag, published
release, signed artifact, notarized app, or Homebrew update.

## Frozen Inputs

| Field | Value |
| --- | --- |
| GUI version | `0.1.0-rc.3` |
| Minimum Core version | `0.3.5` |
| Core final checkpoint | `f7fe1b31dfb237e4062209767a7051c2b2c68b93` |
| Core code checkpoint | `17fa2071398d5eaf30045257163d57d22d99177b` |
| D1 fixture | `crates/types/tests/fixtures/frontend-contract-v1/d1-main-cockpit.json` |
| D1 fixture SHA-256 | `f96ba30cc6e80aa52cb15a2fd1f03c082487a3cd4779c25f61e42ee1548e1e3b` |
| Same-state design reference | `apps/gui/evidence/0.1.0-rc.3/d1-design-reference.html` |
| Design reference capture SHA-256 | `f9209057b5538278da861e04bb43b891438802d9a41dcb5f1476b341b93dc11c` |
| Same-state comparison SHA-256 | `d27302d81afaeadfc156513eed30d251ff09194b1b3392010baeac5602ced5e8` |
| Context Dock bottom capture SHA-256 | `0179f20ac53a484dfb0194392d206d7e182eae1d33d0fd0e94f43c1e2fcc6c30` |
| Historical accepted target | `.worktrees/d1-cockpit-acceptance/apps/gui/evidence/design-qa/d1-target-dark-cockpit.png` |
| Historical target SHA-256 | `d4c97aa4ebe603eddd290785a0e632fd41b72a94de5e7ccb6206352bb0f37e36` |
| Token revision | `826826ee6ddab845897472701add67ee9f55aff25af539651e6089553b7e6398` |
| Locale revision | `65dc527f6a66b12985491a3c51d75b076e624e7b809e91987d6d64a9e0f37f25` |
| Design revision | `5f7a39d10762eaf7c6433599812a6d20c38aa8a1d66c5e0bde3a8bdd0d9fd0f6` |

## Browser Capture Matrix

The production capture source is
`apps/gui/evidence/0.1.0-rc.3/d1-canonical-qa.html`. The same-state design
reference source is `d1-design-reference.html`, an independent HTML
representation populated from the same committed `d1-main-cockpit.json` facts
without importing or calling the production renderer. Both render one running
Lane, one ACP session, Core-owned workspace source, codegraph MCP,
rust-analyzer LSP, one workspace change, and one failing check. Neither
fabricates provider health, extra MCP/LSP success, token usage, cost, approval,
signing, notarization, or release state.

Exact target-size evidence uses `d1-target-viewport-capture.html`, which
Browser-renders the selected source in a `5140x2650` iframe viewport and
captures the full page. Chrome capped the outer browser viewport at
`2560x1267` during this run, so the exact target surface is the nested Browser
viewport recorded by the harness.

Required captures:

| File | State | Expected size |
| --- | --- | --- |
| `d1-design-reference-canonical.png` | EN, Aurora dark, independent design reference, target-size | 5140x2650 |
| `d1-main-dark.png` | EN, Aurora dark, regular, target-size | 5140x2650 |
| `d1-responsive-1280x800-dark.png` | EN, Aurora dark, regular, responsive | 1280x800 |
| `d1-responsive-960x640-dark-drawer.png` | EN, Aurora dark, regular, Context Dock drawer open | 960x640 |
| `d1-context-dock-bottom-1280x800.png` | EN, Aurora dark, regular, Context Dock internally scrolled to lower facts | 1280x800 |
| `d1-main-light.png` | EN, Ice light, regular, target-size | 5140x2650 |
| `d1-main-zh-CN.png` | zh-CN, Aurora dark, regular, target-size | 5140x2650 |
| `d1-compact-readable.png` | EN, Aurora dark, compact, responsive | 1280x800 |
| `d1-design-reference-vs-actual.png` | same-state design reference left, production actual right | 10280x2650 |

## Same-State QA Result

The pass/fail comparison is
`d1-design-reference-vs-actual.png`: same-state design reference on the left,
production actual on the right, both from the canonical Core fixture. The
historical accepted target remains in this directory only as visual lineage. It
is not a same-state target and is not used as pass evidence.

The `960x640` drawer capture is generated from
`d1-canonical-qa.html?drawer=open` through the exact viewport harness. Direct
Browser DOM proof recorded `data-drawer-open="true"` and `aria-expanded="true"`.
The supplemental `1280x800` bottom capture is generated from
`d1-canonical-qa.html?contextScroll=bottom`; Browser DOM proof recorded
`scrollTop=183`, `scrollHeight=1376`, and `clientHeight=1193`, with MCP, LSP,
task checklist, running task, and unavailable context facts visible in the dock
tail.

## Local Artifact Status

`performance.json` contains measured local browser evidence only. Empty or
`null` fields mean the metric was unavailable from the authorized local capture
path and was not invented.
