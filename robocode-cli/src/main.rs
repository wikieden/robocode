use robocode_config::{CliOverrides, load_config};
use robocode_core::{EngineEvent, SessionEngine};
use robocode_model::{ProviderConfig, create_provider, list_supported_provider_strings};
use robocode_types::{ApprovalResponse, PermissionPrompt, RuntimeSnapshot};
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;

fn main() {
    if let Err(err) = run() {
        eprintln!("robocode: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_startup_help();
        return Ok(());
    }
    let startup = parse_startup_options(&args)?;
    let cli_config = CliOverrides {
        provider: startup.provider.clone(),
        model: startup.model.clone(),
        api_base: startup.api_base.clone(),
        api_key: startup.api_key.clone(),
        permission_mode: startup.permission_mode,
        session_home: startup.session_home.clone(),
        request_timeout_secs: startup.request_timeout_secs,
        max_retries: startup.max_retries,
        config_path: startup.config_path.clone(),
    };
    let resolved_config = load_config(&cwd, &cli_config)?;
    let provider_config = ProviderConfig::from_settings(
        &resolved_config.provider,
        resolved_config.model.as_deref(),
        resolved_config.api_base.as_deref(),
        resolved_config.api_key.as_deref(),
        resolved_config.request_timeout_secs,
        resolved_config.max_retries,
    )?;
    let provider_summary = format!(
        "{} | config={} | files={}",
        provider_config.summary(),
        resolved_config.summary(),
        if resolved_config.loaded_files.is_empty() {
            "<none>".to_string()
        } else {
            resolved_config
                .loaded_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    let runtime_snapshot = RuntimeSnapshot {
        cwd: cwd.clone(),
        provider_family: resolved_config.provider.clone(),
        model_label: resolved_config
            .model
            .clone()
            .unwrap_or_else(|| provider_config.model.clone()),
        permission_mode: resolved_config.permission_mode,
        config_summary: resolved_config.summary(),
        loaded_config_files: resolved_config.loaded_files.clone(),
        startup_overrides: startup.summary_overrides(),
    };
    let mut engine = SessionEngine::new_with_home_and_snapshot(
        &cwd,
        create_provider(provider_config),
        resolved_config.session_home.clone(),
        runtime_snapshot,
    )?;
    engine.set_permission_mode(resolved_config.permission_mode)?;

    if let Some(selector) = startup.resume_selector.as_deref() {
        let mut approver = |prompt: PermissionPrompt| prompt_for_approval(prompt);
        for event in
            engine.process_input_with_approval(&format!("/resume {selector}"), &mut approver)?
        {
            render_event(event);
        }
    }

    println!(
        "RoboCode session {}. Type /help for commands, Ctrl-D to exit.",
        engine.session_id()
    );
    println!("Startup provider: {provider_summary}");

    loop {
        print!("robocode> ");
        io::stdout().flush().map_err(|err| err.to_string())?;
        let mut line = String::new();
        let read = io::stdin()
            .read_line(&mut line)
            .map_err(|err| err.to_string())?;
        if read == 0 {
            println!();
            break;
        }
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("exit") || trimmed.eq_ignore_ascii_case("quit") {
            break;
        }
        let mut approver = |prompt: PermissionPrompt| prompt_for_approval(prompt);
        let events = engine.process_input_with_approval(trimmed, &mut approver)?;
        for event in events {
            render_event(event);
        }
    }

    Ok(())
}

#[derive(Default)]
struct StartupOptions {
    provider: Option<String>,
    model: Option<String>,
    api_base: Option<String>,
    api_key: Option<String>,
    permission_mode: Option<robocode_types::PermissionMode>,
    session_home: Option<PathBuf>,
    request_timeout_secs: Option<u64>,
    max_retries: Option<u32>,
    config_path: Option<PathBuf>,
    resume_selector: Option<String>,
}

impl StartupOptions {
    fn summary_overrides(&self) -> Vec<String> {
        let mut overrides = Vec::new();
        if self.provider.is_some() {
            overrides.push("--provider".to_string());
        }
        if self.model.is_some() {
            overrides.push("--model".to_string());
        }
        if self.api_base.is_some() {
            overrides.push("--api-base".to_string());
        }
        if self.api_key.is_some() {
            overrides.push("--api-key".to_string());
        }
        if self.permission_mode.is_some() {
            overrides.push("--permissions".to_string());
        }
        if self.session_home.is_some() {
            overrides.push("--session-home".to_string());
        }
        if self.request_timeout_secs.is_some() {
            overrides.push("--request-timeout".to_string());
        }
        if self.max_retries.is_some() {
            overrides.push("--max-retries".to_string());
        }
        if self.config_path.is_some() {
            overrides.push("--config".to_string());
        }
        if self.resume_selector.is_some() {
            overrides.push("--resume".to_string());
        }
        overrides
    }
}

fn parse_startup_options(args: &[String]) -> Result<StartupOptions, String> {
    let mut options = StartupOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--provider" => {
                index += 1;
                options.provider = Some(required_flag_value(args, index, "--provider")?);
            }
            "--model" => {
                index += 1;
                options.model = Some(required_flag_value(args, index, "--model")?);
            }
            "--api-base" => {
                index += 1;
                options.api_base = Some(required_flag_value(args, index, "--api-base")?);
            }
            "--api-key" => {
                index += 1;
                options.api_key = Some(required_flag_value(args, index, "--api-key")?);
            }
            "--permissions" => {
                index += 1;
                let value = required_flag_value(args, index, "--permissions")?;
                options.permission_mode = Some(
                    robocode_types::PermissionMode::parse_cli(&value)
                        .ok_or_else(|| format!("Unknown permission mode `{value}`"))?,
                );
            }
            "--session-home" => {
                index += 1;
                options.session_home = Some(PathBuf::from(required_flag_value(
                    args,
                    index,
                    "--session-home",
                )?));
            }
            "--request-timeout" => {
                index += 1;
                let value = required_flag_value(args, index, "--request-timeout")?;
                options.request_timeout_secs = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| "--request-timeout must be an integer".to_string())?,
                );
            }
            "--max-retries" => {
                index += 1;
                let value = required_flag_value(args, index, "--max-retries")?;
                options.max_retries = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| "--max-retries must be an integer".to_string())?,
                );
            }
            "--config" => {
                index += 1;
                options.config_path =
                    Some(PathBuf::from(required_flag_value(args, index, "--config")?));
            }
            "--resume" => {
                let next = args.get(index + 1);
                if matches!(next, Some(value) if !value.starts_with("--")) {
                    index += 1;
                    options.resume_selector = next.cloned();
                } else {
                    options.resume_selector = Some("latest".to_string());
                }
            }
            unknown if unknown.starts_with("--") => {
                return Err(format!("Unknown startup flag `{unknown}`"));
            }
            _ => {}
        }
        index += 1;
    }
    Ok(options)
}

