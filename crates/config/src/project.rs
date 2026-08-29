use toml::Value;

const MAX_PROJECT_CONFIG_BYTES: usize = 1024 * 1024;

/// Repository policy declared in root `viden.toml`.
///
/// This type is the parse and validation result only. The parser guarantees
/// that every returned policy is well-formed, but enforcement wiring
/// (permission decisions, tool/MCP dispatch filtering, lane network egress) is
/// deliberately not part of this change and is not implemented anywhere yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFileConfig {
    pub project_name: String,
    pub pack: String,
    /// Additional domain packs beyond the primary `pack`, in declaration
    /// order. Empty when `[project] packs` is absent.
    pub packs: Vec<String>,
    /// Path-pattern ownership rules. `toml::Table` is a `BTreeMap`, so rules
    /// are returned sorted lexicographically by pattern, which keeps the
    /// result deterministic across parses. Empty when `[ownership]` is absent.
    pub ownership: Vec<OwnershipRule>,
    /// Tool and MCP allowlists. `None` when `[allowlists]` is absent.
    pub allowlists: Option<ToolAllowlists>,
    /// Data egress policy. `None` when `[egress]` is absent.
    pub egress: Option<EgressPolicy>,
}

/// One `[ownership]` entry: who owns a path pattern and who may review it.
///
/// The pattern is stored verbatim (trimmed); glob semantics are the concern of
/// later enforcement, not of this parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipRule {
    pub pattern: String,
    pub owner: String,
    /// Reviewers other than the owner, in declaration order. Empty when the
    /// `reviewers` key is absent.
    pub reviewers: Vec<String>,
}

/// Declared tool and MCP allowlists.
///
/// `None` means the key was absent and the section states no policy for that
/// surface. `Some(vec![])` is an explicit, meaningful "allow none" policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolAllowlists {
    pub tools: Option<Vec<String>>,
    pub mcp: Option<Vec<String>>,
}

/// Declared data egress policy. `allow` is non-empty only for
/// [`EgressMode::Allowlist`] and empty for every other mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressPolicy {
    pub policy: EgressMode,
    pub allow: Vec<String>,
}

/// Accepted `[egress] policy` values. Parsing is exact-match and fail-closed:
/// an unknown value is an error rather than a silent downgrade to a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EgressMode {
    Blocked,
    Loopback,
    Allowlist,
    Unrestricted,
}

impl EgressMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "blocked" => Some(Self::Blocked),
            "loopback" => Some(Self::Loopback),
            "allowlist" => Some(Self::Allowlist),
            "unrestricted" => Some(Self::Unrestricted),
            _ => None,
        }
    }
}

/// How the individual entries of a `viden.toml` string array are validated.
#[derive(Clone, Copy)]
enum EntryRule {
    /// A plain name: surrounding whitespace is trimmed and the remainder must
    /// be non-empty.
    TrimmedName,
    /// A tool or MCP identifier. Kept verbatim so that ids such as
    /// `server:tool` and `mcp.fs:read_file` survive unchanged.
    AllowlistEntry,
    /// A host name with an optional `:port` suffix.
    EgressHost,
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
    reject_credential_shaped_values(&value, "")?;
    let root = value
        .as_table()
        .ok_or_else(|| "viden.toml must be a TOML table".to_string())?;
    reject_unknown_keys(
        root,
        &[
            "project",
            "gates",
            "runner",
            "budget",
            "targets",
            "ownership",
            "allowlists",
            "egress",
        ],
        "root",
    )?;
    let project = required_table(root, "project")?;
    reject_unknown_keys(project, &["name", "pack", "packs"], "project")?;
    let project_name = required_nonempty_string(project, "name", "project")?;
    let pack = required_nonempty_string(project, "pack", "project")?;
    let packs = parse_packs(project, &pack)?;

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

