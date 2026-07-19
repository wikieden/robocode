use viden_core::RuntimeCommand;

use super::lane::{attach_lane_intent, detach_lane_intent, stop_lane_intent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideScreen {
    Main,
    Agent,
    Ops,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScreenLaneAction {
    Attach,
    Detach,
    Stop,
}

pub(super) fn screen_lane_intent(
    action: ScreenLaneAction,
    lane_id: impl Into<String>,
) -> RuntimeCommand {
    let lane_id = lane_id.into();
    match action {
        ScreenLaneAction::Attach => attach_lane_intent(lane_id),
        ScreenLaneAction::Detach => detach_lane_intent(lane_id),
        ScreenLaneAction::Stop => stop_lane_intent(lane_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_actions_are_pure_lane_intents() {
        assert_eq!(
            screen_lane_intent(ScreenLaneAction::Attach, "lane-1"),
            RuntimeCommand::AttachLane {
                lane_id: "lane-1".to_string()
            }
        );
    }
}
