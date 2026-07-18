use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use viden_types::{ContextContentKind, ContextQualityRecord};

const REDUCER_ID: &str = "viden-context-native";
const REDUCER_VERSION: &str = "native-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReductionPolicy {
    pub max_output_bytes: usize,
    pub max_output_tokens: u64,
    pub max_input_bytes: usize,
    pub max_json_depth: usize,
    pub max_json_values: usize,
    /// Required markers are semantic retained marker ids, such as `key:errors`.
    /// Use `literal:<text>` only when the policy intentionally requires exact
    /// output text. Empty markers are invalid policy.
    pub required_markers: Vec<String>,
    pub selected_line_ranges: Vec<LineRange>,
    pub recent_turns: usize,
}

impl Default for ReductionPolicy {
    fn default() -> Self {
        Self {
            max_output_bytes: 8 * 1024,
            max_output_tokens: 2_000,
            max_input_bytes: 2 * 1024 * 1024,
            max_json_depth: 64,
            max_json_values: 20_000,
            required_markers: Vec::new(),
            selected_line_ranges: Vec::new(),
            recent_turns: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReductionEstimate {
    pub byte_count: usize,
    pub token_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReductionOmission {
    pub reason: String,
    pub omitted_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReductionResult {
    pub content: String,
    pub original: ReductionEstimate,
    pub reduced: ReductionEstimate,
    pub omissions: Vec<ReductionOmission>,
    pub retained_markers: Vec<String>,
    pub reducer_id: String,
    pub reducer_version: String,
    pub quality: ContextQualityRecord,
    pub fallback_raw: bool,
}

pub fn reduce(
    kind: ContextContentKind,
    input: &[u8],
    policy: &ReductionPolicy,
) -> Result<ReductionResult, crate::ContextError> {
    validate_policy(policy)?;
    if input.len() > policy.max_input_bytes {
        return Err(crate::ContextError::ReductionInputTooLarge {
            byte_count: input.len(),
            max_input_bytes: policy.max_input_bytes,
        });
    }

    let original = estimate_bytes(input);
    let mut view = match kind {
        ContextContentKind::Json => reduce_json(input, policy),
        ContextContentKind::Code => reduce_code(input, policy),
        ContextContentKind::Diff => reduce_diff(input, policy),
        ContextContentKind::Log | ContextContentKind::Diagnostic => reduce_log(input, policy),
        ContextContentKind::Transcript | ContextContentKind::Text => reduce_text(input, policy),
    };
    if kind != ContextContentKind::Json {
        let redacted = redact_text(&view.content);
        if redacted != view.content {
            view.content = redacted;
            view.omissions.push(omission("secret_values_redacted", 1));
        }
    }
    if kind != ContextContentKind::Json || view.fallback_raw {
        bound_output(&mut view.content, policy, &mut view.omissions);
    }
    view.retained_markers.sort();
    view.retained_markers.dedup();
    view.original = original;
    view.reduced = estimate_str(&view.content);
    view.quality = quality_record(&kind, &view.content, true, Vec::new(), None);

    let missing_markers = policy
        .required_markers
        .iter()
        .filter(|marker| !required_marker_retained(marker, &view))
        .map(|marker| redact_text(marker))
        .collect::<Vec<_>>();
    if !missing_markers.is_empty() {
        let quality = quality_record(
            &kind,
            &view.content,
            false,
            vec!["required_markers_present".to_string()],
            Some(format!(
                "missing required markers: {}",
                missing_markers.join(", ")
            )),
        );
        return Err(crate::ContextError::QualityFailed {
            missing_markers,
            quality: Box::new(quality),
        });
    }

    Ok(view)
}

fn reduce_json(input: &[u8], policy: &ReductionPolicy) -> ReductionResult {
    match serde_json::from_slice::<serde_json::Value>(input) {
        Ok(mut value) => {
            let mut omissions = Vec::new();
            if let Some(reason) = json_limit_reason(&value, policy) {
                omissions.push(omission(reason, 1));
                omissions.push(omission("minimal_json_fallback", 1));
                let mut view = result("0".to_string(), Vec::new(), false);
                view.omissions = omissions;
                return view;
            }
            redact_json_value(&mut value, &mut omissions);
            let content = bounded_json_content(&value, policy, &mut omissions);
            let retained_markers = collect_json_markers_from_content(&content);
            let mut view = result(content, retained_markers, false);
            view.omissions = omissions;
            view
        }
        Err(_) => raw_fallback(input, policy, "parse_failure"),
    }
}

fn bounded_json_content(
    value: &serde_json::Value,
    policy: &ReductionPolicy,
    omissions: &mut Vec<ReductionOmission>,
) -> String {
    let limit = effective_max_bytes(policy);
    let content = serde_json::to_string_pretty(value).expect("serializing JSON value");
    if content.len() <= limit {
        return content;
    }

    omissions.push(omission("json_values_pruned", count_json_values(value)));
    omissions.push(omission("size_bound", content.len().saturating_sub(limit)));

    if let Some(candidate_content) = pruned_json_content(value, limit) {
        return candidate_content;
    }

    if "0".len() <= limit {
        omissions.push(omission("minimal_json_fallback", 1));
        return "0".to_string();
    }

    "null".to_string()
}

fn pruned_json_content(value: &serde_json::Value, limit: usize) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            if "{}".len() > limit {
                return None;
            }
            let mut lines = Vec::new();
            for (key, _) in map {
                if is_secret_label(key) {
                    continue;
                }
                let encoded_key = serde_json::to_string(key).expect("serializing JSON key");
                let next_line = format!("  {encoded_key}: null");
                let mut candidate_lines = lines.clone();
                candidate_lines.push(next_line);
                let candidate = format!("{{\n{}\n}}", candidate_lines.join(",\n"));
                if candidate.len() <= limit {
                    lines = candidate_lines;
                }
            }
            if lines.is_empty() {
                None
            } else {
                Some(format!("{{\n{}\n}}", lines.join(",\n")))
            }
        }
        serde_json::Value::Array(values) => {
            if values.is_empty() || "[\n  null\n]".len() > limit {
                ("[]".len() <= limit).then(|| "[]".to_string())
            } else {
                Some("[\n  null\n]".to_string())
            }
        }
        _ => ("0".len() <= limit).then(|| "0".to_string()),
    }
}

fn count_json_values(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => 1 + map.values().map(count_json_values).sum::<usize>(),
        serde_json::Value::Array(values) => 1 + values.iter().map(count_json_values).sum::<usize>(),
        _ => 1,
    }
}

fn collect_json_markers(value: &serde_json::Value, markers: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                markers.push(format!("key:{key}"));
                collect_json_markers(value, markers);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_json_markers(value, markers);
            }
        }
        _ => {}
    }
}

