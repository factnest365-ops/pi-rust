use crate::style::{ThemeKind, ThemePalette};
use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};

pub struct ThemePickerWidget;

impl ThemePickerWidget {
    pub fn filter_themes(query: &str) -> Vec<ThemeKind> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            ThemeKind::ALL.to_vec()
        } else {
            ThemeKind::ALL
                .iter()
                .filter(|t| {
                    t.name().to_lowercase().contains(&q)
                        || t.id_str().to_lowercase().contains(&q)
                        || t.description().to_lowercase().contains(&q)
                })
                .copied()
                .collect()
        }
    }

    pub fn render<'a>(
        query: &'a str,
        filtered_themes: &'a [ThemeKind],
        active_kind: ThemeKind,
        current_palette: &ThemePalette,
    ) -> (Paragraph<'a>, List<'a>, Block<'a>) {
        let search_display = format!("> {}▏", query);
        let search_bar = Paragraph::new(Line::from(vec![
            Span::styled(
                " Search Theme: ",
                Style::default()
                    .fg(current_palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(search_display, Style::default().fg(current_palette.text)),
            Span::styled(
                format!(" ({} themes available)", filtered_themes.len()),
                Style::default().fg(current_palette.muted),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(current_palette.border)),
        );

        let items: Vec<ListItem> = filtered_themes
            .iter()
            .map(|kind| {
                let pal = ThemePalette::from_kind(*kind);
                let is_active = *kind == active_kind;

                let mut spans = vec![
                    Span::styled(
                        if is_active {
                            " [Active] "
                        } else {
                            "          "
                        },
                        if is_active {
                            Style::default()
                                .fg(current_palette.green)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(current_palette.muted)
                        },
                    ),
                    Span::styled(
                        format!("{:<20}", kind.name()),
                        Style::default()
                            .fg(current_palette.text)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" ", Style::default()),
                    // Color swatches preview
                    Span::styled("██", Style::default().fg(pal.accent)),
                    Span::styled("██", Style::default().fg(pal.green)),
                    Span::styled("██", Style::default().fg(pal.yellow)),
                    Span::styled("██", Style::default().fg(pal.red)),
                    Span::styled("██", Style::default().fg(pal.cyan)),
                    Span::styled("██", Style::default().fg(pal.magenta)),
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        kind.description(),
                        Style::default().fg(current_palette.muted),
                    ),
                ];

                if is_active {
                    spans.push(Span::styled(
                        " ✔",
                        Style::default().fg(current_palette.green),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(current_palette.accent))
            .title(" Select Color Theme (Type to Search · Up/Down: Navigate · Enter: Apply · Esc: Cancel) ")
            .title_alignment(Alignment::Center);

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(current_palette.accent)
                    .fg(Color::Black)
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
mod tests {
    use super::*;

    #[test]
    fn test_theme_filtering() {
        let all = ThemePickerWidget::filter_themes("");
        assert_eq!(all.len(), ThemeKind::ALL.len());

        let tokyo = ThemePickerWidget::filter_themes("tokyo");
        assert_eq!(tokyo.len(), 1);
        assert_eq!(tokyo[0], ThemeKind::TokyoNight);

        let dark = ThemePickerWidget::filter_themes("dark");
        assert!(dark.len() >= 3); // GruvboxDark, SolarizedDark, OneDark
    }

    #[test]
    fn test_theme_picker_navigation() {
        let mut state = ListState::default();
        state.select(Some(0));

        ThemePickerWidget::handle_navigation(&mut state, 5, false);
        assert_eq!(state.selected(), Some(1));

        ThemePickerWidget::handle_navigation(&mut state, 5, true);
        assert_eq!(state.selected(), Some(0));

        ThemePickerWidget::handle_navigation(&mut state, 5, true);
        assert_eq!(state.selected(), Some(4)); // wrap around
    }
}
