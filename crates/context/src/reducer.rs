use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use viden_types::{ContextContentKind, ContextQualityRecord};

const REDUCER_ID: &str = "viden-context-native";
const REDUCER_VERSION: &str = "native-v1";
const MAX_REFERENCED_SYMBOLS: usize = 64;
const MAX_REFERENCED_SYMBOL_BYTES: usize = 128;
const MAX_REFERENCE_SITES_PER_SYMBOL: usize = 4;
const SYMBOL_CONTEXT_RADIUS: usize = 1;

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
    /// Explicit Rust identifiers whose declarations and bounded reference
    /// slices should be retained. Reducer v1 performs line-oriented matching,
    /// not AST resolution.
    #[serde(default)]
    pub referenced_symbols: Vec<String>,
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
            referenced_symbols: Vec::new(),
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
    if kind != ContextContentKind::Json || view.fallback_raw {
        let redacted = redact_text(&view.content);
        if redacted != view.content {
            view.content = redacted;
            view.omissions.push(omission("secret_values_redacted", 1));
        }
    }
    if kind != ContextContentKind::Json || view.fallback_raw {
        bound_output(&mut view.content, policy, &mut view.omissions);
    }
    view.retained_markers = final_retained_markers(kind, &view);
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
    let mut retained_markers = Vec::new();
    let mut seen = HashSet::new();
    let mut retained_source_indices = HashSet::new();
    let mut index = 0;

    while index < all_lines.len() {
        let line = all_lines[index];
        let trimmed = line.trim_start();
        let attribute_start = trimmed.starts_with("#[");
        let declaration = if attribute_start {
            next_declaration_marker(&all_lines, index)
        } else {
            code_declaration_marker(trimmed)
        };

        if trimmed.starts_with("use ")
            || trimmed.starts_with("pub use ")
            || trimmed.starts_with("mod ")
        {
            let (block, end_index) = scan_code_statement(&all_lines, index, CodeScanKind::Import);
            push_unique(&mut lines, &mut seen, block);
            retained_source_indices.extend(index..=end_index);
            retained_markers.push("import_or_module".to_string());
            index = end_index;
        } else if let Some(marker) = declaration.as_ref() {
            let (block, end_index) =
                scan_code_statement(&all_lines, index, CodeScanKind::Declaration);
            push_unique(&mut lines, &mut seen, block);
            retained_source_indices.extend(index..=end_index);
            retained_markers.push(marker.clone());
            index = end_index;
        }
        index += 1;
    }

    for (index, line) in all_lines.iter().enumerate() {
        let line_number = index + 1;
        if policy
            .selected_line_ranges
            .iter()
            .any(|range| line_number >= range.start && line_number <= range.end)
        {
            push_unique(&mut lines, &mut seen, format!("L{line_number}: {line}"));
            retained_source_indices.insert(index);
            retained_markers.push("selected_range".to_string());
        }
    }

    let mut missing_symbols = 0;
    let mut omitted_reference_sites = 0;
    for symbol in &policy.referenced_symbols {
        let declaration_ranges = symbol_declaration_ranges(&all_lines, symbol);
        let mut symbol_found = !declaration_ranges.is_empty();
        let mut reference_sites = 0;

        for (index, line) in all_lines.iter().enumerate() {
            if declaration_ranges
                .iter()
                .any(|(start, end)| index >= *start && index <= *end)
                || !contains_identifier(line, symbol)
            {
                continue;
            }
            symbol_found = true;
            if reference_sites >= MAX_REFERENCE_SITES_PER_SYMBOL {
                omitted_reference_sites += 1;
                continue;
            }
            reference_sites += 1;
            let slice_start = index.saturating_sub(SYMBOL_CONTEXT_RADIUS);
            let slice_end = (index + SYMBOL_CONTEXT_RADIUS).min(all_lines.len() - 1);
            for (slice_index, slice_line) in all_lines
                .iter()
                .enumerate()
                .take(slice_end + 1)
                .skip(slice_start)
            {
                if slice_line.trim().is_empty() {
                    continue;
                }
                push_unique(
                    &mut lines,
                    &mut seen,
                    format!("L{}: {slice_line}", slice_index + 1),
                );
                retained_source_indices.insert(slice_index);
            }
        }

        if symbol_found {
            retained_markers.push(format!("symbol:{symbol}"));
        } else {
            missing_symbols += 1;
        }
    }

    let mut view = result(lines.join("\n"), retained_markers, false);
    let omitted = all_lines
        .iter()
        .enumerate()
        .filter(|(index, line)| !line.trim().is_empty() && !retained_source_indices.contains(index))
        .count();
    if omitted > 0 {
        view.omissions.push(omission("code_body_omitted", omitted));
    }
    if missing_symbols > 0 {
        view.omissions
            .push(omission("referenced_symbol_not_found", missing_symbols));
    }
    if omitted_reference_sites > 0 {
        view.omissions.push(omission(
            "referenced_symbol_sites_omitted",
            omitted_reference_sites,
        ));
    }
    view
}

