//! Static plugin registry boundary for Viden runtime integrations.
//!
//! Dynamic loading stays in provider-specific code for now. This host crate is
//! the shared place for plugin discovery, validation, and lifecycle contracts as
//! tools, agents, workflows, and providers move behind the plugin API.

use viden_plugin_api::{
    AgentAuthMode, AgentCommandSpec, AgentEnvRef, AgentPermissionProfile, AgentPluginCapability,
    AgentPluginDescriptor, AgentProtocolVersion, AgentRegistryPackage, AgentSource, AgentTransport,
    PluginKind, PluginManifest,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticPluginRegistry {
    manifests: Vec<PluginManifest>,
    agent_descriptors: Vec<AgentPluginDescriptor>,
}

impl StaticPluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, manifest: PluginManifest) {
        self.manifests.push(manifest);
    }

    pub fn register_agent(&mut self, descriptor: AgentPluginDescriptor) {
        self.agent_descriptors.push(descriptor);
    }

    pub fn manifests(&self) -> &[PluginManifest] {
        &self.manifests
    }

    pub fn agent_descriptors(&self) -> &[AgentPluginDescriptor] {
        &self.agent_descriptors
    }

    pub fn agent_by_id(&self, id: &str) -> Option<&AgentPluginDescriptor> {
        self.agent_descriptors
            .iter()
            .find(|descriptor| descriptor.agent_id == id)
    }

    pub fn by_kind(&self, kind: PluginKind) -> impl Iterator<Item = &PluginManifest> {
        self.manifests
            .iter()
            .filter(move |manifest| manifest.kind == kind)
    }
}

pub fn builtin_agent_descriptors() -> Vec<AgentPluginDescriptor> {
    vec![
        registry_acp_agent(
            "claude-acp",
            "Claude Agent",
            "0.56.0",
            "@agentclientprotocol/claude-agent-acp",
            &[
                AgentPluginCapability::SessionPrompt,
                AgentPluginCapability::SessionLoad,
                AgentPluginCapability::SessionCancel,
                AgentPluginCapability::StreamingUpdates,
                AgentPluginCapability::ToolCalls,
            ],
        ),
        registry_acp_agent(
            "codex-acp",
            "Codex",
            "1.1.0",
            "@agentclientprotocol/codex-acp",
            &[
                AgentPluginCapability::SessionPrompt,
                AgentPluginCapability::SessionCancel,
                AgentPluginCapability::StreamingUpdates,
                AgentPluginCapability::ToolCalls,
            ],
        ),
        local_kiro_agent(),
    ]
}

fn registry_acp_agent(
    agent_id: &str,
    display_name: &str,
    version: &str,
    package: &str,
    capabilities: &[AgentPluginCapability],
) -> AgentPluginDescriptor {
    AgentPluginDescriptor {
        agent_id: agent_id.to_string(),
        display_name: display_name.to_string(),
        version: version.to_string(),
        transport: AgentTransport::Acp,
        source: AgentSource::Registry,
        command: AgentCommandSpec {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), format!("{package}@{version}")],
            env: Vec::new(),
        },
        registry_package: Some(AgentRegistryPackage {
            package: package.to_string(),
            version: version.to_string(),
        }),
        protocol_versions: vec![AgentProtocolVersion::AcpV1],
        auth_modes: vec![AgentAuthMode::AgentNative],
        capabilities: capabilities.to_vec(),
        permission_profile: AgentPermissionProfile::RuntimeGated,
        experimental_methods: Vec::new(),
        config_schema_version: 1,
    }
}

