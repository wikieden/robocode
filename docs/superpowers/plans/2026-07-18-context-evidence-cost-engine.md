# Context, Evidence, And Cost Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust-native context, evidence, and cost engine that gives every AgentTask role-scoped, reversible context while measuring cost and preserving canonical evidence.

**Architecture:** Add a neutral `crates/context` implementation crate behind stable records in `crates/types`. `crates/runtime` builds bundles, enforces budgets, emits events, and integrates provider usage; workflow and session logs remain canonical durable history. Headroom remains an optional plugin/MCP/benchmark adapter and is never required for native execution.

**Tech Stack:** Rust 2024, serde/serde_json, sha2, existing JSONL workflow/session stores, `RuntimeSupervisor`, existing provider telemetry, shell/Python release-smoke scripts.

**Design:** [Context, Evidence, And Cost Engine Design](../specs/2026-07-18-context-evidence-cost-engine-design.md)

---

## Delivery Sequence

Implement as four independently releasable slices:

1. `0.2.1-a`: contracts, canonical store, deterministic reducers, and retrieval.
2. `0.2.1-b`: runtime bundle integration, budget enforcement, event replay, and cost ledger.
3. `0.2.3`: canonical evidence verification in Merge Gate.
4. `0.2.4` / `0.2.5`: optional Headroom adapter boundary and DeepSeek A/B release gate.

Do not begin UI implementation in this plan. TUI/GUI integration is limited to
stable `RuntimeViewState` fields and client-facing documentation.

### Integration prerequisite

The current main branch still declares direct `viden-provider` and
`viden-runtime` dependencies in `apps/tui/Cargo.toml`. Core/context tasks may
proceed in parallel, but Task 10 and release acceptance are blocked until the
0.2.0 runtime-facade dependency cut removes those direct UI dependencies. Do
not weaken the dependency guard to accommodate the current state.

## File Map

| Path | Responsibility |
| --- | --- |
| `crates/types/src/context.rs` | Stable context, retrieval, quality, and cost records. |
| `crates/types/src/runtime.rs` | Runtime commands/events/view-state projection. |
| `crates/context/src/store.rs` | Content-addressed canonical raw storage. |
| `crates/context/src/reducer.rs` | Content routing, deterministic reducers, derivation records. |
| `crates/context/src/cost.rs` | Append-only usage entries and exact aggregation. |
| `crates/context/src/lib.rs` | Public context engine facade. |
| `crates/runtime/src/context_bundle.rs` | Role policy, bundle assembly, budget enforcement. |
| `crates/runtime/src/runtime_contract.rs` | Commands, events, retrieval, cost projection, Merge Gate integration. |
| `crates/provider/src/transport.rs` | Provider usage/cache facts only. |
| `crates/plugin-api/src/lib.rs` | Optional context-adapter capability descriptor. |
| `scripts/context-engine-benchmark.sh` | Deterministic and live A/B benchmark runner. |
| `scripts/deepseek-dev-scenario-smoke.sh` | Release evidence fields and engine on/off mode. |

### Task 1: Freeze Context And Cost Contracts

**Files:**
- Create: `crates/types/src/context.rs`
- Modify: `crates/types/src/lib.rs`
- Modify: `crates/types/src/runtime.rs`
- Modify: `crates/types/src/tests.rs`
- Modify: `crates/types/tests/fixtures/runtime-contract-phase2.json`

- [ ] **Step 1: Write failing serialization and invariant tests**

Add tests that construct a canonical item, derived view, handle, retrieval, and
cost entry, serialize them, and assert stable tagged values:

```rust
#[test]
fn context_contracts_round_trip_without_exposing_storage_paths() {
    let handle = ContextHandleRecord {
        handle_id: "ctxh-1".into(),
        item_id: "ctxi-1".into(),
        preferred_view_id: Some("ctxv-1".into()),
        content_sha256: "ab".repeat(32),
        scope: ContextScope::Task("task-1".into()),
        expires_at: None,
    };
    let json = serde_json::to_value(&handle).unwrap();
    assert_eq!(json["scope"]["type"], "task");
    assert!(json.get("storage_path").is_none());
    assert_eq!(serde_json::from_value::<ContextHandleRecord>(json).unwrap(), handle);
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p viden-types context_contracts_round_trip_without_exposing_storage_paths -- --exact`

