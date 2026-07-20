use super::{
    canvas::Frame,
    command_palette::render_command_suggestions,
    jump::JumpIndex,
    keymap::OverlayKind,
    panel::panel,
    projection::CockpitProjection,
    state::{InteractionPanel, TuiState, has_active_work},
    text::truncate,
};

pub(super) const DEFAULT_APPROVAL_FOCUS: usize = 3;
const APPROVAL_FOCUS_APPLY_ALL: usize = 0;
const APPROVAL_FOCUS_DENY: usize = 1;
const APPROVAL_FOCUS_DIFF: usize = 2;
const APPROVAL_FOCUS_APPROVE: usize = 3;
const GLOBAL_JUMP_VISIBLE_ROWS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApprovalAction {
    ToggleApplyAll,
    Deny,
    Diff,
    Approve,
}

pub(super) fn render_overlays(frame: &mut Frame, state: &TuiState, _right_rail_width: usize) {
    if let Some(overlay) = state.ui.overlay.as_ref() {
        let title = match overlay.kind {
            OverlayKind::GlobalJump => "GLOBAL JUMP",
            OverlayKind::Lane => "LANES",
            OverlayKind::Session => "SESSIONS",
            OverlayKind::NewSession => "NEW SESSION",
            OverlayKind::CommandPalette => "COMMANDS",
            OverlayKind::Board => "BOARD",
            OverlayKind::Decisions => "DECISIONS",
            OverlayKind::ContextHelp => "CONTEXT HELP",
            OverlayKind::ExitConfirm => "EXIT CONFIRMATION",
            OverlayKind::Approval => "APPROVAL",
            OverlayKind::InteractionPanel => "SELECT",
            OverlayKind::ComposerCommands => "COMMANDS",
        };
        let overlay_height = match overlay.kind {
            OverlayKind::Approval | OverlayKind::Decisions => 14,
            _ => 10,
        };
        let block = panel(
            title,
            global_overlay_rows(state, overlay.kind, &overlay.filter),
            frame.width.min(76),
            overlay_height,
            Some(if overlay.kind == OverlayKind::GlobalJump {
                "Esc restore · arrows/jk move · Enter jump"
            } else {
                "Esc close · arrows move · Enter open"
            }),
        );
        frame.write_block(
            4,
            frame.width.saturating_sub(frame.width.min(76)) / 2,
            &block,
        );
    } else if let Some(approval) = state.runtime.pending_approvals.first() {
        let rows = vec![
            format!("{} · {:?}", approval.title, approval.risk),
            truncate(&approval.message, 68),
            format!("TARGET  {}", truncate(&approval.target.display, 56)),
            format!("INPUT   {}", truncate(&approval.input_preview, 56)),
            "PINNED · Ctrl-G Decisions · Enter select to focus".to_string(),
        ];
        let block = panel(
            "APPROVAL",
            rows,
            frame.width.min(76),
            8,
            Some(&approval.audit_id),
        );
        frame.write_block(
            4,
            frame.width.saturating_sub(frame.width.min(76)) / 2,
            &block,
        );
    } else if let Some(lane) = state
        .ui
        .focused_lane
        .as_ref()
        .and_then(|lane_id| state.runtime.lanes.iter().find(|lane| &lane.id == lane_id))
    {
        let mut rows = vec![
            format!("{} {}", lane.id, lane.role),
            "ROUTE main→side-1".to_string(),
            format!("STATE  {:?}", lane.status),
        ];
        rows.extend(lane.evidence.iter().cloned());
        rows.push("CONTROL [stop] [tmux] [pty] [send] [inspect]".to_string());
        let block = panel("LANE DETAIL", rows, frame.width.min(76), 10, Some(&lane.id));
        frame.write_block(
            4,
            frame.width.saturating_sub(frame.width.min(76)) / 2,
            &block,
        );
    } else if state.ui.interaction_panel.is_some() {
        let title = match state.ui.interaction_panel.as_ref() {
            Some(InteractionPanel::Setup { .. }) => "SETUP SELECTOR",
            Some(InteractionPanel::ConnectProvider { .. }) => "Connect a provider",
            Some(InteractionPanel::ModelPicker { .. }) => "Select model",
            Some(InteractionPanel::ProviderConfig { .. }) => "SELECT",
            None => unreachable!("panel presence checked above"),
        };
        let block = panel(
            title,
            interaction_rows(state),
            frame.width.min(72),
            10,
            None,
        );
        frame.write_block(
            4,
            frame.width.saturating_sub(frame.width.min(72)) / 2,
            &block,
        );
    } else {
        render_command_suggestions(frame, state);
    }
}

