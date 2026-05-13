use std::fs;
use std::path::{Path, PathBuf};

use robocode_types::{ToolInput, ToolSpec};

use crate::{BuiltinTool, ToolExecutionContext, ToolExecutionOutput, resolve_path};

pub(crate) struct GlobTool;
pub(crate) struct GrepTool;

impl BuiltinTool for GlobTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "glob".to_string(),
            description: "Find files by wildcard pattern".to_string(),
            is_mutating: false,
            input_schema_hint: "pattern=src/*.rs path=optional/base".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let pattern = input
            .get("pattern")
            .ok_or_else(|| "glob requires `pattern`".to_string())?;
        let base = resolve_optional_base(ctx, input)?;
        let mut results = Vec::new();
        walk(&base, &mut |path| {
            let relative = path
                .strip_prefix(&base)
                .unwrap_or(path)
                .display()
                .to_string()
                .replace('\\', "/");
            if wildcard_match(pattern, &relative) {
                results.push(path.display().to_string());
            }
        })?;
        Ok(ToolExecutionOutput {
            output: if results.is_empty() {
                "No matches".to_string()
            } else {
                results.join("\n")
            },
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GrepTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "grep".to_string(),
            description: "Search files for a text pattern".to_string(),
            is_mutating: false,
            input_schema_hint: "pattern=needle path=optional/base".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let pattern = input
            .get("pattern")
            .ok_or_else(|| "grep requires `pattern`".to_string())?;
        let base = resolve_optional_base(ctx, input)?;
        let mut matches = Vec::new();
        walk(&base, &mut |path| {
            if path.is_dir() {
                return;
            }
            if let Ok(contents) = fs::read_to_string(path) {
                for (line_number, line) in contents.lines().enumerate() {
                    if line.contains(pattern) {
                        matches.push(format!(
                            "{}:{}:{}",
                            path.display(),
                            line_number + 1,
                            line.trim()
                        ));
                    }
                }
            }
        })?;
        Ok(ToolExecutionOutput {
            output: if matches.is_empty() {
                "No matches".to_string()
            } else {
                matches.join("\n")
            },
            diff: None,
            success: true,
        })
    }
}

fn resolve_optional_base(ctx: &ToolExecutionContext, input: &ToolInput) -> Result<PathBuf, String> {
    let raw = input.get("path").map(String::as_str).unwrap_or(".");
    let path = resolve_path(&ctx.cwd, raw);
    if !path.exists() {
        return Err(format!("Base path does not exist: {}", path.display()));
    }
    Ok(path)
}

fn walk(root: &Path, f: &mut dyn FnMut(&Path)) -> Result<(), String> {
    f(root);
    if root.is_dir() {
        for entry in fs::read_dir(root).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, f)?;
            } else {
                f(&path);
            }
        }
    }
    Ok(())
}

fn wildcard_match(pattern: &str, candidate: &str) -> bool {
    wildcard_match_inner(pattern.as_bytes(), candidate.as_bytes())
}

fn wildcard_match_inner(pattern: &[u8], candidate: &[u8]) -> bool {
    if pattern.is_empty() {
        return candidate.is_empty();
    }
    match pattern[0] {
        b'*' => {
            wildcard_match_inner(&pattern[1..], candidate)
                || (!candidate.is_empty() && wildcard_match_inner(pattern, &candidate[1..]))
        }
        b'?' => !candidate.is_empty() && wildcard_match_inner(&pattern[1..], &candidate[1..]),
        byte => {
            !candidate.is_empty()
                && byte == candidate[0]
                && wildcard_match_inner(&pattern[1..], &candidate[1..])
        }
    }
}
