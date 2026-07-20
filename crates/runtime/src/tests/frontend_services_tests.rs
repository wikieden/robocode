use super::*;
use crate::{RuntimeSupervisor, SessionEngine};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use viden_config::CliOverrides;
use viden_types::{
    AgentRole, ApprovalResponse, EventCursor, LocaleId, Message, RecentWorkQuery, ReplayRequest,
    Role, RuntimeCommand, RuntimeEvent, RuntimeEventEnvelope, RuntimeEventKind, RuntimeOwner,
    RuntimeWireEvent, SessionMetaEntry, StarterLanePreset, StarterLanePreviewInvalidationReason,
    StarterLaneRequest, TranscriptEntry, UiColorMode, UiDensity, UiMotion, UiPreferencePatch,
    UiPreferences, UiSkin, WorkMode,
};

use crate::lane_runtime::{
    LaneEffectExecutor, LaneEffectRequest, LaneEffectResult, LocalLaneEffectExecutor,
};
use crate::lane_supervisor::LanePersistence;
use viden_workflows::lanes::LaneEvent;

fn append_recent_runtime_fixture(home: &Path, root: &Path, session_id: &str, timestamp: u64) {
    let root = root.canonicalize().unwrap();
    let store =
        viden_session::SessionStore::new_with_home(home, &root, Some(session_id.to_string()))
            .unwrap();
    store
        .append_entries_atomic(&[
            TranscriptEntry::SessionMeta {
                entry: SessionMetaEntry {
                    timestamp,
                    key: "canonical_root".to_string(),
                    value: root.display().to_string(),
                },
            },
            TranscriptEntry::SessionMeta {
                entry: SessionMetaEntry {
                    timestamp,
                    key: "session_created_at".to_string(),
                    value: timestamp.to_string(),
                },
            },
            TranscriptEntry::Message {
                message: Message {
                    id: format!("message-{session_id}"),
                    role: Role::User,
                    content: "sk-secret-runtime-body".to_string(),
                    timestamp: timestamp + 1,
                    tool_name: None,
                    tool_call_id: None,
                },
            },
        ])
        .unwrap();
}

#[derive(Default)]
struct StarterLaneEffects {
    calls: Mutex<Vec<String>>,
}

impl LaneEffectExecutor for StarterLaneEffects {
    fn execute(&self, request: LaneEffectRequest) -> Result<LaneEffectResult, String> {
        let LaneEffectRequest::Create { lane, .. } = request else {
            panic!("starter lane test received a non-create effect");
        };
        self.calls.lock().unwrap().push(lane.id);
        Ok(LaneEffectResult::success("starter lane created"))
    }
}

fn starter_lane_repo(name: &str) -> PathBuf {
    let repo = temp_dir(name);
    run_git(&repo, &["init", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "viden@example.com"]);
    run_git(&repo, &["config", "user.name", "Viden Test"]);
    fs::write(repo.join("README.md"), "starter\n").unwrap();
    run_git(&repo, &["add", "README.md"]);
    run_git(&repo, &["commit", "-m", "initial"]);
    repo
}

fn run_git(repo: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn starter_request(lane_id: &str, preset: StarterLanePreset) -> StarterLaneRequest {
    StarterLaneRequest {
        lane_id: lane_id.to_string(),
        preset,
        branch: None,
        worktree_path: None,
    }
}

fn starter_owner(lane_id: &str) -> RuntimeOwner {
    RuntimeOwner {
        workspace_id: "workspace-starter".to_string(),
        project_id: "project-starter".to_string(),
        lane_id: Some(lane_id.to_string()),
        session_id: Some(format!("session-{lane_id}")),
        task_id: None,
        turn_id: Some(format!("turn-{lane_id}")),
    }
}

fn starter_supervisor(
    repo: &Path,
    home: PathBuf,
    effects: Arc<StarterLaneEffects>,
) -> RuntimeSupervisor {
    let provider: Box<dyn ModelProvider> = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(repo, provider, Some(home)).unwrap();
    engine
        .set_permission_mode(viden_types::PermissionMode::DontAsk)
        .unwrap();
    RuntimeSupervisor::start_with_lane_effects_for_test(
        engine,
        effects as Arc<dyn LaneEffectExecutor>,
    )
}

fn collect_starter_envelopes_until(
    supervisor: &RuntimeSupervisor,
    done: impl Fn(&[RuntimeEventEnvelope]) -> bool,
) -> Vec<RuntimeEventEnvelope> {
    let started = Instant::now();
    let mut events = Vec::new();
    while started.elapsed() < Duration::from_secs(10) {
        if let Some(event) = supervisor.recv_event_envelope_timeout(Duration::from_millis(50)) {
            events.push(event);
            if done(&events) {
                return events;
            }
        }
    }
    panic!("timed out waiting for starter lane events: {events:#?}");
}

#[test]
fn starter_lane_preview_resolves_presets_without_git_workflow_or_effect_mutation() {
    let repo = starter_lane_repo("starter_lane_preview_repo");
    let home = temp_dir("starter_lane_preview_home");
    let effects = Arc::new(StarterLaneEffects::default());
    let supervisor = starter_supervisor(&repo, home.clone(), Arc::clone(&effects));
    let base_revision = run_git(&repo, &["rev-parse", "HEAD"]);

    for (preset, role, lane_id) in [
        (StarterLanePreset::Coder, AgentRole::Coder, "starter-coder"),
        (
            StarterLanePreset::Reviewer,
            AgentRole::Reviewer,
            "starter-reviewer",
        ),
        (
            StarterLanePreset::Tester,
            AgentRole::Tester,
            "starter-tester",
        ),
    ] {
        let owner = starter_owner(lane_id);
        supervisor
            .send_command_from_owner(
                owner.clone(),
                format!("preview-{lane_id}"),
                RuntimeCommand::PreviewStarterLane {
                    request: starter_request(lane_id, preset),
                },
            )
            .unwrap();
        let events = collect_starter_envelopes_until(&supervisor, |events| {
            events.iter().any(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::StarterLanePreviewed { preview },
                        ..
                    }) if preview.lane.id == lane_id
                )
            })
        });
        let (event_owner, preview) = events
            .iter()
            .find_map(|envelope| match &envelope.event {
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { preview },
                    ..
                }) if preview.lane.id == lane_id => Some((&envelope.owner, preview)),
                _ => None,
            })
            .unwrap();
        assert_eq!(event_owner, &owner);
        assert_eq!(&preview.owner, &owner);
        assert_eq!(preview.lane.role, role);
        assert_eq!(preview.branch, format!("codex/{lane_id}"));
        assert_eq!(
            preview.worktree_path,
            repo.canonicalize()
                .unwrap()
                .join(".worktrees")
                .join(lane_id)
                .display()
                .to_string()
        );
        assert_eq!(preview.base_revision, base_revision);
        assert_eq!(preview.content_sha256.len(), 64);
        assert!(!preview.preview_id.is_empty());
        assert!(preview.diagnostics.is_empty());
        assert!(!Path::new(&preview.worktree_path).exists());
    }

    assert!(effects.calls.lock().unwrap().is_empty());
    assert!(
        viden_workflows::stores::WorkflowStore::new(home, &repo)
            .unwrap()
            .load_lane_events()
            .unwrap()
            .is_empty()
    );
    assert_eq!(run_git(&repo, &["branch", "--list", "codex/*"]), "");
}

