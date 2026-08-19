use crate::style::ThemePalette;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryScope {
    Global,
    Workspace,
    Episodic,
    CounterRule,
}

impl MemoryScope {
    pub const ALL: &'static [MemoryScope] = &[
        MemoryScope::Global,
        MemoryScope::Workspace,
        MemoryScope::Episodic,
        MemoryScope::CounterRule,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            MemoryScope::Global => "Global",
            MemoryScope::Workspace => "Workspace",
            MemoryScope::Episodic => "Episodic",
            MemoryScope::CounterRule => "Counter-Rule",
        }
    }

    pub fn badge_str(&self) -> &'static str {
        match self {
            MemoryScope::Global => "[GLOBAL]",
            MemoryScope::Workspace => "[REPO]",
            MemoryScope::Episodic => "[EPISODIC]",
            MemoryScope::CounterRule => "[ANTI-PATTERN]",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub scope: MemoryScope,
    pub topic: String,
    pub content: String,
    pub counter_pattern: Option<String>,
    pub correct_pattern: Option<String>,
    pub tags: Vec<String>,
    pub confidence: f32,
    pub access_count: u32,
}

#[derive(Debug, Clone)]
pub struct MemoryOverlayState {
    pub items: Vec<MemoryItem>,
    pub search_query: String,
    pub is_searching: bool,
    pub list_state: ListState,
    pub selected_scope_filter: Option<MemoryScope>,
    pub status_message: Option<String>,
}

impl Default for MemoryOverlayState {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryOverlayState {
    pub fn new() -> Self {
        let items = Self::default_system_memories();
        let mut list_state = ListState::default();
        if !items.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            items,
            search_query: String::new(),
            is_searching: false,
            list_state,
            selected_scope_filter: None,
            status_message: None,
        }
    }

