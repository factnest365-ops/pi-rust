use anyhow::Result;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default()
    })
}

pub struct WebTool;

impl WebTool {
    pub async fn fetch_url(url: &str, max_length: Option<usize>) -> Result<String> {
        let client = get_http_client();
        let res = client.get(url).send().await?;
        let status = res.status();

        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP error {} fetching {}", status, url));
        }

        let content_type = res
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let raw_text = res.text().await?;

        let parsed_markdown = if content_type.contains("text/html") || raw_text.contains("<html") || raw_text.contains("<body") {
            Self::html_to_markdown(&raw_text)
        } else {
            raw_text
        };

        let max_len = max_length.unwrap_or(32_000);
        if parsed_markdown.len() > max_len {
            let boundary = parsed_markdown.floor_char_boundary(max_len);
            let truncated = &parsed_markdown[..boundary];
            Ok(format!(
                "{}\n\n[... Truncated: {} characters omitted (showing first {} bytes) ...]",
                truncated,
                parsed_markdown.len() - boundary,
                boundary
            ))
        } else {
            Ok(parsed_markdown)
        }
    }

    pub async fn search_web(query: &str, max_results: Option<usize>) -> Result<String> {
        let client = get_http_client();
        let res = client
            .get("https://html.duckduckgo.com/html/")
            .query(&[("q", query)])
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("Search request failed with HTTP {}", status));
        }

        let raw_html = res.text().await?;
        let parsed = Self::html_to_markdown(&raw_html);

        let max_len = max_results.unwrap_or(5) * 800;
        let boundary = parsed.floor_char_boundary(max_len.min(parsed.len()));
        let preview = &parsed[..boundary];

        Ok(format!("### Web Search Results for '{}':\n\n{}", query, preview))
    }

    pub fn execute(args: &Value) -> Result<String> {
        let url = args["url"]
            .as_str()
            .or_else(|| args["link"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' argument"))?;

        let max_length = args["max_length"].as_u64().map(|v| v as usize);

        // Async execution handled in tokio runtime
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(Self::fetch_url(url, max_length))
            }),
            Err(_) => {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(Self::fetch_url(url, max_length))
            }
        }
    }

    pub async fn execute_async(args: &Value) -> Result<String> {
        let url = args["url"]
            .as_str()
            .or_else(|| args["link"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' argument"))?;

        let max_length = args["max_length"].as_u64().map(|v| v as usize);
        Self::fetch_url(url, max_length).await
    }

    pub fn execute_search(args: &Value) -> Result<String> {
        let query = args["query"]
            .as_str()
            .or_else(|| args["q"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' argument"))?;

        let max_results = args["max_results"].as_u64().map(|v| v as usize);

        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(Self::search_web(query, max_results))
            }),
            Err(_) => {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(Self::search_web(query, max_results))
            }
        }
    }

    pub async fn execute_search_async(args: &Value) -> Result<String> {
        let query = args["query"]
            .as_str()
            .or_else(|| args["q"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' argument"))?;

        let max_results = args["max_results"].as_u64().map(|v| v as usize);
        Self::search_web(query, max_results).await
    }

    /// Fast pure-Rust HTML to Markdown converter
    pub fn html_to_markdown(html: &str) -> String {
        let mut cleaned = String::with_capacity(html.len());

        // 1. Remove <script>, <style>, <noscript>, <svg> tags and contents
        let mut in_ignored_block = false;
        let mut ignored_tag = "";

        let mut i = 0;
        let bytes = html.as_bytes();
        let len = bytes.len();

        while i < len {
            if in_ignored_block {
                let close_tag = format!("</{}>", ignored_tag);
                if let Some(pos) = html[i..].to_lowercase().find(&close_tag) {
                    i += pos + close_tag.len();
                    in_ignored_block = false;
                } else {
                    break;
                }
            } else if bytes[i] == b'<' {
                let rest = &html[i..];
                let rest_lower = rest.to_lowercase();
                if rest_lower.starts_with("<script") {
                    in_ignored_block = true;
                    ignored_tag = "script";
                    i += 7;
                } else if rest_lower.starts_with("<style") {
                    in_ignored_block = true;
                    ignored_tag = "style";
                    i += 6;
                } else if rest_lower.starts_with("<noscript") {
                    in_ignored_block = true;
                    ignored_tag = "noscript";
                    i += 9;
                } else if rest_lower.starts_with("<svg") {
                    in_ignored_block = true;
                    ignored_tag = "svg";
                    i += 4;
                } else {
                    // Check standard tags
                    if let Some(tag_end) = rest.find('>') {
                        let tag_content = rest[1..tag_end].trim();
                        let tag_name = tag_content.split_whitespace().next().unwrap_or("").to_lowercase();

                        match tag_name.as_str() {
                            "h1" => cleaned.push_str("\n\n# "),
                            "h2" => cleaned.push_str("\n\n## "),
                            "h3" => cleaned.push_str("\n\n### "),
                            "h4" => cleaned.push_str("\n\n#### "),
                            "h5" => cleaned.push_str("\n\n##### "),
                            "h6" => cleaned.push_str("\n\n###### "),
                            "/h1" | "/h2" | "/h3" | "/h4" | "/h5" | "/h6" => cleaned.push_str("\n\n"),
                            "p" | "/p" => cleaned.push_str("\n\n"),
                            "br" | "br/" | "br /" => cleaned.push('\n'),
                            "li" => cleaned.push_str("\n- "),
                            "/li" => cleaned.push('\n'),
                            "pre" => cleaned.push_str("\n\n```\n"),
                            "/pre" => cleaned.push_str("\n```\n\n"),
                            "code" => cleaned.push('`'),
                            "/code" => cleaned.push('`'),
                            "blockquote" => cleaned.push_str("\n> "),
                            "/blockquote" => cleaned.push_str("\n\n"),
                            "tr" => cleaned.push('\n'),
                            "td" | "th" => cleaned.push_str(" | "),
                            _ => {}
                        }

                        i += tag_end + 1;
                    } else {
                        cleaned.push('<');
                        i += 1;
                    }
                }
            } else {
                let ch = html[i..].chars().next().unwrap_or(' ');
                cleaned.push(ch);
                i += ch.len_utf8();
            }
        }

        // Decode basic HTML entities
        let decoded = cleaned
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&apos;", "'")
            .replace("&#x27;", "'")
            .replace("&ndash;", "–")
            .replace("&mdash;", "—");

        // Clean redundant whitespace and blank lines
        let mut final_lines = Vec::new();
        let mut consecutive_blank = 0;

        for line in decoded.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                consecutive_blank += 1;
                if consecutive_blank <= 2 {
                    final_lines.push("");
                }
            } else {
                consecutive_blank = 0;
                final_lines.push(trimmed);
            }
        }

        final_lines.join("\n").trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_markdown_conversion() {
        let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Test Page</title>
            <style>body { color: red; }</style>
            <script>console.log("ignore me");</script>
        </head>
        <body>
            <h1>Main Title</h1>
            <p>This is a paragraph with <code>inline code</code> and &amp; entities.</p>
            <p>Unicode text with emojis: 🚀 — “smart quotes” and 中文.</p>
            <ul>
                <li>Item 1</li>
                <li>Item 2</li>
            </ul>
            <pre>fn main() { println!("Hello"); }</pre>
        </body>
        </html>
        "#;

        let md = WebTool::html_to_markdown(html);
        assert!(md.contains("# Main Title"));
        assert!(md.contains("This is a paragraph with `inline code` and & entities."));
        assert!(md.contains("Unicode text with emojis: 🚀 — “smart quotes” and 中文."));
        assert!(md.contains("- Item 1"));
        assert!(md.contains("- Item 2"));
        assert!(md.contains("```\nfn main()"));
        assert!(!md.contains("ignore me"));
        assert!(!md.contains("color: red"));
    }
}
