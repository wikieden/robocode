use crate::adapters::{builtin_default_api_base, builtin_default_model, builtin_provider_id};
use crate::config::{ProviderConfig, ProviderKind, resolve_api_key};
use crate::descriptor::{ProtocolFamily, ProviderDescriptor};
use crate::fallback::{fallback_events, parse_explicit_tool_call_from_messages};
use crate::parse::{
    extract_error_message, parse_anthropic_events, parse_anthropic_stream_events,
    parse_ollama_events, parse_openai_events, parse_openai_stream_events,
};
use crate::registry::ProviderRegistry;
use crate::render::{
    build_anthropic_body_with_stream, build_ollama_body, build_openai_body_with_stream,
};
use crate::transport::post_json_with_control;
use crate::{ModelProvider, ModelRequestControl};
use robocode_types::{ModelEvent, ModelRequest};

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

    fn next_events_with_control(
        &mut self,
        request: &ModelRequest,
        control: &ModelRequestControl,
    ) -> Result<Vec<ModelEvent>, String> {
        self.inner.next_events_with_control(request, control)
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
        let api_base = resolve_descriptor_api_base(descriptor, api_base)?;
        let api_key = api_key
            .map(ToString::to_string)
            .or_else(|| resolve_env_mapping(descriptor.env_mappings.api_key_env.as_deref()));
        Ok(Self {
            provider_name: descriptor.provider_id.clone(),
            mode,
            model: model.to_string(),
            api_base,
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
        self.next_events_with_control(request, &ModelRequestControl::default())
    }

    fn next_events_with_control(
        &mut self,
        request: &ModelRequest,
        control: &ModelRequestControl,
    ) -> Result<Vec<ModelEvent>, String> {
        control.check_cancelled()?;
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
            HttpMode::Anthropic => {
                build_anthropic_body_with_stream(&self.model, request, control.prefer_streaming())
            }
            HttpMode::OpenAiCompatible => {
                build_openai_body_with_stream(&self.model, request, control.prefer_streaming())
            }
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
        let response = post_json_with_control(
            &self.api_base,
            path,
            &headers,
            &body,
            self.request_timeout_secs,
            self.max_retries,
            control,
        )?;
        control.check_cancelled()?;
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
            HttpMode::Anthropic => {
                parse_stream_or_json(&response.body, parse_anthropic_stream_events, parse_anthropic_events)
            }
            HttpMode::OpenAiCompatible => {
                parse_stream_or_json(&response.body, parse_openai_stream_events, parse_openai_events)
            }
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

fn parse_stream_or_json(
    response: &str,
    parse_stream: fn(&str) -> Option<Vec<ModelEvent>>,
    parse_json: fn(&str) -> Option<Vec<ModelEvent>>,
) -> Option<Vec<ModelEvent>> {
    if response.trim_start().starts_with("data:") {
        parse_stream(response)
    } else {
        parse_json(response)
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
                    .unwrap_or_else(|| builtin_default_model(kind))
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

fn resolve_descriptor_api_base(
    descriptor: &ProviderDescriptor,
    explicit_api_base: Option<&str>,
) -> Result<String, String> {
    let (api_base, source) = explicit_api_base
        .filter(|value| !value.trim().is_empty())
        .map(|value| (value.to_string(), "explicit API base".to_string()))
        .or_else(|| {
            let env_name = descriptor.env_mappings.api_base_env.as_deref()?;
            resolve_env_mapping(Some(env_name))
                .filter(|value| !value.trim().is_empty())
                .map(|value| (value, format!("environment variable `{env_name}`")))
        })
        .or_else(|| {
            descriptor
                .default_api_base
                .clone()
                .map(|value| (value, "descriptor default_api_base".to_string()))
        })
        .ok_or_else(|| {
            format!(
                "Provider `{}` does not define a default API base; pass an API base explicitly",
                descriptor.provider_id
            )
        })?;

    validate_resolved_api_base(&descriptor.provider_id, &api_base, &source)?;
    Ok(api_base)
}

fn validate_resolved_api_base(
    provider_id: &str,
    api_base: &str,
    source: &str,
) -> Result<(), String> {
    if !(api_base.starts_with("https://") || api_base.starts_with("http://")) {
        return Err(format!(
            "Provider `{provider_id}` resolved API base from {source} as `{api_base}`, but it must start with http:// or https://"
        ));
    }
    Ok(())
}
