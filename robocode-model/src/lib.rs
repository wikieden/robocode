use std::env;
use std::process::Command;

use robocode_types::{
    Message, ModelEvent, ModelRequest, Role, ToolCall, ToolInput, ToolSpec, decode_tool_input,
    fresh_id, parse_tool_input,
};
use serde_json::{Map, Value, json};

pub trait ModelProvider: Send {
    fn provider_name(&self) -> &str;
    fn model(&self) -> &str;
    fn set_model(&mut self, model: String);
    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Anthropic,
    OpenAi,
    OpenAiCompatible,
    Ollama,
    Fallback,
}

impl ProviderKind {
    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "anthropic" => Some(Self::Anthropic),
            "openai" => Some(Self::OpenAi),
            "openai-compatible" | "openai_compatible" | "compat" => Some(Self::OpenAiCompatible),
            "ollama" => Some(Self::Ollama),
            "fallback" | "local" => Some(Self::Fallback),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
            Self::OpenAiCompatible => "openai-compatible",
            Self::Ollama => "ollama",
            Self::Fallback => "fallback",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub model: String,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub request_timeout_secs: u64,
    pub max_retries: u32,
}

impl ProviderConfig {
    pub fn from_env() -> Self {
        let kind = env::var("ROBOCODE_PROVIDER")
            .ok()
            .and_then(|value| ProviderKind::parse(&value))
            .unwrap_or(ProviderKind::Anthropic);
        let model = env::var("ROBOCODE_MODEL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_model_for(kind).to_string());
        let api_base = env::var("ROBOCODE_API_BASE").ok();
        let api_key = resolve_api_key(kind);
        Self {
            kind,
            model,
            api_base,
            api_key,
            request_timeout_secs: env::var("ROBOCODE_REQUEST_TIMEOUT_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(90),
            max_retries: env::var("ROBOCODE_MAX_RETRIES")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(1),
        }
    }

    pub fn from_settings(
        provider: &str,
        model: Option<&str>,
        api_base: Option<&str>,
        api_key: Option<&str>,
        request_timeout_secs: u64,
        max_retries: u32,
    ) -> Result<Self, String> {
        let kind = ProviderKind::parse(provider)
            .ok_or_else(|| format!("Unknown provider `{provider}`"))?;
        Ok(Self {
            kind,
            model: model
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(default_model_for(kind))
                .to_string(),
            api_base: api_base.map(ToString::to_string),
            api_key: api_key
                .map(ToString::to_string)
                .or_else(|| resolve_api_key(kind)),
            request_timeout_secs: request_timeout_secs.max(1),
            max_retries,
        })
    }

    pub fn with_overrides(
        mut self,
        provider: Option<&str>,
        model: Option<&str>,
        api_base: Option<&str>,
        api_key: Option<&str>,
    ) -> Result<Self, String> {
        if let Some(provider) = provider {
            self.kind = ProviderKind::parse(provider)
                .ok_or_else(|| format!("Unknown provider `{provider}`"))?;
            if self.model == default_model_for(ProviderKind::Anthropic)
                || self.model == default_model_for(ProviderKind::OpenAi)
                || self.model == default_model_for(ProviderKind::OpenAiCompatible)
                || self.model == default_model_for(ProviderKind::Ollama)
                || self.model == default_model_for(ProviderKind::Fallback)
            {
                self.model = default_model_for(self.kind).to_string();
            }
            self.api_key = resolve_api_key(self.kind);
        }
        if let Some(model) = model {
            self.model = model.to_string();
        }
        if let Some(api_base) = api_base {
            self.api_base = Some(api_base.to_string());
        }
        if let Some(api_key) = api_key {
            self.api_key = Some(api_key.to_string());
        }
        Ok(self)
    }

    pub fn summary(&self) -> String {
        format!(
            "provider={} model={} api_base={} key={} timeout={}s retries={}",
            self.kind.as_str(),
            self.model,
            self.api_base.as_deref().unwrap_or("<default>"),
            if self.api_key.is_some() {
                "present"
            } else {
                "missing"
            },
            self.request_timeout_secs,
            self.max_retries,
        )
    }
}

pub fn create_provider(config: ProviderConfig) -> Box<dyn ModelProvider> {
    match config.kind {
        ProviderKind::Anthropic => Box::new(HttpProvider::anthropic(config)),
        ProviderKind::OpenAi => Box::new(HttpProvider::openai(config)),
        ProviderKind::OpenAiCompatible => Box::new(HttpProvider::openai_compatible(config)),
        ProviderKind::Ollama => Box::new(HttpProvider::ollama(config)),
        ProviderKind::Fallback => Box::new(FallbackProvider::from_config(config)),
    }
}

pub fn list_supported_provider_strings() -> &'static [&'static str] {
    &[
        "anthropic",
        "openai",
        "openai-compatible",
        "ollama",
        "fallback",
    ]
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
            provider_name: config.kind.as_str().to_string(),
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
    fn anthropic(config: ProviderConfig) -> Self {
        Self {
            provider_name: "anthropic".to_string(),
            mode: HttpMode::Anthropic,
            model: config.model,
            api_base: config
                .api_base
                .unwrap_or_else(|| "https://api.anthropic.com".to_string()),
            api_key: config.api_key,
            request_timeout_secs: config.request_timeout_secs,
            max_retries: config.max_retries,
        }
    }

    fn openai(config: ProviderConfig) -> Self {
        Self {
            provider_name: "openai".to_string(),
            mode: HttpMode::OpenAiCompatible,
            model: config.model,
            api_base: config
                .api_base
                .unwrap_or_else(|| "https://api.openai.com".to_string()),
            api_key: config
                .api_key
                .or_else(|| resolve_api_key(ProviderKind::OpenAi)),
            request_timeout_secs: config.request_timeout_secs,
            max_retries: config.max_retries,
        }
    }

    fn openai_compatible(config: ProviderConfig) -> Self {
        Self {
            provider_name: "openai-compatible".to_string(),
            mode: HttpMode::OpenAiCompatible,
            model: config.model,
            api_base: config
                .api_base
                .unwrap_or_else(|| "https://api.openai.com".to_string()),
            api_key: config.api_key,
            request_timeout_secs: config.request_timeout_secs,
            max_retries: config.max_retries,
        }
    }

    fn ollama(config: ProviderConfig) -> Self {
        Self {
            provider_name: "ollama".to_string(),
            mode: HttpMode::Ollama,
            model: config.model,
            api_base: config
                .api_base
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
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

fn fallback_events(
    provider_name: &str,
    model: &str,
    last_message: Option<&Message>,
) -> Vec<ModelEvent> {
    let Some(last_message) = last_message else {
        return vec![ModelEvent::AssistantText {
            content: "RoboCode is ready.".to_string(),
        }];
    };

    if last_message.role == Role::Tool {
        return vec![
            ModelEvent::AssistantText {
                content: format!(
                    "Tool `{}` completed.\n\n{}",
                    last_message
                        .tool_name
                        .clone()
                        .unwrap_or_else(|| "tool".to_string()),
                    last_message.content
                ),
            },
            ModelEvent::Done,
        ];
    }

    if last_message.role != Role::User {
        return vec![ModelEvent::Done];
    }

    if let Some(tool_call) = parse_explicit_tool_call(&last_message.content) {
        return vec![ModelEvent::ToolCall(tool_call), ModelEvent::Done];
    }

    vec![
        ModelEvent::AssistantText {
            content: format!(
                "{} provider is running in local fallback mode for model `{}`.\n\n\
Use `tool <name> key=value ...` to force a tool call, or configure API credentials.\n\n\
You said:\n{}",
                provider_name, model, last_message.content
            ),
        },
        ModelEvent::Done,
    ]
}

fn parse_explicit_tool_call_from_messages(messages: &[Message]) -> Option<ToolCall> {
    messages.last().and_then(|message| {
        if message.role == Role::User {
            parse_explicit_tool_call(&message.content)
        } else {
            None
        }
    })
}

fn parse_explicit_tool_call(input: &str) -> Option<ToolCall> {
    let trimmed = input.trim();
    let prefixes = ["tool ", "use "];
    for prefix in prefixes {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let mut parts = rest.splitn(2, ' ');
            let name = parts.next()?.trim();
            let payload = parts.next().unwrap_or("").trim();
            return Some(ToolCall {
                id: fresh_id("tool"),
                name: name.to_string(),
                input: parse_tool_input(payload),
            });
        }
    }
    None
}

fn build_anthropic_body(model: &str, request: &ModelRequest) -> String {
    let mut payload = json!({
        "model": model,
        "max_tokens": 2048,
        "system": provider_system_prompt(),
        "messages": render_anthropic_messages(&request.messages),
    });
    if !request.tools.is_empty() {
        payload["tools"] = Value::Array(render_anthropic_tools(&request.tools));
    }
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

fn build_openai_body(model: &str, request: &ModelRequest) -> String {
    let mut messages = vec![json!({
        "role": "system",
        "content": provider_system_prompt(),
    })];
    messages.extend(render_openai_messages(&request.messages));
    let mut payload = json!({
        "model": model,
        "messages": messages,
        "temperature": 0.2,
    });
    if !request.tools.is_empty() {
        payload["tools"] = Value::Array(render_openai_tools(&request.tools));
    }
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

fn build_ollama_body(model: &str, request: &ModelRequest) -> String {
    let mut messages = vec![json!({
        "role": "system",
        "content": provider_system_prompt(),
    })];
    messages.extend(render_simple_messages(&request.messages));
    let payload = json!({
        "model": model,
        "stream": false,
        "messages": messages,
    });
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

fn render_anthropic_messages(messages: &[Message]) -> Vec<Value> {
    let mut rendered = Vec::new();
    for message in messages {
        if message.role == Role::Tool {
            rendered.push(json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": message.tool_call_id.clone().unwrap_or_else(|| fresh_id("tool")),
                    "content": message.content,
                }],
            }));
            continue;
        }
        if message.role == Role::Assistant
            && message.tool_name.is_some()
            && message.tool_call_id.is_some()
        {
            rendered.push(json!({
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": message.tool_call_id.clone().unwrap_or_else(|| fresh_id("tool")),
                    "name": message.tool_name.clone().unwrap_or_else(|| "tool".to_string()),
                    "input": tool_input_to_json(&decode_tool_input(&message.content)),
                }],
            }));
            continue;
        }
        rendered.push(json!({
            "role": match message.role {
                Role::Assistant => "assistant",
                Role::User => "user",
                Role::System | Role::Tool => "user",
            },
            "content": [{
                "type": "text",
                "text": normalized_message_content(message),
            }],
        }));
    }
    rendered
}

