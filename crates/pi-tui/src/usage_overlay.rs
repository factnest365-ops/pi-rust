use crate::style::ThemePalette;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

pub struct UsageOverlay;

impl UsageOverlay {
    pub fn render_progress_bar(pct: f32, width: usize, theme: &ThemePalette) -> Line<'static> {
        let clamped = pct.clamp(0.0, 100.0);
        let filled_chars = ((clamped / 100.0) * (width as f32)).round() as usize;
        let empty_chars = width.saturating_sub(filled_chars);

        let color = if clamped >= 80.0 {
            theme.red
        } else if clamped >= 50.0 {
            theme.yellow
        } else {
            theme.green
        };

        Line::from(vec![
            Span::raw("["),
            Span::styled("█".repeat(filled_chars), Style::default().fg(color)),
            Span::styled(
                "░".repeat(empty_chars),
                Style::default().fg(theme.muted),
            ),
            Span::raw(format!("] {:.1}%", clamped)),
        ])
    }

    pub fn render_summary(
        tokens: usize,
        max_limit: usize,
        theme: &ThemePalette,
    ) -> Vec<Line<'static>> {
        let pct = (tokens as f32 / max_limit as f32) * 100.0;
        vec![
            Line::from(vec![Span::styled(
                "Context Window Capacity: ",
                Style::default()
                    .fg(theme.cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Self::render_progress_bar(pct, 25, theme),
            Line::from(vec![Span::styled(
                format!(
                    "Tokens Used: {} / {} (Remaining: {})",
                    tokens,
                    max_limit,
                    max_limit.saturating_sub(tokens)
                ),
                Style::default().fg(theme.muted),
            )]),
        ]
    }
}
