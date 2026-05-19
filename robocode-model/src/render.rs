use robocode_types::{
    Message, ModelRequest, Role, ToolInput, ToolSpec, decode_tool_input, fresh_id,
};
use serde_json::{Map, Value, json};

use crate::PROVIDER_REASONING_CONTENT_KEY;

pub(crate) fn build_anthropic_body_with_stream(
    model: &str,
    request: &ModelRequest,
    stream: bool,
) -> String {
    let mut payload = json!({
        "model": model,
        "max_tokens": 2048,
        "system": provider_system_prompt(),
        "messages": render_anthropic_messages(&request.messages),
    });
    if stream {
        payload["stream"] = Value::Bool(true);
    }
    if !request.tools.is_empty() {
        payload["tools"] = Value::Array(render_anthropic_tools(&request.tools));
    }
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

pub(crate) fn build_openai_body_with_stream(
    model: &str,
    request: &ModelRequest,
    stream: bool,
) -> String {
    let mut messages = vec![json!({
        "role": "system",
        "content": provider_system_prompt(),
    })];
    messages.extend(render_openai_messages(&request.messages));
    let mut payload = json!({
        "model": model,
        "messages": messages,
        "temperature": 0.2,
    });
    if stream {
        payload["stream"] = Value::Bool(true);
    }
    if !request.tools.is_empty() {
        payload["tools"] = Value::Array(render_openai_tools(&request.tools));
    }
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

pub(crate) fn build_ollama_body(model: &str, request: &ModelRequest) -> String {
    let mut messages = vec![json!({
        "role": "system",
        "content": provider_system_prompt(),
    })];
    messages.extend(render_simple_messages(&request.messages));
    let payload = json!({
        "model": model,
        "stream": false,
        "messages": messages,
    });
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

fn render_anthropic_messages(messages: &[Message]) -> Vec<Value> {
    let mut rendered = Vec::new();
    for message in messages {
        if message.role == Role::Tool {
            rendered.push(json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": message.tool_call_id.clone().unwrap_or_else(|| fresh_id("tool")),
                    "content": message.content,
                }],
            }));
            continue;
        }
        if message.role == Role::Assistant
            && message.tool_name.is_some()
            && message.tool_call_id.is_some()
        {
            rendered.push(json!({
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": message.tool_call_id.clone().unwrap_or_else(|| fresh_id("tool")),
                    "name": message.tool_name.clone().unwrap_or_else(|| "tool".to_string()),
                    "input": tool_input_to_json(&decode_tool_input(&message.content)),
                }],
            }));
            continue;
        }
        rendered.push(json!({
            "role": match message.role {
                Role::Assistant => "assistant",
                Role::User => "user",
                Role::System | Role::Tool => "user",
            },
            "content": [{
                "type": "text",
                "text": normalized_message_content(message),
            }],
        }));
    }
    rendered
}

fn render_openai_messages(messages: &[Message]) -> Vec<Value> {
    let mut rendered = Vec::new();
    for message in messages {
        match message.role {
            Role::Tool => {
                rendered.push(json!({
                    "role": "tool",
                    "tool_call_id": message.tool_call_id.clone().unwrap_or_else(|| fresh_id("tool")),
                    "content": message.content,
                }));
            }
            Role::Assistant if message.tool_name.is_some() && message.tool_call_id.is_some() => {
                let mut input = decode_tool_input(&message.content);
                let reasoning_content = input.remove(PROVIDER_REASONING_CONTENT_KEY);
                let mut rendered_message = json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "tool_calls": [{
                        "id": message.tool_call_id.clone().unwrap_or_else(|| fresh_id("tool")),
                        "type": "function",
                        "function": {
                            "name": message.tool_name.clone().unwrap_or_else(|| "tool".to_string()),
                            "arguments": serde_json::to_string(&tool_input_to_json(&input))
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }],
                });
                if let Some(reasoning_content) = reasoning_content {
                    rendered_message["reasoning_content"] = Value::String(reasoning_content);
                }
                rendered.push(rendered_message);
            }
            Role::System => {
                rendered.push(json!({
                    "role": "user",
                    "content": normalized_message_content(message),
                }));
            }
            Role::Assistant | Role::User => {
                rendered.push(json!({
                    "role": match message.role {
                        Role::Assistant => "assistant",
                        _ => "user",
                    },
                    "content": normalized_message_content(message),
                }));
            }
        }
    }
    rendered
}

fn render_simple_messages(messages: &[Message]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            json!({
                "role": match message.role {
                    Role::Assistant => "assistant",
                    _ => "user",
                },
                "content": normalized_message_content(message),
            })
        })
        .collect()
}

fn normalized_message_content(message: &Message) -> String {
    match message.role {
        Role::Tool => format!(
            "[tool_result:{}]\n{}",
            message.tool_name.as_deref().unwrap_or("tool"),
            message.content
        ),
        Role::System => format!("[system]\n{}", message.content),
        _ => message.content.clone(),
    }
}

fn render_openai_tools(tools: &[ToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool_parameters_schema(tool),
                }
            })
        })
        .collect()
}

fn render_anthropic_tools(tools: &[ToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool_parameters_schema(tool),
            })
        })
        .collect()
}

fn tool_parameters_schema(tool: &ToolSpec) -> Value {
    let mut properties = Map::new();
    for key in extract_input_keys(&tool.input_schema_hint) {
        properties.insert(
            key.clone(),
            json!({
                "type": "string",
                "description": format!("Input field `{}` for {}", key, tool.name),
            }),
        );
    }
    json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": { "type": "string" },
    })
}

fn extract_input_keys(hint: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for segment in hint.split_whitespace() {
        let cleaned = segment.trim_matches(|char: char| char == ',' || char == ';');
        if let Some((key, _)) = cleaned.split_once('=') {
            let key = key
                .trim()
                .trim_matches(|char: char| char == '\'' || char == '"');
            if !key.is_empty()
                && key
                    .chars()
                    .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
                && !keys.iter().any(|existing| existing == key)
            {
                keys.push(key.to_string());
            }
        }
    }
    keys
}

fn tool_input_to_json(input: &ToolInput) -> Value {
    let mut object = Map::new();
    for (key, value) in input {
        object.insert(key.clone(), Value::String(value.clone()));
    }
    Value::Object(object)
}

fn provider_system_prompt() -> String {
    [
        "You are RoboCode, a coding assistant running in a terminal.",
        "When native tool calling is available, prefer the provided tool interface.",
        "If native tool calling is unavailable, respond with exactly one line in this format:",
        "tool <tool_name> key=value key=value",
        "Do not wrap tool calls in JSON or markdown fences.",
        "Available tools include shell, read_file, write_file, edit_file, glob, grep, and git helpers.",
        "If no tool is required, answer normally in plain text.",
    ]
    .join("\n")
}