fn render_openai_messages(messages: &[Message]) -> Vec<Value> {
    let mut rendered = Vec::new();
    for message in messages {
        match message.role {
            Role::Tool => {
                rendered.push(json!({
                    "role": "tool",
                    "tool_call_id": message.tool_call_id.clone().unwrap_or_else(|| fresh_id("tool")),
                    "content": message.content,
                }));
            }
            Role::Assistant if message.tool_name.is_some() && message.tool_call_id.is_some() => {
                rendered.push(json!({
                    "role": "assistant",
                    "content": "",
                    "tool_calls": [{
                        "id": message.tool_call_id.clone().unwrap_or_else(|| fresh_id("tool")),
                        "type": "function",
                        "function": {
                            "name": message.tool_name.clone().unwrap_or_else(|| "tool".to_string()),
                            "arguments": serde_json::to_string(&tool_input_to_json(&decode_tool_input(&message.content)))
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }],
                }));
            }
            Role::System => {
                rendered.push(json!({
                    "role": "user",
                    "content": normalized_message_content(message),
                }));
            }
            Role::Assistant | Role::User => {
                rendered.push(json!({
                    "role": match message.role {
                        Role::Assistant => "assistant",
                        _ => "user",
                    },
                    "content": normalized_message_content(message),
                }));
            }
        }
    }
    rendered
}

fn render_simple_messages(messages: &[Message]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            json!({
                "role": match message.role {
                    Role::Assistant => "assistant",
                    _ => "user",
                },
                "content": normalized_message_content(message),
            })
        })
        .collect()
}

