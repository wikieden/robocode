use std::cell::{Cell, RefCell};
use std::{fs, path::Path};

use crate::{DependencyStatus, DoctorReport, EngineEvent, SessionEngine};
use viden_provider::ProviderHost;
use viden_types::{AgentTaskStatus, ApprovalResponse, PermissionMode, WorkMode};

use super::{SequenceProvider, temp_dir};

fn init_git_repo(cwd: &Path) {
    let status = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(status.success());
}

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
                && text.contains("Work mode:")
                && text.contains("Permission level:")
                && text.contains("Transcript:")
                && text.contains("Index:")
    )));
}

#[test]
fn context_command_reports_missing_bundle_before_provider_turn() {
    let home = temp_dir("context_command_home");
    let cwd = temp_dir("context_command_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/context", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("No provider ContextBundle yet")
                && text.contains("Send a provider turn first")
    )));
}

#[test]
fn brief_command_creates_shows_and_status_reports_active_brief() {
    let home = temp_dir("brief_home");
    let cwd = temp_dir("brief_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/brief improve provider setup recovery", &mut approver)
        .unwrap();
    assert!(cwd.join(".viden/briefs/active.md").exists());
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Active brief created")
                && text.contains("improve provider setup recovery")
    )));

    let output = engine
        .process_input_with_approval("/brief show", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Active brief")
                && text.contains("## Goal")
                && text.contains("improve provider setup recovery")
    )));

    let status = engine
        .process_input_with_approval("/status", &mut approver)
        .unwrap();
    assert!(status.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Active brief:")
                && text.contains("Title: improve provider setup recovery")
    )));
}

#[test]
fn spec_alias_and_steering_init_create_context_files() {
    let home = temp_dir("brief_steering_home");
    let cwd = temp_dir("brief_steering_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .process_input_with_approval("/spec add deterministic daily loop smoke", &mut approver)
        .unwrap();
    let output = engine
        .process_input_with_approval("/brief steering init", &mut approver)
        .unwrap();

    assert!(cwd.join(".viden/briefs/active.md").exists());
    assert!(cwd.join(".viden/steering/conventions.md").exists());
    assert!(cwd.join(".viden/steering/architecture.md").exists());
    assert!(cwd.join(".viden/steering/workflows.md").exists());
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Steering files ready")
                && text.contains(".viden/steering")
    )));
}

#[test]
fn status_command_reports_dirty_files_active_tasks_and_lanes() {
    let home = temp_dir("status_cockpit_home");
    let cwd = temp_dir("status_cockpit_cwd");
    init_git_repo(&cwd);
    std::fs::write(cwd.join("dirty.txt"), "changed\n").unwrap();
    std::fs::create_dir_all(cwd.join(".viden")).unwrap();
    std::fs::write(
        cwd.join(".viden").join("lanes.tsv"),
        "L1\tcodex\tfix status cockpit\trunning\tmain\t42\tchecking status output\t\n",
    )
    .unwrap();

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .process_input_with_approval("/task add Improve status cockpit", &mut approver)
        .unwrap();
    let output = engine
        .process_input_with_approval("/status", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Workspace:")
                && text.contains("Dirty files: 2")
                && text.contains("dirty.txt")
                && text.contains(".viden/lanes.tsv")
                && text.contains("Workflows:")
                && text.contains("Active tasks: 1")
                && text.contains("Improve status cockpit")
                && text.contains("Lanes:")
                && text.contains("Active lanes: 1/1")
                && text.contains("L1 codex running 42%")
    )));
}

#[test]
fn agent_list_reports_builtin_agent_transports() {
    let home = temp_dir("agent_list_home");
    let cwd = temp_dir("agent_list_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/agent list", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Agent adapters:")
                && text.contains("codex")
                && text.contains("codex-acp")
                && text.contains("claude-acp")
                && text.contains("kiro-cli")
                && text.contains("template")
                && text.contains("tmux")
                && text.contains("pty")
                && text.contains("acp")
    )));
}