fn global_overlay_rows(state: &TuiState, kind: OverlayKind, filter: &str) -> Vec<String> {
    let mut rows = match kind {
        OverlayKind::GlobalJump => return global_jump_rows(state, filter),
        OverlayKind::Lane => state
            .runtime
            .lanes
            .iter()
            .map(|lane| format!("{}  {}  {:?}", lane.id, lane.role, lane.status))
            .collect(),
        OverlayKind::Board => state
            .runtime
            .tasks
            .iter()
            .map(|task| format!("{}  {}  {}", task.id, task.role, task.status.as_str()))
            .collect(),
        OverlayKind::Decisions => decision_center_rows(state),
        OverlayKind::ContextHelp => vec![
            "i Insert · Esc back · ? help".to_string(),
            "Ctrl-L lanes · Ctrl-S sessions · Ctrl-T new session".to_string(),
            "Ctrl-K commands · Ctrl-B board · Ctrl-G decisions".to_string(),
            "Ctrl-C cancel current work · double Ctrl-C exit confirm".to_string(),
        ],
        OverlayKind::ExitConfirm if has_active_work(state) => vec![
            "Active work is still running; exit is blocked.".to_string(),
            "Wait for Core to resolve it or expose a cancellable owner.".to_string(),
            "Press Esc to stay.".to_string(),
        ],
        OverlayKind::ExitConfirm => vec![
            "No current work is active.".to_string(),
            "Press Enter to exit or Esc to stay.".to_string(),
        ],
        OverlayKind::Session => state
            .ui
            .focused_lane
            .as_ref()
            .and_then(|lane_id| state.runtime.lanes.iter().find(|lane| &lane.id == lane_id))
            .map(|lane| {
                lane.active_session_ids
                    .iter()
                    .map(|session_id| format!("{}  {}", lane.id, session_id))
                    .collect()
            })
            .unwrap_or_default(),
        OverlayKind::NewSession => {
            vec!["New session awaits the Core command contract.".to_string()]
        }
        OverlayKind::CommandPalette | OverlayKind::ComposerCommands => vec![
            "Ctrl-L lanes".to_string(),
            "Ctrl-S sessions".to_string(),
            "Ctrl-B board".to_string(),
            "Ctrl-G decisions".to_string(),
        ],
        OverlayKind::Approval => focused_approval_rows(state),
        OverlayKind::InteractionPanel => Vec::new(),
    };
    if !filter.is_empty() {
        let needle = filter.to_ascii_lowercase();
        rows.retain(|row| row.to_ascii_lowercase().contains(&needle));
        rows.insert(0, format!("filter: {filter}"));
    }
    if rows.is_empty() {
        rows.push("No matching items.".to_string());
    }
    rows
}

fn decision_center_rows(state: &TuiState) -> Vec<String> {
    let projection = CockpitProjection::from(&state.runtime, &state.ui);
    let mut rows = projection
        .approval_actions
        .iter()
        .map(|approval| {
            let title = projection
                .approvals
                .iter()
                .find(|request| request.id == approval.request_id)
                .map_or("approval", |request| request.title.as_str());
            format!(
                "APPROVAL {} · {} · {:?} · AUDIT {}",
                approval.request_id, title, approval.expiry, approval.audit_id
            )
        })
        .collect::<Vec<_>>();
    rows.extend(projection.merge_gates.iter().map(|gate| {
        format!(
            "GATE {} · {:?} · {:?}",
            gate.gate_id, gate.status, gate.decision
        )
    }));
    rows.extend(projection.recovery_actions.iter().map(|recovery| {
        format!(
            "RECOVERY {} · {} · {}",
            recovery.lane_id.as_deref().unwrap_or("runtime"),
            recovery.reason,
            recovery.action
        )
    }));
    if let Some(command) = projection.pending_command.as_ref() {
        rows.push(format!(
            "COMMAND {} · pending Core fact",
            command.command_id
        ));
    }
    rows.extend(
        projection
            .errors
            .iter()
            .map(|error| format!("ERROR {}", error.message)),
    );
    rows
}

