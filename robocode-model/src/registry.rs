use std::collections::BTreeMap;

use crate::{
    adapters::builtin_provider_descriptors, config::ProviderKind, descriptor::ProviderDescriptor,
    descriptor::validate_provider_descriptor, plugin::discover_plugin_descriptors,
    plugin::discover_plugin_descriptors_in_dirs,
};
use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct ProviderRegistry {
    descriptors: Vec<ProviderDescriptor>,
}

impl ProviderRegistry {
    pub fn with_builtins() -> Self {
        Self {
            descriptors: builtin_provider_descriptors(),
        }
    }

    pub fn load_default() -> Result<Self, String> {
        let plugin_descriptors = discover_plugin_descriptors()?
            .into_iter()
            .map(|loaded| loaded.descriptor)
            .collect();
        Self::from_descriptor_sources(builtin_provider_descriptors(), plugin_descriptors)
    }

    pub fn load_from_dirs(plugin_dirs: Vec<PathBuf>) -> Result<Self, String> {
        let plugin_descriptors = discover_plugin_descriptors_in_dirs(plugin_dirs)?
            .into_iter()
            .map(|loaded| loaded.descriptor)
            .collect();
        Self::from_descriptor_sources(builtin_provider_descriptors(), plugin_descriptors)
    }

    pub(crate) fn from_descriptor_sources(
        builtin_descriptors: Vec<ProviderDescriptor>,
        plugin_descriptors: Vec<ProviderDescriptor>,
    ) -> Result<Self, String> {
        let mut by_id = BTreeMap::<String, ProviderDescriptor>::new();
        for descriptor in builtin_descriptors {
            validate_provider_descriptor(&descriptor)?;
            by_id.insert(descriptor.provider_id.clone(), descriptor);
        }
        for descriptor in plugin_descriptors {
            validate_provider_descriptor(&descriptor)?;
            if by_id.contains_key(&descriptor.provider_id) {
                return Err(format!(
                    "Provider plugin `{}` conflicts with an existing provider id",
                    descriptor.provider_id
                ));
            }
            by_id.insert(descriptor.provider_id.clone(), descriptor);
        }
        Ok(Self {
            descriptors: by_id.into_values().collect(),
        })
    }

    pub fn descriptors(&self) -> &[ProviderDescriptor] {
        &self.descriptors
    }

    pub fn provider_ids(&self) -> Vec<String> {
        self.descriptors
            .iter()
            .map(|item| item.provider_id.clone())
            .collect()
    }

    pub fn creatable_provider_ids(&self) -> Vec<String> {
        self.descriptors
            .iter()
            .filter_map(|descriptor| {
                ProviderKind::parse(&descriptor.provider_id).map(|_| descriptor.provider_id.clone())
            })
            .collect()
    }

    pub fn descriptor(&self, provider_id: &str) -> Option<&ProviderDescriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.provider_id == provider_id)
    }
}
