use crate::style::ThemePalette;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

pub struct ToolDisplay;

impl ToolDisplay {
    pub fn render_executing(tool_name: &str, tool_call_id: &str) -> Vec<Line<'static>> {
        Self::render_executing_styled(tool_name, tool_call_id, &ThemePalette::default_pi())
    }

    pub fn render_executing_styled(
        tool_name: &str,
        tool_call_id: &str,
        theme: &ThemePalette,
    ) -> Vec<Line<'static>> {
        vec![Line::from(vec![
            Span::styled("⚙ ", Style::default().fg(theme.yellow)),
            Span::styled("Executing Tool: ", theme.tool_label()),
            Span::styled(
                tool_name.to_string(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" (call_id: {})", tool_call_id),
                Style::default().fg(theme.muted),
            ),
        ])]
    }

    pub fn render_completed(tool_name: &str, is_error: bool) -> Vec<Line<'static>> {
        Self::render_completed_styled(tool_name, is_error, &ThemePalette::default_pi())
    }

    pub fn render_completed_styled(
        tool_name: &str,
        is_error: bool,
        theme: &ThemePalette,
    ) -> Vec<Line<'static>> {
        let (icon, color, status) = if is_error {
            ("✗", theme.red, "Error")
        } else {
            ("✓", theme.green, "Completed")
        };

        vec![Line::from(vec![
            Span::styled(
                format!("  {} ", icon),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("Tool [{}] {}", tool_name, status),
                Style::default().fg(color),
            ),
        ])]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_display_render_executing() {
        let lines = ToolDisplay::render_executing("read", "call_123");
        assert_eq!(lines.len(), 1);
        let text = lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(text.contains("read"));
        assert!(text.contains("call_123"));
    }

    #[test]
    fn test_tool_display_render_completed() {
        let ok_lines = ToolDisplay::render_completed("write", false);
        assert!(ok_lines[0].spans[1].content.contains("Completed"));

        let err_lines = ToolDisplay::render_completed("bash", true);
        assert!(err_lines[0].spans[1].content.contains("Error"));
    }
}
