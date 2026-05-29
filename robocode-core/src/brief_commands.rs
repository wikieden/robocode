use std::fs;
use std::path::PathBuf;

use crate::SessionEngine;
use crate::presentation::{
    join_lines, render_field, render_section_title, render_subsection_title,
};
use robocode_types::{ApprovalResponse, now_timestamp};

const ACTIVE_BRIEF_PATH: &str = ".robocode/briefs/active.md";
const STEERING_DIR: &str = ".robocode/steering";
const STEERING_FILES: [(&str, &str, &str); 3] = [
    (
        "conventions.md",
        "Project conventions",
        "Coding style, naming, documentation, and review rules that should guide delegated work.",
    ),
    (
        "architecture.md",
        "Project architecture",
        "Runtime boundaries, module responsibilities, and invariants that should not be violated.",
    ),
    (
        "workflows.md",
        "Project workflows",
        "Build, test, release, and daily coding-loop commands that agents should prefer.",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BriefSnapshot {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) goal: String,
    pub(crate) path: PathBuf,
}

impl SessionEngine {
    pub(crate) fn handle_brief_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let Some(subcommand) = args.first().map(String::as_str) else {
            return self.render_active_brief_or_help();
        };
        match subcommand {
            "show" => self.render_active_brief_or_help(),
            "clear" => self.clear_active_brief(approver),
            "steering" => self.handle_steering_command(&args[1..], approver),
            "help" => Ok(render_brief_help()),
            other if other.starts_with('-') => Err("Usage: /brief <task goal>".to_string()),
            _ => {
                let goal = args.join(" ");
                self.create_active_brief(&goal, approver)
            }
        }
    }

    pub(crate) fn active_brief_snapshot(&self) -> Option<BriefSnapshot> {
        let path = self.cwd.join(ACTIVE_BRIEF_PATH);
        read_brief_snapshot(path).ok().flatten()
    }

    pub(crate) fn steering_summaries(&self) -> Vec<(String, String)> {
        STEERING_FILES
            .iter()
            .filter_map(|(file, _, _)| {
                let path = self.cwd.join(STEERING_DIR).join(file);
                let content = fs::read_to_string(path).ok()?;
                let summary = first_meaningful_lines(&content, 4);
                (!summary.trim().is_empty()).then(|| ((*file).to_string(), summary))
            })
            .collect()
    }

    fn create_active_brief<F>(&mut self, goal: &str, approver: &mut F) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let goal = goal.trim();
        if goal.is_empty() {
            return Err("Usage: /brief <task goal>".to_string());
        }
        if let Some(denied) = self.ensure_workflow_permission("brief_create", goal, approver)? {
            return Ok(denied);
        }
        let id = format!("brief_{}", now_timestamp());
        let title = brief_title(goal);
        let path = self.cwd.join(ACTIVE_BRIEF_PATH);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(
            &path,
            render_brief_document(&id, &title, goal, self.session_id()),
        )
        .map_err(|err| err.to_string())?;
        Ok(format!(
            "Active brief created.\n  id: {id}\n  title: {title}\n  path: {}",
            path.display()
        ))
    }

    fn clear_active_brief<F>(&mut self, approver: &mut F) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        if let Some(denied) =
            self.ensure_workflow_permission("brief_clear", ACTIVE_BRIEF_PATH, approver)?
        {
            return Ok(denied);
        }
        let path = self.cwd.join(ACTIVE_BRIEF_PATH);
        if path.exists() {
            fs::remove_file(&path).map_err(|err| err.to_string())?;
            Ok(format!("Cleared active brief at {}", path.display()))
        } else {
            Ok("No active brief to clear.".to_string())
        }
    }

    fn render_active_brief_or_help(&self) -> Result<String, String> {
        if let Some(brief) = self.active_brief_snapshot() {
            let content = fs::read_to_string(&brief.path).map_err(|err| err.to_string())?;
            return Ok(format!(
                "{}\n{}",
                render_brief_summary(&brief),
                compact_markdown(&content, 36)
            ));
        }
        Ok(render_brief_help())
    }

    fn handle_steering_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        match args.first().map(String::as_str) {
            Some("init") => self.init_steering_files(approver),
            Some("show") | None => Ok(self.render_steering_summaries()),
            Some(other) => Ok(format!(
                "Unknown steering subcommand `{other}`.\n\n{}",
                render_brief_help()
            )),
        }
    }

    fn init_steering_files<F>(&mut self, approver: &mut F) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        if let Some(denied) =
            self.ensure_workflow_permission("brief_steering_init", STEERING_DIR, approver)?
        {
            return Ok(denied);
        }
        let dir = self.cwd.join(STEERING_DIR);
        fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let mut rows = Vec::new();
        for (file, title, description) in STEERING_FILES {
            let path = dir.join(file);
            if !path.exists() {
                fs::write(&path, render_steering_template(title, description))
                    .map_err(|err| err.to_string())?;
            }
            rows.push(format!("  - {}", path.display()));
        }
        Ok(format!(
            "Steering files ready. Edit them when you want project facts to guide agents:\n{}",
            rows.join("\n")
        ))
    }

    fn render_steering_summaries(&self) -> String {
        let summaries = self.steering_summaries();
        if summaries.is_empty() {
            return format!(
                "No steering files found. Run `/brief steering init` to create templates under `{STEERING_DIR}`."
            );
        }
        let mut lines = vec![
            render_section_title("Project steering")
                .trim_end()
                .to_string(),
        ];
        for (file, summary) in summaries {
            lines.push(render_subsection_title(&file));
            lines.push(summary);
        }
        join_lines(&lines)
    }
}