fn normalized_message_content(message: &Message) -> String {
    match message.role {
        Role::Tool => format!(
            "[tool_result:{}]\n{}",
            message.tool_name.as_deref().unwrap_or("tool"),
            message.content
        ),
        Role::System => format!("[system]\n{}", message.content),
        _ => message.content.clone(),
    }
}

fn render_openai_tools(tools: &[ToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool_parameters_schema(tool),
                }
            })
        })
        .collect()
}

fn render_anthropic_tools(tools: &[ToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool_parameters_schema(tool),
            })
        })
        .collect()
}

fn tool_parameters_schema(tool: &ToolSpec) -> Value {
    let mut properties = Map::new();
    for key in extract_input_keys(&tool.input_schema_hint) {
        properties.insert(
            key.clone(),
            json!({
                "type": "string",
                "description": format!("Input field `{}` for {}", key, tool.name),
            }),
        );
    }
    json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": { "type": "string" },
    })
}

fn extract_input_keys(hint: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for segment in hint.split_whitespace() {
        let cleaned = segment.trim_matches(|char: char| char == ',' || char == ';');
        if let Some((key, _)) = cleaned.split_once('=') {
            let key = key
                .trim()
                .trim_matches(|char: char| char == '\'' || char == '"');
            if !key.is_empty()
                && key
                    .chars()
                    .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
                && !keys.iter().any(|existing| existing == key)
            {
                keys.push(key.to_string());
            }
        }
    }
    keys
}