fn global_jump_rows(state: &TuiState, filter: &str) -> Vec<String> {
    let index = JumpIndex::from_view(&state.runtime);
    let mut rows = Vec::new();
    let mut previous_kind = None;
    for (position, item) in index.search(filter).into_iter().enumerate() {
        if previous_kind != Some(item.kind) {
            rows.push((format!("[{}]", item.kind.label()), None, item.kind));
            previous_kind = Some(item.kind);
        }
        let marker = state
            .ui
            .overlay
            .as_ref()
            .is_some_and(|overlay| overlay.selected == position);
        let detail = item.disabled_reason.as_deref().unwrap_or(&item.context);
        rows.push((
            format!(
                "{} {:<16} {}{}",
                if marker { ">" } else { " " },
                truncate(&item.title, 16),
                truncate(detail, 42),
                if item.enabled { "" } else { " · unavailable" },
            ),
            Some(position),
            item.kind,
        ));
    }
    if rows.is_empty() {
        return vec!["No matching items. Try : @ # > or ~.".to_string()];
    }
    let selected = state
        .ui
        .overlay
        .as_ref()
        .map_or(0, |overlay| overlay.selected);
    let selected_row = rows
        .iter()
        .position(|(_, result_index, _)| *result_index == Some(selected))
        .unwrap_or(0);
    let start = selected_row
        .saturating_sub(GLOBAL_JUMP_VISIBLE_ROWS / 2)
        .min(rows.len().saturating_sub(GLOBAL_JUMP_VISIBLE_ROWS));
    let mut visible = rows
        .iter()
        .skip(start)
        .take(GLOBAL_JUMP_VISIBLE_ROWS)
        .map(|(text, _, _)| text.clone())
        .collect::<Vec<_>>();
    if start > 0 && rows[start].1.is_some() && rows[start - 1].2 == rows[start].2 {
        visible.insert(0, format!("[{}] · continued", rows[start].2.label()));
        if visible
            .iter()
            .position(|row| row.starts_with("> "))
            .is_some_and(|position| position >= GLOBAL_JUMP_VISIBLE_ROWS)
        {
            visible.remove(1);
        }
        visible.truncate(GLOBAL_JUMP_VISIBLE_ROWS);
    }
    visible
}

fn interaction_rows(state: &TuiState) -> Vec<String> {
    match state.ui.interaction_panel.as_ref() {
        Some(InteractionPanel::Setup { selected, draft }) => {
            let mut rows = vec![
                setup_action_row(*selected, 0, "Probe project through Core"),
                setup_action_row(*selected, 1, "Preview exact draft through Core"),
            ];
            if state
                .runtime
                .project_config_preview
                .as_ref()
                .is_some_and(|preview| setup_preview_matches_draft(preview, draft))
            {
                let preview = state.runtime.project_config_preview.as_ref().unwrap();
                rows.push(setup_action_row(
                    *selected,
                    2,
                    &format!("Confirm {} through Core", preview.relative_path),
                ));
            }
            rows.push("DRAFT viden.toml (paste/type to edit)".to_string());
            rows.extend(draft.lines().map(|line| format!("  {line}")));
            rows
        }
        Some(InteractionPanel::ConnectProvider { search, .. }) => state
            .ui
            .provider_catalog
            .iter()
            .filter(|provider| {
                provider.provider_id.contains(search)
                    || provider
                        .display_name
                        .to_ascii_lowercase()
                        .contains(&search.to_ascii_lowercase())
            })
            .map(|provider| format!("{}  {}", provider.provider_id, provider.display_name))
            .collect(),
        Some(InteractionPanel::ProviderConfig { provider_id, .. }) => vec![
            format!("configure {provider_id}"),
            "credential handles are read-only".to_string(),
            "trusted ingress unavailable".to_string(),
        ],
        Some(InteractionPanel::ModelPicker {
            provider_id,
            search,
            ..
        }) => state
            .ui
            .provider_catalog
            .iter()
            .filter(|provider| !provider.enabled_models.is_empty())
            .filter(|provider| {
                provider_id
                    .as_ref()
                    .is_none_or(|id| id == &provider.provider_id)
            })
            .flat_map(|provider| {
                provider
                    .enabled_models
                    .iter()
                    .filter(|model| model.contains(search))
                    .map(|model| {
                        format!(
                            "{}  {model}  {}",
                            provider.provider_id, provider.display_name
                        )
                    })
            })
            .collect(),
        None => Vec::new(),
    }
}

