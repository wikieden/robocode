use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use viden_core::{CORE_CLIENT_CAPABILITIES, CORE_EXTENSION_CAPABILITIES, frontend_capabilities};
use viden_types::{
    AgentDagRecord, AgentDagStatus, AgentDagTaskSpec, AgentLaneRecord, AgentNextAction, AgentRole,
    AgentRoute, AgentTaskKind, AgentTaskRecord, AgentTaskStatus, ApprovalDecision,
    ApprovalDefaultAction, ApprovalRequestView, ApprovalResponse, ApprovalRisk, ApprovalScope,
    ApprovalTarget, CapabilityId, ContextBundleRecord, ContextOmittedSourceRecord,
    ContextSourceRecord, CostScope, CostUsageOutcome, CostUsageRecord, EventCursor, EvidenceView,
    ExecutionTarget, FRONTEND_SCHEMA_V1, GateStrength, LaneBudget, LaneStatus, MergeGateRecord,
    MergeGateStatus, MutationPolicy, PermissionLevel, PermissionMode, QueuedInputView,
    ResolvedUiPreferences, RuntimeErrorView, RuntimeEvent, RuntimeEventEnvelope, RuntimeOwner,
    RuntimeSnapshot, RuntimeViewState, RuntimeWireEvent, SchemaVersion, TokenCostView, TokenUsage,
    UiColorMode, UiDensity, UiMotion, UiPreferences, UiSkin, WorkMode,
};

const FIXTURE_DIR: &str = "tests/fixtures/frontend-contract-v1";
const REQUIRED_FIXTURES: [&str; 9] = [
    "stream-tool.json",
    "approval-allow-deny.json",
    "queued-follow-up.json",
    "dag-blocker.json",
    "multi-lane.json",
    "merge-gate.json",
    "context-pressure-cost-blind.json",
    "plan-denial.json",
    "d1-vertical-slice.json",
];

#[derive(Debug, Deserialize)]
struct FrontendContractFixture {
    fixture_id: String,
    schema_version: SchemaVersion,
    required_capabilities: Vec<CapabilityId>,
    initial_snapshot: RuntimeSnapshot,
    events: Vec<RuntimeEventEnvelope>,
    expected_final_cursor: EventCursor,
    expected_view_sha256: String,
}

#[derive(Debug, Serialize)]
struct FrontendContractFixtureOut {
    fixture_id: String,
    schema_version: SchemaVersion,
    required_capabilities: Vec<CapabilityId>,
    initial_snapshot: RuntimeSnapshot,
    events: Vec<RuntimeEventEnvelope>,
    expected_final_cursor: EventCursor,
    expected_view_sha256: String,
}

#[test]
fn frontend_contract_v1_corpus_replays_deterministically() {
    let root = fixture_root();
    let missing = REQUIRED_FIXTURES
        .iter()
        .filter(|name| !root.join(name).exists())
        .copied()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "missing frontend-contract-v1 fixtures: {}",
        missing.join(", ")
    );

    for name in REQUIRED_FIXTURES {
        let fixture = read_fixture(&root, name);
        assert_fixture_identity(name, &fixture);
        assert_capabilities_are_sorted_unique_and_advertised(name, &fixture);
        assert_cursors_are_contiguous(name, &fixture);
        let (first_view, first_cursor, first_digest) = replay_fixture(&fixture);
        let (second_view, second_cursor, second_digest) = replay_fixture(&fixture);
        assert_eq!(first_cursor, fixture.expected_final_cursor, "{name}");
        assert_eq!(first_cursor, second_cursor, "{name}");
        assert_eq!(first_view, second_view, "{name}");
        assert_eq!(first_digest, second_digest, "{name}");
        assert_eq!(first_digest, fixture.expected_view_sha256, "{name}");
        assert_scenario_facts(name, &first_view);
    }
}