    if let Some(runners) = optional_table(root, "runner")? {
        if runners.is_empty() {
            return Err("viden.toml [runner] cannot be empty".to_string());
        }
        for (name, runner) in runners {
            let runner = runner
                .as_table()
                .ok_or_else(|| format!("viden.toml runner.{name} must be an inline table"))?;
            reject_unknown_keys(
                runner,
                &[
                    "bin", "headless", "version", "cmd", "gpus", "scene", "realtime",
                ],
                &format!("runner.{name}"),
            )?;
            validate_runner_fields(name, runner)?;
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
        let mut defaults = 0;
        for (name, target) in targets {
            let target = target
                .as_table()
                .ok_or_else(|| format!("viden.toml targets.{name} must be an inline table"))?;
            reject_unknown_keys(target, &["default"], &format!("targets.{name}"))?;
            let is_default = target
                .get("default")
                .ok_or_else(|| format!("viden.toml targets.{name}.default is required"))?
                .as_bool()
                .ok_or_else(|| format!("viden.toml targets.{name}.default must be a boolean"))?;
            defaults += usize::from(is_default);
        }
        if defaults != 1 {
            return Err("viden.toml [targets] must declare exactly one default target".to_string());
        }
    }

    let ownership = parse_ownership(root)?;
    let allowlists = parse_allowlists(root)?;
    let egress = parse_egress(root)?;

    Ok(ProjectFileConfig {
        project_name,
        pack,
        packs,
        ownership,
        allowlists,
        egress,
    })
}

/// Parses the optional `[project] packs` list of additional domain packs.
///
/// The primary `pack` stays required and separate, so repeating it here is an
/// error rather than a redundant no-op.
fn parse_packs(project: &toml::Table, pack: &str) -> Result<Vec<String>, String> {
    let Some(packs) = optional_string_array(project, "packs", "project", EntryRule::TrimmedName)?
    else {
        return Ok(Vec::new());
    };
    if packs.is_empty() {
        return Err("viden.toml project.packs cannot be empty".to_string());
    }
    if packs.iter().any(|entry| entry == pack) {
        return Err(format!(
            "viden.toml project.packs cannot repeat the primary pack `{pack}`"
        ));
    }
    Ok(packs)
}

/// Parses `[ownership]`: a table keyed by path pattern, each value an inline
/// table naming one owner and optional reviewers.
fn parse_ownership(root: &toml::Table) -> Result<Vec<OwnershipRule>, String> {
    let Some(entries) = optional_table(root, "ownership")? else {
        return Ok(Vec::new());
    };
    if entries.is_empty() {
        return Err("viden.toml [ownership] cannot be empty".to_string());
    }
    let mut rules = Vec::with_capacity(entries.len());
    for (pattern, rule) in entries {
        let section = format!("ownership.{pattern}");
        let rule = rule
            .as_table()
            .ok_or_else(|| format!("viden.toml {section} must be an inline table"))?;
        reject_unknown_keys(rule, &["owner", "reviewers"], &section)?;
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return Err("viden.toml [ownership] pattern keys cannot be empty".to_string());
        }
        let owner = required_nonempty_string(rule, "owner", &section)?;
        let reviewers = optional_string_array(rule, "reviewers", &section, EntryRule::TrimmedName)?
            .unwrap_or_default();
        if reviewers.contains(&owner) {
            return Err(format!(
                "viden.toml {section}.reviewers cannot contain the owner `{owner}`"
            ));
        }
        // Patterns are stored trimmed, so distinct TOML keys such as
        // `"src/**"` and `" src/**"` would otherwise collapse into two rules
        // with the same pattern and ambiguous ownership. Fail closed instead.
        if rules
            .iter()
            .any(|existing: &OwnershipRule| existing.pattern == pattern)
        {
            return Err(format!(
                "viden.toml [ownership] contains duplicate pattern `{pattern}`"
            ));
        }
        rules.push(OwnershipRule {
            pattern: pattern.to_string(),
            owner,
            reviewers,
        });
    }
    Ok(rules)
}

