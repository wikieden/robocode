use std::time::Instant;

use super::*;
use robocode_types::{AgentTaskStatus, ApprovalResponse, ToolInput};

const TEST_OUTPUT_TAIL_LINES: usize = 12;

impl SessionEngine {
    pub(super) fn handle_test_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        if args.is_empty() {
            return Ok(self.render_test_help());
        }

        let command = args.join(" ");
        let running_task = self.test_task(
            &command,
            AgentTaskStatus::Testing,
            format!("running test `{command}`"),
            20,
            None,
        );
        self.upsert_agent_task(running_task);
        let mut input = ToolInput::new();
        input.insert("command".to_string(), command.clone());

        // Tests are still shell commands, so they deliberately use the shared
        // mutating-tool permission path instead of bypassing approvals.
        let started = Instant::now();
        let result = self.run_named_tool_result("shell", input, approver)?;
        let failure_details = if result.success {
            TestFailureDetails::default()
        } else {
            extract_failure_details(&result.output)
        };
        let evidence = TestEvidence {
            command: command.clone(),
            status: if result.success {
                "passed".to_string()
            } else {
                "failed".to_string()
            },
            exit_code: result.exit_code,
            duration_ms: started.elapsed().as_millis(),
            failure_summary: failure_details.summary,
            failing_files: failure_details.files,
            output_tail: tail_lines(&result.output, TEST_OUTPUT_TAIL_LINES),
        };
        let rendered = render_test_evidence(&evidence);
        self.last_test = Some(evidence);
        let final_task = self.test_task(
            &command,
            if self
                .last_test
                .as_ref()
                .map(|evidence| evidence.status == "passed")
                .unwrap_or(false)
            {
                AgentTaskStatus::Done
            } else {
                AgentTaskStatus::Failed
            },
            format!(
                "test {}",
                self.last_test
                    .as_ref()
                    .map(|evidence| evidence.status.as_str())
                    .unwrap_or("finished")
            ),
            100,
            self.last_test.as_ref(),
        );
        self.upsert_agent_task(final_task);
        Ok(rendered)
    }

    pub(super) fn render_test_help(&self) -> String {
        [
            "Test commands:",
            "  /test <command>",
            "",
            "Runs a test command through the normal shell permission path and records the latest result for /status.",
        ]
        .join("\n")
    }
}

pub(crate) fn render_test_evidence(evidence: &TestEvidence) -> String {
    let mut lines = vec![
        "Test result:".to_string(),
        format!("  status: {}", evidence.status),
        format!(
            "  exit code: {}",
            evidence
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "<unknown>".to_string())
        ),
        format!("  command: {}", evidence.command),
        format!("  duration: {}ms", evidence.duration_ms),
    ];
    if !evidence.failure_summary.is_empty() {
        lines.push("  failure summary:".to_string());
        lines.extend(
            evidence
                .failure_summary
                .iter()
                .map(|line| format!("    - {line}")),
        );
    }
    if !evidence.failing_files.is_empty() {
        lines.push("  failing files:".to_string());
        lines.extend(
            evidence
                .failing_files
                .iter()
                .map(|line| format!("    - {line}")),
        );
    }
    if evidence.output_tail.trim().is_empty() {
        lines.push("  output: <empty>".to_string());
    } else {
        lines.push("  output tail:".to_string());
        lines.extend(
            evidence
                .output_tail
                .lines()
                .map(|line| format!("    {line}")),
        );
    }
    lines.join("\n")
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    let mut lines = text.lines().rev().take(max_lines).collect::<Vec<_>>();
    lines.reverse();
    lines.join("\n")
}

#[derive(Default)]
struct TestFailureDetails {
    summary: Vec<String>,
    files: Vec<String>,
}

fn extract_failure_details(output: &str) -> TestFailureDetails {
    let mut details = TestFailureDetails::default();
    let mut capture_next_failure_name = false;
    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(location) = rust_location(line) {
            push_unique(&mut details.files, location);
        }
        if let Some(path) = pytest_failure_path(line) {
            push_unique(&mut details.files, path);
        }

        let lower = line.to_ascii_lowercase();
        let is_summary_line = lower.starts_with("error")
            || lower.starts_with("failed ")
            || lower.starts_with("thread '")
            || lower.starts_with("panic")
            || lower.contains("assertion failed");
        if is_summary_line {
            push_unique(&mut details.summary, line.to_string());
            capture_next_failure_name = false;
            continue;
        }
        if lower == "failures:" {
            capture_next_failure_name = true;
            continue;
        }
        if capture_next_failure_name {
            push_unique(&mut details.summary, line.to_string());
            capture_next_failure_name = false;
        }
    }
    details.summary.truncate(5);
    details.files.truncate(5);
    details
}

fn rust_location(line: &str) -> Option<String> {
    let marker = "-->";
    let index = line.find(marker)?;
    let location = line[index + marker.len()..].trim();
    if location.is_empty() {
        None
    } else {
        Some(location.to_string())
    }
}

fn pytest_failure_path(line: &str) -> Option<String> {
    let rest = line.strip_prefix("FAILED ")?;
    let token = rest.split_whitespace().next()?;
    let path = token.split("::").next()?.trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
