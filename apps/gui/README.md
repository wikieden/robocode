# Viden GUI

Chinese version: [README.zh-CN.md](README.zh-CN.md)

This directory is the GUI implementation track for Viden. The alpha evidence
gate selected Tauri, and `0.1.0-rc.3` certifies the canonical D1 cockpit
against the Core `0.3.5` same-state fixture. `0.1.0-beta.1` established the
single production
desktop bootstrap. The native launcher opens a frontend-safe `LocalCoreHost`
workspace and injects its `CoreClient` through `GuiCoreAdapter`. Once connected,
the app always presents the D1 cockpit shell. With no bound workspace, `Open
project` opens the native folder picker and rebinds through
`LocalCoreHost::open_workspace`; it never enters D11 or asks for a model. A
bound zero-Lane project renders the project cockpit with `New Lane`, which
opens the D1 New Lane popover for quick native or ACP lane startup. The exact
Core Lane receipt returns focus to D1.

## Run the desktop client locally

From `apps/gui`, install the pinned frontend dependencies and start the native
Tauri development window:

```bash
npm ci
npm run tauri -- dev
```

Vite and Tauri share the strict development endpoint
`http://localhost:1420`; the command fails instead of silently moving to a
different port. The native bootstrap binds a workspace only when
`VIDEN_GUI_WORKSPACE` is explicitly set. Otherwise the D1 welcome stays
available for native folder
selection; a real Core bootstrap failure renders explicit D6 disconnected state
rather than simulating success.

On macOS the native traffic-light controls stay available in an overlay title
bar, while the HTML shell supplies the dark draggable surface. The prototype
window border, rounded frame, and white native title strip are not rendered.

To build and open a self-contained macOS debug app that does not depend on the
Vite development server:

```bash
npm run tauri -- build --debug --bundles app
open ../../target/debug/bundle/macos/Viden.app
```

To bind an explicit project when launching the binary directly:

```bash
VIDEN_GUI_WORKSPACE=/absolute/project/path \
  ../../target/debug/bundle/macos/Viden.app/Contents/MacOS/viden-gui
```

Before Core bootstrap, the desktop host prepends existing standard user tool
directories such as `~/.local/bin`, Bun, Volta, asdf, mise/fnm, and Homebrew to
the inherited `PATH`. This keeps agent discovery and the later ACP spawn on the
same command path when the App is opened through Finder, without executing a
login shell or embedding a machine-specific absolute path.

## Frozen input

| Field | Value |
| --- | --- |
| GUI component version | `0.1.0-rc.3` |
| Minimum Core version | `0.3.5` |
| Supported frontend schemas | `[1]` |
| Common branch base | `3a7740ea72e58f4a22248a80f9e7324c49bb0f73` |
| Core final checkpoint | `f7fe1b31dfb237e4062209767a7051c2b2c68b93` |
| Core code checkpoint | `17fa2071398d5eaf30045257163d57d22d99177b` |
| Contract payload | `5bd2b80b0953f4194d082940a7b9164c7231ca2d` |
| Canonical D1 fixture | `d1-main-cockpit.json`, SHA-256 `f96ba30cc6e80aa52cb15a2fd1f03c082487a3cd4779c25f61e42ee1548e1e3b` |
| Required Core capabilities | 15 frozen values plus additive extension capabilities, including `runtime.cockpit_context_v1` |
| Built-in locales | `en`, `zh-CN` |
| Appearance | 5 skins, 8 valid skin/mode pairs, 3 densities, 3 motion policies |

The active machine-readable manifest is
[release-manifest.toml](release-manifest.toml). Its immutable rc.3 snapshot is
[manifests/0.1.0-rc.3.toml](manifests/0.1.0-rc.3.toml); both files must
remain byte-equivalent for this release checkpoint. Earlier alpha, beta, and
rc.2 snapshots remain historical evidence and are not rewritten.

## Design source order

Visual and interaction inventory starts from the accepted design hierarchy:

