use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProtocolFamily {
    Anthropic,
    OpenAi,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderEnvMappings {
    pub api_key_env: Option<String>,
    pub api_base_env: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub supports_streaming: bool,
    pub supports_native_tool_calling: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderCompatibility {
    #[serde(default = "default_true")]
    pub supports_tool_choice: bool,
    #[serde(default)]
    pub requires_reasoning_content_for_tool_calls: bool,
    #[serde(default)]
    pub requires_non_null_tool_call_content: bool,
    #[serde(default)]
    pub reasoning_effort_high: Option<String>,
    #[serde(default)]
    pub reasoning_effort_max: Option<String>,
}

impl Default for ProviderCompatibility {
    fn default() -> Self {
        Self {
            supports_tool_choice: true,
            requires_reasoning_content_for_tool_calls: false,
            requires_non_null_tool_call_content: false,
            reasoning_effort_high: None,
            reasoning_effort_max: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub provider_id: String,
    pub display_name: String,
    pub version: String,
    pub protocol_family: ProtocolFamily,
    pub default_api_base: Option<String>,
    pub default_model: Option<String>,
    pub env_mappings: ProviderEnvMappings,
    pub capabilities: ProviderCapabilities,
    #[serde(default)]
    pub compatibility: ProviderCompatibility,
    pub config_schema_version: u32,
}

const fn default_true() -> bool {
    true
}

pub(crate) fn validate_provider_descriptor(descriptor: &ProviderDescriptor) -> Result<(), String> {
    validate_provider_id(&descriptor.provider_id)?;
    validate_non_empty("display_name", &descriptor.display_name)?;
    validate_non_empty("version", &descriptor.version)?;
    if let Some(default_model) = descriptor.default_model.as_deref() {
        validate_non_empty("default_model", default_model)?;
    }
    if let Some(default_api_base) = descriptor.default_api_base.as_deref() {
        validate_api_base(default_api_base)?;
    }
    if let Some(api_key_env) = descriptor.env_mappings.api_key_env.as_deref() {
        validate_env_var_name("api_key_env", api_key_env)?;
    }
    if let Some(api_base_env) = descriptor.env_mappings.api_base_env.as_deref() {
        validate_env_var_name("api_base_env", api_base_env)?;
    }
    if let Some(reasoning_effort_high) = descriptor.compatibility.reasoning_effort_high.as_deref() {
        validate_non_empty("reasoning_effort_high", reasoning_effort_high)?;
    }
    if let Some(reasoning_effort_max) = descriptor.compatibility.reasoning_effort_max.as_deref() {
        validate_non_empty("reasoning_effort_max", reasoning_effort_max)?;
    }
    if descriptor.config_schema_version != 1 {
        return Err(format!(
            "Provider `{}` uses unsupported config_schema_version `{}`",
            descriptor.provider_id, descriptor.config_schema_version
        ));
    }
    Ok(())
}

fn validate_provider_id(provider_id: &str) -> Result<(), String> {
    validate_non_empty("provider_id", provider_id)?;
    if !provider_id
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(format!(
            "Provider descriptor provider_id `{provider_id}` must contain only lowercase ASCII letters, digits, '-' or '_'"
        ));
    }
    Ok(())
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("Provider descriptor `{field}` must not be empty"));
    }
    Ok(())
}

fn validate_api_base(value: &str) -> Result<(), String> {
    validate_non_empty("default_api_base", value)?;
    if !(value.starts_with("https://") || value.starts_with("http://")) {
        return Err(format!(
            "Provider descriptor default_api_base `{value}` must start with http:// or https://"
        ));
    }
    Ok(())
}

fn validate_env_var_name(field: &str, value: &str) -> Result<(), String> {
    validate_non_empty(field, value)?;
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(format!("Provider descriptor `{field}` must not be empty"));
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "Provider descriptor `{field}` value `{value}` must start with a letter or '_'"
        ));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Err(format!(
            "Provider descriptor `{field}` value `{value}` must contain only ASCII letters, digits, or '_'"
        ));
    }
    Ok(())
}
