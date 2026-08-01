//! D2 Decision Center projections and intents.
//!
//! D2 is one queue over every decision Core is currently holding for a human:
//! tool-approval gates, contract confirmations, and review requests. The GUI
//! projects the three fact families onto one card skeleton (queue, context,
//! evidence, action bar) and never merges them into a private decision model.
//!
//! Facts that `frontend-contract-v1` does not carry — a structured diff for an
//! approval, and a review-decision command — are declared unavailable with
//! their contract-request code instead of being synthesized from display text.

use serde::{Deserialize, Serialize};

use crate::PermissionChoice;

/// Queue group discriminants. These are stable Core-fact families, not labels;
/// the frontend maps them to localized copy.
pub const D2_KIND_GATE: &str = "gate";
pub const D2_KIND_CONTRACT: &str = "contract";
pub const D2_KIND_REVIEW: &str = "review";

/// A capability the design shows but `frontend-contract-v1` does not carry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D2UnavailableProjection {
    /// Stable reason key; the frontend owns the localized sentence.
    pub key: &'static str,
    /// Contract request that closes this gap.
    pub code: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D2QueueItemProjection {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub project_id: String,
    pub lane_id: Option<String>,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    /// Only approvals carry a Core risk bucket.
    pub risk: Option<String>,
    pub status: String,
    pub audit_id: String,
    pub updated_at: Option<u64>,
    pub expires_at: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D2GroupProjection {
    pub kind: String,
    pub items: Vec<D2QueueItemProjection>,
    /// Set when the group cannot mean what the design implies. The contract
    /// group is the current case: Core records decided contracts and has no
    /// "awaiting confirmation" fact, so the group must not read as a backlog.
    pub unavailable: Option<D2UnavailableProjection>,
}

/// Left pane of the decision card: the Core-owned context for the decision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D2ContextProjection {
    /// Which Core fact the text came from, so the shell cannot relabel it.
    pub source: &'static str,
    pub text: String,
    pub unavailable: Option<D2UnavailableProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D2EvidenceProjection {
    pub id: String,
    pub kind: String,
    pub summary: String,
    pub path: Option<String>,
    pub source: Option<String>,
    pub timestamp: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D2ActionProjection {
    pub kind: String,
    pub available: bool,
    pub session_id: Option<String>,
    pub paths: Vec<String>,
    pub code: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D2DetailProjection {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub project_id: String,
    pub lane_id: Option<String>,
    pub task_id: Option<String>,
    /// Where Core writes the decision and its reason.
    pub audit_id: String,
    pub policy_reason_key: Option<String>,
    pub blocked_by_plan: bool,
    pub context: D2ContextProjection,
    pub evidence: Vec<D2EvidenceProjection>,
    pub actions: Vec<D2ActionProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D2DecisionsProjection {
    pub work_mode: String,
    pub permission_level: String,
    /// Command-bar count across the whole queue, never a per-group count.
    pub pending_total: usize,
    pub selected_id: Option<String>,
    pub groups: Vec<D2GroupProjection>,
    pub detail: Option<D2DetailProjection>,
}

impl D2DecisionsProjection {
    pub fn group(&self, kind: &str) -> Option<&D2GroupProjection> {
        self.groups.iter().find(|group| group.kind == kind)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum D2Intent {
    /// Selection is presentation-only; it never mutates Core state.
    Select {
        id: String,
    },
    RespondApproval {
        request_id: String,
        choice: PermissionChoice,
        feedback: Option<String>,
    },
    DecideContract {
        contract_id: String,
        accept: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D2IntentResult {
    pub projection: D2DecisionsProjection,
    pub pending_command_id: Option<String>,
    pub outcome: crate::PermissionOutcomeProjection,
}