1. `docs/viden-design/Viden/index.html`
2. `docs/viden-design/Viden/GUI/Viden - 设计稿索引 (GUI).html`
3. `docs/viden-design/Viden/GUI/Viden - 组件库 (GUI).html`
4. `docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html` (D1)

D11 project configuration, D4 Lane creation, and D6 operational recovery are
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

## Inventory against Core `0.3.5`

| GUI area | Design intent | Core `0.3.5` status | GUI handling |
| --- | --- | --- | --- |
| Project open / D11 intake | native folder open plus project probe, provider health, config preview/confirm, and credential handles | `LocalCoreHost::open_workspace` provides trusted folder rebinding; secure credential ingress and the GUI recent-work adapter remain incomplete | Welcome uses the native folder picker and host rebind directly; D11 stays an explicit in-project configuration flow and never owns folder open |
| D4 lane creation | typed role, route, gate strength, mutation policy, target, budget, worktree preview, lane receipt | `PreviewStarterLane`/`CreateStarterLane`, Core-resolved preview, invalidation, approval, exact receipt, and `runtime.starter_lane_preview` advertisement are available | Task 8 renders the four-step reviewed flow; older Core handshakes still fail closed visibly with zero sends |
| D1 cockpit | no-project welcome center, zero-Lane project cockpit, activity/lane rails, streaming transcript/tool rows, Environment, Live Work, composer, evidence/context/cost facts | Stream/tool/approval/queue/task/lane/owner/evidence/context/cost/preferences facts exist; diff/apply, stable audit timeline, actionable lane recovery, and GUI recent-work projection remain incomplete | No bound host renders Welcome; a bound empty project remains D1 and exposes `New Lane`; live work renders from `RuntimeViewState` |
| Permission dock | scoped approve/deny, risk, target, expiry, default action, audit id | `ApprovalRequestView` and `RespondToApproval` exist | Usable through Core; GUI cannot execute tools directly |
| D6 recovery | connecting, disconnected, agent stopped, budget exhausted, gate queue clear, reconnect/restart/close actions | Runtime errors, CoreClient snapshot recovery, context budget facts, queue/gate facts exist; structured lane lifecycle recovery commands are missing | Task 10 renders operational Core-owned recovery states; the no-project `empty` state is handled by D1 Welcome Center, while restart/close/checkpoint remain visibly unavailable under `GUI-CORE-003` |
| Locale and skin system | `en`/`zh-CN`, Aurora/Ice/Mono/Amber/Phosphor, dark/light constraints, density, motion | `RuntimeSnapshot.ui_preferences`, `SetUiPreferences`, `ResetUiPreferences`, and `UiPreferencesUpdated` exist with persistence and safe fallback diagnostics | GUI renders resolved Core preferences; production Settings controls remain frontend implementation work and must wait for the ordered Core event |

Open requests are recorded in [contract-requests.md](contract-requests.md) and
[contract-requests.zh-CN.md](contract-requests.zh-CN.md). GUI must not close
those gaps with private reducers or direct runtime access.

The remaining open requests block only the production screens named in their
rows. They do not block the framework-neutral, fixture-only Tasks 2-3 or their
evidence; no spike result authorizes production mutation or persistence.

## D11 project intake

Task 7 implements the D11 subordinate project-configuration flow after the fixed Core
`0.3.2` integration checkpoint. `GuiCoreAdapter` gates project onboarding,
credential-handle intents on their advertised extension capabilities. Probe,
preview, and confirm remain distinct Core commands; D11 starter selection is a
local ordered review queue and never sends legacy `CreateLane`. A read-only
`d11_poll` command keeps receiving late facts after
the initial bounded wait, while the adapter serializes pending intake commands
and matches preview hashes, confirmation ids/hashes, Lane ids, and the active
approval request id before clearing pending state. Project-config approvals are
bound to the exact Core metadata token `sha256=<64 lowercase hex>` before the
GUI accepts their request id; if a bounded `preview_id=` token is present, it
must also equal the pending preview id. Hash substrings, non-lowercase hashes,
free text, and non-`sha256` fields cannot retarget or clear the pending command.
Allow decisions keep waiting
for the matching business fact; deny/expiry decisions clear pending and keep
draining the following Core error projection. Transient poll failures keep the
local draft and pending identity, then retry with bounded backoff. Intermediate
Core projection changes remain visible during that wait. Cancel clears only the
in-memory navigation state, performs no Core mutation, and returns to D1.
Welcome never enters this flow: folder selection and host rebinding complete first.

