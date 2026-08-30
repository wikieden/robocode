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

/// Writes bounded run facts onto a Core lane record through its own wire form.
///
/// `viden-core` re-exports `AgentLaneRecord` but not `LaneRunStats`, and the
/// GUI track must not reach around the facade into `viden-types`, so the
/// fixture is built the way Core serializes it.
fn with_run_stats(
    view: &mut RuntimeViewState,
    lane_index: usize,
    wall_time_ms: u64,
    run_count: u64,
    diff_bytes: u64,
    last_exit_code: Option<i32>,
) {
    let mut wire = serde_json::to_value(&view.lanes[lane_index]).expect("serialize lane record");
    wire["run_stats"] = serde_json::json!({
        "wall_time_ms": wall_time_ms,
        "run_count": run_count,
        "diff_bytes": diff_bytes,
        "last_exit_code": last_exit_code,
    });
    view.lanes[lane_index] = serde_json::from_value(wire).expect("lane record with run stats");
}

fn lane_index_by_route(view: &RuntimeViewState, route: viden_core::AgentRoute) -> usize {
    view.lanes
        .iter()
        .position(|lane| lane.route == route)
        .unwrap_or_else(|| panic!("the multi-lane fixture must carry a {route:?} lane"))
}

#[test]
fn d10_marks_every_lane_with_the_cost_meterability_core_derives_from_its_route() {
    let view = multi_lane_view();
    let terminal = view.lanes[lane_index_by_route(&view, viden_core::AgentRoute::Terminal)]
        .id
        .clone();
    let acp = view.lanes[lane_index_by_route(&view, viden_core::AgentRoute::Acp)]
        .id
        .clone();

    let monitor = connected(view).d10_lane_monitor().expect("D10 projection");
    let blind = monitor
        .lanes
        .iter()
        .find(|lane| lane.id == terminal)
        .unwrap();
    let metered = monitor.lanes.iter().find(|lane| lane.id == acp).unwrap();

    // `AgentRoute::cost_meterability` is the authority. The client never
    // re-derives the policy from the route name.
    assert_eq!(blind.cost_meterability, "blind");
    assert_eq!(metered.cost_meterability, "metered");
}

#[test]
fn d10_surfaces_the_bounded_run_facts_core_recorded_for_a_cost_blind_lane() {
    let mut view = multi_lane_view();
    let index = lane_index_by_route(&view, viden_core::AgentRoute::Terminal);
    let lane_id = view.lanes[index].id.clone();
    with_run_stats(&mut view, index, 200_400, 3, 8_192, Some(0));

    let monitor = connected(view).d10_lane_monitor().expect("D10 projection");
    let lane = monitor
        .lanes
        .iter()
        .find(|lane| lane.id == lane_id)
        .unwrap();
    let stats = lane.run_stats.as_ref().expect("recorded run stats");

    assert_eq!(stats.wall_time_ms, 200_400);
    // Humanized on the host, so both frontends read the same duration.
    assert_eq!(stats.wall_time, "3m 20s");
    assert_eq!(stats.run_count, 3);
    assert_eq!(stats.diff_bytes, 8_192);
    assert_eq!(stats.last_exit_code, Some(0));
}

#[test]
fn d10_humanizes_wall_time_across_the_three_host_side_bands() {
    for (ms, expected) in [
        (0_u64, "0ms"),
        (850, "850ms"),
        (999, "999ms"),
        (1_000, "1.0s"),
        (12_400, "12.4s"),
        (59_900, "59.9s"),
        (60_000, "1m 0s"),
        (200_400, "3m 20s"),
    ] {
        let mut view = multi_lane_view();
        let index = lane_index_by_route(&view, viden_core::AgentRoute::Terminal);
        let lane_id = view.lanes[index].id.clone();
        with_run_stats(&mut view, index, ms, 1, 0, None);
        let monitor = connected(view).d10_lane_monitor().expect("D10 projection");
        let lane = monitor
            .lanes
            .iter()
            .find(|lane| lane.id == lane_id)
            .unwrap();
        assert_eq!(
            lane.run_stats.as_ref().unwrap().wall_time,
            expected,
            "{ms}ms"
        );
    }
}

#[test]
fn d10_leaves_run_stats_absent_for_a_blind_lane_core_never_observed_running() {
    let view = multi_lane_view();
    let index = lane_index_by_route(&view, viden_core::AgentRoute::Terminal);
    let lane_id = view.lanes[index].id.clone();
    assert!(
        view.lanes[index].run_stats.is_none(),
        "the fixture lane must start unobserved"
    );

    let monitor = connected(view).d10_lane_monitor().expect("D10 projection");
    let lane = monitor
        .lanes
        .iter()
        .find(|lane| lane.id == lane_id)
        .unwrap();
    // Absence is absence: an unobserved lane must not be projected as a
    // measured zero, which is a different Core fact.
    assert_eq!(lane.run_stats, None);
}

#[test]
fn d10_keeps_a_force_killed_exit_code_unknown_rather_than_defaulting_it() {
    let mut view = multi_lane_view();
    let index = lane_index_by_route(&view, viden_core::AgentRoute::Terminal);
    let lane_id = view.lanes[index].id.clone();
    with_run_stats(&mut view, index, 1_500, 1, 0, None);

    let monitor = connected(view).d10_lane_monitor().expect("D10 projection");
    let lane = monitor
        .lanes
        .iter()
        .find(|lane| lane.id == lane_id)
        .unwrap();
    assert_eq!(lane.run_stats.as_ref().unwrap().last_exit_code, None);
}

#[test]
fn d10_never_projects_process_run_facts_as_a_metered_lane_cost_surface() {
    let mut view = multi_lane_view();
    let index = lane_index_by_route(&view, viden_core::AgentRoute::Acp);
    let lane_id = view.lanes[index].id.clone();
    // Even when Core recorded process facts for a metered lane, D10 keeps its
    // cost story the token/cost ledger; these bounded facts exist to replace a
    // cost figure that does not exist, not to add a second one.
    with_run_stats(&mut view, index, 5_000, 2, 64, Some(1));

    let monitor = connected(view).d10_lane_monitor().expect("D10 projection");
    let lane = monitor
        .lanes
        .iter()
        .find(|lane| lane.id == lane_id)
        .unwrap();
    assert_eq!(lane.cost_meterability, "metered");
    assert_eq!(lane.run_stats, None);
}
