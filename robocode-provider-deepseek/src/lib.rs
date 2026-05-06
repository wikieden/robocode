use std::ffi::c_char;

use robocode_provider_sdk::{
    PluginDescriptor, PluginDescriptorFn, ProtocolFamily, ProviderCapabilities, ProviderEnvMappings,
};

pub fn descriptor() -> PluginDescriptor {
    PluginDescriptor {
        provider_id: "deepseek".to_string(),
        display_name: "DeepSeek".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://api.deepseek.com".to_string()),
        default_model: Some("deepseek-v4-flash".to_string()),
        env_mappings: ProviderEnvMappings {
            api_key_env: Some("DEEPSEEK_API_KEY".to_string()),
            api_base_env: Some("DEEPSEEK_API_BASE".to_string()),
        },
        capabilities: ProviderCapabilities {
            supports_streaming: true,
            supports_native_tool_calling: true,
        },
        config_schema_version: 1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robocode_provider_descriptor_json() -> *const c_char {
    static DESCRIPTOR_JSON: &str = concat!(
        "{\"provider_id\":\"deepseek\",",
        "\"display_name\":\"DeepSeek\",",
        "\"version\":\"1\",",
        "\"protocol_family\":\"OpenAi\",",
        "\"default_api_base\":\"https://api.deepseek.com\",",
        "\"default_model\":\"deepseek-v4-flash\",",
        "\"env_mappings\":{\"api_key_env\":\"DEEPSEEK_API_KEY\",\"api_base_env\":\"DEEPSEEK_API_BASE\"},",
        "\"capabilities\":{\"supports_streaming\":true,\"supports_native_tool_calling\":true},",
        "\"config_schema_version\":1}\0"
    );
    let _fn_type: PluginDescriptorFn = robocode_provider_descriptor_json;
    DESCRIPTOR_JSON.as_ptr().cast()
}
