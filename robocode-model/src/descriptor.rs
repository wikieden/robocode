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
pub struct ProviderDescriptor {
    pub provider_id: String,
    pub display_name: String,
    pub version: String,
    pub protocol_family: ProtocolFamily,
    pub default_api_base: Option<String>,
    pub default_model: Option<String>,
    pub env_mappings: ProviderEnvMappings,
    pub capabilities: ProviderCapabilities,
    pub config_schema_version: u32,
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
