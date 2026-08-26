use crate::style::ThemePalette;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestionKind {
    SingleChoice,
    MultiChoice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionModalState {
    pub title: String,
    pub question: String,
    pub kind: QuestionKind,
    pub options: Vec<QuestionOption>,
    pub custom_input: String,
    pub is_custom_focused: bool,
    pub is_submitted: bool,
    pub is_dismissed: bool,
    #[serde(skip)]
    pub list_state: ListState,
}

impl Default for QuestionModalState {
    fn default() -> Self {
        Self::sample_question()
    }
}

impl QuestionModalState {
    pub fn new_single_choice(title: &str, question: &str, options: Vec<QuestionOption>) -> Self {
        let mut list_state = ListState::default();
        if !options.is_empty() {
            list_state.select(Some(0));
        }
        let mut state = Self {
            title: title.to_string(),
            question: question.to_string(),
            kind: QuestionKind::SingleChoice,
            options,
            custom_input: String::new(),
            is_custom_focused: false,
            is_submitted: false,
            is_dismissed: false,
            list_state,
        };
        // Ensure at most 1 is selected in SingleChoice mode
        if state.options.iter().filter(|o| o.selected).count() == 0 && !state.options.is_empty() {
            state.options[0].selected = true;
        }
        state
    }

    pub fn new_multi_choice(title: &str, question: &str, options: Vec<QuestionOption>) -> Self {
        let mut list_state = ListState::default();
        if !options.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            title: title.to_string(),
            question: question.to_string(),
            kind: QuestionKind::MultiChoice,
            options,
            custom_input: String::new(),
            is_custom_focused: false,
            is_submitted: false,
            is_dismissed: false,
            list_state,
        }
    }

    pub fn sample_question() -> Self {
        let options = vec![
            QuestionOption {
                id: "opt-1".to_string(),
                label: "Strict Invariant: Zero Warnings & Pure Safe Rust".to_string(),
                description: Some(
                    "Ensure 100% safe Rust and zero compiler warnings across all workspace crates."
                        .to_string(),
                ),
                selected: true,
            },
            QuestionOption {
                id: "opt-2".to_string(),
                label: "Non-Blocking Async Subprocess Execution".to_string(),
                description: Some(
                    format!(
                        "Guarantee {}s timeout and automatic zombie process termination on abort.",
                        pi_core::plan::VERIFY_TIMEOUT_SECS
                    )
                    .to_string(),
                ),
                selected: false,
            },
            QuestionOption {
                id: "opt-3".to_string(),
                label: "Surgical Memory & Plan Cockpit Overlay".to_string(),
                description: Some(
                    "Provide keyboard-navigable TUI widgets for interactive task management."
                        .to_string(),
                ),
                selected: false,
            },
        ];

        Self::new_single_choice(
            "Agent Clarification Request",
            "Which architectural priority should the agent optimize for during this execution phase?",
            options,
        )
    }

    pub fn handle_navigation(&mut self, up: bool) {
        let total_items = self.options.len() + 1; // +1 for the Custom / Other input row
        if total_items == 0 {
            return;
        }

        let next = match self.list_state.selected() {
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

        self.list_state.select(Some(next));
        self.is_custom_focused = next == self.options.len();
    }

    pub fn toggle_selected(&mut self) {
        let Some(idx) = self.list_state.selected() else {
            return;
        };

        if idx < self.options.len() {
            match self.kind {
                QuestionKind::SingleChoice => {
                    for (i, opt) in self.options.iter_mut().enumerate() {
                        opt.selected = i == idx;
                    }
                }
                QuestionKind::MultiChoice => {
                    self.options[idx].selected = !self.options[idx].selected;
                }
            }
        }
    }

    pub fn selected_answers(&self) -> Vec<String> {
        let mut answers = Vec::new();
        for opt in &self.options {
            if opt.selected {
                answers.push(opt.label.clone());
            }
        }
        let trimmed_custom = self.custom_input.trim();
        if !trimmed_custom.is_empty() {
            answers.push(format!("Custom: {}", trimmed_custom));
        }
        answers
    }

    pub fn summary_answer(&self) -> String {
        let answers = self.selected_answers();
        if answers.is_empty() {
            "None selected".to_string()
        } else {
            answers.join("; ")
        }
    }

    pub fn submit(&mut self) -> Vec<String> {
        self.is_submitted = true;
        self.selected_answers()
    }

    pub fn dismiss(&mut self) {
        self.is_dismissed = true;
    }
}

pub struct QuestionModalWidget;

impl QuestionModalWidget {
    pub fn render(state: &QuestionModalState, f: &mut Frame, area: Rect, theme: &ThemePalette) {
        f.render_widget(Clear, area);

        let modal_border_color = match state.kind {
            QuestionKind::SingleChoice => theme.cyan,
            QuestionKind::MultiChoice => theme.yellow,
        };

        let kind_str = match state.kind {
            QuestionKind::SingleChoice => "Single Choice (Radio)",
            QuestionKind::MultiChoice => "Multi-Choice (Checkboxes)",
        };

        let title_text = format!(" 🤔 {} [{}] ", state.title, kind_str);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(modal_border_color))
            .title(title_text)
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(theme.bg));

        let inner_area = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Question text
                Constraint::Min(4),    // Options list
                Constraint::Length(3), // Custom input box
                Constraint::Length(2), // Cheatsheet footer
            ])
            .margin(1)
            .split(inner_area);

        // 1. Question Text Header
        let question_paragraph = Paragraph::new(Line::from(vec![
            Span::styled(
                "Q: ",
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &state.question,
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
        ]))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border)),
        );
        f.render_widget(question_paragraph, chunks[0]);

        // 2. Options List
        let list_items: Vec<ListItem> = state
            .options
            .iter()
            .map(|opt| {
                let badge = match state.kind {
                    QuestionKind::SingleChoice => {
                        if opt.selected {
                            "(•)"
                        } else {
                            "( )"
                        }
                    }
                    QuestionKind::MultiChoice => {
                        if opt.selected {
                            "[✔]"
                        } else {
                            "[ ]"
                        }
                    }
                };

                let badge_color = if opt.selected {
                    theme.green
                } else {
                    theme.muted
                };
                let label_style = if opt.selected {
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text)
                };

                let mut lines = vec![Line::from(vec![
                    Span::styled(
                        format!("{} ", badge),
                        Style::default()
                            .fg(badge_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&opt.label, label_style),
                ])];

                if let Some(ref desc) = opt.description {
                    lines.push(Line::from(vec![
                        Span::styled("    ", Style::default()),
                        Span::styled(desc, Style::default().fg(theme.muted)),
                    ]));
                }

                ListItem::new(lines)
            })
            .collect();

        let list_widget = List::new(list_items)
            .highlight_style(
                Style::default()
                    .bg(theme.surface)
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        // We only render state for option items 0..len-1
        let mut list_state_clone = ListState::default();
        if let Some(sel) = state.list_state.selected()
            && sel < state.options.len()
        {
            list_state_clone.select(Some(sel));
        }

        f.render_stateful_widget(list_widget, chunks[1], &mut list_state_clone);

        // 3. Custom Write-in Box
        let is_custom_selected = state.list_state.selected() == Some(state.options.len());
        let custom_border_color = if is_custom_selected {
            theme.yellow
        } else {
            theme.border
        };
        let cursor_char = if is_custom_selected { "█" } else { "" };

        let custom_line = Line::from(vec![
            Span::styled(
                " Write-in / Other: ",
                Style::default().fg(if is_custom_selected {
                    theme.yellow
                } else {
                    theme.muted
                }),
            ),
            Span::styled(
                format!("{}{}", state.custom_input, cursor_char),
                Style::default().fg(theme.text),
            ),
        ]);

        let custom_box = Paragraph::new(custom_line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(custom_border_color)),
        );
        f.render_widget(custom_box, chunks[2]);

        // 4. Bottom Cheatsheet Footer
        let footer_spans = vec![Span::styled(
            "[↑/↓: Navigate · Space: Toggle · Enter: Submit · Esc: Dismiss]",
            Style::default().fg(modal_border_color),
        )];

        let footer = Paragraph::new(Line::from(footer_spans)).style(Style::default().bg(theme.bg));
        f.render_widget(footer, chunks[3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_choice_toggle() {
        let mut state = QuestionModalState::sample_question();
        assert_eq!(state.options.len(), 3);
        assert!(state.options[0].selected);
        assert!(!state.options[1].selected);

        // Navigate to item 1 and toggle
        state.handle_navigation(false);
        assert_eq!(state.list_state.selected(), Some(1));
        state.toggle_selected();

        assert!(!state.options[0].selected);
        assert!(state.options[1].selected);
        assert!(!state.options[2].selected);
    }

    #[test]
    fn test_multi_choice_toggle() {
        let options = vec![
            QuestionOption {
                id: "1".to_string(),
                label: "Option 1".to_string(),
                description: None,
                selected: false,
            },
            QuestionOption {
                id: "2".to_string(),
                label: "Option 2".to_string(),
                description: None,
                selected: false,
            },
        ];
        let mut state = QuestionModalState::new_multi_choice("Title", "Prompt?", options);

        state.list_state.select(Some(0));
        state.toggle_selected();
        assert!(state.options[0].selected);

        state.list_state.select(Some(1));
        state.toggle_selected();
        assert!(state.options[1].selected);

        let answers = state.selected_answers();
        assert_eq!(answers.len(), 2);
    }

    #[test]
    fn test_custom_input_submission() {
        let mut state = QuestionModalState::sample_question();
        state.custom_input = "Custom tailored response".to_string();
        let answers = state.submit();
        assert!(
            answers
                .iter()
                .any(|a| a.contains("Custom: Custom tailored response"))
        );
        assert!(state.is_submitted);
    }
}