#[test]
fn frontend_contract_v1_capability_source_is_frozen_and_sorted() {
    let expected = [
        "runtime.agent_dag",
        "runtime.approvals",
        "runtime.commands",
        "runtime.context",
        "runtime.cost",
        "runtime.events",
        "runtime.evidence",
        "runtime.merge_gate",
        "runtime.queued_input",
        "runtime.replay",
        "runtime.snapshot",
        "runtime.transcript_page",
        "runtime.typed_lanes",
        "runtime.typed_tasks",
        "ui.preferences",
    ];
    assert_eq!(CORE_CLIENT_CAPABILITIES, expected);
    assert!(
        CORE_CLIENT_CAPABILITIES
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    let advertised = frontend_capabilities();
    assert_eq!(CORE_EXTENSION_CAPABILITIES, ["runtime.lane_lifecycle"]);
    assert_eq!(
        advertised.len(),
        expected.len() + CORE_EXTENSION_CAPABILITIES.len()
    );
    for capability in expected {
        assert!(advertised.contains(&CapabilityId(capability.to_string())));
    }
    assert!(advertised.contains(&CapabilityId("runtime.lane_lifecycle".to_string())));
    let extension_manifest = include_str!("../frontend-contract-extensions.toml");
    assert!(extension_manifest.contains("base_component_version = \"0.3.0\""));
    assert!(extension_manifest.contains("candidate_component_version = \"0.3.1\""));
    assert!(extension_manifest.contains("compatibility = \"additive_capability_gated\""));
}

#[test]
fn frontend_contract_v1_migrations_are_idempotent_before_fixture_replay() {
    let legacy_lanes = parse_legacy_lanes_tsv(include_str!(
        "../../types/tests/fixtures/frontend-contract-v1/legacy-lanes.tsv"
    ));
    let typed_lanes: Vec<AgentLaneRecord> = serde_json::from_str(include_str!(
        "../../types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
    ))
    .expect("typed lane fixture should parse");
    assert_eq!(legacy_lanes, typed_lanes);
    let reparsed: Vec<AgentLaneRecord> =
        serde_json::from_value(serde_json::to_value(&legacy_lanes).unwrap()).unwrap();
    assert_eq!(reparsed, typed_lanes);

    let legacy_cost = r#"{
        "provider_id": "fixture-provider",
        "model": "fixture-model",
        "scope": {"type": "task", "id": "task_migration"},
        "input_tokens": 10,
        "output_tokens": 5,
        "cached_input_tokens": 2,
        "total_tokens": 15,
        "estimated_cost_micro_usd": 42,
        "actual_cost_micro_usd": null,
        "request_id": "request_migration",
        "attempt_index": 1,
        "outcome": "success",
        "recorded_at": 1700000000
    }"#;
    let first_cost: CostUsageRecord = serde_json::from_str(legacy_cost).unwrap();
    let second_cost: CostUsageRecord =
        serde_json::from_value(serde_json::to_value(&first_cost).unwrap()).unwrap();
    assert_eq!(first_cost, second_cost);
    assert_eq!(first_cost.actual_cost, None);

    let legacy_approval = r#"{"approved": false, "feedback": "deny mutation"}"#;
    let first_approval: ApprovalResponse = serde_json::from_str(legacy_approval).unwrap();
    let second_approval: ApprovalResponse =
        serde_json::from_value(serde_json::to_value(&first_approval).unwrap()).unwrap();
    assert_eq!(first_approval, second_approval);
    assert!(matches!(second_approval.decision, ApprovalDecision::Deny));
}

#[test]
#[ignore = "manual fixture refresh; normal tests validate committed JSON only"]
fn refresh_frontend_contract_v1_fixtures() {
    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("typed-lanes.json"),
        serde_json::to_string_pretty(&typed_lanes_fixture()).unwrap() + "\n",
    )
    .unwrap();
    for fixture in build_fixtures() {
        let path = root.join(format!("{}.json", fixture.fixture_id));
        fs::write(path, serde_json::to_string_pretty(&fixture).unwrap() + "\n").unwrap();
    }
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core crate should have workspace sibling crates")
        .join("types")
        .join(FIXTURE_DIR)
}

fn read_fixture(root: &Path, name: &str) -> FrontendContractFixture {
    let path = root.join(name);
    let raw = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "failed to read frontend-contract-v1 fixture {}: {err}",
            path.display()
        )
    });
    assert_no_local_paths_or_secrets(name, &raw);
    serde_json::from_str(&raw).unwrap_or_else(|err| {
        panic!(
            "failed to parse frontend-contract-v1 fixture {}: {err}",
            path.display()
        )
    })
}

fn assert_fixture_identity(name: &str, fixture: &FrontendContractFixture) {
    assert_eq!(fixture.schema_version, FRONTEND_SCHEMA_V1, "{name}");
    assert_eq!(
        format!("{}.json", fixture.fixture_id),
        name,
        "{name} fixture_id must match file name"
    );
    assert_eq!(fixture.expected_view_sha256.len(), 64, "{name}");
    assert!(
        fixture
            .expected_view_sha256
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()),
        "{name} digest must be lowercase hex"
    );
}

fn assert_capabilities_are_sorted_unique_and_advertised(
    name: &str,
    fixture: &FrontendContractFixture,
) {
    assert!(
        fixture
            .required_capabilities
            .windows(2)
            .all(|pair| pair[0] < pair[1]),
        "{name} required capabilities must be sorted and unique"
    );
    let advertised = frontend_capabilities();
    for capability in &fixture.required_capabilities {
        assert!(
            advertised.contains(capability),
            "{name} requires unadvertised capability {}",
            capability.0
        );
    }
}

fn assert_cursors_are_contiguous(name: &str, fixture: &FrontendContractFixture) {
    assert!(!fixture.events.is_empty(), "{name} must be non-empty");
    let stream_id = format!("fixture:{}", fixture.fixture_id);
    for (index, envelope) in fixture.events.iter().enumerate() {
        let expected_sequence = index as u64 + 1;
        assert_eq!(envelope.schema_version, FRONTEND_SCHEMA_V1, "{name}");
        assert_eq!(envelope.cursor.stream_id, stream_id, "{name}");
        assert_eq!(envelope.cursor.sequence, expected_sequence, "{name}");
        if let RuntimeWireEvent::Known(event) = &envelope.event {
            assert_eq!(event.sequence, expected_sequence, "{name}");
        }
    }
    assert_eq!(
        fixture.expected_final_cursor,
        fixture.events.last().unwrap().cursor,
        "{name}"
    );
}

