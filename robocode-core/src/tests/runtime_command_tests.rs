use std::cell::Cell;

use crate::{DependencyStatus, DoctorReport, EngineEvent, SessionEngine};
use robocode_model::ProviderHost;
use robocode_types::ApprovalResponse;

use super::{SequenceProvider, temp_dir};

#[test]
fn web_help_command_is_available() {
    let home = temp_dir("web_help_home");
    let cwd = temp_dir("web_help_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/web", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains("Web commands:")
    )));
}

#[test]
fn status_command_reports_current_runtime_state() {
    let home = temp_dir("status_home");
    let cwd = temp_dir("status_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/status", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Session:")
                && text.contains("Provider:")
                && text.contains("Permission mode:")
                && text.contains("Transcript:")
                && text.contains("Index:")
    )));
}

#[test]
fn test_command_records_last_test_evidence_in_status() {
    let home = temp_dir("test_evidence_home");
    let cwd = temp_dir("test_evidence_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let approvals = Cell::new(0usize);
    let mut approver = |_prompt| {
        approvals.set(approvals.get() + 1);
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };

    let test_output = engine
        .process_input_with_approval("/test echo robocode-test-ok", &mut approver)
        .unwrap();

    assert_eq!(approvals.get(), 1);
    assert!(test_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Test result:")
                && text.contains("status: passed")
                && text.contains("command: echo robocode-test-ok")
                && text.contains("robocode-test-ok")
    )));

    let status_output = engine
        .process_input_with_approval("/status", &mut approver)
        .unwrap();
    assert!(status_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Last test:")
                && text.contains("passed")
                && text.contains("echo robocode-test-ok")
    )));
}

#[test]
fn test_command_records_exit_code_for_failed_shell_command() {
    let home = temp_dir("test_exit_code_home");
    let cwd = temp_dir("test_exit_code_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let test_output = engine
        .process_input_with_approval("/test sh -c 'echo failing-test; exit 7'", &mut approver)
        .unwrap();

    assert!(test_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("status: failed")
                && text.contains("exit code: 7")
                && text.contains("failing-test")
    )));

    let status_output = engine
        .process_input_with_approval("/status", &mut approver)
        .unwrap();
    assert!(status_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains("Last test: failed (exit 7)")
    )));
}

#[test]
fn config_command_reports_runtime_configuration_summary() {
    let home = temp_dir("config_home");
    let cwd = temp_dir("config_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/config", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Runtime configuration:")
                && text.contains("provider=")
                && text.contains("Loaded config files:")
    )));
}

#[test]
fn provider_list_reports_registry_and_current_provider() {
    let home = temp_dir("provider_list_home");
    let cwd = temp_dir("provider_list_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/provider list", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider registry:")
                && text.contains("Current provider: sequence (test-model)")
                && text.contains("openai-compatible")
                && text.contains("openrouter")
                && text.contains("streaming=true")
                && text.contains("tools=true")
                && text.contains("compat=default")
    )));
}

#[test]
fn provider_doctor_reports_registry_capabilities_and_env_mappings() {
    let home = temp_dir("provider_doctor_home");
    let cwd = temp_dir("provider_doctor_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/provider doctor", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider diagnostics:")
                && text.contains("Registry providers:")
                && text.contains("openrouter")
                && text.contains("api_key_env=OPENROUTER_API_KEY")
                && text.contains("streaming=true")
                && text.contains("tools=true")
                && text.contains("compat=default")
    )));
}

#[test]
fn provider_doctor_can_focus_on_one_registered_provider() {
    let home = temp_dir("provider_doctor_one_home");
    let cwd = temp_dir("provider_doctor_one_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/provider doctor openrouter", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider diagnostics: openrouter")
                && text.contains("api_key_env=OPENROUTER_API_KEY")
                && text.contains("streaming=true")
                && text.contains("tools=true")
                && !text.contains("  - openai ")
    )));
}

#[test]
fn provider_doctor_surfaces_deepseek_v4_compatibility_contract() {
    let home = temp_dir("provider_doctor_deepseek_home");
    let cwd = temp_dir("provider_doctor_deepseek_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/provider doctor deepseek", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider diagnostics: deepseek")
                && text.contains("compat=tool_choice=false, reasoning_content=required, tool_call_content=non-null, effort_high=high, effort_max=max")
    )));
}

#[test]
fn provider_doctor_reports_unknown_provider() {
    let home = temp_dir("provider_doctor_unknown_home");
    let cwd = temp_dir("provider_doctor_unknown_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/provider doctor missing-provider", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider `missing-provider` is not registered.")
    )));
}

#[test]
fn provider_reload_reports_success_without_replacing_current_provider_instance() {
    let home = temp_dir("provider_reload_home");
    let cwd = temp_dir("provider_reload_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/provider reload", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider registry reloaded:")
                && text.contains("Current provider instance remains sequence (test-model)")
    )));
}

