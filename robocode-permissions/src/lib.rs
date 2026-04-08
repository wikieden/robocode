use std::path::{Component, Path, PathBuf};

use robocode_types::{
    AdditionalWorkingDirectory, ApprovalResponse, PermissionAllowDecision, PermissionAskDecision,
    PermissionBehavior, PermissionDecision, PermissionDecisionReason, PermissionDenyDecision,
    PermissionMode, PermissionPrompt, PermissionRule, PermissionRuleSource, ToolInput, ToolSpec,
};

#[derive(Debug, Clone)]
pub struct PermissionContext {
    pub mode: PermissionMode,
    pub additional_working_directories: Vec<AdditionalWorkingDirectory>,
    pub allow_rules: Vec<PermissionRule>,
    pub deny_rules: Vec<PermissionRule>,
    pub ask_rules: Vec<PermissionRule>,
}

impl Default for PermissionContext {
    fn default() -> Self {
        Self {
            mode: PermissionMode::Default,
            additional_working_directories: Vec::new(),
            allow_rules: Vec::new(),
            deny_rules: Vec::new(),
            ask_rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PermissionEngine {
    cwd: PathBuf,
    context: PermissionContext,
}

impl PermissionEngine {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            context: PermissionContext::default(),
        }
    }

    pub fn mode(&self) -> PermissionMode {
        self.context.mode
    }

    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.context.mode = mode;
    }

    pub fn add_directory(&mut self, path: impl Into<String>, source: PermissionRuleSource) {
        self.context
            .additional_working_directories
            .push(AdditionalWorkingDirectory {
                path: path.into(),
                source,
            });
    }

    pub fn add_rule(&mut self, rule: PermissionRule) {
        match rule.rule_behavior {
            PermissionBehavior::Allow => self.context.allow_rules.push(rule),
            PermissionBehavior::Deny => self.context.deny_rules.push(rule),
            PermissionBehavior::Ask => self.context.ask_rules.push(rule),
        }
    }

    pub fn decide(&self, tool: &ToolSpec, input: &ToolInput) -> PermissionDecision {
        let scoped_paths = extract_paths(input);
        if !scoped_paths.iter().all(|path| self.is_path_in_scope(path)) {
            if tool.name == "git_worktree_add" || tool.name == "git_worktree_remove" {
                return PermissionDecision::Ask(PermissionAskDecision {
                    message: format!(
                        "Approve {} outside the current working directory?",
                        tool.name
                    ),
                    updated_input: None,
                    decision_reason: Some(PermissionDecisionReason::RequiresApproval),
                });
            }
            return PermissionDecision::Deny(PermissionDenyDecision {
                message: "Path is outside the allowed working directory scope".to_string(),
                decision_reason: PermissionDecisionReason::OutOfScopePath,
            });
        }

        if self.matches_rule(&self.context.deny_rules, tool, input) {
            return PermissionDecision::Deny(PermissionDenyDecision {
                message: format!("Denied by permission rule for {}", tool.name),
                decision_reason: PermissionDecisionReason::RuleDeny,
            });
        }

        if self.matches_rule(&self.context.ask_rules, tool, input) {
            return PermissionDecision::Ask(PermissionAskDecision {
                message: format!("Approval required by permission rule for {}", tool.name),
                updated_input: None,
                decision_reason: Some(PermissionDecisionReason::RuleAsk),
            });
        }

        if self.matches_rule(&self.context.allow_rules, tool, input) {
            return PermissionDecision::Allow(PermissionAllowDecision {
                updated_input: None,
                user_modified: false,
                decision_reason: Some(PermissionDecisionReason::RuleAllow),
                accept_feedback: None,
            });
        }

        match self.context.mode {
            PermissionMode::BypassPermissions => {
                PermissionDecision::Allow(PermissionAllowDecision {
                    updated_input: None,
                    user_modified: false,
                    decision_reason: Some(PermissionDecisionReason::BypassMode),
                    accept_feedback: None,
                })
            }
            PermissionMode::DontAsk => PermissionDecision::Allow(PermissionAllowDecision {
                updated_input: None,
                user_modified: false,
                decision_reason: Some(PermissionDecisionReason::DontAskMode),
                accept_feedback: None,
            }),
            PermissionMode::Plan if tool.is_mutating => {
                PermissionDecision::Deny(PermissionDenyDecision {
                    message: format!("{} is blocked while plan mode is active", tool.name),
                    decision_reason: PermissionDecisionReason::PlanMode,
                })
            }
            PermissionMode::AcceptEdits
                if tool.name == "write_file" || tool.name == "edit_file" =>
            {
                PermissionDecision::Allow(PermissionAllowDecision {
                    updated_input: None,
                    user_modified: false,
                    decision_reason: Some(PermissionDecisionReason::AcceptEditsMode),
                    accept_feedback: None,
                })
            }
            _ if !tool.is_mutating && tool.name != "shell" => {
                PermissionDecision::Allow(PermissionAllowDecision {
                    updated_input: None,
                    user_modified: false,
                    decision_reason: Some(PermissionDecisionReason::SafeRead),
                    accept_feedback: None,
                })
            }
            _ => PermissionDecision::Ask(PermissionAskDecision {
                message: format!("Approve {}?", tool.name),
                updated_input: None,
                decision_reason: Some(PermissionDecisionReason::RequiresApproval),
            }),
        }
    }

