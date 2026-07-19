# Independent Core, TUI, and GUI Release Train Design

Chinese version: [2026-07-19-independent-core-tui-gui-release-train-design.zh-CN.md](2026-07-19-independent-core-tui-gui-release-train-design.zh-CN.md)

## 1. Goal

Viden will evolve through three independent SemVer lines: Core, TUI, and GUI. Core first combines TUI requirements, GUI requirements, and Core-native reliability, safety, and evolution work into a compatible contract and immutable checkpoint. TUI and GUI then develop concurrently from that checkpoint. Component versions do not need to align or ship together, but every frontend release must declare and verify its Core compatibility range.

This round delivers an operable local loop:

- Core provides `frontend-contract-v1`, multi-lane runtime, CoreClient, recovery semantics, and shared fixtures.
- TUI becomes a thin client with stable interaction and the canonical unified-prototype cockpit.
- GUI passes the framework gate and turns the desktop cockpit P0 path into a running product.
- English, Simplified Chinese, skins, light/dark modes, and density are modeled as system capabilities from the first round. Configuration surfaces open incrementally instead of relying on scattered constants.

Complete P1 beta, team collaboration, Fleet, remote targets, and plugin-contributed UI are out of scope.

## 2. Chosen Approach

### 2.1 Three independent release lines

Use independent releases with coordinated integration gates:

- Core, TUI, and GUI maintain separate SemVer and changelogs.
- Core may release contract or runtime capability changes independently.
- TUI and GUI may release independently according to their needs.
- Every TUI and GUI build records its minimum Core version, supported wire schema, base Core checkpoint SHA, and required capabilities.
- Integration baselines are validated together without forcing synchronized releases.

A unified product version was rejected because it couples UI cadence to Core internals. Fully loose development was also rejected because frontends would otherwise guess the contract without a compatibility matrix or checkpoint.

### 2.2 Keep three version identities separate

Distinguish:

1. Component SemVer, such as Core `0.3.0`, TUI `0.2.0`, and GUI `0.1.0`.
2. Wire/schema version, such as `frontend-contract-v1` and `schema_version = 1`.
3. Immutable implementation checkpoint, represented by an exact Git SHA.

Clients discover capabilities before issuing commands. They do not infer support from SemVer strings. A breaking wire change raises the schema major and ships with migration, legacy fixtures, and three-surface parity evidence.

## 3. Design Sources and Inspection Order

All TUI and GUI requirements, acceptance work, and visual review follow this order:

1. `docs/viden-design/Viden/index.html`: global design entry and Core/TUI/GUI grouping.
2. `TUI/Viden - 设计稿索引 (TUI).html` and `GUI/Viden - 设计稿索引 (GUI).html`: screen hierarchy, status, and roadmap boundary.
3. `TUI/Viden - 组件库 (TUI).html` and `GUI/Viden - 组件库 (GUI).html`: reusable component and interaction vocabulary.
4. TUI uses `TUI/Viden - 统一原型 (TUI).html` as its primary product prototype.
5. GUI uses `GUI/Viden - 桌面驾驶舱 (GUI).html` as its primary entry and visual master.

`tokens.css` is the single source of numerical and visual tokens. `tui-kit.css` and `gui-kit.css` are the component sources. `reference-shots/` is convenient comparison evidence and never overrides live HTML, component kits, or the design registry.

## 4. Independent Versions for This Round

| Integration baseline | Core | TUI | GUI | Joint outcome |
| --- | --- | --- | --- | --- |
| `I0 · Contract` | `0.3.0` | `0.2.0-alpha.1` | `0.1.0-alpha.1` | Freeze `frontend-contract-v1`; both clients replay shared fixtures; GUI completes the framework gate and TUI completes the client spike |
| `I1 · Operable` | `0.3.1` | `0.2.0` | `0.1.0-beta.1` | Core owns multi-lane effects; TUI cockpit works; GUI desktop shell and D11/D4/D1/permission/recovery vertical path work |
| `I2 · Local Loop` | `0.3.2` | `0.2.1` | `0.1.0` | A local request → work → test/review → evidence → gate → apply/recovery loop with identical facts across all surfaces |

These are target release lines for the round. A patch release on one line does not force the other lines to change version; only the compatibility matrix and integration baseline record are updated.

## 5. Core Release Packages

### Core 0.3.0: Frontend Contract v1

- Add versioned envelopes, stream/session identity, cursors, and capabilities to commands, events, and snapshots.
- Define sequence, duplicate, out-of-order, gap, snapshot, replay, and reconnect semantics.
- Provide a transport-neutral `CoreClient`; frontends do not create or mutate `SessionEngine`.
- Replace stringly task, lane, role, route, gate strength, mutation policy, target, and budget fields with typed contracts.
- Enrich approvals with risk, target, scope, policy reason, expiry, default action, and stable audit id.
- Define paged/streamed transcript rows and stable scroll anchors.
- Ship migrations and a shared parity corpus.

