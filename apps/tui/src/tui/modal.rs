use super::{
    canvas::Frame,
    command_palette::render_command_suggestions,
    indicators::{progress_bar, status_dot},
    lane_presenter::{command_hint, interaction_hint, pid_hint, pty_label, terminal_label},
    panel::panel,
    state::{InteractionPanel, ProviderOption, TerminalLane, TuiState, lane_runtime_evidence},
    text::{char_width, horizontal, pad, truncate},
};

const APPROVAL_FOCUS_APPLY_ALL: usize = 0;
const APPROVAL_FOCUS_DENY: usize = 1;
const APPROVAL_FOCUS_DIFF: usize = 2;
const APPROVAL_FOCUS_APPROVE: usize = 3;
pub(super) const DEFAULT_APPROVAL_FOCUS: usize = APPROVAL_FOCUS_APPROVE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApprovalAction {
    ToggleApplyAll,
    Deny,
    Diff,
    Approve,
}

pub(super) fn render_overlays(frame: &mut Frame, state: &TuiState, right_rail_width: usize) {
    if let Some(lane) = focused_lane(state) {
        render_lane_modal(frame, state, lane, right_rail_width);
    }
    if let Some(approval) = latest_approval(state) {
        render_approval_modal(frame, approval, state, right_rail_width);
    } else if state.interaction_panel.is_some() {
        render_interaction_panel(frame, state, right_rail_width);
    } else {
        render_command_suggestions(frame, state);
    }
}

pub(super) fn interaction_panel_index_at(
    state: &TuiState,
    column: u16,
    row: u16,
    frame_width: u16,
    frame_height: u16,
    right_rail_width: usize,
) -> Option<usize> {
    state.interaction_panel.as_ref()?;
    let bounds = interaction_panel_bounds(
        frame_width as usize,
        frame_height as usize,
        right_rail_width,
    );
    let column = column as usize;
    let row = row as usize;
    if !(bounds.left + 2..bounds.left + bounds.width.saturating_sub(2)).contains(&column) {
        return None;
    }
    let content_row = row.checked_sub(bounds.top + 1)?;
    interaction_panel_selectable_rows(state)
        .into_iter()
        .find_map(|(selectable_row, index)| (selectable_row == content_row).then_some(index))
}

fn render_interaction_panel(frame: &mut Frame, state: &TuiState, right_rail_width: usize) {
    let bounds = interaction_panel_bounds(frame.width, frame.height, right_rail_width);
    let (title, rows, right_title) = match state.interaction_panel.as_ref() {
        Some(InteractionPanel::ConnectProvider { search, selected }) => (
            "Connect a provider",
            provider_panel_rows(state, search, *selected, bounds.width),
            "esc",
        ),
        Some(InteractionPanel::ProviderConfig {
            provider_id,
            selected,
        }) => (
            "Provider config",
            provider_config_panel_rows(state, provider_id, *selected, bounds.width),
            "esc",
        ),
        Some(InteractionPanel::ProviderApiKey { provider_id, input }) => (
            "API key",
            api_key_panel_rows(state, provider_id, input, bounds.width),
            "esc",
        ),
        Some(InteractionPanel::ModelPicker {
            provider_id,
            search,
            selected,
        }) => (
            "Select model",
            model_panel_rows(
                state,
                provider_id.as_deref(),
                search,
                *selected,
                bounds.width,
            ),
            "esc",
        ),
        None => return,
    };
    let modal = panel(title, rows, bounds.width, bounds.height, Some(right_title));
    clear_overlay_bounds(frame, bounds.top, bounds.height, bounds.transcript_width);
    render_modal_shadow(frame, bounds.top, bounds.left, bounds.width, bounds.height);
    frame.write_block(bounds.top, bounds.left, &modal);
}

fn provider_panel_rows(
    state: &TuiState,
    search: &str,
    selected: usize,
    modal_width: usize,
) -> Vec<String> {
    let choices = filtered_providers(state, search);
    let mut rows = vec![
        format!("Search {}", search_cursor(search)),
        "".to_string(),
        "Popular".to_string(),
    ];
    rows.extend(choices.iter().enumerate().map(|(index, provider)| {
        selectable_row(index, selected, &provider.display_name, modal_width)
    }));
    rows.extend([
        "".to_string(),
        "Enter select    type search    esc close".to_string(),
    ]);
    rows
}

