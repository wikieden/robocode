use std::path::PathBuf;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceSelection {
    pub path: Option<PathBuf>,
}