#[test]
fn agent_doctor_reports_adapter_readiness_without_mutation() {
    let home = temp_dir("agent_doctor_home");
    let cwd = temp_dir("agent_doctor_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/agent doctor codex", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Agent diagnostics:")
                && text.contains("codex")
                && text.contains("readiness:")
                && text.contains("mutation: read-only by default")
                && text.contains("evidence: job result")
                && text.contains("binary:")
                && text.contains("template:")
    )));
}

#[test]
fn agent_probe_codex_write_turn_is_guarded_by_default() {
    let home = temp_dir("agent_codex_write_probe_guard_home");
    let cwd = temp_dir("agent_codex_write_probe_guard_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let err = engine
        .process_input_with_approval("/agent probe codex --turn-write create file", &mut approver)
        .expect_err("write probe should be guarded");

    assert!(err.contains("turn-write` is disabled by default"));
    assert!(err.contains("VIDEN_EXPERIMENTAL_CODEX_APP_SERVER_WRITE=1"));
}

#[test]
fn agent_probe_acp_reports_unknown_agent_before_launch() {
    let home = temp_dir("agent_acp_probe_home");
    let cwd = temp_dir("agent_acp_probe_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let err = engine
        .process_input_with_approval("/agent probe acp unknown-agent", &mut approver)
        .expect_err("unknown ACP agent should fail before process launch");

    assert!(err.contains("Unknown ACP agent `unknown-agent`"));
    assert!(err.contains("kiro-cli"));
}

#[test]
fn agent_run_acp_reports_unknown_agent_before_launch() {
    let home = temp_dir("agent_acp_run_home");
    let cwd = temp_dir("agent_acp_run_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let err = engine
        .process_input_with_approval("/agent run acp unknown-agent build a plan", &mut approver)
        .expect_err("unknown ACP agent should fail before process launch");

    assert!(err.contains("Unknown ACP agent `unknown-agent`"));
    assert!(err.contains("kiro-cli"));
}

#[test]
fn agent_run_acp_requires_task() {
    let home = temp_dir("agent_acp_run_empty_home");
    let cwd = temp_dir("agent_acp_run_empty_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let err = engine
        .process_input_with_approval("/agent run acp kiro-cli", &mut approver)
        .expect_err("ACP run requires a task");

    assert!(err.contains("Usage: /agent run acp"));
    assert!(err.contains("--load-session"));
}

#[test]
fn agent_doctor_reports_kiro_acp_descriptor_without_global_command_env() {
    let home = temp_dir("agent_kiro_doctor_home");
    let cwd = temp_dir("agent_kiro_doctor_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/agent doctor kiro-cli", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Kiro CLI")
                && text.contains("transport: acp")
                && text.contains("source: local-command")
                && text.contains("readiness: installed; auth unknown")
                && text.contains("auth: agent-native; run `kiro-cli login`")
                && text.contains("command: kiro-cli acp")
                && text.contains("session/set_model")
                && text.contains("_kiro.dev/commands/execute")
    )));
}

#[test]
fn agent_auth_kiro_reports_native_login_flow() {
    let home = temp_dir("agent_kiro_auth_home");
    let cwd = temp_dir("agent_kiro_auth_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/agent auth acp kiro-cli", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Kiro CLI uses native authentication")
                && text.contains("kiro-cli login --use-device-flow")
                && text.contains("/agent smoke acp --live")
    )));
}

#[test]
fn agent_doctor_reports_experimental_acp_readiness() {
    let home = temp_dir("agent_acp_doctor_home");
    let cwd = temp_dir("agent_acp_doctor_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/agent doctor acp", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("ACP agent server")
                && text.contains("transport: acp")
                && text.contains("mutation: agent-native; mutating file/terminal requests require runtime bridge")
                && text.contains("evidence: JSONL wire log, session result, permission decisions")
                && text.contains("descriptor probe, minimal session run, and tracked async session jobs")
                && text.contains("VIDEN_AGENT_ACP_COMMAND")
                && text.contains("command: missing")
    )));
}