    pub fn with_items(items: Vec<MemoryItem>) -> Self {
        let mut list_state = ListState::default();
        if !items.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            items,
            search_query: String::new(),
            is_searching: false,
            list_state,
            selected_scope_filter: None,
            status_message: None,
        }
    }

    pub fn default_system_memories() -> Vec<MemoryItem> {
        vec![
            MemoryItem {
                id: "mem-01".to_string(),
                scope: MemoryScope::Workspace,
                topic: "String Slicing UTF-8 Boundaries".to_string(),
                content: "Never slice UTF-8 strings by raw byte indices (&s[..len]). Always use s.floor_char_boundary(len) to prevent runtime panics on multibyte Unicode characters and emojis.".to_string(),
                counter_pattern: Some("&s[..len] or &text[0..max]".to_string()),
                correct_pattern: Some("&s[..s.floor_char_boundary(len.min(s.len()))]".to_string()),
                tags: vec!["rust".to_string(), "safety".to_string(), "unicode".to_string()],
                confidence: 0.99,
                access_count: 14,
            },
            MemoryItem {
                id: "mem-02".to_string(),
                scope: MemoryScope::CounterRule,
                topic: "Subprocess Safety & Zombie Prevention".to_string(),
                content: "Always wrap async subprocess execution in tokio::time::timeout and guarantee child.kill().await and child.wait().await are called on abort or timeout.".to_string(),
                counter_pattern: Some("std::process::Command without timeout or unhandled child handle".to_string()),
                correct_pattern: Some("tokio::process::Command with 120s timeout and child.kill().await on timeout".to_string()),
                tags: vec!["async".to_string(), "process".to_string(), "safety".to_string()],
                confidence: 0.98,
                access_count: 22,
            },
            MemoryItem {
                id: "mem-03".to_string(),
                scope: MemoryScope::Global,
                topic: "Zero Warnings Clippy Policy".to_string(),
                content: "All code in pi-rust workspace must strictly pass cargo clippy --workspace --all-targets -- -D warnings with zero warnings.".to_string(),
                counter_pattern: Some("Allowing #[allow(clippy::...)] indiscriminately".to_string()),
                correct_pattern: Some("Refactoring code to idiomatic Rust 2024 patterns".to_string()),
                tags: vec!["quality".to_string(), "clippy".to_string(), "standards".to_string()],
                confidence: 1.0,
                access_count: 45,
            },
            MemoryItem {
                id: "mem-04".to_string(),
                scope: MemoryScope::Workspace,
                topic: "JSON-RPC 2.0 Stdout Hygiene".to_string(),
                content: "In --rpc mode, never emit human-readable logs to stdout. Stdout is reserved exclusively for valid JSON-RPC frames. Operational logs must route to eprintln!.".to_string(),
                counter_pattern: Some("println!(\"debug: ...\") in RPC server code paths".to_string()),
                correct_pattern: Some("eprintln!(\"[rpc] debug: ...\") for internal logging".to_string()),
                tags: vec!["rpc".to_string(), "protocol".to_string(), "stdout".to_string()],
                confidence: 0.96,
                access_count: 8,
            },
            MemoryItem {
                id: "mem-05".to_string(),
                scope: MemoryScope::Episodic,
                topic: "Session DAG Message Causality".to_string(),
                content: "Always append Role::Assistant containing tool call metadata to SessionTree BEFORE executing tools and appending Role::Tool results.".to_string(),
                counter_pattern: Some("Appending Tool output before Assistant tool call node".to_string()),
                correct_pattern: Some("Session causal chain: User -> Assistant (with tool_calls) -> Tool (with tool_call_id)".to_string()),
                tags: vec!["session".to_string(), "dag".to_string(), "causality".to_string()],
                confidence: 0.99,
                access_count: 19,
            },
            MemoryItem {
                id: "mem-06".to_string(),
                scope: MemoryScope::CounterRule,
                topic: "Edit Tool Unambiguity Invariant".to_string(),
                content: "The edit tool must verify target pattern exists and occurs exactly once in the file. Reject edit if target occurrences != 1 to prevent accidental multi-replace.".to_string(),
                counter_pattern: Some("Replacing first match blindly without occurrence check".to_string()),
                correct_pattern: Some("assert occurrences == 1 before performing replacement".to_string()),
                tags: vec!["tools".to_string(), "edit".to_string(), "safety".to_string()],
                confidence: 0.95,
                access_count: 12,
            },
        ]
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        let q = self.search_query.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if let Some(scope_filter) = self.selected_scope_filter
                    && item.scope != scope_filter
                {
                    return false;
                }
                if q.is_empty() {
                    return true;
                }
                item.topic.to_lowercase().contains(&q)
                    || item.content.to_lowercase().contains(&q)
                    || item.tags.iter().any(|t| t.to_lowercase().contains(&q))
                    || item.counter_pattern.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    || item.correct_pattern.as_deref().unwrap_or("").to_lowercase().contains(&q)
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    pub fn selected_filtered_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn selected_memory(&self) -> Option<&MemoryItem> {
        let indices = self.filtered_indices();
        let selected = self.list_state.selected()?;
        let original_idx = *indices.get(selected)?;
        self.items.get(original_idx)
    }

    pub fn handle_navigation(&mut self, up: bool) {
        let total = self.filtered_indices().len();
        if total == 0 {
            self.list_state.select(None);
            return;
        }

        let next = match self.list_state.selected() {
            Some(i) => {
                if up {
                    if i == 0 { total - 1 } else { i - 1 }
                } else if i >= total - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };

        self.list_state.select(Some(next));
    }

    pub fn delete_selected(&mut self) -> Option<MemoryItem> {
        let indices = self.filtered_indices();
        let selected = self.list_state.selected()?;
        if selected >= indices.len() {
            return None;
        }
        let original_idx = indices[selected];
        let removed = self.items.remove(original_idx);
        self.status_message = Some(format!("Deleted memory: [{}] {}", removed.id, removed.topic));

        let new_total = self.filtered_indices().len();
        if new_total == 0 {
            self.list_state.select(None);
        } else if selected >= new_total {
            self.list_state.select(Some(new_total - 1));
        }
        Some(removed)
    }

    pub fn cycle_scope_filter(&mut self) {
        self.selected_scope_filter = match self.selected_scope_filter {
            None => Some(MemoryScope::Global),
            Some(MemoryScope::Global) => Some(MemoryScope::Workspace),
            Some(MemoryScope::Workspace) => Some(MemoryScope::Episodic),
            Some(MemoryScope::Episodic) => Some(MemoryScope::CounterRule),
            Some(MemoryScope::CounterRule) => None,
        };
        let new_total = self.filtered_indices().len();
        if new_total == 0 {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    pub fn add_tag_to_selected(&mut self, tag: &str) -> bool {
        let indices = self.filtered_indices();
        if let Some(selected) = self.list_state.selected()
            && let Some(&orig_idx) = indices.get(selected)
            && let Some(item) = self.items.get_mut(orig_idx)
        {
            let tag_clean = tag.trim().to_string();
            if !tag_clean.is_empty() && !item.tags.contains(&tag_clean) {
                item.tags.push(tag_clean);
                return true;
            }
        }
        false
    }
}

pub struct MemoryOverlayWidget;

impl MemoryOverlayWidget {
    pub fn render(
        state: &MemoryOverlayState,
        f: &mut Frame,
        area: Rect,
        theme: &ThemePalette,
    ) {
        f.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.accent))
            .title(" 🧠 Tau Cognitive Memory Vault Explorer (TauVault) ")
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(theme.bg));

        let inner_area = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search & Scope filter header
                Constraint::Min(6),    // Main list & detail split view
                Constraint::Length(2), // Bottom cheatsheet / status bar
            ])
            .margin(1)
            .split(inner_area);

        // 1. Search Bar & Scope Filter Header
        let search_cursor = if state.is_searching { "▏" } else { "" };
        let scope_indicator = match state.selected_scope_filter {
            None => "[All Scopes]",
            Some(s) => s.display_name(),
        };

        let header_spans = vec![
            Span::styled(" Search: ", Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("> {}{} ", state.search_query, search_cursor), Style::default().fg(theme.text)),
            Span::styled(format!("· Scope: [{}] ", scope_indicator), Style::default().fg(theme.cyan).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("({} memories stored)", state.items.len()),
                Style::default().fg(theme.muted),
            ),
        ];

        let search_box = Paragraph::new(Line::from(header_spans))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(theme.border)),
            );
        f.render_widget(search_box, chunks[0]);

        // 2. Main Content Split View (Left: List, Right: Memory Inspector)
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(chunks[1]);

        let filtered_indices = state.filtered_indices();

        let list_items: Vec<ListItem> = filtered_indices
            .iter()
            .map(|&idx| {
                let item = &state.items[idx];
                let scope_color = match item.scope {
                    MemoryScope::Global => theme.accent,
                    MemoryScope::Workspace => theme.cyan,
                    MemoryScope::Episodic => theme.magenta,
                    MemoryScope::CounterRule => theme.red,
                };

                let badge = item.scope.badge_str();
                let topic_preview = if item.topic.len() > 28 {
                    let cut = item.topic.floor_char_boundary(28);
                    format!("{}…", &item.topic[..cut])
                } else {
                    item.topic.clone()
                };

                let conf_pct = (item.confidence * 100.0).round() as u8;

                let line = Line::from(vec![
                    Span::styled(format!("{} ", badge), Style::default().fg(scope_color).add_modifier(Modifier::BOLD)),
                    Span::styled(topic_preview, Style::default().fg(theme.text)),
                    Span::styled(format!(" ({}%)", conf_pct), Style::default().fg(theme.muted)),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list_title = format!(" Memories ({}) ", filtered_indices.len());
        let list_widget = List::new(list_items)
            .block(
                Block::default()
                    .borders(Borders::RIGHT)
                    .border_style(Style::default().fg(theme.border))
                    .title(list_title),
            )
            .highlight_style(
                Style::default()
                    .bg(theme.accent)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        // We render stateful list with a clone of state's list_state
        let mut list_state_clone = state.list_state.clone();
        f.render_stateful_widget(list_widget, main_chunks[0], &mut list_state_clone);

        // Detail Inspector View
        let detail_block = Block::default()
            .borders(Borders::NONE)
            .title(" Memory Inspector ")
            .title_style(Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD));

        if let Some(selected_item) = state.selected_memory() {
            let mut detail_lines = Vec::new();

            detail_lines.push(Line::from(vec![
                Span::styled("Topic: ", Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD)),
                Span::styled(&selected_item.topic, Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
            ]));

            let scope_color = match selected_item.scope {
                MemoryScope::Global => theme.accent,
                MemoryScope::Workspace => theme.cyan,
                MemoryScope::Episodic => theme.magenta,
                MemoryScope::CounterRule => theme.red,
            };

            detail_lines.push(Line::from(vec![
                Span::styled("Scope: ", Style::default().fg(theme.muted)),
                Span::styled(selected_item.scope.display_name(), Style::default().fg(scope_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ID: {}", selected_item.id), Style::default().fg(theme.muted)),
                Span::styled(format!("  Recalls: {}", selected_item.access_count), Style::default().fg(theme.muted)),
                Span::styled(format!("  Confidence: {:.0}%", selected_item.confidence * 100.0), Style::default().fg(theme.green)),
            ]));

            if !selected_item.tags.is_empty() {
                let tags_str = selected_item.tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ");
                detail_lines.push(Line::from(vec![
                    Span::styled("Tags: ", Style::default().fg(theme.muted)),
                    Span::styled(tags_str, Style::default().fg(theme.cyan)),
                ]));
            }

            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(vec![
                Span::styled("Rule / Insight:", Style::default().fg(theme.text).add_modifier(Modifier::UNDERLINED)),
            ]));
            detail_lines.push(Line::from(Span::styled(&selected_item.content, Style::default().fg(theme.text))));

            if let Some(ref bad) = selected_item.counter_pattern {
                detail_lines.push(Line::from(""));
                detail_lines.push(Line::from(vec![
                    Span::styled("✖ Anti-Pattern to Avoid: ", Style::default().fg(theme.red).add_modifier(Modifier::BOLD)),
                    Span::styled(bad, Style::default().fg(theme.red)),
                ]));
            }

            if let Some(ref good) = selected_item.correct_pattern {
                detail_lines.push(Line::from(vec![
                    Span::styled("✔ Verified Correct Pattern: ", Style::default().fg(theme.green).add_modifier(Modifier::BOLD)),
                    Span::styled(good, Style::default().fg(theme.green)),
                ]));
            }

            let detail_paragraph = Paragraph::new(detail_lines)
                .block(detail_block)
                .wrap(Wrap { trim: false });
            f.render_widget(detail_paragraph, main_chunks[1]);
        } else {
            let empty_msg = Paragraph::new(Line::from(vec![
                Span::styled("No memories match current search or filter.", Style::default().fg(theme.muted)),
            ]))
            .block(detail_block);
            f.render_widget(empty_msg, main_chunks[1]);
        }

        // 3. Bottom Cheatsheet / Status Bar
        let status_text = if let Some(ref msg) = state.status_message {
            format!(" {}", msg)
        } else {
            String::new()
        };

        let footer_spans = vec![
            Span::styled("[↑/↓: Navigate · /: Search · d: Delete · t: Scope Filter · Esc: Close]", Style::default().fg(theme.accent)),
            Span::styled(status_text, Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD)),
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
    fn test_memory_overlay_state_navigation() {
        let mut state = MemoryOverlayState::new();
        assert!(!state.items.is_empty());
        assert_eq!(state.selected_filtered_index(), Some(0));

        let total = state.filtered_indices().len();
        state.handle_navigation(false);
        assert_eq!(state.selected_filtered_index(), Some(1));

        state.handle_navigation(true);
        assert_eq!(state.selected_filtered_index(), Some(0));

        // Wrap around backward
        state.handle_navigation(true);
        assert_eq!(state.selected_filtered_index(), Some(total - 1));
    }

    #[test]
    fn test_memory_overlay_search_filtering() {
        let mut state = MemoryOverlayState::new();
        state.search_query = "slicing".to_string();
        let filtered = state.filtered_indices();
        assert!(!filtered.is_empty());
        assert!(state.items[filtered[0]].topic.to_lowercase().contains("slicing"));

        state.search_query = "nonexistent_query_xyz".to_string();
        assert!(state.filtered_indices().is_empty());
    }

    #[test]
    fn test_memory_overlay_scope_cycling() {
        let mut state = MemoryOverlayState::new();
        assert_eq!(state.selected_scope_filter, None);

        state.cycle_scope_filter();
        assert_eq!(state.selected_scope_filter, Some(MemoryScope::Global));

        state.cycle_scope_filter();
        assert_eq!(state.selected_scope_filter, Some(MemoryScope::Workspace));

        state.cycle_scope_filter();
        assert_eq!(state.selected_scope_filter, Some(MemoryScope::Episodic));

        state.cycle_scope_filter();
        assert_eq!(state.selected_scope_filter, Some(MemoryScope::CounterRule));

        state.cycle_scope_filter();
        assert_eq!(state.selected_scope_filter, None);
    }

    #[test]
    fn test_memory_overlay_delete_item() {
        let mut state = MemoryOverlayState::new();
        let initial_count = state.items.len();
        state.list_state.select(Some(0));

        let deleted = state.delete_selected();
        assert!(deleted.is_some());
        assert_eq!(state.items.len(), initial_count - 1);
        assert!(state.status_message.is_some());
    }

    #[test]
    fn test_memory_overlay_add_tag() {
        let mut state = MemoryOverlayState::new();
        state.list_state.select(Some(0));
        let added = state.add_tag_to_selected("verified");
        assert!(added);
        assert!(state.selected_memory().unwrap().tags.contains(&"verified".to_string()));
    }
}