fn assert_no_local_paths_or_secrets(name: &str, raw: &str) {
    let forbidden = ["/Users/", "\\Users\\", "sk-", "OPENAI_API_KEY", "password"];
    for token in forbidden {
        assert!(
            !raw.contains(token),
            "{name} contains local path or secret marker `{token}`"
        );
    }
}

fn replay_fixture(fixture: &FrontendContractFixture) -> (RuntimeViewState, EventCursor, String) {
    let mut view = RuntimeViewState::new(fixture.initial_snapshot.clone());
    let mut cursor = EventCursor {
        stream_id: format!("fixture:{}", fixture.fixture_id),
        sequence: 0,
    };
    for envelope in &fixture.events {
        if let RuntimeWireEvent::Known(event) = &envelope.event {
            view.apply_event(event);
        }
        cursor = envelope.cursor.clone();
    }
    let digest = canonical_view_sha256(&view);
    (view, cursor, digest)
}

fn canonical_view_sha256(view: &RuntimeViewState) -> String {
    let value = serde_json::to_value(view).expect("runtime view must serialize");
    let sorted = sort_json(value);
    let bytes = serde_json::to_vec(&sorted).expect("canonical json must serialize");
    format!("{:x}", Sha256::digest(bytes))
}

fn sort_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let sorted = map
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>();
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_json).collect())
        }
        other => other,
    }
}

fn assert_scenario_facts(name: &str, view: &RuntimeViewState) {
    match name {
        "stream-tool.json" => {
            assert_eq!(view.assistant_stream, "Checking repository");
            assert!(view.active_tool_calls.is_empty());
            assert!(
                view.latest_evidence
                    .iter()
                    .any(|evidence| evidence.id == "ev_tool_ok")
            );
        }
        "approval-allow-deny.json" => {
            assert!(view.pending_approvals.is_empty());
            assert_eq!(view.errors.len(), 1);
            assert!(view.errors[0].message.contains("denied"));
        }
        "queued-follow-up.json" => {
            assert!(view.queued_inputs.is_empty());
            assert_eq!(view.assistant_stream, "working");
        }
        "dag-blocker.json" => {
            assert!(
                view.agent_dags
                    .iter()
                    .any(|dag| dag.status == AgentDagStatus::Blocked)
            );
            assert!(
                view.tasks
                    .iter()
                    .any(|task| task.status == AgentTaskStatus::Blocked)
            );
            assert!(view.tasks.iter().any(|task| {
                task.next_action
                    .as_ref()
                    .is_some_and(|action| action.label == "Retry blocker")
            }));
        }
        "multi-lane.json" => {
            assert_eq!(view.lanes.len(), 2);
            assert!(view.lanes.iter().any(|lane| lane.role == AgentRole::Coder));
            assert!(
                view.lanes
                    .iter()
                    .any(|lane| lane.role == AgentRole::Reviewer)
            );
        }
        "merge-gate.json" => {
            assert!(
                view.latest_evidence
                    .iter()
                    .any(|evidence| evidence.kind == "review")
            );
            assert!(
                view.merge_gates
                    .iter()
                    .any(|gate| gate.status == MergeGateStatus::Accepted)
            );
        }
        "context-pressure-cost-blind.json" => {
            let context = view.context.as_ref().expect("context must be projected");
            assert_eq!(context.pressure_percent(), 92);
            assert_eq!(view.cost_ledger.total_actual_cost_micro_usd, None);
        }
        "plan-denial.json" => {
            assert_eq!(view.snapshot.work_mode, WorkMode::Plan);
            assert!(
                view.errors
                    .iter()
                    .any(|error| error.message.contains("Plan mode"))
            );
            assert!(view.latest_evidence.is_empty());
        }
        "d1-vertical-slice.json" => {
            assert_eq!(view.assistant_stream, "D1 cockpit state");
            assert!(view.lanes.iter().any(|lane| lane.id == "lane_d1_core"));
            assert!(view.pending_approvals.is_empty());
            assert!(
                view.merge_gates
                    .iter()
                    .any(|gate| gate.gate_id == "gate_d1")
            );
            assert_eq!(view.token_cost.as_ref().unwrap().cost_micro_usd, None);
            assert_d1_preferences_are_snapshot_capabilities_not_view_fields(view);
        }
        other => panic!("unhandled fixture {other}"),
    }
}

fn assert_d1_preferences_are_snapshot_capabilities_not_view_fields(view: &RuntimeViewState) {
    assert_eq!(
        view.snapshot.ui_preferences,
        ResolvedUiPreferences {
            locale: viden_types::LocaleId::ZhCn,
            skin: UiSkin::Aurora,
            mode: UiColorMode::Dark,
            density: UiDensity::Regular,
            motion: UiMotion::Reduced,
            diagnostics: Vec::new(),
        }
    );
    let config_summary: serde_json::Value =
        serde_json::from_str(&view.snapshot.config_summary).unwrap();
    assert_eq!(
        config_summary["ui"]["effective"],
        serde_json::json!({
            "locale": "zh-CN",
            "skin": "aurora",
            "mode": "dark",
            "density": "regular",
            "motion": "reduced"
        })
    );
    assert_eq!(
        config_summary["design_entry"]["hierarchy"],
        serde_json::json!([
            "docs/viden-design/Viden/index.html",
            "client design index",
            "component library",
            "TUI unified prototype or GUI D1 desktop cockpit"
        ])
    );
    assert_eq!(
        config_summary["design_entry"]["d11"],
        "onboarding subordinate"
    );
}

