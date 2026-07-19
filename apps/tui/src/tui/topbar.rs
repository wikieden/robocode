use super::{
    canvas::Frame,
    panel::bordered_row,
    state::TuiState,
    text::{bottom_border, char_width, compact_middle, top_border, truncate},
};

pub(super) fn render_top_bar(frame: &mut Frame, state: &TuiState) {
    frame.write_line(0, &top_border(frame.width));
    let content = top_bar_content(state, frame.width.saturating_sub(4));
    frame.write_line(1, &bordered_row(&content, frame.width));
    frame.write_line(2, &bottom_border(frame.width));
}

pub(super) fn render_side_top_bar(frame: &mut Frame, state: &TuiState) {
    frame.write_line(0, &top_border(frame.width));
    let active = active_lane_count(state);
    let content = side_bar_content(
        "SIDE-1",
        &[
            chip("SESSION", &compact_middle(&state.session_id, 8)),
            chip("LANES", &format!("{active}/{}", state.lanes.len())),
            chip("FOCUS", "tail"),
            chip("LINK", "main"),
            chip("GIT", &truncate(&state.workspace.git_branch, 10)),
        ],
        frame.width.saturating_sub(4),
    );
    frame.write_line(
        1,
        &bordered_row(&truncate(&content, frame.width - 4), frame.width),
    );
    frame.write_line(2, &bottom_border(frame.width));
}

pub(super) fn render_ops_top_bar(frame: &mut Frame, state: &TuiState) {
    frame.write_line(0, &top_border(frame.width));
    let content = side_bar_content(
        "SIDE-2",
        &[
            chip("WORK", &truncate(&state.workspace.display_root, 8)),
            chip("FILES", &state.workspace.file_count.to_string()),
            chip("DIAG", &state.workspace.diagnostics.len().to_string()),
            chip("LINK", "side-1"),
            chip("PROV", &truncate(&state.provider, 7)),
        ],
        frame.width.saturating_sub(4),
    );
    frame.write_line(
        1,
        &bordered_row(&truncate(&content, frame.width - 4), frame.width),
    );
    frame.write_line(2, &bottom_border(frame.width));
}

fn top_bar_content(state: &TuiState, width: usize) -> String {
    let session = compact_middle(&state.session_id, 8);
    let branch = truncate(&state.workspace.git_branch, 10);
    let provider = display_provider(&state.provider);
    let work_mode = state.provider_status.work_mode.label();
    let permission_level = state.provider_status.permission_level.label();
    let mut chips = vec![
        chip("PROVIDER", &format!("● {provider}")),
        chip("MODEL", &state.model),
        chip("WORK", work_mode),
        chip("SESSION", &session),
        chip("CONTEXT", state.provider_status.context_window.as_str()),
        chip("GIT", &branch),
        chip("PERM", &format!("{permission_level} ▾")),
    ];
    let status = right_status_cluster(state, StatusDensity::Full);
    let mut content = top_bar_with_status(&chips, &status, width);
    if top_bar_fits(&chips, &status, width) {
        return content;
    }

    chips = vec![
        chip("PROVIDER", &compact_middle(&format!("● {provider}"), 12)),
        chip("MODEL", &compact_middle(&state.model, 12)),
        chip("WORK", work_mode),
        chip("SESSION", &session),
        chip("CONTEXT", state.provider_status.context_window.as_str()),
        chip("GIT", &branch),
        chip("PERM", permission_level),
    ];
    let status = right_status_cluster(state, StatusDensity::Compact);
    content = top_bar_with_status(&chips, &status, width);
    if top_bar_fits(&chips, &status, width) {
        return content;
    }
    chips = vec![
        chip("PROVIDER", &compact_middle(&format!("● {provider}"), 11)),
        chip("MODEL", &compact_middle(&state.model, 10)),
        chip("WORK", work_mode),
        chip("SESSION", &session),
        chip("CONTEXT", state.provider_status.context_window.as_str()),
        chip("GIT", &branch),
        chip("PERM", permission_level),
    ];
    let status = right_status_cluster(state, StatusDensity::Tiny);
    content = top_bar_with_status(&chips, &status, width);
    if top_bar_fits(&chips, &status, width) {
        return content;
    }
    top_bar_without_status(&chips, width)
}

fn top_bar_fits(chips: &[String], status: &str, width: usize) -> bool {
    let left = format!("{}  {}", product_label(), chips.join(" "));
    char_width(&left) + char_width(status) + 3 <= width
}

fn top_bar_with_status(chips: &[String], status: &str, width: usize) -> String {
    let left = format!("{}  {}", product_label(), chips.join(" "));
    let left_width = char_width(&left);
    let status_width = char_width(status);
    if left_width + status_width + 3 <= width {
        return format!("{left}   {status}");
    }
    left
}

fn top_bar_without_status(chips: &[String], width: usize) -> String {
    let mut chips = chips.to_vec();
    while !chips.is_empty() {
        let left = format!("{}  {}", product_label(), chips.join(" "));
        if char_width(&left) <= width {
            return left;
        }
        chips.remove(0);
    }
    truncate(&product_label(), width)
}

fn product_label() -> String {
    format!("Viden  v{}", env!("CARGO_PKG_VERSION"))
}

#[derive(Debug, Clone, Copy)]
enum StatusDensity {
    Full,
    Compact,
    Tiny,
}

