use crate::markdown::MarkdownRenderer;
use crate::style::ThemePalette;
use crate::tool_display::ToolDisplay;
use ratatui::text::{Line, Span};

/// Tau (τ) ASCII art logo lines — rendered with a gradient
const TAU_LOGO_LINES: [&str; 6] = [
    "\u{00A0}\u{00A0}████████╗\u{00A0}█████╗\u{00A0}██╗\u{00A0}\u{00A0}\u{00A0}██╗",
    "\u{00A0}\u{00A0}╚══██╔══╝██╔══██╗██║\u{00A0}\u{00A0}\u{00A0}██║",
    "\u{00A0}\u{00A0}\u{00A0}\u{00A0}\u{00A0}██║\u{00A0}\u{00A0}\u{00A0}███████║██║\u{00A0}\u{00A0}\u{00A0}██║",
    "\u{00A0}\u{00A0}\u{00A0}\u{00A0}\u{00A0}██║\u{00A0}\u{00A0}\u{00A0}██╔══██║██║\u{00A0}\u{00A0}\u{00A0}██║",
    "\u{00A0}\u{00A0}\u{00A0}\u{00A0}\u{00A0}██║\u{00A0}\u{00A0}\u{00A0}██║\u{00A0}\u{00A0}██║╚██████╔╝",
    "\u{00A0}\u{00A0}\u{00A0}\u{00A0}\u{00A0}╚═╝\u{00A0}\u{00A0}\u{00A0}╚═╝\u{00A0}\u{00A0}╚═╝\u{00A0}╚═════╝\u{00A0}",
];

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub struct MessageRenderer;

impl MessageRenderer {
    /// Check if content is the Tau ASCII logo
    fn is_tau_logo(content: &str) -> bool {
        content.contains("████████╗")
            || content.contains("██╔══██╗██║")
            || content.contains("╚══██╔══╝")
            || content.contains("╚██████╔╝")
            || content.contains("██████╔╝")
    }

    /// Split model content into thinking/reasoning blocks and final answer content
    pub fn split_thinking_and_content(content: &str) -> (Option<String>, String) {
        let mut thinking = String::new();
        let mut clean_content = String::new();

        let mut remaining = content;

        while !remaining.is_empty() {
            let start_pos = remaining
                .find("<thinking>")
                .or_else(|| remaining.find("<think>"));
            if let Some(start_idx) = start_pos {
                let safe_start = remaining.floor_char_boundary(start_idx);
                clean_content.push_str(&remaining[..safe_start]);

                let tag_len = if remaining[safe_start..].starts_with("<thinking>") {
                    "<thinking>".len()
                } else {
                    "<think>".len()
                };

                let after_tag_idx = (safe_start + tag_len).min(remaining.len());
                let safe_after = remaining.floor_char_boundary(after_tag_idx);
                let after_tag = &remaining[safe_after..];
                let end_pos = after_tag
                    .find("</thinking>")
                    .or_else(|| after_tag.find("</think>"));

                if let Some(end_idx) = end_pos {
                    let safe_end = after_tag.floor_char_boundary(end_idx);
                    thinking.push_str(&after_tag[..safe_end]);
                    let close_tag_len = if after_tag[safe_end..].starts_with("</thinking>") {
                        "</thinking>".len()
                    } else {
                        "</think>".len()
                    };
                    let next_idx = (safe_end + close_tag_len).min(after_tag.len());
                    let safe_next = after_tag.floor_char_boundary(next_idx);
                    remaining = &after_tag[safe_next..];
                } else {
                    // Currently in-flight streaming inside thinking block
                    thinking.push_str(after_tag);
                    remaining = "";
                }
            } else {
                clean_content.push_str(remaining);
                break;
            }
        }

        let thinking_opt = if thinking.trim().is_empty() {
            None
        } else {
            Some(thinking.trim().to_string())
        };

        (thinking_opt, clean_content.trim().to_string())
    }