fn tool_input_to_json(input: &ToolInput) -> Value {
    let mut object = Map::new();
    for (key, value) in input {
        object.insert(key.clone(), Value::String(value.clone()));
    }
    Value::Object(object)
}

fn provider_system_prompt() -> String {
    [
        "You are RoboCode, a coding assistant running in a terminal.",
        "When native tool calling is available, prefer the provided tool interface.",
        "If native tool calling is unavailable, respond with exactly one line in this format:",
        "tool <tool_name> key=value key=value",
        "Do not wrap tool calls in JSON or markdown fences.",
        "Available tools include shell, read_file, write_file, edit_file, glob, grep, and git helpers.",
        "If no tool is required, answer normally in plain text.",
    ]
    .join("\n")
}

struct HttpResponse {
    status_code: u16,
    body: String,
}

fn post_json(
    api_base: &str,
    path: &str,
    headers: &[String],
    body: &str,
    timeout_secs: u64,
    max_retries: u32,
) -> Result<HttpResponse, String> {
    let url = format!("{}{}", api_base.trim_end_matches('/'), path);
    let mut last_error = String::new();
    for attempt in 0..=max_retries {
        let mut command = Command::new("curl");
        command
            .arg("--silent")
            .arg("--show-error")
            .arg("--max-time")
            .arg(timeout_secs.to_string())
            .arg("-X")
            .arg("POST")
            .arg("-w")
            .arg("\n%{http_code}")
            .arg(url.clone());
        for header in headers {
            command.arg("-H").arg(header);
        }
        command.arg("-d").arg(body);
        let output = command.output().map_err(|err| err.to_string())?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            last_error = if !stderr.is_empty() { stderr } else { stdout };
            if attempt < max_retries {
                continue;
            }
            return Err(if last_error.is_empty() {
                format!("curl failed with status {}", output.status)
            } else {
                last_error
            });
        }
        let rendered = String::from_utf8_lossy(&output.stdout).to_string();
        if let Some((body, status_code)) = split_response_and_status(&rendered) {
            if status_code >= 500 && attempt < max_retries {
                last_error = format!("HTTP {}", status_code);
                continue;
            }
            return Ok(HttpResponse { status_code, body });
        }
        last_error = "Could not parse HTTP status from curl output".to_string();
    }
    Err(last_error)
}

fn split_response_and_status(rendered: &str) -> Option<(String, u16)> {
    let trimmed = rendered.trim_end_matches('\n');
    let (body, status) = trimmed.rsplit_once('\n')?;
    let status_code = status.trim().parse::<u16>().ok()?;
    Some((body.to_string(), status_code))
}

fn parse_anthropic_events(response: &str) -> Option<Vec<ModelEvent>> {
    let value: Value = serde_json::from_str(response).ok()?;
    let content_blocks = value.get("content")?.as_array()?;
    let mut tool_calls = Vec::new();
    let mut text_parts = Vec::new();
    for block in content_blocks {
        match block.get("type")?.as_str()? {
            "tool_use" => {
                let name = block.get("name")?.as_str()?.to_string();
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| fresh_id("tool"));
                let input = json_value_to_tool_input(block.get("input").unwrap_or(&Value::Null));
                tool_calls.push(ModelEvent::ToolCall(ToolCall { id, name, input }));
            }
            "text" => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        text_parts.push(text.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    if !tool_calls.is_empty() {
        Some(tool_calls)
    } else if text_parts.is_empty() {
        None
    } else {
        Some(vec![ModelEvent::AssistantText {
            content: text_parts.join("\n\n"),
        }])
    }
}

fn parse_openai_events(response: &str) -> Option<Vec<ModelEvent>> {
    let value: Value = serde_json::from_str(response).ok()?;
    let message = value.get("choices")?.as_array()?.first()?.get("message")?;
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        let events: Vec<ModelEvent> = tool_calls
            .iter()
            .filter_map(|tool_call| {
                let function = tool_call.get("function")?;
                let name = function.get("name")?.as_str()?.to_string();
                let id = tool_call
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| fresh_id("tool"));
                let arguments = function
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                Some(ModelEvent::ToolCall(ToolCall {
                    id,
                    name,
                    input: parse_json_tool_arguments(arguments),
                }))
            })
            .collect();
        if !events.is_empty() {
            return Some(events);
        }
    }
    extract_openai_content(message).map(|content| vec![ModelEvent::AssistantText { content }])
}

