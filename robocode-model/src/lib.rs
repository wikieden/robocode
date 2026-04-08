use std::env;
use std::process::Command;

use robocode_types::{
    Message, ModelEvent, ModelRequest, Role, ToolCall, fresh_id, parse_tool_input,
};

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
        }
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
            "provider={} model={} api_base={} key={}",
            self.kind.as_str(),
            self.model,
            self.api_base.as_deref().unwrap_or("<default>"),
            if self.api_key.is_some() {
                "present"
            } else {
                "missing"
            }
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
            HttpMode::Anthropic => build_anthropic_body(&self.model, &request.messages),
            HttpMode::OpenAiCompatible => build_openai_body(&self.model, &request.messages),
            HttpMode::Ollama => build_ollama_body(&self.model, &request.messages),
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
        let response = post_json(&self.api_base, path, &headers, &body)?;
        let content = match self.mode {
            HttpMode::Anthropic => parse_anthropic_response(&response),
            HttpMode::OpenAiCompatible => parse_openai_response(&response),
            HttpMode::Ollama => parse_ollama_response(&response),
        }
        .or_else(|| extract_error_message(&response).map(|message| format!("API error: {message}")))
        .unwrap_or_else(|| {
            format!(
                "{} returned a response, but RoboCode could not parse assistant content.\n\nRaw response:\n{}",
                self.provider_name(),
                response
            )
        });
        Ok(vec![
            ModelEvent::AssistantText { content },
            ModelEvent::Done,
        ])
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

fn build_anthropic_body(model: &str, messages: &[Message]) -> String {
    format!(
        "{{\"model\":\"{}\",\"max_tokens\":2048,\"system\":\"{}\",\"messages\":[{}]}}",
        escape_json(model),
        escape_json(&provider_system_prompt()),
        render_message_array(messages, true)
    )
}

fn build_openai_body(model: &str, messages: &[Message]) -> String {
    format!(
        "{{\"model\":\"{}\",\"messages\":[{{\"role\":\"system\",\"content\":\"{}\"}},{}],\"temperature\":0.2}}",
        escape_json(model),
        escape_json(&provider_system_prompt()),
        render_message_array(messages, false)
    )
}

fn build_ollama_body(model: &str, messages: &[Message]) -> String {
    format!(
        "{{\"model\":\"{}\",\"stream\":false,\"messages\":[{{\"role\":\"system\",\"content\":\"{}\"}},{}]}}",
        escape_json(model),
        escape_json(&provider_system_prompt()),
        render_message_array(messages, false)
    )
}

fn render_message_array(messages: &[Message], anthropic_style: bool) -> String {
    let mut rendered = Vec::new();
    for message in messages {
        let role = match message.role {
            Role::Assistant => "assistant",
            Role::User => "user",
            Role::System | Role::Tool => {
                if anthropic_style {
                    "user"
                } else {
                    "user"
                }
            }
        };
        let content = normalized_message_content(message);
        rendered.push(format!(
            "{{\"role\":\"{}\",\"content\":\"{}\"}}",
            role,
            escape_json(&content)
        ));
    }
    rendered.join(",")
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

fn provider_system_prompt() -> String {
    [
        "You are RoboCode, a coding assistant running in a terminal.",
        "When you need a tool, respond with exactly one line in this format:",
        "tool <tool_name> key=value key=value",
        "Do not wrap tool calls in JSON or markdown fences.",
        "Available tools include shell, read_file, write_file, edit_file, glob, grep.",
        "If no tool is required, answer normally in plain text.",
    ]
    .join("\n")
}

fn post_json(api_base: &str, path: &str, headers: &[String], body: &str) -> Result<String, String> {
    let url = format!("{}{}", api_base.trim_end_matches('/'), path);
    let mut command = Command::new("curl");
    command
        .arg("--silent")
        .arg("--show-error")
        .arg("--fail-with-body")
        .arg("--max-time")
        .arg("90")
        .arg("-X")
        .arg("POST")
        .arg(url);
    for header in headers {
        command.arg("-H").arg(header);
    }
    command.arg("-d").arg(body);
    let output = command.output().map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let error_text = if !stdout.is_empty() { stdout } else { stderr };
        return Err(if error_text.is_empty() {
            format!("curl failed with status {}", output.status)
        } else {
            error_text
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_anthropic_response(response: &str) -> Option<String> {
    extract_string_after(response, "\"text\":\"")
}

fn parse_openai_response(response: &str) -> Option<String> {
    if let Some(idx) = response.find("\"message\":") {
        extract_string_after(&response[idx..], "\"content\":\"")
    } else {
        extract_string_after(response, "\"content\":\"")
    }
}

fn parse_ollama_response(response: &str) -> Option<String> {
    if let Some(idx) = response.find("\"message\":") {
        extract_string_after(&response[idx..], "\"content\":\"")
    } else {
        extract_string_after(response, "\"response\":\"")
    }
}

fn extract_error_message(response: &str) -> Option<String> {
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

fn escape_json(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
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
        assert_eq!(
            parse_openai_response(response).as_deref(),
            Some("hello world")
        );
    }

    #[test]
    fn provider_without_key_falls_back_cleanly() {
        let mut provider = create_provider(ProviderConfig {
            kind: ProviderKind::OpenAi,
            model: "gpt-5.2".to_string(),
            api_base: Some("https://api.openai.com".to_string()),
            api_key: None,
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
}
