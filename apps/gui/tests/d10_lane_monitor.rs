use std::sync::{Arc, Mutex};

use viden_core::{RuntimeSnapshot, RuntimeViewState};
use viden_gui::GuiCoreAdapter;

mod support;
use support::TestCoreClient;

const MULTI_LANE_FIXTURE: &str =
    include_str!("../../../crates/types/tests/fixtures/frontend-contract-v1/multi-lane.json");

fn multi_lane_view() -> RuntimeViewState {
    #[derive(serde::Deserialize)]
    struct Fixture {
        initial_snapshot: RuntimeSnapshot,
        events: Vec<viden_core::RuntimeEventEnvelope>,
    }
    let fixture: Fixture = serde_json::from_str(MULTI_LANE_FIXTURE).unwrap();
    let mut view = RuntimeViewState::new(fixture.initial_snapshot);
    for envelope in fixture.events {
        if let viden_core::RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }
    }
    view
}

fn connected(view: RuntimeViewState) -> GuiCoreAdapter {
    let mut adapter = GuiCoreAdapter::new(Box::new(TestCoreClient::new(
        view,
        Arc::new(Mutex::new(Vec::new())),
    )));
    adapter.connect().unwrap();
    adapter
}

#[test]
fn d10_projects_one_card_per_core_lane_with_its_own_gate_strength() {
    let view = multi_lane_view();
    let expected: Vec<(String, String)> = view
        .lanes
        .iter()
        .map(|lane| {
            (
                lane.id.clone(),
                match lane.gate_strength {
                    viden_core::GateStrength::Full => "full",
                    viden_core::GateStrength::Cooperative => "cooperative",
                    viden_core::GateStrength::Containment => "containment",
                }
                .to_string(),
            )
        })
        .collect();
    assert!(
        expected.len() >= 2,
        "the multi-lane fixture must carry more than one lane"
    );

    let monitor = connected(view).d10_lane_monitor().expect("D10 projection");
    let actual: Vec<(String, String)> = monitor
        .lanes
        .iter()
        .map(|lane| (lane.id.clone(), lane.gate_strength.clone()))
        .collect();

    // Gate strength is a first-class Core lane fact. The design prototype
    // derived it from the agent label; the client must not.
    assert_eq!(actual, expected);
    assert_eq!(monitor.total_lanes, expected.len());
}

#[test]
fn d10_counts_only_core_states_that_actually_await_a_human() {
    let mut view = multi_lane_view();
    for lane in view.lanes.iter_mut() {
        lane.status = viden_core::LaneStatus::Running;
    }
    assert_eq!(
        connected(view.clone())
            .d10_lane_monitor()
            .unwrap()
            .awaiting_total,
        0
    );

    view.lanes[0].status = viden_core::LaneStatus::WaitingApproval;
    view.lanes[1].status = viden_core::LaneStatus::NeedsInput;
    let monitor = connected(view).d10_lane_monitor().expect("D10 projection");
    assert_eq!(monitor.awaiting_total, 2);
    assert!(monitor.lanes[0].awaits_human);
    assert!(monitor.lanes[1].awaits_human);
}

#[test]
fn d10_binds_a_lane_to_its_project_only_through_the_core_owner_binding() {
    let mut view = multi_lane_view();
    assert!(
        view.lane_runtime_owners.is_empty(),
        "the fixture starts with no owner binding, so both branches are real"
    );
    let bound_lane = view.lanes[0].id.clone();
    let unbound_lane = view.lanes[1].id.clone();
    view.lane_runtime_owners
        .push(viden_core::LaneRuntimeOwnerBinding {
            lane_id: bound_lane.clone(),
            owner: viden_core::RuntimeOwner {
                workspace_id: "workspace-viden".to_string(),
                project_id: "project-boss-rush".to_string(),
                lane_id: Some(bound_lane.clone()),
                ..Default::default()
            },
        });

    let monitor = connected(view).d10_lane_monitor().expect("D10 projection");
    let bound = monitor
        .lanes
        .iter()
        .find(|lane| lane.id == bound_lane)
        .unwrap();
    let unbound = monitor
        .lanes
        .iter()
        .find(|lane| lane.id == unbound_lane)
        .unwrap();

    assert_eq!(bound.project_id.as_deref(), Some("project-boss-rush"));
    // No binding means no project. The client never guesses one from the lane
    // id, branch, or worktree path.
    assert_eq!(unbound.project_id, None);
    assert_eq!(monitor.total_projects, 1);
}

#[test]
fn d10_reports_task_progress_only_for_the_lane_task_core_named() {
    let mut view = multi_lane_view();
    let lane_id = view.lanes[0].id.clone();
    let task_id = view.lanes[0]
        .task_id
        .clone()
        .expect("the fixture lane must carry a task id");
    for task in view.tasks.iter_mut() {
        if task.id == task_id {
            task.progress = 42;
        }
    }
    let monitor = connected(view).d10_lane_monitor().expect("D10 projection");
    let lane = monitor
        .lanes
        .iter()
        .find(|lane| lane.id == lane_id)
        .expect("projected lane");
    assert_eq!(lane.progress, Some(42));

    // A lane without a Core task has no progress; it is not defaulted to 0.
    let mut detached = multi_lane_view();
    detached.lanes[0].task_id = None;
    let detached_monitor = connected(detached).d10_lane_monitor().unwrap();
    assert_eq!(detached_monitor.lanes[0].progress, None);
}

#[test]
fn d10_declares_the_event_stream_unavailable_instead_of_inventing_one() {
    let monitor = connected(multi_lane_view())
        .d10_lane_monitor()
        .expect("D10 projection");
    // The design shows a scribe-compiled event ticker. frontend-contract-v1
    // publishes no ordered event log in the view state.
    let unavailable = monitor
        .unavailable
        .iter()
        .find(|entry| entry.code == "GUI-CORE-014")
        .expect("the event stream gap must be declared");
    assert_eq!(unavailable.key, "d10.events.noOrderedLog");
}