Expected: compilation fails because `ContextHandleRecord` and `ContextScope`
do not exist.

- [ ] **Step 3: Add the stable contract module**

Define these records in `crates/types/src/context.rs` and re-export them from
`lib.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum ContextScope {
    Task(String),
    Dag(String),
    Workflow(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextContentKind { Json, Code, Diff, Log, Diagnostic, Transcript, Text }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextHandleRecord {
    pub handle_id: String,
    pub item_id: String,
    pub preferred_view_id: Option<String>,
    pub content_sha256: String,
    pub scope: ContextScope,
    pub expires_at: Option<u64>,
}
```

Also add `ContextItemRecord`, `ContextViewRecord`, `ContextRetrievalRecord`,
`ContextQualityRecord`, `ContextBudgetRecord`, and `CostUsageRecord`. Use
integer micro-units for money and integer token counts; represent unknown actual
cost as `None`, not zero.

- [ ] **Step 4: Add runtime commands and events**

Add `RetrieveContext { handle_id, reason }` to `RuntimeCommand`, and add typed
events for bundle built, item stored, view derived, retrieval, budget exceeded,
quality failure, cost usage, cache observation, and evidence canonicalization.
Extend `RuntimeViewState` with bounded summary collections and ledger totals.

- [ ] **Step 5: Run contract tests and update the fixture**

Run: `cargo test -p viden-types`

Expected: all type tests pass and the fixture contains the new event variants
without local storage paths or raw secret-bearing content.

- [ ] **Step 6: Commit the contract slice**

```bash
git add crates/types
git commit -m "feat: add context and cost runtime contracts"
```

### Task 2: Add The Canonical Context Store