#[test]
fn starter_lane_create_waits_for_permission_and_emits_exact_owner_receipt() {
    let repo = starter_lane_repo("starter_lane_create_repo");
    let home = temp_dir("starter_lane_create_home");
    let effects = Arc::new(StarterLaneEffects::default());
    let supervisor = starter_supervisor(&repo, home, Arc::clone(&effects));
    let request = starter_request("starter-create", StarterLanePreset::Coder);
    let owner = starter_owner(&request.lane_id);
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "starter-preview",
            RuntimeCommand::PreviewStarterLane {
                request: request.clone(),
            },
        )
        .unwrap();
    let preview_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { .. },
                    ..
                })
            )
        })
    });
    let preview = preview_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) => Some(preview.clone()),
            _ => None,
        })
        .unwrap();

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "starter-create",
            RuntimeCommand::CreateStarterLane {
                request,
                preview_id: preview.preview_id.clone(),
                content_sha256: preview.content_sha256.clone(),
            },
        )
        .unwrap();
    let approval_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { approval },
                    ..
                }) if approval.tool_name == "lane_create"
            )
        })
    });
    assert!(effects.calls.lock().unwrap().is_empty());
    let approval_id = approval_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalRequested { approval },
                ..
            }) => Some(approval.id.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "starter-approve",
            RuntimeCommand::RespondToApproval {
                request_id: approval_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    let created_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLaneCreated { .. },
                    ..
                })
            )
        })
    });
    let (event_owner, receipt) = created_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLaneCreated { receipt },
                ..
            }) => Some((&envelope.owner, receipt)),
            _ => None,
        })
        .unwrap();
    assert_eq!(event_owner, &owner);
    assert_eq!(receipt.owner, owner);
    assert_eq!(receipt.preview_id, preview.preview_id);
    assert_eq!(receipt.content_sha256, preview.content_sha256);
    assert_eq!(receipt.base_revision, preview.base_revision);
    assert_eq!(receipt.lane.id, "starter-create");
    assert_eq!(
        receipt.lane.active_session_ids,
        vec!["session-starter-create"]
    );
    let completed = supervisor.snapshot_envelope().unwrap();
    assert!(completed.view.starter_lane_previews.is_empty());
    assert_eq!(completed.view.starter_lane_receipts, vec![receipt.clone()]);
    assert_eq!(effects.calls.lock().unwrap().as_slice(), ["starter-create"]);
}

#[test]
fn starter_lane_pending_creation_cannot_be_replaced_by_a_second_preview() {
    let repo = starter_lane_repo("starter_lane_pending_duplicate_repo");
    let home = temp_dir("starter_lane_pending_duplicate_home");
    let effects = Arc::new(StarterLaneEffects::default());
    let supervisor = starter_supervisor(&repo, home, Arc::clone(&effects));
    let request = starter_request("starter-pending", StarterLanePreset::Coder);
    let owner = starter_owner(&request.lane_id);

    let preview = |command_id: &str| {
        supervisor
            .send_command_from_owner(
                owner.clone(),
                command_id,
                RuntimeCommand::PreviewStarterLane {
                    request: request.clone(),
                },
            )
            .unwrap();
        collect_starter_envelopes_until(&supervisor, |events| {
            events.iter().any(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::StarterLanePreviewed { .. },
                        ..
                    })
                )
            })
        })
        .into_iter()
        .find_map(|envelope| match envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) => Some(preview),
            _ => None,
        })
        .unwrap()
    };
    let first = preview("preview-pending-first");
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "create-pending-first",
            RuntimeCommand::CreateStarterLane {
                request: request.clone(),
                preview_id: first.preview_id.clone(),
                content_sha256: first.content_sha256.clone(),
            },
        )
        .unwrap();
    let approval_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { .. },
                    ..
                })
            )
        })
    });
    let approval_id = approval_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalRequested { approval },
                ..
            }) => Some(approval.id.clone()),
            _ => None,
        })
        .unwrap();

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "create-pending-legacy",
            RuntimeCommand::CreateLane {
                lane: first.lane.clone(),
            },
        )
        .unwrap();
    let legacy_rejected = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandRejected { command_id, .. },
                    ..
                }) if command_id == "create-pending-legacy"
            )
        })
    });
    assert!(!legacy_rejected.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewInvalidated { preview_id, .. },
                ..
            }) if preview_id == &first.preview_id
        )
    }));
    assert!(
        supervisor
            .snapshot_envelope()
            .unwrap()
            .view
            .starter_lane_previews
            .iter()
            .any(|preview| preview.preview_id == first.preview_id)
    );
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "start-pending-early",
            RuntimeCommand::StartLane {
                lane_id: request.lane_id.clone(),
                command: "worker".to_string(),
                args: Vec::new(),
                env: Vec::new(),
                output_log: None,
            },
        )
        .unwrap();
    let start_rejected = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandRejected { command_id, .. },
                    ..
                }) if command_id == "start-pending-early"
            )
        })
    });
    assert!(!start_rejected.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewInvalidated { preview_id, .. },
                ..
            }) if preview_id == &first.preview_id
        )
    }));
    assert!(
        supervisor
            .snapshot_envelope()
            .unwrap()
            .view
            .starter_lane_previews
            .iter()
            .any(|preview| preview.preview_id == first.preview_id)
    );

    let second = preview("preview-pending-second");
    assert_ne!(first.preview_id, second.preview_id);
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "create-pending-second",
            RuntimeCommand::CreateStarterLane {
                request,
                preview_id: second.preview_id.clone(),
                content_sha256: second.content_sha256,
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "approve-pending-first",
            RuntimeCommand::RespondToApproval {
                request_id: approval_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    let completed = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLaneCreated { .. },
                    ..
                })
            )
        })
    });
    assert!(completed.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::CommandRejected { command_id, .. },
                ..
            }) if command_id == "create-pending-second"
        )
    }));
    assert!(completed.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewInvalidated {
                    preview_id,
                    reason: StarterLanePreviewInvalidationReason::LaneAlreadyRegistered,
                    ..
                },
                ..
            }) if preview_id == &second.preview_id
        )
    }));
    assert!(completed.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLaneCreated { receipt },
                ..
            }) if receipt.preview_id == first.preview_id && receipt.owner == owner
        )
    }));
    assert_eq!(
        effects.calls.lock().unwrap().as_slice(),
        ["starter-pending"]
    );
}

