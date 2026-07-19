use std::process::Command;
use std::sync::Arc;

use super::{SequenceProvider, temp_dir};
use crate::{CredentialBackend, RuntimeSupervisor, SessionEngine};
use viden_types::{
    ApprovalResponse, CredentialHandle, CredentialStatus, ProjectConfigState, RuntimeCommand,
    RuntimeEventKind, WorkMode,
};

#[test]
fn project_runtime_probes_git_and_non_git_projects_with_provider_health() {
    let git_root = temp_dir("project_probe_git");
    assert!(
        Command::new("git")
            .arg("init")
            .arg(&git_root)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(
        git_root.join("viden.toml"),
        b"[project]\nname = \"demo\"\npack = \"robot-pack\"\n",
    )
    .unwrap();
    let mut engine =
        SessionEngine::new(&git_root, Box::new(SequenceProvider::new(Vec::new()))).unwrap();
    let events = engine
        .handle_runtime_command("probe-git", RuntimeCommand::ProbeProject, &mut |_| {
            ApprovalResponse::deny(None)
        })
        .unwrap();
    let probe = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::ProjectProbed { probe } => Some(probe),
            _ => None,
        })
        .expect("project probe event");
    assert!(probe.is_git_repository);
    assert_eq!(probe.config_state, ProjectConfigState::Valid);
    assert_eq!(probe.project_name.as_deref(), Some("demo"));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ProviderHealthUpdated { provider }
            if provider.provider_id == "sequence"
                && provider.model == "test-model"
                && provider.status == "ready"
    )));

    let plain_root = temp_dir("project_probe_plain");
    let mut plain_engine =
        SessionEngine::new(&plain_root, Box::new(SequenceProvider::new(Vec::new()))).unwrap();
    let events = plain_engine
        .handle_runtime_command("probe-plain", RuntimeCommand::ProbeProject, &mut |_| {
            ApprovalResponse::deny(None)
        })
        .unwrap();
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ProjectProbed { probe }
            if !probe.is_git_repository && probe.config_state == ProjectConfigState::Missing
    )));
}

#[test]
fn project_runtime_preview_is_read_only_and_confirm_writes_exact_reviewed_bytes() {
    let root = temp_dir("project_preview_confirm");
    let path = root.join("viden.toml");
    let mut engine =
        SessionEngine::new(&root, Box::new(SequenceProvider::new(Vec::new()))).unwrap();
    let preview_events = engine
        .handle_runtime_command(
            "preview",
            RuntimeCommand::PreviewProjectConfig {
                contents: "[project]\nname = \"demo\"\npack = \"robot-pack\"\n".to_string(),
            },
            &mut |_| ApprovalResponse::deny(None),
        )
        .unwrap();
    assert!(!path.exists(), "preview must not write viden.toml");
    let preview = preview_events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::ProjectConfigPreviewed { preview } => Some(preview.clone()),
            _ => None,
        })
        .expect("preview event");
    assert_eq!(
        preview.exact_contents.as_deref(),
        Some("[project]\nname = \"demo\"\npack = \"robot-pack\"\n")
    );

    let events = engine
        .handle_runtime_command(
            "confirm",
            RuntimeCommand::ConfirmProjectConfig {
                preview_id: preview.preview_id.clone(),
                content_sha256: preview.content_sha256.clone(),
            },
            &mut |_| ApprovalResponse::allow_once(None),
        )
        .unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"[project]\nname = \"demo\"\npack = \"robot-pack\"\n"
    );
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ProjectConfigConfirmed { preview: confirmed }
            if confirmed.content_sha256 == preview.content_sha256
    )));
}

