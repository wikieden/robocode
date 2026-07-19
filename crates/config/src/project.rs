use toml::Value;

const MAX_PROJECT_CONFIG_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFileConfig {
    pub project_name: String,
    pub pack: String,
}

/// Parses repository policy from root `viden.toml`, deliberately separate from
/// the machine-local `.viden/config.toml` provider configuration.
pub fn parse_project_config(bytes: &[u8]) -> Result<ProjectFileConfig, String> {
    if bytes.len() > MAX_PROJECT_CONFIG_BYTES {
        return Err("viden.toml exceeds the 1 MiB limit".to_string());
    }
    let text =
        std::str::from_utf8(bytes).map_err(|error| format!("viden.toml must be UTF-8: {error}"))?;
    let value: Value = toml::from_str(text).map_err(|error: toml::de::Error| {
        error.span().map_or_else(
            || "invalid viden.toml syntax".to_string(),
            |span| {
                format!(
                    "invalid viden.toml syntax at bytes {}..{}",
                    span.start, span.end
                )
            },
        )
    })?;
    reject_secret_keys(&value, "")?;
    let root = value
        .as_table()
        .ok_or_else(|| "viden.toml must be a TOML table".to_string())?;
    let project = required_table(root, "project")?;
    reject_unknown_keys(project, &["name", "pack"], "project")?;
    let project_name = required_nonempty_string(project, "name", "project")?;
    let pack = required_nonempty_string(project, "pack", "project")?;

    if let Some(gates) = optional_table(root, "gates")? {
        if gates.is_empty() {
            return Err("viden.toml [gates] cannot be empty".to_string());
        }
        let runners = optional_table(root, "runner")?;
        for (name, gate) in gates {
            let gate = gate
                .as_table()
                .ok_or_else(|| format!("viden.toml gates.{name} must be an inline table"))?;
            reject_unknown_keys(
                gate,
                &["type", "strength", "runner", "approvers"],
                &format!("gates.{name}"),
            )?;
            required_nonempty_string(gate, "type", &format!("gates.{name}"))?;
            if let Some(runner) =
                optional_nonempty_string(gate, "runner", &format!("gates.{name}"))?
                && !runners.is_some_and(|entries| entries.contains_key(&runner))
            {
                return Err(format!(
                    "viden.toml gates.{name}.runner references missing runner `{runner}`"
                ));
            }
            if let Some(approvers) = gate.get("approvers") {
                let approvers = approvers.as_integer().ok_or_else(|| {
                    format!("viden.toml gates.{name}.approvers must be an integer")
                })?;
                if approvers < 1 {
                    return Err(format!(
                        "viden.toml gates.{name}.approvers must be at least 1"
                    ));
                }
            }
        }
    }

    if let Some(budget) = optional_table(root, "budget")? {
        reject_unknown_keys(budget, &["tokens_per_week", "warn_at"], "budget")?;
        if let Some(tokens) = budget.get("tokens_per_week")
            && tokens.as_integer().is_none_or(|value| value <= 0)
        {
            return Err("viden.toml budget.tokens_per_week must be positive".to_string());
        }
        if let Some(warn_at) = budget.get("warn_at") {
            let value = warn_at
                .as_float()
                .or_else(|| warn_at.as_integer().map(|value| value as f64))
                .ok_or_else(|| "viden.toml budget.warn_at must be a number".to_string())?;
            if !(0.0 < value && value <= 1.0) {
                return Err("viden.toml budget.warn_at must be in (0, 1]".to_string());
            }
        }
    }

    if let Some(targets) = optional_table(root, "targets")? {
        if targets.is_empty() {
            return Err("viden.toml [targets] cannot be empty".to_string());
        }
        let defaults = targets
            .values()
            .filter_map(Value::as_table)
            .filter(|target| target.get("default").and_then(Value::as_bool) == Some(true))
            .count();
        if defaults != 1 {
            return Err("viden.toml [targets] must declare exactly one default target".to_string());
        }
    }

    Ok(ProjectFileConfig { project_name, pack })
}

fn required_table<'a>(table: &'a toml::Table, key: &str) -> Result<&'a toml::Table, String> {
    optional_table(table, key)?.ok_or_else(|| format!("viden.toml requires [{key}]"))
}

fn optional_table<'a>(
    table: &'a toml::Table,
    key: &str,
) -> Result<Option<&'a toml::Table>, String> {
    table
        .get(key)
        .map(|value| {
            value
                .as_table()
                .ok_or_else(|| format!("viden.toml [{key}] must be a table"))
        })
        .transpose()
}

fn required_nonempty_string(
    table: &toml::Table,
    key: &str,
    section: &str,
) -> Result<String, String> {
    optional_nonempty_string(table, key, section)?
        .ok_or_else(|| format!("viden.toml {section}.{key} is required"))
}

fn optional_nonempty_string(
    table: &toml::Table,
    key: &str,
    section: &str,
) -> Result<Option<String>, String> {
    table
        .get(key)
        .map(|value| {
            let value = value
                .as_str()
                .ok_or_else(|| format!("viden.toml {section}.{key} must be a string"))?;
            let value = value.trim();
            if value.is_empty() {
                Err(format!("viden.toml {section}.{key} cannot be empty"))
            } else {
                Ok(value.to_string())
            }
        })
        .transpose()
}

fn reject_unknown_keys(table: &toml::Table, allowed: &[&str], section: &str) -> Result<(), String> {
    if let Some(key) = table.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("viden.toml {section}.{key} is not supported"));
    }
    Ok(())
}

fn reject_secret_keys(value: &Value, prefix: &str) -> Result<(), String> {
    const SECRET_KEYS: &[&str] = &[
        "api_key",
        "secret",
        "password",
        "access_token",
        "refresh_token",
        "private_key",
    ];
    match value {
        Value::Table(table) => {
            for (key, child) in table {
                let path = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{prefix}.{key}")
                };
                if SECRET_KEYS.contains(&key.as_str()) {
                    return Err(format!("viden.toml cannot contain secret field `{path}`"));
                }
                reject_secret_keys(child, &path)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                reject_secret_keys(child, prefix)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_project_config;

    const VALID: &str = r#"[project]
name = "boss-rush"
pack = "musegameworkshop"

[gates]
"src/**" = { type = "replay-regression", runner = "godot" }

[runner]
godot = { bin = "godot4", headless = true }

[budget]
tokens_per_week = 50000
warn_at = 0.8

[targets]
local = { default = true }
"#;

    #[test]
    fn project_config_accepts_valid_viden_toml() {
        let parsed = parse_project_config(VALID.as_bytes()).expect("valid project config");
        assert_eq!(parsed.project_name, "boss-rush");
        assert_eq!(parsed.pack, "musegameworkshop");
    }

    #[test]
    fn project_config_rejects_invalid_or_secret_bearing_viden_toml() {
        for invalid in [
            "[project]\nname = \"\"\npack = \"robot-pack\"\n",
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\napi_key = \"secret\"\n",
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\n[gates]\nsim = { type = \"sim\", runner = \"missing\" }\n",
            "[project\n",
        ] {
            assert!(
                parse_project_config(invalid.as_bytes()).is_err(),
                "{invalid}"
            );
        }
    }
}
