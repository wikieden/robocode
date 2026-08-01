# Core, TUI, and GUI Independent Release Train Implementation Plan Index

Chinese: [2026-07-19-independent-release-plan-index.zh-CN.md](2026-07-19-independent-release-plan-index.zh-CN.md)

> **For agentic workers:** Use `superpowers:executing-plans` before implementing any product line. Use `superpowers:using-git-worktrees` and `superpowers:finishing-a-development-branch` for concurrent branch setup and completion.

**Goal:** Branch independent TUI and GUI release lines from one immutable Core contract checkpoint and deliver a verified local operator loop through the I0, I1, and I2 integration gates.

**Architecture:** Core exclusively owns business facts, permissions, persistence, and side effects. TUI and GUI are independent clients that communicate only through `CoreClient`. Each line has its own SemVer, while frontend manifests pin the Core SHA, schema, and capabilities.

**Tech Stack:** Rust workspace, JSONL plus rebuildable SQLite, Ratatui/Crossterm, the GUI framework-gate winner, Serde fixtures, and bilingual Markdown.

## Global Constraints

- Inspect design sources in this order: global `index.html`, client design index, component library, then the TUI unified prototype or GUI desktop cockpit.
- `tokens.css` is the numeric visual source of truth. Model and persist `en`, `zh-CN`, skin, mode, density, and motion from I0.
- Before the Core checkpoint is frozen, TUI and GUI may only build spikes, fixture consumers, and local UI state; they may not invent business contracts.
- Branch ownership is Core `crates/**`, TUI `apps/tui/**`, and GUI `apps/gui/**`. Route cross-domain gaps back through the Core branch.
- Merge and verify in the fixed order Core → TUI → GUI, rerunning shared fixture, migration, and workspace gates at every step.
- Do not rewrite historical release evidence. Point active visual documents at the current design package and label old previews as archives only.

## Plan Set

1. [Core 0.3 runtime contract](2026-07-19-core-0.3-runtime-contract.md)
2. [TUI 0.3 thin client](2026-07-19-tui-0.3-thin-client.md)
3. [GUI 0.1 desktop cockpit](2026-07-19-gui-0.1-desktop-cockpit.md)
4. [Independent release integration](2026-07-19-independent-release-integration.md)

Design specification: [Core, TUI, and GUI independent release train](../specs/2026-07-19-independent-core-tui-gui-release-train-design.md)

## Branch Topology and Gates

```text
main@baseline
  └─ codex/v3-core-runtime
       ├─ I0: Core 0.3.0 / frontend-contract-v1 / immutable SHA
       ├─ codex/v3-tui-client  -> TUI 0.3.0-alpha.1 -> 0.3.0 -> 0.3.1
       └─ codex/v3-gui-client  -> GUI 0.1.0-alpha.1 -> beta.1 -> 0.1.0

integration: Core 0.3.0 -> I0 -> Core 0.3.1 + TUI 0.3.0 + GUI beta.1 -> I1
             -> Core 0.3.2 + TUI 0.3.1 + GUI 0.1.0 -> I2
```

## Definition of Done

- All three components have independent versions and changelogs, and TUI/GUI manifests record the exact Core checkpoint.
- Core, TUI, and GUI reduce the same fixture to the same business facts; stream drops, gaps, reconnects, and migrations are tested.
- TUI no longer owns engine/provider/Git/process authority, and GUI uses only `CoreClient`.
- `en`/`zh-CN` key parity, all eight valid skin/mode combinations, density, reduced motion, CJK, keyboard, and accessibility gates pass.
- One real local task completes request → work → test/review → evidence → gate → apply/recovery with append-only audit evidence.
- `cargo test --workspace --quiet` passes and active documentation plus non-obvious code comments match the final behavior.
