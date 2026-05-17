use super::*;

#[test]
fn read_write_edit_round_trip() {
    let cwd = temp_dir("files");
    let ctx = ToolExecutionContext {
        cwd: cwd.clone(),
        semantic: None,
    };
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
    assert!(edit_result.diff.unwrap().contains("+hello rust"));
}
