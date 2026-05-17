use super::*;

impl SessionEngine {
    pub(super) fn render_help(&self) -> String {
        [
            "RoboCode commands:",
            "",
            "Runtime:",
            "  /help                Show available commands",
            "  /provider            Show current provider and model",
            "  /model [name]        Show or change the active model label",
            "  /permissions [mode]  Show or change permission mode",
            "  /plan [on|off]       Toggle plan mode",
            "  /status              Show current runtime status",
            "  /config              Show resolved runtime configuration",
            "  /doctor              Check local dependency availability",
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
            "  /memory <subcommand> Manage project/session memory",
            "",
            "Fallback tool syntax:",
            "  tool read_file path=Cargo.toml",
            "  tool grep pattern=fn path=src",
        ]
        .join("\n")
    }

    pub(super) fn render_status(&self) -> String {
        [
            "Runtime status:".to_string(),
            format!("  Session: {}", self.session_id()),
            format!("  CWD: {}", self.cwd.display()),
            format!("  Provider: {}", self.provider.provider_name()),
            format!("  Model: {}", self.provider.model()),
            format!("  Permission mode: {}", self.permissions.mode().cli_name()),
            format!("  Transcript: {}", self.store.transcript_path().display()),
            format!("  Session home: {}", self.store.home_dir().display()),
            format!("  Index: {}", self.store.index_db_path().display()),
        ]
        .join("\n")
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
        let mut lines = vec![
            "Sessions for this project:".to_string(),
            "  Use `/resume latest`, `/resume #<index>`, or `/resume <session-id-prefix>`."
                .to_string(),
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
            lines.push(format!(
                "  {}. {}{}  {}  {}",
                index + 1,
                summary.session_id,
                current,
                format_relative_age(summary.last_updated_at),
                title
            ));
            lines.push(format!(
                "     messages={} tools={} commands={} last={}",
                summary.message_count,
                summary.tool_call_count,
                summary.command_count,
                summary
                    .last_activity_kind
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string())
            ));
            lines.push(format!("     {}", preview));
        }
        lines.join("\n")
    }
}
