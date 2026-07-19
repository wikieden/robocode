# Viden Design Adoption And Visual Source Map

Chinese version: [viden-design-adoption.zh-CN.md](viden-design-adoption.zh-CN.md)

Last updated: 2026-07-19

## Decision

The latest design directory under `docs/viden-design/Viden/` is the accepted
source for Viden product visuals and interaction behavior. Active product,
TUI, and GUI documents must derive from it instead of preserving independent
visual targets.

This decision does not by itself rename Rust crates, binaries, configuration
paths, release artifacts, or compatibility commands. Those remain migration
work.

## Source Precedence

Use the following order whenever sources disagree:

1. `docs/viden-design/Viden/docs/SPEC.md` and
   `docs/viden-design/Viden/docs/screens-status.js` define accepted decisions,
   open questions, roadmap status, and the screen registry.
2. `docs/viden-design/Viden/tokens.css` defines visual values.
3. Live prototypes plus `tui-kit.css` / `gui-kit.css` define current layout,
   components, states, and interaction behavior.
4. `docs/viden-design/reference-shots/` provides review snapshots of those live
   sources.
5. Functional documents and roadmaps translate the design into requirements.
6. Generated previews and release screenshots are implementation or historical
   evidence only.

If a reference shot conflicts with a live prototype, the live prototype wins.
If a functional document conflicts with `SPEC.md`, `SPEC.md` wins.

The deleted `docs/design/canvas-export`, old files under
`docs/viden-design/Viden/screenshots/`, and `docs/previews/` must not be used as
current visual targets.

## TUI Target Map

| Purpose | Canonical source | Review snapshot |
| --- | --- | --- |
| Integrated cockpit and welcome | `docs/viden-design/Viden/TUI/Viden - 统一原型 (TUI).html` | `docs/viden-design/reference-shots/TUI-统一原型驾驶舱.png` |
| Reusable components and states | `docs/viden-design/Viden/TUI/Viden - 组件库 (TUI).html` and `docs/viden-design/Viden/TUI/tui-kit.css` | `docs/viden-design/reference-shots/TUI-组件库.png` |
| Input, focus, overlay, and approval behavior | `docs/viden-design/Viden/TUI/pages/Viden - T4 交互规则 (TUI).html` | Use the integrated prototype and component snapshot together |
| Screen inventory | `docs/viden-design/Viden/TUI.html` and `docs/viden-design/Viden/docs/screens-status.js` | No independent target image |

The interaction contract is Normal / Insert / Overlay, keyboard-first with an
optional mouse, `Esc` unwinding one layer, and `Ctrl-C` interrupting active
work. The four-option approval gate uses `1` through `4`, arrow keys, `Enter`,
and safe deny on `Esc` or timeout.

## GUI Target Map

| Purpose | Canonical source | Review snapshot |
| --- | --- | --- |
| D1 desktop cockpit shell | `docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html` | `docs/viden-design/reference-shots/GUI-D1-桌面驾驶舱.png` |
| GUI component vocabulary | `docs/viden-design/Viden/GUI/Viden - 组件库 (GUI).html` and `docs/viden-design/Viden/GUI/gui-kit.css` | `docs/viden-design/reference-shots/GUI-KIT-组件库.png` |
| D2 Decision Center | `docs/viden-design/Viden/GUI/pages/Viden - D2 决策中心 (GUI).html` | `docs/viden-design/reference-shots/GUI-D2-决策中心.png` |
| D4 Lane creation | `docs/viden-design/Viden/GUI/pages/Viden - D4 Lane创建流程 (GUI).html` | `docs/viden-design/reference-shots/GUI-D4-Lane创建流程.png` |
| D10 Lane Monitor | `docs/viden-design/Viden/GUI/pages/Viden - D10 Lane监视器 (GUI).html` | `docs/viden-design/reference-shots/GUI-D10-Lane监视器.png` |
| D11 onboarding | `docs/viden-design/Viden/GUI/pages/Viden - D11 首启与项目接入 (GUI).html` | `docs/viden-design/reference-shots/GUI-D11-首启与项目接入.png` |
| D12 conflict bounce | `docs/viden-design/Viden/GUI/pages/Viden - D12 集成闸冲突退回 (GUI).html` | `docs/viden-design/reference-shots/GUI-D12-集成闸冲突退回.png` |
| D13 Fleet and workflow | `docs/viden-design/Viden/GUI/pages/Viden - D13 Fleet 编排与 Workflow (GUI).html` | `docs/viden-design/reference-shots/GUI-D13-Fleet编排.png` |
| D14 audit timeline | `docs/viden-design/Viden/GUI/pages/Viden - D14 审计与时间线 (GUI).html` | `docs/viden-design/reference-shots/GUI-D14-审计与时间线.png` |
| D5 gallery and D6 system states | Matching files under `docs/viden-design/Viden/GUI/pages/` | Matching `GUI-D5-*` and `GUI-D6-*` snapshots |

D7, D8, and D9 are roadmap screens. D2h, D3, and Pip are concepts or decorative
extensions. A built design artifact is not automatically a first-release
requirement; `screens-status.js` and `SPEC.md` decide its status.

The D1 cockpit is the GUI shell: fixed activity rail, floating or pinned lane
rail, central work surface, Environment/context rail, and on-demand dock or
inspector. Permission is an inline, pre-execution dock. D2 owns asynchronous
gate and review decisions, D12 owns merge conflict recovery, and D14 owns the
append-only audit trail. Evidence remains a linked artifact, not a synonym for
the audit log.

## Implementation Rules

- TUI and GUI consume shared Core facts, commands, events, snapshots, replay,
  tasks, lanes, permissions, context, cost, evidence, and audit identity.
- Frontends must not invent business state or create a second execution path.
- Visual values come from `tokens.css`; reusable components use the registered
  component vocabulary before adding local styles.
- Current implementation previews may be compared with target snapshots for
  regression review, but they do not become a new design source.
- Historical release documents retain the meaning of their original evidence;
  do not replace historical screenshots with current target images.
- Any accepted deviation records the affected source, reason, owner, and
  follow-up gate.

## Governance

Changes to the design directory must update the relevant live source and, when
required, `DESIGN-REF.md`, `SPEC.md`, `screens-status.js`, token baselines,
design checks, and the design changelog. Consumer documents link back here
instead of copying a separate visual baseline.