#[test]
fn project_runtime_plan_denies_confirm_before_write() {
    let root = temp_dir("project_plan_denial");
    let path = root.join("viden.toml");
    let mut engine =
        SessionEngine::new(&root, Box::new(SequenceProvider::new(Vec::new()))).unwrap();
    let preview = engine
        .preview_project_config("[project]\nname = \"demo\"\npack = \"robot-pack\"\n".to_string())
        .expect("preview");
    engine.set_work_mode(WorkMode::Plan).unwrap();
    let mut approver_called = false;
    let events = engine
        .handle_runtime_command(
            "confirm-plan",
            RuntimeCommand::ConfirmProjectConfig {
                preview_id: preview.preview_id,
                content_sha256: preview.content_sha256,
            },
            &mut |_| {
                approver_called = true;
                ApprovalResponse::allow_once(None)
            },
        )
        .unwrap();
    assert!(!approver_called, "Plan denial must happen before approval");
    assert!(!path.exists(), "Plan denial must happen before write");
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::CommandRejected { reason, .. } if reason.contains("Plan")
    )));
}

#[test]
fn project_runtime_invalid_preview_and_stale_confirm_never_write_reviewed_target() {
    let root = temp_dir("project_invalid_stale");
    let path = root.join("viden.toml");
    let mut engine =
        SessionEngine::new(&root, Box::new(SequenceProvider::new(Vec::new()))).unwrap();
    let invalid = engine
        .preview_project_config("[project\n".to_string())
        .expect("structured invalid preview");
    assert!(!invalid.is_valid());
    assert!(invalid.exact_contents.is_none());
    assert!(!path.exists());

    let preview = engine
        .preview_project_config(
            "[project]\nname = \"reviewed\"\npack = \"robot-pack\"\n".to_string(),
        )
        .unwrap();
    std::fs::write(
        &path,
        b"[project]\nname = \"external\"\npack = \"robot-pack\"\n",
    )
    .unwrap();
    let events = engine
        .handle_runtime_command(
            "confirm-stale",
            RuntimeCommand::ConfirmProjectConfig {
                preview_id: preview.preview_id,
                content_sha256: preview.content_sha256,
            },
            &mut |_| ApprovalResponse::allow_once(None),
        )
        .unwrap();
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::CommandRejected { reason, .. }
            if reason.contains("changed after preview")
    )));
    assert!(std::fs::read_to_string(path).unwrap().contains("external"));
}

#[test]
fn project_runtime_confirm_rolls_back_exact_bytes_when_audit_append_fails() {
    let root = temp_dir("project_confirm_rollback");
    let path = root.join("viden.toml");
    let original = b"[project]\nname = \"original\"\npack = \"robot-pack\"\n";
    std::fs::write(&path, original).unwrap();
    let mut engine =
        SessionEngine::new(&root, Box::new(SequenceProvider::new(Vec::new()))).unwrap();
    let preview = engine
        .preview_project_config(
            "[project]\nname = \"reviewed\"\npack = \"robot-pack\"\n".to_string(),
        )
        .unwrap();
    engine.fail_next_workflow_append_for_test();
    let events = engine
        .handle_runtime_command(
            "confirm-audit-failure",
            RuntimeCommand::ConfirmProjectConfig {
                preview_id: preview.preview_id.clone(),
                content_sha256: preview.content_sha256.clone(),
            },
            &mut |_| ApprovalResponse::allow_once(None),
        )
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::CommandRejected { .. }))
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);

    let retried = engine
        .handle_runtime_command(
            "confirm-audit-retry",
            RuntimeCommand::ConfirmProjectConfig {
                preview_id: preview.preview_id,
                content_sha256: preview.content_sha256,
            },
            &mut |_| ApprovalResponse::allow_once(None),
        )
        .unwrap();
    assert!(
        retried
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ProjectConfigConfirmed { .. }))
    );
}

