use std::fs;
use std::process::Command;

use crate::SessionEngine;
use viden_provider::{ProviderConfig, create_provider};
use viden_types::ApprovalResponse;

use super::temp_dir;

#[derive(Debug, Clone, Copy)]
struct DeepSeekPriceCny {
    input_cache_miss_per_million: f64,
    output_per_million: f64,
}

#[test]
#[ignore = "requires DEEPSEEK_API_KEY, live network access, and billable DeepSeek usage"]
fn deepseek_live_development_scenario_creates_and_runs_program() {
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .expect("DEEPSEEK_API_KEY is required for this ignored live smoke test");
    let model = std::env::var("VIDEN_LIVE_DEEPSEEK_MODEL")
        .unwrap_or_else(|_| "deepseek-v4-flash".to_string());
    let api_base = std::env::var("VIDEN_LIVE_DEEPSEEK_API_BASE")
        .or_else(|_| std::env::var("DEEPSEEK_API_BASE"))
        .ok();
    let cwd = temp_dir("deepseek_live_development_workspace");
    let home = temp_dir("deepseek_live_development_home");
    fs::write(
        cwd.join("README.md"),
        "Disposable Viden live development smoke workspace.\n",
    )
    .unwrap();

    let provider = create_provider(
        ProviderConfig::from_settings(
            "deepseek",
            Some(&model),
            api_base.as_deref(),
            Some(&api_key),
            120,
            1,
        )
        .unwrap(),
    );
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let prompt = r#"Real development smoke test in this disposable workspace.
Create `math_tools.py` with an `add(a, b)` function that returns `a + b`.
Create `test_math_tools.py` that imports `add`, asserts `add(2, 3) == 5`, and prints `viden-dev-scenario-ok`.
Use the available write_file tool for both files. Then run `python3 test_math_tools.py` with the shell tool."#;

    let events = engine
        .process_input_with_approval(prompt, &mut approver)
        .expect("DeepSeek provider turn should complete");
    assert!(
        events
            .iter()
            .any(|event| format!("{event:?}").contains("write_file")),
        "provider did not use write_file; events:\n{events:#?}"
    );

    let math_path = cwd.join("math_tools.py");
    let test_path = cwd.join("test_math_tools.py");
    let math_source = fs::read_to_string(&math_path).expect("math_tools.py should exist");
    let test_source = fs::read_to_string(&test_path).expect("test_math_tools.py should exist");
    assert!(
        math_source.contains("def add") && math_source.contains("a + b"),
        "unexpected math_tools.py:\n{math_source}"
    );
    assert!(
        test_source.contains("add(2, 3)") && test_source.contains("viden-dev-scenario-ok"),
        "unexpected test_math_tools.py:\n{test_source}"
    );

    let test_output = Command::new("python3")
        .arg("test_math_tools.py")
        .current_dir(&cwd)
        .output()
        .expect("python3 should run test_math_tools.py");
    assert!(
        test_output.status.success(),
        "generated test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&test_output.stdout),
        String::from_utf8_lossy(&test_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&test_output.stdout).contains("viden-dev-scenario-ok"),
        "test output missing marker:\n{}",
        String::from_utf8_lossy(&test_output.stdout)
    );

    let telemetry = engine.provider_telemetry();
    let input_tokens = telemetry.total_input_tokens;
    let output_tokens = telemetry.total_output_tokens;
    let total_tokens = telemetry.total_tokens;
    assert!(
        input_tokens > 0 && output_tokens > 0 && total_tokens > 0,
        "DeepSeek usage tokens were not reported: {telemetry:#?}"
    );

    let price = deepseek_price_cny(&model);
    let estimated_cost_cny = price.map(|price| {
        estimate_cost_cny(
            input_tokens,
            output_tokens,
            price.input_cache_miss_per_million,
            price.output_per_million,
        )
    });
    let usage_json = render_usage_json(
        &model,
        &cwd.to_string_lossy(),
        &telemetry,
        price,
        estimated_cost_cny,
    );

    println!("VIDEN_LIVE_USAGE_JSON={usage_json}");
    println!(
        "VIDEN_LIVE_USAGE_SUMMARY provider=deepseek model={} input_tokens={} output_tokens={} total_tokens={} estimated_cost_cny={} pricing_basis=deepseek_cache_miss_estimate",
        model,
        input_tokens,
        output_tokens,
        total_tokens,
        estimated_cost_cny
            .map(|cost| format!("{cost:.6}"))
            .unwrap_or_else(|| "unknown".to_string()),
    );
    println!("VIDEN_LIVE_WORKSPACE={}", cwd.display());
}

fn deepseek_price_cny(model: &str) -> Option<DeepSeekPriceCny> {
    let input_override = std::env::var("VIDEN_DEEPSEEK_INPUT_CNY_PER_MTOK")
        .ok()
        .and_then(|value| value.parse::<f64>().ok());
    let output_override = std::env::var("VIDEN_DEEPSEEK_OUTPUT_CNY_PER_MTOK")
        .ok()
        .and_then(|value| value.parse::<f64>().ok());
    let default = if model == "deepseek-v4-pro" {
        Some(DeepSeekPriceCny {
            input_cache_miss_per_million: 3.0,
            output_per_million: 6.0,
        })
    } else if matches!(
        model,
        "deepseek-v4-flash" | "deepseek-chat" | "deepseek-reasoner"
    ) {
        Some(DeepSeekPriceCny {
            input_cache_miss_per_million: 1.0,
            output_per_million: 2.0,
        })
    } else {
        None
    }?;
    Some(DeepSeekPriceCny {
        input_cache_miss_per_million: input_override
            .unwrap_or(default.input_cache_miss_per_million),
        output_per_million: output_override.unwrap_or(default.output_per_million),
    })
}

fn estimate_cost_cny(
    input_tokens: u64,
    output_tokens: u64,
    input_cny_per_million: f64,
    output_cny_per_million: f64,
) -> f64 {
    ((input_tokens as f64) * input_cny_per_million
        + (output_tokens as f64) * output_cny_per_million)
        / 1_000_000.0
}

fn render_usage_json(
    model: &str,
    workspace: &str,
    telemetry: &crate::ProviderTelemetry,
    price: Option<DeepSeekPriceCny>,
    estimated_cost_cny: Option<f64>,
) -> String {
    let estimated_cost = estimated_cost_cny
        .map(|cost| format!("{cost:.8}"))
        .unwrap_or_else(|| "null".to_string());
    let input_price = price
        .map(|price| price.input_cache_miss_per_million.to_string())
        .unwrap_or_else(|| "null".to_string());
    let output_price = price
        .map(|price| price.output_per_million.to_string())
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"provider\":\"deepseek\",\"model\":\"{}\",\"workspace\":\"{}\",\"scenario\":\"python_add_module_with_test\",\"request_count\":{},\"success_count\":{},\"failure_count\":{},\"input_tokens\":{},\"output_tokens\":{},\"total_tokens\":{},\"estimated_cost_cny\":{},\"input_cny_per_million_cache_miss\":{},\"output_cny_per_million\":{},\"pricing_basis\":\"deepseek_cache_miss_estimate\"}}",
        json_escape(model),
        json_escape(workspace),
        telemetry.request_count,
        telemetry.success_count,
        telemetry.failure_count,
        telemetry.total_input_tokens,
        telemetry.total_output_tokens,
        telemetry.total_tokens,
        estimated_cost,
        input_price,
        output_price,
    )
}

fn json_escape(input: &str) -> String {
    input
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}