fn collect_json_markers_from_content(content: &str) -> Vec<String> {
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(value) => {
            let mut markers = Vec::new();
            collect_json_markers(&value, &mut markers);
            markers
        }
        Err(_) => Vec::new(),
    }
}

fn json_limit_reason(value: &serde_json::Value, policy: &ReductionPolicy) -> Option<&'static str> {
    let mut stack = vec![(value, 1_usize)];
    let mut values = 0_usize;
    while let Some((current, depth)) = stack.pop() {
        values = values.saturating_add(1);
        if values > policy.max_json_values {
            return Some("json_value_limit");
        }
        if depth > policy.max_json_depth {
            return Some("json_depth_limit");
        }
        match current {
            serde_json::Value::Object(map) => {
                for child in map.values() {
                    stack.push((child, depth.saturating_add(1)));
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    stack.push((child, depth.saturating_add(1)));
                }
            }
            _ => {}
        }
    }
    None
}

fn redact_json_value(value: &mut serde_json::Value, omissions: &mut Vec<ReductionOmission>) {
    let mut redacted = 0_usize;
    redact_json_value_inner(value, None, &mut redacted);
    if redacted > 0 {
        omissions.push(omission("secret_values_redacted", redacted));
    }
}

fn redact_json_value_inner(value: &mut serde_json::Value, key: Option<&str>, redacted: &mut usize) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                redact_json_value_inner(child, Some(key), redacted);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                redact_json_value_inner(child, key, redacted);
            }
        }
        serde_json::Value::String(text) => {
            if key.is_some_and(is_secret_label) {
                *text = "[REDACTED]".to_string();
                *redacted += 1;
            } else {
                let next = redact_text(text);
                if next != *text {
                    *text = next;
                    *redacted += 1;
                }
            }
        }
        _ => {}
    }
}