fn parse_legacy_lanes_tsv(raw: &str) -> Vec<AgentLaneRecord> {
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let parts = line.split('\t').collect::<Vec<_>>();
            assert_eq!(parts.len(), 7, "legacy lane row should have seven columns");
            let stable_id = parts[0].trim_start_matches("L-").replace('-', "_");
            let legacy = serde_json::json!({
                "id": parts[0],
                "task_id": format!("task_{stable_id}"),
                "agent": parts[1],
                "screen": parts[2],
                "transport": parts[4],
                "status": parts[3],
                "summary": parts[6],
                "evidence": [format!("evidence_{stable_id}")],
            });
            serde_json::from_value(legacy).unwrap()
        })
        .collect()
}

fn build_fixtures() -> Vec<FrontendContractFixtureOut> {
    vec![
        fixture(
            "stream-tool",
            &["runtime.events", "runtime.evidence", "runtime.snapshot"],
            snapshot(WorkMode::Build),
            envelopes(
                "stream-tool",
                vec![
                    RuntimeEventKindExt::Assistant("msg_stream", "Checking "),
                    RuntimeEventKindExt::ToolStarted("tool_rg", "rg", "rg TODO"),
                    RuntimeEventKindExt::ToolFinished(
                        "tool_rg",
                        "rg",
                        true,
                        Some(evidence("ev_tool_ok", "tool_log", "rg completed")),
                    ),
                    RuntimeEventKindExt::Assistant("msg_stream", "repository"),
                ],
            ),
        ),
        fixture(
            "approval-allow-deny",
            &[
                "runtime.approvals",
                "runtime.commands",
                "runtime.events",
                "runtime.snapshot",
            ],
            snapshot(WorkMode::Build),
            envelopes(
                "approval-allow-deny",
                vec![
                    RuntimeEventKindExt::ApprovalRequested("approval_allow", "Edit docs", true),
                    RuntimeEventKindExt::ApprovalResolved("approval_allow", true),
                    RuntimeEventKindExt::ApprovalRequested("approval_deny", "Delete file", true),
                    RuntimeEventKindExt::ApprovalResolved("approval_deny", false),
                    RuntimeEventKindExt::Error("approval denied before effect execution", true),
                ],
            ),
        ),
        fixture(
            "queued-follow-up",
            &["runtime.events", "runtime.queued_input", "runtime.snapshot"],
            snapshot(WorkMode::Build),
            envelopes(
                "queued-follow-up",
                vec![
                    RuntimeEventKindExt::Assistant("msg_queue", "working"),
                    RuntimeEventKindExt::InputQueued("input_continue", "continue with tests"),
                    RuntimeEventKindExt::InputDequeued("input_continue"),
                ],
            ),
        ),
        fixture(
            "dag-blocker",
            &[
                "runtime.agent_dag",
                "runtime.events",
                "runtime.snapshot",
                "runtime.typed_tasks",
            ],
            snapshot(WorkMode::Build),
            envelopes(
                "dag-blocker",
                vec![
                    RuntimeEventKindExt::Dag("dag_blocker", AgentDagStatus::Blocked),
                    RuntimeEventKindExt::Task(
                        "task_blocked",
                        AgentRole::Coder,
                        AgentTaskStatus::Blocked,
                        "dependency missing",
                        Some("Retry blocker"),
                    ),
                    RuntimeEventKindExt::Error("blocked by task_dependency_missing", true),
                ],
            ),
        ),
        fixture(
            "multi-lane",
            &[
                "runtime.events",
                "runtime.snapshot",
                "runtime.typed_lanes",
                "runtime.typed_tasks",
            ],
            snapshot(WorkMode::Build),
            envelopes(
                "multi-lane",
                vec![
                    RuntimeEventKindExt::Lane(
                        "lane_core",
                        "task_core",
                        AgentRole::Coder,
                        AgentRoute::Terminal,
                        LaneStatus::Running,
                        ExecutionTarget::Local,
                    ),
                    RuntimeEventKindExt::Lane(
                        "lane_review",
                        "task_review",
                        AgentRole::Reviewer,
                        AgentRoute::Acp,
                        LaneStatus::WaitingApproval,
                        ExecutionTarget::Ssh {
                            host: "review.example.test".to_string(),
                        },
                    ),
                    RuntimeEventKindExt::Task(
                        "task_core",
                        AgentRole::Coder,
                        AgentTaskStatus::Running,
                        "core coding",
                        None,
                    ),
                    RuntimeEventKindExt::Task(
                        "task_review",
                        AgentRole::Reviewer,
                        AgentTaskStatus::WaitingApproval,
                        "review queue",
                        None,
                    ),
                ],
            ),
        ),
        fixture(
            "merge-gate",
            &[
                "runtime.events",
                "runtime.evidence",
                "runtime.merge_gate",
                "runtime.snapshot",
            ],
            snapshot(WorkMode::Build),
            envelopes(
                "merge-gate",
                vec![
                    RuntimeEventKindExt::Evidence(evidence("ev_patch", "patch", "patch applied")),
                    RuntimeEventKindExt::Evidence(evidence(
                        "ev_test",
                        "test_result",
                        "tests passed",
                    )),
                    RuntimeEventKindExt::Evidence(evidence("ev_review", "review", "review passed")),
                    RuntimeEventKindExt::MergeGate(
                        "gate_merge",
                        "task_merge",
                        MergeGateStatus::Accepted,
                        vec!["ev_patch", "ev_test", "ev_review"],
                    ),
                ],
            ),
        ),
        fixture(
            "context-pressure-cost-blind",
            &[
                "runtime.context",
                "runtime.cost",
                "runtime.events",
                "runtime.snapshot",
            ],
            snapshot(WorkMode::Build),
            envelopes(
                "context-pressure-cost-blind",
                vec![
                    RuntimeEventKindExt::ContextPressure,
                    RuntimeEventKindExt::CostUnknown,
                ],
            ),
        ),
        fixture(
            "plan-denial",
            &["runtime.commands", "runtime.events", "runtime.snapshot"],
            snapshot(WorkMode::Plan),
            envelopes(
                "plan-denial",
                vec![
                    RuntimeEventKindExt::CommandRejected(
                        "cmd_mutate",
                        "Plan mode blocks file/shell/Git/workflow/memory/task mutations before execution",
                    ),
                    RuntimeEventKindExt::Error("Plan mode denied mutation before execution", true),
                ],
            ),
        ),
        fixture(
            "d1-vertical-slice",
            &[
                "runtime.agent_dag",
                "runtime.approvals",
                "runtime.context",
                "runtime.cost",
                "runtime.events",
                "runtime.evidence",
                "runtime.merge_gate",
                "runtime.typed_lanes",
                "runtime.typed_tasks",
                "ui.preferences",
            ],
            snapshot(WorkMode::Build),
            envelopes(
                "d1-vertical-slice",
                vec![
                    RuntimeEventKindExt::Assistant("msg_d1", "D1 cockpit state"),
                    RuntimeEventKindExt::ToolStarted(
                        "tool_d1",
                        "cargo",
                        "cargo test -p viden-core",
                    ),
                    RuntimeEventKindExt::ToolFinished(
                        "tool_d1",
                        "cargo",
                        true,
                        Some(evidence("ev_d1_test", "test_result", "core tests passed")),
                    ),
                    RuntimeEventKindExt::Lane(
                        "lane_d1_core",
                        "task_d1_core",
                        AgentRole::Coder,
                        AgentRoute::Terminal,
                        LaneStatus::Running,
                        ExecutionTarget::Local,
                    ),
                    RuntimeEventKindExt::Task(
                        "task_d1_core",
                        AgentRole::Coder,
                        AgentTaskStatus::Running,
                        "Core contract freeze",
                        Some("Run parity fixtures"),
                    ),
                    RuntimeEventKindExt::ApprovalRequested(
                        "approval_d1",
                        "Allow fixture write",
                        true,
                    ),
                    RuntimeEventKindExt::ApprovalResolved("approval_d1", true),
                    RuntimeEventKindExt::Evidence(evidence(
                        "ev_d1_review",
                        "review",
                        "contract review passed",
                    )),
                    RuntimeEventKindExt::MergeGate(
                        "gate_d1",
                        "task_d1_core",
                        MergeGateStatus::CollectingEvidence,
                        vec!["ev_d1_test", "ev_d1_review"],
                    ),
                    RuntimeEventKindExt::ContextPressure,
                    RuntimeEventKindExt::CostUnknown,
                    RuntimeEventKindExt::Error(
                        "recovered from missing optional GUI panel state",
                        true,
                    ),
                    RuntimeEventKindExt::TokenCostUnknown,
                ],
            ),
        ),
    ]
}