fn symbol_declaration_ranges(lines: &[&str], symbol: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        let declaration = if trimmed.starts_with("#[") {
            next_declaration_marker(lines, index)
        } else {
            code_declaration_marker(trimmed)
        };
        if declaration.is_some() {
            let (block, end_index) = scan_code_statement(lines, index, CodeScanKind::Declaration);
            if declaration_header_declares_symbol(&block, symbol) {
                ranges.push((index, end_index));
            }
            index = end_index;
        }
        index += 1;
    }
    ranges
}

fn declaration_header_declares_symbol(header: &str, symbol: &str) -> bool {
    let identifiers = header
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|identifier| !identifier.is_empty())
        .collect::<Vec<_>>();

    for (index, identifier) in identifiers.iter().enumerate() {
        if matches!(
            *identifier,
            "fn" | "struct" | "enum" | "trait" | "type" | "const" | "static" | "mod"
        ) && identifiers
            .get(index + 1)
            .is_some_and(|name| *name == symbol)
        {
            return true;
        }
        if *identifier == "impl"
            && identifiers[index + 1..]
                .iter()
                .take_while(|name| **name != "where")
                .any(|name| *name == symbol)
        {
            return true;
        }
    }
    false
}

fn contains_identifier(line: &str, identifier: &str) -> bool {
    line.match_indices(identifier).any(|(start, matched)| {
        let before = line[..start].chars().next_back();
        let end = start + matched.len();
        let after = line[end..].chars().next();
        !before.is_some_and(is_identifier_character) && !after.is_some_and(is_identifier_character)
    })
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
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
        } else if let Some(marker) = diff_metadata_marker(line) {
            lines.push(line.to_string());
            retained_markers.push(marker.to_string());
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

fn diff_metadata_marker(line: &str) -> Option<&'static str> {
    [
        ("new file mode ", "diff:new_file_mode"),
        ("deleted file mode ", "diff:deleted_file_mode"),
        ("old mode ", "diff:old_mode"),
        ("new mode ", "diff:new_mode"),
        ("rename from ", "diff:rename_from"),
        ("rename to ", "diff:rename_to"),
        ("copy from ", "diff:copy_from"),
        ("copy to ", "diff:copy_to"),
        ("similarity index ", "diff:similarity_index"),
        ("dissimilarity index ", "diff:dissimilarity_index"),
        ("Binary files ", "diff:binary_file"),
        ("GIT binary patch", "diff:binary_file"),
    ]
    .into_iter()
    .find_map(|(prefix, marker)| line.starts_with(prefix).then_some(marker))
}

fn is_changed_diff_line(line: &str) -> bool {
    (line.starts_with('+') && !line.starts_with("+++ "))
        || (line.starts_with('-') && !line.starts_with("--- "))
}