fn reduce_code(input: &[u8], policy: &ReductionPolicy) -> ReductionResult {
    let text = String::from_utf8_lossy(input);
    let all_lines = text.lines().collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut omitted = 0;
    let mut retained_markers = Vec::new();
    let mut seen = HashSet::new();
    let mut index = 0;

    while index < all_lines.len() {
        let line = all_lines[index];
        let line_number = index + 1;
        let trimmed = line.trim_start();
        let attribute_start = trimmed.starts_with("#[");
        let declaration = if attribute_start {
            next_declaration_marker(&all_lines, index)
        } else {
            code_declaration_marker(trimmed)
        };
        let selected = policy
            .selected_line_ranges
            .iter()
            .any(|range| line_number >= range.start && line_number <= range.end);

        if trimmed.starts_with("use ")
            || trimmed.starts_with("pub use ")
            || trimmed.starts_with("mod ")
        {
            let (block, end_index) = scan_code_statement(&all_lines, index, CodeScanKind::Import);
            push_unique(&mut lines, &mut seen, block);
            retained_markers.push("import_or_module".to_string());
            index = end_index;
        } else if let Some(marker) = declaration.as_ref() {
            let (block, end_index) =
                scan_code_statement(&all_lines, index, CodeScanKind::Declaration);
            push_unique(&mut lines, &mut seen, block);
            retained_markers.push(marker.clone());
            index = end_index;
        }

        if selected {
            push_unique(&mut lines, &mut seen, format!("L{line_number}: {line}"));
            retained_markers.push("selected_range".to_string());
        } else if !trimmed.is_empty()
            && !trimmed.starts_with("use ")
            && !trimmed.starts_with("pub use ")
            && !trimmed.starts_with("mod ")
            && !attribute_start
            && declaration.is_none()
        {
            omitted += 1;
        }
        index += 1;
    }

    let mut view = result(lines.join("\n"), retained_markers, false);
    if omitted > 0 {
        view.omissions.push(omission("code_body_omitted", omitted));
    }
    view
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodeScanKind {
    Import,
    Declaration,
}

fn scan_code_statement(lines: &[&str], start: usize, kind: CodeScanKind) -> (String, usize) {
    let mut captured = Vec::new();
    let mut balance = 0_i64;
    let mut end = start;

    for (offset, line) in lines[start..].iter().enumerate() {
        captured.push((*line).to_string());
        balance += match kind {
            CodeScanKind::Import => delimiter_delta(line),
            CodeScanKind::Declaration => signature_delta(line),
        };
        end = start + offset;

        let trimmed = line.trim_end();
        let complete = match kind {
            CodeScanKind::Import => trimmed.ends_with(';') && balance <= 0,
            CodeScanKind::Declaration => declaration_header_complete(trimmed, balance),
        };
        if complete {
            break;
        }
    }

    (captured.join("\n"), end)
}

fn declaration_header_complete(trimmed: &str, balance: i64) -> bool {
    balance <= 0
        && (trimmed.ends_with(';')
            || trimmed.ends_with('{')
            || trimmed.contains("{ ")
            || trimmed.contains("{\t")
            || trimmed.contains("{}")
            || trimmed.contains("{ //")
            || trimmed.contains("{ /*"))
}

fn delimiter_delta(line: &str) -> i64 {
    let mut delta = 0;
    for character in line.chars() {
        match character {
            '(' | '[' | '{' => delta += 1,
            ')' | ']' | '}' => delta -= 1,
            _ => {}
        }
    }
    delta
}

fn signature_delta(line: &str) -> i64 {
    let mut delta = 0;
    for character in line.chars() {
        match character {
            '(' | '[' => delta += 1,
            ')' | ']' => delta -= 1,
            _ => {}
        }
    }
    delta
}

fn code_declaration_marker(trimmed: &str) -> Option<String> {
    let normalized = trimmed
        .replace("(", " ")
        .replace(")", " ")
        .replace("{", " ");
    let words = normalized.split_whitespace().collect::<Vec<_>>();
    if words.contains(&"impl") {
        Some("declaration:impl".to_string())
    } else if words.contains(&"struct") {
        Some("declaration:struct".to_string())
    } else if words.contains(&"enum") {
        Some("declaration:enum".to_string())
    } else if words.contains(&"trait") {
        Some("declaration:trait".to_string())
    } else if words.contains(&"fn") {
        Some("declaration:fn".to_string())
    } else if words.contains(&"type") {
        Some("declaration:type".to_string())
    } else if words.contains(&"const") {
        Some("declaration:const".to_string())
    } else if words.contains(&"static") {
        Some("declaration:static".to_string())
    } else {
        None
    }
}

fn next_declaration_marker(lines: &[&str], start: usize) -> Option<String> {
    for line in &lines[start..] {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[") || trimmed.is_empty() {
            continue;
        }
        return code_declaration_marker(trimmed);
    }
    None
}

fn reduce_diff(input: &[u8], _policy: &ReductionPolicy) -> ReductionResult {
    let text = String::from_utf8_lossy(input);
    let mut lines = Vec::new();
    let mut omitted = 0;
    let mut retained_markers = Vec::new();

    for line in text.lines() {
        if line.starts_with("diff --git ") {
            lines.push(line.to_string());
            retained_markers.push("diff_file".to_string());
        } else if line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("@@ ")
        {
            lines.push(line.to_string());
            if line.starts_with("@@ ") {
                retained_markers.push("diff_hunk".to_string());
            }
        } else if is_changed_diff_line(line) {
            lines.push(line.to_string());
            retained_markers.push("changed_line".to_string());
            if line.contains("unsafe") || line.contains("TODO") {
                retained_markers.push("risky_change".to_string());
            }
        } else if !line.trim().is_empty() {
            omitted += 1;
        }
    }

    let mut view = result(lines.join("\n"), retained_markers, false);
    if omitted > 0 {
        view.omissions
            .push(omission("diff_context_omitted", omitted));
    }
    view
}

fn is_changed_diff_line(line: &str) -> bool {
    (line.starts_with('+') && !line.starts_with("+++ "))
        || (line.starts_with('-') && !line.starts_with("--- "))
}

fn reduce_log(input: &[u8], _policy: &ReductionPolicy) -> ReductionResult {
    let text = String::from_utf8_lossy(input);
    let all_lines = text.lines().collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut seen_errors = HashSet::new();
    let mut first_failure_kept = false;
    let mut omitted_duplicates = 0;
    let mut retained_markers = Vec::new();

    for line in &all_lines {
        let lower = line.to_ascii_lowercase();
        let interesting = lower.contains("error")
            || lower.contains("failed")
            || lower.contains("failure")
            || lower.contains("panicked")
            || lower.contains("panic");
        if interesting {
            if !first_failure_kept {
                lines.push((*line).to_string());
                retained_markers.push("first_failure".to_string());
                seen_errors.insert((*line).to_string());
                first_failure_kept = true;
            } else if seen_errors.insert((*line).to_string()) {
                lines.push((*line).to_string());
                retained_markers.push("unique_error".to_string());
            } else {
                omitted_duplicates += 1;
            }
        }
    }

    for line in all_lines
        .iter()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        if !lines.iter().any(|kept| kept == line) {
            lines.push((*line).to_string());
            retained_markers.push("tail".to_string());
        }
    }

    let mut view = result(lines.join("\n"), retained_markers, false);
    let omitted = all_lines.len().saturating_sub(lines.len()) + omitted_duplicates;
    if omitted > 0 {
        view.omissions
            .push(omission("log_lines_omitted_or_deduplicated", omitted));
    }
    view
}

fn reduce_text(input: &[u8], policy: &ReductionPolicy) -> ReductionResult {
    let text = String::from_utf8_lossy(input);
    let all_lines = text.lines().collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut seen = HashSet::new();
    let mut retained_markers = Vec::new();

    for line in &all_lines {
        let lower = line.to_ascii_lowercase();
        if lower.contains("constraint:") || lower.contains("must ") || lower.contains("do not ") {
            push_unique(&mut lines, &mut seen, (*line).to_string());
            retained_markers.push("constraint".to_string());
        } else if lower.contains("decision:") || lower.contains("decided") {
            push_unique(&mut lines, &mut seen, (*line).to_string());
            retained_markers.push("decision".to_string());
        } else if lower.contains("unresolved")
            || lower.contains("question:")
            || lower.ends_with('?')
        {
            push_unique(&mut lines, &mut seen, (*line).to_string());
            retained_markers.push("question".to_string());
        }
    }

    let recent_count = policy.recent_turns.min(all_lines.len());
    for line in all_lines
        .iter()
        .skip(all_lines.len().saturating_sub(recent_count))
    {
        push_unique(&mut lines, &mut seen, (*line).to_string());
        retained_markers.push("recent_turn".to_string());
    }

    let mut view = result(lines.join("\n"), retained_markers, false);
    let omitted = all_lines.len().saturating_sub(lines.len());
    if omitted > 0 {
        view.omissions.push(omission("text_lines_omitted", omitted));
    }
    view
}