The standalone host drains ordered command events before refreshing its
authoritative snapshot, so acceptance cannot hide the later probe, preview, or
confirmation fact. A new-project draft includes the required `name` and `pack`
fields. When confirmation requires Core approval, D11 embeds the same typed
Permission Dock used by D1; `Allow once` or `Deny` remains an explicit Core
command, never a GUI-side bypass.

## D4 reviewed starter Lane creation

Task 8 opens D4 from the project cockpit's `New Lane` action and reviews one
seed at a time. Cancel or Skip before creation returns to D1 without a mutation;
after creation is sent, the screen exposes
the exact Core approval allow/deny actions and no fake cancel command. Each
complete `StarterLaneCreated.receipt` advances the queue, and the final receipt
emits a typed D1 navigation request focused on the last created Lane.

The adapter sends `PreviewStarterLane`, retains the exact original request,
and accepts only a same-owner `StarterLanePreviewed` fact. Branch, worktree,
base revision, route, gate, target, mutation policy, and budget are read-only
Core facts. Build mode may create only the unchanged reviewed request; Plan
mode may preview but cannot create. `CommandAccepted` and `LaneUpdated` remain
intermediate facts and never navigate. Request changes, rejection, approval
denial, and typed preview invalidation preserve the webview draft and require
a new preview. Only a full owner/id/hash/Lane/branch/worktree/base/config match
on `StarterLaneCreated` authorizes queue advancement.

Core `0.3.5` advertises the exact additive capabilities
`runtime.starter_lane_preview` and `runtime.cockpit_context_v1`, so the
production D4 path and D1 Context Dock can use the reviewed typed flow.
Connections to older or partial Core handshakes still show the gate and send
nothing; `runtime.lane_lifecycle` is deliberately not accepted as a substitute.

Deterministic browser evidence for rc.3 is retained under
`evidence/0.1.0-rc.3/`. The complete eight-pair theme matrix, all three
density values, both catalogs, and reduced-motion behavior remain automated
tests.

The config rail renders only Core's exact reviewed `viden.toml` contents, and
confirmation copies the preview id and SHA from the current Core projection.
Credential rows contain masked handles only. Because no frontend-safe platform
credential staging channel exists, raw credential entry and the webview
`StoreCredentialHandle` path are disabled with `GUI-CORE-001`. Cross-project
recent work remains typed unavailable as `GUI-CORE-007`; the GUI does not scan
local storage, JSONL, or SQLite. Project switching now uses the Core-owned
`LocalCoreHost::open_workspace` boundary; secure raw credential staging remains
the outstanding `GUI-CORE-001` part.

## D1 streaming cockpit

Task 9 makes D1 the persistent application shell and canonical work surface.
It renders first with D6 connecting/disconnected state or the host-owned
no-project welcome. A bound project with no Lanes remains D1, while D4 is the
project-only creation workflow and D11 is explicit configuration. The activity
rail, Lane rail, Environment, Live Work,
transcript/tool rows, queue state, evidence, and composer are transport-safe
projections of Core's latest `RuntimeViewState`.
The webview owns only focus, draft, layout, bounded-row, and scroll-anchor
state. It does not parse display strings, persist a second workspace model, or
claim command acceptance as business success.

