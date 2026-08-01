# Core 0.3 Compatibility

Chinese version: [core-0.3-compatibility.zh-CN.md](core-0.3-compatibility.zh-CN.md)

This document is the human-readable compatibility manifest for the frozen Core
0.3.0 `frontend-contract-v1` payload and its backward-compatible Core 0.3.3
extension candidate. It records the frontend schema, handshake capabilities,
migration order, deterministic fixture corpus, UI preference contract, and
design-entry hierarchy.

## Freeze Status

```text
component = viden-core
component_version = 0.3.0
supported_schema_versions = [1]
active_schema_version = 1
contract_payload_sha: 5bd2b80b0953f4194d082940a7b9164c7231ca2d
```

The recorded 40-character SHA identifies the reviewed contract payload commit.
This document is committed separately as evidence, and that evidence commit is
the exact common branch base for TUI and GUI; its parent must equal the recorded
payload SHA. This document does not authorize a tag, push, publish, or Homebrew
change.

## Frozen Capability Set

The frozen capability constant exposed to Core as `CORE_CLIENT_CAPABILITIES` is
the single source consumed by the handshake. Core 0.3.0 advertises this exact
unique, lexically sorted set:

```text
runtime.agent_dag
runtime.approvals
runtime.commands
runtime.context
runtime.cost
runtime.events
runtime.evidence
runtime.merge_gate
runtime.queued_input
runtime.replay
runtime.snapshot
runtime.transcript_page
runtime.typed_lanes
runtime.typed_tasks
ui.preferences
```

Every frozen base-fixture requirement must be present in this set. The separately
registered extension fixture may require only advertised extension capabilities.
A fixture requiring any unknown mandatory capability fails compatibility
validation; malformed or ambiguous legacy input is rejected rather than guessed.

## Schema-1 Post-Freeze Extension Candidate

The Core 0.3.0 frozen capability set and its original nine fixture bytes remain
unchanged. The Core 0.3.3 candidate advertises this exact lexically sorted
additive set through `FRONTEND_V1_EXTENSION_CAPABILITIES` and
`crates/core/frontend-contract-extensions.toml`:

```text
core.workspace_host
runtime.agent_adapters
runtime.agent_permission_bridge
runtime.agent_sessions
runtime.credential_handles
runtime.credential_staging
runtime.lane_lifecycle
runtime.lane_owner_projection
runtime.project_onboarding
runtime.recent_work
runtime.starter_lane_preview
runtime.trust_loop
ui.preference_persistence
```

The schema remains `1`. A client may connect with only the frozen base set;
missing extension capabilities disable only the corresponding feature and must
not block unrelated startup. A disabled feature stays visibly unavailable and
sends no command. In particular, the TUI gates stable Settings on
`ui.preference_persistence`; GUI gates D11 recent work on
`core.workspace_host` plus `runtime.recent_work`; TUI and GUI gate reviewed D4
creation on `runtime.starter_lane_preview`; and exact active-Lane cancellation
requires `runtime.lane_owner_projection` plus one authoritative binding.

Agent selection requires `runtime.agent_adapters`; starting and cancelling a
typed external session requires `runtime.agent_sessions`; and ACP permission
requests are interactive only when `runtime.agent_permission_bridge` is
negotiated. Adapter views contain safe availability/auth facts, never raw
commands, environment references, or agent-native credentials. Foreground and
asynchronous ACP sessions share the Core-owned approval queue. On restart, Core
terminates an interrupted external process before publishing a recoverable
failed session, so replay never invents a live process.

Clients written against Core 0.3.0 continue to require only the frozen set and
must preserve unsupported schema-1 events as `RuntimeWireEvent::Unknown`. A
client enables the 13 lane lifecycle commands and the `LaneUpdated`,
`LaneCommandAccepted`, `LaneOutputAppended`, `LaneConflictDetected`, and
`LaneRecoveryRequired` event projections only after negotiating
`runtime.lane_lifecycle`. Lane command receipts use the extension-specific
top-level event so a 0.3.0 client preserves the whole payload as unknown rather
than failing on a nested command variant. Empty extension
projection vectors are omitted during serialization, so replaying the frozen
0.3.0 corpus retains its recorded canonical bytes and digests.