**Files:**
- Create: `crates/context/Cargo.toml`
- Create: `crates/context/src/lib.rs`
- Create: `crates/context/src/store.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: Write failing store tests**

Cover deduplication, byte-identical retrieval, restart/reopen, hash mismatch,
and scope denial:

```rust
#[test]
fn repeated_content_reuses_one_canonical_blob() {
    let root = temp_dir("context-store-dedup");
    let mut store = ContextStore::open(&root).unwrap();
    let first = store.put(ContextPutRequest::task("task-1", ContextContentKind::Log, b"same")).unwrap();
    let second = store.put(ContextPutRequest::task("task-1", ContextContentKind::Log, b"same")).unwrap();
    assert_eq!(first.item.content_sha256, second.item.content_sha256);
    assert_eq!(store.blob_count().unwrap(), 1);
    assert_eq!(store.retrieve(&first.handle, &ContextScope::Task("task-1".into())).unwrap(), b"same");
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p viden-context repeated_content_reuses_one_canonical_blob -- --exact`

Expected: the package or `ContextStore` is missing.

- [ ] **Step 3: Implement content-addressed storage**

Use SHA-256 for blob names. Store blobs under `blobs/<first-two>/<sha256>` and
append metadata to `context-items.jsonl`. Write blobs through a temporary file
and atomic rename. Never put API keys, credentials, or local storage paths in
handles or runtime events.

Public facade:

```rust
pub struct ContextEngine {
    store: ContextStore,
}

impl ContextEngine {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ContextError>;
    pub fn store(&mut self, request: ContextPutRequest<'_>) -> Result<StoredContext, ContextError>;
    pub fn retrieve(&self, handle: &ContextHandleRecord, scope: &ContextScope) -> Result<Vec<u8>, ContextError>;
}
```

- [ ] **Step 4: Verify corruption and scope behavior**

Run: `cargo test -p viden-context store::tests`

Expected: all store tests pass; a modified blob returns `HashMismatch`; a task
handle requested from another task returns `ScopeDenied`.

- [ ] **Step 5: Run workspace dependency checks**

Run: `cargo test -p viden-types -p viden-context`

Expected: all tests pass with no UI crate depending directly on `viden-context`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/context
git commit -m "feat: add canonical context store"
```

### Task 3: Implement Deterministic Content Routing And Reducers

**Files:**
- Create: `crates/context/src/reducer.rs`
- Modify: `crates/context/src/lib.rs`
- Modify: `crates/context/Cargo.toml`

- [ ] **Step 1: Write reducer golden tests**

Create fixtures in test functions for JSON, Rust source, unified diff, failing
test logs, and transcript text. Assert deterministic output, required markers,
bounded size, omission records, and reducer version.

```rust
#[test]
fn log_reducer_keeps_first_failure_and_unique_errors() {
    let input = "running 9 tests\nERROR src/a.rs:9 boom\nERROR src/a.rs:9 boom\nfinal tail";
    let view = reduce(ContextContentKind::Log, input.as_bytes(), &ReductionPolicy::default()).unwrap();
    assert!(view.content.contains("src/a.rs:9 boom"));
    assert_eq!(view.content.matches("src/a.rs:9 boom").count(), 1);
    assert!(!view.omissions.is_empty());
    assert_eq!(view.reducer_version, "native-v1");
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p viden-context reducer::tests`

Expected: reducer symbols are missing.

- [ ] **Step 3: Implement router and native-v1 reducers**

Use structured parsers where available: `serde_json` for JSON and explicit
unified-diff/log scanners for line-oriented formats. Code v1 preserves imports,
declarations, signatures, and task-selected line ranges; it does not claim full
AST semantics. Text v1 preserves user constraints, decisions, unresolved
questions, and recent turns.

- [ ] **Step 4: Add quality validation and safe fallback**

`ReductionResult` must contain content, original/reduced estimates, omissions,
retained markers, reducer id/version, and `ContextQualityRecord`. Parse failure
returns bounded raw content with `fallback_raw=true`; missing required markers
returns `ContextError::QualityFailed`.

- [ ] **Step 5: Verify determinism**

Run twice: `cargo test -p viden-context reducer::tests`

Expected: byte-identical golden output on both runs.

- [ ] **Step 6: Commit**

```bash
git add crates/context
git commit -m "feat: add deterministic context reducers"
```

### Task 4: Build Role-Scoped Bundles And Enforce Budgets

**Files:**
- Modify: `crates/runtime/Cargo.toml`
- Modify: `crates/runtime/src/lib.rs`
- Modify: `crates/runtime/src/context_bundle.rs`
- Modify: `crates/runtime/src/runtime_contract.rs`
- Modify: `crates/runtime/src/tests/runtime_contract_tests.rs`
- Modify: `crates/runtime/src/tests/runtime_supervisor_tests.rs`

- [ ] **Step 1: Write failing runtime tests**

Test planner/coder/reviewer/tester source differences, shared-handle dedup,
preflight hard-limit rejection, and successful replay of bundle events.

```rust
#[test]
fn hard_context_limit_rejects_before_provider_request() {
    let (mut supervisor, provider) = test_supervisor_with_counting_provider();
    supervisor.set_context_budget_for_test(10, 20);
    let events = supervisor.dispatch(RuntimeCommand::SubmitUserInput { content: "x".repeat(500) }).unwrap();
    assert!(events.iter().any(|event| matches!(event.kind, RuntimeEventKind::ContextBudgetExceeded { .. })));
    assert_eq!(provider.request_count(), 0);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p viden-runtime hard_context_limit_rejects_before_provider_request -- --exact`

Expected: missing budget event or provider request count is one.

- [ ] **Step 3: Refactor `build_main_context_bundle`**

Preserve existing role-selection behavior, but store each selected source in
`ContextEngine`, derive a view, add a handle to the bundle, and calculate the
provider estimate from actual selected views. Keep source summaries for backward
compatible UI projection during the migration.

- [ ] **Step 4: Add pre-provider budget enforcement**

Soft budget triggers deterministic reduction and source priority eviction.
Hard limit rejects before provider transport and emits largest sources,
omissions, policy, and recovery actions. A provider 413 may rebuild once using
a stricter policy; the second failure remains visible and is not retried.

- [ ] **Step 5: Verify runtime and replay**

Run: `cargo test -p viden-runtime runtime_contract_tests runtime_supervisor_tests`

Expected: role bundles differ as specified, duplicate sources share hashes,
hard-limit calls never reach the provider, and replay reconstructs context view
state.

- [ ] **Step 6: Commit**

```bash
git add crates/runtime
git commit -m "feat: build reversible role context bundles"
```

### Task 5: Add Permission-Gated Context Retrieval

**Files:**
- Modify: `crates/runtime/src/runtime_contract.rs`
- Modify: `crates/runtime/src/runtime_supervisor.rs`
- Modify: `crates/runtime/src/tests/runtime_command_tests.rs`
- Modify: `crates/runtime/src/tests/runtime_supervisor_tests.rs`
- Modify: `crates/tools/src/lib.rs`

- [ ] **Step 1: Write failing retrieval tests**

Cover valid task retrieval, cross-task denial, secret exclusion, expired handle,
missing item, and cancellation while retrieval is queued.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p viden-runtime retrieve_context`

Expected: `RetrieveContext` is rejected as unsupported.

- [ ] **Step 3: Implement runtime retrieval**

Resolve handles only inside runtime. Run permission and scope checks before
reading bytes. Return bounded content through a tool result and emit a
`ContextRetrieved` record containing counts and reason, not the raw body.

- [ ] **Step 4: Verify input remains responsive**

Add a supervisor test that queues `QueueFollowUp` and `CancelActiveTurn` while a
retrieval worker is blocked. Expected: both commands are accepted and the
retrieval receives cancellation without locking the command loop.

- [ ] **Step 5: Run focused tests**

Run: `cargo test -p viden-runtime retrieve_context -- --nocapture`

Expected: allowed retrieval is byte-identical; denied retrieval emits a
recoverable error and no raw content.

- [ ] **Step 6: Commit**

```bash
git add crates/runtime crates/tools
git commit -m "feat: add scoped context retrieval"
```

### Task 6: Add Provider-Aware Cost Ledger And Cache Facts

**Files:**
- Create: `crates/context/src/cost.rs`
- Modify: `crates/context/src/lib.rs`
- Modify: `crates/provider/src/transport.rs`
- Modify: `crates/provider/src/parse.rs`
- Modify: `crates/runtime/src/runtime_contract.rs`
- Modify: `crates/runtime/src/tests/live_deepseek_tests.rs`

- [ ] **Step 1: Write failing aggregation tests**

Test exact token summation, actual-vs-estimated labels, cached input tokens,
retry attribution, and task/DAG/workflow rollups.

```rust
#[test]
fn ledger_never_converts_unknown_actual_cost_to_zero() {
    let mut ledger = CostLedger::default();
    ledger.append(CostUsageRecord::provider_usage("req-1", 100, 20, None));
    let total = ledger.total(CostScope::Workflow("wf-1".into()));
    assert_eq!(total.actual_cost_micro_usd, None);
    assert_eq!(total.input_tokens, 100);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p viden-context ledger_never_converts_unknown_actual_cost_to_zero -- --exact`

Expected: ledger symbols are missing.

- [ ] **Step 3: Normalize provider usage facts**

Extend provider telemetry with optional cached-input tokens and provider-reported
cost when a protocol exposes them. Keep pricing lookup outside protocol parsing;
estimated cost records include provider/model, price-table version, currency,
and `estimated=true`.

- [ ] **Step 4: Emit ledger events and project totals**

Append one entry per provider attempt and retrieval. Aggregate by request,
AgentTask, DAG, workflow, and smoke-run ids. Emit `CostUsageRecorded` and
`ProviderCacheObserved`; update `RuntimeViewState` from events only.

- [ ] **Step 5: Verify accounting**

Run: `cargo test -p viden-context -p viden-provider -p viden-runtime cost`

Expected: token totals match provider telemetry exactly; unknown actual cost
remains unknown; retries appear as separate entries.

- [ ] **Step 6: Commit**

```bash
git add crates/context crates/provider crates/runtime
git commit -m "feat: add task and workflow cost ledger"
```

### Task 7: Require Canonical Evidence In Merge Gate

**Files:**
- Modify: `crates/types/src/lib.rs`
- Modify: `crates/runtime/src/runtime_contract.rs`
- Modify: `crates/runtime/src/tests/runtime_contract_tests.rs`
- Modify: `crates/runtime/src/tests/runtime_supervisor_tests.rs`
- Modify: `crates/workflows/src/tasks.rs`
- Modify: `crates/workflows/src/tasks/tests.rs`

- [ ] **Step 1: Write failing gate tests**

Test rejection of summary-only evidence, acceptance of verified canonical
evidence, hash mismatch, missing source, and replay after restart.

```rust
#[test]
fn merge_gate_rejects_summary_only_patch_evidence() {
    let evidence = evidence_view("patch", "looks good");
    assert_eq!(canonical_evidence_status(&evidence), EvidenceCanonicalStatus::Missing);
    assert_eq!(reduce_merge_gate_status(&gate_requiring_patch(), &[evidence]), MergeGateStatus::CollectingEvidence);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p viden-runtime merge_gate_rejects_summary_only_patch_evidence -- --exact`

Expected: canonical evidence status is missing from contracts.

- [ ] **Step 3: Link evidence to canonical context**

Add canonical item id, bundle id, source hash, producer, permission snapshot id,
and verification state to evidence records. Preserve append-only workflow events.

- [ ] **Step 4: Enforce gate rules**

Required patch/test/review/doc/release evidence counts only when canonical source
exists, hash verifies, scope is valid, and quality status passes. A failed check
moves the gate to `blocked` or `needs_changes` with a machine-readable reason.

- [ ] **Step 5: Verify reducer and replay**

Run: `cargo test -p viden-workflows -p viden-runtime merge_gate`

Expected: summary-only evidence never accepts a gate; restart/replay reaches the
same status as the live reducer.

- [ ] **Step 6: Commit**

```bash
git add crates/types crates/runtime crates/workflows
git commit -m "feat: verify canonical merge gate evidence"
```

### Task 8: Define The Optional Context Adapter Boundary

**Files:**
- Modify: `crates/plugin-api/src/lib.rs`
- Modify: `crates/plugin-host/src/lib.rs`
- Modify: `crates/plugin-api/Cargo.toml`
- Modify: `crates/plugin-host/Cargo.toml`
- Modify: `docs/plugin-architecture.md`
- Modify: `docs/plugin-architecture.zh-CN.md`

- [ ] **Step 1: Write failing descriptor and fallback tests**

Test registration of a `context_reducer` capability, version negotiation,
timeout, malformed response, process absence, and native fallback.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p viden-plugin-api -p viden-plugin-host context_reducer`

Expected: the capability is unknown.

- [ ] **Step 3: Add adapter contracts**

Define request/response envelopes containing content kind, canonical hash,
policy, reduced content, omissions, reducer identity/version, and quality facts.
Do not expose credentials or canonical storage paths. External reducers are
disabled by default and require explicit configuration.

- [ ] **Step 4: Implement native fallback semantics**

Adapter timeout, crash, invalid hash, or quality failure must produce health
evidence and fall back to the native reducer. It must never block startup or
provider access when native processing is healthy.

- [ ] **Step 5: Verify no mandatory Headroom dependency**

Run: `cargo tree | rg -i 'headroom|pyo3|python'`

Expected: no output from production dependencies.

Run: `cargo test -p viden-plugin-api -p viden-plugin-host`

Expected: all adapter and fallback tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/plugin-api crates/plugin-host docs/plugin-architecture.md docs/plugin-architecture.zh-CN.md
git commit -m "feat: add optional context reducer capability"
```

### Task 9: Add Deterministic And DeepSeek A/B Gates

**Files:**
- Create: `scripts/context-engine-benchmark.sh`
- Modify: `scripts/deepseek-dev-scenario-smoke.sh`
- Modify: `scripts/release-gate.sh`
- Modify: `crates/runtime/src/tests/live_deepseek_tests.rs`
- Modify: `docs/testing-validation-plan.md`
- Modify: `docs/testing-validation-plan.zh-CN.md`

- [ ] **Step 1: Write failing script contract tests**

Add a dry-run mode using fixture usage JSON. It must reject missing fields,
task-success mismatch, evidence mismatch, or input-token median reduction below
20%.

- [ ] **Step 2: Verify RED**

Run: `scripts/context-engine-benchmark.sh --fixtures crates/runtime/src/tests/fixtures/context-benchmark --out-dir /tmp/viden-context-benchmark`

Expected: command is missing.

- [ ] **Step 3: Implement benchmark runner**

For release candidates, run the same disposable development scenario three
times with `VIDEN_CONTEXT_ENGINE=off` and three times with it `on`. Store each
run's prompt version, provider/model, task result, test result, evidence hashes,
input/output/cached tokens, estimated/actual cost, first-token/total latency,
retrieval count, retry count, compression ratio, and failure class.

- [ ] **Step 4: Implement release thresholds**

Pass only when:

- median input tokens improve by at least 20%;
- all six runs complete the task and tests;
- required evidence hashes are present in both cohorts;
- no permission bypass or unclassified failure occurs;
- engine-on p95 local bundle build for the fixture is at most 200 ms;
- no provider 413/context-overflow occurs in the engine-on cohort.

- [ ] **Step 5: Run deterministic gate**

Run: `scripts/context-engine-benchmark.sh --fixtures crates/runtime/src/tests/fixtures/context-benchmark --out-dir /tmp/viden-context-benchmark`

Expected: exit 0 and produce `summary.md`, `comparison.json`, per-run usage JSON,
and failure classification.

- [ ] **Step 6: Run billable live gate**

Run: `scripts/context-engine-benchmark.sh --provider deepseek --model "${VIDEN_LIVE_DEEPSEEK_MODEL:-deepseek-v4-flash}" --runs 3 --out-dir /tmp/viden-context-live`

Expected: exit 0 with six successful runs and a summary containing median token,
cost, latency, retrieval, and success comparisons. This step requires
`DEEPSEEK_API_KEY` and must report the final billable token/cost totals.

- [ ] **Step 7: Commit**

```bash
git add scripts crates/runtime/src/tests/live_deepseek_tests.rs docs/testing-validation-plan.md docs/testing-validation-plan.zh-CN.md
git commit -m "test: add context engine release benchmark"
```

### Task 10: Synchronize Product, Architecture, And Client Contracts

**Files:**
- Modify: `PLAN.md`
- Modify: `docs/product-requirements.md`
- Modify: `docs/product-requirements.zh-CN.md`
- Modify: `docs/architecture.md`
- Modify: `docs/architecture.zh-CN.md`
- Modify: `docs/staged-roadmap.md`
- Modify: `docs/staged-roadmap.zh-CN.md`
- Modify: `docs/long-term-roadmap.md`
- Modify: `docs/long-term-roadmap.zh-CN.md`
- Modify: `docs/multi-agent-core-orchestration.md`
- Modify: `docs/multi-agent-core-orchestration.zh-CN.md`
- Create: `docs/ui-collaboration-guide.md` as the English counterpart of the
  existing Chinese collaboration guide.
- Modify: `docs/ui-collaboration-guide.zh-CN.md`
- Create: `scripts/check-doc-pairs.sh`
- Create: `scripts/check-doc-links.sh`
- Create: `scripts/check-dependency-boundaries.sh`

- [ ] **Step 1: Add one source-of-truth link from each overview document**

State the native-core decision, version allocation, canonical evidence
invariant, and UI dependency rule. Link to the design spec instead of copying
the full component descriptions into every overview.

- [ ] **Step 2: Document TUI/GUI integration**

List the new commands/events/view-state fields and explicitly prohibit direct
dependencies from UI apps to `crates/context`, `crates/provider`, `crates/tools`,
or workflow internals.

- [ ] **Step 3: Add reusable documentation and dependency guards**

`check-doc-pairs.sh` accepts edited Markdown paths and requires a matching
English/`*.zh-CN.md` pair. `check-doc-links.sh` accepts edited Markdown paths
and verifies relative local links. `check-dependency-boundaries.sh` parses
frontend Cargo manifests and rejects direct dependencies matching:

```text
viden-context
viden-provider
viden-tools
viden-workflows
viden-runtime
```

Allow `viden-core`, `viden-types`, configuration, and UI-only crates. Add shell
fixture tests that prove each guard exits non-zero for a deliberately invalid
temporary fixture and zero for a valid fixture.

- [ ] **Step 4: Run bilingual and link checks**

Run: `scripts/check-doc-pairs.sh docs/product-requirements.md docs/product-requirements.zh-CN.md docs/architecture.md docs/architecture.zh-CN.md docs/staged-roadmap.md docs/staged-roadmap.zh-CN.md docs/long-term-roadmap.md docs/long-term-roadmap.zh-CN.md docs/multi-agent-core-orchestration.md docs/multi-agent-core-orchestration.zh-CN.md`

Expected: exit 0 with all edited English/Chinese pairs present.

Run: `scripts/check-doc-links.sh docs/superpowers/specs/2026-07-18-context-evidence-cost-engine-design.md docs/superpowers/specs/2026-07-18-context-evidence-cost-engine-design.zh-CN.md docs/superpowers/plans/2026-07-18-context-evidence-cost-engine.md docs/superpowers/plans/2026-07-18-context-evidence-cost-engine.zh-CN.md`

Expected: exit 0 with no broken local links.

- [ ] **Step 5: Run formatting and full tests**

Run: `cargo fmt --all -- --check`

Expected: exit 0.

Run: `cargo test --workspace --quiet`

Expected: all non-live workspace tests pass.

- [ ] **Step 6: Run dependency guard**

Run: `scripts/check-dependency-boundaries.sh`

Expected: UI apps consume core/runtime contracts only and do not import context,
provider, tool, or workflow internals.

- [ ] **Step 7: Commit documentation and contract guidance**

```bash
git add PLAN.md docs scripts/check-doc-pairs.sh scripts/check-doc-links.sh scripts/check-dependency-boundaries.sh
git commit -m "docs: define context evidence and cost rollout"
```

## Final Acceptance Matrix

| Area | Blocking acceptance |
| --- | --- |
| Correctness | Canonical retrieval is byte-identical; hash corruption and scope violation are rejected. |
| Context | Every provider-backed AgentTask has bundle id, role policy, handles, omissions, and hard-limit result. |
| Reduction | Deterministic golden tests pass for JSON/code/diff/log/transcript; every view records reducer/version/omissions/quality. |
| Cost | Provider token totals reconcile exactly; unknown actual cost remains unknown; estimates are labelled and versioned. |
| Evidence | Summary-only evidence cannot accept a Merge Gate; canonical source and permission snapshot are required. |
| Runtime | All new state is replayable through runtime events; composer/command loop remains responsive during build/retrieval. |
| Security | Secrets and excluded sources are not projected or retrievable across scopes; no permission path is bypassed. |
| Architecture | TUI/GUI have no direct dependencies on context/provider/tool/workflow internals; Headroom is absent from mandatory dependencies. |
| Reliability | Missing adapter, corrupt store, reducer failure, provider 413, cancellation, and restart have deterministic recovery tests. |
| Performance | Deterministic fixture bundle build p95 is at most 200 ms and canonical retrieval p95 is at most 50 ms on the release machine. |
| Live quality | Three DeepSeek A/B runs per cohort show at least 20% median input-token reduction, identical task/test success, complete evidence, and no new failure class. |
| Release | Workspace tests, doc checks, dependency guard, deterministic benchmark, billable DeepSeek benchmark, release gate, GitHub assets, and synchronized Homebrew validation all pass. |

## Completion Evidence

The release status must link:

- exact commit and branch;
- workspace test log;
- deterministic benchmark summary;
- six-run DeepSeek A/B summary with total tokens, cost, and duration;
- context/retrieval/cost event replay fixture;
- dependency-boundary result;
- remaining risks and any metric waiver approved by the user.

No threshold may be waived silently. A waiver records owner, reason, affected
metric, expiry release, and follow-up task.
