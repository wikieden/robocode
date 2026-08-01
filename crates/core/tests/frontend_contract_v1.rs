use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use viden_core::{
    CORE_CLIENT_CAPABILITIES, CORE_CLIENT_VERSION, CORE_EXTENSION_CAPABILITIES, CheckRunView,
    RuntimeServiceHealthView, WorkspaceChangeView, WorkspaceSourceView, frontend_capabilities,
    local_core_handshake,
};
use viden_types::{
    AgentAdapterSource, AgentAdapterView, AgentAuthState, AgentAvailability, AgentDagRecord,
    AgentDagStatus, AgentDagTaskSpec, AgentLaneRecord, AgentNextAction, AgentRole, AgentRoute,
    AgentSessionStatus, AgentSessionView, AgentStartability, AgentTaskKind, AgentTaskRecord,
    AgentTaskStatus, ApprovalDecision, ApprovalDefaultAction, ApprovalRequestView,
    ApprovalResponse, ApprovalRisk, ApprovalScope, ApprovalTarget, CapabilityId,
    ContextBundleRecord, ContextOmittedSourceRecord, ContextSourceRecord, CostScope,
    CostUsageOutcome, CostUsageRecord, EventCursor, EvidenceView, ExecutionTarget,
    FRONTEND_SCHEMA_V1, GateStrength, LaneBudget, LaneRuntimeOwnerBinding, LaneStatus,
    MergeGateDecision, MergeGateDecisionOutcome, MergeGatePolicySnapshot, MergeGateRecord,
    MergeGateStatus, MergeGateType, MutationPolicy, PermissionLevel, PermissionMode,
    ProjectConfigState, ProjectProbe, QueuedInputView, RecentProjectSummary, RecentSessionSummary,
    ResolvedUiPreferences, RuntimeErrorView, RuntimeEvent, RuntimeEventEnvelope, RuntimeEventKind,
    RuntimeOwner, RuntimeSnapshot, RuntimeViewState, RuntimeWireEvent, SchemaVersion,
    StarterLanePreview, StarterLanePreviewInvalidationReason, StarterLaneReceipt, TokenCostView,
    TokenUsage, UiColorMode, UiDensity, UiMotion, UiPreferences, UiSkin, WorkMode,
    WorkspaceEligibility,
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
    assert!(
        CORE_EXTENSION_CAPABILITIES
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert_eq!(
        advertised.len(),
        expected.len() + CORE_EXTENSION_CAPABILITIES.len()
    );
    for capability in expected {
        assert!(advertised.contains(&CapabilityId(capability.to_string())));
    }
    assert!(advertised.contains(&CapabilityId("runtime.lane_lifecycle".to_string())));
    assert!(advertised.contains(&CapabilityId("runtime.project_onboarding".to_string())));
    assert!(advertised.contains(&CapabilityId("runtime.credential_handles".to_string())));
    let extension_manifest = include_str!("../frontend-contract-extensions.toml");
    assert!(extension_manifest.contains("base_component_version = \"0.3.0\""));
    assert!(extension_manifest.contains("candidate_component_version = \"0.3.5\""));
    assert!(extension_manifest.contains("compatibility = \"additive_capability_gated\""));
    assert!(extension_manifest.contains("[runtime_trust_loop]\ncommand_count = 7"));
    assert_eq!(CORE_CLIENT_VERSION, "0.3.5");
    assert_eq!(local_core_handshake().core_version, "0.3.5");
}

#[test]
fn frontend_host_capabilities_are_schema_one_core_0_3_5_and_additive() {
    let frozen_base = [
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
    let extensions = [
        "core.workspace_host",
        "runtime.agent_adapters",
        "runtime.agent_conversation",
        "runtime.agent_permission_bridge",
        "runtime.agent_session_input",
        "runtime.agent_sessions",
        "runtime.cockpit_context_v1",
        "runtime.credential_handles",
        "runtime.credential_staging",
        "runtime.lane_lifecycle",
        "runtime.lane_owner_projection",
        "runtime.project_onboarding",
        "runtime.recent_work",
        "runtime.starter_lane_preview",
        "runtime.trust_loop",
        "runtime.workspace_eligibility",
        "ui.preference_persistence",
    ];

    assert_eq!(FRONTEND_SCHEMA_V1, SchemaVersion(1));
    assert_eq!(CORE_CLIENT_VERSION, "0.3.5");
    assert_eq!(CORE_CLIENT_CAPABILITIES, frozen_base);
    assert_eq!(CORE_EXTENSION_CAPABILITIES, extensions);
    assert!(
        CORE_EXTENSION_CAPABILITIES
            .windows(2)
            .all(|pair| pair[0] < pair[1]),
        "extension capabilities must be sorted and unique"
    );
    let advertised = frontend_capabilities();
    assert_eq!(advertised.len(), frozen_base.len() + extensions.len());
    assert_eq!(
        local_core_handshake().active_schema_version,
        SchemaVersion(1)
    );

    let base_only = viden_types::CoreHandshake {
        core_version: "0.3.0".to_string(),
        supported_schema_versions: vec![FRONTEND_SCHEMA_V1],
        active_schema_version: FRONTEND_SCHEMA_V1,
        capabilities: frozen_base
            .into_iter()
            .map(|capability| CapabilityId(capability.to_string()))
            .collect(),
    };
    viden_core::validate_handshake(&base_only)
        .expect("missing optional extensions must not block a frozen-base client");

    let extension_manifest = include_str!("../frontend-contract-extensions.toml");
    assert!(extension_manifest.contains("candidate_component_version = \"0.3.5\""));
    assert!(extension_manifest.contains("schema_version = 1"));
    assert!(extension_manifest.contains("runtime.cockpit_context_v1"));
    assert!(!extension_manifest.contains("runtime.workspace_facts"));
    assert!(extension_manifest.contains("runtime.lane_owner_projection"));
    assert!(extension_manifest.contains(
        "extension_fixture_sha256 = \"96dd5fde9f1241eb50f9d8978cf478d0ac5d3327448dc6ccde9d0e5018ce1580\""
    ));
    assert!(extension_manifest.contains(
        "interaction_fixture_sha256 = \"78e8993fa455149d05744d15e70bc4c2072f3d4726bf76026203826f500204a5\""
    ));
}

#[test]
fn frontend_contract_v1_exports_cockpit_fact_types() {
    assert!(std::any::type_name::<WorkspaceSourceView>().contains("WorkspaceSourceView"));
    assert!(std::any::type_name::<RuntimeServiceHealthView>().contains("RuntimeServiceHealthView"));
    assert!(std::any::type_name::<WorkspaceChangeView>().contains("WorkspaceChangeView"));
    assert!(std::any::type_name::<CheckRunView>().contains("CheckRunView"));
}

#[test]
fn frontend_host_capabilities_fixture_replays_known_facts_and_tolerates_future_events() {
    let name = "frontend-host-services.json";
    let root = fixture_root();
    let fixture_bytes = fs::read(root.join(name)).expect("read extension fixture bytes");
    assert_eq!(
        format!("{:x}", Sha256::digest(&fixture_bytes)),
        "96dd5fde9f1241eb50f9d8978cf478d0ac5d3327448dc6ccde9d0e5018ce1580"
    );
    let fixture = read_fixture(&root, name);
    assert_fixture_identity(name, &fixture);
    assert_capabilities_are_sorted_unique_and_advertised(name, &fixture);
    assert_cursors_are_contiguous(name, &fixture);

    let known_types = fixture
        .events
        .iter()
        .filter_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(event) => Some(match &event.kind {
                RuntimeEventKind::UiPreferencesUpdated { .. } => "ui_preferences_updated",
                RuntimeEventKind::RecentWorkLoaded { .. } => "recent_work_loaded",
                RuntimeEventKind::StarterLanePreviewed { .. } => "starter_lane_previewed",
                RuntimeEventKind::StarterLaneCreated { .. } => "starter_lane_created",
                RuntimeEventKind::StarterLanePreviewInvalidated { .. } => {
                    "starter_lane_preview_invalidated"
                }
                RuntimeEventKind::LaneRuntimeOwnerBound { .. } => "lane_runtime_owner_bound",
                other => panic!("extension fixture contains transient placeholder {other:?}"),
            }),
            RuntimeWireEvent::Unknown { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        known_types,
        [
            "ui_preferences_updated",
            "recent_work_loaded",
            "starter_lane_previewed",
            "starter_lane_created",
            "starter_lane_preview_invalidated",
            "lane_runtime_owner_bound",
        ]
    );
    assert!(fixture.events.iter().any(|envelope| matches!(
        envelope.event,
        RuntimeWireEvent::Unknown { ref event_type, .. }
            if event_type == "future_frontend_host_fact"
    )));

    let (first_view, first_cursor, first_digest) = replay_fixture(&fixture);
    let (second_view, second_cursor, second_digest) = replay_fixture(&fixture);
    assert_eq!(first_view, second_view);
    assert_eq!(first_cursor, second_cursor);
    assert_eq!(first_digest, second_digest);
    assert_eq!(first_cursor, fixture.expected_final_cursor);
    assert_eq!(first_digest, fixture.expected_view_sha256);
    assert_eq!(
        first_view.ui_preferences.locale,
        viden_types::LocaleId::ZhCn
    );
    assert_eq!(first_view.recent_projects.len(), 1);
    assert_eq!(first_view.recent_sessions.len(), 1);
    assert!(first_view.starter_lane_previews.is_empty());
    assert_eq!(first_view.starter_lane_receipts.len(), 1);
    assert_eq!(first_view.lane_runtime_owners.len(), 1);
}

#[test]
fn interaction_closed_loop_fixture_replays_identically_after_a_gap() {
    let name = "interaction-closed-loop.json";
    let root = fixture_root();
    let fixture_bytes = fs::read(root.join(name)).expect("read interaction fixture bytes");
    assert_eq!(
        format!("{:x}", Sha256::digest(&fixture_bytes)),
        "a6f1c436a15f7c77a5410c3563d8c3f67c5a5a3864692de61db61623f93ed891"
    );
    let fixture = read_fixture(&root, name);
    assert_fixture_identity(name, &fixture);
    assert_capabilities_are_sorted_unique_and_advertised(name, &fixture);
    assert_cursors_are_contiguous(name, &fixture);
    let event_types = fixture
        .events
        .iter()
        .map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(event) => match &event.kind {
                RuntimeEventKind::ProjectProbed { .. } => "project_open_no_lane",
                RuntimeEventKind::WorkspaceEligibilityUpdated { .. } => "workspace_eligible",
                RuntimeEventKind::StarterLanePreviewed { .. } => "starter_lane_previewed",
                RuntimeEventKind::StarterLaneCreated { .. } => "starter_lane_created",
                RuntimeEventKind::AgentAdaptersLoaded { .. } => "agent_adapters_loaded",
                RuntimeEventKind::AgentSessionStarted { .. } => "agent_session_started",
                RuntimeEventKind::AgentSessionCompleted { .. } => "agent_session_completed",
                RuntimeEventKind::AgentSessionInputAccepted { .. } => {
                    "agent_session_input_accepted"
                }
                RuntimeEventKind::ToolCallStarted { .. } => "tool_call_started",
                RuntimeEventKind::ToolCallFinished { .. } => "tool_call_finished",
                RuntimeEventKind::ApprovalRequested { .. } => "approval_requested",
                RuntimeEventKind::AgentSessionUpdated { .. } => "agent_session_updated",
                RuntimeEventKind::ApprovalResolved { .. } => "approval_resolved",
                RuntimeEventKind::EvidenceRecorded { .. } => "evidence_recorded",
                RuntimeEventKind::MergeGateUpdated { .. } => "merge_gate_updated",
                RuntimeEventKind::LaneConflictDetected { .. } => "apply_conflict",
                RuntimeEventKind::LaneRecoveryRequired { .. } => "recovery_required",
                ref other => panic!("unexpected interaction event {other:?}"),
            },
            RuntimeWireEvent::Unknown { event_type, .. } => event_type,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        event_types,
        [
            "project_open_no_lane",
            "workspace_eligible",
            "starter_lane_previewed",
            "starter_lane_created",
            "agent_adapters_loaded",
            "agent_session_started",
            "agent_session_completed",
            "agent_session_started",
            "tool_call_started",
            "tool_call_finished",
            "approval_requested",
            "agent_session_updated",
            "approval_resolved",
            "agent_session_updated",
            "evidence_recorded",
            "merge_gate_updated",
            "apply_conflict",
            "recovery_required",
            "agent_session_completed",
            "agent_session_input_accepted",
            "agent_session_started",
            "agent_session_completed",
        ]
    );

    let (full_view, full_cursor, full_digest) = replay_fixture(&fixture);
    let (reconnected_view, reconnected_cursor, reconnected_digest) =
        replay_fixture_after_gap(&fixture, 16);
    assert_eq!(full_view, reconnected_view);
    assert_eq!(full_cursor, reconnected_cursor);
    assert_eq!(full_digest, reconnected_digest);
    assert_eq!(full_digest, fixture.expected_view_sha256);

    assert_eq!(
        full_view.project_probe.as_ref().unwrap().config_state,
        ProjectConfigState::Missing
    );
    assert_eq!(full_view.starter_lane_receipts.len(), 1);
    assert!(full_view.starter_lane_previews.is_empty());
    assert!(
        full_view
            .workspace_eligibility
            .as_ref()
            .is_some_and(|eligibility| eligibility.can_create_lane)
    );
    assert_eq!(full_view.agent_adapters.len(), 4);
    assert!(full_view.agent_adapters.iter().any(|adapter| {
        adapter.route == AgentRoute::BuiltIn && adapter.availability == AgentAvailability::Available
    }));
    assert!(
        full_view
            .agent_adapters
            .iter()
            .any(|adapter| { adapter.route == AgentRoute::Acp && adapter.agent_id == "codex-acp" })
    );
    assert_eq!(full_view.agent_sessions.len(), 1);
    assert_eq!(full_view.agent_session_inputs.len(), 1);
    assert_eq!(
        full_view.agent_session_inputs[0].input_id,
        "agent-input-loop-follow-up"
    );
    assert!(
        full_view
            .agent_sessions
            .iter()
            .all(|session| session.status == AgentSessionStatus::Completed)
    );
    assert!(full_view.pending_approvals.is_empty());
    assert!(
        full_view
            .latest_evidence
            .iter()
            .any(|item| item.id == "evidence-loop-test")
    );
    assert!(full_view.merge_gates.iter().any(|gate| {
        gate.gate_id == "gate-loop-apply" && gate.status == MergeGateStatus::Accepted
    }));
    assert_eq!(full_view.lane_conflicts.len(), 1);
    assert_eq!(full_view.lane_recoveries.len(), 1);

    let release_manifest = include_str!("../release-manifest.toml");
    assert!(release_manifest.contains("component_version = \"0.3.5\""));
    assert!(release_manifest.contains("runtime.cockpit_context_v1"));
    assert!(!release_manifest.contains("runtime.workspace_facts"));
    assert!(release_manifest.contains(
        "contract_implementation_checkpoint = \"17fa2071398d5eaf30045257163d57d22d99177b\""
    ));
    assert!(release_manifest.contains(
        "payload_sha256 = \"78e8993fa455149d05744d15e70bc4c2072f3d4726bf76026203826f500204a5\""
    ));
    assert!(release_manifest.contains(
        "view_sha256 = \"46db05abaaae36cf37cb7ffa0493a4ef8c158a2d5b4ffeef08d01dbf8e284ed0\""
    ));
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

#[test]
#[ignore = "manual extension fixture refresh; normal tests validate committed JSON only"]
fn refresh_frontend_host_services_extension_fixture() {
    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let fixture = frontend_host_services_fixture();
    fs::write(
        root.join("frontend-host-services.json"),
        serde_json::to_string_pretty(&fixture).unwrap() + "\n",
    )
    .unwrap();
}

#[test]
#[ignore = "manual interaction fixture refresh; normal tests validate committed JSON only"]
fn refresh_interaction_closed_loop_extension_fixture() {
    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let fixture = interaction_closed_loop_fixture();
    fs::write(
        root.join("interaction-closed-loop.json"),
        serde_json::to_string_pretty(&fixture).unwrap() + "\n",
    )
    .unwrap();
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

fn replay_fixture_after_gap(
    fixture: &FrontendContractFixture,
    delivered_before_gap: usize,
) -> (RuntimeViewState, EventCursor, String) {
    assert!(delivered_before_gap < fixture.events.len());
    let mut view = RuntimeViewState::new(fixture.initial_snapshot.clone());
    let mut cursor = EventCursor {
        stream_id: format!("fixture:{}", fixture.fixture_id),
        sequence: 0,
    };
    for envelope in fixture.events.iter().take(delivered_before_gap) {
        if let RuntimeWireEvent::Known(event) = &envelope.event {
            view.apply_event(event);
        }
        cursor = envelope.cursor.clone();
    }
    let gap = &fixture.events[delivered_before_gap + 1];
    assert!(gap.cursor.sequence > cursor.sequence + 1);

    // Reconnect asks Core for the missing contiguous batch and reduces those
    // ordered facts before accepting the already observed post-gap event.
    for envelope in fixture.events.iter().skip(delivered_before_gap) {
        assert_eq!(envelope.cursor.sequence, cursor.sequence + 1);
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

fn frontend_host_services_fixture() -> FrontendContractFixtureOut {
    let fixture_id = "frontend-host-services";
    let owner = RuntimeOwner {
        workspace_id: "workspace-host-fixture".to_string(),
        project_id: "project-host-fixture".to_string(),
        lane_id: Some("lane-host-fixture".to_string()),
        session_id: Some("session-host-fixture".to_string()),
        task_id: Some("task_host_fixture".to_string()),
        turn_id: Some("turn-host-fixture".to_string()),
    };
    let lane = AgentLaneRecord {
        id: "lane-host-fixture".to_string(),
        task_id: Some("task_host_fixture".to_string()),
        role: AgentRole::Coder,
        route: AgentRoute::BuiltIn,
        gate_strength: GateStrength::Full,
        mutation_policy: MutationPolicy::ProposeOnly,
        worktree: Some("workspace/.worktrees/lane-host-fixture".to_string()),
        branch: Some("codex/lane-host-fixture".to_string()),
        target: ExecutionTarget::Local,
        data_egress: viden_types::DataEgressPolicy::Deny,
        status: LaneStatus::Running,
        budget: LaneBudget::default(),
        active_session_ids: vec!["session-host-fixture".to_string()],
        summary: "reviewed starter Lane".to_string(),
        evidence: Vec::new(),
    };
    let preview = StarterLanePreview {
        preview_id: "preview-host-fixture".to_string(),
        content_sha256: "ab".repeat(32),
        owner: owner.clone(),
        lane: lane.clone(),
        branch: "codex/lane-host-fixture".to_string(),
        worktree_path: "workspace/.worktrees/lane-host-fixture".to_string(),
        base_revision: "cd".repeat(20),
        diagnostics: Vec::new(),
    };
    let kinds = vec![
        RuntimeEventKind::UiPreferencesUpdated {
            resolved: ResolvedUiPreferences {
                locale: viden_types::LocaleId::ZhCn,
                skin: UiSkin::Ice,
                mode: UiColorMode::Dark,
                density: UiDensity::Compact,
                motion: UiMotion::Reduced,
                diagnostics: Vec::new(),
            },
            persisted: Some(UiPreferences {
                locale: viden_types::LocaleId::ZhCn,
                skin: UiSkin::Ice,
                mode: UiColorMode::Dark,
                density: UiDensity::Compact,
                motion: UiMotion::Reduced,
            }),
            diagnostics: Vec::new(),
        },
        RuntimeEventKind::RecentWorkLoaded {
            projects: vec![RecentProjectSummary {
                canonical_root: "workspace/project".to_string(),
                display_name: "project".to_string(),
                last_updated_at: 1_700_000_020,
                latest_session_id: Some("session-host-fixture".to_string()),
            }],
            sessions: vec![RecentSessionSummary {
                canonical_root: "workspace/project".to_string(),
                session_id: "session-host-fixture".to_string(),
                created_at: 1_700_000_010,
                last_updated_at: 1_700_000_020,
                message_count: 2,
                tool_call_count: 1,
                command_count: 1,
            }],
            diagnostics: Vec::new(),
        },
        RuntimeEventKind::StarterLanePreviewed {
            preview: preview.clone(),
        },
        RuntimeEventKind::StarterLaneCreated {
            receipt: StarterLaneReceipt {
                preview_id: preview.preview_id.clone(),
                content_sha256: preview.content_sha256.clone(),
                lane,
                branch: preview.branch.clone(),
                worktree_path: preview.worktree_path.clone(),
                base_revision: preview.base_revision.clone(),
                owner: owner.clone(),
            },
        },
        RuntimeEventKind::StarterLanePreviewInvalidated {
            owner: owner.clone(),
            preview_id: preview.preview_id,
            reason: StarterLanePreviewInvalidationReason::BaseRevisionChanged,
        },
        RuntimeEventKind::LaneRuntimeOwnerBound {
            binding: LaneRuntimeOwnerBinding {
                lane_id: "lane-host-fixture".to_string(),
                owner: owner.clone(),
            },
        },
    ];
    let mut events = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            let sequence = index as u64 + 1;
            RuntimeEventEnvelope {
                schema_version: FRONTEND_SCHEMA_V1,
                owner: owner.clone(),
                cursor: EventCursor {
                    stream_id: format!("fixture:{fixture_id}"),
                    sequence,
                },
                event: RuntimeWireEvent::Known(RuntimeEvent::with_timestamp(
                    sequence,
                    Some(1_700_000_000 + sequence),
                    kind,
                )),
            }
        })
        .collect::<Vec<_>>();
    events.push(RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner,
        cursor: EventCursor {
            stream_id: format!("fixture:{fixture_id}"),
            sequence: 7,
        },
        event: RuntimeWireEvent::Unknown {
            event_type: "future_frontend_host_fact".to_string(),
            payload: serde_json::json!({"optional": true}),
        },
    });
    fixture(
        fixture_id,
        &[
            "core.workspace_host",
            "runtime.credential_staging",
            "runtime.lane_owner_projection",
            "runtime.recent_work",
            "runtime.starter_lane_preview",
            "ui.preference_persistence",
        ],
        snapshot(WorkMode::Build),
        events,
    )
}

fn interaction_closed_loop_fixture() -> FrontendContractFixtureOut {
    let fixture_id = "interaction-closed-loop";
    let acp_owner = RuntimeOwner {
        workspace_id: "workspace-loop".to_string(),
        project_id: "project-loop".to_string(),
        lane_id: Some("lane-loop-coder".to_string()),
        session_id: Some("session-loop-acp".to_string()),
        task_id: Some("task_loop".to_string()),
        turn_id: Some("turn-loop-acp".to_string()),
    };
    let built_in_owner = RuntimeOwner {
        session_id: Some("session-loop-built-in".to_string()),
        turn_id: Some("turn-loop-built-in".to_string()),
        ..acp_owner.clone()
    };
    let lane = AgentLaneRecord {
        id: "lane-loop-coder".to_string(),
        task_id: Some("task_loop".to_string()),
        role: AgentRole::Coder,
        route: AgentRoute::Acp,
        gate_strength: GateStrength::Cooperative,
        mutation_policy: MutationPolicy::ProposeOnly,
        worktree: Some("workspace/.worktrees/lane-loop-coder".to_string()),
        branch: Some("codex/lane-loop-coder".to_string()),
        target: ExecutionTarget::Local,
        data_egress: viden_types::DataEgressPolicy::Deny,
        status: LaneStatus::Running,
        budget: LaneBudget {
            token_limit: Some(24_000),
            cost_limit_micro_usd: Some(750_000),
            wall_time_limit_secs: Some(2_400),
        },
        active_session_ids: vec![
            "session-loop-built-in".to_string(),
            "session-loop-acp".to_string(),
        ],
        summary: "lane.loop.running".to_string(),
        evidence: Vec::new(),
    };
    let preview = StarterLanePreview {
        preview_id: "preview-loop-coder".to_string(),
        content_sha256: "12".repeat(32),
        owner: acp_owner.clone(),
        lane: lane.clone(),
        branch: "codex/lane-loop-coder".to_string(),
        worktree_path: "workspace/.worktrees/lane-loop-coder".to_string(),
        base_revision: "34".repeat(20),
        diagnostics: Vec::new(),
    };
    let adapters = vec![
        AgentAdapterView {
            agent_id: "viden-built-in".to_string(),
            display_name: "Viden Built-in".to_string(),
            route: AgentRoute::BuiltIn,
            source: AgentAdapterSource::BuiltIn,
            availability: AgentAvailability::Available,
            auth_state: AgentAuthState::Ready,
            startability: AgentStartability::Ready,
            capabilities: vec![CapabilityId("agent.session.prompt".to_string())],
            models: vec!["workspace-default".to_string()],
            diagnostics: Vec::new(),
        },
        AgentAdapterView {
            agent_id: "claude-acp".to_string(),
            display_name: "Claude ACP".to_string(),
            route: AgentRoute::Acp,
            source: AgentAdapterSource::Registry,
            availability: AgentAvailability::Available,
            auth_state: AgentAuthState::Ready,
            startability: AgentStartability::Ready,
            capabilities: vec![CapabilityId("agent.session.prompt".to_string())],
            models: Vec::new(),
            diagnostics: Vec::new(),
        },
        AgentAdapterView {
            agent_id: "codex-acp".to_string(),
            display_name: "Codex ACP".to_string(),
            route: AgentRoute::Acp,
            source: AgentAdapterSource::Registry,
            availability: AgentAvailability::Available,
            auth_state: AgentAuthState::Ready,
            startability: AgentStartability::Ready,
            capabilities: vec![
                CapabilityId("agent.permission.request".to_string()),
                CapabilityId("agent.session.cancel".to_string()),
                CapabilityId("agent.session.prompt".to_string()),
            ],
            models: vec!["gpt-5".to_string()],
            diagnostics: Vec::new(),
        },
        AgentAdapterView {
            agent_id: "kiro-cli".to_string(),
            display_name: "Kiro CLI".to_string(),
            route: AgentRoute::Acp,
            source: AgentAdapterSource::LocalCommand,
            availability: AgentAvailability::NeedsAuth,
            auth_state: AgentAuthState::LoggedOut,
            startability: AgentStartability::AuthenticationRequired,
            capabilities: vec![CapabilityId("agent.session.prompt".to_string())],
            models: Vec::new(),
            diagnostics: vec!["agent.auth.required".to_string()],
        },
    ];
    let built_in_session = AgentSessionView {
        session_id: "session-loop-built-in".to_string(),
        lane_id: lane.id.clone(),
        agent_id: "viden-built-in".to_string(),
        model: Some("workspace-default".to_string()),
        status: AgentSessionStatus::Starting,
        owner: built_in_owner.clone(),
        task: "task.loop.preflight".to_string(),
        diagnostic: None,
        output: None,
    };
    let acp_session = AgentSessionView {
        session_id: "session-loop-acp".to_string(),
        lane_id: lane.id.clone(),
        agent_id: "codex-acp".to_string(),
        model: Some("gpt-5".to_string()),
        status: AgentSessionStatus::Starting,
        owner: acp_owner.clone(),
        task: "task.loop.implement".to_string(),
        diagnostic: None,
        output: None,
    };
    let mut approval = approval("approval-loop-tool", "approval.tool.execute", true);
    approval.owner = acp_owner.clone();
    approval.policy_reason_key = "approval.agent_tool.mutation".to_string();
    approval.policy_reason_args = BTreeMap::from([
        ("agent_id".to_string(), "codex-acp".to_string()),
        ("lane_id".to_string(), lane.id.clone()),
    ]);
    let loop_evidence = evidence("evidence-loop-test", "test_result", "evidence.test.passed");
    let gate = MergeGateRecord {
        gate_id: "gate-loop-apply".to_string(),
        task_id: "task_loop".to_string(),
        status: MergeGateStatus::Accepted,
        required_evidence: vec!["test_result".to_string()],
        evidence_ids: vec![loop_evidence.id.clone()],
        gate_type: MergeGateType::Artifact,
        owner: acp_owner.clone(),
        validator: None,
        policy_snapshot: MergeGatePolicySnapshot {
            required_evidence: vec!["test_result".to_string()],
            permission_snapshot_id: Some("permission-loop-1".to_string()),
            requires_independent_validator: false,
            captured_at: Some(1_700_100_014),
        },
        decision: Some(MergeGateDecision {
            outcome: MergeGateDecisionOutcome::Accepted,
            reason: "gate.evidence.satisfied".to_string(),
            owner: acp_owner.clone(),
            evidence_ids: vec![loop_evidence.id.clone()],
            reviewed_evidence: Vec::new(),
            review_request_id: None,
            audit_id: "audit-loop-gate".to_string(),
            decided_at: 1_700_100_014,
        }),
        conflict: None,
        applied_change_id: None,
        recovery_snapshot: None,
        audit_ids: vec!["audit-loop-gate".to_string()],
        updated_at: Some(1_700_100_014),
    };
    let events_with_owners = vec![
        (
            acp_owner.clone(),
            RuntimeEventKind::ProjectProbed {
                probe: ProjectProbe {
                    root: "workspace/project".to_string(),
                    is_git_repository: true,
                    git_root: Some("workspace/project".to_string()),
                    config_path: "workspace/project/viden.toml".to_string(),
                    config_state: ProjectConfigState::Missing,
                    project_name: Some("project".to_string()),
                    pack: None,
                    diagnostics: Vec::new(),
                },
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::WorkspaceEligibilityUpdated {
                eligibility: WorkspaceEligibility {
                    is_git_repository: true,
                    has_head: true,
                    can_create_lane: true,
                    diagnostic: None,
                },
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::StarterLanePreviewed {
                preview: preview.clone(),
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::StarterLaneCreated {
                receipt: StarterLaneReceipt {
                    preview_id: preview.preview_id.clone(),
                    content_sha256: preview.content_sha256.clone(),
                    lane,
                    branch: preview.branch.clone(),
                    worktree_path: preview.worktree_path.clone(),
                    base_revision: preview.base_revision.clone(),
                    owner: acp_owner.clone(),
                },
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::AgentAdaptersLoaded { adapters },
        ),
        (
            built_in_owner.clone(),
            RuntimeEventKind::AgentSessionStarted {
                session: built_in_session.clone(),
            },
        ),
        (
            built_in_owner,
            RuntimeEventKind::AgentSessionCompleted {
                session: AgentSessionView {
                    status: AgentSessionStatus::Completed,
                    ..built_in_session
                },
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::AgentSessionStarted {
                session: acp_session.clone(),
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::ToolCallStarted {
                tool_call_id: "tool-loop-test".to_string(),
                name: "shell".to_string(),
                input_preview: "command.test.core".to_string(),
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::ToolCallFinished {
                tool_call_id: "tool-loop-test".to_string(),
                name: "shell".to_string(),
                success: true,
                exit_code: Some(0),
                evidence: None,
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::ApprovalRequested { approval },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::AgentSessionUpdated {
                session: AgentSessionView {
                    status: AgentSessionStatus::WaitingApproval,
                    ..acp_session.clone()
                },
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::ApprovalResolved {
                request_id: "approval-loop-tool".to_string(),
                decision: ApprovalDecision::Allow {
                    scope: ApprovalScope::Once,
                },
                owner: acp_owner.clone(),
                audit_id: "audit-approval-loop-tool".to_string(),
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::AgentSessionUpdated {
                session: AgentSessionView {
                    status: AgentSessionStatus::Running,
                    ..acp_session.clone()
                },
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::EvidenceRecorded {
                evidence: loop_evidence,
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::MergeGateUpdated { gate },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::LaneConflictDetected {
                lane_id: "lane-loop-coder".to_string(),
                summary: "conflict.apply.non_fast_forward".to_string(),
                paths: vec!["src/lib.rs".to_string()],
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::LaneRecoveryRequired {
                lane_id: "lane-loop-coder".to_string(),
                reason: "recovery.apply_conflict".to_string(),
                next_action: "action.revalidate_merge_conflict".to_string(),
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::AgentSessionCompleted {
                session: AgentSessionView {
                    status: AgentSessionStatus::Completed,
                    ..acp_session.clone()
                },
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::AgentSessionInputAccepted {
                session_id: acp_session.session_id.clone(),
                input_id: "agent-input-loop-follow-up".to_string(),
            },
        ),
        (
            acp_owner.clone(),
            RuntimeEventKind::AgentSessionStarted {
                session: AgentSessionView {
                    status: AgentSessionStatus::Starting,
                    task: "task.loop.follow_up".to_string(),
                    ..acp_session.clone()
                },
            },
        ),
        (
            acp_owner,
            RuntimeEventKind::AgentSessionCompleted {
                session: AgentSessionView {
                    status: AgentSessionStatus::Completed,
                    task: "task.loop.follow_up".to_string(),
                    ..acp_session
                },
            },
        ),
    ];
    let events = events_with_owners
        .into_iter()
        .enumerate()
        .map(|(index, (owner, kind))| {
            let sequence = index as u64 + 1;
            RuntimeEventEnvelope {
                schema_version: FRONTEND_SCHEMA_V1,
                owner,
                cursor: EventCursor {
                    stream_id: format!("fixture:{fixture_id}"),
                    sequence,
                },
                event: RuntimeWireEvent::Known(RuntimeEvent::with_timestamp(
                    sequence,
                    Some(1_700_100_000 + sequence),
                    kind,
                )),
            }
        })
        .collect();

    fixture(
        fixture_id,
        &[
            "core.workspace_host",
            "runtime.agent_adapters",
            "runtime.agent_conversation",
            "runtime.agent_permission_bridge",
            "runtime.agent_session_input",
            "runtime.agent_sessions",
            "runtime.approvals",
            "runtime.events",
            "runtime.evidence",
            "runtime.lane_lifecycle",
            "runtime.merge_gate",
            "runtime.project_onboarding",
            "runtime.replay",
            "runtime.snapshot",
            "runtime.starter_lane_preview",
            "runtime.typed_lanes",
            "runtime.workspace_eligibility",
        ],
        snapshot(WorkMode::Build),
        events,
    )
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
                    gate_type: MergeGateType::Artifact,
                    owner: RuntimeOwner::default(),
                    validator: None,
                    policy_snapshot: MergeGatePolicySnapshot::default(),
                    decision: Some(MergeGateDecision {
                        outcome: MergeGateDecisionOutcome::Legacy,
                        reason: "core facts satisfied gate".to_string(),
                        owner: RuntimeOwner::default(),
                        evidence_ids: Vec::new(),
                        reviewed_evidence: Vec::new(),
                        review_request_id: None,
                        audit_id: "legacy".to_string(),
                        decided_at: 0,
                    }),
                    conflict: None,
                    applied_change_id: None,
                    recovery_snapshot: None,
                    audit_ids: Vec::new(),
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
