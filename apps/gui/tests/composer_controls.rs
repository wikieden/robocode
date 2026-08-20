//! Composer-control commands: work mode, permission level, and model
//! selection flow through the shared adapter boundary as their exact Core
//! commands, and the returned projection is always the snapshot Core
//! republished — never an optimistic client edit.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use viden_core::{
    CoreClientError, PermissionLevel, RuntimeCommand, RuntimeEventEnvelope, RuntimeEventKind,
    RuntimeSnapshot, RuntimeViewState, RuntimeWireEvent, WorkMode,
};
use viden_gui::{ComposerControlIntent, GuiCoreAdapter};

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

fn connected(client: TestCoreClient) -> GuiCoreAdapter {
    let mut adapter = GuiCoreAdapter::new(Box::new(client));
    adapter.connect().expect("connect composer-control client");
    adapter
}

fn sent_commands(
    sent: &Arc<Mutex<Vec<viden_core::RuntimeCommandEnvelope>>>,
) -> Vec<viden_core::RuntimeCommandEnvelope> {
    sent.lock().expect("sent commands").clone()
}

#[test]
fn set_work_mode_sends_the_exact_core_command_with_the_default_owner() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));

    let result = adapter
        .send_composer_control_and_wait(
            "gui-ctl-mode",
            ComposerControlIntent::SetWorkMode {
                mode: "plan".into(),
            },
            Some("lane_d1_core"),
            Duration::ZERO,
        )
        .expect("work-mode control dispatches");

    let commands = sent_commands(&sent);
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        commands[0].command,
        RuntimeCommand::SetWorkMode {
            mode: WorkMode::Plan
        }
    ));
    assert_eq!(commands[0].owner, viden_core::RuntimeOwner::default());
    assert_eq!(commands[0].command_id, "gui-ctl-mode");
    // No receipt was observed yet, so the command stays pending and the
    // projection still carries the last confirmed Core snapshot.
    assert_eq!(result.pending_command_id.as_deref(), Some("gui-ctl-mode"));
    assert_eq!(result.outcome.state, "pending");
    assert_eq!(result.projection.environment.work_mode, "build");
}

#[test]
fn set_permission_level_sends_the_exact_core_command() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));

    adapter
        .send_composer_control_and_wait(
            "gui-ctl-perm",
            ComposerControlIntent::SetPermissionLevel {
                level: "auto_edit".into(),
            },
            None,
            Duration::ZERO,
        )
        .expect("permission control dispatches");

    let commands = sent_commands(&sent);
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        commands[0].command,
        RuntimeCommand::SetPermissionLevel {
            level: PermissionLevel::AutoEdit
        }
    ));
}

#[test]
fn select_model_sends_the_exact_core_command_for_a_published_option() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    // The active provider's current model is a published option.
    let provider_event: RuntimeEventKind = serde_json::from_value(serde_json::json!({
        "type": "provider_health_updated",
        "payload": {
            "provider": {
                "provider_id": "deepseek",
                "model": "deepseek-v4-flash",
                "status": "ready",
                "request_count": 3,
                "error_count": 0,
                "last_latency_ms": 120,
                "average_latency_ms": 140,
                "tokens_per_second": 25
            }
        }
    }))
    .expect("typed provider event");
    let mut view = d1_view();
    view.apply_event(&viden_core::RuntimeEvent::new(90, provider_event));
    let mut adapter = connected(TestCoreClient::new(view, Arc::clone(&sent)));

    adapter
        .send_composer_control_and_wait(
            "gui-ctl-model",
            ComposerControlIntent::SelectModel {
                provider_id: "deepseek".into(),
                model: "deepseek-v4-flash".into(),
            },
            None,
            Duration::ZERO,
        )
        .expect("model control dispatches");

    let commands = sent_commands(&sent);
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0].command,
        RuntimeCommand::SelectModel { provider_id, model }
            if provider_id == "deepseek" && model == "deepseek-v4-flash"
    ));
}

#[test]
fn select_model_accepts_an_adapter_published_model_option() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let adapters_event: RuntimeEventKind = serde_json::from_value(serde_json::json!({
        "type": "agent_adapters_loaded",
        "payload": {
            "adapters": [{
                "agent_id": "codex-acp",
                "display_name": "Codex",
                "route": "acp",
                "source": "built_in",
                "availability": "available",
                "auth_state": "ready",
                "startability": "ready",
                "models": ["gpt-5.3-codex", "gpt-5.3-codex-mini"]
            }]
        }
    }))
    .expect("typed adapters event");
    let mut view = d1_view();
    view.apply_event(&viden_core::RuntimeEvent::new(91, adapters_event));
    let mut adapter = connected(TestCoreClient::new(view, Arc::clone(&sent)));

    adapter
        .send_composer_control_and_wait(
            "gui-ctl-model-acp",
            ComposerControlIntent::SelectModel {
                provider_id: "codex-acp".into(),
                model: "gpt-5.3-codex-mini".into(),
            },
            None,
            Duration::ZERO,
        )
        .expect("adapter-published model dispatches");

    let commands = sent_commands(&sent);
    assert!(matches!(
        &commands[0].command,
        RuntimeCommand::SelectModel { provider_id, model }
            if provider_id == "codex-acp" && model == "gpt-5.3-codex-mini"
    ));
}

