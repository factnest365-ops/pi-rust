use crate::style::ThemePalette;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

#[derive(Debug, Clone)]
pub struct HelpItem {
    pub category: &'static str,
    pub key_or_cmd: &'static str,
    pub description: &'static str,
    pub details: &'static str,
}

pub const HELP_CATEGORIES: &[&str] = &[
    "Navigation & Shortcuts",
    "Slash Commands",
    "Dual Tool Calling & Worktree Architecture",
];

pub const ALL_HELP_ITEMS: &[HelpItem] = &[
    // Navigation & Shortcuts
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Ctrl+L",
        description: "Open AI Model Picker",
        details: "Live searchable catalog across 33+ providers and local daemons",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Ctrl+P",
        description: "Open AI Provider Picker",
        details: "Browse 25+ gateways (Anthropic, OpenAI, Gemini, DeepSeek, Ollama, etc.)",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Ctrl+T",
        description: "Open Theme Picker",
        details: "Switch between 7 aesthetic color themes with live preview",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Ctrl+R",
        description: "Refresh Catalogs",
        details: "Live background scan for local LLMs (Ollama, LM Studio) & remote models",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Ctrl+N",
        description: "Start New Session",
        details: "Clear active turn context and reset conversation DAG tree",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Ctrl+O",
        description: "Toggle Tool Drawer",
        details: "Expand or collapse tool execution details and stderr output",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Ctrl+K",
        description: "Clear Terminal Screen",
        details: "Clear transcript viewport while preserving session history",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Ctrl+A",
        description: "Auth & Login Wizard",
        details: "Interactive API key configuration wizard for providers",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "? / F1",
        description: "Toggle Help Overlay",
        details: "Open this searchable interactive cheatsheet manual",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Esc / Ctrl+C",
        description: "Cancel / Abort Execution",
        details: "Interrupt active streaming turn or close open modal overlays",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Alt+Enter",
        description: "Queue Message",
        details: "Queue prompt to execute immediately after the active turn finishes",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Shift+Enter",
        description: "Insert Newline",
        details: "Add a line break in multi-line prompt input box",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "Tab",
        description: "Autocomplete",
        details: "Accept slash command or provider name suggestion from popup",
    },
    HelpItem {
        category: "Navigation & Shortcuts",
        key_or_cmd: "PageUp / PageDown",
        description: "Scroll Viewport",
        details: "Scroll up or down in chat transcript or help list",
    },

    // Slash Commands
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/diff [file]",
        description: "Unified Diff Inspector",
        details: "Visual side-by-side diff of pending edits or git changes with Y/N review",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/tree",
        description: "Session DAG Navigator",
        details: "Interactive visual time-travel navigator to rewind or fork history",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/replay",
        description: "Session Replay",
        details: "Step-by-step turn playback of current session trajectory",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/fork",
        description: "Fork Session Branch",
        details: "Create a new branch in the session DAG at current active node",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/new",
        description: "Start New Session",
        details: "Reset session DAG tree and start with clean context",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/session",
        description: "Session & Token Metrics",
        details: "Display token capacity, context window limit, and active node count",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/compact",
        description: "Compact Context Window",
        details: "Trigger context summarization and archive older turns into summary node",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/tools",
        description: "Toggle Tool Execution Drawer",
        details: "Expand or collapse tool call parameters and output blocks",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/thinking",
        description: "Toggle Chain-of-Thought",
        details: "Show or hide model reasoning and thinking blocks (DeepSeek R1 / Claude)",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/login <p> [key]",
        description: "Provider Authentication",
        details: "Authenticate provider credentials and persist to ~/.pi/config.json",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/auth",
        description: "API Key Setup",
        details: "Interactive API key setup wizard for currently active model provider",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/models [name]",
        description: "Search & Switch AI Model",
        details: "Open interactive model browser or switch directly by name/id",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/skills",
        description: "Agent Skills Registry",
        details: "List all auto-discovered agent skills and operational instructions",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/mcp [refresh]",
        description: "Discovered MCP Servers",
        details: "List active Model Context Protocol tools and scan client configs",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/theme [name]",
        description: "Theme Engine & Switcher",
        details: "Switch active color scheme (TokyoNight, Catppuccin, Gruvbox, Nord, etc.)",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/export [file.md]",
        description: "Export Conversation",
        details: "Export full formatted Markdown transcript of current session",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/clear",
        description: "Clear Transcript View",
        details: "Clear terminal screen output without losing session state",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/help",
        description: "Interactive Help Cheatsheet",
        details: "Display searchable cheatsheet with all hotkeys, commands & features",
    },
    HelpItem {
        category: "Slash Commands",
        key_or_cmd: "/quit",
        description: "Exit pi-rust Agent",
        details: "Graceful shutdown and save session state to disk",
    },

    // Dual Tool Calling & Worktree Architecture
    HelpItem {
        category: "Dual Tool Calling & Worktree Architecture",
        key_or_cmd: "Frontier Native Tool Calling",
        description: "JSON Schema Protocols",
        details: "Native function calling with tool_call_id for OpenAI, Anthropic, Gemini, DeepSeek",
    },
    HelpItem {
        category: "Dual Tool Calling & Worktree Architecture",
        key_or_cmd: "Local Markdown Fallback",
        description: "Fenced Code Tool Extraction",
        details: "Dual-protocol markdown parsing for Ollama, LM Studio, llamacpp, vLLM models",
    },
    HelpItem {
        category: "Dual Tool Calling & Worktree Architecture",
        key_or_cmd: "Isolated Git Worktrees",
        description: "Parallel Subagent Isolation",
        details: "Branching workspace worktrees with conflict diagnostics and auto-pruning",
    },
    HelpItem {
        category: "Dual Tool Calling & Worktree Architecture",
        key_or_cmd: "Subprocess Safety & Timeouts",
        description: "Process Tree Kill Guarantees",
        // Details intentionally static because `HelpItem.details` is `&'static str`.
        // The shared timeout value lives in `pi_core::plan::VERIFY_TIMEOUT_SECS`.
        details: "Async 120s execution timeout with SIGKILL cleanup to prevent zombie processes",
    },
];

