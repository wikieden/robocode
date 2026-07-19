use super::*;
use crate::presentation::{
    join_lines, render_field, render_section_title, render_subsection_title, render_summary_fields,
};
use std::path::Path;
use std::process::Command;
use viden_types::SessionSummary;

impl SessionEngine {
    pub(super) fn render_help(&self) -> String {
        [
            "Viden commands:",
            "",
            "Runtime:",
            "  /help                Show available commands",
            "  /connect             Connect or configure a provider",
            "  /provider            Show current provider and model",
            "  /provider list       List registered providers",
            "  /provider doctor     Show provider registry diagnostics",
            "  /provider reload     Reload provider plugin registry",
            "  /provider use <id>   Switch provider, optionally with a model",
            "  /settings            Configure and save provider/model defaults",
            "  /setup               First-run provider/model setup guide",
            "  /agent list          List configured agent adapters",
            "  /agent doctor [id]   Check agent adapter readiness",
            "  /agent run codex [--write] <task>",
            "  /agent review codex  Start a tracked Codex review job",
            "  /agent status        Show tracked external agent jobs",
            "  /extensions list     List extension surfaces",
            "  /mcp list            List MCP config visibility",
            "  /skills list         List local skills",
            "  /model [name]        Show or change the active model label",
            "  /mode [build|plan]   Show or change work mode",
            "  /permissions [level] Show or change permission level",
            "  /plan [on|off]       Shortcut for Plan work mode",
            "  /status              Show current runtime status",
            "  /config              Show resolved runtime configuration",
            "  /doctor              Check local dependency availability",
            "  /test <command>      Run a test command and record evidence",
            "",
            "Sessions:",
            "  /sessions            List prior sessions for this project",
            "  /resume [selector]   List or resume by latest, #index, or id prefix",
            "  /diff                Show the latest file diff recorded in session",
            "",
            "Repository and web:",
            "  /git <subcommand>    Git status/diff/add/push/worktree flows",
            "  /web <subcommand>    Search or fetch web content",
            "",
            "Code intelligence:",
            "  /lsp status          Show language server configuration",
            "  /lsp diagnostics <path>",
            "  /lsp symbols <path>",
            "  /lsp references <path> <line> <character>",
            "",
            "Workflows:",
            "  /tasks               List active project tasks",
            "  /task <subcommand>   Manage tasks or render resume context",
            "  /brief <task goal>   Create or show the active task brief",
            "  /spec <task goal>    Alias for /brief",
            "  /memory <subcommand> Manage project/session memory",
            "",
            "Fallback tool syntax:",
            "  tool read_file path=Cargo.toml",
            "  tool grep pattern=fn path=src",
        ]
        .join("\n")
    }

    pub(super) fn render_status(&self) -> String {
        let mut lines = vec![
            "Runtime status:".to_string(),
            format!("  Session: {}", self.session_id()),
            format!("  CWD: {}", self.cwd.display()),
            format!("  Provider: {}", self.provider.provider_name()),
            format!("  Model: {}", self.provider.model()),
            format!("  Work mode: {}", self.work_mode().cli_name()),
            format!("  Permission level: {}", self.permission_level().cli_name()),
            format!("  Transcript: {}", self.store.transcript_path().display()),
            format!("  Session home: {}", self.store.home_dir().display()),
            format!("  Index: {}", self.store.index_db_path().display()),
            self.last_test
                .as_ref()
                .map(|test| {
                    let exit = test
                        .exit_code
                        .map(|code| format!(" (exit {code})"))
                        .unwrap_or_default();
                    let files = if test.failing_files.is_empty() {
                        String::new()
                    } else {
                        format!(" files={}", test.failing_files.len())
                    };
                    format!(
                        "  Last test: {}{}{} in {}ms | {}",
                        test.status, exit, files, test.duration_ms, test.command
                    )
                })
                .unwrap_or_else(|| "  Last test: <none>".to_string()),
        ];
        lines.extend(render_workspace_status(&self.cwd));
        if let Some(bundle) = self.provider_context_bundle() {
            lines.push("Provider context:".to_string());
            lines.push(format!("  Policy: {}", bundle.policy));
            lines.push(format!(
                "  Pressure: {}% ({}/{})",
                bundle.pressure_percent(),
                bundle.estimated_tokens,
                bundle.hard_token_limit
            ));
            lines.push(format!("  Sources: {}", bundle.sources.len()));
            lines.push(format!("  Omitted: {}", bundle.omitted_sources.len()));
            if !bundle.largest_sources.is_empty() {
                lines.push(format!("  Largest: {}", bundle.largest_sources.join(", ")));
            }
            if !bundle.compaction_notes.is_empty() {
                lines.push(format!(
                    "  Compaction: {}",
                    bundle.compaction_notes.join("; ")
                ));
            }
        }
        if let Some(brief) = self.active_brief_snapshot() {
            lines.push("Active brief:".to_string());
            lines.push(format!("  Id: {}", brief.id));
            lines.push(format!("  Title: {}", brief.title));
            lines.push(format!("  Path: {}", brief.path.display()));
        }
        lines.extend(render_workflow_status(&self.workflows));
        lines.extend(render_lane_status(&self.workflows));
        lines.join("\n")
    }

