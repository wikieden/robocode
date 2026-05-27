use std::collections::BTreeMap;

use robocode_types::{ModelEvent, ModelUsage, ToolCall, ToolInput, fresh_id, parse_tool_input};
use serde_json::Value;

use crate::PROVIDER_REASONING_CONTENT_KEY;

pub(crate) fn parse_anthropic_events(response: &str) -> Option<Vec<ModelEvent>> {
    let value: Value = serde_json::from_str(response).ok()?;
    let usage = parse_anthropic_usage(&value);
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
                if let Some(text) = block.get("text").and_then(Value::as_str)
                    && !text.trim().is_empty()
                {
                    text_parts.push(text.to_string());
                }
            }
            _ => {}
        }
    }
    if !tool_calls.is_empty() {
        Some(with_usage(tool_calls, usage))
    } else if text_parts.is_empty() {
        None
    } else {
        Some(with_usage(
            vec![ModelEvent::AssistantText {
                content: text_parts.join("\n\n"),
            }],
            usage,
        ))
    }
}

pub(crate) fn parse_openai_events(response: &str) -> Option<Vec<ModelEvent>> {
    let value: Value = serde_json::from_str(response).ok()?;
    let usage = parse_openai_usage(&value);
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
            return Some(with_usage(events, usage));
        }
    }
    extract_openai_content(message)
        .map(|content| with_usage(vec![ModelEvent::AssistantText { content }], usage))
}

pub(crate) fn parse_openai_stream_events(response: &str) -> Option<Vec<ModelEvent>> {
    let mut text_parts = Vec::new();
    let mut reasoning_parts = Vec::new();
    let mut tool_calls = BTreeMap::<usize, OpenAiStreamToolCall>::new();
    let mut usage = None;

    for payload in sse_payloads(response) {
        if payload == "[DONE]" {
            break;
        }
        let value: Value = serde_json::from_str(payload).ok()?;
        usage = merge_usage(usage, parse_openai_usage(&value));
        let delta = value.get("choices")?.as_array()?.first()?.get("delta")?;
        if let Some(content) = delta.get("content").and_then(Value::as_str)
            && !content.is_empty()
        {
            text_parts.push(content.to_string());
        }
        if let Some(reasoning) = delta.get("reasoning_content").and_then(Value::as_str)
            && !reasoning.is_empty()
        {
            reasoning_parts.push(reasoning.to_string());
        }
        if let Some(delta_tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tool_call in delta_tool_calls {
                let index = tool_call
                    .get("index")
                    .and_then(Value::as_u64)
                    .unwrap_or(tool_calls.len() as u64) as usize;
                let entry = tool_calls.entry(index).or_default();
                if let Some(id) = tool_call.get("id").and_then(Value::as_str) {
                    entry.id = Some(id.to_string());
                }
                if let Some(function) = tool_call.get("function") {
                    if let Some(name) = function.get("name").and_then(Value::as_str) {
                        entry.name = Some(name.to_string());
                    }
                    if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                        entry.arguments.push_str(arguments);
                    }
                }
            }
        }
    }

    let reasoning_content = joined_non_empty(&reasoning_parts);
    let calls = tool_calls
        .into_values()
        .filter_map(|call| call.into_tool_call(reasoning_content.as_deref()))
        .map(ModelEvent::ToolCall)
        .collect::<Vec<_>>();
    if !calls.is_empty() {
        Some(with_usage(calls, usage))
    } else {
        joined_non_empty(&text_parts)
            .map(|content| with_usage(vec![ModelEvent::AssistantText { content }], usage))
    }
}

pub(crate) fn parse_ollama_events(response: &str) -> Option<Vec<ModelEvent>> {
    let value: Value = serde_json::from_str(response).ok()?;
    let usage = parse_ollama_usage(&value);
    if let Some(message) = value.get("message")
        && let Some(content) = message.get("content").and_then(Value::as_str)
        && !content.trim().is_empty()
    {
        return Some(with_usage(
            vec![ModelEvent::AssistantText {
                content: content.to_string(),
            }],
            usage,
        ));
    }
    if let Some(content) = value.get("response").and_then(Value::as_str)
        && !content.trim().is_empty()
    {
        return Some(with_usage(
            vec![ModelEvent::AssistantText {
                content: content.to_string(),
            }],
            usage,
        ));
    }
    None
}