#[test]
fn agent_run_codex_write_requires_permission_before_launch() {
    let home = temp_dir("agent_codex_write_home");
    let cwd = temp_dir("agent_codex_write_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let approvals = Cell::new(0usize);
    let prompts = RefCell::new(Vec::new());
    let mut approver = |prompt| {
        approvals.set(approvals.get() + 1);
        prompts.borrow_mut().push(prompt);
        ApprovalResponse {
            approved: false,
            feedback: None,
        }
    };

    let output = engine
        .process_input_with_approval("/agent run codex --write modify src/lib.rs", &mut approver)
        .unwrap();

    assert_eq!(approvals.get(), 1);
    let prompts = prompts.borrow();
    assert_eq!(prompts[0].tool_name, "agent_codex_write");
    assert!(prompts[0].input_preview.contains("mode: workspace-write"));
    assert!(prompts[0].input_preview.contains("task: modify src/lib.rs"));
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Permission decision:")
                && text.contains("tool: agent_codex_write")
                && text.contains("decision=deny")
    )));
    assert!(
        !cwd.join(".viden")
            .join("agents")
            .join("codex-jobs.jsonl")
            .exists()
    );
}

#[test]
fn extension_visibility_commands_report_read_only_surfaces() {
    let home = temp_dir("extensions_home");
    let cwd = temp_dir("extensions_cwd");
    std::fs::write(cwd.join(".mcp.json"), "{\"mcpServers\":{\"demo\":{}}}").unwrap();
    std::fs::create_dir_all(cwd.join(".codex").join("skills").join("demo-skill")).unwrap();
    std::fs::write(
        cwd.join(".codex")
            .join("skills")
            .join("demo-skill")
            .join("SKILL.md"),
        "# Demo Skill\n",
    )
    .unwrap();

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let extensions = engine
        .process_input_with_approval("/extensions list", &mut approver)
        .unwrap();
    let mcp = engine
        .process_input_with_approval("/mcp list", &mut approver)
        .unwrap();
    let skills = engine
        .process_input_with_approval("/skills list", &mut approver)
        .unwrap();
    let extension_doctor = engine
        .process_input_with_approval("/extensions doctor", &mut approver)
        .unwrap();
    let mcp_doctor = engine
        .process_input_with_approval("/mcp doctor", &mut approver)
        .unwrap();

    assert!(extensions.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Extension surfaces:")
                && text.contains("agents")
                && text.contains("mcp")
                && text.contains("skills")
    )));
    assert!(mcp.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("MCP visibility:")
                && text.contains(".mcp.json")
                && text.contains("demo")
    )));
    assert!(skills.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Skills:")
                && text.contains("demo-skill")
                && text.contains("project")
    )));
    assert!(extension_doctor.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Extension diagnostics:")
                && text.contains("provider plugins")
                && text.contains("mcp: found")
                && text.contains("servers: demo")
                && text.contains("skills/project: found 1 skill")
                && text.contains("boundary: extensions remain read-only")
    )));
    assert!(mcp_doctor.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("MCP diagnostics:")
                && text.contains("found")
                && text.contains("servers: demo")
                && text.contains("permission")
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
        .process_input_with_approval("/test echo viden-test-ok", &mut approver)
        .unwrap();

    assert_eq!(approvals.get(), 1);
    assert!(test_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Test result:")
                && text.contains("status: passed")
                && text.contains("command: echo viden-test-ok")
                && text.contains("viden-test-ok")
    )));

    let status_output = engine
        .process_input_with_approval("/status", &mut approver)
        .unwrap();
    assert!(status_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Last test:")
                && text.contains("passed")
                && text.contains("echo viden-test-ok")
    )));
    let snapshot = engine.agent_task_snapshot();
    assert!(snapshot.iter().any(|task| {
        task.kind == "test"
            && task.status == AgentTaskStatus::Done.as_str()
            && task
                .evidence
                .iter()
                .any(|item| item == "command echo viden-test-ok")
    }));
}