#[test]
fn starter_lane_pending_creation_cancel_is_a_clean_denial_without_recovery_or_effect() {
    let repo = starter_lane_repo("starter_lane_pending_cancel_repo");
    let home = temp_dir("starter_lane_pending_cancel_home");
    let effects = Arc::new(StarterLaneEffects::default());
    let supervisor = starter_supervisor(&repo, home.clone(), Arc::clone(&effects));
    let request = starter_request("starter-cancel", StarterLanePreset::Tester);
    let owner = starter_owner(&request.lane_id);
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "preview-cancel",
            RuntimeCommand::PreviewStarterLane {
                request: request.clone(),
            },
        )
        .unwrap();
    let preview_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { .. },
                    ..
                })
            )
        })
    });
    let preview = preview_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) => Some(preview.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "create-cancel",
            RuntimeCommand::CreateStarterLane {
                request,
                preview_id: preview.preview_id.clone(),
                content_sha256: preview.content_sha256,
            },
        )
        .unwrap();
    collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { .. },
                    ..
                })
            )
        })
    });
    supervisor
        .send_command_from_owner(
            owner,
            "cancel-pending-starter",
            RuntimeCommand::CancelActiveTurn,
        )
        .unwrap();
    let cancelled = collect_starter_envelopes_until(&supervisor, |events| {
        let invalidated = events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewInvalidated {
                        preview_id,
                        reason: StarterLanePreviewInvalidationReason::PermissionDenied,
                        ..
                    },
                    ..
                }) if preview_id == &preview.preview_id
            )
        });
        let accepted = events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandAccepted {
                        command_id,
                        command: RuntimeCommand::CancelActiveTurn,
                    },
                    ..
                }) if command_id == "cancel-pending-starter"
            )
        });
        invalidated && accepted
    });
    assert!(cancelled.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalResolved {
                    decision: viden_types::ApprovalDecision::Deny,
                    ..
                },
                ..
            })
        )
    }));
    assert!(!cancelled.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::LaneUpdated { .. }
                    | RuntimeEventKind::LaneRecoveryRequired { .. }
                    | RuntimeEventKind::Error { .. }
                    | RuntimeEventKind::StarterLaneCreated { .. },
                ..
            })
        )
    }));
    assert!(effects.calls.lock().unwrap().is_empty());
    assert!(
        viden_workflows::stores::WorkflowStore::new(home, &repo)
            .unwrap()
            .load_lane_events()
            .unwrap()
            .is_empty()
    );
    assert!(
        supervisor
            .snapshot_envelope()
            .unwrap()
            .view
            .starter_lane_previews
            .is_empty()
    );
}

#[test]
fn starter_lane_create_rejects_changed_request_stale_base_and_duplicate_worktree_without_effects() {
    for scenario in ["changed", "stale", "duplicate", "branch"] {
        let repo = starter_lane_repo(&format!("starter_lane_reject_{scenario}_repo"));
        let home = temp_dir(&format!("starter_lane_reject_{scenario}_home"));
        let effects = Arc::new(StarterLaneEffects::default());
        let supervisor = starter_supervisor(&repo, home.clone(), Arc::clone(&effects));
        let lane_id = format!("starter-{scenario}");
        let request = starter_request(&lane_id, StarterLanePreset::Coder);
        let owner = starter_owner(&lane_id);
        supervisor
            .send_command_from_owner(
                owner.clone(),
                format!("preview-{scenario}"),
                RuntimeCommand::PreviewStarterLane {
                    request: request.clone(),
                },
            )
            .unwrap();
        let preview_events = collect_starter_envelopes_until(&supervisor, |events| {
            events.iter().any(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::StarterLanePreviewed { .. },
                        ..
                    })
                )
            })
        });
        let preview = preview_events
            .iter()
            .find_map(|envelope| match &envelope.event {
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { preview },
                    ..
                }) => Some(preview.clone()),
                _ => None,
            })
            .unwrap();

        let create_request = match scenario {
            "changed" => StarterLaneRequest {
                branch: Some(format!("codex/{lane_id}-changed")),
                ..request
            },
            "stale" => {
                fs::write(repo.join("README.md"), "changed\n").unwrap();
                run_git(&repo, &["add", "README.md"]);
                run_git(&repo, &["commit", "-m", "advance base"]);
                request
            }
            "duplicate" => {
                fs::create_dir_all(&preview.worktree_path).unwrap();
                request
            }
            "branch" => {
                run_git(&repo, &["branch", &preview.branch]);
                request
            }
            _ => unreachable!(),
        };
        supervisor
            .send_command_from_owner(
                owner,
                format!("create-{scenario}"),
                RuntimeCommand::CreateStarterLane {
                    request: create_request,
                    preview_id: preview.preview_id.clone(),
                    content_sha256: preview.content_sha256.clone(),
                },
            )
            .unwrap();
        let expected_invalidation = match scenario {
            "changed" => StarterLanePreviewInvalidationReason::RequestChanged,
            "stale" => StarterLanePreviewInvalidationReason::BaseRevisionChanged,
            "duplicate" => StarterLanePreviewInvalidationReason::WorktreeUnavailable,
            "branch" => StarterLanePreviewInvalidationReason::BranchUnavailable,
            _ => unreachable!(),
        };
        let rejected = collect_starter_envelopes_until(&supervisor, |events| {
            let rejected = events.iter().any(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::CommandRejected { .. },
                        ..
                    })
                )
            });
            let invalidated = events.iter().any(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::StarterLanePreviewInvalidated { reason, .. },
                        ..
                    }) if *reason == expected_invalidation
                )
            });
            rejected && invalidated
        });
        assert!(rejected.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandRejected { reason, .. },
                    ..
                }) if reason.contains(scenario)
                    || (scenario == "stale" && reason.contains("base revision"))
                    || (scenario == "duplicate" && reason.contains("worktree"))
                    || (scenario == "branch" && reason.contains("branch"))
            )
        }));
        assert!(effects.calls.lock().unwrap().is_empty());
        assert!(
            viden_workflows::stores::WorkflowStore::new(home, &repo)
                .unwrap()
                .load_lane_events()
                .unwrap()
                .is_empty()
        );
        assert!(
            supervisor
                .snapshot_envelope()
                .unwrap()
                .view
                .starter_lane_previews
                .is_empty()
        );
        assert!(!rejected.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { .. }
                        | RuntimeEventKind::StarterLaneCreated { .. },
                    ..
                })
            )
        }));
    }
}

#[test]
fn starter_lane_create_plan_mode_denies_before_approval_and_consumes_preview() {
    let repo = starter_lane_repo("starter_lane_plan_repo");
    let home = temp_dir("starter_lane_plan_home");
    let effects = Arc::new(StarterLaneEffects::default());
    let provider: Box<dyn ModelProvider> = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&repo, provider, Some(home)).unwrap();
    engine.set_work_mode(WorkMode::Plan).unwrap();
    let supervisor = RuntimeSupervisor::start_with_lane_effects_for_test(
        engine,
        effects.clone() as Arc<dyn LaneEffectExecutor>,
    );
    let request = starter_request("starter-plan", StarterLanePreset::Tester);
    let owner = starter_owner(&request.lane_id);
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "preview-plan",
            RuntimeCommand::PreviewStarterLane {
                request: request.clone(),
            },
        )
        .unwrap();
    let preview_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { .. },
                    ..
                })
            )
        })
    });
    let preview = preview_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) => Some(preview.clone()),
            _ => None,
        })
        .unwrap();

    supervisor
        .send_command_from_owner(
            owner,
            "create-plan",
            RuntimeCommand::CreateStarterLane {
                request,
                preview_id: preview.preview_id,
                content_sha256: preview.content_sha256,
            },
        )
        .unwrap();
    let rejected = collect_starter_envelopes_until(&supervisor, |events| {
        let rejected = events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandRejected { .. },
                    ..
                })
            )
        });
        let invalidated = events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewInvalidated {
                        reason: StarterLanePreviewInvalidationReason::PlanModeDenied,
                        ..
                    },
                    ..
                })
            )
        });
        rejected && invalidated
    });
    assert!(rejected.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::CommandRejected { reason, .. },
                ..
            }) if reason.contains("Plan mode")
        )
    }));
    assert!(effects.calls.lock().unwrap().is_empty());
    assert!(
        supervisor
            .snapshot_envelope()
            .unwrap()
            .view
            .starter_lane_previews
            .is_empty()
    );
}