fn reduce_log(input: &[u8], _policy: &ReductionPolicy) -> ReductionResult {
    let text = String::from_utf8_lossy(input);
    let all_lines = text.lines().collect::<Vec<_>>();
    let mut kept_indices = HashSet::new();
    let mut seen_errors = HashSet::new();
    let mut first_failure_kept = false;
    let mut retained_markers = Vec::new();

    for (index, line) in all_lines.iter().enumerate() {
        let command = is_log_command_line(line);
        let failure = !command && is_log_failure_line(line);
        let mut keep_failure = false;
        if command {
            kept_indices.insert(index);
            retained_markers.push("command".to_string());
        }
        if is_log_exit_status(line) {
            kept_indices.insert(index);
            retained_markers.push("exit_status".to_string());
        }
        if failure {
            if !first_failure_kept {
                kept_indices.insert(index);
                retained_markers.push("first_failure".to_string());
                seen_errors.insert(redact_line(line));
                first_failure_kept = true;
                keep_failure = true;
            } else if seen_errors.insert(redact_line(line)) {
                kept_indices.insert(index);
                retained_markers.push("unique_error".to_string());
                keep_failure = true;
            }
        }
        if has_failing_location(line) && (!failure || keep_failure) {
            kept_indices.insert(index);
            retained_markers.push("failing_location".to_string());
        }
    }

    let kept_values = kept_indices
        .iter()
        .map(|index| all_lines[*index])
        .collect::<HashSet<_>>();
    let tail_start = all_lines.len().saturating_sub(3);
    for (index, line) in all_lines.iter().enumerate().skip(tail_start) {
        if !kept_values.contains(line) {
            kept_indices.insert(index);
            retained_markers.push("tail".to_string());
        }
    }

    let lines = all_lines
        .iter()
        .enumerate()
        .filter(|(index, _)| kept_indices.contains(index))
        .map(|(_, line)| (*line).to_string())
        .collect::<Vec<_>>();
    let mut view = result(lines.join("\n"), retained_markers, false);
    let omitted = all_lines.len().saturating_sub(kept_indices.len());
    if omitted > 0 {
        view.omissions
            .push(omission("log_lines_omitted_or_deduplicated", omitted));
    }
    view
}

fn is_log_command_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.starts_with("$ ")
        || trimmed.starts_with("> ")
        || trimmed.starts_with("+ ")
        || lower.starts_with("##[command]")
        || [
            "command:",
            "command=",
            "cmd:",
            "cmd=",
            "executing command:",
            "run command:",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        return true;
    }

    [
        "cargo ", "git ", "make ", "cmake ", "npm ", "pnpm ", "yarn ", "pytest ", "python ",
        "node ", "go test", "dotnet ", "mvn ", "gradle ", "./",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn is_log_exit_status(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    [
        "exit status",
        "exit code",
        "exited with status",
        "exited with code",
    ]
    .iter()
    .any(|phrase| {
        lower.find(phrase).is_some_and(|index| {
            lower[index + phrase.len()..]
                .chars()
                .any(|character| character.is_ascii_digit())
        })
    })
}

fn is_log_failure_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("error")
        || lower.contains("failed")
        || lower.contains("failure")
        || lower.contains("panicked")
        || lower.contains("panic")
}

fn has_failing_location(line: &str) -> bool {
    line.split_whitespace().any(|word| {
        let candidate = word.trim_matches(|character: char| {
            matches!(character, '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';')
        });
        let segments = candidate.split(':').collect::<Vec<_>>();
        segments.iter().enumerate().skip(1).any(|(index, segment)| {
            let number = segment.trim_matches(|character: char| !character.is_ascii_digit());
            if number.is_empty() || !number.chars().all(|character| character.is_ascii_digit()) {
                return false;
            }
            let path = segments[..index].join(":");
            path.contains('/')
                || path.contains('\\')
                || path
                    .rsplit_once('.')
                    .is_some_and(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        })
    })
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
    if policy.referenced_symbols.len() > MAX_REFERENCED_SYMBOLS {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "referenced_symbols",
            reason: "contains too many entries",
        });
    }
    if policy.referenced_symbols.iter().any(|symbol| {
        symbol.is_empty()
            || symbol.len() > MAX_REFERENCED_SYMBOL_BYTES
            || !is_valid_identifier(symbol)
            || is_secret_like_symbol(symbol)
    }) {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "referenced_symbols",
            reason: "entries must be bounded non-secret identifiers",
        });
    }
    Ok(())
}

fn is_valid_identifier(identifier: &str) -> bool {
    let mut characters = identifier.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && characters.all(is_identifier_character)
}