fn setup_action_row(selected: usize, index: usize, label: &str) -> String {
    format!("{} {label}", if selected == index { ">" } else { " " })
}

fn focused_approval_rows(state: &TuiState) -> Vec<String> {
    focused_approval_request(state).map_or_else(Vec::new, |approval| {
        let once = approval
            .allowed_scopes
            .iter()
            .any(|scope| matches!(scope, viden_core::ApprovalScope::Once));
        let session = approval
            .allowed_scopes
            .iter()
            .any(|scope| matches!(scope, viden_core::ApprovalScope::Session { .. }));
        let repo = approval
            .allowed_scopes
            .iter()
            .any(|scope| matches!(scope, viden_core::ApprovalScope::RepoAllowlist { .. }));
        let availability = |available: bool| if available { "" } else { " · unavailable" };
        let expiry = if approval_is_expired(approval) {
            "EXPIRED · default Deny · awaiting Core ApprovalResolved".to_string()
        } else if approval.expires_at > 0 {
            format!("auto-deny @{} · default Deny", approval.expires_at)
        } else {
            "default Deny · no local expiry action".to_string()
        };
        vec![
            format!("{} · {:?}", approval.title, approval.risk),
            truncate(&approval.message, 68),
            format!("TARGET  {}", truncate(&approval.target.display, 56)),
            format!("INPUT   {}", truncate(&approval.input_preview, 56)),
            format!("1 Allow once{}", availability(once)),
            format!("2 Allow for session{}", availability(session)),
            format!("3 Add repo allowlist{}", availability(repo)),
            "4 Deny".to_string(),
            expiry,
            format!("AUDIT   {}", approval.audit_id),
        ]
    })
}

pub(super) fn focused_approval_request(
    state: &TuiState,
) -> Option<&viden_core::ApprovalRequestView> {
    let overlay = state
        .ui
        .overlay
        .as_ref()
        .filter(|overlay| overlay.kind == OverlayKind::Approval)?;
    overlay.selected_id.as_ref().map_or_else(
        || state.runtime.pending_approvals.get(overlay.selected),
        |request_id| {
            state
                .runtime
                .pending_approvals
                .iter()
                .find(|approval| &approval.id == request_id)
        },
    )
}

pub(super) fn approval_is_expired(approval: &viden_core::ApprovalRequestView) -> bool {
    if approval.expires_at == 0 {
        return false;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| approval.expires_at <= duration.as_secs())
        .unwrap_or(false)
}

pub(super) fn interaction_panel_index_at(
    state: &TuiState,
    _width: u16,
    _height: u16,
    _rail: usize,
    _column: u16,
    row: u16,
) -> Option<usize> {
    state.ui.interaction_panel.as_ref()?;
    let index = usize::from(row.saturating_sub(5));
    (index < interaction_panel_choice_count(state)).then_some(index)
}

pub(super) fn interaction_panel_choice_count(state: &TuiState) -> usize {
    match state.ui.interaction_panel.as_ref() {
        Some(InteractionPanel::Setup { .. }) => {
            let draft = match state.ui.interaction_panel.as_ref() {
                Some(InteractionPanel::Setup { draft, .. }) => draft,
                _ => unreachable!("setup panel matched"),
            };
            2 + usize::from(
                state
                    .runtime
                    .project_config_preview
                    .as_ref()
                    .is_some_and(|preview| setup_preview_matches_draft(preview, draft)),
            )
        }
        _ => interaction_rows(state).len(),
    }
}

