use crossterm::event::{KeyCode, KeyEvent};
use robocode_types::MemoryStatus;

use super::{
    canvas::Frame,
    composer::COMPOSER_HEIGHT,
    panel::{bordered_row, panel_top},
    state::TuiState,
    statusbar::BOTTOM_BAR_HEIGHT,
    text::{bottom_border, pad, truncate},
    theme::TuiTheme,
};

const MAX_VISIBLE_COMMANDS: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommandSuggestion {
    pub(super) command: String,
    pub(super) summary: String,
}

pub(super) fn is_command_palette_query(input: &str) -> bool {
    if !input.starts_with('/') {
        return false;
    }
    !input.contains(char::is_whitespace) || is_nested_command_query(input)
}

pub(super) fn is_command_palette_visible(state: &TuiState) -> bool {
    if !is_command_palette_query(&state.input) {
        return false;
    }
    state
        .command_palette_hidden_for
        .as_ref()
        .is_none_or(|hidden| hidden != &state.input)
        && !command_suggestions_for_state(state).is_empty()
}

fn command_suggestions_for_state(state: &TuiState) -> Vec<CommandSuggestion> {
    if let Some(suggestions) = selector_root_suggestions(&state.input, state) {
        return suggestions;
    }
    nested_command_suggestions(&state.input, state)
        .unwrap_or_else(|| static_command_suggestions(&state.input))
}

fn static_command_suggestions(query: &str) -> Vec<CommandSuggestion> {
    COMMANDS
        .into_iter()
        .filter(|item| item.command.starts_with(query))
        .map(command_from_template)
        .collect()
}

pub(super) fn selected_command(state: &TuiState) -> Option<CommandSuggestion> {
    let suggestions = command_suggestions_for_state(state);
    let selected = state
        .command_selection
        .min(suggestions.len().saturating_sub(1));
    suggestions.get(selected).cloned()
}

pub(super) fn move_selection(state: &mut TuiState, delta: i8) -> bool {
    if !is_command_palette_visible(state) {
        return false;
    }
    let count = command_suggestions_for_state(state).len();
    if count == 0 {
        state.command_selection = 0;
        return false;
    }
    state.command_selection = if delta < 0 {
        state.command_selection.saturating_sub(1)
    } else {
        (state.command_selection + 1).min(count - 1)
    };
    true
}

pub(super) fn reset_for_input_change(state: &mut TuiState) {
    state.command_selection = 0;
    state.command_palette_hidden_for = None;
}

pub(super) fn close_on_escape(key: KeyEvent, state: &mut TuiState) -> bool {
    if key.code != KeyCode::Esc || !is_command_palette_visible(state) {
        return false;
    }
    state.command_palette_hidden_for = Some(state.input.clone());
    true
}

pub(super) fn complete_selected(state: &mut TuiState) -> bool {
    let Some(suggestion) = selected_command(state) else {
        return false;
    };
    state.input = format!("{} ", suggestion.command);
    state.command_selection = 0;
    state.command_palette_hidden_for = None;
    true
}

pub(super) fn select_suggestion_at(state: &mut TuiState, index: usize) -> bool {
    if !is_command_palette_visible(state) {
        return false;
    }
    let suggestions = command_suggestions_for_state(state);
    if index >= suggestions.len() {
        return false;
    }
    state.command_selection = index;
    true
}

pub(super) fn command_suggestion_index_at(
    state: &TuiState,
    column: u16,
    row: u16,
    frame_width: u16,
    frame_height: u16,
) -> Option<usize> {
    if !is_command_palette_visible(state) {
        return None;
    }
    let suggestions = command_suggestions_for_state(state);
    if selector_kind(&state.input).is_some() {
        return selector_suggestion_index_at(
            state,
            &suggestions,
            column,
            row,
            frame_width,
            frame_height,
        );
    }
    let visible = suggestions.len().min(MAX_VISIBLE_COMMANDS);
    if visible == 0 {
        return None;
    }
    let width = usize::from(frame_width).saturating_sub(8).clamp(48, 104);
    let height = visible + 2;
    let left = 2usize.min(usize::from(frame_width).saturating_sub(width));
    let composer_top = usize::from(frame_height)
        .saturating_sub(BOTTOM_BAR_HEIGHT)
        .saturating_sub(COMPOSER_HEIGHT);
    let top = composer_top.saturating_sub(height);
    let column = usize::from(column);
    let row = usize::from(row);
    if !(left..left + width).contains(&column) || !(top + 1..top + 1 + visible).contains(&row) {
        return None;
    }
    let selected = state
        .command_selection
        .min(suggestions.len().saturating_sub(1));
    let start = visible_command_window_start(selected, suggestions.len(), visible);
    Some(start + row - top - 1)
}

pub(super) fn should_complete_on_enter(state: &TuiState) -> bool {
    let Some(selected) = selected_command(state) else {
        return false;
    };
    state.input != selected.command && is_command_palette_visible(state)
}

pub(super) fn render_command_suggestions(frame: &mut Frame, state: &TuiState) {
    if !is_command_palette_visible(state) {
        return;
    }
    let suggestions = command_suggestions_for_state(state);
    if suggestions.is_empty() {
        return;
    }
    if let Some(kind) = selector_kind(&state.input) {
        render_selector(frame, state, &suggestions, kind);
        return;
    }

    let width = frame.width.saturating_sub(8).clamp(48, 104);
    let visible = suggestions.len().min(MAX_VISIBLE_COMMANDS);
    let height = visible + 2;
    let left = 2usize.min(frame.width.saturating_sub(width));
    let composer_top = frame
        .height
        .saturating_sub(BOTTOM_BAR_HEIGHT)
        .saturating_sub(COMPOSER_HEIGHT);
    let top = composer_top.saturating_sub(height);
    let content_width = width.saturating_sub(4);
    let detail_width = content_width.saturating_mul(2).saturating_div(5).max(22);
    let command_width = content_width.saturating_sub(detail_width + 4);
    let selected = state
        .command_selection
        .min(suggestions.len().saturating_sub(1));
    let start = visible_command_window_start(selected, suggestions.len(), visible);
    let end = (start + visible).min(suggestions.len());
    let mut rows = Vec::with_capacity(height);
    let hint = if suggestions.len() > visible {
        format!(
            "{}-{}/{} ↑↓ tab enter esc",
            start + 1,
            end,
            suggestions.len()
        )
    } else {
        "↑↓ tab enter esc".to_string()
    };
    rows.push(panel_top("COMMANDS", width, Some(&hint)));
    for (index, suggestion) in suggestions.iter().enumerate().skip(start).take(visible) {
        rows.push(command_suggestion_row(
            suggestion,
            index == selected,
            width,
            command_width,
            detail_width,
        ));
    }
    rows.push(bottom_border(width));
    frame.fill_rect_pattern(top, 0, frame.width, rows.len(), |_x, _y| ' ');
    frame.write_block(top, left, &rows);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorKind {
    Setup,
    Settings,
    Provider,
    Model,
    Lane,
    Permissions,
    Theme,
}

impl SelectorKind {
    fn footer(self) -> &'static str {
        match self {
            Self::Setup => "Enter open   step-by-step first run   esc close",
            Self::Settings => "Enter apply   ↑↓ select   / search   esc close",
            Self::Provider => "Enter open   click opens config   esc close",
            Self::Model => "Enter switch   grouped by provider   free-type any model",
            Self::Lane => "Enter open/run   lane ids appear after a lane starts",
            Self::Permissions => "Enter apply   Suggest is the safe default",
            Self::Theme => "Enter apply   Ctrl+T cycles theme",
        }
    }
}

fn selector_kind(input: &str) -> Option<SelectorKind> {
    let normalized = input.trim_end();
    if normalized == "/setup" {
        return Some(SelectorKind::Setup);
    }
    if normalized == "/settings" {
        return Some(SelectorKind::Settings);
    }
    if matches!(
        normalized,
        "/provider" | "/settings provider" | "/setup provider"
    ) || normalized.starts_with("/provider ")
        || normalized.starts_with("/settings provider ")
        || normalized.starts_with("/setup provider ")
    {
        return Some(SelectorKind::Provider);
    }
    if matches!(
        normalized,
        "/model" | "/models" | "/settings model" | "/setup model"
    ) || normalized.starts_with("/model ")
        || normalized.starts_with("/models ")
        || normalized.starts_with("/settings model ")
        || normalized.starts_with("/setup model ")
    {
        return Some(SelectorKind::Model);
    }
    if input == "/lane" {
        return Some(SelectorKind::Lane);
    }
    if matches!(
        normalized,
        "/permissions" | "/settings permissions" | "/setup permissions"
    ) || normalized.starts_with("/permissions ")
        || normalized.starts_with("/settings permissions ")
        || normalized.starts_with("/setup permissions ")
    {
        return Some(SelectorKind::Permissions);
    }
    if matches!(normalized, "/theme" | "/settings theme" | "/setup theme")
        || normalized.starts_with("/theme ")
        || normalized.starts_with("/settings theme ")
        || normalized.starts_with("/setup theme ")
    {
        return Some(SelectorKind::Theme);
    }
    None
}

fn render_selector(
    frame: &mut Frame,
    state: &TuiState,
    suggestions: &[CommandSuggestion],
    kind: SelectorKind,
) {
    let width = selector_width(frame.width, kind);
    let visible = suggestions.len().clamp(1, 12);
    let header_len = selector_header_len(state, kind);
    let height = (visible + header_len + 4)
        .min(frame.height.saturating_sub(4))
        .max(header_len + 5);
    let left = frame.width.saturating_sub(width).saturating_div(2);
    let top = frame.height.saturating_sub(height).saturating_div(2);
    let selected = state
        .command_selection
        .min(suggestions.len().saturating_sub(1));
    let start = visible_command_window_start(selected, suggestions.len(), visible);
    let content_width = width.saturating_sub(4);
    let mut rows = selector_header_rows(state, kind, width, content_width);
    for (index, suggestion) in suggestions.iter().enumerate().skip(start).take(visible) {
        let label = selector_row_text(suggestion, kind);
        let marker = if index == selected { "●" } else { " " };
        let line = if index == selected {
            format!("{marker} {label}")
        } else {
            format!("  {label}")
        };
        rows.push(format!(
            "│ {} │",
            pad(&truncate(&line, content_width), content_width)
        ));
    }
    rows.extend([
        format!("│ {} │", pad("", content_width)),
        format!("│ {} │", pad(&selector_footer(state, kind), content_width)),
        bottom_border(width),
    ]);
    while rows.len() < height {
        let insert_at = rows.len().saturating_sub(1);
        rows.insert(insert_at, format!("│ {} │", pad("", content_width)));
    }
    frame.fill_rect_pattern(
        top.saturating_sub(1),
        0,
        frame.width,
        height + 2,
        |_x, _y| ' ',
    );
    frame.write_block(top, left, &rows);
}

fn selector_suggestion_index_at(
    state: &TuiState,
    suggestions: &[CommandSuggestion],
    column: u16,
    row: u16,
    frame_width: u16,
    frame_height: u16,
) -> Option<usize> {
    if suggestions.is_empty() {
        return None;
    }
    let kind = selector_kind(&state.input)?;
    let width = selector_width(usize::from(frame_width), kind);
    let visible = suggestions.len().clamp(1, 12);
    let header_len = selector_header_len(state, kind);
    let height = (visible + header_len + 4)
        .min(usize::from(frame_height).saturating_sub(4))
        .max(header_len + 5);
    let left = usize::from(frame_width)
        .saturating_sub(width)
        .saturating_div(2);
    let top = usize::from(frame_height)
        .saturating_sub(height)
        .saturating_div(2);
    let column = usize::from(column);
    let row = usize::from(row);
    let item_top = top + header_len;
    if !(left..left + width).contains(&column) || !(item_top..item_top + visible).contains(&row) {
        return None;
    }
    let selected = state
        .command_selection
        .min(suggestions.len().saturating_sub(1));
    let start = visible_command_window_start(selected, suggestions.len(), visible);
    Some(start + row - item_top)
}

fn selector_width(frame_width: usize, kind: SelectorKind) -> usize {
    match kind {
        SelectorKind::Provider | SelectorKind::Model => frame_width
            .saturating_mul(11)
            .saturating_div(20)
            .clamp(72, 100),
        SelectorKind::Lane => frame_width
            .saturating_mul(9)
            .saturating_div(20)
            .clamp(64, 90),
        SelectorKind::Setup => frame_width
            .saturating_mul(9)
            .saturating_div(20)
            .clamp(64, 84),
        _ => frame_width
            .saturating_mul(2)
            .saturating_div(5)
            .clamp(56, 76),
    }
}

