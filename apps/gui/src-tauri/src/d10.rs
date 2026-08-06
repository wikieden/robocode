//! D10 Lane monitor projections.
//!
//! One card per Core Lane across every project, plus the counts the monitor
//! header shows. Every field is a published Core fact: gate strength, status,
//! project binding, and task progress are read from the contract, never
//! derived from an agent label, a branch name, or a worktree path.

use serde::Serialize;

use crate::d2::D2UnavailableProjection;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D10LaneProjection {
    pub id: String,
    /// Core project binding, or `None` when Core published no owner binding.
    pub project_id: Option<String>,
    pub summary: String,
    pub role: String,
    pub route: String,
    /// First-class lane fact (`full`/`cooperative`/`containment`).
    pub gate_strength: String,
    pub mutation_policy: String,
    pub status: String,
    /// True for the Core statuses that actually block on a human.
    pub awaits_human: bool,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    /// Progress of the Core task this lane names; `None` when it names none.
    pub progress: Option<u8>,
    pub agents: Vec<D10AgentProjection>,
    pub evidence: Vec<D10EvidenceProjection>,
    pub token_limit: Option<u64>,
    pub cost_limit_micro_usd: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D10AgentProjection {
    pub session_id: String,
    pub agent_id: String,
    pub model: Option<String>,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D10EvidenceProjection {
    pub id: String,
    pub kind: String,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D10LaneMonitorProjection {
    pub total_lanes: usize,
    /// Distinct Core project ids across bound lanes.
    pub total_projects: usize,
    pub awaiting_total: usize,
    pub lanes: Vec<D10LaneProjection>,
    /// Design affordances with no Core fact behind them.
    pub unavailable: Vec<D2UnavailableProjection>,
}
