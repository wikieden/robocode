mod adapters;
mod config;
mod descriptor;
mod fallback;
mod host;
mod http;
mod parse;
mod plugin;
mod registry;
mod render;
mod transport;

use adapters::{builtin_default_api_base, builtin_provider_id};
use config::resolve_api_key;
use fallback::{fallback_events, parse_explicit_tool_call_from_messages};
use parse::{
    extract_error_message, parse_anthropic_events, parse_ollama_events, parse_openai_events,
};
use render::{build_anthropic_body, build_ollama_body, build_openai_body};
use robocode_types::{ModelEvent, ModelRequest};
use transport::post_json;

pub use config::{ProviderConfig, ProviderKind};
pub use descriptor::{
    ProtocolFamily, ProviderCapabilities, ProviderDescriptor, ProviderEnvMappings,
};
pub use host::ProviderHost;
pub use registry::ProviderRegistry;

const PROVIDER_REASONING_CONTENT_KEY: &str = "__provider_reasoning_content";

pub trait ModelProvider: Send {
    fn provider_name(&self) -> &str;
    fn model(&self) -> &str;
    fn set_model(&mut self, model: String);
    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String>;
}

pub fn create_provider(config: ProviderConfig) -> Box<dyn ModelProvider> {
    ProviderHost::with_builtins()
        .create(config)
        .expect("builtin provider construction should succeed")
}

pub fn list_supported_provider_strings() -> Vec<String> {
    ProviderRegistry::with_builtins().creatable_provider_ids()
}

#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    inner: HttpProvider,
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self {
            inner: HttpProvider::anthropic(ProviderConfig::from_env()),
        }
    }
}

impl AnthropicProvider {
    pub fn new(model: impl Into<String>) -> Self {
        let config = ProviderConfig {
            kind: ProviderKind::Anthropic,
            model: model.into(),
            api_base: None,
            api_key: resolve_api_key(ProviderKind::Anthropic),
            request_timeout_secs: 90,
            max_retries: 1,
        };
        Self {
            inner: HttpProvider::anthropic(config),
        }
    }
}

impl ModelProvider for AnthropicProvider {
    fn provider_name(&self) -> &str {
        self.inner.provider_name()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }

    fn set_model(&mut self, model: String) {
        self.inner.set_model(model);
    }

    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        self.inner.next_events(request)
    }
}

#[derive(Debug, Clone)]
struct FallbackProvider {
    provider_name: String,
    model: String,
}

impl FallbackProvider {
    fn from_config(config: ProviderConfig) -> Self {
        Self {
            provider_name: builtin_provider_id(config.kind).to_string(),
            model: config.model,
        }
    }
}

impl ModelProvider for FallbackProvider {
    fn provider_name(&self) -> &str {
        &self.provider_name
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: String) {
        self.model = model;
    }

    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        Ok(fallback_events(
            self.provider_name(),
            self.model(),
            request.messages.last(),
        ))
    }
}

#[derive(Debug, Clone)]
struct HttpProvider {
    provider_name: String,
    mode: HttpMode,
    model: String,
    api_base: String,
    api_key: Option<String>,
    request_timeout_secs: u64,
    max_retries: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpMode {
    Anthropic,
    OpenAiCompatible,
    Ollama,
}

impl HttpProvider {
    fn from_descriptor(
        descriptor: &ProviderDescriptor,
        model: Option<&str>,
        api_base: Option<&str>,
        api_key: Option<&str>,
        request_timeout_secs: u64,
        max_retries: u32,
    ) -> Result<Self, String> {
        let mode = match descriptor.protocol_family {
            ProtocolFamily::Anthropic => HttpMode::Anthropic,
            ProtocolFamily::OpenAi => HttpMode::OpenAiCompatible,
        };
        let model = model
            .filter(|value| !value.trim().is_empty())
            .or(descriptor.default_model.as_deref())
            .ok_or_else(|| {
                format!(
                    "Provider `{}` does not define a default model; pass a model explicitly",
                    descriptor.provider_id
                )
            })?;
        let api_base = api_base
            .filter(|value| !value.trim().is_empty())
            .or(descriptor.default_api_base.as_deref())
            .ok_or_else(|| {
                format!(
                    "Provider `{}` does not define a default API base; pass an API base explicitly",
                    descriptor.provider_id
                )
            })?;
        let api_key = api_key
            .map(ToString::to_string)
            .or_else(|| resolve_env_mapping(descriptor.env_mappings.api_key_env.as_deref()));
        Ok(Self {
            provider_name: descriptor.provider_id.clone(),
            mode,
            model: model.to_string(),
            api_base: api_base.to_string(),
            api_key,
            request_timeout_secs: request_timeout_secs.max(1),
            max_retries,
        })
    }