fn selector_context(state: &TuiState, kind: SelectorKind) -> String {
    match kind {
        SelectorKind::Setup => format!(
            "current {} / {}  key {}",
            state.provider,
            state.model,
            current_provider_key_hint(state)
        ),
        SelectorKind::Settings => format!(
            "{}  {}  theme {}",
            state.provider, state.model, state.theme_name
        ),
        SelectorKind::Provider => format!("current {} / {}", state.provider, state.model),
        SelectorKind::Model => format!("current {} / {}", state.provider, state.model),
        SelectorKind::Lane => format!("{} lane(s) tracked", state.lanes.len()),
        SelectorKind::Permissions => "current mode is shown in the top bar".to_string(),
        SelectorKind::Theme => format!("current {}", state.theme_name),
    }
}

fn selector_title(state: &TuiState, kind: SelectorKind) -> &'static str {
    match kind {
        SelectorKind::Setup => "SETUP WIZARD",
        SelectorKind::Settings => "SETTINGS",
        SelectorKind::Provider if selected_provider_for_detail(&state.input, state).is_some() => {
            "PROVIDER CONFIG"
        }
        SelectorKind::Provider => "SELECT PROVIDER",
        SelectorKind::Model => "SELECT MODEL",
        SelectorKind::Lane => "LANE ACTIONS",
        SelectorKind::Permissions => "PERMISSIONS",
        SelectorKind::Theme => "SELECT THEME",
    }
}

fn selector_footer(state: &TuiState, kind: SelectorKind) -> String {
    if kind == SelectorKind::Provider && selected_provider_for_detail(&state.input, state).is_some()
    {
        return "Enter action   /models for all models   esc close".to_string();
    }
    kind.footer().to_string()
}

fn selector_header_len(state: &TuiState, kind: SelectorKind) -> usize {
    if kind == SelectorKind::Provider && selected_provider_for_detail(&state.input, state).is_some()
    {
        7
    } else {
        5
    }
}

fn selector_header_rows(
    state: &TuiState,
    kind: SelectorKind,
    width: usize,
    content_width: usize,
) -> Vec<String> {
    if kind == SelectorKind::Provider
        && let Some(provider) = selected_provider_for_detail(&state.input, state)
    {
        let models = provider_model_candidates(provider);
        let models = if models.is_empty() {
            "models: free-type".to_string()
        } else {
            format!("models: {}", models.join(", "))
        };
        return vec![
            panel_top(selector_title(state, kind), width, Some("esc")),
            format!("│ {} │", pad("", content_width)),
            format!(
                "│ {} │",
                pad(
                    &format!("{} / {}", provider.provider_id, provider.display_name),
                    content_width
                )
            ),
            format!(
                "│ {} │",
                pad(
                    &format!("key: {}", provider_key_detail(provider)),
                    content_width
                )
            ),
            format!(
                "│ {} │",
                pad(
                    &format!("endpoint: {}", provider_endpoint_detail(provider)),
                    content_width
                )
            ),
            format!(
                "│ {} │",
                pad(&truncate(&models, content_width), content_width)
            ),
            format!("│ {} │", pad("", content_width)),
        ];
    }

    let search = selector_search_text(state, kind);
    vec![
        panel_top(selector_title(state, kind), width, Some("esc")),
        format!("│ {} │", pad("", content_width)),
        format!(
            "│ {} │",
            pad(
                &format!("Search {}", if search.is_empty() { "_" } else { &search }),
                content_width
            )
        ),
        format!("│ {} │", pad("", content_width)),
        format!(
            "│ {} │",
            pad(
                &truncate(&selector_context(state, kind), content_width),
                content_width
            )
        ),
    ]
}

fn selector_search_text(state: &TuiState, kind: SelectorKind) -> String {
    if kind == SelectorKind::Provider
        && let Some(provider) = selected_provider_for_detail(&state.input, state)
    {
        let prefix = if state.input.starts_with("/setup provider ") {
            format!("/setup provider {}", provider.provider_id)
        } else if state.input.starts_with("/settings provider ") {
            format!("/settings provider {}", provider.provider_id)
        } else {
            format!("/provider {}", provider.provider_id)
        };
        return state
            .input
            .strip_prefix(&prefix)
            .unwrap_or("")
            .trim_start()
            .to_string();
    }
    state
        .input
        .strip_prefix(selector_search_prefix(&state.input, kind))
        .unwrap_or("")
        .trim_start()
        .to_string()
}

fn selector_search_prefix(input: &str, kind: SelectorKind) -> &str {
    let normalized = input.trim_end();
    match kind {
        SelectorKind::Setup => "/setup",
        SelectorKind::Settings => normalized.split_whitespace().next().unwrap_or(normalized),
        SelectorKind::Provider => ["/settings provider", "/setup provider", "/provider"]
            .into_iter()
            .find(|prefix| normalized.starts_with(prefix))
            .unwrap_or("/provider"),
        SelectorKind::Model => ["/settings model", "/setup model", "/models", "/model"]
            .into_iter()
            .find(|prefix| normalized.starts_with(prefix))
            .unwrap_or("/models"),
        SelectorKind::Lane => "/lane",
        SelectorKind::Permissions => [
            "/settings permissions",
            "/setup permissions",
            "/permissions",
        ]
        .into_iter()
        .find(|prefix| normalized.starts_with(prefix))
        .unwrap_or("/permissions"),
        SelectorKind::Theme => ["/settings theme", "/setup theme", "/theme"]
            .into_iter()
            .find(|prefix| normalized.starts_with(prefix))
            .unwrap_or("/theme"),
    }
}

fn selector_label(suggestion: &CommandSuggestion, kind: SelectorKind) -> String {
    let command = suggestion.command.as_str();
    match kind {
        SelectorKind::Setup => command
            .strip_prefix("/setup ")
            .or_else(|| command.strip_prefix("/settings "))
            .or_else(|| command.strip_prefix('/'))
            .unwrap_or(command)
            .to_string(),
        SelectorKind::Settings => command
            .strip_prefix("/settings ")
            .or_else(|| command.strip_prefix("/setup "))
            .unwrap_or(command)
            .to_string(),
        SelectorKind::Provider => command
            .strip_prefix("/settings provider ")
            .or_else(|| command.strip_prefix("/setup provider "))
            .map(provider_settings_label)
            .or_else(|| {
                command
                    .strip_prefix("/provider use ")
                    .map(|_| "use now".to_string())
            })
            .or_else(|| {
                command
                    .strip_prefix("/provider doctor ")
                    .map(|_| "doctor".to_string())
            })
            .or_else(|| command.strip_prefix("/provider ").map(str::to_string))
            .unwrap_or_else(|| command.to_string()),
        SelectorKind::Model => {
            if let Some(value) = command.strip_prefix("/settings provider ") {
                value.to_string()
            } else {
                command
                    .split_whitespace()
                    .last()
                    .unwrap_or(command)
                    .to_string()
            }
        }
        SelectorKind::Lane => command
            .strip_prefix("/lane ")
            .unwrap_or(command)
            .to_string(),
        SelectorKind::Permissions => command
            .split_whitespace()
            .last()
            .unwrap_or(command)
            .to_string(),
        SelectorKind::Theme => command
            .split_whitespace()
            .last()
            .unwrap_or(command)
            .to_string(),
    }
}

fn provider_settings_label(value: &str) -> String {
    let words = value.split_whitespace().collect::<Vec<_>>();
    match words.as_slice() {
        [_provider] => "set default provider".to_string(),
        [_provider, model] => format!("model {model}"),
        _ => value.to_string(),
    }
}

fn selector_row_text(suggestion: &CommandSuggestion, kind: SelectorKind) -> String {
    let label = selector_label(suggestion, kind);
    match kind {
        SelectorKind::Setup => format!("{label}  {}", suggestion.summary),
        SelectorKind::Provider if suggestion.summary.is_empty() => label,
        SelectorKind::Provider | SelectorKind::Model | SelectorKind::Lane => {
            format!("{label}  {}", suggestion.summary)
        }
        _ => label,
    }
}

fn visible_command_window_start(selected: usize, total: usize, visible: usize) -> usize {
    if total <= visible || visible == 0 {
        return 0;
    }
    let half = visible / 2;
    selected
        .saturating_sub(half)
        .min(total.saturating_sub(visible))
}

fn selector_root_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    match selector_kind(query)? {
        SelectorKind::Setup if query.trim_end() == "/setup" => Some(setup_templates(state)),
        SelectorKind::Settings if query.trim_end() == "/settings" => {
            Some(settings_templates_for_prefix(query.trim_end()))
        }
        SelectorKind::Provider if provider_selector_root_query(query) => {
            Some(provider_selector_suggestions(query, state))
        }
        SelectorKind::Model if model_selector_root_query(query) => {
            Some(model_selector_suggestions(query, state))
        }
        SelectorKind::Lane if query.trim_end() == "/lane" => Some(lane_selector_suggestions(state)),
        SelectorKind::Permissions if permission_selector_root_query(query) => {
            Some(permission_selector_suggestions(query))
        }
        SelectorKind::Theme if theme_selector_root_query(query) => {
            Some(theme_selector_suggestions(query))
        }
        _ => None,
    }
}

fn provider_selector_root_query(query: &str) -> bool {
    matches!(
        query.trim_end(),
        "/provider" | "/settings provider" | "/setup provider"
    )
}

fn model_selector_root_query(query: &str) -> bool {
    matches!(
        query.trim_end(),
        "/model" | "/models" | "/settings model" | "/setup model"
    )
}

fn permission_selector_root_query(query: &str) -> bool {
    matches!(
        query.trim_end(),
        "/permissions" | "/settings permissions" | "/setup permissions"
    )
}

fn theme_selector_root_query(query: &str) -> bool {
    matches!(
        query.trim_end(),
        "/theme" | "/settings theme" | "/setup theme"
    )
}

fn lane_selector_suggestions(state: &TuiState) -> Vec<CommandSuggestion> {
    let mut suggestions = Vec::new();
    if !state.lanes.is_empty() {
        suggestions.extend(
            [
                "/lane inspect",
                "/lane timeline",
                "/lane diff",
                "/lane artifacts",
            ]
            .into_iter()
            .flat_map(|prefix| {
                state.lanes.iter().map(move |lane| CommandSuggestion {
                    command: format!("{prefix} {}", lane.id),
                    summary: format!("{} [{}]", lane.title, lane.status),
                })
            }),
        );
    }
    suggestions.extend(LANE_COMMANDS.into_iter().map(command_from_template));
    suggestions
}

fn provider_selector_suggestions(query: &str, state: &TuiState) -> Vec<CommandSuggestion> {
    let prefix = selector_search_prefix(query, SelectorKind::Provider);
    state
        .provider_catalog
        .iter()
        .filter(|provider| provider.provider_id != "fallback")
        .map(|provider| {
            let command = if prefix == "/provider" {
                format!("/provider {}", provider.provider_id)
            } else {
                format!("{prefix} {}", provider.provider_id)
            };
            CommandSuggestion {
                command,
                summary: String::new(),
            }
        })
        .collect()
}

fn model_selector_suggestions(query: &str, state: &TuiState) -> Vec<CommandSuggestion> {
    let prefix = selector_search_prefix(query, SelectorKind::Model);
    if prefix == "/models" {
        all_provider_model_suggestions("", state)
    } else {
        current_provider_model_suggestions(prefix, "", state)
    }
}

fn permission_selector_suggestions(query: &str) -> Vec<CommandSuggestion> {
    let prefix = selector_search_prefix(query, SelectorKind::Permissions);
    PERMISSION_COMMANDS
        .into_iter()
        .map(|item| CommandSuggestion {
            command: item.command.replacen("/permissions", prefix, 1),
            summary: item.summary.to_string(),
        })
        .collect()
}

