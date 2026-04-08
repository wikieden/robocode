use robocode_core::{EngineEvent, SessionEngine};
use robocode_model::{ProviderConfig, create_provider, list_supported_provider_strings};
use robocode_types::{ApprovalResponse, PermissionPrompt};
use std::env;
use std::io::{self, Write};

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
    let provider_config = ProviderConfig::from_env().with_overrides(
        startup.provider.as_deref(),
        startup.model.as_deref(),
        startup.api_base.as_deref(),
        startup.api_key.as_deref(),
    )?;
    let provider_summary = provider_config.summary();
    let mut engine = SessionEngine::new(cwd, create_provider(provider_config))?;

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
    resume_selector: Option<String>,
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
    println!("  --resume [id|latest] Resume a prior session");
    println!();
    println!(
        "Supported providers: {}",
        list_supported_provider_strings().join(", ")
    );
    println!();
    println!("Environment variables:");
    println!("  ROBOCODE_PROVIDER, ROBOCODE_MODEL, ROBOCODE_API_BASE, ROBOCODE_API_KEY");
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
