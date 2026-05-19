use super::*;

#[test]
fn build_openai_body_includes_tools() {
    let request = ModelRequest {
        session_id: "session_test".to_string(),
        model: "gpt-5.2".to_string(),
        messages: vec![Message::new(Role::User, "inspect Cargo.toml")],
        tools: vec![ToolSpec {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            is_mutating: false,
            input_schema_hint: "path=file max_bytes=8192".to_string(),
        }],
        permission_mode: PermissionMode::Default,
    };
    let body = build_openai_body_with_stream("gpt-5.2", &request, false);
    assert!(body.contains("\"tools\""));
    assert!(body.contains("\"read_file\""));
    assert!(body.contains("\"path\""));
}

#[test]
fn build_openai_body_can_request_streaming() {
    let request = ModelRequest {
        session_id: "session_test".to_string(),
        model: "gpt-5.2".to_string(),
        messages: vec![Message::new(Role::User, "hello")],
        tools: Vec::new(),
        permission_mode: PermissionMode::Default,
    };

    let body: Value =
        serde_json::from_str(&build_openai_body_with_stream("gpt-5.2", &request, true))
            .expect("openai body should be valid json");

    assert_eq!(body["stream"], true);
}

#[test]
fn build_anthropic_body_can_request_streaming() {
    let request = ModelRequest {
        session_id: "session_test".to_string(),
        model: "claude-test".to_string(),
        messages: vec![Message::new(Role::User, "hello")],
        tools: Vec::new(),
        permission_mode: PermissionMode::Default,
    };

    let body: Value = serde_json::from_str(&build_anthropic_body_with_stream(
        "claude-test",
        &request,
        true,
    ))
    .expect("anthropic body should be valid json");

    assert_eq!(body["stream"], true);
}

#[test]
fn build_openai_body_renders_tool_call_turns_with_null_content() {
    let request = ModelRequest {
        session_id: "session_test".to_string(),
        model: "deepseek-v4-flash".to_string(),
        messages: vec![
            Message::new(Role::User, "create a file"),
            Message {
                id: "msg_tool_call".to_string(),
                role: Role::Assistant,
                content: "path=hello_world.py content=print('Hello')".to_string(),
                timestamp: 1,
                tool_name: Some("write_file".to_string()),
                tool_call_id: Some("call_123".to_string()),
            },
            Message {
                id: "msg_tool_result".to_string(),
                role: Role::Tool,
                content: "Wrote hello_world.py".to_string(),
                timestamp: 2,
                tool_name: Some("write_file".to_string()),
                tool_call_id: Some("call_123".to_string()),
            },
        ],
        tools: vec![ToolSpec {
            name: "write_file".to_string(),
            description: "Write a file".to_string(),
            is_mutating: true,
            input_schema_hint: "path=file content=text".to_string(),
        }],
        permission_mode: PermissionMode::Default,
    };

    let body: Value = serde_json::from_str(&build_openai_body_with_stream(
        "deepseek-v4-flash",
        &request,
        false,
    ))
    .expect("openai body should be valid json");
    let messages = body["messages"].as_array().unwrap();
    let assistant = messages
        .iter()
        .find(|message| message["tool_calls"][0]["id"] == "call_123")
        .unwrap();
    assert!(assistant["content"].is_null());
    let tool_index = messages
        .iter()
        .position(|message| message["role"] == "tool")
        .unwrap();
    assert_eq!(messages[tool_index]["tool_call_id"], "call_123");
    assert_eq!(messages[tool_index - 1]["tool_calls"][0]["id"], "call_123");
}

#[test]
fn build_openai_body_replays_reasoning_content_without_tool_argument_leak() {
    let request = ModelRequest {
        session_id: "session_test".to_string(),
        model: "deepseek-v4-flash".to_string(),
        messages: vec![Message {
            id: "msg_tool_call".to_string(),
            role: Role::Assistant,
            content: format!(
                "path=hello_world.py\tcontent=print('Hello')\t{PROVIDER_REASONING_CONTENT_KEY}=need a file"
            ),
            timestamp: 1,
            tool_name: Some("write_file".to_string()),
            tool_call_id: Some("call_123".to_string()),
        }],
        tools: Vec::new(),
        permission_mode: PermissionMode::Default,
    };

    let body: Value = serde_json::from_str(&build_openai_body_with_stream(
        "deepseek-v4-flash",
        &request,
        false,
    ))
    .expect("openai body should be valid json");
    let assistant = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["tool_calls"][0]["id"] == "call_123")
        .unwrap();
    assert_eq!(assistant["reasoning_content"], "need a file");
    let arguments = assistant["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .unwrap();
    assert!(!arguments.contains(PROVIDER_REASONING_CONTENT_KEY));
}
