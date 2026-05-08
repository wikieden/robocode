use std::collections::BTreeMap;

use crate::{
    adapters::builtin_provider_descriptors, config::ProviderKind, descriptor::ProviderDescriptor,
    plugin::discover_plugin_descriptors,
};

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
        let mut by_id = BTreeMap::<String, ProviderDescriptor>::new();
        for descriptor in builtin_provider_descriptors() {
            by_id.insert(descriptor.provider_id.clone(), descriptor);
        }
        for loaded in discover_plugin_descriptors()? {
            if by_id.contains_key(&loaded.descriptor.provider_id) {
                return Err(format!(
                    "Provider plugin `{}` conflicts with an existing provider id",
                    loaded.descriptor.provider_id
                ));
            }
            by_id.insert(loaded.descriptor.provider_id.clone(), loaded.descriptor);
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
