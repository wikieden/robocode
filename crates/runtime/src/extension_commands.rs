use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::SessionEngine;
use viden_types::{RuntimeServiceHealthView, RuntimeServiceKind, RuntimeServiceStatus};

impl SessionEngine {
    pub(super) fn handle_extensions_command(&self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str).unwrap_or("list") {
            "list" => Ok(self.render_extension_list()),
            "doctor" => Ok(self.render_extension_doctor()),
            subcommand => Ok(format!(
                "Unknown extensions subcommand `{subcommand}`.\n\n{}",
                self.render_extensions_help()
            )),
        }
    }

    pub(super) fn handle_mcp_command(&self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str).unwrap_or("list") {
            "list" => Ok(render_mcp_list(&self.cwd)),
            "doctor" => Ok(render_mcp_doctor(&self.cwd)),
            subcommand => Ok(format!(
                "Unknown MCP subcommand `{subcommand}`.\n\n{}",
                self.render_mcp_help()
            )),
        }
    }

    pub(super) fn handle_skills_command(&self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str).unwrap_or("list") {
            "list" => Ok(render_skills_list(
                &self.cwd,
                args.iter().any(|arg| arg == "--all"),
            )),
            subcommand => Ok(format!(
                "Unknown skills subcommand `{subcommand}`.\n\n{}",
                self.render_skills_help()
            )),
        }
    }

    pub(super) fn render_extensions_help(&self) -> String {
        [
            "Extension commands:",
            "  /extensions list",
            "  /extensions doctor",
        ]
        .join("\n")
    }

    pub(super) fn render_mcp_help(&self) -> String {
        ["MCP commands:", "  /mcp list", "  /mcp doctor"].join("\n")
    }

    pub(super) fn render_skills_help(&self) -> String {
        ["Skills commands:", "  /skills list [--all]"].join("\n")
    }

    fn render_extension_list(&self) -> String {
        let mcp_configs = mcp_config_candidates(&self.cwd)
            .into_iter()
            .filter(|path| path.exists())
            .count();
        let skills = discover_skills(&self.cwd);
        [
            "Extension surfaces:".to_string(),
            format!(
                "  providers: {} plugin dir(s)",
                self.provider_plugin_dirs.len()
            ),
            "  agents: built-in template/tmux/pty adapters; use `/agent list`".to_string(),
            format!("  mcp: {mcp_configs} config file(s); use `/mcp list`"),
            format!(
                "  skills: {} local skill(s); use `/skills list`",
                skills.len()
            ),
            "  tools: built-in tool registry; MCP-backed tools not wired yet".to_string(),
            "  context: workspace snapshots, LSP cache, future MCP context servers".to_string(),
        ]
        .join("\n")
    }

    fn render_extension_doctor(&self) -> String {
        let mut lines = vec!["Extension diagnostics:".to_string()];
        lines.extend(provider_plugin_diagnostics(&self.provider_plugin_dirs));
        lines.extend(mcp_config_diagnostics(&self.cwd));
        lines.extend(skill_root_diagnostics(&self.cwd));
        lines.push("  agents: use `/agent doctor` for adapter readiness".to_string());
        lines.push(
            "  boundary: extensions remain read-only unless routed through permissions".to_string(),
        );
        lines.join("\n")
    }
}

fn render_mcp_list(cwd: &Path) -> String {
    let configs = mcp_config_candidates(cwd)
        .into_iter()
        .filter(|path| path.exists())
        .collect::<Vec<_>>();
    let mut lines = vec!["MCP visibility:".to_string()];
    if configs.is_empty() {
        lines.push("  No MCP config files found for this workspace.".to_string());
    } else {
        for path in configs {
            lines.push(format!("  {}", path.display()));
            lines.push(format!("    preview: {}", file_preview(&path)));
        }
    }
    lines.push(
        "  Runtime: MCP-backed tools are not wired into the permission path yet.".to_string(),
    );
    lines.join("\n")
}

fn render_mcp_doctor(cwd: &Path) -> String {
    let mut lines = vec!["MCP diagnostics:".to_string()];
    lines.extend(mcp_config_diagnostics(cwd).into_iter().map(|line| {
        line.strip_prefix("  mcp: ")
            .map(|detail| format!("  {detail}"))
            .unwrap_or(line)
    }));
    lines.push(
        "  boundary: MCP tools must enter through tool permissions before mutation.".to_string(),
    );
    lines.join("\n")
}

pub(crate) fn mcp_runtime_service_health(cwd: &Path) -> Vec<RuntimeServiceHealthView> {
    let config_count = mcp_config_candidates(cwd)
        .into_iter()
        .filter(|path| path.exists())
        .count();
    vec![RuntimeServiceHealthView {
        id: "mcp-runtime".to_string(),
        kind: RuntimeServiceKind::Mcp,
        label: "MCP runtime".to_string(),
        // Configuration discovery is visibility only. Until MCP tools enter
        // the shared permission path, Core must not publish a connected fact.
        status: RuntimeServiceStatus::Unavailable,
        detail_key: Some(if config_count == 0 {
            "mcp.config_not_found".to_string()
        } else {
            "mcp.config_visible_runtime_unavailable".to_string()
        }),
    }]
}

