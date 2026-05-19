use super::*;

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
fn provider_request_control_cancels_before_dispatch() {
    let mut provider = create_provider(ProviderConfig {
        kind: ProviderKind::OpenAi,
        model: "gpt-5.2".to_string(),
        api_base: Some("https://api.openai.com".to_string()),
        api_key: None,
        request_timeout_secs: 90,
        max_retries: 1,
    });
    let control = ModelRequestControl::new();
    control.cancel();

    let err = provider
        .next_events_with_control(
            &ModelRequest {
                session_id: "session_test".to_string(),
                model: provider.model().to_string(),
                messages: vec![Message::new(Role::User, "hello")],
                tools: Vec::new(),
                permission_mode: PermissionMode::Default,
            },
            &control,
        )
        .unwrap_err();

    assert_eq!(err, "Model request cancelled");
}