fn fixture(
    fixture_id: &str,
    required_capabilities: &[&str],
    initial_snapshot: RuntimeSnapshot,
    events: Vec<RuntimeEventEnvelope>,
) -> FrontendContractFixtureOut {
    let mut view = RuntimeViewState::new(initial_snapshot.clone());
    for envelope in &events {
        if let RuntimeWireEvent::Known(event) = &envelope.event {
            view.apply_event(event);
        }
    }
    let expected_final_cursor = events.last().unwrap().cursor.clone();
    let mut required_capabilities = required_capabilities
        .iter()
        .map(|capability| CapabilityId((*capability).to_string()))
        .collect::<Vec<_>>();
    required_capabilities.sort();
    FrontendContractFixtureOut {
        fixture_id: fixture_id.to_string(),
        schema_version: FRONTEND_SCHEMA_V1,
        required_capabilities,
        initial_snapshot,
        events,
        expected_final_cursor,
        expected_view_sha256: canonical_view_sha256(&view),
    }
}

fn envelopes(fixture_id: &str, kinds: Vec<RuntimeEventKindExt>) -> Vec<RuntimeEventEnvelope> {
    kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            let sequence = index as u64 + 1;
            RuntimeEventEnvelope {
                schema_version: FRONTEND_SCHEMA_V1,
                owner: RuntimeOwner {
                    workspace_id: "workspace_contract_v1".to_string(),
                    project_id: "project_viden".to_string(),
                    lane_id: None,
                    session_id: Some(format!("session_{fixture_id}")),
                    task_id: None,
                    turn_id: Some(format!("turn_{fixture_id}")),
                },
                cursor: EventCursor {
                    stream_id: format!("fixture:{fixture_id}"),
                    sequence,
                },
                event: RuntimeWireEvent::Known(RuntimeEvent::with_timestamp(
                    sequence,
                    Some(1_700_000_000 + sequence),
                    to_runtime_event_kind(kind),
                )),
            }
        })
        .collect()
}