fn theme_selector_suggestions(query: &str) -> Vec<CommandSuggestion> {
    let prefix = selector_search_prefix(query, SelectorKind::Theme);
    TuiTheme::names()
        .iter()
        .map(|theme| CommandSuggestion {
            command: format!("{prefix} {theme}"),
            summary: "Apply TUI theme".to_string(),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CommandTemplate {
    command: &'static str,
    summary: &'static str,
}

const COMMANDS: [CommandTemplate; 30] = [
    CommandTemplate {
        command: "/help",
        summary: "Show commands",
    },
    CommandTemplate {
        command: "/provider",
        summary: "List or switch providers",
    },
    CommandTemplate {
        command: "/permissions",
        summary: "Select approval mode",
    },
    CommandTemplate {
        command: "/settings",
        summary: "Configure provider/model",
    },
    CommandTemplate {
        command: "/setup",
        summary: "First-run setup guide",
    },
    CommandTemplate {
        command: "/model",
        summary: "Set current model",
    },
    CommandTemplate {
        command: "/models",
        summary: "Select model",
    },
    CommandTemplate {
        command: "/theme",
        summary: "Select TUI theme",
    },
    CommandTemplate {
        command: "/plan",
        summary: "Toggle planning mode",
    },
    CommandTemplate {
        command: "/git",
        summary: "Git status, diff, branch ops",
    },
    CommandTemplate {
        command: "/diff",
        summary: "Show latest diff",
    },
    CommandTemplate {
        command: "/test",
        summary: "Run tests and record evidence",
    },
    CommandTemplate {
        command: "/lsp",
        summary: "Diagnostics and symbols",
    },
    CommandTemplate {
        command: "/task",
        summary: "Create or update tasks",
    },
    CommandTemplate {
        command: "/brief",
        summary: "Create or show active task brief",
    },
    CommandTemplate {
        command: "/spec",
        summary: "Alias for task brief",
    },
    CommandTemplate {
        command: "/tasks",
        summary: "List active tasks",
    },
    CommandTemplate {
        command: "/memory",
        summary: "Project and session memory",
    },
    CommandTemplate {
        command: "/context",
        summary: "Inspect latest context bundle",
    },
    CommandTemplate {
        command: "/screen",
        summary: "Open side screen route",
    },
    CommandTemplate {
        command: "/lane",
        summary: "Run or inspect agent lanes",
    },
    CommandTemplate {
        command: "/agent",
        summary: "List or diagnose agents",
    },
    CommandTemplate {
        command: "/extensions",
        summary: "List extension surfaces",
    },
    CommandTemplate {
        command: "/mcp",
        summary: "List MCP context servers",
    },
    CommandTemplate {
        command: "/skills",
        summary: "List local skills",
    },
    CommandTemplate {
        command: "/status",
        summary: "Runtime status",
    },
    CommandTemplate {
        command: "/config",
        summary: "Show active config",
    },
    CommandTemplate {
        command: "/doctor",
        summary: "Check setup health",
    },
    CommandTemplate {
        command: "/exit",
        summary: "Exit RoboCode",
    },
    CommandTemplate {
        command: "/quit",
        summary: "Exit RoboCode",
    },
];

const AGENT_COMMANDS: [CommandTemplate; 13] = [
    CommandTemplate {
        command: "/agent list",
        summary: "List agent adapters",
    },
    CommandTemplate {
        command: "/agent doctor",
        summary: "Check agent readiness",
    },
    CommandTemplate {
        command: "/agent review codex",
        summary: "Start Codex review",
    },
    CommandTemplate {
        command: "/agent challenge codex",
        summary: "Start adversarial review",
    },
    CommandTemplate {
        command: "/agent probe codex --thread",
        summary: "Probe app-server thread",
    },
    CommandTemplate {
        command: "/agent probe codex --turn",
        summary: "Probe app-server turn",
    },
    CommandTemplate {
        command: "/agent run codex",
        summary: "Start Codex task",
    },
    CommandTemplate {
        command: "/agent run codex --app-server",
        summary: "Start app-server task",
    },
    CommandTemplate {
        command: "/agent run codex --write",
        summary: "Start write-capable Codex task",
    },
    CommandTemplate {
        command: "/agent status",
        summary: "Show agent jobs",
    },
    CommandTemplate {
        command: "/agent result",
        summary: "Show job output",
    },
    CommandTemplate {
        command: "/agent cancel",
        summary: "Cancel job",
    },
    CommandTemplate {
        command: "/agent logs",
        summary: "Inspect agent logs",
    },
];

const EXTENSION_COMMANDS: [CommandTemplate; 2] = [
    CommandTemplate {
        command: "/extensions list",
        summary: "List extension surfaces",
    },
    CommandTemplate {
        command: "/extensions doctor",
        summary: "Check extension surfaces",
    },
];

const MCP_COMMANDS: [CommandTemplate; 2] = [
    CommandTemplate {
        command: "/mcp list",
        summary: "List MCP configs",
    },
    CommandTemplate {
        command: "/mcp doctor",
        summary: "Check MCP readiness",
    },
];

const SKILL_COMMANDS: [CommandTemplate; 1] = [CommandTemplate {
    command: "/skills list",
    summary: "List local skills",
}];

const LANE_COMMANDS: [CommandTemplate; 24] = [
    CommandTemplate {
        command: "/lane codex",
        summary: "Start Codex lane",
    },
    CommandTemplate {
        command: "/lane codex-review",
        summary: "Start read-only Codex review lane",
    },
    CommandTemplate {
        command: "/lane claude",
        summary: "Start Claude lane",
    },
    CommandTemplate {
        command: "/lane run",
        summary: "Run shell lane",
    },
    CommandTemplate {
        command: "/lane ask",
        summary: "Start custom tool lane",
    },
    CommandTemplate {
        command: "/lane inspect",
        summary: "Inspect lane evidence",
    },
    CommandTemplate {
        command: "/lane timeline",
        summary: "Show lane event timeline",
    },
    CommandTemplate {
        command: "/lane diff",
        summary: "Show lane patch",
    },
    CommandTemplate {
        command: "/lane artifacts",
        summary: "List lane artifacts",
    },
    CommandTemplate {
        command: "/lane stop",
        summary: "Stop running lane",
    },
    CommandTemplate {
        command: "/lane retry",
        summary: "Retry lane task",
    },
    CommandTemplate {
        command: "/lane attach",
        summary: "Open lane terminal",
    },
    CommandTemplate {
        command: "/lane tmux",
        summary: "Open tmux lane",
    },
    CommandTemplate {
        command: "/lane pty",
        summary: "Open embedded PTY",
    },
    CommandTemplate {
        command: "/lane send",
        summary: "Send PTY input",
    },
    CommandTemplate {
        command: "/lane detach",
        summary: "Detach lane terminal",
    },
    CommandTemplate {
        command: "/lane accept",
        summary: "Accept lane result",
    },
    CommandTemplate {
        command: "/lane revise",
        summary: "Request revision",
    },
    CommandTemplate {
        command: "/lane discard",
        summary: "Discard lane result",
    },
    CommandTemplate {
        command: "/lane apply",
        summary: "Apply accepted patch",
    },
    CommandTemplate {
        command: "/lane resolve",
        summary: "Retry apply conflict",
    },
    CommandTemplate {
        command: "/lane archive",
        summary: "Archive lane evidence",
    },
    CommandTemplate {
        command: "/lane cleanup",
        summary: "Archive worktree",
    },
    CommandTemplate {
        command: "/lane close",
        summary: "Close lane focus",
    },
];

const LANE_ID_COMMANDS: [&str; 18] = [
    "/lane inspect",
    "/lane timeline",
    "/lane diff",
    "/lane artifacts",
    "/lane stop",
    "/lane retry",
    "/lane attach",
    "/lane tmux",
    "/lane pty",
    "/lane send",
    "/lane detach",
    "/lane accept",
    "/lane revise",
    "/lane discard",
    "/lane apply",
    "/lane resolve",
    "/lane archive",
    "/lane cleanup",
];

const PROVIDER_COMMANDS: [CommandTemplate; 5] = [
    CommandTemplate {
        command: "/provider list",
        summary: "List providers",
    },
    CommandTemplate {
        command: "/provider doctor",
        summary: "Provider diagnostics",
    },
    CommandTemplate {
        command: "/provider reload",
        summary: "Reload registry",
    },
    CommandTemplate {
        command: "/provider use",
        summary: "Switch provider",
    },
    CommandTemplate {
        command: "/provider help",
        summary: "Provider help",
    },
];

const SETTINGS_COMMANDS: [CommandTemplate; 8] = [
    CommandTemplate {
        command: "/settings provider",
        summary: "Switch and save provider",
    },
    CommandTemplate {
        command: "/settings model",
        summary: "Switch and save model",
    },
    CommandTemplate {
        command: "/settings save",
        summary: "Save current defaults",
    },
    CommandTemplate {
        command: "/settings permissions",
        summary: "Select approval mode",
    },
    CommandTemplate {
        command: "/settings theme",
        summary: "Select TUI theme",
    },
    CommandTemplate {
        command: "/settings doctor",
        summary: "Provider diagnostics",
    },
    CommandTemplate {
        command: "/settings show",
        summary: "Show settings",
    },
    CommandTemplate {
        command: "/settings help",
        summary: "Settings help",
    },
];

const PERMISSION_COMMANDS: [CommandTemplate; 5] = [
    CommandTemplate {
        command: "/permissions default",
        summary: "Suggest before mutations",
    },
    CommandTemplate {
        command: "/permissions acceptEdits",
        summary: "Auto-accept file edits",
    },
    CommandTemplate {
        command: "/permissions plan",
        summary: "Read-only planning mode",
    },
    CommandTemplate {
        command: "/permissions bypassPermissions",
        summary: "YOLO trusted workspace",
    },
    CommandTemplate {
        command: "/permissions dontAsk",
        summary: "Deny prompts instead of asking",
    },
];

const SCREEN_COMMANDS: [CommandTemplate; 5] = [
    CommandTemplate {
        command: "/screen main",
        summary: "Show main screen info",
    },
    CommandTemplate {
        command: "/screen side-1",
        summary: "Launch side screen 1",
    },
    CommandTemplate {
        command: "/screen side-2",
        summary: "Launch side screen 2",
    },
    CommandTemplate {
        command: "/screen list",
        summary: "List side screens",
    },
    CommandTemplate {
        command: "/screen close",
        summary: "Stop tracking side screen",
    },
];

const LSP_COMMANDS: [CommandTemplate; 4] = [
    CommandTemplate {
        command: "/lsp status",
        summary: "LSP runtime status",
    },
    CommandTemplate {
        command: "/lsp diagnostics",
        summary: "Diagnostics for file",
    },
    CommandTemplate {
        command: "/lsp symbols",
        summary: "Document symbols",
    },
    CommandTemplate {
        command: "/lsp references",
        summary: "References at position",
    },
];

const TASK_COMMANDS: [CommandTemplate; 10] = [
    CommandTemplate {
        command: "/task add",
        summary: "Create task",
    },
    CommandTemplate {
        command: "/task view",
        summary: "View task",
    },
    CommandTemplate {
        command: "/task update",
        summary: "Rename task",
    },
    CommandTemplate {
        command: "/task status",
        summary: "Set task status",
    },
    CommandTemplate {
        command: "/task link",
        summary: "Add dependency",
    },
    CommandTemplate {
        command: "/task block",
        summary: "Block task",
    },
    CommandTemplate {
        command: "/task unblock",
        summary: "Unblock task",
    },
    CommandTemplate {
        command: "/task archive",
        summary: "Archive task",
    },
    CommandTemplate {
        command: "/task restore",
        summary: "Restore task",
    },
    CommandTemplate {
        command: "/task resume-context",
        summary: "Render resume context",
    },
];

const BRIEF_COMMANDS: [CommandTemplate; 5] = [
    CommandTemplate {
        command: "/brief show",
        summary: "Show active brief",
    },
    CommandTemplate {
        command: "/brief clear",
        summary: "Clear active brief",
    },
    CommandTemplate {
        command: "/brief steering init",
        summary: "Create steering templates",
    },
    CommandTemplate {
        command: "/brief steering show",
        summary: "Show steering summaries",
    },
    CommandTemplate {
        command: "/brief help",
        summary: "Brief command help",
    },
];

const SPEC_COMMANDS: [CommandTemplate; 3] = [
    CommandTemplate {
        command: "/spec show",
        summary: "Show active brief",
    },
    CommandTemplate {
        command: "/spec steering init",
        summary: "Create steering templates",
    },
    CommandTemplate {
        command: "/spec help",
        summary: "Brief command help",
    },
];

const TASK_ID_COMMANDS: [&str; 7] = [
    "/task view",
    "/task update",
    "/task status",
    "/task block",
    "/task unblock",
    "/task archive",
    "/task restore",
];

const TASK_STATUS_COMMAND: &str = "/task status";

const TASK_STATUSES: [CommandTemplate; 5] = [
    CommandTemplate {
        command: "todo",
        summary: "Todo",
    },
    CommandTemplate {
        command: "in_progress",
        summary: "In progress",
    },
    CommandTemplate {
        command: "blocked",
        summary: "Blocked",
    },
    CommandTemplate {
        command: "done",
        summary: "Done",
    },
    CommandTemplate {
        command: "archived",
        summary: "Archived",
    },
];

const MEMORY_COMMANDS: [CommandTemplate; 8] = [
    CommandTemplate {
        command: "/memory project",
        summary: "Project memory",
    },
    CommandTemplate {
        command: "/memory session",
        summary: "Session memory",
    },
    CommandTemplate {
        command: "/memory suggest",
        summary: "Suggest memory",
    },
    CommandTemplate {
        command: "/memory confirm",
        summary: "Confirm memory",
    },
    CommandTemplate {
        command: "/memory reject",
        summary: "Reject memory",
    },
    CommandTemplate {
        command: "/memory prune",
        summary: "Prune memory",
    },
    CommandTemplate {
        command: "/memory add",
        summary: "Add session memory",
    },
    CommandTemplate {
        command: "/memory export",
        summary: "Export memory",
    },
];

const GIT_COMMANDS: [CommandTemplate; 11] = [
    CommandTemplate {
        command: "/git status",
        summary: "Working tree status",
    },
    CommandTemplate {
        command: "/git diff",
        summary: "Show diff",
    },
    CommandTemplate {
        command: "/git branch",
        summary: "List branches",
    },
    CommandTemplate {
        command: "/git add",
        summary: "Stage paths",
    },
    CommandTemplate {
        command: "/git restore",
        summary: "Restore paths",
    },
    CommandTemplate {
        command: "/git switch",
        summary: "Switch branch",
    },
    CommandTemplate {
        command: "/git commit",
        summary: "Commit changes",
    },
    CommandTemplate {
        command: "/git push",
        summary: "Push branch",
    },
    CommandTemplate {
        command: "/git stash",
        summary: "Stash flows",
    },
    CommandTemplate {
        command: "/git worktree",
        summary: "Worktree flows",
    },
    CommandTemplate {
        command: "/git help",
        summary: "Git help",
    },
];

const GIT_STASH_COMMANDS: [CommandTemplate; 5] = [
    CommandTemplate {
        command: "/git stash list",
        summary: "List stashes",
    },
    CommandTemplate {
        command: "/git stash push",
        summary: "Create stash",
    },
    CommandTemplate {
        command: "/git stash pop",
        summary: "Apply stash",
    },
    CommandTemplate {
        command: "/git stash drop",
        summary: "Drop stash",
    },
    CommandTemplate {
        command: "/git stash help",
        summary: "Stash help",
    },
];

const GIT_WORKTREE_COMMANDS: [CommandTemplate; 3] = [
    CommandTemplate {
        command: "/git worktree list",
        summary: "List worktrees",
    },
    CommandTemplate {
        command: "/git worktree add",
        summary: "Add worktree",
    },
    CommandTemplate {
        command: "/git worktree remove",
        summary: "Remove worktree",
    },
];

fn command_from_template(template: CommandTemplate) -> CommandSuggestion {
    CommandSuggestion {
        command: template.command.to_string(),
        summary: template.summary.to_string(),
    }
}

fn is_nested_command_query(input: &str) -> bool {
    [
        "/lane",
        "/agent",
        "/extensions",
        "/mcp",
        "/skills",
        "/screen",
        "/provider",
        "/settings",
        "/setup",
        "/model",
        "/models",
        "/permissions",
        "/theme",
        "/lsp",
        "/task",
        "/brief",
        "/spec",
        "/memory",
        "/git",
        "/git stash",
        "/git worktree",
    ]
    .into_iter()
    .any(|root| input == format!("{root} ") || input.starts_with(&format!("{root} ")))
}

fn nested_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    lane_command_suggestions(query, state)
        .or_else(|| agent_command_suggestions(query))
        .or_else(|| extension_command_suggestions(query))
        .or_else(|| mcp_command_suggestions(query))
        .or_else(|| skill_command_suggestions(query))
        .or_else(|| screen_command_suggestions(query, state))
        .or_else(|| task_command_suggestions(query, state))
        .or_else(|| brief_command_suggestions(query))
        .or_else(|| lsp_command_suggestions(query, state))
        .or_else(|| git_stash_command_suggestions(query, state))
        .or_else(|| git_worktree_command_suggestions(query, state))
        .or_else(|| provider_command_suggestions(query, state))
        .or_else(|| settings_command_suggestions(query, state))
        .or_else(|| model_command_suggestions(query, state))
        .or_else(|| permission_command_suggestions(query))
        .or_else(|| theme_command_suggestions(query))
        .or_else(|| memory_command_suggestions(query, state))
        .or_else(|| git_command_suggestions(query, state))
}

fn template_group_suggestions(
    query: &str,
    root: &str,
    templates: &[CommandTemplate],
) -> Option<Vec<CommandSuggestion>> {
    if !query.starts_with(&format!("{root} ")) {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= 1 || query.ends_with(' ') && words.len() == 1 {
        templates
            .iter()
            .copied()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 2 && !query.ends_with(' ') {
        templates
            .iter()
            .filter(|item| item.command.starts_with(query))
            .copied()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn agent_command_suggestions(query: &str) -> Option<Vec<CommandSuggestion>> {
    template_group_suggestions(query, "/agent", &AGENT_COMMANDS)
}

fn extension_command_suggestions(query: &str) -> Option<Vec<CommandSuggestion>> {
    template_group_suggestions(query, "/extensions", &EXTENSION_COMMANDS)
}

fn mcp_command_suggestions(query: &str) -> Option<Vec<CommandSuggestion>> {
    template_group_suggestions(query, "/mcp", &MCP_COMMANDS)
}

fn skill_command_suggestions(query: &str) -> Option<Vec<CommandSuggestion>> {
    template_group_suggestions(query, "/skills", &SKILL_COMMANDS)
}

fn lane_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    query
        .starts_with("/lane ")
        .then(|| command_group_or_lane_ids(query, state))
}

fn command_group_or_lane_ids(query: &str, state: &TuiState) -> Vec<CommandSuggestion> {
    let words = query.split_whitespace().collect::<Vec<_>>();
    if words.len() <= 1 || query.ends_with(' ') && words.len() == 1 {
        LANE_COMMANDS
            .into_iter()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 2 && !query.ends_with(' ') {
        LANE_COMMANDS
            .into_iter()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if should_suggest_lane_ids(query, &words) {
        lane_id_suggestions(query, &words, state)
    } else {
        Vec::new()
    }
}

fn settings_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    let prefix = if query.starts_with("/settings ") {
        "/settings"
    } else if query.starts_with("/setup ") {
        "/setup"
    } else {
        return None;
    };
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= 1 || query.ends_with(' ') && words.len() == 1 {
        if prefix == "/setup" {
            setup_templates(state)
        } else {
            settings_templates_for_prefix(prefix)
        }
    } else if words.len() == 2 && !query.ends_with(' ') {
        if prefix == "/setup" {
            setup_templates(state)
        } else {
            settings_templates_for_prefix(prefix)
        }
        .into_iter()
        .filter(|item| item.command.starts_with(query))
        .collect()
    } else if should_suggest_settings_provider_ids(query, &words) {
        settings_provider_id_suggestions(query, &words, state)
    } else if selected_provider_for_detail(query, state).is_some() {
        provider_detail_suggestions(query, &words, state)
    } else if should_suggest_settings_provider_models(query, &words) {
        settings_provider_model_suggestions(query, &words, state)
    } else if should_suggest_settings_models(query, &words) {
        settings_model_suggestions(query, &words, state)
    } else if should_suggest_settings_permissions(query, &words) {
        settings_permission_suggestions(query, &words)
    } else if should_suggest_settings_themes(query, &words) {
        settings_theme_suggestions(query, &words)
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn settings_templates_for_prefix(prefix: &str) -> Vec<CommandSuggestion> {
    SETTINGS_COMMANDS
        .into_iter()
        .map(|item| CommandSuggestion {
            command: item.command.replacen("/settings", prefix, 1),
            summary: item.summary.to_string(),
        })
        .collect()
}

fn setup_templates(state: &TuiState) -> Vec<CommandSuggestion> {
    vec![
        CommandSuggestion {
            command: "/setup provider".to_string(),
            summary: "Open provider config: key, endpoint, models".to_string(),
        },
        CommandSuggestion {
            command: "/models".to_string(),
            summary: "Choose any model grouped by provider".to_string(),
        },
        CommandSuggestion {
            command: "/settings permissions".to_string(),
            summary: "Pick approval mode before edits run".to_string(),
        },
        CommandSuggestion {
            command: "/settings theme".to_string(),
            summary: "Pick cockpit color theme".to_string(),
        },
        CommandSuggestion {
            command: format!("/provider doctor {}", state.provider),
            summary: "Check current provider key and endpoint".to_string(),
        },
        CommandSuggestion {
            command: "/provider fallback test-local".to_string(),
            summary: "Use offline fallback for smoke tests".to_string(),
        },
        CommandSuggestion {
            command: "/settings save".to_string(),
            summary: "Persist current provider/model defaults".to_string(),
        },
    ]
}

fn should_suggest_settings_provider_ids(query: &str, words: &[&str]) -> bool {
    let exact_subcommand = matches!(query.trim_end(), "/settings provider" | "/setup provider");
    words.len() >= 2
        && words.len() <= 3
        && matches!(
            format!("{} {}", words[0], words[1]).as_str(),
            "/settings provider" | "/setup provider"
        )
        && (exact_subcommand
            || query.ends_with(' ') && words.len() == 2
            || !query.ends_with(' ') && words.get(2).is_some())
}

fn settings_provider_id_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    let partial_provider = if query.ends_with(' ') {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    state
        .provider_catalog
        .iter()
        .filter(|provider| provider.provider_id.starts_with(partial_provider))
        .map(|provider| CommandSuggestion {
            command: format!("{} provider {}", words[0], provider.provider_id),
            summary: provider_list_summary(provider),
        })
        .collect()
}

fn should_suggest_settings_provider_models(query: &str, words: &[&str]) -> bool {
    words.len() >= 3
        && words.len() <= 4
        && matches!(
            format!("{} {}", words[0], words[1]).as_str(),
            "/settings provider" | "/setup provider"
        )
        && (query.ends_with(' ') || words.get(3).is_some())
}

fn settings_provider_model_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    let provider_id = words.get(2).copied().unwrap_or("");
    let partial_model = if query.ends_with(' ') {
        ""
    } else {
        words.get(3).copied().unwrap_or("")
    };
    state
        .provider_catalog
        .iter()
        .filter(|provider| provider.provider_id == provider_id)
        .flat_map(|provider| {
            provider_model_candidates(provider)
                .into_iter()
                .filter(move |model| model.starts_with(partial_model))
                .map(|model| CommandSuggestion {
                    command: format!("{} provider {provider_id} {model}", words[0]),
                    summary: model_summary(provider, &model),
                })
        })
        .collect()
}

fn should_suggest_settings_models(query: &str, words: &[&str]) -> bool {
    let exact_subcommand = matches!(query.trim_end(), "/settings model" | "/setup model");
    words.len() >= 2
        && words.len() <= 3
        && matches!(
            format!("{} {}", words[0], words[1]).as_str(),
            "/settings model" | "/setup model"
        )
        && (exact_subcommand || query.ends_with(' ') || words.get(2).is_some())
}

fn settings_model_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    let partial_model = if query.ends_with(' ') {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    current_provider_model_suggestions(&format!("{} model", words[0]), partial_model, state)
}

fn should_suggest_settings_permissions(query: &str, words: &[&str]) -> bool {
    let exact_subcommand = matches!(
        query.trim_end(),
        "/settings permissions" | "/setup permissions"
    );
    words.len() >= 2
        && words.len() <= 3
        && matches!(
            format!("{} {}", words[0], words[1]).as_str(),
            "/settings permissions" | "/setup permissions"
        )
        && (exact_subcommand || query.ends_with(' ') || words.get(2).is_some())
}

fn settings_permission_suggestions(query: &str, words: &[&str]) -> Vec<CommandSuggestion> {
    let root = format!("{} {}", words[0], words[1]);
    let partial = if query.ends_with(' ')
        || matches!(
            query.trim_end(),
            "/settings permissions" | "/setup permissions"
        ) {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    PERMISSION_COMMANDS
        .into_iter()
        .filter(|item| {
            item.command
                .strip_prefix("/permissions ")
                .is_some_and(|mode| mode.starts_with(partial))
        })
        .map(|item| CommandSuggestion {
            command: item.command.replacen("/permissions", &root, 1),
            summary: item.summary.to_string(),
        })
        .collect()
}

fn should_suggest_settings_themes(query: &str, words: &[&str]) -> bool {
    let exact_subcommand = matches!(query.trim_end(), "/settings theme" | "/setup theme");
    words.len() >= 2
        && words.len() <= 3
        && matches!(
            format!("{} {}", words[0], words[1]).as_str(),
            "/settings theme" | "/setup theme"
        )
        && (exact_subcommand || query.ends_with(' ') || words.get(2).is_some())
}

fn settings_theme_suggestions(query: &str, words: &[&str]) -> Vec<CommandSuggestion> {
    let root = format!("{} {}", words[0], words[1]);
    let partial =
        if query.ends_with(' ') || matches!(query.trim_end(), "/settings theme" | "/setup theme") {
            ""
        } else {
            words.get(2).copied().unwrap_or("")
        };
    TuiTheme::names()
        .iter()
        .filter(|theme| theme.starts_with(partial))
        .map(|theme| CommandSuggestion {
            command: format!("{root} {theme}"),
            summary: "Apply TUI theme".to_string(),
        })
        .collect()
}

fn provider_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    if !query.starts_with("/provider ") {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= 1 || query.ends_with(' ') && words.len() == 1 {
        PROVIDER_COMMANDS
            .into_iter()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 2 && !query.ends_with(' ') && provider_subcommand_prefix(words[1]) {
        PROVIDER_COMMANDS
            .into_iter()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if selected_provider_for_detail(query, state).is_some() {
        provider_detail_suggestions(query, &words, state)
    } else if should_suggest_provider_ids(query, &words) {
        provider_id_suggestions(query, &words, state)
    } else if should_suggest_provider_models(query, &words) {
        provider_model_suggestions(query, &words, state)
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn should_suggest_provider_ids(query: &str, words: &[&str]) -> bool {
    if words.first().copied() != Some("/provider") || words.len() > 3 {
        return false;
    }
    if words.get(1).copied() == Some("use") {
        return query.ends_with(' ') && words.len() == 2
            || !query.ends_with(' ') && words.get(2).is_some();
    }
    words.len() == 2 && !provider_subcommand_prefix(words[1])
}

fn provider_id_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    let partial_provider = if query.ends_with(' ') {
        ""
    } else if words.get(1).copied() == Some("use") {
        words.get(2).copied().unwrap_or("")
    } else {
        words.get(1).copied().unwrap_or("")
    };
    state
        .provider_catalog
        .iter()
        .filter(|provider| provider.provider_id.starts_with(partial_provider))
        .map(|provider| {
            let command = if words.get(1).copied() == Some("use") {
                format!("/provider use {}", provider.provider_id)
            } else {
                format!("/provider {}", provider.provider_id)
            };
            CommandSuggestion {
                command,
                summary: provider_list_summary(provider),
            }
        })
        .collect()
}

fn provider_detail_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    let Some(provider) = selected_provider_for_detail(query, state) else {
        return Vec::new();
    };
    let partial = match words {
        ["/provider", _provider] if !query.ends_with(' ') => "",
        ["/provider", _provider, partial, ..] => partial,
        ["/setup" | "/settings", "provider", _provider] if !query.ends_with(' ') => "",
        ["/setup" | "/settings", "provider", _provider, partial, ..] => partial,
        _ => "",
    };
    let mut suggestions = vec![
        CommandSuggestion {
            command: format!("/settings provider {}", provider.provider_id),
            summary: provider.provider_id.clone(),
        },
        CommandSuggestion {
            command: format!("/provider use {}", provider.provider_id),
            summary: provider.provider_id.clone(),
        },
        CommandSuggestion {
            command: format!("/provider doctor {}", provider.provider_id),
            summary: provider.provider_id.clone(),
        },
        CommandSuggestion {
            command: "/models".to_string(),
            summary: provider.provider_id.clone(),
        },
    ];
    suggestions.extend(
        provider_model_candidates(provider)
            .into_iter()
            .map(|model| CommandSuggestion {
                command: format!("/settings provider {} {model}", provider.provider_id),
                summary: String::new(),
            }),
    );
    suggestions
        .into_iter()
        .filter(|suggestion| {
            partial.is_empty()
                || selector_label(suggestion, SelectorKind::Provider)
                    .to_ascii_lowercase()
                    .contains(&partial.to_ascii_lowercase())
                || suggestion
                    .summary
                    .to_ascii_lowercase()
                    .contains(&partial.to_ascii_lowercase())
        })
        .collect()
}

fn should_suggest_provider_models(query: &str, words: &[&str]) -> bool {
    if words.first().copied() != Some("/provider") {
        return false;
    }
    if words.get(1).copied() == Some("use") {
        return words.len() >= 3
            && words.len() <= 4
            && (query.ends_with(' ') || words.get(3).is_some());
    }
    words.len() >= 2
        && words.len() <= 3
        && !provider_subcommand_prefix(words[1])
        && (query.ends_with(' ') || words.get(2).is_some())
}

fn provider_model_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    let provider_id = if words.get(1).copied() == Some("use") {
        words.get(2).copied().unwrap_or("")
    } else {
        words.get(1).copied().unwrap_or("")
    };
    let partial_model = if query.ends_with(' ') {
        ""
    } else if words.get(1).copied() == Some("use") {
        words.get(3).copied().unwrap_or("")
    } else {
        words.get(2).copied().unwrap_or("")
    };
    state
        .provider_catalog
        .iter()
        .filter(|provider| provider.provider_id == provider_id)
        .flat_map(|provider| {
            provider_model_candidates(provider)
                .into_iter()
                .filter(move |model| model.starts_with(partial_model))
                .map(|model| CommandSuggestion {
                    command: if words.get(1).copied() == Some("use") {
                        format!("/provider use {provider_id} {model}")
                    } else {
                        format!("/provider {provider_id} {model}")
                    },
                    summary: model_summary(provider, &model),
                })
        })
        .collect()
}

fn model_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    let (prefix, all_providers) = if query == "/model " || query.starts_with("/model ") {
        ("/model", false)
    } else if query == "/models " || query.starts_with("/models ") {
        ("/models", true)
    } else {
        return None;
    };
    let words = query.split_whitespace().collect::<Vec<_>>();
    if words.len() > 2 {
        return Some(Vec::new());
    }
    let partial_model = if query.ends_with(' ') {
        ""
    } else {
        words.get(1).copied().unwrap_or("")
    };
    let suggestions = if all_providers {
        all_provider_model_suggestions(partial_model, state)
    } else {
        current_provider_model_suggestions(prefix, partial_model, state)
    };
    Some(suggestions)
}

fn permission_command_suggestions(query: &str) -> Option<Vec<CommandSuggestion>> {
    if !(query == "/permissions "
        || query.starts_with("/permissions ")
        || query.trim_end() == "/permissions")
    {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    if words.len() > 2 {
        return Some(Vec::new());
    }
    let partial = if query.ends_with(' ') || query.trim_end() == "/permissions" {
        ""
    } else {
        words.get(1).copied().unwrap_or("")
    };
    Some(
        PERMISSION_COMMANDS
            .into_iter()
            .filter(|item| {
                item.command
                    .strip_prefix("/permissions ")
                    .is_some_and(|mode| mode.starts_with(partial))
            })
            .map(command_from_template)
            .collect(),
    )
}

fn theme_command_suggestions(query: &str) -> Option<Vec<CommandSuggestion>> {
    if !(query == "/theme " || query.starts_with("/theme ") || query.trim_end() == "/theme") {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    if words.len() > 2 {
        return Some(Vec::new());
    }
    let partial = if query.ends_with(' ') || query.trim_end() == "/theme" {
        ""
    } else {
        words.get(1).copied().unwrap_or("")
    };
    Some(
        TuiTheme::names()
            .iter()
            .filter(|theme| theme.starts_with(partial))
            .map(|theme| CommandSuggestion {
                command: format!("/theme {theme}"),
                summary: "Apply TUI theme".to_string(),
            })
            .collect(),
    )
}

fn provider_subcommand_prefix(value: &str) -> bool {
    ["list", "doctor", "reload", "use", "help"]
        .iter()
        .any(|subcommand| subcommand.starts_with(value))
}

fn selected_provider_for_detail<'a>(
    input: &str,
    state: &'a TuiState,
) -> Option<&'a super::state::ProviderOption> {
    let words = input.split_whitespace().collect::<Vec<_>>();
    let provider_id = match words.as_slice() {
        ["/provider", provider_id, ..] => Some(*provider_id),
        ["/setup", "provider", provider_id, ..] | ["/settings", "provider", provider_id, ..] => {
            Some(*provider_id)
        }
        _ => None,
    }?;
    if provider_subcommand_prefix(provider_id) {
        return None;
    }
    state
        .provider_catalog
        .iter()
        .find(|provider| provider.provider_id == provider_id)
}

fn current_provider_key_hint(state: &TuiState) -> String {
    state
        .provider_catalog
        .iter()
        .find(|provider| provider.provider_id == state.provider)
        .map(provider_key_detail)
        .unwrap_or_else(|| "catalog unavailable".to_string())
}

fn provider_list_summary(provider: &super::state::ProviderOption) -> String {
    provider.provider_id.clone()
}

fn provider_key_detail(provider: &super::state::ProviderOption) -> String {
    provider
        .api_key_env
        .as_deref()
        .map(|env| {
            if let Ok(value) = std::env::var(env) {
                let value = value.trim();
                if value.is_empty() {
                    format!("{env} empty")
                } else {
                    format!("{env} {}", mask_secret(value))
                }
            } else {
                format!("{env} missing")
            }
        })
        .unwrap_or_else(|| "key not required".to_string())
}

fn mask_secret(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    if len <= 4 {
        return "*".repeat(len.max(1));
    }
    let head = if len <= 12 { 2 } else { 4 };
    let tail = if len <= 12 { 2 } else { 4 };
    let masked = len.saturating_sub(head + tail).max(3);
    format!(
        "{}{}{}",
        chars.iter().take(head).collect::<String>(),
        "*".repeat(masked),
        chars
            .iter()
            .skip(len.saturating_sub(tail))
            .collect::<String>()
    )
}

fn provider_endpoint_detail(provider: &super::state::ProviderOption) -> String {
    if let Some(env) = provider.api_base_env.as_deref() {
        if let Some(value) = std::env::var_os(env) {
            return format!("{env}={}", value.to_string_lossy());
        }
        if let Some(base) = provider.default_api_base.as_deref() {
            return format!("{base} ({env} override)");
        }
        return format!("{env} override");
    }
    provider
        .default_api_base
        .clone()
        .unwrap_or_else(|| "built-in".to_string())
}

fn env_status(name: &str) -> String {
    if std::env::var_os(name).is_some() {
        format!("{name}:present")
    } else {
        format!("{name}:missing")
    }
}

fn current_provider_model_suggestions(
    prefix: &str,
    partial_model: &str,
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    state
        .provider_catalog
        .iter()
        .filter(|provider| provider.provider_id == state.provider)
        .flat_map(|provider| {
            provider_model_candidates(provider)
                .into_iter()
                .filter(move |model| model.starts_with(partial_model))
                .map(move |model| CommandSuggestion {
                    command: format!("{prefix} {model}"),
                    summary: model_summary(provider, &model),
                })
        })
        .collect()
}

fn all_provider_model_suggestions(partial: &str, state: &TuiState) -> Vec<CommandSuggestion> {
    state
        .provider_catalog
        .iter()
        .flat_map(|provider| {
            provider_model_candidates(provider)
                .into_iter()
                .filter(move |model| {
                    partial.is_empty()
                        || model.starts_with(partial)
                        || provider.provider_id.starts_with(partial)
                        || provider
                            .display_name
                            .to_ascii_lowercase()
                            .starts_with(partial)
                })
                .map(move |model| CommandSuggestion {
                    command: format!("/settings provider {} {model}", provider.provider_id),
                    summary: model_summary(provider, &model),
                })
        })
        .collect()
}

fn model_summary(provider: &super::state::ProviderOption, model: &str) -> String {
    let default = if provider
        .default_model
        .as_deref()
        .is_some_and(|default| default == model)
    {
        " default"
    } else {
        ""
    };
    format!(
        "{}{} · {}",
        provider.display_name,
        default,
        provider_config_short(provider)
    )
}

fn provider_config_short(provider: &super::state::ProviderOption) -> String {
    provider
        .api_key_env
        .as_deref()
        .map(env_status)
        .unwrap_or_else(|| "key not required".to_string())
}

fn provider_model_candidates(provider: &super::state::ProviderOption) -> Vec<String> {
    let mut models = Vec::new();
    if let Some(default_model) = provider.default_model.as_deref() {
        push_unique_string(&mut models, default_model);
    }
    match provider.provider_id.as_str() {
        "anthropic" => {
            push_unique_string(&mut models, "claude-sonnet-4-6");
            push_unique_string(&mut models, "claude-haiku-4-6");
        }
        "deepseek" | "deepseek-anthropic" => {
            push_unique_string(&mut models, "deepseek-v4-flash");
            push_unique_string(&mut models, "deepseek-v4-pro");
        }
        "fallback" => {
            push_unique_string(&mut models, "test-local");
            push_unique_string(&mut models, "fallback-local");
        }
        "openai" => {
            push_unique_string(&mut models, "gpt-5.2");
            push_unique_string(&mut models, "gpt-5.2-codex");
            push_unique_string(&mut models, "gpt-4o-mini");
        }
        "openai-compatible" => {
            push_unique_string(&mut models, "gpt-4o-mini");
        }
        "openrouter" => {
            push_unique_string(&mut models, "openai/gpt-5.2");
            push_unique_string(&mut models, "anthropic/claude-sonnet-4-6");
            push_unique_string(&mut models, "deepseek/deepseek-v4-flash");
        }
        "groq" => {
            push_unique_string(&mut models, "openai/gpt-oss-20b");
        }
        "kimi" => {
            push_unique_string(&mut models, "kimi-k2.5");
            push_unique_string(&mut models, "kimi-k2.6");
        }
        "mistral" => {
            push_unique_string(&mut models, "mistral-medium-latest");
        }
        "qwen" => {
            push_unique_string(&mut models, "qwen-plus");
        }
        "zhipu" => {
            push_unique_string(&mut models, "glm-4.6");
        }
        "volcengine" => {
            push_unique_string(&mut models, "ark-code-latest");
            push_unique_string(&mut models, "deepseek-v3.2");
            push_unique_string(&mut models, "doubao-seed-2.0-code");
        }
        _ => {}
    }
    models
}

fn push_unique_string(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn should_suggest_lane_ids(query: &str, words: &[&str]) -> bool {
    if words.len() < 2 || words.len() > 3 {
        return false;
    }
    let base = format!("{} {}", words[0], words[1]);
    LANE_ID_COMMANDS.contains(&base.as_str())
        && (query.ends_with(' ')
            || words
                .get(2)
                .is_some_and(|value| value.starts_with('L') || value.starts_with('l')))
}

fn lane_id_suggestions(query: &str, words: &[&str], state: &TuiState) -> Vec<CommandSuggestion> {
    let base = format!("{} {}", words[0], words[1]);
    let partial_id = if query.ends_with(' ') {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    state
        .lanes
        .iter()
        .filter(|lane| {
            lane.id
                .to_ascii_lowercase()
                .starts_with(&partial_id.to_ascii_lowercase())
        })
        .map(|lane| CommandSuggestion {
            command: format!("{base} {}", lane.id),
            summary: format!("{} [{}]", lane.title, lane.status),
        })
        .collect()
}

fn screen_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    if !query.starts_with("/screen ") {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= 1 || query.ends_with(' ') && words.len() == 1 {
        SCREEN_COMMANDS
            .into_iter()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 2 && !query.ends_with(' ') {
        SCREEN_COMMANDS
            .into_iter()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() <= 3 && words.get(1) == Some(&"close") {
        screen_id_suggestions(query, &words, state)
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn screen_id_suggestions(query: &str, words: &[&str], state: &TuiState) -> Vec<CommandSuggestion> {
    let partial_id = if query.ends_with(' ') {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    let mut screens = state
        .screens
        .iter()
        .map(|screen| (screen.id.as_str(), screen.summary.as_str()))
        .collect::<Vec<_>>();
    if screens.is_empty() {
        screens = vec![("side-1", "Side screen 1"), ("side-2", "Side screen 2")];
    }
    screens
        .into_iter()
        .filter(|(id, _)| id.starts_with(partial_id))
        .map(|(id, summary)| CommandSuggestion {
            command: format!("/screen close {id}"),
            summary: summary.to_string(),
        })
        .collect()
}

fn task_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    if !query.starts_with("/task ") {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if should_suggest_task_status(query, &words) {
        task_status_suggestions(query, &words)
    } else if words.len() <= 1 || query.ends_with(' ') && words.len() == 1 {
        TASK_COMMANDS
            .into_iter()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 2 && !query.ends_with(' ') {
        TASK_COMMANDS
            .into_iter()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if should_suggest_task_ids(query, &words) {
        task_id_suggestions(query, &words, state)
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn should_suggest_task_ids(query: &str, words: &[&str]) -> bool {
    if words.len() < 2 || words.len() > 3 {
        return false;
    }
    let base = format!("{} {}", words[0], words[1]);
    TASK_ID_COMMANDS.contains(&base.as_str())
        && (query.ends_with(' ') || words.get(2).is_some_and(|value| value.starts_with("task")))
}

fn task_id_suggestions(query: &str, words: &[&str], state: &TuiState) -> Vec<CommandSuggestion> {
    let base = format!("{} {}", words[0], words[1]);
    let partial_id = if query.ends_with(' ') {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    state
        .tasks
        .iter()
        .filter(|task| task.task_id.starts_with(partial_id))
        .map(|task| CommandSuggestion {
            command: format!("{base} {}", task.task_id),
            summary: format!("{} [{}]", task.title, task.status),
        })
        .collect()
}

fn should_suggest_task_status(query: &str, words: &[&str]) -> bool {
    words.len() >= 3
        && words.len() <= 4
        && format!("{} {}", words[0], words[1]) == TASK_STATUS_COMMAND
        && (query.ends_with(' ') || words.len() == 4)
}

fn task_status_suggestions(query: &str, words: &[&str]) -> Vec<CommandSuggestion> {
    let task_id = words.get(2).copied().unwrap_or("");
    let partial_status = if query.ends_with(' ') {
        ""
    } else {
        words.get(3).copied().unwrap_or("")
    };
    TASK_STATUSES
        .into_iter()
        .filter(|status| status.command.starts_with(partial_status))
        .map(|status| CommandSuggestion {
            command: format!("/task status {task_id} {}", status.command),
            summary: status.summary.to_string(),
        })
        .collect()
}

fn brief_command_suggestions(query: &str) -> Option<Vec<CommandSuggestion>> {
    if query.starts_with("/brief ") {
        return template_group_suggestions(query, "/brief", &BRIEF_COMMANDS);
    }
    if query.starts_with("/spec ") {
        return template_group_suggestions(query, "/spec", &SPEC_COMMANDS);
    }
    None
}

fn memory_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    if !query.starts_with("/memory ") {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= 1 || query.ends_with(' ') && words.len() == 1 {
        MEMORY_COMMANDS
            .into_iter()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 2 && !query.ends_with(' ') {
        MEMORY_COMMANDS
            .into_iter()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if should_suggest_memory_ids(query, &words) {
        memory_id_suggestions(query, &words, state)
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn should_suggest_memory_ids(query: &str, words: &[&str]) -> bool {
    if words.len() < 2 || words.len() > 3 {
        return false;
    }
    matches!(
        format!("{} {}", words[0], words[1]).as_str(),
        "/memory confirm" | "/memory reject" | "/memory prune"
    ) && (query.ends_with(' ') || words.get(2).is_some())
}

fn memory_id_suggestions(query: &str, words: &[&str], state: &TuiState) -> Vec<CommandSuggestion> {
    let base = format!("{} {}", words[0], words[1]);
    let partial_id = if query.ends_with(' ') {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    state
        .memory
        .iter()
        .filter(|entry| memory_matches_action(&base, entry.status))
        .filter(|entry| entry.memory_id.starts_with(partial_id))
        .map(|entry| CommandSuggestion {
            command: format!("{base} {}", entry.memory_id),
            summary: format!("{} [{}]", entry.content, entry.status),
        })
        .collect()
}

fn memory_matches_action(base: &str, status: MemoryStatus) -> bool {
    match base {
        "/memory confirm" | "/memory reject" => status == MemoryStatus::Suggested,
        "/memory prune" => matches!(status, MemoryStatus::Active | MemoryStatus::Suggested),
        _ => false,
    }
}

fn git_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    if !query.starts_with("/git ") {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= 1 || query.ends_with(' ') && words.len() == 1 {
        GIT_COMMANDS
            .into_iter()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 2 && !query.ends_with(' ') {
        GIT_COMMANDS
            .into_iter()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if should_suggest_git_paths(query, &words) {
        git_path_suggestions(query, &words, state)
    } else if should_suggest_git_push_targets(query, &words) {
        git_push_target_suggestions(query, &words, state)
    } else if should_suggest_git_branches(query, &words) {
        git_branch_suggestions(query, &words, state)
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn should_suggest_git_paths(query: &str, words: &[&str]) -> bool {
    if words.len() < 2 {
        return false;
    }
    match format!("{} {}", words[0], words[1]).as_str() {
        "/git diff" => words.len() <= 3 && (query.ends_with(' ') || words.get(2).is_some()),
        "/git add" => {
            words.len() <= 4
                && (query.ends_with(' ') || words.last().is_some_and(|word| !is_git_add_flag(word)))
        }
        "/git restore" => {
            words.len() <= 6
                && !restore_source_ref_is_current_token(query, words)
                && (query.ends_with(' ')
                    || words
                        .last()
                        .is_some_and(|word| !is_git_restore_option_value(words, word)))
        }
        _ => false,
    }
}

fn restore_source_ref_is_current_token(query: &str, words: &[&str]) -> bool {
    let Some(source_index) = words.iter().position(|word| *word == "--source") else {
        return false;
    };
    words.len() == source_index + 1 || (!query.ends_with(' ') && words.len() == source_index + 2)
}

fn git_path_suggestions(query: &str, words: &[&str], state: &TuiState) -> Vec<CommandSuggestion> {
    let (prefix, partial_path) = command_prefix_and_partial_path(query, words);
    workspace_path_suggestions(prefix, partial_path, state, "Workspace file")
}

fn command_prefix_and_partial_path<'a>(query: &str, words: &'a [&str]) -> (String, &'a str) {
    if query.ends_with(' ') {
        (query.to_string(), "")
    } else {
        let partial_path = words.last().copied().unwrap_or("");
        let prefix = query
            .strip_suffix(partial_path)
            .unwrap_or(query)
            .to_string();
        (prefix, partial_path)
    }
}

fn workspace_path_suggestions(
    prefix: String,
    partial_path: &str,
    state: &TuiState,
    summary: &str,
) -> Vec<CommandSuggestion> {
    state
        .workspace
        .workspace_paths
        .iter()
        .filter(|path| path.starts_with(partial_path))
        .map(|path| CommandSuggestion {
            command: format!("{prefix}{path}"),
            summary: summary.to_string(),
        })
        .collect()
}

fn is_git_add_flag(word: &str) -> bool {
    matches!(word, "--all" | "-A")
}

fn is_git_restore_option_value(words: &[&str], word: &str) -> bool {
    word.starts_with('-') || words.windows(2).any(|pair| pair == ["--source", word])
}

fn should_suggest_git_branches(query: &str, words: &[&str]) -> bool {
    words.len() >= 2
        && words.len() <= 3
        && format!("{} {}", words[0], words[1]) == "/git switch"
        && (query.ends_with(' ') || words.get(2).is_some())
}

fn git_branch_suggestions(query: &str, words: &[&str], state: &TuiState) -> Vec<CommandSuggestion> {
    let partial_branch = if query.ends_with(' ') {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    state
        .workspace
        .git_branches
        .iter()
        .filter(|branch| branch.starts_with(partial_branch))
        .map(|branch| CommandSuggestion {
            command: format!("/git switch {branch}"),
            summary: if branch == &state.workspace.git_branch {
                "Current branch".to_string()
            } else {
                "Local branch".to_string()
            },
        })
        .collect()
}

fn should_suggest_git_push_targets(query: &str, words: &[&str]) -> bool {
    words.len() >= 2
        && words.len() <= 4
        && format!("{} {}", words[0], words[1]) == "/git push"
        && (query.ends_with(' ') || words.get(2).is_some())
}

fn git_push_target_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    if words.get(2).is_some_and(|target| {
        state
            .workspace
            .git_remotes
            .iter()
            .any(|remote| remote == target)
    }) && (query.ends_with(' ') || words.len() == 4)
    {
        return git_push_remote_branch_suggestions(query, words, state);
    }

    let partial_target = if query.ends_with(' ') {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    let mut suggestions = Vec::new();
    suggestions.extend(
        state
            .workspace
            .git_branches
            .iter()
            .filter(|branch| branch.starts_with(partial_target))
            .map(|branch| CommandSuggestion {
                command: format!("/git push {branch}"),
                summary: "Local branch".to_string(),
            }),
    );
    suggestions.extend(
        state
            .workspace
            .git_remotes
            .iter()
            .filter(|remote| remote.starts_with(partial_target))
            .map(|remote| CommandSuggestion {
                command: format!("/git push {remote}"),
                summary: "Remote".to_string(),
            }),
    );
    suggestions
}

fn git_push_remote_branch_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    let remote = words.get(2).copied().unwrap_or("");
    let partial_branch = if query.ends_with(' ') {
        ""
    } else {
        words.get(3).copied().unwrap_or("")
    };
    state
        .workspace
        .git_remote_branches
        .iter()
        .filter(|branch| branch.remote == remote)
        .filter(|branch| branch.branch.starts_with(partial_branch))
        .map(|branch| CommandSuggestion {
            command: format!("/git push {remote} {}", branch.branch),
            summary: format!("Remote branch {remote}/{}", branch.branch),
        })
        .collect()
}

fn git_stash_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    if !(query == "/git stash " || query.starts_with("/git stash ")) {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= 2 && query.ends_with(' ') {
        GIT_STASH_COMMANDS
            .into_iter()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 3 && !query.ends_with(' ') {
        GIT_STASH_COMMANDS
            .into_iter()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if should_suggest_stash_refs(query, &words) {
        git_stash_ref_suggestions(query, &words, state)
    } else if should_suggest_stash_push_paths(query, &words) {
        git_stash_push_path_suggestions(query, &words, state)
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn should_suggest_stash_refs(query: &str, words: &[&str]) -> bool {
    words.len() >= 3
        && words.len() <= 4
        && matches!(
            format!("{} {} {}", words[0], words[1], words[2]).as_str(),
            "/git stash pop" | "/git stash drop"
        )
        && (query.ends_with(' ') || words.get(3).is_some())
}

fn should_suggest_stash_push_paths(query: &str, words: &[&str]) -> bool {
    words.len() >= 3
        && words.len() <= 6
        && format!("{} {} {}", words[0], words[1], words[2]) == "/git stash push"
        && (query.ends_with(' ')
            || words
                .last()
                .is_some_and(|word| !word.starts_with('-') && *word != "-m"))
}

fn git_stash_push_path_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    let (prefix, partial_path) = command_prefix_and_partial_path(query, words);
    workspace_path_suggestions(prefix, partial_path, state, "Workspace file")
}

fn git_stash_ref_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    let base = format!("{} {} {}", words[0], words[1], words[2]);
    let partial_ref = if query.ends_with(' ') {
        ""
    } else {
        words.get(3).copied().unwrap_or("")
    };
    state
        .workspace
        .git_stashes
        .iter()
        .filter(|stash| stash.reference.starts_with(partial_ref))
        .map(|stash| CommandSuggestion {
            command: format!("{base} {}", stash.reference),
            summary: stash.summary.clone(),
        })
        .collect()
}

fn git_worktree_command_suggestions(
    query: &str,
    state: &TuiState,
) -> Option<Vec<CommandSuggestion>> {
    if !(query == "/git worktree " || query.starts_with("/git worktree ")) {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= 2 && query.ends_with(' ') {
        GIT_WORKTREE_COMMANDS
            .into_iter()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 3 && !query.ends_with(' ') {
        GIT_WORKTREE_COMMANDS
            .into_iter()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if should_suggest_worktree_paths(query, &words) {
        git_worktree_path_suggestions(query, &words, state)
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn should_suggest_worktree_paths(query: &str, words: &[&str]) -> bool {
    words.len() >= 3
        && words.len() <= 4
        && format!("{} {} {}", words[0], words[1], words[2]) == "/git worktree remove"
        && (query.ends_with(' ') || words.get(3).is_some())
}

fn git_worktree_path_suggestions(
    query: &str,
    words: &[&str],
    state: &TuiState,
) -> Vec<CommandSuggestion> {
    let partial_path = if query.ends_with(' ') {
        ""
    } else {
        words.get(3).copied().unwrap_or("")
    };
    state
        .workspace
        .git_worktrees
        .iter()
        .filter(|worktree| worktree.path != state.workspace.root.to_string_lossy())
        .filter(|worktree| worktree.path.starts_with(partial_path))
        .map(|worktree| CommandSuggestion {
            command: format!("/git worktree remove {}", worktree.path),
            summary: worktree
                .branch
                .as_ref()
                .map(|branch| format!("Branch {branch}"))
                .unwrap_or_else(|| "Detached worktree".to_string()),
        })
        .collect()
}

fn lsp_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    if !query.starts_with("/lsp ") {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= 1 || query.ends_with(' ') && words.len() == 1 {
        LSP_COMMANDS
            .into_iter()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 2 && !query.ends_with(' ') {
        LSP_COMMANDS
            .into_iter()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if should_suggest_lsp_paths(query, &words) {
        lsp_path_suggestions(query, &words, state)
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn should_suggest_lsp_paths(query: &str, words: &[&str]) -> bool {
    if words.len() < 2 || words.len() > 3 {
        return false;
    }
    matches!(
        format!("{} {}", words[0], words[1]).as_str(),
        "/lsp diagnostics" | "/lsp symbols" | "/lsp references"
    ) && (query.ends_with(' ') || words.get(2).is_some())
}

fn lsp_path_suggestions(query: &str, words: &[&str], state: &TuiState) -> Vec<CommandSuggestion> {
    let base = format!("{} {}", words[0], words[1]);
    let partial_path = if query.ends_with(' ') {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    state
        .workspace
        .workspace_paths
        .iter()
        .filter(|path| path.starts_with(partial_path))
        .map(|path| {
            let suffix = if base == "/lsp references" {
                " 0 0"
            } else {
                ""
            };
            CommandSuggestion {
                command: format!("{base} {path}{suffix}"),
                summary: "Workspace file".to_string(),
            }
        })
        .collect()
}

fn command_suggestion_row(
    suggestion: &CommandSuggestion,
    selected: bool,
    width: usize,
    command_width: usize,
    detail_width: usize,
) -> String {
    let content_width = width.saturating_sub(4);
    let detail_width = detail_width.min(content_width.saturating_sub(4));
    let command_width = command_width.min(content_width.saturating_sub(detail_width + 4));
    let marker = if selected { "›" } else { " " };
    bordered_row(
        &format!(
            "{marker} {}{}{}",
            pad(&suggestion.command, command_width),
            " ".repeat(2),
            pad(&suggestion.summary, detail_width)
        ),
        width,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{CompanionScreen, ProviderStatus, TerminalLane, WorkspaceSnapshot};
    use crate::tui::text::char_width;
    use robocode_types::{
        MemoryEntry, MemoryKind, MemoryScope, MemorySource, MemoryStatus, TaskPriority, TaskRecord,
        TaskStatus,
    };

    fn state_with_input(input: &str) -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: input.to_string(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
        }
    }

    fn task(id: &str, title: &str, status: TaskStatus) -> TaskRecord {
        TaskRecord {
            task_id: id.to_string(),
            title: title.to_string(),
            description: None,
            status,
            priority: TaskPriority::Medium,
            labels: Vec::new(),
            assignee_hint: None,
            parent_task_id: None,
            dependency_ids: Vec::new(),
            blocked_by: None,
            notes: Vec::new(),
            created_at: 1,
            updated_at: 2,
            last_session_id: None,
            last_seen_at: None,
            archived_at: None,
        }
    }

    fn memory(id: &str, content: &str, status: MemoryStatus) -> MemoryEntry {
        MemoryEntry {
            memory_id: id.to_string(),
            scope: MemoryScope::Project,
            session_id: Some("session_123".to_string()),
            kind: MemoryKind::Fact,
            content: content.to_string(),
            source: MemorySource::AssistantSuggestion,
            status,
            created_at: 1,
            updated_at: 2,
            related_task_ids: Vec::new(),
            confidence_hint: None,
        }
    }

    #[test]
    fn filters_slash_commands_by_prefix() {
        let suggestions = static_command_suggestions("/p");

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.command.as_str())
                .collect::<Vec<_>>(),
            vec!["/provider", "/permissions", "/plan"]
        );
    }

    #[test]
    fn suggests_lane_subcommands_after_lane_space() {
        let state = state_with_input("/lane ");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/lane codex");
        assert!(suggestions.iter().any(|item| item.command == "/lane ask"));
        assert!(suggestions.iter().any(|item| item.command == "/lane diff"));
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/lane timeline")
        );
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/lane artifacts")
        );
        assert!(suggestions.iter().any(|item| item.command == "/lane apply"));
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/lane resolve")
        );
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/lane archive")
        );
        assert!(is_command_palette_visible(&state));
    }

    #[test]
    fn suggests_tmux_lane_command_and_lane_ids() {
        let state = state_with_input("/lane tmux ");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.command.as_str())
                .collect::<Vec<_>>(),
            vec!["/lane tmux L1", "/lane tmux L2", "/lane tmux L3"]
        );
    }

    #[test]
    fn suggests_archive_lane_command_and_lane_ids() {
        let state = state_with_input("/lane archive ");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.command.as_str())
                .collect::<Vec<_>>(),
            vec!["/lane archive L1", "/lane archive L2", "/lane archive L3"]
        );
    }

    #[test]
    fn filters_lane_subcommands_by_partial_argument() {
        let state = state_with_input("/lane a");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.command.as_str())
                .collect::<Vec<_>>(),
            vec![
                "/lane ask",
                "/lane artifacts",
                "/lane attach",
                "/lane accept",
                "/lane apply",
                "/lane archive"
            ]
        );
    }

    #[test]
    fn suggests_lane_ids_for_lane_actions() {
        let state = state_with_input("/lane inspect l");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.command.as_str())
                .collect::<Vec<_>>(),
            vec!["/lane inspect L1", "/lane inspect L2", "/lane inspect L3"]
        );
        assert!(suggestions[0].summary.contains("[running]"));
    }

    #[test]
    fn suggests_common_nested_command_families() {
        let provider = state_with_input("/provider ");
        let settings = state_with_input("/settings ");
        let settings_provider = state_with_input("/settings provider dee");
        let settings_model = state_with_input("/settings model fall");
        let setup_provider = state_with_input("/setup provider dee");
        let setup_model = state_with_input("/setup model fall");
        let agent = state_with_input("/agent ");
        let brief = state_with_input("/brief ");
        let spec = state_with_input("/spec ");
        let extensions = state_with_input("/extensions ");
        let mcp = state_with_input("/mcp ");
        let skills = state_with_input("/skills ");
        let git = state_with_input("/git st");
        let stash = state_with_input("/git stash p");
        let memory = state_with_input("/memory ");

        assert!(
            command_suggestions_for_state(&provider)
                .iter()
                .any(|item| item.command == "/provider deepseek")
        );
        assert!(
            command_suggestions_for_state(&settings)
                .iter()
                .any(|item| item.command == "/settings provider")
        );
        assert!(
            command_suggestions_for_state(&settings)
                .iter()
                .any(|item| item.command == "/settings permissions")
        );
        assert!(
            command_suggestions_for_state(&settings)
                .iter()
                .any(|item| item.command == "/settings theme")
        );
        assert!(
            command_suggestions_for_state(&settings_provider)
                .iter()
                .any(|item| item.command == "/settings provider deepseek")
        );
        assert!(
            command_suggestions_for_state(&settings_model)
                .iter()
                .any(|item| item.command == "/settings model fallback-local")
        );
        assert!(
            command_suggestions_for_state(&setup_provider)
                .iter()
                .any(|item| item.command == "/setup provider deepseek")
        );
        assert!(
            command_suggestions_for_state(&setup_model)
                .iter()
                .any(|item| item.command == "/setup model fallback-local")
        );
        assert!(
            command_suggestions_for_state(&agent)
                .iter()
                .any(|item| item.command == "/agent doctor")
        );
        assert!(
            command_suggestions_for_state(&brief)
                .iter()
                .any(|item| item.command == "/brief steering init")
        );
        assert!(
            command_suggestions_for_state(&spec)
                .iter()
                .any(|item| item.command == "/spec show")
        );
        assert!(
            command_suggestions_for_state(&extensions)
                .iter()
                .any(|item| item.command == "/extensions doctor")
        );
        assert!(
            command_suggestions_for_state(&mcp)
                .iter()
                .any(|item| item.command == "/mcp doctor")
        );
        assert!(
            command_suggestions_for_state(&skills)
                .iter()
                .any(|item| item.command == "/skills list")
        );
        assert_eq!(
            command_suggestions_for_state(&git)
                .iter()
                .map(|suggestion| suggestion.command.as_str())
                .collect::<Vec<_>>(),
            vec!["/git status", "/git stash"]
        );
        assert_eq!(
            command_suggestions_for_state(&stash)[0].command,
            "/git stash push"
        );
        assert!(
            command_suggestions_for_state(&memory)
                .iter()
                .any(|item| item.command == "/memory confirm")
        );
    }

    #[test]
    fn suggests_screen_ids_for_close() {
        let mut state = state_with_input("/screen close s");
        state.screens = vec![CompanionScreen {
            id: "side-1".to_string(),
            title: "Lane monitor".to_string(),
            status: "launched".to_string(),
            pid: Some(4242),
            summary: "lane monitor".to_string(),
        }];

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/screen close side-1");
        assert_eq!(suggestions[0].summary, "lane monitor");
    }

    #[test]
    fn suggests_task_ids_and_task_statuses() {
        let mut state = state_with_input("/task status task_");
        state.tasks = vec![task(
            "task_load_config",
            "Implement load_config",
            TaskStatus::InProgress,
        )];

        let task_ids = command_suggestions_for_state(&state);

        assert_eq!(task_ids[0].command, "/task status task_load_config");
        assert!(task_ids[0].summary.contains("[in_progress]"));

        state.input = "/task status task_load_config ".to_string();
        let statuses = command_suggestions_for_state(&state);

        assert_eq!(statuses[0].command, "/task status task_load_config todo");
        assert!(
            statuses
                .iter()
                .any(|item| item.command.ends_with(" in_progress"))
        );
    }

    #[test]
    fn suggests_memory_ids_for_confirmation_and_pruning() {
        let mut state = state_with_input("/memory confirm mem_");
        state.memory = vec![
            memory(
                "mem_pending",
                "Keep TUI docs current",
                MemoryStatus::Suggested,
            ),
            memory("mem_active", "Use aurora-cyan theme", MemoryStatus::Active),
        ];

        let confirm = command_suggestions_for_state(&state);

        assert_eq!(confirm[0].command, "/memory confirm mem_pending");
        assert!(confirm[0].summary.contains("[suggested]"));
        assert!(
            !confirm
                .iter()
                .any(|item| item.command.contains("mem_active"))
        );

        state.input = "/memory prune mem_".to_string();
        let prune = command_suggestions_for_state(&state);

        assert_eq!(
            prune
                .iter()
                .map(|suggestion| suggestion.command.as_str())
                .collect::<Vec<_>>(),
            vec!["/memory prune mem_pending", "/memory prune mem_active"]
        );
    }

    #[test]
    fn suggests_provider_ids_and_default_models() {
        let mut state = state_with_input("/provider dee");

        let providers = command_suggestions_for_state(&state);

        assert_eq!(providers[0].command, "/provider deepseek");
        assert_eq!(providers[0].summary, "deepseek");

        state.input = "/provider".to_string();
        let root = command_suggestions_for_state(&state);
        assert!(
            root.iter()
                .any(|item| item.command == "/provider openrouter")
        );
        assert!(!root.iter().any(|item| item.command == "/provider fallback"));
        assert!(root.iter().all(|item| item.summary.is_empty()));

        state.input = "/provider deepseek ".to_string();
        let actions = command_suggestions_for_state(&state);

        assert_eq!(actions[0].command, "/settings provider deepseek");
        assert_eq!(actions[0].summary, "deepseek");
        assert!(
            actions
                .iter()
                .any(|item| item.command == "/provider doctor deepseek")
        );
        assert!(
            actions
                .iter()
                .any(|item| item.command == "/settings provider deepseek deepseek-v4-flash")
        );
        let model_action = actions
            .iter()
            .find(|item| item.command == "/settings provider deepseek deepseek-v4-flash")
            .expect("deepseek model action");
        assert!(model_action.summary.is_empty());
        assert_eq!(mask_secret("sk-1234567890abcd"), "sk-1*********abcd");
        assert_eq!(mask_secret("abcd"), "****");
    }

    #[test]
    fn keeps_legacy_provider_use_suggestions() {
        let mut state = state_with_input("/provider use dee");

        let providers = command_suggestions_for_state(&state);

        assert_eq!(providers[0].command, "/provider use deepseek");

        state.input = "/provider use deepseek deep".to_string();
        let models = command_suggestions_for_state(&state);

        assert_eq!(
            models[0].command,
            "/provider use deepseek deepseek-v4-flash"
        );
    }

    #[test]
    fn suggests_models_for_current_provider() {
        let mut state = state_with_input("/model deep");
        state.provider = "deepseek".to_string();

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/model deepseek-v4-flash");
        assert!(suggestions[0].summary.contains("DeepSeek default"));
    }

    #[test]
    fn suggests_models_alias_grouped_by_provider() {
        let mut state = state_with_input("/models deep");
        state.provider = "deepseek".to_string();

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(
            suggestions[0].command,
            "/settings provider deepseek deepseek-v4-flash"
        );
        assert!(suggestions[0].summary.contains("DeepSeek default"));
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/settings provider deepseek deepseek-v4-pro")
        );
    }

    #[test]
    fn settings_root_renders_actionable_selector_items() {
        let state = state_with_input("/settings");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/settings provider");
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/settings permissions")
        );
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/settings theme")
        );
        assert_eq!(selector_kind(&state.input), Some(SelectorKind::Settings));
    }

    #[test]
    fn setup_root_renders_first_run_wizard_items() {
        let mut state = state_with_input("/setup");
        state.provider = "deepseek".to_string();

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/setup provider");
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/provider doctor deepseek")
        );
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/provider fallback test-local")
        );
        assert_eq!(selector_kind(&state.input), Some(SelectorKind::Setup));
    }

    #[test]
    fn suggests_permission_and_theme_settings_values() {
        let permissions = state_with_input("/settings permissions");
        let themes = state_with_input("/settings theme");

        assert!(
            command_suggestions_for_state(&permissions)
                .iter()
                .any(|item| item.command == "/settings permissions acceptEdits")
        );
        assert!(
            command_suggestions_for_state(&themes)
                .iter()
                .any(|item| item.command == "/settings theme ember-gold")
        );
    }

    #[test]
    fn exact_provider_and_model_roots_open_selectors() {
        let mut provider = state_with_input("/provider");
        provider.provider = "deepseek".to_string();
        let mut model = state_with_input("/models");
        model.provider = "deepseek".to_string();

        assert_eq!(
            command_suggestions_for_state(&provider)[0].command,
            "/provider anthropic"
        );
        assert_eq!(
            command_suggestions_for_state(&model)[0].command,
            "/settings provider anthropic claude-sonnet-4-6"
        );
    }

    #[test]
    fn lane_root_opens_action_selector_with_lane_ids() {
        let mut state = state_with_input("/lane");
        state.lanes = TerminalLane::preview_lanes();

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(selector_kind(&state.input), Some(SelectorKind::Lane));
        assert_eq!(suggestions[0].command, "/lane inspect L1");
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/lane inspect L1")
        );
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/lane artifacts L1")
        );
    }

    #[test]
    fn suggests_git_branches_for_switch() {
        let state = state_with_input("/git switch codex/");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/git switch codex/tui-cockpit");
        assert_eq!(suggestions[0].summary, "Local branch");
    }

    #[test]
    fn suggests_git_push_remotes_and_remote_branches() {
        let mut state = state_with_input("/git push ori");

        let remotes = command_suggestions_for_state(&state);

        assert_eq!(remotes[0].command, "/git push origin");
        assert_eq!(remotes[0].summary, "Remote");

        state.input = "/git push origin rel".to_string();
        let branches = command_suggestions_for_state(&state);

        assert_eq!(branches[0].command, "/git push origin release/v0.1.4");
        assert_eq!(branches[0].summary, "Remote branch origin/release/v0.1.4");

        state.input = "/git push codex/".to_string();
        let local_branches = command_suggestions_for_state(&state);

        assert_eq!(local_branches[0].command, "/git push codex/tui-cockpit");
        assert_eq!(local_branches[0].summary, "Local branch");
    }

    #[test]
    fn suggests_workspace_paths_for_git_path_commands() {
        let mut state = state_with_input("/git add src/");

        let add = command_suggestions_for_state(&state);

        assert_eq!(add[0].command, "/git add src/config.rs");
        assert_eq!(add[0].summary, "Workspace file");

        state.input = "/git restore --staged tests/".to_string();
        let restore = command_suggestions_for_state(&state);

        assert_eq!(
            restore[0].command,
            "/git restore --staged tests/config_tests.rs"
        );

        state.input = "/git diff Cargo".to_string();
        let diff = command_suggestions_for_state(&state);

        assert_eq!(diff[0].command, "/git diff Cargo.toml");
    }

    #[test]
    fn suggests_workspace_paths_for_git_stash_push() {
        let state = state_with_input("/git stash push src/");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/git stash push src/config.rs");
        assert_eq!(suggestions[0].summary, "Workspace file");
    }

    #[test]
    fn suggests_stash_refs_for_pop_and_drop() {
        let mut state = state_with_input("/git stash pop stash@");

        let pop = command_suggestions_for_state(&state);

        assert_eq!(pop[0].command, "/git stash pop stash@{0}");
        assert!(pop[0].summary.contains("tune cockpit palette"));

        state.input = "/git stash drop stash@{1".to_string();
        let drop = command_suggestions_for_state(&state);

        assert_eq!(drop[0].command, "/git stash drop stash@{1}");
        assert!(drop[0].summary.contains("checkpoint preview assets"));
    }

    #[test]
    fn suggests_worktree_paths_for_remove() {
        let state = state_with_input("/git worktree remove /tmp/robocode/.worktrees/");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(
            suggestions[0].command,
            "/git worktree remove /tmp/robocode/.worktrees/codex-tui-cockpit"
        );
        assert_eq!(suggestions[0].summary, "Branch codex/tui-cockpit");
    }

    #[test]
    fn suggests_recent_files_for_lsp_commands() {
        let state = state_with_input("/lsp diagnostics src/");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/lsp diagnostics src/config.rs");
        assert!(
            suggestions
                .iter()
                .any(|item| item.command == "/lsp diagnostics src/lib.rs")
        );
    }

    #[test]
    fn moves_and_clamps_selection() {
        let mut state = state_with_input("/p");

        assert!(move_selection(&mut state, 1));
        assert_eq!(state.command_selection, 1);
        assert!(move_selection(&mut state, 1));
        assert_eq!(state.command_selection, 2);
        assert!(move_selection(&mut state, 1));
        assert_eq!(state.command_selection, 2);
        assert!(move_selection(&mut state, -1));
        assert_eq!(state.command_selection, 1);
        assert!(move_selection(&mut state, -1));
        assert_eq!(state.command_selection, 0);
    }

    #[test]
    fn completes_selected_command_with_trailing_space() {
        let mut state = state_with_input("/lane a");
        state.command_selection = 4;

        assert!(complete_selected(&mut state));

        assert_eq!(state.input, "/lane apply ");
        assert_eq!(state.command_selection, 0);
    }

    #[test]
    fn mouse_hit_testing_selects_visible_suggestion_rows() {
        let mut state = state_with_input("/p");

        assert_eq!(command_suggestion_index_at(&state, 4, 25, 140, 36), Some(0));
        assert_eq!(command_suggestion_index_at(&state, 4, 26, 140, 36), Some(1));
        assert_eq!(command_suggestion_index_at(&state, 4, 27, 140, 36), Some(2));
        assert_eq!(command_suggestion_index_at(&state, 4, 24, 140, 36), None);
        assert_eq!(command_suggestion_index_at(&state, 4, 28, 140, 36), None);
        assert!(select_suggestion_at(&mut state, 1));

        assert_eq!(state.command_selection, 1);
        assert_eq!(selected_command(&state).unwrap().command, "/permissions");
    }

    #[test]
    fn long_command_lists_scroll_to_keep_keyboard_selection_visible() {
        let mut state = state_with_input("/lane ");
        state.command_selection = 8;
        let mut frame = Frame::new(140, 36);

        render_command_suggestions(&mut frame, &state);
        let rendered = frame.to_string();

        assert_eq!(visible_command_window_start(8, 24, MAX_VISIBLE_COMMANDS), 5);
        assert!(rendered.contains("6-11/24"));
        assert!(rendered.contains("› /lane artifacts"));
        assert!(!rendered.contains(" /lane codex "));
    }

    #[test]
    fn mouse_hit_testing_uses_scrolled_visible_window_indices() {
        let mut state = state_with_input("/lane ");
        state.command_selection = 8;

        assert_eq!(command_suggestion_index_at(&state, 4, 25, 140, 36), Some(8));
        assert!(select_suggestion_at(&mut state, 8));
        assert!(complete_selected(&mut state));

        assert_eq!(state.input, "/lane artifacts ");
    }

    #[test]
    fn mouse_hit_testing_supports_centered_setting_selectors() {
        let mut state = state_with_input("/settings");

        assert_eq!(
            command_suggestion_index_at(&state, 70, 14, 140, 36),
            Some(0)
        );
        assert!(select_suggestion_at(&mut state, 2));
        assert!(complete_selected(&mut state));

        assert_eq!(state.input, "/settings save ");
    }

    #[test]
    fn escape_hides_until_query_changes() {
        let mut state = state_with_input("/p");

        assert!(is_command_palette_visible(&state));
        assert!(close_on_escape(
            KeyEvent::new(KeyCode::Esc, crossterm::event::KeyModifiers::empty()),
            &mut state
        ));
        assert!(!is_command_palette_visible(&state));

        state.input.push('r');
        reset_for_input_change(&mut state);

        assert!(is_command_palette_visible(&state));
    }

    #[test]
    fn enter_only_completes_partial_commands() {
        let partial = state_with_input("/p");
        let exact = state_with_input("/help");
        let partial_quit = state_with_input("/q");
        let exact_quit = state_with_input("/quit");
        let partial_exit = state_with_input("/ex");
        let exact_exit = state_with_input("/exit");

        assert!(should_complete_on_enter(&partial));
        assert!(!should_complete_on_enter(&exact));
        assert!(should_complete_on_enter(&partial_quit));
        assert!(!should_complete_on_enter(&exact_quit));
        assert!(should_complete_on_enter(&partial_exit));
        assert!(!should_complete_on_enter(&exact_exit));
    }

    #[test]
    fn suggestion_rows_preserve_summary_inside_border_width() {
        let row = command_suggestion_row(
            &CommandSuggestion {
                command: "/git add src/config.rs".to_string(),
                summary: "1234567890123456789012".to_string(),
            },
            true,
            60,
            34,
            22,
        );

        assert_eq!(char_width(&row), 60);
        assert!(row.contains("1234567890123456789012"), "{row}");
    }
}