fn read_brief_snapshot(path: PathBuf) -> Result<Option<BriefSnapshot>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let id = front_matter_field(&content, "id").unwrap_or_else(|| "brief_unknown".to_string());
    let title =
        front_matter_field(&content, "title").unwrap_or_else(|| "Untitled brief".to_string());
    let goal = markdown_section(&content, "Goal").unwrap_or_else(|| title.clone());
    Ok(Some(BriefSnapshot {
        id,
        title,
        goal,
        path,
    }))
}

fn render_brief_summary(brief: &BriefSnapshot) -> String {
    join_lines(&[
        render_section_title("Active brief").trim_end().to_string(),
        render_field("id", &brief.id),
        render_field("title", &brief.title),
        render_field("path", brief.path.display().to_string()),
        render_field("goal", &brief.goal),
        String::new(),
    ])
}

fn render_brief_document(id: &str, title: &str, goal: &str, session_id: &str) -> String {
    format!(
        "---\nid: {id}\ntitle: {title}\ncreated_at: {}\norigin_session: {session_id}\n---\n\n# {title}\n\n## Goal\n{goal}\n\n## Constraints\n- Keep changes scoped to this task.\n- Preserve unrelated user changes.\n- Ask only for destructive or credential-gated actions.\n\n## Files\n- <discover during implementation>\n\n## Checks\n- Run focused tests for changed behavior.\n- Capture real TUI or smoke evidence when UI behavior changes.\n\n## Risks\n- Unknown until implementation discovers affected modules.\n",
        now_timestamp()
    )
}

fn render_steering_template(title: &str, description: &str) -> String {
    format!(
        "# {title}\n\n{description}\n\n## Guidance\n- Replace this template with real project guidance before relying on it for delegated lanes.\n"
    )
}

fn render_brief_help() -> String {
    [
        "Brief commands:",
        "  /brief <task goal>          Create or replace the active task brief",
        "  /spec <task goal>           Alias for /brief",
        "  /brief show                 Show the active brief",
        "  /brief clear                Clear the active brief",
        "  /brief steering init        Create steering templates",
        "  /brief steering show        Show steering summaries",
    ]
    .join("\n")
}

fn brief_title(goal: &str) -> String {
    let title = goal
        .lines()
        .next()
        .unwrap_or(goal)
        .split_whitespace()
        .take(12)
        .collect::<Vec<_>>()
        .join(" ");
    if title.chars().count() > 80 {
        let mut truncated = title.chars().take(77).collect::<String>();
        truncated.push_str("...");
        truncated
    } else {
        title
    }
}

fn front_matter_field(content: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    content
        .lines()
        .skip_while(|line| line.trim() == "---")
        .take_while(|line| line.trim() != "---")
        .find_map(|line| line.trim().strip_prefix(&prefix).map(str::trim))
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn markdown_section(content: &str, title: &str) -> Option<String> {
    let heading = format!("## {title}");
    let mut lines = Vec::new();
    let mut in_section = false;
    for line in content.lines() {
        if line.trim() == heading {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if in_section {
            lines.push(line);
        }
    }
    let section = lines.join("\n").trim().to_string();
    (!section.is_empty()).then_some(section)
}

fn compact_markdown(content: &str, max_lines: usize) -> String {
    let lines = content.lines().collect::<Vec<_>>();
    if lines.len() <= max_lines {
        return content.to_string();
    }
    let tail = lines[lines.len().saturating_sub(max_lines)..].join("\n");
    format!(
        "[summary] {} line(s) compacted; tail follows\n{tail}",
        lines.len()
    )
}

fn first_meaningful_lines(content: &str, max_lines: usize) -> String {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("---"))
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n")
}