`runtime.trust_loop` adds typed handoff, review request, contract, dependency,
merge-gate policy/validator/decision, conflict-bounce, and revert facts. The
seven new cross-lane commands, including explicit `RevalidateMergeConflict`,
and their events are permission-gated and replay through
the shared reducer. Schema remains `1`: new record fields use defaults, unknown
fields remain ignorable, and a pre-extension string merge decision deserializes
as a read-only `legacy` decision. New writes always serialize a typed decision.
Only real ContextStore bytes with a Core-issued permission receipt can produce
canonical acceptance; display summaries never substitute for evidence. Assigned
validators bind the exact id/hash set, while `RequestReview` itself is
authorized by the requesting gate owner. Dependency ids are stable edge ids and
cannot be rebound to different endpoints. Pure trust preflight completes before
approval. Merge persists a private content-addressed recovery snapshot and
workflow precommit before changing files; duplicate preimage blobs are reused,
and the private recovery lock refuses symlink traversal. Audited revert remains
available after restart without placing raw preimages in event logs.

Core owns lane permission evaluation and refreshes it from the current runtime
mode before every lane command. Side-effecting commands are evaluated against
one canonical worktree or repository target shared by permission checks and the
effect executor. Existing symlinks may resolve only inside the repository;
missing targets resolve through their nearest real parent, reject symlink
parents and `..`, and are revalidated immediately before local effects.
Approval previews redact command, argument, environment, input, and diff
payloads. Interrupted starting, running, or approval-waiting lanes hydrate as
blocked recovery facts and remain bound to their durable session owner.

Project onboarding probes the current directory without mutation.
`PreviewProjectConfig` validates repository-root `viden.toml` policy and
returns the exact reviewable UTF-8 contents plus its SHA-256 without writing.
The D11 parser accepts only the documented `project`, `gates`, `runner`,
`budget`, and `targets` schema, rejects unknown nested fields, and withholds
exact contents from candidates containing secret fields or credential-shaped
values.
`ConfirmProjectConfig` accepts only the cached preview id and hash, rechecks
the destination base hash, and writes those exact bytes after a Build-mode
permission approval. Credential commands carry only provider, backend, and
one-use ingress identifiers. Those identifiers use a bounded ASCII opaque-id
grammar and reject secret-like markers and path syntax. Secret bytes remain in the injected backend,
while replay and audit contain only `CredentialHandle` metadata.

Ordinary tool and lane approval responses observe supervisor command ordering
with permission and mode changes, but use two deliberately different generation
semantics. Ordinary tools consult submitted permission-control reservations, so
a queued permission or work-mode command invalidates a blocked approval
immediately and permanently, before the worker applies that command. Its
submitted generation is never decremented or reused, even when the control's
SessionMeta batch later fails to persist. The stale ordinary request resolves as
`Deny` and cannot be restored; the user must retrigger the tool to obtain a new
request. A failed reservation is still removed from the projected applied-state
queue so it cannot leak policy into later controls. Lane requests instead
capture the worker's applied generation atomically with the permission engine it
describes; that generation advances only after the queued control command is
successfully applied, so a lane approval may survive a failed control.
Permission and work-mode controls persist their complete session-metadata batch
before publishing the new live snapshot or permission engine; a failed batch
leaves the engine, snapshot, lane pair, and applied generation unchanged. Any
intervening applied permission or work-mode generation change invalidates the
pending lane approval even if the visible flags later return to their original
values. Once a lane response is accepted, the
supervisor waits
for its terminal `ApprovalResolved` and effect/persistence completion before it
processes or publishes a later permission snapshot. Lane approval-derived
session/repository allow rules are kept
inside the owning lane worker, so they survive normal authoritative permission
refreshes for that lane without authorizing another lane or owner; Plan/ReadOnly
refreshes discard them immediately.
Create and lane status transitions follow the same permission and mutation-policy
gate as other durable effects. Terminal workers unregister and join through the
completion reaper without waiting for another lane command.

## Client Boundary

Frontend clients use only `CoreClient` and protocol/view contracts re-exported
by `viden-core`. The transport interface is limited to discovery, command send,
event receive, snapshot, replay, and transcript paging. Frontends do not import
or call runtime, provider, tool, permission, session, or workflow internals.