enum RuntimeEventKindExt {
    Assistant(&'static str, &'static str),
    ToolStarted(&'static str, &'static str, &'static str),
    ToolFinished(&'static str, &'static str, bool, Option<EvidenceView>),
    ApprovalRequested(&'static str, &'static str, bool),
    ApprovalResolved(&'static str, bool),
    InputQueued(&'static str, &'static str),
    InputDequeued(&'static str),
    Dag(&'static str, AgentDagStatus),
    Task(
        &'static str,
        AgentRole,
        AgentTaskStatus,
        &'static str,
        Option<&'static str>,
    ),
    Lane(
        &'static str,
        &'static str,
        AgentRole,
        AgentRoute,
        LaneStatus,
        ExecutionTarget,
    ),
    Evidence(EvidenceView),
    MergeGate(
        &'static str,
        &'static str,
        MergeGateStatus,
        Vec<&'static str>,
    ),
    ContextPressure,
    CostUnknown,
    TokenCostUnknown,
    CommandRejected(&'static str, &'static str),
    Error(&'static str, bool),
}

fn to_runtime_event_kind(kind: RuntimeEventKindExt) -> viden_types::RuntimeEventKind {
    match kind {
        RuntimeEventKindExt::Assistant(message_id, content) => {
            viden_types::RuntimeEventKind::AssistantDelta {
                message_id: message_id.to_string(),
                task_id: None,
                content: content.to_string(),
            }
        }
        RuntimeEventKindExt::ToolStarted(tool_call_id, name, input_preview) => {
            viden_types::RuntimeEventKind::ToolCallStarted {
                tool_call_id: tool_call_id.to_string(),
                name: name.to_string(),
                input_preview: input_preview.to_string(),
            }
        }
        RuntimeEventKindExt::ToolFinished(tool_call_id, name, success, evidence) => {
            viden_types::RuntimeEventKind::ToolCallFinished {
                tool_call_id: tool_call_id.to_string(),
                name: name.to_string(),
                success,
                exit_code: Some(if success { 0 } else { 1 }),
                evidence,
            }
        }
        RuntimeEventKindExt::ApprovalRequested(id, title, is_mutating) => {
            viden_types::RuntimeEventKind::ApprovalRequested {
                approval: approval(id, title, is_mutating),
            }
        }
        RuntimeEventKindExt::ApprovalResolved(request_id, allow) => {
            viden_types::RuntimeEventKind::ApprovalResolved {
                request_id: request_id.to_string(),
                decision: if allow {
                    ApprovalDecision::Allow {
                        scope: ApprovalScope::Once,
                    }
                } else {
                    ApprovalDecision::Deny
                },
                owner: RuntimeOwner::default(),
                audit_id: format!("audit_{request_id}"),
            }
        }
        RuntimeEventKindExt::InputQueued(id, content_preview) => {
            viden_types::RuntimeEventKind::InputQueued {
                input: QueuedInputView {
                    id: id.to_string(),
                    content_preview: content_preview.to_string(),
                    created_at: Some(1_700_000_010),
                },
            }
        }
        RuntimeEventKindExt::InputDequeued(input_id) => {
            viden_types::RuntimeEventKind::InputDequeued {
                input_id: input_id.to_string(),
            }
        }
        RuntimeEventKindExt::Dag(dag_id, status) => {
            viden_types::RuntimeEventKind::AgentDagUpdated {
                dag: AgentDagRecord {
                    dag_id: dag_id.to_string(),
                    goal: "freeze frontend contract v1".to_string(),
                    status,
                    tasks: vec![AgentDagTaskSpec {
                        task_id: "task_blocked".to_string(),
                        role: AgentRole::Coder,
                        title: "resolve blocker".to_string(),
                        objective: "recover dependency".to_string(),
                        dependencies: vec!["task_dependency".to_string()],
                        workspace: Some(".worktrees/v3-core-runtime".to_string()),
                        file_scope: vec!["crates/core".to_string()],
                        context_bundle_id: Some("ctx_bundle_contract".to_string()),
                        required_evidence: vec!["test_result".to_string()],
                        permission_policy: "ask_before_mutation".to_string(),
                    }],
                    created_at: Some(1_700_000_000),
                    updated_at: Some(1_700_000_030),
                },
            }
        }
        RuntimeEventKindExt::Task(id, role, status, activity, next_action) => {
            viden_types::RuntimeEventKind::TaskUpdated {
                task: task(id, role, status, activity, next_action),
            }
        }
        RuntimeEventKindExt::Lane(id, task_id, role, route, status, target) => {
            viden_types::RuntimeEventKind::LaneUpdated {
                lane: AgentLaneRecord {
                    id: id.to_string(),
                    task_id: Some(task_id.to_string()),
                    role,
                    route,
                    gate_strength: match route {
                        AgentRoute::BuiltIn => GateStrength::Full,
                        AgentRoute::Acp => GateStrength::Cooperative,
                        AgentRoute::Terminal | AgentRoute::Tmux => GateStrength::Containment,
                    },
                    mutation_policy: MutationPolicy::ProposeOnly,
                    worktree: Some(format!(".worktrees/{id}")),
                    branch: Some(format!("codex/{id}")),
                    target,
                    data_egress: viden_types::DataEgressPolicy::AllowListed {
                        domains: vec!["docs.example.test".to_string()],
                    },
                    status,
                    budget: LaneBudget {
                        token_limit: Some(16_000),
                        cost_limit_micro_usd: Some(500_000),
                        wall_time_limit_secs: Some(1_800),
                    },
                    active_session_ids: vec![format!("session_{id}")],
                    summary: format!("{id} active"),
                    evidence: vec![format!("evidence_{id}")],
                },
            }
        }
        RuntimeEventKindExt::Evidence(evidence) => {
            viden_types::RuntimeEventKind::EvidenceRecorded { evidence }
        }
        RuntimeEventKindExt::MergeGate(gate_id, task_id, status, evidence_ids) => {
            viden_types::RuntimeEventKind::MergeGateUpdated {
                gate: MergeGateRecord {
                    gate_id: gate_id.to_string(),
                    task_id: task_id.to_string(),
                    status,
                    required_evidence: vec![
                        "patch".to_string(),
                        "test_result".to_string(),
                        "review".to_string(),
                    ],
                    evidence_ids: evidence_ids.into_iter().map(str::to_string).collect(),
                    decision: Some("core facts satisfied gate".to_string()),
                    updated_at: Some(1_700_000_050),
                },
            }
        }
        RuntimeEventKindExt::ContextPressure => viden_types::RuntimeEventKind::ContextUpdated {
            context: ContextBundleRecord {
                bundle_id: "ctx_bundle_pressure".to_string(),
                task_id: "task_context".to_string(),
                policy: "pressure-aware".to_string(),
                sources: vec![ContextSourceRecord {
                    name: "contract-spec".to_string(),
                    kind: "doc".to_string(),
                    priority: 1,
                    estimated_tokens: 23_000,
                    summary: "frontend contract spec".to_string(),
                    include_reason: "required by contract freeze".to_string(),
                    handle_id: Some("ctxh_contract_spec".to_string()),
                    item_id: Some("ctxi_contract_spec".to_string()),
                    view_id: Some("ctxv_contract_spec".to_string()),
                    content_sha256: Some("a".repeat(64)),
                    view_sha256: Some("b".repeat(64)),
                    quality_id: Some("ctxq_contract_spec".to_string()),
                }],
                omitted_sources: vec![ContextOmittedSourceRecord {
                    name: "large-history".to_string(),
                    kind: "transcript".to_string(),
                    estimated_tokens: 9_000,
                    reason: "hard_token_limit".to_string(),
                }],
                estimated_tokens: 92_000,
                largest_sources: vec!["contract-spec".to_string()],
                compaction_notes: vec!["omitted large-history".to_string()],
                soft_token_budget: 80_000,
                hard_token_limit: 100_000,
            },
        },
        RuntimeEventKindExt::CostUnknown => viden_types::RuntimeEventKind::CostUsageRecorded {
            cost: CostUsageRecord {
                usage_id: "usage_unknown_actual".to_string(),
                provider_id: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                scopes: vec![CostScope::AgentTask("task_context".to_string())],
                tokens: TokenUsage {
                    input_tokens: Some(100),
                    output_tokens: Some(25),
                    cached_input_tokens: Some(10),
                    retrieval_tokens: Some(5),
                    total_tokens: Some(125),
                },
                estimate: None,
                actual_cost: None,
                attempt_index: 1,
                outcome: CostUsageOutcome::Success,
                recorded_at: Some(1_700_000_060),
            },
        },
        RuntimeEventKindExt::TokenCostUnknown => viden_types::RuntimeEventKind::TokenCostUpdated {
            cost: TokenCostView {
                input_tokens: 100,
                output_tokens: 25,
                total_tokens: 125,
                cost_micro_usd: None,
            },
        },
        RuntimeEventKindExt::CommandRejected(command_id, reason) => {
            viden_types::RuntimeEventKind::CommandRejected {
                command_id: command_id.to_string(),
                reason: reason.to_string(),
            }
        }
        RuntimeEventKindExt::Error(message, recoverable) => viden_types::RuntimeEventKind::Error {
            error: RuntimeErrorView {
                message: message.to_string(),
                recoverable,
                hint: Some("retry after resolving contract request".to_string()),
            },
        },
    }
}

fn task(
    id: &str,
    role: AgentRole,
    status: AgentTaskStatus,
    activity: &str,
    next_action: Option<&str>,
) -> AgentTaskRecord {
    AgentTaskRecord {
        id: id.to_string(),
        parent_id: None,
        role,
        kind: AgentTaskKind::Agent,
        route: AgentRoute::Terminal,
        title: format!("{id} title"),
        status,
        activity: activity.to_string(),
        summary: format!("{id} summary"),
        progress: if status == AgentTaskStatus::Done {
            100
        } else {
            40
        },
        started_at: Some(1_700_000_000),
        updated_at: Some(1_700_000_040),
        workspace: Some(".worktrees/v3-core-runtime".to_string()),
        evidence: vec![format!("evidence_{id}")],
        permissions: vec!["ask".to_string()],
        decision: None,
        result: None,
        resume_handle: Some(format!("resume_{id}")),
        pid: None,
        next_action: next_action.map(|label| AgentNextAction {
            label: label.to_string(),
            command: Some("retry".to_string()),
            reason: Some("blocked dependency recovered".to_string()),
        }),
    }
}

fn approval(id: &str, title: &str, is_mutating: bool) -> ApprovalRequestView {
    ApprovalRequestView {
        id: id.to_string(),
        tool_name: "shell".to_string(),
        title: title.to_string(),
        message: "Core requests scoped permission".to_string(),
        input_preview: "cargo test".to_string(),
        is_mutating,
        reason: Some("contract fixture coverage".to_string()),
        owner: RuntimeOwner {
            workspace_id: "workspace_contract_v1".to_string(),
            project_id: "project_viden".to_string(),
            lane_id: Some("lane_core".to_string()),
            session_id: Some("session_contract".to_string()),
            task_id: Some("task_contract".to_string()),
            turn_id: Some("turn_contract".to_string()),
        },
        risk: ApprovalRisk::High,
        target: ApprovalTarget {
            kind: "repo_path".to_string(),
            display: "crates/core".to_string(),
            canonical_ref: Some("repo://crates/core".to_string()),
        },
        allowed_scopes: vec![ApprovalScope::Once],
        policy_reason_key: "approval.contract_fixture".to_string(),
        policy_reason_args: BTreeMap::new(),
        expires_at: 1_700_003_600,
        default_action: ApprovalDefaultAction::Deny,
        audit_id: format!("audit_{id}"),
    }
}

fn evidence(id: &str, kind: &str, summary: &str) -> EvidenceView {
    EvidenceView {
        id: id.to_string(),
        kind: kind.to_string(),
        summary: summary.to_string(),
        path: Some(format!("artifacts/{id}.txt")),
        source: Some("core".to_string()),
        canonical: None,
        metadata: None,
        timestamp: Some(1_700_000_070),
    }
}

fn snapshot(work_mode: WorkMode) -> RuntimeSnapshot {
    RuntimeSnapshot {
        cwd: PathBuf::from("workspace/viden"),
        provider_family: "deepseek".to_string(),
        model_label: "deepseek-v4-flash".to_string(),
        work_mode,
        permission_mode: PermissionMode::Default,
        permission_level: PermissionLevel::Ask,
        config_summary: serde_json::json!({
            "ui": {
                "locales": ["en", "zh-CN"],
                "skin_mode_pairs": [
                    ["aurora", "dark"],
                    ["aurora", "light"],
                    ["ice", "dark"],
                    ["ice", "light"],
                    ["mono", "dark"],
                    ["mono", "light"],
                    ["amber", "dark"],
                    ["phosphor", "dark"]
                ],
                "density": ["compact", "regular", "comfy"],
                "motion": ["system", "reduced", "full"],
                "effective": UiPreferences {
                    locale: viden_types::LocaleId::ZhCn,
                    skin: UiSkin::Aurora,
                    mode: UiColorMode::Dark,
                    density: UiDensity::Regular,
                    motion: UiMotion::Reduced,
                }
            },
            "design_entry": {
                "hierarchy": [
                    "docs/viden-design/Viden/index.html",
                    "client design index",
                    "component library",
                    "TUI unified prototype or GUI D1 desktop cockpit"
                ],
                "tui": "TUI index -> component library -> unified prototype",
                "gui": "GUI index -> component library -> D1 desktop cockpit",
                "d11": "onboarding subordinate"
            }
        })
        .to_string(),
        loaded_config_files: vec![PathBuf::from("config/viden.toml")],
        startup_overrides: vec!["--provider=deepseek".to_string()],
        ui_preferences: ResolvedUiPreferences {
            locale: viden_types::LocaleId::ZhCn,
            skin: UiSkin::Aurora,
            mode: UiColorMode::Dark,
            density: UiDensity::Regular,
            motion: UiMotion::Reduced,
            diagnostics: Vec::new(),
        },
    }
}

fn typed_lanes_fixture() -> Vec<AgentLaneRecord> {
    parse_legacy_lanes_tsv(include_str!(
        "../../types/tests/fixtures/frontend-contract-v1/legacy-lanes.tsv"
    ))
}
