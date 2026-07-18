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
const MAX_SELECTED_JSON_PATHS: usize = 64;
const MAX_SELECTED_JSON_PATH_BYTES: usize = 256;
const MAX_JSON_PATH_SEGMENTS: usize = 32;
const MAX_REQUIRED_MARKERS: usize = 64;
const MAX_REQUIRED_MARKER_BYTES: usize = 256;
const MAX_SELECTED_LINE_RANGES: usize = 128;
const MAX_SELECTED_LINE_NUMBER: usize = 1_000_000;
const MAX_SELECTED_LINE_RANGE_SPAN: usize = 100_000;
const MAX_JSON_PRIORITY_CANDIDATES: usize = 256;
const MAX_COMPACT_JSON_VALUES: usize = 16;
const MAX_DIFF_HUNK_LINES_LIMIT: usize = 4_096;

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
    /// Maximum changed/context lines retained independently for each diff hunk.
    #[serde(default = "default_max_diff_hunk_lines")]
    pub max_diff_hunk_lines: usize,
    /// Explicit JSON Pointer (`/a/b`) or dot-path (`a.b`) values to prioritize.
    #[serde(default)]
    pub selected_json_paths: Vec<String>,
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
            max_diff_hunk_lines: default_max_diff_hunk_lines(),
            selected_json_paths: Vec::new(),
            required_markers: Vec::new(),
            selected_line_ranges: Vec::new(),
            referenced_symbols: Vec::new(),
            recent_turns: 8,
        }
    }
}

const fn default_max_diff_hunk_lines() -> usize {
    64
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
            let mut retained_markers = collect_json_markers_from_content(&content);
            retained_markers.extend(selected_json_path_markers(&content, policy));
            let omitted_paths = policy.selected_json_paths.len().saturating_sub(
                retained_markers
                    .iter()
                    .filter(|marker| marker.starts_with("path:"))
                    .count(),
            );
            if omitted_paths > 0 {
                omissions.push(omission("selected_json_paths_omitted", omitted_paths));
            }
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

    if let Some(candidate_content) = pruned_json_content(value, policy, limit, omissions) {
        return candidate_content;
    }

    if "0".len() <= limit {
        omissions.push(omission("minimal_json_fallback", 1));
        return "0".to_string();
    }

    "null".to_string()
}

fn pruned_json_content(
    value: &serde_json::Value,
    policy: &ReductionPolicy,
    limit: usize,
    omissions: &mut Vec<ReductionOmission>,
) -> Option<String> {
    let mut candidates = json_priority_candidates(value, policy);
    candidates.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.sort_key.cmp(&right.sort_key))
    });

    let mut output = match value {
        serde_json::Value::Object(_) => serde_json::Value::Object(serde_json::Map::new()),
        serde_json::Value::Array(_) => serde_json::Value::Array(Vec::new()),
        _ => compact_json_value(value),
    };
    let mut retained = 0;
    let mut skipped = 0;
    for candidate in candidates {
        let mut next = output.clone();
        if !insert_json_path(&mut next, &candidate.path, candidate.value) {
            skipped += 1;
            continue;
        }
        let serialized = serde_json::to_string(&next).expect("serializing pruned JSON");
        if serialized.len() <= limit {
            output = next;
            retained += 1;
        } else {
            skipped += 1;
        }
    }
    if skipped > 0 {
        omissions.push(omission("json_priority_values_omitted", skipped));
    }
    if retained == 0 {
        return None;
    }
    Some(serde_json::to_string(&output).expect("serializing pruned JSON"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum JsonPathSegment {
    Key(String),
    Index(usize),
}

struct JsonCandidate {
    priority: u8,
    sort_key: String,
    path: Vec<JsonPathSegment>,
    value: serde_json::Value,
}

fn json_priority_candidates(
    value: &serde_json::Value,
    policy: &ReductionPolicy,
) -> Vec<JsonCandidate> {
    let mut candidates = Vec::new();
    for path in &policy.selected_json_paths {
        let tokens = parse_json_path(path).expect("validated selected JSON path");
        if let Some((segments, selected)) = resolve_json_path(value, &tokens) {
            candidates.push(JsonCandidate {
                priority: 0,
                sort_key: path.clone(),
                path: segments,
                value: compact_json_value(selected),
            });
        }
    }
    collect_semantic_json_candidates(value, &mut Vec::new(), &mut candidates);
    candidates.truncate(MAX_JSON_PRIORITY_CANDIDATES);
    candidates
}

fn collect_semantic_json_candidates(
    value: &serde_json::Value,
    path: &mut Vec<JsonPathSegment>,
    candidates: &mut Vec<JsonCandidate>,
) {
    if candidates.len() >= MAX_JSON_PRIORITY_CANDIDATES {
        return;
    }
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                path.push(JsonPathSegment::Key(key.clone()));
                if let Some(priority) = json_semantic_priority(key) {
                    candidates.push(JsonCandidate {
                        priority,
                        sort_key: json_path_sort_key(path),
                        path: path.clone(),
                        value: compact_json_value(child),
                    });
                }
                collect_semantic_json_candidates(child, path, candidates);
                path.pop();
                if candidates.len() >= MAX_JSON_PRIORITY_CANDIDATES {
                    break;
                }
            }
        }
        serde_json::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                path.push(JsonPathSegment::Index(index));
                collect_semantic_json_candidates(child, path, candidates);
                path.pop();
                if candidates.len() >= MAX_JSON_PRIORITY_CANDIDATES {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn json_semantic_priority(key: &str) -> Option<u8> {
    let lower = key.to_ascii_lowercase();
    if lower == "error"
        || lower == "errors"
        || lower.ends_with("_error")
        || lower.ends_with("_errors")
    {
        Some(1)
    } else if lower == "id" || lower == "identifier" || lower.ends_with("_id") {
        Some(2)
    } else if matches!(lower.as_str(), "count" | "total" | "size")
        || lower.ends_with("_count")
        || lower.ends_with("_total")
        || lower.ends_with("_size")
    {
        Some(3)
    } else {
        None
    }
}

fn compact_json_value(value: &serde_json::Value) -> serde_json::Value {
    let mut remaining = MAX_COMPACT_JSON_VALUES;
    compact_json_value_with_budget(value, &mut remaining)
}

fn compact_json_value_with_budget(
    value: &serde_json::Value,
    remaining: &mut usize,
) -> serde_json::Value {
    if *remaining == 0 {
        return match value {
            serde_json::Value::Array(_) => serde_json::Value::Array(Vec::new()),
            serde_json::Value::Object(_) => serde_json::Value::Object(serde_json::Map::new()),
            _ => value.clone(),
        };
    }
    *remaining -= 1;
    match value {
        serde_json::Value::Array(values) => {
            let mut compact = Vec::new();
            for child in values {
                if *remaining == 0 {
                    break;
                }
                compact.push(compact_json_value_with_budget(child, remaining));
            }
            serde_json::Value::Array(compact)
        }
        serde_json::Value::Object(map) => {
            let mut compact = serde_json::Map::new();
            for (key, child) in map {
                if *remaining == 0 {
                    break;
                }
                compact.insert(
                    key.clone(),
                    compact_json_value_with_budget(child, remaining),
                );
            }
            serde_json::Value::Object(compact)
        }
        _ => value.clone(),
    }
}

fn insert_json_path(
    current: &mut serde_json::Value,
    path: &[JsonPathSegment],
    value: serde_json::Value,
) -> bool {
    let Some((segment, rest)) = path.split_first() else {
        *current = value;
        return true;
    };
    match segment {
        JsonPathSegment::Key(key) => {
            let serde_json::Value::Object(map) = current else {
                return false;
            };
            if rest.is_empty() {
                map.insert(key.clone(), value);
                return true;
            }
            let child = map
                .entry(key.clone())
                .or_insert_with(|| empty_json_container(&rest[0]));
            insert_json_path(child, rest, value)
        }
        JsonPathSegment::Index(index) => {
            let serde_json::Value::Array(values) = current else {
                return false;
            };
            while values.len() <= *index {
                values.push(json_omission_sentinel());
            }
            if rest.is_empty() {
                values[*index] = value;
                return true;
            }
            if values[*index].get("$omitted").is_some() {
                values[*index] = empty_json_container(&rest[0]);
            }
            insert_json_path(&mut values[*index], rest, value)
        }
    }
}

fn empty_json_container(segment: &JsonPathSegment) -> serde_json::Value {
    match segment {
        JsonPathSegment::Key(_) => serde_json::Value::Object(serde_json::Map::new()),
        JsonPathSegment::Index(_) => serde_json::Value::Array(Vec::new()),
    }
}

fn json_omission_sentinel() -> serde_json::Value {
    serde_json::json!({"$omitted": true})
}

fn json_path_sort_key(path: &[JsonPathSegment]) -> String {
    path.iter()
        .map(|segment| match segment {
            JsonPathSegment::Key(key) => format!("/{key}"),
            JsonPathSegment::Index(index) => format!("/{index:020}"),
        })
        .collect()
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

fn selected_json_path_markers(content: &str, policy: &ReductionPolicy) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    policy
        .selected_json_paths
        .iter()
        .filter(|path| {
            parse_json_path(path)
                .as_ref()
                .is_some_and(|tokens| resolve_json_path(&value, tokens).is_some())
        })
        .map(|path| format!("path:{path}"))
        .collect()
}

fn parse_json_path(path: &str) -> Option<Vec<String>> {
    if path.starts_with('/') {
        let mut segments = Vec::new();
        for encoded in path.split('/').skip(1) {
            let mut decoded = String::new();
            let mut characters = encoded.chars();
            while let Some(character) = characters.next() {
                if character == '~' {
                    match characters.next()? {
                        '0' => decoded.push('~'),
                        '1' => decoded.push('/'),
                        _ => return None,
                    }
                } else {
                    decoded.push(character);
                }
            }
            segments.push(decoded);
        }
        Some(segments)
    } else {
        path.split('.')
            .map(|segment| {
                if segment.is_empty()
                    || !segment.chars().all(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                    })
                {
                    None
                } else {
                    Some(segment.to_string())
                }
            })
            .collect()
    }
}

fn resolve_json_path<'a>(
    mut value: &'a serde_json::Value,
    tokens: &[String],
) -> Option<(Vec<JsonPathSegment>, &'a serde_json::Value)> {
    let mut resolved = Vec::with_capacity(tokens.len());
    for token in tokens {
        value = match value {
            serde_json::Value::Object(map) => {
                resolved.push(JsonPathSegment::Key(token.clone()));
                map.get(token)?
            }
            serde_json::Value::Array(values) => {
                let index = parse_json_array_index(token)?;
                resolved.push(JsonPathSegment::Index(index));
                values.get(index)?
            }
            _ => return None,
        };
    }
    Some((resolved, value))
}

