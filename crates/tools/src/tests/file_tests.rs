use super::*;

#[test]
fn read_write_edit_round_trip() {
    let cwd = temp_dir("files");
    let ctx = ToolExecutionContext::local(cwd.clone());
    let registry = ToolRegistry::builtin();

    let mut write_input = ToolInput::new();
    write_input.insert("path".into(), "notes.txt".into());
    write_input.insert("content".into(), "hello world".into());
    let write_result = registry
        .execute(
            &ToolCall {
                id: "tool_write".into(),
                name: "write_file".into(),
                input: write_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(write_result.success);
    assert!(write_result.output.contains("write_file completed"));
    assert!(write_result.output.contains("path:"));
    assert!(write_result.output.contains("notes.txt"));
    assert!(write_result.output.contains("size: 11 B"));
    assert!(write_result.output.contains("effect: wrote 1 line"));

    let mut read_input = ToolInput::new();
    read_input.insert("path".into(), "notes.txt".into());
    let read_result = registry
        .execute(
            &ToolCall {
                id: "tool_read".into(),
                name: "read_file".into(),
                input: read_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(read_result.output.contains("hello world"));

    let mut edit_input = ToolInput::new();
    edit_input.insert("path".into(), "notes.txt".into());
    edit_input.insert("old".into(), "world".into());
    edit_input.insert("new".into(), "rust".into());
    let edit_result = registry
        .execute(
            &ToolCall {
                id: "tool_edit".into(),
                name: "edit_file".into(),
                input: edit_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(edit_result.output.contains("edit_file completed"));
    assert!(edit_result.output.contains("path:"));
    assert!(edit_result.output.contains("notes.txt"));
    assert!(edit_result.output.contains("size: 10 B"));
    assert!(edit_result.output.contains("effect: edited 1 line"));
    assert!(edit_result.diff.unwrap().contains("+hello rust"));
}

#[test]
fn file_tools_report_directory_paths_clearly() {
    let cwd = temp_dir("file_tool_directory_errors");
    std::fs::create_dir_all(cwd.join("src")).unwrap();
    let ctx = ToolExecutionContext::local(cwd);
    let registry = ToolRegistry::builtin();

    let mut read_input = ToolInput::new();
    read_input.insert("path".into(), "src".into());
    let read_error = registry
        .execute(
            &ToolCall {
                id: "tool_read_dir".into(),
                name: "read_file".into(),
                input: read_input,
            },
            &ctx,
        )
        .unwrap_err();
    assert!(read_error.contains("expected a file"));
    assert!(read_error.contains("is a directory"));

    let mut write_input = ToolInput::new();
    write_input.insert("path".into(), "src".into());
    write_input.insert("content".into(), "hello".into());
    let write_error = registry
        .execute(
            &ToolCall {
                id: "tool_write_dir".into(),
                name: "write_file".into(),
                input: write_input,
            },
            &ctx,
        )
        .unwrap_err();
    assert!(write_error.contains("existing directory"));
}
