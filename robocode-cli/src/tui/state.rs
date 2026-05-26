use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

use robocode_core::{EngineEvent, ProviderTelemetry};
use robocode_model::ProviderDescriptor;
use robocode_types::{MemoryEntry, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiEntry {
    pub(super) label: String,
    pub(super) body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiState {
    pub(super) session_id: String,
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) provider_catalog: Vec<ProviderOption>,
    pub(super) provider_status: ProviderStatus,
    pub(super) theme_name: String,
    pub(super) input: String,
    pub(super) command_selection: usize,
    pub(super) command_palette_hidden_for: Option<String>,
    pub(super) approval_focus: usize,
    pub(super) approval_apply_all: bool,
    pub(super) entries: Vec<TuiEntry>,
    pub(super) workspace: WorkspaceSnapshot,
    pub(super) tasks: Vec<TaskRecord>,
    pub(super) memory: Vec<MemoryEntry>,
    pub(super) screens: Vec<CompanionScreen>,
    pub(super) lanes: Vec<TerminalLane>,
    pub(super) lane_store: Option<PathBuf>,
    pub(super) focused_lane: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderOption {
    pub(super) provider_id: String,
    pub(super) display_name: String,
    pub(super) default_model: Option<String>,
}

impl ProviderOption {
    pub(super) fn from_descriptor(descriptor: &ProviderDescriptor) -> Self {
        Self {
            provider_id: descriptor.provider_id.clone(),
            display_name: descriptor.display_name.clone(),
            default_model: descriptor.default_model.clone(),
        }
    }

    pub(super) fn fixture() -> Vec<Self> {
        vec![
            Self {
                provider_id: "anthropic".to_string(),
                display_name: "Anthropic".to_string(),
                default_model: Some("claude-sonnet-4-6".to_string()),
            },
            Self {
                provider_id: "deepseek".to_string(),
                display_name: "DeepSeek".to_string(),
                default_model: Some("deepseek-v4-flash".to_string()),
            },
            Self {
                provider_id: "fallback".to_string(),
                display_name: "Fallback".to_string(),
                default_model: Some("fallback-local".to_string()),
            },
            Self {
                provider_id: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                default_model: Some("gpt-5.2".to_string()),
            },
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompanionScreen {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) status: String,
    pub(super) pid: Option<u32>,
    pub(super) summary: String,
}

impl CompanionScreen {
    fn to_tsv(&self) -> String {
        [
            escape_tsv(&self.id),
            escape_tsv(&self.title),
            escape_tsv(&self.status),
            self.pid.map(|pid| pid.to_string()).unwrap_or_default(),
            escape_tsv(&self.summary),
        ]
        .join("\t")
    }

    fn from_tsv(value: &str) -> Option<Self> {
        let fields = value.split('\t').map(unescape_tsv).collect::<Vec<_>>();
        if fields.len() != 5 {
            return None;
        }
        let pid = fields[3].parse::<u32>().ok();
        Some(Self {
            id: fields[0].clone(),
            title: fields[1].clone(),
            status: fields[2].clone(),
            pid,
            summary: fields[4].clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TerminalLane {
    pub(super) id: String,
    pub(super) tool: String,
    pub(super) title: String,
    pub(super) status: String,
    pub(super) target: String,
    pub(super) progress: u8,
    pub(super) summary: String,
    pub(super) worktree: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LaneRuntimeEvidence {
    pub(super) log_path: PathBuf,
    pub(super) done_path: PathBuf,
    pub(super) envelope_path: PathBuf,
    pub(super) exit_code: Option<String>,
    pub(super) log_tail: Vec<String>,
    pub(super) envelope_preview: Vec<String>,
}

impl TerminalLane {
    pub(super) fn from_command(index: usize, command: &str) -> Option<Self> {
        let mut parts = command.split_whitespace();
        let slash = parts.next()?;
        if slash != "/lane" {
            return None;
        }
        let tool = parts.next()?;
        let (tool, title) = if tool == "ask" {
            let tool = parts.next()?;
            let title = parts.collect::<Vec<_>>().join(" ");
            (tool, title)
        } else {
            (tool, parts.collect::<Vec<_>>().join(" "))
        };
        if title.is_empty() {
            return None;
        }
        let status = match tool {
            "codex" | "claude" | "run" => "queued",
            _ => "manual",
        };
        Some(Self {
            id: format!("L{index}"),
            tool: tool.to_string(),
            title,
            status: status.to_string(),
            target: "main".to_string(),
            progress: 0,
            summary: "waiting for terminal adapter".to_string(),
            worktree: None,
        })
    }

    pub(super) fn preview_lanes() -> Vec<Self> {
        vec![
            Self {
                id: "L1".to_string(),
                tool: "codex".to_string(),
                title: "test fixes".to_string(),
                status: "running".to_string(),
                target: "main".to_string(),
                progress: 64,
                summary: "patched failing tests; rerunning cargo".to_string(),
                worktree: None,
            },
            Self {
                id: "L2".to_string(),
                tool: "claude".to_string(),
                title: "review diff".to_string(),
                status: "attached".to_string(),
                target: "tmux robocode-c4f2b7e-l2".to_string(),
                progress: 32,
                summary: "tmux session ready; reviewing config architecture".to_string(),
                worktree: None,
            },
            Self {
                id: "L3".to_string(),
                tool: "shell".to_string(),
                title: "cargo test".to_string(),
                status: "idle".to_string(),
                target: "ops".to_string(),
                progress: 100,
                summary: "last run green; no failures cached".to_string(),
                worktree: None,
            },
        ]
    }

    fn to_tsv(&self) -> String {
        [
            escape_tsv(&self.id),
            escape_tsv(&self.tool),
            escape_tsv(&self.title),
            escape_tsv(&self.status),
            escape_tsv(&self.target),
            self.progress.to_string(),
            escape_tsv(&clean_display_fragment(&self.summary, 120)),
            escape_tsv(
                self.worktree
                    .as_ref()
                    .map(|path| path.to_string_lossy())
                    .as_deref()
                    .unwrap_or_default(),
            ),
        ]
        .join("\t")
    }

    fn from_tsv(value: &str) -> Option<Self> {
        let fields = value.split('\t').map(unescape_tsv).collect::<Vec<_>>();
        if fields.len() != 5 && fields.len() != 7 && fields.len() != 8 {
            return None;
        }
        let progress = fields
            .get(5)
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(0)
            .min(100);
        let summary = fields
            .get(6)
            .map(|value| clean_display_fragment(value, 120))
            .unwrap_or_else(|| "restored from lane store".to_string());
        let worktree = fields
            .get(7)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Some(Self {
            id: fields[0].clone(),
            tool: fields[1].clone(),
            title: fields[2].clone(),
            status: fields[3].clone(),
            target: fields[4].clone(),
            progress,
            summary,
            worktree,
        })
    }
}

pub(super) fn lane_store_path(root: &Path) -> PathBuf {
    root.join(".robocode").join("lanes.tsv")
}

pub(super) fn screen_store_path(root: &Path) -> PathBuf {
    root.join(".robocode").join("screens.tsv")
}

pub(super) fn diagnostics_store_path(root: &Path) -> PathBuf {
    root.join(".robocode").join("diagnostics.txt")
}

pub(super) fn save_diagnostics(root: &Path, diagnostics: &[String]) -> Result<(), String> {
    let path = diagnostics_store_path(root);
    if diagnostics.is_empty() {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.to_string()),
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, format!("{}\n", diagnostics.join("\n"))).map_err(|err| err.to_string())
}

pub(super) fn load_lanes(path: &Path) -> Vec<TerminalLane> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content.lines().filter_map(TerminalLane::from_tsv).collect()
}

pub(super) fn save_lanes(path: &Path, lanes: &[TerminalLane]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let content = lanes
        .iter()
        .map(TerminalLane::to_tsv)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, format!("{content}\n")).map_err(|err| err.to_string())
}

pub(super) fn load_screens(path: &Path) -> Vec<CompanionScreen> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(CompanionScreen::from_tsv)
        .collect()
}

pub(super) fn save_screens(path: &Path, screens: &[CompanionScreen]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let content = screens
        .iter()
        .map(CompanionScreen::to_tsv)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, format!("{content}\n")).map_err(|err| err.to_string())
}

pub(super) fn refresh_lane_runtime(path: &Path, lanes: &mut [TerminalLane]) {
    for lane in lanes {
        // Operator decisions are durable states; runtime artifacts must not downgrade them.
        if matches!(
            lane.status.as_str(),
            "accepted"
                | "revise"
                | "discarded"
                | "applied"
                | "apply_conflict"
                | "archived"
                | "detached"
                | "stopped"
        ) {
            continue;
        }
        let Some(evidence) = lane_runtime_evidence(path, &lane.id) else {
            continue;
        };
        if let Some(summary) = evidence.log_tail.last().cloned() {
            lane.summary = summary;
            lane.progress = lane.progress.max(35).min(95);
        }
        let Some(exit_code) = evidence.exit_code else {
            continue;
        };
        lane.progress = 100;
        if exit_code == "0" {
            lane.status = "completed".to_string();
            if lane.summary.is_empty() {
                lane.summary = "completed successfully".to_string();
            }
        } else {
            lane.status = "failed".to_string();
            lane.summary = if lane.summary.is_empty() {
                format!("exited with status {exit_code}")
            } else {
                format!("{} (exit {exit_code})", lane.summary)
            };
        }
    }
}

pub(super) fn lane_runtime_evidence(path: &Path, lane_id: &str) -> Option<LaneRuntimeEvidence> {
    let artifact_dir = path.parent()?.join("lanes");
    let log_path = artifact_dir.join(format!("{lane_id}.log"));
    let done_path = artifact_dir.join(format!("{lane_id}.done"));
    let envelope_path = artifact_dir.join(format!("{lane_id}.envelope.md"));
    let log_tail = log_tail(&log_path, 5);
    let envelope_preview = file_head(&envelope_path, 12);
    let exit_code = fs::read_to_string(&done_path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Some(LaneRuntimeEvidence {
        log_path,
        done_path,
        envelope_path,
        exit_code,
        log_tail,
        envelope_preview,
    })
}

fn log_tail(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines = content
        .lines()
        .map(|line| clean_display_fragment(line, 120))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .filter(|line| !is_terminal_prompt_noise(line))
        .collect::<Vec<_>>();
    let keep_from = lines.len().saturating_sub(max_lines);
    lines.drain(0..keep_from);
    lines
}

fn clean_display_fragment(value: &str, max_chars: usize) -> String {
    sanitize_terminal_controls(value)
        .chars()
        .take(max_chars)
        .collect()
}

fn sanitize_terminal_controls(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            skip_escape_sequence(&mut chars);
            continue;
        }
        match ch {
            // Terminal logs can include carriage-return redraws from shells and
            // progress UIs. Keep only the final visible segment for summaries.
            '\r' => output.clear(),
            '\u{8}' => {
                output.pop();
            }
            '\t' => output.push(' '),
            _ if ch.is_control() => {}
            _ => output.push(ch),
        }
    }
    output
}

fn is_terminal_prompt_noise(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed == "%"
        || trimmed == "$"
        || trimmed == "#"
        || trimmed.starts_with("➜ ")
        || trimmed.starts_with("➜\u{a0}")
}

fn skip_escape_sequence<I>(chars: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    match chars.peek().copied() {
        Some('[') => {
            chars.next();
            for ch in chars.by_ref() {
                if ('@'..='~').contains(&ch) {
                    break;
                }
            }
        }
        Some(']') => {
            chars.next();
            let mut saw_escape = false;
            for ch in chars.by_ref() {
                if ch == '\u{7}' || (saw_escape && ch == '\\') {
                    break;
                }
                saw_escape = ch == '\u{1b}';
            }
        }
        Some('P' | '^' | '_' | 'X') => {
            chars.next();
            let mut saw_escape = false;
            for ch in chars.by_ref() {
                if saw_escape && ch == '\\' {
                    break;
                }
                saw_escape = ch == '\u{1b}';
            }
        }
        Some('(' | ')' | '*' | '+' | '-' | '.' | '/') => {
            chars.next();
            let _ = chars.next();
        }
        Some(_) => {
            let _ = chars.next();
        }
        None => {}
    }
}

fn file_head(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .take(max_lines)
        .map(|line| line.chars().take(120).collect::<String>())
        .collect()
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape_tsv(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }
        match chars.next() {
            Some('t') => output.push('\t'),
            Some('n') => output.push('\n'),
            Some('\\') => output.push('\\'),
            Some(other) => {
                output.push('\\');
                output.push(other);
            }
            None => output.push('\\'),
        }
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderStatus {
    pub(super) connection: String,
    pub(super) telemetry: String,
    pub(super) context_window: String,
    pub(super) request_count: u64,
    pub(super) success_count: u64,
    pub(super) failure_count: u64,
    pub(super) last_latency_ms: Option<u128>,
    pub(super) average_latency_ms: Option<u128>,
    pub(super) last_event_count: usize,
    pub(super) last_error: Option<String>,
    pub(super) last_input_tokens: Option<u64>,
    pub(super) last_output_tokens: Option<u64>,
    pub(super) last_total_tokens: Option<u64>,
    pub(super) total_tokens: u64,
    pub(super) last_tokens_per_second: Option<u64>,
    pub(super) last_cost_micro_usd: Option<u64>,
    pub(super) total_cost_micro_usd: Option<u64>,
}

impl ProviderStatus {
    pub(super) fn configured() -> Self {
        Self::from_telemetry(&ProviderTelemetry::default())
    }

    pub(super) fn from_telemetry(telemetry: &ProviderTelemetry) -> Self {
        let connection = if telemetry.last_error.is_some() {
            "Error"
        } else if telemetry.request_count > 0 {
            "Healthy"
        } else {
            "Configured"
        };
        let telemetry_label = if telemetry.request_count == 0 {
            "not sampled".to_string()
        } else {
            format!(
                "{} req / {} ok / {} err",
                telemetry.request_count, telemetry.success_count, telemetry.failure_count
            )
        };
        Self {
            connection: connection.to_string(),
            telemetry: telemetry_label,
            context_window: "128k".to_string(),
            request_count: telemetry.request_count,
            success_count: telemetry.success_count,
            failure_count: telemetry.failure_count,
            last_latency_ms: telemetry.last_latency_ms,
            average_latency_ms: telemetry.average_latency_ms,
            last_event_count: telemetry.last_event_count,
            last_error: telemetry.last_error.clone(),
            last_input_tokens: telemetry.last_input_tokens,
            last_output_tokens: telemetry.last_output_tokens,
            last_total_tokens: telemetry.last_total_tokens,
            total_tokens: telemetry.total_tokens,
            last_tokens_per_second: telemetry.last_tokens_per_second,
            last_cost_micro_usd: telemetry.last_cost_micro_usd,
            total_cost_micro_usd: telemetry.total_cost_micro_usd,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkspaceSnapshot {
    pub(super) root: PathBuf,
    pub(super) display_root: String,
    pub(super) git_branch: String,
    pub(super) git_branches: Vec<String>,
    pub(super) git_remotes: Vec<String>,
    pub(super) git_remote_branches: Vec<GitRemoteBranchEntry>,
    pub(super) git_stashes: Vec<GitStashEntry>,
    pub(super) git_worktrees: Vec<GitWorktreeEntry>,
    pub(super) file_count: usize,
    pub(super) line_count: usize,
    pub(super) recent_files: Vec<RecentFile>,
    pub(super) top_files: Vec<String>,
    pub(super) workspace_paths: Vec<String>,
    pub(super) diagnostics: Vec<String>,
    pub(super) agent_jobs: Vec<AgentJob>,
    pub(super) primary_language: String,
    pub(super) rust_edition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentJob {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) status: String,
    pub(super) task: String,
    pub(super) pid: Option<u32>,
    pub(super) log_path: Option<PathBuf>,
    pub(super) result_path: Option<PathBuf>,
    pub(super) evidence: Vec<String>,
    pub(super) updated_at: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RecentFile {
    pub(super) path: String,
    pub(super) modified: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitStashEntry {
    pub(super) reference: String,
    pub(super) summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitRemoteBranchEntry {
    pub(super) remote: String,
    pub(super) branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitWorktreeEntry {
    pub(super) path: String,
    pub(super) branch: Option<String>,
}

impl WorkspaceSnapshot {
    pub(super) fn load_current() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::load(root)
    }

    pub(super) fn load(root: PathBuf) -> Self {
        let git_branch = git_branch(&root).unwrap_or_else(|| "main".to_string());
        let git_branches = git_branches(&root).unwrap_or_else(|| vec![git_branch.clone()]);
        let git_remotes = git_remotes(&root).unwrap_or_default();
        let git_remote_branches = git_remote_branches(&root).unwrap_or_default();
        let git_stashes = git_stashes(&root).unwrap_or_default();
        let git_worktrees = git_worktrees(&root).unwrap_or_default();
        let display_root = display_path(&root);
        let mut files = Vec::new();
        collect_files(&root, &root, &mut files, 0);
        files.sort_by(|left, right| right.modified.cmp(&left.modified));
        let file_count = files.len();
        let line_count = files.iter().map(|file| file.lines).sum();
        let primary_language = primary_language(&files);
        let rust_edition = rust_edition(&root);
        let recent_files = files
            .iter()
            .take(3)
            .map(|file| RecentFile {
                path: file.path.clone(),
                modified: file.modified,
            })
            .collect::<Vec<_>>();
        let top_files = files
            .iter()
            .filter(|file| visible_top_file(&file.path))
            .take(4)
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let workspace_paths = files
            .iter()
            .take(96)
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let diagnostics = load_diagnostics(&root);
        let agent_jobs = load_agent_jobs(&root);

        Self {
            root,
            display_root,
            git_branch,
            git_branches,
            git_remotes,
            git_remote_branches,
            git_stashes,
            git_worktrees,
            file_count,
            line_count,
            recent_files,
            top_files,
            workspace_paths,
            diagnostics,
            agent_jobs,
            primary_language,
            rust_edition,
        }
    }

    pub(super) fn refresh_agent_jobs(&mut self) {
        self.agent_jobs = load_agent_jobs(&self.root);
    }

    pub(super) fn fixture() -> Self {
        Self {
            root: PathBuf::from("/tmp/robocode"),
            display_root: "~/projects/robocode".to_string(),
            git_branch: "main".to_string(),
            git_branches: vec![
                "main".to_string(),
                "codex/tui-cockpit".to_string(),
                "release/v0.1.4".to_string(),
            ],
            git_remotes: vec!["origin".to_string(), "upstream".to_string()],
            git_remote_branches: vec![
                GitRemoteBranchEntry {
                    remote: "origin".to_string(),
                    branch: "main".to_string(),
                },
                GitRemoteBranchEntry {
                    remote: "origin".to_string(),
                    branch: "release/v0.1.4".to_string(),
                },
                GitRemoteBranchEntry {
                    remote: "upstream".to_string(),
                    branch: "main".to_string(),
                },
            ],
            git_stashes: vec![
                GitStashEntry {
                    reference: "stash@{0}".to_string(),
                    summary: "WIP on main: tune cockpit palette".to_string(),
                },
                GitStashEntry {
                    reference: "stash@{1}".to_string(),
                    summary: "On codex/tui-cockpit: checkpoint preview assets".to_string(),
                },
            ],
            git_worktrees: vec![
                GitWorktreeEntry {
                    path: "/tmp/robocode".to_string(),
                    branch: Some("main".to_string()),
                },
                GitWorktreeEntry {
                    path: "/tmp/robocode/.worktrees/codex-tui-cockpit".to_string(),
                    branch: Some("codex/tui-cockpit".to_string()),
                },
            ],
            file_count: 128,
            line_count: 24_531,
            recent_files: vec![
                RecentFile::fixture("src/config.rs", 0),
                RecentFile::fixture("tests/config_tests.rs", 60),
                RecentFile::fixture("src/lib.rs", 120),
                RecentFile::fixture("src/main.rs", 180),
                RecentFile::fixture("Cargo.toml", 240),
            ],
            top_files: vec![
                "src/".to_string(),
                "tests/".to_string(),
                "Cargo.toml".to_string(),
                "README.md".to_string(),
            ],
            workspace_paths: vec![
                "src/config.rs".to_string(),
                "src/lib.rs".to_string(),
                "src/main.rs".to_string(),
                "tests/config_tests.rs".to_string(),
                "Cargo.toml".to_string(),
                "README.md".to_string(),
            ],
            diagnostics: Vec::new(),
            agent_jobs: Vec::new(),
            primary_language: "Rust".to_string(),
            rust_edition: Some("2024".to_string()),
        }
    }
}

impl RecentFile {
    fn fixture(path: &str, offset_seconds: u64) -> Self {
        Self {
            path: path.to_string(),
            modified: SystemTime::now()
                .checked_sub(Duration::from_secs(offset_seconds))
                .unwrap_or_else(SystemTime::now),
        }
    }
}

pub(super) fn entry_from_event(event: EngineEvent) -> TuiEntry {
    match event {
        EngineEvent::System(text) => TuiEntry {
            label: "system".to_string(),
            body: text,
        },
        EngineEvent::Assistant(text) => TuiEntry {
            label: "assistant".to_string(),
            body: text,
        },
        EngineEvent::ToolCall(text) => TuiEntry {
            label: "tool-call".to_string(),
            body: text,
        },
        EngineEvent::ToolResult(text) => TuiEntry {
            label: "tool-result".to_string(),
            body: text,
        },
        EngineEvent::Command(text) => TuiEntry {
            label: "command".to_string(),
            body: text,
        },
    }
}

pub(super) fn latest_lsp_diagnostics(entries: &[TuiEntry]) -> Option<Vec<String>> {
    entries
        .iter()
        .rev()
        .find_map(|entry| parse_lsp_diagnostics(&entry.body))
}

fn parse_lsp_diagnostics(body: &str) -> Option<Vec<String>> {
    let mut lines = body.lines().skip_while(|line| {
        !line
            .trim_end_matches(':')
            .trim()
            .eq_ignore_ascii_case("LSP diagnostics")
    });
    lines.next()?;

    let mut current_path = None::<String>;
    let mut diagnostics = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "<none>" {
            return Some(Vec::new());
        }
        if !line.starts_with(' ') && trimmed.ends_with(':') {
            current_path = Some(trimmed.trim_end_matches(':').to_string());
            continue;
        }
        if line.starts_with("  ") {
            let rendered = current_path
                .as_ref()
                .map(|path| format!("{path}:{trimmed}"))
                .unwrap_or_else(|| trimmed.to_string());
            diagnostics.push(rendered);
        }
    }
    (!diagnostics.is_empty()).then_some(diagnostics)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSnapshot {
    path: String,
    lines: usize,
    modified: SystemTime,
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<FileSnapshot>, depth: usize) {
    if depth > 4 || files.len() > 512 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if should_skip(&name) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            collect_files(root, &path, files, depth + 1);
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let lines = count_lines(&path, metadata.len());
            files.push(FileSnapshot {
                path: relative,
                lines,
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
}

fn should_skip(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".ref" | ".worktrees" | ".omx" | ".codegraph" | "target" | "node_modules"
    )
}

fn visible_top_file(path: &str) -> bool {
    !path.starts_with('.') && path.split('/').count() <= 2
}

fn primary_language(files: &[FileSnapshot]) -> String {
    let rust_files = files
        .iter()
        .filter(|file| file.path.ends_with(".rs"))
        .count();
    if rust_files > 0 {
        "Rust".to_string()
    } else {
        "mixed".to_string()
    }
}

fn rust_edition(root: &Path) -> Option<String> {
    let content = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    content
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("edition = "))
        .map(|value| value.trim_matches('"').to_string())
}

fn count_lines(path: &Path, size: u64) -> usize {
    if size > 256 * 1024 || !looks_text(path) {
        return 0;
    }
    fs::read_to_string(path)
        .map(|content| content.lines().count())
        .unwrap_or(0)
}

fn looks_text(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        extension,
        "rs" | "toml" | "md" | "txt" | "json" | "yaml" | "yml" | "sh" | "lock"
    )
}

fn git_branch(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!branch.is_empty()).then_some(branch)
}

fn git_branches(root: &Path) -> Option<Vec<String>> {
    let output = Command::new("git")
        .arg("branch")
        .arg("--format=%(refname:short)")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branches = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    (!branches.is_empty()).then_some(branches)
}

fn git_remotes(root: &Path) -> Option<Vec<String>> {
    let output = Command::new("git")
        .arg("remote")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let remotes = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    (!remotes.is_empty()).then_some(remotes)
}

fn git_remote_branches(root: &Path) -> Option<Vec<GitRemoteBranchEntry>> {
    let output = Command::new("git")
        .arg("branch")
        .arg("-r")
        .arg("--format=%(refname:short)")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branches = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            // `git branch -r` includes symbolic refs such as
            // `origin/HEAD -> origin/main`; suggestions should target real
            // remote branch names only.
            if line.is_empty() || line.contains(" -> ") {
                return None;
            }
            let (remote, branch) = line.split_once('/')?;
            Some(GitRemoteBranchEntry {
                remote: remote.to_string(),
                branch: branch.to_string(),
            })
        })
        .collect::<Vec<_>>();
    (!branches.is_empty()).then_some(branches)
}

fn git_stashes(root: &Path) -> Option<Vec<GitStashEntry>> {
    let output = Command::new("git")
        .arg("stash")
        .arg("list")
        .arg("--format=%gd%x09%gs")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stashes = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (reference, summary) = line.split_once('\t')?;
            let reference = reference.trim();
            if reference.is_empty() {
                return None;
            }
            Some(GitStashEntry {
                reference: reference.to_string(),
                summary: summary.trim().to_string(),
            })
        })
        .collect::<Vec<_>>();
    (!stashes.is_empty()).then_some(stashes)
}

fn git_worktrees(root: &Path) -> Option<Vec<GitWorktreeEntry>> {
    let output = Command::new("git")
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut entries = Vec::new();
    let mut current_path = None::<String>;
    let mut current_branch = None::<String>;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.is_empty() {
            if let Some(path) = current_path.take() {
                entries.push(GitWorktreeEntry {
                    path,
                    branch: current_branch.take(),
                });
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(previous_path) = current_path.replace(path.to_string()) {
                entries.push(GitWorktreeEntry {
                    path: previous_path,
                    branch: current_branch.take(),
                });
            }
        } else if let Some(branch) = line.strip_prefix("branch ") {
            current_branch = Some(branch.trim_start_matches("refs/heads/").to_string());
        }
    }
    if let Some(path) = current_path {
        entries.push(GitWorktreeEntry {
            path,
            branch: current_branch,
        });
    }
    (!entries.is_empty()).then_some(entries)
}

fn display_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    if let Some(home) = std::env::var_os("HOME") {
        let home = home.to_string_lossy();
        if path.starts_with(home.as_ref()) {
            return path.replacen(home.as_ref(), "~", 1);
        }
    }
    path.to_string()
}

fn load_diagnostics(root: &Path) -> Vec<String> {
    fs::read_to_string(diagnostics_store_path(root))
        .map(|content| {
            content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn load_agent_jobs(root: &Path) -> Vec<AgentJob> {
    let path = root
        .join(".robocode")
        .join("agents")
        .join("codex-jobs.jsonl");
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut jobs = Vec::<AgentJob>::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let Some(job) = parse_agent_job(line) else {
            continue;
        };
        if let Some(existing) = jobs.iter_mut().find(|existing| existing.id == job.id) {
            *existing = job;
        } else {
            jobs.push(job);
        }
    }
    jobs.sort_by_key(|job| job.updated_at);
    jobs
}

fn parse_agent_job(line: &str) -> Option<AgentJob> {
    let log_path = json_string_field(line, "log").map(PathBuf::from);
    let result_path = json_string_field(line, "result").map(PathBuf::from);
    let evidence = agent_job_evidence(log_path.as_deref(), result_path.as_deref());
    Some(AgentJob {
        id: json_string_field(line, "id")?,
        kind: json_string_field(line, "kind")?,
        status: json_string_field(line, "status")?,
        task: json_string_field(line, "task").unwrap_or_default(),
        pid: json_number_field(line, "pid").and_then(|value| value.parse().ok()),
        log_path,
        result_path,
        evidence,
        updated_at: json_number_field(line, "ts")?.parse().ok()?,
    })
}

fn agent_job_evidence(log_path: Option<&Path>, result_path: Option<&Path>) -> Vec<String> {
    let mut evidence = Vec::new();
    if let Some(result_path) = result_path {
        for line in file_head(result_path, 16) {
            if let Some((key, value)) = line.split_once(':') {
                let value = value.trim();
                if value.is_empty() || value == "unknown" || value == "none" {
                    continue;
                }
                match key.trim() {
                    "thread" => evidence.push(format!("thread {value}")),
                    "turn" => evidence.push(format!("turn {value}")),
                    "status" => evidence.push(format!("turn status {value}")),
                    "approvals" => evidence.push(format!("approvals {value}")),
                    _ => {}
                }
            }
        }
    }
    if evidence.len() < 4 {
        if let Some(log_path) = log_path {
            let log = file_head(log_path, 80).join("\n");
            for (needle, label) in [
                ("thread/started", "thread started"),
                ("turn/started", "turn started"),
                ("turn/completed", "turn completed"),
                ("requestApproval", "approval request captured"),
            ] {
                if log.contains(needle) && !evidence.iter().any(|item| item == label) {
                    evidence.push(label.to_string());
                }
                if evidence.len() >= 4 {
                    break;
                }
            }
        }
    }
    evidence.truncate(4);
    evidence
}

fn json_string_field(value: &str, field: &str) -> Option<String> {
    let marker = format!(r#""{field}":"#);
    let start = value.find(&marker)? + marker.len();
    let rest = value[start..].trim_start().strip_prefix('"')?;
    let mut output = String::new();
    let mut chars = rest.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(output),
            '\\' => match chars.next() {
                Some('n') => output.push('\n'),
                Some('r') => output.push('\r'),
                Some('t') => output.push('\t'),
                Some('"') => output.push('"'),
                Some('\\') => output.push('\\'),
                Some(other) => output.push(other),
                None => return None,
            },
            other => output.push(other),
        }
    }
    None
}

fn json_number_field(value: &str, field: &str) -> Option<String> {
    let marker = format!(r#""{field}":"#);
    let start = value.find(&marker)? + marker.len();
    let number = value[start..]
        .chars()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!number.is_empty()).then_some(number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use robocode_core::EngineEvent;

    #[test]
    fn entry_from_event_preserves_command_output() {
        let entry = entry_from_event(EngineEvent::Command("Provider registry:".to_string()));

        assert_eq!(entry.label, "command");
        assert_eq!(entry.body, "Provider registry:");
    }

    #[test]
    fn latest_lsp_diagnostics_extracts_real_rendered_diagnostics() {
        let entries = vec![
            TuiEntry {
                label: "system".to_string(),
                body: "older".to_string(),
            },
            TuiEntry {
                label: "command".to_string(),
                body: "LSP diagnostics:\nsrc/lib.rs:\n  7:2 warning [rust-analyzer/E0308] mismatched types\n".to_string(),
            },
        ];

        let diagnostics = latest_lsp_diagnostics(&entries).expect("diagnostics");

        assert_eq!(
            diagnostics,
            vec!["src/lib.rs:7:2 warning [rust-analyzer/E0308] mismatched types"]
        );
    }

    #[test]
    fn latest_lsp_diagnostics_clears_cache_on_empty_lsp_result() {
        let entries = vec![TuiEntry {
            label: "command".to_string(),
            body: "LSP diagnostics:\n  <none>".to_string(),
        }];

        let diagnostics = latest_lsp_diagnostics(&entries).expect("empty diagnostics");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn workspace_snapshot_loads_persisted_diagnostics_cache() {
        let root = temp_state_root();
        save_diagnostics(
            &root,
            &["src/main.rs:1:2 error [fake/E1] broken value".to_string()],
        )
        .expect("save diagnostics");

        let workspace = WorkspaceSnapshot::load(root);

        assert_eq!(
            workspace.diagnostics,
            vec!["src/main.rs:1:2 error [fake/E1] broken value"]
        );
    }

    #[test]
    fn save_empty_diagnostics_removes_persisted_cache() {
        let root = temp_state_root();
        save_diagnostics(
            &root,
            &["src/main.rs:1:2 error [fake/E1] broken value".to_string()],
        )
        .expect("save diagnostics");
        save_diagnostics(&root, &[]).expect("clear diagnostics");

        let workspace = WorkspaceSnapshot::load(root);

        assert!(workspace.diagnostics.is_empty());
    }

    #[test]
    fn terminal_lane_tsv_loads_legacy_five_field_rows() {
        let lane = TerminalLane::from_tsv("L1\tcodex\tfix tests\tqueued\tmain")
            .expect("legacy lane row should load");

        assert_eq!(lane.progress, 0);
        assert_eq!(lane.summary, "restored from lane store");
    }

    #[test]
    fn terminal_lane_tsv_round_trips_progress_and_summary() {
        let mut lane = TerminalLane::preview_lanes()
            .into_iter()
            .next()
            .expect("preview lane");
        lane.worktree = Some(PathBuf::from("/tmp/robocode-lane"));
        let loaded = TerminalLane::from_tsv(&lane.to_tsv()).expect("lane row should load");

        assert_eq!(loaded.progress, 64);
        assert_eq!(loaded.summary, "patched failing tests; rerunning cargo");
        assert_eq!(loaded.worktree, Some(PathBuf::from("/tmp/robocode-lane")));
    }

    #[test]
    fn terminal_lane_tsv_sanitizes_control_sequences_in_summary() {
        let lane = TerminalLane {
            id: "L1".to_string(),
            tool: "run".to_string(),
            title: "printf ok".to_string(),
            status: "attached".to_string(),
            target: "tmux session".to_string(),
            progress: 35,
            summary: "\u{1b}]697;PreExec\u{7}\u{1b}[31mold\rvisible\u{8}!".to_string(),
            worktree: None,
        };

        let loaded = TerminalLane::from_tsv(&lane.to_tsv()).expect("lane row should load");

        assert_eq!(loaded.summary, "visibl!");
        assert!(!loaded.summary.contains('\u{1b}'));
        assert!(!loaded.summary.contains('\u{7}'));
    }

    #[test]
    fn refresh_lane_runtime_updates_attached_lane_from_log_tail() {
        let root = temp_state_root();
        let lane_store = lane_store_path(&root);
        let artifact_dir = root.join(".robocode").join("lanes");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        fs::write(
            artifact_dir.join("L1.log"),
            "tmux booted\nlive pane output\n",
        )
        .expect("runtime log");
        let mut lanes = vec![TerminalLane {
            id: "L1".to_string(),
            tool: "claude".to_string(),
            title: "review interactively".to_string(),
            status: "attached".to_string(),
            target: "tmux robocode-session-l1".to_string(),
            progress: 10,
            summary: "tmux session ready".to_string(),
            worktree: None,
        }];

        refresh_lane_runtime(&lane_store, &mut lanes);

        assert_eq!(lanes[0].status, "attached");
        assert_eq!(lanes[0].summary, "live pane output");
        assert_eq!(lanes[0].progress, 35);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_lane_runtime_sanitizes_tmux_log_tail() {
        let root = temp_state_root();
        let lane_store = lane_store_path(&root);
        let artifact_dir = root.join(".robocode").join("lanes");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        fs::write(
            artifact_dir.join("L1.log"),
            "printf smoke\r\u{1b}]697;PreExec\u{7}\u{1b}[32msmoke-ok\u{1b}[0m\n\u{1b}[01;32m➜  \u{1b}[36mwork\u{1b}[00m \n",
        )
        .expect("runtime log");
        let mut lanes = vec![TerminalLane {
            id: "L1".to_string(),
            tool: "run".to_string(),
            title: "printf smoke".to_string(),
            status: "attached".to_string(),
            target: "tmux robocode-session-l1".to_string(),
            progress: 10,
            summary: "tmux session ready".to_string(),
            worktree: None,
        }];

        refresh_lane_runtime(&lane_store, &mut lanes);

        assert_eq!(lanes[0].summary, "smoke-ok");
        assert!(!lanes[0].summary.contains('\u{1b}'));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn companion_screen_store_round_trips_registry_rows() {
        let root = temp_state_root();
        let path = screen_store_path(&root);
        let screens = vec![CompanionScreen {
            id: "side-1".to_string(),
            title: "Agent lanes".to_string(),
            status: "launched".to_string(),
            pid: Some(4242),
            summary: "provider=deepseek model=deepseek-v4-flash".to_string(),
        }];

        save_screens(&path, &screens).expect("save screens");
        let loaded = load_screens(&path);

        assert_eq!(loaded, screens);
    }

    #[test]
    fn workspace_snapshot_loads_latest_codex_agent_jobs() {
        let root = temp_state_root();
        let agents = root.join(".robocode").join("agents");
        fs::create_dir_all(&agents).expect("agent dir");
        let codex_2_log = agents.join("codex-2.jsonl");
        let codex_2_result = agents.join("codex-2.result.md");
        fs::write(
            &codex_2_log,
            r#"{"direction":"server","payload":"{\"method\":\"turn/started\"}"}"#,
        )
        .expect("codex log");
        fs::write(
            &codex_2_result,
            "# Codex app-server turn\n\nthread: thread_2\nturn: turn_2\nstatus: completed\n",
        )
        .expect("codex result");
        fs::write(
            agents.join("codex-jobs.jsonl"),
            format!(
                "{}\n{}\n{}\n",
                r#"{"ts":10,"event":"started","id":"codex-1","kind":"run","status":"running","pid":4242,"command":"codex exec","task":"first task","log":"a","result":"b"}"#,
                r#"{"ts":20,"event":"completed","id":"codex-1","kind":"run","status":"finished","pid":4242,"command":"codex exec","task":"first task done","log":"a","result":"b"}"#,
                format!(
                    r#"{{"ts":30,"event":"started","id":"codex-2","kind":"review","status":"running","pid":5252,"command":"codex review","task":"review diff","log":"{}","result":"{}"}}"#,
                    codex_2_log.display(),
                    codex_2_result.display()
                )
            ),
        )
        .expect("jobs jsonl");

        let workspace = WorkspaceSnapshot::load(root);

        assert_eq!(workspace.agent_jobs.len(), 2);
        assert_eq!(workspace.agent_jobs[0].id, "codex-1");
        assert_eq!(workspace.agent_jobs[0].status, "finished");
        assert_eq!(workspace.agent_jobs[1].id, "codex-2");
        assert_eq!(workspace.agent_jobs[1].status, "running");
        assert_eq!(workspace.agent_jobs[1].task, "review diff");
        assert_eq!(
            workspace.agent_jobs[1].evidence,
            vec![
                "thread thread_2".to_string(),
                "turn turn_2".to_string(),
                "turn status completed".to_string(),
                "turn started".to_string()
            ]
        );
    }

    fn temp_state_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("robocode-tui-state-test-{nanos}"));
        fs::create_dir_all(&root).expect("temp root");
        root
    }
}