#[test]
fn starter_lane_create_rejects_unknown_hash_and_wrong_owner_as_one_time_preview_failures() {
    for scenario in ["unknown", "hash", "owner"] {
        let repo = starter_lane_repo(&format!("starter_lane_identity_{scenario}_repo"));
        let home = temp_dir(&format!("starter_lane_identity_{scenario}_home"));
        let effects = Arc::new(StarterLaneEffects::default());
        let supervisor = starter_supervisor(&repo, home, Arc::clone(&effects));
        let lane_id = format!("starter-{scenario}");
        let request = starter_request(&lane_id, StarterLanePreset::Reviewer);
        let owner = starter_owner(&lane_id);
        supervisor
            .send_command_from_owner(
                owner.clone(),
                format!("preview-{scenario}"),
                RuntimeCommand::PreviewStarterLane {
                    request: request.clone(),
                },
            )
            .unwrap();
        let preview_events = collect_starter_envelopes_until(&supervisor, |events| {
            events.iter().any(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::StarterLanePreviewed { .. },
                        ..
                    })
                )
            })
        });
        let preview = preview_events
            .iter()
            .find_map(|envelope| match &envelope.event {
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { preview },
                    ..
                }) => Some(preview.clone()),
                _ => None,
            })
            .unwrap();
        let (attempt_owner, preview_id, hash) = match scenario {
            "unknown" => (
                owner.clone(),
                "missing-preview".to_string(),
                preview.content_sha256.clone(),
            ),
            "hash" => (owner.clone(), preview.preview_id.clone(), "00".repeat(32)),
            "owner" => {
                let mut wrong = owner.clone();
                wrong.workspace_id = "wrong-workspace".to_string();
                (
                    wrong,
                    preview.preview_id.clone(),
                    preview.content_sha256.clone(),
                )
            }
            _ => unreachable!(),
        };
        supervisor
            .send_command_from_owner(
                attempt_owner,
                format!("create-{scenario}"),
                RuntimeCommand::CreateStarterLane {
                    request: request.clone(),
                    preview_id,
                    content_sha256: hash,
                },
            )
            .unwrap();
        let rejected = collect_starter_envelopes_until(&supervisor, |events| {
            events.iter().any(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::CommandRejected { .. },
                        ..
                    })
                )
            })
        });
        assert!(rejected.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandRejected { reason, .. },
                    ..
                }) if reason.contains(scenario) || (scenario == "unknown" && reason.contains("preview"))
            )
        }));
        assert!(effects.calls.lock().unwrap().is_empty());
        let snapshot = supervisor.snapshot_envelope().unwrap();
        assert_eq!(
            snapshot.view.starter_lane_previews.len(),
            usize::from(scenario != "hash")
        );
        assert_eq!(
            rejected.iter().any(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::StarterLanePreviewInvalidated {
                            reason: StarterLanePreviewInvalidationReason::HashMismatch,
                            ..
                        },
                        ..
                    })
                )
            }),
            scenario == "hash"
        );

        if scenario == "owner" {
            supervisor
                .send_command_from_owner(
                    owner.clone(),
                    "create-owner-after-mismatch",
                    RuntimeCommand::CreateStarterLane {
                        request,
                        preview_id: preview.preview_id,
                        content_sha256: preview.content_sha256,
                    },
                )
                .unwrap();
            let approval = collect_starter_envelopes_until(&supervisor, |events| {
                events.iter().any(|envelope| {
                    matches!(
                        &envelope.event,
                        RuntimeWireEvent::Known(RuntimeEvent {
                            kind: RuntimeEventKind::ApprovalRequested { .. },
                            ..
                        })
                    )
                })
            });
            let approval_id = approval
                .iter()
                .find_map(|envelope| match &envelope.event {
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::ApprovalRequested { approval },
                        ..
                    }) => Some(approval.id.clone()),
                    _ => None,
                })
                .unwrap();
            supervisor
                .send_command_from_owner(
                    owner,
                    "deny-owner-after-mismatch",
                    RuntimeCommand::RespondToApproval {
                        request_id: approval_id,
                        response: ApprovalResponse::deny(None),
                    },
                )
                .unwrap();
            collect_starter_envelopes_until(&supervisor, |events| {
                events.iter().any(|envelope| {
                    matches!(
                        &envelope.event,
                        RuntimeWireEvent::Known(RuntimeEvent {
                            kind: RuntimeEventKind::StarterLanePreviewInvalidated {
                                reason: StarterLanePreviewInvalidationReason::PermissionDenied,
                                ..
                            },
                            ..
                        })
                    )
                })
            });
        }
    }
}

#[test]
fn starter_lane_previews_are_owner_scoped_interleaved_and_replayable_in_process() {
    let repo = starter_lane_repo("starter_lane_multi_owner_repo");
    let home = temp_dir("starter_lane_multi_owner_home");
    let effects = Arc::new(StarterLaneEffects::default());
    let supervisor = starter_supervisor(&repo, home, Arc::clone(&effects));
    let initial_cursor = supervisor.snapshot_envelope().unwrap().cursor;
    let request_a = starter_request("starter-owner-a", StarterLanePreset::Coder);
    let request_b = starter_request("starter-owner-b", StarterLanePreset::Reviewer);
    let owner_a = starter_owner(&request_a.lane_id);
    let owner_b = starter_owner(&request_b.lane_id);
    for (command_id, owner, request) in [
        ("preview-owner-a", owner_a.clone(), request_a.clone()),
        ("preview-owner-b", owner_b.clone(), request_b.clone()),
    ] {
        supervisor
            .send_command_from_owner(
                owner,
                command_id,
                RuntimeCommand::PreviewStarterLane { request },
            )
            .unwrap();
    }
    let preview_events = collect_starter_envelopes_until(&supervisor, |events| {
        events
            .iter()
            .filter(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::StarterLanePreviewed { .. },
                        ..
                    })
                )
            })
            .count()
            == 2
    });
    let preview_a = preview_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) if preview.owner == owner_a => Some(preview.clone()),
            _ => None,
        })
        .unwrap();
    let preview_b = preview_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) if preview.owner == owner_b => Some(preview.clone()),
            _ => None,
        })
        .unwrap();
    let preview_a_id = preview_a.preview_id.clone();
    let preview_b_id = preview_b.preview_id.clone();
    let snapshot = supervisor.snapshot_envelope().unwrap();
    assert_eq!(snapshot.view.starter_lane_previews.len(), 2);
    let serialized_snapshot = serde_json::to_string(&snapshot.view).unwrap();
    assert!(serialized_snapshot.contains("starter_lane_previews"));
    assert!(serialized_snapshot.contains(&preview_a.preview_id));
    assert!(serialized_snapshot.contains(&preview_b.preview_id));
    let preview_replay = supervisor
        .replay_events(ReplayRequest {
            after: initial_cursor.clone(),
            limit: 100,
        })
        .unwrap();
    assert!(preview_replay.events.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) if preview.owner == owner_a
        )
    }));
    let replay_json = serde_json::to_string(&preview_replay).unwrap();
    assert!(replay_json.contains(&preview_a.preview_id));
    assert!(replay_json.contains(&preview_b.preview_id));

    supervisor
        .send_command_from_owner(
            owner_a.clone(),
            "reject-owner-a",
            RuntimeCommand::CreateStarterLane {
                request: request_a,
                preview_id: preview_a.preview_id,
                content_sha256: "00".repeat(32),
            },
        )
        .unwrap();
    let rejected_a = collect_starter_envelopes_until(&supervisor, |events| {
        let rejected = events.iter().any(|envelope| matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::CommandRejected { command_id, .. }, .. }) if command_id == "reject-owner-a"));
        let invalidated = events.iter().any(|envelope| matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::StarterLanePreviewInvalidated { owner, preview_id, reason: StarterLanePreviewInvalidationReason::HashMismatch }, .. }) if owner == &owner_a && preview_id == &preview_a_id));
        rejected && invalidated
    });
    assert!(rejected_a.iter().all(|envelope| {
        !matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::StarterLanePreviewInvalidated { owner, preview_id, .. }, .. }) if owner == &owner_b || preview_id == &preview_b_id)
    }));
    let after_a = supervisor.snapshot_envelope().unwrap();
    assert_eq!(after_a.view.starter_lane_previews.len(), 1);
    assert_eq!(after_a.view.starter_lane_previews[0].owner, owner_b);

    supervisor
        .send_command_from_owner(
            owner_b.clone(),
            "create-owner-b",
            RuntimeCommand::CreateStarterLane {
                request: request_b,
                preview_id: preview_b.preview_id,
                content_sha256: preview_b.content_sha256,
            },
        )
        .unwrap();
    let approval_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            envelope.owner == owner_b
                && matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::ApprovalRequested { .. },
                        ..
                    })
                )
        })
    });
    let approval_id = approval_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalRequested { approval },
                ..
            }) if envelope.owner == owner_b => Some(approval.id.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner_b.clone(),
            "approve-owner-b",
            RuntimeCommand::RespondToApproval {
                request_id: approval_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    let created = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| envelope.owner == owner_b && matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::StarterLaneCreated { receipt }, .. }) if receipt.owner == owner_b))
    });
    assert!(created.iter().any(|envelope| envelope.owner == owner_b && matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::StarterLaneCreated { receipt }, .. }) if receipt.lane.id == "starter-owner-b")));
    let completed_replay = supervisor
        .replay_events(ReplayRequest {
            after: initial_cursor,
            limit: 100,
        })
        .unwrap();
    assert!(completed_replay.events.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLaneCreated { receipt },
                ..
            }) if receipt.owner == owner_b
        )
    }));
    assert!(completed_replay.events.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewInvalidated {
                    owner,
                    preview_id,
                    reason: StarterLanePreviewInvalidationReason::HashMismatch,
                },
                ..
            }) if owner == &owner_a && preview_id == &preview_a_id
        )
    }));
    let completed_snapshot = supervisor.snapshot_envelope().unwrap();
    assert!(completed_snapshot.view.starter_lane_previews.is_empty());
    assert_eq!(completed_snapshot.view.starter_lane_receipts.len(), 1);
    assert_eq!(
        completed_snapshot.view.starter_lane_receipts[0].preview_id,
        preview_b_id
    );
    assert_eq!(
        effects.calls.lock().unwrap().as_slice(),
        ["starter-owner-b"]
    );
}

