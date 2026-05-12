use robocode_types::{ModelEvent, ToolCall, ToolInput, fresh_id, parse_tool_input};
use serde_json::Value;

use crate::PROVIDER_REASONING_CONTENT_KEY;

pub(crate) fn parse_anthropic_events(response: &str) -> Option<Vec<ModelEvent>> {
    let value: Value = serde_json::from_str(response).ok()?;
    let content_blocks = value.get("content")?.as_array()?;
    let mut tool_calls = Vec::new();
    let mut text_parts = Vec::new();
    for block in content_blocks {
        match block.get("type")?.as_str()? {
            "tool_use" => {
                let name = block.get("name")?.as_str()?.to_string();
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| fresh_id("tool"));
                let input = json_value_to_tool_input(block.get("input").unwrap_or(&Value::Null));
                tool_calls.push(ModelEvent::ToolCall(ToolCall { id, name, input }));
            }
            "text" => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        text_parts.push(text.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    if !tool_calls.is_empty() {
        Some(tool_calls)
    } else if text_parts.is_empty() {
        None
    } else {
        Some(vec![ModelEvent::AssistantText {
            content: text_parts.join("\n\n"),
        }])
    }
}

pub(crate) fn parse_openai_events(response: &str) -> Option<Vec<ModelEvent>> {
    let value: Value = serde_json::from_str(response).ok()?;
    let message = value.get("choices")?.as_array()?.first()?.get("message")?;
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        let events: Vec<ModelEvent> = tool_calls
            .iter()
            .filter_map(|tool_call| {
                let function = tool_call.get("function")?;
                let name = function.get("name")?.as_str()?.to_string();
                let id = tool_call
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| fresh_id("tool"));
                let arguments = function
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                let mut input = parse_json_tool_arguments(arguments);
                if let Some(reasoning_content) =
                    message.get("reasoning_content").and_then(Value::as_str)
                {
                    input.insert(
                        PROVIDER_REASONING_CONTENT_KEY.to_string(),
                        reasoning_content.to_string(),
                    );
                }
                Some(ModelEvent::ToolCall(ToolCall { id, name, input }))
            })
            .collect();
        if !events.is_empty() {
            return Some(events);
        }
    }
    extract_openai_content(message).map(|content| vec![ModelEvent::AssistantText { content }])
}

pub(crate) fn parse_ollama_events(response: &str) -> Option<Vec<ModelEvent>> {
    let value: Value = serde_json::from_str(response).ok()?;
    if let Some(message) = value.get("message") {
        if let Some(content) = message.get("content").and_then(Value::as_str) {
            if !content.trim().is_empty() {
                return Some(vec![ModelEvent::AssistantText {
                    content: content.to_string(),
                }]);
            }
        }
    }
    if let Some(content) = value.get("response").and_then(Value::as_str) {
        if !content.trim().is_empty() {
            return Some(vec![ModelEvent::AssistantText {
                content: content.to_string(),
            }]);
        }
    }
    None
}

fn extract_openai_content(message: &Value) -> Option<String> {
    if let Some(content) = message.get("content").and_then(Value::as_str) {
        if !content.trim().is_empty() {
            return Some(content.to_string());
        }
    }
    if let Some(parts) = message.get("content").and_then(Value::as_array) {
        let text = parts
            .iter()
            .filter_map(|part| {
                if part.get("type").and_then(Value::as_str) == Some("text") {
                    part.get("text")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

fn parse_json_tool_arguments(arguments: &str) -> ToolInput {
    if let Ok(value) = serde_json::from_str::<Value>(arguments) {
        json_value_to_tool_input(&value)
    } else {
        parse_tool_input(arguments)
    }
}

fn json_value_to_tool_input(value: &Value) -> ToolInput {
    let mut input = ToolInput::new();
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            input.insert(key.clone(), json_value_to_string(value));
        }
    }
    input
}

fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(string) => string.clone(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| String::new())
        }
    }
}

pub(crate) fn extract_error_message(response: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(response) {
        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message").or_else(|| error.get("error")))
            .and_then(Value::as_str)
        {
            return Some(message.to_string());
        }
        if let Some(message) = value.get("message").and_then(Value::as_str) {
            return Some(message.to_string());
        }
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return Some(error.to_string());
        }
    }
    extract_string_after(response, "\"message\":\"")
        .or_else(|| extract_string_after(response, "\"error\":\""))
}

fn extract_string_after(input: &str, marker: &str) -> Option<String> {
    let start = input.find(marker)? + marker.len();
    let bytes = input.as_bytes();
    let mut index = start;
    let mut escaped = false;
    let mut output = String::new();
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if escaped {
            output.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(output);
        } else {
            output.push(ch);
        }
        index += 1;
    }
    None
}
