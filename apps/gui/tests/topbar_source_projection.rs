//! The cockpit titlebar's git block is a read-only projection of the workspace
//! source Core sampled. It exists only while Core publishes usable facts: an
//! absent or unavailable source omits the block rather than rendering zeroes
//! that would read as "clean tree, in sync".

use std::sync::{Arc, Mutex};

use viden_core::{
    RuntimeEvent, RuntimeEventEnvelope, RuntimeEventKind, RuntimeSnapshot, RuntimeViewState,
    RuntimeWireEvent,
};
use viden_gui::GuiCoreAdapter;

mod support;
use support::TestCoreClient;

const D1_FIXTURE: &str = include_str!(
    "../../../crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
);

#[derive(serde::Deserialize)]
struct Fixture {
    initial_snapshot: RuntimeSnapshot,
    events: Vec<RuntimeEventEnvelope>,
}

fn d1_view() -> RuntimeViewState {
    let fixture: Fixture = serde_json::from_str(D1_FIXTURE).expect("parse D1 fixture");
    let mut view = RuntimeViewState::new(fixture.initial_snapshot);
    for envelope in fixture.events {
        if let RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }
    }
    view
}

fn connected(view: RuntimeViewState) -> GuiCoreAdapter {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = GuiCoreAdapter::new(Box::new(TestCoreClient::new(view, sent)));
    adapter.connect().expect("connect topbar client");
    adapter
}

/// A `workspace_source_updated` event carrying exactly the sampled facts.
fn workspace_source(status: &str, ahead: u32, behind: u32, dirty: bool) -> RuntimeEventKind {
    serde_json::from_value(serde_json::json!({
        "type": "workspace_source_updated",
        "payload": {
            "source": {
                "status": status,
                "branch": "codex/v3-gui-client",
                "worktree": ".worktrees/v3-gui-client",
                "ahead": ahead,
                "behind": behind,
                "added": 4,
                "deleted": 1,
                "dirty": dirty
            }
        }
    }))
    .expect("typed workspace source event")
}

/// A `lane_updated` event mirroring the fixture's Lane shape.
fn lane(id: &str, status: &str, worktree: Option<&str>) -> RuntimeEventKind {
    serde_json::from_value(serde_json::json!({
        "type": "lane_updated",
        "payload": {
            "lane": {
                "id": id,
                "task_id": null,
                "role": "coder",
                "route": "terminal",
                "gate_strength": "containment",
                "mutation_policy": "propose_only",
                "worktree": worktree,
                "branch": format!("codex/{id}"),
                "target": "local",
                "data_egress": "deny",
                "status": status,
                "budget": {
                    "token_limit": 16000,
                    "cost_limit_micro_usd": 500000,
                    "wall_time_limit_secs": 1800
                },
                "active_session_ids": [],
                "summary": format!("{id} summary"),
                "evidence": []
            }
        }
    }))
    .expect("typed lane event")
}

#[test]
fn topbar_source_projects_the_git_facts_core_sampled() {
    let mut view = d1_view();
    view.apply_event(&RuntimeEvent::new(
        90,
        workspace_source("ready", 2, 1, true),
    ));

    let projection = connected(view)
        .d1_cockpit(Some("lane_d1_core"))
        .expect("D1 projection");
    let topbar = projection
        .topbar_source
        .as_ref()
        .expect("a sampled workspace source projects the titlebar git block");

    assert_eq!(topbar.status, "ready");
    assert_eq!(topbar.branch.as_deref(), Some("codex/v3-gui-client"));
    assert_eq!(topbar.ahead, 2);
    assert_eq!(topbar.behind, 1);
    assert!(topbar.dirty);
    assert!(!topbar.truncated);
    // The fixture publishes no project probe, so no project name is invented;
    // the frontend falls back to the workspace path it already renders.
    assert_eq!(topbar.project, None);
    // The fixture's single active Lane owns one worktree.
    assert_eq!(topbar.lane_worktree_count, 1);
}

#[test]
fn topbar_source_is_absent_until_core_publishes_a_workspace_source() {
    let projection = connected(d1_view())
        .d1_cockpit(Some("lane_d1_core"))
        .expect("D1 projection");
    assert!(projection.topbar_source.is_none());
}

#[test]
fn topbar_source_is_absent_when_core_reports_the_source_unavailable() {
    let mut view = d1_view();
    view.apply_event(&RuntimeEvent::new(
        90,
        workspace_source("unavailable", 0, 0, false),
    ));

    let projection = connected(view)
        .d1_cockpit(Some("lane_d1_core"))
        .expect("D1 projection");
    // Zeroed counts from an unavailable sample would read as "clean and in
    // sync". The block is omitted instead.
    assert!(projection.topbar_source.is_none());
}

#[test]
fn a_truncated_sample_projects_its_partial_facts_behind_a_truncation_flag() {
    let mut view = d1_view();
    view.apply_event(&RuntimeEvent::new(
        90,
        workspace_source("truncated", 3, 0, true),
    ));

    let topbar = connected(view)
        .d1_cockpit(Some("lane_d1_core"))
        .expect("D1 projection")
        .topbar_source
        .expect("a truncated sample still carries the facts Core did publish");
    assert_eq!(topbar.status, "truncated");
    assert!(topbar.truncated);
    assert_eq!(topbar.ahead, 3);
}

#[test]
fn the_worktree_count_covers_distinct_worktrees_of_active_lanes_only() {
    let mut view = d1_view();
    view.apply_event(&RuntimeEvent::new(
        90,
        workspace_source("ready", 0, 0, false),
    ));
    // A second active Lane in its own worktree counts.
    view.apply_event(&RuntimeEvent::new(
        91,
        lane("lane_second", "running", Some(".worktrees/lane_second")),
    ));
    // A finished Lane does not: its worktree is no longer live work.
    view.apply_event(&RuntimeEvent::new(
        92,
        lane("lane_done", "done", Some(".worktrees/lane_done")),
    ));
    // An active Lane working directly in the workspace has no worktree.
    view.apply_event(&RuntimeEvent::new(93, lane("lane_direct", "running", None)));
    // Two active Lanes sharing one worktree are one worktree, not two: the
    // chip counts worktrees, which is what its label claims.
    view.apply_event(&RuntimeEvent::new(
        94,
        lane("lane_shared", "running", Some(".worktrees/lane_second")),
    ));

    let topbar = connected(view)
        .d1_cockpit(Some("lane_d1_core"))
        .expect("D1 projection")
        .topbar_source
        .expect("workspace source is published");
    // `lane_d1_core` plus `lane_second` (shared with `lane_shared`).
    assert_eq!(topbar.lane_worktree_count, 2);
}