/// Parses `[allowlists]` tool and MCP identifier lists.
fn parse_allowlists(root: &toml::Table) -> Result<Option<ToolAllowlists>, String> {
    let Some(allowlists) = optional_table(root, "allowlists")? else {
        return Ok(None);
    };
    if allowlists.is_empty() {
        return Err("viden.toml [allowlists] cannot be empty".to_string());
    }
    reject_unknown_keys(allowlists, &["tools", "mcp"], "allowlists")?;
    // An explicitly empty array is a meaningful fail-closed policy ("allow
    // none") and must stay distinguishable from an absent key, which declares
    // no policy for that surface at all.
    Ok(Some(ToolAllowlists {
        tools: optional_string_array(allowlists, "tools", "allowlists", EntryRule::AllowlistEntry)?,
        mcp: optional_string_array(allowlists, "mcp", "allowlists", EntryRule::AllowlistEntry)?,
    }))
}

/// Parses `[egress]` and enforces that `allow` is present and non-empty for
/// the allowlist policy, and absent for every other policy.
fn parse_egress(root: &toml::Table) -> Result<Option<EgressPolicy>, String> {
    let Some(egress) = optional_table(root, "egress")? else {
        return Ok(None);
    };
    reject_unknown_keys(egress, &["policy", "allow"], "egress")?;
    let policy = required_nonempty_string(egress, "policy", "egress")?;
    let policy = EgressMode::parse(&policy).ok_or_else(|| {
        "viden.toml egress.policy must be one of \"blocked\", \"loopback\", \"allowlist\", or \"unrestricted\""
            .to_string()
    })?;
    let allow = optional_string_array(egress, "allow", "egress", EntryRule::EgressHost)?;
    let allow = match (policy, allow) {
        (EgressMode::Allowlist, Some(hosts)) if !hosts.is_empty() => hosts,
        (EgressMode::Allowlist, _) => {
            return Err(
                "viden.toml egress.allow must list at least one host when policy = \"allowlist\""
                    .to_string(),
            );
        }
        (_, Some(_)) => {
            return Err(
                "viden.toml egress.allow is only valid with policy = \"allowlist\"".to_string(),
            );
        }
        (_, None) => Vec::new(),
    };
    Ok(Some(EgressPolicy { policy, allow }))
}

/// Reads an optional array of strings, rejecting non-string entries and
/// duplicates and applying `rule` to each entry.
fn optional_string_array(
    table: &toml::Table,
    key: &str,
    section: &str,
    rule: EntryRule,
) -> Result<Option<Vec<String>>, String> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    let items = value
        .as_array()
        .ok_or_else(|| format!("viden.toml {section}.{key} must be an array of strings"))?;
    let mut entries: Vec<String> = Vec::with_capacity(items.len());
    for item in items {
        let raw = item
            .as_str()
            .ok_or_else(|| format!("viden.toml {section}.{key} must be an array of strings"))?;
        let entry = match rule {
            EntryRule::TrimmedName => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    return Err(format!(
                        "viden.toml {section}.{key} entries cannot be empty"
                    ));
                }
                trimmed.to_string()
            }
            EntryRule::AllowlistEntry => {
                if raw.is_empty() || !raw.chars().all(is_allowlist_char) {
                    return Err(format!(
                        "viden.toml {section}.{key} entry `{raw}` must use only lowercase ASCII letters, digits, `_`, `-`, `.`, or `:`"
                    ));
                }
                raw.to_string()
            }
            EntryRule::EgressHost => {
                validate_egress_host(raw, section, key)?;
                raw.to_string()
            }
        };
        if entries.contains(&entry) {
            return Err(format!(
                "viden.toml {section}.{key} contains duplicate entry `{entry}`"
            ));
        }
        entries.push(entry);
    }
    Ok(Some(entries))
}

/// Tool and MCP ids stay machine-comparable: no case folding, no whitespace,
/// and no separators beyond the ones real ids use.
fn is_allowlist_char(ch: char) -> bool {
    ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '-' | '.' | ':')
}