fn raw_fallback(input: &[u8], policy: &ReductionPolicy, reason: &str) -> ReductionResult {
    let mut content = String::from_utf8_lossy(input).into_owned();
    let mut view = result(
        std::mem::take(&mut content),
        vec!["raw_fallback".to_string()],
        true,
    );
    view.omissions.push(omission(reason, 1));
    bound_output(&mut view.content, policy, &mut view.omissions);
    view
}

fn result(content: String, retained_markers: Vec<String>, fallback_raw: bool) -> ReductionResult {
    ReductionResult {
        content,
        original: ReductionEstimate {
            byte_count: 0,
            token_count: 0,
        },
        reduced: ReductionEstimate {
            byte_count: 0,
            token_count: 0,
        },
        omissions: Vec::new(),
        retained_markers,
        reducer_id: REDUCER_ID.to_string(),
        reducer_version: REDUCER_VERSION.to_string(),
        quality: quality_record(&ContextContentKind::Text, "", true, Vec::new(), None),
        fallback_raw,
    }
}

fn bound_output(
    content: &mut String,
    policy: &ReductionPolicy,
    omissions: &mut Vec<ReductionOmission>,
) {
    let token_byte_limit = token_byte_limit(policy);
    let effective_max_bytes = effective_max_bytes(policy);
    let bound_reason = if token_byte_limit < policy.max_output_bytes {
        "token_bound"
    } else {
        "size_bound"
    };

    if effective_max_bytes == 0 {
        if !content.is_empty() {
            omissions.push(omission(bound_reason, content.len()));
            content.clear();
        }
        return;
    }

    if content.len() <= effective_max_bytes {
        return;
    }

    let original_len = content.len();
    let mut end = effective_max_bytes;
    while end > 0 && !content.is_char_boundary(end) {
        end -= 1;
    }
    content.truncate(end);
    omissions.push(omission(bound_reason, original_len - end));
}

fn effective_max_bytes(policy: &ReductionPolicy) -> usize {
    policy.max_output_bytes.min(token_byte_limit(policy))
}

fn token_byte_limit(policy: &ReductionPolicy) -> usize {
    (policy.max_output_tokens as usize).saturating_mul(4)
}

fn validate_policy(policy: &ReductionPolicy) -> Result<(), crate::ContextError> {
    if policy.max_output_bytes == 0 {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "max_output_bytes",
            reason: "must be at least 1",
        });
    }
    if policy.max_output_tokens == 0 {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "max_output_tokens",
            reason: "must be at least 1",
        });
    }
    if policy.max_input_bytes == 0 {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "max_input_bytes",
            reason: "must be at least 1",
        });
    }
    if policy.max_json_depth == 0 {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "max_json_depth",
            reason: "must be at least 1",
        });
    }
    if policy.max_json_values == 0 {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "max_json_values",
            reason: "must be at least 1",
        });
    }
    if policy
        .required_markers
        .iter()
        .any(|marker| marker.is_empty())
    {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "required_markers",
            reason: "must not contain empty marker ids",
        });
    }
    Ok(())
}

fn required_marker_retained(marker: &str, view: &ReductionResult) -> bool {
    if let Some(literal) = marker.strip_prefix("literal:") {
        view.content.contains(literal)
    } else {
        view.retained_markers
            .iter()
            .any(|retained| retained == marker)
    }
}

fn redact_text(input: &str) -> String {
    let mut output = Vec::new();
    for line in input.lines() {
        output.push(redact_line(line));
    }
    if input.ends_with('\n') {
        format!("{}\n", output.join("\n"))
    } else {
        output.join("\n")
    }
}

fn redact_line(line: &str) -> String {
    let bearer_redacted = redact_bearer(line);
    redact_assignment(&bearer_redacted)
}

fn redact_bearer(line: &str) -> String {
    let Some(index) = line.to_ascii_lowercase().find("bearer ") else {
        return line.to_string();
    };
    let value_start = index + "bearer ".len();
    let value_end = line[value_start..]
        .find(|character: char| character.is_whitespace() || character == ',' || character == ';')
        .map_or(line.len(), |offset| value_start + offset);
    format!("{}[REDACTED]{}", &line[..value_start], &line[value_end..])
}

fn redact_assignment(line: &str) -> String {
    for separator in ['=', ':'] {
        if let Some(index) = line.find(separator) {
            let label = line[..index].trim();
            if is_secret_label(label) {
                let prefix = &line[..=index];
                let quote = line[index + 1..]
                    .chars()
                    .find(|character| *character == '"' || *character == '\'');
                return match quote {
                    Some(character) => format!("{prefix} {character}[REDACTED]{character}"),
                    None => format!("{prefix} [REDACTED]"),
                };
            }
        }
    }
    line.to_string()
}

fn is_secret_label(label: &str) -> bool {
    let lower = label.to_ascii_lowercase();
    lower.contains("authorization")
        || lower.contains("password")
        || lower.contains("secret")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.ends_with("_token")
        || lower.contains("token")
}

fn push_unique(lines: &mut Vec<String>, seen: &mut HashSet<String>, line: String) {
    if seen.insert(line.clone()) {
        lines.push(line);
    }
}

fn omission(reason: &str, omitted_count: usize) -> ReductionOmission {
    ReductionOmission {
        reason: reason.to_string(),
        omitted_count,
    }
}

fn estimate_bytes(input: &[u8]) -> ReductionEstimate {
    ReductionEstimate {
        byte_count: input.len(),
        token_count: estimate_tokens(input.len()),
    }
}

fn estimate_str(input: &str) -> ReductionEstimate {
    ReductionEstimate {
        byte_count: input.len(),
        token_count: estimate_tokens(input.len()),
    }
}

fn estimate_tokens(byte_count: usize) -> u64 {
    byte_count.div_ceil(4) as u64
}

