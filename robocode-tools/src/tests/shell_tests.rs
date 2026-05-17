use super::*;

#[test]
fn shell_adapter_builds_cross_platform_invocations() {
    let (program_unix, args_unix) = build_shell_invocation("echo hi", false);
    assert_eq!(program_unix, "sh");
    assert_eq!(args_unix[0], "-lc");

    let (program_windows, args_windows) = build_shell_invocation("echo hi", true);
    assert_eq!(program_windows, "powershell");
    assert_eq!(args_windows[2], "-Command");
}
