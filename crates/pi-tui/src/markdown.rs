use crate::mermaid::MermaidRenderer;
use crate::style::Theme;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// Render markdown text into styled Ratatui lines
    pub fn render(markdown: &str) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let mut in_code_block = false;
        let mut code_lang = String::new();
        let mut code_lines: Vec<String> = Vec::new();

        for raw_line in markdown.lines() {
            let trimmed = raw_line.trim();

            if trimmed.starts_with("```") {
                if in_code_block {
                    // Closing code block
                    if code_lang == "mermaid" {
                        let mermaid_code = code_lines.join("\n");
                        lines.extend(MermaidRenderer::render(&mermaid_code));
                    } else {
                        let label = if code_lang.is_empty() { "code" } else { &code_lang };
                        lines.push(Line::from(vec![
                            Span::styled(format!(" ── [{}] ──", label), Theme::code_border()),
                        ]));
                        for cl in &code_lines {
                            lines.push(Self::highlight_code_line(cl, &code_lang));
                        }
                        lines.push(Line::from(Span::styled(" ────────────", Theme::code_border())));
                    }
                    in_code_block = false;
                    code_lang.clear();
                    code_lines.clear();
                } else {
                    // Opening code block
                    in_code_block = true;
                    code_lang = trimmed.trim_start_matches('`').trim().to_lowercase();
                    code_lines.clear();
                }
                continue;
            }

            if in_code_block {
                code_lines.push(raw_line.to_string());
                continue;
            }

            // Headings
            if let Some(rest) = raw_line.strip_prefix("### ") {
                lines.push(Line::from(Span::styled(
                    format!("■ {}", rest.trim()),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
                continue;
            }
            if let Some(rest) = raw_line.strip_prefix("## ") {
                lines.push(Line::from(Span::styled(
                    format!("◆ {}", rest.trim()),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )));
                continue;
            }
            if let Some(rest) = raw_line.strip_prefix("# ") {
                lines.push(Line::from(Span::styled(
                    format!("● {}", rest.trim()),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )));
                continue;
            }

            // Blockquotes
            if let Some(rest) = raw_line.strip_prefix("> ") {
                lines.push(Line::from(vec![
                    Span::styled(" │ ", Style::default().fg(Color::Cyan)),
                    Span::styled(rest.to_string(), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
                ]));
                continue;
            }

            // Bullet points
            if let Some(rest) = raw_line.strip_prefix("- ").or_else(|| raw_line.strip_prefix("* ")) {
                let mut spans = vec![Span::styled("  • ", Style::default().fg(Color::Cyan))];
                spans.extend(Self::parse_inline_spans(rest));
                lines.push(Line::from(spans));
                continue;
            }

            // Standard line with inline formatting
            lines.push(Line::from(Self::parse_inline_spans(raw_line)));
        }

        // Clean up unclosed code block if in streaming state
        if in_code_block {
            if code_lang == "mermaid" {
                let mermaid_code = code_lines.join("\n");
                lines.extend(MermaidRenderer::render(&mermaid_code));
            } else {
                let label = if code_lang.is_empty() { "code (streaming...)" } else { &code_lang };
                lines.push(Line::from(vec![
                    Span::styled(format!(" ── [{}] ──", label), Theme::code_border()),
                ]));
                for cl in &code_lines {
                    lines.push(Self::highlight_code_line(cl, &code_lang));
                }
            }
        }

        lines
    }

    fn parse_inline_spans(text: &str) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        let mut i = 0;
        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut curr = String::new();

        while i < len {
            // Bold: **text**
            if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'*' {
                if !curr.is_empty() {
                    spans.push(Span::styled(curr.clone(), Style::default().fg(Color::White)));
                    curr.clear();
                }
                if let Some(end) = text[i + 2..].find("**") {
                    let bold_text = &text[i + 2..i + 2 + end];
                    spans.push(Span::styled(bold_text.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
                    i += end + 4;
                    continue;
                }
            }

            // Inline code: `code`
            if bytes[i] == b'`' {
                if !curr.is_empty() {
                    spans.push(Span::styled(curr.clone(), Style::default().fg(Color::White)));
                    curr.clear();
                }
                if let Some(end) = text[i + 1..].find('`') {
                    let code_text = &text[i + 1..i + 1 + end];
                    spans.push(Span::styled(format!("`{}`", code_text), Style::default().fg(Color::Yellow).bg(Color::Rgb(30, 30, 30))));
                    i += end + 2;
                    continue;
                }
            }

            curr.push(text[i..].chars().next().unwrap_or(' '));
            i += text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        }

        if !curr.is_empty() {
            spans.push(Span::styled(curr, Style::default().fg(Color::White)));
        }

        if spans.is_empty() {
            spans.push(Span::raw(""));
        }

        spans
    }

    pub fn highlight_code_line(line: &str, _lang: &str) -> Line<'static> {
        let mut spans = Vec::new();
        let trimmed = line.trim_start();
        let indent_len = line.len() - trimmed.len();
        if indent_len > 0 {
            spans.push(Span::raw(" ".repeat(indent_len)));
        }

        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            spans.push(Span::styled(trimmed.to_string(), Theme::highlight_comment()));
            return Line::from(spans);
        }

        let words: Vec<&str> = trimmed
            .split_inclusive(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '{' || c == '}' || c == '[' || c == ']' || c == ';' || c == ',' || c == ':')
            .collect();

        for word in words {
            let clean = word.trim_matches(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '{' || c == '}' || c == '[' || c == ']' || c == ';' || c == ',' || c == ':');
            let style = match clean {
                "fn" | "let" | "mut" | "pub" | "struct" | "enum" | "trait" | "impl" | "async" | "await" | "match" | "if" | "else" | "return" | "use" | "mod" | "type" | "const" | "static" | "self" | "Self" | "def" | "class" | "import" | "from" | "function" | "export" => {
                    Theme::highlight_keyword()
                }
                "String" | "str" | "i32" | "i64" | "u32" | "u64" | "usize" | "f32" | "f64" | "bool" | "Option" | "Result" | "Vec" | "Some" | "None" | "Ok" | "Err" | "true" | "false" => {
                    Theme::highlight_type()
                }
                s if s.starts_with('"') || s.ends_with('"') || s.starts_with('\'') || s.ends_with('\'') => {
                    Theme::highlight_string()
                }
                s if s.chars().all(|c| c.is_ascii_digit() || c == '_') && !s.is_empty() => {
                    Theme::highlight_number()
                }
                _ => Style::default().fg(Color::White),
            };
            spans.push(Span::styled(word.to_string(), style));
        }

        Line::from(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_renderer_headings_and_code() {
        let md = "# Title\n## Subtitle\n```rust\nfn main() {}\n```";
        let lines = MarkdownRenderer::render(md);
        assert_eq!(lines.len(), 5);
    }
}
