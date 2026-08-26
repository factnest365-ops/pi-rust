use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: &'static str,
    pub signature: &'static str,
    pub description: &'static str,
}

pub const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/model",
        signature: "/model [name]",
        description: "Search & switch AI models (Ctrl+L)",
    },
    SlashCommand {
        name: "/models",
        signature: "/models [name]",
        description: "Search & switch AI models (Ctrl+L)",
    },
    SlashCommand {
        name: "/provider",
        signature: "/provider [name]",
        description: "Browse & switch AI providers (Ctrl+P)",
    },
    SlashCommand {
        name: "/login",
        signature: "/login <provider> [key]",
        description: "Authenticate provider credentials (Ctrl+A)",
    },
    SlashCommand {
        name: "/auth",
        signature: "/auth",
        description: "Interactive API Key Setup Wizard",
    },
    SlashCommand {
        name: "/theme",
        signature: "/theme [name]",
        description: "Open theme picker or set theme (Ctrl+T)",
    },
    SlashCommand {
        name: "/mcp",
        signature: "/mcp [refresh]",
        description: "List discovered Model Context Protocol servers",
    },
    SlashCommand {
        name: "/skills",
        signature: "/skills",
        description: "List auto-discovered agent skills and rules",
    },
    SlashCommand {
        name: "/tree",
        signature: "/tree",
        description: "Interactive DAG session time-travel navigator",
    },
    SlashCommand {
        name: "/replay",
        signature: "/replay",
        description: "Step-by-step playback of session turns",
    },
    SlashCommand {
        name: "/fork",
        signature: "/fork",
        description: "Fork current session into a new branch",
    },
    SlashCommand {
        name: "/new",
        signature: "/new",
        description: "Start a fresh session with empty context (Ctrl+N)",
    },
    SlashCommand {
        name: "/session",
        signature: "/session",
        description: "Inspect context window capacity & token metrics",
    },
    SlashCommand {
        name: "/compact",
        signature: "/compact",
        description: "Trigger context summarization & compaction",
    },
    SlashCommand {
        name: "/refresh",
        signature: "/refresh",
        description: "Force live refresh of models & local daemons (Ctrl+R)",
    },
    SlashCommand {
        name: "/tools",
        signature: "/tools",
        description: "Toggle collapsible tool drawer (Ctrl+O)",
    },
    SlashCommand {
        name: "/thinking",
        signature: "/thinking",
        description: "Toggle model chain-of-thought blocks",
    },
    SlashCommand {
        name: "/export",
        signature: "/export [file.md]",
        description: "Export current conversation to Markdown",
    },
    SlashCommand {
        name: "/diff",
        signature: "/diff [file]",
        description: "Inspect visual unified diff of pending file edits or git changes",
    },
    SlashCommand {
        name: "/memory",
        signature: "/memory [query]",
        description: "Search & explore TauVault cognitive memories & rules",
    },
    SlashCommand {
        name: "/plan",
        signature: "/plan [toggle|status]",
        description: "Interactive task checklist & verification status",
    },
    SlashCommand {
        name: "/ask",
        signature: "/ask [question]",
        description: "Interactive clarification questionnaire modal",
    },
    SlashCommand {
        name: "/clear",
        signature: "/clear",
        description: "Clear terminal transcript screen (Ctrl+K)",
    },
    SlashCommand {
        name: "/help",
        signature: "/help",
        description: "Display cheatsheet & keyboard shortcuts (?, F1)",
    },
    SlashCommand {
        name: "/quit",
        signature: "/quit",
        description: "Exit pi agent (Ctrl+D)",
    },
];

pub const KNOWN_PROVIDERS: &[&str] = &[
    "anthropic",
    "openai",
    "gemini",
    "deepseek",
    "groq",
    "openrouter",
    "mistral",
    "cerebras",
    "copilot",
    "bedrock",
    "xai",
    "together",
    "fireworks",
    "perplexity",
    "qwen",
    "xiaomi",
    "moonshot",
    "huggingface",
    "ollama",
    "lmstudio",
    "llamacpp",
    "vllm",
    "opencode",
    "kilo",
    "agnes",
];

pub const KNOWN_THEMES: &[(&str, &str)] = &[
    ("default", "Clean GitHub dark with vibrant blue accents"),
    ("tokyonight", "Deep indigo with neon pastel accents"),
    (
        "catppuccin",
        "Soothing macchiato palette with warm sapphire",
    ),
    (
        "gruvbox",
        "Retro groove warm dark palette with earthy tones",
    ),
    ("nord", "Arctic ice blue with serene frosty hues"),
    (
        "solarized",
        "Precision dark solarized teal and cyan contrasts",
    ),
    (
        "onedark",
        "Iconic balanced dark aesthetic with vibrant syntax",
    ),
];

pub struct AutocompleteEngine;

