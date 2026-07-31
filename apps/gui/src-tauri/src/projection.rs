use serde::Serialize;
use viden_core::{
    AgentLaneRecord, AgentRoute, AgentSessionStatus, AgentStartability, AgentTaskStatus,
    ApprovalDefaultAction, ApprovalRisk, ApprovalScope, COCKPIT_CONTEXT_CAPABILITY, CheckRunStatus,
    CredentialHandle, EventCursor, LaneStatus, LocaleId, ProjectConfigPreview, ProjectProbe,
    ProviderHealthView, RuntimeServiceKind, RuntimeServiceStatus, RuntimeSnapshotEnvelope,
    RuntimeViewState, UiColorMode, UiDensity, UiMotion, UiSkin, WorkMode, WorkspaceChangeKind,
    WorkspaceSourceStatus,
};

use crate::d1::{
    D1_OWNER_CAPABILITY, D1AgentAdapterProjection, D1AgentSessionProjection, D1ApprovalProjection,
    D1ChecklistItemProjection, D1CockpitProjection, D1ComposerProjection, D1ContextDockProjection,
    D1CostUsageProjection, D1CursorProjection, D1EnvironmentProjection, D1LaneAgentProjection,
    D1LaneProjection, D1LiveWorkProjection, D1ProviderHealthProjection, D1RuntimeServiceProjection,
    D1StarterLanePreviewProjection, D1StarterLaneReceiptProjection, D1TranscriptRowProjection,
    D1WorkspaceEligibilityProjection, D1WorkspaceSourceProjection, unavailable_features,
};
use crate::{
    D6ActionProjection, D6ConnectionState, D6RecoveryProjection, D6State,
    PermissionActionProjection, PermissionDockProjection, PermissionRequestProjection,
    PermissionTargetProjection,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceDiagnosticProjection {
    pub code: String,
    pub key: String,
    pub field: Option<String>,
    pub rejected_value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedPreferencesProjection {
    pub locale: &'static str,
    pub skin: &'static str,
    pub mode: &'static str,
    pub density: &'static str,
    pub motion: &'static str,
    pub diagnostics: Vec<PreferenceDiagnosticProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D11ProjectProjection {
    pub root: String,
    pub is_git_repository: bool,
    pub config_state: &'static str,
    pub project_name: Option<String>,
    pub mode: Option<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D11ConfigProjection {
    pub preview_id: String,
    pub relative_path: String,
    pub content_sha256: String,
    pub exact_contents: Option<String>,
    pub valid: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D11ConfirmedConfigProjection {
    pub preview_id: String,
    pub relative_path: String,
    pub content_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D11ProviderProjection {
    pub provider_id: String,
    pub model: String,
    pub status: String,
    pub warning: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D11CredentialProjection {
    pub provider_id: String,
    pub masked_handle: String,
    pub status: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D11StarterLaneProjection {
    pub id: String,
    pub role: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct D11ApprovalProjection {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D11AvailabilityProjection {
    pub available: bool,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D11CapabilityProjection {
    pub project_onboarding: bool,
    pub credential_handles: bool,
    pub lane_lifecycle: bool,
    pub starter_lane_preview: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D11IntakeProjection {
    pub project: Option<D11ProjectProjection>,
    pub preview: Option<D11ConfigProjection>,
    pub confirmed_config: Option<D11ConfirmedConfigProjection>,
    pub provider: Option<D11ProviderProjection>,
    pub credential_handles: Vec<D11CredentialProjection>,
    pub starter_lanes: Vec<D11StarterLaneProjection>,
    pub pending_approval: Option<D11ApprovalProjection>,
    pub last_error: Option<String>,
    pub recent_work: D11AvailabilityProjection,
    pub credential_ingress: D11AvailabilityProjection,
    pub capabilities: D11CapabilityProjection,
}

/// Last authoritative snapshot published by Core for presentation.
#[derive(Clone, Debug, Default)]
pub struct RuntimeProjection {
    confirmed: Option<RuntimeSnapshotEnvelope>,
}

impl RuntimeProjection {
    pub(crate) fn replace(&mut self, snapshot: RuntimeSnapshotEnvelope) {
        self.confirmed = Some(snapshot);
    }

    pub(crate) fn d11_approval_by_id(&self, request_id: &str) -> Option<D11ApprovalProjection> {
        self.confirmed.as_ref().and_then(|confirmed| {
            confirmed
                .view
                .pending_approvals
                .iter()
                .find(|approval| approval.id == request_id)
                .map(|approval| D11ApprovalProjection {
                    id: approval.id.clone(),
                    title: approval.title.clone(),
                })
        })
    }

    pub fn view(&self) -> Option<&RuntimeViewState> {
        self.confirmed.as_ref().map(|snapshot| &snapshot.view)
    }

    pub fn cursor(&self) -> Option<&EventCursor> {
        self.confirmed.as_ref().map(|snapshot| &snapshot.cursor)
    }

    pub fn permission_dock(&self) -> Option<PermissionDockProjection> {
        self.permission_dock_matching(|_| true)
    }

    pub(crate) fn permission_dock_for_owner(
        &self,
        owner: &viden_core::RuntimeOwner,
    ) -> Option<PermissionDockProjection> {
        self.permission_dock_matching(|approval| approval.owner == *owner)
    }

    fn empty_permission_dock(&self) -> Option<PermissionDockProjection> {
        self.permission_dock_matching(|_| false)
    }

    fn permission_dock_matching(
        &self,
        matches_owner: impl Fn(&viden_core::ApprovalRequestView) -> bool,
    ) -> Option<PermissionDockProjection> {
        let view = self.view()?;
        let request = view
            .pending_approvals
            .iter()
            .rev()
            .find(|approval| matches_owner(approval))
            .map(|approval| {
                let blocked_by_plan =
                    approval.is_mutating && view.snapshot.work_mode == WorkMode::Plan;
                let mut actions = approval
                    .allowed_scopes
                    .iter()
                    .map(|scope| match scope {
                        ApprovalScope::Once => PermissionActionProjection {
                            kind: "once".to_string(),
                            available: !blocked_by_plan,
                            session_id: None,
                            paths: Vec::new(),
                            code: None,
                        },
                        ApprovalScope::Session { session_id } => PermissionActionProjection {
                            kind: "session".to_string(),
                            available: !blocked_by_plan,
                            session_id: Some(session_id.clone()),
                            paths: Vec::new(),
                            code: None,
                        },
                        ApprovalScope::RepoAllowlist { paths } => PermissionActionProjection {
                            kind: "repo_allowlist".to_string(),
                            available: !blocked_by_plan,
                            session_id: None,
                            paths: paths.clone(),
                            code: None,
                        },
                    })
                    .collect::<Vec<_>>();
                // The design names Always/Edit, but schema 1 has no corresponding
                // decision variants. Keep them visible and fail closed.
                actions.extend([
                    PermissionActionProjection {
                        kind: "always".to_string(),
                        available: false,
                        session_id: None,
                        paths: Vec::new(),
                        code: Some("GUI-CORE-003"),
                    },
                    PermissionActionProjection {
                        kind: "edit".to_string(),
                        available: false,
                        session_id: None,
                        paths: Vec::new(),
                        code: Some("GUI-CORE-003"),
                    },
                    PermissionActionProjection {
                        kind: "deny".to_string(),
                        available: !blocked_by_plan,
                        session_id: None,
                        paths: Vec::new(),
                        code: None,
                    },
                ]);
                PermissionRequestProjection {
                    id: approval.id.clone(),
                    tool_name: approval.tool_name.clone(),
                    title: approval.title.clone(),
                    message: approval.message.clone(),
                    input_preview: approval.input_preview.clone(),
                    is_mutating: approval.is_mutating,
                    reason: approval.reason.clone(),
                    risk: match approval.risk {
                        ApprovalRisk::Low => "low",
                        ApprovalRisk::Medium => "medium",
                        ApprovalRisk::High => "high",
                        ApprovalRisk::Critical => "critical",
                    }
                    .to_string(),
                    target: PermissionTargetProjection {
                        kind: approval.target.kind.clone(),
                        display: approval.target.display.clone(),
                        canonical_ref: approval.target.canonical_ref.clone(),
                    },
                    policy_reason_key: approval.policy_reason_key.clone(),
                    policy_reason_args: approval.policy_reason_args.clone(),
                    expires_at: approval.expires_at,
                    default_action: match approval.default_action {
                        ApprovalDefaultAction::Deny => "deny",
                    },
                    audit_id: approval.audit_id.clone(),
                    blocked_by_plan,
                    actions,
                }
            });
        Some(PermissionDockProjection {
            work_mode: view.snapshot.work_mode.cli_name().to_string(),
            permission_level: view.snapshot.permission_level.cli_name().to_string(),
            request,
        })
    }

    pub fn d6_recovery(
        &self,
        connection: D6ConnectionState,
        connection_detail: Option<&str>,
        supports_approvals: bool,
    ) -> Option<D6RecoveryProjection> {
        let view = self.view()?;
        let latest_error = view.errors.last();
        let exceeded = view
            .context_budgets
            .iter()
            .rev()
            .find(|budget| budget.exceeded);
        let agent_stopped = view.tasks.iter().any(|task| {
            matches!(
                task.status,
                AgentTaskStatus::Failed | AgentTaskStatus::Cancelled | AgentTaskStatus::Discarded
            )
        }) || view.lanes.iter().any(|lane| {
            matches!(
                lane.status,
                LaneStatus::Failed | LaneStatus::Cancelled | LaneStatus::Detached
            )
        });
        let provider_error = view
            .provider
            .as_ref()
            .is_some_and(|provider| provider.error_count > 0);
        let open_merge_gate = view.merge_gates.iter().any(|gate| gate.status.is_open());
        let missing_capabilities = (!supports_approvals)
            .then(|| "runtime.approvals".to_string())
            .into_iter()
            .collect::<Vec<_>>();

        let state = match connection {
            D6ConnectionState::Connecting => D6State::Connecting,
            D6ConnectionState::Disconnected => D6State::Disconnected,
            D6ConnectionState::Recovering => D6State::EventGap,
            D6ConnectionState::Incompatible => D6State::IncompatibleSchema,
            D6ConnectionState::Live if !missing_capabilities.is_empty() => {
                D6State::MissingFeatureCapability
            }
            D6ConnectionState::Live if view.lanes.is_empty() => D6State::Empty,
            D6ConnectionState::Live if exceeded.is_some() => D6State::ContextOverflow,
            D6ConnectionState::Live if agent_stopped => D6State::AgentStopped,
            D6ConnectionState::Live if provider_error => D6State::ProviderError,
            D6ConnectionState::Live if view.pending_approvals.is_empty() && !open_merge_gate => {
                D6State::GateQueueClear
            }
            D6ConnectionState::Live => D6State::Live,
        };
        let blocked = matches!(
            state,
            D6State::Connecting
                | D6State::Disconnected
                | D6State::ProviderError
                | D6State::AgentStopped
                | D6State::ContextOverflow
                | D6State::IncompatibleSchema
                | D6State::MissingFeatureCapability
                | D6State::EventGap
        );
        let detail = connection_detail
            .map(str::to_string)
            .or_else(|| latest_error.map(|error| error.message.clone()));
        let hint = latest_error.and_then(|error| error.hint.clone());
        let recoverable = latest_error.is_some_and(|error| error.recoverable)
            || matches!(state, D6State::Disconnected | D6State::EventGap);
        let reconnect_available = matches!(state, D6State::Disconnected | D6State::EventGap);
        Some(D6RecoveryProjection {
            connection,
            state,
            detail,
            hint,
            recoverable,
            business_success_blocked: blocked,
            used_tokens: exceeded.map(|budget| budget.used_tokens),
            hard_token_limit: exceeded.map(|budget| budget.hard_token_limit),
            missing_capabilities,
            actions: vec![
                D6ActionProjection {
                    kind: "reconnect",
                    available: reconnect_available,
                    code: if reconnect_available {
                        "core_client"
                    } else {
                        "GUI-CORE-003"
                    },
                },
                D6ActionProjection {
                    kind: "inspect",
                    available: true,
                    code: "presentation_only",
                },
                D6ActionProjection {
                    kind: "restart",
                    available: false,
                    code: "GUI-CORE-003",
                },
                D6ActionProjection {
                    kind: "close_lane",
                    available: false,
                    code: "GUI-CORE-003",
                },
                D6ActionProjection {
                    kind: "checkpoint",
                    available: false,
                    code: "GUI-CORE-003",
                },
            ],
        })
    }

    /// Projects only Core-resolved preferences into a transport-safe GUI view.
    pub fn preferences(&self) -> Option<ResolvedPreferencesProjection> {
        let resolved = &self.view()?.snapshot.ui_preferences;
        Some(ResolvedPreferencesProjection {
            locale: match resolved.locale {
                LocaleId::System | LocaleId::En => "en",
                LocaleId::ZhCn => "zh-CN",
            },
            skin: match resolved.skin {
                UiSkin::Aurora => "aurora",
                UiSkin::Ice => "ice",
                UiSkin::Mono => "mono",
                UiSkin::Amber => "amber",
                UiSkin::Phosphor => "phosphor",
            },
            mode: match resolved.mode {
                UiColorMode::System | UiColorMode::Dark => "dark",
                UiColorMode::Light => "light",
            },
            density: match resolved.density {
                UiDensity::Compact => "compact",
                UiDensity::Regular => "regular",
                UiDensity::Comfy => "comfy",
            },
            motion: match resolved.motion {
                UiMotion::System => "system",
                UiMotion::Reduced => "reduced",
                UiMotion::Full => "full",
            },
            diagnostics: resolved
                .diagnostics
                .iter()
                .map(|diagnostic| PreferenceDiagnosticProjection {
                    code: diagnostic.code.clone(),
                    key: diagnostic.key.clone(),
                    field: diagnostic.field.clone(),
                    rejected_value: diagnostic.rejected_value.clone(),
                })
                .collect(),
        })
    }

    /// Selects D11 facts without creating a second onboarding reducer in the GUI.
    pub fn d11_intake(&self) -> Option<D11IntakeProjection> {
        let confirmed = self.confirmed.as_ref()?;
        let view = &confirmed.view;
        let supports = |capability: &str| {
            confirmed
                .capabilities
                .iter()
                .any(|candidate| candidate.0 == capability)
        };
        Some(D11IntakeProjection {
            project: view.project_probe.as_ref().map(project_projection),
            preview: view.project_config_preview.as_ref().map(config_projection),
            confirmed_config: view
                .confirmed_project_config
                .as_ref()
                .map(confirmed_config_projection),
            provider: view.provider.as_ref().map(provider_projection),
            credential_handles: view
                .credential_handles
                .iter()
                .map(credential_projection)
                .collect(),
            starter_lanes: view.lanes.iter().map(starter_lane_projection).collect(),
            pending_approval: view
                .pending_approvals
                .iter()
                .rev()
                .find(|approval| {
                    approval.tool_name.contains("project_config_confirm")
                        || approval.tool_name.contains("credential_handle_store")
                        || approval.tool_name == "lane_create"
                })
                .map(|approval| D11ApprovalProjection {
                    id: approval.id.clone(),
                    title: approval.title.clone(),
                }),
            last_error: view.errors.last().map(|error| error.message.clone()),
            recent_work: D11AvailabilityProjection {
                available: false,
                code: "GUI-CORE-007",
                message: "Recent project and session history is unavailable.",
            },
            credential_ingress: D11AvailabilityProjection {
                available: false,
                code: "GUI-CORE-001",
                message: "Platform credential intake is unavailable.",
            },
            capabilities: D11CapabilityProjection {
                project_onboarding: supports("runtime.project_onboarding"),
                credential_handles: supports("runtime.credential_handles"),
                lane_lifecycle: supports("runtime.lane_lifecycle"),
                starter_lane_preview: supports("runtime.starter_lane_preview"),
            },
        })
    }

    /// Builds the canonical D1 cockpit from Core's reduced view and nothing else.
    pub fn d1_cockpit(&self, requested_lane_id: Option<&str>) -> Option<D1CockpitProjection> {
        let confirmed = self.confirmed.as_ref()?;
        let view = &confirmed.view;
        // Preserve an explicit selection even when its Lane disappears: choosing another
        // Lane would silently retarget a subsequent mutation.
        let selected_lane_id = requested_lane_id
            .map(str::to_string)
            .or_else(|| view.lanes.first().map(|lane| lane.id.clone()));
        let selected_lane = selected_lane_id
            .as_deref()
            .and_then(|lane_id| view.lanes.iter().find(|lane| lane.id == lane_id));
        let exact_owner_count = selected_lane
            .and_then(|_| selected_lane_id.as_deref())
            .map_or(0, |lane_id| exact_owner_count(view, lane_id));
        let exact_binding = selected_lane
            .and_then(|_| selected_lane_id.as_deref())
            .and_then(|lane_id| exact_owner_binding(view, lane_id));
        let restored_agent_session = exact_binding
            .is_none()
            .then(|| {
                selected_lane
                    .and_then(|_| selected_lane_id.as_deref())
                    .and_then(|lane_id| exact_terminal_agent_session(view, lane_id))
            })
            .flatten();
        let selected_owner = exact_binding
            .map(|binding| &binding.owner)
            .or_else(|| restored_agent_session.map(|session| &session.owner));
        let supports_owner = confirmed
            .capabilities
            .iter()
            .any(|capability| capability.0 == D1_OWNER_CAPABILITY);
        let supports_context_dock = confirmed
            .capabilities
            .iter()
            .any(|capability| capability.0 == COCKPIT_CONTEXT_CAPABILITY);
        // `RuntimeOwner.turn_id` is the owner-scoped Core fact that proves an active turn.
        // Do not infer queueing from broad Lane lifecycle states.
        let busy = selected_owner.is_some_and(|owner| owner.turn_id.is_some());

        let provider_id = view
            .provider
            .as_ref()
            .map(|provider| provider.provider_id.clone())
            .unwrap_or_else(|| view.snapshot.provider_family.clone());
        let model = view
            .provider
            .as_ref()
            .map(|provider| provider.model.clone())
            .unwrap_or_else(|| view.snapshot.model_label.clone());
        let token_total = view.token_cost.as_ref().map_or(0, |cost| cost.total_tokens);
        let cost_micro_usd = view
            .token_cost
            .as_ref()
            .and_then(|cost| cost.cost_micro_usd);

        let mut transcript = Vec::new();
        if exact_binding.is_some() {
            transcript.extend(
                view.lane_outputs
                    .iter()
                    .filter(|output| selected_lane_id.as_deref() == Some(output.lane_id.as_str()))
                    .enumerate()
                    .map(|(ordinal, output)| D1TranscriptRowProjection {
                        // Core does not expose a lane-output identity yet. This ordinal only
                        // disambiguates rows inside one projection; it is not durable identity.
                        id: format!(
                            "lane-output-{}-{}-{ordinal}",
                            output.lane_id,
                            output.timestamp.unwrap_or(0)
                        ),
                        kind: "lane_output",
                        content: output.content.clone(),
                    }),
            );
        }
        if transcript.len() > 240 {
            transcript.drain(..transcript.len() - 240);
        }

        let context_dock = if supports_context_dock {
            let lane_agent = exact_binding
                .map(|binding| (&binding.lane_id, &binding.owner))
                .or_else(|| {
                    restored_agent_session.map(|session| (&session.lane_id, &session.owner))
                })
                .map(|(lane_id, owner)| D1LaneAgentProjection {
                    lane_id: lane_id.clone(),
                    workspace_id: owner.workspace_id.clone(),
                    project_id: owner.project_id.clone(),
                    session_id: owner.session_id.clone(),
                    task_id: owner.task_id.clone(),
                    turn_id: owner.turn_id.clone(),
                });
            let selected_lane_matches =
                |owner: &viden_core::RuntimeOwner| selected_owner == Some(owner);
            let mut checklist = view
                .workspace_changes
                .iter()
                .filter(|change| selected_lane_matches(&change.owner))
                .map(|change| D1ChecklistItemProjection {
                    id: change.id.clone(),
                    kind: "workspace_change",
                    label: change.path.clone(),
                    status: workspace_change_kind(change.kind),
                    command: None,
                    path: Some(change.path.clone()),
                    summary: None,
                    patch: change.patch.clone(),
                    failing_location: None,
                    additions: Some(change.additions),
                    deletions: Some(change.deletions),
                })
                .collect::<Vec<_>>();
            checklist.extend(
                view.check_runs
                    .iter()
                    .filter(|check| selected_lane_matches(&check.owner))
                    .map(|check| D1ChecklistItemProjection {
                        id: check.id.clone(),
                        kind: "check_run",
                        label: check.label.clone(),
                        status: check_run_status(check.status),
                        command: Some(check.command.clone()),
                        path: None,
                        summary: Some(check.summary.clone()),
                        patch: None,
                        failing_location: check.failing_location.clone(),
                        additions: None,
                        deletions: None,
                    }),
            );
            D1ContextDockProjection {
                source: view
                    .workspace_source
                    .as_ref()
                    .map(|source| D1WorkspaceSourceProjection {
                        status: workspace_source_status(source.status),
                        branch: source.branch.clone(),
                        worktree: source.worktree.clone(),
                        ahead: source.ahead,
                        behind: source.behind,
                        added: source.added,
                        deleted: source.deleted,
                        dirty: source.dirty,
                    }),
                // Core exposes the budget collection through RuntimeViewState,
                // but its task scope type is not available through viden-core.
                // Stay unavailable until the facade can prove selected-Lane scope.
                context: None,
                lane_agent,
                provider: view
                    .provider
                    .as_ref()
                    .map(|provider| D1ProviderHealthProjection {
                        provider_id: provider.provider_id.clone(),
                        model: provider.model.clone(),
                        status: provider.status.clone(),
                        request_count: provider.request_count,
                        error_count: provider.error_count,
                        last_latency_ms: provider.last_latency_ms,
                        average_latency_ms: provider.average_latency_ms,
                        tokens_per_second: provider.tokens_per_second,
                    }),
                services: view
                    .runtime_services
                    .iter()
                    .map(|service| D1RuntimeServiceProjection {
                        id: service.id.clone(),
                        kind: runtime_service_kind(service.kind),
                        label: service.label.clone(),
                        status: runtime_service_status(service.status),
                        detail_key: service.detail_key.clone(),
                    })
                    .collect(),
                checklist,
            }
        } else {
            D1ContextDockProjection {
                source: None,
                context: None,
                lane_agent: None,
                provider: None,
                services: Vec::new(),
                checklist: Vec::new(),
            }
        };
        let recovery = if supports_owner && exact_owner_count > 1 {
            // An ambiguous execution identity is not renderable. Enter the
            // existing snapshot/replay recovery path instead of choosing one.
            self.d6_recovery(
                D6ConnectionState::Recovering,
                Some("GUI-CORE-D1-OWNER-CARDINALITY"),
                true,
            )?
        } else {
            self.d6_recovery(D6ConnectionState::Live, None, true)?
        };

        Some(D1CockpitProjection {
            preferences: self.preferences()?,
            selected_lane_id,
            context_dock,
            lanes: view
                .lanes
                .iter()
                .map(|lane| D1LaneProjection {
                    id: lane.id.clone(),
                    role: lane.role.to_string(),
                    status: lane_status(lane.status).to_string(),
                    summary: lane.summary.clone(),
                    branch: lane.branch.clone(),
                })
                .collect(),
            environment: D1EnvironmentProjection {
                cwd: view.snapshot.cwd.display().to_string(),
                provider_id,
                model,
                work_mode: view.snapshot.work_mode.cli_name().to_string(),
                permission_level: view.snapshot.permission_level.cli_name().to_string(),
                token_total,
                cost_micro_usd,
            },
            live_work: D1LiveWorkProjection {
                // These collections have no RuntimeOwner in frontend-contract-v1. Do not
                // project global work into the selected Lane until Core supplies ownership.
                tasks: Vec::new(),
                tools: Vec::new(),
                approvals: view
                    .pending_approvals
                    .iter()
                    .filter(|approval| selected_owner == Some(&approval.owner))
                    .map(|approval| D1ApprovalProjection {
                        id: approval.id.clone(),
                        title: approval.title.clone(),
                        risk: format!("{:?}", approval.risk).to_lowercase(),
                    })
                    .collect(),
                queued_inputs: Vec::new(),
                evidence: Vec::new(),
            },
            transcript,
            workspace_eligibility: view.workspace_eligibility.as_ref().map(|eligibility| {
                D1WorkspaceEligibilityProjection {
                    is_git_repository: eligibility.is_git_repository,
                    has_head: eligibility.has_head,
                    can_create_lane: eligibility.can_create_lane,
                    diagnostic: eligibility.diagnostic.clone(),
                }
            }),
            starter_lane_previews: view
                .starter_lane_previews
                .iter()
                .map(|preview| D1StarterLanePreviewProjection {
                    preview_id: preview.preview_id.clone(),
                    content_sha256: preview.content_sha256.clone(),
                    lane_id: preview.lane.id.clone(),
                    branch: preview.lane.branch.clone(),
                    diagnostics: preview.diagnostics.clone(),
                })
                .collect(),
            starter_lane_receipts: view
                .starter_lane_receipts
                .iter()
                .map(|receipt| D1StarterLaneReceiptProjection {
                    preview_id: receipt.preview_id.clone(),
                    lane_id: receipt.lane.id.clone(),
                })
                .collect(),
            agent_adapters: view
                .agent_adapters
                .iter()
                .filter(|adapter| adapter.route == AgentRoute::Acp)
                .map(|adapter| D1AgentAdapterProjection {
                    agent_id: adapter.agent_id.clone(),
                    display_name: adapter.display_name.clone(),
                    startability: agent_startability(adapter.startability).to_string(),
                    diagnostics: adapter.diagnostics.clone(),
                })
                .collect(),
            agent_sessions: view
                .agent_sessions
                .iter()
                .filter(|session| selected_owner == Some(&session.owner))
                .map(|session| D1AgentSessionProjection {
                    session_id: session.session_id.clone(),
                    lane_id: session.lane_id.clone(),
                    agent_id: session.agent_id.clone(),
                    model: session.model.clone(),
                    status: agent_session_status(session.status).to_string(),
                    task: session.task.clone(),
                    diagnostic: session.diagnostic.clone(),
                    output: session.output.clone(),
                })
                .collect(),
            // Session-input views do not carry a RuntimeOwner. Exclude them rather
            // than associating a global history entry with the selected Lane.
            agent_session_inputs: Vec::new(),
            cost_usage: view
                .cost_usage
                .iter()
                .map(|cost| D1CostUsageProjection {
                    usage_id: cost.usage_id.clone(),
                    attempt_index: cost.attempt_index,
                    total_tokens: cost.tokens.total_tokens.unwrap_or(0),
                    actual_cost_micro_usd: cost
                        .actual_cost
                        .as_ref()
                        .map(|amount| amount.micro_units),
                    outcome: cost.outcome.as_str().to_string(),
                })
                .collect(),
            replay_cursor: D1CursorProjection {
                stream_id: confirmed.cursor.stream_id.clone(),
                sequence: confirmed.cursor.sequence,
            },
            composer: D1ComposerProjection {
                editable: supports_owner && selected_owner.is_some(),
                busy,
                can_cancel: supports_owner
                    && selected_lane.is_some_and(|lane| lane.is_active())
                    && exact_binding.is_some(),
                can_submit_immediately: supports_owner && selected_owner.is_some() && !busy,
            },
            permission_dock: match selected_owner {
                Some(owner) => self.permission_dock_for_owner(owner)?,
                None => self.empty_permission_dock()?,
            },
            recovery,
            unavailable_features: unavailable_features(),
        })
    }
}

/// A GUI mutation may use an owner only when Core publishes exactly one binding for that Lane.
fn exact_owner_binding<'a>(
    view: &'a RuntimeViewState,
    lane_id: &str,
) -> Option<&'a viden_core::LaneRuntimeOwnerBinding> {
    let mut bindings = view
        .lane_runtime_owners
        .iter()
        .filter(|binding| binding.lane_id == lane_id);
    let binding = bindings.next()?;
    (bindings.next().is_none() && binding.owner.lane_id.as_deref() == Some(lane_id))
        .then_some(binding)
}

fn exact_owner_count(view: &RuntimeViewState, lane_id: &str) -> usize {
    view.lane_runtime_owners
        .iter()
        .filter(|binding| binding.lane_id == lane_id)
        .count()
}

/// Restored terminal ACP sessions are durable Core facts even though process-local
/// Lane runtime owners intentionally disappear across a Core restart.
pub(crate) fn exact_terminal_agent_session<'a>(
    view: &'a RuntimeViewState,
    lane_id: &str,
) -> Option<&'a viden_core::AgentSessionView> {
    let mut sessions = view.agent_sessions.iter().filter(|session| {
        session.lane_id == lane_id
            && session.owner.lane_id.as_deref() == Some(lane_id)
            && view
                .agent_adapters
                .iter()
                .find(|adapter| adapter.agent_id == session.agent_id)
                .map_or(session.agent_id != "viden-built-in", |adapter| {
                    adapter.route == AgentRoute::Acp
                })
            && matches!(
                session.status,
                AgentSessionStatus::Completed
                    | AgentSessionStatus::Failed
                    | AgentSessionStatus::Cancelled
            )
    });
    let session = sessions.next()?;
    sessions.next().is_none().then_some(session)
}

fn project_projection(probe: &ProjectProbe) -> D11ProjectProjection {
    D11ProjectProjection {
        root: probe.root.clone(),
        is_git_repository: probe.is_git_repository,
        config_state: match probe.config_state {
            viden_core::ProjectConfigState::Missing => "missing",
            viden_core::ProjectConfigState::Valid => "valid",
            viden_core::ProjectConfigState::Invalid => "invalid",
        },
        project_name: probe.project_name.clone(),
        mode: probe.pack.clone(),
        diagnostics: probe.diagnostics.clone(),
    }
}

fn config_projection(preview: &ProjectConfigPreview) -> D11ConfigProjection {
    D11ConfigProjection {
        preview_id: preview.preview_id.clone(),
        relative_path: preview.relative_path.clone(),
        content_sha256: preview.content_sha256.clone(),
        exact_contents: preview.exact_contents.clone(),
        valid: preview.is_valid() && preview.exact_contents.is_some(),
        diagnostics: preview.diagnostics.clone(),
    }
}

fn confirmed_config_projection(preview: &ProjectConfigPreview) -> D11ConfirmedConfigProjection {
    D11ConfirmedConfigProjection {
        preview_id: preview.preview_id.clone(),
        relative_path: preview.relative_path.clone(),
        content_sha256: preview.content_sha256.clone(),
    }
}

fn provider_projection(provider: &ProviderHealthView) -> D11ProviderProjection {
    D11ProviderProjection {
        provider_id: provider.provider_id.clone(),
        model: provider.model.clone(),
        status: provider.status.clone(),
        warning: !matches!(provider.status.as_str(), "healthy" | "ready" | "available"),
    }
}

fn credential_projection(handle: &CredentialHandle) -> D11CredentialProjection {
    D11CredentialProjection {
        provider_id: handle.provider_id.clone(),
        masked_handle: mask_handle(&handle.backend_id),
        status: match handle.status {
            viden_core::CredentialStatus::Available => "available",
            viden_core::CredentialStatus::Missing => "missing",
            viden_core::CredentialStatus::Locked => "locked",
            viden_core::CredentialStatus::Error => "error",
        },
    }
}

fn mask_handle(handle: &str) -> String {
    let chars = handle.chars().collect::<Vec<_>>();
    if chars.len() <= 4 {
        return "••••".to_string();
    }
    format!(
        "{}{}••••{}{}",
        chars[0],
        chars[1],
        chars[chars.len() - 2],
        chars[chars.len() - 1]
    )
}

fn starter_lane_projection(lane: &AgentLaneRecord) -> D11StarterLaneProjection {
    D11StarterLaneProjection {
        id: lane.id.clone(),
        role: lane.role.to_string(),
        status: format!("{:?}", lane.status).to_lowercase(),
    }
}

fn lane_status(status: viden_core::LaneStatus) -> &'static str {
    match status {
        viden_core::LaneStatus::Draft => "draft",
        viden_core::LaneStatus::Queued => "queued",
        viden_core::LaneStatus::Starting => "starting",
        viden_core::LaneStatus::Running => "running",
        viden_core::LaneStatus::WaitingApproval => "waiting_approval",
        viden_core::LaneStatus::NeedsInput => "needs_input",
        viden_core::LaneStatus::Blocked => "blocked",
        viden_core::LaneStatus::Attached => "attached",
        viden_core::LaneStatus::Detached => "detached",
        viden_core::LaneStatus::Done => "done",
        viden_core::LaneStatus::Failed => "failed",
        viden_core::LaneStatus::Cancelled => "cancelled",
        viden_core::LaneStatus::Archived => "archived",
    }
}

fn agent_startability(startability: AgentStartability) -> &'static str {
    match startability {
        AgentStartability::Ready => "ready",
        AgentStartability::ProbeRequired => "probe_required",
        AgentStartability::InstallRequired => "install_required",
        AgentStartability::AuthenticationRequired => "authentication_required",
        AgentStartability::Unavailable => "unavailable",
    }
}

fn agent_session_status(status: AgentSessionStatus) -> &'static str {
    match status {
        AgentSessionStatus::Starting => "starting",
        AgentSessionStatus::Running => "running",
        AgentSessionStatus::WaitingApproval => "waiting_approval",
        AgentSessionStatus::Completed => "completed",
        AgentSessionStatus::Failed => "failed",
        AgentSessionStatus::Cancelled => "cancelled",
    }
}

fn workspace_source_status(status: WorkspaceSourceStatus) -> &'static str {
    match status {
        WorkspaceSourceStatus::Ready => "ready",
        WorkspaceSourceStatus::Unavailable => "unavailable",
        WorkspaceSourceStatus::Truncated => "truncated",
    }
}

fn runtime_service_kind(kind: RuntimeServiceKind) -> &'static str {
    match kind {
        RuntimeServiceKind::Mcp => "mcp",
        RuntimeServiceKind::Lsp => "lsp",
    }
}

fn runtime_service_status(status: RuntimeServiceStatus) -> &'static str {
    match status {
        RuntimeServiceStatus::Connected => "connected",
        RuntimeServiceStatus::Ready => "ready",
        RuntimeServiceStatus::Degraded => "degraded",
        RuntimeServiceStatus::Offline => "offline",
        RuntimeServiceStatus::Unavailable => "unavailable",
    }
}

fn workspace_change_kind(kind: WorkspaceChangeKind) -> &'static str {
    match kind {
        WorkspaceChangeKind::Added => "added",
        WorkspaceChangeKind::Modified => "modified",
        WorkspaceChangeKind::Deleted => "deleted",
        WorkspaceChangeKind::Renamed => "renamed",
        WorkspaceChangeKind::Untracked => "untracked",
    }
}

fn check_run_status(status: CheckRunStatus) -> &'static str {
    match status {
        CheckRunStatus::Queued => "queued",
        CheckRunStatus::Running => "running",
        CheckRunStatus::Passed => "passed",
        CheckRunStatus::Failed => "failed",
        CheckRunStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use viden_core::{
        AgentLaneRecord, AgentRole, AgentRoute, AgentSessionStatus, AgentSessionView,
        COCKPIT_CONTEXT_CAPABILITY, CapabilityId, DataEgressPolicy, EventCursor, ExecutionTarget,
        FRONTEND_SCHEMA_V1, GateStrength, LaneBudget, LaneStatus, MutationPolicy, PermissionLevel,
        PermissionMode, ResolvedUiPreferences, RuntimeOwner, RuntimeSnapshot,
        RuntimeSnapshotEnvelope, RuntimeViewState, WorkMode,
    };

    use crate::d1::D1_OWNER_CAPABILITY;

    use super::RuntimeProjection;

    #[test]
    fn d1_projects_restored_completed_acp_output_without_a_live_lane_owner() {
        let snapshot = RuntimeSnapshot {
            cwd: PathBuf::from("/workspace/viden"),
            provider_family: "deepseek".to_string(),
            model_label: "deepseek-v4-flash".to_string(),
            work_mode: WorkMode::Build,
            permission_mode: PermissionMode::Default,
            permission_level: PermissionLevel::Ask,
            config_summary: String::new(),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
            ui_preferences: ResolvedUiPreferences::default(),
        };
        let owner = RuntimeOwner {
            workspace_id: "workspace-contract-v1".to_string(),
            project_id: "project-viden".to_string(),
            lane_id: Some("lane-acp".to_string()),
            session_id: Some("session-acp".to_string()),
            task_id: None,
            turn_id: None,
        };
        let mut view = RuntimeViewState::new(snapshot.clone());
        view.lanes.push(AgentLaneRecord {
            id: "lane-acp".to_string(),
            task_id: None,
            role: AgentRole::Coder,
            route: AgentRoute::Acp,
            gate_strength: GateStrength::Full,
            mutation_policy: MutationPolicy::ProposeOnly,
            worktree: Some("/workspace/viden/.worktrees/lane-acp".to_string()),
            branch: Some("viden/lane-acp".to_string()),
            target: ExecutionTarget::Local,
            data_egress: DataEgressPolicy::Deny,
            status: LaneStatus::Done,
            budget: LaneBudget::default(),
            active_session_ids: vec!["session-acp".to_string()],
            summary: "Return an exact response".to_string(),
            evidence: Vec::new(),
        });
        view.agent_sessions.push(AgentSessionView {
            session_id: "session-acp".to_string(),
            lane_id: "lane-acp".to_string(),
            agent_id: "codex-acp".to_string(),
            model: None,
            status: AgentSessionStatus::Completed,
            owner,
            task: "Return an exact response".to_string(),
            diagnostic: None,
            output: Some("ACP-GUI-CLOSED-LOOP-OK".to_string()),
        });
        let mut projection = RuntimeProjection::default();
        projection.replace(RuntimeSnapshotEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            capabilities: BTreeSet::from([
                CapabilityId(D1_OWNER_CAPABILITY.to_string()),
                CapabilityId(COCKPIT_CONTEXT_CAPABILITY.to_string()),
            ]),
            cursor: EventCursor {
                stream_id: "stream-acp".to_string(),
                sequence: 1,
            },
            snapshot,
            view,
        });

        let cockpit = projection
            .d1_cockpit(Some("lane-acp"))
            .expect("D1 cockpit projection");

        assert_eq!(
            cockpit.agent_sessions[0].output.as_deref(),
            Some("ACP-GUI-CLOSED-LOOP-OK")
        );
        assert_eq!(
            cockpit
                .context_dock
                .lane_agent
                .as_ref()
                .and_then(|agent| agent.session_id.as_deref()),
            Some("session-acp")
        );
    }
}
