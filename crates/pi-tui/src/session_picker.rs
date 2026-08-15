use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};

pub struct SessionPicker;

impl SessionPicker {
    pub fn render_widget<'a>(
        items: &'a [String],
    ) -> (List<'a>, Block<'a>) {
        let list_items: Vec<ListItem> = items
            .iter()
            .map(|label| ListItem::new(Span::styled(label, Style::default().fg(Color::Cyan))))
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Session Tree Navigator (Up/Down: Inspect | Enter: Rewind | Esc: Close) ");

        let list = List::new(list_items)
            .highlight_style(
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        (list, block)
    }

    pub fn handle_navigation(state: &mut ListState, total_items: usize, up: bool) {
        if total_items == 0 {
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