fn provider_config_panel_rows(
    state: &TuiState,
    provider_id: &str,
    selected: usize,
    modal_width: usize,
) -> Vec<String> {
    let provider = state
        .provider_catalog
        .iter()
        .find(|provider| provider.provider_id == provider_id);
    let display_name = provider
        .map(|provider| provider.display_name.as_str())
        .unwrap_or(provider_id);
    let key_status = provider
        .and_then(|provider| provider.api_key_env.as_deref())
        .map(|env| {
            let status = std::env::var(env)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|value| mask_api_key(&value))
                .unwrap_or_else(|| "missing".to_string());
            format!("{env}: {status}")
        })
        .unwrap_or_else(|| "not required".to_string());
    let model = provider
        .and_then(|provider| provider.default_model.as_deref())
        .unwrap_or("<choose model>");
    let actions = [
        format!("choose model    {model}"),
        "change API key".to_string(),
        "clear session key".to_string(),
        "run doctor".to_string(),
    ];
    let mut rows = vec![
        format!("{provider_id} / {display_name}"),
        format!("key: {key_status}"),
        "".to_string(),
    ];
    rows.extend(
        actions
            .iter()
            .enumerate()
            .map(|(index, label)| selectable_row(index, selected, label, modal_width)),
    );
    rows.extend([
        "".to_string(),
        "Enter apply    ↑↓ select    esc close".to_string(),
    ]);
    rows
}

fn api_key_panel_rows(
    state: &TuiState,
    provider_id: &str,
    input: &str,
    modal_width: usize,
) -> Vec<String> {
    let provider = state
        .provider_catalog
        .iter()
        .find(|provider| provider.provider_id == provider_id);
    let display_name = provider
        .map(|provider| provider.display_name.as_str())
        .unwrap_or(provider_id);
    let key_env = provider
        .and_then(|provider| provider.api_key_env.as_deref())
        .unwrap_or("API_KEY");
    vec![
        format!("{display_name} needs an API key."),
        format!("It will be used for this session via {key_env}."),
        "Viden will save the env var name, not the raw key.".to_string(),
        "".to_string(),
        format!(
            "API key {}",
            input_cursor(input, modal_width.saturating_sub(12))
        ),
        "".to_string(),
        "Enter submit    esc back".to_string(),
    ]
}

fn model_panel_rows(
    state: &TuiState,
    provider_filter: Option<&str>,
    search: &str,
    selected: usize,
    modal_width: usize,
) -> Vec<String> {
    let choices = filtered_models(state, provider_filter, search);
    let mut rows = vec![format!("Search {}", search_cursor(search)), "".to_string()];
    let mut last_provider = "";
    for (index, choice) in choices.iter().enumerate() {
        if choice.provider_id != last_provider {
            rows.push(choice.provider_name.clone());
            last_provider = &choice.provider_id;
        }
        let label = format!("  {}", choice.model);
        rows.push(selectable_row(index, selected, &label, modal_width));
    }
    rows.extend([
        "".to_string(),
        "Enter switch    type search    esc close".to_string(),
    ]);
    rows
}

fn selectable_row(index: usize, selected: usize, label: &str, modal_width: usize) -> String {
    let marker = if index == selected { "› " } else { "  " };
    truncate(&format!("{marker}{label}"), modal_width.saturating_sub(4))
}

fn search_cursor(value: &str) -> String {
    if value.is_empty() {
        "_".to_string()
    } else {
        format!("{value}_")
    }
}

fn input_cursor(value: &str, width: usize) -> String {
    if value.is_empty() {
        "_".to_string()
    } else {
        truncate(&format!("{}_", mask_api_key(value)), width)
    }
}

fn mask_api_key(value: &str) -> String {
    let value = value.trim();
    if value.len() <= 8 {
        "*".repeat(value.len().max(1))
    } else {
        format!(
            "{}{}{}",
            &value[..4],
            "*".repeat(value.len() - 8),
            &value[value.len() - 4..]
        )
    }
}

