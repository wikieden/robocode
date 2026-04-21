use std::path::Path;

use robocode_types::{LspDiagnostic, LspLocation, LspPosition, LspSymbol};

use crate::LspServerRegistry;

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
        LspRuntimeStatus::default()
    }
}
