#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerConfig {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub file_extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LspServerRegistry {
    servers: Vec<LspServerConfig>,
}