fn is_secret_like_symbol(symbol: &str) -> bool {
    let lower = symbol.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "authorization"
            | "password"
            | "secret"
            | "token"
            | "api_key"
            | "apikey"
            | "api_token"
            | "access_token"
            | "auth_token"
            | "bearer_token"
            | "refresh_token"
    ) || lower.starts_with("password_")
        || lower.starts_with("secret_")
        || lower.ends_with("_password")
        || lower.ends_with("_secret")
        || lower.ends_with("_api_key")
        || lower.ends_with("_apikey")
        || lower.contains("credential")
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

fn final_retained_markers(kind: ContextContentKind, view: &ReductionResult) -> Vec<String> {
    if view.fallback_raw {
        return Vec::new();
    }
    match kind {
        ContextContentKind::Json => collect_json_markers_from_content(&view.content),
        _ => view
            .retained_markers
            .iter()
            .filter(|marker| final_marker_supported(marker, &view.content))
            .cloned()
            .collect(),
    }
}

fn final_marker_supported(marker: &str, content: &str) -> bool {
    match marker {
        "first_failure" | "unique_error" => contains_failure_line(content),
        "command" => content.lines().any(is_log_command_line),
        "exit_status" => content.lines().any(is_log_exit_status),
        "failing_location" => content.lines().any(has_failing_location),
        "tail" => !content.is_empty(),
        "constraint" => {
            content.to_ascii_lowercase().contains("constraint")
                || content.to_ascii_lowercase().contains("must ")
                || content.to_ascii_lowercase().contains("do not ")
        }
        "decision" => {
            content.to_ascii_lowercase().contains("decision")
                || content.to_ascii_lowercase().contains("decided")
        }
        "question" => {
            content.to_ascii_lowercase().contains("unresolved")
                || content.to_ascii_lowercase().contains("question")
                || content.contains('?')
        }
        "recent_turn" => !content.is_empty(),
        "import_or_module" => content.lines().any(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("use ")
                || trimmed.starts_with("pub use ")
                || trimmed.starts_with("mod ")
        }),
        "selected_range" => content.lines().any(|line| line.starts_with('L')),
        "diff_file" => content.contains("diff --git "),
        "diff_hunk" => content.contains("@@ "),
        "changed_line" => content.lines().any(is_changed_diff_line),
        "risky_change" => content.contains("unsafe") || content.contains("TODO"),
        marker if marker.starts_with("diff:") => content
            .lines()
            .filter_map(diff_metadata_marker)
            .any(|retained| retained == marker),
        marker if marker.starts_with("symbol:") => marker
            .strip_prefix("symbol:")
            .is_some_and(|symbol| final_symbol_supported(content, symbol)),
        marker if marker.starts_with("declaration:") => content
            .lines()
            .filter_map(|line| code_declaration_marker(line.trim_start()))
            .any(|declaration| declaration == marker),
        _ => content.contains(marker),
    }
}

fn final_symbol_supported(content: &str, symbol: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim_start();
        let labeled_reference = trimmed
            .strip_prefix('L')
            .and_then(|rest| rest.split_once(": "))
            .is_some_and(|(number, source)| {
                !number.is_empty()
                    && number.chars().all(|character| character.is_ascii_digit())
                    && contains_identifier(source, symbol)
            });
        labeled_reference || declaration_header_declares_symbol(trimmed, symbol)
    })
}

