use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

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
    ("catppuccin", "Soothing macchiato palette with warm sapphire"),
    ("gruvbox", "Retro groove warm dark palette with earthy tones"),
    ("nord", "Arctic ice blue with serene frosty hues"),
    ("solarized", "Precision dark solarized teal and cyan contrasts"),
    ("onedark", "Iconic balanced dark aesthetic with vibrant syntax"),
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
        if suggestions.is_empty() {
            return;
        }

        let count = suggestions.len().min(6);
        let height = (count as u16) + 2;
        let width = input_rect.width.min(75);

        // Position directly above the prompt box
        let y = input_rect.y.saturating_sub(height);
        let x = input_rect.x;
        let area = Rect {
            x,
            y,
            width,
            height,
        };

        f.render_widget(Clear, area);

        let lines: Vec<Line> = suggestions
            .iter()
            .take(6)
            .enumerate()
            .map(|(idx, (name, sig, desc))| {
                let is_selected = idx == selected_index;
                let bg_style = if is_selected {
                    Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let name_style = if is_selected {
                    Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                };

                let desc_style = if is_selected {
                    Style::default().bg(Color::Cyan).fg(Color::Rgb(40, 40, 40))
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                Line::from(vec![
                    Span::styled(if is_selected { "▶ " } else { "  " }, bg_style),
                    Span::styled(format!("{:<14}", name), name_style),
                    Span::styled(format!(" {:<24}", sig), bg_style),
                    Span::styled(format!(" {}", desc), desc_style),
                ])
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Commands (Tab: Select · Up/Down: Navigate · Esc: Close) ");

        let popup = Paragraph::new(lines).block(block);
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
    }
}