#[test]
fn plan_mode_blocks_test_command_shell_execution() {
    let home = temp_dir("plan_test_command_home");
    let cwd = temp_dir("plan_test_command_cwd");
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

    engine
        .process_input_with_approval("/plan on", &mut approver)
        .unwrap();
    let output = engine
        .process_input_with_approval("/test printf plan-should-not-run", &mut approver)
        .unwrap();
    let rendered = output
        .iter()
        .filter_map(|event| match event {
            EngineEvent::Command(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(approvals.get(), 0);
    assert!(rendered.contains("Test result:"));
    assert!(rendered.contains("status: failed"));
    assert!(rendered.contains("command: printf plan-should-not-run"));
    assert!(rendered.contains("reason: PlanMode"));
    assert!(rendered.contains("message: shell is blocked while plan mode is active"));
    assert!(!rendered.contains("status: passed"));

    let status_output = engine
        .process_input_with_approval("/status", &mut approver)
        .unwrap();
    assert!(status_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains("Last test: failed")
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
fn test_command_extracts_failure_summary_and_failing_files() {
    let home = temp_dir("test_failure_summary_home");
    let cwd = temp_dir("test_failure_summary_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let test_output = engine
        .process_input_with_approval(
            "/test printf 'error[E0308]: mismatched types\\n --> src/lib.rs:12:5\\nfailures:\\n    tests::loads_config\\nFAILED tests/test_cli.py::test_help - AssertionError\\n'; false",
            &mut approver,
        )
        .unwrap();

    assert!(test_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("failure summary:")
                && text.contains("error[E0308]: mismatched types")
                && text.contains("tests::loads_config")
                && text.contains("failing files:")
                && text.contains("src/lib.rs:12:5")
                && text.contains("tests/test_cli.py")
    )));

    let status_output = engine
        .process_input_with_approval("/status", &mut approver)
        .unwrap();
    assert!(status_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains("Last test: failed (exit 1) files=2")
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
fn settings_command_reports_first_run_provider_model_setup() {
    let home = temp_dir("settings_home");
    let cwd = temp_dir("settings_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/settings", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Settings picker:")
                && text.contains("Current: provider sequence / model test-model / permissions default")
                && text.contains("- Provider       /settings provider")
                && text.contains("- Permissions    /settings permissions")
                && text.contains("- Theme          /settings theme")
                && text.contains("Available providers:")
                && text.contains("fallback")
    )));
}

#[test]
fn setup_command_renders_interactive_provider_model_flow() {
    let home = temp_dir("setup_alias_home");
    let cwd = temp_dir("setup_alias_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/setup", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider/model setup:")
                && text.contains("/setup provider")
                && text.contains("/models")
                && text.contains("/provider fallback test-local")
                && text.contains("Provider choices:")
                && text.contains("/setup provider deepseek")
    )));
}

#[test]
fn settings_permissions_without_args_renders_actionable_picker() {
    let home = temp_dir("settings_permissions_picker_home");
    let cwd = temp_dir("settings_permissions_picker_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/settings permissions", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Choose permission level:")
                && text.contains("/settings permissions ask")
                && text.contains("/settings permissions auto_edit")
                && text.contains("/settings permissions read_only")
                && text.contains("/settings permissions full_access")
    )));
}

#[test]
fn settings_permissions_sets_permission_mode() {
    let home = temp_dir("settings_permissions_home");
    let cwd = temp_dir("settings_permissions_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/settings permissions plan", &mut approver)
        .unwrap();

    assert_eq!(engine.mode().cli_name(), "plan");
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Permission level set to read_only")
                && text.contains("provider sequence / model test-model / permissions read_only")
    )));
}

#[test]
fn mode_command_switches_between_plan_and_build_work_modes() {
    let home = temp_dir("mode_command_home");
    let cwd = temp_dir("mode_command_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let plan_output = engine
        .process_input_with_approval("/mode plan", &mut approver)
        .unwrap();
    assert_eq!(engine.work_mode(), WorkMode::Plan);
    assert_eq!(engine.mode(), PermissionMode::Plan);
    assert!(plan_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Work mode set to plan")
                && text.contains("permission read_only")
    )));

    let build_output = engine
        .process_input_with_approval("/mode build", &mut approver)
        .unwrap();
    assert_eq!(engine.work_mode(), WorkMode::Build);
    assert_eq!(engine.mode(), PermissionMode::Default);
    assert!(build_output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Work mode set to build")
                && text.contains("permission ask")
    )));
}