fn setup_preview_matches_draft(preview: &viden_core::ProjectConfigPreview, draft: &str) -> bool {
    preview.is_valid() && preview.exact_contents.as_deref() == Some(draft)
}

pub(super) fn selected_interaction_command(state: &TuiState) -> Option<String> {
    match state.ui.interaction_panel.as_ref()? {
        InteractionPanel::Setup { .. } => None,
        InteractionPanel::ConnectProvider { selected, .. } => state
            .ui
            .provider_catalog
            .get(*selected)
            .map(|provider| format!("/provider use {}", provider.provider_id)),
        InteractionPanel::ProviderConfig { provider_id, .. } => {
            Some(format!("/provider configure {provider_id}"))
        }
        InteractionPanel::ModelPicker { selected, .. } => {
            interaction_rows(state).get(*selected).and_then(|row| {
                row.split_whitespace()
                    .collect::<Vec<_>>()
                    .get(..2)
                    .map(|parts| format!("/model use {} {}", parts[0], parts[1]))
            })
        }
    }
}

pub(super) fn has_pending_approval(state: &TuiState) -> bool {
    !state.runtime.pending_approvals.is_empty()
}

pub(super) fn approval_action_at(
    state: &TuiState,
    _width: u16,
    _height: u16,
    _rail: usize,
    column: u16,
    _row: u16,
) -> Option<ApprovalAction> {
    has_explicit_approval_focus(state).then_some({
        if column < 28 {
            ApprovalAction::Deny
        } else if column < 48 {
            ApprovalAction::Diff
        } else {
            ApprovalAction::Approve
        }
    })
}

pub(super) fn approval_focus_cursor(
    state: &TuiState,
    width: u16,
    _height: u16,
    _rail: usize,
) -> Option<(u16, u16)> {
    has_explicit_approval_focus(state).then(|| {
        let column = match focused_approval_action(state) {
            ApprovalAction::ToggleApplyAll => 8,
            ApprovalAction::Deny => 18,
            ApprovalAction::Diff => 34,
            ApprovalAction::Approve => 52,
        };
        (column.min(width.saturating_sub(1)), 10)
    })
}

fn has_explicit_approval_focus(state: &TuiState) -> bool {
    state
        .ui
        .overlay
        .as_ref()
        .is_some_and(|overlay| overlay.kind == OverlayKind::Approval)
        && !focused_approval_rows(state).is_empty()
}

pub(super) fn move_approval_focus(state: &mut TuiState, delta: i8) {
    state.ui.approval_focus = if delta < 0 {
        state.ui.approval_focus.saturating_sub(1)
    } else {
        (state.ui.approval_focus + 1).min(APPROVAL_FOCUS_APPROVE)
    };
}

pub(super) fn set_approval_focus_for_action(state: &mut TuiState, action: ApprovalAction) {
    state.ui.approval_focus = match action {
        ApprovalAction::ToggleApplyAll => APPROVAL_FOCUS_APPLY_ALL,
        ApprovalAction::Deny => APPROVAL_FOCUS_DENY,
        ApprovalAction::Diff => APPROVAL_FOCUS_DIFF,
        ApprovalAction::Approve => APPROVAL_FOCUS_APPROVE,
    };
}