#[derive(Debug, Clone)]
struct ModelChoice {
    provider_id: String,
    provider_name: String,
    model: String,
}

fn interaction_panel_selectable_rows(state: &TuiState) -> Vec<(usize, usize)> {
    match state.interaction_panel.as_ref() {
        Some(InteractionPanel::ConnectProvider { search, .. }) => filtered_providers(state, search)
            .iter()
            .enumerate()
            .map(|(index, _)| (3 + index, index))
            .collect(),
        Some(InteractionPanel::ProviderConfig { .. }) => {
            (0..4).map(|index| (3 + index, index)).collect()
        }
        Some(InteractionPanel::ModelPicker {
            provider_id,
            search,
            ..
        }) => {
            let choices = filtered_models(state, provider_id.as_deref(), search);
            let mut rows = Vec::new();
            let mut content_row = 2usize;
            let mut last_provider = "";
            for (index, choice) in choices.iter().enumerate() {
                if choice.provider_id != last_provider {
                    content_row += 1;
                    last_provider = &choice.provider_id;
                }
                rows.push((content_row, index));
                content_row += 1;
            }
            rows
        }
        _ => Vec::new(),
    }
}

pub(super) fn interaction_panel_choice_count(state: &TuiState) -> usize {
    interaction_panel_selectable_rows(state).len()
}

pub(super) fn selected_interaction_command(state: &TuiState) -> Option<String> {
    match state.interaction_panel.as_ref()? {
        InteractionPanel::ConnectProvider { search, selected } => {
            let providers = filtered_providers(state, search);
            let provider = providers.get((*selected).min(providers.len().saturating_sub(1)))?;
            Some(format!("/connect {}", provider.provider_id))
        }
        InteractionPanel::ModelPicker {
            provider_id,
            search,
            selected,
        } => {
            let choices = filtered_models(state, provider_id.as_deref(), search);
            let choice = choices.get((*selected).min(choices.len().saturating_sub(1)))?;
            Some(format!("/models {} {}", choice.provider_id, choice.model))
        }
        InteractionPanel::ProviderConfig { .. } | InteractionPanel::ProviderApiKey { .. } => None,
    }
}

fn filtered_providers<'a>(state: &'a TuiState, search: &str) -> Vec<&'a ProviderOption> {
    let needle = search.trim().to_ascii_lowercase();
    let mut providers = state
        .provider_catalog
        .iter()
        .filter(|provider| provider.provider_id != "fallback")
        .filter(|provider| {
            needle.is_empty()
                || provider.provider_id.to_ascii_lowercase().contains(&needle)
                || provider.display_name.to_ascii_lowercase().contains(&needle)
        })
        .collect::<Vec<_>>();
    providers.sort_by_key(|provider| {
        (
            provider.provider_id != state.provider,
            !matches!(
                provider.provider_id.as_str(),
                "deepseek" | "dashscope-coding-plan" | "openrouter" | "openai" | "anthropic"
            ),
            provider.display_name.to_ascii_lowercase(),
        )
    });
    providers
}

fn filtered_models(
    state: &TuiState,
    provider_filter: Option<&str>,
    search: &str,
) -> Vec<ModelChoice> {
    let needle = search.trim().to_ascii_lowercase();
    let mut choices = Vec::new();
    for provider in &state.provider_catalog {
        if provider_filter.is_some_and(|filter| filter != provider.provider_id) {
            continue;
        }
        let models = if provider_filter.is_some() {
            provider_models(provider)
        } else if provider_is_available_for_model_picker(provider) {
            configured_provider_models(provider)
        } else {
            Vec::new()
        };
        for model in models {
            if needle.is_empty()
                || model.to_ascii_lowercase().contains(&needle)
                || provider.display_name.to_ascii_lowercase().contains(&needle)
            {
                choices.push(ModelChoice {
                    provider_id: provider.provider_id.clone(),
                    provider_name: provider.display_name.clone(),
                    model,
                });
            }
        }
    }
    choices.sort_by_key(|choice| {
        (
            choice.provider_id != state.provider,
            choice.provider_name.to_ascii_lowercase(),
            choice.model.to_ascii_lowercase(),
        )
    });
    choices
}

