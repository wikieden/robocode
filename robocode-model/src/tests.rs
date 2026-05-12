use super::*;
use crate::adapters::{builtin_default_api_base, builtin_provider_id};
use crate::fallback::parse_explicit_tool_call;
use crate::parse::{parse_anthropic_events, parse_openai_events};
use crate::render::build_openai_body;
use crate::transport::split_response_and_status;
use robocode_types::{Message, ModelRequest, PermissionMode, Role, ToolSpec};
use serde_json::Value;

#[test]
fn config_overrides_provider_and_model() {
    let config = ProviderConfig::from_env()
        .with_overrides(
            Some("openai-compatible"),
            Some("deepseek-chat"),
            Some("https://api.example.com"),
            Some("secret"),
        )
        .unwrap();
    assert_eq!(config.kind, ProviderKind::OpenAiCompatible);
    assert_eq!(config.model, "deepseek-chat");
    assert_eq!(config.api_base.as_deref(), Some("https://api.example.com"));
    assert_eq!(config.api_key.as_deref(), Some("secret"));
}

#[test]
fn from_settings_applies_timeout_and_retries() {
    let config = ProviderConfig::from_settings(
        "openai",
        Some("gpt-5.2"),
        Some("https://api.openai.com"),
        Some("secret"),
        120,
        3,
    )
    .unwrap();
    assert_eq!(config.request_timeout_secs, 120);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn explicit_tool_syntax_still_creates_tool_calls() {
    let call = parse_explicit_tool_call("tool read_file path=Cargo.toml").unwrap();
    assert_eq!(call.name, "read_file");
    assert_eq!(
        call.input.get("path").map(String::as_str),
        Some("Cargo.toml")
    );
}

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
    let body = build_openai_body("gpt-5.2", &request);
    assert!(body.contains("\"tools\""));
    assert!(body.contains("\"read_file\""));
    assert!(body.contains("\"path\""));
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

    let body: Value = serde_json::from_str(&build_openai_body("deepseek-v4-flash", &request))
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

    let body: Value = serde_json::from_str(&build_openai_body("deepseek-v4-flash", &request))
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

#[test]
fn provider_without_key_falls_back_cleanly() {
    let mut provider = create_provider(ProviderConfig {
        kind: ProviderKind::OpenAi,
        model: "gpt-5.2".to_string(),
        api_base: Some("https://api.openai.com".to_string()),
        api_key: None,
        request_timeout_secs: 90,
        max_retries: 1,
    });
    let events = provider
        .next_events(&ModelRequest {
            session_id: "session_test".to_string(),
            model: provider.model().to_string(),
            messages: vec![Message::new(Role::User, "hello")],
            tools: vec![ToolSpec {
                name: "read_file".to_string(),
                description: "Read".to_string(),
                is_mutating: false,
                input_schema_hint: String::new(),
            }],
            permission_mode: PermissionMode::Default,
        })
        .unwrap();
    assert!(
        matches!(&events[0], ModelEvent::AssistantText { content } if content.contains("fallback mode"))
    );
}

#[test]
fn registry_lists_builtin_provider_ids() {
    let registry = ProviderRegistry::with_builtins();
    let ids = registry.provider_ids();
    assert!(ids.contains(&"anthropic".to_string()));
    assert!(ids.contains(&"deepseek".to_string()));
    assert!(ids.contains(&"openai".to_string()));
    assert!(ids.contains(&"fallback".to_string()));
}

#[test]
fn provider_kind_parse_roundtrips_builtin_provider_ids() {
    for provider_id in list_supported_provider_strings() {
        let kind = ProviderKind::parse(&provider_id)
            .expect("every builtin provider id should parse through shared metadata");
        assert_eq!(builtin_provider_id(kind), provider_id);
    }
}

#[test]
fn supported_provider_strings_match_builtin_registry_ids() {
    let registry = ProviderRegistry::with_builtins();
    let mut ids = registry.provider_ids();
    let mut supported = list_supported_provider_strings();
    ids.sort();
    supported.sort();
    assert_eq!(supported, ids);
}

#[test]
fn descriptor_keeps_provider_identity_separate_from_protocol_family() {
    let descriptor = ProviderDescriptor {
        provider_id: "deepseek".to_string(),
        display_name: "DeepSeek".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://api.deepseek.com".to_string()),
        default_model: Some("deepseek-v4-flash".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    assert_eq!(descriptor.provider_id, "deepseek");
    assert_eq!(descriptor.protocol_family, ProtocolFamily::OpenAi);
}

#[test]
fn provider_descriptor_validation_rejects_invalid_plugin_identity() {
    let descriptor = ProviderDescriptor {
        provider_id: "../bad".to_string(),
        display_name: "Bad Plugin".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://example.com".to_string()),
        default_model: Some("bad-model".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    let err = descriptor::validate_provider_descriptor(&descriptor).unwrap_err();
    assert!(err.contains("provider_id"), "{err}");
}

#[test]
fn provider_descriptor_validation_rejects_unsupported_schema_version() {
    let descriptor = ProviderDescriptor {
        provider_id: "future-provider".to_string(),
        display_name: "Future Provider".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://example.com".to_string()),
        default_model: Some("future-model".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 99,
    };

    let err = descriptor::validate_provider_descriptor(&descriptor).unwrap_err();
    assert!(err.contains("config_schema_version"), "{err}");
}

#[test]
fn provider_descriptor_validation_accepts_builtin_deepseek_shape() {
    let registry = ProviderRegistry::with_builtins();
    let descriptor = registry.descriptor("deepseek").unwrap();
    descriptor::validate_provider_descriptor(descriptor).unwrap();
}

#[test]
fn registry_rejects_plugin_descriptor_that_conflicts_with_builtin_provider_id() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "openai".to_string(),
        display_name: "Conflicting OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://example.com".to_string()),
        default_model: Some("conflict-model".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    let err = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap_err();
    assert!(err.contains("conflicts"), "{err}");
}

#[test]
fn registry_accepts_valid_non_builtin_plugin_descriptor() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "custom-openai".to_string(),
        display_name: "Custom OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://models.example.com".to_string()),
        default_model: Some("custom-model".to_string()),
        env_mappings: ProviderEnvMappings {
            api_key_env: Some("CUSTOM_OPENAI_API_KEY".to_string()),
            api_base_env: Some("CUSTOM_OPENAI_API_BASE".to_string()),
        },
        capabilities: ProviderCapabilities {
            supports_streaming: true,
            supports_native_tool_calling: true,
        },
        config_schema_version: 1,
    };

    let registry = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap();
    assert!(registry.descriptor("custom-openai").is_some());
    assert!(
        registry
            .provider_ids()
            .contains(&"custom-openai".to_string())
    );
}

#[test]
fn builtin_openai_descriptor_matches_runtime_api_base_behavior() {
    let registry = ProviderRegistry::with_builtins();
    let descriptor = registry
        .descriptors()
        .iter()
        .find(|descriptor| descriptor.provider_id == "openai")
        .expect("openai descriptor should exist");

    assert_eq!(
        descriptor.default_api_base.as_deref(),
        builtin_default_api_base(ProviderKind::OpenAi)
    );
    assert_eq!(
        descriptor.env_mappings.api_base_env.as_deref(),
        Some("ROBOCODE_API_BASE")
    );
}

#[test]
fn registry_exposes_deepseek_as_independent_provider_id() {
    let registry = ProviderRegistry::with_builtins();
    assert!(registry.provider_ids().contains(&"deepseek".to_string()));
}

#[test]
fn deepseek_provider_uses_openai_protocol_family() {
    let registry = ProviderRegistry::with_builtins();
    let descriptor = registry.descriptor("deepseek").unwrap();
    assert_eq!(descriptor.provider_id, "deepseek");
    assert_eq!(descriptor.protocol_family, ProtocolFamily::OpenAi);
}

#[test]
fn deepseek_anthropic_provider_uses_official_anthropic_endpoint() {
    let registry = ProviderRegistry::with_builtins();
    let descriptor = registry.descriptor("deepseek-anthropic").unwrap();
    assert_eq!(descriptor.provider_id, "deepseek-anthropic");
    assert_eq!(descriptor.protocol_family, ProtocolFamily::Anthropic);
    assert_eq!(
        descriptor.default_api_base.as_deref(),
        Some("https://api.deepseek.com/anthropic")
    );
    assert_eq!(
        descriptor.default_model.as_deref(),
        Some("deepseek-v4-flash")
    );
}

#[test]
fn provider_host_can_refresh_registry_without_replacing_existing_provider_instance() {
    let mut host = ProviderHost::with_builtins();
    let before_registry = host.registry();
    let mut provider = host
        .create(
            ProviderConfig::from_settings(
                "openai-compatible",
                Some("deepseek-chat"),
                None,
                None,
                90,
                1,
            )
            .unwrap(),
        )
        .unwrap();

    host.refresh().unwrap();
    let after_registry = host.registry();

    let mut before_ids = before_registry.provider_ids();
    let mut after_ids = after_registry.provider_ids();
    before_ids.sort();
    after_ids.sort();

    assert!(!std::sync::Arc::ptr_eq(&before_registry, &after_registry));
    assert_eq!(after_ids, before_ids);
    assert!(
        after_registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.provider_id == "openai-compatible")
    );
    assert_eq!(provider.provider_name(), "openai-compatible");
    provider.set_model("deepseek-v4-pro".to_string());
    assert_eq!(provider.model(), "deepseek-v4-pro");
}

#[test]
fn provider_host_creates_independent_provider_instances_per_engine() {
    let host = ProviderHost::with_builtins();
    let mut first = host
        .create(
            ProviderConfig::from_settings(
                "openai-compatible",
                Some("deepseek-chat"),
                None,
                None,
                90,
                1,
            )
            .unwrap(),
        )
        .unwrap();
    let second = host
        .create(
            ProviderConfig::from_settings(
                "openai-compatible",
                Some("deepseek-chat"),
                None,
                None,
                90,
                1,
            )
            .unwrap(),
        )
        .unwrap();

    first.set_model("deepseek-v4-pro".to_string());

    assert_eq!(first.provider_name(), "openai-compatible");
    assert_eq!(first.model(), "deepseek-v4-pro");
    assert_eq!(second.provider_name(), "openai-compatible");
    assert_eq!(second.model(), "deepseek-chat");
}

#[test]
fn provider_host_creates_dynamic_openai_provider_from_registry_descriptor() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "custom-openai".to_string(),
        display_name: "Custom OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://models.example.com".to_string()),
        default_model: Some("custom-model".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities {
            supports_streaming: true,
            supports_native_tool_calling: true,
        },
        config_schema_version: 1,
    };
    let registry = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap();
    let host = ProviderHost::with_registry(registry);

    let provider = host
        .create_registered("custom-openai", None, None, None, 90, 1)
        .unwrap();

    assert_eq!(provider.provider_name(), "custom-openai");
    assert_eq!(provider.model(), "custom-model");
}

#[test]
fn provider_host_keeps_dynamic_provider_instances_independent() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "team-provider".to_string(),
        display_name: "Team Provider".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::Anthropic,
        default_api_base: Some("https://team.example.com".to_string()),
        default_model: Some("team-default".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities {
            supports_streaming: true,
            supports_native_tool_calling: true,
        },
        config_schema_version: 1,
    };
    let registry = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap();
    let host = ProviderHost::with_registry(registry);
    let mut first = host
        .create_registered("team-provider", Some("agent-a-model"), None, None, 90, 1)
        .unwrap();
    let second = host
        .create_registered("team-provider", Some("agent-b-model"), None, None, 90, 1)
        .unwrap();

    first.set_model("agent-a-updated".to_string());

    assert_eq!(first.provider_name(), "team-provider");
    assert_eq!(first.model(), "agent-a-updated");
    assert_eq!(second.provider_name(), "team-provider");
    assert_eq!(second.model(), "agent-b-model");
}

#[test]
fn split_response_and_status_parses_curl_suffix() {
    let response = split_response_and_status("{\"ok\":true}\n200").unwrap();
    assert_eq!(response.0, "{\"ok\":true}");
    assert_eq!(response.1, 200);
}
