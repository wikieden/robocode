use super::*;

#[test]
fn openai_response_parser_extracts_content() {
    let response = r#"{"choices":[{"message":{"role":"assistant","content":"hello world"}}],"usage":{"prompt_tokens":11,"completion_tokens":7,"total_tokens":18}}"#;
    let events = parse_openai_events(response).unwrap();
    assert!(matches!(
        &events[0],
        ModelEvent::AssistantText { content } if content == "hello world"
    ));
    assert!(matches!(
        events.last(),
        Some(ModelEvent::Usage(usage))
            if usage.input_tokens == Some(11)
                && usage.output_tokens == Some(7)
                && usage.total_tokens == Some(18)
    ));
}

#[test]
fn openai_response_parser_extracts_cached_input_tokens_and_explicit_cost() {
    let response = r#"{
        "choices":[{"message":{"role":"assistant","content":"cached"}}],
        "usage":{
            "prompt_tokens":100,
            "completion_tokens":40,
            "total_tokens":140,
            "prompt_tokens_details":{"cached_tokens":75},
            "cost_micro_usd":1234
        }
    }"#;
    let events = parse_openai_events(response).unwrap();

    assert!(matches!(
        events.last(),
        Some(ModelEvent::Usage(usage))
            if usage.input_tokens == Some(100)
                && usage.output_tokens == Some(40)
                && usage.total_tokens == Some(140)
                && usage.cached_input_tokens == Some(75)
                && usage.actual_cost_micro_usd == Some(1234)
                && usage.cost_micro_usd.is_none()
    ));
}

#[test]
fn openai_response_parser_extracts_cached_token_provider_variants_without_pricing() {
    let response = r#"{
        "choices":[{"message":{"role":"assistant","content":"variant"}}],
        "usage":{
            "input_tokens":55,
            "output_tokens":11,
            "input_token_details":{"cache_read":22},
            "provider_cost":{"currency":"USD","micro_units":999}
        }
    }"#;
    let events = parse_openai_events(response).unwrap();

    assert!(matches!(
        events.last(),
        Some(ModelEvent::Usage(usage))
            if usage.input_tokens == Some(55)
                && usage.output_tokens == Some(11)
                && usage.total_tokens == Some(66)
                && usage.cached_input_tokens == Some(22)
                && usage.actual_cost_micro_usd == Some(999)
                && usage.cost_micro_usd.is_none()
    ));
}

#[test]
fn openai_response_parser_saturates_derived_total_tokens_on_overflow() {
    let response = format!(
        r#"{{
        "choices":[{{"message":{{"role":"assistant","content":"overflow"}}}}],
        "usage":{{
            "input_tokens":{},
            "output_tokens":1,
            "input_tokens_details":{{"cached_tokens":{}}}
        }}
    }}"#,
        u64::MAX,
        u64::MAX
    );
    let events = parse_openai_events(&response).unwrap();

    assert!(matches!(
        events.last(),
        Some(ModelEvent::Usage(usage))
            if usage.input_tokens == Some(u64::MAX)
                && usage.output_tokens == Some(1)
                && usage.cached_input_tokens == Some(u64::MAX)
                && usage.total_tokens == Some(u64::MAX)
    ));
}

#[test]
fn openai_response_parser_preserves_explicit_total_tokens_when_derived_would_overflow() {
    let response = format!(
        r#"{{
        "choices":[{{"message":{{"role":"assistant","content":"explicit"}}}}],
        "usage":{{
            "prompt_tokens":{},
            "completion_tokens":1,
            "total_tokens":42
        }}
    }}"#,
        u64::MAX
    );
    let events = parse_openai_events(&response).unwrap();

    assert!(matches!(
        events.last(),
        Some(ModelEvent::Usage(usage))
            if usage.input_tokens == Some(u64::MAX)
                && usage.output_tokens == Some(1)
                && usage.total_tokens == Some(42)
    ));
}