    pub(super) fn render_config(&self) -> String {
        let loaded_files = if self.runtime_snapshot.loaded_config_files.is_empty() {
            "<none>".to_string()
        } else {
            self.runtime_snapshot
                .loaded_config_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let overrides = if self.runtime_snapshot.startup_overrides.is_empty() {
            "<none>".to_string()
        } else {
            self.runtime_snapshot.startup_overrides.join(", ")
        };
        [
            "Runtime configuration:".to_string(),
            format!("  {}", self.runtime_snapshot.config_summary),
            format!("  Loaded config files: {}", loaded_files),
            format!("  Startup overrides: {}", overrides),
        ]
        .join("\n")
    }

    pub(super) fn render_doctor(&self) -> String {
        DoctorReport::from_probe(system_dependency_status).render()
    }

    pub(super) fn render_task_help(&self) -> String {
        [
            "Task commands:",
            "  /tasks",
            "  /task add <title>",
            "  /task resume-context",
        ]
        .join("\n")
    }

    pub(super) fn render_git_help(&self) -> String {
        [
            "Git commands:",
            "  /git status",
            "  /git diff [path]",
            "  /git branch",
            "  /git add [--all|-A] <path...>",
            "  /git restore [--staged] [--source <ref>] <path...>",
            "  /git switch <branch> [--create]",
            "  /git commit [--all] <message>",
            "  /git push [branch] | [remote branch] [--set-upstream|-u]",
            "  /git stash <list|push|pop|drop>",
            "  /git worktree <list|add|remove>",
        ]
        .join("\n")
    }

    pub(super) fn render_web_help(&self) -> String {
        [
            "Web commands:",
            "  /web search <query> [--limit <n>] [--site <domain>]",
            "  /web fetch <url> [--max-bytes <n>] [--raw]",
        ]
        .join("\n")
    }

    pub(super) fn render_git_stash_help(&self) -> String {
        [
            "Git stash commands:",
            "  /git stash list",
            "  /git stash push [-m <message>] [-u] [path...]",
            "  /git stash pop [stash@{0}]",
            "  /git stash drop [stash@{0}]",
        ]
        .join("\n")
    }

    pub(super) fn render_git_worktree_help(&self) -> String {
        [
            "Git worktree commands:",
            "  /git worktree list",
            "  /git worktree add <path> [branch] [--create]",
            "  /git worktree remove <path> [--force]",
        ]
        .join("\n")
    }

    pub(super) fn render_session_list(&self, sessions: &[SessionSummary]) -> String {
        if sessions.is_empty() {
            return "No resumable sessions found for the current project.".to_string();
        }
        let current_count = sessions
            .iter()
            .filter(|summary| summary.session_id == self.session_id())
            .count();
        let mut lines = vec![
            render_section_title("Sessions for this project")
                .trim_end()
                .to_string(),
            render_summary_fields(&[
                ("total", sessions.len().to_string()),
                ("current", current_count.to_string()),
                (
                    "resumable",
                    sessions.len().saturating_sub(current_count).to_string(),
                ),
            ]),
            "  Use `/resume latest`, `/resume #<index>`, or `/resume <session-id-prefix>`."
                .to_string(),
            String::new(),
            render_subsection_title("Session entries"),
        ];
        for (index, summary) in sessions.iter().enumerate() {
            let title = summary
                .title
                .clone()
                .unwrap_or_else(|| "untitled".to_string());
            let preview = summary
                .last_preview
                .clone()
                .unwrap_or_else(|| "No preview available".to_string());
            let current = if summary.session_id == self.session_id() {
                " [current]"
            } else {
                ""
            };
            let last_activity = summary
                .last_activity_kind
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let last_activity_preview = summary
                .last_activity_preview
                .clone()
                .unwrap_or_else(|| preview.clone());
            lines.push(format!(
                "  #{} {}{}",
                index + 1,
                summary.session_id,
                current
            ));
            lines.push(render_field("title", title));
            lines.push(render_field(
                "updated",
                format_relative_age(summary.last_updated_at),
            ));
            lines.push(render_field(
                "activity",
                format!(
                    "messages={} tools={} commands={} last={}",
                    summary.message_count,
                    summary.tool_call_count,
                    summary.command_count,
                    last_activity
                ),
            ));
            lines.push(render_field("preview", preview));
            lines.push(render_field("last activity", last_activity_preview));
        }
        join_lines(&lines)
    }
}

fn render_workspace_status(cwd: &Path) -> Vec<String> {
    let mut lines = vec!["Workspace:".to_string()];
    // `/status` is an operator snapshot, so each collector degrades independently
    // instead of failing the whole command when git/workflow/lane data is missing.
    match git_dirty_files(cwd) {
        Ok(files) if files.is_empty() => lines.push("  Dirty files: 0".to_string()),
        Ok(files) => {
            let preview = files.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
            let suffix = if files.len() > 5 {
                format!(" (+{} more)", files.len() - 5)
            } else {
                String::new()
            };
            lines.push(format!(
                "  Dirty files: {} ({preview}{suffix})",
                files.len()
            ));
        }
        Err(err) => lines.push(format!("  Dirty files: <unavailable: {err}>")),
    }
    lines
}

fn git_dirty_files(cwd: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["status", "--short", "--untracked-files=all"])
        .current_dir(cwd)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "git status failed".to_string()
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.get(3..).or_else(|| line.get(0..)))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToString::to_string)
        .collect())
}