/// Validates one `[egress] allow` host: a bare lowercase host name with an
/// optional numeric port. URLs, paths, and userinfo are rejected so that the
/// declared policy cannot be widened by an ambiguous entry.
fn validate_egress_host(raw: &str, section: &str, key: &str) -> Result<(), String> {
    let invalid = |reason: &str| format!("viden.toml {section}.{key} host `{raw}` {reason}");
    if raw.is_empty() || raw != raw.trim() {
        return Err(invalid("must be a non-empty trimmed host name"));
    }
    if raw.contains('/') || raw.contains('@') || raw.chars().any(char::is_whitespace) {
        return Err(invalid(
            "must not contain a scheme, path, `@`, or whitespace",
        ));
    }
    let (host, port) = match raw.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (raw, None),
    };
    if host.is_empty()
        || !host
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '-'))
    {
        return Err(invalid(
            "must use lowercase ASCII letters, digits, `.`, or `-`",
        ));
    }
    if let Some(port) = port {
        if port.is_empty() || !port.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(invalid("must use a numeric port suffix"));
        }
        let port: u32 = port
            .parse()
            .map_err(|_| invalid("must use a port in 1..=65535"))?;
        if !(1..=65535).contains(&port) {
            return Err(invalid("must use a port in 1..=65535"));
        }
    }
    Ok(())
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
        "api_token",
        "auth_token",
        "bearer_token",
        "client_secret",
        "secret",
        "password",
        "token",
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

