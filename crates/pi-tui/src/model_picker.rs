use pi_providers::{AuthResolver, ModelInfo};
use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::style::ThemePalette;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModelCategoryTab {
    #[default]
    All,
    Reasoning,
    Frontier,
    Local,
    Free,
    Configured,
}

impl ModelCategoryTab {
    pub const ALL: &'static [ModelCategoryTab] = &[
        ModelCategoryTab::All,
        ModelCategoryTab::Reasoning,
        ModelCategoryTab::Frontier,
        ModelCategoryTab::Local,
        ModelCategoryTab::Free,
        ModelCategoryTab::Configured,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            ModelCategoryTab::All => "All",
            ModelCategoryTab::Reasoning => "🧠 Reasoning",
            ModelCategoryTab::Frontier => "⭐ Frontier",
            ModelCategoryTab::Local => "⚡ Local",
            ModelCategoryTab::Free => "🎁 Free/Open",
            ModelCategoryTab::Configured => "✓ Configured",
        }
    }

    pub fn matches(&self, model: &ModelInfo) -> bool {
        match self {
            ModelCategoryTab::All => true,
            ModelCategoryTab::Reasoning => model.supports_reasoning,
            ModelCategoryTab::Frontier => {
                let p = model.provider.to_lowercase();
                p.contains("anthropic")
                    || p.contains("openai")
                    || p.contains("gemini")
                    || p.contains("deepseek")
                    || p.contains("mistral")
                    || p.contains("xai")
            }
            ModelCategoryTab::Local => {
                let id = model.id.to_lowercase();
                let p = model.provider.to_lowercase();
                id.starts_with("ollama/")
                    || id.starts_with("llamacpp/")
                    || id.starts_with("lmstudio/")
                    || id.starts_with("vllm/")
                    || p == "ollama"
                    || p == "llama.cpp"
                    || p == "lm studio"
                    || p == "vllm"
            }
            ModelCategoryTab::Free => {
                let id = model.id.to_lowercase();
                id.contains("free")
                    || id.starts_with("opencode/")
                    || id.starts_with("ollama/")
                    || id.starts_with("llamacpp/")
                    || id.starts_with("lmstudio/")
                    || id.starts_with("vllm/")
            }
            ModelCategoryTab::Configured => {
                let p = model.provider.to_lowercase();
                let is_local = p == "ollama"
                    || p == "llama.cpp"
                    || p == "lm studio"
                    || p == "vllm"
                    || model.id.starts_with("ollama/")
                    || model.id.starts_with("llamacpp/")
                    || model.id.starts_with("lmstudio/")
                    || model.id.starts_with("vllm/");
                let has_auth = AuthResolver::resolve_key(&p).is_some();
                let is_free = model.id.contains("free") || model.id.starts_with("opencode/");
                is_local || has_auth || is_free
            }
        }
    }

    pub fn next(&self) -> Self {
        match self {
            ModelCategoryTab::All => ModelCategoryTab::Reasoning,
            ModelCategoryTab::Reasoning => ModelCategoryTab::Frontier,
            ModelCategoryTab::Frontier => ModelCategoryTab::Local,
            ModelCategoryTab::Local => ModelCategoryTab::Free,
            ModelCategoryTab::Free => ModelCategoryTab::Configured,
            ModelCategoryTab::Configured => ModelCategoryTab::All,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            ModelCategoryTab::All => ModelCategoryTab::Configured,
            ModelCategoryTab::Reasoning => ModelCategoryTab::All,
            ModelCategoryTab::Frontier => ModelCategoryTab::Reasoning,
            ModelCategoryTab::Local => ModelCategoryTab::Frontier,
            ModelCategoryTab::Free => ModelCategoryTab::Local,
            ModelCategoryTab::Configured => ModelCategoryTab::Free,
        }
    }
}

pub struct ModelPickerWidget;

