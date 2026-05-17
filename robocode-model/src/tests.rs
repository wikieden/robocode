use super::*;
use crate::adapters::{builtin_default_api_base, builtin_provider_id};
use crate::fallback::parse_explicit_tool_call;
use crate::parse::{parse_anthropic_events, parse_openai_events};
use crate::render::build_openai_body;
use crate::transport::split_response_and_status;
use robocode_types::{Message, ModelRequest, PermissionMode, Role, ToolSpec};
use serde_json::Value;

mod config_tests;
mod fallback_tests;
mod parse_tests;
mod provider_host_tests;
mod registry_tests;
mod render_tests;
mod transport_tests;
