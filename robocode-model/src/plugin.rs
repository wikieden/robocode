use std::ffi::CStr;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use libloading::{Library, Symbol};
use robocode_provider_sdk::{
    PluginDescriptor, PluginDescriptorFn, ProtocolFamily as PluginProtocolFamily,
    ROBOCODE_PLUGIN_DESCRIPTOR_SYMBOL,
};

use crate::{
    ProtocolFamily, ProviderCapabilities, ProviderDescriptor, ProviderEnvMappings,
    descriptor::validate_provider_descriptor,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderPluginErrorKind {
    ReadDirectory,
    ReadDirectoryEntry,
    LoadLibrary,
    MissingDescriptorSymbol,
    NullDescriptor,
    NonUtf8Descriptor,
    DecodeDescriptor,
    InvalidDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPluginError {
    pub kind: ProviderPluginErrorKind,
    pub path: PathBuf,
    pub message: String,
}

impl ProviderPluginError {
    fn new(
        kind: ProviderPluginErrorKind,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ProviderPluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ProviderPluginErrorKind::ReadDirectory => {
                write!(
                    f,
                    "Failed to read provider plugin dir {}: {}",
                    self.path.display(),
                    self.message
                )
            }
            ProviderPluginErrorKind::ReadDirectoryEntry => {
                write!(
                    f,
                    "Failed to read provider plugin dir entry under {}: {}",
                    self.path.display(),
                    self.message
                )
            }
            ProviderPluginErrorKind::LoadLibrary => {
                write!(
                    f,
                    "Failed to load provider plugin {}: {}",
                    self.path.display(),
                    self.message
                )
            }
            ProviderPluginErrorKind::MissingDescriptorSymbol => {
                write!(
                    f,
                    "Failed to load descriptor symbol `{}` from {}: {}",
                    plugin_descriptor_symbol(),
                    self.path.display(),
                    self.message
                )
            }
            ProviderPluginErrorKind::NullDescriptor => {
                write!(
                    f,
                    "Provider plugin {} returned a null descriptor pointer",
                    self.path.display()
                )
            }
            ProviderPluginErrorKind::NonUtf8Descriptor => {
                write!(
                    f,
                    "Provider plugin {} returned a non-UTF8 descriptor: {}",
                    self.path.display(),
                    self.message
                )
            }
            ProviderPluginErrorKind::DecodeDescriptor => {
                write!(
                    f,
                    "Failed to decode provider descriptor from {}: {}",
                    self.path.display(),
                    self.message
                )
            }
            ProviderPluginErrorKind::InvalidDescriptor => {
                write!(
                    f,
                    "Invalid provider descriptor from {}: {}",
                    self.path.display(),
                    self.message
                )
            }
        }
    }
}

impl std::error::Error for ProviderPluginError {}

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
pub fn load_plugin_descriptor(path: &Path) -> Result<LoadedPluginDescriptor, ProviderPluginError> {
    let library = unsafe { Library::new(path) }.map_err(|err| {
        ProviderPluginError::new(ProviderPluginErrorKind::LoadLibrary, path, err.to_string())
    })?;
    let descriptor_fn: Symbol<'_, PluginDescriptorFn> = unsafe {
        library
            .get(plugin_descriptor_symbol().as_bytes())
            .map_err(|err| {
                ProviderPluginError::new(
                    ProviderPluginErrorKind::MissingDescriptorSymbol,
                    path,
                    err.to_string(),
                )
            })?
    };
    let descriptor_ptr = unsafe { descriptor_fn() };
    if descriptor_ptr.is_null() {
        return Err(ProviderPluginError::new(
            ProviderPluginErrorKind::NullDescriptor,
            path,
            "descriptor pointer was null",
        ));
    }
    let descriptor_json = unsafe { CStr::from_ptr(descriptor_ptr) }
        .to_str()
        .map_err(|err| {
            ProviderPluginError::new(
                ProviderPluginErrorKind::NonUtf8Descriptor,
                path,
                err.to_string(),
            )
        })?;
    let descriptor = serde_json::from_str::<PluginDescriptor>(descriptor_json).map_err(|err| {
        ProviderPluginError::new(
            ProviderPluginErrorKind::DecodeDescriptor,
            path,
            err.to_string(),
        )
    })?;
    let descriptor = into_provider_descriptor(descriptor);
    validate_provider_descriptor(&descriptor).map_err(|err| {
        ProviderPluginError::new(ProviderPluginErrorKind::InvalidDescriptor, path, err)
    })?;

    Ok(LoadedPluginDescriptor {
        source_path: path.to_path_buf(),
        descriptor,
        _library: library,
    })
}