#[test]
fn starter_lane_create_rechecks_base_after_permission_before_effect() {
    let repo = starter_lane_repo("starter_lane_toctou_repo");
    let home = temp_dir("starter_lane_toctou_home");
    let effects = Arc::new(StarterLaneEffects::default());
    let supervisor = starter_supervisor(&repo, home, Arc::clone(&effects));
    let request = starter_request("starter-toctou", StarterLanePreset::Coder);
    let owner = starter_owner(&request.lane_id);
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "preview-toctou",
            RuntimeCommand::PreviewStarterLane {
                request: request.clone(),
            },
        )
        .unwrap();
    let preview_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { .. },
                    ..
                })
            )
        })
    });
    let preview = preview_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) => Some(preview.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "create-toctou",
            RuntimeCommand::CreateStarterLane {
                request,
                preview_id: preview.preview_id,
                content_sha256: preview.content_sha256,
            },
        )
        .unwrap();
    let approval_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { .. },
                    ..
                })
            )
        })
    });
    let approval_id = approval_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalRequested { approval },
                ..
            }) => Some(approval.id.clone()),
            _ => None,
        })
        .unwrap();
    fs::write(repo.join("README.md"), "advanced during approval\n").unwrap();
    run_git(&repo, &["add", "README.md"]);
    run_git(&repo, &["commit", "-m", "advance while pending"]);
    supervisor
        .send_command_from_owner(
            owner,
            "approve-toctou",
            RuntimeCommand::RespondToApproval {
                request_id: approval_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    let failed = collect_starter_envelopes_until(&supervisor, |events| {
        let failed = events.iter().any(|envelope| matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::Error { error }, .. }) if error.message.contains("base revision changed")));
        let invalidated = events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewInvalidated {
                        reason: StarterLanePreviewInvalidationReason::EffectFailed,
                        ..
                    },
                    ..
                })
            )
        });
        failed && invalidated
    });
    assert!(failed.iter().any(|envelope| matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::Error { error }, .. }) if error.message.contains("approval was pending"))));
    assert!(effects.calls.lock().unwrap().is_empty());
    assert!(
        supervisor
            .snapshot_envelope()
            .unwrap()
            .view
            .starter_lane_previews
            .is_empty()
    );
    assert!(!failed.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::StarterLaneCreated { .. },
            ..
        })
    )));
}

#[test]
fn starter_lane_preview_rejects_parent_absolute_and_symlink_escape_paths() {
    let repo = starter_lane_repo("starter_lane_path_scope_repo");
    let home = temp_dir("starter_lane_path_scope_home");
    let outside = temp_dir("starter_lane_path_scope_outside");
    let effects = Arc::new(StarterLaneEffects::default());
    let supervisor = starter_supervisor(&repo, home, Arc::clone(&effects));
    let symlink_parent = repo.join("linked");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &symlink_parent).unwrap();

    let mut paths = vec![
        "../escape".to_string(),
        outside.join("absolute-escape").display().to_string(),
    ];
    #[cfg(unix)]
    paths.push("linked/escape".to_string());
    for (index, worktree_path) in paths.into_iter().enumerate() {
        let lane_id = format!("starter-escape-{index}");
        let owner = starter_owner(&lane_id);
        supervisor
            .send_command_from_owner(
                owner,
                format!("preview-escape-{index}"),
                RuntimeCommand::PreviewStarterLane {
                    request: StarterLaneRequest {
                        worktree_path: Some(worktree_path),
                        ..starter_request(&lane_id, StarterLanePreset::Tester)
                    },
                },
            )
            .unwrap();
        let rejected = collect_starter_envelopes_until(&supervisor, |events| {
            events.iter().any(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::CommandRejected { .. },
                        ..
                    })
                )
            })
        });
        assert!(!rejected.iter().any(|envelope| matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { .. },
                ..
            })
        )));
    }
    let invalid_branch_lane = "starter-invalid-branch";
    supervisor
        .send_command_from_owner(
            starter_owner(invalid_branch_lane),
            "preview-invalid-branch",
            RuntimeCommand::PreviewStarterLane {
                request: StarterLaneRequest {
                    branch: Some("invalid branch".to_string()),
                    ..starter_request(invalid_branch_lane, StarterLanePreset::Coder)
                },
            },
        )
        .unwrap();
    let invalid_branch = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::CommandRejected { reason, .. }, .. }) if reason.contains("branch")))
    });
    assert!(!invalid_branch.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::StarterLanePreviewed { .. },
            ..
        })
    )));
    assert!(effects.calls.lock().unwrap().is_empty());
}

