use super::*;

#[test]
fn parse_duckduckgo_results_extracts_links_and_titles() {
    let html = r#"
    <div class="results">
      <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F">Rust Programming Language</a>
      <a class="result__snippet">Fast and reliable systems programming language.</a>
    </div>
    "#;
    let results = parse_duckduckgo_results(html, 5);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust Programming Language");
    assert_eq!(results[0].url, "https://www.rust-lang.org/");
    assert!(results[0].snippet.contains("systems programming"));
}

#[test]
fn html_to_text_strips_tags_and_entities() {
    let html = r#"
    <html>
      <head><title>Test</title><style>.x { color: red; }</style></head>
      <body><h1>Hello &amp; Welcome</h1><p>Rust &quot;rocks&quot;.</p></body>
    </html>
    "#;
    let text = html_to_text(html, 10_000);
    assert!(text.contains("Hello & Welcome"));
    assert!(text.contains("Rust \"rocks\"."));
    assert!(!text.contains("<h1>"));
}

#[test]
fn url_encode_escapes_spaces_and_symbols() {
    assert_eq!(url_encode("rust cli"), "rust+cli");
    assert_eq!(url_encode("site:docs.rs tokio"), "site%3Adocs.rs+tokio");
}
