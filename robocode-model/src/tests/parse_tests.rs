use super::*;

#[test]
fn openai_response_parser_extracts_content() {
    let response = r#"{"choices":[{"message":{"role":"assistant","content":"hello world"}}]}"#;
    let events = parse_openai_events(response).unwrap();
    assert!(matches!(
        &events[0],
        ModelEvent::AssistantText { content } if content == "hello world"
    ));
}

#[test]
fn openai_response_parser_extracts_tool_calls() {
    let response = r#"{"choices":[{"message":{"role":"assistant","tool_calls":[{"id":"call_123","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"Cargo.toml\",\"max_bytes\":\"1024\"}"}}]}}]}"#;
    let events = parse_openai_events(response).unwrap();
    assert!(matches!(
        &events[0],
        ModelEvent::ToolCall(call)
            if call.id == "call_123"
                && call.name == "read_file"
                && call.input.get("path").map(String::as_str) == Some("Cargo.toml")
    ));
}

#[test]
fn openai_response_parser_preserves_reasoning_content_for_tool_calls() {
    let response = r#"{"choices":[{"message":{"role":"assistant","reasoning_content":"need to create the requested file","tool_calls":[{"id":"call_123","type":"function","function":{"name":"write_file","arguments":"{\"path\":\"hello_world.py\",\"content\":\"print(\\\"Hello, world!\\\")\"}"}}]}}]}"#;
    let events = parse_openai_events(response).unwrap();
    assert!(matches!(
        &events[0],
        ModelEvent::ToolCall(call)
            if call.input.get(PROVIDER_REASONING_CONTENT_KEY).map(String::as_str)
                == Some("need to create the requested file")
    ));
}

#[test]
fn anthropic_response_parser_extracts_tool_use() {
    let response = r#"{"content":[{"type":"tool_use","id":"toolu_1","name":"grep","input":{"pattern":"main","path":"src"}}]}"#;
    let events = parse_anthropic_events(response).unwrap();
    assert!(matches!(
        &events[0],
        ModelEvent::ToolCall(call)
            if call.id == "toolu_1"
                && call.name == "grep"
                && call.input.get("pattern").map(String::as_str) == Some("main")
    ));
}
