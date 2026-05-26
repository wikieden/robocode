use std::time::Instant;

use super::*;
use robocode_types::{ApprovalResponse, ToolInput};

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
        let mut input = ToolInput::new();
        input.insert("command".to_string(), command.clone());

        // Tests are still shell commands, so they deliberately use the shared
        // mutating-tool permission path instead of bypassing approvals.
        let started = Instant::now();
        let result = self.run_named_tool_result("shell", input, approver)?;
        let evidence = TestEvidence {
            command,
            status: if result.success {
                "passed".to_string()
            } else {
                "failed".to_string()
            },
            duration_ms: started.elapsed().as_millis(),
            output_tail: tail_lines(&result.output, TEST_OUTPUT_TAIL_LINES),
        };
        let rendered = render_test_evidence(&evidence);
        self.last_test = Some(evidence);
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
        format!("  command: {}", evidence.command),
        format!("  duration: {}ms", evidence.duration_ms),
    ];
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