    fn anthropic(config: ProviderConfig) -> Self {
        Self {
            provider_name: builtin_provider_id(ProviderKind::Anthropic).to_string(),
            mode: HttpMode::Anthropic,
            model: config.model,
            api_base: config.api_base.unwrap_or_else(|| {
                builtin_default_api_base(ProviderKind::Anthropic)
                    .expect("anthropic builtin API base should exist")
                    .to_string()
            }),
            api_key: config.api_key,
            request_timeout_secs: config.request_timeout_secs,
            max_retries: config.max_retries,
        }
    }

    fn openai(config: ProviderConfig) -> Self {
        Self {
            provider_name: builtin_provider_id(ProviderKind::OpenAi).to_string(),
            mode: HttpMode::OpenAiCompatible,
            model: config.model,
            api_base: config.api_base.unwrap_or_else(|| {
                builtin_default_api_base(ProviderKind::OpenAi)
                    .expect("openai builtin API base should exist")
                    .to_string()
            }),
            api_key: config
                .api_key
                .or_else(|| resolve_api_key(ProviderKind::OpenAi)),
            request_timeout_secs: config.request_timeout_secs,
            max_retries: config.max_retries,
        }
    }

    fn openai_compatible(config: ProviderConfig) -> Self {
        Self {
            provider_name: builtin_provider_id(ProviderKind::OpenAiCompatible).to_string(),
            mode: HttpMode::OpenAiCompatible,
            model: config.model,
            api_base: config.api_base.unwrap_or_else(|| {
                builtin_default_api_base(ProviderKind::OpenAiCompatible)
                    .expect("openai-compatible builtin API base should exist")
                    .to_string()
            }),
            api_key: config.api_key,
            request_timeout_secs: config.request_timeout_secs,
            max_retries: config.max_retries,
        }
    }

    fn deepseek(config: ProviderConfig) -> Self {
        Self {
            provider_name: builtin_provider_id(ProviderKind::DeepSeek).to_string(),
            mode: HttpMode::OpenAiCompatible,
            model: config.model,
            api_base: config.api_base.unwrap_or_else(|| {
                builtin_default_api_base(ProviderKind::DeepSeek)
                    .expect("deepseek builtin API base should exist")
                    .to_string()
            }),
            api_key: config
                .api_key
                .or_else(|| resolve_api_key(ProviderKind::DeepSeek)),
            request_timeout_secs: config.request_timeout_secs,
            max_retries: config.max_retries,
        }
    }

    fn deepseek_anthropic(config: ProviderConfig) -> Self {
        Self {
            provider_name: builtin_provider_id(ProviderKind::DeepSeekAnthropic).to_string(),
            mode: HttpMode::Anthropic,
            model: config.model,
            api_base: config.api_base.unwrap_or_else(|| {
                builtin_default_api_base(ProviderKind::DeepSeekAnthropic)
                    .expect("deepseek anthropic-compatible builtin API base should exist")
                    .to_string()
            }),
            api_key: config
                .api_key
                .or_else(|| resolve_api_key(ProviderKind::DeepSeekAnthropic)),
            request_timeout_secs: config.request_timeout_secs,
            max_retries: config.max_retries,
        }
    }

    fn ollama(config: ProviderConfig) -> Self {
        Self {
            provider_name: builtin_provider_id(ProviderKind::Ollama).to_string(),
            mode: HttpMode::Ollama,
            model: config.model,
            api_base: config.api_base.unwrap_or_else(|| {
                builtin_default_api_base(ProviderKind::Ollama)
                    .expect("ollama builtin API base should exist")
                    .to_string()
            }),
            api_key: config.api_key,
            request_timeout_secs: config.request_timeout_secs,
            max_retries: config.max_retries,
        }
    }
}

impl ModelProvider for HttpProvider {
    fn provider_name(&self) -> &str {
        &self.provider_name
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: String) {
        self.model = model;
    }

    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        if let Some(tool_call) = parse_explicit_tool_call_from_messages(&request.messages) {
            return Ok(vec![ModelEvent::ToolCall(tool_call), ModelEvent::Done]);
        }

        if self.mode != HttpMode::Ollama && self.api_key.is_none() {
            return Ok(fallback_events(
                self.provider_name(),
                self.model(),
                request.messages.last(),
            ));
        }