fn local_kiro_agent() -> AgentPluginDescriptor {
    AgentPluginDescriptor {
        agent_id: "kiro-cli".to_string(),
        display_name: "Kiro CLI".to_string(),
        version: "local".to_string(),
        transport: AgentTransport::Acp,
        source: AgentSource::LocalCommand,
        command: AgentCommandSpec {
            command: "kiro-cli".to_string(),
            args: vec!["acp".to_string()],
            env: vec![
                AgentEnvRef {
                    name: "VIDEN_KIRO_AGENT".to_string(),
                    required: false,
                },
                AgentEnvRef {
                    name: "VIDEN_KIRO_MODEL".to_string(),
                    required: false,
                },
                AgentEnvRef {
                    name: "VIDEN_KIRO_EFFORT".to_string(),
                    required: false,
                },
                AgentEnvRef {
                    name: "VIDEN_KIRO_TRUST_ALL_TOOLS".to_string(),
                    required: false,
                },
                AgentEnvRef {
                    name: "VIDEN_KIRO_TRUST_TOOLS".to_string(),
                    required: false,
                },
                AgentEnvRef {
                    name: "VIDEN_KIRO_AGENT_ENGINE".to_string(),
                    required: false,
                },
            ],
        },
        registry_package: None,
        protocol_versions: vec![AgentProtocolVersion::AcpV1],
        auth_modes: vec![AgentAuthMode::AgentNative],
        capabilities: vec![
            AgentPluginCapability::SessionPrompt,
            AgentPluginCapability::SessionLoad,
            AgentPluginCapability::SessionCancel,
            AgentPluginCapability::SessionSetMode,
            AgentPluginCapability::SessionSetModel,
            AgentPluginCapability::StreamingUpdates,
            AgentPluginCapability::ToolCalls,
            AgentPluginCapability::ImageInput,
            AgentPluginCapability::SlashCommands,
            AgentPluginCapability::McpEvents,
        ],
        permission_profile: AgentPermissionProfile::RuntimeGated,
        experimental_methods: vec![
            "_kiro.dev/commands/available".to_string(),
            "_kiro.dev/commands/options".to_string(),
            "_kiro.dev/commands/execute".to_string(),
            "_kiro.dev/mcp/oauth_request".to_string(),
            "_kiro.dev/mcp/server_initialized".to_string(),
            "_kiro.dev/compaction/status".to_string(),
            "_kiro.dev/clear/status".to_string(),
            "_session/terminate".to_string(),
        ],
        config_schema_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viden_plugin_api::{
        AgentPluginCapability, AgentSource, AgentTransport, PluginCapability, PluginPermission,
    };

    #[test]
    fn registry_filters_static_plugins_by_kind() {
        let mut registry = StaticPluginRegistry::new();
        registry.register(PluginManifest {
            id: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            version: "1".to_string(),
            kind: PluginKind::Provider,
            capabilities: vec![PluginCapability::Provider],
            permissions: vec![PluginPermission::Network],
            config_schema_version: 1,
        });

        assert_eq!(registry.by_kind(PluginKind::Provider).count(), 1);
        assert_eq!(registry.by_kind(PluginKind::Tool).count(), 0);
    }

    #[test]
    fn registry_resolves_static_agents_by_id() {
        let mut registry = StaticPluginRegistry::new();
        let kiro = builtin_agent_descriptors()
            .into_iter()
            .find(|agent| agent.agent_id == "kiro-cli")
            .expect("kiro descriptor exists");

        registry.register_agent(kiro);

        let descriptor = registry
            .agent_by_id("kiro-cli")
            .expect("registered descriptor resolves by id");
        assert_eq!(descriptor.transport, AgentTransport::Acp);
        assert_eq!(descriptor.source, AgentSource::LocalCommand);
        assert_eq!(descriptor.command.command, "kiro-cli");
        assert_eq!(descriptor.command.args, vec!["acp"]);
        let env_names = descriptor
            .command
            .env
            .iter()
            .map(|env| env.name.as_str())
            .collect::<Vec<_>>();
        assert!(env_names.contains(&"VIDEN_KIRO_AGENT"));
        assert!(env_names.contains(&"VIDEN_KIRO_MODEL"));
        assert!(env_names.contains(&"VIDEN_KIRO_EFFORT"));
        assert!(env_names.contains(&"VIDEN_KIRO_TRUST_ALL_TOOLS"));
        assert!(env_names.contains(&"VIDEN_KIRO_TRUST_TOOLS"));
        assert!(env_names.contains(&"VIDEN_KIRO_AGENT_ENGINE"));
        assert!(
            descriptor
                .capabilities
                .contains(&AgentPluginCapability::SessionSetModel)
        );
    }

    #[test]
    fn builtin_acp_agents_cover_claude_codex_and_kiro() {
        let agents = builtin_agent_descriptors();
        let ids = agents
            .iter()
            .map(|agent| agent.agent_id.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"claude-acp"));
        assert!(ids.contains(&"codex-acp"));
        assert!(ids.contains(&"kiro-cli"));
        assert!(
            agents
                .iter()
                .all(|agent| agent.transport == AgentTransport::Acp)
        );
        let claude = agents
            .iter()
            .find(|agent| agent.agent_id == "claude-acp")
            .expect("claude descriptor");
        assert_eq!(claude.version, "0.56.0");
        assert_eq!(
            claude.command.args,
            vec!["-y", "@agentclientprotocol/claude-agent-acp@0.56.0"]
        );
    }
}