fn render_skills_list(cwd: &Path, show_all: bool) -> String {
    let skills = discover_skills(cwd);
    let total = skills.len();
    let visible_limit = if show_all { usize::MAX } else { 30 };
    let mut lines = vec![
        "Skills:".to_string(),
        format!("  total: {total}"),
        "  Use `/skills list --all` to show every discovered skill.".to_string(),
    ];
    if skills.is_empty() {
        lines.push("  No local skills found in project or user skill roots.".to_string());
    } else {
        for skill in skills.iter().take(visible_limit) {
            lines.push(format!(
                "  {}  [{}] {}",
                skill.name,
                skill.scope,
                skill.path.display()
            ));
        }
        if total > visible_limit {
            lines.push(format!("  ... {} more skill(s)", total - visible_limit));
        }
    }
    lines.push(
        "  Skills are task recipes, not tools; lane/tool mutation still needs permission routing."
            .to_string(),
    );
    lines.join("\n")
}

fn mcp_config_candidates(cwd: &Path) -> Vec<PathBuf> {
    let mut paths = vec![cwd.join(".mcp.json"), cwd.join(".cursor").join("mcp.json")];
    if let Some(home) = home_dir() {
        paths.push(home.join(".codex").join("mcp.json"));
    }
    paths
}

fn provider_plugin_diagnostics(paths: &[PathBuf]) -> Vec<String> {
    let mut lines = vec![format!("  provider plugins: {} dir(s)", paths.len())];
    if paths.is_empty() {
        lines.push("    none configured".to_string());
        return lines;
    }
    for path in paths {
        let status = if path.is_dir() { "found" } else { "missing" };
        lines.push(format!("    {status}: {}", path.display()));
    }
    lines
}

fn mcp_config_diagnostics(cwd: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    for path in mcp_config_candidates(cwd) {
        let status = if path.exists() { "found" } else { "missing" };
        let servers = if path.exists() {
            let names = mcp_server_names(&path);
            if names.is_empty() {
                "servers: none detected".to_string()
            } else {
                format!("servers: {}", names.join(", "))
            }
        } else {
            "servers: -".to_string()
        };
        lines.push(format!("  mcp: {status} {} ({servers})", path.display()));
    }
    lines
}

fn skill_root_diagnostics(cwd: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    for (scope, root) in skill_roots(cwd) {
        let count = discover_skills_in_root(scope, &root).len();
        let status = if root.is_dir() { "found" } else { "missing" };
        lines.push(format!(
            "  skills/{scope}: {status} {count} skill(s) {}",
            root.display()
        ));
    }
    lines
}

fn file_preview(path: &Path) -> String {
    let Ok(content) = fs::read_to_string(path) else {
        return "<unreadable>".to_string();
    };
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() > 120 {
        format!("{}...", &compact[..120])
    } else {
        compact
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillEntry {
    name: String,
    scope: &'static str,
    path: PathBuf,
}

fn discover_skills(cwd: &Path) -> Vec<SkillEntry> {
    let mut skills = skill_roots(cwd)
        .into_iter()
        .flat_map(|(scope, root)| discover_skills_in_root(scope, &root))
        .collect::<Vec<_>>();
    skills.sort_by(|left, right| {
        scope_rank(left.scope)
            .cmp(&scope_rank(right.scope))
            .then(left.name.cmp(&right.name))
    });
    skills
}

fn skill_roots(cwd: &Path) -> Vec<(&'static str, PathBuf)> {
    let mut roots = vec![("project", cwd.join(".codex").join("skills"))];
    if let Some(home) = home_dir() {
        roots.push(("user", home.join(".codex").join("skills")));
        roots.push(("legacy", home.join(".agents").join("skills")));
    }
    roots
}

fn scope_rank(scope: &str) -> u8 {
    match scope {
        "project" => 0,
        "user" => 1,
        "legacy" => 2,
        _ => 3,
    }
}

fn discover_skills_in_root(scope: &'static str, root: &Path) -> Vec<SkillEntry> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("SKILL.md").is_file())
        .filter_map(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToString::to_string)?;
            Some(SkillEntry { name, scope, path })
        })
        .collect()
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn mcp_server_names(path: &Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Some(start) = content.find("\"mcpServers\"") else {
        return Vec::new();
    };
    let Some(open) = content[start..].find('{') else {
        return Vec::new();
    };
    let mut depth = 0usize;
    let mut names = Vec::new();
    let chars = content[start + open..].chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        match chars[index] {
            '{' => {
                depth += 1;
                index += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
                index += 1;
            }
            '"' if depth == 1 => {
                let Some((name, next)) = read_json_string(&chars, index) else {
                    break;
                };
                let after = chars[next..]
                    .iter()
                    .position(|ch| !ch.is_whitespace())
                    .map(|offset| next + offset);
                if after.is_some_and(|pos| chars.get(pos) == Some(&':')) {
                    names.push(name);
                }
                index = next;
            }
            _ => index += 1,
        }
    }
    names.sort();
    names.dedup();
    names
}

fn read_json_string(chars: &[char], start: usize) -> Option<(String, usize)> {
    if chars.get(start) != Some(&'"') {
        return None;
    }
    let mut output = String::new();
    let mut index = start + 1;
    while index < chars.len() {
        match chars[index] {
            '"' => return Some((output, index + 1)),
            '\\' => {
                index += 1;
                output.push(*chars.get(index)?);
                index += 1;
            }
            ch => {
                output.push(ch);
                index += 1;
            }
        }
    }
    None
}
