//! D12 integration gate projections.
//!
//! The integration gate is the failure path of the acceptance loop: two Lanes
//! touched the same place and the merge conflicts. The client never offers a
//! manual merge. It shows the Core gate, the bounce timeline back to the
//! origin Lane, and any post-merge revert, and it keeps `accept` closed until
//! every evidence id the gate policy requires is present.

use serde::Serialize;

use crate::d2::D2UnavailableProjection;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D12GateProjection {
    pub gate_id: String,
    pub task_id: String,
    pub status: String,
    pub gate_type: String,
    pub project_id: String,
    pub lane_id: Option<String>,
    pub requires_independent_validator: bool,
    pub has_validator: bool,
    pub required_evidence: Vec<String>,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D12BounceProjection {
    pub bounce_id: String,
    pub original_lane_id: String,
    pub task_id: String,
    pub reason: String,
    pub status: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D12RevertProjection {
    pub revert_id: String,
    pub applied_change_id: String,
    pub reason: String,
    pub restored_paths: Vec<String>,
    pub audit_id: String,
    pub reverted_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D12CheckProjection {
    pub id: String,
    pub name: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D12ActionProjection {
    pub kind: String,
    pub available: bool,
    pub code: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D12GateDetailProjection {
    pub gate: D12GateProjection,
    /// Evidence ids the policy requires that Core has not recorded yet.
    pub missing_evidence: Vec<String>,
    pub bounces: Vec<D12BounceProjection>,
    pub reverts: Vec<D12RevertProjection>,
    pub checks: Vec<D12CheckProjection>,
    pub actions: Vec<D12ActionProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D12IntegrationGateProjection {
    pub gates: Vec<D12GateProjection>,
    pub selected_gate_id: Option<String>,
    pub detail: Option<D12GateDetailProjection>,
    pub unavailable: Vec<D2UnavailableProjection>,
}
