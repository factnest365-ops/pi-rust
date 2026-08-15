use crate::style::Theme;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub struct ToolDisplay;

impl ToolDisplay {
    pub fn render_executing(tool_name: &str, tool_call_id: &str) -> Vec<Line<'static>> {
        vec![
            Line::from(vec![
                Span::styled("⚙ ", Style::default().fg(Color::Yellow)),
                Span::styled("Executing Tool: ", Theme::tool_label()),
                Span::styled(tool_name.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" (call_id: {})", tool_call_id), Style::default().fg(Color::DarkGray)),
            ]),
        ]
    }

    pub fn render_completed(tool_name: &str, is_error: bool) -> Vec<Line<'static>> {
        let (icon, color, status) = if is_error {
            ("✗", Color::Red, "Error")
        } else {
            ("✓", Color::Green, "Completed")
        };

        vec![
            Line::from(vec![
                Span::styled(format!("  {} ", icon), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("Tool [{}] {}", tool_name, status), Style::default().fg(color)),
            ]),
        ]
    }
}
