//! Static plugin registry boundary for Viden runtime integrations.
//!
//! Dynamic loading stays in provider-specific code for now. This host crate is
//! the shared place for plugin discovery, validation, and lifecycle contracts as
//! tools, agents, workflows, and providers move behind the plugin API.

use viden_plugin_api::{PluginKind, PluginManifest};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticPluginRegistry {
    manifests: Vec<PluginManifest>,
}

impl StaticPluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, manifest: PluginManifest) {
        self.manifests.push(manifest);
    }

    pub fn manifests(&self) -> &[PluginManifest] {
        &self.manifests
    }

    pub fn by_kind(&self, kind: PluginKind) -> impl Iterator<Item = &PluginManifest> {
        self.manifests
            .iter()
            .filter(move |manifest| manifest.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viden_plugin_api::{PluginCapability, PluginPermission};

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
}