fn contains_failure_line(content: &str) -> bool {
    content
        .lines()
        .any(|line| !is_log_command_line(line) && is_log_failure_line(line))
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
            referenced_symbols: Vec::new(),
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
    fn diff_reducer_preserves_exact_file_operation_metadata() {
        let input = b"diff --git a/old.rs b/new.rs\nsimilarity index 91%\nrename from old.rs\nrename to new.rs\nold mode 100644\nnew mode 100755\nBinary files a/old.rs and b/new.rs differ\ncontext omitted\ndiff --git a/add.rs b/add.rs\nnew file mode 100644\nindex 000..111 100644\n--- /dev/null\n+++ b/add.rs\n@@ -0,0 +1 @@\n+new\ndeleted file mode 100644\ndissimilarity index 12%\ncopy from source.rs\ncopy to copied.rs\nGIT binary patch\nliteral 0\n";

        let view = reduce(ContextContentKind::Diff, input, &ReductionPolicy::default()).unwrap();
        let again = reduce(ContextContentKind::Diff, input, &ReductionPolicy::default()).unwrap();

        assert_eq!(
            view.content,
            "diff --git a/old.rs b/new.rs\n\
             similarity index 91%\n\
             rename from old.rs\n\
             rename to new.rs\n\
             old mode 100644\n\
             new mode 100755\n\
             Binary files a/old.rs and b/new.rs differ\n\
             diff --git a/add.rs b/add.rs\n\
             new file mode 100644\n\
             index 000..111 100644\n\
             --- /dev/null\n\
             +++ b/add.rs\n\
             @@ -0,0 +1 @@\n\
             +new\n\
             deleted file mode 100644\n\
             dissimilarity index 12%\n\
             copy from source.rs\n\
             copy to copied.rs\n\
             GIT binary patch"
        );
        assert_eq!(
            view.retained_markers,
            vec![
                "changed_line",
                "diff:binary_file",
                "diff:copy_from",
                "diff:copy_to",
                "diff:deleted_file_mode",
                "diff:dissimilarity_index",
                "diff:new_file_mode",
                "diff:new_mode",
                "diff:old_mode",
                "diff:rename_from",
                "diff:rename_to",
                "diff:similarity_index",
                "diff_file",
                "diff_hunk",
            ]
        );
        assert_eq!(view.omissions, vec![omission("diff_context_omitted", 2)]);
        assert_eq!(
            serde_json::to_string(&view).unwrap(),
            serde_json::to_string(&again).unwrap()
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
    fn referenced_symbol_preserves_exact_declaration_and_labeled_slice() {
        let input = b"struct Worker;\nfn target(value: i32) -> i32 {\n    value + 1\n}\nfn caller() {\n    let password = \"body-secret\";\n    let result = target(1);\n    let target_suffix = result;\n}\n";
        let policy = ReductionPolicy {
            referenced_symbols: vec!["target".to_string()],
            ..ReductionPolicy::default()
        };

        let view = reduce(ContextContentKind::Code, input, &policy).unwrap();
        let again = reduce(ContextContentKind::Code, input, &policy).unwrap();

        assert_eq!(
            view.content,
            "struct Worker;\n\
             fn target(value: i32) -> i32 {\n\
             fn caller() {\n\
             L6:     let password = \"[REDACTED]\"\n\
             L7:     let result = target(1);\n\
             L8:     let target_suffix = result;"
        );
        assert_eq!(
            view.retained_markers,
            vec!["declaration:fn", "declaration:struct", "symbol:target"]
        );
        assert_eq!(
            view.omissions,
            vec![
                omission("code_body_omitted", 3),
                omission("secret_values_redacted", 1),
            ]
        );
        let serialized = serde_json::to_string(&view).unwrap();
        assert_eq!(serialized, serde_json::to_string(&again).unwrap());
        assert!(!serialized.contains("body-secret"));
    }

    #[test]
    fn referenced_symbol_absence_and_substrings_do_not_create_evidence() {
        let policy = ReductionPolicy {
            referenced_symbols: vec!["target".to_string(), "missing".to_string()],
            ..ReductionPolicy::default()
        };

        let view = reduce(
            ContextContentKind::Code,
            b"fn target_suffix() {}\nfn caller() { target_suffix(); }\n",
            &policy,
        )
        .unwrap();

        assert!(!view.content.lines().any(|line| line.starts_with('L')));
        assert!(
            !view
                .retained_markers
                .iter()
                .any(|marker| marker == "symbol:target")
        );
        assert!(
            !view
                .retained_markers
                .iter()
                .any(|marker| marker == "symbol:missing")
        );
        assert_eq!(
            view.omissions
                .iter()
                .find(|omission| omission.reason == "referenced_symbol_not_found")
                .map(|omission| omission.omitted_count),
            Some(2)
        );
    }

    #[test]
    fn referenced_symbol_policy_rejects_empty_unbounded_and_secret_like_entries() {
        let invalid_symbols = [
            vec![String::new()],
            vec!["x".repeat(129)],
            vec!["name".to_string(); 65],
            vec!["API_TOKEN".to_string()],
        ];

        for referenced_symbols in invalid_symbols {
            let policy = ReductionPolicy {
                referenced_symbols,
                ..ReductionPolicy::default()
            };
            let error = reduce(ContextContentKind::Code, b"fn ok() {}", &policy)
                .expect_err("invalid referenced symbol policy must fail");

            assert!(matches!(
                error,
                crate::ContextError::InvalidReductionPolicy {
                    field: "referenced_symbols",
                    ..
                }
            ));
            assert!(!error.to_string().contains("API_TOKEN"));
        }
    }

    #[test]
    fn referenced_symbol_bounds_are_deterministic_and_required_after_bounding() {
        let input = b"struct Keep {}\nfn run() {}\nfn caller() {\n    run();\n    run();\n    run();\n    run();\n    run();\n}\n";
        let bounded_policy = ReductionPolicy {
            max_output_bytes: 72,
            referenced_symbols: vec!["run".to_string()],
            ..ReductionPolicy::default()
        };
        let first = reduce(ContextContentKind::Code, input, &bounded_policy).unwrap();
        let second = reduce(ContextContentKind::Code, input, &bounded_policy).unwrap();
        assert!(first.content.len() <= 72);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );

        let required_policy = ReductionPolicy {
            max_output_bytes: 16,
            referenced_symbols: vec!["run".to_string()],
            required_markers: vec!["symbol:run".to_string()],
            ..ReductionPolicy::default()
        };
        assert!(matches!(
            reduce(ContextContentKind::Code, input, &required_policy),
            Err(crate::ContextError::QualityFailed { .. })
        ));
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
                r#"{"content":"ERROR src/a.rs:1 boom\nwarning\nfinal tail","original":{"byte_count":77,"token_count":20},"reduced":{"byte_count":40,"token_count":10},"omissions":[{"reason":"log_lines_omitted_or_deduplicated","omitted_count":2}],"retained_markers":["failing_location","first_failure","tail"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_a242b2a84535c876","target_id":"viden-context-native:native-v1:Log","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Diagnostic,
                b"running tests\nERROR src/a.rs:1 boom\nERROR src/a.rs:1 boom\nwarning\nfinal tail\n",
                ReductionPolicy::default(),
                r#"{"content":"ERROR src/a.rs:1 boom\nwarning\nfinal tail","original":{"byte_count":77,"token_count":20},"reduced":{"byte_count":40,"token_count":10},"omissions":[{"reason":"log_lines_omitted_or_deduplicated","omitted_count":2}],"retained_markers":["failing_location","first_failure","tail"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_78dfd578c8672422","target_id":"viden-context-native:native-v1:Diagnostic","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
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
            Some(2)
        );
    }

    #[test]
    fn duplicate_log_omissions_count_source_lines_once() {
        let view = reduce(
            ContextContentKind::Log,
            b"ignored\nERROR one\nERROR one\nERROR one\nfinal tail\n",
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

    #[test]
    fn log_reducer_preserves_exact_workflow_evidence_anywhere() {
        let input = b"noise before\n$ cargo test -p demo --token=raw-secret\nsetup noise\nat src/lib.rs:42:17\nERROR first failure\nERROR first failure\nerror[E0308]: mismatched types\nignored tail\nexit status: 101\ntail final\n";

        for kind in [ContextContentKind::Log, ContextContentKind::Diagnostic] {
            let view = reduce(kind, input, &ReductionPolicy::default()).unwrap();
            let again = reduce(kind, input, &ReductionPolicy::default()).unwrap();

            assert_eq!(
                view.content,
                "$ cargo test -p demo --token= [REDACTED]\n\
                 at src/lib.rs:42:17\n\
                 ERROR first failure\n\
                 error[E0308]: mismatched types\n\
                 ignored tail\n\
                 exit status: 101\n\
                 tail final"
            );
            assert_eq!(
                view.retained_markers,
                vec![
                    "command",
                    "exit_status",
                    "failing_location",
                    "first_failure",
                    "tail",
                    "unique_error",
                ]
            );
            assert_eq!(
                view.omissions,
                vec![
                    omission("log_lines_omitted_or_deduplicated", 3),
                    omission("secret_values_redacted", 1),
                ]
            );
            assert_eq!(
                serde_json::to_string(&view).unwrap(),
                serde_json::to_string(&again).unwrap()
            );
        }
    }

    #[test]
    fn log_reducer_recognizes_common_command_and_exit_formats() {
        let input = b"##[command]cargo test --workspace\nCommand: npm test\n+ pytest -q\nintermediate\nexit code: 1\nProcess exited with status 2\nlast\n";

        let view = reduce(ContextContentKind::Log, input, &ReductionPolicy::default()).unwrap();

        assert_eq!(
            view.content,
            "##[command]cargo test --workspace\n\
             Command: npm test\n\
             + pytest -q\n\
             exit code: 1\n\
             Process exited with status 2\n\
             last"
        );
        assert_eq!(
            view.retained_markers,
            vec!["command", "exit_status", "tail"]
        );
        assert_eq!(
            view.omissions,
            vec![omission("log_lines_omitted_or_deduplicated", 1)]
        );
    }

    #[test]
    fn log_command_text_does_not_fabricate_failure_evidence() {
        let input = b"$ cargo test failed_case\nsetup\nERROR actual failure\nend\n";
        let view = reduce(ContextContentKind::Log, input, &ReductionPolicy::default()).unwrap();

        assert_eq!(
            view.retained_markers,
            vec!["command", "first_failure", "tail"]
        );

        let tight = ReductionPolicy {
            max_output_bytes: 25,
            required_markers: vec!["first_failure".to_string()],
            ..ReductionPolicy::default()
        };
        assert!(matches!(
            reduce(ContextContentKind::Log, input, &tight),
            Err(crate::ContextError::QualityFailed { .. })
        ));
    }

    #[test]
    fn invalid_json_raw_fallback_redacts_secrets_before_output() {
        let input =
            b"{ api_key = raw-secret-token, Authorization: Bearer bearer-secret, password=hunter2";

        let view = reduce(ContextContentKind::Json, input, &ReductionPolicy::default()).unwrap();
        let serialized = serde_json::to_string(&view).unwrap();

        assert!(view.fallback_raw);
        assert!(view.content.contains("[REDACTED]"));
        assert!(!serialized.contains("raw-secret-token"));
        assert!(!serialized.contains("bearer-secret"));
        assert!(!serialized.contains("hunter2"));
        assert!(
            !view
                .retained_markers
                .iter()
                .any(|marker| marker.contains("raw-secret"))
        );
        assert!(
            !view
                .omissions
                .iter()
                .any(|omission| omission.reason.contains("raw-secret"))
        );
    }

    #[test]
    fn required_markers_are_checked_against_final_bounded_output() {
        let semantic_policy = ReductionPolicy {
            max_output_bytes: 4,
            required_markers: vec!["first_failure".to_string()],
            ..ReductionPolicy::default()
        };
        assert!(matches!(
            reduce(
                ContextContentKind::Log,
                b"ERROR src/a.rs:1 boom",
                &semantic_policy
            ),
            Err(crate::ContextError::QualityFailed { .. })
        ));

        let literal_policy = ReductionPolicy {
            max_output_bytes: 10,
            required_markers: vec!["literal:must-keep".to_string()],
            ..ReductionPolicy::default()
        };
        assert!(matches!(
            reduce(
                ContextContentKind::Text,
                b"constraint: must-keep",
                &literal_policy
            ),
            Err(crate::ContextError::QualityFailed { .. })
        ));

        let retained_policy = ReductionPolicy {
            required_markers: vec!["decision".to_string(), "literal:keep".to_string()],
            ..ReductionPolicy::default()
        };
        let retained = reduce(
            ContextContentKind::Text,
            b"decision: keep",
            &retained_policy,
        )
        .unwrap();
        assert!(
            retained
                .retained_markers
                .iter()
                .any(|marker| marker == "decision")
        );
    }

    #[test]
    fn required_declaration_marker_must_match_final_declaration_kind() {
        let policy = ReductionPolicy {
            max_output_bytes: 16,
            required_markers: vec!["declaration:fn".to_string()],
            ..ReductionPolicy::default()
        };

        assert!(matches!(
            reduce(
                ContextContentKind::Code,
                b"struct Config {}\nfn run() {}\n",
                &policy
            ),
            Err(crate::ContextError::QualityFailed { .. })
        ));
    }
}