#[test]
fn project_runtime_supervisor_resumes_confirm_after_approval() {
    let root = temp_dir("project_supervisor_confirm");
    let path = root.join("viden.toml");
    let engine = SessionEngine::new(&root, Box::new(SequenceProvider::new(Vec::new()))).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    let contents = "[project]\nname = \"supervised\"\npack = \"robot-pack\"\n";
    supervisor
        .send_command(
            "preview-supervised",
            RuntimeCommand::PreviewProjectConfig {
                contents: contents.to_string(),
            },
        )
        .unwrap();
    let preview = (0..12)
        .find_map(|_| {
            supervisor
                .recv_event_timeout(std::time::Duration::from_secs(1))
                .and_then(|event| match event.kind {
                    RuntimeEventKind::ProjectConfigPreviewed { preview } => Some(preview),
                    _ => None,
                })
        })
        .expect("supervised preview event");
    supervisor
        .send_command(
            "confirm-supervised",
            RuntimeCommand::ConfirmProjectConfig {
                preview_id: preview.preview_id,
                content_sha256: preview.content_sha256,
            },
        )
        .unwrap();
    let request_id = (0..12)
        .find_map(|_| {
            supervisor
                .recv_event_timeout(std::time::Duration::from_secs(1))
                .and_then(|event| match event.kind {
                    RuntimeEventKind::ApprovalRequested { approval } => Some(approval.id),
                    _ => None,
                })
        })
        .expect("project confirmation approval");
    supervisor
        .send_command(
            "approve-supervised",
            RuntimeCommand::RespondToApproval {
                request_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    assert!((0..12).any(|_| {
        supervisor
            .recv_event_timeout(std::time::Duration::from_secs(1))
            .is_some_and(|event| {
                matches!(event.kind, RuntimeEventKind::ProjectConfigConfirmed { .. })
            })
    }));
    assert_eq!(std::fs::read_to_string(path).unwrap(), contents);
}

struct SeededCredentialBackend {
    secret: Vec<u8>,
}

impl CredentialBackend for SeededCredentialBackend {
    fn store(
        &self,
        provider_id: &str,
        backend_id: &str,
        credential_request_id: &str,
    ) -> Result<CredentialHandle, String> {
        assert!(
            !self.secret.is_empty(),
            "injected backend owns the secret bytes"
        );
        assert_eq!(credential_request_id, "ingress-1");
        Ok(CredentialHandle {
            provider_id: provider_id.to_string(),
            backend_id: backend_id.to_string(),
            status: CredentialStatus::Available,
        })
    }
}

#[test]
fn project_runtime_credential_secret_never_enters_command_event_transcript_or_audit_json() {
    let root = temp_dir("project_credential_redaction");
    let secret = "task11-super-secret-value";
    let backend = Arc::new(SeededCredentialBackend {
        secret: secret.as_bytes().to_vec(),
    });
    let mut engine = SessionEngine::new(&root, Box::new(SequenceProvider::new(Vec::new())))
        .unwrap()
        .with_credential_backend(backend);
    let command = RuntimeCommand::StoreCredentialHandle {
        provider_id: "sequence".to_string(),
        backend_id: "test-keychain:item-1".to_string(),
        credential_request_id: "ingress-1".to_string(),
    };
    assert!(!serde_json::to_string(&command).unwrap().contains(secret));
    let events = engine
        .handle_runtime_command("credential", command, &mut |_| {
            ApprovalResponse::allow_once(None)
        })
        .unwrap();
    assert!(!serde_json::to_string(&events).unwrap().contains(secret));

    let transcript_json = serde_json::to_string(
        &events
            .iter()
            .map(|event| viden_types::TranscriptRowKind::RuntimeEvent {
                event: Box::new(event.clone()),
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(!transcript_json.contains(secret));

    let audit_json = engine
        .workflow_store()
        .load_agent_events()
        .unwrap()
        .into_iter()
        .flat_map(|event| event.payload.into_values())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!audit_json.contains(secret));
    assert!(
        audit_json.contains("test-keychain:item-1"),
        "safe credential handle must remain auditable"
    );
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ProviderHealthUpdated { provider }
            if provider.credential.as_ref().is_some_and(|handle| {
                handle.provider_id == "sequence"
                    && handle.backend_id == "test-keychain:item-1"
                    && handle.status == CredentialStatus::Available
            })
    )));
}
