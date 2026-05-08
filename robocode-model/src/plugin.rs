use std::ffi::CStr;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use libloading::{Library, Symbol};
use robocode_provider_sdk::{
    PluginDescriptor, PluginDescriptorFn, ProtocolFamily as PluginProtocolFamily,
    ROBOCODE_PLUGIN_DESCRIPTOR_SYMBOL,
};

use crate::{ProtocolFamily, ProviderCapabilities, ProviderDescriptor, ProviderEnvMappings};

pub struct LoadedPluginDescriptor {
    pub source_path: PathBuf,
    pub descriptor: ProviderDescriptor,
    _library: Library,
}

impl std::fmt::Debug for LoadedPluginDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedPluginDescriptor")
            .field("source_path", &self.source_path)
            .field("descriptor", &self.descriptor)
            .finish()
    }
}

pub fn plugin_descriptor_symbol() -> &'static str {
    ROBOCODE_PLUGIN_DESCRIPTOR_SYMBOL
}

#[allow(dead_code)]
pub fn dynamic_library_suffixes() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &["dylib"]
    }
    #[cfg(target_os = "linux")]
    {
        &["so"]
    }
    #[cfg(target_os = "windows")]
    {
        &["dll"]
    }
}

fn default_plugin_search_dirs() -> Vec<PathBuf> {
    std::env::var_os("ROBOCODE_PROVIDER_PLUGIN_DIRS")
        .map(|raw| std::env::split_paths(&raw).collect())
        .unwrap_or_default()
}

fn into_provider_descriptor(descriptor: PluginDescriptor) -> ProviderDescriptor {
    ProviderDescriptor {
        provider_id: descriptor.provider_id,
        display_name: descriptor.display_name,
        version: descriptor.version,
        protocol_family: match descriptor.protocol_family {
            PluginProtocolFamily::Anthropic => ProtocolFamily::Anthropic,
            PluginProtocolFamily::OpenAi => ProtocolFamily::OpenAi,
        },
        default_api_base: descriptor.default_api_base,
        default_model: descriptor.default_model,
        env_mappings: ProviderEnvMappings {
            api_key_env: descriptor.env_mappings.api_key_env,
            api_base_env: descriptor.env_mappings.api_base_env,
        },
        capabilities: ProviderCapabilities {
            supports_streaming: descriptor.capabilities.supports_streaming,
            supports_native_tool_calling: descriptor.capabilities.supports_native_tool_calling,
        },
        config_schema_version: descriptor.config_schema_version,
    }
}

#[allow(dead_code)]
pub fn load_plugin_descriptor(path: &Path) -> Result<LoadedPluginDescriptor, String> {
    let library = unsafe { Library::new(path) }
        .map_err(|err| format!("Failed to load provider plugin {}: {err}", path.display()))?;
    let descriptor_fn: Symbol<'_, PluginDescriptorFn> = unsafe {
        library
            .get(plugin_descriptor_symbol().as_bytes())
            .map_err(|err| {
                format!(
                    "Failed to load descriptor symbol `{}` from {}: {err}",
                    plugin_descriptor_symbol(),
                    path.display()
                )
            })?
    };
    let descriptor_ptr = unsafe { descriptor_fn() };
    if descriptor_ptr.is_null() {
        return Err(format!(
            "Provider plugin {} returned a null descriptor pointer",
            path.display()
        ));
    }
    let descriptor_json = unsafe { CStr::from_ptr(descriptor_ptr) }
        .to_str()
        .map_err(|err| {
            format!(
                "Provider plugin {} returned a non-UTF8 descriptor: {err}",
                path.display()
            )
        })?;
    let descriptor = serde_json::from_str::<PluginDescriptor>(descriptor_json).map_err(|err| {
        format!(
            "Failed to decode provider descriptor from {}: {err}",
            path.display()
        )
    })?;

    Ok(LoadedPluginDescriptor {
        source_path: path.to_path_buf(),
        descriptor: into_provider_descriptor(descriptor),
        _library: library,
    })
}

pub fn discover_plugin_descriptors() -> Result<Vec<LoadedPluginDescriptor>, String> {
    let mut loaded = Vec::new();
    for dir in default_plugin_search_dirs() {
        if !dir.exists() {
            continue;
        }
        let entries = std::fs::read_dir(&dir).map_err(|err| {
            format!(
                "Failed to read provider plugin dir {}: {err}",
                dir.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            let Some(ext) = path.extension().and_then(OsStr::to_str) else {
                continue;
            };
            if dynamic_library_suffixes()
                .iter()
                .any(|suffix| suffix == &ext)
            {
                loaded.push(load_plugin_descriptor(&path)?);
            }
        }
    }
    Ok(loaded)
}