fn quality_record(
    kind: &ContextContentKind,
    content: &str,
    passed: bool,
    checks: Vec<String>,
    failure_reason: Option<String>,
) -> ContextQualityRecord {
    let mut hasher = Sha256::new();
    hasher.update(format!("{kind:?}:{REDUCER_ID}:{REDUCER_VERSION}:"));
    hasher.update(content.as_bytes());
    let quality_id = format!("ctxq_{}", hex_prefix(&hasher.finalize(), 16));
    ContextQualityRecord {
        quality_id,
        target_id: format!("{REDUCER_ID}:{REDUCER_VERSION}:{kind:?}"),
        passed,
        score_microunits: passed.then_some(1_000_000),
        checks,
        failure_reason,
        checked_at: None,
    }
}

fn hex_prefix(bytes: &[u8], hex_chars: usize) -> String {
    let mut output = String::new();
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
        if output.len() >= hex_chars {
            break;
        }
    }
    output.truncate(hex_chars);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use viden_types::ContextContentKind;

    fn tight_policy(max_bytes: usize) -> ReductionPolicy {
        ReductionPolicy {
            max_output_bytes: max_bytes,
            max_output_tokens: 1_000,
            max_input_bytes: 2 * 1024 * 1024,
            max_json_depth: 64,
            max_json_values: 20_000,
            required_markers: Vec::new(),
            selected_line_ranges: Vec::new(),
            recent_turns: 4,
        }
    }

    #[test]
    fn json_reducer_sorts_keys_and_records_omissions() {
        let input = br#"{"z":3,"errors":[{"path":"src/a.rs","message":"boom"}],"id":"ctx-1","a":{"b":2,"c":1},"items":[1,2,3,4]}"#;

        let view = reduce(ContextContentKind::Json, input, &tight_policy(96)).unwrap();
        let again = reduce(ContextContentKind::Json, input, &tight_policy(96)).unwrap();

        assert_eq!(view.content, again.content);
        assert_eq!(view.reducer_id, "viden-context-native");
        assert_eq!(view.reducer_version, "native-v1");
        assert!(view.content.starts_with("{\n  \"a\":"));
        assert!(view.content.len() <= 96);
        assert!(
            view.omissions
                .iter()
                .any(|omission| omission.reason == "size_bound")
        );
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker == "key:errors")
        );
        assert!(view.quality.passed);
        assert!(!view.fallback_raw);
    }

    #[test]
    fn rust_source_reducer_preserves_imports_declarations_and_selected_ranges() {
        let input = b"use std::path::Path;\nmod parser;\n\nstruct Engine {\n    value: u64,\n}\n\nimpl Engine {\n    pub fn run(&self) -> u64 {\n        self.value\n    }\n}\n\nfn helper() {}\n";
        let policy = ReductionPolicy {
            selected_line_ranges: vec![LineRange { start: 9, end: 11 }],
            ..tight_policy(512)
        };

        let view = reduce(ContextContentKind::Code, input, &policy).unwrap();

        assert!(view.content.contains("use std::path::Path;"));
        assert!(view.content.contains("mod parser;"));
        assert!(view.content.contains("struct Engine {"));
        assert!(view.content.contains("impl Engine {"));
        assert!(view.content.contains("pub fn run(&self) -> u64 {"));
        assert!(view.content.contains("L9:     pub fn run(&self) -> u64 {"));
        assert!(
            view.omissions
                .iter()
                .any(|omission| omission.reason == "code_body_omitted")
        );
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker == "declaration:struct")
        );
    }

    #[test]
    fn diff_reducer_preserves_file_headers_and_changed_lines() {
        let input = b"diff --git a/src/lib.rs b/src/lib.rs\nindex 111..222 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,4 @@\n use a;\n-pub fn old() {}\n+pub fn new() {}\n+// TODO: risky\n context\n";

        let view = reduce(ContextContentKind::Diff, input, &tight_policy(512)).unwrap();

        assert!(
            view.content
                .contains("diff --git a/src/lib.rs b/src/lib.rs")
        );
        assert!(view.content.contains("@@ -1,3 +1,4 @@"));
        assert!(view.content.contains("-pub fn old() {}"));
        assert!(view.content.contains("+pub fn new() {}"));
        assert!(view.content.contains("+// TODO: risky"));
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker == "diff_hunk")
        );
    }

    #[test]
    fn log_reducer_keeps_first_failure_and_unique_errors() {
        let input = b"running 9 tests\nERROR src/a.rs:9 boom\nERROR src/a.rs:9 boom\nthread 'x' panicked at src/b.rs:2: bad\nerror[E0308]: mismatched types\nfinal tail";

        let view = reduce(ContextContentKind::Log, input, &ReductionPolicy::default()).unwrap();

        assert!(view.content.contains("src/a.rs:9 boom"));
        assert_eq!(view.content.matches("src/a.rs:9 boom").count(), 1);
        assert!(view.content.contains("src/b.rs:2: bad"));
        assert!(view.content.contains("error[E0308]: mismatched types"));
        assert!(view.content.contains("final tail"));
        assert!(!view.omissions.is_empty());
        assert_eq!(view.reducer_version, "native-v1");
    }

    #[test]
    fn text_and_transcript_preserve_constraints_decisions_questions_and_recent_turns() {
        let input = b"User: constraint: do not edit other files\nAssistant: older note\nUser: decision: use native reducer\nAssistant: unresolved question: marker policy?\nUser: turn one\nAssistant: turn two\nUser: turn three\nAssistant: turn four\nUser: turn five\n";

        let view = reduce(
            ContextContentKind::Transcript,
            input,
            &ReductionPolicy::default(),
        )
        .unwrap();

        assert!(view.content.contains("constraint: do not edit other files"));
        assert!(view.content.contains("decision: use native reducer"));
        assert!(view.content.contains("unresolved question: marker policy?"));
        assert!(view.content.contains("turn five"));
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker == "constraint")
        );
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker == "decision")
        );
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker == "question")
        );
    }

    #[test]
    fn parse_failure_returns_bounded_raw_fallback() {
        let input = b"{not valid json but still useful marker}";

        let view = reduce(ContextContentKind::Json, input, &tight_policy(16)).unwrap();

        assert_eq!(view.content, "{not valid json ");
        assert!(view.content.len() <= 16);
        assert!(view.fallback_raw);
        assert!(
            view.omissions
                .iter()
                .any(|omission| omission.reason == "parse_failure")
        );
        assert!(view.quality.passed);
    }

    #[test]
    fn missing_required_marker_returns_quality_failed() {
        let policy = ReductionPolicy {
            required_markers: vec!["must-keep".to_string()],
            ..tight_policy(128)
        };

        assert!(matches!(
            reduce(ContextContentKind::Text, b"ordinary text", &policy),
            Err(crate::ContextError::QualityFailed { .. })
        ));
    }

    #[test]
    fn raw_fallback_respects_utf8_boundaries() {
        let input = b"{not valid json \xF0\x9F\x98\x80 tail}";

        let view = reduce(ContextContentKind::Json, input, &tight_policy(18)).unwrap();

        assert!(view.fallback_raw);
        assert!(view.content.is_char_boundary(view.content.len()));
        assert!(view.content.len() <= 18);
    }

    #[test]
    fn repeated_runs_are_byte_identical() {
        let input = b"ERROR src/a.rs:9 boom\nERROR src/a.rs:9 boom\nfinal tail";

        let first = reduce(ContextContentKind::Log, input, &ReductionPolicy::default()).unwrap();
        let second = reduce(ContextContentKind::Log, input, &ReductionPolicy::default()).unwrap();

        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
    }

    #[test]
    fn every_reducer_respects_size_bounds() {
        for kind in [
            ContextContentKind::Json,
            ContextContentKind::Code,
            ContextContentKind::Diff,
            ContextContentKind::Log,
            ContextContentKind::Diagnostic,
            ContextContentKind::Transcript,
            ContextContentKind::Text,
        ] {
            let input = match kind {
                ContextContentKind::Json => br#"{"message":"abcdefghijklmnopqrstuvwxyz","items":[1,2,3]}"#.as_slice(),
                ContextContentKind::Code => b"use a::b;\nfn main() {\n    println!(\"abcdefghijklmnopqrstuvwxyz\");\n}",
                ContextContentKind::Diff => b"diff --git a/a b/a\n@@ -1 +1 @@\n-abcdefghijklmnopqrstuvwxyz\n+ABCDEFGHIJKLMNOPQRSTUVWXYZ",
                ContextContentKind::Log | ContextContentKind::Diagnostic => b"ERROR abcdefghijklmnopqrstuvwxyz\nfinal tail abcdefghijklmnopqrstuvwxyz",
                ContextContentKind::Transcript | ContextContentKind::Text => b"constraint: abcdefghijklmnopqrstuvwxyz\ndecision: ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            };

            let view = reduce(kind, input, &tight_policy(32)).unwrap();

            assert!(view.content.len() <= 32, "{kind:?} exceeded bound");
            assert!(
                view.reduced.byte_count <= 32,
                "{kind:?} estimate exceeded bound"
            );
        }
    }

    #[test]
    fn token_bound_limits_output_even_when_byte_bound_is_larger() {
        let policy = ReductionPolicy {
            max_output_bytes: 1_024,
            max_output_tokens: 4,
            ..ReductionPolicy::default()
        };

        let view = reduce(
            ContextContentKind::Text,
            b"constraint: abcdefghijklmnopqrstuvwxyz",
            &policy,
        )
        .unwrap();

        assert!(view.reduced.token_count <= 4);
        assert!(view.content.len() <= 16);
        assert!(
            view.omissions
                .iter()
                .any(|omission| omission.reason == "token_bound")
        );
    }

    #[test]
    fn json_output_remains_valid_when_bounded() {
        let input = br#"{"z":[1,2,3],"a":{"b":"long value that must be pruned","c":true}}"#;
        let policy = tight_policy(48);

        let view = reduce(ContextContentKind::Json, input, &policy).unwrap();
        let again = reduce(ContextContentKind::Json, input, &policy).unwrap();

        assert_eq!(
            serde_json::to_string(&view).unwrap(),
            serde_json::to_string(&again).unwrap()
        );
        assert!(view.content.len() <= 48);
        serde_json::from_str::<serde_json::Value>(&view.content).unwrap();
        assert!(
            view.omissions
                .iter()
                .any(|omission| omission.reason == "json_values_pruned")
        );
    }

    #[test]
    fn rust_source_reducer_preserves_multiline_imports_and_signatures() {
        let input = b"use crate::{\n    alpha::Alpha,\n    beta::Beta,\n};\n\npub fn build_engine(\n    alpha: Alpha,\n    beta: Beta,\n) -> Result<Engine, Error> {\n    Engine::new(alpha, beta)\n}\n";

        let view = reduce(ContextContentKind::Code, input, &tight_policy(512)).unwrap();
        let again = reduce(ContextContentKind::Code, input, &tight_policy(512)).unwrap();

        assert_eq!(
            serde_json::to_string(&view).unwrap(),
            serde_json::to_string(&again).unwrap()
        );
        assert!(
            view.content
                .contains("use crate::{\n    alpha::Alpha,\n    beta::Beta,\n};")
        );
        assert!(view.content.contains(
            "pub fn build_engine(\n    alpha: Alpha,\n    beta: Beta,\n) -> Result<Engine, Error> {"
        ));
        assert!(!view.content.contains("Engine::new(alpha, beta)"));
    }

    #[test]
    fn reducers_have_exact_golden_outputs_and_serialized_determinism() {
        let cases = [
            (
                ContextContentKind::Json,
                br#"{"b":2,"a":1}"#.as_slice(),
                ReductionPolicy::default(),
                r#"{"content":"{\n  \"a\": 1,\n  \"b\": 2\n}","original":{"byte_count":13,"token_count":4},"reduced":{"byte_count":22,"token_count":6},"omissions":[],"retained_markers":["key:a","key:b"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_24608f0e4e5e478c","target_id":"viden-context-native:native-v1:Json","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Code,
                b"use crate::{\n    a::A,\n    b::B,\n};\n\npub fn run(\n    value: A,\n) -> B {\n    convert(value)\n}\n",
                ReductionPolicy::default(),
                r#"{"content":"use crate::{\n    a::A,\n    b::B,\n};\npub fn run(\n    value: A,\n) -> B {","original":{"byte_count":93,"token_count":24},"reduced":{"byte_count":70,"token_count":18},"omissions":[{"reason":"code_body_omitted","omitted_count":2}],"retained_markers":["declaration:fn","import_or_module"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_339dfa093b3ffce7","target_id":"viden-context-native:native-v1:Code","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Diff,
                b"diff --git a/a.rs b/a.rs\nindex 1..2 100644\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\n unchanged\n",
                ReductionPolicy::default(),
                r#"{"content":"diff --git a/a.rs b/a.rs\nindex 1..2 100644\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new","original":{"byte_count":98,"token_count":25},"reduced":{"byte_count":86,"token_count":22},"omissions":[{"reason":"diff_context_omitted","omitted_count":1}],"retained_markers":["changed_line","diff_file","diff_hunk"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_d36fa53297fb7676","target_id":"viden-context-native:native-v1:Diff","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Log,
                b"running tests\nERROR src/a.rs:1 boom\nERROR src/a.rs:1 boom\nwarning\nfinal tail\n",
                ReductionPolicy::default(),
                r#"{"content":"ERROR src/a.rs:1 boom\nwarning\nfinal tail","original":{"byte_count":77,"token_count":20},"reduced":{"byte_count":40,"token_count":10},"omissions":[{"reason":"log_lines_omitted_or_deduplicated","omitted_count":3}],"retained_markers":["first_failure","tail"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_a242b2a84535c876","target_id":"viden-context-native:native-v1:Log","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Diagnostic,
                b"running tests\nERROR src/a.rs:1 boom\nERROR src/a.rs:1 boom\nwarning\nfinal tail\n",
                ReductionPolicy::default(),
                r#"{"content":"ERROR src/a.rs:1 boom\nwarning\nfinal tail","original":{"byte_count":77,"token_count":20},"reduced":{"byte_count":40,"token_count":10},"omissions":[{"reason":"log_lines_omitted_or_deduplicated","omitted_count":3}],"retained_markers":["first_failure","tail"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_78dfd578c8672422","target_id":"viden-context-native:native-v1:Diagnostic","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Transcript,
                b"User: constraint: keep scope\nAssistant: old\nUser: decision: native\nUser: unresolved question: retry?\nAssistant: recent\n",
                ReductionPolicy::default(),
                r#"{"content":"User: constraint: keep scope\nUser: decision: native\nUser: unresolved question: retry?\nAssistant: old\nAssistant: recent","original":{"byte_count":119,"token_count":30},"reduced":{"byte_count":118,"token_count":30},"omissions":[],"retained_markers":["constraint","decision","question","recent_turn"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_1931e50bf52c15ee","target_id":"viden-context-native:native-v1:Transcript","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Text,
                b"constraint: keep scope\ndecision: native\nunresolved question: retry?\nplain old\nplain recent\n",
                ReductionPolicy::default(),
                r#"{"content":"constraint: keep scope\ndecision: native\nunresolved question: retry?\nplain old\nplain recent","original":{"byte_count":91,"token_count":23},"reduced":{"byte_count":90,"token_count":23},"omissions":[],"retained_markers":["constraint","decision","question","recent_turn"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_6664ad4544b62071","target_id":"viden-context-native:native-v1:Text","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
        ];

        for (kind, input, policy, expected_serialized) in cases {
            let view = reduce(kind, input, &policy).unwrap();
            let again = reduce(kind, input, &policy).unwrap();
            let serialized = serde_json::to_string(&view).unwrap();

            assert_eq!(serialized, expected_serialized, "{kind:?}");
            assert_eq!(
                serialized,
                serde_json::to_string(&again).unwrap(),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn tiny_json_bounds_return_parseable_minimal_values_or_typed_errors() {
        let input = br#"{"long":"value","items":[1,2,3]}"#;
        for byte_limit in [1, 2, 3] {
            let policy = ReductionPolicy {
                max_output_bytes: byte_limit,
                max_output_tokens: 1,
                ..ReductionPolicy::default()
            };

            let view = reduce(ContextContentKind::Json, input, &policy).unwrap();

            assert_eq!(view.content, "0");
            assert!(view.content.len() <= byte_limit);
            assert!(view.reduced.token_count <= 1);
            serde_json::from_str::<serde_json::Value>(&view.content).unwrap();
            assert!(
                view.omissions
                    .iter()
                    .any(|omission| omission.reason == "minimal_json_fallback")
            );
        }
    }

    #[test]
    fn zero_bounds_return_typed_invalid_policy_for_every_reducer() {
        for kind in [
            ContextContentKind::Json,
            ContextContentKind::Code,
            ContextContentKind::Diff,
            ContextContentKind::Log,
            ContextContentKind::Diagnostic,
            ContextContentKind::Transcript,
            ContextContentKind::Text,
        ] {
            for policy in [
                ReductionPolicy {
                    max_output_bytes: 0,
                    max_output_tokens: 1,
                    ..ReductionPolicy::default()
                },
                ReductionPolicy {
                    max_output_bytes: 8,
                    max_output_tokens: 0,
                    ..ReductionPolicy::default()
                },
            ] {
                let err = reduce(kind, b"{\"a\":1}\nERROR boom\nconstraint: x", &policy)
                    .expect_err("zero bounds must be invalid policy");

                assert!(matches!(
                    err,
                    crate::ContextError::InvalidReductionPolicy { .. }
                ));
                assert!(!err.to_string().contains("{\"a\":1}"));
                assert!(!err.to_string().contains("ERROR boom"));
            }
        }
    }

    #[test]
    fn marker_validation_uses_retained_markers_not_literal_content() {
        let pass_policy = ReductionPolicy {
            required_markers: vec!["key:errors".to_string()],
            ..tight_policy(256)
        };
        let pass = reduce(
            ContextContentKind::Json,
            br#"{"errors":[{"message":"boom"}],"ok":true}"#,
            &pass_policy,
        )
        .unwrap();
        assert!(
            pass.retained_markers
                .iter()
                .any(|marker| marker == "key:errors")
        );

        let fail_policy = ReductionPolicy {
            required_markers: vec!["key:errors".to_string()],
            ..tight_policy(12)
        };
        assert!(matches!(
            reduce(
                ContextContentKind::Json,
                br#"{"errors":[{"message":"boom"}],"ok":true}"#,
                &fail_policy
            ),
            Err(crate::ContextError::QualityFailed { .. })
        ));

        let empty_policy = ReductionPolicy {
            required_markers: vec!["".to_string()],
            ..ReductionPolicy::default()
        };
        assert!(matches!(
            reduce(ContextContentKind::Text, b"anything", &empty_policy),
            Err(crate::ContextError::InvalidReductionPolicy { .. })
        ));
    }

    #[test]
    fn rust_scanner_does_not_capture_bodies_secrets_or_next_declarations() {
        let input = b"#[inline]\npub(crate) async unsafe extern \"C\" fn compute(\n    token: &str,\n) -> Result<(), Error> { // comment\n    let API_TOKEN = \"raw-secret\";\n}\n\npub(super) fn next() {}\n";

        let view = reduce(ContextContentKind::Code, input, &ReductionPolicy::default()).unwrap();

        assert!(
            view.content
                .contains("#[inline]\npub(crate) async unsafe extern \"C\" fn compute(")
        );
        assert!(view.content.contains(") -> Result<(), Error> { // comment"));
        assert!(view.content.contains("pub(super) fn next() {"));
        assert!(!view.content.contains("raw-secret"));
        assert!(!view.content.contains("let API_TOKEN"));
        assert_eq!(view.content.matches("pub(super) fn next").count(), 1);
    }

    #[test]
    fn secret_redaction_applies_to_every_reducer_route() {
        let cases = [
            (
                ContextContentKind::Json,
                br#"{"authorization":"Bearer abc.def.ghi","nested":{"api_key":"sk-live-secret"}}"#.as_slice(),
            ),
            (
                ContextContentKind::Code,
                b"const API_TOKEN: &str = \"secret-token\";\nfn call() {\n    let password = \"hunter2\";\n}\n",
            ),
            (
                ContextContentKind::Diff,
                b"diff --git a/.env b/.env\n@@ -1 +1 @@\n-API_KEY=old-secret\n+API_KEY=new-secret\n",
            ),
            (
                ContextContentKind::Log,
                b"ERROR Authorization: Bearer raw-secret-token\nTOKEN=secret-token\n",
            ),
            (
                ContextContentKind::Diagnostic,
                b"ERROR password=hunter2\nSECRET=raw-secret\n",
            ),
            (
                ContextContentKind::Transcript,
                b"User: constraint: Authorization: Bearer raw-secret-token\nAssistant: recent\n",
            ),
            (
                ContextContentKind::Text,
                b"decision: password = hunter2\n.env API_KEY=raw-secret\n",
            ),
        ];

        for (kind, input) in cases {
            let view = reduce(kind, input, &ReductionPolicy::default()).unwrap();
            let serialized = serde_json::to_string(&view).unwrap();

            assert!(view.content.contains("[REDACTED]"), "{kind:?}");
            assert!(!serialized.contains("raw-secret"), "{kind:?}");
            assert!(!serialized.contains("hunter2"), "{kind:?}");
            assert!(!serialized.contains("secret-token"), "{kind:?}");
            assert!(!serialized.contains("sk-live-secret"), "{kind:?}");
        }
    }

    #[test]
    fn json_resource_limits_are_deterministic_and_non_leaking() {
        let input_policy = ReductionPolicy {
            max_input_bytes: 8,
            ..ReductionPolicy::default()
        };
        let err = reduce(
            ContextContentKind::Json,
            br#"{"password":"secret-value","items":[1,2,3]}"#,
            &input_policy,
        )
        .expect_err("oversized input should be rejected");
        assert!(matches!(
            err,
            crate::ContextError::ReductionInputTooLarge { .. }
        ));
        assert!(!err.to_string().contains("secret-value"));

        let deep_policy = ReductionPolicy {
            max_json_depth: 2,
            ..ReductionPolicy::default()
        };
        let deep = reduce(
            ContextContentKind::Json,
            br#"{"a":{"b":{"c":1}}}"#,
            &deep_policy,
        )
        .unwrap();
        let again = reduce(
            ContextContentKind::Json,
            br#"{"a":{"b":{"c":1}}}"#,
            &deep_policy,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&deep).unwrap(),
            serde_json::to_string(&again).unwrap()
        );
        assert_eq!(deep.content, "0");
        assert!(
            deep.omissions
                .iter()
                .any(|omission| omission.reason == "json_depth_limit")
        );

        let value_policy = ReductionPolicy {
            max_json_values: 3,
            ..ReductionPolicy::default()
        };
        let value_heavy = reduce(
            ContextContentKind::Json,
            br#"{"a":1,"b":2,"c":3,"d":4}"#,
            &value_policy,
        )
        .unwrap();
        assert_eq!(value_heavy.content, "0");
        assert!(
            value_heavy
                .omissions
                .iter()
                .any(|omission| omission.reason == "json_value_limit")
        );
    }

    #[test]
    fn log_omission_accounting_counts_each_omitted_line_once() {
        let view = reduce(
            ContextContentKind::Log,
            b"ignored one\nERROR src/a.rs:1 boom\nERROR src/a.rs:1 boom\nignored two\nfinal tail\n",
            &ReductionPolicy::default(),
        )
        .unwrap();

        assert_eq!(
            view.omissions
                .iter()
                .find(|omission| omission.reason == "log_lines_omitted_or_deduplicated")
                .map(|omission| omission.omitted_count),
            Some(3)
        );
    }
}
