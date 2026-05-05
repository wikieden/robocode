use std::os::raw::c_char;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
pub struct PluginDescriptor {
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

pub const ROBOCODE_PLUGIN_DESCRIPTOR_SYMBOL: &str = "robocode_provider_descriptor_json";
pub type PluginDescriptorFn = unsafe extern "C" fn() -> *const c_char;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_descriptor_roundtrips_through_json() {
        let descriptor = PluginDescriptor {
            provider_id: "deepseek".to_string(),
            display_name: "DeepSeek".to_string(),
            version: "1".to_string(),
            protocol_family: ProtocolFamily::OpenAi,
            default_api_base: Some("https://api.deepseek.com".to_string()),
            default_model: Some("deepseek-v4".to_string()),
            env_mappings: ProviderEnvMappings {
                api_key_env: Some("DEEPSEEK_API_KEY".to_string()),
                api_base_env: Some("DEEPSEEK_API_BASE".to_string()),
            },
            capabilities: ProviderCapabilities {
                supports_streaming: true,
                supports_native_tool_calling: true,
            },
            config_schema_version: 1,
        };

        let json = serde_json::to_string(&descriptor).unwrap();
        let decoded: PluginDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.provider_id, "deepseek");
        assert_eq!(decoded.protocol_family, ProtocolFamily::OpenAi);
    }
}
