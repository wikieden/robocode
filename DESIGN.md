# Viden Design

This root document is a compact pointer to the current design system. Detailed
product and implementation requirements belong under `docs/`; visual and
interaction decisions belong under `docs/viden-design/Viden/`.

Chinese reference: [Viden design adoption](docs/viden-design-adoption.zh-CN.md)

Last updated: 2026-07-19

## Source Precedence

When two documents or images disagree, use this order:

1. `docs/viden-design/Viden/docs/SPEC.md` for accepted decisions and open
   questions, and `docs/viden-design/Viden/docs/screens-status.js` for screen
   status.
2. `docs/viden-design/Viden/tokens.css` for visual tokens.
3. The live TUI/GUI prototypes and their component kits for layout, component,
   state, and interaction behavior.
4. `docs/viden-design/reference-shots/` for convenient review snapshots.
5. Product specifications and roadmaps as derived implementation guidance.
6. Generated previews and release screenshots as implementation or historical
   evidence only.

The old `docs/design/canvas-export`, `docs/previews/`, individual files under
`docs/viden-design/Viden/screenshots/`, and generated concept art are not
current visual targets.

## Canonical TUI Sources

- Integrated prototype:
  `docs/viden-design/Viden/TUI/Viden - 统一原型 (TUI).html`
- Component kit:
  `docs/viden-design/Viden/TUI/Viden - 组件库 (TUI).html` and
  `docs/viden-design/Viden/TUI/tui-kit.css`
- Interaction contract:
  `docs/viden-design/Viden/TUI/pages/Viden - T4 交互规则 (TUI).html`
- Review snapshots:
  `docs/viden-design/reference-shots/TUI-统一原型驾驶舱.png` and
  `docs/viden-design/reference-shots/TUI-组件库.png`

The TUI is a dense terminal cockpit. It uses Normal, Insert, and Overlay modes;
keyboard-first interaction; one-layer-at-a-time `Esc`; and truecolor to ANSI
degradation. The canonical approval gate uses `1` through `4`, arrow keys,
`Enter`, and safe deny on `Esc` or timeout. `Ctrl-C` interrupts active work; it
is not an approval answer or a generic exit shortcut.

## Canonical GUI Sources

- Flagship cockpit:
  `docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html`
- Component kit:
  `docs/viden-design/Viden/GUI/Viden - 组件库 (GUI).html` and
  `docs/viden-design/Viden/GUI/gui-kit.css`
- Workflow screens: D2 Decision Center, D4 Lane creation, D10 Lane Monitor,
  D11 onboarding, D12 conflict bounce, D13 Fleet, and D14 Audit Timeline under
  `docs/viden-design/Viden/GUI/pages/`.
- Review snapshots: the matching `GUI-*` files under
  `docs/viden-design/reference-shots/`.

The GUI uses D1 as its shell: a fixed activity rail, a floating or pinned lane
rail, the central work surface, an Environment/context rail, and on-demand dock
or inspector layers. Pre-execution permission stays in the inline permission
dock. Post-output decisions belong in D2, merge conflict recovery in D12, and
append-only operational history in D14. These are distinct surfaces and data
contracts.

## Shared Rules

- TUI and GUI consume the same Core commands, events, snapshots, replay, and
  identity model. Frontends do not invent business state.
- All visual values come from `tokens.css`; new reusable components must be
  registered in `DESIGN-REF.md` and the appropriate kit.
- A screenshot is a review aid, not a substitute for the live prototype,
  interaction contract, responsive states, or accessibility checks.
- Current implementation screenshots may demonstrate behavior or regressions,
  but they never override the accepted design source.
- Changes to the design source follow the package checklist, status registry,
  guards, and changelog.

See [Viden Design Adoption](docs/viden-design-adoption.md) for the detailed
screen map and implementation boundary.
