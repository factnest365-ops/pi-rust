use pi_providers::AuthResolver;
use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};

#[derive(Debug, Clone)]
pub struct ProviderEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub endpoint: &'static str,
    pub is_local: bool,
}

pub const ALL_PROVIDERS: &[ProviderEntry] = &[
    ProviderEntry { id: "anthropic", name: "Anthropic Claude", endpoint: "https://api.anthropic.com/v1", is_local: false },
    ProviderEntry { id: "openai", name: "OpenAI Frontier", endpoint: "https://api.openai.com/v1", is_local: false },
    ProviderEntry { id: "gemini", name: "Google Gemini", endpoint: "https://generativelanguage.googleapis.com", is_local: false },
    ProviderEntry { id: "deepseek", name: "DeepSeek AI", endpoint: "https://api.deepseek.com/v1", is_local: false },
    ProviderEntry { id: "groq", name: "Groq LPU Engine", endpoint: "https://api.groq.com/openai/v1", is_local: false },
    ProviderEntry { id: "openrouter", name: "OpenRouter (250+ Models)", endpoint: "https://openrouter.ai/api/v1", is_local: false },
    ProviderEntry { id: "mistral", name: "Mistral & Codestral", endpoint: "https://api.mistral.ai/v1", is_local: false },
    ProviderEntry { id: "cerebras", name: "Cerebras CS-3 Wafer", endpoint: "https://api.cerebras.ai/v1", is_local: false },
    ProviderEntry { id: "copilot", name: "GitHub Copilot", endpoint: "https://api.githubcopilot.com", is_local: false },
    ProviderEntry { id: "bedrock", name: "Amazon Bedrock", endpoint: "https://bedrock-runtime.amazonaws.com", is_local: false },
    ProviderEntry { id: "xai", name: "xAI Grok", endpoint: "https://api.x.ai/v1", is_local: false },
    ProviderEntry { id: "together", name: "Together AI", endpoint: "https://api.together.xyz/v1", is_local: false },
    ProviderEntry { id: "fireworks", name: "Fireworks AI", endpoint: "https://api.fireworks.ai/inference/v1", is_local: false },
    ProviderEntry { id: "perplexity", name: "Perplexity AI", endpoint: "https://api.perplexity.ai", is_local: false },
    ProviderEntry { id: "qwen", name: "Qwen Token Plan (Alibaba)", endpoint: "https://dashscope-intl.aliyuncs.com", is_local: false },
    ProviderEntry { id: "xiaomi", name: "Xiaomi MiMo", endpoint: "https://api.mimo.xiaomi.com/v1", is_local: false },
    ProviderEntry { id: "moonshot", name: "Moonshot AI / Kimi", endpoint: "https://api.moonshot.cn/v1", is_local: false },
    ProviderEntry { id: "huggingface", name: "Hugging Face Hub", endpoint: "https://api-inference.huggingface.co", is_local: false },
    ProviderEntry { id: "opencode", name: "OpenCode Zen Gateway", endpoint: "https://opencode.ai/zen/v1", is_local: false },
    ProviderEntry { id: "kilo", name: "Kilo Gateway", endpoint: "https://api.kilo.ai/v1", is_local: false },
    ProviderEntry { id: "agnes", name: "Agnes Orchestrator", endpoint: "https://api.agnes.ai/v1", is_local: false },
    ProviderEntry { id: "ollama", name: "Ollama (Local :11434)", endpoint: "http://localhost:11434/v1", is_local: true },
    ProviderEntry { id: "lmstudio", name: "LM Studio (Local :1234)", endpoint: "http://localhost:1234/v1", is_local: true },
    ProviderEntry { id: "llamacpp", name: "llama.cpp (Local :8080)", endpoint: "http://localhost:8080/v1", is_local: true },
    ProviderEntry { id: "vllm", name: "vLLM (Local :8000)", endpoint: "http://localhost:8000/v1", is_local: true },
];

pub struct ProviderPickerWidget;

impl ProviderPickerWidget {
    pub fn render<'a>(
        query: &'a str,
        filtered_providers: &'a [&'static ProviderEntry],
    ) -> (Paragraph<'a>, List<'a>, Block<'a>) {
        let search_display = format!("> {}▏", query);
        let search_bar = Paragraph::new(Line::from(vec![
            Span::styled(" Filter: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(search_display, Style::default().fg(Color::White)),
            Span::styled(format!(" (Showing {} of {} providers)", filtered_providers.len(), ALL_PROVIDERS.len()), Style::default().fg(Color::DarkGray)),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

        let items: Vec<ListItem> = filtered_providers
            .iter()
            .map(|p| {
                let has_auth = AuthResolver::resolve_key(p.id).is_some();
                let status_badge = if p.is_local {
                    Span::styled(" [⚡ Local Zero-Config] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                } else if has_auth {
                    Span::styled(" [✓ Configured] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled(" [🔑 Key Needed] ", Style::default().fg(Color::Yellow))
                };

                let spans = vec![
                    Span::styled(format!(" {:<20} ", p.name), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    status_badge,
                    Span::styled(format!("({})", p.endpoint), Style::default().fg(Color::DarkGray)),
                ];

                ListItem::new(Line::from(spans))
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green))
            .title(" AI Providers (Type to Filter · Enter: Select/Auth · Esc: Close) ")
            .title_alignment(Alignment::Center);

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Green)
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
pub mod tests {
    use super::*;

    #[test]
    fn test_all_providers_coverage() {
        assert!(ALL_PROVIDERS.len() >= 20);
        assert!(ALL_PROVIDERS.iter().any(|p| p.id == "anthropic"));
        assert!(ALL_PROVIDERS.iter().any(|p| p.id == "openai"));
        assert!(ALL_PROVIDERS.iter().any(|p| p.id == "gemini"));
        assert!(ALL_PROVIDERS.iter().any(|p| p.id == "deepseek"));
        assert!(ALL_PROVIDERS.iter().any(|p| p.id == "ollama" && p.is_local));
    }

    #[test]
    fn test_provider_navigation() {
        let mut state = ListState::default();
        ProviderPickerWidget::handle_navigation(&mut state, 5, false);
        assert_eq!(state.selected(), Some(0));

        ProviderPickerWidget::handle_navigation(&mut state, 5, false);
        assert_eq!(state.selected(), Some(1));

        ProviderPickerWidget::handle_navigation(&mut state, 5, true);
        assert_eq!(state.selected(), Some(0));

        ProviderPickerWidget::handle_navigation(&mut state, 5, true);
        assert_eq!(state.selected(), Some(4));
    }
}
