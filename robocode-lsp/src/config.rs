use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerConfig {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub file_extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerRegistry {
    servers: Vec<LspServerConfig>,
}

impl Default for LspServerRegistry {
    fn default() -> Self {
        Self {
            servers: vec![LspServerConfig {
                id: "rust-analyzer".to_string(),
                command: "rust-analyzer".to_string(),
                args: Vec::new(),
                file_extensions: vec!["rs".to_string()],
            }],
        }
    }
}

impl LspServerRegistry {
    pub fn all(&self) -> &[LspServerConfig] {
        &self.servers
    }

    pub fn for_path(&self, path: &Path) -> Option<&LspServerConfig> {
        let ext = path.extension()?.to_string_lossy();
        self.servers.iter().find(|server| {
            server
                .file_extensions
                .iter()
                .any(|candidate| candidate == ext.as_ref())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_resolves_rust_files_to_rust_analyzer() {
        let registry = LspServerRegistry::default();
        let config = registry
            .for_path(Path::new("robocode-core/src/lib.rs"))
            .unwrap();
        assert_eq!(config.id, "rust-analyzer");
        assert!(config.file_extensions.contains(&"rs".to_string()));
    }

    #[test]
    fn registry_returns_none_for_unknown_extension() {
        let registry = LspServerRegistry::default();
        assert!(registry.for_path(Path::new("README.md")).is_none());
    }
}