#[test]
fn starter_lane_denied_create_consumes_only_its_preview_without_any_fact_or_effect() {
    let repo = starter_lane_repo("starter_lane_denied_repo");
    let home = temp_dir("starter_lane_denied_home");
    let effects = Arc::new(StarterLaneEffects::default());
    let supervisor = starter_supervisor(&repo, home.clone(), Arc::clone(&effects));
    let request = starter_request("starter-denied", StarterLanePreset::Tester);
    let owner = starter_owner(&request.lane_id);
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "preview-denied",
            RuntimeCommand::PreviewStarterLane {
                request: request.clone(),
            },
        )
        .unwrap();
    let preview_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { .. },
                    ..
                })
            )
        })
    });
    let preview = preview_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) => Some(preview.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "create-denied",
            RuntimeCommand::CreateStarterLane {
                request: request.clone(),
                preview_id: preview.preview_id.clone(),
                content_sha256: preview.content_sha256.clone(),
            },
        )
        .unwrap();
    let approval_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { .. },
                    ..
                })
            )
        })
    });
    let approval_id = approval_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalRequested { approval },
                ..
            }) => Some(approval.id.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "deny-starter",
            RuntimeCommand::RespondToApproval {
                request_id: approval_id,
                response: ApprovalResponse::deny(None),
            },
        )
        .unwrap();
    let denied = collect_starter_envelopes_until(&supervisor, |events| {
        let rejected = events.iter().any(|envelope| matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::CommandRejected { command_id, .. }, .. }) if command_id == "create-denied"));
        let invalidated = events.iter().any(|envelope| matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::StarterLanePreviewInvalidated { owner: event_owner, preview_id, reason: StarterLanePreviewInvalidationReason::PermissionDenied }, .. }) if event_owner == &owner && preview_id == &preview.preview_id));
        rejected && invalidated
    });
    assert!(denied.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::StarterLanePreviewInvalidated {
                reason: StarterLanePreviewInvalidationReason::PermissionDenied,
                ..
            },
            ..
        })
    )));
    assert!(
        supervisor
            .snapshot_envelope()
            .unwrap()
            .view
            .starter_lane_previews
            .is_empty()
    );
    supervisor
        .send_command_from_owner(
            owner,
            "reuse-denied",
            RuntimeCommand::CreateStarterLane {
                request,
                preview_id: preview.preview_id,
                content_sha256: preview.content_sha256,
            },
        )
        .unwrap();
    let reused = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::CommandRejected { command_id, reason }, .. }) if command_id == "reuse-denied" && reason.contains("unknown")))
    });
    assert!(effects.calls.lock().unwrap().is_empty());
    assert!(!Path::new(&preview.worktree_path).exists());
    assert_eq!(run_git(&repo, &["branch", "--list", &preview.branch]), "");
    assert!(!reused.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::StarterLaneCreated { .. },
            ..
        })
    )));
    assert!(
        viden_workflows::stores::WorkflowStore::new(home, &repo)
            .unwrap()
            .load_lane_events()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn starter_lane_preview_is_process_local_and_invalid_after_supervisor_restart() {
    let repo = starter_lane_repo("starter_lane_restart_repo");
    let home = temp_dir("starter_lane_restart_home");
    let effects = Arc::new(StarterLaneEffects::default());
    let request = starter_request("starter-restart", StarterLanePreset::Coder);
    let owner = starter_owner(&request.lane_id);
    let supervisor = starter_supervisor(&repo, home.clone(), Arc::clone(&effects));
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "preview-restart",
            RuntimeCommand::PreviewStarterLane {
                request: request.clone(),
            },
        )
        .unwrap();
    let preview_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { .. },
                    ..
                })
            )
        })
    });
    let preview = preview_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) => Some(preview.clone()),
            _ => None,
        })
        .unwrap();
    drop(supervisor);

    let restarted = starter_supervisor(&repo, home, Arc::clone(&effects));
    restarted
        .send_command_from_owner(
            owner,
            "create-restart",
            RuntimeCommand::CreateStarterLane {
                request,
                preview_id: preview.preview_id,
                content_sha256: preview.content_sha256,
            },
        )
        .unwrap();
    let rejected = collect_starter_envelopes_until(&restarted, |events| {
        events.iter().any(|envelope| matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::CommandRejected { reason, .. }, .. }) if reason.contains("unknown")))
    });
    assert!(rejected.iter().any(|envelope| matches!(&envelope.event, RuntimeWireEvent::Known(RuntimeEvent { kind: RuntimeEventKind::CommandRejected { reason, .. }, .. }) if reason.contains("preview"))));
    assert!(effects.calls.lock().unwrap().is_empty());
}

struct AlwaysFailLanePersistence;

impl LanePersistence for AlwaysFailLanePersistence {
    fn append(&self, _event: &LaneEvent) -> Result<(), String> {
        Err("injected starter lane append failure".to_string())
    }

    fn load_lanes(&self) -> Result<BTreeMap<String, viden_types::AgentLaneRecord>, String> {
        Ok(BTreeMap::new())
    }
}

#[test]
fn starter_lane_real_git_compensation_removes_worktree_and_new_branch_on_append_failure() {
    let repo = starter_lane_repo("starter_lane_compensation_repo");
    let home = temp_dir("starter_lane_compensation_home");
    let provider: Box<dyn ModelProvider> = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&repo, provider, Some(home)).unwrap();
    engine
        .set_permission_mode(viden_types::PermissionMode::DontAsk)
        .unwrap();
    let supervisor = RuntimeSupervisor::start_with_lane_effects_and_persistence_for_test(
        engine,
        Arc::new(LocalLaneEffectExecutor::default()) as Arc<dyn LaneEffectExecutor>,
        Arc::new(AlwaysFailLanePersistence),
    );
    let request = starter_request("starter-compensate", StarterLanePreset::Coder);
    let owner = starter_owner(&request.lane_id);
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "preview-compensate",
            RuntimeCommand::PreviewStarterLane {
                request: request.clone(),
            },
        )
        .unwrap();
    let preview_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { .. },
                    ..
                })
            )
        })
    });
    let preview = preview_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::StarterLanePreviewed { preview },
                ..
            }) => Some(preview.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "create-compensate",
            RuntimeCommand::CreateStarterLane {
                request,
                preview_id: preview.preview_id,
                content_sha256: preview.content_sha256,
            },
        )
        .unwrap();
    let approval_events = collect_starter_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { .. },
                    ..
                })
            )
        })
    });
    let approval_id = approval_events
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalRequested { approval },
                ..
            }) => Some(approval.id.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner,
            "approve-compensate",
            RuntimeCommand::RespondToApproval {
                request_id: approval_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    let failed = collect_starter_envelopes_until(&supervisor, |events| {
        let recovery = events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::LaneRecoveryRequired { .. },
                    ..
                })
            )
        });
        let invalidated = events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewInvalidated {
                        reason: StarterLanePreviewInvalidationReason::EffectFailed,
                        ..
                    },
                    ..
                })
            )
        });
        recovery && invalidated
    });
    assert!(!Path::new(&preview.worktree_path).exists());
    assert_eq!(run_git(&repo, &["branch", "--list", &preview.branch]), "");
    assert!(
        supervisor
            .snapshot_envelope()
            .unwrap()
            .view
            .starter_lane_previews
            .is_empty()
    );
    assert!(!failed.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::StarterLaneCreated { .. },
            ..
        })
    )));
}