    pub fn prompt_for(
        tool_name: &str,
        decision: &PermissionAskDecision,
        input: &ToolInput,
    ) -> PermissionPrompt {
        PermissionPrompt {
            tool_name: tool_name.to_string(),
            message: decision.message.clone(),
            input_preview: robocode_types::encode_tool_input(input),
        }
    }

    pub fn apply_approval(
        &self,
        response: ApprovalResponse,
        decision: &PermissionAskDecision,
    ) -> PermissionDecision {
        if response.approved {
            PermissionDecision::Allow(PermissionAllowDecision {
                updated_input: decision.updated_input.clone(),
                user_modified: false,
                decision_reason: decision.decision_reason.clone(),
                accept_feedback: response.feedback,
            })
        } else {
            PermissionDecision::Deny(PermissionDenyDecision {
                message: "User denied the permission request".to_string(),
                decision_reason: decision
                    .decision_reason
                    .clone()
                    .unwrap_or(PermissionDecisionReason::RequiresApproval),
            })
        }
    }

    fn matches_rule(&self, rules: &[PermissionRule], tool: &ToolSpec, input: &ToolInput) -> bool {
        let rendered = robocode_types::encode_tool_input(input);
        rules.iter().any(|rule| {
            rule.rule_value.tool_name == tool.name
                && match &rule.rule_value.rule_content {
                    Some(expected) => rendered.contains(expected),
                    None => true,
                }
        })
    }

    fn is_path_in_scope(&self, raw: &str) -> bool {
        let resolved = normalize_path(&self.cwd, raw);
        if resolved.starts_with(&self.cwd) {
            return true;
        }
        self.context
            .additional_working_directories
            .iter()
            .map(|directory| normalize_path(&self.cwd, &directory.path))
            .any(|directory| resolved.starts_with(directory))
    }
}

fn extract_paths(input: &ToolInput) -> Vec<String> {
    ["path", "from", "to"]
        .iter()
        .filter_map(|key| input.get(*key).cloned())
        .collect()
}

fn normalize_path(cwd: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    let joined = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use robocode_types::{PermissionRuleValue, ToolInput, ToolSpec};

    fn tool(name: &str, is_mutating: bool) -> ToolSpec {
        ToolSpec {
            name: name.to_string(),
            description: name.to_string(),
            is_mutating,
            input_schema_hint: String::new(),
        }
    }

    fn input(path: &str) -> ToolInput {
        let mut input = ToolInput::new();
        input.insert("path".to_string(), path.to_string());
        input
    }

    #[test]
    fn default_mode_allows_reads_in_scope() {
        let engine = PermissionEngine::new("/tmp/project");
        let decision = engine.decide(&tool("read_file", false), &input("src/main.rs"));
        assert!(matches!(decision, PermissionDecision::Allow(_)));
    }

    #[test]
    fn default_mode_asks_for_mutations() {
        let engine = PermissionEngine::new("/tmp/project");
        let decision = engine.decide(&tool("write_file", true), &input("src/main.rs"));
        assert!(matches!(decision, PermissionDecision::Ask(_)));
    }

    #[test]
    fn plan_mode_denies_mutations() {
        let mut engine = PermissionEngine::new("/tmp/project");
        engine.set_mode(PermissionMode::Plan);
        let decision = engine.decide(&tool("write_file", true), &input("src/main.rs"));
        assert!(matches!(decision, PermissionDecision::Deny(_)));
    }

    #[test]
    fn allow_rule_overrides_default_behavior() {
        let mut engine = PermissionEngine::new("/tmp/project");
        engine.add_rule(PermissionRule {
            source: PermissionRuleSource::Session,
            rule_behavior: PermissionBehavior::Allow,
            rule_value: PermissionRuleValue {
                tool_name: "shell".to_string(),
                rule_content: Some("cargo test".to_string()),
            },
        });
        let mut shell_input = ToolInput::new();
        shell_input.insert("command".into(), "cargo test".into());
        let decision = engine.decide(&tool("shell", true), &shell_input);
        assert!(matches!(decision, PermissionDecision::Allow(_)));
    }

    #[test]
    fn additional_directory_expands_scope() {
        let mut engine = PermissionEngine::new("/tmp/project");
        engine.add_directory("/tmp/shared", PermissionRuleSource::Session);
        let decision = engine.decide(&tool("read_file", false), &input("/tmp/shared/file.txt"));
        assert!(matches!(decision, PermissionDecision::Allow(_)));
    }

    #[test]
    fn git_worktree_out_of_scope_path_asks_instead_of_denying() {
        let engine = PermissionEngine::new("/tmp/project");
        let decision = engine.decide(
            &tool("git_worktree_add", true),
            &input("/tmp/project-worktrees/feature-demo"),
        );
        assert!(matches!(decision, PermissionDecision::Ask(_)));
    }
}