        let body = match self.mode {
            HttpMode::Anthropic => build_anthropic_body(&self.model, request),
            HttpMode::OpenAiCompatible => build_openai_body(&self.model, request),
            HttpMode::Ollama => build_ollama_body(&self.model, request),
        };
        let path = match self.mode {
            HttpMode::Anthropic => "/v1/messages",
            HttpMode::OpenAiCompatible => "/v1/chat/completions",
            HttpMode::Ollama => "/api/chat",
        };
        let mut headers = vec!["Content-Type: application/json".to_string()];
        match self.mode {
            HttpMode::Anthropic => {
                headers.push(format!(
                    "x-api-key: {}",
                    self.api_key.clone().unwrap_or_default()
                ));
                headers.push("anthropic-version: 2023-06-01".to_string());
            }
            HttpMode::OpenAiCompatible => {
                if let Some(api_key) = &self.api_key {
                    headers.push(format!("Authorization: Bearer {api_key}"));
                }
            }
            HttpMode::Ollama => {}
        }
        let response = post_json(
            &self.api_base,
            path,
            &headers,
            &body,
            self.request_timeout_secs,
            self.max_retries,
        )?;
        if response.status_code >= 400 {
            let message = extract_error_message(&response.body).unwrap_or_else(|| {
                format!(
                    "{} returned HTTP {}",
                    self.provider_name(),
                    response.status_code
                )
            });
            return Err(format!("API error ({}): {}", response.status_code, message));
        }
        let mut events = match self.mode {
            HttpMode::Anthropic => parse_anthropic_events(&response.body),
            HttpMode::OpenAiCompatible => parse_openai_events(&response.body),
            HttpMode::Ollama => parse_ollama_events(&response.body),
        }
        .unwrap_or_else(|| {
            vec![ModelEvent::AssistantText {
                content: extract_error_message(&response.body)
                    .map(|message| format!("API error: {message}"))
                    .unwrap_or_else(|| {
                        format!(
                            "{} returned a response, but RoboCode could not parse assistant content.\n\nRaw response:\n{}",
                            self.provider_name(),
                            response.body
                        )
                    }),
            }]
        });
        events.push(ModelEvent::Done);
        Ok(events)
    }
}

pub(crate) fn load_builtin_provider(
    registry: &ProviderRegistry,
    config: ProviderConfig,
) -> Result<Box<dyn ModelProvider>, String> {
    let provider_id = builtin_provider_id(config.kind);
    if !registry
        .descriptors()
        .iter()
        .any(|descriptor| descriptor.provider_id == provider_id)
    {
        return Err(format!(
            "Provider `{provider_id}` is not registered in the active provider host"
        ));
    }

    let provider: Box<dyn ModelProvider> = match config.kind {
        ProviderKind::Anthropic => Box::new(HttpProvider::anthropic(config)),
        ProviderKind::DeepSeek => Box::new(HttpProvider::deepseek(config)),
        ProviderKind::DeepSeekAnthropic => Box::new(HttpProvider::deepseek_anthropic(config)),
        ProviderKind::OpenAi => Box::new(HttpProvider::openai(config)),
        ProviderKind::OpenAiCompatible => Box::new(HttpProvider::openai_compatible(config)),
        ProviderKind::Ollama => Box::new(HttpProvider::ollama(config)),
        ProviderKind::Fallback => Box::new(FallbackProvider::from_config(config)),
    };
    Ok(provider)
}

pub(crate) fn load_registered_provider(
    registry: &ProviderRegistry,
    provider_id: &str,
    model: Option<&str>,
    api_base: Option<&str>,
    api_key: Option<&str>,
    request_timeout_secs: u64,
    max_retries: u32,
) -> Result<Box<dyn ModelProvider>, String> {
    if let Some(kind) = ProviderKind::parse(provider_id) {
        return load_builtin_provider(
            registry,
            ProviderConfig {
                kind,
                model: model
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| adapters::builtin_default_model(kind))
                    .to_string(),
                api_base: api_base.map(ToString::to_string),
                api_key: api_key.map(ToString::to_string),
                request_timeout_secs: request_timeout_secs.max(1),
                max_retries,
            },
        );
    }
    let descriptor = registry
        .descriptor(provider_id)
        .ok_or_else(|| format!("Provider `{provider_id}` is not registered"))?;
    Ok(Box::new(HttpProvider::from_descriptor(
        descriptor,
        model,
        api_base,
        api_key,
        request_timeout_secs,
        max_retries,
    )?))
}

fn resolve_env_mapping(env_name: Option<&str>) -> Option<String> {
    env_name.and_then(|name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fallback::parse_explicit_tool_call;
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
}
