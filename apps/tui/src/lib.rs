//! Viden TUI app boundary.
//!
//! The current terminal UI still lives in `viden-cli` during the first
//! structural migration. This crate marks the app boundary that future TUI work
//! should move into: it may render runtime snapshots and send runtime commands,
//! but it must not own provider, tool, or workflow business logic.

use viden_types::{RuntimeCommand, RuntimeSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiAppBoundary {
    pub app_id: &'static str,
    pub consumes_runtime_snapshots: bool,
    pub emits_runtime_commands: bool,
}

impl TuiAppBoundary {
    pub const fn current() -> Self {
        Self {
            app_id: "viden-tui",
            consumes_runtime_snapshots: true,
            emits_runtime_commands: true,
        }
    }

    pub fn accepts_runtime_contract(
        &self,
        _snapshot: &RuntimeSnapshot,
        _command: &RuntimeCommand,
    ) -> bool {
        self.consumes_runtime_snapshots && self.emits_runtime_commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tui_boundary_is_an_app_layer() {
        let boundary = TuiAppBoundary::current();
        assert_eq!(boundary.app_id, "viden-tui");
        assert!(boundary.consumes_runtime_snapshots);
        assert!(boundary.emits_runtime_commands);
    }
}