`New Lane` opens one anchored popover with the built-in Viden Agent, discovered
ACP Agents, the task draft, Core-projected eligibility/probe diagnostics, and a
presentation-only isolation hint. Git workspaces preview the derived
branch/worktree; non-Git directories explicitly state that the Lane runs in the
opened workspace without creating either. Agent
selection stays inside the popover, the task textarea receives focus, and
Create Lane is disabled until the task is non-empty. Create dispatches the
existing ordered path: `preview_default_lane`, `create_starter_lane`, then
native `submit` or ACP `start_agent_session` only after the exact Core Lane is
projected. Transport or Core rejection preserves the draft and uses the typed
D1 rejection surface.

The composer remains editable while an assistant stream, tool, task, approval,
or queued input is active. Enter sends `QueueFollowUp` in that state and
`SubmitUserInput` when idle; Shift+Enter preserves multiline input and CJK IME
composition never submits early. Both commands use an exact Core-published
Lane owner from a live owner binding or the D4 receipt. Cancel is stricter: it
is visible and transport-enabled only when the selected Lane is active,
`runtime.lane_owner_projection` is advertised, and exactly one matching
`lane_runtime_owners` binding supplies the complete owner. Missing, mismatched,
or ambiguous owners fail closed with zero sends.
If another ordered Core command or Agent probe still owns the client command
slot, one composer submission waits visibly as `Queue follow-up`, disables a
duplicate Send, and dispatches as soon as that slot is released; input is never
silently dropped.
After a desktop restart, the sole terminal ACP session restored by Core may
accept a follow-up through its exact durable session owner even though the
process-local Lane binding has expired. Before the continuation starts, Core
publishes a fresh `LaneRuntimeOwnerBound` fact for that same durable owner, so
the Lane remains visible as busy while the ACP response is in flight instead
of temporarily falling into Agent Stopped. Duplicate sessions, owner mismatch,
and non-ACP restoration still fail closed.
Within one app/Core lifetime, completed ACP turns keep their healthy process and
remote session resident. The next Send therefore goes straight to
`session/prompt`; a dead or incompatible resident connection falls back to the
persisted `session/load` path before prompt delivery. The GUI does not own this
cache and continues to render only ordered Core facts. Immediate Starting/busy
feedback measures Core dispatch, while time to first assistant content also
includes agent startup (for cold turns), context work, and model inference.
Core caps the resident pool at eight sessions and expires connections after 15
idle minutes; a later Send transparently uses the persisted reload path. Core
shutdown also retires the workspace's entire resident pool.
Cancelling an active built-in model turn now stops that turn before considering
Lane lifecycle cancellation, so the Lane and its exact owner remain routable.
A legacy terminal native Lane without an owner renders a disabled composer and
Send action instead of a clickable no-op.

The transcript retains at most 240 rows. Leaving the latest edge sets
`follow_latest=false`, preserves the current anchor, and increments a visible
new-output count instead of forcing a scroll. Rust and webview tests cover
10,000-event bursts, 50,000 rows, resize/idle reads, CJK composition,
multiline paste/undo, keyboard traversal, ARIA regions, and visible focus.
Browser-controlled rc.3 evidence under `evidence/0.1.0-rc.3/` includes
Aurora dark/regular English, Ice light/regular English, Aurora dark/regular
Chinese, compact density, responsive drawer states, and an independent
same-state design reference populated from `d1-main-cockpit.json`. It also
includes a supplemental Context Dock bottom-state capture that proves lower
facts are reachable by internal scrolling. Diff, apply, audit, and untyped
recovery actions remain explicit unavailable facts; D1 never fabricates a
successful placeholder.

## Permission dock and D6 recovery

Task 10 places the canonical `.gperm.dock` immediately above the D1 composer.
It renders the exact Core approval risk, target, allowed scopes, reason, input
preview, expiry, default action, and audit id. Once, Session, repository
allowlist, and Deny map only to `RespondToApproval`; Always and Edit remain
disabled as `GUI-CORE-003`. Plan-mode mutation responses fail closed before
transport. Command acceptance is not success: pending clears only after the
matching ordered `ApprovalResolved` owner/request/audit fact.

