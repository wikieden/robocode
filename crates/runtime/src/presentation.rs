pub(crate) fn render_section_title(title: &str) -> String {
    format!("{title}:\n")
}

pub(crate) fn render_subsection_title(title: &str) -> String {
    format!("{title}:")
}

pub(crate) fn render_summary_fields(fields: &[(&str, String)]) -> String {
    let values = fields
        .iter()
        .map(|(label, value)| format!("{label}={value}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("  Summary: {values}")
}

pub(crate) fn render_entry_heading(value: &str) -> String {
    format!("  {value}")
}

pub(crate) fn render_field(label: &str, value: impl AsRef<str>) -> String {
    format!("     {label}: {}", value.as_ref())
}

pub(crate) fn render_empty_section(title: &str) -> String {
    join_lines(&[
        render_section_title(title).trim_end().to_string(),
        "  <none>".to_string(),
    ])
}

pub(crate) fn render_permission_denial(tool_name: &str, reason: &str, message: &str) -> String {
    join_lines(&[
        render_section_title("Permission decision")
            .trim_end()
            .to_string(),
        render_summary_fields(&[("decision", "deny".to_string())]),
        String::new(),
        render_subsection_title("Details"),
        render_field("tool", tool_name),
        render_field("reason", reason),
        render_field("message", message),
    ])
}

pub(crate) fn render_diff_view(title: &str, diff: &str) -> String {
    let stats = DiffStats::from_diff(diff);
    let mut lines = vec![
        render_section_title(title).trim_end().to_string(),
        render_summary_fields(&[
            ("files", stats.files.to_string()),
            ("additions", stats.additions.to_string()),
            ("deletions", stats.deletions.to_string()),
        ]),
        String::new(),
        render_subsection_title("Diff"),
    ];
    if stats.is_empty {
        lines.push("  <none>".to_string());
    } else {
        lines.extend(diff.trim_end().lines().map(ToString::to_string));
    }
    join_lines(&lines)
}

pub(crate) fn join_lines<S: AsRef<str>>(lines: &[S]) -> String {
    lines
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join("\n")
}

struct DiffStats {
    files: usize,
    additions: usize,
    deletions: usize,
    is_empty: bool,
}

impl DiffStats {
    fn from_diff(diff: &str) -> Self {
        let trimmed = diff.trim();
        if trimmed.is_empty() || trimmed == "No diff" {
            return Self {
                files: 0,
                additions: 0,
                deletions: 0,
                is_empty: true,
            };
        }
        let files = diff
            .lines()
            .filter(|line| line.starts_with("diff --git "))
            .count()
            .max(1);
        let additions = diff
            .lines()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .count();
        let deletions = diff
            .lines()
            .filter(|line| line.starts_with('-') && !line.starts_with("---"))
            .count();
        Self {
            files,
            additions,
            deletions,
            is_empty: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_section_title_adds_consistent_header_spacing() {
        let rendered = render_section_title("Diagnostics");
        assert_eq!(rendered, "Diagnostics:\n");
    }

    #[test]
    fn render_subsection_title_adds_trailing_colon_without_newline() {
        let rendered = render_subsection_title("src/lib.rs");
        assert_eq!(rendered, "src/lib.rs:");
    }

    #[test]
    fn join_lines_preserves_line_order() {
        let rendered = join_lines(&["alpha", "beta", "gamma"]);
        assert_eq!(rendered, "alpha\nbeta\ngamma");
    }

    #[test]
    fn join_lines_returns_empty_string_for_empty_input() {
        let lines: [&str; 0] = [];
        let rendered = join_lines(&lines);

        assert_eq!(rendered, "");
    }

    #[test]
    fn render_diff_view_counts_changed_lines() {
        let diff = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@\n-old\n+new\n+next\n";
        let rendered = render_diff_view("Git diff", diff);

        assert!(rendered.contains("Summary: files=1 additions=2 deletions=1"));
        assert!(rendered.contains("+next"));
    }

    #[test]
    fn render_permission_denial_uses_structured_details() {
        let rendered = render_permission_denial("write_file", "PlanMode", "blocked");

        assert!(rendered.contains("Permission decision:"));
        assert!(rendered.contains("Summary: decision=deny"));
        assert!(rendered.contains("tool: write_file"));
    }
}