fn provider_models(provider: &ProviderOption) -> Vec<String> {
    let mut models = provider.favorite_models.clone();
    for model in &provider.enabled_models {
        if !models.contains(model) {
            models.push(model.clone());
        }
    }
    if let Some(default_model) = &provider.default_model
        && !models.contains(default_model)
    {
        models.push(default_model.clone());
    }
    for model in &provider.known_models {
        if !models.contains(model) {
            models.push(model.clone());
        }
    }
    models
}

fn active_provider_models(provider: &ProviderOption) -> Vec<String> {
    let mut models = provider.favorite_models.clone();
    for model in &provider.enabled_models {
        if !models.contains(model) {
            models.push(model.clone());
        }
    }
    models
}

fn configured_provider_models(provider: &ProviderOption) -> Vec<String> {
    let mut models = active_provider_models(provider);
    if let Some(default_model) = &provider.default_model
        && !models.contains(default_model)
    {
        models.push(default_model.clone());
    }
    for model in &provider.known_models {
        if !models.contains(model) {
            models.push(model.clone());
        }
    }
    models
}

fn provider_is_available_for_model_picker(provider: &ProviderOption) -> bool {
    !active_provider_models(provider).is_empty()
        || provider.api_key_env.as_deref().is_some_and(|env| {
            std::env::var(env)
                .ok()
                .is_some_and(|value| !value.trim().is_empty())
        })
}

#[derive(Debug, Clone, Copy)]
struct InteractionBounds {
    top: usize,
    left: usize,
    width: usize,
    height: usize,
    transcript_width: usize,
}

fn interaction_panel_bounds(
    frame_width: usize,
    frame_height: usize,
    right_rail_width: usize,
) -> InteractionBounds {
    let width = frame_width
        .saturating_mul(3)
        .saturating_div(7)
        .clamp(58, 82);
    let height = frame_height
        .saturating_mul(3)
        .saturating_div(5)
        .clamp(18, 30);
    let top = frame_height
        .saturating_sub(height)
        .saturating_div(2)
        .min(frame_height.saturating_sub(height));
    let transcript_width = frame_width.saturating_sub(right_rail_width + 1);
    let left = transcript_width
        .saturating_sub(width)
        .saturating_div(2)
        .min(transcript_width.saturating_sub(width));
    InteractionBounds {
        top,
        left,
        width,
        height,
        transcript_width,
    }
}

pub(super) fn has_pending_approval(state: &TuiState) -> bool {
    latest_approval(state).is_some()
}

pub(super) fn latest_approval(state: &TuiState) -> Option<&str> {
    for (index, entry) in state.entries.iter().enumerate().rev() {
        if entry.label != "approval" {
            continue;
        }
        if entry.body.contains("Press y") {
            return (!state.entries[index + 1..]
                .iter()
                .any(closes_pending_approval_modal))
            .then_some(entry.body.as_str());
        }
        if entry.body.contains("Approved") || entry.body.contains("Denied") {
            return None;
        }
    }
    None
}

fn closes_pending_approval_modal(entry: &super::state::TuiEntry) -> bool {
    matches!(
        entry.label.as_str(),
        "tool-result" | "assistant" | "command"
    ) || (entry.label == "approval" && !entry.body.contains("Press y"))
}

fn focused_lane(state: &TuiState) -> Option<&TerminalLane> {
    let id = state.focused_lane.as_deref()?;
    state
        .lanes
        .iter()
        .find(|lane| lane.id.eq_ignore_ascii_case(id))
}

