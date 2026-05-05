pub(crate) fn render_section_title(title: &str) -> String {
    format!("{title}:\n")
}

pub(crate) fn join_lines<S: AsRef<str>>(lines: &[S]) -> String {
    lines
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join("\n")
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
}
