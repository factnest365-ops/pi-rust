use crate::mermaid::MermaidRenderer;
use crate::style::ThemePalette;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// Render markdown text into styled Ratatui lines using the active theme
    pub fn render_styled(markdown: &str, theme: &ThemePalette) -> Vec<Line<'static>> {
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
                            Span::styled(format!(" ── [{}] ──", label), theme.code_border()),
                        ]));
                        for cl in &code_lines {
                            lines.push(Self::highlight_code_line_styled(cl, &code_lang, theme));
                        }
                        lines.push(Line::from(Span::styled(" ────────────", theme.code_border())));
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
                    Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD),
                )));
                continue;
            }
            if let Some(rest) = raw_line.strip_prefix("## ") {
                lines.push(Line::from(Span::styled(
                    format!("◆ {}", rest.trim()),
                    Style::default().fg(theme.cyan).add_modifier(Modifier::BOLD),
                )));
                continue;
            }
            if let Some(rest) = raw_line.strip_prefix("# ") {
                lines.push(Line::from(Span::styled(
                    format!("● {}", rest.trim()),
                    Style::default().fg(theme.green).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )));
                continue;
            }

            // Blockquotes
            if let Some(rest) = raw_line.strip_prefix("> ") {
                lines.push(Line::from(vec![
                    Span::styled(" │ ", Style::default().fg(theme.cyan)),
                    Span::styled(rest.to_string(), Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC)),
                ]));
                continue;
            }

            // Bullet points
            if let Some(rest) = raw_line.strip_prefix("- ").or_else(|| raw_line.strip_prefix("* ")) {
                let mut spans = vec![Span::styled("  • ", Style::default().fg(theme.cyan))];
                spans.extend(Self::parse_inline_spans_styled(rest, theme));
                lines.push(Line::from(spans));
                continue;
            }

            // Standard line with inline formatting
            lines.push(Line::from(Self::parse_inline_spans_styled(raw_line, theme)));
        }

        // Clean up unclosed code block if in streaming state
        if in_code_block {
            if code_lang == "mermaid" {
                let mermaid_code = code_lines.join("\n");
                lines.extend(MermaidRenderer::render(&mermaid_code));
            } else {
                let label = if code_lang.is_empty() { "code (streaming...)" } else { &code_lang };
                lines.push(Line::from(vec![
                    Span::styled(format!(" ── [{}] ──", label), theme.code_border()),
                ]));
                for cl in &code_lines {
                    lines.push(Self::highlight_code_line_styled(cl, &code_lang, theme));
                }
            }
        }

        lines
    }

    pub fn parse_inline_spans(text: &str) -> Vec<Span<'static>> {
        Self::parse_inline_spans_styled(text, &ThemePalette::default_pi())
    }

    pub fn parse_inline_spans_styled(text: &str, theme: &ThemePalette) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        let mut i = 0;
        let mut curr = String::new();

        while i < text.len() {
            let rem = &text[i..];

            // Bold: **text**
            if let Some(stripped) = rem.strip_prefix("**")
                && let Some(end) = stripped.find("**")
            {
                if !curr.is_empty() {
                    spans.push(Span::styled(curr.clone(), Style::default().fg(theme.text)));
                    curr.clear();
                }
                let bold_text = &stripped[..end];
                spans.push(Span::styled(
                    bold_text.to_string(),
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ));
                i += 4 + end;
                continue;
            }

            // Inline code: `code`
            if let Some(stripped) = rem.strip_prefix('`')
                && let Some(end) = stripped.find('`')
            {
                if !curr.is_empty() {
                    spans.push(Span::styled(curr.clone(), Style::default().fg(theme.text)));
                    curr.clear();
                }
                let code_text = &stripped[..end];
                spans.push(Span::styled(
                    format!("`{}`", code_text),
                    Style::default().fg(theme.yellow).bg(theme.surface),
                ));
                i += 2 + end;
                continue;
            }

            if let Some(c) = rem.chars().next() {
                curr.push(c);
                i += c.len_utf8();
            } else {
                break;
            }
        }

        if !curr.is_empty() {
            spans.push(Span::styled(curr, Style::default().fg(theme.text)));
        }

        if spans.is_empty() {
            spans.push(Span::raw(""));
        }

        spans
    }

    pub fn render(markdown: &str) -> Vec<Line<'static>> {
        Self::render_styled(markdown, &ThemePalette::default_pi())
    }

    pub fn highlight_code_line(line: &str, lang: &str) -> Line<'static> {
        Self::highlight_code_line_styled(line, lang, &ThemePalette::default_pi())
    }

    pub fn highlight_code_line_styled(line: &str, _lang: &str, theme: &ThemePalette) -> Line<'static> {
        let mut spans = Vec::new();
        let trimmed = line.trim_start();
        let indent_len = line.len() - trimmed.len();
        if indent_len > 0 {
            spans.push(Span::raw(" ".repeat(indent_len)));
        }

        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            spans.push(Span::styled(trimmed.to_string(), theme.highlight_comment()));
            return Line::from(spans);
        }

        let words: Vec<&str> = trimmed
            .split_inclusive(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '{' || c == '}' || c == '[' || c == ']' || c == ';' || c == ',' || c == ':')
            .collect();

        for word in words {
            let clean = word.trim_matches(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '{' || c == '}' || c == '[' || c == ']' || c == ';' || c == ',' || c == ':');
            let style = match clean {
                "fn" | "let" | "mut" | "pub" | "struct" | "enum" | "trait" | "impl" | "async" | "await" | "match" | "if" | "else" | "return" | "use" | "mod" | "type" | "const" | "static" | "self" | "Self" | "def" | "class" | "import" | "from" | "function" | "export" => {
                    theme.highlight_keyword()
                }
                "String" | "str" | "i32" | "i64" | "u32" | "u64" | "usize" | "f32" | "f64" | "bool" | "Option" | "Result" | "Vec" | "Some" | "None" | "Ok" | "Err" | "true" | "false" => {
                    theme.highlight_type()
                }
                s if s.starts_with('"') || s.ends_with('"') || s.starts_with('\'') || s.ends_with('\'') => {
                    theme.highlight_string()
                }
                s if s.chars().all(|c| c.is_ascii_digit() || c == '_') && !s.is_empty() => {
                    theme.highlight_number()
                }
                _ => Style::default().fg(theme.text),
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
        let md = "# Title\n## Subtitle\n### Section\n> quote\n- item\n```rust\nfn main() {}\n```";
        let lines = MarkdownRenderer::render(md);
        assert_eq!(lines.len(), 8);
    }

    #[test]
    fn test_markdown_renderer_styled_themes() {
        for kind in crate::style::ThemeKind::ALL {
            let palette = ThemePalette::from_kind(*kind);
            let md = "# Title\n**bold text** and `code`\n```python\ndef hello(): pass\n```";
            let lines = MarkdownRenderer::render_styled(md, &palette);
            assert!(!lines.is_empty());
        }
    }

    #[test]
    fn test_markdown_unicode_and_emojis() {
        let md = "# 🚀 Title with emoji\n**🦀 Rust 2024** and `let x = \"✨\";`\n- 🎯 Bullet point";
        let lines = MarkdownRenderer::render(md);
        assert_eq!(lines.len(), 3);
    }
}