fn render_workflow_status(workflows: &WorkflowStore) -> Vec<String> {
    let mut lines = vec!["Workflows:".to_string()];
    match workflows.load_task_state() {
        Ok(state) => {
            let active = state.active_tasks();
            if active.is_empty() {
                lines.push("  Active tasks: 0".to_string());
            } else {
                lines.push(format!("  Active tasks: {}", active.len()));
                lines.extend(active.into_iter().take(3).map(|task| {
                    format!(
                        "    {} {} {}",
                        task.task_id,
                        format!("{:?}", task.status).to_lowercase(),
                        task.title
                    )
                }));
            }
        }
        Err(err) => lines.push(format!("  Active tasks: <unavailable: {err}>")),
    }
    lines
}

fn render_lane_status(workflows: &WorkflowStore) -> Vec<String> {
    let state = match workflows.load_lane_state() {
        Ok(state) => state,
        Err(_) => {
            return vec![
                "Lanes:".to_string(),
                format!(
                    "  Active lanes: <unavailable: {}>",
                    LANE_STATE_UNAVAILABLE_MESSAGE
                ),
            ];
        }
    };
    let lanes = state.lanes().values().collect::<Vec<_>>();
    let active = lanes.iter().filter(|lane| lane.is_active()).count();
    let mut lines = vec![
        "Lanes:".to_string(),
        format!("  Active lanes: {active}/{}", lanes.len()),
    ];
    lines.extend(lanes.into_iter().take(3).map(|lane| {
        format!(
            "    {} {} {} {}",
            lane.id,
            lane.role,
            serde_json::to_value(lane.status)
                .ok()
                .and_then(|value| value.as_str().map(ToString::to_string))
                .unwrap_or_else(|| "unknown".to_string()),
            lane.summary
        )
    }));
    lines
}
