use robocode_types::{Message, ModelEvent, Role, ToolCall, fresh_id, parse_tool_input};

pub(crate) fn fallback_events(
    provider_name: &str,
    model: &str,
    last_message: Option<&Message>,
) -> Vec<ModelEvent> {
    let Some(last_message) = last_message else {
        return vec![ModelEvent::AssistantText {
            content: "RoboCode is ready.".to_string(),
        }];
    };

    if last_message.role == Role::Tool {
        return vec![
            ModelEvent::AssistantText {
                content: format!(
                    "Tool `{}` completed.\n\n{}",
                    last_message
                        .tool_name
                        .clone()
                        .unwrap_or_else(|| "tool".to_string()),
                    last_message.content
                ),
            },
            ModelEvent::Done,
        ];
    }

    if last_message.role != Role::User {
        return vec![ModelEvent::Done];
    }

    if let Some(tool_call) = parse_explicit_tool_call(&last_message.content) {
        return vec![ModelEvent::ToolCall(tool_call), ModelEvent::Done];
    }

    vec![
        ModelEvent::AssistantText {
            content: format!(
                "{} provider is running in local fallback mode for model `{}`.\n\n\
Use `tool <name> key=value ...` to force a tool call, or configure API credentials.\n\n\
You said:\n{}",
                provider_name, model, last_message.content
            ),
        },
        ModelEvent::Done,
    ]
}

pub(crate) fn parse_explicit_tool_call_from_messages(messages: &[Message]) -> Option<ToolCall> {
    messages.last().and_then(|message| {
        if message.role == Role::User {
            parse_explicit_tool_call(&message.content)
        } else {
            None
        }
    })
}

pub(crate) fn parse_explicit_tool_call(input: &str) -> Option<ToolCall> {
    let trimmed = input.trim();
    let prefixes = ["tool ", "use "];
    for prefix in prefixes {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let mut parts = rest.splitn(2, ' ');
            let name = parts.next()?.trim();
            let payload = parts.next().unwrap_or("").trim();
            return Some(ToolCall {
                id: fresh_id("tool"),
                name: name.to_string(),
                input: parse_tool_input(payload),
            });
        }
    }
    None
}