pub fn discover_plugin_descriptors() -> Result<Vec<LoadedPluginDescriptor>, ProviderPluginError> {
    discover_plugin_descriptors_in_dirs(default_plugin_search_dirs())
}

pub(crate) fn discover_plugin_descriptors_in_dirs(
    dirs: Vec<PathBuf>,
) -> Result<Vec<LoadedPluginDescriptor>, ProviderPluginError> {
    let mut loaded = Vec::new();
    for path in discover_plugin_paths(dirs)? {
        loaded.push(load_plugin_descriptor(&path)?);
    }
    Ok(loaded)
}

fn discover_plugin_paths(dirs: Vec<PathBuf>) -> Result<Vec<PathBuf>, ProviderPluginError> {
    let mut discovered = Vec::new();
    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        let entries = std::fs::read_dir(&dir).map_err(|err| {
            ProviderPluginError::new(
                ProviderPluginErrorKind::ReadDirectory,
                &dir,
                err.to_string(),
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| {
                ProviderPluginError::new(
                    ProviderPluginErrorKind::ReadDirectoryEntry,
                    &dir,
                    err.to_string(),
                )
            })?;
            let path = entry.path();
            let Some(ext) = path.extension().and_then(OsStr::to_str) else {
                continue;
            };
            if dynamic_library_suffixes()
                .iter()
                .any(|suffix| suffix == &ext)
            {
                discovered.push(path);
            }
        }
    }
    discovered.sort();
    Ok(discovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("robocode_plugin_{name}_{nanos}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn discovery_skips_missing_plugin_dirs() {
        let dir = temp_dir("missing_parent").join("missing");
        let paths = discover_plugin_paths(vec![dir]).unwrap();
        assert!(paths.is_empty());
    }

    #[test]
    fn discovery_filters_to_platform_dynamic_libraries() {
        let dir = temp_dir("suffix_filter");
        let expected = dir.join(format!("provider.{}", dynamic_library_suffixes()[0]));
        std::fs::write(&expected, b"not a real library").unwrap();
        std::fs::write(dir.join("provider.txt"), b"ignored").unwrap();
        std::fs::write(dir.join("provider"), b"ignored").unwrap();

        let paths = discover_plugin_paths(vec![dir]).unwrap();
        assert_eq!(paths, vec![expected]);
    }

    #[test]
    fn discovery_returns_stable_sorted_paths_across_multiple_dirs() {
        let first_dir = temp_dir("multi_dir_z");
        let second_dir = temp_dir("multi_dir_a");
        let suffix = dynamic_library_suffixes()[0];
        let later = first_dir.join(format!("z_provider.{suffix}"));
        let earlier = second_dir.join(format!("a_provider.{suffix}"));
        let middle = second_dir.join(format!("m_provider.{suffix}"));
        std::fs::write(&later, b"not a real library").unwrap();
        std::fs::write(&earlier, b"not a real library").unwrap();
        std::fs::write(&middle, b"not a real library").unwrap();

        let paths = discover_plugin_paths(vec![first_dir, second_dir]).unwrap();

        assert_eq!(paths, vec![earlier, middle, later]);
    }

    #[test]
    fn loading_invalid_dynamic_library_reports_source_path() {
        let dir = temp_dir("invalid_library");
        let path = dir.join(format!("broken.{}", dynamic_library_suffixes()[0]));
        std::fs::write(&path, b"not a real library").unwrap();

        let err = load_plugin_descriptor(&path).unwrap_err();

        assert_eq!(err.kind, ProviderPluginErrorKind::LoadLibrary);
        assert_eq!(err.path, path);
        assert!(
            err.to_string().contains(&path.display().to_string()),
            "{err}"
        );
    }

    #[test]
    fn discovery_reports_unreadable_plugin_dir_path() {
        let path = temp_dir("not_a_dir").join("plugin-dir-file");
        std::fs::write(&path, b"not a directory").unwrap();

        let err = discover_plugin_paths(vec![path.clone()]).unwrap_err();
        assert_eq!(err.kind, ProviderPluginErrorKind::ReadDirectory);
        assert_eq!(err.path, path);
        assert!(
            err.to_string().contains(&path.display().to_string()),
            "{err}"
        );
    }
}
