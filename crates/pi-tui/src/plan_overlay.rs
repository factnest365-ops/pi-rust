use crate::style::ThemePalette;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanTaskStatus {
    Pending,
    Running { progress_pct: u8 },
    Completed { summary: Option<String> },
    Failed { error: String },
}

impl PlanTaskStatus {
    pub fn badge(&self) -> (&'static str, Color) {
        match self {
            PlanTaskStatus::Pending => ("[ ]", Color::DarkGray),
            PlanTaskStatus::Running { .. } => ("[◐]", Color::Yellow),
            PlanTaskStatus::Completed { .. } => ("[✔]", Color::Green),
            PlanTaskStatus::Failed { .. } => ("[✖]", Color::Red),
        }
    }

    pub fn display_label(&self) -> String {
        match self {
            PlanTaskStatus::Pending => "Pending".to_string(),
            PlanTaskStatus::Running { progress_pct } => format!("Running ({}%)", progress_pct),
            PlanTaskStatus::Completed { summary } => {
                if let Some(s) = summary {
                    format!("Done: {}", s)
                } else {
                    "Completed".to_string()
                }
            }
            PlanTaskStatus::Failed { error } => format!("Failed: {}", error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTaskItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: PlanTaskStatus,
    pub verification_cmd: Option<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanState {
    pub plan_id: String,
    pub goal: String,
    pub tasks: Vec<PlanTaskItem>,
    pub active_task_idx: Option<usize>,
    pub is_collapsed: bool,
    #[serde(skip)]
    pub list_state: ListState,
}

impl Default for PlanState {
    fn default() -> Self {
        Self::sample_plan()
    }
}

impl PlanState {
    pub fn new(plan_id: &str, goal: &str, tasks: Vec<PlanTaskItem>) -> Self {
        let mut list_state = ListState::default();
        if !tasks.is_empty() {
            list_state.select(Some(0));
        }
        let active_task_idx = tasks.iter().position(|t| matches!(t.status, PlanTaskStatus::Running { .. }));
        Self {
            plan_id: plan_id.to_string(),
            goal: goal.to_string(),
            tasks,
            active_task_idx,
            is_collapsed: false,
            list_state,
        }
    }

    pub fn sample_plan() -> Self {
        let tasks = vec![
            PlanTaskItem {
                id: "task-1".to_string(),
                title: "Memory Explorer Overlay (`/memory`)".to_string(),
                description: "Build searchable TauVault memory inspector with scope tabs, tag management, and counter-rule display.".to_string(),
                status: PlanTaskStatus::Completed { summary: Some("Implemented memory_overlay.rs".to_string()) },
                verification_cmd: Some("cargo test -p pi-tui -- test_memory".to_string()),
                dependencies: Vec::new(),
            },
            PlanTaskItem {
                id: "task-2".to_string(),
                title: "Live Plan & Todo Checklist Widget (`/plan`)".to_string(),
                description: "Implement collapsible interactive checklist showing [✔], [◐], [ ], [✖] tasks with live progress calculation.".to_string(),
                status: PlanTaskStatus::Running { progress_pct: 75 },
                verification_cmd: Some("cargo test -p pi-tui -- test_plan".to_string()),
                dependencies: vec!["task-1".to_string()],
            },
            PlanTaskItem {
                id: "task-3".to_string(),
                title: "Clarification Questionnaire Modal (`/ask`)".to_string(),
                description: "Implement interactive single-choice and multi-choice modal for agent queries and user confirmations.".to_string(),
                status: PlanTaskStatus::Pending,
                verification_cmd: Some("cargo test -p pi-tui -- test_question".to_string()),
                dependencies: vec!["task-2".to_string()],
            },
            PlanTaskItem {
                id: "task-4".to_string(),
                title: "Wiring & Autocomplete in `pi-tui/src/lib.rs`".to_string(),
                description: "Connect state fields, slash commands (/memory, /plan, /ask), and keyboard event handlers in main loop.".to_string(),
                status: PlanTaskStatus::Pending,
                verification_cmd: Some("cargo check -p pi-tui --all-targets".to_string()),
                dependencies: vec!["task-1".to_string(), "task-2".to_string(), "task-3".to_string()],
            },
            PlanTaskItem {
                id: "task-5".to_string(),
                title: "Quality Verification & Zero Warnings Gate".to_string(),
                description: "Verify all 47+ workspace unit tests pass and cargo clippy --all-targets passes with zero warnings.".to_string(),
                status: PlanTaskStatus::Pending,
                verification_cmd: Some("cargo clippy -p pi-tui --all-targets -- -D warnings".to_string()),
                dependencies: vec!["task-4".to_string()],
            },
        ];

        Self::new(
            "plan-phase-12",
            "Phase 12: Super-TUI Cockpit (Memory Explorer, Plan Overlay, Clarification Modal)",
            tasks,
        )
    }

    pub fn completed_count(&self) -> usize {
        self.tasks.iter().filter(|t| matches!(t.status, PlanTaskStatus::Completed { .. })).count()
    }

    pub fn failed_count(&self) -> usize {
        self.tasks.iter().filter(|t| matches!(t.status, PlanTaskStatus::Failed { .. })).count()
    }

    pub fn progress_pct(&self) -> f32 {
        if self.tasks.is_empty() {
            return 100.0;
        }
        let completed = self.completed_count();
        (completed as f32 / self.tasks.len() as f32) * 100.0
    }

    pub fn selected_task(&self) -> Option<&PlanTaskItem> {
        let sel = self.list_state.selected()?;
        self.tasks.get(sel)
    }

    pub fn handle_navigation(&mut self, up: bool) {
        if self.tasks.is_empty() {
            self.list_state.select(None);
            return;
        }

        let next = match self.list_state.selected() {
            Some(i) => {
                if up {
                    if i == 0 { self.tasks.len() - 1 } else { i - 1 }
                } else if i >= self.tasks.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };

        self.list_state.select(Some(next));
    }

    pub fn toggle_selected_task_status(&mut self) {
        if let Some(sel) = self.list_state.selected()
            && let Some(task) = self.tasks.get_mut(sel)
        {
            task.status = match task.status {
                PlanTaskStatus::Pending => PlanTaskStatus::Running { progress_pct: 50 },
                PlanTaskStatus::Running { .. } => PlanTaskStatus::Completed { summary: None },
                PlanTaskStatus::Completed { .. } => PlanTaskStatus::Failed { error: "Manual fail".to_string() },
                PlanTaskStatus::Failed { .. } => PlanTaskStatus::Pending,
            };
            self.update_active_index();
        }
    }

    pub fn mark_selected_completed(&mut self) {
        if let Some(sel) = self.list_state.selected()
            && let Some(task) = self.tasks.get_mut(sel)
        {
            task.status = PlanTaskStatus::Completed { summary: None };
            self.update_active_index();
        }
    }

    pub fn toggle_collapsed(&mut self) {
        self.is_collapsed = !self.is_collapsed;
    }

    fn update_active_index(&mut self) {
        self.active_task_idx = self.tasks.iter().position(|t| matches!(t.status, PlanTaskStatus::Running { .. }));
    }

    pub fn add_task(&mut self, item: PlanTaskItem) {
        self.tasks.push(item);
        if self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }
        self.update_active_index();
    }
}

pub struct PlanOverlayWidget;

impl PlanOverlayWidget {
    /// Renders the compact checklist widget suitable for inline transcript or status bar display
    pub fn render_compact(state: &PlanState, theme: &ThemePalette) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        if state.tasks.is_empty() {
            return lines;
        }

        let progress = state.progress_pct();
        let completed = state.completed_count();
        let total = state.tasks.len();

        let progress_color = if progress >= 100.0 {
            theme.green
        } else if progress >= 50.0 {
            theme.cyan
        } else {
            theme.yellow
        };

        // Header Line
        let collapse_indicator = if state.is_collapsed { " ▶ (collapsed)" } else { " ▼" };
        lines.push(Line::from(vec![
            Span::styled("📋 Active Plan: ", Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD)),
            Span::styled(state.goal.clone(), Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" [{}/{} · {:.0}%]{}", completed, total, progress, collapse_indicator), Style::default().fg(progress_color).add_modifier(Modifier::BOLD)),
        ]));

        if !state.is_collapsed {
            for (idx, task) in state.tasks.iter().enumerate() {
                let (badge_str, badge_color) = task.status.badge();
                let is_active = Some(idx) == state.active_task_idx;

                let title_style = if is_active {
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
                } else if matches!(task.status, PlanTaskStatus::Completed { .. }) {
                    Style::default().fg(theme.muted)
                } else {
                    Style::default().fg(theme.text)
                };

                let mut spans = vec![
                    Span::styled(format!("   {} ", badge_str), Style::default().fg(badge_color).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{}. ", idx + 1), Style::default().fg(theme.muted)),
                    Span::styled(task.title.clone(), title_style),
                ];

                if is_active {
                    spans.push(Span::styled(" ⚡ Active", Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD)));
                }

                lines.push(Line::from(spans));
            }
        }

        lines
    }

    /// Renders the full interactive modal dialog overlay
    pub fn render_modal(
        state: &PlanState,
        f: &mut Frame,
        area: Rect,
        theme: &ThemePalette,
    ) {
        f.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.green))
            .title(" 📋 Tau Stateful Plan & Task Verification Engine (/plan) ")
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(theme.bg));

        let inner_area = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Goal & Progress Bar Header
                Constraint::Min(6),    // Split view (Tasks list & Detail Inspector)
                Constraint::Length(2), // Cheatsheet footer
            ])
            .margin(1)
            .split(inner_area);

        // 1. Goal & Progress Bar Header
        let progress = state.progress_pct();
        let completed = state.completed_count();
        let total = state.tasks.len();

        let filled_chars = ((progress / 100.0) * 20.0).round() as usize;
        let empty_chars = 20usize.saturating_sub(filled_chars);

        let progress_color = if progress >= 100.0 {
            theme.green
        } else if progress >= 50.0 {
            theme.cyan
        } else {
            theme.yellow
        };

        let header_line_1 = Line::from(vec![
            Span::styled("Goal: ", Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD)),
            Span::styled(&state.goal, Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
        ]);

        let header_line_2 = Line::from(vec![
            Span::styled("Progress: [", Style::default().fg(theme.muted)),
            Span::styled("█".repeat(filled_chars), Style::default().fg(progress_color)),
            Span::styled("░".repeat(empty_chars), Style::default().fg(theme.border)),
            Span::styled(format!("] {:.0}% ({} of {} tasks done)", progress, completed, total), Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
        ]);

        let header_widget = Paragraph::new(vec![header_line_1, header_line_2])
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(theme.border)),
            );
        f.render_widget(header_widget, chunks[0]);

        // 2. Main Content Split View
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        // Task Items List
        let list_items: Vec<ListItem> = state
            .tasks
            .iter()
            .enumerate()
            .map(|(idx, task)| {
                let (badge_str, badge_color) = task.status.badge();
                let is_active = Some(idx) == state.active_task_idx;

                let title_style = if matches!(task.status, PlanTaskStatus::Completed { .. }) {
                    Style::default().fg(theme.muted)
                } else {
                    Style::default().fg(theme.text)
                };

                let mut spans = vec![
                    Span::styled(format!("{} ", badge_str), Style::default().fg(badge_color).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{}. ", idx + 1), Style::default().fg(theme.muted)),
                    Span::styled(&task.title, title_style),
                ];

                if is_active {
                    spans.push(Span::styled(" ⚡", Style::default().fg(theme.yellow)));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list_widget = List::new(list_items)
            .block(
                Block::default()
                    .borders(Borders::RIGHT)
                    .border_style(Style::default().fg(theme.border))
                    .title(format!(" Checklist ({} tasks) ", state.tasks.len())),
            )
            .highlight_style(
                Style::default()
                    .bg(theme.green)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        let mut list_state_clone = state.list_state.clone();
        f.render_stateful_widget(list_widget, main_chunks[0], &mut list_state_clone);

        // Task Detail Inspector
        let inspector_block = Block::default()
            .borders(Borders::NONE)
            .title(" Task Inspector ")
            .title_style(Style::default().fg(theme.cyan).add_modifier(Modifier::BOLD));

        if let Some(selected_task) = state.selected_task() {
            let mut inspector_lines = Vec::new();

            inspector_lines.push(Line::from(vec![
                Span::styled("Title: ", Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD)),
                Span::styled(&selected_task.title, Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
            ]));

            let (badge_str, badge_color) = selected_task.status.badge();
            inspector_lines.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(theme.muted)),
                Span::styled(format!("{} {}", badge_str, selected_task.status.display_label()), Style::default().fg(badge_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("   ID: {}", selected_task.id), Style::default().fg(theme.muted)),
            ]));

            if let Some(ref cmd) = selected_task.verification_cmd {
                inspector_lines.push(Line::from(vec![
                    Span::styled("Verification Gate: ", Style::default().fg(theme.magenta).add_modifier(Modifier::BOLD)),
                    Span::styled(cmd, Style::default().fg(theme.text)),
                ]));
            }

            if !selected_task.dependencies.is_empty() {
                inspector_lines.push(Line::from(vec![
                    Span::styled("Dependencies: ", Style::default().fg(theme.muted)),
                    Span::styled(selected_task.dependencies.join(", "), Style::default().fg(theme.cyan)),
                ]));
            }

            inspector_lines.push(Line::from(""));
            inspector_lines.push(Line::from(vec![
                Span::styled("Description:", Style::default().fg(theme.text).add_modifier(Modifier::UNDERLINED)),
            ]));
            inspector_lines.push(Line::from(Span::styled(&selected_task.description, Style::default().fg(theme.text))));

            let inspector_paragraph = Paragraph::new(inspector_lines)
                .block(inspector_block)
                .wrap(Wrap { trim: false });
            f.render_widget(inspector_paragraph, main_chunks[1]);
        } else {
            let empty_msg = Paragraph::new(Line::from(vec![
                Span::styled("No task selected.", Style::default().fg(theme.muted)),
            ]))
            .block(inspector_block);
            f.render_widget(empty_msg, main_chunks[1]);
        }

        // 3. Cheatsheet Footer
        let footer_spans = vec![
            Span::styled("[↑/↓: Navigate · Space: Cycle Status · c: Toggle Collapse · Esc: Close]", Style::default().fg(theme.green)),
        ];

        let footer = Paragraph::new(Line::from(footer_spans))
            .style(Style::default().bg(theme.bg));
        f.render_widget(footer, chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_state_progress_calculation() {
        let state = PlanState::sample_plan();
        assert_eq!(state.tasks.len(), 5);
        assert_eq!(state.completed_count(), 1);
        assert_eq!(state.progress_pct(), 20.0);
    }

    #[test]
    fn test_plan_state_navigation() {
        let mut state = PlanState::sample_plan();
        assert_eq!(state.list_state.selected(), Some(0));

        state.handle_navigation(false);
        assert_eq!(state.list_state.selected(), Some(1));

        state.handle_navigation(true);
        assert_eq!(state.list_state.selected(), Some(0));

        // Wrap around backward
        state.handle_navigation(true);
        assert_eq!(state.list_state.selected(), Some(4));
    }

    #[test]
    fn test_plan_state_toggle_status() {
        let mut state = PlanState::sample_plan();
        state.list_state.select(Some(2)); // Task 3 is Pending
        assert_eq!(state.tasks[2].status, PlanTaskStatus::Pending);

        state.toggle_selected_task_status();
        assert!(matches!(state.tasks[2].status, PlanTaskStatus::Running { .. }));

        state.toggle_selected_task_status();
        assert!(matches!(state.tasks[2].status, PlanTaskStatus::Completed { .. }));

        state.toggle_selected_task_status();
        assert!(matches!(state.tasks[2].status, PlanTaskStatus::Failed { .. }));

        state.toggle_selected_task_status();
        assert_eq!(state.tasks[2].status, PlanTaskStatus::Pending);
    }

    #[test]
    fn test_plan_compact_rendering() {
        let state = PlanState::sample_plan();
        let theme = ThemePalette::default();
        let compact_lines = PlanOverlayWidget::render_compact(&state, &theme);
        assert!(!compact_lines.is_empty());
        assert!(compact_lines[0].spans[0].content.contains("Active Plan"));
    }
}