#[test]
fn select_model_refuses_an_option_core_never_published() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));

    let error = adapter
        .send_composer_control_and_wait(
            "gui-ctl-model",
            ComposerControlIntent::SelectModel {
                provider_id: "deepseek".into(),
                model: "invented-model".into(),
            },
            None,
            Duration::ZERO,
        )
        .expect_err("an unpublished model is not a target");
    assert!(error.contains("invented-model"), "{error}");
    assert!(sent_commands(&sent).is_empty());
}

#[test]
fn unknown_mode_and_level_names_are_rejected_before_any_command_is_sent() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));

    let error = adapter
        .send_composer_control_and_wait(
            "gui-ctl-mode",
            ComposerControlIntent::SetWorkMode {
                mode: "turbo".into(),
            },
            None,
            Duration::ZERO,
        )
        .expect_err("unknown work mode fails closed");
    assert!(error.contains("turbo"), "{error}");

    let error = adapter
        .send_composer_control_and_wait(
            "gui-ctl-perm",
            ComposerControlIntent::SetPermissionLevel {
                level: "yolo".into(),
            },
            None,
            Duration::ZERO,
        )
        .expect_err("unknown permission level fails closed");
    assert!(error.contains("yolo"), "{error}");
    assert!(sent_commands(&sent).is_empty());
}

#[test]
fn a_rejected_transport_send_surfaces_an_error() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let client = TestCoreClient::new(d1_view(), Arc::clone(&sent))
        .with_send_error(CoreClientError::Transport("core pipe closed".into()));
    let mut adapter = connected(client);

    let error = adapter
        .send_composer_control_and_wait(
            "gui-ctl-mode",
            ComposerControlIntent::SetWorkMode {
                mode: "plan".into(),
            },
            None,
            Duration::ZERO,
        )
        .expect_err("a failed transport send is an error");
    assert!(error.contains("core pipe closed"), "{error}");
}

#[test]
fn a_core_rejection_becomes_a_rejected_outcome() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let adapters_event: RuntimeEventKind = serde_json::from_value(serde_json::json!({
        "type": "agent_adapters_loaded",
        "payload": {
            "adapters": [{
                "agent_id": "codex-acp",
                "display_name": "Codex",
                "route": "acp",
                "source": "built_in",
                "availability": "available",
                "auth_state": "ready",
                "startability": "ready",
                "models": ["gpt-5.3-codex"]
            }]
        }
    }))
    .expect("typed adapters event");
    // Publish the adapter option through the canonical reducer path, then let
    // Core reject the command (SelectModel only accepts the active provider).
    let mut view = d1_view();
    view.apply_event(&viden_core::RuntimeEvent::new(91, adapters_event));
    let client = TestCoreClient::new(view, Arc::clone(&sent)).with_event(
        RuntimeEventKind::CommandRejected {
            command_id: "gui-ctl-model".into(),
            reason: "provider `codex-acp` is not the active provider `deepseek`".into(),
        },
    );
    let mut adapter = connected(client);

    let result = adapter
        .send_composer_control_and_wait(
            "gui-ctl-model",
            ComposerControlIntent::SelectModel {
                provider_id: "codex-acp".into(),
                model: "gpt-5.3-codex".into(),
            },
            None,
            Duration::from_millis(10),
        )
        .expect("rejection is a projected outcome, not a transport error");

    assert_eq!(result.outcome.state, "rejected");
    assert_eq!(
        result.outcome.reason.as_deref(),
        Some("provider `codex-acp` is not the active provider `deepseek`")
    );
    assert_eq!(result.pending_command_id, None);
}

#[test]
fn coupled_mode_and_permission_changes_render_the_snapshot_core_published() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let view = d1_view();
    // Core's coupling rule turned Plan mode into ReadOnly permission; the
    // adapter must render that republished snapshot, never the local click.
    let mut coupled = view.snapshot.clone();
    coupled.work_mode = WorkMode::Plan;
    coupled.permission_level = PermissionLevel::ReadOnly;
    let client = TestCoreClient::new(view, Arc::clone(&sent))
        .with_event(RuntimeEventKind::CommandAccepted {
            command_id: "gui-ctl-mode".into(),
            command: RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
        })
        .with_event(RuntimeEventKind::SnapshotUpdated { snapshot: coupled });
    let mut adapter = connected(client);

    let result = adapter
        .send_composer_control_and_wait(
            "gui-ctl-mode",
            ComposerControlIntent::SetWorkMode {
                mode: "plan".into(),
            },
            Some("lane_d1_core"),
            Duration::from_millis(10),
        )
        .expect("accepted control confirms through the snapshot receipt");

    assert_eq!(result.outcome.state, "confirmed");
    assert_eq!(result.pending_command_id, None);
    assert_eq!(result.projection.environment.work_mode, "plan");
    assert_eq!(result.projection.environment.permission_level, "read_only");
    assert_eq!(result.projection.statusbar.work_mode, "plan");
    assert_eq!(result.projection.statusbar.permission_level, "read_only");
}
