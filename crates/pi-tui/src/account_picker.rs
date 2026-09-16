use ratatui::layout::Alignment;
use crate::style::ThemePalette;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

pub struct AccountPicker;

impl AccountPicker {
    pub fn render_modal(provider: &str, input: &str, theme: &ThemePalette) -> Paragraph<'static> {
        let masked_key: String = input.chars().map(|_| '•').collect();
        let cursor_display = format!("{}_", masked_key);

        let input_display_span = if masked_key.is_empty() {
            Span::styled("<paste or enter key>", Style::default().fg(theme.muted))
        } else {
            Span::styled(cursor_display, Style::default().fg(theme.text))
        };

        let text = vec![
            Line::from(vec![
                Span::styled(
                    "Provider: ",
                    Style::default()
                        .fg(theme.yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    provider.to_string(),
                    Style::default()
                        .fg(theme.text)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "API Key: ",
                    Style::default()
                        .fg(theme.cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                input_display_span,
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "💾 Enter: Save to ~/.pi/config.json  |  Esc: Skip",
                Style::default().fg(theme.muted),
            )]),
        ];

        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.yellow))
                .title(" API Key Setup & Authentication Wizard ")
                .title_alignment(Alignment::Center),
        )
    }
}