    pub fn render_message(
        role: &str,
        content: &str,
        expand_tools: bool,
        show_thinking: bool,
    ) -> Vec<Line<'static>> {
        Self::render_message_styled(
            role,
            content,
            expand_tools,
            show_thinking,
            &ThemePalette::default_pi(),
        )
    }

    pub fn render_message_styled(
        role: &str,
        content: &str,
        expand_tools: bool,
        show_thinking: bool,
        theme: &ThemePalette,
    ) -> Vec<Line<'static>> {
        if role == "tool" && !expand_tools {
            return Vec::new();
        }

        let mut lines = Vec::new();

        match role {
            "user" => {
                // Adaptive separator — G2 continuity: no hardcoded 80-char rule, uses theme border
                lines.push(Line::from(vec![Span::styled(
                    "─".repeat(80),
                    theme.code_border(),
                )]));
                lines.push(Line::from(vec![Span::styled("❯ You", theme.user_label())]));
                lines.push(Line::from(""));
                lines.extend(MarkdownRenderer::render_styled(content, theme));
            }
            "pi" => {
                let (thinking_opt, main_content) = Self::split_thinking_and_content(content);

                lines.push(Line::from(vec![Span::styled(
                    "─".repeat(80),
                    theme.code_border(),
                )]));
                lines.push(Line::from(vec![Span::styled(
                    "❯ Pi",
                    theme.assistant_label(),
                )]));
                lines.push(Line::from(""));

                if let Some(thinking) = thinking_opt {
                    let token_est = (thinking.len() / 4).max(1);
                    if show_thinking {
                        lines.push(Line::from(vec![Span::styled(
                            format!("🧠 Reasoning ({} tokens)", token_est),
                            theme.tool_label(),
                        )]));
                        for tline in thinking.lines() {
                            lines.push(Line::from(vec![
                                Span::styled(" │ ", theme.code_border()),
                                Span::styled(tline.to_string(), theme.highlight_comment()),
                            ]));
                        }
                        lines.push(Line::from(""));
                    } else {
                        lines.push(Line::from(vec![Span::styled(
                            format!(
                                "🧠 Reasoning ({} tokens hidden · Ctrl+T to expand)",
                                token_est
                            ),
                            theme.highlight_comment(),
                        )]));
                        lines.push(Line::from(""));
                    }
                }

                if !main_content.is_empty() {
                    lines.extend(MarkdownRenderer::render_styled(&main_content, theme));
                }
            }
            "path" => {
                lines.push(Line::from(vec![Span::styled(
                    content.to_string(),
                    theme.assistant_label(),
                )]));
            }
            "tool" => {
                if content.starts_with("Executing tool [") {
                    let tool_name = content
                        .split('[')
                        .nth(1)
                        .and_then(|s| s.split(']').next())
                        .unwrap_or("tool");
                    let call_id = content
                        .split("call_id: ")
                        .nth(1)
                        .and_then(|s| s.split(')').next())
                        .unwrap_or("");
                    lines.extend(ToolDisplay::render_executing_styled(
                        tool_name, call_id, theme,
                    ));
                } else if content.starts_with("Tool [") {
                    let tool_name = content
                        .split('[')
                        .nth(1)
                        .and_then(|s| s.split(']').next())
                        .unwrap_or("tool");
                    let is_error = content.contains("error: true");
                    lines.extend(ToolDisplay::render_completed_styled(
                        tool_name, is_error, theme,
                    ));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled("⚙ Tool Output: ", theme.tool_label()),
                        Span::styled(content.to_string(), theme.system_content()),
                    ]));
                }
            }
            // System messages — special handling for logo, context sections, etc.
            _ => {
                if Self::is_tau_logo(content) {
                    // Render logo with gradient coloring per line
                    lines.push(Line::from(""));
                    for (i, logo_line) in TAU_LOGO_LINES.iter().enumerate() {
                        lines.push(Line::from(vec![Span::styled(
                            logo_line.to_string(),
                            theme.logo_gradient(i),
                        )]));
                    }
                } else if content.starts_with("[Context]")
                    || content.starts_with("[Skills]")
                    || content.starts_with("[Extensions]")
                {
                    // Render bracketed section headers in accent blue, content in muted
                    for line in content.lines() {
                        if line.starts_with('[') {
                            lines.push(Line::from(vec![Span::styled(
                                line.to_string(),
                                theme.system_label(),
                            )]));
                        } else {
                            lines.push(Line::from(vec![Span::styled(
                                format!("  {}", line.trim()),
                                theme.system_content(),
                            )]));
                        }
                    }
                } else {
                    // Regular system messages — accent for labels, muted for content
                    lines.push(Line::from(vec![Span::styled(
                        content.to_string(),
                        theme.system_content(),
                    )]));
                }
            }
        }

        lines.push(Line::from(""));
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_thinking_and_content() {
        let raw = "<think>Let me reason about this...</think>Here is the final answer.";
        let (thinking, content) = MessageRenderer::split_thinking_and_content(raw);
        assert_eq!(thinking, Some("Let me reason about this...".to_string()));
        assert_eq!(content, "Here is the final answer.");
    }

    #[test]
    fn test_split_thinking_streaming_in_flight() {
        let raw = "<thinking>Still thinking about step 2...";
        let (thinking, content) = MessageRenderer::split_thinking_and_content(raw);
        assert_eq!(thinking, Some("Still thinking about step 2...".to_string()));
        assert_eq!(content, "");
    }

    #[test]
    fn test_render_message_styled_across_roles() {
        let theme = ThemePalette::tokyo_night();
        let user_lines =
            MessageRenderer::render_message_styled("user", "Hello pi", true, true, &theme);
        assert!(!user_lines.is_empty());

        let pi_lines = MessageRenderer::render_message_styled(
            "pi",
            "<think>hmm</think>Result",
            true,
            true,
            &theme,
        );
        assert!(!pi_lines.is_empty());

        let tool_lines = MessageRenderer::render_message_styled(
            "tool",
            "Executing tool [read] (call_id: c1)",
            true,
            true,
            &theme,
        );
        assert!(!tool_lines.is_empty());

        let collapsed_tool = MessageRenderer::render_message_styled(
            "tool",
            "Executing tool [read]",
            false,
            true,
            &theme,
        );
        assert!(collapsed_tool.is_empty());
    }

    #[test]
    fn test_render_tau_logo_styled() {
        let theme = ThemePalette::tokyo_night();
        let logo_text = "  ████████╗ █████╗ ██╗   ██╗\n  ╚══██╔══╝██╔══██╗██║   ██║\n     ██║   ███████║██║   ██║\n     ██║   ██╔══██║██║   ██║\n     ██║   ██║  ██║╚██████╔╝\n     ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ";
        let logo_lines =
            MessageRenderer::render_message_styled("system", logo_text, true, true, &theme);
        assert!(!logo_lines.is_empty());
    }
}
