use std::env;

use crate::adapters::{
    builtin_default_model, builtin_provider_id, builtin_provider_kind, is_builtin_default_model,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Anthropic,
    DeepSeek,
    DeepSeekAnthropic,
    OpenAi,
    OpenAiCompatible,
    Ollama,
    Fallback,
}

impl ProviderKind {
    pub fn parse(input: &str) -> Option<Self> {
        let normalized = input.trim().to_ascii_lowercase();
        builtin_provider_kind(&normalized).or(match normalized.as_str() {
            "openai_compatible" | "compat" => Some(Self::OpenAiCompatible),
            "local" => Some(Self::Fallback),
            _ => None,
        })
    }

    pub fn as_str(&self) -> &'static str {
        builtin_provider_id(*self)
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
            .unwrap_or(ProviderKind::DeepSeek);
        let model = env::var("ROBOCODE_MODEL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| builtin_default_model(kind).to_string());
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
                .unwrap_or(builtin_default_model(kind))
                .to_string(),
            api_base: api_base.map(ToString::to_string),
            api_key: api_key
                .filter(|value| !value.trim().is_empty())
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
            if is_builtin_default_model(self.model.as_str()) {
                self.model = builtin_default_model(self.kind).to_string();
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
            self.api_key = if api_key.trim().is_empty() {
                resolve_api_key(self.kind)
            } else {
                Some(api_key.to_string())
            };
        }
        Ok(self)
    }

    pub fn summary(&self) -> String {
        format!(
            "provider={} model={} api_base={} key={} timeout={}s retries={}",
            builtin_provider_id(self.kind),
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

pub(crate) fn resolve_api_key(kind: ProviderKind) -> Option<String> {
    match kind {
        ProviderKind::Anthropic => read_non_blank_env("ANTHROPIC_API_KEY")
            .or_else(|| read_non_blank_env("ROBOCODE_ANTHROPIC_API_KEY")),
        ProviderKind::DeepSeek | ProviderKind::DeepSeekAnthropic => {
            read_non_blank_env("DEEPSEEK_API_KEY")
                .or_else(|| read_non_blank_env("ROBOCODE_DEEPSEEK_API_KEY"))
                .or_else(|| read_non_blank_env("ROBOCODE_API_KEY"))
        }
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            read_non_blank_env("OPENAI_API_KEY")
                .or_else(|| read_non_blank_env("ROBOCODE_OPENAI_API_KEY"))
        }
        ProviderKind::Ollama | ProviderKind::Fallback => None,
    }
    .or_else(|| read_non_blank_env("ROBOCODE_API_KEY"))
}

fn read_non_blank_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}
