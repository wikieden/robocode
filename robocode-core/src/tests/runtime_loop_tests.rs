use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::{EngineEvent, SessionEngine};
use robocode_lsp::{LspRuntime, LspServerConfig, LspServerRegistry};
use robocode_model::{ModelProvider, ModelRequestControl};
use robocode_types::{
    ApprovalResponse, ModelEvent, ModelRequest, PermissionMode, ToolCall, ToolInput,
};

use super::{SequenceProvider, temp_dir};

#[test]
fn single_turn_text_response_is_recorded() {
    let home = temp_dir("single_home");
    let cwd = temp_dir("single_cwd");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "hello from test".to_string(),
        },
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let events = engine
        .process_input_with_approval("hi", &mut approver)
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::Assistant(text) if text.contains("hello")))
    );
}

#[test]
fn cancelled_model_request_stops_before_provider_turn() {
    let home = temp_dir("cancel_home");
    let cwd = temp_dir("cancel_cwd");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "should not be observed".to_string(),
        },
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let control = ModelRequestControl::new();
    control.cancel();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let err = engine
        .process_input_with_approval_and_control("hi", &mut approver, &control)
        .unwrap_err();

    assert_eq!(err, "Model request cancelled");
}

#[test]
fn tool_loop_executes_and_reinjects_result() {
    let home = temp_dir("tool_home");
    let cwd = temp_dir("tool_cwd");
    fs::write(cwd.join("sample.txt"), "hello").unwrap();
    let mut read_input = ToolInput::new();
    read_input.insert("path".to_string(), "sample.txt".to_string());
    let provider = Box::new(SequenceProvider::new(vec![
        vec![ModelEvent::ToolCall(ToolCall {
            id: "tool_read".to_string(),
            name: "read_file".to_string(),
            input: read_input,
        })],
        vec![ModelEvent::AssistantText {
            content: "Tool finished".to_string(),
        }],
    ]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let events = engine
        .process_input_with_approval("read it", &mut approver)
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::ToolResult(text) if text.contains("hello")))
    );
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Assistant(text) if text.contains("Tool finished"))
    ));
}

#[test]
fn post_edit_lsp_diagnostics_are_reinjected_after_file_writes() {
    let home = temp_dir("post_edit_lsp_home");
    let cwd = temp_dir("post_edit_lsp_cwd");
    let fake_lsp_dir = temp_dir("post_edit_lsp_server");
    let mut write_input = ToolInput::new();
    write_input.insert("path".to_string(), "src/main.rs".to_string());
    write_input.insert(
        "content".to_string(),
        "fn main() {\n    let value = missing;\n}\n".to_string(),
    );
    let provider = Box::new(SequenceProvider::new(vec![
        vec![ModelEvent::ToolCall(ToolCall {
            id: "tool_write".to_string(),
            name: "write_file".to_string(),
            input: write_input,
        })],
        vec![ModelEvent::AssistantText {
            content: "Saw diagnostics".to_string(),
        }],
    ]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.lsp_runtime = Arc::new(LspRuntime::new(fake_lsp_registry(&fake_lsp_dir)));
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let events = engine
        .process_input_with_approval("write broken rust", &mut approver)
        .unwrap();

    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::ToolResult(text) if text.contains("src/main.rs"))
    ));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::System(text)
            if text.contains("Post-edit LSP diagnostics after `write_file`")
                && text.contains("LSP diagnostics:")
                && text.contains("fake-lsp/E100")
                && text.contains("fake diagnostic")))
    );
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Assistant(text) if text.contains("Saw diagnostics"))
    ));
}

#[test]
fn plan_mode_blocks_mutating_tools() {
    let home = temp_dir("plan_home");
    let cwd = temp_dir("plan_cwd");
    let mut write_input = ToolInput::new();
    write_input.insert("path".to_string(), "a.txt".to_string());
    write_input.insert("content".to_string(), "new".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::ToolCall(
        ToolCall {
            id: "tool_write".to_string(),
            name: "write_file".to_string(),
            input: write_input,
        },
    )]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .process_input_with_approval("/plan on", &mut approver)
        .unwrap();
    assert_eq!(engine.mode(), PermissionMode::Plan);
    let events = engine
        .process_input_with_approval("write a file", &mut approver)
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::System(text)
            if text.contains("Permission decision:")
                && text.contains("Summary: decision=deny")
                && text.contains("tool: write_file")
                && text.contains("reason: PlanMode")
                && text.contains("message: write_file is blocked while plan mode is active")))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::ToolResult(text)
            if text.contains("Permission decision:")
                && text.contains("decision=deny")
                && text.contains("tool: write_file")))
    );
}

