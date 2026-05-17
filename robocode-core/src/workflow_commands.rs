mod memory_commands;
mod task_commands;

use super::SessionEngine;
use robocode_permissions::PermissionEngine;
use robocode_types::{
    ApprovalResponse, PermissionDecision, PermissionLogEntry, ToolInput, ToolSpec, TranscriptEntry,
    now_timestamp,
};

impl SessionEngine {
    fn ensure_workflow_permission<F>(
        &mut self,
        action: &str,
        preview: &str,
        approver: &mut F,
    ) -> Result<Option<String>, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let tool_name = format!("workflow_{action}");
        let tool = ToolSpec {
            name: tool_name.clone(),
            description: format!("Workflow mutation: {action}"),
            is_mutating: true,
            input_schema_hint: "workflow action".to_string(),
        };
        let mut input = ToolInput::new();
        input.insert("action".to_string(), action.to_string());
        input.insert("preview".to_string(), preview.to_string());
        let mut decision = self.permissions.decide(&tool, &input);
        if let PermissionDecision::Ask(ask) = &decision {
            let prompt = PermissionEngine::prompt_for(&tool_name, ask, &input);
            let approval = approver(prompt);
            decision = self.permissions.apply_approval(approval, ask);
        }
        match decision {
            PermissionDecision::Allow(allow) => {
                self.store_entry(TranscriptEntry::Permission {
                    entry: PermissionLogEntry {
                        timestamp: now_timestamp(),
                        tool_name,
                        decision: "allow".to_string(),
                        reason: format!("{:?}", allow.decision_reason),
                        message: allow.accept_feedback,
                    },
                })?;
                Ok(None)
            }
            PermissionDecision::Ask(_) => unreachable!("ask decisions should be resolved"),
            PermissionDecision::Deny(deny) => {
                self.store_entry(TranscriptEntry::Permission {
                    entry: PermissionLogEntry {
                        timestamp: now_timestamp(),
                        tool_name: tool_name.clone(),
                        decision: "deny".to_string(),
                        reason: format!("{:?}", deny.decision_reason),
                        message: Some(deny.message.clone()),
                    },
                })?;
                Ok(Some(format!(
                    "Permission denied for {tool_name}: {}",
                    deny.message
                )))
            }
        }
    }
}