#[test]
fn recent_work_command_is_read_only_in_plan_and_emits_exactly_accepted_then_loaded() {
    let cwd = temp_dir("recent_work_runtime_cwd");
    let other = temp_dir("recent_work_runtime_other");
    let home = temp_dir("recent_work_runtime_home");
    append_recent_runtime_fixture(&home, &other, "recent-runtime", 10);
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    engine.set_work_mode(WorkMode::Plan).unwrap();
    let mut approver = |_prompt| panic!("read-only recent work must not request approval");

    let events = engine
        .handle_runtime_command(
            "recent-work",
            RuntimeCommand::QueryRecentWork {
                query: RecentWorkQuery { limit: 10 },
            },
            &mut approver,
        )
        .unwrap();

    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0].kind,
        RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "recent-work"
    ));
    assert!(matches!(
        &events[1].kind,
        RuntimeEventKind::RecentWorkLoaded { sessions, .. }
            if sessions.len() == 1 && sessions[0].session_id == "recent-runtime"
    ));
    assert!(
        !serde_json::to_string(&events)
            .unwrap()
            .contains("sk-secret-runtime-body")
    );
    assert!(
        engine
            .workflow_store()
            .load_agent_events()
            .unwrap()
            .is_empty()
    );
    assert!(
        !fs::read_to_string(engine.store.transcript_path())
            .unwrap()
            .contains("recent_work_loaded")
    );
}

#[test]
fn recent_work_new_transcript_starts_with_canonical_root_and_stable_timestamp_metadata() {
    let cwd = temp_dir("recent_work_initial_metadata_cwd")
        .canonicalize()
        .unwrap();
    let home = temp_dir("recent_work_initial_metadata_home");
    let engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();

    let entries = engine.store.load_entries().unwrap();
    let root = entries.iter().find_map(|entry| match entry {
        TranscriptEntry::SessionMeta { entry } if entry.key == "canonical_root" => {
            Some((entry.timestamp, entry.value.clone()))
        }
        _ => None,
    });
    let created = entries.iter().find_map(|entry| match entry {
        TranscriptEntry::SessionMeta { entry } if entry.key == "session_created_at" => {
            Some((entry.timestamp, entry.value.clone()))
        }
        _ => None,
    });

    assert_eq!(root.as_ref().map(|(_, value)| value.as_str()), cwd.to_str());
    assert_eq!(
        created
            .as_ref()
            .and_then(|(_, value)| value.parse::<u64>().ok()),
        created.as_ref().map(|(timestamp, _)| *timestamp)
    );
    let raw = fs::read_to_string(engine.store.transcript_path()).unwrap();
    assert!(raw.starts_with("{\"type\":\"runtime_event_batch_begin\""));
}

#[test]
fn recent_work_supervisor_snapshot_and_replay_restore_the_last_loaded_result() {
    let cwd = temp_dir("recent_work_supervisor_cwd")
        .canonicalize()
        .unwrap();
    let other = temp_dir("recent_work_supervisor_other")
        .canonicalize()
        .unwrap();
    let home = temp_dir("recent_work_supervisor_home");
    append_recent_runtime_fixture(&home, &other, "other-session", 10);
    let engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "recent-work-supervisor",
            RuntimeCommand::QueryRecentWork {
                query: RecentWorkQuery { limit: 10 },
            },
        )
        .unwrap();

    let events = collect_until(&supervisor, |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::RecentWorkLoaded { .. }))
    });
    let loaded = events
        .iter()
        .find(|event| matches!(event.kind, RuntimeEventKind::RecentWorkLoaded { .. }))
        .unwrap();
    let snapshot = supervisor.snapshot_envelope().unwrap();
    assert_eq!(snapshot.view.recent_sessions.len(), 2);
    assert!(
        snapshot
            .view
            .recent_sessions
            .iter()
            .any(|session| session.session_id == "other-session")
    );

    let replay = supervisor
        .replay_events(ReplayRequest {
            after: EventCursor {
                stream_id: snapshot.cursor.stream_id.clone(),
                sequence: 0,
            },
            limit: 10,
        })
        .unwrap();
    assert!(replay.events.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(event)
                if event.sequence == loaded.sequence
                    && matches!(event.kind, RuntimeEventKind::RecentWorkLoaded { .. })
        )
    }));
}

fn preference_engine(slug: &str) -> (SessionEngine, PathBuf, PathBuf) {
    let cwd = temp_dir(slug);
    let config_path = cwd.join("user-config.toml");
    let home = cwd.join("session-home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    engine.set_ui_preference_context(
        None,
        Some(config_path.clone()),
        UiPreferences::client_default(),
    );
    (engine, cwd, config_path)
}

#[test]
fn ui_preferences_command_set_emits_fact_and_reducer_syncs_view_and_snapshot() {
    let (mut engine, _cwd, config_path) = preference_engine("ui_preferences_set");
    let before = engine.runtime_view_state();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let events = engine
        .handle_runtime_command(
            "set-ui",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    locale: Some(LocaleId::ZhCn),
                    skin: Some(UiSkin::Ice),
                    mode: Some(UiColorMode::Light),
                    density: Some(UiDensity::Compact),
                    motion: Some(UiMotion::Reduced),
                },
            },
            &mut approver,
        )
        .unwrap();

    let fact = events
        .iter()
        .find(|event| matches!(event.kind, RuntimeEventKind::UiPreferencesUpdated { .. }))
        .expect("successful preference command emits a fact");
    let mut replayed = before;
    replayed.apply_event(fact);
    assert_eq!(replayed.ui_preferences.locale, LocaleId::ZhCn);
    assert_eq!(replayed.ui_preferences, replayed.snapshot.ui_preferences);
    assert_eq!(
        engine.runtime_snapshot().ui_preferences,
        replayed.ui_preferences
    );
    assert!(fs::read_to_string(config_path).unwrap().contains("zh-CN"));
}

#[test]
fn ui_preferences_command_invalid_profile_is_rejected_before_approval() {
    let (mut engine, cwd, config_path) = preference_engine("ui_preferences_invalid");
    let original = b"[ui]\nskin = \"amber\"\nmode = \"dark\"\n";
    fs::write(&config_path, original).unwrap();
    let approvals = AtomicUsize::new(0);
    let mut approver = |_prompt| {
        approvals.fetch_add(1, Ordering::SeqCst);
        ApprovalResponse::allow_once(None)
    };
    let events = engine
        .handle_runtime_command(
            "invalid-ui",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    mode: Some(UiColorMode::Light),
                    ..UiPreferencePatch::default()
                },
            },
            &mut approver,
        )
        .unwrap();

    assert_eq!(approvals.load(Ordering::SeqCst), 0);
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::CommandRejected { reason, .. }
            if reason.contains("ui.invalid_skin_mode_pair")
    )));
    assert_eq!(fs::read(&config_path).unwrap(), original);
    assert!(ui_temp_files(&cwd).is_empty());
}

#[test]
fn ui_preferences_command_cli_override_wins_after_persisting_user_profile() {
    let (mut engine, _cwd, config_path) = preference_engine("ui_preferences_cli_wins");
    let cli = UiPreferences {
        locale: LocaleId::En,
        skin: UiSkin::Mono,
        mode: UiColorMode::Dark,
        density: UiDensity::Regular,
        motion: UiMotion::Full,
    };
    engine.set_ui_preference_context(
        Some(cli),
        Some(config_path),
        UiPreferences::client_default(),
    );
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let events = engine
        .handle_runtime_command(
            "cli-wins",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    locale: Some(LocaleId::ZhCn),
                    skin: Some(UiSkin::Ice),
                    mode: Some(UiColorMode::Light),
                    density: Some(UiDensity::Compact),
                    motion: Some(UiMotion::Reduced),
                },
            },
            &mut approver,
        )
        .unwrap();

    let (resolved, persisted) = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::UiPreferencesUpdated {
                resolved,
                persisted,
                ..
            } => Some((resolved, persisted)),
            _ => None,
        })
        .unwrap();
    assert_eq!(resolved.locale, LocaleId::En);
    assert_eq!(resolved.skin, UiSkin::Mono);
    assert_eq!(persisted.as_ref().unwrap().locale, LocaleId::ZhCn);
}

