use std::process::Command;

use robocode_types::{ContextBundleRecord, ContextSourceRecord};

use crate::{SessionEngine, TestEvidence};

const MAIN_SOFT_BUDGET: u64 = 48_000;
const MAIN_HARD_LIMIT: u64 = 128_000;

impl SessionEngine {
    pub fn provider_context_bundle(&self) -> Option<ContextBundleRecord> {
        self.last_context_bundle.clone()
    }

    pub(crate) fn build_main_context_bundle(&self, input: &str) -> ContextBundleRecord {
        let mut sources = vec![
            context_source("user-task", "task", input, 240),
            context_source(
                "workspace",
                "workspace",
                &format!(
                    "{} on {} with {} dirty files",
                    self.cwd.display(),
                    git_branch_label(self),
                    dirty_file_count(self).unwrap_or(0)
                ),
                320,
            ),
        ];
        if let Some(diff) = self
            .last_diff
            .as_deref()
            .filter(|diff| !diff.trim().is_empty())
        {
            sources.push(context_source(
                "latest-diff",
                "diff",
                &compact_text(diff, 18),
                720,
            ));
        }
        if let Some(test) = self.last_test.as_ref() {
            sources.push(context_source(
                "latest-test",
                "test",
                &test_summary(test),
                520,
            ));
        }
        let transcript = self
            .messages
            .iter()
            .rev()
            .take(6)
            .map(|message| {
                format!(
                    "{:?}: {}",
                    message.role,
                    compact_text(&message.content, 2).replace('\n', " ")
                )
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        if !transcript.trim().is_empty() {
            sources.push(context_source(
                "recent-transcript",
                "transcript-summary",
                &transcript,
                480,
            ));
        }
        let tasks = self
            .workflows
            .load_task_state()
            .ok()
            .map(|state| {
                state
                    .active_tasks()
                    .into_iter()
                    .take(4)
                    .map(|task| format!("{} {:?} {}", task.task_id, task.status, task.title))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if !tasks.trim().is_empty() {
            sources.push(context_source("active-tasks", "task-summary", &tasks, 360));
        }
        let memory = self
            .workflows
            .load_memory_state()
            .ok()
            .map(|state| {
                state
                    .active_project_memory()
                    .into_iter()
                    .chain(state.active_session_memory(self.session_id()))
                    .take(4)
                    .map(|entry| format!("{:?} {}", entry.kind, entry.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if !memory.trim().is_empty() {
            sources.push(context_source("memory", "memory", &memory, 360));
        }
        let runtime = self
            .runtime_tasks
            .iter()
            .rev()
            .take(4)
            .map(|task| format!("{} {} {}", task.kind, task.status, task.activity))
            .collect::<Vec<_>>()
            .join("\n");
        if !runtime.trim().is_empty() {
            sources.push(context_source(
                "runtime-tasks",
                "runtime-summary",
                &runtime,
                360,
            ));
        }
        let estimated_tokens = sources.iter().map(|source| source.estimated_tokens).sum();
        let largest_sources = largest_sources(&sources);
        let mut compaction_notes = vec![
            "long tool/test/lane output is summarized plus tail".to_string(),
            "raw transcript and tool audit remain in session storage".to_string(),
        ];
        if estimated_tokens > MAIN_SOFT_BUDGET {
            compaction_notes.push(
                "soft budget exceeded; low-priority sources should be trimmed first".to_string(),
            );
        }
        if estimated_tokens > MAIN_HARD_LIMIT {
            compaction_notes.push(
                "hard limit exceeded; provider input must omit lowest-priority sources".to_string(),
            );
        }
        ContextBundleRecord {
            bundle_id: format!("ctx-main-{}", self.session_id()),
            task_id: format!("turn-{}", self.session_id()),
            sources,
            estimated_tokens,
            largest_sources,
            compaction_notes,
            soft_token_budget: MAIN_SOFT_BUDGET,
            hard_token_limit: MAIN_HARD_LIMIT,
        }
    }
}

pub(crate) fn render_provider_context_message(bundle: &ContextBundleRecord) -> String {
    let sources = bundle
        .sources
        .iter()
        .map(|source| {
            format!(
                "- {} [{}] ~{} tok: {}",
                source.name, source.kind, source.estimated_tokens, source.summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "RoboCode ContextBundle\nBundle: {}\nEstimated tokens: {}\nContext pressure: {}%\nSoft budget: {}\nHard limit: {}\nSources:\n{}\nCompaction notes:\n{}",
        bundle.bundle_id,
        bundle.estimated_tokens,
        bundle.pressure_percent(),
        bundle.soft_token_budget,
        bundle.hard_token_limit,
        if sources.is_empty() {
            "- <none>".to_string()
        } else {
            sources
        },
        list_or_none(&bundle.compaction_notes)
    )
}

pub(crate) fn context_evidence_rows(bundle: &ContextBundleRecord) -> Vec<String> {
    let mut rows = vec![
        format!("context_bundle {}", bundle.bundle_id),
        format!(
            "context_pressure {}% ({}/{})",
            bundle.pressure_percent(),
            bundle.estimated_tokens,
            bundle.hard_token_limit
        ),
        format!("context_sources {}", bundle.sources.len()),
    ];
    if let Some(source) = bundle.largest_sources.first() {
        rows.push(format!("largest_context_source {source}"));
    }
    rows
}

fn context_source(
    name: &str,
    kind: &str,
    summary: &str,
    minimum_tokens: u64,
) -> ContextSourceRecord {
    ContextSourceRecord {
        name: name.to_string(),
        kind: kind.to_string(),
        estimated_tokens: estimate_tokens(summary).max(minimum_tokens),
        summary: summary.to_string(),
    }
}

fn estimate_tokens(text: &str) -> u64 {
    text.chars().count().saturating_div(4).max(1) as u64
}

fn compact_text(text: &str, max_lines: usize) -> String {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() <= max_lines {
        return lines.join("\n");
    }
    let head = max_lines / 2;
    let tail = max_lines.saturating_sub(head);
    let mut compacted = lines.iter().take(head).copied().collect::<Vec<_>>();
    compacted.push("<... summarized middle ...>");
    compacted.extend(lines.iter().skip(lines.len().saturating_sub(tail)).copied());
    compacted.join("\n")
}

fn test_summary(test: &TestEvidence) -> String {
    let exit = test
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "<unknown>".to_string());
    format!(
        "{} exit={} duration={}ms command={}\n{}",
        test.status,
        exit,
        test.duration_ms,
        test.command,
        compact_text(&test.output_tail, 8)
    )
}

fn largest_sources(sources: &[ContextSourceRecord]) -> Vec<String> {
    let mut rows = sources
        .iter()
        .map(|source| {
            (
                source.estimated_tokens,
                format!("{} {} tok", source.name, source.estimated_tokens),
            )
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| std::cmp::Reverse(row.0));
    rows.into_iter().take(3).map(|(_, row)| row).collect()
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "- <none>".to_string()
    } else {
        values
            .iter()
            .map(|value| format!("- {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn dirty_file_count(engine: &SessionEngine) -> Option<usize> {
    let output = Command::new("git")
        .args(["status", "--short", "--untracked-files=all"])
        .current_dir(&engine.cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
    )
}

fn git_branch_label(engine: &SessionEngine) -> String {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&engine.cwd)
        .output();
    output
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|branch| !branch.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
