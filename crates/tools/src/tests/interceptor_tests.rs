use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use viden_types::{ToolCall, ToolInput, ToolResult, ToolSpec};

use super::*;
use crate::{FilesystemCapability, InterceptVerdict, ToolExecutionInterceptor};

/// Shared in-memory filesystem so tests can prove whether a tool actually ran.
#[derive(Default)]
struct MemoryFilesystem {
    files: Mutex<BTreeMap<PathBuf, String>>,
}

impl FilesystemCapability for MemoryFilesystem {
    fn is_dir(&self, _path: &Path) -> bool {
        false
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>, String> {
        self.read_to_string(path).map(String::into_bytes)
    }

    fn read_to_string(&self, path: &Path) -> Result<String, String> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| format!("memory fs has no file at {}", path.display()))
    }

    fn create_dir_all(&self, _path: &Path) -> Result<(), String> {
        Ok(())
    }

    fn write(&self, path: &Path, contents: &str) -> Result<(), String> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), contents.to_string());
        Ok(())
    }
}

/// Records every hook invocation into a shared log to assert ordering.
struct RecordingInterceptor {
    label: &'static str,
    log: Arc<Mutex<Vec<String>>>,
    verdict: fn(&'static str) -> InterceptVerdict,
}

impl RecordingInterceptor {
    fn proceeding(label: &'static str, log: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            label,
            log,
            verdict: |_| InterceptVerdict::Proceed,
        }
    }

    fn rejecting(label: &'static str, log: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            label,
            log,
            verdict: |label| InterceptVerdict::Reject {
                message: format!("{label} rejected the call"),
            },
        }
    }
}

impl ToolExecutionInterceptor for RecordingInterceptor {
    fn before_execute(
        &self,
        spec: &ToolSpec,
        call: &ToolCall,
        _ctx: &ToolExecutionContext,
    ) -> InterceptVerdict {
        self.log
            .lock()
            .unwrap()
            .push(format!("before:{}:{}:{}", self.label, spec.name, call.name));
        (self.verdict)(self.label)
    }

    fn after_execute(&self, call: &ToolCall, result: &ToolResult) {
        self.log.lock().unwrap().push(format!(
            "after:{}:{}:{}",
            self.label, call.name, result.success
        ));
    }
}

fn write_call(path: &str, content: &str) -> ToolCall {
    let mut input = ToolInput::new();
    input.insert("path".into(), path.into());
    input.insert("content".into(), content.into());
    ToolCall {
        id: "tool_write".into(),
        name: "write_file".into(),
        input,
    }
}

fn memory_ctx(fs: Arc<MemoryFilesystem>) -> ToolExecutionContext {
    let mut ctx = ToolExecutionContext::local(PathBuf::from("/virtual/workspace"));
    ctx.fs = fs;
    ctx
}

#[test]
fn interceptors_run_before_in_order_and_after_in_reverse() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let fs = Arc::new(MemoryFilesystem::default());
    let mut registry = ToolRegistry::builtin();
    registry.register_interceptor(Arc::new(RecordingInterceptor::proceeding(
        "first",
        log.clone(),
    )));
    registry.register_interceptor(Arc::new(RecordingInterceptor::proceeding(
        "second",
        log.clone(),
    )));

    let result = registry
        .execute(&write_call("notes.txt", "content"), &memory_ctx(fs.clone()))
        .unwrap();

    assert!(result.success);
    assert_eq!(
        fs.files.lock().unwrap().len(),
        1,
        "the tool must run when every interceptor proceeds"
    );
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            "before:first:write_file:write_file".to_string(),
            "before:second:write_file:write_file".to_string(),
            "after:second:write_file:true".to_string(),
            "after:first:write_file:true".to_string(),
        ]
    );
}

#[test]
fn first_reject_short_circuits_and_the_tool_does_not_run() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let fs = Arc::new(MemoryFilesystem::default());
    let mut registry = ToolRegistry::builtin();
    registry.register_interceptor(Arc::new(RecordingInterceptor::rejecting(
        "guard",
        log.clone(),
    )));
    registry.register_interceptor(Arc::new(RecordingInterceptor::proceeding(
        "later",
        log.clone(),
    )));

    let error = registry
        .execute(&write_call("notes.txt", "content"), &memory_ctx(fs.clone()))
        .unwrap_err();

    assert_eq!(error, "guard rejected the call");
    assert!(
        fs.files.lock().unwrap().is_empty(),
        "a rejected call must not reach the tool"
    );
    // The rejecting interceptor short-circuits: later interceptors never run
    // and no after_execute hooks fire because no result was produced.
    assert_eq!(
        *log.lock().unwrap(),
        vec!["before:guard:write_file:write_file".to_string()]
    );
}

#[test]
fn registry_without_interceptors_executes_directly() {
    let fs = Arc::new(MemoryFilesystem::default());
    let registry = ToolRegistry::builtin();
    let result = registry
        .execute(&write_call("notes.txt", "content"), &memory_ctx(fs.clone()))
        .unwrap();
    assert!(result.success);
    assert_eq!(fs.files.lock().unwrap().len(), 1);
}
