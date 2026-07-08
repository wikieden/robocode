use std::os::raw::c_char;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginKind {
    Provider,
    Tool,
    Agent,
    Context,
    Workflow,
    Ui,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginCapability {
    Provider,
    ToolExecution,
    AgentRole,
    ContextSource,
    WorkflowGate,
    UiSurface,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginPermission {
    Network,
    FileRead,
    FileWrite,
    Shell,
    Git,
    Memory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: PluginKind,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    pub config_schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProtocolFamily {
    Anthropic,
    OpenAi,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderAuthMode {
    ApiKey,
    WebLogin,
    Local,
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
pub struct PluginDescriptor {
    pub provider_id: String,
    pub display_name: String,
    pub version: String,
    pub protocol_family: ProtocolFamily,
    pub default_api_base: Option<String>,
    pub default_model: Option<String>,
    #[serde(default)]
    pub known_models: Vec<String>,
    pub env_mappings: ProviderEnvMappings,
    pub capabilities: ProviderCapabilities,
    #[serde(default)]
    pub compatibility: ProviderCompatibility,
    #[serde(default)]
    pub auth_modes: Vec<ProviderAuthMode>,
    pub config_schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentTransport {
    Acp,
    AppServer,
    Template,
    Pty,
    Tmux,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentSource {
    Registry,
    LocalCommand,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentAuthMode {
    AgentNative,
    ApiKey,
    WebLogin,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentProtocolVersion {
    AcpV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentPluginCapability {
    SessionPrompt,
    SessionLoad,
    SessionCancel,
    SessionSetMode,
    SessionSetModel,
    StreamingUpdates,
    ToolCalls,
    ImageInput,
    SlashCommands,
    McpEvents,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentPermissionProfile {
    ReadOnlyProbe,
    RuntimeGated,
    AgentNative,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentEnvRef {
    pub name: String,
    pub required: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCommandSpec {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<AgentEnvRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRegistryPackage {
    pub package: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPluginDescriptor {
    pub agent_id: String,
    pub display_name: String,
    pub version: String,
    pub transport: AgentTransport,
    pub source: AgentSource,
    pub command: AgentCommandSpec,
    pub registry_package: Option<AgentRegistryPackage>,
    #[serde(default)]
    pub protocol_versions: Vec<AgentProtocolVersion>,
    #[serde(default)]
    pub auth_modes: Vec<AgentAuthMode>,
    #[serde(default)]
    pub capabilities: Vec<AgentPluginCapability>,
    pub permission_profile: AgentPermissionProfile,
    #[serde(default)]
    pub experimental_methods: Vec<String>,
    pub config_schema_version: u32,
}

const fn default_true() -> bool {
    true
}

pub const VIDEN_PLUGIN_DESCRIPTOR_SYMBOL: &str = "viden_plugin_descriptor_json";
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
            default_model: Some("deepseek-v4-flash".to_string()),
            known_models: vec![
                "deepseek-v4-flash".to_string(),
                "deepseek-v4-pro".to_string(),
            ],
            env_mappings: ProviderEnvMappings {
                api_key_env: Some("DEEPSEEK_API_KEY".to_string()),
                api_base_env: Some("DEEPSEEK_API_BASE".to_string()),
            },
            capabilities: ProviderCapabilities {
                supports_streaming: true,
                supports_native_tool_calling: true,
            },
            compatibility: ProviderCompatibility {
                supports_tool_choice: false,
                requires_reasoning_content_for_tool_calls: true,
                requires_non_null_tool_call_content: true,
                reasoning_effort_high: Some("high".to_string()),
                reasoning_effort_max: Some("max".to_string()),
            },
            auth_modes: vec![ProviderAuthMode::ApiKey],
            config_schema_version: 1,
        };

        let json = serde_json::to_string(&descriptor).unwrap();
        let decoded: PluginDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.provider_id, "deepseek");
        assert_eq!(decoded.protocol_family, ProtocolFamily::OpenAi);
        assert!(!decoded.compatibility.supports_tool_choice);
    }

    #[test]
    fn plugin_manifest_roundtrips_through_json() {
        let manifest = PluginManifest {
            id: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            version: "1".to_string(),
            kind: PluginKind::Provider,
            capabilities: vec![PluginCapability::Provider],
            permissions: vec![PluginPermission::Network],
            config_schema_version: 1,
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let decoded: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.kind, PluginKind::Provider);
        assert_eq!(decoded.capabilities, vec![PluginCapability::Provider]);
    }

    #[test]
    fn agent_plugin_descriptor_roundtrips_through_json() {
        let descriptor = AgentPluginDescriptor {
            agent_id: "kiro-cli".to_string(),
            display_name: "Kiro CLI".to_string(),
            version: "1".to_string(),
            transport: AgentTransport::Acp,
            source: AgentSource::LocalCommand,
            command: AgentCommandSpec {
                command: "kiro-cli".to_string(),
                args: vec!["acp".to_string()],
                env: vec![],
            },
            registry_package: None,
            protocol_versions: vec![AgentProtocolVersion::AcpV1],
            auth_modes: vec![AgentAuthMode::AgentNative],
            capabilities: vec![
                AgentPluginCapability::SessionPrompt,
                AgentPluginCapability::SessionCancel,
                AgentPluginCapability::SessionSetMode,
                AgentPluginCapability::SessionSetModel,
                AgentPluginCapability::StreamingUpdates,
                AgentPluginCapability::ToolCalls,
            ],
            permission_profile: AgentPermissionProfile::RuntimeGated,
            experimental_methods: vec!["_kiro.dev/commands/execute".to_string()],
            config_schema_version: 1,
        };

        let json = serde_json::to_string(&descriptor).unwrap();
        let decoded: AgentPluginDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.agent_id, "kiro-cli");
        assert_eq!(decoded.transport, AgentTransport::Acp);
        assert_eq!(decoded.command.command, "kiro-cli");
        assert!(
            decoded
                .capabilities
                .contains(&AgentPluginCapability::ToolCalls)
        );
    }
}
