use viden_core::{AgentLaneRecord, RuntimeCommand};

pub(super) fn create_lane_intent(lane: AgentLaneRecord) -> RuntimeCommand {
    RuntimeCommand::CreateLane { lane }
}

pub(super) fn start_lane_intent(
    lane_id: impl Into<String>,
    command: impl Into<String>,
    args: Vec<String>,
    env: Vec<(String, String)>,
    output_log: Option<String>,
) -> RuntimeCommand {
    RuntimeCommand::StartLane {
        lane_id: lane_id.into(),
        command: command.into(),
        args,
        env,
        output_log,
    }
}

pub(super) fn stop_lane_intent(lane_id: impl Into<String>) -> RuntimeCommand {
    RuntimeCommand::StopLane {
        lane_id: lane_id.into(),
    }
}

pub(super) fn attach_lane_intent(lane_id: impl Into<String>) -> RuntimeCommand {
    RuntimeCommand::AttachLane {
        lane_id: lane_id.into(),
    }
}

pub(super) fn detach_lane_intent(lane_id: impl Into<String>) -> RuntimeCommand {
    RuntimeCommand::DetachLane {
        lane_id: lane_id.into(),
    }
}

pub(super) fn send_lane_input_intent(
    lane_id: impl Into<String>,
    input: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::SendLaneInput {
        lane_id: lane_id.into(),
        input: input.into(),
    }
}

pub(super) fn accept_lane_output_intent(
    lane_id: impl Into<String>,
    summary: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::AcceptLaneOutput {
        lane_id: lane_id.into(),
        summary: summary.into(),
    }
}

pub(super) fn revise_lane_output_intent(
    lane_id: impl Into<String>,
    feedback: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::ReviseLaneOutput {
        lane_id: lane_id.into(),
        feedback: feedback.into(),
    }
}

pub(super) fn discard_lane_output_intent(
    lane_id: impl Into<String>,
    reason: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::DiscardLaneOutput {
        lane_id: lane_id.into(),
        reason: reason.into(),
    }
}

pub(super) fn apply_lane_changes_intent(
    lane_id: impl Into<String>,
    unified_diff: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::ApplyLaneChanges {
        lane_id: lane_id.into(),
        unified_diff: unified_diff.into(),
    }
}

pub(super) fn resolve_lane_conflict_intent(
    lane_id: impl Into<String>,
    unified_diff: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::ResolveLaneConflict {
        lane_id: lane_id.into(),
        unified_diff: unified_diff.into(),
    }
}

pub(super) fn archive_lane_intent(
    lane_id: impl Into<String>,
    summary: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::ArchiveLane {
        lane_id: lane_id.into(),
        summary: summary.into(),
    }
}

pub(super) fn cleanup_lane_intent(lane_id: impl Into<String>, force: bool) -> RuntimeCommand {
    RuntimeCommand::CleanupLane {
        lane_id: lane_id.into(),
        force,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_builders_return_frozen_core_commands() {
        assert_eq!(
            stop_lane_intent("lane-1"),
            RuntimeCommand::StopLane {
                lane_id: "lane-1".to_string()
            }
        );
        assert_eq!(
            attach_lane_intent("lane-1"),
            RuntimeCommand::AttachLane {
                lane_id: "lane-1".to_string()
            }
        );
        assert_eq!(
            cleanup_lane_intent("lane-1", true),
            RuntimeCommand::CleanupLane {
                lane_id: "lane-1".to_string(),
                force: true
            }
        );
    }

    #[test]
    fn apply_builder_carries_data_without_executing_it() {
        assert_eq!(
            apply_lane_changes_intent("lane-1", "diff --git a/a b/a"),
            RuntimeCommand::ApplyLaneChanges {
                lane_id: "lane-1".to_string(),
                unified_diff: "diff --git a/a b/a".to_string(),
            }
        );
    }
}
