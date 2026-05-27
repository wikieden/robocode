use robocode_config::{CliOverrides, load_config};
use robocode_core::{EngineEvent, SessionEngine};
use robocode_model::{
    ModelProvider, ProviderConfig, ProviderHost, ProviderPluginError, ProviderRegistry,
    list_supported_provider_strings,
};
use robocode_types::{ApprovalResponse, PermissionPrompt, RuntimeSnapshot};
use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

mod tui;

fn main() {
    if let Err(err) = run() {
        eprintln!("robocode: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("robocode-cli {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
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
        provider_plugin_dirs: startup.provider_plugin_dirs.clone(),
        permission_mode: startup.permission_mode,
        session_home: startup.session_home.clone(),
        request_timeout_secs: startup.request_timeout_secs,
        max_retries: startup.max_retries,
        config_path: startup.config_path.clone(),
    };
    let resolved_config = load_config(&cwd, &cli_config)?;
    let preview_provider = resolved_config.provider.as_str();
    let preview_model = resolved_config.model.as_deref().unwrap_or("default");
    if startup.tui_preview || startup.tui_preview_ansi {
        if startup.tui_preview_ansi {
            print!(
                "{}",
                tui::render_ansi_preview_with_theme(
                    preview_provider,
                    preview_model,
                    startup.tui_theme.as_deref()
                )
            );
        } else {
            println!("{}", tui::render_preview(preview_provider, preview_model));
        }
        return Ok(());
    }
    if startup.tui_preview_idle || startup.tui_preview_idle_ansi {
        if startup.tui_preview_idle_ansi {
            print!(
                "{}",
                tui::render_ansi_idle_preview_with_theme(
                    preview_provider,
                    preview_model,
                    startup.tui_theme.as_deref()
                )
            );
        } else {
            println!(
                "{}",
                tui::render_idle_preview(preview_provider, preview_model)
            );
        }
        return Ok(());
    }
    if startup.tui_preview_live_turn || startup.tui_preview_live_turn_ansi {
        if startup.tui_preview_live_turn_ansi {
            print!(
                "{}",
                tui::render_ansi_live_turn_preview_with_theme(
                    preview_provider,
                    preview_model,
                    startup.tui_theme.as_deref()
                )
            );
        } else {
            println!(
                "{}",
                tui::render_live_turn_preview(preview_provider, preview_model)
            );
        }
        return Ok(());
    }
    if startup.tui_preview_resize || startup.tui_preview_resize_ansi {
        if startup.tui_preview_resize_ansi {
            print!(
                "{}",
                tui::render_ansi_resize_preview_with_theme(
                    preview_provider,
                    preview_model,
                    startup.tui_theme.as_deref()
                )
            );
        } else {
            println!(
                "{}",
                tui::render_resize_preview(preview_provider, preview_model)
            );
        }
        return Ok(());
    }
    if startup.tui_preview_cjk_input || startup.tui_preview_cjk_input_ansi {
        if startup.tui_preview_cjk_input_ansi {
            print!(
                "{}",
                tui::render_ansi_cjk_input_preview_with_theme(
                    preview_provider,
                    preview_model,
                    startup.tui_theme.as_deref()
                )
            );
        } else {
            println!(
                "{}",
                tui::render_cjk_input_preview(preview_provider, preview_model)
            );
        }
        return Ok(());
    }
    if startup.tui_preview_command_palette || startup.tui_preview_command_palette_ansi {
        if startup.tui_preview_command_palette_ansi {
            print!(
                "{}",
                tui::render_ansi_command_palette_preview_with_theme(
                    preview_provider,
                    preview_model,
                    startup.tui_theme.as_deref()
                )
            );
        } else {
            println!(
                "{}",
                tui::render_command_palette_preview(preview_provider, preview_model)
            );
        }
        return Ok(());
    }
    if startup.tui_preview_lane || startup.tui_preview_lane_ansi {
        if startup.tui_preview_lane_ansi {
            print!(
                "{}",
                tui::render_ansi_lane_preview_with_theme(
                    preview_provider,
                    preview_model,
                    startup.tui_theme.as_deref()
                )
            );
        } else {
            println!(
                "{}",
                tui::render_lane_preview(preview_provider, preview_model)
            );
        }
        return Ok(());
    }
    if startup.tui_preview_side || startup.tui_preview_side_ansi {
        if startup.tui_preview_side_ansi {
            print!(
                "{}",
                tui::render_ansi_side_preview_with_theme(
                    preview_provider,
                    preview_model,
                    startup.tui_theme.as_deref()
                )
            );
        } else {
            println!(
                "{}",
                tui::render_side_preview(preview_provider, preview_model)
            );
        }
        return Ok(());
    }
    if startup.tui_preview_side_2 || startup.tui_preview_side_2_ansi {
        if startup.tui_preview_side_2_ansi {
            print!(
                "{}",
                tui::render_ansi_ops_preview_with_theme(
                    preview_provider,
                    preview_model,
                    startup.tui_theme.as_deref()
                )
            );
        } else {
            println!(
                "{}",
                tui::render_ops_preview(preview_provider, preview_model)
            );
        }
        return Ok(());
    }
    let provider_host = load_startup_provider_host(&resolved_config)?;
    let provider_selection = create_startup_provider(&provider_host, &resolved_config)?;
    let provider_summary = format!(
        "{} | config={} | files={}",
        provider_selection.summary,
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
        model_label: provider_selection.model_label.clone(),
        permission_mode: resolved_config.permission_mode,
        config_summary: resolved_config.summary(),
        loaded_config_files: resolved_config.loaded_files.clone(),
        startup_overrides: startup.summary_overrides(),
    };
    let mut engine = SessionEngine::new_with_home_and_snapshot(
        &cwd,
        provider_selection.provider,
        resolved_config.session_home.clone(),
        runtime_snapshot,
    )?;
    engine.set_provider_runtime(
        provider_host,
        resolved_config.provider_plugin_dirs.clone(),
        resolved_config.api_base.clone(),
        resolved_config.api_key.clone(),
        resolved_config.request_timeout_secs,
        resolved_config.max_retries,
    );
    engine.set_permission_mode(resolved_config.permission_mode)?;

    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    if let Some(selector) = startup.resume_selector.as_deref() {
        let mut approver = |prompt: PermissionPrompt| prompt_for_approval(prompt, &mut stdin);
        for event in
            engine.process_input_with_approval(&format!("/resume {selector}"), &mut approver)?
        {
            render_event(event);
        }
    }

    if startup.should_start_tui() {
        if let Some(screen) = startup
            .tui_screen
            .as_deref()
            .and_then(tui::SideScreen::parse)
        {
            return tui::run_side_tui_with_theme(
                &engine,
                &provider_summary,
                screen,
                startup.tui_theme.as_deref(),
            );
        }
        return tui::run_tui_with_theme(
            &mut engine,
            &provider_summary,
            startup.tui_theme.as_deref(),
        );
    }

    println!(
        "RoboCode session {}. Type /help for commands, Ctrl-D to exit.",
        engine.session_id()
    );
    println!("Startup provider: {provider_summary}");

    loop {
        print!("robocode> ");
        io::stdout().flush().map_err(|err| err.to_string())?;
        let Some(line) = read_lossy_line(&mut stdin).map_err(|err| err.to_string())? else {
            println!();
            break;
        };
        let trimmed = line.trim();
        if is_exit_command(trimmed) {
            break;
        }
        let mut approver = |prompt: PermissionPrompt| prompt_for_approval(prompt, &mut stdin);
        let events = engine.process_input_with_approval(trimmed, &mut approver)?;
        for event in events {
            render_event(event);
        }
    }

    Ok(())
}

fn is_exit_command(input: &str) -> bool {
    matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "exit" | "quit" | "/exit" | "/quit"
    )
}

fn load_startup_provider_host(
    resolved_config: &robocode_config::ResolvedConfig,
) -> Result<ProviderHost, String> {
    if resolved_config.provider_plugin_dirs.is_empty() {
        ProviderHost::load_default_diagnostic().map_err(format_provider_plugin_error)
    } else {
        ProviderHost::load_from_dirs_diagnostic(resolved_config.provider_plugin_dirs.clone())
            .map_err(format_provider_plugin_error)
    }
}

fn format_provider_plugin_error(err: ProviderPluginError) -> String {
    let path = if err.path.as_os_str().is_empty() {
        "<registry>".to_string()
    } else {
        err.path.display().to_string()
    };
    format!(
        "provider plugin loading failed\n  kind: {:?}\n  path: {}\n  message: {}\n  detail: {}",
        err.kind, path, err.message, err
    )
}

struct StartupProviderSelection {
    provider: Box<dyn ModelProvider>,
    model_label: String,
    summary: String,
}

fn create_startup_provider(
    host: &ProviderHost,
    resolved_config: &robocode_config::ResolvedConfig,
) -> Result<StartupProviderSelection, String> {
    match ProviderConfig::from_settings(
        &resolved_config.provider,
        resolved_config.model.as_deref(),
        resolved_config.api_base.as_deref(),
        resolved_config.api_key.as_deref(),
        resolved_config.request_timeout_secs,
        resolved_config.max_retries,
    ) {
        Ok(provider_config) => {
            let model_label = provider_config.model.clone();
            let summary = provider_config.summary();
            let provider = host.create(provider_config)?;
            Ok(StartupProviderSelection {
                provider,
                model_label,
                summary,
            })
        }
        Err(builtin_error) => create_dynamic_startup_provider(host, resolved_config)
            .map_err(|dynamic_error| format!("{builtin_error}; {dynamic_error}")),
    }
}

fn create_dynamic_startup_provider(
    host: &ProviderHost,
    resolved_config: &robocode_config::ResolvedConfig,
) -> Result<StartupProviderSelection, String> {
    let registry = host.registry();
    let descriptor = registry
        .descriptor(&resolved_config.provider)
        .ok_or_else(|| format!("Provider `{}` is not registered", resolved_config.provider))?;
    let model_label = resolved_config
        .model
        .clone()
        .or_else(|| descriptor.default_model.clone())
        .ok_or_else(|| {
            format!(
                "Provider `{}` does not define a default model; pass --model",
                resolved_config.provider
            )
        })?;
    let provider = host.create_registered(
        &resolved_config.provider,
        resolved_config.model.as_deref(),
        resolved_config.api_base.as_deref(),
        resolved_config.api_key.as_deref(),
        resolved_config.request_timeout_secs,
        resolved_config.max_retries,
    )?;
    Ok(StartupProviderSelection {
        provider,
        model_label,
        summary: dynamic_provider_summary(&registry, resolved_config),
    })
}

fn dynamic_provider_summary(
    registry: &ProviderRegistry,
    resolved_config: &robocode_config::ResolvedConfig,
) -> String {
    let descriptor = registry.descriptor(&resolved_config.provider);
    let model = resolved_config
        .model
        .as_deref()
        .or_else(|| descriptor.and_then(|descriptor| descriptor.default_model.as_deref()))
        .unwrap_or("<required>");
    let api_base = resolved_config
        .api_base
        .clone()
        .or_else(|| {
            descriptor
                .and_then(|descriptor| descriptor.env_mappings.api_base_env.as_deref())
                .and_then(|name| std::env::var(name).ok())
        })
        .or_else(|| descriptor.and_then(|descriptor| descriptor.default_api_base.clone()))
        .unwrap_or_else(|| "<required>".to_string());
    let key_present = resolved_config.api_key.is_some()
        || descriptor
            .and_then(|descriptor| descriptor.env_mappings.api_key_env.as_deref())
            .and_then(|name| std::env::var(name).ok())
            .is_some();
    format!(
        "provider={} model={} api_base={} key={} timeout={}s retries={}",
        resolved_config.provider,
        model,
        api_base,
        if key_present { "present" } else { "missing" },
        resolved_config.request_timeout_secs,
        resolved_config.max_retries,
    )
}

#[derive(Debug, Default)]
struct StartupOptions {
    provider: Option<String>,
    model: Option<String>,
    api_base: Option<String>,
    api_key: Option<String>,
    provider_plugin_dirs: Vec<PathBuf>,
    permission_mode: Option<robocode_types::PermissionMode>,
    session_home: Option<PathBuf>,
    request_timeout_secs: Option<u64>,
    max_retries: Option<u32>,
    config_path: Option<PathBuf>,
    resume_selector: Option<String>,
    tui: bool,
    no_tui: bool,
    tui_screen: Option<String>,
    tui_preview: bool,
    tui_preview_ansi: bool,
    tui_preview_idle: bool,
    tui_preview_idle_ansi: bool,
    tui_preview_live_turn: bool,
    tui_preview_live_turn_ansi: bool,
    tui_preview_resize: bool,
    tui_preview_resize_ansi: bool,
    tui_preview_cjk_input: bool,
    tui_preview_cjk_input_ansi: bool,
    tui_preview_command_palette: bool,
    tui_preview_command_palette_ansi: bool,
    tui_preview_lane: bool,
    tui_preview_lane_ansi: bool,
    tui_preview_side: bool,
    tui_preview_side_ansi: bool,
    tui_preview_side_2: bool,
    tui_preview_side_2_ansi: bool,
    tui_theme: Option<String>,
}

impl StartupOptions {
    fn should_start_tui(&self) -> bool {
        self.tui || self.tui_screen.is_some() || !self.no_tui
    }

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
        if !self.provider_plugin_dirs.is_empty() {
            overrides.push("--provider-plugin-dir".to_string());
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
        if self.tui {
            overrides.push("--tui".to_string());
        }
        if self.no_tui {
            overrides.push("--no-tui".to_string());
        }
        if self.tui_screen.is_some() {
            overrides.push("--tui-screen".to_string());
        }
        if self.tui_preview {
            overrides.push("--tui-preview".to_string());
        }
        if self.tui_preview_ansi {
            overrides.push("--tui-preview-ansi".to_string());
        }
        if self.tui_preview_idle {
            overrides.push("--tui-preview-idle".to_string());
        }
        if self.tui_preview_idle_ansi {
            overrides.push("--tui-preview-idle-ansi".to_string());
        }
        if self.tui_preview_live_turn {
            overrides.push("--tui-preview-live-turn".to_string());
        }
        if self.tui_preview_live_turn_ansi {
            overrides.push("--tui-preview-live-turn-ansi".to_string());
        }
        if self.tui_preview_resize {
            overrides.push("--tui-preview-resize".to_string());
        }
        if self.tui_preview_resize_ansi {
            overrides.push("--tui-preview-resize-ansi".to_string());
        }
        if self.tui_preview_cjk_input {
            overrides.push("--tui-preview-cjk-input".to_string());
        }
        if self.tui_preview_cjk_input_ansi {
            overrides.push("--tui-preview-cjk-input-ansi".to_string());
        }
        if self.tui_preview_command_palette {
            overrides.push("--tui-preview-command-palette".to_string());
        }
        if self.tui_preview_command_palette_ansi {
            overrides.push("--tui-preview-command-palette-ansi".to_string());
        }
        if self.tui_preview_lane {
            overrides.push("--tui-preview-lane".to_string());
        }
        if self.tui_preview_lane_ansi {
            overrides.push("--tui-preview-lane-ansi".to_string());
        }
        if self.tui_preview_side {
            overrides.push("--tui-preview-side".to_string());
        }
        if self.tui_preview_side_ansi {
            overrides.push("--tui-preview-side-ansi".to_string());
        }
        if self.tui_preview_side_2 {
            overrides.push("--tui-preview-side-2".to_string());
        }
        if self.tui_preview_side_2_ansi {
            overrides.push("--tui-preview-side-2-ansi".to_string());
        }
        if self.tui_theme.is_some() {
            overrides.push("--tui-theme".to_string());
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
            "--provider-plugin-dir" => {
                index += 1;
                options
                    .provider_plugin_dirs
                    .push(PathBuf::from(required_flag_value(
                        args,
                        index,
                        "--provider-plugin-dir",
                    )?));
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
            "--tui" => {
                options.tui = true;
            }
            "--no-tui" => {
                options.no_tui = true;
            }
            "--tui-screen" => {
                index += 1;
                let value = required_flag_value(args, index, "--tui-screen")?;
                if value != "main" && tui::SideScreen::parse(&value).is_none() {
                    return Err("--tui-screen must be `main`, `side-1`, or `side-2`".to_string());
                }
                options.tui_screen = Some(value);
            }
            "--tui-preview" => {
                options.tui_preview = true;
            }
            "--tui-preview-ansi" => {
                options.tui_preview_ansi = true;
            }
            "--tui-preview-idle" => {
                options.tui_preview_idle = true;
            }
            "--tui-preview-idle-ansi" => {
                options.tui_preview_idle_ansi = true;
            }
            "--tui-preview-live-turn" => {
                options.tui_preview_live_turn = true;
            }
            "--tui-preview-live-turn-ansi" => {
                options.tui_preview_live_turn_ansi = true;
            }
            "--tui-preview-resize" => {
                options.tui_preview_resize = true;
            }
            "--tui-preview-resize-ansi" => {
                options.tui_preview_resize_ansi = true;
            }
            "--tui-preview-cjk-input" => {
                options.tui_preview_cjk_input = true;
            }
            "--tui-preview-cjk-input-ansi" => {
                options.tui_preview_cjk_input_ansi = true;
            }
            "--tui-preview-command-palette" => {
                options.tui_preview_command_palette = true;
            }
            "--tui-preview-command-palette-ansi" => {
                options.tui_preview_command_palette_ansi = true;
            }
            "--tui-preview-lane" => {
                options.tui_preview_lane = true;
            }
            "--tui-preview-lane-ansi" => {
                options.tui_preview_lane_ansi = true;
            }
            "--tui-preview-side" => {
                options.tui_preview_side = true;
            }
            "--tui-preview-side-ansi" => {
                options.tui_preview_side_ansi = true;
            }
            "--tui-preview-side-2" => {
                options.tui_preview_side_2 = true;
            }
            "--tui-preview-side-2-ansi" => {
                options.tui_preview_side_2_ansi = true;
            }
            "--tui-theme" => {
                index += 1;
                let value = required_flag_value(args, index, "--tui-theme")?;
                if !tui::is_known_theme(&value) {
                    return Err(format!(
                        "--tui-theme must be one of: {}",
                        tui::theme_names().join(", ")
                    ));
                }
                options.tui_theme = Some(value);
            }
            unknown if unknown.starts_with("--") => {
                return Err(format!("Unknown startup flag `{unknown}`"));
            }
            _ => {}
        }
        index += 1;
    }
    if options.no_tui && options.tui {
        return Err("--no-tui cannot be combined with --tui".to_string());
    }
    if options.no_tui && options.tui_screen.is_some() {
        return Err("--no-tui cannot be combined with --tui-screen".to_string());
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
    println!("  --version, -V       Print the RoboCode CLI version");
    println!("  --provider <name>    Choose provider family");
    println!("  --model <name>       Override model name");
    println!("  --api-base <url>     Override provider base URL");
    println!("  --api-key <value>    Override API key");
    println!("  --provider-plugin-dir <dir>");
    println!("                       Add a dynamic provider plugin directory");
    println!("  --permissions <mode> Set default permission mode");
    println!("  --session-home <dir> Override transcript/index home");
    println!("  --request-timeout <s> Override provider HTTP timeout");
    println!("  --max-retries <n>    Override provider retry count");
    println!("  --config <path>      Load config from an explicit TOML file");
    println!("  --resume [id|latest] Resume a prior session");
    println!("  --tui                Start the cockpit terminal UI (default)");
    println!("  --no-tui             Start the legacy line REPL");
    println!("  --tui-screen <main|side-1|side-2>");
    println!("                       Start a specific TUI screen surface");
    println!(
        "  --tui-theme <name>   Select TUI theme: {}",
        tui::theme_names().join(", ")
    );
    println!("  --tui-preview        Print a non-interactive 140x40 TUI preview");
    println!("  --tui-preview-ansi   Print a themed ANSI 140x40 TUI preview");
    println!("  --tui-preview-idle   Print a 140x40 TUI preview without modal overlay");
    println!("  --tui-preview-idle-ansi");
    println!("                       Print a themed ANSI TUI preview without modal overlay");
    println!("  --tui-preview-live-turn");
    println!("                       Print a 140x40 TUI preview with a live provider turn");
    println!("  --tui-preview-live-turn-ansi");
    println!("                       Print a themed ANSI live provider turn preview");
    println!("  --tui-preview-resize");
    println!("                       Print a 100x30 resize-redraw TUI preview");
    println!("  --tui-preview-resize-ansi");
    println!("                       Print a themed ANSI resize-redraw preview");
    println!("  --tui-preview-cjk-input");
    println!("                       Print a 100x30 CJK input and cursor-placement preview");
    println!("  --tui-preview-cjk-input-ansi");
    println!("                       Print a themed ANSI CJK input preview");
    println!("  --tui-preview-command-palette");
    println!("                       Print a 140x40 TUI preview with slash command palette");
    println!("  --tui-preview-command-palette-ansi");
    println!("                       Print a themed ANSI slash command palette preview");
    println!("  --tui-preview-lane   Print a 140x40 focused lane-detail preview");
    println!("  --tui-preview-lane-ansi");
    println!("                       Print a themed ANSI focused lane-detail preview");
    println!("  --tui-preview-side   Print a non-interactive 80x40 side-screen preview");
    println!("  --tui-preview-side-ansi");
    println!("                       Print a themed ANSI 80x40 side-screen preview");
    println!("  --tui-preview-side-2 Print a non-interactive 80x40 ops-screen preview");
    println!("  --tui-preview-side-2-ansi");
    println!("                       Print a themed ANSI 80x40 ops-screen preview");
    println!();
    println!(
        "Supported providers: {}",
        list_supported_provider_strings().join(", ")
    );
    println!();
    println!("Environment variables:");
    println!("  ROBOCODE_PROVIDER, ROBOCODE_MODEL, ROBOCODE_API_BASE, ROBOCODE_API_KEY");
    println!("  ROBOCODE_PROVIDER_PLUGIN_DIRS");
    println!("  ROBOCODE_PERMISSION_MODE, ROBOCODE_SESSION_HOME");
    println!("  ROBOCODE_REQUEST_TIMEOUT_SECS, ROBOCODE_MAX_RETRIES, ROBOCODE_CONFIG");
    println!("  ROBOCODE_SCREEN_LAUNCH_TEMPLATE, ROBOCODE_LANE_ATTACH_TEMPLATE");
    println!("  ROBOCODE_LANE_CODEX_TEMPLATE, ROBOCODE_LANE_CLAUDE_TEMPLATE");
    println!("  ANTHROPIC_API_KEY, OPENAI_API_KEY, DEEPSEEK_API_KEY, DEEPSEEK_API_BASE");
}

fn prompt_for_approval(prompt: PermissionPrompt, stdin: &mut impl BufRead) -> ApprovalResponse {
    println!();
    println!("Permission request for `{}`", prompt.tool_name);
    println!("{}", prompt.message);
    println!("{}", prompt.input_preview);
    print!("Allow? [y/N]: ");
    io::stdout().flush().ok();
    let Ok(Some(response)) = read_lossy_line(stdin) else {
        return ApprovalResponse {
            approved: false,
            feedback: None,
        };
    };
    let approved = matches!(response.trim(), "y" | "Y" | "yes" | "YES");
    ApprovalResponse {
        approved,
        feedback: None,
    }
}

fn read_lossy_line(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    let read = reader.read_until(b'\n', &mut bytes)?;
    if read == 0 {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use robocode_model::{ProviderPluginError, ProviderPluginErrorKind};

    #[test]
    fn read_lossy_line_replaces_invalid_utf8() {
        let mut input = Cursor::new(vec![b'o', b'k', 0xff, b'\n']);
        let line = read_lossy_line(&mut input).unwrap().unwrap();
        assert_eq!(line, "ok\u{fffd}\n");
    }

    #[test]
    fn read_lossy_line_returns_none_on_eof() {
        let mut input = Cursor::new(Vec::<u8>::new());
        assert_eq!(read_lossy_line(&mut input).unwrap(), None);
    }

    #[test]
    fn parse_startup_options_collects_provider_plugin_dirs() {
        let args = vec![
            "--provider-plugin-dir".to_string(),
            "plugins-a".to_string(),
            "--provider-plugin-dir".to_string(),
            "plugins-b".to_string(),
        ];

        let options = parse_startup_options(&args).unwrap();

        assert_eq!(
            options.provider_plugin_dirs,
            vec![PathBuf::from("plugins-a"), PathBuf::from("plugins-b")]
        );
        assert_eq!(
            options.summary_overrides(),
            vec!["--provider-plugin-dir".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_flag() {
        let args = vec!["--tui".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui);
        assert!(options.should_start_tui());
        assert_eq!(options.summary_overrides(), vec!["--tui".to_string()]);
    }

    #[test]
    fn parse_startup_options_defaults_to_tui() {
        let args = Vec::<String>::new();

        let options = parse_startup_options(&args).unwrap();

        assert!(options.should_start_tui());
        assert!(options.summary_overrides().is_empty());
    }

    #[test]
    fn parse_startup_options_accepts_no_tui_escape_hatch() {
        let args = vec!["--no-tui".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.no_tui);
        assert!(!options.should_start_tui());
        assert_eq!(options.summary_overrides(), vec!["--no-tui".to_string()]);
    }

    #[test]
    fn parse_startup_options_rejects_conflicting_tui_flags() {
        let tui_err = parse_startup_options(&["--no-tui".to_string(), "--tui".to_string()])
            .expect_err("--no-tui plus --tui should fail");
        assert!(tui_err.contains("--no-tui cannot be combined with --tui"));

        let screen_err = parse_startup_options(&[
            "--no-tui".to_string(),
            "--tui-screen".to_string(),
            "side-1".to_string(),
        ])
        .expect_err("--no-tui plus --tui-screen should fail");
        assert!(screen_err.contains("--no-tui cannot be combined with --tui-screen"));
    }

    #[test]
    fn parse_startup_options_accepts_tui_screen_side_flag() {
        let args = vec!["--tui-screen".to_string(), "side-1".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert_eq!(options.tui_screen.as_deref(), Some("side-1"));
        assert!(options.should_start_tui());
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-screen".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_screen_side_2_flag() {
        let args = vec!["--tui-screen".to_string(), "side-2".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert_eq!(options.tui_screen.as_deref(), Some("side-2"));
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-screen".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_rejects_unknown_tui_screen() {
        let args = vec!["--tui-screen".to_string(), "side-3".to_string()];

        let err = match parse_startup_options(&args) {
            Ok(_) => panic!("unknown TUI screen should fail"),
            Err(err) => err,
        };

        assert!(err.contains("main`, `side-1`, or `side-2"));
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_flag() {
        let args = vec!["--tui-preview".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_ansi_flag() {
        let args = vec!["--tui-preview-ansi".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_ansi);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-ansi".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_idle_flag() {
        let args = vec!["--tui-preview-idle".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_idle);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-idle".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_idle_ansi_flag() {
        let args = vec!["--tui-preview-idle-ansi".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_idle_ansi);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-idle-ansi".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_live_turn_flags() {
        let args = vec![
            "--tui-preview-live-turn".to_string(),
            "--tui-preview-live-turn-ansi".to_string(),
        ];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_live_turn);
        assert!(options.tui_preview_live_turn_ansi);
        assert_eq!(
            options.summary_overrides(),
            vec![
                "--tui-preview-live-turn".to_string(),
                "--tui-preview-live-turn-ansi".to_string()
            ]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_reliability_flags() {
        let args = vec![
            "--tui-preview-resize".to_string(),
            "--tui-preview-resize-ansi".to_string(),
            "--tui-preview-cjk-input".to_string(),
            "--tui-preview-cjk-input-ansi".to_string(),
        ];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_resize);
        assert!(options.tui_preview_resize_ansi);
        assert!(options.tui_preview_cjk_input);
        assert!(options.tui_preview_cjk_input_ansi);
        assert_eq!(
            options.summary_overrides(),
            vec![
                "--tui-preview-resize".to_string(),
                "--tui-preview-resize-ansi".to_string(),
                "--tui-preview-cjk-input".to_string(),
                "--tui-preview-cjk-input-ansi".to_string()
            ]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_command_palette_flag() {
        let args = vec!["--tui-preview-command-palette".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_command_palette);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-command-palette".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_command_palette_ansi_flag() {
        let args = vec!["--tui-preview-command-palette-ansi".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_command_palette_ansi);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-command-palette-ansi".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_lane_flag() {
        let args = vec!["--tui-preview-lane".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_lane);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-lane".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_lane_ansi_flag() {
        let args = vec!["--tui-preview-lane-ansi".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_lane_ansi);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-lane-ansi".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_side_flag() {
        let args = vec!["--tui-preview-side".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_side);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-side".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_side_ansi_flag() {
        let args = vec!["--tui-preview-side-ansi".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_side_ansi);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-side-ansi".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_side_2_flag() {
        let args = vec!["--tui-preview-side-2".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_side_2);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-side-2".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_preview_side_2_ansi_flag() {
        let args = vec!["--tui-preview-side-2-ansi".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert!(options.tui_preview_side_2_ansi);
        assert_eq!(
            options.summary_overrides(),
            vec!["--tui-preview-side-2-ansi".to_string()]
        );
    }

    #[test]
    fn parse_startup_options_accepts_tui_theme_flag() {
        let args = vec!["--tui-theme".to_string(), "ember-gold".to_string()];

        let options = parse_startup_options(&args).unwrap();

        assert_eq!(options.tui_theme.as_deref(), Some("ember-gold"));
        assert_eq!(options.summary_overrides(), vec!["--tui-theme".to_string()]);
    }

    #[test]
    fn parse_startup_options_rejects_unknown_tui_theme() {
        let args = vec!["--tui-theme".to_string(), "unknown".to_string()];

        let err = match parse_startup_options(&args) {
            Ok(_) => panic!("unknown TUI theme should fail"),
            Err(err) => err,
        };

        assert!(err.contains("aurora-cyan"), "{err}");
        assert!(err.contains("monochrome-ice"), "{err}");
    }

    #[test]
    fn provider_plugin_error_format_includes_structured_diagnostics() {
        let err = ProviderPluginError {
            kind: ProviderPluginErrorKind::LoadLibrary,
            path: PathBuf::from("/tmp/broken-provider.dylib"),
            message: "not a dynamic library".to_string(),
        };

        let formatted = format_provider_plugin_error(err);

        assert!(formatted.contains("kind: LoadLibrary"), "{formatted}");
        assert!(
            formatted.contains("path: /tmp/broken-provider.dylib"),
            "{formatted}"
        );
        assert!(
            formatted.contains("message: not a dynamic library"),
            "{formatted}"
        );
    }
}