`StatefulCoreClient` validates the handshake and schema before committing
state. It ignores duplicate/older cursors, applies only the contiguous next
event, stages gap replay until it is complete and valid, and requests a
validated snapshot for stream mismatch or snapshot-required recovery. A
frontend never synthesizes successful effect state.

`viden_core::legacy` is deprecated. It exists temporarily for the pre-v3 TUI
bootstrap and must not be used by new TUI, GUI, CLI, API, or plugin clients.

## Schema-1 Fixture Corpus

Fixture files live under
`crates/types/tests/fixtures/frontend-contract-v1/`. The digest cells below are
the tested fixture values and must change with the corresponding fixture state.

| Fixture id | Frozen scenario | Expected final view SHA-256 |
| --- | --- | --- |
| `stream-tool` | Assistant stream, tool start, successful tool finish | `8478c7c0ce6f0adc3efdd3aa11497462e96b3aba50cf66e81b0ad9ddcd992eef` |
| `approval-allow-deny` | Structured scoped allow and deny without frontend-owned effects | `7788f2f4b34ce54893ab8ed41beb6e37958ff5fda95642d045ef2d1dedbf7b39` |
| `queued-follow-up` | Queue and dequeue while active work stays visible | `eb1bc1a00185d5642f9a95a2cffde7a81f2bd4ac4417385c5c1b6e2aefa8354a` |
| `dag-blocker` | Typed DAG/task dependency blocker and recovery action | `a496d331e42f730d41565afe58a3308bf38a7b7e3b92e0279d198e9c407e7719` |
| `multi-lane` | Multiple typed lanes with distinct role, route, gate, owner, target, budget, and session facts | `e491d3bc547601b3c54eae05dc1b1259c9cc8ccac948908be8519d432b62fe38` |
| `merge-gate` | Typed evidence and Core-owned MergeGate reduction | `41f4d842a12356586a461b173d072d7e7efedb4d7707471c99eb77dd37533321` |
| `context-pressure-cost-blind` | Context pressure/omission and explicit unknown or unmetered terminal cost | `2e39ec2e32fac56ae6279e8f681bcf4357701de51a6772bad14caee0ddb4ba5e` |
| `plan-denial` | Plan-mode mutation rejection without a successful mutation fact | `fa1fa859af8f056686c06b30b789706539d9ed19e02756519757993d5ee31b2d` |
| `d1-vertical-slice` | D1-visible transcript/tool, lane/task, decision, evidence/gate, context/cost, recovery, and UI preferences | `7dd8faf04cca9f3013198e25823894eae91c2869e27087aa1eb0a34890cdf804` |

The original nine rows above are the frozen base corpus. The separately
registered schema-1 extension fixture is:

| Fixture id | Extension scenario | Expected final view SHA-256 | Canonical fixture bytes SHA-256 |
| --- | --- | --- | --- |
| `frontend-host-services` | UI preference persistence, safe recent work, reviewed starter-Lane preview/create/invalidation, exact live Lane owner, and one tolerated future optional event | `b118534bb0a568a6a1e781171cecf0512c7d987736c06e4f84d51b5835022a0e` | `96dd5fde9f1241eb50f9d8978cf478d0ac5d3327448dc6ccde9d0e5018ce1580` |
| `interaction-closed-loop` | Folder binding without implicit setup, reviewed Lane creation, built-in and ACP adapters/sessions, shared approval, evidence/gate, apply conflict, typed recovery, reconnect replay, and completion | `31b71bf154d42c8c7923fe9c64763a5245f785a2cd953913124f30a981589b51` | `596e82efa03d21b1f9645f40cf500ca8c4c1b86b2aa78be85a6bea0184822bff` |

The extension fixture uses the six real known events
`UiPreferencesUpdated`, `RecentWorkLoaded`, `StarterLanePreviewed`,
`StarterLaneCreated`, `StarterLanePreviewInvalidated`, and
`LaneRuntimeOwnerBound`; it does not substitute a transient error or display
placeholder. The normal in-memory journal, snapshot, and replay path must
reduce these facts to the same `RuntimeViewState`. The optional future event
advances the cursor without mutating that state.

