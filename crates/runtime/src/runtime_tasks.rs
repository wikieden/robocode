use viden_types::{AgentNextAction, AgentTaskRecord, AgentTaskStatus, EvidenceView, now_timestamp};

use crate::{SessionEngine, TestEvidence};

impl SessionEngine {
    pub fn agent_task_snapshot(&self) -> Vec<AgentTaskRecord> {
        let mut tasks = self.runtime_tasks.clone();
        tasks.sort_by(|left, right| {
            right
                .is_active()
                .cmp(&left.is_active())
                .then(right.priority().cmp(&left.priority()))
                .then(right.updated_at.cmp(&left.updated_at))
                .then(left.id.cmp(&right.id))
        });
        tasks
    }

    pub(crate) fn upsert_agent_task(&mut self, task: AgentTaskRecord) {
        if let Some(existing) = self
            .runtime_tasks
            .iter_mut()
            .find(|item| item.id == task.id)
        {
            *existing = task;
        } else {
            self.runtime_tasks.push(task);
        }
        self.runtime_tasks.retain(|task| {
            task.is_active()
                || task
                    .updated_at
                    .map(|updated| updated >= now_millis().saturating_sub(15 * 60 * 1000))
                    .unwrap_or(true)
        });
    }

    pub(crate) fn upsert_runtime_evidence(&mut self, evidence: EvidenceView) {
        if let Some(existing) = self
            .runtime_evidence
            .iter_mut()
            .find(|item| item.id == evidence.id)
        {
            *existing = evidence;
        } else {
            self.runtime_evidence.push(evidence);
        }
    }

    pub(crate) fn provider_task(
        &self,
        input: &str,
        status: AgentTaskStatus,
        activity: impl Into<String>,
        progress: u8,
    ) -> AgentTaskRecord {
        let now = now_millis();
        AgentTaskRecord {
            id: format!("turn-{}-{now}", compact_session_id(self.session_id())),
            parent_id: None,
            agent: "viden".to_string(),
            kind: "provider".to_string(),
            transport: self.provider_name().to_string(),
            title: first_line(input),
            status: status.as_str().to_string(),
            activity: activity.into(),
            summary: format!(
                "{} / {} is processing",
                self.provider_name(),
                self.model_name()
            ),
            progress,
            started_at: Some(now),
            updated_at: Some(now),
            workspace: Some(self.cwd.to_string_lossy().to_string()),
            evidence: vec![
                "live provider request".to_string(),
                format!("provider {}", self.provider_name()),
                format!("model {}", self.model_name()),
            ],
            permissions: Vec::new(),
            decision: None,
            result: None,
            resume_handle: None,
            pid: None,
            next_action: Some(AgentNextAction {
                label: "watch provider".to_string(),
                command: Some("/status".to_string()),
                reason: Some("provider turn is active".to_string()),
            }),
        }
    }

    pub(crate) fn tool_task(
        &self,
        call_id: &str,
        tool_name: &str,
        encoded_input: &str,
        status: AgentTaskStatus,
        activity: impl Into<String>,
        progress: u8,
    ) -> AgentTaskRecord {
        let now = now_millis();
        AgentTaskRecord {
            id: format!("tool-{call_id}"),
            parent_id: None,
            agent: agent_for_tool(tool_name).to_string(),
            kind: if tool_name == "shell" {
                "shell".to_string()
            } else {
                "tool".to_string()
            },
            transport: "local".to_string(),
            title: format!("{tool_name} {encoded_input}"),
            status: status.as_str().to_string(),
            activity: activity.into(),
            summary: format!("{tool_name} {encoded_input}"),
            progress,
            started_at: Some(now),
            updated_at: Some(now),
            workspace: Some(self.cwd.to_string_lossy().to_string()),
            evidence: vec![format!("tool_call_id {call_id}")],
            permissions: Vec::new(),
            decision: None,
            result: None,
            resume_handle: None,
            pid: None,
            next_action: Some(AgentNextAction {
                label: "inspect transcript".to_string(),
                command: Some("/status".to_string()),
                reason: Some("tool call is part of current turn".to_string()),
            }),
        }
    }

    pub(crate) fn test_task(
        &self,
        command: &str,
        status: AgentTaskStatus,
        activity: impl Into<String>,
        progress: u8,
        evidence: Option<&TestEvidence>,
    ) -> AgentTaskRecord {
        let now = now_millis();
        let mut rows = vec![format!("command {command}")];
        let mut result = None;
        if let Some(evidence) = evidence {
            rows.push(format!("exit_code {:?}", evidence.exit_code));
            rows.push(format!("duration {}ms", evidence.duration_ms));
            if !evidence.output_tail.trim().is_empty() {
                rows.push(format!(
                    "tail {}",
                    first_line(&evidence.output_tail)
                        .chars()
                        .take(80)
                        .collect::<String>()
                ));
            }
            result = Some(format!(
                "{} exit={}",
                evidence.status,
                evidence
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string())
            ));
        }
        AgentTaskRecord {
            id: format!("test-{}", stable_task_suffix(command)),
            parent_id: None,
            agent: "shell".to_string(),
            kind: "test".to_string(),
            transport: "shell".to_string(),
            title: command.to_string(),
            status: status.as_str().to_string(),
            activity: activity.into(),
            summary: command.to_string(),
            progress,
            started_at: Some(now),
            updated_at: Some(now),
            workspace: Some(self.cwd.to_string_lossy().to_string()),
            evidence: rows,
            permissions: vec!["shell approval".to_string()],
            decision: None,
            result,
            resume_handle: None,
            pid: None,
            next_action: Some(AgentNextAction {
                label: "rerun test".to_string(),
                command: Some(format!("/test {command}")),
                reason: Some("latest test command can be rerun".to_string()),
            }),
        }
    }
}

fn now_millis() -> u128 {
    u128::from(now_timestamp()) * 1000
}

fn first_line(input: &str) -> String {
    input
        .lines()
        .next()
        .unwrap_or(input)
        .chars()
        .take(120)
        .collect()
}

fn compact_session_id(session_id: &str) -> String {
    session_id.chars().take(8).collect()
}

fn agent_for_tool(tool_name: &str) -> &str {
    match tool_name {
        "shell" => "shell",
        "git_diff" => "git",
        _ => "viden",
    }
}

fn stable_task_suffix(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