fn parse_ollama_events(response: &str) -> Option<Vec<ModelEvent>> {
    let value: Value = serde_json::from_str(response).ok()?;
    if let Some(message) = value.get("message") {
        if let Some(content) = message.get("content").and_then(Value::as_str) {
            if !content.trim().is_empty() {
                return Some(vec![ModelEvent::AssistantText {
                    content: content.to_string(),
                }]);
            }
        }
    }
    if let Some(content) = value.get("response").and_then(Value::as_str) {
        if !content.trim().is_empty() {
            return Some(vec![ModelEvent::AssistantText {
                content: content.to_string(),
            }]);
        }
    }
    None
}

fn extract_openai_content(message: &Value) -> Option<String> {
    if let Some(content) = message.get("content").and_then(Value::as_str) {
        if !content.trim().is_empty() {
            return Some(content.to_string());
        }
    }
    if let Some(parts) = message.get("content").and_then(Value::as_array) {
        let text = parts
            .iter()
            .filter_map(|part| {
                if part.get("type").and_then(Value::as_str) == Some("text") {
                    part.get("text")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

fn parse_json_tool_arguments(arguments: &str) -> ToolInput {
    if let Ok(value) = serde_json::from_str::<Value>(arguments) {
        json_value_to_tool_input(&value)
    } else {
        parse_tool_input(arguments)
    }
}

fn json_value_to_tool_input(value: &Value) -> ToolInput {
    let mut input = ToolInput::new();
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            input.insert(key.clone(), json_value_to_string(value));
        }
    }
    input
}

fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(string) => string.clone(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| String::new())
        }
    }
}

fn extract_error_message(response: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(response) {
        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message").or_else(|| error.get("error")))
            .and_then(Value::as_str)
        {
            return Some(message.to_string());
        }
        if let Some(message) = value.get("message").and_then(Value::as_str) {
            return Some(message.to_string());
        }
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return Some(error.to_string());
        }
    }
    extract_string_after(response, "\"message\":\"")
        .or_else(|| extract_string_after(response, "\"error\":\""))
}

fn extract_string_after(input: &str, marker: &str) -> Option<String> {
    let start = input.find(marker)? + marker.len();
    let bytes = input.as_bytes();
    let mut index = start;
    let mut escaped = false;
    let mut output = String::new();
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if escaped {
            output.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(output);
        } else {
            output.push(ch);
        }
        index += 1;
    }
    None
}

fn default_model_for(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "claude-sonnet-4-6",
        ProviderKind::OpenAi => "gpt-5.2",
        ProviderKind::OpenAiCompatible => "gpt-4o-mini",
        ProviderKind::Ollama => "llama3.1",
        ProviderKind::Fallback => "fallback-local",
    }
}

fn resolve_api_key(kind: ProviderKind) -> Option<String> {
    env::var("ROBOCODE_API_KEY").ok().or_else(|| match kind {
        ProviderKind::Anthropic => env::var("ANTHROPIC_API_KEY")
            .ok()
            .or_else(|| env::var("ROBOCODE_ANTHROPIC_API_KEY").ok()),
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => env::var("OPENAI_API_KEY")
            .ok()
            .or_else(|| env::var("ROBOCODE_OPENAI_API_KEY").ok()),
        ProviderKind::Ollama | ProviderKind::Fallback => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use robocode_types::{Message, ModelRequest, PermissionMode, Role, ToolSpec};

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
    fn split_response_and_status_parses_curl_suffix() {
        let response = split_response_and_status("{\"ok\":true}\n200").unwrap();
        assert_eq!(response.0, "{\"ok\":true}");
        assert_eq!(response.1, 200);
    }
}
