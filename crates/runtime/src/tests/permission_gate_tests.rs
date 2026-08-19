use std::sync::Arc;

use viden_permissions::PermissionEngine;
use viden_tools::{ToolExecutionContext, ToolRegistry};
use viden_types::{
    ApprovalDecision, ApprovalResponse, ApprovalScope, PermissionDecision, PermissionMode,
    ToolCall, ToolInput, ToolSpec,
};

use super::temp_dir;
use crate::permission_gate::{self, PermissionBackstopInterceptor, SharedPermissionEngine};

fn allow_once() -> ApprovalResponse {
    ApprovalResponse {
        decision: ApprovalDecision::Allow {
            scope: ApprovalScope::Once,
        },
        feedback: None,
    }
}

fn deny() -> ApprovalResponse {
    ApprovalResponse {
        decision: ApprovalDecision::Deny,
        feedback: None,
    }
}

fn write_file_spec() -> ToolSpec {
    ToolSpec {
        name: "write_file".to_string(),
        description: "test write".to_string(),
        is_mutating: true,
        input_schema_hint: "path content".to_string(),
    }
}

fn write_file_input(path: &str) -> ToolInput {
    let mut input = ToolInput::new();
    input.insert("path".to_string(), path.to_string());
    input.insert("content".to_string(), "gated content".to_string());
    input
}

#[test]
fn gate_resolves_interactive_ask_to_allow() {
    let cwd = temp_dir("gate_allow");
    let mut engine = PermissionEngine::new(&cwd);
    let tool = write_file_spec();
    let input = write_file_input("notes.txt");
    let mut prompted = Vec::new();
    let decision =
        permission_gate::resolve(&mut engine, &tool, "write_file", &input, |_, prompt| {
            prompted.push(prompt.tool_name.clone());
            allow_once()
        });
    assert!(matches!(decision, PermissionDecision::Allow(_)));
    assert_eq!(prompted, vec!["write_file".to_string()]);
}

#[test]
fn gate_maps_denied_approval_to_deny() {
    let cwd = temp_dir("gate_deny");
    let mut engine = PermissionEngine::new(&cwd);
    let tool = write_file_spec();
    let input = write_file_input("notes.txt");
    let decision =
        permission_gate::resolve(&mut engine, &tool, "write_file", &input, |_, _| deny());
    assert!(matches!(decision, PermissionDecision::Deny(_)));
}

#[test]
fn gate_preserves_plan_mode_recheck_after_approval() {
    // The fail-closed double checkpoint: even an operator "allow" cannot pass
    // once plan mode became active between decide() and apply_approval().
    let cwd = temp_dir("gate_plan_recheck");
    let shared = SharedPermissionEngine::new(PermissionEngine::new(&cwd));
    let tool = write_file_spec();
    let input = write_file_input("notes.txt");
    let flip = shared.clone();
    let mut decider = shared.clone();
    let decision = permission_gate::resolve(&mut decider, &tool, "write_file", &input, |_, _| {
        flip.set_mode(PermissionMode::Plan);
        allow_once()
    });
    match decision {
        PermissionDecision::Deny(deny) => {
            assert!(deny.message.contains("plan mode"), "got: {}", deny.message)
        }
        other => panic!("expected plan-mode deny, got {other:?}"),
    }
}

#[test]
fn backstop_blocks_mutating_tool_in_plan_mode_without_gate() {
    let cwd = temp_dir("backstop_plan");
    let shared = SharedPermissionEngine::new(PermissionEngine::new(&cwd));
    shared.set_mode(PermissionMode::Plan);
    let mut registry = ToolRegistry::builtin();
    registry.register_interceptor(Arc::new(PermissionBackstopInterceptor::new(shared)));
    // A forgetful call site dispatching straight to execute() must still be
    // stopped before the tool mutates anything.
    let call = ToolCall {
        id: "tool_backstop_plan".into(),
        name: "write_file".into(),
        input: write_file_input("blocked.txt"),
    };
    let error = registry
        .execute(&call, &ToolExecutionContext::local(&cwd))
        .unwrap_err();
    assert!(error.contains("plan mode"), "got: {error}");
    assert!(!cwd.join("blocked.txt").exists());
}

#[test]
fn backstop_blocks_unresolved_ask_fail_closed() {
    let cwd = temp_dir("backstop_ask");
    let shared = SharedPermissionEngine::new(PermissionEngine::new(&cwd));
    let mut registry = ToolRegistry::builtin();
    registry.register_interceptor(Arc::new(PermissionBackstopInterceptor::new(shared)));
    let call = ToolCall {
        id: "tool_backstop_ask".into(),
        name: "write_file".into(),
        input: write_file_input("blocked.txt"),
    };
    let error = registry
        .execute(&call, &ToolExecutionContext::local(&cwd))
        .unwrap_err();
    assert!(error.contains("approval was not resolved"), "got: {error}");
    assert!(!cwd.join("blocked.txt").exists());
}

#[test]
fn backstop_admits_execution_after_gate_resolution() {
    let cwd = temp_dir("backstop_cleared");
    let shared = SharedPermissionEngine::new(PermissionEngine::new(&cwd));
    let mut registry = ToolRegistry::builtin();
    registry.register_interceptor(Arc::new(PermissionBackstopInterceptor::new(shared.clone())));
    let tool = write_file_spec();
    let input = write_file_input("cleared.txt");
    let mut decider = shared.clone();
    let decision = permission_gate::resolve(&mut decider, &tool, "write_file", &input, |_, _| {
        allow_once()
    });
    assert!(matches!(decision, PermissionDecision::Allow(_)));
    let call = ToolCall {
        id: "tool_backstop_cleared".into(),
        name: "write_file".into(),
        input,
    };
    let result = registry
        .execute(&call, &ToolExecutionContext::local(&cwd))
        .unwrap();
    assert!(result.success);
    assert!(cwd.join("cleared.txt").exists());
}
