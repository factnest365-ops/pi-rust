use pi_providers::AuthResolver;
pub use pi_providers::registry::{PROVIDERS, ProviderDef};

use ratatui::layout::Alignment;
use crate::style::ThemePalette;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};

/// Back-compat alias: canonical definition lives in `pi_providers::registry`.
pub type ProviderEntry = ProviderDef;
/// Back-compat alias over the canonical registry (single source of truth).
pub const ALL_PROVIDERS: &[ProviderEntry] = PROVIDERS;

pub struct ProviderPickerWidget;

impl ProviderPickerWidget {
    pub fn render<'a>(
        query: &'a str,
        filtered_providers: &'a [&'static ProviderEntry],
        theme: &'a ThemePalette,
    ) -> (Paragraph<'a>, List<'a>, Block<'a>) {
        let search_display = format!("> {}▏", query);
        let search_bar = Paragraph::new(Line::from(vec![
            Span::styled(
                " Filter: ",
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(search_display, Style::default().fg(theme.text)),
            Span::styled(
                format!(
                    " (Showing {} of {} providers)",
                    filtered_providers.len(),
                    ALL_PROVIDERS.len()
                ),
                Style::default().fg(theme.muted),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.muted)),
        );

        let items: Vec<ListItem> = filtered_providers
            .iter()
            .map(|p| {
                let has_auth = AuthResolver::resolve_key(p.id).is_some();
                let status_badge = if p.is_local {
                    Span::styled(
                        " [⚡ Local Zero-Config] ",
                        Style::default()
                            .fg(theme.cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                } else if has_auth {
                    Span::styled(
                        " [✓ Configured] ",
                        Style::default()
                            .fg(theme.green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(" [🔑 Key Needed] ", Style::default().fg(theme.yellow))
                };

                let spans = vec![
                    Span::styled(
                        format!(" {:<20} ", p.name),
                        Style::default()
                            .fg(theme.text)
                            .add_modifier(Modifier::BOLD),
                    ),
                    status_badge,
                    Span::styled(
                        format!("({})", p.endpoint),
                        Style::default().fg(theme.muted),
                    ),
                ];

                ListItem::new(Line::from(spans))
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.green))
            .title(" AI Providers (Type to Filter · Enter: Select/Auth · Esc: Close) ")
            .title_alignment(Alignment::Center);

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(theme.green)
                    .fg(theme.on_accent)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        (search_bar, list, block)
    }

    pub fn handle_navigation(state: &mut ListState, total_items: usize, up: bool) {
        if total_items == 0 {
            state.select(None);
            return;
        }

        let next = match state.selected() {
            Some(i) => {
                if up {
                    if i == 0 { total_items - 1 } else { i - 1 }
                } else if i >= total_items - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };

        state.select(Some(next));
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_all_providers_coverage() {
        assert!(ALL_PROVIDERS.len() >= 20);
        assert!(ALL_PROVIDERS.iter().any(|p| p.id == "anthropic"));
        assert!(ALL_PROVIDERS.iter().any(|p| p.id == "openai"));
        assert!(ALL_PROVIDERS.iter().any(|p| p.id == "gemini"));
        assert!(ALL_PROVIDERS.iter().any(|p| p.id == "deepseek"));
        assert!(ALL_PROVIDERS.iter().any(|p| p.id == "ollama" && p.is_local));
    }

    #[test]
    fn test_provider_navigation() {
        let mut state = ListState::default();
        ProviderPickerWidget::handle_navigation(&mut state, 5, false);
        assert_eq!(state.selected(), Some(0));

        ProviderPickerWidget::handle_navigation(&mut state, 5, false);
        assert_eq!(state.selected(), Some(1));

        ProviderPickerWidget::handle_navigation(&mut state, 5, true);
        assert_eq!(state.selected(), Some(0));

        ProviderPickerWidget::handle_navigation(&mut state, 5, true);
        assert_eq!(state.selected(), Some(4));
    }
}