#[test]
fn denied_tool_calls_are_followed_by_tool_result_messages() {
    let home = temp_dir("deny_tool_result_home");
    let cwd = temp_dir("deny_tool_result_cwd");
    let mut write_input = ToolInput::new();
    write_input.insert("path".to_string(), "a.txt".to_string());
    write_input.insert("content".to_string(), "new".to_string());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingSequenceProvider::new(
        vec![
            vec![ModelEvent::ToolCall(ToolCall {
                id: "tool_write".to_string(),
                name: "write_file".to_string(),
                input: write_input,
            })],
            vec![ModelEvent::AssistantText {
                content: "Denied safely".to_string(),
            }],
        ],
        Arc::clone(&requests),
    ));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .process_input_with_approval("/plan on", &mut approver)
        .unwrap();

    let events = engine
        .process_input_with_approval("write a file", &mut approver)
        .unwrap();

    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Assistant(text) if text.contains("Denied safely"))
    ));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let replay = &requests[1].messages;
    let tool_call_index = replay
        .iter()
        .position(|message| {
            message.tool_call_id.as_deref() == Some("tool_write")
                && message.role == robocode_types::Role::Assistant
        })
        .expect("assistant tool call message");
    let tool_result_index = replay
        .iter()
        .position(|message| {
            message.tool_call_id.as_deref() == Some("tool_write")
                && message.role == robocode_types::Role::Tool
        })
        .expect("tool result message");
    assert_eq!(tool_result_index, tool_call_index + 1);
}

struct RecordingSequenceProvider {
    model: String,
    turns: std::collections::VecDeque<Vec<ModelEvent>>,
    requests: Arc<Mutex<Vec<ModelRequest>>>,
}

impl RecordingSequenceProvider {
    fn new(turns: Vec<Vec<ModelEvent>>, requests: Arc<Mutex<Vec<ModelRequest>>>) -> Self {
        Self {
            model: "test-model".to_string(),
            turns: turns.into(),
            requests,
        }
    }
}

impl ModelProvider for RecordingSequenceProvider {
    fn provider_name(&self) -> &str {
        "recording-sequence"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: String) {
        self.model = model;
    }

    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(self
            .turns
            .pop_front()
            .unwrap_or_else(|| vec![ModelEvent::Done]))
    }
}

fn fake_lsp_registry(workdir: &Path) -> LspServerRegistry {
    let script_path = workdir.join("fake_lsp_server.py");
    fs::write(
        &script_path,
        r#"import json
import sys

def read_message():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line == b"\r\n":
            break
        key, value = line.decode("utf-8").split(":", 1)
        headers[key.lower()] = value.strip()
    body = sys.stdin.buffer.read(int(headers["content-length"]))
    return json.loads(body.decode("utf-8"))

def send(payload):
    body = json.dumps(payload).encode("utf-8")
    sys.stdout.buffer.write(f"Content-Length: {len(body)}\r\n\r\n".encode("utf-8"))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

while True:
    message = read_message()
    if message is None:
        break
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"capabilities": {}}})
    elif method == "initialized":
        continue
    elif method == "textDocument/didOpen":
        send({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": message["params"]["textDocument"]["uri"],
                "diagnostics": [{
                    "range": {
                        "start": {"line": 1, "character": 16},
                        "end": {"line": 1, "character": 23}
                    },
                    "severity": 1,
                    "source": "fake-lsp",
                    "code": "E100",
                    "message": "fake diagnostic"
                }]
            }
        })
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": message["id"], "result": None})
    elif method == "exit":
        break
"#,
    )
    .unwrap();

    LspServerRegistry::new(vec![LspServerConfig {
        id: "fake-rust".to_string(),
        command: std::env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string()),
        args: vec![script_path.to_string_lossy().to_string()],
        file_extensions: vec!["rs".to_string()],
    }])
}