#[test]
fn settings_theme_without_args_renders_actionable_picker() {
    let home = temp_dir("settings_theme_home");
    let cwd = temp_dir("settings_theme_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/settings theme", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Choose TUI theme:")
                && text.contains("/settings theme aurora-cyan")
                && text.contains("/settings theme ember-gold")
    )));
}

#[test]
fn setup_provider_without_args_renders_actionable_picker() {
    let home = temp_dir("setup_provider_picker_home");
    let cwd = temp_dir("setup_provider_picker_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/setup provider", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider configuration:")
                && text.contains("DEEPSEEK_API_KEY")
                && text.contains("inspect: /provider doctor deepseek")
                && text.contains("select default: /setup provider deepseek")
    )));
}

#[test]
fn setup_provider_switches_and_saves_like_settings() {
    let home = temp_dir("setup_provider_home");
    let cwd = temp_dir("setup_provider_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_user_config_path_override(cwd.join("user-config.toml"));
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/setup provider fallback test-local", &mut approver)
        .unwrap();

    assert_eq!(engine.provider_name(), "fallback");
    assert_eq!(engine.model_name(), "test-local");
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider set to fallback (test-local)")
                && text.contains("Saved default provider/model")
    )));
}

#[test]
fn settings_provider_can_save_provider_scoped_config() {
    let home = temp_dir("settings_provider_config_home");
    let cwd = temp_dir("settings_provider_config_cwd");
    let config_path = cwd.join("user-config.toml");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_user_config_path_override(config_path.clone());
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let endpoint = engine
        .process_input_with_approval(
            "/settings provider deepseek endpoint https://api.deepseek.com",
            &mut approver,
        )
        .unwrap();
    let key_env = engine
        .process_input_with_approval(
            "/settings provider deepseek key-env DEEPSEEK_API_KEY",
            &mut approver,
        )
        .unwrap();
    let model = engine
        .process_input_with_approval(
            "/settings provider deepseek default-model deepseek-v4-pro",
            &mut approver,
        )
        .unwrap();

    assert!(endpoint.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Saved provider config: deepseek endpoint https://api.deepseek.com")
    )));
    assert!(key_env.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Saved provider config: deepseek key env DEEPSEEK_API_KEY")
    )));
    assert!(model.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Saved provider config: deepseek default model deepseek-v4-pro")
    )));

    let contents = std::fs::read_to_string(config_path).unwrap();
    assert!(contents.contains("[providers.deepseek]"));
    assert!(contents.contains(r#"api_base = "https://api.deepseek.com""#));
    assert!(contents.contains(r#"api_key_env = "DEEPSEEK_API_KEY""#));
    assert!(contents.contains(r#"default_model = "deepseek-v4-pro""#));
    assert!(!contents.contains("api_key ="));
}

#[test]
fn provider_direct_switches_and_saves_defaults() {
    let home = temp_dir("provider_direct_home");
    let cwd = temp_dir("provider_direct_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_user_config_path_override(cwd.join("user-config.toml"));
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/provider fallback test-local", &mut approver)
        .unwrap();

    assert_eq!(engine.provider_name(), "fallback");
    assert_eq!(engine.model_name(), "test-local");
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider set to fallback (test-local)")
                && text.contains("Saved default provider/model")
                && text.contains("Next live turn uses")
    )));
}

#[test]
fn model_command_sets_and_saves_defaults() {
    let home = temp_dir("model_direct_home");
    let cwd = temp_dir("model_direct_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_user_config_path_override(cwd.join("user-config.toml"));
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/model test-local", &mut approver)
        .unwrap();

    assert_eq!(engine.model_name(), "test-local");
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Model set to test-local")
                && text.contains("Saved default provider/model")
                && text.contains("Current provider")
    )));
}

#[test]
fn model_without_args_shows_actionable_picker() {
    let home = temp_dir("model_picker_home");
    let cwd = temp_dir("model_picker_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/model", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Choose a model:")
                && text.contains("/model test-model")
                && text.contains("writes user config")
    )));
}

