# Viden Design Adoption

Chinese version: [viden-design-adoption.zh-CN.md](viden-design-adoption.zh-CN.md)

Last updated: 2026-06-26

## Decision

Viden is the active product direction. The older RoboCode product framing is a
legacy implementation plan and should no longer guide product, TUI, or GUI
decisions.

This is a product and design decision first. It does not immediately rename
Rust crates, binaries, package artifacts, transcript paths, or compatibility
commands. Those names remain implementation migration work until a dedicated
rename plan is approved.

## Accepted Design Source

The accepted design source is:

- `docs/viden-design/Viden/CLAUDE.md`
- `docs/viden-design/Viden/docs/DESIGN-REF.md`
- `docs/viden-design/Viden/tokens.css`
- `docs/viden-design/Viden/TUI/tui-kit.css`
- `docs/viden-design/Viden/screenshots/`
- `docs/viden-design/Viden/Core/`
- `docs/viden-design/Viden/TUI/`
- `docs/viden-design/Viden/GUI/`

The old `docs/design/canvas-export` import is deleted and must not be used as a
design source.

## Product Mapping

| Legacy term | Viden direction |
| --- | --- |
| RoboCode product | Viden product |
| RoboCode cockpit | Viden cockpit |
| RoboCode TUI / GUI | Viden TUI / GUI |
| RoboCode visual identity | Viden Aurora identity |
| Generated canvas export | Reviewed Viden design source |

Implementation-specific names such as crates, binary names, config paths, and
release artifacts can keep their current names until a migration plan covers
backward compatibility, Homebrew, GitHub releases, config migration, and user
data migration.

## Target Screens

Primary TUI target:

- `docs/viden-design/Viden/screenshots/cockpit-final.png`
- `docs/viden-design/Viden/screenshots/welcome-watcher.png`
- `docs/viden-design/Viden/screenshots/lane-monitor-wide.png`

Primary GUI target:

- `docs/viden-design/Viden/screenshots/d1v2.png`
- `docs/viden-design/Viden/screenshots/s13.png`

These images define visual direction and information architecture, not pixel
implementation by themselves. Implementation must still pass component, token,
screenshot, and runtime-state acceptance gates.

## Implementation Rules

- TUI and GUI must consume the shared runtime facts: `RuntimeSnapshot`, event
  stream, tasks, lanes, approvals, provider health, context, cost, and evidence.
- UI must not invent business state that the runtime cannot replay.
- New UI work must use the Viden token source and component vocabulary before
  adding new styles.
- When source designs and current implementation disagree, the Viden source
  wins for product direction; current implementation wins only for compatibility
  until the migration is explicitly planned.
- The product name shown in user-facing design and planning docs should be
  Viden. RoboCode should appear only when discussing legacy implementation
  names or migration compatibility.

## Open Migration Work

1. Decide whether and when to rename the binary, crates, config directories,
   release artifacts, and Homebrew formula.
2. Define compatibility policy for existing `robocode` commands and
   `.robocode` user data.
3. Convert active PRD, roadmap, TUI, and GUI documents from RoboCode framing to
   Viden framing.
4. Build screenshot baselines from the accepted Viden target images.
5. Add a release gate that fails when UI screenshots drift from accepted Viden
   targets without a documented deviation.
