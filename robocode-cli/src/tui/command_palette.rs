use crossterm::event::{KeyCode, KeyEvent};
use robocode_types::MemoryStatus;

use super::{
    canvas::Frame,
    composer::COMPOSER_HEIGHT,
    panel::{bordered_row, panel_top},
    state::TuiState,
    statusbar::BOTTOM_BAR_HEIGHT,
    text::{bottom_border, pad},
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

    let width = frame.width.saturating_sub(8).clamp(48, 104);
    let visible = suggestions.len().min(MAX_VISIBLE_COMMANDS);
    let height = visible + 2;
    let left = 2usize.min(frame.width.saturating_sub(width));
    let composer_top = frame
        .height
        .saturating_sub(BOTTOM_BAR_HEIGHT)
        .saturating_sub(COMPOSER_HEIGHT);
    let top = composer_top.saturating_sub(height);
    let detail_width = width.saturating_mul(2).saturating_div(5).max(22);
    let command_width = width.saturating_sub(detail_width + 4);
    let selected = state
        .command_selection
        .min(suggestions.len().saturating_sub(1));
    let mut rows = Vec::with_capacity(height);
    rows.push(panel_top("COMMANDS", width, Some("↑↓ tab enter esc")));
    for (index, suggestion) in suggestions.iter().take(visible).enumerate() {
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
struct CommandTemplate {
    command: &'static str,
    summary: &'static str,
}

const COMMANDS: [CommandTemplate; 16] = [
    CommandTemplate {
        command: "/help",
        summary: "Show commands",
    },
    CommandTemplate {
        command: "/provider",
        summary: "List or switch providers",
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
        command: "/lsp",
        summary: "Diagnostics and symbols",
    },
    CommandTemplate {
        command: "/task",
        summary: "Create or update tasks",
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
        command: "/screen",
        summary: "Open side screen route",
    },
    CommandTemplate {
        command: "/lane",
        summary: "Run or inspect agent lanes",
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

const LANE_COMMANDS: [CommandTemplate; 13] = [
    CommandTemplate {
        command: "/lane codex",
        summary: "Start Codex lane",
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
        command: "/lane inspect",
        summary: "Inspect lane evidence",
    },
    CommandTemplate {
        command: "/lane stop",
        summary: "Stop running lane",
    },
    CommandTemplate {
        command: "/lane attach",
        summary: "Open lane terminal",
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
        command: "/lane cleanup",
        summary: "Archive worktree",
    },
    CommandTemplate {
        command: "/lane close",
        summary: "Close lane focus",
    },
];

const LANE_ID_COMMANDS: [&str; 9] = [
    "/lane inspect",
    "/lane stop",
    "/lane attach",
    "/lane detach",
    "/lane accept",
    "/lane revise",
    "/lane discard",
    "/lane apply",
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
        "/screen",
        "/provider",
        "/lsp",
        "/task",
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
        .or_else(|| screen_command_suggestions(query, state))
        .or_else(|| task_command_suggestions(query, state))
        .or_else(|| lsp_command_suggestions(query, state))
        .or_else(|| command_group_suggestions(query, "/git stash", &GIT_STASH_COMMANDS))
        .or_else(|| command_group_suggestions(query, "/git worktree", &GIT_WORKTREE_COMMANDS))
        .or_else(|| command_group_suggestions(query, "/provider", &PROVIDER_COMMANDS))
        .or_else(|| memory_command_suggestions(query, state))
        .or_else(|| git_command_suggestions(query, state))
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

fn command_group_suggestions(
    query: &str,
    root: &str,
    commands: &[CommandTemplate],
) -> Option<Vec<CommandSuggestion>> {
    if !(query == format!("{root} ") || query.starts_with(&format!("{root} "))) {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= root.split_whitespace().count() && query.ends_with(' ') {
        commands
            .iter()
            .copied()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if query.ends_with(' ') {
        Vec::new()
    } else {
        commands
            .iter()
            .copied()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    };
    Some(suggestions)
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
    } else if should_suggest_git_branches(query, &words) {
        git_branch_suggestions(query, &words, state)
    } else {
        Vec::new()
    };
    Some(suggestions)
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
        .recent_files
        .iter()
        .filter(|file| file.path.starts_with(partial_path))
        .map(|file| {
            let suffix = if base == "/lsp references" {
                " 0 0"
            } else {
                ""
            };
            CommandSuggestion {
                command: format!("{base} {}{suffix}", file.path),
                summary: "Recent file".to_string(),
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
    use robocode_types::{
        MemoryEntry, MemoryKind, MemoryScope, MemorySource, MemoryStatus, TaskPriority, TaskRecord,
        TaskStatus,
    };

    fn state_with_input(input: &str) -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: input.to_string(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
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
            vec!["/provider", "/plan"]
        );
    }

    #[test]
    fn suggests_lane_subcommands_after_lane_space() {
        let state = state_with_input("/lane ");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/lane codex");
        assert!(suggestions.iter().any(|item| item.command == "/lane apply"));
        assert!(is_command_palette_visible(&state));
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
            vec!["/lane attach", "/lane accept", "/lane apply"]
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
        let git = state_with_input("/git st");
        let stash = state_with_input("/git stash p");
        let memory = state_with_input("/memory ");

        assert!(
            command_suggestions_for_state(&provider)
                .iter()
                .any(|item| item.command == "/provider use")
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
    fn suggests_git_branches_for_switch() {
        let state = state_with_input("/git switch codex/");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/git switch codex/tui-cockpit");
        assert_eq!(suggestions[0].summary, "Local branch");
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
        assert_eq!(state.command_selection, 1);
        assert!(move_selection(&mut state, -1));
        assert_eq!(state.command_selection, 0);
    }

    #[test]
    fn completes_selected_command_with_trailing_space() {
        let mut state = state_with_input("/lane a");
        state.command_selection = 2;

        assert!(complete_selected(&mut state));

        assert_eq!(state.input, "/lane apply ");
        assert_eq!(state.command_selection, 0);
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

        assert!(should_complete_on_enter(&partial));
        assert!(!should_complete_on_enter(&exact));
    }
}