fn render_lane_modal(
    frame: &mut Frame,
    state: &TuiState,
    lane: &TerminalLane,
    right_rail_width: usize,
) {
    let modal_width = frame
        .width
        .saturating_mul(2)
        .saturating_div(5)
        .clamp(54, 72);
    let modal_height = 16usize.min(frame.height.saturating_sub(4));
    let top = frame.height.saturating_sub(modal_height).saturating_div(2);
    let transcript_width = frame.width.saturating_sub(right_rail_width + 1);
    let centered_left = transcript_width
        .saturating_sub(modal_width)
        .saturating_div(2);
    let left = centered_left
        .max(22)
        .min(transcript_width.saturating_sub(modal_width));
    let mut rows = vec![
        format!(
            "{} {}  [{}]",
            lane.id,
            lane.tool,
            terminal_label(&lane.tool)
        ),
        format!(
            "PTY    {}  PID {}     ROUTE {}→{}",
            pty_label(&lane.tool),
            pid_hint(lane),
            truncate(&lane.target, 8),
            lane_screen_hint(lane)
        ),
        format!(
            "TASK   {}",
            truncate(&lane.title, modal_width.saturating_sub(11))
        ),
        format!(
            "STATE  {} {}",
            status_dot(&lane.status),
            progress_bar(lane.progress)
        ),
        format!(
            "CMD    {}",
            truncate(
                &command_hint(&lane.tool, &lane.title),
                modal_width.saturating_sub(11)
            )
        ),
        format!(
            "ATTACH {}",
            truncate(&interaction_hint(lane), modal_width.saturating_sub(11))
        ),
        scan_divider(modal_width),
        "LATEST OUTPUT".to_string(),
    ];
    rows.extend(lane_latest_output_rows(
        state,
        lane,
        modal_width.saturating_sub(6),
        3,
    ));
    rows.extend([
        scan_divider(modal_width),
        "CONTROL [stop] [tmux] [pty] [send] [inspect]".to_string(),
        "SIDE    --tui-screen side-1   live tail".to_string(),
    ]);
    let modal = panel(
        "LANE DETAIL",
        rows,
        modal_width,
        modal_height,
        Some("focus"),
    );
    clear_overlay_bounds(frame, top, modal_height, transcript_width);
    render_modal_shadow(frame, top, left, modal_width, modal_height);
    frame.write_block(top, left, &modal);
}

fn lane_latest_output_rows(
    state: &TuiState,
    lane: &TerminalLane,
    max_width: usize,
    max_lines: usize,
) -> Vec<String> {
    let tail = state
        .lane_store
        .as_deref()
        .and_then(|store| lane_runtime_evidence(store, &lane.id))
        .map(|evidence| evidence.log_tail)
        .unwrap_or_default();
    if tail.is_empty() {
        return vec![format!("  {}", truncate(&lane.summary, max_width))];
    }
    let keep_from = tail.len().saturating_sub(max_lines);
    tail.iter()
        .skip(keep_from)
        .map(|line| format!("  {}", truncate(line, max_width)))
        .collect()
}

fn lane_screen_hint(lane: &TerminalLane) -> &'static str {
    match lane.tool.as_str() {
        "codex" | "claude" => "side-1",
        "shell" | "run" => "side-2",
        _ => "main",
    }
}

fn render_approval_modal(
    frame: &mut Frame,
    approval: &str,
    state: &TuiState,
    right_rail_width: usize,
) {
    let details = ApprovalDetails::parse(approval);
    let bounds = approval_modal_bounds(frame.width, frame.height, right_rail_width);
    let mut rows = vec![
        format!(
            "APPROVAL REQUIRED: {:<14} ID: call_7f2a9c1e",
            truncate(details.tool, 14)
        ),
        format!(
            "PATH    {}",
            truncate(details.path, bounds.width.saturating_sub(12))
        ),
        "ACTION  Write (new content)  [MODIFIES FILE]".to_string(),
        "SIZE    +48 lines (2.1 KB)".to_string(),
        if focused_approval_action(state) == ApprovalAction::Diff {
            "DIFF / EVIDENCE (focused)".to_string()
        } else {
            "PREVIEW (first lines)".to_string()
        },
    ];
    rows.extend(code_preview_rows(&details, bounds.width));
    rows.extend([
        apply_all_row(state),
        format!(
            "{}{}{}",
            pad(
                &approval_button("[Deny (n)]", APPROVAL_FOCUS_DENY, state),
                20
            ),
            pad(&approval_button("[Diff]", APPROVAL_FOCUS_DIFF, state), 16),
            approval_button("[Approve (y)]", APPROVAL_FOCUS_APPROVE, state)
        ),
    ]);
    let modal = panel(
        "APPROVAL",
        rows,
        bounds.width,
        bounds.height,
        Some("tab/enter/click"),
    );
    clear_overlay_bounds(frame, bounds.top, bounds.height, bounds.transcript_width);
    render_modal_shadow(frame, bounds.top, bounds.left, bounds.width, bounds.height);
    frame.write_block(bounds.top, bounds.left, &modal);
}

