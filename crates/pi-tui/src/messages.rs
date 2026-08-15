use crate::markdown::MarkdownRenderer;
use crate::style::Theme;
use crate::tool_display::ToolDisplay;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

/// Tau (τ) ASCII art logo lines — rendered with a gradient
const TAU_LOGO_LINES: [&str; 6] = [
    "  ████████╗ █████╗ ██╗   ██╗",
    "  ╚══██╔══╝██╔══██╗██║   ██║",
    "     ██║   ███████║██║   ██║",
    "     ██║   ██╔══██║██║   ██║",
    "     ██║   ██║  ██║╚██████╔╝",
    "     ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ",
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
        content.contains("████████╗") || (content.contains("██████╔╝") && content.contains("██║"))
    }

    /// Split model content into thinking/reasoning blocks and final answer content
    pub fn split_thinking_and_content(content: &str) -> (Option<String>, String) {
        let mut thinking = String::new();
        let mut clean_content = String::new();

        let mut remaining = content;

        while !remaining.is_empty() {
            let start_pos = remaining.find("<thinking>").or_else(|| remaining.find("<think>"));
            if let Some(start_idx) = start_pos {
                clean_content.push_str(&remaining[..start_idx]);

                let tag_len = if remaining[start_idx..].starts_with("<thinking>") {
                    "<thinking>".len()
                } else {
                    "<think>".len()
                };

                let after_tag = &remaining[start_idx + tag_len..];
                let end_pos = after_tag.find("</thinking>").or_else(|| after_tag.find("</think>"));

                if let Some(end_idx) = end_pos {
                    thinking.push_str(&after_tag[..end_idx]);
                    let close_tag_len = if after_tag[end_idx..].starts_with("</thinking>") {
                        "</thinking>".len()
                    } else {
                        "</think>".len()
                    };
                    remaining = &after_tag[end_idx + close_tag_len..];
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

    pub fn render_message(role: &str, content: &str, expand_tools: bool, show_thinking: bool) -> Vec<Line<'static>> {
        if role == "tool" && !expand_tools {
            return Vec::new();
        }

        let mut lines = Vec::new();

        match role {
            "user" => {
                lines.push(Line::from(vec![
                    Span::styled("────────────────────────────────────────────────────────────────────────────────", Theme::code_border()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("❯ You", Theme::user_label()),
                ]));
                lines.push(Line::from(""));
                lines.extend(MarkdownRenderer::render(content));
            }
            "pi" => {
                let (thinking_opt, main_content) = Self::split_thinking_and_content(content);

                lines.push(Line::from(vec![
                    Span::styled("────────────────────────────────────────────────────────────────────────────────", Theme::code_border()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("❯ Pi", Theme::assistant_label()),
                ]));
                lines.push(Line::from(""));

                if let Some(thinking) = thinking_opt {
                    if show_thinking {
                        lines.push(Line::from(vec![
                            Span::styled("💭 Thought", Theme::tool_label()),
                        ]));
                        for tline in thinking.lines() {
                            lines.push(Line::from(vec![
                                Span::styled(format!("  {}", tline), Theme::highlight_comment()),
                            ]));
                        }
                        lines.push(Line::from(""));
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled("💭 Thought (Ctrl+T to expand)", Theme::highlight_comment()),
                        ]));
                        lines.push(Line::from(""));
                    }
                }

                if !main_content.is_empty() {
                    lines.extend(MarkdownRenderer::render(&main_content));
                }
            }
            "path" => {
                lines.push(Line::from(vec![
                    Span::styled(content.to_string(), Theme::assistant_label()),
                ]));
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
                    lines.extend(ToolDisplay::render_executing(tool_name, call_id));
                } else if content.starts_with("Tool [") {
                    let tool_name = content
                        .split('[')
                        .nth(1)
                        .and_then(|s| s.split(']').next())
                        .unwrap_or("tool");
                    let is_error = content.contains("error: true");
                    lines.extend(ToolDisplay::render_completed(tool_name, is_error));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled("⚙ Tool Output: ", Theme::tool_label()),
                        Span::styled(content.to_string(), Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }
            // System messages — special handling for logo, context sections, etc.
            _ => {
                if Self::is_tau_logo(content) {
                    // Render logo with gradient coloring per line
                    lines.push(Line::from(""));
                    for (i, logo_line) in TAU_LOGO_LINES.iter().enumerate() {
                        lines.push(Line::from(vec![
                            Span::styled(logo_line.to_string(), Theme::logo_gradient(i)),
                        ]));
                    }
                } else if content.starts_with("[Context]") || content.starts_with("[Skills]") || content.starts_with("[Extensions]") {
                    // Render bracketed section headers in accent blue, content in muted
                    for line in content.lines() {
                        if line.starts_with('[') {
                            lines.push(Line::from(vec![
                                Span::styled(line.to_string(), Theme::system_label()),
                            ]));
                        } else {
                            lines.push(Line::from(vec![
                                Span::styled(format!("  {}", line.trim()), Theme::system_content()),
                            ]));
                        }
                    }
                } else {
                    // Regular system messages — accent for labels, muted for content
                    lines.push(Line::from(vec![
                        Span::styled(content.to_string(), Theme::system_content()),
                    ]));
                }
            }
        }

        lines.push(Line::from(""));
        lines
    }
}