The interaction fixture has 18 ordered events and locale-neutral fact keys.
Its reconnect test deliberately observes a cursor gap, replays the missing
contiguous batch, and proves that the normalized final `RuntimeViewState`,
cursor, and digest equal uninterrupted replay. The manifest at
`crates/core/release-manifest.toml` records both fixture payloads and the Core
0.3.3 contract implementation checkpoint; it does not authorize a tag.

Each JSON fixture envelope contains:

- `fixture_id`;
- `schema_version: 1`;
- unique, lexically sorted `required_capabilities`;
- an `initial_snapshot`;
- non-empty `RuntimeEventEnvelope` values in contiguous cursor order;
- `expected_final_cursor`;
- `expected_view_sha256`.

For every known event, the event sequence equals the cursor sequence. Replaying
the parsed fixture twice from the same initial snapshot must produce
byte-identical canonical state plus the same cursor and digest. Fixture values
are deterministic and contain no machine-specific absolute path or secret.

## Canonical Digest

The final-view digest is SHA-256 over compact JSON for `RuntimeViewState` after
recursively sorting every object key. Array order is preserved because it is
semantic. The test compares the generated lowercase 64-character hex digest
with the fixture and this manifest.

## Migration Gate And Order

Migration runs before schema-1 fixture replay and must be idempotent:

1. Parse `legacy-lanes.tsv` through the supported v0 lane input boundary into
   typed `AgentLaneRecord` values.
2. Compare those values with `typed-lanes.json`, serialize the normalized typed
   values, parse them again, and require equality.
3. Parse the supported legacy flat cost shape into structured
   `CostUsageRecord`; preserve an unknown actual cost as `None`, serialize the
   normalized record, parse it again, and require equality.
4. Parse the supported legacy approval boolean into structured
   `ApprovalResponse`, serialize the normalized response without the legacy
   boolean, parse it again, and require equality.
5. Only after all migrations pass, replay every schema-1 fixture twice and
   validate identity, capabilities, cursor continuity, final state, and digest.

Unknown lane roles/routes/statuses, ambiguous cost shapes, malformed approval
records, and unknown mandatory fixture capabilities fail the gate. Migration
does not silently coerce them.

## UI Preference Compatibility

The schema-1 preference surface is frontend-neutral:

The effective frontend fact is
`RuntimeSnapshot.ui_preferences: ResolvedUiPreferences`. A client renders this
resolved value and does not re-run precedence or fallback policy locally.

| Dimension | Supported values |
| --- | --- |
| Built-in locale | `en`, `zh-CN` (`system` resolves to one of them) |
| Skin | `aurora`, `ice`, `mono`, `amber`, `phosphor` |
| Effective mode | `dark`, `light` |
| Density | `compact`, `regular`, `comfy` |
| Motion | `system`, `reduced`, `full` |

The eight valid effective skin/mode pairs are:

```text
aurora/dark
aurora/light
ice/dark
ice/light
mono/dark
mono/light
amber/dark
phosphor/dark
```

`amber` and `phosphor` are dark-only. Preference precedence is CLI, user,
project, then client default. An invalid effective pair uses the safe
`aurora/dark` and regular-density fallback and records a
`ui.invalid_skin_mode_pair` diagnostic; locale and motion remain resolved.

## Design Entry Hierarchy

Visual verification follows one path:

1. global index: `docs/viden-design/Viden/index.html`;
2. client index: `TUI/Viden - 设计稿索引 (TUI).html` or
   `GUI/Viden - 设计稿索引 (GUI).html`;
3. component library: `TUI/Viden - 组件库 (TUI).html` or
   `GUI/Viden - 组件库 (GUI).html`;
4. canonical product entry: `TUI/Viden - 统一原型 (TUI).html` or
   `GUI/Viden - 桌面驾驶舱 (GUI).html` (D1).

GUI `pages/Viden - D11 首启与项目接入 (GUI).html` is subordinate onboarding. It
is not the cockpit and cannot replace D1 as the GUI baseline. All relative
paths above start at `docs/viden-design/Viden/`. Old screenshots and generated
previews are historical evidence only; they do not override this hierarchy.

## Historical Compatibility

The v0 lane TSV input, legacy flat cost shape, legacy approval boolean, and
`viden_core::legacy` bridge are migration surfaces, not new client APIs. Keep
historical release evidence unchanged while clients move to schema `1` and the
CoreClient-only boundary.