pub(super) fn approval_action_at(
    state: &TuiState,
    column: u16,
    row: u16,
    frame_width: u16,
    frame_height: u16,
    right_rail_width: usize,
) -> Option<ApprovalAction> {
    latest_approval(state)?;
    let bounds = approval_modal_bounds(
        frame_width as usize,
        frame_height as usize,
        right_rail_width,
    );
    let column = column as usize;
    let row = row as usize;
    if row == bounds.apply_row() && column >= bounds.left + 2 && column < bounds.left + bounds.width
    {
        return Some(ApprovalAction::ToggleApplyAll);
    }
    if row != bounds.action_row() {
        return None;
    }
    let content_left = bounds.left + 2;
    if (content_left..content_left + 20).contains(&column) {
        Some(ApprovalAction::Deny)
    } else if (content_left + 20..content_left + 36).contains(&column) {
        Some(ApprovalAction::Diff)
    } else if (content_left + 36..bounds.left + bounds.width).contains(&column) {
        Some(ApprovalAction::Approve)
    } else {
        None
    }
}

pub(super) fn approval_focus_cursor(
    state: &TuiState,
    frame_width: u16,
    frame_height: u16,
    right_rail_width: usize,
) -> Option<(u16, u16)> {
    latest_approval(state)?;
    let bounds = approval_modal_bounds(
        frame_width as usize,
        frame_height as usize,
        right_rail_width,
    );
    let (column, row) = match focused_approval_action(state) {
        ApprovalAction::ToggleApplyAll => (bounds.left + 2, bounds.apply_row()),
        ApprovalAction::Deny => (bounds.left + 2, bounds.action_row()),
        ApprovalAction::Diff => (bounds.left + 22, bounds.action_row()),
        ApprovalAction::Approve => (bounds.left + 38, bounds.action_row()),
    };
    Some((column as u16, row as u16))
}

pub(super) fn move_approval_focus(state: &mut TuiState, delta: i8) {
    let current = state.approval_focus.min(APPROVAL_FOCUS_APPROVE);
    state.approval_focus = if delta < 0 {
        current.saturating_sub(1)
    } else {
        (current + 1).min(APPROVAL_FOCUS_APPROVE)
    };
}

pub(super) fn set_approval_focus_for_action(state: &mut TuiState, action: ApprovalAction) {
    state.approval_focus = match action {
        ApprovalAction::ToggleApplyAll => APPROVAL_FOCUS_APPLY_ALL,
        ApprovalAction::Deny => APPROVAL_FOCUS_DENY,
        ApprovalAction::Diff => APPROVAL_FOCUS_DIFF,
        ApprovalAction::Approve => APPROVAL_FOCUS_APPROVE,
    };
}

pub(super) fn focused_approval_action(state: &TuiState) -> ApprovalAction {
    match state.approval_focus {
        APPROVAL_FOCUS_DENY => ApprovalAction::Deny,
        APPROVAL_FOCUS_DIFF => ApprovalAction::Diff,
        APPROVAL_FOCUS_APPROVE => ApprovalAction::Approve,
        _ => ApprovalAction::ToggleApplyAll,
    }
}

fn apply_all_row(state: &TuiState) -> String {
    let checkbox = if state.approval_apply_all {
        "[x]"
    } else {
        "[ ]"
    };
    let marker = if state.approval_focus == APPROVAL_FOCUS_APPLY_ALL {
        "› "
    } else {
        "  "
    };
    format!("{marker}{checkbox} Apply to all write_file calls in this session")
}

fn approval_button(label: &str, focus: usize, state: &TuiState) -> String {
    if state.approval_focus == focus {
        format!("› {label}")
    } else {
        format!("  {label}")
    }
}

#[derive(Debug, Clone, Copy)]
struct ApprovalBounds {
    top: usize,
    left: usize,
    width: usize,
    height: usize,
    transcript_width: usize,
}

impl ApprovalBounds {
    fn apply_row(self) -> usize {
        self.top + self.height.saturating_sub(3)
    }