#[test]
fn ui_preferences_command_bootstrap_retains_only_safe_reresolution_context() {
    let cwd = temp_dir("ui_preferences_bootstrap_context");
    let config_path = cwd.join("user-config.toml");
    let cli = UiPreferences {
        locale: LocaleId::En,
        skin: UiSkin::Mono,
        mode: UiColorMode::Dark,
        density: UiDensity::Regular,
        motion: UiMotion::Full,
    };
    let bootstrap = crate::bootstrap_runtime(crate::RuntimeBootstrapRequest::new(
        &cwd,
        CliOverrides {
            provider: Some("fallback".to_string()),
            model: Some("test-local".to_string()),
            api_key: Some("must-not-be-retained-for-ui-reresolution".to_string()),
            session_home: Some(cwd.join("session-home")),
            config_path: Some(config_path),
            ui: Some(cli),
            ..CliOverrides::default()
        },
    ))
    .unwrap();
    let mut engine = bootstrap.engine;
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let events = engine
        .handle_runtime_command(
            "bootstrap-ui",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    locale: Some(LocaleId::ZhCn),
                    skin: Some(UiSkin::Ice),
                    mode: Some(UiColorMode::Light),
                    ..UiPreferencePatch::default()
                },
            },
            &mut approver,
        )
        .unwrap();

    let serialized = serde_json::to_string(&events).unwrap();
    assert!(!serialized.contains("must-not-be-retained"));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::UiPreferencesUpdated { resolved, .. }
            if resolved.locale == LocaleId::En && resolved.skin == UiSkin::Mono
    )));
}

#[test]
fn ui_preferences_command_plan_mode_preserves_bytes_mtime_and_temp_state() {
    let (mut engine, cwd, config_path) = preference_engine("ui_preferences_plan");
    let original = b"[ui]\nskin = \"ice\"\nmode = \"dark\"\n";
    fs::write(&config_path, original).unwrap();
    let before_modified = fs::metadata(&config_path).unwrap().modified().unwrap();
    engine.set_work_mode(WorkMode::Plan).unwrap();
    let mut approver = |_prompt| panic!("Plan mode must deny before approval");
    let events = engine
        .handle_runtime_command("plan-ui", RuntimeCommand::ResetUiPreferences, &mut approver)
        .unwrap();

    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::CommandRejected { .. }))
    );
    assert_eq!(fs::read(&config_path).unwrap(), original);
    assert_eq!(
        fs::metadata(&config_path).unwrap().modified().unwrap(),
        before_modified
    );
    assert!(ui_temp_files(&cwd).is_empty());
}

#[test]
fn ui_preferences_command_reset_removes_ui_and_emits_non_durable_projection_fact() {
    let (mut engine, _cwd, config_path) = preference_engine("ui_preferences_reset");
    fs::write(
        &config_path,
        "custom = 7\n[ui]\nskin = \"ice\"\nmode = \"dark\"\nfuture = \"gone\"\n",
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let events = engine
        .handle_runtime_command(
            "reset-ui",
            RuntimeCommand::ResetUiPreferences,
            &mut approver,
        )
        .unwrap();

    let persisted = events.iter().find_map(|event| match &event.kind {
        RuntimeEventKind::UiPreferencesUpdated { persisted, .. } => Some(persisted),
        _ => None,
    });
    assert_eq!(persisted, Some(&None));
    let value = fs::read_to_string(&config_path).unwrap();
    assert!(!value.contains("[ui]"));
    assert!(value.contains("custom = 7"));
    assert!(
        engine
            .workflow_store()
            .load_agent_events()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn ui_preferences_command_supervisor_routes_mutation_through_approval() {
    let (engine, _cwd, _config_path) = preference_engine("ui_preferences_supervisor");
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "supervised-ui",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    skin: Some(UiSkin::Ice),
                    mode: Some(UiColorMode::Light),
                    ..UiPreferencePatch::default()
                },
            },
        )
        .unwrap();

    let events = collect_until(&supervisor, |events| {
        events.iter().any(|event| {
            matches!(
                event.kind,
                RuntimeEventKind::ApprovalRequested { .. }
                    | RuntimeEventKind::CommandRejected { .. }
            )
        })
    });
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::CommandRejected { .. }))
    );
}

#[test]
fn ui_preferences_command_supervisor_success_updates_live_snapshot_and_replay() {
    let (engine, _cwd, config_path) = preference_engine("ui_preferences_supervisor_success");
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "supervised-ui-success",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    locale: Some(LocaleId::ZhCn),
                    skin: Some(UiSkin::Mono),
                    mode: Some(UiColorMode::Light),
                    ..UiPreferencePatch::default()
                },
            },
        )
        .unwrap();
    let pending = collect_until(&supervisor, |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let request_id = pending
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::ApprovalRequested { approval } => Some(approval.id.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command(
            "approve-ui",
            RuntimeCommand::RespondToApproval {
                request_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();

    let success = collect_until(&supervisor, |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::UiPreferencesUpdated { .. }))
    });
    assert!(success.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::UiPreferencesUpdated { resolved, .. }
            if resolved.locale == LocaleId::ZhCn && resolved.mode == UiColorMode::Light
    )));
    let snapshot = supervisor.snapshot_envelope().unwrap();
    assert_eq!(
        snapshot.view.ui_preferences,
        snapshot.view.snapshot.ui_preferences
    );
    assert_eq!(snapshot.view.ui_preferences.locale, LocaleId::ZhCn);
    assert!(fs::read_to_string(config_path).unwrap().contains("zh-CN"));
}

#[test]
fn ui_preferences_command_supervisor_rejects_invalid_profile_without_approval() {
    let (engine, cwd, config_path) = preference_engine("ui_preferences_supervisor_invalid");
    let original = b"[ui]\nskin = \"amber\"\nmode = \"dark\"\n";
    fs::write(&config_path, original).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "supervised-invalid-ui",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    mode: Some(UiColorMode::Light),
                    ..UiPreferencePatch::default()
                },
            },
        )
        .unwrap();

    let events = collect_until(&supervisor, |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::CommandRejected { .. }))
    });
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert_eq!(fs::read(&config_path).unwrap(), original);
    assert!(ui_temp_files(&cwd).is_empty());
}

fn collect_until(
    supervisor: &RuntimeSupervisor,
    done: impl Fn(&[viden_types::RuntimeEvent]) -> bool,
) -> Vec<viden_types::RuntimeEvent> {
    let started = Instant::now();
    let mut events = Vec::new();
    while started.elapsed() < Duration::from_secs(10) {
        if let Some(event) = supervisor.recv_event_timeout(Duration::from_millis(50)) {
            events.push(event);
            if done(&events) {
                return events;
            }
        }
    }
    panic!("timed out waiting for UI preference supervisor events: {events:#?}");
}

fn ui_temp_files(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".ui-") && name.ends_with(".tmp"))
        })
        .collect()
}
