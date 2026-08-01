use std::sync::{Arc, Mutex};

use viden_core::{
    DependencyRecord, DependencyState, RuntimeOwner, RuntimeSnapshot, RuntimeViewState,
};
use viden_gui::GuiCoreAdapter;

mod support;
use support::TestCoreClient;

const DAG_FIXTURE: &str =
    include_str!("../../../crates/types/tests/fixtures/frontend-contract-v1/dag-blocker.json");

fn dag_view() -> RuntimeViewState {
    #[derive(serde::Deserialize)]
    struct Fixture {
        initial_snapshot: RuntimeSnapshot,
        events: Vec<viden_core::RuntimeEventEnvelope>,
    }
    let fixture: Fixture = serde_json::from_str(DAG_FIXTURE).unwrap();
    let mut view = RuntimeViewState::new(fixture.initial_snapshot);
    for envelope in fixture.events {
        if let viden_core::RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }
    }
    assert!(
        !view.agent_dags.is_empty(),
        "the dag fixture must publish a workflow"
    );
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
fn d13_projects_the_core_dag_with_its_declared_edges() {
    let view = dag_view();
    let dag = view.agent_dags[0].clone();
    let projection = connected(view).d13_fleet_workflow().expect("D13");

    let workflow = &projection.workflows[0];
    assert_eq!(workflow.dag_id, dag.dag_id);
    assert_eq!(workflow.goal, dag.goal);
    assert_eq!(workflow.nodes.len(), dag.tasks.len());
    for (node, spec) in workflow.nodes.iter().zip(dag.tasks.iter()) {
        // Edges are the Core-declared dependency list, not an inferred order.
        assert_eq!(node.task_id, spec.task_id);
        assert_eq!(node.depends_on, spec.dependencies);
        assert_eq!(node.required_evidence, spec.required_evidence);
    }
}

#[test]
fn d13_reports_runtime_status_only_for_a_task_core_is_actually_running() {
    let mut view = dag_view();
    let known_task = view.agent_dags[0].tasks[0].task_id.clone();
    view.tasks.retain(|task| task.id == known_task);
    let has_live_task = !view.tasks.is_empty();

    let projection = connected(view).d13_fleet_workflow().unwrap();
    let workflow = &projection.workflows[0];
    for node in &workflow.nodes {
        if node.task_id == known_task && has_live_task {
            assert!(node.status.is_some(), "a live Core task must report status");
        } else {
            // A planned node with no Core task is not shown as pending work.
            assert!(node.status.is_none());
        }
    }
}

#[test]
fn d13_names_the_blocking_dependency_from_the_core_record() {
    let mut view = dag_view();
    let blocked = view.agent_dags[0].tasks[0].task_id.clone();
    view.dependencies.push(DependencyRecord {
        dependency_id: "dependency-1".to_string(),
        task_id: blocked.clone(),
        depends_on_task_id: "task-upstream".to_string(),
        owner: RuntimeOwner {
            workspace_id: "workspace-viden".to_string(),
            project_id: "project-viden".to_string(),
            ..Default::default()
        },
        state: DependencyState::Blocked,
        reason: "waits for the upstream contract".to_string(),
        audit_id: "audit-dependency-1".to_string(),
        updated_at: 1_700_000_400,
    });

    let projection = connected(view).d13_fleet_workflow().unwrap();
    let node = projection.workflows[0]
        .nodes
        .iter()
        .find(|node| node.task_id == blocked)
        .expect("blocked node");
    assert_eq!(node.blockers.len(), 1);
    assert_eq!(node.blockers[0].depends_on_task_id, "task-upstream");
    assert_eq!(node.blockers[0].reason, "waits for the upstream contract");
    assert!(node.blocked);
}

#[test]
fn d13_lists_handoffs_between_lanes_without_inventing_a_route() {
    let projection = connected(dag_view()).d13_fleet_workflow().unwrap();
    // The fixture declares no handoff; the screen shows none rather than
    // deriving one from the dependency edges.
    assert!(projection.handoffs.is_empty());
}