    fn action_row(self) -> usize {
        self.top + self.height.saturating_sub(2)
    }
}

fn approval_modal_bounds(
    frame_width: usize,
    frame_height: usize,
    right_rail_width: usize,
) -> ApprovalBounds {
    let width = frame_width.saturating_div(2).clamp(56, 64);
    let height = 15usize.min(frame_height.saturating_sub(4));
    let top = frame_height
        .saturating_sub(height)
        .saturating_div(3)
        .saturating_add(1)
        .min(frame_height.saturating_sub(height));
    let transcript_width = frame_width.saturating_sub(right_rail_width + 1);
    let centered_left = transcript_width.saturating_sub(width).saturating_div(2);
    let left = centered_left
        .max(22)
        .min(transcript_width.saturating_sub(width));
    ApprovalBounds {
        top,
        left,
        width,
        height,
        transcript_width,
    }
}

fn scan_divider(modal_width: usize) -> String {
    let width = modal_width.saturating_sub(4).min(64);
    "┄".repeat(width)
}

fn code_preview_rows(details: &ApprovalDetails<'_>, modal_width: usize) -> Vec<String> {
    let box_width = modal_width.saturating_sub(8).max(28);
    let label = format!(" {} ", truncate(details.path, box_width.saturating_sub(6)));
    let top_rule = horizontal(box_width.saturating_sub(char_width(&label) + 2));
    let bottom_rule = horizontal(box_width.saturating_sub(2));
    let line_width = box_width.saturating_sub(10);
    let preview_lines = code_preview_lines(details);
    let mut rows = vec![format!("  ┌{label}{top_rule}┐")];
    rows.extend(
        preview_lines
            .iter()
            .take(4)
            .enumerate()
            .map(|(index, line)| {
                format!(
                    "  │ +{:>2} │ {} │",
                    index + 1,
                    pad(&truncate(line, line_width), line_width)
                )
            }),
    );
    rows.push(format!("  └{bottom_rule}┘"));
    rows
}

fn code_preview_lines<'a>(details: &'a ApprovalDetails<'a>) -> Vec<&'a str> {
    if !details.preview_lines.is_empty() {
        return details.preview_lines.clone();
    }
    if details.tool == "write_file" {
        return vec![
            "use std::{fs, path::Path};",
            "use anyhow::{Context, Result};",
            "pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {",
            "    let content = fs::read_to_string(path.as_ref())?;",
        ];
    }
    vec![
        "let command = PermissionRequest::current();",
        "let result = workspace.apply(command)?;",
        "session.append_event(result.summary());",
        "Ok(())",
    ]
}

fn render_modal_shadow(frame: &mut Frame, top: usize, left: usize, width: usize, height: usize) {
    let _ = (frame, top, left, width, height);
}

fn clear_overlay_bounds(frame: &mut Frame, top: usize, height: usize, transcript_width: usize) {
    let clear_top = top.saturating_sub(1);
    let clear_left = 1;
    let clear_width = transcript_width
        .saturating_sub(2)
        .min(frame.width.saturating_sub(clear_left));
    let clear_height = (height + 1).min(frame.height.saturating_sub(clear_top));
    frame.fill_rect_pattern(
        clear_top,
        clear_left,
        clear_width,
        clear_height,
        |_x, _y| ' ',
    );
}

#[derive(Debug, Clone)]
struct ApprovalDetails<'a> {
    tool: &'a str,
    path: &'a str,
    preview_lines: Vec<&'a str>,
}

