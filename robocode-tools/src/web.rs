use std::process::Command;

use robocode_types::{ToolInput, ToolSpec};

use crate::{BuiltinTool, ToolExecutionContext, ToolExecutionOutput};

pub(crate) struct WebSearchTool;
pub(crate) struct WebFetchTool;

impl BuiltinTool for WebSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_search".to_string(),
            description: "Search the web and return the top results".to_string(),
            is_mutating: false,
            input_schema_hint: "query='rust http client' limit=5 site=optional/domain".to_string(),
        }
    }

    fn run(
        &self,
        _ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let query = input
            .get("query")
            .ok_or_else(|| "web_search requires `query`".to_string())?;
        let limit = input
            .get("limit")
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(5)
            .clamp(1, 10);
        let site = input.get("site").map(String::as_str);
        let scoped_query = if let Some(site) = site {
            format!("site:{site} {query}")
        } else {
            query.clone()
        };
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            url_encode(&scoped_query)
        );
        let html = fetch_url(&url, 30)?;
        let results = parse_duckduckgo_results(&html, limit);
        Ok(ToolExecutionOutput {
            output: if results.is_empty() {
                "No search results found.".to_string()
            } else {
                results
                    .iter()
                    .enumerate()
                    .map(|(index, result)| {
                        format!(
                            "{}. {}\n   {}\n   {}",
                            index + 1,
                            result.title,
                            result.url,
                            result.snippet
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for WebFetchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_fetch".to_string(),
            description: "Fetch a web page and return extracted text".to_string(),
            is_mutating: false,
            input_schema_hint: "url=https://example.com max_bytes=20000 raw=false".to_string(),
        }
    }

    fn run(
        &self,
        _ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let url = input
            .get("url")
            .ok_or_else(|| "web_fetch requires `url`".to_string())?;
        let max_bytes = input
            .get("max_bytes")
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(20_000);
        let raw = input
            .get("raw")
            .map(|value| value == "true")
            .unwrap_or(false);
        let response = fetch_url(url, 30)?;
        let output = if raw {
            truncate_bytes(&response, max_bytes)
        } else {
            html_to_text(&response, max_bytes)
        };
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchResult {
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) snippet: String,
}

fn fetch_url(url: &str, timeout_secs: u64) -> Result<String, String> {
    let output = Command::new("curl")
        .arg("--location")
        .arg("--silent")
        .arg("--show-error")
        .arg("--max-time")
        .arg(timeout_secs.to_string())
        .arg("--user-agent")
        .arg("RoboCode/0.1 (+https://github.com/wikieden/robocode)")
        .arg(url)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("curl failed with status {}", output.status)
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(crate) fn url_encode(input: &str) -> String {
    let mut output = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char)
            }
            b' ' => output.push('+'),
            other => output.push_str(&format!("%{:02X}", other)),
        }
    }
    output
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &input[index + 1..index + 3];
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    output.push(value as char);
                    index += 3;
                } else {
                    output.push('%');
                    index += 1;
                }
            }
            byte => {
                output.push(byte as char);
                index += 1;
            }
        }
    }
    output
}

pub(crate) fn parse_duckduckgo_results(html: &str, limit: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut offset = 0;
    while results.len() < limit {
        let Some(anchor_start_rel) = html[offset..].find("result__a") else {
            break;
        };
        let anchor_start = offset + anchor_start_rel;
        let href_search_start = html[..anchor_start].rfind("<a").unwrap_or(anchor_start);
        let Some(href_rel) = html[href_search_start..].find("href=\"") else {
            offset = anchor_start + 8;
            continue;
        };
        let href_start = href_search_start + href_rel + 6;
        let Some(href_end_rel) = html[href_start..].find('"') else {
            break;
        };
        let href_end = href_start + href_end_rel;
        let raw_href = &html[href_start..href_end];
        let Some(title_end_rel) = html[href_end..].find("</a>") else {
            break;
        };
        let title_end = href_end + title_end_rel;
        let title_html = html[href_end + 2..title_end].trim_start_matches('>');
        let title = clean_html_fragment(title_html);
        let snippet = extract_result_snippet(&html[title_end..]);
        let url = normalize_search_result_url(raw_href);
        if !title.is_empty() && !url.is_empty() {
            results.push(SearchResult {
                title,
                url,
                snippet,
            });
        }
        offset = title_end + 4;
    }
    results
}

fn extract_result_snippet(html: &str) -> String {
    let Some(snippet_rel) = html.find("result__snippet") else {
        return String::new();
    };
    let snippet_start = snippet_rel;
    let Some(tag_end_rel) = html[snippet_start..].find('>') else {
        return String::new();
    };
    let content_start = snippet_start + tag_end_rel + 1;
    let Some(content_end_rel) = html[content_start..].find("</") else {
        return String::new();
    };
    clean_html_fragment(&html[content_start..content_start + content_end_rel])
}

fn normalize_search_result_url(raw_href: &str) -> String {
    if let Some(uddg_index) = raw_href.find("uddg=") {
        let encoded = &raw_href[uddg_index + 5..];
        let encoded = encoded.split('&').next().unwrap_or(encoded);
        return percent_decode(encoded);
    }
    percent_decode(raw_href)
}

pub(crate) fn html_to_text(html: &str, max_bytes: usize) -> String {
    let stripped = strip_html_tags(&remove_html_noise(html));
    let decoded = decode_html_entities(&stripped);
    let normalized = normalize_whitespace(&decoded);
    truncate_bytes(&normalized, max_bytes)
}

fn remove_html_noise(html: &str) -> String {
    let without_script = remove_tag_block(html, "script");
    let without_style = remove_tag_block(&without_script, "style");
    remove_tag_block(&without_style, "noscript")
}

fn remove_tag_block(input: &str, tag: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    let start_marker = format!("<{tag}");
    let end_marker = format!("</{tag}>");
    while let Some(start_rel) = input[cursor..].to_ascii_lowercase().find(&start_marker) {
        let start = cursor + start_rel;
        output.push_str(&input[cursor..start]);
        if let Some(end_rel) = input[start..].to_ascii_lowercase().find(&end_marker) {
            cursor = start + end_rel + end_marker.len();
        } else {
            cursor = input.len();
        }
    }
    output.push_str(&input[cursor..]);
    output
}

fn strip_html_tags(input: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn normalize_whitespace(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn clean_html_fragment(input: &str) -> String {
    normalize_whitespace(&decode_html_entities(&strip_html_tags(input)))
}

fn truncate_bytes(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }
    let mut output = String::new();
    for ch in input.chars() {
        if output.len() + ch.len_utf8() > max_bytes.saturating_sub(3) {
            break;
        }
        output.push(ch);
    }
    output.push_str("...");
    output
}