#[test]
fn models_without_args_groups_choices_by_provider() {
    let home = temp_dir("models_picker_home");
    let cwd = temp_dir("models_picker_cwd");
    let config_path = cwd.join("user-config.toml");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_user_config_path_override(config_path);
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .process_input_with_approval(
            "/settings provider deepseek models deepseek-v4-flash deepseek-v4-pro",
            &mut approver,
        )
        .unwrap();
    engine
        .process_input_with_approval(
            "/settings provider deepseek favorite-model deepseek-v4-pro",
            &mut approver,
        )
        .unwrap();
    let output = engine
        .process_input_with_approval("/models", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Models are grouped by configured provider")
                && text.contains("DeepSeek (deepseek)")
                && text.contains("/models deepseek deepseek-v4-flash")
                && text.contains("/models deepseek deepseek-v4-pro")
                && !text.contains("Kimi (kimi)")
                && !text.contains("<free-type>")
    )));
}

#[test]
fn connect_without_args_opens_provider_connection_picker() {
    let home = temp_dir("connect_picker_home");
    let cwd = temp_dir("connect_picker_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/connect", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider configuration:")
                && text.contains("select default: /connect deepseek")
                && text.contains("Use `/models` to choose a model across providers.")
    )));
}

#[test]
fn connect_provider_detail_reports_auth_mode() {
    let home = temp_dir("connect_auth_mode_home");
    let cwd = temp_dir("connect_auth_mode_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/connect openai", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Connect provider: openai / OpenAI")
                && text.contains("auth: web login or API key")
                && text.contains("key env: OPENAI_API_KEY")
    )));
}

#[test]
fn settings_provider_enable_model_writes_active_model_list() {
    let home = temp_dir("provider_enable_model_home");
    let cwd = temp_dir("provider_enable_model_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let config_path = cwd.join("user-config.toml");
    engine.set_user_config_path_override(config_path.clone());
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval(
            "/settings provider deepseek enable-model deepseek-v4-pro",
            &mut approver,
        )
        .unwrap();

    let contents = fs::read_to_string(config_path).unwrap();
    assert!(contents.contains("[providers.deepseek]"));
    assert!(contents.contains(r#"models = ["deepseek-v4-pro"]"#));
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Enabled model for /models: deepseek / deepseek-v4-pro")
    )));
}

#[test]
fn settings_provider_favorite_model_writes_favorite_and_active_model() {
    let home = temp_dir("provider_favorite_model_home");
    let cwd = temp_dir("provider_favorite_model_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let config_path = cwd.join("user-config.toml");
    engine.set_user_config_path_override(config_path.clone());
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval(
            "/settings provider deepseek favorite-model deepseek-v4-pro",
            &mut approver,
        )
        .unwrap();

    let contents = fs::read_to_string(config_path).unwrap();
    assert!(contents.contains("[providers.deepseek]"));
    assert!(contents.contains(r#"models = ["deepseek-v4-pro"]"#));
    assert!(contents.contains(r#"favorite_models = ["deepseek-v4-pro"]"#));
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Favorited model for /models: deepseek / deepseek-v4-pro")
                && text.contains("without duplicating in provider groups")
    )));
}

#[test]
fn models_command_switches_provider_and_model() {
    let home = temp_dir("models_switch_home");
    let cwd = temp_dir("models_switch_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_user_config_path_override(cwd.join("user-config.toml"));
    engine.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let output = engine
        .process_input_with_approval("/models fallback test-local", &mut approver)
        .unwrap();

    assert_eq!(engine.provider_name(), "fallback");
    assert_eq!(engine.model_name(), "test-local");
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Provider set to fallback (test-local)")
                && text.contains("Saved default provider/model")
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
                && text.contains("live smoke: scripts/provider-live-smoke.sh --provider openrouter")
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