fn parse_json_array_index(token: &str) -> Option<usize> {
    if token == "0" {
        return Some(0);
    }
    if token.starts_with('0')
        || token.is_empty()
        || !token.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    token.parse().ok()
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
    if key.is_some_and(is_secret_label) {
        *value = serde_json::Value::String("[REDACTED]".to_string());
        *redacted += 1;
        return;
    }
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
            let next = redact_text(text);
            if next != *text {
                *text = next;
                *redacted += 1;
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

    let selected_ranges = normalized_line_ranges(&policy.selected_line_ranges);
    let mut selected_range_index = 0;
    for (index, line) in all_lines.iter().enumerate() {
        let line_number = index + 1;
        while selected_range_index < selected_ranges.len()
            && selected_ranges[selected_range_index].end < line_number
        {
            selected_range_index += 1;
        }
        if selected_ranges
            .get(selected_range_index)
            .is_some_and(|range| line_number >= range.start && line_number <= range.end)
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

fn reduce_diff(input: &[u8], policy: &ReductionPolicy) -> ReductionResult {
    let text = String::from_utf8_lossy(input);
    let mut lines = Vec::new();
    let mut omitted = 0;
    let mut omitted_hunk_lines = 0;
    let mut retained_markers = Vec::new();
    let mut in_hunk = false;
    let mut retained_hunk_lines = 0;

    for line in text.lines() {
        if line.starts_with("diff --git ") {
            lines.push(line.to_string());
            retained_markers.push("diff_file".to_string());
            in_hunk = false;
        } else if let Some(marker) = diff_metadata_marker(line) {
            lines.push(line.to_string());
            retained_markers.push(marker.to_string());
        } else if line.starts_with("index ") || line.starts_with("--- ") || line.starts_with("+++ ")
        {
            lines.push(line.to_string());
        } else if line.starts_with("@@ ") {
            lines.push(line.to_string());
            retained_markers.push("diff_hunk".to_string());
            in_hunk = true;
            retained_hunk_lines = 0;
        } else if in_hunk && is_diff_hunk_body_line(line) {
            if retained_hunk_lines < policy.max_diff_hunk_lines {
                lines.push(line.to_string());
                if is_changed_diff_line(line) {
                    retained_markers.push("changed_line".to_string());
                    if line.contains("unsafe") || line.contains("TODO") {
                        retained_markers.push("risky_change".to_string());
                    }
                    if let Some(symbol) = changed_diff_symbol(line) {
                        retained_markers.push(format!("symbol:{symbol}"));
                    }
                }
            } else {
                omitted_hunk_lines += 1;
            }
            retained_hunk_lines += 1;
        } else if !line.trim().is_empty() {
            omitted += 1;
        }
    }

    let mut view = result(lines.join("\n"), retained_markers, false);
    if omitted > 0 {
        view.omissions
            .push(omission("diff_context_omitted", omitted));
    }
    if omitted_hunk_lines > 0 {
        view.omissions
            .push(omission("diff_hunk_lines_omitted", omitted_hunk_lines));
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

fn is_diff_hunk_body_line(line: &str) -> bool {
    line.starts_with(' ')
        || is_changed_diff_line(line)
        || line.starts_with("\\ No newline at end of file")
}

fn changed_diff_symbol(line: &str) -> Option<&str> {
    if !is_changed_diff_line(line) {
        return None;
    }
    let source = line[1..].trim_start();
    let identifiers = source
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|identifier| !identifier.is_empty())
        .collect::<Vec<_>>();
    for (index, identifier) in identifiers.iter().enumerate() {
        if matches!(
            *identifier,
            "fn" | "def" | "function" | "class" | "struct" | "enum" | "trait"
        ) {
            return identifiers.get(index + 1).copied();
        }
        if matches!(*identifier, "const" | "let" | "var")
            && (source.contains("=>") || source.contains("function"))
        {
            return identifiers.get(index + 1).copied();
        }
    }
    None
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
        if index == all_lines.len() - 1 || !kept_values.contains(line) {
            kept_indices.insert(index);
        }
    }
    if let Some(last_line) = all_lines.last() {
        let normalized_last_line = normalized_evidence_line(last_line);
        for (index, line) in all_lines.iter().enumerate() {
            if normalized_evidence_line(line) == normalized_last_line {
                kept_indices.insert(index);
            }
        }
        let ordinal = all_lines
            .iter()
            .filter(|line| normalized_evidence_line(line) == normalized_last_line)
            .count();
        retained_markers.push(evidence_line_marker("tail", last_line, ordinal));
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
    let mut retained_indices = HashSet::new();
    let mut retained_markers = Vec::new();

    for (index, line) in all_lines.iter().enumerate() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("constraint:") || lower.contains("must ") || lower.contains("do not ") {
            if push_unique_if_new(&mut lines, &mut seen, (*line).to_string()) {
                retained_indices.insert(index);
            }
            retained_markers.push("constraint".to_string());
        } else if lower.contains("decision:") || lower.contains("decided") {
            if push_unique_if_new(&mut lines, &mut seen, (*line).to_string()) {
                retained_indices.insert(index);
            }
            retained_markers.push("decision".to_string());
        } else if lower.contains("unresolved")
            || lower.contains("question:")
            || lower.ends_with('?')
        {
            if push_unique_if_new(&mut lines, &mut seen, (*line).to_string()) {
                retained_indices.insert(index);
            }
            retained_markers.push("question".to_string());
        }
    }

    let recent_count = policy.recent_turns.min(all_lines.len());
    if recent_count > 0 {
        let normalized_last_line = normalized_evidence_line(all_lines[all_lines.len() - 1]);
        for (index, line) in all_lines
            .iter()
            .enumerate()
            .take(all_lines.len().saturating_sub(1))
        {
            if !retained_indices.contains(&index)
                && normalized_evidence_line(line) == normalized_last_line
            {
                lines.push((*line).to_string());
                seen.insert((*line).to_string());
                retained_indices.insert(index);
            }
        }
    }
    for (index, line) in all_lines
        .iter()
        .enumerate()
        .skip(all_lines.len().saturating_sub(recent_count))
    {
        if retained_indices.contains(&index) {
            continue;
        }
        if index == all_lines.len() - 1 {
            lines.push((*line).to_string());
            seen.insert((*line).to_string());
            retained_indices.insert(index);
        } else if push_unique_if_new(&mut lines, &mut seen, (*line).to_string()) {
            retained_indices.insert(index);
        }
    }
    if recent_count > 0 {
        let last_line = all_lines[all_lines.len() - 1];
        let normalized_last_line = normalized_evidence_line(last_line);
        let ordinal = all_lines
            .iter()
            .filter(|line| normalized_evidence_line(line) == normalized_last_line)
            .count();
        retained_markers.push(evidence_line_marker("recent_turn", last_line, ordinal));
    }

    let mut view = result(lines.join("\n"), retained_markers, false);
    let omitted = all_lines.len().saturating_sub(retained_indices.len());
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
    if policy.max_diff_hunk_lines == 0 || policy.max_diff_hunk_lines > MAX_DIFF_HUNK_LINES_LIMIT {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "max_diff_hunk_lines",
            reason: "must be between 1 and 4096",
        });
    }
    if policy.selected_json_paths.len() > MAX_SELECTED_JSON_PATHS
        || policy.selected_json_paths.iter().any(|path| {
            path.is_empty()
                || path.len() > MAX_SELECTED_JSON_PATH_BYTES
                || parse_json_path(path)
                    .is_none_or(|segments| segments.len() > MAX_JSON_PATH_SEGMENTS)
        })
    {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "selected_json_paths",
            reason: "entries must be bounded JSON pointers or dot paths",
        });
    }
    if policy.required_markers.len() > MAX_REQUIRED_MARKERS
        || policy
            .required_markers
            .iter()
            .any(|marker| marker.is_empty() || marker.len() > MAX_REQUIRED_MARKER_BYTES)
    {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "required_markers",
            reason: "entries must be non-empty and bounded",
        });
    }
    if policy.selected_line_ranges.len() > MAX_SELECTED_LINE_RANGES
        || policy.selected_line_ranges.iter().any(|range| {
            range.start == 0
                || range.start > range.end
                || range.end > MAX_SELECTED_LINE_NUMBER
                || range.end - range.start + 1 > MAX_SELECTED_LINE_RANGE_SPAN
        })
    {
        return Err(crate::ContextError::InvalidReductionPolicy {
            field: "selected_line_ranges",
            reason: "entries must be ordered, positive, and bounded",
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

fn normalized_line_ranges(ranges: &[LineRange]) -> Vec<LineRange> {
    let mut normalized = ranges.to_vec();
    normalized.sort_by_key(|range| (range.start, range.end));

    let mut merged: Vec<LineRange> = Vec::with_capacity(normalized.len());
    for range in normalized {
        if let Some(previous) = merged.last_mut()
            && range.start <= previous.end.saturating_add(1)
        {
            previous.end = previous.end.max(range.end);
            continue;
        }
        merged.push(range);
    }
    merged
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
    let mut markers = match kind {
        ContextContentKind::Json => {
            let mut markers = collect_json_markers_from_content(&view.content);
            markers.extend(
                view.retained_markers
                    .iter()
                    .filter(|marker| marker.starts_with("path:"))
                    .filter(|marker| {
                        marker.strip_prefix("path:").is_some_and(|path| {
                            let Ok(value) =
                                serde_json::from_str::<serde_json::Value>(&view.content)
                            else {
                                return false;
                            };
                            parse_json_path(path)
                                .as_ref()
                                .is_some_and(|tokens| resolve_json_path(&value, tokens).is_some())
                        })
                    })
                    .cloned(),
            );
            markers
        }
        _ => view
            .retained_markers
            .iter()
            .filter(|marker| final_marker_supported(marker, &view.content))
            .cloned()
            .collect::<Vec<_>>(),
    };
    if markers.iter().any(|marker| marker.starts_with("tail:")) {
        markers.push("tail".to_string());
    }
    if markers
        .iter()
        .any(|marker| marker.starts_with("recent_turn:"))
    {
        markers.push("recent_turn".to_string());
    }
    markers
}

fn final_marker_supported(marker: &str, content: &str) -> bool {
    match marker {
        "first_failure" | "unique_error" => contains_failure_line(content),
        "command" => content.lines().any(is_log_command_line),
        "exit_status" => content.lines().any(is_log_exit_status),
        "failing_location" => content.lines().any(has_failing_location),
        marker if marker.starts_with("tail:") => evidence_line_marker_supported(marker, content),
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
        marker if marker.starts_with("recent_turn:") => {
            evidence_line_marker_supported(marker, content)
        }
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

fn normalized_evidence_line(line: &str) -> String {
    redact_line(line).trim().to_string()
}

fn evidence_line_marker(prefix: &str, line: &str, ordinal: usize) -> String {
    let hash = evidence_line_hash(line);
    format!("{prefix}:{ordinal}:{hash}")
}

fn evidence_line_hash(line: &str) -> String {
    let normalized = normalized_evidence_line(line);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex_prefix(&hasher.finalize(), 16)
}

fn evidence_line_marker_supported(marker: &str, content: &str) -> bool {
    let mut parts = marker.split(':');
    let (Some(_prefix), Some(ordinal), Some(hash), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let Ok(ordinal) = ordinal.parse::<usize>() else {
        return false;
    };
    content
        .lines()
        .filter(|line| evidence_line_hash(line) == hash)
        .count()
        >= ordinal
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
        labeled_reference
            || declaration_header_declares_symbol(trimmed, symbol)
            || changed_diff_symbol(trimmed).is_some_and(|retained| retained == symbol)
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
    let authorization_redacted = redact_authorization(line);
    let bearer_redacted = redact_bearer(&authorization_redacted);
    redact_assignment(&bearer_redacted)
}

fn redact_authorization(line: &str) -> String {
    let mut output = line.to_string();
    let mut cursor = 0;
    while let Some(label_start) = find_ascii_case_insensitive(&output, cursor, "authorization") {
        let label_end = label_start + "authorization".len();
        if !has_token_boundary_before(&output, label_start) {
            cursor = label_end;
            continue;
        }
        let separator = skip_whitespace(&output, label_end);
        if !matches!(output.as_bytes().get(separator), Some(b':' | b'=')) {
            cursor = label_end;
            continue;
        }
        let Some((value_start, value_end)) = authorization_value_span(&output, separator) else {
            cursor = separator + 1;
            continue;
        };
        if &output[value_start..value_end] == "[REDACTED]" {
            cursor = value_end;
            continue;
        }
        output.replace_range(value_start..value_end, "[REDACTED]");
        cursor = value_start + "[REDACTED]".len();
    }
    output
}

fn redact_bearer(line: &str) -> String {
    let mut output = line.to_string();
    let mut cursor = 0;
    while let Some(label_start) = find_ascii_case_insensitive(&output, cursor, "bearer") {
        let label_end = label_start + "bearer".len();
        if !has_token_boundary_before(&output, label_start) {
            cursor = label_end;
            continue;
        }
        let Some(next) = output[label_end..].chars().next() else {
            break;
        };
        let value_hint = if next.is_whitespace() {
            label_end
        } else if matches!(next, ':' | '=') {
            label_end + next.len_utf8()
        } else {
            cursor = label_end;
            continue;
        };
        let Some((value_start, value_end)) = value_span_at(&output, value_hint) else {
            cursor = label_end;
            continue;
        };
        if &output[value_start..value_end] == "[REDACTED]" {
            cursor = value_end;
            continue;
        }
        output.replace_range(value_start..value_end, "[REDACTED]");
        cursor = value_start + "[REDACTED]".len();
    }
    output
}

fn find_ascii_case_insensitive(line: &str, start: usize, needle: &str) -> Option<usize> {
    line[start..]
        .to_ascii_lowercase()
        .find(needle)
        .map(|offset| start + offset)
}

fn has_token_boundary_before(line: &str, start: usize) -> bool {
    line[..start]
        .chars()
        .next_back()
        .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_')
}

fn authorization_value_span(line: &str, separator: usize) -> Option<(usize, usize)> {
    let value_start = skip_whitespace(line, separator + 1);
    let scheme_end = line[value_start..]
        .find(|character: char| !character.is_ascii_alphabetic())
        .map_or(line.len(), |offset| value_start + offset);
    let scheme = line[value_start..scheme_end].to_ascii_lowercase();
    if matches!(scheme.as_str(), "bearer" | "basic") {
        let mut credential_hint = scheme_end;
        if matches!(line[credential_hint..].chars().next(), Some(':' | '=')) {
            credential_hint += 1;
        }
        return value_span_at(line, credential_hint);
    }
    value_span_at(line, value_start)
}

fn skip_whitespace(line: &str, mut start: usize) -> usize {
    while let Some(character) = line[start..].chars().next() {
        if !character.is_whitespace() {
            break;
        }
        start += character.len_utf8();
    }
    start
}

fn redact_assignment(line: &str) -> String {
    let mut output = line.to_string();
    let mut search_start = 0;
    while let Some((offset, separator)) = output[search_start..]
        .char_indices()
        .find(|(_, character)| matches!(character, '=' | ':'))
    {
        let separator_index = search_start + offset;
        let Some(label) = assignment_label(&output, separator_index, separator) else {
            search_start = separator_index + separator.len_utf8();
            continue;
        };
        if label == "authorization" {
            search_start = separator_index + separator.len_utf8();
            continue;
        }
        if !is_secret_label(&label) {
            search_start = separator_index + separator.len_utf8();
            continue;
        }
        let Some((value_start, value_end)) = assignment_value_span(&output, separator_index) else {
            search_start = separator_index + separator.len_utf8();
            continue;
        };
        if &output[value_start..value_end] == "[REDACTED]" {
            search_start = value_end;
            continue;
        }
        output.replace_range(value_start..value_end, "[REDACTED]");
        search_start = value_start + "[REDACTED]".len();
    }
    output
}

fn assignment_label(line: &str, separator_index: usize, separator: char) -> Option<String> {
    if separator == ':' && looks_like_typed_declaration(&line[..separator_index]) {
        return None;
    }
    let direct = normalized_label_before(line, separator_index)?;
    if separator == '='
        && !is_secret_label(&direct)
        && let Some(type_separator) = line[..separator_index].rfind(':')
    {
        let typed_label = normalized_label_before(line, type_separator)?;
        if is_secret_label(&typed_label) {
            return Some(typed_label);
        }
    }
    Some(direct)
}

fn looks_like_typed_declaration(prefix: &str) -> bool {
    let trimmed = prefix.trim_start_matches(|character: char| {
        character.is_whitespace() || matches!(character, '+' | '-' | 'L')
    });
    matches!(
        trimmed.split_ascii_whitespace().next(),
        Some("const" | "static" | "let")
    )
}

fn normalized_label_before(line: &str, end: usize) -> Option<String> {
    let mut token_end = end;
    while let Some(character) = line[..token_end].chars().next_back() {
        if character.is_whitespace() || matches!(character, '"' | '\'') {
            token_end -= character.len_utf8();
        } else {
            break;
        }
    }
    let mut token_start = token_end;
    while let Some(character) = line[..token_start].chars().next_back() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            token_start -= character.len_utf8();
        } else {
            break;
        }
    }
    let label = line[token_start..token_end].trim_matches('-');
    (!label.is_empty()).then(|| label.to_ascii_lowercase().replace('-', "_"))
}

fn assignment_value_span(line: &str, separator_index: usize) -> Option<(usize, usize)> {
    value_span_at(line, separator_index + 1)
}

fn value_span_at(line: &str, hint: usize) -> Option<(usize, usize)> {
    let value_start = skip_whitespace(line, hint);
    if line[value_start..].starts_with("[REDACTED]") {
        return Some((value_start, value_start + "[REDACTED]".len()));
    }
    let first = line[value_start..].chars().next()?;
    if matches!(first, ',' | ';' | ')' | ']' | '}') {
        return None;
    }
    if matches!(first, '"' | '\'') {
        let content_start = value_start + first.len_utf8();
        if line[content_start..]
            .chars()
            .next()
            .is_none_or(|character| {
                character.is_whitespace() || matches!(character, ',' | ';' | ')' | ']' | '}')
            })
        {
            return None;
        }
        let mut escaped = false;
        for (offset, character) in line[content_start..].char_indices() {
            if character == first && !escaped {
                return (offset > 0).then_some((content_start, content_start + offset));
            }
            escaped = character == '\\' && !escaped;
            if character != '\\' {
                escaped = false;
            }
        }
        return (content_start < line.len()).then_some((content_start, line.len()));
    }
    let value_end = line[value_start..]
        .find(|character: char| {
            character.is_whitespace()
                || matches!(character, ',' | ';' | ')' | ']' | '}' | '"' | '\'')
        })
        .map_or(line.len(), |offset| value_start + offset);
    (value_start < value_end).then_some((value_start, value_end))
}

fn is_secret_label(label: &str) -> bool {
    let normalized = label.to_ascii_lowercase().replace('-', "_");
    matches!(
        normalized.as_str(),
        "authorization"
            | "password"
            | "passwd"
            | "secret"
            | "api_key"
            | "apikey"
            | "token"
            | "api_token"
            | "access_token"
            | "auth_token"
            | "bearer_token"
            | "refresh_token"
            | "credential"
            | "credentials"
            | "client_credential"
            | "client_credentials"
            | "client_secret"
    ) || normalized.ends_with("_password")
        || normalized.ends_with("_secret")
        || normalized.ends_with("_api_key")
        || normalized.ends_with("_credential")
        || normalized.ends_with("_credentials")
}

fn push_unique(lines: &mut Vec<String>, seen: &mut HashSet<String>, line: String) {
    let _ = push_unique_if_new(lines, seen, line);
}

fn push_unique_if_new(lines: &mut Vec<String>, seen: &mut HashSet<String>, line: String) -> bool {
    if seen.insert(line.clone()) {
        lines.push(line);
        true
    } else {
        false
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
            max_diff_hunk_lines: default_max_diff_hunk_lines(),
            selected_json_paths: Vec::new(),
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
        let parsed = serde_json::from_str::<serde_json::Value>(&view.content).unwrap();
        assert_eq!(parsed["errors"][0]["message"], "boom");
        assert_eq!(parsed["id"], "ctx-1");
        assert!(parsed.get("a").is_none());
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
    fn json_reducer_preserves_semantic_and_selected_values_before_noise() {
        let input = br#"{
            "noise":{"blob":"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz","items":[1,2,3,4,5]},
            "errors":[{"code":"E1","message":"boom"}],
            "request_id":"req-7",
            "count":3,
            "total":9,
            "size":42,
            "profile":{"name":"Ada","password":"raw-secret","bio":"long unneeded biography"},
            "settings":{"mode":"strict","retries":2},
            "meta":{"trace":"trace-9","noise":"discard me"}
        }"#;
        let policy = ReductionPolicy {
            max_output_bytes: 280,
            selected_json_paths: vec![
                "/profile/name".to_string(),
                "settings".to_string(),
                "meta.trace".to_string(),
            ],
            ..ReductionPolicy::default()
        };

        let view = reduce(ContextContentKind::Json, input, &policy).unwrap();
        let again = reduce(ContextContentKind::Json, input, &policy).unwrap();
        let parsed = serde_json::from_str::<serde_json::Value>(&view.content).unwrap();

        assert_eq!(
            view.content,
            r#"{"count":3,"errors":[{"code":"E1","message":"boom"}],"meta":{"trace":"trace-9"},"profile":{"name":"Ada"},"request_id":"req-7","settings":{"mode":"strict","retries":2},"size":42,"total":9}"#
        );
        assert_eq!(parsed["errors"][0]["message"], "boom");
        assert_eq!(parsed["request_id"], "req-7");
        assert_eq!(parsed["count"], 3);
        assert_eq!(parsed["total"], 9);
        assert_eq!(parsed["size"], 42);
        assert_eq!(parsed["profile"]["name"], "Ada");
        assert_eq!(parsed["settings"]["mode"], "strict");
        assert_eq!(parsed["settings"]["retries"], 2);
        assert_eq!(parsed["meta"]["trace"], "trace-9");
        assert!(parsed.get("noise").is_none());
        assert!(parsed["profile"].get("bio").is_none());
        assert!(view.content.len() <= 280);
        assert_eq!(
            view.retained_markers,
            vec![
                "key:code",
                "key:count",
                "key:errors",
                "key:message",
                "key:meta",
                "key:mode",
                "key:name",
                "key:profile",
                "key:request_id",
                "key:retries",
                "key:settings",
                "key:size",
                "key:total",
                "key:trace",
                "path:/profile/name",
                "path:meta.trace",
                "path:settings",
            ]
        );
        let serialized = serde_json::to_string(&view).unwrap();
        assert_eq!(serialized, serde_json::to_string(&again).unwrap());
        assert!(!serialized.contains("raw-secret"));
    }

    #[test]
    fn selected_json_credentials_are_redacted_and_required_paths_use_final_output() {
        let input = br#"{"profile":{"api_key":"raw-secret","name":"Ada"},"noise":"abcdefghijklmnopqrstuvwxyz"}"#;
        let policy = ReductionPolicy {
            max_output_bytes: 96,
            selected_json_paths: vec!["/profile/api_key".to_string()],
            ..ReductionPolicy::default()
        };
        let view = reduce(ContextContentKind::Json, input, &policy).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&view.content).unwrap()["profile"]["api_key"],
            "[REDACTED]"
        );
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker == "path:/profile/api_key")
        );

        let tight = ReductionPolicy {
            max_output_bytes: 8,
            selected_json_paths: vec!["/profile/name".to_string()],
            required_markers: vec!["path:/profile/name".to_string()],
            ..ReductionPolicy::default()
        };
        assert!(matches!(
            reduce(ContextContentKind::Json, input, &tight),
            Err(crate::ContextError::QualityFailed { .. })
        ));
    }

    #[test]
    fn secret_json_keys_replace_every_value_type_before_priority_reduction() {
        let input = br#"{
            "api_key":123456,
            "token":true,
            "password":null,
            "secret":["array-secret",{"nested":"array-object-secret"}],
            "profile":{"password":{"raw":"object-secret","nested":["deeper-secret"]}},
            "errors":[{"api_key":{"raw":"error-object-secret"},"message":"boom"}],
            "noise":"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz"
        }"#;
        let selected_json_paths = vec![
            "/api_key".to_string(),
            "/token".to_string(),
            "/password".to_string(),
            "/secret".to_string(),
            "/profile/password".to_string(),
            "/errors/0/api_key".to_string(),
        ];
        let policy = ReductionPolicy {
            max_output_bytes: 320,
            selected_json_paths: selected_json_paths.clone(),
            ..ReductionPolicy::default()
        };

        let view = reduce(ContextContentKind::Json, input, &policy).unwrap();
        let parsed = serde_json::from_str::<serde_json::Value>(&view.content).unwrap();
        let serialized = serde_json::to_string(&view).unwrap();

        for path in &selected_json_paths {
            let tokens = parse_json_path(path).unwrap();
            let (_, value) = resolve_json_path(&parsed, &tokens).unwrap();
            assert_eq!(value, "[REDACTED]", "{path}");
            assert!(
                view.retained_markers
                    .iter()
                    .any(|marker| marker == &format!("path:{path}")),
                "{path}"
            );
        }
        assert_eq!(parsed["errors"][0]["message"], "boom");
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker == "key:errors")
        );
        assert_eq!(
            view.omissions
                .iter()
                .find(|omission| omission.reason == "secret_values_redacted")
                .map(|omission| omission.omitted_count),
            Some(6)
        );
        for raw in [
            "123456",
            "array-secret",
            "array-object-secret",
            "object-secret",
            "deeper-secret",
            "error-object-secret",
        ] {
            assert!(!serialized.contains(raw), "{raw}");
        }
    }

    #[test]
    fn selected_json_paths_are_bounded_and_validated() {
        let invalid_paths = [
            vec![String::new()],
            vec!["/bad/~2escape".to_string()],
            vec!["/bad/~".to_string()],
            vec![format!("/bad/~{}", "raw-secret".repeat(32))],
            vec!["bad..path".to_string()],
            vec!["x".repeat(257)],
            vec!["field".to_string(); 65],
        ];

        for selected_json_paths in invalid_paths {
            let policy = ReductionPolicy {
                selected_json_paths,
                ..ReductionPolicy::default()
            };
            let error = reduce(ContextContentKind::Json, br#"{"field":1}"#, &policy)
                .expect_err("invalid JSON paths must fail policy validation");
            assert!(matches!(
                error,
                crate::ContextError::InvalidReductionPolicy {
                    field: "selected_json_paths",
                    ..
                }
            ));
            assert!(!error.to_string().contains("raw-secret"));
        }
    }

    #[test]
    fn json_pointer_resolves_numeric_object_keys_and_array_indices_at_runtime() {
        let input = br#"{
            "0":{"a/b":{"~key":"object-zero"}},
            "items":[{"name":"zero"},{"name":"one"}],
            "noise":"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz"
        }"#;
        let policy = ReductionPolicy {
            max_output_bytes: 112,
            selected_json_paths: vec!["/0/a~1b/~0key".to_string(), "/items/1/name".to_string()],
            ..ReductionPolicy::default()
        };

        let view = reduce(ContextContentKind::Json, input, &policy).unwrap();
        let parsed = serde_json::from_str::<serde_json::Value>(&view.content).unwrap();

        assert_eq!(parsed["0"]["a/b"]["~key"], "object-zero");
        assert_eq!(parsed["items"][1]["name"], "one");
        assert!(parsed.get("noise").is_none());
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker == "path:/0/a~1b/~0key")
        );
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker == "path:/items/1/name")
        );
    }

    #[test]
    fn required_marker_and_line_range_caps_are_non_leaking() {
        let policies = [
            ReductionPolicy {
                required_markers: vec!["marker".to_string(); 65],
                ..ReductionPolicy::default()
            },
            ReductionPolicy {
                required_markers: vec![format!("literal:{}", "raw-secret".repeat(32))],
                ..ReductionPolicy::default()
            },
            ReductionPolicy {
                selected_line_ranges: vec![LineRange { start: 1, end: 1 }; 129],
                ..ReductionPolicy::default()
            },
            ReductionPolicy {
                selected_line_ranges: vec![LineRange { start: 0, end: 1 }],
                ..ReductionPolicy::default()
            },
            ReductionPolicy {
                selected_line_ranges: vec![LineRange { start: 8, end: 7 }],
                ..ReductionPolicy::default()
            },
            ReductionPolicy {
                selected_line_ranges: vec![LineRange {
                    start: 1,
                    end: 1_000_001,
                }],
                ..ReductionPolicy::default()
            },
            ReductionPolicy {
                selected_line_ranges: vec![LineRange {
                    start: 1,
                    end: 100_002,
                }],
                ..ReductionPolicy::default()
            },
        ];

        for policy in policies {
            let error = reduce(ContextContentKind::Code, b"fn ok() {}", &policy)
                .expect_err("invalid bounded policy must fail");
            assert!(matches!(
                error,
                crate::ContextError::InvalidReductionPolicy { .. }
            ));
            assert!(!error.to_string().contains("raw-secret"));
        }
    }

    #[test]
    fn selected_line_ranges_are_sorted_and_merged_deterministically() {
        let input = b"one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\n";
        let policy = ReductionPolicy {
            selected_line_ranges: vec![
                LineRange { start: 5, end: 6 },
                LineRange { start: 2, end: 3 },
                LineRange { start: 3, end: 4 },
                LineRange { start: 8, end: 8 },
                LineRange { start: 7, end: 7 },
            ],
            ..ReductionPolicy::default()
        };

        let first = reduce(ContextContentKind::Code, input, &policy).unwrap();
        let second = reduce(ContextContentKind::Code, input, &policy).unwrap();

        assert_eq!(
            first.content,
            "L2: two\nL3: three\nL4: four\nL5: five\nL6: six\nL7: seven\nL8: eight"
        );
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
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
    fn diff_reducer_bounds_each_hunk_and_marks_changed_symbols() {
        let input = b"diff --git a/src/lib.rs b/src/lib.rs\nnew file mode 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,4 +1,4 @@\n-pub fn old() {}\n+pub fn new() {}\n+// TODO unsafe migration\n context omitted\n@@ -10,4 +10,4 @@\n class Context {}\n-class Old {}\n+class New {}\n context omitted\n";
        let policy = ReductionPolicy {
            max_diff_hunk_lines: 3,
            ..ReductionPolicy::default()
        };

        let view = reduce(ContextContentKind::Diff, input, &policy).unwrap();
        let again = reduce(ContextContentKind::Diff, input, &policy).unwrap();

        assert_eq!(
            view.content,
            "diff --git a/src/lib.rs b/src/lib.rs\n\
             new file mode 100644\n\
             --- a/src/lib.rs\n\
             +++ b/src/lib.rs\n\
             @@ -1,4 +1,4 @@\n\
             -pub fn old() {}\n\
             +pub fn new() {}\n\
             +// TODO unsafe migration\n\
             @@ -10,4 +10,4 @@\n\
             \x20class Context {}\n\
             -class Old {}\n\
             +class New {}"
        );
        assert_eq!(
            view.retained_markers,
            vec![
                "changed_line",
                "diff:new_file_mode",
                "diff_file",
                "diff_hunk",
                "risky_change",
                "symbol:New",
                "symbol:Old",
                "symbol:new",
                "symbol:old",
            ]
        );
        assert_eq!(view.omissions, vec![omission("diff_hunk_lines_omitted", 2)]);
        assert_eq!(
            serde_json::to_string(&view).unwrap(),
            serde_json::to_string(&again).unwrap()
        );
    }

    #[test]
    fn diff_hunk_limit_is_validated() {
        for max_diff_hunk_lines in [0, 4_097] {
            let policy = ReductionPolicy {
                max_diff_hunk_lines,
                ..ReductionPolicy::default()
            };
            assert!(matches!(
                reduce(
                    ContextContentKind::Diff,
                    b"@@ -1 +1 @@\n-old\n+new",
                    &policy
                ),
                Err(crate::ContextError::InvalidReductionPolicy {
                    field: "max_diff_hunk_lines",
                    ..
                })
            ));
        }
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
             L6:     let password = \"[REDACTED]\";\n\
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
                r#"{"content":"diff --git a/a.rs b/a.rs\nindex 1..2 100644\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\n unchanged","original":{"byte_count":98,"token_count":25},"reduced":{"byte_count":97,"token_count":25},"omissions":[],"retained_markers":["changed_line","diff_file","diff_hunk"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_c30ff7bf279ca5ec","target_id":"viden-context-native:native-v1:Diff","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Log,
                b"running tests\nERROR src/a.rs:1 boom\nERROR src/a.rs:1 boom\nwarning\nfinal tail\n",
                ReductionPolicy::default(),
                r#"{"content":"ERROR src/a.rs:1 boom\nwarning\nfinal tail","original":{"byte_count":77,"token_count":20},"reduced":{"byte_count":40,"token_count":10},"omissions":[{"reason":"log_lines_omitted_or_deduplicated","omitted_count":2}],"retained_markers":["failing_location","first_failure","tail","tail:1:35cf3d46823685e9"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_a242b2a84535c876","target_id":"viden-context-native:native-v1:Log","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Diagnostic,
                b"running tests\nERROR src/a.rs:1 boom\nERROR src/a.rs:1 boom\nwarning\nfinal tail\n",
                ReductionPolicy::default(),
                r#"{"content":"ERROR src/a.rs:1 boom\nwarning\nfinal tail","original":{"byte_count":77,"token_count":20},"reduced":{"byte_count":40,"token_count":10},"omissions":[{"reason":"log_lines_omitted_or_deduplicated","omitted_count":2}],"retained_markers":["failing_location","first_failure","tail","tail:1:35cf3d46823685e9"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_78dfd578c8672422","target_id":"viden-context-native:native-v1:Diagnostic","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Transcript,
                b"User: constraint: keep scope\nAssistant: old\nUser: decision: native\nUser: unresolved question: retry?\nAssistant: recent\n",
                ReductionPolicy::default(),
                r#"{"content":"User: constraint: keep scope\nUser: decision: native\nUser: unresolved question: retry?\nAssistant: old\nAssistant: recent","original":{"byte_count":119,"token_count":30},"reduced":{"byte_count":118,"token_count":30},"omissions":[],"retained_markers":["constraint","decision","question","recent_turn","recent_turn:1:6fee5befdca71649"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_1931e50bf52c15ee","target_id":"viden-context-native:native-v1:Transcript","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
            ),
            (
                ContextContentKind::Text,
                b"constraint: keep scope\ndecision: native\nunresolved question: retry?\nplain old\nplain recent\n",
                ReductionPolicy::default(),
                r#"{"content":"constraint: keep scope\ndecision: native\nunresolved question: retry?\nplain old\nplain recent","original":{"byte_count":91,"token_count":23},"reduced":{"byte_count":90,"token_count":23},"omissions":[],"retained_markers":["constraint","decision","question","recent_turn","recent_turn:1:01ab832cb9769a9b"],"reducer_id":"viden-context-native","reducer_version":"native-v1","quality":{"quality_id":"ctxq_6664ad4544b62071","target_id":"viden-context-native:native-v1:Text","passed":true,"score_microunits":1000000,"checks":[],"failure_reason":null,"checked_at":null},"fallback_raw":false}"#,
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
    fn token_metrics_are_not_redacted_across_text_routes() {
        let metrics = "total_tokens=11 max_output_tokens=12 token_count=13 tokens_per_second=14 input_tokens=15 output_tokens=16";
        let code_policy = ReductionPolicy {
            selected_line_ranges: vec![LineRange { start: 1, end: 1 }],
            ..ReductionPolicy::default()
        };
        let cases = [
            (
                ContextContentKind::Log,
                format!("$ report {metrics}\n"),
                ReductionPolicy::default(),
            ),
            (
                ContextContentKind::Diff,
                format!("diff --git a/a b/a\n@@ -1 +1 @@\n+{metrics}\n"),
                ReductionPolicy::default(),
            ),
            (
                ContextContentKind::Code,
                format!("let metrics = ({metrics});\n"),
                code_policy,
            ),
            (
                ContextContentKind::Text,
                format!("decision: metrics {metrics}\n"),
                ReductionPolicy::default(),
            ),
        ];

        for (kind, input, policy) in cases {
            let view = reduce(kind, input.as_bytes(), &policy).unwrap();

            for metric in metrics.split_ascii_whitespace() {
                assert!(view.content.contains(metric), "{kind:?}: {metric}");
            }
            assert!(!view.content.contains("[REDACTED]"), "{kind:?}");
        }
    }

    #[test]
    fn sensitive_assignments_redact_only_values_and_keep_suffix_context() {
        let code_policy = ReductionPolicy {
            selected_line_ranges: vec![LineRange { start: 1, end: 1 }],
            ..ReductionPolicy::default()
        };
        let cases = [
            (
                ContextContentKind::Log,
                "ERROR auth_token=log-secret at src/lib.rs:9 retryable\n".to_string(),
                ReductionPolicy::default(),
                "auth_token=[REDACTED] at src/lib.rs:9 retryable",
                "log-secret",
            ),
            (
                ContextContentKind::Diff,
                "diff --git a/.env b/.env\n@@ -1 +1 @@\n+api-token=diff-secret --retry=2\n"
                    .to_string(),
                ReductionPolicy::default(),
                "+api-token=[REDACTED] --retry=2",
                "diff-secret",
            ),
            (
                ContextContentKind::Code,
                "let api_token = \"code-secret\"; call();\n".to_string(),
                code_policy,
                "api_token = \"[REDACTED]\"; call();",
                "code-secret",
            ),
            (
                ContextContentKind::Text,
                "decision: credentials='text-secret' continue=yes\n".to_string(),
                ReductionPolicy::default(),
                "credentials='[REDACTED]' continue=yes",
                "text-secret",
            ),
            (
                ContextContentKind::Log,
                "ERROR Authorization: Bearer bearer-secret at src/auth.rs:4\n".to_string(),
                ReductionPolicy::default(),
                "Authorization: Bearer [REDACTED] at src/auth.rs:4",
                "bearer-secret",
            ),
        ];

        for (kind, input, policy, expected, raw_secret) in cases {
            let view = reduce(kind, input.as_bytes(), &policy).unwrap();
            let serialized = serde_json::to_string(&view).unwrap();

            assert!(
                view.content.contains(expected),
                "{kind:?}: {}",
                view.content
            );
            assert!(!serialized.contains(raw_secret), "{kind:?}");
        }
    }

    #[test]
    fn every_inline_authorization_and_bearer_credential_is_redacted_across_routes() {
        let code_policy = ReductionPolicy {
            selected_line_ranges: vec![LineRange { start: 1, end: 1 }],
            ..ReductionPolicy::default()
        };
        let cases = [
            (
                ContextContentKind::Log,
                "ERROR Authorization:Bearer log-auth, proxy bEaReR=log-loose); AUTHORIZATION=Basic log-basic; total_tokens=9 tail=kept dangling Bearer ; Authorization:\n"
                    .to_string(),
                ReductionPolicy::default(),
                ["log-auth", "log-loose", "log-basic"],
            ),
            (
                ContextContentKind::Text,
                "decision: Authorization:Bearer text-auth, proxy bEaReR=text-loose); AUTHORIZATION=Basic text-basic; total_tokens=9 tail=kept dangling Bearer ; Authorization:\n"
                    .to_string(),
                ReductionPolicy::default(),
                ["text-auth", "text-loose", "text-basic"],
            ),
            (
                ContextContentKind::Diff,
                "diff --git a/a b/a\n@@ -1 +1 @@\n+Authorization:Bearer diff-auth, proxy bEaReR=diff-loose); AUTHORIZATION=Basic diff-basic; total_tokens=9 tail=kept dangling Bearer ; Authorization:\n"
                    .to_string(),
                ReductionPolicy::default(),
                ["diff-auth", "diff-loose", "diff-basic"],
            ),
            (
                ContextContentKind::Code,
                "let message = \"Authorization:Bearer code-auth, proxy bEaReR=code-loose); AUTHORIZATION=Basic code-basic; total_tokens=9 tail=kept dangling Bearer ; Authorization:\";\n"
                    .to_string(),
                code_policy,
                ["code-auth", "code-loose", "code-basic"],
            ),
        ];

        for (kind, input, policy, raw_credentials) in cases {
            let view = reduce(kind, input.as_bytes(), &policy).unwrap();
            let serialized = serde_json::to_string(&view).unwrap();

            assert_eq!(
                view.content.matches("[REDACTED]").count(),
                3,
                "{kind:?}: {}",
                view.content
            );
            assert!(view.content.contains("[REDACTED],"), "{kind:?}");
            assert!(view.content.contains("[REDACTED]);"), "{kind:?}");
            assert!(view.content.contains("Basic [REDACTED];"), "{kind:?}");
            assert!(view.content.contains("total_tokens=9"), "{kind:?}");
            assert!(view.content.contains("tail=kept"), "{kind:?}");
            assert!(
                view.content.contains("dangling Bearer ; Authorization:"),
                "{kind:?}"
            );
            for credential in raw_credentials {
                assert!(!serialized.contains(credential), "{kind:?}: {credential}");
            }
        }
    }

    #[test]
    fn malformed_json_fallback_redacts_every_inline_credential() {
        let input = b"{broken: \"Authorization:Bearer json-auth, proxy bEaReR=json-loose); AUTHORIZATION=Basic json-basic; total_tokens=9 tail=kept dangling Bearer ; Authorization:\"";

        let view = reduce(ContextContentKind::Json, input, &ReductionPolicy::default()).unwrap();
        let serialized = serde_json::to_string(&view).unwrap();

        assert!(view.fallback_raw);
        assert_eq!(view.content.matches("[REDACTED]").count(), 3);
        assert!(view.content.contains("[REDACTED],"));
        assert!(view.content.contains("[REDACTED]);"));
        assert!(view.content.contains("Basic [REDACTED];"));
        assert!(view.content.contains("total_tokens=9"));
        assert!(view.content.contains("tail=kept"));
        assert!(view.content.contains("dangling Bearer ; Authorization:"));
        for credential in ["json-auth", "json-loose", "json-basic"] {
            assert!(!serialized.contains(credential), "{credential}");
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
                "$ cargo test -p demo --token=[REDACTED]\n\
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
                    "tail:1:1a8c37b26b0d1f12",
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
            vec!["command", "exit_status", "tail", "tail:1:3547cb112ac4489a"]
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
            vec![
                "command",
                "first_failure",
                "tail",
                "tail:1:361e48d0308f20e3"
            ]
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
    fn tail_marker_tracks_the_intended_final_log_line_after_bounding() {
        let input = b"ERROR first failure that remains\nmiddle\nACTUAL FINAL LINE\n";
        let full = reduce(ContextContentKind::Log, input, &ReductionPolicy::default()).unwrap();
        assert!(full.retained_markers.iter().any(|marker| marker == "tail"));
        assert!(
            full.retained_markers
                .iter()
                .any(|marker| marker.starts_with("tail:"))
        );

        let bounded_policy = ReductionPolicy {
            max_output_bytes: 24,
            ..ReductionPolicy::default()
        };
        let bounded = reduce(ContextContentKind::Log, input, &bounded_policy).unwrap();
        assert!(
            !bounded
                .retained_markers
                .iter()
                .any(|marker| marker == "tail")
        );
        assert!(
            !bounded
                .retained_markers
                .iter()
                .any(|marker| marker.starts_with("tail:"))
        );

        let required_policy = ReductionPolicy {
            required_markers: vec!["tail".to_string()],
            ..bounded_policy
        };
        assert!(matches!(
            reduce(ContextContentKind::Log, input, &required_policy),
            Err(crate::ContextError::QualityFailed { .. })
        ));
    }

    #[test]
    fn recent_turn_marker_tracks_the_intended_final_turn_after_bounding() {
        let input = b"constraint: preserve scope\nAssistant: older\nUser: ACTUAL RECENT TURN\n";
        let full = reduce(
            ContextContentKind::Transcript,
            input,
            &ReductionPolicy::default(),
        )
        .unwrap();
        assert!(
            full.retained_markers
                .iter()
                .any(|marker| marker == "recent_turn")
        );
        assert!(
            full.retained_markers
                .iter()
                .any(|marker| marker.starts_with("recent_turn:"))
        );

        let bounded_policy = ReductionPolicy {
            max_output_bytes: 28,
            ..ReductionPolicy::default()
        };
        let bounded = reduce(ContextContentKind::Transcript, input, &bounded_policy).unwrap();
        assert!(
            !bounded
                .retained_markers
                .iter()
                .any(|marker| marker == "recent_turn")
        );
        assert!(
            !bounded
                .retained_markers
                .iter()
                .any(|marker| marker.starts_with("recent_turn:"))
        );

        let required_policy = ReductionPolicy {
            required_markers: vec!["recent_turn".to_string()],
            ..bounded_policy
        };
        assert!(matches!(
            reduce(ContextContentKind::Transcript, input, &required_policy),
            Err(crate::ContextError::QualityFailed { .. })
        ));
    }

    #[test]
    fn duplicate_final_log_occurrence_has_distinct_tail_provenance() {
        let input = b"ERROR SAME\nnoise one\nnoise two\nERROR SAME\n";
        let full = reduce(ContextContentKind::Log, input, &ReductionPolicy::default()).unwrap();
        assert_eq!(full.content.matches("ERROR SAME").count(), 2);
        assert!(
            full.retained_markers
                .iter()
                .any(|marker| marker.starts_with("tail:2:"))
        );

        let bounded_policy = ReductionPolicy {
            max_output_bytes: 20,
            ..ReductionPolicy::default()
        };
        let bounded = reduce(ContextContentKind::Log, input, &bounded_policy).unwrap();
        assert_eq!(bounded.content.matches("ERROR SAME").count(), 1);
        assert!(
            !bounded
                .retained_markers
                .iter()
                .any(|marker| marker == "tail")
        );

        let required_policy = ReductionPolicy {
            required_markers: vec!["tail".to_string()],
            ..bounded_policy
        };
        assert!(matches!(
            reduce(ContextContentKind::Log, input, &required_policy),
            Err(crate::ContextError::QualityFailed { .. })
        ));
    }

    #[test]
    fn duplicate_recent_turn_occurrence_has_distinct_provenance() {
        let input = b"constraint: SAME\nAssistant: filler\nconstraint: SAME\n";
        let policy = ReductionPolicy {
            recent_turns: 1,
            ..ReductionPolicy::default()
        };
        let full = reduce(ContextContentKind::Transcript, input, &policy).unwrap();
        assert_eq!(full.content.matches("constraint: SAME").count(), 2);
        assert!(
            full.retained_markers
                .iter()
                .any(|marker| marker.starts_with("recent_turn:2:"))
        );

        let bounded_policy = ReductionPolicy {
            max_output_bytes: 20,
            ..policy
        };
        let bounded = reduce(ContextContentKind::Transcript, input, &bounded_policy).unwrap();
        assert_eq!(bounded.content.matches("constraint: SAME").count(), 1);
        assert!(
            !bounded
                .retained_markers
                .iter()
                .any(|marker| marker == "recent_turn")
        );

        let required_policy = ReductionPolicy {
            required_markers: vec!["recent_turn".to_string()],
            ..bounded_policy
        };
        assert!(matches!(
            reduce(ContextContentKind::Transcript, input, &required_policy),
            Err(crate::ContextError::QualityFailed { .. })
        ));
    }

    #[test]
    fn recent_semantic_turn_is_not_duplicated_as_the_same_source_occurrence() {
        let input = b"Assistant: filler\nconstraint: final unique\n";
        let policy = ReductionPolicy {
            recent_turns: 1,
            ..ReductionPolicy::default()
        };

        let view = reduce(ContextContentKind::Transcript, input, &policy).unwrap();

        assert_eq!(view.content.matches("constraint: final unique").count(), 1);
        assert!(
            view.retained_markers
                .iter()
                .any(|marker| marker.starts_with("recent_turn:1:"))
        );
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
