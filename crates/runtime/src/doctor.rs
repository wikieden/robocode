use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DependencyStatus {
    Ok,
    Missing,
    #[cfg_attr(not(test), allow(dead_code))]
    NotRequired,
}

impl DependencyStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Missing => "missing",
            Self::NotRequired => "not required for current path",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DoctorReport {
    checks: Vec<(&'static str, DependencyStatus)>,
}

impl DoctorReport {
    pub(crate) fn from_probe<F>(mut probe: F) -> Self
    where
        F: FnMut(&str) -> DependencyStatus,
    {
        Self {
            checks: ["git", "rg", "sqlite3", "curl"]
                .into_iter()
                .map(|tool| (tool, probe(tool)))
                .collect(),
        }
    }

    pub(crate) fn render(&self) -> String {
        let mut lines = vec!["Environment diagnostics:".to_string()];
        lines.extend(
            self.checks
                .iter()
                .map(|(tool, status)| format!("  {tool}: {}", status.label())),
        );
        lines.join("\n")
    }
}

pub(crate) fn system_dependency_status(tool: &str) -> DependencyStatus {
    match Command::new(tool).arg("--version").output() {
        Ok(output) if output.status.success() => DependencyStatus::Ok,
        Ok(_) => DependencyStatus::Missing,
        Err(_) => DependencyStatus::Missing,
    }
}