fn reject_credential_shaped_values(value: &Value, prefix: &str) -> Result<(), String> {
    match value {
        Value::String(candidate) if looks_like_credential(candidate) => {
            let path = if prefix.is_empty() { "root" } else { prefix };
            Err(format!(
                "viden.toml cannot contain credential-shaped value at `{path}`"
            ))
        }
        Value::Table(table) => {
            for (key, child) in table {
                let path = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{prefix}.{key}")
                };
                reject_credential_shaped_values(child, &path)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for child in values {
                reject_credential_shaped_values(child, prefix)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn looks_like_credential(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    [
        "sk-",
        "sk_",
        "ghp_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "akia",
        "bearer ",
        "-----begin private key-----",
        "-----begin rsa private key-----",
        "-----begin ec private key-----",
    ]
    .iter()
    .any(|prefix| normalized.starts_with(prefix))
}

fn validate_runner_fields(name: &str, runner: &toml::Table) -> Result<(), String> {
    for field in ["bin", "version", "cmd", "scene"] {
        if runner.contains_key(field) {
            required_nonempty_string(runner, field, &format!("runner.{name}"))?;
        }
    }
    for field in ["headless", "realtime"] {
        if runner
            .get(field)
            .is_some_and(|value| value.as_bool().is_none())
        {
            return Err(format!(
                "viden.toml runner.{name}.{field} must be a boolean"
            ));
        }
    }
    if runner
        .get("gpus")
        .is_some_and(|value| value.as_integer().is_none_or(|count| count < 1))
    {
        return Err(format!(
            "viden.toml runner.{name}.gpus must be a positive integer"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{EgressMode, EgressPolicy, OwnershipRule, ToolAllowlists, parse_project_config};

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
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\n[provider]\napi_token = \"sk-live-secret\"\n",
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\n[local]\npath = \"/tmp/secret\"\n",
            "[project]\nname = \"sk-live-secret\"\npack = \"robot-pack\"\n",
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\n[targets]\nlocal = { default = true, token = \"plain-secret\" }\n",
            "[project\n",
        ] {
            assert!(
                parse_project_config(invalid.as_bytes()).is_err(),
                "{invalid}"
            );
        }
    }

    const VALID_POLICY: &str = r#"[project]
name = "boss-rush"
pack = "musegameworkshop"
packs = ["robotics", "web"]

[gates]
"src/**" = { type = "replay-regression", runner = "godot" }

[runner]
godot = { bin = "godot4", headless = true }

[budget]
tokens_per_week = 50000
warn_at = 0.8

[targets]
local = { default = true }

[ownership]
"src/**" = { owner = "core-team", reviewers = ["alice", "bob"] }
"docs/**" = { owner = "docs-team" }

[allowlists]
tools = ["bash", "read", "mcp.fs:read_file"]
mcp = []

[egress]
policy = "allowlist"
allow = ["registry.example.com", "localhost:8080"]
"#;

    /// Builds a `viden.toml` document with a minimal valid `[project]` header
    /// followed by the section under test.
    fn with_project(section: &str) -> String {
        format!("[project]\nname = \"arm\"\npack = \"robot-pack\"\n{section}")
    }

    #[test]
    fn project_config_accepts_all_policy_sections() {
        let parsed = parse_project_config(VALID_POLICY.as_bytes()).expect("valid policy config");
        assert_eq!(parsed.project_name, "boss-rush");
        assert_eq!(parsed.pack, "musegameworkshop");
        assert_eq!(
            parsed.packs,
            vec!["robotics".to_string(), "web".to_string()]
        );
        assert_eq!(
            parsed.ownership,
            vec![
                OwnershipRule {
                    pattern: "docs/**".to_string(),
                    owner: "docs-team".to_string(),
                    reviewers: Vec::new(),
                },
                OwnershipRule {
                    pattern: "src/**".to_string(),
                    owner: "core-team".to_string(),
                    reviewers: vec!["alice".to_string(), "bob".to_string()],
                },
            ],
            "ownership rules are returned in sorted pattern order"
        );
        assert_eq!(
            parsed.allowlists,
            Some(ToolAllowlists {
                tools: Some(vec![
                    "bash".to_string(),
                    "read".to_string(),
                    "mcp.fs:read_file".to_string(),
                ]),
                mcp: Some(Vec::new()),
            })
        );
        assert_eq!(
            parsed.egress,
            Some(EgressPolicy {
                policy: EgressMode::Allowlist,
                allow: vec![
                    "registry.example.com".to_string(),
                    "localhost:8080".to_string(),
                ],
            })
        );
    }

    #[test]
    fn project_config_defaults_absent_policy_sections() {
        let parsed = parse_project_config(VALID.as_bytes()).expect("valid project config");
        assert!(parsed.packs.is_empty());
        assert!(parsed.ownership.is_empty());
        assert_eq!(parsed.allowlists, None);
        assert_eq!(parsed.egress, None);
    }

    #[test]
    fn project_config_rejects_invalid_packs() {
        for invalid in [
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\npacks = [\"a\", \"a\"]\n",
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\npacks = [\"robot-pack\"]\n",
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\npacks = []\n",
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\npacks = [\"a\", 3]\n",
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\npacks = [\"a\", \"\"]\n",
            "[project]\nname = \"arm\"\npack = \"robot-pack\"\npacks = \"a\"\n",
        ] {
            assert!(
                parse_project_config(invalid.as_bytes()).is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn project_config_rejects_invalid_ownership() {
        for invalid in [
            "[ownership]\n",
            "[ownership]\n\"src/**\" = { reviewers = [\"alice\"] }\n",
            "[ownership]\n\"src/**\" = { owner = \"alice\", reviewers = [\"alice\"] }\n",
            "[ownership]\n\"src/**\" = { owner = \"alice\", reviewers = [\"bob\", \"bob\"] }\n",
            "[ownership]\n\"src/**\" = { owner = \"alice\", team = \"core\" }\n",
            "[ownership]\n\"src/**\" = \"alice\"\n",
            "[ownership]\n\"src/**\" = { owner = \"\" }\n",
            "[ownership]\n\"  \" = { owner = \"alice\" }\n",
            // Distinct TOML keys that trim to the same pattern would yield two
            // rules with the same pattern and possibly different owners.
            "[ownership]\n\"src/**\" = { owner = \"alice\" }\n\" src/**\" = { owner = \"bob\" }\n",
        ] {
            let document = with_project(invalid);
            assert!(
                parse_project_config(document.as_bytes()).is_err(),
                "{document}"
            );
        }
    }

    #[test]
    fn project_config_accepts_explicit_empty_allowlists() {
        // An explicit empty array is an intentional "allow none" policy and
        // must survive parsing as `Some(vec![])`, distinct from an absent key.
        let document = with_project("[allowlists]\ntools = []\n");
        let parsed = parse_project_config(document.as_bytes()).expect("empty tool allowlist");
        assert_eq!(
            parsed.allowlists,
            Some(ToolAllowlists {
                tools: Some(Vec::new()),
                mcp: None,
            })
        );
    }

    #[test]
    fn project_config_rejects_invalid_allowlists() {
        for invalid in [
            "[allowlists]\n",
            "[allowlists]\ntools = [\"Bash\"]\n",
            "[allowlists]\ntools = [\"read file\"]\n",
            "[allowlists]\ntools = [\" read\"]\n",
            "[allowlists]\ntools = [\"read\", \"read\"]\n",
            "[allowlists]\ntools = [\"\"]\n",
            "[allowlists]\ntools = [\"read\", 3]\n",
            "[allowlists]\ntools = \"read\"\n",
            "[allowlists]\nshell = [\"bash\"]\n",
            "[allowlists]\nmcp = [\"server/tool\"]\n",
        ] {
            let document = with_project(invalid);
            assert!(
                parse_project_config(document.as_bytes()).is_err(),
                "{document}"
            );
        }
    }

    #[test]
    fn project_config_accepts_every_egress_policy() {
        for (policy, expected) in [
            ("blocked", EgressMode::Blocked),
            ("loopback", EgressMode::Loopback),
            ("unrestricted", EgressMode::Unrestricted),
        ] {
            let document = with_project(&format!("[egress]\npolicy = \"{policy}\"\n"));
            let parsed = parse_project_config(document.as_bytes()).expect("valid egress policy");
            assert_eq!(
                parsed.egress,
                Some(EgressPolicy {
                    policy: expected,
                    allow: Vec::new(),
                })
            );
        }
        let document =
            with_project("[egress]\npolicy = \"allowlist\"\nallow = [\"api.example.com:443\"]\n");
        let parsed = parse_project_config(document.as_bytes()).expect("valid allowlist policy");
        assert_eq!(
            parsed.egress,
            Some(EgressPolicy {
                policy: EgressMode::Allowlist,
                allow: vec!["api.example.com:443".to_string()],
            })
        );
    }

    #[test]
    fn project_config_names_every_supported_egress_policy_on_error() {
        let document = with_project("[egress]\npolicy = \"open\"\n");
        let error = parse_project_config(document.as_bytes()).expect_err("unknown egress policy");
        for expected in ["blocked", "loopback", "allowlist", "unrestricted"] {
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn project_config_rejects_invalid_egress() {
        for invalid in [
            "[egress]\n",
            "[egress]\npolicy = \"open\"\n",
            "[egress]\npolicy = \"\"\n",
            "[egress]\npolicy = \"blocked\"\nallow = [\"a.example.com\"]\n",
            "[egress]\npolicy = \"loopback\"\nallow = []\n",
            "[egress]\npolicy = \"allowlist\"\n",
            "[egress]\npolicy = \"allowlist\"\nallow = []\n",
            "[egress]\npolicy = \"allowlist\"\nmode = \"strict\"\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"https://a.example.com\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"a.example.com/path\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"user@a.example.com\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"A.example.com\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"a .example.com\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\" a.example.com\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"a.example.com:0\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"a.example.com:70000\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"a.example.com:http\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"a.example.com:\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"a.example.com\", \"a.example.com\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [\"\"]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = [3]\n",
            "[egress]\npolicy = \"allowlist\"\nallow = \"a.example.com\"\n",
        ] {
            let document = with_project(invalid);
            assert!(
                parse_project_config(document.as_bytes()).is_err(),
                "{document}"
            );
        }
    }

    #[test]
    fn project_config_rejects_credentials_inside_new_policy_sections() {
        // The document-wide secret/credential guards run before section
        // validation, so they must also cover array entries in new sections.
        for invalid in [
            "[egress]\npolicy = \"allowlist\"\nallow = [\"sk-live-abc\"]\n",
            "[allowlists]\ntools = [\"ghp_abcdef\"]\n",
            "[ownership]\n\"src/**\" = { owner = \"alice\", token = \"plain\" }\n",
        ] {
            let document = with_project(invalid);
            let error = parse_project_config(document.as_bytes())
                .expect_err("credential-bearing policy section must be rejected");
            assert!(
                error.contains("secret") || error.contains("credential"),
                "{error} for {document}"
            );
        }
    }
}