#[derive(Debug, Clone, Default)]
pub struct HelpOverlayState {
    pub search_query: String,
    pub scroll_offset: u16,
}

impl HelpOverlayState {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            scroll_offset: 0,
        }
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: u16, max_lines: usize, viewport_height: u16) {
        let max_scroll = max_lines.saturating_sub(viewport_height as usize) as u16;
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
    }

    pub fn scroll_home(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_end(&mut self, max_lines: usize, viewport_height: u16) {
        self.scroll_offset = max_lines.saturating_sub(viewport_height as usize) as u16;
    }
}

pub struct HelpOverlay;

impl HelpOverlay {
    pub fn filter_items(query: &str) -> Vec<&'static HelpItem> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            ALL_HELP_ITEMS.iter().collect()
        } else {
            ALL_HELP_ITEMS
                .iter()
                .filter(|item| {
                    item.key_or_cmd.to_lowercase().contains(&q)
                        || item.description.to_lowercase().contains(&q)
                        || item.details.to_lowercase().contains(&q)
                        || item.category.to_lowercase().contains(&q)
                })
                .collect()
        }
    }

    pub fn render(
        f: &mut Frame,
        area: Rect,
        state: &HelpOverlayState,
        palette: &ThemePalette,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette.accent))
            .title(" 📖 Pi Coding Agent — Interactive Cheatsheet & Manual ")
            .title_alignment(Alignment::Center);

        let inner_area = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Search bar
                Constraint::Min(4),    // Cheatsheet content
                Constraint::Length(1), // Footer navigation hints
            ])
            .split(inner_area);

        let filtered = Self::filter_items(&state.search_query);

        // Search Bar
        let search_display = format!("> {}▏", state.search_query);
        let search_bar = Paragraph::new(Line::from(vec![
            Span::styled(
                " Filter Cheatsheet: ",
                Style::default()
                    .fg(palette.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(search_display, Style::default().fg(palette.text)),
            Span::styled(
                format!(" (Showing {} of {} entries)", filtered.len(), ALL_HELP_ITEMS.len()),
                Style::default().fg(palette.muted),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(palette.border)),
        );
        f.render_widget(search_bar, chunks[0]);

        // Build Cheatsheet Lines
        let mut lines: Vec<Line> = Vec::new();

        for category in HELP_CATEGORIES {
            let cat_items: Vec<&&HelpItem> = filtered
                .iter()
                .filter(|item| item.category == *category)
                .collect();

            if cat_items.is_empty() {
                continue;
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" ═══ {} ═══", category),
                    Style::default()
                        .fg(palette.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(""));

            for item in cat_items {
                lines.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        format!("{:<28}", item.key_or_cmd),
                        Style::default()
                            .fg(palette.yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<32}", item.description),
                        Style::default()
                            .fg(palette.text)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        item.details,
                        Style::default().fg(palette.muted),
                    ),
                ]));
            }
        }

        if lines.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    "   No matching commands or shortcuts found for query.",
                    Style::default().fg(palette.muted),
                ),
            ]));
        }

        let total_lines = lines.len();
        let viewport_height = chunks[1].height;
        let content_paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((state.scroll_offset, 0))
            .style(Style::default().bg(palette.bg));
        f.render_widget(content_paragraph, chunks[1]);

        // Footer Hints
        let footer_text = format!(
            " Type to search · Up/Down/PageUp/PageDown: Scroll ({}/{}) · Esc/q/F1: Close ",
            state.scroll_offset.min(total_lines as u16),
            total_lines.saturating_sub(viewport_height as usize)
        );
        let footer = Paragraph::new(Line::from(vec![
            Span::styled(footer_text, Style::default().fg(palette.muted).bg(palette.surface)),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(footer, chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_items_filtering() {
        let all = HelpOverlay::filter_items("");
        assert_eq!(all.len(), ALL_HELP_ITEMS.len());

        let model_filter = HelpOverlay::filter_items("model");
        assert!(model_filter.iter().any(|i| i.key_or_cmd == "Ctrl+L"));
        assert!(model_filter.iter().any(|i| i.key_or_cmd == "/models [name]"));

        let diff_filter = HelpOverlay::filter_items("diff");
        assert_eq!(diff_filter.len(), 1);
        assert_eq!(diff_filter[0].key_or_cmd, "/diff [file]");
    }

    #[test]
    fn test_help_overlay_state_scroll() {
        let mut state = HelpOverlayState::new();
        assert_eq!(state.scroll_offset, 0);

        state.scroll_down(5, 50, 20);
        assert_eq!(state.scroll_offset, 5);

        state.scroll_up(2);
        assert_eq!(state.scroll_offset, 3);

        state.scroll_home();
        assert_eq!(state.scroll_offset, 0);

        state.scroll_end(50, 20);
        assert_eq!(state.scroll_offset, 30);
    }
}
