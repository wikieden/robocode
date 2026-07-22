# Viden TUI 0.3.3 Native / ACP Interaction

Chinese version: [release-tui-0.3.3-native-acp.zh-CN.md](release-tui-0.3.3-native-acp.zh-CN.md)

TUI `0.3.3` consumes the immutable Core `0.3.4` checkpoint
`54965464e87860f9c39a1fb656c2f528e354da94`. Core remains authoritative for
Lane identity, agent readiness, session ownership, process execution,
persistence, and recovery.

## Operator Flow

- Press `n` in Normal mode to enter the first task for a Viden-native Lane.
  The TUI waits for `WorkspaceEligibilityUpdated`, then sends
  `PreviewDefaultStarterLane`. It does not submit the task until Core has
  emitted both `StarterLanePreviewed` and `StarterLaneCreated`.
- Select a Lane and enter `/acp` to open the keyboard-first ACP picker. Existing
  sessions appear before Codex, Claude, Kiro, or other Core-discovered ACP
  adapters. Arrow keys move, Enter selects, and Esc returns from task entry to
  the picker before closing it.
- A `Ready` adapter opens task entry. `ProbeRequired` asks Core to probe it.
  Install, authentication, and unavailable states remain visible but cannot
  start a process.
- Selecting an existing ACP session focuses it. Composer input sends
  `SendAgentSessionInput`; `Ctrl-C` sends `CancelAgentSession` with the exact
  Core-published owner. Press `r` on a failed or cancelled session row to send
  `RetryAgentSession`.

The composer stays editable during running and approval states. English and
Simplified Chinese labels are resolved from the existing Core-owned locale
preference; the TUI does not persist a separate locale or skin.