impl<'a> ApprovalDetails<'a> {
    fn parse(value: &'a str) -> Self {
        let mut lines = value.lines();
        let first = lines.next().unwrap_or("Permission request");
        let rest = lines
            .filter(|line| !line.starts_with("Press "))
            .collect::<Vec<_>>();
        let tool = first
            .split('`')
            .nth(1)
            .filter(|value| !value.is_empty())
            .unwrap_or("tool action");
        let path = rest
            .iter()
            .find_map(|line| {
                line.strip_prefix("path: ")
                    .or_else(|| line.strip_prefix("path="))
            })
            .unwrap_or("current session");
        let preview_lines = rest
            .iter()
            .copied()
            .filter(|line| !line.trim().is_empty())
            .filter(|line| !line.starts_with("path: ") && !line.starts_with("path="))
            .take(8)
            .collect::<Vec<_>>();
        Self {
            tool,
            path,
            preview_lines,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{
        ProviderStatus, TerminalLane, TuiEntry, WorkspaceSnapshot, lane_store_path,
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn approval_state() -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: DEFAULT_APPROVAL_FOCUS,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![TuiEntry {
                label: "approval".to_string(),
                body: "Permission request for `write_file`\npath: src/lib.rs\nPress y to allow, n/Esc to deny.".to_string(),
            }],
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        }
    }

    #[test]
    fn latest_approval_stops_after_resolution_entry() {
        let mut state = approval_state();
        assert!(latest_approval(&state).is_some());

        state.entries.push(TuiEntry {
            label: "approval".to_string(),
            body: "Approved `write_file`.".to_string(),
        });

        assert!(latest_approval(&state).is_none());
    }

    #[test]
    fn latest_approval_stops_after_runtime_closure_event() {
        let mut state = approval_state();
        state.entries.push(TuiEntry {
            label: "command".to_string(),
            body: "Test result:\n  status: failed".to_string(),
        });

        assert!(latest_approval(&state).is_none());
    }

    #[test]
    fn approval_mouse_hit_testing_maps_footer_actions() {
        let state = approval_state();
        let bounds = approval_modal_bounds(140, 36, 38);

        assert_eq!(
            approval_action_at(
                &state,
                (bounds.left + 4) as u16,
                bounds.action_row() as u16,
                140,
                36,
                38,
            ),
            Some(ApprovalAction::Deny)
        );
        assert_eq!(
            approval_action_at(
                &state,
                (bounds.left + 40) as u16,
                bounds.action_row() as u16,
                140,
                36,
                38,
            ),
            Some(ApprovalAction::Approve)
        );
    }

    #[test]
    fn approval_focus_cursor_tracks_selected_action() {
        let mut state = approval_state();
        state.approval_focus = APPROVAL_FOCUS_APPROVE;
        let bounds = approval_modal_bounds(140, 36, 38);

        assert_eq!(
            approval_focus_cursor(&state, 140, 36, 38),
            Some(((bounds.left + 38) as u16, bounds.action_row() as u16))
        );
    }

    #[test]
    fn approval_diff_focus_renders_prompt_evidence_not_fake_preview_only() {
        let mut state = approval_state();
        state.approval_focus = APPROVAL_FOCUS_DIFF;
        state.entries[0].body = [
            "Permission request for `write_file`",
            "path: hello.py",
            "content=print(\"Hello, World!\")",
            "Press y to allow, n/Esc to deny.",
        ]
        .join("\n");
        let mut frame = Frame::new(140, 36);

        render_approval_modal(&mut frame, latest_approval(&state).unwrap(), &state, 38);
        let rendered = frame.to_string();

        assert!(rendered.contains("DIFF / EVIDENCE"));
        assert!(rendered.contains("hello.py"));
        assert!(rendered.contains("content=print"));
    }

    #[test]
    fn default_approval_focus_is_approve_for_fast_enter() {
        let state = approval_state();

        assert_eq!(focused_approval_action(&state), ApprovalAction::Approve);
    }

    #[test]
    fn focused_lane_latest_output_prefers_persisted_log_tail() {
        let root = temp_root("lane-modal-tail");
        let lane_store = lane_store_path(&root);
        let artifact_dir = root.join(".viden").join("lanes");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        fs::write(
            artifact_dir.join("L1.log"),
            "old line\ncargo test --workspace\nfinished cleanly\n",
        )
        .expect("lane log");
        let mut state = approval_state();
        state.lane_store = Some(lane_store);
        let lane = state.lanes.first().expect("preview lane");

        let rows = lane_latest_output_rows(&state, lane, 80, 2).join("\n");

        assert!(rows.contains("cargo test --workspace"));
        assert!(rows.contains("finished cleanly"));
        assert!(!rows.contains("patched failing tests"));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(suffix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("viden-modal-test-{nanos}-{suffix}"))
    }
}
