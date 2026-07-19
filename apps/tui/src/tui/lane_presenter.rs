#![allow(dead_code)]

use viden_core::{AgentLaneRecord, AgentRoute, LaneStatus};

pub(super) fn status_badge(status: LaneStatus) -> &'static str {
    match status {
        LaneStatus::Running | LaneStatus::Starting | LaneStatus::Attached => "LIVE",
        LaneStatus::WaitingApproval | LaneStatus::NeedsInput => "REVIEW",
        LaneStatus::Blocked | LaneStatus::Failed => "BLOCKED",
        LaneStatus::Done | LaneStatus::Archived => "DONE",
        LaneStatus::Cancelled | LaneStatus::Detached => "STOPPED",
        LaneStatus::Draft | LaneStatus::Queued => "QUEUED",
    }
}

pub(super) fn terminal_label(route: AgentRoute) -> &'static str {
    match route {
        AgentRoute::BuiltIn => "CORE",
        AgentRoute::Acp => "ACP",
        AgentRoute::Terminal => "TERM",
        AgentRoute::Tmux => "TMUX",
    }
}

pub(super) fn pty_label(route: AgentRoute) -> &'static str {
    terminal_label(route)
}

pub(super) fn pid_hint(_lane: &AgentLaneRecord) -> String {
    "Core managed".to_string()
}

pub(super) fn command_hint(lane: &AgentLaneRecord) -> String {
    format!("/lane inspect {}", lane.id)
}

pub(super) fn interaction_hint(lane: &AgentLaneRecord) -> String {
    format!("select {} for actions", lane.id)
}

pub(super) fn lane_next_action(lane: &AgentLaneRecord) -> String {
    match lane.status {
        LaneStatus::WaitingApproval => "review the pending Core approval".to_string(),
        LaneStatus::Blocked => "inspect Core recovery guidance".to_string(),
        LaneStatus::Done => "review structured evidence".to_string(),
        _ => "wait for the next Core event".to_string(),
    }
}
