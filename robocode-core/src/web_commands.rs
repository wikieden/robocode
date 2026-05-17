use super::*;
use robocode_types::ApprovalResponse;

impl SessionEngine {
    pub(super) fn handle_web_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let Some(subcommand) = args.first().map(String::as_str) else {
            return Ok(self.render_web_help());
        };
        match subcommand {
            "help" => Ok(self.render_web_help()),
            "search" => {
                let mut limit = None;
                let mut site = None;
                let mut query_parts = Vec::new();
                let mut iter = args.iter().skip(1);
                while let Some(arg) = iter.next() {
                    match arg.as_str() {
                        "--limit" => {
                            limit = Some(iter.next().cloned().ok_or_else(|| {
                                "Usage: /web search <query> [--limit <n>] [--site <domain>]"
                                    .to_string()
                            })?);
                        }
                        "--site" => {
                            site = Some(iter.next().cloned().ok_or_else(|| {
                                "Usage: /web search <query> [--limit <n>] [--site <domain>]"
                                    .to_string()
                            })?);
                        }
                        other if other.starts_with("--limit=") => {
                            limit = Some(other.trim_start_matches("--limit=").to_string());
                        }
                        other if other.starts_with("--site=") => {
                            site = Some(other.trim_start_matches("--site=").to_string());
                        }
                        other => query_parts.push(other.to_string()),
                    }
                }
                if query_parts.is_empty() {
                    return Err(
                        "Usage: /web search <query> [--limit <n>] [--site <domain>]".to_string()
                    );
                }
                let mut input = robocode_types::ToolInput::new();
                input.insert("query".to_string(), query_parts.join(" "));
                if let Some(limit) = limit {
                    input.insert("limit".to_string(), limit);
                }
                if let Some(site) = site {
                    input.insert("site".to_string(), site);
                }
                self.run_named_tool("web_search", input, approver)
            }
            "fetch" => {
                let url = args.get(1).cloned().ok_or_else(|| {
                    "Usage: /web fetch <url> [--max-bytes <n>] [--raw]".to_string()
                })?;
                let mut max_bytes = None;
                let mut raw = false;
                let mut iter = args.iter().skip(2);
                while let Some(arg) = iter.next() {
                    match arg.as_str() {
                        "--raw" => raw = true,
                        "--max-bytes" => {
                            max_bytes = Some(iter.next().cloned().ok_or_else(|| {
                                "Usage: /web fetch <url> [--max-bytes <n>] [--raw]".to_string()
                            })?);
                        }
                        other if other.starts_with("--max-bytes=") => {
                            max_bytes = Some(other.trim_start_matches("--max-bytes=").to_string());
                        }
                        _ => {}
                    }
                }
                let mut input = robocode_types::ToolInput::new();
                input.insert("url".to_string(), url);
                if let Some(max_bytes) = max_bytes {
                    input.insert("max_bytes".to_string(), max_bytes);
                }
                if raw {
                    input.insert("raw".to_string(), "true".to_string());
                }
                self.run_named_tool("web_fetch", input, approver)
            }
            _ => Ok(format!(
                "Unknown web subcommand `{subcommand}`.\n\n{}",
                self.render_web_help()
            )),
        }
    }
}