pub(crate) fn parse_anthropic_stream_events(response: &str) -> Option<Vec<ModelEvent>> {
    let mut text_blocks = BTreeMap::<usize, String>::new();
    let mut tool_blocks = BTreeMap::<usize, AnthropicStreamToolCall>::new();
    let mut usage = None;

    for payload in sse_payloads(response) {
        let value: Value = serde_json::from_str(payload).ok()?;
        usage = merge_usage(usage, parse_anthropic_usage(&value));
        match value.get("type").and_then(Value::as_str)? {
            "content_block_start" => {
                let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let block = value.get("content_block")?;
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        text_blocks.entry(index).or_default();
                    }
                    Some("tool_use") => {
                        tool_blocks.insert(
                            index,
                            AnthropicStreamToolCall {
                                id: block
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .map(ToString::to_string),
                                name: block
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .map(ToString::to_string),
                                input_json: String::new(),
                            },
                        );
                    }
                    _ => {}
                }
            }
            "content_block_delta" => {
                let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let delta = value.get("delta")?;
                match delta.get("type").and_then(Value::as_str) {
                    Some("text_delta") => {
                        if let Some(text) = delta.get("text").and_then(Value::as_str) {
                            text_blocks.entry(index).or_default().push_str(text);
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(partial_json) =
                            delta.get("partial_json").and_then(Value::as_str)
                        {
                            tool_blocks
                                .entry(index)
                                .or_default()
                                .input_json
                                .push_str(partial_json);
                        }
                    }
                    _ => {}
                }
            }
            "message_stop" => break,
            _ => {}
        }
    }

    let calls = tool_blocks
        .into_values()
        .filter_map(AnthropicStreamToolCall::into_tool_call)
        .map(ModelEvent::ToolCall)
        .collect::<Vec<_>>();
    if !calls.is_empty() {
        Some(with_usage(calls, usage))
    } else {
        let text_parts = text_blocks.into_values().collect::<Vec<_>>();
        joined_non_empty(&text_parts)
            .map(|content| with_usage(vec![ModelEvent::AssistantText { content }], usage))
    }
}

fn with_usage(mut events: Vec<ModelEvent>, usage: Option<ModelUsage>) -> Vec<ModelEvent> {
    if let Some(usage) = usage {
        events.push(ModelEvent::Usage(usage));
    }
    events
}

fn merge_usage(current: Option<ModelUsage>, next: Option<ModelUsage>) -> Option<ModelUsage> {
    match (current, next) {
        (None, usage) | (usage, None) => usage,
        (Some(current), Some(next)) => Some(ModelUsage {
            input_tokens: next.input_tokens.or(current.input_tokens),
            output_tokens: next.output_tokens.or(current.output_tokens),
            total_tokens: next.total_tokens.or(current.total_tokens),
            cost_micro_usd: next.cost_micro_usd.or(current.cost_micro_usd),
        }),
    }
}

fn parse_openai_usage(value: &Value) -> Option<ModelUsage> {
    let usage = value.get("usage")?;
    let input_tokens = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(Value::as_u64);
    let output_tokens = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(Value::as_u64);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            input_tokens
                .zip(output_tokens)
                .map(|(input, output)| input + output)
        });
    usage_from_parts(input_tokens, output_tokens, total_tokens)
}

fn parse_anthropic_usage(value: &Value) -> Option<ModelUsage> {
    let usage = value.get("usage")?;
    let input_tokens = usage.get("input_tokens").and_then(Value::as_u64);
    let output_tokens = usage.get("output_tokens").and_then(Value::as_u64);
    let total_tokens = input_tokens
        .zip(output_tokens)
        .map(|(input, output)| input + output);
    usage_from_parts(input_tokens, output_tokens, total_tokens)
}

fn parse_ollama_usage(value: &Value) -> Option<ModelUsage> {
    let input_tokens = value.get("prompt_eval_count").and_then(Value::as_u64);
    let output_tokens = value.get("eval_count").and_then(Value::as_u64);
    let total_tokens = input_tokens
        .zip(output_tokens)
        .map(|(input, output)| input + output);
    usage_from_parts(input_tokens, output_tokens, total_tokens)
}

fn usage_from_parts(
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
) -> Option<ModelUsage> {
    (input_tokens.is_some() || output_tokens.is_some() || total_tokens.is_some()).then_some(
        ModelUsage {
            input_tokens,
            output_tokens,
            total_tokens,
            cost_micro_usd: None,
        },
    )
}

#[derive(Default)]
struct OpenAiStreamToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl OpenAiStreamToolCall {
    fn into_tool_call(self, reasoning_content: Option<&str>) -> Option<ToolCall> {
        let name = self.name?;
        let mut input = parse_json_tool_arguments(&self.arguments);
        if let Some(reasoning_content) = reasoning_content {
            input.insert(
                PROVIDER_REASONING_CONTENT_KEY.to_string(),
                reasoning_content.to_string(),
            );
        }
        Some(ToolCall {
            id: self.id.unwrap_or_else(|| fresh_id("tool")),
            name,
            input,
        })
    }
}

#[derive(Default)]
struct AnthropicStreamToolCall {
    id: Option<String>,
    name: Option<String>,
    input_json: String,
}

impl AnthropicStreamToolCall {
    fn into_tool_call(self) -> Option<ToolCall> {
        Some(ToolCall {
            id: self.id.unwrap_or_else(|| fresh_id("tool")),
            name: self.name?,
            input: parse_json_tool_arguments(&self.input_json),
        })
    }
}

fn sse_payloads(response: &str) -> impl Iterator<Item = &str> {
    response.lines().filter_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix("data:")
            .map(str::trim)
            .filter(|payload| !payload.is_empty())
    })
}

fn joined_non_empty(parts: &[String]) -> Option<String> {
    let content = parts.join("");
    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}

fn extract_openai_content(message: &Value) -> Option<String> {
    if let Some(content) = message.get("content").and_then(Value::as_str)
        && !content.trim().is_empty()
    {
        return Some(content.to_string());
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