fn right_status_cluster(state: &TuiState, density: StatusDensity) -> String {
    let active = active_lane_count(state);
    let telemetry = telemetry_status(state);
    let activity = activity_status(state);
    match density {
        StatusDensity::Full => format!("· {activity}  {telemetry}  L{active}"),
        StatusDensity::Compact => format!("· {activity} {telemetry} L{active}"),
        StatusDensity::Tiny => format!("· {activity} L{active}"),
    }
}

fn activity_status(state: &TuiState) -> &'static str {
    if let Some(turn) = &state.pending_turn {
        if turn.next_action.contains("approve") || turn.phase.contains("approval") {
            return "approval";
        }
        return "working";
    }
    if state.runtime_tasks.iter().any(|task| {
        matches!(
            task.status.as_str(),
            "queued"
                | "thinking"
                | "streaming"
                | "editing"
                | "running_tool"
                | "testing"
                | "waiting_approval"
                | "needs_input"
                | "blocked"
                | "reviewing"
                | "running"
                | "attached"
        )
    }) {
        return "working";
    }
    if state.provider_status.failure_count > 0 {
        return "check";
    }
    "idle"
}

fn telemetry_status(state: &TuiState) -> String {
    if state.provider_status.request_count == 0 {
        return "telemetry idle".to_string();
    }
    if state.provider_status.failure_count > 0 {
        return format!(
            "REQ {} err {}",
            state.provider_status.request_count, state.provider_status.failure_count
        );
    }
    if let Some(tokens) = state.provider_status.last_total_tokens {
        return match state.provider_status.last_tokens_per_second {
            Some(rate) => format!("TOK {} {}t/s", compact_count(tokens), compact_count(rate)),
            None => format!("TOK {}", compact_count(tokens)),
        };
    }
    format!(
        "REQ {} {}",
        state.provider_status.request_count,
        state
            .provider_status
            .last_latency_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "-".to_string())
    )
}

fn compact_count(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}m", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn chip(label: &str, value: &str) -> String {
    format!("[{label} {}]", value.trim())
}

fn display_provider(provider: &str) -> String {
    match provider {
        "openai" => "OpenAI".to_string(),
        "deepseek" => "DeepSeek".to_string(),
        "anthropic" => "Anthropic".to_string(),
        value => value.to_string(),
    }
}

fn side_bar_content(label: &str, chips: &[String], width: usize) -> String {
    let mut chips = chips.to_vec();
    loop {
        let left = format!("Viden  {label}  {}", chips.join(" "));
        if char_width(&left) <= width || chips.is_empty() {
            return truncate(&left, width);
        }
        chips.pop();
    }
}

fn active_lane_count(state: &TuiState) -> usize {
    state
        .lanes
        .iter()
        .filter(|lane| matches!(lane.status.as_str(), "running" | "queued" | "attached"))
        .count()
}

#[cfg(test)]
mod tests {
    use super::telemetry_status;
    use crate::tui::state::{
        PendingTurn, ProviderOption, ProviderStatus, ProviderTelemetry, TuiEntry, TuiState,
        WorkspaceSnapshot,
    };
    use viden_types::{PermissionLevel, WorkMode};

    fn state_with_status(provider_status: ProviderStatus) -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status,
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![TuiEntry {
                label: "system".to_string(),
                body: "ready".to_string(),
            }],
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        }
    }

    #[test]
    fn telemetry_status_reflects_real_provider_requests() {
        let state = state_with_status(ProviderStatus::from_telemetry(&ProviderTelemetry {
            request_count: 2,
            success_count: 1,
            failure_count: 1,
            last_latency_ms: Some(42),
            average_latency_ms: Some(21),
            last_event_count: 3,
            last_error: Some("provider timeout".to_string()),
            ..ProviderTelemetry::default()
        }));

        assert_eq!(telemetry_status(&state), "REQ 2 err 1");
    }

    #[test]
    fn telemetry_status_reports_token_usage_when_available() {
        let state = state_with_status(ProviderStatus::from_telemetry(&ProviderTelemetry {
            request_count: 1,
            success_count: 1,
            last_total_tokens: Some(1200),
            total_tokens: 1200,
            last_tokens_per_second: Some(60),
            ..ProviderTelemetry::default()
        }));

        assert_eq!(telemetry_status(&state), "TOK 1.2k 60t/s");
    }

    #[test]
    fn telemetry_status_reports_idle_before_first_request() {
        let state = state_with_status(ProviderStatus::configured());

        assert_eq!(telemetry_status(&state), "telemetry idle");
    }

    #[test]
    fn top_bar_uses_runtime_mode_and_permission_state() {
        let mut state = state_with_status(ProviderStatus::configured());
        state.provider_status.work_mode = WorkMode::Plan;
        state.provider_status.permission_level = PermissionLevel::ReadOnly;

        let content = super::top_bar_content(&state, 200);

        assert!(content.contains("[WORK Plan]"));
        assert!(content.contains("[PERM Read Only"));
    }

    #[test]
    fn top_bar_status_reflects_active_turn_instead_of_static_auto_text() {
        let mut state = state_with_status(ProviderStatus::configured());
        state.pending_turn = Some(PendingTurn::new(
            "session",
            "deepseek",
            "deepseek-v4-flash",
            "fix tests",
            "/tmp/project",
        ));

        let content = super::top_bar_content(&state, 200);

        assert!(content.contains("· working"));
        assert!(!content.contains("· auto"));
    }
}
