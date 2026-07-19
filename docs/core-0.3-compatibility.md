# Core 0.3 Compatibility

Chinese version: [core-0.3-compatibility.zh-CN.md](core-0.3-compatibility.zh-CN.md)

This document is the human-readable compatibility manifest for the Core 0.3.0
`frontend-contract-v1` payload. It records the frontend schema, handshake
capabilities, migration order, deterministic fixture corpus, UI preference
contract, and design-entry hierarchy.

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

Every fixture requirement must be present in this set. A fixture requiring an
unknown mandatory capability fails compatibility validation; malformed or
ambiguous legacy input is rejected rather than guessed.

## Schema-1 Post-Freeze Extension Candidate

The Core 0.3.0 frozen capability set and fixture digests above remain unchanged.
The Core 0.3.1 candidate advertises the additive
`runtime.lane_lifecycle` capability separately through
`FRONTEND_V1_EXTENSION_CAPABILITIES` and
`crates/core/frontend-contract-extensions.toml`.

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

Core owns lane permission evaluation and refreshes it from the current runtime
mode before every lane command. Side-effecting commands are evaluated against
their actual worktree or repository target; approval previews redact command,
argument, environment, input, and diff payloads. Interrupted starting, running,
or approval-waiting lanes hydrate as blocked recovery facts and remain bound to
their durable session owner.

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

Each JSON envelope contains:

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