fn required_flag_value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn print_startup_help() {
    println!("RoboCode startup flags:");
    println!("  --provider <name>    Choose provider family");
    println!("  --model <name>       Override model name");
    println!("  --api-base <url>     Override provider base URL");
    println!("  --api-key <value>    Override API key");
    println!("  --permissions <mode> Set default permission mode");
    println!("  --session-home <dir> Override transcript/index home");
    println!("  --request-timeout <s> Override provider HTTP timeout");
    println!("  --max-retries <n>    Override provider retry count");
    println!("  --config <path>      Load config from an explicit TOML file");
    println!("  --resume [id|latest] Resume a prior session");
    println!();
    println!(
        "Supported providers: {}",
        list_supported_provider_strings().join(", ")
    );
    println!();
    println!("Environment variables:");
    println!("  ROBOCODE_PROVIDER, ROBOCODE_MODEL, ROBOCODE_API_BASE, ROBOCODE_API_KEY");
    println!("  ROBOCODE_PERMISSION_MODE, ROBOCODE_SESSION_HOME");
    println!("  ROBOCODE_REQUEST_TIMEOUT_SECS, ROBOCODE_MAX_RETRIES, ROBOCODE_CONFIG");
    println!("  ANTHROPIC_API_KEY, OPENAI_API_KEY");
}

fn prompt_for_approval(prompt: PermissionPrompt) -> ApprovalResponse {
    println!();
    println!("Permission request for `{}`", prompt.tool_name);
    println!("{}", prompt.message);
    println!("{}", prompt.input_preview);
    print!("Allow? [y/N]: ");
    io::stdout().flush().ok();
    let mut response = String::new();
    if io::stdin().read_line(&mut response).is_err() {
        return ApprovalResponse {
            approved: false,
            feedback: None,
        };
    }
    let approved = matches!(response.trim(), "y" | "Y" | "yes" | "YES");
    ApprovalResponse {
        approved,
        feedback: None,
    }
}

fn render_event(event: EngineEvent) {
    match event {
        EngineEvent::System(text) => println!("[system] {text}"),
        EngineEvent::Assistant(text) => println!("[assistant]\n{text}"),
        EngineEvent::ToolCall(text) => println!("[tool-call] {text}"),
        EngineEvent::ToolResult(text) => println!("[tool-result]\n{text}"),
        EngineEvent::Command(text) => println!("{text}"),
    }
}
