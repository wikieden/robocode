use serde::{Deserialize, Serialize};

use crate::{D6RecoveryProjection, PermissionDockProjection, ResolvedPreferencesProjection};

pub const D1_OWNER_CAPABILITY: &str = "runtime.lane_owner_projection";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct D1WorkspaceSourceProjection {
    pub status: &'static str,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub added: u32,
    pub deleted: u32,
    pub dirty: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1ContextUsageProjection {
    pub budget_id: String,
    pub used_tokens: u64,
    pub soft_token_limit: u64,
    pub hard_token_limit: u64,
    pub remaining_tokens: u64,
    pub exceeded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1LaneAgentProjection {
    pub lane_id: String,
    pub workspace_id: String,
    pub project_id: String,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub turn_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1ProviderHealthProjection {
    pub provider_id: String,
    pub model: String,
    pub status: String,
    pub request_count: u64,
    pub error_count: u64,
    pub last_latency_ms: Option<u64>,
    pub average_latency_ms: Option<u64>,
    pub tokens_per_second: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1RuntimeServiceProjection {
    pub id: String,
    pub kind: &'static str,
    pub label: String,
    pub status: &'static str,
    pub detail_key: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1ChecklistItemProjection {
    pub id: String,
    pub kind: &'static str,
    pub label: String,
    pub status: &'static str,
    pub command: Option<String>,
    pub path: Option<String>,
    pub summary: Option<String>,
    pub patch: Option<String>,
    pub failing_location: Option<String>,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1ContextDockProjection {
    pub source: Option<D1WorkspaceSourceProjection>,
    pub context: Option<D1ContextUsageProjection>,
    pub lane_agent: Option<D1LaneAgentProjection>,
    pub provider: Option<D1ProviderHealthProjection>,
    pub services: Vec<D1RuntimeServiceProjection>,
    pub checklist: Vec<D1ChecklistItemProjection>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum D1Intent {
    Submit {
        lane_id: String,
        content: String,
    },
    Cancel {
        lane_id: String,
    },
    QueryAgentAdapters,
    ProbeAgentAdapter {
        agent_id: String,
    },
    PreviewDefaultLane {
        preset: String,
    },
    CreateStarterLane {
        lane_id: String,
        preset: String,
        branch: Option<String>,
        preview_id: String,
        content_sha256: String,
    },
    StartAgentSession {
        lane_id: String,
        agent_id: String,
        model: Option<String>,
        task: String,
    },
    SendAgentSessionInput {
        lane_id: String,
        session_id: String,
        content: String,
    },
    RetryAgentSession {
        lane_id: String,
        session_id: String,
    },
    CancelAgentSession {
        lane_id: String,
        session_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct D1OutcomeProjection {
    pub state: &'static str,
    pub reason: Option<String>,
}

impl D1OutcomeProjection {
    pub(crate) fn idle() -> Self {
        Self {
            state: "idle",
            reason: None,
        }
    }

    pub(crate) fn pending() -> Self {
        Self {
            state: "pending",
            reason: None,
        }
    }

    pub(crate) fn confirmed() -> Self {
        Self {
            state: "confirmed",
            reason: None,
        }
    }

    pub(crate) fn rejected(reason: String) -> Self {
        Self {
            state: "rejected",
            reason: Some(reason),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1LaneProjection {
    pub id: String,
    pub role: String,
    pub status: String,
    pub summary: String,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1EnvironmentProjection {
    pub cwd: String,
    pub provider_id: String,
    pub model: String,
    pub work_mode: String,
    pub permission_level: String,
    pub token_total: u64,
    pub cost_micro_usd: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct D1TaskProjection {
    pub id: String,
    pub title: String,
    pub status: String,
    pub progress: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1ToolProjection {
    pub id: String,
    pub name: String,
    pub input_preview: String,
    pub state: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct D1ApprovalProjection {
    pub id: String,
    pub title: String,
    pub risk: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1QueuedInputProjection {
    pub id: String,
    pub content_preview: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct D1EvidenceProjection {
    pub id: String,
    pub kind: String,
    pub summary: String,
    pub path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1LiveWorkProjection {
    pub tasks: Vec<D1TaskProjection>,
    pub tools: Vec<D1ToolProjection>,
    pub approvals: Vec<D1ApprovalProjection>,
    pub queued_inputs: Vec<D1QueuedInputProjection>,
    pub evidence: Vec<D1EvidenceProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct D1TranscriptRowProjection {
    pub id: String,
    pub kind: &'static str,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1WorkspaceEligibilityProjection {
    pub is_git_repository: bool,
    pub has_head: bool,
    pub can_create_lane: bool,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1StarterLanePreviewProjection {
    pub preview_id: String,
    pub content_sha256: String,
    pub lane_id: String,
    pub branch: Option<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1StarterLaneReceiptProjection {
    pub preview_id: String,
    pub lane_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1AgentAdapterProjection {
    pub agent_id: String,
    pub display_name: String,
    pub startability: String,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1AgentSessionProjection {
    pub session_id: String,
    pub lane_id: String,
    pub agent_id: String,
    pub model: Option<String>,
    pub status: String,
    pub task: String,
    pub diagnostic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1AgentSessionInputProjection {
    pub session_id: String,
    pub input_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1CostUsageProjection {
    pub usage_id: String,
    pub attempt_index: u32,
    pub total_tokens: u64,
    pub actual_cost_micro_usd: Option<u64>,
    pub outcome: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1CursorProjection {
    pub stream_id: String,
    pub sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1ComposerProjection {
    pub editable: bool,
    pub busy: bool,
    pub can_cancel: bool,
    pub can_submit_immediately: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct D1UnavailableFeatureProjection {
    pub id: &'static str,
    pub available: bool,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1CockpitProjection {
    pub preferences: ResolvedPreferencesProjection,
    pub selected_lane_id: Option<String>,
    pub context_dock: D1ContextDockProjection,
    pub lanes: Vec<D1LaneProjection>,
    pub environment: D1EnvironmentProjection,
    pub live_work: D1LiveWorkProjection,
    pub transcript: Vec<D1TranscriptRowProjection>,
    pub workspace_eligibility: Option<D1WorkspaceEligibilityProjection>,
    pub starter_lane_previews: Vec<D1StarterLanePreviewProjection>,
    pub starter_lane_receipts: Vec<D1StarterLaneReceiptProjection>,
    pub agent_adapters: Vec<D1AgentAdapterProjection>,
    pub agent_sessions: Vec<D1AgentSessionProjection>,
    pub agent_session_inputs: Vec<D1AgentSessionInputProjection>,
    pub cost_usage: Vec<D1CostUsageProjection>,
    pub replay_cursor: D1CursorProjection,
    pub composer: D1ComposerProjection,
    pub permission_dock: PermissionDockProjection,
    pub recovery: D6RecoveryProjection,
    pub unavailable_features: Vec<D1UnavailableFeatureProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D1IntentResult {
    pub projection: D1CockpitProjection,
    pub pending_command_id: Option<String>,
    pub outcome: D1OutcomeProjection,
}

pub(crate) fn unavailable_features() -> Vec<D1UnavailableFeatureProjection> {
    vec![
        D1UnavailableFeatureProjection {
            id: "diff",
            available: false,
            code: "GUI-CORE-006",
            message: "Typed diff facts are unavailable.",
        },
        D1UnavailableFeatureProjection {
            id: "apply",
            available: false,
            code: "GUI-CORE-006",
            message: "Typed apply receipts are unavailable.",
        },
        D1UnavailableFeatureProjection {
            id: "audit",
            available: false,
            code: "GUI-CORE-004",
            message: "Typed audit history is unavailable.",
        },
        D1UnavailableFeatureProjection {
            id: "recovery",
            available: false,
            code: "GUI-CORE-003",
            message: "Typed recovery actions are unavailable.",
        },
        D1UnavailableFeatureProjection {
            id: "transcript_user",
            available: false,
            code: "GUI-CORE-009",
            message: "Typed user prompt rows are unavailable.",
        },
        D1UnavailableFeatureProjection {
            id: "transcript_assistant",
            available: false,
            code: "GUI-CORE-009",
            message: "Owner-scoped assistant rows are unavailable.",
        },
        D1UnavailableFeatureProjection {
            id: "live_work_scope",
            available: false,
            code: "GUI-CORE-010",
            message: "Owner-scoped live-work facts are unavailable.",
        },
    ]
}