D6 is a subordinate central work surface inside D1, never a second cockpit
shell. Empty, connection, provider, stopped-agent, context-overflow,
capability, incompatible-schema, queue-clear, and event-gap states come only
from Core projection or CoreClient errors. Event-gap reconnect uses the
CoreClient snapshot path and remains busy until a validated live snapshot is
published. Restart, close-Lane, and checkpoint controls stay visible but
disabled under `GUI-CORE-003`; the GUI never fabricates recovery receipts.

## Production bootstrap

`src-tauri` is the only GUI member of the root Rust workspace and declares its
own `0.1.0-rc.3` version. `GuiCoreAdapter`, its D4 adapter extension, and
`RuntimeProjection` are the only production boundary modules that hold Core
contracts. `GuiPreferences`,
`WorkspaceSelection`, `ComposerDraft`, and `TranscriptViewport` are
presentation-only state. Closing the window drops the injected client without
sending a mutation.

## Locale and appearance

The production webview requests a transport-safe projection of
`RuntimeViewState.snapshot.ui_preferences`. That resolved Core state alone sets
the document language, skin, effective mode, density, and motion attributes.
The built-in `en` and `zh-CN` catalogs have checked key and placeholder parity;
shortcuts, paths, and code remain literal.

The eight accepted appearance pairs are Aurora, Ice, and Mono in dark or light,
plus dark-only Amber and Phosphor. Invalid or corrupt values use a deterministic
safe fallback and retain diagnostics. The Tauri CSS adapter imports
`docs/viden-design/Viden/tokens.css` directly. Run
`tools/check-generated-tokens.sh` to validate its SHA-256, semantic roles,
theme/density matrices, adapter import, and generated metadata. Production GUI
source contains no copied token values.

Preference controls may keep an unsaved in-memory draft. Save and restore must
use `SetUiPreferences` or `ResetUiPreferences`: the GUI does not use browser
storage, files, config, or a private preference authority, and rendered state
changes only after `UiPreferencesUpdated` supplies a new resolved projection.

The default native binary launches without a bound workspace unless
`VIDEN_GUI_WORKSPACE` is explicit. D1 Welcome opens a native folder chooser,
and `LocalCoreHost` constructs and owns the runtime behind the injected
frontend-safe `CoreClient`. A real host/bootstrap failure renders D6
disconnected; it does not enter D11. The D11 adapter still requires an injected
frontend-safe `CoreClient`; it must
not construct `SessionEngine`/`RuntimeSupervisor` or add a private reducer.
Task 6 now owns the resolved locale/appearance projection and unsaved draft
contract; Task 7 owns D11, and Task 9 owns D1.

## rc.3 visual, metadata, and bundle gate

Task 11 adds a framework-neutral component gallery and a deterministic pairwise
case inventory/DOM contract for D1, D11, D4, D6, and the gallery. It enumerates
both locales, every valid skin/mode pair, all densities, system/reduced motion,
and desktop, narrow, and scaled-font requirements. The reviewed visual evidence
is the representative desktop gate plus exact-size D1 same-state QA; gallery,
narrow, and scaled-font captures remain explicitly partial. D1 pass/fail visual
QA compares an independent canonical-state design reference against the
production canonical capture. The older accepted desktop cockpit screenshot is
kept only as historical visual lineage.

Machine-readable accessibility, bounded local performance records,
Browser-controlled same-state screenshots, side-by-side QA, exact methods, and
explicit native audit/profile skips are under
[evidence/0.1.0-rc.3](evidence/0.1.0-rc.3/README.md). The active manifest and
immutable rc.3 snapshot record the same evidence paths and remain
byte-equivalent. The macOS `.app` bundle is a local build artifact only; it is
not installed, signed, notarized, published, tagged, or released.
