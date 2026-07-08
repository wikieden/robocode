use super::*;
use crate::providers::provider_streaming_requested;

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
        work_mode: WorkMode::Build,
        permission_mode: PermissionMode::Default,
        permission_level: PermissionLevel::Ask,
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
        work_mode: WorkMode::Build,
        permission_mode: PermissionMode::Default,
        permission_level: PermissionLevel::Ask,
    };

    let body: Value =
        serde_json::from_str(&build_openai_body_with_stream("gpt-5.2", &request, true))
            .expect("openai body should be valid json");

    assert_eq!(body["stream"], true);
}

#[test]
fn build_openai_body_in_plan_mode_uses_planner_prompt_and_hides_mutating_tools() {
    let request = ModelRequest {
        session_id: "session_test".to_string(),
        model: "gpt-5.2".to_string(),
        messages: vec![Message::new(Role::User, "规划这个功能怎么做")],
        tools: vec![
            ToolSpec {
                name: "read_file".to_string(),
                description: "Read a file".to_string(),
                is_mutating: false,
                input_schema_hint: "path=file max_bytes=8192".to_string(),
            },
            ToolSpec {
                name: "write_file".to_string(),
                description: "Write a file".to_string(),
                is_mutating: true,
                input_schema_hint: "path=file content=text".to_string(),
            },
        ],
        work_mode: WorkMode::Plan,
        permission_mode: PermissionMode::Plan,
        permission_level: PermissionLevel::ReadOnly,
    };

    let body: Value =
        serde_json::from_str(&build_openai_body_with_stream("gpt-5.2", &request, false))
            .expect("openai body should be valid json");
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("Plan mode"));
    assert!(system.contains("requirements, architecture, implementation approach"));
    assert!(system.contains("Do not write code"));
    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["function"]["name"], "read_file");
}

#[test]
fn build_anthropic_body_can_request_streaming() {
    let request = ModelRequest {
        session_id: "session_test".to_string(),
        model: "claude-test".to_string(),
        messages: vec![Message::new(Role::User, "hello")],
        tools: Vec::new(),
        work_mode: WorkMode::Build,
        permission_mode: PermissionMode::Default,
        permission_level: PermissionLevel::Ask,
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
fn build_anthropic_body_in_plan_mode_uses_planner_prompt_and_hides_mutating_tools() {
    let request = ModelRequest {
        session_id: "session_test".to_string(),
        model: "claude-test".to_string(),
        messages: vec![Message::new(Role::User, "plan the architecture")],
        tools: vec![
            ToolSpec {
                name: "read_file".to_string(),
                description: "Read a file".to_string(),
                is_mutating: false,
                input_schema_hint: "path=file max_bytes=8192".to_string(),
            },
            ToolSpec {
                name: "edit_file".to_string(),
                description: "Edit a file".to_string(),
                is_mutating: true,
                input_schema_hint: "path=file old=text new=text".to_string(),
            },
        ],
        work_mode: WorkMode::Plan,
        permission_mode: PermissionMode::Plan,
        permission_level: PermissionLevel::ReadOnly,
    };

    let body: Value = serde_json::from_str(&build_anthropic_body_with_stream(
        "claude-test",
        &request,
        false,
    ))
    .expect("anthropic body should be valid json");
    let system = body["system"].as_str().unwrap();
    assert!(system.contains("Plan mode"));
    assert!(system.contains("Do not write code"));
    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "read_file");
}

#[test]
fn provider_streaming_preference_is_gated_by_capability() {
    assert!(provider_streaming_requested(true, true));
    assert!(!provider_streaming_requested(true, false));
    assert!(!provider_streaming_requested(false, true));
    assert!(!provider_streaming_requested(false, false));
}

#[test]
fn build_openai_body_renders_generic_tool_call_turns_with_null_content() {
    let request = ModelRequest {
        session_id: "session_test".to_string(),
        model: "gpt-5.2".to_string(),
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
        work_mode: WorkMode::Build,
        permission_mode: PermissionMode::Default,
        permission_level: PermissionLevel::Ask,
    };

    let body: Value =
        serde_json::from_str(&build_openai_body_with_stream("gpt-5.2", &request, false))
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
fn build_openai_body_can_render_deepseek_v4_tool_call_turns_with_empty_content() {
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
        tools: vec![ToolSpec {
            name: "write_file".to_string(),
            description: "Write a file".to_string(),
            is_mutating: true,
            input_schema_hint: "path=file content=text".to_string(),
        }],
        work_mode: WorkMode::Build,
        permission_mode: PermissionMode::Default,
        permission_level: PermissionLevel::Ask,
    };

    let body: Value = serde_json::from_str(&build_openai_body_with_stream_and_compat(
        "deepseek-v4-flash",
        &request,
        false,
        OpenAiRenderCompatibility {
            requires_non_null_tool_call_content: true,
        },
    ))
    .expect("openai body should be valid json");
    let assistant = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["tool_calls"][0]["id"] == "call_123")
        .unwrap();

    assert_eq!(assistant["content"], "");
    assert_eq!(assistant["reasoning_content"], "need a file");
    assert!(body.get("tool_choice").is_none());
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
        work_mode: WorkMode::Build,
        permission_mode: PermissionMode::Default,
        permission_level: PermissionLevel::Ask,
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
