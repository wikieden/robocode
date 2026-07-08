use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspLocation {
    pub path: String,
    pub range: LspRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDiagnostic {
    pub path: String,
    pub range: LspRange,
    pub severity: Option<u8>,
    pub source: Option<String>,
    pub code: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspSymbol {
    pub name: String,
    pub kind: u32,
    pub path: String,
    pub range: LspRange,
    pub selection_range: Option<LspRange>,
    pub container_name: Option<String>,
}