#[test]
fn provider_reload_failure_reports_diagnostics_and_keeps_previous_registry() {
    let home = temp_dir("provider_reload_failure_home");
    let cwd = temp_dir("provider_reload_failure_cwd");
    let invalid_plugin_dir = cwd.join("provider-plugin-dir-file");
    std::fs::write(&invalid_plugin_dir, b"not a directory").unwrap();
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(
        ProviderHost::load_from_dirs_diagnostic(Vec::new()).unwrap(),
        vec![invalid_plugin_dir],
        None,
        None,
        90,
        1,
    );
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/provider reload", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider registry reload failed.")
                && text.contains("kind: ReadDirectory")
                && text.contains("Previous registry remains active")
    )));
}

#[test]
fn provider_use_switches_current_provider_and_model() {
    let home = temp_dir("provider_use_home");
    let cwd = temp_dir("provider_use_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/provider use fallback switched-model", &mut approver)
        .unwrap();

    assert_eq!(engine.provider_name(), "fallback");
    assert_eq!(engine.model_name(), "switched-model");
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider set to fallback (switched-model)")
    )));
}

#[test]
fn provider_binding_is_independent_across_sessions() {
    let home = temp_dir("provider_binding_home");
    let cwd_a = temp_dir("provider_binding_cwd_a");
    let cwd_b = temp_dir("provider_binding_cwd_b");
    let mut engine_a = SessionEngine::new_with_home(
        &cwd_a,
        Box::new(SequenceProvider::new(vec![])),
        Some(home.clone()),
    )
    .unwrap();
    let mut engine_b =
        SessionEngine::new_with_home(&cwd_b, Box::new(SequenceProvider::new(vec![])), Some(home))
            .unwrap();
    engine_a.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    engine_b.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine_a
        .process_input_with_approval("/provider use fallback model-a", &mut approver)
        .unwrap();
    engine_b
        .process_input_with_approval("/provider use fallback model-b", &mut approver)
        .unwrap();
    engine_a
        .process_input_with_approval("/provider reload", &mut approver)
        .unwrap();

    assert_eq!(engine_a.provider_name(), "fallback");
    assert_eq!(engine_a.model_name(), "model-a");
    assert_eq!(engine_b.provider_name(), "fallback");
    assert_eq!(engine_b.model_name(), "model-b");

    let output_a = engine_a
        .process_input_with_approval("hello from a", &mut approver)
        .unwrap();
    let output_b = engine_b
        .process_input_with_approval("hello from b", &mut approver)
        .unwrap();

    assert!(output_a.iter().any(|event| matches!(
        event,
        EngineEvent::Assistant(text)
            if text.contains("model `model-a`") && text.contains("hello from a")
    )));
    assert!(output_b.iter().any(|event| matches!(
        event,
        EngineEvent::Assistant(text)
            if text.contains("model `model-b`") && text.contains("hello from b")
    )));
}

#[test]
fn provider_use_reports_unknown_provider() {
    let home = temp_dir("provider_use_unknown_home");
    let cwd = temp_dir("provider_use_unknown_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/provider use missing-provider", &mut approver)
        .unwrap();

    assert_eq!(engine.provider_name(), "sequence");
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider `missing-provider` is not registered")
    )));
}

#[test]
fn doctor_command_reports_dependency_checks() {
    let home = temp_dir("doctor_home");
    let cwd = temp_dir("doctor_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/doctor", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Environment diagnostics:")
                && text.contains("git:")
                && text.contains("rg:")
                && text.contains("sqlite3:")
                && text.contains("curl:")
    )));
}

#[test]
fn doctor_report_renders_injected_dependency_probe_results() {
    let report = DoctorReport::from_probe(|tool| match tool {
        "git" => DependencyStatus::Ok,
        "rg" => DependencyStatus::Missing,
        "sqlite3" => DependencyStatus::NotRequired,
        "curl" => DependencyStatus::Ok,
        other => panic!("unexpected dependency probe for {other}"),
    });

    let rendered = report.render();

    assert!(rendered.contains("Environment diagnostics:"));
    assert!(rendered.contains("git: ok"));
    assert!(rendered.contains("rg: missing"));
    assert!(rendered.contains("sqlite3: not required for current path"));
    assert!(rendered.contains("curl: ok"));
}

#[test]
fn help_output_lists_runtime_inspection_commands() {
    let home = temp_dir("help_home");
    let cwd = temp_dir("help_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/help", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("/status")
                && text.contains("/config")
                && text.contains("/doctor")
                && text.contains("/test <command>")
    )));
}

#[test]
fn help_output_groups_commands_by_purpose() {
    let home = temp_dir("help_groups_home");
    let cwd = temp_dir("help_groups_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/help", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Runtime:")
                && text.contains("Sessions:")
                && text.contains("Repository and web:")
    )));
}
