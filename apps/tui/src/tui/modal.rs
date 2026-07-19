use super::{
    canvas::Frame,
    command_palette::render_command_suggestions,
    keymap::OverlayKind,
    panel::panel,
    state::{InteractionPanel, TuiState},
    text::truncate,
};

pub(super) const DEFAULT_APPROVAL_FOCUS: usize = 3;
const APPROVAL_FOCUS_APPLY_ALL: usize = 0;
const APPROVAL_FOCUS_DENY: usize = 1;
const APPROVAL_FOCUS_DIFF: usize = 2;
const APPROVAL_FOCUS_APPROVE: usize = 3;

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
        let block = panel(
            title,
            global_overlay_rows(state, overlay.kind, &overlay.filter),
            frame.width.min(76),
            10,
            Some("Esc close · arrows move · Enter open"),
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
            "[Deny (n)]  [Diff]  [Approve (y)]".to_string(),
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
            Some(InteractionPanel::ConnectProvider { .. }) => "Connect a provider",
            Some(InteractionPanel::ProviderApiKey { .. }) => "API key",
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
        OverlayKind::Decisions => state
            .runtime
            .pending_approvals
            .iter()
            .map(|approval| format!("{}  {}", approval.id, approval.title))
            .collect(),
        OverlayKind::ContextHelp => vec![
            "i Insert · Esc back · ? help".to_string(),
            "Ctrl-L lanes · Ctrl-S sessions · Ctrl-T new session".to_string(),
            "Ctrl-K commands · Ctrl-B board · Ctrl-G decisions".to_string(),
            "Ctrl-C cancel current work · double Ctrl-C exit confirm".to_string(),
        ],
        OverlayKind::ExitConfirm => vec![
            "No current work is active.".to_string(),
            "Press Enter to exit or Esc to stay.".to_string(),
        ],
        OverlayKind::Session => {
            vec!["Session navigation awaits the Core session list.".to_string()]
        }
        OverlayKind::NewSession => {
            vec!["New session awaits the Core command contract.".to_string()]
        }
        OverlayKind::CommandPalette | OverlayKind::ComposerCommands => vec![
            "Ctrl-L lanes".to_string(),
            "Ctrl-S sessions".to_string(),
            "Ctrl-B board".to_string(),
            "Ctrl-G decisions".to_string(),
        ],
        OverlayKind::Approval | OverlayKind::InteractionPanel => Vec::new(),
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

fn interaction_rows(state: &TuiState) -> Vec<String> {
    match state.ui.interaction_panel.as_ref() {
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
            "API key".to_string(),
            "Endpoint".to_string(),
        ],
        Some(InteractionPanel::ProviderApiKey { provider_id, input }) => vec![
            format!("provider {provider_id}"),
            state
                .ui
                .provider_catalog
                .iter()
                .find(|provider| provider.provider_id == *provider_id)
                .and_then(|provider| provider.api_key_env.clone())
                .unwrap_or_else(|| format!("{}_API_KEY", provider_id.to_ascii_uppercase())),
            format!("key {}", "*".repeat(input.len())),
            "Enter submit".to_string(),
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
    interaction_rows(state).len()
}

pub(super) fn selected_interaction_command(state: &TuiState) -> Option<String> {
    match state.ui.interaction_panel.as_ref()? {
        InteractionPanel::ConnectProvider { selected, .. } => state
            .ui
            .provider_catalog
            .get(*selected)
            .map(|provider| format!("/provider use {}", provider.provider_id)),
        InteractionPanel::ProviderConfig { provider_id, .. } => {
            Some(format!("/provider configure {provider_id}"))
        }
        InteractionPanel::ProviderApiKey { provider_id, input } => {
            Some(format!("/provider key {provider_id} {input}"))
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
    has_pending_approval(state).then_some({
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
    has_pending_approval(state).then(|| {
        let column = match focused_approval_action(state) {
            ApprovalAction::ToggleApplyAll => 8,
            ApprovalAction::Deny => 18,
            ApprovalAction::Diff => 34,
            ApprovalAction::Approve => 52,
        };
        (column.min(width.saturating_sub(1)), 10)
    })
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
}
