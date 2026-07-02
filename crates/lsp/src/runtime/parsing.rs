use std::path::Path;

use serde_json::Value;

use viden_types::{LspDiagnostic, LspLocation, LspPosition, LspRange, LspSymbol};

pub(super) fn language_id_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "rust",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("js") => "javascript",
        Some("jsx") => "javascriptreact",
        Some("py") => "python",
        _ => "plaintext",
    }
}

pub(super) fn file_uri(path: &Path) -> Result<String, String> {
    let absolute = path.canonicalize().map_err(|err| err.to_string())?;
    let rendered = absolute.to_string_lossy().replace(' ', "%20");
    #[cfg(windows)]
    {
        Ok(format!("file:///{}", rendered.replace('\\', "/")))
    }
    #[cfg(not(windows))]
    {
        Ok(format!("file://{rendered}"))
    }
}

pub(super) fn parse_diagnostics(
    value: &Value,
    file_uri: &str,
) -> Result<Vec<LspDiagnostic>, String> {
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    let path = uri_to_path_string(file_uri);
    let mut diagnostics = Vec::new();
    for item in items {
        diagnostics.push(LspDiagnostic {
            path: path.clone(),
            range: parse_range(item.get("range").unwrap_or(&Value::Null))?,
            severity: item
                .get("severity")
                .and_then(Value::as_u64)
                .map(|severity| severity as u8),
            source: item
                .get("source")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            code: parse_optional_code(item.get("code")),
            message: item
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    Ok(diagnostics)
}

pub(super) fn parse_symbol_response(
    response: &Value,
    file_uri: &str,
) -> Result<Vec<LspSymbol>, String> {
    parse_symbols(
        response.get("result").unwrap_or(&Value::Null),
        &uri_to_path_string(file_uri),
        None,
    )
}

fn parse_symbols(
    value: &Value,
    path: &str,
    container_name: Option<String>,
) -> Result<Vec<LspSymbol>, String> {
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    let mut symbols = Vec::new();
    for item in items {
        if let Some(location) = item.get("location") {
            symbols.push(LspSymbol {
                name: item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                kind: item.get("kind").and_then(Value::as_u64).unwrap_or(0) as u32,
                path: uri_to_path_string(
                    location.get("uri").and_then(Value::as_str).unwrap_or(path),
                ),
                range: parse_range(location.get("range").unwrap_or(&Value::Null))?,
                selection_range: None,
                container_name: item
                    .get("containerName")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            });
            continue;
        }

        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let selection_range = item.get("selectionRange").map(parse_range).transpose()?;
        symbols.push(LspSymbol {
            name: name.clone(),
            kind: item.get("kind").and_then(Value::as_u64).unwrap_or(0) as u32,
            path: path.to_string(),
            range: parse_range(item.get("range").unwrap_or(&Value::Null))?,
            selection_range,
            container_name: container_name.clone(),
        });
        if let Some(children) = item.get("children") {
            symbols.extend(parse_symbols(children, path, Some(name))?);
        }
    }
    Ok(symbols)
}

pub(super) fn parse_locations(
    value: &Value,
    fallback_uri: &str,
) -> Result<Vec<LspLocation>, String> {
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    let mut locations = Vec::new();
    for item in items {
        let uri = item
            .get("uri")
            .and_then(Value::as_str)
            .or_else(|| item.get("targetUri").and_then(Value::as_str))
            .unwrap_or(fallback_uri);
        let range_value = item
            .get("range")
            .or_else(|| item.get("targetSelectionRange"))
            .or_else(|| item.get("targetRange"))
            .unwrap_or(&Value::Null);
        locations.push(LspLocation {
            path: uri_to_path_string(uri),
            range: parse_range(range_value)?,
        });
    }
    locations.sort_by(|left, right| {
        (
            left.path.as_str(),
            left.range.start.line,
            left.range.start.character,
        )
            .cmp(&(
                right.path.as_str(),
                right.range.start.line,
                right.range.start.character,
            ))
    });
    locations.dedup_by(|left, right| {
        left.path == right.path
            && left.range.start.line == right.range.start.line
            && left.range.start.character == right.range.start.character
            && left.range.end.line == right.range.end.line
            && left.range.end.character == right.range.end.character
    });
    Ok(locations)
}

fn parse_range(value: &Value) -> Result<LspRange, String> {
    Ok(LspRange {
        start: parse_position(value.get("start").unwrap_or(&Value::Null))?,
        end: parse_position(value.get("end").unwrap_or(&Value::Null))?,
    })
}

fn parse_position(value: &Value) -> Result<LspPosition, String> {
    Ok(LspPosition {
        line: value
            .get("line")
            .and_then(Value::as_u64)
            .ok_or_else(|| "Missing LSP position line".to_string())? as u32,
        character: value
            .get("character")
            .and_then(Value::as_u64)
            .ok_or_else(|| "Missing LSP position character".to_string())? as u32,
    })
}

fn parse_optional_code(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

fn uri_to_path_string(uri: &str) -> String {
    uri.strip_prefix("file://")
        .unwrap_or(uri)
        .replace("%20", " ")
}