impl ModelPickerWidget {
    /// Helper to pick an accent color per provider for visually distinctive list items
    pub fn provider_color(provider: &str, palette: &ThemePalette) -> Color {
        let p = provider.to_lowercase();
        if p.contains("anthropic") || p.contains("claude") {
            palette.magenta
        } else if p.contains("openai") || p.contains("gpt") {
            palette.green
        } else if p.contains("gemini") || p.contains("google") {
            palette.accent
        } else if p.contains("deepseek") {
            palette.cyan
        } else if p.contains("groq") || p.contains("cerebras") {
            palette.yellow
        } else if p.contains("ollama") || p.contains("llama") || p.contains("lmstudio") || p.contains("vllm") {
            Color::Rgb(100, 220, 220)
        } else {
            palette.muted
        }
    }

    /// Formats a large token count with thousands comma grouping
    pub fn format_number(n: usize) -> String {
        let s = n.to_string();
        let bytes = s.as_bytes();
        let mut res = String::with_capacity(s.len() + s.len() / 3);
        let len = bytes.len();
        for (i, &b) in bytes.iter().enumerate() {
            if i > 0 && (len - i).is_multiple_of(3) {
                res.push(',');
            }
            res.push(b as char);
        }
        res
    }

    /// Renders the complete Model Picker UI components
    pub fn render<'a>(
        query: &'a str,
        active_tab: ModelCategoryTab,
        filtered_models: &'a [ModelInfo],
        selected_index: Option<usize>,
        active_model_id: &'a str,
        total_catalog_count: usize,
        palette: &'a ThemePalette,
    ) -> (Paragraph<'a>, Paragraph<'a>, List<'a>, Paragraph<'a>, Block<'a>) {
        // 1. Search Bar Header
        let search_display = if query.is_empty() {
            "> Type to search models, providers, context...▏"
        } else {
            query
        };

        let search_style = if query.is_empty() {
            Style::default().fg(palette.muted)
        } else {
            Style::default().fg(palette.text).add_modifier(Modifier::BOLD)
        };

        let search_bar = Paragraph::new(Line::from(vec![
            Span::styled(" 🔍 Filter: ", Style::default().fg(palette.yellow).add_modifier(Modifier::BOLD)),
            Span::styled(search_display, search_style),
            Span::styled(
                format!("  ({} of {} models available)", filtered_models.len(), total_catalog_count),
                Style::default().fg(palette.muted),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(palette.border)),
        );

        // 2. Category Tabs Bar
        let mut tab_spans = vec![
            Span::styled(" [Tab] View: ", Style::default().fg(palette.muted)),
        ];

        for tab in ModelCategoryTab::ALL {
            let is_active = *tab == active_tab;
            let tab_style = if is_active {
                Style::default()
                    .fg(Color::Black)
                    .bg(palette.cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette.muted)
            };

            tab_spans.push(Span::styled(format!(" [{}] ", tab.title()), tab_style));
            tab_spans.push(Span::raw(" "));
        }

        let tabs_bar = Paragraph::new(Line::from(tab_spans))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(palette.border)),
            );

        // 3. Left List of Models
        let mut items: Vec<ListItem> = filtered_models
            .iter()
            .map(|m| {
                let is_active_model = m.id == active_model_id;
                let p_color = Self::provider_color(&m.provider, palette);

                let active_marker = if is_active_model {
                    Span::styled(" ● ", Style::default().fg(palette.green).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled("   ", Style::default().fg(palette.muted))
                };

                let ctx_str = if m.context_window >= 1_000_000 {
                    format!("{}M ctx", m.context_window / 1_000_000)
                } else {
                    format!("{}k ctx", m.context_window / 1_000)
                };

                let out_str = if m.max_output >= 1_000_000 {
                    format!("{}M out", m.max_output / 1_000_000)
                } else {
                    format!("{}k out", m.max_output / 1_000)
                };

                let inline_spec = if m.max_output > 0 {
                    format!("[{} · {}]", ctx_str, out_str)
                } else {
                    format!("[{}]", ctx_str)
                };

                let provider_tag = Span::styled(
                    format!("[{:<10}] ", m.provider),
                    Style::default().fg(p_color).add_modifier(Modifier::BOLD),
                );

                let model_id_span = Span::styled(
                    m.id.clone(),
                    Style::default().fg(palette.text).add_modifier(Modifier::BOLD),
                );

                let inline_spec_span = Span::styled(
                    format!(" {}", inline_spec),
                    Style::default().fg(palette.muted),
                );

                let mut badge_spans = vec![];
                if m.supports_reasoning {
                    badge_spans.push(Span::styled(" 🧠", Style::default().fg(palette.magenta)));
                }
                if m.supports_vision {
                    badge_spans.push(Span::styled(" 👁", Style::default().fg(palette.green)));
                }

                let is_local = m.id.starts_with("ollama/")
                    || m.id.starts_with("llamacpp/")
                    || m.id.starts_with("lmstudio/")
                    || m.id.starts_with("vllm/");
                let has_auth = AuthResolver::resolve_key(&m.provider).is_some();
                let is_free = m.id.contains("free") || m.id.starts_with("opencode/");

                let auth_badge = if is_local {
                    Span::styled(" [⚡Local]", Style::default().fg(palette.cyan))
                } else if is_free {
                    Span::styled(" [🎁Free]", Style::default().fg(palette.green))
                } else if has_auth {
                    Span::styled(" [✓Ready]", Style::default().fg(palette.green))
                } else {
                    Span::styled(" [🔑Auth]", Style::default().fg(palette.yellow))
                };

                let mut spans = vec![active_marker, provider_tag, model_id_span, inline_spec_span];
                spans.extend(badge_spans);
                spans.push(Span::raw(" "));
                spans.push(auth_badge);

                ListItem::new(Line::from(spans))
            })
            .collect();

        if items.is_empty() && !query.trim().is_empty() {
            let custom_spans = vec![
                Span::styled(" ▶ ", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                Span::styled("[Custom    ] ", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                Span::styled(query.trim(), Style::default().fg(palette.text).add_modifier(Modifier::BOLD)),
                Span::styled(" [Auto-inferred]", Style::default().fg(palette.muted)),
                Span::styled(" [⚡Custom]", Style::default().fg(palette.cyan)),
            ];
            items.push(ListItem::new(Line::from(custom_spans)));
        }

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(palette.surface)
                    .fg(palette.cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        // 4. Right Inspector & Spec Card
        let detail_card = if let Some(idx) = selected_index
            && let Some(m) = filtered_models.get(idx)
        {
            let p_color = Self::provider_color(&m.provider, palette);
            let is_active_model = m.id == active_model_id;

            let is_local = m.id.starts_with("ollama/")
                || m.id.starts_with("llamacpp/")
                || m.id.starts_with("lmstudio/")
                || m.id.starts_with("vllm/");
            let has_auth = AuthResolver::resolve_key(&m.provider).is_some();
            let is_free = m.id.contains("free") || m.id.starts_with("opencode/");

            let auth_status_line = if is_local {
                Line::from(vec![
                    Span::styled("  Auth Status:  ", Style::default().fg(palette.muted)),
                    Span::styled("⚡ Zero-Config Local Daemon (offline, private)", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                ])
            } else if is_free {
                Line::from(vec![
                    Span::styled("  Auth Status:  ", Style::default().fg(palette.muted)),
                    Span::styled("🎁 Free Gateway (no API key required)", Style::default().fg(palette.green).add_modifier(Modifier::BOLD)),
                ])
            } else if has_auth {
                Line::from(vec![
                    Span::styled("  Auth Status:  ", Style::default().fg(palette.muted)),
                    Span::styled("✓ Configured (in ~/.pi/config.json or ENV)", Style::default().fg(palette.green).add_modifier(Modifier::BOLD)),
                ])
            } else {
                Line::from(vec![
                    Span::styled("  Auth Status:  ", Style::default().fg(palette.muted)),
                    Span::styled("🔑 API Key Required (will prompt upon selection)", Style::default().fg(palette.yellow).add_modifier(Modifier::BOLD)),
                ])
            };

            let ctx_tokens_str = format!("{} tokens", Self::format_number(m.context_window));
            let max_output_str = format!("{} tokens", Self::format_number(m.max_output));

            let reasoning_str = if m.supports_reasoning {
                "🧠 Supported (Extended CoT / Deep Verification)"
            } else {
                "• Standard Generation"
            };

            let vision_str = if m.supports_vision {
                "👁 Multimodal Vision & Image Analysis"
            } else {
                "• Text / Code Only"
            };

            let active_badge = if is_active_model {
                Span::styled(" [ACTIVE SESSION MODEL] ", Style::default().fg(Color::Black).bg(palette.green).add_modifier(Modifier::BOLD))
            } else {
                Span::raw("")
            };

            let card_lines = vec![
                Line::from(vec![
                    Span::styled(format!(" [{}] ", m.provider), Style::default().fg(p_color).add_modifier(Modifier::BOLD)),
                    Span::styled(&m.id, Style::default().fg(palette.text).add_modifier(Modifier::BOLD)),
                    active_badge,
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Description:", Style::default().fg(palette.yellow).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled(format!("  {}", m.description), Style::default().fg(palette.text)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Technical Specifications:", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("  Context Window: ", Style::default().fg(palette.muted)),
                    Span::styled(ctx_tokens_str, Style::default().fg(palette.text)),
                ]),
                Line::from(vec![
                    Span::styled("  Max Output:     ", Style::default().fg(palette.muted)),
                    Span::styled(max_output_str, Style::default().fg(palette.text)),
                ]),
                Line::from(vec![
                    Span::styled("  Reasoning Mode: ", Style::default().fg(palette.muted)),
                    Span::styled(reasoning_str, Style::default().fg(if m.supports_reasoning { palette.magenta } else { palette.muted })),
                ]),
                Line::from(vec![
                    Span::styled("  Multimodal:     ", Style::default().fg(palette.muted)),
                    Span::styled(vision_str, Style::default().fg(if m.supports_vision { palette.green } else { palette.muted })),
                ]),
                auth_status_line,
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Quick Actions:", Style::default().fg(palette.yellow).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("  ↵ Enter: ", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                    Span::styled("Switch active model  |  ", Style::default().fg(palette.muted)),
                    Span::styled("⇥ Tab: ", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                    Span::styled("Filter tabs", Style::default().fg(palette.muted)),
                ]),
                Line::from(vec![
                    Span::styled("  Ctrl+R: ", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                    Span::styled("Live refresh  |  ", Style::default().fg(palette.muted)),
                    Span::styled("Esc: ", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                    Span::styled("Close modal", Style::default().fg(palette.muted)),
                ]),
            ];

            Paragraph::new(card_lines)
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(palette.border))
                        .title(" Model Inspector & Specs ")
                        .title_alignment(Alignment::Center),
                )
        } else if !query.trim().is_empty() {
            let (cw, max_out) = pi_providers::ModelCatalogLoader::infer_model_limits(query.trim(), "");
            let (prov, _) = query.trim().split_once('/').unwrap_or(("custom", query.trim()));
            let custom_lines = vec![
                Line::from(vec![
                    Span::styled(format!(" [{}] ", prov), Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(query.trim(), Style::default().fg(palette.text).add_modifier(Modifier::BOLD)),
                    Span::styled(" [CUSTOM MODEL INPUT] ", Style::default().fg(Color::Black).bg(palette.cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Custom Model Direct Activation:", Style::default().fg(palette.yellow).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled(format!("  Press Enter to activate '{}' immediately.", query.trim()), Style::default().fg(palette.text)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Inferred Specifications:", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("  Context Window: ", Style::default().fg(palette.muted)),
                    Span::styled(format!("{} tokens", Self::format_number(cw)), Style::default().fg(palette.text)),
                ]),
                Line::from(vec![
                    Span::styled("  Max Output:     ", Style::default().fg(palette.muted)),
                    Span::styled(format!("{} tokens", Self::format_number(max_out)), Style::default().fg(palette.text)),
                ]),
                Line::from(vec![
                    Span::styled("  Provider:       ", Style::default().fg(palette.muted)),
                    Span::styled(prov, Style::default().fg(palette.accent)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  ↵ Enter: ", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                    Span::styled("Connect and activate this custom model", Style::default().fg(palette.text)),
                ]),
                Line::from(vec![
                    Span::styled("  Esc: ", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                    Span::styled("Close modal", Style::default().fg(palette.muted)),
                ]),
            ];

            Paragraph::new(custom_lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(palette.cyan))
                    .title(" Custom Model Activation ")
                    .title_alignment(Alignment::Center),
            )
        } else {
            let empty_text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  No models matching current filter.", Style::default().fg(palette.muted)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  • Press ", Style::default().fg(palette.muted)),
                    Span::styled("Tab", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(" to switch category view (All, Reasoning, Local, Free)", Style::default().fg(palette.muted)),
                ]),
                Line::from(vec![
                    Span::styled("  • Press ", Style::default().fg(palette.muted)),
                    Span::styled("Ctrl+C", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(" to clear search query", Style::default().fg(palette.muted)),
                ]),
                Line::from(vec![
                    Span::styled("  • Press ", Style::default().fg(palette.muted)),
                    Span::styled("Ctrl+R", Style::default().fg(palette.cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(" to scan for online & local daemon updates", Style::default().fg(palette.muted)),
                ]),
            ];

            Paragraph::new(empty_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(palette.border))
                    .title(" Model Inspector ")
                    .title_alignment(Alignment::Center),
            )
        };

        // 5. Outer Modal Block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette.cyan))
            .title(" 🤖 Model Selection Cockpit [/model · Ctrl+L] ")
            .title_alignment(Alignment::Center);

        (search_bar, tabs_bar, list, detail_card, block)
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

    pub fn handle_page_navigation(state: &mut ListState, total_items: usize, up: bool, page_size: usize) {
        if total_items == 0 {
            state.select(None);
            return;
        }

        let current = state.selected().unwrap_or(0);
        let next = if up {
            current.saturating_sub(page_size)
        } else {
            let target = current + page_size;
            if target < total_items { target } else { total_items - 1 }
        };

        state.select(Some(next));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_tabs_matching() {
        let claude = ModelInfo::new(
            "anthropic/claude-3-7-sonnet-latest",
            "Anthropic",
            200_000,
            64_000,
            true,
            true,
            "Intelligent hybrid model",
        );

        let ollama = ModelInfo::new(
            "ollama/llama3.2",
            "Ollama",
            128_000,
            8_192,
            false,
            false,
            "Local model",
        );

        assert!(ModelCategoryTab::All.matches(&claude));
        assert!(ModelCategoryTab::Reasoning.matches(&claude));
        assert!(ModelCategoryTab::Frontier.matches(&claude));
        assert!(!ModelCategoryTab::Local.matches(&claude));

        assert!(ModelCategoryTab::All.matches(&ollama));
        assert!(!ModelCategoryTab::Reasoning.matches(&ollama));
        assert!(ModelCategoryTab::Local.matches(&ollama));
        assert!(ModelCategoryTab::Free.matches(&ollama));
    }

    #[test]
    fn test_category_tabs_cycling() {
        let mut tab = ModelCategoryTab::All;
        tab = tab.next();
        assert_eq!(tab, ModelCategoryTab::Reasoning);
        tab = tab.next();
        assert_eq!(tab, ModelCategoryTab::Frontier);
        tab = tab.prev();
        assert_eq!(tab, ModelCategoryTab::Reasoning);
        tab = tab.prev();
        assert_eq!(tab, ModelCategoryTab::All);
        tab = tab.prev();
        assert_eq!(tab, ModelCategoryTab::Configured);
    }

    #[test]
    fn test_page_navigation() {
        let mut state = ListState::default();
        state.select(Some(0));

        ModelPickerWidget::handle_page_navigation(&mut state, 20, false, 5);
        assert_eq!(state.selected(), Some(5));

        ModelPickerWidget::handle_page_navigation(&mut state, 20, false, 20);
        assert_eq!(state.selected(), Some(19));

        ModelPickerWidget::handle_page_navigation(&mut state, 20, true, 5);
        assert_eq!(state.selected(), Some(14));

        ModelPickerWidget::handle_page_navigation(&mut state, 20, true, 20);
        assert_eq!(state.selected(), Some(0));
    }
}