impl AutocompleteEngine {
    pub fn get_suggestions(input: &str) -> Vec<(&'static str, &'static str, &'static str)> {
        let trimmed = input.trim_start();
        if !trimmed.starts_with('/') {
            return Vec::new();
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let cmd = parts.first().copied().unwrap_or(trimmed);

        if parts.len() <= 1 && !trimmed.ends_with(' ') {
            // Matching slash command name
            COMMANDS
                .iter()
                .filter(|c| c.name.starts_with(cmd))
                .map(|c| (c.name, c.signature, c.description))
                .collect()
        } else if cmd == "/login" || cmd == "/provider" {
            // Suggesting provider names
            let prov_query = if trimmed.ends_with(' ') {
                ""
            } else {
                parts.get(1).copied().unwrap_or("")
            };
            KNOWN_PROVIDERS
                .iter()
                .filter(|p| p.starts_with(prov_query))
                .map(|p| (*p, *p, "AI Provider Gateway"))
                .collect()
        } else if cmd == "/theme" {
            // Suggesting theme names
            let theme_query = if trimmed.ends_with(' ') {
                ""
            } else {
                parts.get(1).copied().unwrap_or("")
            };
            KNOWN_THEMES
                .iter()
                .filter(|(id, _)| id.starts_with(theme_query))
                .map(|(id, desc)| (*id, *id, *desc))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn render_popup(
        f: &mut Frame,
        suggestions: &[(&'static str, &'static str, &'static str)],
        selected_index: usize,
        input_rect: Rect,
    ) {
        let default_theme = crate::style::ThemePalette::default();
        Self::render_popup_styled(f, suggestions, selected_index, input_rect, &default_theme);
    }

    pub fn render_popup_styled(
        f: &mut Frame,
        suggestions: &[(&'static str, &'static str, &'static str)],
        selected_index: usize,
        input_rect: Rect,
        theme: &crate::style::ThemePalette,
    ) {
        if suggestions.is_empty() {
            return;
        }

        let total = suggestions.len();
        let count = total.min(6);
        let height = (count as u16) + 2;
        let width = input_rect
            .width
            .max(60)
            .min(f.area().width.saturating_sub(input_rect.x));

        // Position directly UNDER the prompt box (omp / oh-my-pi design)
        let y = if input_rect.y + input_rect.height + height <= f.area().height {
            input_rect.y + input_rect.height
        } else {
            input_rect.y.saturating_sub(height)
        };
        let x = input_rect.x;
        let area = Rect {
            x,
            y,
            width,
            height,
        };

        f.render_widget(Clear, area);

        // Window the suggestions based on selected_index
        let start = if selected_index >= 6 {
            selected_index - 6 + 1
        } else {
            0
        };
        let end = (start + 6).min(total);
        let visible_items = &suggestions[start..end];

        let lines: Vec<Line> = visible_items
            .iter()
            .enumerate()
            .map(|(rel_idx, (name, sig, desc))| {
                let actual_idx = start + rel_idx;
                let is_selected = actual_idx == selected_index;

                if is_selected {
                    Line::from(vec![
                        Span::styled(
                            " › ",
                            Style::default()
                                .fg(theme.accent)
                                .bg(theme.surface)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{:<12}", name),
                            Style::default()
                                .fg(theme.accent)
                                .bg(theme.surface)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!(" {:<20}", sig),
                            Style::default().fg(theme.text).bg(theme.surface),
                        ),
                        Span::styled(
                            format!(" {}", desc),
                            Style::default().fg(theme.muted).bg(theme.surface),
                        ),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled("   ", Style::default().fg(theme.muted).bg(theme.bg)),
                        Span::styled(
                            format!("{:<12}", name),
                            Style::default()
                                .fg(theme.text)
                                .bg(theme.bg)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!(" {:<20}", sig),
                            Style::default().fg(theme.muted).bg(theme.bg),
                        ),
                        Span::styled(
                            format!(" {}", desc),
                            Style::default().fg(theme.border).bg(theme.bg),
                        ),
                    ])
                }
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border).bg(theme.bg))
            .title(Span::styled(
                " Commands (Tab: Select · Up/Down: Navigate · Esc: Close) ",
                Style::default().fg(theme.accent),
            ));

        let popup = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme.bg));
        f.render_widget(popup, area);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_autocomplete_slash_matching() {
        let suggestions = AutocompleteEngine::get_suggestions("/");
        assert!(!suggestions.is_empty());
        assert!(suggestions.len() > 6); // More than 6 suggestions
        assert!(suggestions.iter().any(|(name, _, _)| *name == "/model"));
        assert!(suggestions.iter().any(|(name, _, _)| *name == "/provider"));
        assert!(suggestions.iter().any(|(name, _, _)| *name == "/theme"));
        assert!(suggestions.iter().any(|(name, _, _)| *name == "/help"));

        let mod_suggestions = AutocompleteEngine::get_suggestions("/mod");
        assert_eq!(mod_suggestions.len(), 2); // /model and /models
        assert!(mod_suggestions.iter().any(|s| s.0 == "/model"));
        assert!(mod_suggestions.iter().any(|s| s.0 == "/models"));
    }

    #[test]
    fn test_autocomplete_theme_arguments() {
        let suggestions = AutocompleteEngine::get_suggestions("/theme ");
        assert_eq!(suggestions.len(), KNOWN_THEMES.len());

        let tokyo_suggestions = AutocompleteEngine::get_suggestions("/theme tok");
        assert_eq!(tokyo_suggestions.len(), 1);
        assert_eq!(tokyo_suggestions[0].0, "tokyonight");
    }

    #[test]
    fn test_autocomplete_provider_arguments() {
        let prov_suggestions = AutocompleteEngine::get_suggestions("/login ant");
        assert!(!prov_suggestions.is_empty());
        assert_eq!(prov_suggestions[0].0, "anthropic");

        let prov_suggestions_space = AutocompleteEngine::get_suggestions("/provider ");
        assert!(prov_suggestions_space.len() >= 10);
    }

    #[test]
    fn test_autocomplete_non_slash_input() {
        let non_slash = AutocompleteEngine::get_suggestions("Hello agent");
        assert!(non_slash.is_empty());

        let empty = AutocompleteEngine::get_suggestions("");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_autocomplete_windowed_pagination_bounds() {
        let suggestions = AutocompleteEngine::get_suggestions("/");
        let total = suggestions.len();
        assert!(total > 6);

        // Test start/end calculations for various selected indices
        for selected in 0..total {
            let start = if selected >= 6 { selected - 6 + 1 } else { 0 };
            let end = (start + 6).min(total);
            assert!(start <= selected);
            assert!(selected < end);
            assert!(end - start <= 6);
        }
    }
}
