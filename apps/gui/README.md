# Viden GUI

Chinese version: [README.zh-CN.md](README.zh-CN.md)

This directory is the GUI implementation track for Viden. At
`0.1.0-alpha.1` it is a contract and framework-gate workspace, not a
production desktop app. The GUI must remain framework-neutral until the
Tauri/GPUI gate proves which runtime can satisfy the same Core fixture,
accessibility, IME, transcript, and packaging requirements.

## Frozen input

| Field | Value |
| --- | --- |
| GUI component version | `0.1.0-alpha.1` |
| Minimum Core version | `0.3.0` |
| Supported frontend schemas | `[1]` |
| Common branch base | `afd6fcc9aaf3039ba79bb4588ed33bf1547209f5` |
| Contract payload | `5bd2b80b0953f4194d082940a7b9164c7231ca2d` |
| Required Core capabilities | 15 values from `CORE_CLIENT_CAPABILITIES` |
| Built-in locales | `en`, `zh-CN` |
| Appearance | 5 skins, 8 valid skin/mode pairs, 3 densities, 3 motion policies |

The active machine-readable manifest is
[release-manifest.toml](release-manifest.toml). Its immutable alpha snapshot is
[manifests/0.1.0-alpha.1.toml](manifests/0.1.0-alpha.1.toml); both files must
remain byte-equivalent for this release checkpoint.

## Design source order

Visual and interaction inventory starts from the accepted design hierarchy:

1. `docs/viden-design/Viden/index.html`
2. `docs/viden-design/Viden/GUI/Viden - 设计稿索引 (GUI).html`
3. `docs/viden-design/Viden/GUI/Viden - 组件库 (GUI).html`
4. `docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html` (D1)

D11 first-run intake, D4 lane creation, and D6 recovery/empty states are
subordinate screens under `docs/viden-design/Viden/GUI/pages/`. They inform the
operator loop but do not replace D1 as the desktop cockpit baseline.

The deterministic design revision also includes the registered component
semantics and the local sources actually consumed by these screens:
`docs/DESIGN-REF.md`, `GUI/gui-kit.css`, `GUI/gui-icons.jsx`,
`GUI/gui-titlebar.jsx`, `GUI/gui-statusbar.jsx`, `GUI/gui-inbox.jsx`, and
`GUI/gui-settings.jsx`. The manifest records the exact ordered list; archived
or mock sources are excluded.

## Core boundary

GUI code may depend on `viden-core` and GUI-owned framework/platform code only.
The allowed Core entry points are:

- `CoreClient`, `CoreTransport`, `StatefulCoreClient`, and `LocalCoreTransport`;
- `CoreHandshake`, schema/capability constants, command/event envelopes,
  snapshot, replay, transcript paging, and `RuntimeViewState`;
- frontend-neutral domain records re-exported by `viden-core`.

GUI must not import `viden_core::legacy`, `viden-runtime`, `viden-provider`,
`viden-tools`, `viden-permissions`, `viden-session`, `viden-workflows`,
`viden-context`, or config internals. Every mutation is sent as a
`RuntimeCommand`; visible success waits for `CommandAccepted` and the
subsequent ordered state events.

## Inventory against Core `0.3.0`

| GUI area | Design intent | Core `0.3.0` status | GUI handling |
| --- | --- | --- | --- |
| D11 project intake | project probe, recent project/session, provider health, config preview/confirm, starter lanes | Provider/model configuration and provider health exist; typed intake, recent-history discovery, and starter-lane creation remain separate gaps | Block production D11 until `GUI-CORE-001`, `GUI-CORE-002`, and `GUI-CORE-007` land |
| D4 lane creation | typed role, route, gate strength, mutation policy, target, budget, worktree preview, lane receipt | Typed lane records exist; no create-lane/worktree-preview/lane-created command is exported | Block production D4 until `GUI-CORE-002` lands |
| D1 cockpit | activity rail, lane rail, streaming transcript/tool rows, permission dock, worktree board, evidence/gate/context/cost panels, settings entry | Stream/tool/approval/queue/task/lane/evidence/merge/context/cost/preferences facts exist; worktree/lane lifecycle, diff/apply facts, and stable audit timeline are incomplete | Tasks 2-3 may replay fixtures; production D1 waits on `GUI-CORE-002`, `GUI-CORE-003`, `GUI-CORE-004`, and `GUI-CORE-006` |
| Permission dock | scoped approve/deny, risk, target, expiry, default action, audit id | `ApprovalRequestView` and `RespondToApproval` exist | Usable through Core; GUI cannot execute tools directly |
| D6 recovery | empty cockpit, connecting, disconnected, agent stopped, budget exhausted, gate queue clear, reconnect/restart/close actions | Runtime errors, CoreClient recovery, context budget facts, queue/gate facts exist; structured connection/lane lifecycle recovery commands are missing | Read-only/error rendering can start; actionable recovery waits on `GUI-CORE-003` |
| Locale and skin system | `en`/`zh-CN`, Aurora/Ice/Mono/Amber/Phosphor, dark/light constraints, density, motion | `RuntimeSnapshot.ui_preferences: ResolvedUiPreferences` exists with safe fallback diagnostics; mutation and persistence commands do not | GUI renders resolved Core preferences; ephemeral spike controls are allowed, while production controls wait on `GUI-CORE-005` |

Open requests are recorded in [contract-requests.md](contract-requests.md) and
[contract-requests.zh-CN.md](contract-requests.zh-CN.md). GUI must not close
those gaps with private reducers or direct runtime access.

The seven open requests block only the production screens named in their
rows. They do not block the framework-neutral, fixture-only Tasks 2-3 or their
evidence; no spike result authorizes production mutation or persistence.

## Next implementation gate

Task 2 builds a framework-neutral replay harness over `CoreClient` and the
shared `d1-vertical-slice` fixture. It must prove ordered replay, snapshot
recovery, transcript paging anchors, and projection parity before either Tauri
or GPUI production code is introduced. Task 3 may compare equal candidates on
the same fixture while Core requests remain open.
