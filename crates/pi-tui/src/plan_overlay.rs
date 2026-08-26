use crate::style::ThemePalette;
use pi_core::plan::{ExecutionPlan, PlanTask, TaskStatus};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanState {
    pub goal: String,
    pub tasks: Vec<PlanTask>,
    pub active_task_idx: Option<usize>,
    pub is_collapsed: bool,
    #[serde(skip)]
    pub list_state: ListState,
}

impl Default for PlanState {
    fn default() -> Self {
        Self::new(String::new(), Vec::new())
    }
}

impl PlanState {
    pub fn new(goal: impl Into<String>, tasks: Vec<PlanTask>) -> Self {
        let mut list_state = ListState::default();
        if !tasks.is_empty() {
            list_state.select(Some(0));
        }
        let active_task_idx = tasks.iter().position(|t| t.status.is_running());
        Self {
            goal: goal.into(),
            tasks,
            active_task_idx,
            is_collapsed: false,
            list_state,
        }
    }

    pub fn from_execution_plan(plan: ExecutionPlan) -> Self {
        let mut list_state = ListState::default();
        if !plan.tasks.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            goal: plan.goal,
            tasks: plan.tasks,
            active_task_idx: plan.active_task_idx,
            is_collapsed: false,
            list_state,
        }
    }

    pub fn completed_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.status.is_completed())
            .count()
    }

    pub fn failed_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.status.is_failed()).count()
    }

    pub fn progress_pct(&self) -> f32 {
        if self.tasks.is_empty() {
            return 100.0;
        }
        let completed = self.completed_count();
        (completed as f32 / self.tasks.len() as f32) * 100.0
    }

    pub fn selected_task(&self) -> Option<&PlanTask> {
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
                TaskStatus::Pending => TaskStatus::Running {
                    progress_pct: 50,
                    started_at: 0,
                },
                TaskStatus::Running { .. } => TaskStatus::Completed {
                    duration_ms: 0,
                    summary: String::new(),
                },
                TaskStatus::Completed { .. } => TaskStatus::Failed {
                    error: "Manual fail".to_string(),
                    retry_count: 0,
                },
                TaskStatus::Failed { .. } => TaskStatus::Pending,
            };
            self.update_active_index();
        }
    }

    pub fn mark_selected_completed(&mut self) {
        if let Some(sel) = self.list_state.selected()
            && let Some(task) = self.tasks.get_mut(sel)
        {
            task.status = TaskStatus::Completed {
                duration_ms: 0,
                summary: String::new(),
            };
            self.update_active_index();
        }
    }

    pub fn toggle_collapsed(&mut self) {
        self.is_collapsed = !self.is_collapsed;
    }

    fn update_active_index(&mut self) {
        self.active_task_idx = self.tasks.iter().position(|t| t.status.is_running());
    }

    pub fn add_task(&mut self, item: PlanTask) {
        self.tasks.push(item);
        if self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }
        self.update_active_index();
    }
}

pub fn task_status_badge(status: &TaskStatus) -> (&'static str, Color) {
    match status {
        TaskStatus::Pending => ("[ ]", Color::DarkGray),
        TaskStatus::Running { .. } => ("[◐]", Color::Yellow),
        TaskStatus::Completed { .. } => ("[✔]", Color::Green),
        TaskStatus::Failed { .. } => ("[✖]", Color::Red),
    }
}

