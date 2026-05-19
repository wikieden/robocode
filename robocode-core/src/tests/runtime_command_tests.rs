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
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new());
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
    )));
}

#[test]
fn provider_reload_reports_success_without_replacing_current_provider_instance() {
    let home = temp_dir("provider_reload_home");
    let cwd = temp_dir("provider_reload_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new());
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
