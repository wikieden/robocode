use std::path::Path;

use robocode_types::{LspDiagnostic, LspLocation, LspPosition, LspSymbol};

use crate::{LspServerConfig, LspServerRegistry};

pub trait SemanticProvider: Send + Sync {
    fn diagnostics(&self, cwd: &Path, path: &Path) -> Result<Vec<LspDiagnostic>, String>;

    fn symbols(&self, cwd: &Path, path: &Path) -> Result<Vec<LspSymbol>, String>;

    fn references(
        &self,
        cwd: &Path,
        path: &Path,
        position: LspPosition,
    ) -> Result<Vec<LspLocation>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LspRuntimeStatus {
    pub configured_servers: Vec<String>,
    pub running_servers: Vec<String>,
    pub last_error: Option<String>,
}

pub struct LspRuntime {
    #[allow(dead_code)]
    registry: LspServerRegistry,
}

impl LspRuntime {
    pub fn new(registry: LspServerRegistry) -> Self {
        Self { registry }
    }

    pub fn status(&self) -> LspRuntimeStatus {
        LspRuntimeStatus {
            configured_servers: self
                .registry
                .all()
                .iter()
                .map(|server| server.id.clone())
                .collect(),
            running_servers: Vec::new(),
            last_error: None,
        }
    }

    fn server_for_path<'a>(&'a self, path: &Path) -> Result<&'a LspServerConfig, String> {
        self.registry
            .for_path(path)
            .ok_or_else(|| format!("No configured language server for {}", path.display()))
    }
}

impl SemanticProvider for LspRuntime {
    fn diagnostics(&self, _cwd: &Path, path: &Path) -> Result<Vec<LspDiagnostic>, String> {
        let server = self.server_for_path(path)?;
        Err(format!(
            "Language server query execution is not available yet for {}",
            server.id
        ))
    }

    fn symbols(&self, _cwd: &Path, path: &Path) -> Result<Vec<LspSymbol>, String> {
        let server = self.server_for_path(path)?;
        Err(format!(
            "Language server query execution is not available yet for {}",
            server.id
        ))
    }

    fn references(
        &self,
        _cwd: &Path,
        path: &Path,
        _position: LspPosition,
    ) -> Result<Vec<LspLocation>, String> {
        let server = self.server_for_path(path)?;
        Err(format!(
            "Language server query execution is not available yet for {}",
            server.id
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::LspServerRegistry;

    #[test]
    fn status_reports_configured_servers() {
        let runtime = LspRuntime::new(LspServerRegistry::default());
        let status = runtime.status();
        assert_eq!(status.configured_servers, vec!["rust-analyzer"]);
        assert!(status.running_servers.is_empty());
        assert!(status.last_error.is_none());
    }

    #[test]
    fn diagnostics_returns_clean_error_for_unconfigured_path() {
        let runtime = LspRuntime::new(LspServerRegistry::default());
        let error = runtime
            .diagnostics(Path::new("."), Path::new("README.md"))
            .unwrap_err();
        assert_eq!("No configured language server for README.md", error);
    }
}