#[test]
fn openai_response_parser_leaves_absent_usage_parts_unknown() {
    let response = r#"{
        "choices":[{"message":{"role":"assistant","content":"partial"}}],
        "usage":{"completion_tokens":9}
    }"#;
    let events = parse_openai_events(response).unwrap();

    assert!(matches!(
        events.last(),
        Some(ModelEvent::Usage(usage))
            if usage.input_tokens.is_none()
                && usage.output_tokens == Some(9)
                && usage.total_tokens.is_none()
                && usage.cached_input_tokens.is_none()
                && usage.actual_cost_micro_usd.is_none()
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
    let response = r#"{"content":[{"type":"tool_use","id":"toolu_1","name":"grep","input":{"pattern":"main","path":"src"}}],"usage":{"input_tokens":21,"output_tokens":4}}"#;
    let events = parse_anthropic_events(response).unwrap();
    assert!(matches!(
        &events[0],
        ModelEvent::ToolCall(call)
            if call.id == "toolu_1"
                && call.name == "grep"
                && call.input.get("pattern").map(String::as_str) == Some("main")
    ));
    assert!(matches!(
        events.last(),
        Some(ModelEvent::Usage(usage))
            if usage.input_tokens == Some(21)
                && usage.output_tokens == Some(4)
                && usage.total_tokens == Some(25)
    ));
}

#[test]
fn anthropic_response_parser_saturates_derived_total_tokens_on_overflow() {
    let response = format!(
        r#"{{
        "content":[{{"type":"text","text":"overflow"}}],
        "usage":{{"input_tokens":{},"output_tokens":1,"cache_read_input_tokens":{}}}
    }}"#,
        u64::MAX,
        u64::MAX
    );
    let events = parse_anthropic_events(&response).unwrap();

    assert!(matches!(
        events.last(),
        Some(ModelEvent::Usage(usage))
            if usage.input_tokens == Some(u64::MAX)
                && usage.output_tokens == Some(1)
                && usage.cached_input_tokens == Some(u64::MAX)
                && usage.total_tokens == Some(u64::MAX)
    ));
}

#[test]
fn ollama_response_parser_saturates_derived_total_tokens_on_overflow() {
    let response = format!(
        r#"{{
        "message":{{"role":"assistant","content":"overflow"}},
        "prompt_eval_count":{},
        "eval_count":1
    }}"#,
        u64::MAX
    );
    let events = parse_ollama_events(&response).unwrap();

    assert!(matches!(
        events.last(),
        Some(ModelEvent::Usage(usage))
            if usage.input_tokens == Some(u64::MAX)
                && usage.output_tokens == Some(1)
                && usage.total_tokens == Some(u64::MAX)
    ));
}

#[test]
fn openai_stream_parser_joins_text_deltas() {
    let response = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"hello \"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n\n",
        "data: [DONE]\n\n",
    );
    let events = parse_openai_stream_events(response).unwrap();
    assert!(matches!(
        &events[0],
        ModelEvent::AssistantText { content } if content == "hello world"
    ));
}

#[test]
fn openai_stream_parser_reassembles_tool_calls_and_reasoning() {
    let response = concat!(
        "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"need file\",\"tool_calls\":[{\"index\":0,\"id\":\"call_123\",\"type\":\"function\",\"function\":{\"name\":\"write_file\",\"arguments\":\"{\\\"path\\\":\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"hello.py\\\",\\\"content\\\":\\\"print(1)\\\"}\"}}]}}]}\n\n",
        "data: [DONE]\n\n",
    );
    let events = parse_openai_stream_events(response).unwrap();
    assert!(matches!(
        &events[0],
        ModelEvent::ToolCall(call)
            if call.id == "call_123"
                && call.name == "write_file"
                && call.input.get("path").map(String::as_str) == Some("hello.py")
                && call.input.get(PROVIDER_REASONING_CONTENT_KEY).map(String::as_str)
                    == Some("need file")
    ));
}

#[test]
fn anthropic_stream_parser_joins_text_deltas() {
    let response = concat!(
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hello \"}}\n\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"world\"}}\n\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let events = parse_anthropic_stream_events(response).unwrap();
    assert!(matches!(
        &events[0],
        ModelEvent::AssistantText { content } if content == "hello world"
    ));
}

#[test]
fn anthropic_stream_parser_reassembles_tool_use() {
    let response = concat!(
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"read_file\",\"input\":{}}}\n\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\n\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"Cargo.toml\\\"}\"}}\n\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let events = parse_anthropic_stream_events(response).unwrap();
    assert!(matches!(
        &events[0],
        ModelEvent::ToolCall(call)
            if call.id == "toolu_1"
                && call.name == "read_file"
                && call.input.get("path").map(String::as_str) == Some("Cargo.toml")
    ));
}
