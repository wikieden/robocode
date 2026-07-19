use super::{
    state::{TuiEntry, TuiState},
    text::wrap_words,
};

pub(super) fn transcript_rows(state: &TuiState, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for entry in &state.entries {
        append_entry(&mut rows, entry, width);
    }
    if !state.runtime.assistant_stream.is_empty() {
        append_entry(
            &mut rows,
            &TuiEntry {
                label: "assistant".to_string(),
                body: state.runtime.assistant_stream.clone(),
            },
            width,
        );
    }
    for tool in &state.runtime.active_tool_calls {
        append_entry(
            &mut rows,
            &TuiEntry {
                label: "tool-call".to_string(),
                body: format!("{}\n{}", tool.name, tool.input_preview),
            },
            width,
        );
    }
    for evidence in &state.runtime.latest_evidence {
        append_entry(
            &mut rows,
            &TuiEntry {
                label: "evidence".to_string(),
                body: evidence.summary.clone(),
            },
            width,
        );
    }
    for error in &state.runtime.errors {
        append_entry(
            &mut rows,
            &TuiEntry {
                label: "error".to_string(),
                body: error.message.clone(),
            },
            width,
        );
    }
    rows
}

fn append_entry(rows: &mut Vec<String>, entry: &TuiEntry, width: usize) {
    rows.push(entry.label.to_ascii_uppercase());
    rows.extend(
        wrap_words(&entry.body, width.saturating_sub(2))
            .into_iter()
            .map(|line| format!("  {line}")),
    );
    rows.push(String::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_copy_cannot_invent_runtime_errors() {
        let mut state = TuiState::default();
        state.entries.push(TuiEntry {
            label: "assistant".to_string(),
            body: "ERROR fake".to_string(),
        });
        assert!(state.runtime.errors.is_empty());
        assert!(
            transcript_rows(&state, 40)
                .join("\n")
                .contains("ERROR fake")
        );
    }
}