pub fn task_status_label(status: &TaskStatus) -> String {
    match status {
        TaskStatus::Pending => "Pending".to_string(),
        TaskStatus::Running { progress_pct, .. } => format!("Running ({}%)", progress_pct),
        TaskStatus::Completed { summary, .. } => {
            if summary.is_empty() {
                "Completed".to_string()
            } else {
                format!("Done: {}", summary)
            }
        }
        TaskStatus::Failed { error, .. } => format!("Failed: {}", error),
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
        let collapse_indicator = if state.is_collapsed {
            " ▶ (collapsed)"
        } else {
            " ▼"
        };
        lines.push(Line::from(vec![
            Span::styled(
                "📋 Active Plan: ",
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                state.goal.clone(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " [{}/{} · {:.0}%]{}",
                    completed, total, progress, collapse_indicator
                ),
                Style::default()
                    .fg(progress_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        if !state.is_collapsed {
            for (idx, task) in state.tasks.iter().enumerate() {
                let (badge_str, badge_color) = task_status_badge(&task.status);
                let is_active = Some(idx) == state.active_task_idx;

                let title_style = if is_active {
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
                } else if task.status.is_completed() {
                    Style::default().fg(theme.muted)
                } else {
                    Style::default().fg(theme.text)
                };

                let mut spans = vec![
                    Span::styled(
                        format!("   {} ", badge_str),
                        Style::default()
                            .fg(badge_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("{}. ", idx + 1), Style::default().fg(theme.muted)),
                    Span::styled(task.title.clone(), title_style),
                ];

                if is_active {
                    spans.push(Span::styled(
                        " ⚡ Active",
                        Style::default()
                            .fg(theme.yellow)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                lines.push(Line::from(spans));
            }
        }

        lines
    }

    /// Renders the full interactive modal dialog overlay
    pub fn render_modal(state: &PlanState, f: &mut Frame, area: Rect, theme: &ThemePalette) {
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
            Span::styled(
                "Goal: ",
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &state.goal,
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
        ]);

        let header_line_2 = Line::from(vec![
            Span::styled("Progress: [", Style::default().fg(theme.muted)),
            Span::styled(
                "█".repeat(filled_chars),
                Style::default().fg(progress_color),
            ),
            Span::styled("░".repeat(empty_chars), Style::default().fg(theme.border)),
            Span::styled(
                format!("] {:.0}% ({} of {} tasks done)", progress, completed, total),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
        ]);

        let header_widget = Paragraph::new(vec![header_line_1, header_line_2]).block(
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
                let (badge_str, badge_color) = task_status_badge(&task.status);
                let is_active = Some(idx) == state.active_task_idx;

                let title_style = if task.status.is_completed() {
                    Style::default().fg(theme.muted)
                } else {
                    Style::default().fg(theme.text)
                };

                let mut spans = vec![
                    Span::styled(
                        format!("{} ", badge_str),
                        Style::default()
                            .fg(badge_color)
                            .add_modifier(Modifier::BOLD),
                    ),
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
                Span::styled(
                    "Title: ",
                    Style::default()
                        .fg(theme.yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    &selected_task.title,
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ),
            ]));

            let (badge_str, badge_color) = task_status_badge(&selected_task.status);
            inspector_lines.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(theme.muted)),
                Span::styled(
                    format!("{} {}", badge_str, task_status_label(&selected_task.status)),
                    Style::default()
                        .fg(badge_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("   ID: {}", selected_task.id),
                    Style::default().fg(theme.muted),
                ),
            ]));

            if let Some(ref cmd) = selected_task.verification_command {
                inspector_lines.push(Line::from(vec![
                    Span::styled(
                        "Verification Gate: ",
                        Style::default()
                            .fg(theme.magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(cmd, Style::default().fg(theme.text)),
                ]));
            }

            if !selected_task.dependencies.is_empty() {
                inspector_lines.push(Line::from(vec![
                    Span::styled("Dependencies: ", Style::default().fg(theme.muted)),
                    Span::styled(
                        selected_task.dependencies.join(", "),
                        Style::default().fg(theme.cyan),
                    ),
                ]));
            }

            inspector_lines.push(Line::from(""));
            inspector_lines.push(Line::from(vec![Span::styled(
                "Description:",
                Style::default()
                    .fg(theme.text)
                    .add_modifier(Modifier::UNDERLINED),
            )]));
            inspector_lines.push(Line::from(Span::styled(
                &selected_task.description,
                Style::default().fg(theme.text),
            )));

            let inspector_paragraph = Paragraph::new(inspector_lines)
                .block(inspector_block)
                .wrap(Wrap { trim: false });
            f.render_widget(inspector_paragraph, main_chunks[1]);
        } else {
            let empty_msg = Paragraph::new(Line::from(vec![Span::styled(
                "No task selected.",
                Style::default().fg(theme.muted),
            )]))
            .block(inspector_block);
            f.render_widget(empty_msg, main_chunks[1]);
        }

        // 3. Cheatsheet Footer
        let footer_spans = vec![Span::styled(
            "[↑/↓: Navigate · Space: Cycle Status · c: Toggle Collapse · Esc: Close]",
            Style::default().fg(theme.green),
        )];

        let footer = Paragraph::new(Line::from(footer_spans)).style(Style::default().bg(theme.bg));
        f.render_widget(footer, chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_state_progress_calculation() {
        let mut plan = ExecutionPlan::new("p1", "Sample plan");
        let mut t1 = PlanTask::new(
            "task-1",
            "Memory Explorer Overlay (`/memory`)",
            "Build searchable TauVault memory inspector with scope tabs, tag management, and counter-rule display.",
        );
        t1.status = TaskStatus::Completed {
            duration_ms: 0,
            summary: String::new(),
        };
        let mut t2 = PlanTask::new(
            "task-2",
            "Live Plan & Todo Checklist Widget (`/plan`)",
            "Implement collapsible interactive checklist showing [✔], [◐], [ ], [✖] tasks with live progress calculation.",
        );
        t2.status = TaskStatus::Running {
            progress_pct: 75,
            started_at: 0,
        };
        let t3 = PlanTask::new(
            "task-3",
            "Clarification Questionnaire Modal (`/ask`)",
            "Implement interactive single-choice and multi-choice modal for agent queries and user confirmations.",
        );
        let t4 = PlanTask::new(
            "task-4",
            "Wiring & Autocomplete in `pi-tui/src/lib.rs`",
            "Connect state fields, slash commands (/memory, /plan, /ask), and keyboard event handlers in main loop.",
        );
        let t5 = PlanTask::new(
            "task-5",
            "Quality Verification & Zero Warnings Gate",
            "Verify all 47+ workspace unit tests pass and cargo clippy --all-targets passes with zero warnings.",
        );
        plan.add_task(t1);
        plan.add_task(t2);
        plan.add_task(t3);
        plan.add_task(t4);
        plan.add_task(t5);

        let state = PlanState::from_execution_plan(plan);
        assert_eq!(state.tasks.len(), 5);
        assert_eq!(state.completed_count(), 1);
        assert_eq!(state.progress_pct(), 20.0);
    }

    #[test]
    fn test_plan_state_navigation() {
        let mut plan = ExecutionPlan::new("p1", "Nav");
        let t1 = PlanTask::new("t1", "T1", "d1");
        let t2 = PlanTask::new("t2", "T2", "d2");
        let t3 = PlanTask::new("t3", "T3", "d3");
        let t4 = PlanTask::new("t4", "T4", "d4");
        let t5 = PlanTask::new("t5", "T5", "d5");
        plan.add_task(t1);
        plan.add_task(t2);
        plan.add_task(t3);
        plan.add_task(t4);
        plan.add_task(t5);
        let mut state = PlanState::from_execution_plan(plan);
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
        let mut plan = ExecutionPlan::new("p1", "Toggle");
        let t1 = PlanTask::new("t1", "T1", "d1");
        let t2 = PlanTask::new("t2", "T2", "d2");
        let t3 = PlanTask::new("t3", "T3", "d3");
        let t4 = PlanTask::new("t4", "T4", "d4");
        let t5 = PlanTask::new("t5", "T5", "d5");
        plan.add_task(t1);
        plan.add_task(t2);
        plan.add_task(t3);
        plan.add_task(t4);
        plan.add_task(t5);

        let mut state = PlanState::from_execution_plan(plan);
        state.list_state.select(Some(2)); // Task 3 is Pending
        assert_eq!(state.tasks[2].status, TaskStatus::Pending);

        state.toggle_selected_task_status();
        assert!(matches!(state.tasks[2].status, TaskStatus::Running { .. }));

        state.toggle_selected_task_status();
        assert!(matches!(
            state.tasks[2].status,
            TaskStatus::Completed { .. }
        ));

        state.toggle_selected_task_status();
        assert!(matches!(state.tasks[2].status, TaskStatus::Failed { .. }));

        state.toggle_selected_task_status();
        assert_eq!(state.tasks[2].status, TaskStatus::Pending);
    }

    #[test]
    fn test_plan_compact_rendering() {
        let mut plan = ExecutionPlan::new("p1", "Sample plan");
        let mut t1 = PlanTask::new(
            "task-1",
            "Memory Explorer Overlay (`/memory`)",
            "Build searchable TauVault memory inspector with scope tabs, tag management, and counter-rule display.",
        );
        t1.status = TaskStatus::Completed {
            duration_ms: 0,
            summary: String::new(),
        };
        let mut t2 = PlanTask::new(
            "task-2",
            "Live Plan & Todo Checklist Widget (`/plan`)",
            "Implement collapsible interactive checklist showing [✔], [◐], [ ], [✖] tasks with live progress calculation.",
        );
        t2.status = TaskStatus::Running {
            progress_pct: 75,
            started_at: 0,
        };
        let t3 = PlanTask::new(
            "task-3",
            "Clarification Questionnaire Modal (`/ask`)",
            "Implement interactive single-choice and multi-choice modal for agent queries and user confirmations.",
        );
        plan.add_task(t1);
        plan.add_task(t2);
        plan.add_task(t3);

        let state = PlanState::from_execution_plan(plan);
        let theme = ThemePalette::default();
        let compact_lines = PlanOverlayWidget::render_compact(&state, &theme);
        assert!(!compact_lines.is_empty());
        assert!(compact_lines[0].spans[0].content.contains("Active Plan"));
    }
}