### Core 0.3.1: Multi-lane Runtime

- Move authoritative worktree, terminal/tmux/PTY, accept/apply, and conflict recovery effects into Core.
- Implement a lane/session/task-keyed supervisor so waiting, approvals, cancellation, or failure in one lane do not block others.
- Add explicit owners to queue, cancel, approval, error, and command receipts.
- Provide project probing, configuration preview/confirmation, provider/model health, credential handles, and lane lifecycle commands.

### Core 0.3.2: Trusted Local Loop

- Complete P0 evidence, MergeGate, apply/recovery, history/replay, and append-only audit semantics.
- Provide minimal local `handoff`, `review_request`, `contract`, and `dependency` contracts while deferring complex cross-team orchestration.
- Validate the structured fact chain through a real local task. Frontends never infer success from transcript text.

## 6. TUI Release Packages

### TUI 0.2.0-alpha.1: CoreClient Proof

- Branch from the Core 0.3.0 checkpoint.
- Replay shared fixtures and prove the same `RuntimeViewState` facts as Core.
- Do not perform the production visual migration or create private TUI business facts.

### TUI 0.2.0: Canonical Cockpit

- Send only `RuntimeCommand`, consume `RuntimeEvent`, and render `RuntimeViewState`.
- Remove direct engine, provider, permission store, Git, process, and authoritative lane side effects.
- Implement welcome → `/setup` or `/lanes` → cockpit.
- Implement Normal / Insert / Overlay, layered Escape, and Ctrl-C as current-work interruption only.
- Support multiline composer, internal scroll, bracketed paste, CJK width, and independent scrollback.
- Preserve the 0.1.30 zero-bug regression baseline.

### TUI 0.2.1: Local Supervision Loop

- Add lane/session switching, selector-first navigation, global jump, approval, task/DAG, evidence, MergeGate, context/cost, and recovery actions.
- Changes, Evidence, and Context are the primary detail surfaces; Inbox and Fleet only show summaries supported in this round.
- Core fact events confirm every success state.

## 7. GUI Release Packages

### GUI 0.1.0-alpha.1: Framework Gate

- Compare Tauri and GPUI using the same Core fixture and the same D1 vertical slice.
- Measure input latency, event-to-visible, frame work, 10,000 events, 50,000 transcript rows, CJK IME, keyboard operation, accessibility, three-platform support, signing/updating/credentials/crash recovery, and long-term maintenance.
- Choose Tauri if GPUI fails any IME, accessibility, three-platform, bounded transcript, or long-term-fork gate.

### GUI 0.1.0-beta.1: Desktop Cockpit P0

- The GUI entry is the D1 cockpit represented by `Viden - 桌面驾驶舱 (GUI).html`.
- With no project, enter D11 for project probing, mode selection, `viden.toml` preview/confirmation, and starter-lane creation, then enter D1.
- From D1, integrate D4 lane creation, Conversation, Activity Rail, Environment, composer, stream/tool cards, permission dock, and D6 recovery states.
- D2 contains only the minimum decision/permission slice needed for the local loop. The full Decision Center is next-round P1.
- Depend only on CoreClient and frontend-neutral contracts.

### GUI 0.1.0: Local Operator Loop

- Complete project → lane → session → task navigation and recovery.
- Complete P0 diff/test/evidence, MergeGate, apply/recovery, and history/replay surfaces.
- Finish one auditable real local development task.
- Pass visual, CJK, keyboard, accessibility, and performance gates without requiring team, Fleet, or remote features.

## 8. Localization System

### 8.1 Ownership

- Core emits structured facts, stable message keys, parameters, and error codes, not English sentences that clients must parse.
- TUI and GUI render localized strings while sharing locale ids, fallback rules, and key-parity tests.
- Business logs retain raw facts. Switching locale never rewrites transcripts, event logs, or audit logs.

### 8.2 Support in this round

- Built-in `en` and `zh-CN`.
- Startup precedence: explicit CLI/config → stored user preference → system locale → `en`.
- Fallback: requested locale → default variant for the language → `en` → visible key. Never render silent blanks.
- Format time, numbers, tokens, cost, shortcuts, and paths semantically. Do not translate code, commands, or identifiers.
- The same Core fact uses the same key and parameter set in TUI and GUI.

### 8.3 Configuration rollout

- I0 defines `ui.locale` schema, parsing, and persistence without promising a full settings surface.
- I1 provides the EN/中 quick switch shown in the designs and persists the preference.
- I2 exposes stable Settings controls while retaining a CLI override.
- A new locale is registered only after key completeness, layout/CJK, snapshot, and fallback tests pass.

## 9. Skin, Color, and Density System

### 9.1 Configuration model

Shared UI preferences include:

- `skin`: `aurora | ice | mono | amber | phosphor`
- `mode`: `dark | light | system`
- `density`: `compact | regular | comfy`
- `motion`: `system | reduced | full`
- TUI-only `color_depth`: `auto | truecolor | ansi256 | ansi16`

Valid skin combinations are Aurora, Ice, and Mono in dark/light, plus dark-only Amber and Phosphor: eight combinations total. Invalid combinations display a reason and use a safe fallback instead of producing a partial token set.

### 9.2 Single source and adapters

- Keep `tokens.css` as the numerical design source.
- Generate TUI palettes and GUI framework adapters from registered tokens at build time; generated artifacts are validated instead of manually copied.
- `tui-kit.css`, `gui-kit.css`, and component registration define semantic components. Color communicates state and hierarchy only.
- GUI forbids emoji. TUI uses one glyph registry and supports truecolor → 256 → 16 degradation.
- Reduced motion, contrast, visible focus, and non-color-only state communication are hard gates for every skin.

### 9.3 Configuration rollout

- I0 freezes `ui.skin/ui.mode/ui.density/ui.motion` schema and the token registry.
- I1 keeps the skin, mode, and density quick switches from the prototypes and persists them.
- I2 exposes full Settings controls, system mode, and reset-to-default.
- Future plugins may contribute skins only through registered descriptors; they cannot inject arbitrary CSS or override core state colors.

## 10. Preference Precedence and Data Flow

Preference precedence is:

```text
CLI override
  → user preferences
  → project-safe defaults
  → client capability defaults
```

Project configuration cannot force personal locale, skin, mode, density, or reduced-motion preferences. Core may return recommendations and capabilities, but final UI preferences are local user state. Changing a UI preference is not a business mutation. If audited, record only the changed configuration key, not color values or translated text.

## 11. Compatibility Matrix and Integration Flow

Every TUI and GUI release manifest records at least:

```text
component_version
min_core_version
supported_schema_versions
base_core_checkpoint
required_capabilities
design_source_revision
locale_catalog_revision
token_registry_revision
```

Development order:

1. Core branches from synchronized main and publishes an immutable checkpoint.
2. TUI and GUI branch into independent worktrees from the same checkpoint and develop concurrently.
3. Later Core changes remain backward compatible; breaking changes enter the next schema.
4. Integration validation is serialized Core → TUI → GUI.
5. Shared fixtures, migrations, and design baselines must pass before signing an integration baseline.

Write ownership remains Core `crates/**`, TUI `apps/tui/**`, and GUI `apps/gui/**`. A missing contract becomes a Core contract request; clients do not invent private facts.

## 12. Errors and Recovery

- Incompatible schema: block connection before startup and display client version, Core version, schema, and upgrade guidance.
- Missing capability: disable the action and show the missing capability instead of rendering a clickable control that must fail.
- Sequence gap: request snapshot/replay and never declare success before recovery.
- Missing locale key: follow the fallback chain and record the key in tests/diagnostics.
- Invalid skin combination or incomplete token set: fall back to Aurora dark/regular and show one non-blocking diagnostic.
- Corrupt preferences: preserve the original file, use safe defaults, and show a locatable error. Never silently overwrite user configuration.

## 13. Acceptance and Testing

### Core

- Schema, capability, cursor/replay/gap, unknown event, migration, and multi-lane non-blocking tests.
- Plan mode denies every mutation before execution.
- JSONL replay rebuilds the same business facts.
- Core has no UI dependency.

### TUI

- Shared fixture replay, input modes, CJK, paste, scrollback, resize, approval, and multi-lane tests.
- Screenshots at 80/112/160 columns and truecolor/256/16.
- Critical snapshots across two locales, eight valid skins, three densities, and reduced motion.
- `apps/tui/**` contains no authoritative runtime/Git/process/lane effects.

### GUI

- Framework gate metrics, shared fixture replay, D1 path, D11/D4/permission/recovery, crash/reconnect.
- English/Chinese, CJK IME, keyboard, screen reader, visible focus, and three-platform checks.
- Component gallery and accepted HTML main-screen screenshot parity.
- Eight valid skin combinations, three densities, system/reduced motion, and invalid-combination fallback tests.

### Integration

- The same fixture yields the same business facts in Core, TUI, and GUI.
- The same project, lane, and session restore in TUI and GUI.
- One real local development task completes the full P0 loop.
- `cargo test --workspace --quiet` passes.

## 14. Next Round, Not This Round

- Full Decision Center, deeper D10 lane monitor, D12 conflict bounce, and D14 audit P1 experience.
- Trusted-delivery beta, production three-platform packaging, and release gate.
- D13 Fleet, D7/D8 team capabilities, and D9 remote targets.
- Plugin/domain UI contributions, custom locale packs, and third-party skin descriptors.

These capabilities receive separate versions on top of this round's stable contract and cannot expand the P0 scope retroactively.
