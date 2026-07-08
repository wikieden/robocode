use super::*;
use viden_types::{ToolCall, ToolInput};

#[test]
fn shell_adapter_builds_cross_platform_invocations() {
    let (program_unix, args_unix) = build_shell_invocation("echo hi", false);
    assert_eq!(program_unix, "sh");
    assert_eq!(args_unix[0], "-c");

    let (program_windows, args_windows) = build_shell_invocation("echo hi", true);
    assert_eq!(program_windows, "powershell");
    assert_eq!(args_windows[2], "-Command");
}

#[test]
fn shell_adapter_pipes_long_commands_through_stdin() {
    let cwd = temp_dir("long_shell_command");
    let long_comment = "x".repeat(40 * 1024);
    let mut input = ToolInput::new();
    input.insert(
        "command".to_string(),
        format!("printf ok\n# {long_comment}"),
    );
    let call = ToolCall {
        id: "tool_shell".to_string(),
        name: "shell".to_string(),
        input,
    };
    let ctx = ToolExecutionContext {
        cwd: cwd.clone(),
        semantic: None,
    };

    assert!(shell_requires_stdin(call.input.get("command").unwrap()));
    let result = ToolRegistry::builtin().execute(&call, &ctx).unwrap();

    assert!(result.success, "{}", result.output);
    assert_eq!(result.output, "ok");
    let _ = fs::remove_dir_all(cwd);
}
