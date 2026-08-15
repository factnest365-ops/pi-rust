use pi_providers::ModelInfo;
use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};

pub struct ModelPickerWidget;

impl ModelPickerWidget {
    pub fn render<'a>(
        query: &'a str,
        filtered_models: &'a [ModelInfo],
        total_count: usize,
    ) -> (Paragraph<'a>, List<'a>, Block<'a>) {
        let search_display = format!("> {}▏", query);
        let search_bar = Paragraph::new(Line::from(vec![
            Span::styled(" Search: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(search_display, Style::default().fg(Color::White)),
            Span::styled(format!(" (Showing {} of {} models)", filtered_models.len(), total_count), Style::default().fg(Color::DarkGray)),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

        let items: Vec<ListItem> = filtered_models
            .iter()
            .map(|m| {
                let ctx_k = if m.context_window >= 1_000_000 {
                    format!("{}M", m.context_window / 1_000_000)
                } else {
                    format!("{}k", m.context_window / 1_000)
                };

                let mut spans = vec![
                    Span::styled(format!(" [{}] ", m.provider), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(m.id.clone(), Style::default().fg(Color::White)),
                    Span::styled(format!(" ({} ctx)", ctx_k), Style::default().fg(Color::DarkGray)),
                ];

                if m.supports_reasoning {
                    spans.push(Span::styled(" 🧠", Style::default().fg(Color::Magenta)));
                }
                if m.supports_vision {
                    spans.push(Span::styled(" 👁", Style::default().fg(Color::Green)));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Select Model (Type to Search · Up/Down: Navigate · Enter: Select · Ctrl+R: Refresh · Esc: Cancel) ")
            .title_alignment(Alignment::Center);

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Yellow)
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
