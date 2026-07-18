use super::*;
use crate::adapters::{builtin_default_api_base, builtin_provider_id};
use crate::fallback::parse_explicit_tool_call;
use crate::parse::{
    parse_anthropic_events, parse_anthropic_stream_events, parse_ollama_events,
    parse_openai_events, parse_openai_stream_events,
};
use crate::render::{
    OpenAiRenderCompatibility, build_anthropic_body_with_stream, build_openai_body_with_stream,
    build_openai_body_with_stream_and_compat,
};
use crate::transport::split_response_and_status;
use serde_json::Value;
use viden_types::{
    Message, ModelRequest, PermissionLevel, PermissionMode, Role, ToolSpec, WorkMode,
};

mod config_tests;
mod fallback_tests;
mod parse_tests;
mod provider_host_tests;
mod provider_plugin_fixtures;
mod registry_tests;
mod render_tests;
mod transport_tests;