pub(super) fn focused_approval_action(state: &TuiState) -> ApprovalAction {
    match state.ui.approval_focus {
        APPROVAL_FOCUS_APPLY_ALL => ApprovalAction::ToggleApplyAll,
        APPROVAL_FOCUS_DENY => ApprovalAction::Deny,
        APPROVAL_FOCUS_DIFF => ApprovalAction::Diff,
        _ => ApprovalAction::Approve,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::OverlayState;
    use viden_core::{
        ApprovalDefaultAction, ApprovalRequestView, ApprovalRisk, ApprovalScope, ApprovalTarget,
    };
    use viden_types::{
        LaneRecoveryView, RuntimeCommand, RuntimeCommandReceipt, RuntimeEventEnvelope,
        RuntimeSnapshot, RuntimeWireEvent,
    };

    #[test]
    fn approvals_are_not_inferred_from_transcript() {
        let mut state = TuiState::default();
        state.ui.entries.push(super::super::state::TuiEntry {
            label: "approval".to_string(),
            body: "Press y".to_string(),
        });
        assert!(!has_pending_approval(&state));
    }

    #[test]
    fn models_selector_filters_unconfigured_providers() {
        let mut state = TuiState::default();
        let mut catalog = super::super::state::ProviderOption::fixture();
        let unconfigured = catalog
            .iter_mut()
            .find(|provider| provider.provider_id == "anthropic")
            .expect("anthropic fixture");
        unconfigured.enabled_models.clear();
        state.ui.provider_catalog = catalog;
        state.ui.interaction_panel = Some(InteractionPanel::ModelPicker {
            provider_id: None,
            search: String::new(),
            selected: 0,
        });

        let rows = interaction_rows(&state).join("\n");

        assert!(!rows.contains("anthropic"));
        assert!(rows.contains("deepseek"));
    }

    #[test]
    fn global_jump_windows_rows_to_keep_selected_item_visible() {
        let mut state = TuiState::default();
        let mut overlay = OverlayState::global_jump(None);
        overlay.selected = 10;
        state.ui.overlay = Some(overlay);
        let mut frame = Frame::new(120, 40);

        render_overlays(&mut frame, &state, 0);

        let rendered = frame.to_string();
        assert!(
            rendered.contains("> /permissions rea"),
            "selected result must stay visible inside the fixed-height panel:\n{rendered}"
        );
    }

    #[test]
    fn global_jump_empty_and_disabled_windows_keep_rows_aligned_with_selection() {
        let mut state = TuiState::default();
        let mut overlay = OverlayState::global_jump(None);
        overlay.filter = ">no-such-command".to_string();
        state.ui.overlay = Some(overlay);

        assert_eq!(
            global_jump_rows(&state, ">no-such-command"),
            vec!["No matching items. Try : @ # > or ~."]
        );

        state.ui.overlay.as_mut().expect("jump").filter = "~".to_string();
        let rows = global_jump_rows(&state, "~");
        assert_eq!(rows[0], "[FILES]");
        assert!(rows[1].starts_with("> Files unavailabl"));
        assert!(rows[1].contains("Core file inventory is unavailable."));
    }

    #[test]
    fn global_jump_window_keeps_default_disabled_tail_selected() {
        let mut state = TuiState::default();
        let mut overlay = OverlayState::global_jump(None);
        overlay.selected = 12;
        state.ui.overlay = Some(overlay);

        let rows = global_jump_rows(&state, "");

        assert!(
            rows.iter().any(|row| row.starts_with("> Files unavailabl")),
            "the disabled tail result must not be dropped by a continued header: {rows:?}"
        );
    }

    #[test]
    fn approval_overlay_renders_four_core_scopes_and_expiry_without_local_resolution() {
        let mut state = TuiState::default();
        state.runtime.pending_approvals.push(ApprovalRequestView {
            id: "approval-four".to_string(),
            tool_name: "shell".to_string(),
            title: "Dangerous command".to_string(),
            message: "requires operator choice".to_string(),
            input_preview: "git push --force".to_string(),
            is_mutating: true,
            reason: Some("protected branch".to_string()),
            owner: Default::default(),
            risk: ApprovalRisk::Critical,
            target: ApprovalTarget {
                kind: "command".to_string(),
                display: "git push --force".to_string(),
                canonical_ref: Some("command://git-push".to_string()),
            },
            allowed_scopes: vec![
                ApprovalScope::Once,
                ApprovalScope::Session {
                    session_id: "session-four".to_string(),
                },
                ApprovalScope::RepoAllowlist {
                    paths: vec!["refs/heads/main".to_string()],
                },
            ],
            policy_reason_key: "approval.protected_branch".to_string(),
            policy_reason_args: Default::default(),
            expires_at: 1,
            default_action: ApprovalDefaultAction::Deny,
            audit_id: "audit-four".to_string(),
        });
        let mut overlay = OverlayState::new(OverlayKind::Approval);
        overlay.selected_id = Some("approval-four".to_string());
        state.ui.overlay = Some(overlay);

        let rows = focused_approval_rows(&state).join("\n");

        for expected in [
            "1 Allow once",
            "2 Allow for session",
            "3 Add repo allowlist",
            "4 Deny",
            "EXPIRED",
            "awaiting Core ApprovalResolved",
            "audit-four",
        ] {
            assert!(rows.contains(expected), "missing {expected}:\n{rows}");
        }
        assert_eq!(state.runtime.pending_approvals.len(), 1);
    }

    #[test]
    fn approval_overlay_keeps_all_four_scopes_expiry_and_audit_visible() {
        let mut state = TuiState::default();
        state.runtime.pending_approvals.push(ApprovalRequestView {
            id: "approval-visible".to_string(),
            tool_name: "shell".to_string(),
            title: "Visible approval".to_string(),
            message: "all rows must remain visible".to_string(),
            input_preview: "cargo test".to_string(),
            is_mutating: true,
            reason: None,
            owner: Default::default(),
            risk: ApprovalRisk::Medium,
            target: ApprovalTarget {
                kind: "command".to_string(),
                display: "cargo test".to_string(),
                canonical_ref: None,
            },
            allowed_scopes: vec![
                ApprovalScope::Once,
                ApprovalScope::Session {
                    session_id: "session-visible".to_string(),
                },
                ApprovalScope::RepoAllowlist {
                    paths: vec!["Cargo.toml".to_string()],
                },
            ],
            policy_reason_key: "approval.test".to_string(),
            policy_reason_args: Default::default(),
            expires_at: 0,
            default_action: ApprovalDefaultAction::Deny,
            audit_id: "audit-visible-last-row".to_string(),
        });
        let mut overlay = OverlayState::new(OverlayKind::Approval);
        overlay.selected_id = Some("approval-visible".to_string());
        state.ui.overlay = Some(overlay);
        let mut frame = Frame::new(100, 30);

        render_overlays(&mut frame, &state, 0);

        let rendered = frame.to_string();
        assert!(rendered.contains("1 Allow once"));
        assert!(rendered.contains("4 Deny"));
        assert!(rendered.contains("default Deny"));
        assert!(rendered.contains("audit-visible-last-row"));
    }

    #[test]
    fn decisions_overlay_projects_typed_gates_recovery_and_pending_core_command() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            initial_snapshot: RuntimeSnapshot,
            events: Vec<RuntimeEventEnvelope>,
        }

        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/merge-gate.json"
        ))
        .expect("merge gate fixture");
        let mut runtime = viden_types::RuntimeViewState::new(fixture.initial_snapshot);
        for envelope in fixture.events {
            if let RuntimeWireEvent::Known(event) = envelope.event {
                runtime.apply_event(&event);
            }
        }
        runtime.lane_recoveries.push(LaneRecoveryView {
            lane_id: "lane-recover".to_string(),
            reason: "detached".to_string(),
            next_action: "reattach".to_string(),
            timestamp: None,
        });
        runtime.last_command = Some(RuntimeCommandReceipt {
            command_id: "cmd-review".to_string(),
            command: RuntimeCommand::CancelActiveTurn,
        });
        let mut state = TuiState::new(runtime);
        state.ui.overlay = Some(OverlayState::new(OverlayKind::Decisions));

        let rows = global_overlay_rows(&state, OverlayKind::Decisions, "").join("\n");

        assert!(rows.contains("GATE gate_merge"));
        assert!(rows.contains("Accepted"));
        assert!(rows.contains("RECOVERY lane-recover · detached · reattach"));
        assert!(rows.contains("COMMAND cmd-review · pending Core fact"));
    }

    #[test]
    fn exit_confirmation_blocks_ownerless_active_work_without_offering_enter_to_exit() {
        let mut state = TuiState::default();
        state.runtime.lanes = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
        ))
        .expect("typed lanes");

        let active = global_overlay_rows(&state, OverlayKind::ExitConfirm, "").join("\n");
        assert!(active.contains("exit is blocked"));
        assert!(active.contains("cancellable owner"));
        assert!(!active.contains("Press Enter to exit"));

        state.runtime.lanes.clear();
        let inactive = global_overlay_rows(&state, OverlayKind::ExitConfirm, "").join("\n");
        assert!(inactive.contains("No current work is active"));
        assert!(inactive.contains("Press Enter to exit"));
    }
}
